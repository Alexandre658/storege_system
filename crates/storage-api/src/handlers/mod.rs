pub mod backup;

use std::collections::HashMap;

use axum::{
    body::Bytes,
    extract::{Path, Query, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Extension, Json,
};
use storage_auth::rules::{AccessRequest, Operation};
use storage_auth::Claims;
use uuid::Uuid;

use crate::dto::{
    BucketListResponse, BucketResponse, CreateBucketRequest, DownloadQuery,
    HealthResponse, ListObjectsQuery, ObjectListResponse, ObjectQuery, ObjectResponse,
    SignedUrlRequest, SignedUrlResponse,
    UpdateMetadataRequest, UploadChunkQuery, UploadSessionResponse,
};
use crate::error::{ApiError, ApiResult};
use crate::middleware::AuthUser;
use crate::state::AppState;

pub use backup::{
    create_backup, delete_backup, get_backup, list_backups, restore_backup,
};

fn check_access(
    state: &AppState,
    operation: Operation,
    bucket: &str,
    object_path: &str,
    claims: &Option<Claims>,
    metadata: HashMap<String, String>,
) -> ApiResult<()> {
    let request = AccessRequest {
        operation,
        bucket: bucket.to_string(),
        object_path: object_path.to_string(),
        claims: claims.clone(),
        custom_metadata: metadata,
    };

    if !state.rules.evaluate(&request) {
        return Err(ApiError::forbidden("acesso negado pelas regras de segurança"));
    }

    Ok(())
}

pub async fn health() -> impl IntoResponse {
    Json(HealthResponse {
        status: "ok".to_string(),
        service: "moveme-storage".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

pub async fn create_bucket(
    State(state): State<AppState>,
    Extension(AuthUser(claims)): Extension<AuthUser>,
    Json(req): Json<CreateBucketRequest>,
) -> ApiResult<(StatusCode, Json<BucketResponse>)> {
    if claims.as_ref().map(|c| !c.is_admin()).unwrap_or(true) {
        return Err(ApiError::forbidden("apenas admins podem criar buckets"));
    }

    let bucket = state
        .store
        .create_bucket(&req.name, &req.location)
        .await?;

    Ok((StatusCode::OK, Json(BucketResponse::from_bucket(bucket))))
}

pub async fn list_buckets(
    State(state): State<AppState>,
    Extension(AuthUser(claims)): Extension<AuthUser>,
) -> ApiResult<Json<BucketListResponse>> {
    if claims.is_none() {
        return Err(ApiError::unauthorized("token Firebase necessário"));
    }

    let buckets = state.store.list_buckets().await?;
    let items = buckets.into_iter().map(BucketResponse::from_bucket).collect();

    Ok(Json(BucketListResponse { items }))
}

pub async fn get_bucket(
    State(state): State<AppState>,
    Path(bucket): Path<String>,
) -> ApiResult<Json<BucketResponse>> {
    let bucket = state.store.get_bucket(&bucket).await?;
    Ok(Json(BucketResponse::from_bucket(bucket)))
}

pub async fn delete_bucket(
    State(state): State<AppState>,
    Extension(AuthUser(claims)): Extension<AuthUser>,
    Path(bucket): Path<String>,
) -> ApiResult<StatusCode> {
    if claims.as_ref().map(|c| !c.is_admin()).unwrap_or(true) {
        return Err(ApiError::forbidden("apenas admins podem deletar buckets"));
    }

    state.store.delete_bucket(&bucket).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn upload_object(
    State(state): State<AppState>,
    Extension(AuthUser(claims)): Extension<AuthUser>,
    Path(bucket): Path<String>,
    Query(query): Query<ObjectQuery>,
    headers: HeaderMap,
    body: Bytes,
) -> ApiResult<Json<ObjectResponse>> {
    let object_path = storage_auth::normalize_object_path(&query.name, &claims);
    check_access(&state, Operation::Write, &bucket, &object_path, &claims, HashMap::new())?;

    if let Some(max) = state.max_upload_size {
        if body.len() > max {
            return Err(ApiError::bad_request(format!(
                "arquivo excede o tamanho máximo de {max} bytes"
            )));
        }
    }

    let content_type = headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok());

    let custom_metadata = extract_custom_metadata(&headers);

    let metadata = state
        .store
        .put_object(&bucket, &object_path, &body, content_type, custom_metadata)
        .await?;

    Ok(Json(ObjectResponse::from_metadata(metadata, &state.base_url)))
}

pub async fn download_object(
    State(state): State<AppState>,
    Extension(AuthUser(claims)): Extension<AuthUser>,
    Path((bucket, object_path)): Path<(String, String)>,
    Query(query): Query<DownloadQuery>,
) -> ApiResult<Response> {
    let decoded_path = urlencoding::decode(&object_path)
        .map(|s| s.into_owned())
        .unwrap_or(object_path);

    let obj = state.store.get_object(&bucket, &decoded_path).await?;

    check_access(
        &state,
        Operation::Read,
        &bucket,
        &decoded_path,
        &claims,
        obj.metadata.custom_metadata.clone(),
    )?;

    if query.alt.as_deref() == Some("media") {
        let mut response = Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, &obj.metadata.content_type)
            .header(header::CONTENT_LENGTH, obj.metadata.size.to_string());

        if let Some(md5) = &obj.metadata.md5_hash {
            response = response.header("x-goog-hash", format!("md5={md5}"));
        }

        return Ok(response.body(axum::body::Body::from(obj.data)).unwrap());
    }

    Ok(Json(ObjectResponse::from_metadata(
        obj.metadata,
        &state.base_url,
    ))
    .into_response())
}

pub async fn delete_object(
    State(state): State<AppState>,
    Extension(AuthUser(claims)): Extension<AuthUser>,
    Path((bucket, object_path)): Path<(String, String)>,
) -> ApiResult<StatusCode> {
    let decoded_path = urlencoding::decode(&object_path)
        .map(|s| s.into_owned())
        .unwrap_or(object_path);

    let metadata = state
        .store
        .get_object_metadata(&bucket, &decoded_path)
        .await?;

    check_access(
        &state,
        Operation::Delete,
        &bucket,
        &decoded_path,
        &claims,
        metadata.custom_metadata,
    )?;

    state.store.delete_object(&bucket, &decoded_path).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn list_objects(
    State(state): State<AppState>,
    Extension(AuthUser(claims)): Extension<AuthUser>,
    Path(bucket): Path<String>,
    Query(query): Query<ListObjectsQuery>,
) -> ApiResult<Json<ObjectListResponse>> {
    if claims.is_none() {
        return Err(ApiError::unauthorized("token Firebase necessário para listar objetos"));
    }

    let (objects, next_token) = state
        .store
        .list_objects(
            &bucket,
            query.prefix.as_deref(),
            query.delimiter.as_deref(),
            query.max_results,
            query.page_token.as_deref(),
        )
        .await?;

    let items = objects
        .into_iter()
        .map(|m| ObjectResponse::from_metadata(m, &state.base_url))
        .collect();

    Ok(Json(ObjectListResponse {
        items,
        next_page_token: next_token,
        prefixes: vec![],
    }))
}

pub async fn update_object_metadata(
    State(state): State<AppState>,
    Extension(AuthUser(claims)): Extension<AuthUser>,
    Path((bucket, object_path)): Path<(String, String)>,
    Json(req): Json<UpdateMetadataRequest>,
) -> ApiResult<Json<ObjectResponse>> {
    let decoded_path = urlencoding::decode(&object_path)
        .map(|s| s.into_owned())
        .unwrap_or(object_path);

    check_access(&state, Operation::Write, &bucket, &decoded_path, &claims, req.metadata.clone())?;

    let metadata = state
        .store
        .update_metadata(&bucket, &decoded_path, req.metadata)
        .await?;

    Ok(Json(ObjectResponse::from_metadata(metadata, &state.base_url)))
}

pub async fn copy_object(
    State(state): State<AppState>,
    Extension(AuthUser(claims)): Extension<AuthUser>,
    Path((bucket, object_path)): Path<(String, String)>,
    headers: HeaderMap,
) -> ApiResult<Json<ObjectResponse>> {
    let decoded_path = urlencoding::decode(&object_path)
        .map(|s| s.into_owned())
        .unwrap_or(object_path);

    let dest_bucket = headers
        .get("x-goog-copy-source-bucket")
        .or_else(|| headers.get("x-dest-bucket"))
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| ApiError::bad_request("header x-dest-bucket é obrigatório"))?;

    let dest_path = headers
        .get("x-goog-copy-source-object")
        .or_else(|| headers.get("x-dest-object"))
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| ApiError::bad_request("header x-dest-object é obrigatório"))?;

    check_access(&state, Operation::Read, &bucket, &decoded_path, &claims, HashMap::new())?;
    check_access(&state, Operation::Write, dest_bucket, dest_path, &claims, HashMap::new())?;

    let metadata = state
        .store
        .copy_object(&bucket, &decoded_path, dest_bucket, dest_path)
        .await?;

    Ok(Json(ObjectResponse::from_metadata(metadata, &state.base_url)))
}

pub async fn initiate_resumable_upload(
    State(state): State<AppState>,
    Extension(AuthUser(claims)): Extension<AuthUser>,
    Path(bucket): Path<String>,
    Query(query): Query<ObjectQuery>,
    headers: HeaderMap,
) -> ApiResult<(StatusCode, HeaderMap, Json<UploadSessionResponse>)> {
    let object_path = storage_auth::normalize_object_path(&query.name, &claims);
    check_access(&state, Operation::Write, &bucket, &object_path, &claims, HashMap::new())?;

    let content_type = headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok());

    let total_size = headers
        .get("x-upload-content-length")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse().ok());

    let custom_metadata = extract_custom_metadata(&headers);

    let session = state
        .store
        .create_upload_session(
            &bucket,
            &object_path,
            content_type,
            total_size,
            custom_metadata,
            &state.base_url,
            24,
        )
        .await?;

    let mut response_headers = HeaderMap::new();
    response_headers.insert(
        "location",
        session.upload_url.parse().unwrap(),
    );
    response_headers.insert(
        "x-goog-upload-url",
        session.upload_url.parse().unwrap(),
    );

    Ok((
        StatusCode::OK,
        response_headers,
        Json(UploadSessionResponse {
            upload_id: session.session_id.to_string(),
            upload_url: session.upload_url,
            expires_at: session.expires_at.to_rfc3339(),
        }),
    ))
}

