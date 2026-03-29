---
description: Testing conventions for Rust and bats
globs: ["src/**/*.rs", "tests/**/*.rs", "engine/**/*.bats", "engine/**/*.sh"]
alwaysApply: false
---

# Testing Conventions

## Running Tests

```bash
# all tests
make test

# rust only
make editor.test                        # all tests
cargo test editor::tests                # single module
cargo test editor::tests::inserts_char  # single test
make editor.lint                        # lint check

# engine only (bats)
bats engine/tests/                      # all engine tests
bats engine/tests/build.bats            # build integration tests
bats engine/tests/lib.bats              # lib unit tests
bats engine/tests/build.bats --filter "skips unchanged"  # single test
```

## TDD Cycle

1. **RED** — Write the test asserting correct behavior, run it, confirm it fails
2. **GREEN** — Write minimum code to make it pass
3. **REFACTOR** — Clean up while staying green
4. Repeat

### Tests drive code, never the reverse

- The test defines what correct behavior is — **never change a test to match a wrong implementation**
- If the implementation returns the wrong value but the test expects the right one, fix the implementation
- Every match arm, every error variant must have a test that fails without it

## Conventions

- Write tests BEFORE or alongside implementation, never after
- Test public API only — never test private functions
- One assertion focus per test (multiple asserts OK if one logical thing)
- Descriptive test names: `renders_heading_as_styled` not `test_render_1`
- `assert_eq!` / `assert_ne!` over bare `assert!` — they print both values on failure
- Return `Result` from tests to use `?` instead of `unwrap()`
- Unit tests in each module with `#[cfg(test)]`
- Integration tests in `tests/` directory

## Bats Conventions (engine tests)

- `engine/tests/lib.bats` -- unit tests for each lib function
- `engine/tests/build.bats` -- integration tests for full build output
- Every new lib function needs bats tests
- Every new output artifact needs integration assertions
- Use `setup()` to create fixtures in `$(mktemp -d)`, `teardown()` to clean up
- Test names: `@test "build: generates 404.html"` (prefix with module)
- Use `run` to capture exit code and output, then assert with `[ "$status" -eq 0 ]`
- Use `[[ "$output" == *"pattern"* ]]` to check build output messages
- Use `grep -q` to check file contents, `! grep -q` to assert absence
- For incremental build tests, use `sleep 1` + `touch` to change mtime
- Prefer checking output messages over mtime comparisons (less flaky)

## Edge Cases to Test

### Rust (editor)

- Empty buffer / empty input
- Cursor at buffer boundaries (start, end, line start, line end)
- Unicode characters and multi-byte sequences
- Very long lines and large documents
- Rapid mode switching (normal/insert)
- Markdown edge cases (nested formatting, unclosed tags)

### Bats (engine)

- Posts without description field
- Posts with horizontal rules in body (not confused with frontmatter)
- Duplicate posts (same title, different filenames)
- Lists/blockquotes without preceding blank lines (dev.to imports)
- Emoji shortcodes in post content
- Missing optional config fields (no analytics, no license, no tags)
- Incremental builds (unchanged, changed, new posts)
