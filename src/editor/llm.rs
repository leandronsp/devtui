//! LLM provider configuration and API calls.
//! Supports Groq and Claude (Anthropic) via direct HTTP.

pub const GROQ_DEFAULT_MODEL: &str = "llama-3.1-8b-instant";
pub const CLAUDE_DEFAULT_MODEL: &str = "claude-haiku-4-5-20251001";

#[derive(Debug, PartialEq)]
pub enum Provider {
    Groq { api_key: String, model: String },
    Claude { api_key: String, model: String },
}

/// Resolve provider from a lookup function (testable, no global state).
/// Reads `DEVTUI_LLM_PROVIDER` (defaults to "groq"), then reads the
/// matching API key: `GROQ_API_KEY` or `ANTHROPIC_API_KEY`.
pub fn resolve_provider<F>(lookup: F) -> Result<Provider, String>
where
    F: Fn(&str) -> Option<String>,
{
    let provider_name = lookup("DEVTUI_LLM_PROVIDER")
        .unwrap_or_else(|| "groq".to_string());

    match provider_name.as_str() {
        "groq" => {
            let api_key = lookup("GROQ_API_KEY")
                .ok_or("No API key. Set GROQ_API_KEY.")?;
            Ok(Provider::Groq {
                api_key,
                model: GROQ_DEFAULT_MODEL.to_string(),
            })
        }
        "claude" => {
            let api_key = lookup("ANTHROPIC_API_KEY")
                .ok_or("No API key. Set ANTHROPIC_API_KEY.")?;
            Ok(Provider::Claude {
                api_key,
                model: CLAUDE_DEFAULT_MODEL.to_string(),
            })
        }
        other => Err(format!("Unknown LLM provider: {other}. Use 'groq' or 'claude'.")),
    }
}

/// Build the JSON request body for the provider's API.
pub fn build_request_body(provider: &Provider, prompt: &str) -> serde_json::Value {
    match provider {
        Provider::Groq { model, .. } => serde_json::json!({
            "model": model,
            "messages": [{"role": "user", "content": prompt}],
        }),
        Provider::Claude { model, .. } => serde_json::json!({
            "model": model,
            "max_tokens": 1024,
            "messages": [{"role": "user", "content": prompt}],
        }),
    }
}

/// Extract the text content from a provider's JSON response.
pub fn extract_response_text(provider: &Provider, response: &str) -> Result<String, String> {
    let json: serde_json::Value = serde_json::from_str(response)
        .map_err(|e| format!("Invalid JSON response: {e}"))?;

    match provider {
        Provider::Groq { .. } => json["choices"][0]["message"]["content"]
            .as_str()
            .map(String::from)
            .ok_or_else(|| "No content in Groq response".to_string()),
        Provider::Claude { .. } => json["content"][0]["text"]
            .as_str()
            .map(String::from)
            .ok_or_else(|| "No content in Claude response".to_string()),
    }
}

/// Call the LLM API with the given prompt. Blocking HTTP call.
/// Returns the text content from the response.
pub fn call_llm(provider: &Provider, prompt: &str) -> Result<String, String> {
    let (url, auth_header, auth_value) = match provider {
        Provider::Groq { api_key, .. } => (
            "https://api.groq.com/openai/v1/chat/completions",
            "Authorization",
            format!("Bearer {api_key}"),
        ),
        Provider::Claude { api_key, .. } => (
            "https://api.anthropic.com/v1/messages",
            "x-api-key",
            api_key.clone(),
        ),
    };

    let body = build_request_body(provider, prompt);

    let response = ureq::post(url)
        .header(auth_header, &auth_value)
        .send_json(&body)
        .map_err(|e| format!("HTTP request failed: {e}"))?;

    let response_body = response
        .into_body()
        .read_to_string()
        .map_err(|e| format!("Failed to read response: {e}"))?;

    extract_response_text(provider, &response_body)
}

/// Convenience wrapper that reads from actual environment variables.
pub fn provider_from_env() -> Result<Provider, String> {
    resolve_provider(|key| std::env::var(key).ok())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn env_from(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let map: HashMap<String, String> = pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        move |key| map.get(key).cloned()
    }

    #[test]
    fn resolve_provider_returns_groq_when_provider_env_is_groq() {
        let lookup = env_from(&[
            ("DEVTUI_LLM_PROVIDER", "groq"),
            ("GROQ_API_KEY", "test-key"),
        ]);

        let provider = resolve_provider(lookup).unwrap();

        assert_eq!(
            provider,
            Provider::Groq {
                api_key: "test-key".to_string(),
                model: GROQ_DEFAULT_MODEL.to_string(),
            }
        );
    }

    #[test]
    fn resolve_provider_returns_claude_when_provider_env_is_claude() {
        let lookup = env_from(&[
            ("DEVTUI_LLM_PROVIDER", "claude"),
            ("ANTHROPIC_API_KEY", "test-anthropic-key"),
        ]);

        let provider = resolve_provider(lookup).unwrap();

        assert_eq!(
            provider,
            Provider::Claude {
                api_key: "test-anthropic-key".to_string(),
                model: CLAUDE_DEFAULT_MODEL.to_string(),
            }
        );
    }

    #[test]
    fn resolve_provider_defaults_to_groq_when_provider_env_not_set() {
        let lookup = env_from(&[("GROQ_API_KEY", "default-key")]);

        let provider = resolve_provider(lookup).unwrap();

        assert_eq!(
            provider,
            Provider::Groq {
                api_key: "default-key".to_string(),
                model: GROQ_DEFAULT_MODEL.to_string(),
            }
        );
    }

    #[test]
    fn build_request_body_groq_has_model_and_messages() {
        let provider = Provider::Groq {
            api_key: "k".to_string(),
            model: "llama-3.1-8b-instant".to_string(),
        };

        let body = build_request_body(&provider, "check this");

        assert_eq!(body["model"], "llama-3.1-8b-instant");
        assert_eq!(body["messages"][0]["role"], "user");
        assert_eq!(body["messages"][0]["content"], "check this");
    }

    #[test]
    fn build_request_body_claude_has_max_tokens() {
        let provider = Provider::Claude {
            api_key: "k".to_string(),
            model: CLAUDE_DEFAULT_MODEL.to_string(),
        };

        let body = build_request_body(&provider, "check this");

        assert_eq!(body["model"], CLAUDE_DEFAULT_MODEL);
        assert_eq!(body["max_tokens"], 1024);
        assert_eq!(body["messages"][0]["role"], "user");
    }

    #[test]
    fn extract_response_text_groq_extracts_from_choices() {
        let response = r#"{"choices":[{"message":{"content":"the answer"}}]}"#;
        let provider = Provider::Groq {
            api_key: "k".to_string(),
            model: "m".to_string(),
        };

        let text = extract_response_text(&provider, response).unwrap();

        assert_eq!(text, "the answer");
    }

    #[test]
    fn extract_response_text_claude_extracts_from_content() {
        let response = r#"{"content":[{"type":"text","text":"the answer"}]}"#;
        let provider = Provider::Claude {
            api_key: "k".to_string(),
            model: "m".to_string(),
        };

        let text = extract_response_text(&provider, response).unwrap();

        assert_eq!(text, "the answer");
    }

    #[test]
    fn resolve_provider_returns_error_when_api_key_missing() {
        let lookup = env_from(&[("DEVTUI_LLM_PROVIDER", "groq")]);

        let err = resolve_provider(lookup).unwrap_err();

        assert!(err.contains("GROQ_API_KEY"), "error should mention the key: {err}");
    }
}
