use serde::{Deserialize, Serialize};

use crate::error::McplugError;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TokenData {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
    pub token_type: String,
}

impl TokenData {
    pub fn is_expired(&self) -> bool {
        match self.expires_at {
            Some(expires) => chrono::Utc::now() >= expires,
            None => false,
        }
    }
}

/// Raw token response from the OAuth server.
#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: Option<String>,
    expires_in: Option<i64>,
    token_type: String,
}

impl TokenResponse {
    fn into_token_data(self) -> TokenData {
        let expires_at = self
            .expires_in
            .map(|secs| chrono::Utc::now() + chrono::Duration::seconds(secs));
        TokenData {
            access_token: self.access_token,
            refresh_token: self.refresh_token,
            expires_at,
            token_type: self.token_type,
        }
    }
}

pub async fn exchange_code(
    token_endpoint: &str,
    code: &str,
    code_verifier: &str,
    redirect_uri: &str,
    client_id: &str,
) -> Result<TokenData, McplugError> {
    let client = reqwest::Client::new();
    let resp = client
        .post(token_endpoint)
        .form(&[
            ("grant_type", "authorization_code"),
            ("code", code),
            ("code_verifier", code_verifier),
            ("redirect_uri", redirect_uri),
            ("client_id", client_id),
        ])
        .send()
        .await
        .map_err(|e| McplugError::OAuthError(format!("Token exchange request failed: {e}")))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(McplugError::OAuthError(format!(
            "Token exchange failed with status {status}: {body}"
        )));
    }

    let token_resp: TokenResponse = resp.json().await.map_err(|e| {
        McplugError::OAuthError(format!("Failed to parse token response: {e}"))
    })?;

    Ok(token_resp.into_token_data())
}

pub async fn refresh_token(
    token_endpoint: &str,
    refresh_tok: &str,
    client_id: &str,
) -> Result<TokenData, McplugError> {
    let client = reqwest::Client::new();
    let resp = client
        .post(token_endpoint)
        .form(&[
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_tok),
            ("client_id", client_id),
        ])
        .send()
        .await
        .map_err(|e| McplugError::OAuthError(format!("Token refresh request failed: {e}")))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(McplugError::OAuthError(format!(
            "Token refresh failed with status {status}: {body}"
        )));
    }

    let token_resp: TokenResponse = resp.json().await.map_err(|e| {
        McplugError::OAuthError(format!("Failed to parse refresh token response: {e}"))
    })?;

    Ok(token_resp.into_token_data())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_data_serialization_roundtrip() {
        let token = TokenData {
            access_token: "access123".into(),
            refresh_token: Some("refresh456".into()),
            expires_at: Some(chrono::Utc::now() + chrono::Duration::hours(1)),
            token_type: "Bearer".into(),
        };

        let json = serde_json::to_string(&token).unwrap();
        let deserialized: TokenData = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.access_token, "access123");
        assert_eq!(deserialized.refresh_token.as_deref(), Some("refresh456"));
        assert_eq!(deserialized.token_type, "Bearer");
        assert!(deserialized.expires_at.is_some());
    }

    #[test]
    fn token_data_without_optional_fields() {
        let token = TokenData {
            access_token: "access123".into(),
            refresh_token: None,
            expires_at: None,
            token_type: "Bearer".into(),
        };

        let json = serde_json::to_string(&token).unwrap();
        let deserialized: TokenData = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.access_token, "access123");
        assert!(deserialized.refresh_token.is_none());
        assert!(deserialized.expires_at.is_none());
    }

    #[test]
    fn token_not_expired_when_no_expiry() {
        let token = TokenData {
            access_token: "a".into(),
            refresh_token: None,
            expires_at: None,
            token_type: "Bearer".into(),
        };
        assert!(!token.is_expired());
    }

    #[test]
    fn token_not_expired_when_future() {
        let token = TokenData {
            access_token: "a".into(),
            refresh_token: None,
            expires_at: Some(chrono::Utc::now() + chrono::Duration::hours(1)),
            token_type: "Bearer".into(),
        };
        assert!(!token.is_expired());
    }

    #[test]
    fn token_expired_when_past() {
        let token = TokenData {
            access_token: "a".into(),
            refresh_token: None,
            expires_at: Some(chrono::Utc::now() - chrono::Duration::hours(1)),
            token_type: "Bearer".into(),
        };
        assert!(token.is_expired());
    }
}
