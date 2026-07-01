use utoipa::openapi::security::{HttpAuthScheme, SecurityScheme};
use utoipa::{Modify, OpenApi};

use crate::dto::{
    BackupInfoResponse, BackupListResponse, BucketListResponse, BucketResponse,
    CreateBackupRequest, CreateBucketRequest, DownloadQuery, ErrorResponse,
    HealthResponse, ListObjectsQuery, ObjectListResponse, ObjectQuery, ObjectResponse,
    RestoreBackupResponse, SignedUrlRequest, SignedUrlResponse, UpdateMetadataRequest,
    UploadChunkQuery, UploadSessionResponse,
};

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

    #[utoipa::path(
        get,
        path = "/health",
        tag = "health",
        responses(
            (status = 200, description = "Serviço operacional", body = HealthResponse)
        )
    )]
    pub fn health() {}

    #[utoipa::path(
        post,
        path = "/v0/b",
        tag = "buckets",
        request_body = CreateBucketRequest,
        security(("firebase_token" = [])),
        responses(
            (status = 200, description = "Bucket criado", body = BucketResponse),
            (status = 401, description = "Token Firebase inválido ou ausente", body = ErrorResponse),
            (status = 403, description = "Requer custom claim admin", body = ErrorResponse),
            (status = 409, description = "Bucket já existe", body = ErrorResponse),
        )
    )]
    pub fn create_bucket() {}

    #[utoipa::path(
        get,
        path = "/v0/b",
        tag = "buckets",
        security(("firebase_token" = [])),
        responses(
            (status = 200, description = "Lista de buckets", body = BucketListResponse),
            (status = 401, description = "Token Firebase necessário", body = ErrorResponse),
        )
    )]
    pub fn list_buckets() {}

    #[utoipa::path(
        get,
        path = "/v0/b/{bucket}",
        tag = "buckets",
        params(("bucket" = String, Path, description = "Nome do bucket")),
        responses(
            (status = 200, description = "Detalhes do bucket", body = BucketResponse),
            (status = 404, description = "Bucket não encontrado", body = ErrorResponse),
        )
    )]
    pub fn get_bucket() {}

    #[utoipa::path(
        delete,
        path = "/v0/b/{bucket}",
        tag = "buckets",
        params(("bucket" = String, Path, description = "Nome do bucket")),
        security(("firebase_token" = [])),
        responses(
            (status = 204, description = "Bucket deletado"),
            (status = 403, description = "Requer custom claim admin", body = ErrorResponse),
            (status = 404, description = "Bucket não encontrado", body = ErrorResponse),
        )
    )]
    pub fn delete_bucket() {}

    #[utoipa::path(
        get,
        path = "/v0/b/{bucket}/o",
        tag = "objects",
        params(
            ("bucket" = String, Path, description = "Nome do bucket"),
            ListObjectsQuery,
        ),
        security(("firebase_token" = [])),
        responses(
            (status = 200, description = "Lista de objetos", body = ObjectListResponse),
            (status = 401, description = "Token Firebase necessário", body = ErrorResponse),
        )
    )]
    pub fn list_objects() {}

    #[utoipa::path(
        post,
        path = "/v0/b/{bucket}/o",
        tag = "objects",
        params(
            ("bucket" = String, Path, description = "Nome do bucket"),
            ("name" = String, Query, description = "Caminho do objeto (ex: public/foto.jpg)"),
        ),
        request_body(
            content_type = "application/octet-stream",
            description = "Conteúdo binário do arquivo"
        ),
        security(("firebase_token" = [])),
        responses(
            (status = 200, description = "Objeto enviado", body = ObjectResponse),
            (status = 403, description = "Acesso negado pelas regras de segurança", body = ErrorResponse),
        )
    )]
    pub fn upload_object() {}

    #[utoipa::path(
        put,
        path = "/v0/b/{bucket}/o",
        tag = "upload",
        params(
            ("bucket" = String, Path, description = "Nome do bucket"),
            ("name" = String, Query, description = "Caminho do objeto"),
        ),
        security(("firebase_token" = [])),
        responses(
            (status = 200, description = "Sessão de upload iniciada", body = UploadSessionResponse,
                headers(
                    ("location" = String, description = "URL para envio dos chunks"),
                    ("x-goog-upload-url" = String, description = "URL de upload resumível")
                )
            ),
            (status = 403, description = "Acesso negado", body = ErrorResponse),
        )
    )]
    pub fn initiate_resumable_upload() {}

    #[utoipa::path(
        get,
        path = "/v0/b/{bucket}/o/{object_path}",
        tag = "objects",
        params(
            ("bucket" = String, Path, description = "Nome do bucket"),
            ("object_path" = String, Path, description = "Caminho do objeto (URL-encoded)"),
        ),
        security(()),
        responses(
            (status = 200, description = "Metadados do objeto", body = ObjectResponse),
            (status = 404, description = "Objeto não encontrado", body = ErrorResponse),
        )
    )]
    pub fn download_object_metadata() {}

    #[utoipa::path(
        get,
        path = "/v0/b/{bucket}/o/{object_path}",
        tag = "objects",
        params(
            ("bucket" = String, Path, description = "Nome do bucket"),
            ("object_path" = String, Path, description = "Caminho do objeto (URL-encoded)"),
            ("alt" = String, Query, description = "Use `media` para download do conteúdo"),
        ),
        security(()),
        responses(
            (status = 200, description = "Conteúdo binário do objeto", content_type = "application/octet-stream"),
            (status = 403, description = "Acesso negado", body = ErrorResponse),
            (status = 404, description = "Objeto não encontrado", body = ErrorResponse),
        )
    )]
    pub fn download_object_media() {}

    #[utoipa::path(
        delete,
        path = "/v0/b/{bucket}/o/{object_path}",
        tag = "objects",
        params(
            ("bucket" = String, Path, description = "Nome do bucket"),
            ("object_path" = String, Path, description = "Caminho do objeto (URL-encoded)"),
        ),
        security(("firebase_token" = [])),
        responses(
            (status = 204, description = "Objeto deletado"),
            (status = 403, description = "Acesso negado", body = ErrorResponse),
            (status = 404, description = "Objeto não encontrado", body = ErrorResponse),
        )
    )]
    pub fn delete_object() {}

    #[utoipa::path(
        patch,
        path = "/v0/b/{bucket}/o/{object_path}",
        tag = "objects",
        params(
            ("bucket" = String, Path, description = "Nome do bucket"),
            ("object_path" = String, Path, description = "Caminho do objeto (URL-encoded)"),
        ),
        request_body = UpdateMetadataRequest,
        security(("firebase_token" = [])),
        responses(
            (status = 200, description = "Metadados atualizados", body = ObjectResponse),
            (status = 403, description = "Acesso negado", body = ErrorResponse),
        )
    )]
    pub fn update_object_metadata() {}

    #[utoipa::path(
        post,
        path = "/v0/b/{bucket}/o/{object_path}/copy",
        tag = "objects",
        params(
            ("bucket" = String, Path, description = "Bucket de origem"),
            ("object_path" = String, Path, description = "Objeto de origem (URL-encoded)"),
        ),
        security(("firebase_token" = [])),
        responses(
            (status = 200, description = "Objeto copiado", body = ObjectResponse),
            (status = 400, description = "Headers x-dest-bucket e x-dest-object obrigatórios", body = ErrorResponse),
        )
    )]
    pub fn copy_object() {}

    #[utoipa::path(
        put,
        path = "/v0/b/{bucket}/o/upload",
        tag = "upload",
        params(
            ("bucket" = String, Path, description = "Nome do bucket"),
            ("upload_id" = String, Query, description = "ID da sessão de upload resumível"),
        ),
        request_body(
            content_type = "application/octet-stream",
            description = "Chunk de dados"
        ),
        responses(
            (status = 200, description = "Chunk recebido ou upload finalizado", body = ObjectResponse),
            (status = 400, description = "upload_id inválido", body = ErrorResponse),
        )
    )]
    pub fn upload_chunk() {}

    #[utoipa::path(
        post,
        path = "/v1/signed-url",
        tag = "signed-url",
        request_body = SignedUrlRequest,
        security(("firebase_token" = [])),
        responses(
            (status = 200, description = "URL assinada gerada", body = SignedUrlResponse),
            (status = 401, description = "Token Firebase necessário", body = ErrorResponse),
        )
    )]
    pub fn generate_signed_url() {}

    #[utoipa::path(
        post,
        path = "/v1/backups",
        tag = "backups",
        request_body = CreateBackupRequest,
        security(("firebase_token" = [])),
        responses(
            (status = 201, description = "Backup criado", body = BackupInfoResponse),
            (status = 403, description = "Requer admin", body = ErrorResponse),
        )
    )]
    pub fn create_backup() {}

    #[utoipa::path(
        get,
        path = "/v1/backups",
        tag = "backups",
        security(("firebase_token" = [])),
        responses(
            (status = 200, description = "Lista de backups", body = BackupListResponse),
            (status = 403, description = "Requer admin", body = ErrorResponse),
        )
    )]
    pub fn list_backups() {}

    #[utoipa::path(
        get,
        path = "/v1/backups/{id}",
        tag = "backups",
        params(("id" = String, Path, description = "ID do backup")),
        security(("firebase_token" = [])),
        responses(
            (status = 200, description = "Detalhes do backup", body = BackupInfoResponse),
            (status = 404, description = "Backup não encontrado", body = ErrorResponse),
        )
    )]
    pub fn get_backup() {}

    #[utoipa::path(
        delete,
        path = "/v1/backups/{id}",
        tag = "backups",
        params(("id" = String, Path, description = "ID do backup")),
        security(("firebase_token" = [])),
        responses(
            (status = 204, description = "Backup deletado"),
            (status = 404, description = "Backup não encontrado", body = ErrorResponse),
        )
    )]
    pub fn delete_backup() {}

    #[utoipa::path(
        post,
        path = "/v1/backups/{id}/restore",
        tag = "backups",
        params(("id" = String, Path, description = "ID do backup a restaurar")),
        security(("firebase_token" = [])),
        responses(
            (status = 200, description = "Restauração concluída (reinicie o servidor)", body = RestoreBackupResponse),
            (status = 404, description = "Backup não encontrado", body = ErrorResponse),
        )
    )]
    pub fn restore_backup() {}
}

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Moveme Storage API",
        version = "0.1.0",
        description = "API de object storage compatível com Firebase Storage / Google Cloud Storage.\n\n\
            ## Autenticação\n\
            Envie o **Firebase ID Token** do usuário logado no header:\n\
            `Authorization: Bearer <firebase-id-token>`\n\n\
            Obtenha o token no cliente com:\n\
            `await firebase.auth().currentUser.getIdToken()`",
        contact(name = "Moveme")
    ),
    servers(
        (url = "http://localhost:8080", description = "Desenvolvimento local")
    ),
    modifiers(&SecurityAddon),
    tags(
        (name = "health", description = "Health check"),
        (name = "buckets", description = "Gerenciamento de buckets"),
        (name = "objects", description = "Upload, download e metadados de objetos"),
        (name = "upload", description = "Upload resumível em chunks"),
        (name = "signed-url", description = "URLs assinadas temporárias"),
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
        HealthResponse,
        BucketResponse,
        BucketListResponse,
        CreateBucketRequest,
        ObjectResponse,
        ObjectListResponse,
        UpdateMetadataRequest,
        UploadSessionResponse,
        SignedUrlRequest,
        SignedUrlResponse,
        ErrorResponse,
        ListObjectsQuery,
        ObjectQuery,
        DownloadQuery,
        UploadChunkQuery,
        CreateBackupRequest,
        BackupInfoResponse,
        BackupListResponse,
        RestoreBackupResponse,
    ))
)]
pub struct ApiDoc;

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

