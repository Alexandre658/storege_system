use utoipa::openapi::security::{HttpAuthScheme, SecurityScheme};
use utoipa::{Modify, OpenApi};

use crate::dto::{
    BackupInfoResponse, BackupListResponse, BucketListResponse, BucketResponse,
    CreateBackupRequest, CreateBucketRequest, DownloadQuery, ErrorResponse,
    HealthResponse, ListObjectsQuery, ObjectListResponse, ObjectQuery, ObjectResponse,
    RestoreBackupResponse, SignedUrlRequest, SignedUrlResponse, UpdateMetadataRequest,
    UploadChunkQuery,
};

struct SecurityAddon;

impl Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme(
                "firebase_token",
                SecurityScheme::Http(
                    utoipa::openapi::security::Http::builder()
                        .scheme(HttpAuthScheme::Bearer)
                        .bearer_format("Firebase ID Token")
                        .build(),
                ),
            )
        }
    }
}

// Funções referenciadas apenas pelo macro `#[derive(OpenApi)]` do utoipa.
#[allow(dead_code)]
mod doc {
    use super::{
        BackupInfoResponse, BackupListResponse, BucketListResponse, BucketResponse,
        CreateBackupRequest, CreateBucketRequest, DownloadQuery, ErrorResponse,
        HealthResponse, ListObjectsQuery, ObjectListResponse, ObjectQuery, ObjectResponse,
        RestoreBackupResponse, SignedUrlRequest, SignedUrlResponse, UpdateMetadataRequest,
        UploadChunkQuery, UploadSessionResponse,
    };

