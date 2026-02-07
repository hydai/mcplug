use std::time::Duration;

use crate::error::McplugError;
use crate::oauth::cache::{load_cached_token, save_token};
use crate::oauth::callback::listen_for_callback;
use crate::oauth::discovery::discover_oauth_metadata;
use crate::oauth::pkce::generate_pkce;
use crate::oauth::token::{exchange_code, refresh_token, TokenData};

/// Run the full OAuth browser flow for a given server.
pub async fn run_oauth_flow(
    base_url: &str,
    server_name: &str,
    timeout: Duration,
) -> Result<TokenData, McplugError> {
    // 1. Discover OAuth metadata
    let metadata = discover_oauth_metadata(base_url).await?;

    // 2. Generate PKCE challenge
    let pkce = generate_pkce();

    // 3. Find a free port and build redirect URI
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let port = listener.local_addr()?.port();
    drop(listener); // Release the port so the callback server can bind to it
    let redirect_uri = format!("http://localhost:{port}/callback");

    // 4. Build authorization URL and open browser
    let client_id = "mcplug";
    let auth_url = format!(
        "{}?response_type=code&client_id={}&redirect_uri={}&code_challenge={}&code_challenge_method=S256",
        metadata.authorization_endpoint,
        client_id,
        urlencoded(&redirect_uri),
        pkce.code_challenge,
    );

    if webbrowser::open(&auth_url).is_err() {
        tracing::warn!("Could not open browser automatically. Please visit:\n{auth_url}");
    }

    // 5. Listen for callback
    let code = listen_for_callback(port, timeout).await?;

    // 6. Exchange code for tokens
    let token = exchange_code(
        &metadata.token_endpoint,
        &code,
        &pkce.code_verifier,
        &redirect_uri,
        client_id,
    )
    .await?;

    // 7. Cache tokens
    save_token(server_name, &token)?;

    Ok(token)
}

/// Get a valid token for a server, using cache and refresh if possible.
pub async fn get_valid_token(
    server_name: &str,
    base_url: &str,
) -> Result<TokenData, McplugError> {
    // Load cached token
    if let Some(token) = load_cached_token(server_name) {
        if !token.is_expired() {
            return Ok(token);
        }

        // Try to refresh if we have a refresh token
        if let Some(ref refresh_tok) = token.refresh_token {
            let metadata = discover_oauth_metadata(base_url).await?;
            match refresh_token(&metadata.token_endpoint, refresh_tok, "mcplug").await {
                Ok(new_token) => {
                    save_token(server_name, &new_token)?;
                    return Ok(new_token);
                }
                Err(e) => {
                    tracing::debug!("Token refresh failed: {e}");
                }
            }
        }
    }

    Err(McplugError::AuthRequired(server_name.to_string()))
}

fn urlencoded(s: &str) -> String {
    let mut result = String::with_capacity(s.len() * 2);
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                result.push(b as char);
            }
            _ => {
                result.push('%');
                result.push_str(&format!("{b:02X}"));
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urlencoded_basic_characters_pass_through() {
        // Unreserved characters per RFC 3986 should pass through unchanged
        assert_eq!(urlencoded("abcXYZ"), "abcXYZ");
        assert_eq!(urlencoded("0123456789"), "0123456789");
        assert_eq!(urlencoded("-_.~"), "-_.~");
    }

    #[test]
    fn urlencoded_encodes_special_characters() {
        assert_eq!(urlencoded("hello world"), "hello%20world");
        assert_eq!(urlencoded("a&b=c"), "a%26b%3Dc");
        assert_eq!(urlencoded("foo@bar"), "foo%40bar");
    }

    #[test]
    fn urlencoded_encodes_slash() {
        assert_eq!(urlencoded("/"), "%2F");
        assert_eq!(urlencoded("http://localhost:8080/callback"), "http%3A%2F%2Flocalhost%3A8080%2Fcallback");
    }

    #[test]
    fn urlencoded_empty_string() {
        assert_eq!(urlencoded(""), "");
    }

    #[tokio::test]
    async fn get_valid_token_returns_auth_required_when_no_cache() {
        // With no cached token, get_valid_token should return AuthRequired
        let result = get_valid_token("nonexistent-server-xyz-test", "https://example.com").await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, McplugError::AuthRequired(_)));
        assert_eq!(err.code(), "auth_required");
    }
}