// --- Documentação dos endpoints (apenas para OpenAPI) ---

#[allow(dead_code)]

#[utoipa::path(
    get,
    path = "/health",
    tag = "health",
    responses(
        (status = 200, description = "Serviço operacional", body = HealthResponse)
    )
)]
fn doc_health() {}

#[utoipa::path(
    post,
    path = "/v0/b",
    tag = "buckets",
    request_body = CreateBucketRequest,
    security(("firebase_token" = [])),
    responses(
        (status = 200, description = "Bucket criado", body = BucketResponse),
        (status = 401, description = "Token Firebase inválido ou ausente", body = ErrorResponse),
        (status = 403, description = "Requer custom claim admin", body = ErrorResponse),
        (status = 409, description = "Bucket já existe", body = ErrorResponse),
    )
)]
fn doc_create_bucket() {}

#[utoipa::path(
    get,
    path = "/v0/b",
    tag = "buckets",
    security(("firebase_token" = [])),
    responses(
        (status = 200, description = "Lista de buckets", body = BucketListResponse),
        (status = 401, description = "Token Firebase necessário", body = ErrorResponse),
    )
)]
fn doc_list_buckets() {}

#[utoipa::path(
    get,
    path = "/v0/b/{bucket}",
    tag = "buckets",
    params(("bucket" = String, Path, description = "Nome do bucket")),
    responses(
        (status = 200, description = "Detalhes do bucket", body = BucketResponse),
        (status = 404, description = "Bucket não encontrado", body = ErrorResponse),
    )
)]
fn doc_get_bucket() {}

