use crate::claims::Claims;

/// Se o caminho não tiver `/`, prefixa com `users/{uid}/` para utilizadores autenticados.
pub fn normalize_object_path(path: &str, claims: &Option<Claims>) -> String {
    let path = path.trim().trim_start_matches('/');
    if path.is_empty() {
        return path.to_string();
    }
    if path.contains('/') {
        return path.to_string();
    }
    if let Some(claims) = claims {
        return format!("users/{}/{}", claims.uid, path);
    }
    path.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefixes_bare_filename_with_user_folder() {
        let claims = Some(Claims::from_firebase("uid-123", None, false));
        assert_eq!(
            normalize_object_path("foto.jpg", &claims),
            "users/uid-123/foto.jpg"
        );
    }

    #[test]
    fn keeps_prefixed_paths() {
        let claims = Some(Claims::from_firebase("uid-123", None, false));
        assert_eq!(
            normalize_object_path("public/foto.jpg", &claims),
            "public/foto.jpg"
        );
    }
}
