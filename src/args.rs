use crate::McplugError;
use serde_json::{json, Map, Value};

/// Parse a `server.tool` reference, returning (server, tool).
/// The server name is everything before the first dot; the tool name is everything after.
pub fn parse_tool_ref(input: &str) -> Result<(String, String), McplugError> {
    let dot = input.find('.').ok_or_else(|| {
        McplugError::ProtocolError(format!(
            "Invalid tool reference '{input}': expected 'server.tool' format"
        ))
    })?;
    let server = &input[..dot];
    let tool = &input[dot + 1..];
    if server.is_empty() || tool.is_empty() {
        return Err(McplugError::ProtocolError(format!(
            "Invalid tool reference '{input}': server and tool names must be non-empty"
        )));
    }
    Ok((server.to_string(), tool.to_string()))
}

/// Parse CLI arguments into a JSON value.
///
/// Supports `key:value` and `key=value` formats. Values are auto-coerced:
/// - Quoted strings have quotes stripped
/// - `true`/`false` become booleans
/// - Valid numbers become JSON numbers
/// - Everything else stays a string
pub fn parse_args(args: &[String]) -> Result<Value, McplugError> {
    if args.is_empty() {
        return Ok(json!({}));
    }

    let mut map = Map::new();
    for arg in args {
        // Try colon-delimited first, then equals
        let (key, raw_value) = if let Some(pos) = arg.find(':') {
            (&arg[..pos], &arg[pos + 1..])
        } else if let Some(pos) = arg.find('=') {
            (&arg[..pos], &arg[pos + 1..])
        } else {
            return Err(McplugError::ProtocolError(format!(
                "Cannot parse argument '{arg}': expected 'key:value' or 'key=value'"
            )));
        };

        if key.is_empty() {
            return Err(McplugError::ProtocolError(format!(
                "Empty key in argument '{arg}'"
            )));
        }

        let value = coerce_value(raw_value);
        map.insert(key.to_string(), value);
    }

    Ok(Value::Object(map))
}

/// Parse function-call syntax: `server.tool(args)` into (server, tool, args).
///
/// Supports:
/// - Named args: `server.tool(key: "value", count: 42)`
/// - Positional args: `server.tool("value", 42)` → returned as a JSON array
/// - No args: `server.tool()` → empty object
pub fn parse_function_call(input: &str) -> Result<(String, String, Value), McplugError> {
    let paren_open = input.find('(').ok_or_else(|| {
        McplugError::ProtocolError(format!(
            "Invalid function call '{input}': missing '('"
        ))
    })?;

    if !input.ends_with(')') {
        return Err(McplugError::ProtocolError(format!(
            "Invalid function call '{input}': missing closing ')'"
        )));
    }

    let ref_part = &input[..paren_open];
    let (server, tool) = parse_tool_ref(ref_part)?;

    let args_str = &input[paren_open + 1..input.len() - 1].trim();
    if args_str.is_empty() {
        return Ok((server, tool, json!({})));
    }

    // Determine if named or positional by looking for `key:` pattern before a value
    let args = parse_inner_args(args_str)?;
    Ok((server, tool, args))
}

/// Suggest a tool name if the given name is close to a known tool (Levenshtein <= 2).
/// Returns None if no match or if multiple tools are equally close.
pub fn suggest_tool(input: &str, known_tools: &[&str]) -> Option<String> {
    let mut best_dist = usize::MAX;
    let mut best_tool: Option<&str> = None;
    let mut ambiguous = false;

    for &tool in known_tools {
        let dist = strsim::levenshtein(input, tool);
        if dist < best_dist {
            best_dist = dist;
            best_tool = Some(tool);
            ambiguous = false;
        } else if dist == best_dist {
            ambiguous = true;
        }
    }

    if best_dist <= 2 && !ambiguous {
        best_tool.map(|t| t.to_string())
    } else {
        None
    }
}