#[utoipa::path(
    delete,
    path = "/v0/b/{bucket}",
    tag = "buckets",
    params(("bucket" = String, Path, description = "Nome do bucket")),
    security(("firebase_token" = [])),
    responses(
        (status = 204, description = "Bucket deletado"),
        (status = 403, description = "Requer custom claim admin", body = ErrorResponse),
        (status = 404, description = "Bucket não encontrado", body = ErrorResponse),
    )
)]
fn doc_delete_bucket() {}

#[utoipa::path(
    get,
    path = "/v0/b/{bucket}/o",
    tag = "objects",
    params(
        ("bucket" = String, Path, description = "Nome do bucket"),
        ListObjectsQuery,
    ),
    security(("firebase_token" = [])),
    responses(
        (status = 200, description = "Lista de objetos", body = ObjectListResponse),
        (status = 401, description = "Token Firebase necessário", body = ErrorResponse),
    )
)]
fn doc_list_objects() {}

#[utoipa::path(
    post,
    path = "/v0/b/{bucket}/o",
    tag = "objects",
    params(
        ("bucket" = String, Path, description = "Nome do bucket"),
        ("name" = String, Query, description = "Caminho do objeto (ex: public/foto.jpg)"),
    ),
    request_body(
        content_type = "application/octet-stream",
        description = "Conteúdo binário do arquivo"
    ),
    security(("firebase_token" = [])),
    responses(
        (status = 200, description = "Objeto enviado", body = ObjectResponse),
        (status = 403, description = "Acesso negado pelas regras de segurança", body = ErrorResponse),
    )
)]
fn doc_upload_object() {}

