---
name: debug
description: Rust debugging workflow and tools. Use when: debug, debugging, inspect, trace, why is this failing, what's wrong, investigate, diagnose, not working.
---

# Debug - Rust Debugging Workflow

## Quick Reference

### Print Debugging

```rust
// dbg! macro - prints file, line, expression, and value
let result = dbg!(some_expression);

// In pipelines
let value = input
    .parse()
    .map(|v| dbg!(v))
    .unwrap_or_default();

// Multiple values
dbg!(config.title, post.date, template_path);

// eprintln! for labeled output
eprintln!("after render: {:?}", html);
eprintln!("frontmatter={:?}, body_len={}", fm, body.len());
```

### Targeted Test Runs

```bash
# Single module tests
cargo test engine::config::tests

# Single test by name
cargo test engine::config::tests::frontmatter_extracts_title

# With output (see println!/dbg! output)
cargo test -- --nocapture

# With backtrace on panic
RUST_BACKTRACE=1 cargo test

# Full backtrace
RUST_BACKTRACE=full cargo test
```

### Cargo Check (fast feedback)

```bash
# Type-check without building
cargo check

# Lint check
cargo clippy -- -D warnings
```

## Editor Debugging

```rust
// PTY state inspection
dbg!(screen.title());  // vim's titlestring (position + mode)

// Preview rendering
dbg!(&rendered_lines[0..5]);  // first 5 rendered lines
dbg!(offset_map.get(&source_line));  // source-to-rendered mapping
```

## Engine Debugging

```rust
// Config inspection
dbg!(&config);  // BlogConfig fields

// Template rendering
dbg!(&template_content);  // raw template before variable substitution
dbg!(&rendered_html);  // HTML output after rendering

// Frontmatter parsing
dbg!(frontmatter(content, "date"));  // parsed frontmatter map
```

## Common Issues

| Symptom | Check | Likely Cause |
|---------|-------|-------------|
| Wrong HTML output | `dbg!(rendered_html)` | Template variable not substituted |
| Missing frontmatter field | `dbg!(frontmatter)` | Field name mismatch or quoting issue |
| CSS not applied | Check minify output | CSS compilation order or minification bug |
| Broken preview sync | `dbg!(screen.title())` | titlestring format changed |
| SEO tag missing | View page source | Template missing variable or wrong condition |
| Incremental build stale | Check mtime comparison | mtime logic or template change not detected |
| Test passes but shouldn't | Check assertion | Testing the wrong thing |
| Clippy warning | `cargo clippy` | Usually a real issue, fix it |
| Borrow checker error | Scope of borrows | Restructure to drop borrows earlier |

## Rules

- **Never commit debug code** - no `dbg!`, `eprintln!`, or `println!` in committed code
- **Remove after use** - clean up all debug instrumentation before moving on
- **Use `dbg!`** over `println!` - it shows file, line, and expression name
- **Use `--nocapture`** - tests swallow stdout by default
- **Prefer `RUST_BACKTRACE=1`** - when investigating panics
