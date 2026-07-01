use std::collections::HashMap;
use std::sync::RwLock;
use std::time::{Duration, Instant};

use jsonwebtoken::{decode, decode_header, Algorithm, DecodingKey, Validation};
use serde::Deserialize;
use thiserror::Error;

use crate::claims::Claims;

const FIREBASE_JWKS_URL: &str =
    "https://www.googleapis.com/service_accounts/v1/jwk/securetoken@system.gserviceaccount.com";

#[derive(Debug, Error)]
pub enum AuthError {
    #[error("token inválido: {0}")]
    InvalidToken(String),

    #[error("token expirado")]
    Expired,

    #[error("acesso negado")]
    Forbidden,
}

pub type AuthResult<T> = Result<T, AuthError>;

#[derive(Debug, Deserialize)]
struct Jwks {
    keys: Vec<JwkKey>,
}

#[derive(Debug, Deserialize)]
struct JwkKey {
    kid: String,
    n: String,
    e: String,
}

#[derive(Debug, Deserialize)]
struct FirebaseIdToken {
    sub: String,
    #[serde(default)]
    email: Option<String>,
    #[serde(flatten)]
    extra: HashMap<String, serde_json::Value>,
}

struct CachedJwks {
    keys: HashMap<String, DecodingKey>,
    fetched_at: Instant,
}

pub struct FirebaseTokenVerifier {
    project_id: String,
    http: reqwest::Client,
    jwks: RwLock<Option<CachedJwks>>,
    jwks_ttl: Duration,
}

impl FirebaseTokenVerifier {
    pub fn new(project_id: impl Into<String>) -> Self {
        Self {
            project_id: project_id.into(),
            http: reqwest::Client::new(),
            jwks: RwLock::new(None),
            jwks_ttl: Duration::from_secs(3600),
        }
    }

    pub async fn verify(&self, token: &str) -> AuthResult<Claims> {
        let header = decode_header(token)
            .map_err(|e| AuthError::InvalidToken(e.to_string()))?;

        let kid = header
            .kid
            .ok_or_else(|| AuthError::InvalidToken("token sem kid".to_string()))?;

        let decoding_key = self.decoding_key_for(&kid).await?;

        let mut validation = Validation::new(Algorithm::RS256);
        validation.set_issuer(&[format!(
            "https://securetoken.google.com/{}",
            self.project_id
        )]);
        validation.set_audience(&[self.project_id.as_str()]);
        validation.validate_exp = true;

        let token_data = decode::<FirebaseIdToken>(token, &decoding_key, &validation).map_err(
            |e| match e.kind() {
                jsonwebtoken::errors::ErrorKind::ExpiredSignature => AuthError::Expired,
                _ => AuthError::InvalidToken(e.to_string()),
            },
        )?;

        let claims = token_data.claims;

        let admin = claims
            .extra
            .get("admin")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
            || claims
                .extra
                .get("role")
                .and_then(|v| v.as_str())
                .map(|r| r == "admin")
                .unwrap_or(false);

        Ok(Claims::from_firebase(claims.sub, claims.email, admin))
    }

    async fn decoding_key_for(&self, kid: &str) -> AuthResult<DecodingKey> {
        if let Some(key) = self.get_cached_key(kid) {
            return Ok(key);
        }

        self.refresh_jwks().await?;

        self.get_cached_key(kid)
            .ok_or_else(|| AuthError::InvalidToken(format!("chave JWKS não encontrada: {kid}")))
    }

    fn get_cached_key(&self, kid: &str) -> Option<DecodingKey> {
        let cache = self.jwks.read().ok()?;
        let cached = cache.as_ref()?;
        if cached.fetched_at.elapsed() > self.jwks_ttl {
            return None;
        }
        cached.keys.get(kid).cloned()
    }

    async fn refresh_jwks(&self) -> AuthResult<()> {
        let response = self
            .http
            .get(FIREBASE_JWKS_URL)
            .send()
            .await
            .map_err(|e| AuthError::InvalidToken(format!("falha ao buscar JWKS: {e}")))?;

        let jwks: Jwks = response
            .json()
            .await
            .map_err(|e| AuthError::InvalidToken(format!("JWKS inválido: {e}")))?;

        let mut keys = HashMap::new();
        for key in jwks.keys {
            let decoding_key = DecodingKey::from_rsa_components(&key.n, &key.e)
                .map_err(|e| AuthError::InvalidToken(e.to_string()))?;
            keys.insert(key.kid, decoding_key);
        }

        let mut cache = self
            .jwks
            .write()
            .map_err(|_| AuthError::InvalidToken("cache JWKS indisponível".to_string()))?;
        *cache = Some(CachedJwks {
            keys,
            fetched_at: Instant::now(),
        });

        Ok(())
    }
}