#[utoipa::path(
    put,
    path = "/v0/b/{bucket}/o",
    tag = "upload",
    params(
        ("bucket" = String, Path, description = "Nome do bucket"),
        ("name" = String, Query, description = "Caminho do objeto"),
    ),
    security(("firebase_token" = [])),
    responses(
        (status = 200, description = "Sessão de upload iniciada", body = UploadSessionResponse,
            headers(
                ("location" = String, description = "URL para envio dos chunks"),
                ("x-goog-upload-url" = String, description = "URL de upload resumível")
            )
        ),
        (status = 403, description = "Acesso negado", body = ErrorResponse),
    )
)]
fn doc_initiate_resumable_upload() {}

#[utoipa::path(
    get,
    path = "/v0/b/{bucket}/o/{object_path}",
    tag = "objects",
    params(
        ("bucket" = String, Path, description = "Nome do bucket"),
        ("object_path" = String, Path, description = "Caminho do objeto (URL-encoded)"),
    ),
    security(()),
    responses(
        (status = 200, description = "Metadados do objeto", body = ObjectResponse),
        (status = 404, description = "Objeto não encontrado", body = ErrorResponse),
    )
)]
fn doc_download_object_metadata() {}

#[utoipa::path(
    get,
    path = "/v0/b/{bucket}/o/{object_path}",
    tag = "objects",
    params(
        ("bucket" = String, Path, description = "Nome do bucket"),
        ("object_path" = String, Path, description = "Caminho do objeto (URL-encoded)"),
        ("alt" = String, Query, description = "Use `media` para download do conteúdo"),
    ),
    security(()),
    responses(
        (status = 200, description = "Conteúdo binário do objeto", content_type = "application/octet-stream"),
        (status = 403, description = "Acesso negado", body = ErrorResponse),
        (status = 404, description = "Objeto não encontrado", body = ErrorResponse),
    )
)]
fn doc_download_object_media() {}

#[utoipa::path(
    delete,
    path = "/v0/b/{bucket}/o/{object_path}",
    tag = "objects",
    params(
        ("bucket" = String, Path, description = "Nome do bucket"),
        ("object_path" = String, Path, description = "Caminho do objeto (URL-encoded)"),
    ),
    security(("firebase_token" = [])),
    responses(
        (status = 204, description = "Objeto deletado"),
        (status = 403, description = "Acesso negado", body = ErrorResponse),
        (status = 404, description = "Objeto não encontrado", body = ErrorResponse),
    )
)]
fn doc_delete_object() {}

