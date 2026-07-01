use std::path::{Path, PathBuf};

use async_trait::async_trait;
use bytes::Bytes;
use sha2::{Digest, Sha256};
use tokio::fs;
use tokio::io::AsyncWriteExt;

use crate::error::{StorageError, StorageResult};

#[async_trait]
pub trait StorageBackend: Send + Sync {
    async fn write(&self, key: &str, data: &[u8]) -> StorageResult<String>;
    async fn read(&self, key: &str) -> StorageResult<Bytes>;
    async fn delete(&self, key: &str) -> StorageResult<()>;
    async fn exists(&self, key: &str) -> StorageResult<bool>;
    async fn append(&self, key: &str, data: &[u8]) -> StorageResult<u64>;
    async fn size(&self, key: &str) -> StorageResult<u64>;
}

pub struct LocalFilesystemBackend {
    root: PathBuf,
}

impl LocalFilesystemBackend {
    pub fn new(root: impl AsRef<Path>) -> StorageResult<Self> {
        let root = root.as_ref().to_path_buf();
        std::fs::create_dir_all(&root)?;
        Ok(Self { root })
    }

    fn resolve(&self, key: &str) -> PathBuf {
        self.root.join(key)
    }

    async fn ensure_parent(&self, path: &Path) -> StorageResult<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await?;
        }
        Ok(())
    }
}

#[async_trait]
impl StorageBackend for LocalFilesystemBackend {
    async fn write(&self, key: &str, data: &[u8]) -> StorageResult<String> {
        let path = self.resolve(key);
        self.ensure_parent(&path).await?;

        let mut file = fs::File::create(&path).await?;
        file.write_all(data).await?;
        file.flush().await?;

        let mut hasher = Sha256::new();
        hasher.update(data);
        Ok(hex::encode(hasher.finalize()))
    }

    async fn read(&self, key: &str) -> StorageResult<Bytes> {
        let path = self.resolve(key);
        if !path.exists() {
            return Err(StorageError::NotFound(key.to_string()));
        }
        let data = fs::read(&path).await?;
        Ok(Bytes::from(data))
    }

    async fn delete(&self, key: &str) -> StorageResult<()> {
        let path = self.resolve(key);
        if path.exists() {
            fs::remove_file(&path).await?;
        }
        Ok(())
    }

    async fn exists(&self, key: &str) -> StorageResult<bool> {
        Ok(self.resolve(key).exists())
    }

    async fn append(&self, key: &str, data: &[u8]) -> StorageResult<u64> {
        let path = self.resolve(key);
        self.ensure_parent(&path).await?;

        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .await?;
        file.write_all(data).await?;
        file.flush().await?;

        let metadata = fs::metadata(&path).await?;
        Ok(metadata.len())
    }

    async fn size(&self, key: &str) -> StorageResult<u64> {
        let path = self.resolve(key);
        if !path.exists() {
            return Err(StorageError::NotFound(key.to_string()));
        }
        let metadata = fs::metadata(&path).await?;
        Ok(metadata.len())
    }
}
