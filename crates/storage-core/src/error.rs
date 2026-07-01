use thiserror::Error;

pub type StorageResult<T> = Result<T, StorageError>;

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("objeto não encontrado: {0}")]
    NotFound(String),

    #[error("bucket não encontrado: {0}")]
    BucketNotFound(String),

    #[error("sessão de upload não encontrada: {0}")]
    UploadSessionNotFound(String),

    #[error("sessão de upload expirada: {0}")]
    UploadSessionExpired(String),

    #[error("objeto já existe: {0}")]
    AlreadyExists(String),

    #[error("acesso negado: {0}")]
    Forbidden(String),

    #[error("requisição inválida: {0}")]
    BadRequest(String),

    #[error("erro de I/O: {0}")]
    Io(#[from] std::io::Error),

    #[error("erro de banco de dados: {0}")]
    Database(#[from] sqlx::Error),

    #[error("erro interno: {0}")]
    Internal(String),
}