#[utoipa::path(
    patch,
    path = "/v0/b/{bucket}/o/{object_path}",
    tag = "objects",
    params(
        ("bucket" = String, Path, description = "Nome do bucket"),
        ("object_path" = String, Path, description = "Caminho do objeto (URL-encoded)"),
    ),
    request_body = UpdateMetadataRequest,
    security(("firebase_token" = [])),
    responses(
        (status = 200, description = "Metadados atualizados", body = ObjectResponse),
        (status = 403, description = "Acesso negado", body = ErrorResponse),
    )
)]
fn doc_update_object_metadata() {}

#[utoipa::path(
    post,
    path = "/v0/b/{bucket}/o/{object_path}/copy",
    tag = "objects",
    params(
        ("bucket" = String, Path, description = "Bucket de origem"),
        ("object_path" = String, Path, description = "Objeto de origem (URL-encoded)"),
    ),
    security(("firebase_token" = [])),
    responses(
        (status = 200, description = "Objeto copiado", body = ObjectResponse),
        (status = 400, description = "Headers x-dest-bucket e x-dest-object obrigatórios", body = ErrorResponse),
    )
)]
fn doc_copy_object() {}

#[utoipa::path(
    put,
    path = "/v0/b/{bucket}/o/upload",
    tag = "upload",
    params(
        ("bucket" = String, Path, description = "Nome do bucket"),
        ("upload_id" = String, Query, description = "ID da sessão de upload resumível"),
    ),
    request_body(
        content_type = "application/octet-stream",
        description = "Chunk de dados"
    ),
    responses(
        (status = 200, description = "Chunk recebido ou upload finalizado", body = ObjectResponse),
        (status = 400, description = "upload_id inválido", body = ErrorResponse),
    )
)]
fn doc_upload_chunk() {}

#[utoipa::path(
    post,
    path = "/v1/signed-url",
    tag = "signed-url",
    request_body = SignedUrlRequest,
    security(("firebase_token" = [])),
    responses(
        (status = 200, description = "URL assinada gerada", body = SignedUrlResponse),
        (status = 401, description = "Token Firebase necessário", body = ErrorResponse),
    )
)]
fn doc_generate_signed_url() {}

#[utoipa::path(
    post,
    path = "/v1/backups",
    tag = "backups",
    request_body = CreateBackupRequest,
    security(("firebase_token" = [])),
    responses(
        (status = 201, description = "Backup criado", body = BackupInfoResponse),
        (status = 403, description = "Requer admin", body = ErrorResponse),
    )
)]
fn doc_create_backup() {}

#[utoipa::path(
    get,
    path = "/v1/backups",
    tag = "backups",
    security(("firebase_token" = [])),
    responses(
        (status = 200, description = "Lista de backups", body = BackupListResponse),
        (status = 403, description = "Requer admin", body = ErrorResponse),
    )
)]
fn doc_list_backups() {}

#[utoipa::path(
    get,
    path = "/v1/backups/{id}",
    tag = "backups",
    params(("id" = String, Path, description = "ID do backup")),
    security(("firebase_token" = [])),
    responses(
        (status = 200, description = "Detalhes do backup", body = BackupInfoResponse),
        (status = 404, description = "Backup não encontrado", body = ErrorResponse),
    )
)]
fn doc_get_backup() {}

#[utoipa::path(
    delete,
    path = "/v1/backups/{id}",
    tag = "backups",
    params(("id" = String, Path, description = "ID do backup")),
    security(("firebase_token" = [])),
    responses(
        (status = 204, description = "Backup deletado"),
        (status = 404, description = "Backup não encontrado", body = ErrorResponse),
    )
)]
fn doc_delete_backup() {}

#[utoipa::path(
    post,
    path = "/v1/backups/{id}/restore",
    tag = "backups",
    params(("id" = String, Path, description = "ID do backup a restaurar")),
    security(("firebase_token" = [])),
    responses(
        (status = 200, description = "Restauração concluída (reinicie o servidor)", body = RestoreBackupResponse),
        (status = 404, description = "Backup não encontrado", body = ErrorResponse),
    )
)]
fn doc_restore_backup() {}
