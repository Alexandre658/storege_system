use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{StorageError, StorageResult};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ObjectMetadata {
    pub name: String,
    pub bucket: String,
    pub content_type: String,
    pub size: u64,
    pub md5_hash: Option<String>,
    pub crc32c: Option<String>,
    pub generation: i64,
    pub metageneration: i64,
    pub custom_metadata: HashMap<String, String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectRecord {
    pub id: Uuid,
    pub bucket_id: Uuid,
    pub bucket_name: String,
    pub object_path: String,
    pub content_type: String,
    pub size: i64,
    pub md5_hash: Option<String>,
    pub generation: i64,
    pub metageneration: i64,
    pub custom_metadata: String,
    pub storage_path: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, sqlx::FromRow)]
pub(crate) struct ObjectRecordRow {
    pub id: String,
    pub bucket_id: String,
    pub bucket_name: String,
    pub object_path: String,
    pub content_type: String,
    pub size: i64,
    pub md5_hash: Option<String>,
    pub generation: i64,
    pub metageneration: i64,
    pub custom_metadata: String,
    pub storage_path: String,
    pub created_at: String,
    pub updated_at: String,
}

impl ObjectRecordRow {
pub(crate) fn into_record(self) -> StorageResult<ObjectRecord> {
        Ok(ObjectRecord {
            id: Uuid::parse_str(&self.id)
                .map_err(|e| StorageError::Internal(e.to_string()))?,
            bucket_id: Uuid::parse_str(&self.bucket_id)
                .map_err(|e| StorageError::Internal(e.to_string()))?,
            bucket_name: self.bucket_name,
            object_path: self.object_path,
            content_type: self.content_type,
            size: self.size,
            md5_hash: self.md5_hash,
            generation: self.generation,
            metageneration: self.metageneration,
            custom_metadata: self.custom_metadata,
            storage_path: self.storage_path,
            created_at: chrono::DateTime::parse_from_rfc3339(&self.created_at)
                .map(|dt| dt.with_timezone(&Utc))
                .map_err(|e| StorageError::Internal(e.to_string()))?,
            updated_at: chrono::DateTime::parse_from_rfc3339(&self.updated_at)
                .map(|dt| dt.with_timezone(&Utc))
                .map_err(|e| StorageError::Internal(e.to_string()))?,
        })
    }
}

impl ObjectRecord {
    pub fn to_metadata(&self) -> ObjectMetadata {
        let custom_metadata: HashMap<String, String> =
            serde_json::from_str(&self.custom_metadata).unwrap_or_default();

        ObjectMetadata {
            name: self.object_path.clone(),
            bucket: self.bucket_name.clone(),
            content_type: self.content_type.clone(),
            size: self.size as u64,
            md5_hash: self.md5_hash.clone(),
            crc32c: None,
            generation: self.generation,
            metageneration: self.metageneration,
            custom_metadata,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}
