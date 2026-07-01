use axum::{
    middleware,
    routing::{delete, get, patch, post, put},
    Router,
};
use tower_http::cors::{Any, CorsLayer};
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::trace::TraceLayer;

use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use crate::handlers;
use crate::middleware::extract_auth;
use crate::openapi::ApiDoc;
use crate::state::AppState;

pub fn create_router(state: AppState) -> Router {
    let max_upload = state.max_upload_size;

    let api_routes = Router::new()
        .route("/health", get(handlers::health))
        // Buckets (Firebase/GCS compatible)
        .route("/v0/b", post(handlers::create_bucket))
        .route("/v0/b", get(handlers::list_buckets))
        .route("/v0/b/{bucket}", get(handlers::get_bucket))
        .route("/v0/b/{bucket}", delete(handlers::delete_bucket))
        // Objects
        .route("/v0/b/{bucket}/o", get(handlers::list_objects))
        .route("/v0/b/{bucket}/o", post(handlers::upload_object))
        .route("/v0/b/{bucket}/o", put(handlers::initiate_resumable_upload))
        .route(
            "/v0/b/{bucket}/o/{object_path}",
            get(handlers::download_object),
        )
        .route(
            "/v0/b/{bucket}/o/{object_path}",
            delete(handlers::delete_object),
        )
        .route(
            "/v0/b/{bucket}/o/{object_path}",
            patch(handlers::update_object_metadata),
        )
        .route(
            "/v0/b/{bucket}/o/{object_path}/copy",
            post(handlers::copy_object),
        )
        // Resumable upload chunks
        .route("/v0/b/{bucket}/o/upload", put(handlers::upload_chunk))
        // Signed URLs
        .route("/v1/signed-url", post(handlers::generate_signed_url))
        // Backups (admin)
        .route("/v1/backups", post(handlers::create_backup))
        .route("/v1/backups", get(handlers::list_backups))
        .route("/v1/backups/{id}", get(handlers::get_backup))
        .route("/v1/backups/{id}", delete(handlers::delete_backup))
        .route("/v1/backups/{id}/restore", post(handlers::restore_backup))
        .layer(middleware::from_fn_with_state(state.clone(), extract_auth))
        .layer(RequestBodyLimitLayer::new(max_upload))
        .with_state(state);

    Router::new()
        .merge(api_routes)
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        )
        .layer(TraceLayer::new_for_http())
}
