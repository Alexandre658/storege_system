use axum::{
    extract::{Path, State},
    http::StatusCode,
    Extension, Json,
};
use storage_auth::Claims;

use crate::dto::{
    BackupInfoResponse, BackupListResponse, CreateBackupRequest, RestoreBackupResponse,
};
use crate::error::{ApiError, ApiResult};
use crate::middleware::AuthUser;
use crate::state::AppState;

fn require_admin(claims: &Option<Claims>) -> ApiResult<()> {
    match claims {
        Some(c) if c.is_admin() => Ok(()),
        Some(_) => Err(ApiError::forbidden("apenas admins podem gerenciar backups")),
        None => Err(ApiError::unauthorized("token Firebase necessário")),
    }
}

pub async fn create_backup(
    State(state): State<AppState>,
    Extension(AuthUser(claims)): Extension<AuthUser>,
    Json(req): Json<CreateBackupRequest>,
) -> ApiResult<(StatusCode, Json<BackupInfoResponse>)> {
    require_admin(&claims)?;

    let backup = state
        .backup
        .create_backup(req.label)
        .await
        .map_err(ApiError::from)?;

    Ok((StatusCode::CREATED, Json(BackupInfoResponse::from(backup))))
}

pub async fn list_backups(
    State(state): State<AppState>,
    Extension(AuthUser(claims)): Extension<AuthUser>,
) -> ApiResult<Json<BackupListResponse>> {
    require_admin(&claims)?;

    let items = state.backup.list_backups().await.map_err(ApiError::from)?;
    Ok(Json(BackupListResponse {
        items: items.into_iter().map(BackupInfoResponse::from).collect(),
    }))
}

pub async fn get_backup(
    State(state): State<AppState>,
    Extension(AuthUser(claims)): Extension<AuthUser>,
    Path(id): Path<String>,
) -> ApiResult<Json<BackupInfoResponse>> {
    require_admin(&claims)?;

    let backup = state.backup.get_backup(&id).await.map_err(ApiError::from)?;
    Ok(Json(BackupInfoResponse::from(backup)))
}

pub async fn delete_backup(
    State(state): State<AppState>,
    Extension(AuthUser(claims)): Extension<AuthUser>,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    require_admin(&claims)?;

    state.backup.delete_backup(&id).await.map_err(ApiError::from)?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn restore_backup(
    State(state): State<AppState>,
    Extension(AuthUser(claims)): Extension<AuthUser>,
    Path(id): Path<String>,
) -> ApiResult<Json<RestoreBackupResponse>> {
    require_admin(&claims)?;

    let safety = state
        .backup
        .restore_backup(&id)
        .await
        .map_err(ApiError::from)?;

    Ok(Json(RestoreBackupResponse {
        message: "Restauração concluída. Reinicie o servidor para aplicar.".to_string(),
        restored_from: id,
        safety_backup: BackupInfoResponse::from(safety),
    }))
}
