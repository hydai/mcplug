use colored::Colorize;

use crate::error::McplugError;
use crate::types::{CallResult, ContentBlock};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputMode {
    Pretty,
    Raw,
    Json,
}

pub fn print_call_result(result: &CallResult, mode: OutputMode, is_tty: bool) {
    match mode {
        OutputMode::Json => {
            let json = serde_json::json!({
                "content": result.content,
                "isError": result.is_error,
            });
            println!("{}", serde_json::to_string_pretty(&json).unwrap_or_default());
        }
        OutputMode::Raw => {
            print!("{}", result.text());
        }
        OutputMode::Pretty => {
            if result.is_error {
                let label = if is_tty {
                    "Error".red().bold().to_string()
                } else {
                    "Error".to_string()
                };
                eprintln!("{}: {}", label, result.text());
            } else {
                for block in &result.content {
                    match block {
                        ContentBlock::Text { text } => {
                            println!("{}", text);
                        }
                        ContentBlock::Image { mime_type, .. } => {
                            let msg = format!("[image: {}]", mime_type);
                            if is_tty {
                                println!("{}", msg.dimmed());
                            } else {
                                println!("{}", msg);
                            }
                        }
                        ContentBlock::Resource { uri, text } => {
                            if is_tty {
                                println!("{}", uri.underline());
                            } else {
                                println!("{}", uri);
                            }
                            println!("{}", text);
                        }
                    }
                }
            }
        }
    }
}

pub fn print_error(err: &McplugError, json_mode: bool) {
    if json_mode {
        println!("{}", serde_json::to_string_pretty(&err.to_json()).unwrap_or_default());
    } else {
        eprintln!("{}", err);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_mode_equality() {
        assert_eq!(OutputMode::Pretty, OutputMode::Pretty);
        assert_ne!(OutputMode::Pretty, OutputMode::Raw);
        assert_ne!(OutputMode::Raw, OutputMode::Json);
    }

    #[test]
    fn print_call_result_json_captures_content() {
        let result = CallResult {
            content: vec![ContentBlock::Text {
                text: "hello".into(),
            }],
            is_error: false,
            raw_response: None,
        };
        // Just ensure it doesn't panic
        print_call_result(&result, OutputMode::Json, false);
    }

    #[test]
    fn print_call_result_raw_text() {
        let result = CallResult {
            content: vec![ContentBlock::Text {
                text: "raw output".into(),
            }],
            is_error: false,
            raw_response: None,
        };
        print_call_result(&result, OutputMode::Raw, false);
    }

    #[test]
    fn print_call_result_pretty_text() {
        let result = CallResult {
            content: vec![ContentBlock::Text {
                text: "pretty output".into(),
            }],
            is_error: false,
            raw_response: None,
        };
        print_call_result(&result, OutputMode::Pretty, false);
    }

    #[test]
    fn print_call_result_pretty_error() {
        let result = CallResult {
            content: vec![ContentBlock::Text {
                text: "something failed".into(),
            }],
            is_error: true,
            raw_response: None,
        };
        print_call_result(&result, OutputMode::Pretty, true);
    }

    #[test]
    fn print_call_result_pretty_image() {
        let result = CallResult {
            content: vec![ContentBlock::Image {
                data: "base64data".into(),
                mime_type: "image/png".into(),
            }],
            is_error: false,
            raw_response: None,
        };
        print_call_result(&result, OutputMode::Pretty, true);
    }

    #[test]
    fn print_call_result_pretty_resource() {
        let result = CallResult {
            content: vec![ContentBlock::Resource {
                uri: "file://test".into(),
                text: "content".into(),
            }],
            is_error: false,
            raw_response: None,
        };
        print_call_result(&result, OutputMode::Pretty, true);
    }

    #[test]
    fn print_error_json_mode() {
        let err = McplugError::ServerNotFound("test".into());
        print_error(&err, true);
    }

    #[test]
    fn print_error_human_mode() {
        let err = McplugError::ServerNotFound("test".into());
        print_error(&err, false);
    }

    // --- P2/P3 additional tests ---

    #[test]
    fn print_call_result_json_error_content() {
        let result = CallResult {
            content: vec![ContentBlock::Text {
                text: "something went wrong".into(),
            }],
            is_error: true,
            raw_response: None,
        };
        // JSON mode should output valid JSON with isError: true (doesn't panic)
        print_call_result(&result, OutputMode::Json, false);
    }

    #[test]
    fn print_call_result_mixed_content_blocks() {
        let result = CallResult {
            content: vec![
                ContentBlock::Text {
                    text: "Hello".into(),
                },
                ContentBlock::Image {
                    data: "base64data".into(),
                    mime_type: "image/jpeg".into(),
                },
                ContentBlock::Resource {
                    uri: "file://doc.txt".into(),
                    text: "Document content".into(),
                },
            ],
            is_error: false,
            raw_response: None,
        };
        // Pretty mode should handle all three block types without panic
        print_call_result(&result, OutputMode::Pretty, false);
    }

    #[test]
    fn print_call_result_empty_content() {
        let result = CallResult {
            content: vec![],
            is_error: false,
            raw_response: None,
        };
        // Empty content should be handled gracefully in all modes
        print_call_result(&result, OutputMode::Pretty, false);
        print_call_result(&result, OutputMode::Json, false);
        print_call_result(&result, OutputMode::Raw, false);
    }
}
