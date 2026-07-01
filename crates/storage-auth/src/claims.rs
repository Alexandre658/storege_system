use serde::{Deserialize, Serialize};

/// Representa o usuário autenticado via Firebase ID Token.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    /// UID do Firebase (mesmo valor de `sub` no token).
    pub uid: String,
    pub sub: String,
    pub email: Option<String>,
    /// Custom claim `admin` definido via Firebase Admin SDK.
    pub admin: bool,
}

impl Claims {
    pub fn from_firebase(uid: impl Into<String>, email: Option<String>, admin: bool) -> Self {
        let uid = uid.into();
        Self {
            sub: uid.clone(),
            uid,
            email,
            admin,
        }
    }

    #[cfg(test)]
    pub fn new(uid: impl Into<String>, email: Option<String>, admin: bool) -> Self {
        Self::from_firebase(uid, email, admin)
    }

    pub fn is_admin(&self) -> bool {
        self.admin
    }
}
