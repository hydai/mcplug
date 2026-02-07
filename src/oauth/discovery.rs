use serde::Deserialize;

use crate::error::McplugError;

#[derive(Debug, Deserialize)]
pub struct OAuthMetadata {
    pub authorization_endpoint: String,
    pub token_endpoint: String,
    #[serde(default)]
    pub registration_endpoint: Option<String>,
    #[serde(default)]
    pub scopes_supported: Vec<String>,
}

/// Construct the .well-known OAuth metadata URL from a base URL.
fn build_discovery_url(base_url: &str) -> String {
    format!(
        "{}/.well-known/oauth-authorization-server",
        base_url.trim_end_matches('/')
    )
}

pub async fn discover_oauth_metadata(base_url: &str) -> Result<OAuthMetadata, McplugError> {
    let url = build_discovery_url(base_url);
    let resp = reqwest::get(&url).await.map_err(|e| {
        McplugError::OAuthError(format!("Failed to fetch OAuth metadata from {url}: {e}"))
    })?;

    if !resp.status().is_success() {
        return Err(McplugError::OAuthError(format!(
            "OAuth metadata endpoint returned status {}",
            resp.status()
        )));
    }

    let metadata: OAuthMetadata = resp.json().await.map_err(|e| {
        McplugError::OAuthError(format!("Failed to parse OAuth metadata: {e}"))
    })?;

    Ok(metadata)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discovery_url_construction() {
        let url = build_discovery_url("https://auth.example.com");
        assert_eq!(
            url,
            "https://auth.example.com/.well-known/oauth-authorization-server"
        );
    }

    #[test]
    fn discovery_url_strips_trailing_slash() {
        let url = build_discovery_url("https://auth.example.com/");
        assert_eq!(
            url,
            "https://auth.example.com/.well-known/oauth-authorization-server"
        );
        // Multiple trailing slashes: only the last is stripped by trim_end_matches
        // but the function trims all trailing slashes
        let url2 = build_discovery_url("https://auth.example.com///");
        assert_eq!(
            url2,
            "https://auth.example.com/.well-known/oauth-authorization-server"
        );
    }

    #[test]
    fn oauth_metadata_deserialization() {
        let json = r#"{
            "authorization_endpoint": "https://auth.example.com/authorize",
            "token_endpoint": "https://auth.example.com/token",
            "registration_endpoint": "https://auth.example.com/register",
            "scopes_supported": ["read", "write", "admin"]
        }"#;
        let metadata: OAuthMetadata = serde_json::from_str(json).unwrap();
        assert_eq!(
            metadata.authorization_endpoint,
            "https://auth.example.com/authorize"
        );
        assert_eq!(
            metadata.token_endpoint,
            "https://auth.example.com/token"
        );
        assert_eq!(
            metadata.registration_endpoint.as_deref(),
            Some("https://auth.example.com/register")
        );
        assert_eq!(metadata.scopes_supported, vec!["read", "write", "admin"]);
    }

    #[test]
    fn oauth_metadata_minimal_deserialization() {
        let json = r#"{
            "authorization_endpoint": "https://auth.example.com/authorize",
            "token_endpoint": "https://auth.example.com/token"
        }"#;
        let metadata: OAuthMetadata = serde_json::from_str(json).unwrap();
        assert_eq!(
            metadata.authorization_endpoint,
            "https://auth.example.com/authorize"
        );
        assert_eq!(
            metadata.token_endpoint,
            "https://auth.example.com/token"
        );
        assert!(metadata.registration_endpoint.is_none());
        assert!(metadata.scopes_supported.is_empty());
    }
}
