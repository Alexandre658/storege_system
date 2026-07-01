use axum::{
    extract::Request,
    middleware::Next,
    response::Response,
};
use storage_auth::Claims;

use crate::state::AppState;

#[derive(Clone, Debug)]
pub struct AuthUser(pub Option<Claims>);

pub async fn extract_auth(
    axum::extract::State(state): axum::extract::State<AppState>,
    mut request: Request,
    next: Next,
) -> Response {
    let token = request
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|header| header.strip_prefix("Bearer "))
        .map(str::to_string);

    let claims = match token {
        Some(token) => state.firebase.verify(&token).await.ok(),
        None => None,
    };

    request.extensions_mut().insert(AuthUser(claims));
    next.run(request).await
}

pub fn get_claims(request: &Request) -> Option<Claims> {
    request
        .extensions()
        .get::<AuthUser>()
        .and_then(|u| u.0.clone())
}
