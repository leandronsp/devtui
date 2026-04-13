/// Tiny CLI wrapper around harper-core for DevTUI grammar checking.
/// Reads markdown from stdin, outputs JSON annotations to stdout.
///
/// Output format (same as scribe annotations):
/// [{"line": 1, "tier": "error", "message": "Did you mean 'the'?"}]
use std::io::Read;

use harper_core::linting::{LintGroup, Linter};
use harper_core::spell::FstDictionary;
use harper_core::{Dialect, Document};

fn main() {
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input).unwrap();

    let dict = FstDictionary::curated();
    let document = Document::new_markdown_default_curated(&input);
    let mut linter = LintGroup::new_curated(dict, Dialect::American);
    let lints = linter.lint(&document);

    let annotations: Vec<serde_json::Value> = lints
        .iter()
        .filter_map(|lint| {
            let span = lint.span;
            let line = input[..span.start].matches('\n').count() + 1;
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
