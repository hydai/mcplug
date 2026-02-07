use serde::{Deserialize, Serialize};

use crate::error::McplugError;

/// Information about an MCP server returned during initialization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerInfo {
    pub name: String,
    pub version: String,
    pub capabilities: serde_json::Value,
}

/// A tool definition exposed by an MCP server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

/// A single content block returned by a tool call.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ContentBlock {
    Text { text: String },
    Image { data: String, mime_type: String },
    Resource { uri: String, text: String },
}

/// The result of calling an MCP tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallResult {
    pub content: Vec<ContentBlock>,
    #[serde(rename = "isError", default)]
    pub is_error: bool,
    /// The full raw MCP response envelope, if available.
    #[serde(skip)]
    pub raw_response: Option<serde_json::Value>,
}

impl CallResult {
    /// Extract plain text from all text content blocks, joined by newlines.
    pub fn text(&self) -> String {
        self.content
            .iter()
            .filter_map(|block| match block {
                ContentBlock::Text { text } => Some(text.as_str()),
                ContentBlock::Resource { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Deserialize the text content as a typed value.
    pub fn json<T: serde::de::DeserializeOwned>(&self) -> Result<T, McplugError> {
        let text = self.text();
        serde_json::from_str(&text).map_err(|e| {
            McplugError::ProtocolError(format!("Failed to deserialize response: {e}"))
        })
    }

    /// Format content blocks as markdown.
    pub fn markdown(&self) -> String {
        self.content
            .iter()
            .map(|block| match block {
                ContentBlock::Text { text } => text.clone(),
                ContentBlock::Image { data, mime_type } => {
                    format!("![image](data:{mime_type};base64,{data})")
                }
                ContentBlock::Resource { uri, text } => {
                    format!("[{uri}]({uri})\n\n{text}")
                }
            })
            .collect::<Vec<_>>()
            .join("\n\n")
    }

    /// Return the raw content blocks.
    pub fn content(&self) -> &[ContentBlock] {
        &self.content
    }

    /// Return the full raw MCP response envelope.
    pub fn raw(&self) -> Option<&serde_json::Value> {
        self.raw_response.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_text_result(texts: &[&str]) -> CallResult {
        CallResult {
            content: texts
                .iter()
                .map(|t| ContentBlock::Text {
                    text: t.to_string(),
                })
                .collect(),
            is_error: false,
            raw_response: None,
        }
    }

    #[test]
    fn text_joins_text_blocks() {
        let result = make_text_result(&["hello", "world"]);
        assert_eq!(result.text(), "hello\nworld");
    }

    #[test]
    fn text_single_block() {
        let result = make_text_result(&["single"]);
        assert_eq!(result.text(), "single");
    }

    #[test]
    fn text_empty() {
        let result = CallResult {
            content: vec![],
            is_error: false,
            raw_response: None,
        };
        assert_eq!(result.text(), "");
    }

    #[test]
    fn text_includes_resource_text() {
        let result = CallResult {
            content: vec![
                ContentBlock::Text {
                    text: "intro".into(),
                },
                ContentBlock::Resource {
                    uri: "file://x".into(),
                    text: "resource content".into(),
                },
            ],
            is_error: false,
            raw_response: None,
        };
        assert_eq!(result.text(), "intro\nresource content");
    }

    #[test]
    fn text_skips_images() {
        let result = CallResult {
            content: vec![
                ContentBlock::Text {
                    text: "before".into(),
                },
                ContentBlock::Image {
                    data: "abc".into(),
                    mime_type: "image/png".into(),
                },
                ContentBlock::Text {
                    text: "after".into(),
                },
            ],
            is_error: false,
            raw_response: None,
        };
        assert_eq!(result.text(), "before\nafter");
    }

    #[test]
    fn json_deserialize() {
        let result = make_text_result(&[r#"{"key":"value"}"#]);
        let parsed: serde_json::Value = result.json().unwrap();
        assert_eq!(parsed, serde_json::json!({"key": "value"}));
    }

    #[test]
    fn json_deserialize_error() {
        let result = make_text_result(&["not json"]);
        let err = result.json::<serde_json::Value>().unwrap_err();
        assert!(err.to_string().contains("Failed to deserialize"));
    }

    #[test]
    fn markdown_formats_text() {
        let result = make_text_result(&["# Title", "body"]);
        assert_eq!(result.markdown(), "# Title\n\nbody");
    }

    #[test]
    fn markdown_formats_image() {
        let result = CallResult {
            content: vec![ContentBlock::Image {
                data: "abc123".into(),
                mime_type: "image/png".into(),
            }],
            is_error: false,
            raw_response: None,
        };
        assert_eq!(
            result.markdown(),
            "![image](data:image/png;base64,abc123)"
        );
    }

    #[test]
    fn markdown_formats_resource() {
        let result = CallResult {
            content: vec![ContentBlock::Resource {
                uri: "https://example.com".into(),
                text: "Example content".into(),
            }],
            is_error: false,
            raw_response: None,
        };
        assert_eq!(
            result.markdown(),
            "[https://example.com](https://example.com)\n\nExample content"
        );
    }

    #[test]
    fn content_returns_blocks() {
        let result = make_text_result(&["a", "b"]);
        assert_eq!(result.content().len(), 2);
    }

    #[test]
    fn raw_returns_none_when_unset() {
        let result = make_text_result(&["x"]);
        assert!(result.raw().is_none());
    }

    #[test]
    fn raw_returns_value_when_set() {
        let result = CallResult {
            content: vec![],
            is_error: false,
            raw_response: Some(serde_json::json!({"jsonrpc": "2.0", "result": {}})),
        };
        assert!(result.raw().is_some());
    }
}