    #[utoipa::path(get, path = "/health", tag = "health",
        responses((status = 200, description = "Serviço operacional", body = HealthResponse)))]
    pub fn health() {}

    #[utoipa::path(post, path = "/v0/b", tag = "buckets", request_body = CreateBucketRequest,
        security(("firebase_token" = [])),
        responses(
            (status = 200, description = "Bucket criado", body = BucketResponse),
            (status = 401, description = "Token inválido", body = ErrorResponse),
            (status = 403, description = "Requer admin", body = ErrorResponse),
            (status = 409, description = "Bucket já existe", body = ErrorResponse),
        ))]
    pub fn create_bucket() {}

    #[utoipa::path(get, path = "/v0/b", tag = "buckets", security(("firebase_token" = [])),
        responses(
            (status = 200, description = "Lista de buckets", body = BucketListResponse),
            (status = 401, description = "Token necessário", body = ErrorResponse),
        ))]
    pub fn list_buckets() {}

    #[utoipa::path(get, path = "/v0/b/{bucket}", tag = "buckets",
        params(("bucket" = String, Path, description = "Nome do bucket")),
        responses(
            (status = 200, description = "Detalhes do bucket", body = BucketResponse),
            (status = 404, description = "Não encontrado", body = ErrorResponse),
        ))]
    pub fn get_bucket() {}

    #[utoipa::path(delete, path = "/v0/b/{bucket}", tag = "buckets",
        params(("bucket" = String, Path, description = "Nome do bucket")),
        security(("firebase_token" = [])),
        responses(
            (status = 204, description = "Bucket deletado"),
            (status = 403, description = "Requer admin", body = ErrorResponse),
        ))]
    pub fn delete_bucket() {}

    #[utoipa::path(get, path = "/v0/b/{bucket}/o", tag = "objects",
        params(("bucket" = String, Path, description = "Nome do bucket"), ListObjectsQuery),
        security(("firebase_token" = [])),
        responses(
            (status = 200, description = "Lista de objetos", body = ObjectListResponse),
            (status = 401, description = "Token necessário", body = ErrorResponse),
        ))]
    pub fn list_objects() {}

    #[utoipa::path(post, path = "/v0/b/{bucket}/o", tag = "objects",
        params(
            ("bucket" = String, Path, description = "Nome do bucket"),
            ("name" = String, Query, description = "Caminho do objeto"),
        ),
        request_body(content_type = "application/octet-stream", description = "Arquivo binário"),
        security(("firebase_token" = [])),
        responses(
            (status = 200, description = "Objeto enviado", body = ObjectResponse),
            (status = 403, description = "Acesso negado", body = ErrorResponse),
        ))]
    pub fn upload_object() {}

    #[utoipa::path(put, path = "/v0/b/{bucket}/o", tag = "upload",
        params(
            ("bucket" = String, Path, description = "Nome do bucket"),
            ("name" = String, Query, description = "Caminho do objeto"),
        ),
        security(("firebase_token" = [])),
        responses(
            (status = 200, description = "Sessão iniciada", body = UploadSessionResponse),
            (status = 403, description = "Acesso negado", body = ErrorResponse),
        ))]
    pub fn initiate_resumable_upload() {}

    #[utoipa::path(get, path = "/v0/b/{bucket}/o/{object_path}", tag = "objects",
        params(
            ("bucket" = String, Path, description = "Nome do bucket"),
            ("object_path" = String, Path, description = "Caminho URL-encoded"),
        ),
        security(()),
        responses(
            (status = 200, description = "Metadados", body = ObjectResponse),
            (status = 404, description = "Não encontrado", body = ErrorResponse),
        ))]
    pub fn download_object_metadata() {}

    #[utoipa::path(get, path = "/v0/b/{bucket}/o/{object_path}", tag = "objects",
        params(
            ("bucket" = String, Path, description = "Nome do bucket"),
            ("object_path" = String, Path, description = "Caminho URL-encoded"),
            ("alt" = String, Query, description = "Use media para download"),
        ),
        security(()),
        responses(
            (status = 200, description = "Conteúdo binário", content_type = "application/octet-stream"),
            (status = 404, description = "Não encontrado", body = ErrorResponse),
        ))]
    pub fn download_object_media() {}

    #[utoipa::path(delete, path = "/v0/b/{bucket}/o/{object_path}", tag = "objects",
        params(
            ("bucket" = String, Path, description = "Nome do bucket"),
            ("object_path" = String, Path, description = "Caminho URL-encoded"),
        ),
        security(("firebase_token" = [])),
        responses((status = 204, description = "Objeto deletado")))]
    pub fn delete_object() {}

    #[utoipa::path(patch, path = "/v0/b/{bucket}/o/{object_path}", tag = "objects",
        params(
            ("bucket" = String, Path, description = "Nome do bucket"),
            ("object_path" = String, Path, description = "Caminho URL-encoded"),
        ),
        request_body = UpdateMetadataRequest,
        security(("firebase_token" = [])),
        responses((status = 200, description = "Metadados atualizados", body = ObjectResponse)))]
    pub fn update_object_metadata() {}

    #[utoipa::path(post, path = "/v0/b/{bucket}/o/{object_path}/copy", tag = "objects",
        params(
            ("bucket" = String, Path, description = "Bucket de origem"),
            ("object_path" = String, Path, description = "Objeto de origem"),
        ),
        security(("firebase_token" = [])),
        responses((status = 200, description = "Objeto copiado", body = ObjectResponse)))]
    pub fn copy_object() {}

    #[utoipa::path(put, path = "/v0/b/{bucket}/o/upload", tag = "upload",
        params(
            ("bucket" = String, Path, description = "Nome do bucket"),
            ("upload_id" = String, Query, description = "ID da sessão"),
        ),
        request_body(content_type = "application/octet-stream", description = "Chunk"),
        responses((status = 200, description = "Chunk recebido", body = ObjectResponse)))]
    pub fn upload_chunk() {}

    #[utoipa::path(post, path = "/v1/signed-url", tag = "signed-url",
        request_body = SignedUrlRequest, security(("firebase_token" = [])),
        responses((status = 200, description = "URL gerada", body = SignedUrlResponse)))]
    pub fn generate_signed_url() {}

    #[utoipa::path(post, path = "/v1/backups", tag = "backups",
        request_body = CreateBackupRequest, security(("firebase_token" = [])),
        responses((status = 201, description = "Backup criado", body = BackupInfoResponse)))]
    pub fn create_backup() {}

    #[utoipa::path(get, path = "/v1/backups", tag = "backups", security(("firebase_token" = [])),
        responses((status = 200, description = "Lista de backups", body = BackupListResponse)))]
    pub fn list_backups() {}

    #[utoipa::path(get, path = "/v1/backups/{id}", tag = "backups",
        params(("id" = String, Path, description = "ID do backup")),
        security(("firebase_token" = [])),
        responses((status = 200, description = "Detalhes", body = BackupInfoResponse)))]
    pub fn get_backup() {}

    #[utoipa::path(delete, path = "/v1/backups/{id}", tag = "backups",
        params(("id" = String, Path, description = "ID do backup")),
        security(("firebase_token" = [])),
        responses((status = 204, description = "Backup deletado")))]
    pub fn delete_backup() {}

    #[utoipa::path(post, path = "/v1/backups/{id}/restore", tag = "backups",
        params(("id" = String, Path, description = "ID do backup")),
        security(("firebase_token" = [])),
        responses((status = 200, description = "Restaurado", body = RestoreBackupResponse)))]
    pub fn restore_backup() {}
}

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Moveme Storage API",
        version = "0.1.0",
        description = "API de object storage compatível com Firebase Storage / Google Cloud Storage.",
        contact(name = "Moveme")
    ),
    servers((url = "http://localhost:8080", description = "Local")),
    modifiers(&SecurityAddon),
    tags(
        (name = "health", description = "Health check"),
        (name = "buckets", description = "Buckets"),
        (name = "objects", description = "Objetos"),
        (name = "upload", description = "Upload resumível"),
        (name = "signed-url", description = "URLs assinadas"),
        (name = "backups", description = "Backup e restauração (admin)")
    ),
    paths(
        doc::health,
        doc::create_bucket,
        doc::list_buckets,
        doc::get_bucket,
        doc::delete_bucket,
        doc::list_objects,
        doc::upload_object,
        doc::initiate_resumable_upload,
        doc::download_object_metadata,
        doc::download_object_media,
        doc::delete_object,
        doc::update_object_metadata,
        doc::copy_object,
        doc::upload_chunk,
        doc::generate_signed_url,
        doc::create_backup,
        doc::list_backups,
        doc::get_backup,
        doc::delete_backup,
        doc::restore_backup,
    ),
    components(schemas(
        HealthResponse, BucketResponse, BucketListResponse, CreateBucketRequest,
        ObjectResponse, ObjectListResponse, UpdateMetadataRequest, UploadSessionResponse,
        SignedUrlRequest, SignedUrlResponse, ErrorResponse, ListObjectsQuery, ObjectQuery,
        DownloadQuery, UploadChunkQuery, CreateBackupRequest, BackupInfoResponse,
        BackupListResponse, RestoreBackupResponse,
    ))
)]
pub struct ApiDoc;
