/// Tiny CLI wrapper around harper-core for DevTUI grammar checking.
/// Reads markdown from stdin, outputs JSON annotations to stdout.
/// Strips YAML frontmatter before checking.
///
/// Output format (same as scribe annotations):
/// [{"line": 1, "tier": "error", "message": "Did you mean 'the'?"}]
use std::io::Read;

use harper_core::linting::{LintGroup, Linter};
use harper_core::spell::FstDictionary;
use harper_core::{Dialect, Document};

/// Strip YAML frontmatter (--- ... ---) and return (body, line offset).
fn strip_frontmatter(input: &str) -> (&str, usize) {
    if !input.starts_with("---") {
        return (input, 0);
    }
    // Find closing ---
    if let Some(end) = input[3..].find("\n---") {
        let body_start = end + 3 + 4; // skip past "\n---"
        let skip = input[..body_start].matches('\n').count();
        if body_start < input.len() {
            return (&input[body_start..], skip);
        }
    }
    (input, 0)
}

fn main() {
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input).unwrap();

    let (body, line_offset) = strip_frontmatter(&input);

    let dict = FstDictionary::curated();
    let document = Document::new_markdown_default_curated(body);
    let mut linter = LintGroup::new_curated(dict, Dialect::American);

    // Disable noisy rules for blog writing
    linter.config.set_rule_enabled("UseTitleCase", false);
    linter.config.set_rule_enabled("DisjointPrefixes", false);
    linter.config.set_rule_enabled("LongSentences", false);
    linter.config.set_rule_enabled("SentenceCapitalization", false);

    let lints = linter.lint(&document);

    let annotations: Vec<serde_json::Value> = lints
        .iter()
        .filter_map(|lint| {
            let span = lint.span;
            let line = body[..span.start].matches('\n').count() + 1 + line_offset;
            let message = lint.message.to_string();
            if message.is_empty() {
                return None;
            }
            Some(serde_json::json!({
                "line": line,
                "tier": "error",
                "message": message,
            }))
        })
        .collect();

    println!("{}", serde_json::to_string(&annotations).unwrap());
}
