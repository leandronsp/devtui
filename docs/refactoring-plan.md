# DevTUI Massive Refactoring Plan

Goal: reduce total lines (currently ~7048 in src/) without losing test coverage. Remove redundancy, apply DRY, improve cohesion, extract modules with meaning.

---

## Prompt for Execution

Use this prompt in a new context to execute the refactoring:

> You are refactoring a Rust codebase (DevTUI: TUI blog editor + static site engine). The codebase has ~7048 lines across src/. Your goal is to reduce lines by 20-30% while maintaining or improving test coverage. Follow these rules strictly:
>
> 1. **Never change a test to make it pass.** Tests define correctness. Fix production code only.
> 2. **Run `make test` after every change.** All 149 tests must pass. `make lint` must be clean.
> 3. **Incremental commits.** One concern per commit. `refactor: <what>` format.
> 4. **No new dependencies** unless explicitly approved.
> 5. **No `.unwrap()` in production code.** Use `?`, `if let`, `match`, or `expect("reason")`.
> 6. **No `.clone()` to silence the borrow checker.** Restructure ownership instead.
> 7. **Modules under 300 lines.** Extract submodules when growing.
> 8. **Functions under 15 lines.** Extract helpers or rethink the module boundary.
> 9. **No boolean parameters.** Use enums for self-documenting call sites.
> 10. **`pub(crate)` by default.** Only `pub` for the crate's external API.
> 11. **Iterator chains over imperative loops.** Use `filter_map`, `flat_map`, `fold`, `join`.
> 12. **`write!` over `push_str(&format!(...))`.** Avoid intermediate allocations.
> 13. **Exhaustive match on own enums.** No wildcard `_ =>`.
> 14. **Comment WHY, not WHAT.** Delete obvious comments.
> 15. **Delete dead code.** No `_` prefixed unused variables, no placeholder comments.
>
> Read the CLAUDE.md and .claude/rules/*.md files first. They define project conventions.
>
> Execute the refactoring items below in order of priority. After each item, run tests and commit.

---

## Codebase Inventory

| File | Lines | Notes |
|------|-------|-------|
| `src/editor/db.rs` | 931 | Largest file. SQLite CRUD + import. |
| `src/editor/vim.rs` | 864 | `run_loop()` is 395 lines. Main pain point. |
| `src/engine/build.rs` | 852 | 605 are tests. Production code is OK. |
| `src/editor/preview.rs` | 519 | Markdown-to-ratatui renderer. |
| `src/engine/config.rs` | 510 | 325 are tests. Production code is compact. |
| `src/editor/list.rs` | 463 | Article picker/table. |
| `src/engine/minify.rs` | 382 | CSS/HTML minification. |
| `src/engine/markdown.rs` | 337 | Markdown-to-HTML. |
| `src/editor/kitty.rs` | 298 | Kitty image protocol. |
| `src/engine/index.rs` | 268 | Index page assembly. |
| `src/engine/feed.rs` | 254 | RSS generation. |
| `src/editor/chrome.rs` | 244 | Headless Chrome screenshots. |
| `src/editor/mod.rs` | 250 | CMS entry point. |
| `src/engine/seo.rs` | 188 | Sitemap, robots.txt. |
| `src/engine/links.rs` | 174 | Social links HTML. |
| `src/engine/template.rs` | 170 | Template engine. |
| `src/engine/analytics.rs` | 121 | GA injection. |
| `src/editor/tmux.rs` | 87 | Tmux detection. |
| `src/bin/engine.rs` | 92 | CLI entry. |
| `src/bin/editor.rs` | 32 | CLI entry. |

---

## Priority 1: High-Impact Extractions (biggest line reduction)

### 1.1 Split `vim.rs::run_loop()` (395 lines -> ~8 functions)

This function handles 10+ concerns. Extract:

- `poll_content_swap()` - content buffer polling (lines 288-298)
- `poll_chrome_result()` - Chrome screenshot handling (lines 321-372)
- `handle_save_detected()` - `:w` detection + side effects (lines 377-411)
- `manage_flash()` - flash message lifecycle (lines 414-428)
- `calculate_scroll()` - scroll position sync (lines 432-449)
- `draw_frame()` - terminal rendering (lines 462-516)
- `dispatch_key()` - key event routing (lines 518-641)
- `handle_resize()` - PTY resize (lines 627-638)

Each extracted function takes `&mut self` or relevant state. The loop body becomes a ~30-line orchestrator calling these functions.

**Expected reduction:** ~50 net lines (less duplication, clearer scopes).

### 1.2 Extract `PreviewContext` struct

The tuple `(html_config: Option<&HtmlPreviewConfig>, chrome: Option<&ChromeHandle>, picker: &Picker)` appears in 5 function signatures with `#[allow(clippy::too_many_arguments)]`.

```rust
struct PreviewContext<'a> {
    html_config: Option<&'a HtmlPreviewConfig>,
    chrome: Option<&'a ChromeHandle>,
    picker: &'a Picker,
}
```

Eliminates 4 `#[allow(clippy::too_many_arguments)]` annotations and 15 parameters across 5 functions.

### 1.3 Merge `render_placeholders()` and `render_placeholders_tmux()` in `kitty.rs`

Lines 119-165 and 170-218 are near-identical. Same loop, same DIACRITICS access, same `buf.cell_mut()`. Only difference: the symbol string wrapping.

Extract the shared loop, pass a closure for the symbol builder. **Saves ~40 lines.**

### 1.4 Split `list.rs::handle_key()` (102 lines)

Three modal states with full match arms inside each. Extract:

- `handle_key_confirm_delete()`
- `handle_key_help()`
- `handle_key_search()`
- `handle_key_normal()`

---

## Priority 2: DRY Consolidation

### 2.1 Shared `tempdir()` test helper (8 identical copies)

Defined identically in: `build.rs`, `config.rs`, `feed.rs`, `seo.rs`, `analytics.rs`, `template.rs`, `minify.rs`, `db.rs`.

Create `src/testutil.rs`:

```rust
#[cfg(test)]
pub fn tempdir() -> std::path::PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let id = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("devtui-test-{}-{id}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}
```

Add `#[cfg(test)] mod testutil;` to `lib.rs` or `main.rs`. Replace all 8 copies with `use crate::testutil::tempdir`. **Saves ~70 lines.**

### 2.2 Shared `BlogConfig` test fixture (7 full constructions)

In `feed.rs` (3x), `seo.rs` (2x), `analytics.rs` (2x), every test builds the 15-field struct from scratch.

Add to testutil or a `config::tests` helper:

```rust
#[cfg(test)]
pub fn test_blog_config() -> BlogConfig {
    BlogConfig {
        title: "Test".into(),
        subtitle: "Sub".into(),
        // ... sensible defaults for all fields
    }
}
```

Each test overrides only what it needs. **Saves ~80 lines.**

### 2.3 Extract `article_href(prefix, slug) -> String`

The same branch appears in 3 modules:

```rust
if articles_prefix.is_empty() {
    format!("{slug}.html")
} else {
    format!("{articles_prefix}/{slug}.html")
}
```

Locations: `build.rs:133-136`, `index.rs:110-113`, `feed.rs:35-40`.

One function in `config.rs`. **Saves ~15 lines.**

### 2.4 Merge `init_db()` and `init_db_memory()` SQL (17-line duplicate)

Extract `fn create_schema(conn: &Connection) -> Result<()>` with the shared CREATE TABLE SQL. Both functions call it. **Saves ~15 lines.**

### 2.5 Merge `refresh()` and `refresh_search()` in `list.rs`

Lines 434-462. Identical query logic, differ only in post-refresh index selection.

```rust
fn refresh(&mut self, conn: &Connection, reset_selection: bool)
```

**Saves ~12 lines.**

### 2.6 Fix redundant `BlogConfig::from_file()` in `load_html_preview_config()`

`mod.rs:128-134` re-reads `blog.toml` from disk. The caller `run_cms()` already has a parsed `&BlogConfig`. Pass it through. **Saves ~8 lines + removes one disk I/O.**

### 2.7 Template vars overlap in `build.rs`

`render_article_html_from_post()` and `render_preview_html()` share 9 of 12 HashMap entries. Extract `fn base_template_vars(post, config) -> HashMap`. **Saves ~12 lines.**

---

## Priority 3: Dead Code & Visibility Cleanup

### 3.1 Remove dead code in `list.rs:293-303`

```rust
let was_published = article.status == Status::Published;
let slug = article.slug.clone();
// ... immediately suppressed:
let _ = (slug, was_published);
```

These variables are computed and discarded. Delete them. **Saves 5 lines + 1 unnecessary `.clone()`.**

### 3.2 Narrow `pub` to `pub(crate)` or private

| Function | File | Action |
|----------|------|--------|
| `rss_header()` | `feed.rs:8` | Make private. Only called inside `generate()`. |
| `rss_item()` | `feed.rs:21` | Make private. Only called inside `generate()`. |
| `robots_txt()` | `seo.rs:20` | Make private. Only called inside `generate_files()`. |
| `init_db_memory()` | `db.rs:111` | Add `#[cfg(test)]`. No non-test callers. |

### 3.3 Fix `unwrap()` in production code

| Location | Fix |
|----------|-----|
| `mod.rs:98` `article_tpl_path.unwrap()` | Use `let Some(path) = ... else { return None; }` |
| `kitty.rs:150,200` `write!(...).unwrap()` | Use `let _ = write!(...)` (String write never fails) |

### 3.4 Remove duplicate doc comment in `kitty.rs:220-225`

The `place_direct()` doc comment is written twice in sequence.

### 3.5 Flash messages: `String` -> `&'static str`

All flash messages in `list.rs` and `vim.rs` are string literals ("Deleted", "Published", etc.). Change `flash: Option<(String, Instant)>` to `flash: Option<(&'static str, Instant)>`. Eliminates 2 `.clone()` calls.

---

## Priority 4: Test Consolidation

### 4.1 Merge `cfg_reads_*` tests in `config.rs`

5 tests each write the same `blog.toml`, parse it, and assert one field:

- `cfg_reads_title_from_toml`
- `cfg_reads_subtitle_from_toml`
- `cfg_reads_url_from_toml`
- `cfg_reads_author_from_toml`
- `cfg_reads_lang_from_toml`

Merge into one test `cfg_reads_all_required_fields_from_toml` with 5 assertions. **Saves ~40 lines** of repeated setup.

### 4.2 Review `db.rs` "content protection" tests

Tests at lines 724, 742, 756 embed the caller's guard logic (`if !final_content.is_empty() && final_content != "Original"`) directly in the test body. They test the caller's behavior, not `db::update_content`. Consider:

- Moving them to integration tests on `edit_article()`, or
- Keeping them but documenting that they test a contract, not the DB function.

### 4.3 Fix `init_db_is_idempotent` test

Line 466: calls `conn.execute_batch()` with raw SQL instead of calling `init_db()` twice. After extracting `create_schema()` (item 2.4), this test should call the function twice instead.

---

## Priority 5: Functional Rust Patterns

Apply these patterns during refactoring to further reduce lines:

### 5.1 Iterator chains over imperative loops

Replace mutable accumulator patterns:

```rust
// Before
let mut results = Vec::new();
for item in items {
    if item.is_valid() {
        results.push(item.transform());
    }
}

// After
let results: Vec<_> = items.iter()
    .filter(|item| item.is_valid())
    .map(|item| item.transform())
    .collect();
```

Key combinators: `filter_map`, `flat_map`, `fold`, `join`, `any`, `all`, `find`, `enumerate`.

### 5.2 `write!` over `push_str(&format!(...))`

```rust
// Before: allocates temporary String
html.push_str(&format!("<li>{}</li>", title));

// After: writes directly into buffer
write!(html, "<li>{title}</li>").unwrap();
```

### 5.3 `collect::<Result<Vec<_>, _>>()`

Replace loop-with-error-checking:

```rust
let pages: Result<Vec<_>, _> = posts.iter()
    .map(|post| render_post(post, config))
    .collect();
let pages = pages?;
```

### 5.4 `let-else` for early returns

```rust
// Before
let path = resolve_template(name);
if path.is_none() { return Err(...); }
let path = path.unwrap();

// After
let Some(path) = resolve_template(name) else {
    return Err(...);
};
```

### 5.5 String building with `join`

```rust
// Before
let mut html = String::new();
for (i, tag) in tags.iter().enumerate() {
    if i > 0 { html.push_str(", "); }
    html.push_str(tag);
}

// After
let html = tags.join(", ");
```

---

## Priority 6: Module Cohesion

### 6.1 `vim.rs` (864 lines) needs splitting

After extracting functions from `run_loop()`, if the file is still over 300 lines, consider:

- `src/editor/vim/mod.rs` - run(), run_loop() orchestrator
- `src/editor/vim/keys.rs` - key_to_bytes(), dispatch_key()
- `src/editor/vim/render.rs` - render_editor(), render_preview(), draw_frame()

### 6.2 `db.rs` (931 lines) needs splitting

- `src/editor/db/mod.rs` - Article struct, schema, connection setup
- `src/editor/db/queries.rs` - CRUD operations
- `src/editor/db/import.rs` - import_from_filesystem(), build_markdown()

### 6.3 `build.rs` production code is clean

Only 247 lines of production code. The 605 test lines are appropriate for integration tests. No split needed.

---

## Priority 7: Engine Error Type

### 7.1 Replace `Result<_, String>` with a proper error enum

Currently 22 occurrences of `map_err(|e| e.to_string())` across engine modules. All engine functions return `Result<_, String>`.

```rust
// src/engine/error.rs
#[derive(Debug)]
enum EngineError {
    Io(std::io::Error),
    Config(String),
    Template(String),
    Render(String),
}

impl std::fmt::Display for EngineError { /* ... */ }
impl std::error::Error for EngineError {}
impl From<std::io::Error> for EngineError { /* ... */ }
```

This eliminates all `map_err(|e| e.to_string())` calls. Each becomes just `?`.

Consider `thiserror` crate to derive Display/Error/From automatically. One dependency, saves ~30 lines of boilerplate.

### 7.2 Replace `io::Error::other(e.to_string())` in editor

10 occurrences in `mod.rs` and `vim.rs`. Implement `From<CmsError> for io::Error` once:

```rust
impl From<CmsError> for io::Error {
    fn from(e: CmsError) -> Self {
        io::Error::other(e.to_string())
    }
}
```

Then all `map_err(|e| io::Error::other(e.to_string()))` become just `?`. **Saves ~10 lines.**

---

## Expected Impact

| Category | Estimated Lines Saved |
|----------|----------------------|
| `run_loop()` decomposition | ~50 |
| Shared `tempdir()` | ~70 |
| Shared `BlogConfig` test fixture | ~80 |
| Merge config tests | ~40 |
| Kitty placeholder merge | ~40 |
| Template vars dedup | ~12 |
| `article_href` helper | ~15 |
| DB schema dedup | ~15 |
| Dead code removal | ~10 |
| Error type consolidation | ~40 |
| `From<CmsError>` impl | ~10 |
| Iterator/functional patterns | ~30 |
| Flash `&'static str` | ~5 |
| Misc (visibility, unwrap, merge refresh) | ~20 |
| **Total** | **~437 lines (~6% reduction)** |

The 6% reduction is conservative. Applying functional patterns (iterator chains, `write!`, `let-else`) throughout will compound further. The real win is cohesion and readability: `run_loop()` going from 395 to ~30 lines, `db.rs` splitting into focused modules, and engine errors becoming type-safe.

---

## Execution Order

1. **Tests first.** Run `make test` to confirm baseline (149 pass).
2. **Shared test utilities** (2.1, 2.2). Low risk, high line reduction.
3. **Dead code and visibility** (3.x). Zero behavior change.
4. **DRY helpers** (2.3-2.7). Small, focused extractions.
5. **Test consolidation** (4.x). Reduce test maintenance surface.
6. **Big extractions** (1.1-1.4). Split monster functions.
7. **Module splits** (6.x). Only if files still exceed 300 lines.
8. **Error types** (7.x). Cross-cutting, do last.
9. **Functional patterns** (5.x). Apply opportunistically during all steps above.

After each step: `make test && make lint`. Commit. Move to next.