/// Coerce a raw string value into a JSON value.
fn coerce_value(raw: &str) -> Value {
    // Strip surrounding quotes
    if ((raw.starts_with('"') && raw.ends_with('"'))
        || (raw.starts_with('\'') && raw.ends_with('\'')))
        && raw.len() >= 2
    {
        return Value::String(raw[1..raw.len() - 1].to_string());
    }

    // Boolean
    if raw.eq_ignore_ascii_case("true") {
        return Value::Bool(true);
    }
    if raw.eq_ignore_ascii_case("false") {
        return Value::Bool(false);
    }

    // Null
    if raw == "null" {
        return Value::Null;
    }

    // Integer
    if let Ok(n) = raw.parse::<i64>() {
        return json!(n);
    }

    // Float
    if let Ok(f) = raw.parse::<f64>() {
        return json!(f);
    }

    // Try JSON object/array
    if (raw.starts_with('{') && raw.ends_with('}'))
        || (raw.starts_with('[') && raw.ends_with(']'))
    {
        if let Ok(v) = serde_json::from_str::<Value>(raw) {
            return v;
        }
    }

    // Fall back to string
    Value::String(raw.to_string())
}

/// Parse the inner arguments of a function call (the part between parentheses).
/// Returns a JSON object for named args or a JSON array for positional args.
fn parse_inner_args(input: &str) -> Result<Value, McplugError> {
    let parts = split_args(input)?;
    if parts.is_empty() {
        return Ok(json!({}));
    }

    // Check if the first part is a named argument (contains `:` outside of quotes)
    let is_named = parts.iter().any(|p| {
        let trimmed = p.trim();
        has_named_separator(trimmed)
    });

    if is_named {
        let mut map = Map::new();
        for part in &parts {
            let trimmed = part.trim();
            let colon_pos = find_named_separator(trimmed).ok_or_else(|| {
                McplugError::ProtocolError(format!(
                    "Mixed named and positional arguments: '{trimmed}'"
                ))
            })?;
            let key = trimmed[..colon_pos].trim();
            let val_str = trimmed[colon_pos + 1..].trim();
            map.insert(key.to_string(), parse_inner_value(val_str)?);
        }
        Ok(Value::Object(map))
    } else {
        let mut arr = Vec::new();
        for part in &parts {
            let trimmed = part.trim();
            arr.push(parse_inner_value(trimmed)?);
        }
        Ok(Value::Array(arr))
    }
}

/// Parse a single value inside function-call arguments.
fn parse_inner_value(input: &str) -> Result<Value, McplugError> {
    let input = input.trim();

    // Quoted string
    if ((input.starts_with('"') && input.ends_with('"'))
        || (input.starts_with('\'') && input.ends_with('\'')))
        && input.len() >= 2
    {
        return Ok(Value::String(input[1..input.len() - 1].to_string()));
    }

    // Boolean
    if input.eq_ignore_ascii_case("true") {
        return Ok(Value::Bool(true));
    }
    if input.eq_ignore_ascii_case("false") {
        return Ok(Value::Bool(false));
    }

    // Null
    if input == "null" {
        return Ok(Value::Null);
    }

    // Integer
    if let Ok(n) = input.parse::<i64>() {
        return Ok(json!(n));
    }

    // Float
    if let Ok(f) = input.parse::<f64>() {
        return Ok(json!(f));
    }

    // JSON object or array
    if (input.starts_with('{') && input.ends_with('}'))
        || (input.starts_with('[') && input.ends_with(']'))
    {
        return serde_json::from_str::<Value>(input).map_err(|e| {
            McplugError::ProtocolError(format!("Invalid JSON in argument: {e}"))
        });
    }

    // Bare string
    Ok(Value::String(input.to_string()))
}

/// Split a comma-separated argument string, respecting quotes, braces, and brackets.
fn split_args(input: &str) -> Result<Vec<String>, McplugError> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut in_quote: Option<char> = None;
    let mut depth = 0i32; // tracks {}, []

    for ch in input.chars() {
        match in_quote {
            Some(q) => {
                current.push(ch);
                if ch == q {
                    in_quote = None;
                }
            }
            None => match ch {
                '"' | '\'' => {
                    in_quote = Some(ch);
                    current.push(ch);
                }
                '{' | '[' => {
                    depth += 1;
                    current.push(ch);
                }
                '}' | ']' => {
                    depth -= 1;
                    current.push(ch);
                }
                ',' if depth == 0 => {
                    parts.push(current.clone());
                    current.clear();
                }
                _ => {
                    current.push(ch);
                }
            },
        }
    }

    if !current.trim().is_empty() {
        parts.push(current);
    }

    Ok(parts)
}

