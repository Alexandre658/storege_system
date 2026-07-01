use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[allow(non_snake_case)]
pub struct BucketResponse {
    pub name: String,
    pub location: String,
    pub timeCreated: String,
    pub updated: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct BucketListResponse {
    pub items: Vec<BucketResponse>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[allow(non_snake_case)]
pub struct ObjectResponse {
    pub name: String,
    pub bucket: String,
    pub contentType: String,
    pub size: String,
    pub md5Hash: Option<String>,
    pub generation: String,
    pub metageneration: String,
    pub timeCreated: String,
    pub updated: String,
    #[serde(rename = "metadata", skip_serializing_if = "HashMap::is_empty")]
    pub custom_metadata: HashMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mediaLink: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selfLink: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ObjectListResponse {
    pub items: Vec<ObjectResponse>,
    #[serde(rename = "nextPageToken", skip_serializing_if = "Option::is_none")]
    pub next_page_token: Option<String>,
    pub prefixes: Vec<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateBucketRequest {
    pub name: String,
    #[serde(default = "default_location")]
    pub location: String,
}

fn default_location() -> String {
    "us-central1".to_string()
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateMetadataRequest {
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct UploadSessionResponse {
    pub upload_id: String,
    pub upload_url: String,
    pub expires_at: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SignedUrlResponse {
    pub signed_url: String,
    pub expires_at: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct SignedUrlRequest {
    pub bucket: String,
    pub object_path: String,
    #[serde(default = "default_method")]
    pub method: String,
    #[serde(default = "default_expires")]
    pub expires_in_secs: u64,
    pub content_type: Option<String>,
}

fn default_method() -> String {
    "GET".to_string()
}

fn default_expires() -> u64 {
    3600
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateBackupRequest {
    pub label: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct BackupInfoResponse {
    pub id: String,
    pub label: Option<String>,
    pub filename: String,
    pub created_at: String,
    pub size_bytes: u64,
    pub checksum_sha256: String,
    pub bucket_count: u64,
    pub object_count: u64,
}

impl From<storage_core::BackupInfo> for BackupInfoResponse {
    fn from(b: storage_core::BackupInfo) -> Self {
        Self {
            id: b.id,
            label: b.label,
            filename: b.filename,
            created_at: b.created_at.to_rfc3339(),
            size_bytes: b.size_bytes,
            checksum_sha256: b.checksum_sha256,
            bucket_count: b.bucket_count,
            object_count: b.object_count,
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct BackupListResponse {
    pub items: Vec<BackupInfoResponse>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct RestoreBackupResponse {
    pub message: String,
    pub restored_from: String,
    pub safety_backup: BackupInfoResponse,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct HealthResponse {
    pub status: String,
    pub service: String,
    pub version: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ErrorResponse {
    pub error: ErrorDetail,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ErrorDetail {
    pub code: u16,
    pub message: String,
    pub status: String,
}

#[derive(Debug, Deserialize, ToSchema, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ListObjectsQuery {
    pub prefix: Option<String>,
    pub delimiter: Option<String>,
    #[serde(rename = "maxResults")]
    pub max_results: Option<u32>,
    #[serde(rename = "pageToken")]
    pub page_token: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ObjectQuery {
    pub name: String,
    #[serde(rename = "uploadType")]
    pub upload_type: Option<String>,
    #[serde(rename = "upload_id")]
    pub upload_id: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct DownloadQuery {
    pub alt: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UploadChunkQuery {
    #[serde(rename = "upload_id")]
    pub upload_id: Option<String>,
}

impl ObjectResponse {
    pub fn from_metadata(
        metadata: storage_core::ObjectMetadata,
        base_url: &str,
    ) -> Self {
        let encoded_name = urlencoding::encode(&metadata.name);
        let encoded_bucket = urlencoding::encode(&metadata.bucket);

        let media_link = format!(
            "{base_url}/v0/b/{encoded_bucket}/o/{encoded_name}?alt=media"
        );
        let self_link = format!(
            "{base_url}/v0/b/{encoded_bucket}/o/{encoded_name}"
        );

        Self {
            name: metadata.name,
            bucket: metadata.bucket,
            contentType: metadata.content_type,
            size: metadata.size.to_string(),
            md5Hash: metadata.md5_hash,
            generation: metadata.generation.to_string(),
            metageneration: metadata.metageneration.to_string(),
            timeCreated: metadata.created_at.to_rfc3339(),
            updated: metadata.updated_at.to_rfc3339(),
            custom_metadata: metadata.custom_metadata,
            mediaLink: Some(media_link),
            selfLink: Some(self_link),
        }
    }
}

impl BucketResponse {
    pub fn from_bucket(bucket: storage_core::Bucket) -> Self {
        Self {
            name: bucket.name,
            location: bucket.location,
            timeCreated: bucket.created_at.to_rfc3339(),
            updated: bucket.updated_at.to_rfc3339(),
        }
    }
}

pub fn format_datetime(dt: DateTime<Utc>) -> String {
    dt.to_rfc3339()
}
