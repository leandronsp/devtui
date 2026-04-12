use serde::Deserialize;

/// Annotation severity tier.
#[derive(Debug, Clone, PartialEq)]
pub enum Tier {
    Error,
    Hint,
    Research,
}

/// A single annotation from the AI writing companion.
#[derive(Debug, Clone, PartialEq)]
pub struct Annotation {
    pub line: u16,
    pub tier: Tier,
    pub message: String,
}

#[derive(Deserialize)]
struct RawAnnotation {
    line: u16,
    tier: String,
    message: String,
}

/// Parse JSON annotations from the AI response.
///
/// Expected format:
/// ```json
/// [{"line": 5, "tier": "error", "message": "Misspelled: 'teh' → 'the'"}]
/// ```
pub fn parse_annotations(json: &str) -> Vec<Annotation> {
    let raw: Vec<RawAnnotation> = match serde_json::from_str(json) {
        Ok(v) => v,
        Err(err) => {
            log::warn!("parse_annotations: invalid JSON: {err}");
            return Vec::new();
        }
    };

    let mut annotations: Vec<Annotation> = raw
        .into_iter()
        .filter_map(|r| {
            let tier = match r.tier.as_str() {
                "error" => Tier::Error,
                "hint" => Tier::Hint,
                "research" => Tier::Research,
                _ => return None,
            };
            Some(Annotation { line: r.line, tier, message: r.message })
        })
        .collect();

    annotations.sort_by_key(|a| {
        let tier_rank = match a.tier {
            Tier::Error => 0u8,
            Tier::Hint => 1,
            Tier::Research => 2,
        };
        (a.line, tier_rank)
    });
    annotations
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_annotations_from_valid_json() {
        let json = r#"[
            {"line": 10, "tier": "hint", "message": "Consider rephrasing for clarity"},
            {"line": 3, "tier": "error", "message": "Misspelled: 'teh' → 'the'"},
            {"line": 7, "tier": "research", "message": "RFC 7231 supports this claim"}
        ]"#;

        let annotations = parse_annotations(json);

        assert_eq!(annotations.len(), 3);
        assert_eq!(annotations[0].line, 3);
        assert_eq!(annotations[0].tier, Tier::Error);
        assert_eq!(annotations[0].message, "Misspelled: 'teh' → 'the'");
        assert_eq!(annotations[1].line, 7);
        assert_eq!(annotations[1].tier, Tier::Research);
        assert_eq!(annotations[2].line, 10);
        assert_eq!(annotations[2].tier, Tier::Hint);
    }
}
