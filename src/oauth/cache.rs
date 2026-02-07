use std::path::PathBuf;

use crate::error::McplugError;
use crate::oauth::token::TokenData;

pub fn cache_path(server_name: &str) -> PathBuf {
    let base = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".mcplug")
        .join(server_name);
    base.join("tokens.json")
}

pub fn load_cached_token(server_name: &str) -> Option<TokenData> {
    let path = cache_path(server_name);
    let data = std::fs::read_to_string(&path).ok()?;
    serde_json::from_str(&data).ok()
}

pub fn save_token(server_name: &str, token: &TokenData) -> Result<(), McplugError> {
    let path = cache_path(server_name);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let data = serde_json::to_string_pretty(token).map_err(|e| {
        McplugError::OAuthError(format!("Failed to serialize token: {e}"))
    })?;
    std::fs::write(&path, data)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_path_structure() {
        let path = cache_path("github");
        let path_str = path.to_string_lossy();
        assert!(path_str.contains(".mcplug"));
        assert!(path_str.contains("github"));
        assert!(path_str.ends_with("tokens.json"));
    }

    #[test]
    fn cache_path_different_servers() {
        let a = cache_path("server-a");
        let b = cache_path("server-b");
        assert_ne!(a, b);
    }

    #[test]
    fn load_nonexistent_returns_none() {
        assert!(load_cached_token("nonexistent-test-server-xyz").is_none());
    }

    #[test]
    fn save_and_load_roundtrip() {
        let server = "test-roundtrip-oauth-cache";
        let token = TokenData {
            access_token: "test-access".into(),
            refresh_token: Some("test-refresh".into()),
            expires_at: None,
            token_type: "Bearer".into(),
        };

        save_token(server, &token).unwrap();
        let loaded = load_cached_token(server).unwrap();

        assert_eq!(loaded.access_token, "test-access");
        assert_eq!(loaded.refresh_token.as_deref(), Some("test-refresh"));

        // Clean up
        let path = cache_path(server);
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir(path.parent().unwrap());
    }
}
