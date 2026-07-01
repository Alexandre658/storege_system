use hmac::{Hmac, Mac};
use sha2::Sha256;
use thiserror::Error;

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, Error)]
pub enum SignedUrlError {
    #[error("URL expirada")]
    Expired,

    #[error("assinatura inválida")]
    InvalidSignature,

    #[error("parâmetros inválidos: {0}")]
    InvalidParams(String),
}

pub struct SignedUrlGenerator {
    secret: Vec<u8>,
}

impl SignedUrlGenerator {
    pub fn new(secret: impl AsRef<[u8]>) -> Self {
        Self {
            secret: secret.as_ref().to_vec(),
        }
    }

    pub fn generate(
        &self,
        base_url: &str,
        bucket: &str,
        object_path: &str,
        method: &str,
        expires_in_secs: u64,
        content_type: Option<&str>,
    ) -> String {
        let expires = chrono::Utc::now().timestamp() as u64 + expires_in_secs;
        let canonical = Self::canonical_string(method, bucket, object_path, expires, content_type);
        let signature = self.sign(&canonical);

        let encoded_path = urlencoding::encode(object_path);
        let mut url = format!(
            "{base_url}/v0/b/{}/o/{encoded_path}?X-Goog-Algorithm=GOOG4-RSA-SHA256&X-Goog-Credential=storage&X-Goog-Date={expires}&X-Goog-Expires={expires_in_secs}&X-Goog-SignedHeaders=host&X-Goog-Signature={signature}",
            urlencoding::encode(bucket)
        );

        if let Some(ct) = content_type {
            url.push_str(&format!("&response-content-type={}", urlencoding::encode(ct)));
        }

        url
    }

    pub fn verify(
        &self,
        method: &str,
        bucket: &str,
        object_path: &str,
        expires: u64,
        signature: &str,
        content_type: Option<&str>,
    ) -> Result<(), SignedUrlError> {
        let now = chrono::Utc::now().timestamp() as u64;
        if now > expires {
            return Err(SignedUrlError::Expired);
        }

        let canonical = Self::canonical_string(method, bucket, object_path, expires, content_type);
        let expected = self.sign(&canonical);

        if !constant_time_eq(signature.as_bytes(), expected.as_bytes()) {
            return Err(SignedUrlError::InvalidSignature);
        }

        Ok(())
    }

    fn canonical_string(
        method: &str,
        bucket: &str,
        object_path: &str,
        expires: u64,
        content_type: Option<&str>,
    ) -> String {
        let mut parts = vec![
            method.to_uppercase(),
            bucket.to_string(),
            object_path.to_string(),
            expires.to_string(),
        ];
        if let Some(ct) = content_type {
            parts.push(ct.to_string());
        }
        parts.join("\n")
    }

    fn sign(&self, data: &str) -> String {
        let mut mac =
            HmacSha256::new_from_slice(&self.secret).expect("HMAC aceita chaves de qualquer tamanho");
        mac.update(data.as_bytes());
        hex::encode(mac.finalize().into_bytes())
    }
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter()
        .zip(b.iter())
        .fold(0u8, |acc, (x, y)| acc | (x ^ y))
        == 0
}
