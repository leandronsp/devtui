use ratatui::text::Line;
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

/// Render annotations as ratatui lines for the scribe panel.
pub fn render_lines(annotations: &[Annotation]) -> Vec<Line<'static>> {
    annotations
        .iter()
        .map(|a| {
            let icon = match a.tier {
                Tier::Error => "✗",
                Tier::Hint => "💡",
                Tier::Research => "📎",
            };
            Line::from(format!("{icon} L{}: {}", a.line, a.message))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_lines_produces_annotated_output() {
        let annotations = vec![
            Annotation { line: 3, tier: Tier::Error, message: "Misspelled: 'teh'".into() },
            Annotation { line: 7, tier: Tier::Hint, message: "Consider rephrasing".into() },
            Annotation { line: 12, tier: Tier::Research, message: "See RFC 7231".into() },
        ];

        let lines = render_lines(&annotations);

        assert_eq!(lines.len(), 3);
        let text: Vec<String> = lines.iter().map(|l| l.to_string()).collect();
        assert_eq!(text[0], "✗ L3: Misspelled: 'teh'");
        assert_eq!(text[1], "💡 L7: Consider rephrasing");
        assert_eq!(text[2], "📎 L12: See RFC 7231");
    }

    #[test]
    fn render_lines_returns_empty_for_no_annotations() {
        let lines = render_lines(&[]);
        assert!(lines.is_empty());
    }

    #[test]
    fn parse_annotations_sorts_errors_before_hints_on_same_line() {
        let json = r#"[
            {"line": 5, "tier": "hint", "message": "rephrase"},
            {"line": 5, "tier": "error", "message": "spelling"}
        ]"#;
        let annotations = parse_annotations(json);
        assert_eq!(annotations[0].tier, Tier::Error);
        assert_eq!(annotations[1].tier, Tier::Hint);
    }

    #[test]
    fn parse_annotations_skips_unknown_tier() {
        let json = r#"[{"line": 1, "tier": "warning", "message": "x"}]"#;
        let annotations = parse_annotations(json);
        assert_eq!(annotations.len(), 0);
    }

    #[test]
    fn parse_annotations_returns_empty_on_invalid_json() {
        let annotations = parse_annotations("not json at all");
        assert_eq!(annotations.len(), 0);
    }

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