pub async fn upload_chunk(
    State(state): State<AppState>,
    Path(_bucket): Path<String>,
    Query(query): Query<UploadChunkQuery>,
    headers: HeaderMap,
    body: Bytes,
) -> ApiResult<Response> {
    let upload_id = query
        .upload_id
        .ok_or_else(|| ApiError::bad_request("upload_id é obrigatório"))?;

    let session_id = Uuid::parse_str(&upload_id)
        .map_err(|_| ApiError::bad_request("upload_id inválido"))?;

    let offset = headers
        .get("content-range")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("bytes "))
        .and_then(|v| v.split('-').next())
        .and_then(|v| v.parse().ok());

    let (_bytes_received, is_complete) = state
        .store
        .append_upload_chunk(session_id, &body, offset)
        .await?;

    if is_complete || headers.get("x-goog-upload-command").map(|v| v.to_str().ok()) == Some(Some("finalize")) {
        let metadata = state.store.finalize_upload(session_id).await?;
        return Ok(Json(ObjectResponse::from_metadata(metadata, &state.base_url)).into_response());
    }

    let session = state.store.get_upload_session(session_id).await?;

    let mut response = Response::builder().status(StatusCode::OK);
    if let Some(total) = session.total_size {
        response = response.header(
            "range",
            format!("bytes 0-{}/{}", session.bytes_received - 1, total),
        );
    }

    Ok(response.body(axum::body::Body::empty()).unwrap())
}

pub async fn generate_signed_url(
    State(state): State<AppState>,
    Extension(AuthUser(claims)): Extension<AuthUser>,
    Json(req): Json<SignedUrlRequest>,
) -> ApiResult<Json<SignedUrlResponse>> {
    if claims.is_none() {
        return Err(ApiError::unauthorized("token Firebase necessário"));
    }

    let signed_url = state.signed_url.generate(
        &state.base_url,
        &req.bucket,
        &req.object_path,
        &req.method,
        req.expires_in_secs,
        req.content_type.as_deref(),
    );

    let expires_at = (chrono::Utc::now()
        + chrono::Duration::seconds(req.expires_in_secs as i64))
        .to_rfc3339();

    Ok(Json(SignedUrlResponse {
        signed_url,
        expires_at,
    }))
}

fn extract_custom_metadata(headers: &HeaderMap) -> HashMap<String, String> {
    headers
        .iter()
        .filter_map(|(name, value)| {
            let key = name.as_str();
            if let Some(meta_key) = key.strip_prefix("x-goog-meta-") {
                value.to_str().ok().map(|v| (meta_key.to_string(), v.to_string()))
            } else {
                None
            }
        })
        .collect()
}
