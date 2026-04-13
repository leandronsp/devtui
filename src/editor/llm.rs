//! LLM provider configuration and API calls.
//! Supports Groq and Claude (Anthropic) via direct HTTP.

pub const GROQ_DEFAULT_MODEL: &str = "llama-3.3-70b-versatile";
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
    let (url, auth_header, auth_value) = api_credentials(provider);
    let body = build_request_body(provider, prompt);
    let response_body = http_post(url, auth_header, &auth_value, provider, &body)?;
    extract_response_text(provider, &response_body)
}

/// Tool call extracted from a Groq response.
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: String,
}

/// The vault_search tool definition for the Groq API.
fn vault_search_tool() -> serde_json::Value {
    serde_json::json!({
        "type": "function",
        "function": {
            "name": "vault_search",
            "description": "Search the user's Obsidian vault for notes. Use short English keywords for BM25 matching. Always call this when the user mentions vault, notes, or asks for ideas from their knowledge base.",
            "parameters": {
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Short English BM25 keywords (e.g. 'blog post ideas', 'rust async', 'AI machine learning')"
                    }
                },
                "required": ["query"]
            }
        }
    })
}

/// Build a Groq request body with tool definitions.
pub fn build_request_with_tools(
    provider: &Provider,
    messages: &[serde_json::Value],
) -> serde_json::Value {
    match provider {
        Provider::Groq { model, .. } => serde_json::json!({
            "model": model,
            "messages": messages,
            "tools": [vault_search_tool()],
        }),
        Provider::Claude { model, .. } => serde_json::json!({
            "model": model,
            "max_tokens": 1024,
            "messages": messages,
        }),
    }
}

/// Extract a tool call from a Groq response, if present.
pub fn extract_tool_call(response: &str) -> Option<ToolCall> {
    let json: serde_json::Value = serde_json::from_str(response).ok()?;
    let message = &json["choices"][0]["message"];
    let tool_call = &message["tool_calls"][0];
    let id = tool_call["id"].as_str()?;
    let name = tool_call["function"]["name"].as_str()?;
    let arguments = tool_call["function"]["arguments"].as_str()?;
    Some(ToolCall {
        id: id.to_string(),
        name: name.to_string(),
        arguments: arguments.to_string(),
    })
}

/// Parse the query from a vault_search tool call arguments JSON.
pub fn parse_vault_query(arguments: &str) -> Option<String> {
    let json: serde_json::Value = serde_json::from_str(arguments).ok()?;
    json["query"].as_str().map(String::from)
}

/// Build the follow-up messages after a tool call: the assistant's tool call
/// message + the tool result.
pub fn build_tool_result_messages(
    tool_call_id: &str,
    tool_call_name: &str,
    tool_call_arguments: &str,
    result: &str,
) -> Vec<serde_json::Value> {
    vec![
        serde_json::json!({
            "role": "assistant",
            "tool_calls": [{
                "id": tool_call_id,
                "type": "function",
                "function": {
                    "name": tool_call_name,
                    "arguments": tool_call_arguments,
                }
            }]
        }),
        serde_json::json!({
            "role": "tool",
            "tool_call_id": tool_call_id,
            "content": result,
        }),
    ]
}

