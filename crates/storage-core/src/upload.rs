use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{StorageError, StorageResult};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadSession {
    pub id: Uuid,
    pub bucket_name: String,
    pub object_path: String,
    pub content_type: String,
    pub total_size: Option<i64>,
    pub bytes_received: i64,
    pub storage_key: String,
    pub status: String,
    pub custom_metadata: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, sqlx::FromRow)]
pub(crate) struct UploadSessionRow {
    pub id: String,
    pub bucket_name: String,
    pub object_path: String,
    pub content_type: String,
    pub total_size: Option<i64>,
    pub bytes_received: i64,
    pub storage_key: String,
    pub status: String,
    pub custom_metadata: String,
    pub created_at: String,
    pub expires_at: String,
}

impl UploadSessionRow {
    pub fn into_session(self) -> StorageResult<UploadSession> {
        Ok(UploadSession {
            id: Uuid::parse_str(&self.id)
                .map_err(|e| StorageError::Internal(e.to_string()))?,
            bucket_name: self.bucket_name,
            object_path: self.object_path,
            content_type: self.content_type,
            total_size: self.total_size,
            bytes_received: self.bytes_received,
            storage_key: self.storage_key,
            status: self.status,
            custom_metadata: self.custom_metadata,
            created_at: DateTime::parse_from_rfc3339(&self.created_at)
                .map(|dt| dt.with_timezone(&Utc))
                .map_err(|e| StorageError::Internal(e.to_string()))?,
            expires_at: DateTime::parse_from_rfc3339(&self.expires_at)
                .map(|dt| dt.with_timezone(&Utc))
                .map_err(|e| StorageError::Internal(e.to_string()))?,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResumableUpload {
    pub session_id: Uuid,
    pub upload_url: String,
    pub expires_at: DateTime<Utc>,
}

impl UploadSession {
    pub fn new(
        bucket_name: impl Into<String>,
        object_path: impl Into<String>,
        content_type: impl Into<String>,
        total_size: Option<u64>,
        custom_metadata: String,
        storage_key: impl Into<String>,
        ttl_hours: i64,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            bucket_name: bucket_name.into(),
            object_path: object_path.into(),
            content_type: content_type.into(),
            total_size: total_size.map(|s| s as i64),
            bytes_received: 0,
            storage_key: storage_key.into(),
            status: "active".to_string(),
            custom_metadata,
            created_at: now,
            expires_at: now + Duration::hours(ttl_hours),
        }
    }

    pub fn is_expired(&self) -> bool {
        Utc::now() > self.expires_at
    }

    pub fn is_complete(&self) -> bool {
        if let Some(total) = self.total_size {
            self.bytes_received >= total
        } else {
            false
        }
    }
}
