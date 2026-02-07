use std::collections::HashMap;

use crate::error::McplugError;

use super::types::ServerConfig;

/// Expand environment variable references in a string.
///
/// Supported syntaxes:
/// - `${VAR}` - replaced with env var value; error if unset
/// - `${VAR:-fallback}` - replaced with env var value, or fallback if unset
/// - `$env:VAR` - same as `${VAR}`
pub fn expand_env_vars(input: &str) -> Result<String, McplugError> {
    let mut result = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch != '$' {
            result.push(ch);
            continue;
        }

        // Check for ${VAR} or ${VAR:-fallback}
        if chars.peek() == Some(&'{') {
            chars.next(); // consume '{'
            let mut var_expr = String::new();
            let mut found_close = false;
            for c in chars.by_ref() {
                if c == '}' {
                    found_close = true;
                    break;
                }
                var_expr.push(c);
            }
            if !found_close {
                return Err(env_error(&format!(
                    "Unclosed variable reference: ${{{}",
                    var_expr
                )));
            }

            // Check for :-fallback syntax
            if let Some(sep_pos) = var_expr.find(":-") {
                let var_name = &var_expr[..sep_pos];
                let fallback = &var_expr[sep_pos + 2..];
                match std::env::var(var_name) {
                    Ok(val) if !val.is_empty() => result.push_str(&val),
                    _ => result.push_str(fallback),
                }
            } else {
                let var_name = &var_expr;
                match std::env::var(var_name) {
                    Ok(val) => result.push_str(&val),
                    Err(_) => {
                        return Err(env_error(&format!(
                            "Environment variable '{}' is not set",
                            var_name
                        )));
                    }
                }
            }
            continue;
        }

        // Check for $env:VAR
        if input[result.len()..].starts_with("$env:") {
            // We already consumed '$', so check remaining starts with "env:"
            let remaining: String = chars.clone().collect();
            if remaining.starts_with("env:") {
                // consume "env:"
                for _ in 0..4 {
                    chars.next();
                }
                // Read var name (alphanumeric + underscore)
                let mut var_name = String::new();
                while let Some(&c) = chars.peek() {
                    if c.is_alphanumeric() || c == '_' {
                        var_name.push(c);
                        chars.next();
                    } else {
                        break;
                    }
                }
                if var_name.is_empty() {
                    return Err(env_error("Empty variable name in $env: reference"));
                }
                match std::env::var(&var_name) {
                    Ok(val) => result.push_str(&val),
                    Err(_) => {
                        return Err(env_error(&format!(
                            "Environment variable '{}' is not set",
                            var_name
                        )));
                    }
                }
                continue;
            }
        }

        // Not a recognized pattern, output the '$' literally
        result.push('$');
    }

    Ok(result)
}

/// Expand environment variables in all string fields of a ServerConfig.
pub fn expand_server_config(config: &mut ServerConfig) -> Result<(), McplugError> {
    if let Some(ref mut url) = config.base_url {
        *url = expand_env_vars(url)?;
    }
    if let Some(ref mut cmd) = config.command {
        *cmd = expand_env_vars(cmd)?;
    }
    for arg in &mut config.args {
        *arg = expand_env_vars(arg)?;
    }
    let expanded_env: HashMap<String, String> = config
        .env
        .iter()
        .map(|(k, v)| Ok((k.clone(), expand_env_vars(v)?)))
        .collect::<Result<_, McplugError>>()?;
    config.env = expanded_env;
    let expanded_headers: HashMap<String, String> = config
        .headers
        .iter()
        .map(|(k, v)| Ok((k.clone(), expand_env_vars(v)?)))
        .collect::<Result<_, McplugError>>()?;
    config.headers = expanded_headers;
    Ok(())
}