/// Check if a string has a named-argument separator (`:` not inside quotes and not part of a URL/nested).
fn has_named_separator(input: &str) -> bool {
    find_named_separator(input).is_some()
}

/// Find the position of the named-argument separator `:` (not inside quotes).
/// We look for an unquoted `:` that is preceded by a bare identifier (no quotes before it).
fn find_named_separator(input: &str) -> Option<usize> {
    let mut in_quote: Option<char> = None;
    for (i, ch) in input.char_indices() {
        match in_quote {
            Some(q) => {
                if ch == q {
                    in_quote = None;
                }
            }
            None => match ch {
                '"' | '\'' => {
                    // If we hit a quote before finding `:`, this is a positional string arg
                    if i == 0 {
                        return None;
                    }
                    in_quote = Some(ch);
                }
                ':' => {
                    // Only treat as separator if the part before is a plain identifier
                    let key = input[..i].trim();
                    if !key.is_empty()
                        && key
                            .chars()
                            .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
                    {
                        return Some(i);
                    }
                }
                _ => {}
            },
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- parse_tool_ref tests ---

    #[test]
    fn parse_tool_ref_basic() {
        let (server, tool) = parse_tool_ref("server.tool").unwrap();
        assert_eq!(server, "server");
        assert_eq!(tool, "tool");
    }

    #[test]
    fn parse_tool_ref_with_dots_in_tool() {
        let (server, tool) = parse_tool_ref("server.tool.extra").unwrap();
        assert_eq!(server, "server");
        assert_eq!(tool, "tool.extra");
    }

    #[test]
    fn parse_tool_ref_no_dot() {
        assert!(parse_tool_ref("nodot").is_err());
    }

    #[test]
    fn parse_tool_ref_empty_server() {
        assert!(parse_tool_ref(".tool").is_err());
    }

    #[test]
    fn parse_tool_ref_empty_tool() {
        assert!(parse_tool_ref("server.").is_err());
    }

    // --- parse_args tests ---

    #[test]
    fn parse_args_colon_string() {
        let result = parse_args(&["key:value".to_string()]).unwrap();
        assert_eq!(result, json!({"key": "value"}));
    }

    #[test]
    fn parse_args_equals_string() {
        let result = parse_args(&["key=value".to_string()]).unwrap();
        assert_eq!(result, json!({"key": "value"}));
    }

    #[test]
    fn parse_args_colon_number() {
        let result = parse_args(&["count:42".to_string()]).unwrap();
        assert_eq!(result, json!({"count": 42}));
    }

    #[test]
    fn parse_args_colon_float() {
        let result = parse_args(&["rate:3.14".to_string()]).unwrap();
        #[allow(clippy::approx_constant)]
        let expected = json!({"rate": 3.14_f64});
        assert_eq!(result, expected);
    }

    #[test]
    fn parse_args_colon_bool() {
        let result = parse_args(&["flag:true".to_string()]).unwrap();
        assert_eq!(result, json!({"flag": true}));
    }

    #[test]
    fn parse_args_colon_bool_false() {
        let result = parse_args(&["flag:false".to_string()]).unwrap();
        assert_eq!(result, json!({"flag": false}));
    }

    #[test]
    fn parse_args_multiple() {
        let result = parse_args(&[
            "url:https://example.com".to_string(),
            "depth:3".to_string(),
            "verbose:true".to_string(),
        ])
        .unwrap();
        assert_eq!(
            result,
            json!({"url": "https://example.com", "depth": 3, "verbose": true})
        );
    }

    #[test]
    fn parse_args_quoted_value() {
        let result = parse_args(&["name:\"hello world\"".to_string()]).unwrap();
        assert_eq!(result, json!({"name": "hello world"}));
    }

    #[test]
    fn parse_args_empty() {
        let result = parse_args(&[]).unwrap();
        assert_eq!(result, json!({}));
    }

    #[test]
    fn parse_args_no_separator() {
        assert!(parse_args(&["notseparated".to_string()]).is_err());
    }

    #[test]
    fn parse_args_value_with_colon() {
        // Value is everything after the first colon
        let result = parse_args(&["url:http://example.com:8080".to_string()]).unwrap();
        assert_eq!(result, json!({"url": "http://example.com:8080"}));
    }

    // --- parse_function_call tests ---

    #[test]
    fn parse_function_call_named_args() {
        let (server, tool, args) =
            parse_function_call("server.tool(key: \"value\")").unwrap();
        assert_eq!(server, "server");
        assert_eq!(tool, "tool");
        assert_eq!(args, json!({"key": "value"}));
    }

    #[test]
    fn parse_function_call_multiple_named() {
        let (_, _, args) =
            parse_function_call("srv.t(name: \"alice\", count: 42, active: true)").unwrap();
        assert_eq!(args, json!({"name": "alice", "count": 42, "active": true}));
    }

    #[test]
    fn parse_function_call_no_args() {
        let (server, tool, args) = parse_function_call("server.tool()").unwrap();
        assert_eq!(server, "server");
        assert_eq!(tool, "tool");
        assert_eq!(args, json!({}));
    }

    #[test]
    fn parse_function_call_positional() {
        let (_, _, args) = parse_function_call("server.tool(\"value\", 42)").unwrap();
        assert_eq!(args, json!(["value", 42]));
    }

    #[test]
    fn parse_function_call_missing_paren() {
        assert!(parse_function_call("server.tool").is_err());
    }

    #[test]
    fn parse_function_call_missing_close_paren() {
        assert!(parse_function_call("server.tool(key: \"value\"").is_err());
    }

    // --- suggest_tool tests ---

    #[test]
    fn suggest_tool_close_match() {
        let result = suggest_tool("serch", &["search", "crawl"]);
        assert_eq!(result, Some("search".to_string()));
    }

    #[test]
    fn suggest_tool_no_match() {
        let result = suggest_tool("xyz", &["search", "crawl"]);
        assert_eq!(result, None);
    }

    #[test]
    fn suggest_tool_distance_two() {
        // "sear" vs "search" = distance 2 (missing "ch"), should suggest
        let result = suggest_tool("sear", &["search", "crawl"]);
        assert_eq!(result, Some("search".to_string()));
    }

    #[test]
    fn suggest_tool_exact_match() {
        let result = suggest_tool("search", &["search", "crawl"]);
        assert_eq!(result, Some("search".to_string()));
    }

    #[test]
    fn suggest_tool_ambiguous() {
        // Both are distance 1
        let result = suggest_tool("ab", &["aa", "ac"]);
        assert_eq!(result, None);
    }

    #[test]
    fn suggest_tool_empty_list() {
        let result = suggest_tool("search", &[]);
        assert_eq!(result, None);
    }

    // --- P2/P3 additional tests ---

    #[test]
    fn parse_args_null_coercion() {
        let result = parse_args(&["key:null".to_string()]).unwrap();
        assert_eq!(result, json!({"key": null}));
    }

    #[test]
    fn parse_args_json_object_inline() {
        let result = parse_args(&[r#"key:{"nested":"val"}"#.to_string()]).unwrap();
        assert_eq!(result, json!({"key": {"nested": "val"}}));
    }

    #[test]
    fn parse_args_json_array_inline() {
        let result = parse_args(&["key:[1,2,3]".to_string()]).unwrap();
        assert_eq!(result, json!({"key": [1, 2, 3]}));
    }

    #[test]
    fn parse_args_empty_quoted_string() {
        let result = parse_args(&[r#"key:"""#.to_string()]).unwrap();
        assert_eq!(result, json!({"key": ""}));
    }

    #[test]
    fn parse_args_single_quote_stripping() {
        let result = parse_args(&["key:'hello'".to_string()]).unwrap();
        assert_eq!(result, json!({"key": "hello"}));
    }

    #[test]
    fn parse_function_call_nested_json_arg() {
        let (server, tool, args) =
            parse_function_call(r#"srv.tool(data: {"a": [1, 2]})"#).unwrap();
        assert_eq!(server, "srv");
        assert_eq!(tool, "tool");
        assert_eq!(args, json!({"data": {"a": [1, 2]}}));
    }
}
