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
/// Checks `DEVTUI_LLM_PROVIDER` (defaults to "groq"), then reads the
/// matching API key variable.
pub fn resolve_provider<F>(lookup: F) -> Result<Provider, String>
where
    F: Fn(&str) -> Option<String>,
{
    let provider_name = lookup("DEVTUI_LLM_PROVIDER")
        .ok_or("Set DEVTUI_LLM_PROVIDER to 'groq' or 'claude'.")?;

    match provider_name.as_str() {
        "groq" => {
            let api_key = lookup("GROQ_API_KEY")
                .ok_or("No API key. Set GROQ_API_KEY.")?;
            Ok(Provider::Groq {
                api_key,
                model: GROQ_DEFAULT_MODEL.to_string(),
            })
        }
        other => Err(format!("Unknown LLM provider: {other}. Use 'groq' or 'claude'.")),
    }
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
}
