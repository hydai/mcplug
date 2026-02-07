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

pub async fn discover_oauth_metadata(base_url: &str) -> Result<OAuthMetadata, McplugError> {
    let url = format!(
        "{}/.well-known/oauth-authorization-server",
        base_url.trim_end_matches('/')
    );
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
