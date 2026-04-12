use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
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

/// Build the prompt sent to the AI writing companion.
pub fn build_check_prompt(content: &str) -> String {
    format!(
        r#"Review this markdown document for grammar, spelling, factual accuracy, and coherence.
Detect the language (English or Portuguese) and check accordingly.
Skip content inside fenced code blocks.

Return ONLY a JSON array of annotations, no other text. Each annotation:
{{"line": <number>, "tier": "<error|hint|research>", "message": "<description>"}}

Tiers:
- "error": grammar, spelling, wrong word. Include the fix.
- "hint": better phrasing, missing context, factual checks.
- "research": related references, supporting data.

If no issues found, return an empty array: []

Document:
{content}"#
    )
}

/// Extract JSON annotations from a potentially noisy AI response.
/// Looks for the first `[` to last `]` substring and parses it.
pub fn extract_annotations(response: &str) -> Vec<Annotation> {
    let start = match response.find('[') {
        Some(pos) => pos,
        None => return Vec::new(),
    };
    let end = match response.rfind(']') {
        Some(pos) => pos,
        None => return Vec::new(),
    };
    if end <= start {
        return Vec::new();
    }
    parse_annotations(&response[start..=end])
}

/// Render annotations with visible-line focus. Annotations for lines within
/// `visible_start..visible_end` appear first and bright. Others are dimmed.
pub fn render_lines_with_focus(
    annotations: &[Annotation],
    visible_start: u16,
    visible_end: u16,
) -> Vec<Line<'static>> {
    let is_visible = |a: &Annotation| a.line >= visible_start && a.line <= visible_end;

    let mut visible: Vec<&Annotation> = annotations.iter().filter(|a| is_visible(a)).collect();
    let mut offscreen: Vec<&Annotation> = annotations.iter().filter(|a| !is_visible(a)).collect();
    visible.sort_by_key(|a| a.line);
    offscreen.sort_by_key(|a| a.line);

    let mut lines = Vec::with_capacity(annotations.len());

    for a in visible {
        let icon = tier_icon(&a.tier);
        lines.push(Line::from(format!("{icon} L{}: {}", a.line, a.message)));
    }

    let dim = Style::default().fg(Color::DarkGray);
    for a in offscreen {
        let icon = tier_icon(&a.tier);
        lines.push(Line::from(Span::styled(
            format!("{icon} L{}: {}", a.line, a.message),
            dim,
        )));
    }

    lines
}

fn tier_icon(tier: &Tier) -> &'static str {
    match tier {
        Tier::Error => "✗",
        Tier::Hint => "💡",
        Tier::Research => "📎",
    }
}

/// Render annotations as ratatui lines for the scribe panel.
pub fn render_lines(annotations: &[Annotation]) -> Vec<Line<'static>> {
    annotations
        .iter()
        .map(|a| {
            let icon = tier_icon(&a.tier);
            Line::from(format!("{icon} L{}: {}", a.line, a.message))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_lines_with_focus_sorts_visible_first() {
        let annotations = vec![
            Annotation { line: 2, tier: Tier::Error, message: "off-screen error".into() },
            Annotation { line: 10, tier: Tier::Hint, message: "visible hint".into() },
            Annotation { line: 15, tier: Tier::Error, message: "visible error".into() },
            Annotation { line: 30, tier: Tier::Research, message: "off-screen ref".into() },
        ];

        let lines = render_lines_with_focus(&annotations, 8, 20);

        assert_eq!(lines.len(), 4);
        // Visible annotations (lines 10, 15) come first
        let text: Vec<String> = lines.iter().map(|l| l.to_string()).collect();
        assert!(text[0].contains("L10"));
        assert!(text[1].contains("L15"));
        // Off-screen annotations follow
        assert!(text[2].contains("L2"));
        assert!(text[3].contains("L30"));
    }

    #[test]
    fn render_lines_with_focus_dims_offscreen_annotations() {
        let annotations = vec![
            Annotation { line: 5, tier: Tier::Error, message: "off-screen".into() },
            Annotation { line: 15, tier: Tier::Error, message: "visible".into() },
        ];

        let lines = render_lines_with_focus(&annotations, 10, 20);

        // Visible annotation (line 15) should not be dimmed
        let visible_line = &lines[0];
        let visible_styles: Vec<_> = visible_line.spans.iter().map(|s| s.style).collect();
        assert!(visible_styles.iter().any(|s| s.fg != Some(Color::DarkGray)));

        // Off-screen annotation (line 5) should be dimmed
        let dim_line = &lines[1];
        let dim_styles: Vec<_> = dim_line.spans.iter().map(|s| s.style).collect();
        assert!(dim_styles.iter().all(|s| s.fg == Some(Color::DarkGray)));
    }

    #[test]
    fn build_check_prompt_includes_content_and_json_instruction() {
        let content = "# Hello\n\nSome text here.";
        let prompt = build_check_prompt(content);
        assert!(prompt.contains("# Hello"));
        assert!(prompt.contains("Some text here."));
        assert!(prompt.contains("JSON"));
    }

    #[test]
    fn build_check_prompt_instructs_to_skip_code_blocks() {
        let prompt = build_check_prompt("some text");
        assert!(prompt.contains("code block"));
    }

    #[test]
    fn extract_annotations_from_noisy_response() {
        let response = r#"Here are the annotations I found:

[{"line": 3, "tier": "error", "message": "typo: 'teh' should be 'the'"}]

Let me know if you need more details."#;

        let annotations = extract_annotations(response);
        assert_eq!(annotations.len(), 1);
        assert_eq!(annotations[0].line, 3);
        assert_eq!(annotations[0].tier, Tier::Error);
    }

    #[test]
    fn extract_annotations_returns_empty_on_no_json() {
        let annotations = extract_annotations("No issues found in this document.");
        assert!(annotations.is_empty());
    }

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