/// Call the LLM with vault_search tool support. If the LLM requests a tool
/// call, DevTUI executes qmd search locally and sends the result back.
/// Max 1 tool round-trip to keep latency bounded.
pub fn call_llm_with_tools(provider: &Provider, prompt: &str) -> Result<String, String> {
    let log_path = std::env::temp_dir().join("devtui-llm-debug.log");
    let log = |msg: &str| {
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&log_path) {
            let _ = writeln!(f, "[{}] {msg}", chrono_lite());
        }
    };

    let (url, auth_header, auth_value) = api_credentials(provider);

    let mut messages = vec![serde_json::json!({"role": "user", "content": prompt})];
    let body = build_request_with_tools(provider, &messages);

    log(">>> First LLM call (with tools)");
    let response_body = http_post(url, auth_header, &auth_value, provider, &body)?;
    log(&format!("<<< Response: {}", &response_body[..response_body.len().min(500)]));

    // Check if the LLM wants to call a tool
    if let Some(tc) = extract_tool_call(&response_body) {
        log(&format!("Tool call: {} args={}", tc.name, tc.arguments));
        if tc.name == "vault_search" {
            if let Some(query) = parse_vault_query(&tc.arguments) {
                log(&format!("Running qmd search: {query}"));
                let vault_result = super::chat::query_vault(&query)
                    .unwrap_or_else(|| "No results found in vault.".to_string());
                log(&format!("Vault result: {}", &vault_result[..vault_result.len().min(300)]));

                let tool_msgs = build_tool_result_messages(
                    &tc.id, &tc.name, &tc.arguments, &vault_result,
                );
                messages.extend(tool_msgs);

                // Second call without tools (just get the final answer)
                let body2 = match provider {
                    Provider::Groq { model, .. } => serde_json::json!({
                        "model": model,
                        "messages": messages,
                    }),
                    Provider::Claude { model, .. } => serde_json::json!({
                        "model": model,
                        "max_tokens": 1024,
                        "messages": messages,
                    }),
                };

                log(">>> Second LLM call (with tool result)");
                let response2 = http_post(url, auth_header, &auth_value, provider, &body2)?;
                log(&format!("<<< Response: {}", &response2[..response2.len().min(500)]));
                return extract_response_text(provider, &response2);
            }
        }
    } else {
        log("No tool call in response");
    }

    extract_response_text(provider, &response_body)
}

fn chrono_lite() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    format!("{secs}")
}

fn api_credentials(provider: &Provider) -> (&'static str, &'static str, String) {
    match provider {
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
    }
}

fn http_post(
    url: &str,
    auth_header: &str,
    auth_value: &str,
    provider: &Provider,
    body: &serde_json::Value,
) -> Result<String, String> {
    let mut request = ureq::post(url).header(auth_header, auth_value);
    if matches!(provider, Provider::Claude { .. }) {
        request = request.header("anthropic-version", "2023-06-01");
    }
    let response = request
        .send_json(body)
        .map_err(|e| format!("HTTP request failed: {e}"))?;
    response
        .into_body()
        .read_to_string()
        .map_err(|e| format!("Failed to read response: {e}"))
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
    fn build_request_with_tools_includes_vault_search_tool() {
        let provider = Provider::Groq {
            api_key: "k".to_string(),
            model: GROQ_DEFAULT_MODEL.to_string(),
        };
        let messages = vec![serde_json::json!({"role": "user", "content": "search vault"})];

        let body = build_request_with_tools(&provider, &messages);

        assert_eq!(body["tools"][0]["function"]["name"], "vault_search");
        assert_eq!(body["messages"][0]["content"], "search vault");
    }

    #[test]
    fn extract_tool_call_parses_groq_tool_response() {
        let response = r#"{
            "choices": [{
                "message": {
                    "tool_calls": [{
                        "id": "call_123",
                        "function": {
                            "name": "vault_search",
                            "arguments": "{\"query\": \"IA blog ideias\"}"
                        }
                    }]
                }
            }]
        }"#;

        let tc = extract_tool_call(response).unwrap();

        assert_eq!(tc.id, "call_123");
        assert_eq!(tc.name, "vault_search");
        assert!(tc.arguments.contains("IA blog ideias"));
    }

    #[test]
    fn extract_tool_call_returns_none_for_regular_response() {
        let response = r#"{"choices":[{"message":{"content":"just text"}}]}"#;

        assert!(extract_tool_call(response).is_none());
    }

    #[test]
    fn parse_vault_query_extracts_query_from_arguments() {
        let args = r#"{"query": "rust async tokio"}"#;

        assert_eq!(parse_vault_query(args).unwrap(), "rust async tokio");
    }

    #[test]
    fn build_tool_result_messages_has_assistant_and_tool_roles() {
        let msgs = build_tool_result_messages("call_1", "vault_search", r#"{"query":"IA"}"#, "found notes");

        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[0]["role"], "assistant");
        assert_eq!(msgs[0]["tool_calls"][0]["id"], "call_1");
        assert_eq!(msgs[1]["role"], "tool");
        assert_eq!(msgs[1]["tool_call_id"], "call_1");
        assert_eq!(msgs[1]["content"], "found notes");
    }

    #[test]
    fn resolve_provider_returns_error_when_api_key_missing() {
        let lookup = env_from(&[("DEVTUI_LLM_PROVIDER", "groq")]);

        let err = resolve_provider(lookup).unwrap_err();

        assert!(err.contains("GROQ_API_KEY"), "error should mention the key: {err}");
    }
}
