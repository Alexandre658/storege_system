use std::collections::HashMap;
use std::sync::Arc;

use chrono::Utc;
use mime_guess::from_path;
use md5::{Digest, Md5};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};
use sqlx::Row;
use uuid::Uuid;

use crate::backend::StorageBackend;
use crate::bucket::Bucket;
use crate::error::{StorageError, StorageResult};
use crate::metadata::{ObjectMetadata, ObjectRecord, ObjectRecordRow};
use crate::object::StoredObject;
use crate::upload::{ResumableUpload, UploadSession, UploadSessionRow};

#[derive(Debug, Clone)]
pub struct StoreConfig {
    pub auto_create_buckets: bool,
    pub default_bucket_location: String,
}

impl Default for StoreConfig {
    fn default() -> Self {
        Self {
            auto_create_buckets: true,
            default_bucket_location: "us-central1".to_string(),
        }
    }
}

pub struct StorageStore {
    pool: SqlitePool,
    backend: Arc<dyn StorageBackend>,
    config: StoreConfig,
}

impl StorageStore {
    pub async fn new(
        database_url: &str,
        backend: Arc<dyn StorageBackend>,
        config: StoreConfig,
    ) -> StorageResult<Self> {
        let options: SqliteConnectOptions = database_url.parse()?;
        let pool = SqlitePoolOptions::new()
            .max_connections(10)
            .connect_with(options.create_if_missing(true))
            .await?;

        let store = Self {
            pool,
            backend,
            config,
        };
        store.migrate().await?;
        Ok(store)
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    async fn migrate(&self) -> StorageResult<()> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS buckets (
                id TEXT PRIMARY KEY NOT NULL,
                name TEXT NOT NULL UNIQUE,
                location TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS objects (
                id TEXT PRIMARY KEY NOT NULL,
                bucket_id TEXT NOT NULL,
                bucket_name TEXT NOT NULL,
                object_path TEXT NOT NULL,
                content_type TEXT NOT NULL,
                size INTEGER NOT NULL,
                md5_hash TEXT,
                generation INTEGER NOT NULL DEFAULT 1,
                metageneration INTEGER NOT NULL DEFAULT 1,
                custom_metadata TEXT NOT NULL DEFAULT '{}',
                storage_path TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                UNIQUE(bucket_name, object_path)
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS upload_sessions (
                id TEXT PRIMARY KEY NOT NULL,
                bucket_name TEXT NOT NULL,
                object_path TEXT NOT NULL,
                content_type TEXT NOT NULL,
                total_size INTEGER,
                bytes_received INTEGER NOT NULL DEFAULT 0,
                storage_key TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'active',
                custom_metadata TEXT NOT NULL DEFAULT '{}',
                created_at TEXT NOT NULL,
                expires_at TEXT NOT NULL
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_objects_bucket_prefix ON objects(bucket_name, object_path)",
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn create_bucket(&self, name: &str, location: &str) -> StorageResult<Bucket> {
        let bucket = Bucket::new(name, location);

        let result = sqlx::query(
            r#"
            INSERT INTO buckets (id, name, location, created_at, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5)
            "#,
        )
        .bind(bucket.id.to_string())
        .bind(&bucket.name)
        .bind(&bucket.location)
        .bind(bucket.created_at.to_rfc3339())
        .bind(bucket.updated_at.to_rfc3339())
        .execute(&self.pool)
        .await;

        match result {
            Ok(_) => Ok(bucket),
            Err(sqlx::Error::Database(db_err)) if db_err.is_unique_violation() => {
                Err(StorageError::AlreadyExists(format!("bucket '{name}'")))
            }
            Err(e) => Err(e.into()),
        }
    }

    pub async fn get_bucket(&self, name: &str) -> StorageResult<Bucket> {
        let row = sqlx::query(
            r#"
            SELECT id, name, location, created_at, updated_at
            FROM buckets WHERE name = ?1
            "#,
        )
        .bind(name)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| StorageError::BucketNotFound(name.to_string()))?;

        Ok(Bucket {
            id: Uuid::parse_str(row.get("id")).map_err(|e: uuid::Error| StorageError::Internal(e.to_string()))?,
            name: row.get("name"),
            location: row.get("location"),
            created_at: chrono::DateTime::parse_from_rfc3339(&row.get::<String, _>("created_at"))
                .map(|dt| dt.with_timezone(&Utc))
                .map_err(|e: chrono::ParseError| StorageError::Internal(e.to_string()))?,
            updated_at: chrono::DateTime::parse_from_rfc3339(&row.get::<String, _>("updated_at"))
                .map(|dt| dt.with_timezone(&Utc))
                .map_err(|e: chrono::ParseError| StorageError::Internal(e.to_string()))?,
        })
    }

    /// Cria o bucket se ainda não existir (idempotente).
    pub async fn ensure_bucket(&self, name: &str) -> StorageResult<Bucket> {
        match self.get_bucket(name).await {
            Ok(bucket) => Ok(bucket),
            Err(StorageError::BucketNotFound(_)) => match self
                .create_bucket(name, &self.config.default_bucket_location)
                .await
            {
                Ok(bucket) => Ok(bucket),
                Err(StorageError::AlreadyExists(_)) => self.get_bucket(name).await,
                Err(e) => Err(e),
            },
            Err(e) => Err(e),
        }
    }

    async fn resolve_bucket(&self, name: &str) -> StorageResult<Bucket> {
        if self.config.auto_create_buckets {
            self.ensure_bucket(name).await
        } else {
            self.get_bucket(name).await
        }
    }

    /// Garante buckets padrão do Firebase (startup).
    pub async fn ensure_firebase_buckets(
        &self,
        project_id: &str,
        storage_bucket: Option<&str>,
    ) -> StorageResult<()> {
        if !self.config.auto_create_buckets {
            return Ok(());
        }

        let mut names = Vec::new();
        if let Some(bucket) = storage_bucket.filter(|s| !s.is_empty()) {
            names.push(bucket.to_string());
        }
        names.push(format!("{project_id}.firebasestorage.app"));
        names.push(format!("{project_id}.appspot.com"));
        names.push(project_id.to_string());

        names.sort();
        names.dedup();

        for name in names {
            self.ensure_bucket(&name).await?;
        }

        Ok(())
    }

    pub async fn list_buckets(&self) -> StorageResult<Vec<Bucket>> {
        let rows = sqlx::query(
            r#"
            SELECT id, name, location, created_at, updated_at
            FROM buckets ORDER BY name
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter()
            .map(|row| {
                Ok(Bucket {
                    id: Uuid::parse_str(row.get("id"))
                        .map_err(|e: uuid::Error| StorageError::Internal(e.to_string()))?,
                    name: row.get("name"),
                    location: row.get("location"),
                    created_at: chrono::DateTime::parse_from_rfc3339(&row.get::<String, _>("created_at"))
                        .map(|dt| dt.with_timezone(&Utc))
                        .map_err(|e: chrono::ParseError| StorageError::Internal(e.to_string()))?,
                    updated_at: chrono::DateTime::parse_from_rfc3339(&row.get::<String, _>("updated_at"))
                        .map(|dt| dt.with_timezone(&Utc))
                        .map_err(|e: chrono::ParseError| StorageError::Internal(e.to_string()))?,
                })
            })
            .collect()
    }

    pub async fn delete_bucket(&self, name: &str) -> StorageResult<()> {
        let count: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM objects WHERE bucket_name = ?1")
                .bind(name)
                .fetch_one(&self.pool)
                .await?;

        if count.0 > 0 {
            return Err(StorageError::BadRequest(
                "bucket não está vazio; delete os objetos primeiro".to_string(),
            ));
        }

        let result = sqlx::query("DELETE FROM buckets WHERE name = ?1")
            .bind(name)
            .execute(&self.pool)
            .await?;

        if result.rows_affected() == 0 {
            return Err(StorageError::BucketNotFound(name.to_string()));
        }

        Ok(())
    }

    fn storage_key(bucket: &str, object_path: &str, generation: i64) -> String {
        format!("{bucket}/{generation}/{object_path}")
    }

    pub async fn put_object(
        &self,
        bucket_name: &str,
        object_path: &str,
        data: &[u8],
        content_type: Option<&str>,
        custom_metadata: HashMap<String, String>,
    ) -> StorageResult<ObjectMetadata> {
        let bucket = self.resolve_bucket(bucket_name).await?;
        let now = Utc::now();

        let content_type = content_type
            .map(|s| s.to_string())
            .unwrap_or_else(|| {
                from_path(object_path)
                    .first_or_octet_stream()
                    .to_string()
            });

        let mut hasher = Md5::new();
        hasher.update(data);
        let md5_hash = hex::encode(hasher.finalize());

        let existing = self.get_object_record(bucket_name, object_path).await.ok();

        let (generation, metageneration, object_id) = if let Some(existing) = existing {
            (
                existing.generation + 1,
                existing.metageneration + 1,
                existing.id,
            )
        } else {
            (1, 1, Uuid::new_v4())
        };

        let storage_path = Self::storage_key(bucket_name, object_path, generation);
        self.backend.write(&storage_path, data).await?;

        let custom_metadata_json = serde_json::to_string(&custom_metadata)
            .map_err(|e| StorageError::Internal(e.to_string()))?;

        if generation > 1 {
            sqlx::query(
                r#"
                UPDATE objects SET
                    content_type = ?1, size = ?2, md5_hash = ?3,
                    generation = ?4, metageneration = ?5,
                    custom_metadata = ?6, storage_path = ?7, updated_at = ?8
                WHERE bucket_name = ?9 AND object_path = ?10
                "#,
            )
            .bind(&content_type)
            .bind(data.len() as i64)
            .bind(&md5_hash)
            .bind(generation)
            .bind(metageneration)
            .bind(&custom_metadata_json)
            .bind(&storage_path)
            .bind(now.to_rfc3339())
            .bind(bucket_name)
            .bind(object_path)
            .execute(&self.pool)
            .await?;
        } else {
            sqlx::query(
                r#"
                INSERT INTO objects (
                    id, bucket_id, bucket_name, object_path, content_type,
                    size, md5_hash, generation, metageneration,
                    custom_metadata, storage_path, created_at, updated_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
                "#,
            )
            .bind(object_id.to_string())
            .bind(bucket.id.to_string())
            .bind(bucket_name)
            .bind(object_path)
            .bind(&content_type)
            .bind(data.len() as i64)
            .bind(&md5_hash)
            .bind(generation)
            .bind(metageneration)
            .bind(&custom_metadata_json)
            .bind(&storage_path)
            .bind(now.to_rfc3339())
            .bind(now.to_rfc3339())
            .execute(&self.pool)
            .await?;
        }

        Ok(ObjectMetadata {
            name: object_path.to_string(),
            bucket: bucket_name.to_string(),
            content_type,
            size: data.len() as u64,
            md5_hash: Some(md5_hash),
            crc32c: None,
            generation,
            metageneration,
            custom_metadata,
            created_at: now,
            updated_at: now,
        })
    }

    async fn get_object_record(
        &self,
        bucket_name: &str,
        object_path: &str,
    ) -> StorageResult<ObjectRecord> {
        let row = sqlx::query_as::<_, ObjectRecordRow>(
            r#"
            SELECT id, bucket_id, bucket_name, object_path, content_type,
                   size, md5_hash, generation, metageneration,
                   custom_metadata, storage_path, created_at, updated_at
            FROM objects WHERE bucket_name = ?1 AND object_path = ?2
            "#,
        )
        .bind(bucket_name)
        .bind(object_path)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| StorageError::NotFound(format!("{bucket_name}/{object_path}")))?;

        row.into_record()
    }

    pub async fn get_object(
        &self,
        bucket_name: &str,
        object_path: &str,
    ) -> StorageResult<StoredObject> {
        let record = self.get_object_record(bucket_name, object_path).await?;
        let data = self.backend.read(&record.storage_path).await?;

        Ok(StoredObject::new(record.to_metadata(), data))
    }

    pub async fn get_object_metadata(
        &self,
        bucket_name: &str,
        object_path: &str,
    ) -> StorageResult<ObjectMetadata> {
        let record = self.get_object_record(bucket_name, object_path).await?;
        Ok(record.to_metadata())
    }

    pub async fn delete_object(&self, bucket_name: &str, object_path: &str) -> StorageResult<()> {
        let record = self.get_object_record(bucket_name, object_path).await?;
        self.backend.delete(&record.storage_path).await?;

        sqlx::query("DELETE FROM objects WHERE bucket_name = ?1 AND object_path = ?2")
            .bind(bucket_name)
            .bind(object_path)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    pub async fn list_objects(
        &self,
        bucket_name: &str,
        prefix: Option<&str>,
        delimiter: Option<&str>,
        max_results: Option<u32>,
        page_token: Option<&str>,
    ) -> StorageResult<(Vec<ObjectMetadata>, Option<String>)> {
        self.get_bucket(bucket_name).await?;

        let limit = max_results.unwrap_or(1000).min(1000) as i64;
        let offset: i64 = page_token.and_then(|t| t.parse().ok()).unwrap_or(0);

        let record_rows = if let Some(prefix) = prefix {
            if let Some(delimiter) = delimiter {
                let pattern = format!("{prefix}%");
                sqlx::query_as::<_, ObjectRecordRow>(
                    r#"
                    SELECT id, bucket_id, bucket_name, object_path, content_type,
                           size, md5_hash, generation, metageneration,
                           custom_metadata, storage_path, created_at, updated_at
                    FROM objects
                    WHERE bucket_name = ?1 AND object_path LIKE ?2
                    ORDER BY object_path
                    LIMIT ?3 OFFSET ?4
                    "#,
                )
                .bind(bucket_name)
                .bind(&pattern)
                .bind(limit + 1)
                .bind(offset)
                .fetch_all(&self.pool)
                .await?
                .into_iter()
                .filter(|r| {
                    let remainder = r.object_path.strip_prefix(prefix).unwrap_or(&r.object_path);
                    !remainder.contains(delimiter)
                })
                .collect::<Vec<_>>()
            } else {
                let pattern = format!("{prefix}%");
                sqlx::query_as::<_, ObjectRecordRow>(
                    r#"
                    SELECT id, bucket_id, bucket_name, object_path, content_type,
                           size, md5_hash, generation, metageneration,
                           custom_metadata, storage_path, created_at, updated_at
                    FROM objects
                    WHERE bucket_name = ?1 AND object_path LIKE ?2
                    ORDER BY object_path
                    LIMIT ?3 OFFSET ?4
                    "#,
                )
                .bind(bucket_name)
                .bind(&pattern)
                .bind(limit + 1)
                .bind(offset)
                .fetch_all(&self.pool)
                .await?
            }
        } else {
            sqlx::query_as::<_, ObjectRecordRow>(
                r#"
                SELECT id, bucket_id, bucket_name, object_path, content_type,
                       size, md5_hash, generation, metageneration,
                       custom_metadata, storage_path, created_at, updated_at
                FROM objects
                WHERE bucket_name = ?1
                ORDER BY object_path
                LIMIT ?2 OFFSET ?3
                "#,
            )
            .bind(bucket_name)
            .bind(limit + 1)
            .bind(offset)
            .fetch_all(&self.pool)
            .await?
        };

        let has_more = record_rows.len() > limit as usize;
        let items: Vec<ObjectMetadata> = record_rows
            .into_iter()
            .take(limit as usize)
            .map(|r| r.into_record().map(|rec| rec.to_metadata()))
            .collect::<StorageResult<Vec<_>>>()?;

        let next_token = if has_more {
            Some((offset + limit).to_string())
        } else {
            None
        };

        Ok((items, next_token))
    }

    pub async fn update_metadata(
        &self,
        bucket_name: &str,
        object_path: &str,
        custom_metadata: HashMap<String, String>,
    ) -> StorageResult<ObjectMetadata> {
        let record = self.get_object_record(bucket_name, object_path).await?;
        let now = Utc::now();
        let metageneration = record.metageneration + 1;

        let custom_metadata_json = serde_json::to_string(&custom_metadata)
            .map_err(|e| StorageError::Internal(e.to_string()))?;

        sqlx::query(
            r#"
            UPDATE objects SET custom_metadata = ?1, metageneration = ?2, updated_at = ?3
            WHERE bucket_name = ?4 AND object_path = ?5
            "#,
        )
        .bind(&custom_metadata_json)
        .bind(metageneration)
        .bind(now.to_rfc3339())
        .bind(bucket_name)
        .bind(object_path)
        .execute(&self.pool)
        .await?;

        let mut metadata = record.to_metadata();
        metadata.custom_metadata = custom_metadata;
        metadata.metageneration = metageneration;
        metadata.updated_at = now;
        Ok(metadata)
    }

    pub async fn copy_object(
        &self,
        src_bucket: &str,
        src_path: &str,
        dest_bucket: &str,
        dest_path: &str,
    ) -> StorageResult<ObjectMetadata> {
        let src = self.get_object(src_bucket, src_path).await?;
        self.put_object(
            dest_bucket,
            dest_path,
            &src.data,
            Some(&src.metadata.content_type),
            src.metadata.custom_metadata,
        )
        .await
    }

    // --- Resumable Upload ---

    pub async fn create_upload_session(
        &self,
        bucket_name: &str,
        object_path: &str,
        content_type: Option<&str>,
        total_size: Option<u64>,
        custom_metadata: HashMap<String, String>,
        base_url: &str,
        ttl_hours: i64,
    ) -> StorageResult<ResumableUpload> {
        self.resolve_bucket(bucket_name).await?;

        let content_type = content_type
            .map(|s| s.to_string())
            .unwrap_or_else(|| {
                from_path(object_path)
                    .first_or_octet_stream()
                    .to_string()
            });

        let custom_metadata_json = serde_json::to_string(&custom_metadata)
            .map_err(|e| StorageError::Internal(e.to_string()))?;

        let storage_key = format!("uploads/{bucket_name}/{object_path}/{}", Uuid::new_v4());
        let session = UploadSession::new(
            bucket_name,
            object_path,
            content_type,
            total_size,
            custom_metadata_json,
            storage_key,
            ttl_hours,
        );

        sqlx::query(
            r#"
            INSERT INTO upload_sessions (
                id, bucket_name, object_path, content_type, total_size,
                bytes_received, storage_key, status, custom_metadata,
                created_at, expires_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
            "#,
        )
        .bind(session.id.to_string())
        .bind(&session.bucket_name)
        .bind(&session.object_path)
        .bind(&session.content_type)
        .bind(session.total_size)
        .bind(session.bytes_received)
        .bind(&session.storage_key)
        .bind(&session.status)
        .bind(&session.custom_metadata)
        .bind(session.created_at.to_rfc3339())
        .bind(session.expires_at.to_rfc3339())
        .execute(&self.pool)
        .await?;

        let upload_url = format!(
            "{base_url}/v0/b/{}/o?uploadType=resumable&upload_id={}",
            urlencoding::encode(bucket_name),
            session.id
        );

        Ok(ResumableUpload {
            session_id: session.id,
            upload_url,
            expires_at: session.expires_at,
        })
    }

    pub async fn get_upload_session(&self, session_id: Uuid) -> StorageResult<UploadSession> {
        let row = sqlx::query_as::<_, UploadSessionRow>(
            r#"
            SELECT id, bucket_name, object_path, content_type, total_size,
                   bytes_received, storage_key, status, custom_metadata,
                   created_at, expires_at
            FROM upload_sessions WHERE id = ?1
            "#,
        )
        .bind(session_id.to_string())
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| StorageError::UploadSessionNotFound(session_id.to_string()))?;

        let session = row.into_session()?;

        if session.is_expired() {
            return Err(StorageError::UploadSessionExpired(session_id.to_string()));
        }

        Ok(session)
    }

    pub async fn append_upload_chunk(
        &self,
        session_id: Uuid,
        data: &[u8],
        offset: Option<u64>,
    ) -> StorageResult<(i64, bool)> {
        let session = self.get_upload_session(session_id).await?;

        if let Some(expected_offset) = offset {
            if expected_offset != session.bytes_received as u64 {
                return Err(StorageError::BadRequest(format!(
                    "offset esperado: {}, recebido: {expected_offset}",
                    session.bytes_received
                )));
            }
        }

        let new_size = self.backend.append(&session.storage_key, data).await?;

        let is_complete = session
            .total_size
            .map(|total| new_size >= total as u64)
            .unwrap_or(false);

        sqlx::query(
            "UPDATE upload_sessions SET bytes_received = ?1 WHERE id = ?2",
        )
        .bind(new_size as i64)
        .bind(session_id.to_string())
        .execute(&self.pool)
        .await?;

        Ok((new_size as i64, is_complete))
    }

    pub async fn finalize_upload(&self, session_id: Uuid) -> StorageResult<ObjectMetadata> {
        let session = self.get_upload_session(session_id).await?;

        let data = self.backend.read(&session.storage_key).await?;
        let custom_metadata: HashMap<String, String> =
            serde_json::from_str(&session.custom_metadata).unwrap_or_default();

        let metadata = self
            .put_object(
                &session.bucket_name,
                &session.object_path,
                &data,
                Some(&session.content_type),
                custom_metadata,
            )
            .await?;

        self.backend.delete(&session.storage_key).await?;

        sqlx::query("UPDATE upload_sessions SET status = 'completed' WHERE id = ?1")
            .bind(session_id.to_string())
            .execute(&self.pool)
            .await?;

        Ok(metadata)
    }
}
