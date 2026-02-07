use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use crate::error::McplugError;

pub async fn listen_for_callback(port: u16, timeout: Duration) -> Result<String, McplugError> {
    let listener = TcpListener::bind(format!("127.0.0.1:{port}")).await?;

    let accept_future = async {
        let (mut stream, _) = listener.accept().await?;

        let mut buf = vec![0u8; 4096];
        let n = stream.read(&mut buf).await?;
        let request = String::from_utf8_lossy(&buf[..n]);

        // Parse the GET request line to extract the code parameter
        let code = parse_code_from_request(&request).ok_or_else(|| {
            McplugError::OAuthError(
                "No authorization code found in callback request".to_string(),
            )
        })?;

        // Send a simple HTML response
        let body = "<!DOCTYPE html><html><body><h1>Authentication successful!</h1>\
                     <p>You can close this window and return to the terminal.</p></body></html>";
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        stream.write_all(response.as_bytes()).await?;
        stream.shutdown().await?;

        Ok::<String, McplugError>(code)
    };

    tokio::time::timeout(timeout, accept_future)
        .await
        .map_err(|_| {
            McplugError::OAuthError(format!(
                "Timed out waiting for OAuth callback after {}s",
                timeout.as_secs()
            ))
        })?
}

fn parse_code_from_request(request: &str) -> Option<String> {
    // Extract the request path from "GET /callback?code=... HTTP/1.1"
    let first_line = request.lines().next()?;
    let path = first_line.split_whitespace().nth(1)?;
    let query = path.split('?').nth(1)?;

    for param in query.split('&') {
        if let Some(value) = param.strip_prefix("code=") {
            let decoded = urldecode(value);
            if !decoded.is_empty() {
                return Some(decoded);
            }
        }
    }
    None
}

fn urldecode(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.bytes();
    while let Some(b) = chars.next() {
        if b == b'%' {
            let hi = chars.next();
            let lo = chars.next();
            if let (Some(h), Some(l)) = (hi, lo) {
                let hex = [h, l];
                if let Ok(s) = std::str::from_utf8(&hex) {
                    if let Ok(val) = u8::from_str_radix(s, 16) {
                        result.push(val as char);
                        continue;
                    }
                }
            }
            result.push('%');
        } else if b == b'+' {
            result.push(' ');
        } else {
            result.push(b as char);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_code_from_valid_request() {
        let request = "GET /callback?code=abc123&state=xyz HTTP/1.1\r\nHost: localhost\r\n";
        assert_eq!(parse_code_from_request(request), Some("abc123".into()));
    }

    #[test]
    fn parse_code_missing() {
        let request = "GET /callback?state=xyz HTTP/1.1\r\nHost: localhost\r\n";
        assert_eq!(parse_code_from_request(request), None);
    }

    #[test]
    fn parse_code_urlencoded() {
        let request = "GET /callback?code=abc%20123 HTTP/1.1\r\nHost: localhost\r\n";
        assert_eq!(parse_code_from_request(request), Some("abc 123".into()));
    }

    #[test]
    fn urldecode_basic() {
        assert_eq!(urldecode("hello%20world"), "hello world");
        assert_eq!(urldecode("a+b"), "a b");
        assert_eq!(urldecode("plain"), "plain");
    }

    #[test]
    fn parse_code_with_error_param() {
        // When an error parameter is present but no code, should return None
        let request =
            "GET /callback?error=access_denied&state=xyz HTTP/1.1\r\nHost: localhost\r\n";
        assert_eq!(parse_code_from_request(request), None);
    }

    #[test]
    fn parse_code_empty_code_value() {
        // code= with empty value should return None
        let request = "GET /callback?code=&state=xyz HTTP/1.1\r\nHost: localhost\r\n";
        assert_eq!(parse_code_from_request(request), None);
    }
}