fn env_error(detail: &str) -> McplugError {
    McplugError::ConfigError {
        path: std::path::PathBuf::from("<env>"),
        detail: detail.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expand_dollar_brace_var() {
        std::env::set_var("MCPLUG_TEST_VAR1", "hello");
        let result = expand_env_vars("prefix-${MCPLUG_TEST_VAR1}-suffix").unwrap();
        assert_eq!(result, "prefix-hello-suffix");
        std::env::remove_var("MCPLUG_TEST_VAR1");
    }

    #[test]
    fn expand_dollar_brace_unset_errors() {
        std::env::remove_var("MCPLUG_TEST_UNSET_XYZ");
        let err = expand_env_vars("${MCPLUG_TEST_UNSET_XYZ}").unwrap_err();
        assert!(err.to_string().contains("MCPLUG_TEST_UNSET_XYZ"));
        assert!(err.to_string().contains("not set"));
    }

    #[test]
    fn expand_fallback_when_unset() {
        std::env::remove_var("MCPLUG_TEST_FB_UNSET");
        let result = expand_env_vars("${MCPLUG_TEST_FB_UNSET:-default_val}").unwrap();
        assert_eq!(result, "default_val");
    }

    #[test]
    fn expand_fallback_when_set() {
        std::env::set_var("MCPLUG_TEST_FB_SET", "real");
        let result = expand_env_vars("${MCPLUG_TEST_FB_SET:-default_val}").unwrap();
        assert_eq!(result, "real");
        std::env::remove_var("MCPLUG_TEST_FB_SET");
    }

    #[test]
    fn expand_fallback_when_empty() {
        std::env::set_var("MCPLUG_TEST_FB_EMPTY", "");
        let result = expand_env_vars("${MCPLUG_TEST_FB_EMPTY:-fallback}").unwrap();
        assert_eq!(result, "fallback");
        std::env::remove_var("MCPLUG_TEST_FB_EMPTY");
    }

    #[test]
    fn expand_env_colon_var() {
        std::env::set_var("MCPLUG_TEST_ENV_COLON", "envval");
        let result = expand_env_vars("Bearer $env:MCPLUG_TEST_ENV_COLON").unwrap();
        assert_eq!(result, "Bearer envval");
        std::env::remove_var("MCPLUG_TEST_ENV_COLON");
    }

    #[test]
    fn expand_env_colon_unset_errors() {
        std::env::remove_var("MCPLUG_TEST_ENV_COLON_MISSING");
        let err = expand_env_vars("$env:MCPLUG_TEST_ENV_COLON_MISSING").unwrap_err();
        assert!(err.to_string().contains("MCPLUG_TEST_ENV_COLON_MISSING"));
    }

    #[test]
    fn no_expansion_needed() {
        let result = expand_env_vars("plain string with no vars").unwrap();
        assert_eq!(result, "plain string with no vars");
    }

    #[test]
    fn multiple_expansions() {
        std::env::set_var("MCPLUG_TEST_A", "aaa");
        std::env::set_var("MCPLUG_TEST_B", "bbb");
        let result = expand_env_vars("${MCPLUG_TEST_A}/${MCPLUG_TEST_B}").unwrap();
        assert_eq!(result, "aaa/bbb");
        std::env::remove_var("MCPLUG_TEST_A");
        std::env::remove_var("MCPLUG_TEST_B");
    }

    #[test]
    fn expand_server_config_expands_all_fields() {
        std::env::set_var("MCPLUG_TEST_SC_URL", "https://example.com");
        std::env::set_var("MCPLUG_TEST_SC_CMD", "mycmd");
        std::env::set_var("MCPLUG_TEST_SC_KEY", "secret123");
        std::env::set_var("MCPLUG_TEST_SC_TOK", "tok456");

        let mut cfg = ServerConfig {
            description: None,
            base_url: Some("${MCPLUG_TEST_SC_URL}/mcp".into()),
            command: Some("${MCPLUG_TEST_SC_CMD}".into()),
            args: vec!["--key=${MCPLUG_TEST_SC_KEY}".into()],
            env: HashMap::from([("API_KEY".into(), "${MCPLUG_TEST_SC_KEY}".into())]),
            headers: HashMap::from([(
                "Authorization".into(),
                "Bearer $env:MCPLUG_TEST_SC_TOK".into(),
            )]),
            lifecycle: None,
        };
        expand_server_config(&mut cfg).unwrap();

        assert_eq!(cfg.base_url.as_deref(), Some("https://example.com/mcp"));
        assert_eq!(cfg.command.as_deref(), Some("mycmd"));
        assert_eq!(cfg.args, vec!["--key=secret123"]);
        assert_eq!(cfg.env.get("API_KEY").unwrap(), "secret123");
        assert_eq!(cfg.headers.get("Authorization").unwrap(), "Bearer tok456");

        std::env::remove_var("MCPLUG_TEST_SC_URL");
        std::env::remove_var("MCPLUG_TEST_SC_CMD");
        std::env::remove_var("MCPLUG_TEST_SC_KEY");
        std::env::remove_var("MCPLUG_TEST_SC_TOK");
    }
}
