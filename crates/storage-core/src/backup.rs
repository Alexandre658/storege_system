use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use flate2::write::GzEncoder;
use flate2::Compression;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::sqlite::SqlitePool;
use tar::Builder;
use tokio::fs;
use tokio::io::AsyncReadExt;
use uuid::Uuid;
use walkdir::WalkDir;

use crate::error::{StorageError, StorageResult};

const MANIFEST_FILE: &str = "manifest.json";
const DB_FILE: &str = "storage.db";
const OBJECTS_DIR: &str = "objects";
const BACKUP_VERSION: &str = "1";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupManifest {
    pub id: String,
    pub label: Option<String>,
    pub version: String,
    pub created_at: DateTime<Utc>,
    pub database_file: String,
    pub objects_dir: String,
    pub size_bytes: u64,
    pub checksum_sha256: String,
    pub bucket_count: u64,
    pub object_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupInfo {
    pub id: String,
    pub label: Option<String>,
    pub filename: String,
    pub created_at: DateTime<Utc>,
    pub size_bytes: u64,
    pub checksum_sha256: String,
    pub bucket_count: u64,
    pub object_count: u64,
}

pub struct BackupService {
    data_dir: PathBuf,
    database_path: PathBuf,
    objects_dir: PathBuf,
    backup_dir: PathBuf,
    retention_count: usize,
    pool: SqlitePool,
}

impl BackupService {
    pub fn new(
        data_dir: PathBuf,
        database_url: &str,
        backup_dir: PathBuf,
        retention_count: usize,
        pool: SqlitePool,
    ) -> StorageResult<Self> {
        let database_path = resolve_database_path(database_url, &data_dir)?;
        let objects_dir = data_dir.join(OBJECTS_DIR);

        std::fs::create_dir_all(&backup_dir)?;

        Ok(Self {
            data_dir,
            database_path,
            objects_dir,
            backup_dir,
            retention_count,
            pool,
        })
    }

    pub fn backup_dir(&self) -> &Path {
        &self.backup_dir
    }

    pub async fn create_backup(&self, label: Option<String>) -> StorageResult<BackupInfo> {
        let id = Uuid::new_v4();
        let created_at = Utc::now();
        let stamp = created_at.format("%Y%m%d-%H%M%S");
        let filename = format!("backup-{stamp}-{id}.tar.gz");
        let archive_path = self.backup_dir.join(&filename);
        let temp_dir = self.backup_dir.join(".tmp").join(id.to_string());

        if temp_dir.exists() {
            fs::remove_dir_all(&temp_dir).await?;
        }
        fs::create_dir_all(&temp_dir).await?;
        fs::create_dir_all(temp_dir.join(OBJECTS_DIR)).await?;

        self.checkpoint_database().await?;
        fs::copy(&self.database_path, temp_dir.join(DB_FILE)).await?;

        if self.objects_dir.exists() {
            copy_dir_recursive(&self.objects_dir, &temp_dir.join(OBJECTS_DIR)).await?;
        }

        let (bucket_count, object_count) = self.count_stats().await?;

        let manifest = BackupManifest {
            id: id.to_string(),
            label: label.clone(),
            version: BACKUP_VERSION.to_string(),
            created_at,
            database_file: DB_FILE.to_string(),
            objects_dir: OBJECTS_DIR.to_string(),
            size_bytes: 0,
            checksum_sha256: String::new(),
            bucket_count,
            object_count,
        };

        let manifest_path = temp_dir.join(MANIFEST_FILE);
        let manifest_json = serde_json::to_string_pretty(&manifest)
            .map_err(|e| StorageError::Internal(e.to_string()))?;
        fs::write(&manifest_path, &manifest_json).await?;

        let temp_dir_clone = temp_dir.clone();
        let archive_path_clone = archive_path.clone();
        tokio::task::spawn_blocking(move || create_tar_gz(&temp_dir_clone, &archive_path_clone))
            .await
            .map_err(|e| StorageError::Internal(e.to_string()))??;

        fs::remove_dir_all(&temp_dir).await?;

        let size_bytes = fs::metadata(&archive_path).await?.len();
        let checksum_sha256 = file_sha256(&archive_path).await?;

        let mut final_manifest = manifest;
        final_manifest.size_bytes = size_bytes;
        final_manifest.checksum_sha256 = checksum_sha256.clone();
        self.write_sidecar_manifest(&archive_path, &final_manifest).await?;

        self.apply_retention().await?;

        tracing::info!(
            backup_id = %id,
            filename = %filename,
            size_bytes,
            "Backup criado com sucesso"
        );

        Ok(BackupInfo {
            id: id.to_string(),
            label,
            filename,
            created_at,
            size_bytes,
            checksum_sha256,
            bucket_count,
            object_count,
        })
    }

    pub async fn list_backups(&self) -> StorageResult<Vec<BackupInfo>> {
        let mut backups = Vec::new();
        let mut entries = fs::read_dir(&self.backup_dir).await?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("gz") {
                continue;
            }
            if let Some(info) = self.backup_info_from_archive(&path).await.ok() {
                backups.push(info);
            }
        }

        backups.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(backups)
    }

    pub async fn get_backup(&self, id: &str) -> StorageResult<BackupInfo> {
        let path = self.find_backup_archive(id).await?;
        self.backup_info_from_archive(&path).await
    }

    pub async fn delete_backup(&self, id: &str) -> StorageResult<()> {
        let path = self.find_backup_archive(id).await?;
        fs::remove_file(&path).await?;
        let sidecar = path.with_extension("json");
        if sidecar.exists() {
            fs::remove_file(sidecar).await?;
        }
        Ok(())
    }

    /// Restaura um backup. Cria automaticamente um backup de segurança antes.
    pub async fn restore_backup(&self, id: &str) -> StorageResult<BackupInfo> {
        let archive_path = self.find_backup_archive(id).await?;

        tracing::warn!(backup_id = %id, "Iniciando restauração — criando backup de segurança");
        let safety = self
            .create_backup(Some(format!("pre-restore-{id}")))
            .await?;

        let temp_extract = self.backup_dir.join(".tmp").join(format!("restore-{id}"));
        if temp_extract.exists() {
            fs::remove_dir_all(&temp_extract).await?;
        }
        fs::create_dir_all(&temp_extract).await?;

        let archive_clone = archive_path.clone();
        let temp_clone = temp_extract.clone();
        tokio::task::spawn_blocking(move || extract_tar_gz(&archive_clone, &temp_clone))
            .await
            .map_err(|e| StorageError::Internal(e.to_string()))??;

        let manifest_path = temp_extract.join(MANIFEST_FILE);
        if !manifest_path.exists() {
            return Err(StorageError::BadRequest(
                "backup inválido: manifest.json ausente".to_string(),
            ));
        }

        let manifest_bytes = fs::read(&manifest_path).await?;
        let manifest: BackupManifest = serde_json::from_slice(&manifest_bytes)
            .map_err(|e| StorageError::BadRequest(format!("manifest inválido: {e}")))?;

        if manifest.version != BACKUP_VERSION {
            return Err(StorageError::BadRequest(format!(
                "versão de backup incompatível: {}",
                manifest.version
            )));
        }

        let db_src = temp_extract.join(&manifest.database_file);
        let objects_src = temp_extract.join(&manifest.objects_dir);

        if !db_src.exists() {
            return Err(StorageError::BadRequest(
                "backup inválido: storage.db ausente".to_string(),
            ));
        }

        fs::create_dir_all(&self.data_dir).await?;

        let db_old = self.data_dir.join(format!("{DB_FILE}.restoring-old"));
        if self.database_path.exists() {
            if db_old.exists() {
                fs::remove_file(&db_old).await?;
            }
            fs::rename(&self.database_path, &db_old).await?;
        }

        if let Err(e) = fs::copy(&db_src, &self.database_path).await {
            if db_old.exists() {
                let _ = fs::rename(&db_old, &self.database_path).await;
            }
            return Err(e.into());
        }

        let objects_old = self.data_dir.join(format!("{OBJECTS_DIR}.restoring-old"));
        if self.objects_dir.exists() {
            if objects_old.exists() {
                fs::remove_dir_all(&objects_old).await?;
            }
            fs::rename(&self.objects_dir, &objects_old).await?;
        }

        if objects_src.exists() {
            if let Err(e) = fs::rename(&objects_src, &self.objects_dir).await {
                let _ = fs::rename(&objects_old, &self.objects_dir).await;
                let _ = fs::rename(&db_old, &self.database_path).await;
                return Err(e.into());
            }
        } else {
            fs::create_dir_all(&self.objects_dir).await?;
        }

        if db_old.exists() {
            fs::remove_file(db_old).await?;
        }
        if objects_old.exists() {
            fs::remove_dir_all(objects_old).await?;
        }

        fs::remove_dir_all(&temp_extract).await?;

        tracing::info!(
            backup_id = %id,
            safety_backup_id = %safety.id,
            "Restauração concluída — reinicie o servidor para recarregar o banco"
        );

        Ok(safety)
    }

    async fn checkpoint_database(&self) -> StorageResult<()> {
        sqlx::query("PRAGMA wal_checkpoint(FULL)")
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn count_stats(&self) -> StorageResult<(u64, u64)> {
        let buckets: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM buckets")
            .fetch_one(&self.pool)
            .await?;
        let objects: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM objects")
            .fetch_one(&self.pool)
            .await?;
        Ok((buckets.0 as u64, objects.0 as u64))
    }

    async fn apply_retention(&self) -> StorageResult<()> {
        if self.retention_count == 0 {
            return Ok(());
        }

        let backups = self.list_backups().await?;
        if backups.len() <= self.retention_count {
            return Ok(());
        }

        for backup in backups.iter().skip(self.retention_count) {
            if let Err(e) = self.delete_backup(&backup.id).await {
                tracing::warn!(backup_id = %backup.id, error = %e, "Falha ao aplicar retenção");
            }
        }

        Ok(())
    }

    async fn find_backup_archive(&self, id: &str) -> StorageResult<PathBuf> {
        let backups = self.list_backups().await?;
        backups
            .into_iter()
            .find(|b| b.id == id)
            .map(|b| self.backup_dir.join(b.filename))
            .ok_or_else(|| StorageError::NotFound(format!("backup '{id}'")))
    }

    async fn backup_info_from_archive(&self, path: &Path) -> StorageResult<BackupInfo> {
        let sidecar = path.with_extension("json");
        if sidecar.exists() {
            let bytes = fs::read(&sidecar).await?;
            let manifest: BackupManifest = serde_json::from_slice(&bytes)
                .map_err(|e| StorageError::Internal(e.to_string()))?;
            return Ok(BackupInfo {
                id: manifest.id,
                label: manifest.label,
                filename: path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or_default()
                    .to_string(),
                created_at: manifest.created_at,
                size_bytes: manifest.size_bytes,
                checksum_sha256: manifest.checksum_sha256,
                bucket_count: manifest.bucket_count,
                object_count: manifest.object_count,
            });
        }

        let filename = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
            .to_string();

        let id = filename
            .strip_prefix("backup-")
            .and_then(|s| s.strip_suffix(".tar.gz"))
            .and_then(|s| s.rsplit('-').next())
            .unwrap_or(&filename)
            .to_string();

        let metadata = fs::metadata(path).await?;
        let checksum_sha256 = file_sha256(path).await?;

        Ok(BackupInfo {
            id,
            label: None,
            filename,
            created_at: Utc::now(),
            size_bytes: metadata.len(),
            checksum_sha256,
            bucket_count: 0,
            object_count: 0,
        })
    }

    async fn write_sidecar_manifest(
        &self,
        archive_path: &Path,
        manifest: &BackupManifest,
    ) -> StorageResult<()> {
        let sidecar = archive_path.with_extension("json");
        let json = serde_json::to_string_pretty(manifest)
            .map_err(|e| StorageError::Internal(e.to_string()))?;
        fs::write(sidecar, json).await?;
        Ok(())
    }
}

fn resolve_database_path(database_url: &str, _data_dir: &Path) -> StorageResult<PathBuf> {
    let path = database_url
        .strip_prefix("sqlite://")
        .ok_or_else(|| StorageError::BadRequest("database_url deve ser sqlite://".to_string()))?;

    let path = PathBuf::from(path);
    if path.is_absolute() {
        Ok(path)
    } else {
        Ok(std::env::current_dir()
            .map_err(|e| StorageError::Internal(e.to_string()))?
            .join(path))
    }
}

async fn copy_dir_recursive(src: &Path, dst: &Path) -> StorageResult<()> {
    fs::create_dir_all(dst).await?;
    let mut entries = fs::read_dir(src).await?;
    while let Some(entry) = entries.next_entry().await? {
        let ty = entry.file_type().await?;
        let dest_path = dst.join(entry.file_name());
        if ty.is_dir() {
            Box::pin(copy_dir_recursive(&entry.path(), &dest_path)).await?;
        } else {
            fs::copy(entry.path(), dest_path).await?;
        }
    }
    Ok(())
}

fn create_tar_gz(source_dir: &Path, archive_path: &Path) -> StorageResult<()> {
    let file = std::fs::File::create(archive_path)?;
    let encoder = GzEncoder::new(file, Compression::default());
    let mut tar = Builder::new(encoder);

    for entry in WalkDir::new(source_dir).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        let relative = path
            .strip_prefix(source_dir)
            .map_err(|e| StorageError::Internal(e.to_string()))?;
        if relative.as_os_str().is_empty() {
            continue;
        }
        if path.is_file() {
            let mut file = std::fs::File::open(path)?;
            tar.append_file(relative, &mut file)?;
        } else if path.is_dir() {
            tar.append_dir(relative, path)?;
        }
    }

    tar.finish()?;
    Ok(())
}

fn extract_tar_gz(archive_path: &Path, dest_dir: &Path) -> StorageResult<()> {
    let file = std::fs::File::open(archive_path)?;
    let decoder = flate2::read::GzDecoder::new(file);
    let mut archive = tar::Archive::new(decoder);
    archive.unpack(dest_dir)?;
    Ok(())
}

async fn file_sha256(path: &Path) -> StorageResult<String> {
    let mut file = fs::File::open(path).await?;
    let mut buffer = vec![0u8; 8192];
    let mut hasher = Sha256::new();

    loop {
        let n = file.read(&mut buffer).await?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
    }

    Ok(hex::encode(hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_relative_db_path() {
        let path = resolve_database_path("sqlite://./data/storage.db", Path::new("./data"))
            .unwrap();
        assert!(path.ends_with("storage.db"));
    }
}
