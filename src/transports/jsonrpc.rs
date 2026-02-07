use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Debug, Serialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: u64,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

/// A JSON-RPC 2.0 notification (no id field).
#[derive(Debug, Serialize)]
pub struct JsonRpcNotification {
    pub jsonrpc: String,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct JsonRpcResponse {
    #[allow(dead_code)]
    pub jsonrpc: String,
    pub id: Option<u64>,
    pub result: Option<serde_json::Value>,
    pub error: Option<JsonRpcError>,
}

#[derive(Debug, Deserialize)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
    pub data: Option<serde_json::Value>,
}

/// Helper that generates JSON-RPC requests with auto-incrementing IDs.
pub struct RequestBuilder {
    next_id: AtomicU64,
}

impl RequestBuilder {
    pub fn new() -> Self {
        Self {
            next_id: AtomicU64::new(1),
        }
    }

    pub fn next_request(
        &self,
        method: &str,
        params: Option<serde_json::Value>,
    ) -> JsonRpcRequest {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id,
            method: method.to_string(),
            params,
        }
    }

    pub fn notification(
        method: &str,
        params: Option<serde_json::Value>,
    ) -> JsonRpcNotification {
        JsonRpcNotification {
            jsonrpc: "2.0".to_string(),
            method: method.to_string(),
            params,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_serialization() {
        let builder = RequestBuilder::new();
        let req = builder.next_request("initialize", Some(serde_json::json!({"key": "value"})));
        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["jsonrpc"], "2.0");
        assert_eq!(json["id"], 1);
        assert_eq!(json["method"], "initialize");
        assert_eq!(json["params"]["key"], "value");
    }

    #[test]
    fn request_ids_auto_increment() {
        let builder = RequestBuilder::new();
        let r1 = builder.next_request("a", None);
        let r2 = builder.next_request("b", None);
        let r3 = builder.next_request("c", None);
        assert_eq!(r1.id, 1);
        assert_eq!(r2.id, 2);
        assert_eq!(r3.id, 3);
    }

    #[test]
    fn request_without_params_omits_field() {
        let builder = RequestBuilder::new();
        let req = builder.next_request("test", None);
        let json_str = serde_json::to_string(&req).unwrap();
        assert!(!json_str.contains("params"));
    }

    #[test]
    fn notification_serialization() {
        let notif =
            RequestBuilder::notification("notifications/initialized", None);
        let json = serde_json::to_value(&notif).unwrap();
        assert_eq!(json["jsonrpc"], "2.0");
        assert_eq!(json["method"], "notifications/initialized");
        assert!(json.get("id").is_none());
    }

    #[test]
    fn response_parsing_success() {
        let raw = r#"{"jsonrpc":"2.0","id":1,"result":{"tools":[]}}"#;
        let resp: JsonRpcResponse = serde_json::from_str(raw).unwrap();
        assert_eq!(resp.id, Some(1));
        assert!(resp.result.is_some());
        assert!(resp.error.is_none());
    }

    #[test]
    fn response_parsing_error() {
        let raw =
            r#"{"jsonrpc":"2.0","id":1,"error":{"code":-32600,"message":"Invalid Request"}}"#;
        let resp: JsonRpcResponse = serde_json::from_str(raw).unwrap();
        assert_eq!(resp.id, Some(1));
        assert!(resp.result.is_none());
        let err = resp.error.unwrap();
        assert_eq!(err.code, -32600);
        assert_eq!(err.message, "Invalid Request");
        assert!(err.data.is_none());
    }

    #[test]
    fn response_parsing_error_with_data() {
        let raw = r#"{"jsonrpc":"2.0","id":2,"error":{"code":-32601,"message":"Method not found","data":"details"}}"#;
        let resp: JsonRpcResponse = serde_json::from_str(raw).unwrap();
        let err = resp.error.unwrap();
        assert_eq!(err.code, -32601);
        assert!(err.data.is_some());
    }

    #[test]
    fn response_parsing_notification_no_id() {
        // Notifications have no id field
        let raw = r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#;
        let resp: JsonRpcResponse = serde_json::from_str(raw).unwrap();
        assert!(resp.id.is_none());
        assert!(resp.result.is_none());
        assert!(resp.error.is_none());
    }

    #[test]
    fn request_builder_starts_at_one() {
        let builder = RequestBuilder::new();
        let first = builder.next_request("test", None);
        assert_eq!(first.id, 1);
    }
}
