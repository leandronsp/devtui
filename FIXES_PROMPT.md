# DevTUI - Fix Brief

## What is DevTUI

A terminal markdown editor built in Rust with ratatui. Left pane embeds vim via PTY (portable-pty + vt100 + tui-term). Right pane shows live markdown preview rendered with pulldown-cmark.

Run: `cargo run -- test-article.md`

## What works

- Vim runs inside a PTY, all keybindings work
- Markdown preview renders headings, bold, italic, strikethrough, inline code, links, blockquotes, horizontal rules, lists (including nested)
- Mode detection (NORMAL/INSERT/VISUAL/COMMAND) from vt100 screen
- Content sync: vim writes buffer to `/tmp/devtui-content` on TextChanged
- 25 unit tests pass for preview rendering (`cargo test`)
- Terminal resize handled

## What is broken

### 1. Preview scroll sync (critical)

The preview pane does not stay aligned with the editor as you scroll through a document longer than one screen.

**The core challenge:** Markdown source has N lines. Rendered preview has M lines (M > N because headings add blank lines, lists add spacing). When the editor viewport starts at source line X, the preview needs to scroll to the corresponding rendered line Y, but Y != X.

**Current approach:** vim writes `line('w0')` (first visible line number) to `/tmp/devtui-pos` via `CursorHold` autocmd (fires after 100ms pause). The preview slices the source at that line and renders from there. This doesn't work because:
- CursorHold has delay (stale position during fast scrolling)
- Slicing the source mid-document breaks markdown context (a heading at line X-1 affects rendering at line X)

**Constraint:** Any approach that writes to disk on every CursorMoved event causes unacceptable lag. The vim `writefile()` call is synchronous with fsync.

Research what other projects do. Think freely. Maybe the answer is not syncing line numbers at all.

### 2. Code blocks render on single line

```
Expected:                    Actual:
defmodule Blog do            defmodule Blog do  def list_articles...
  def list_articles do
```

The pulldown-cmark `Event::Text` inside a `Tag::CodeBlock` delivers the entire block content as one string with embedded `\n`. The renderer pushes it as a single Span, so ratatui puts it all on one line.

### 3. Lists have excessive blank lines

After nested lists end, multiple blank lines accumulate in the preview. Each `TagEnd::List` at depth 0 adds a blank line. Nested lists cause this to stack.

### 4. Minor: blockquote styling on continuation lines

Lower priority. Multi-line blockquotes may lose the `> ` prefix or italic styling on some lines.

## Key files

- `src/main.rs` - PTY setup, event loop, vim spawn, preview scroll logic
- `src/preview.rs` - Markdown to ratatui Lines rendering + tests
- `test-article.md` - Comprehensive test document covering all markdown features
- `FIXES.md` - Detailed issue descriptions and failed approaches

## Testing with `tu` (terminal-use CLI)

The `tu` CLI is a headless virtual terminal for AI agents. Use it to run devtui without a real terminal and inspect output programmatically.

```bash
# Start devtui in a virtual terminal
tu run --name devtest --size 120x40 -- ./target/release/devtui test-article.md

# Take a screenshot (returns JSON with screen content)
tu screenshot --name devtest

# Send keystrokes to vim
tu press --name devtest j j j j j    # scroll down 5 lines
tu press --name devtest G             # go to bottom
tu press --name devtest g g           # go to top

# Type text
tu type --name devtest "# Hello World"
tu press --name devtest Enter

# Get cursor position
tu cursor --name devtest

# Kill the session
tu kill --name devtest
```

**Known issue:** ratatui uses alternate screen buffer which may not render in `tu screenshot`. If screenshots come back empty, try using `tu wait` to wait for content to appear, or send a keystroke first to trigger a redraw.

Research `tu` docs with `tu --help`, `tu run --help`, `tu screenshot --help` etc. The tool supports JSON output for programmatic assertions.

Use `tu` to build automated integration tests:
1. Start devtui with test-article.md
2. Send scroll commands (G, gg, Ctrl+D)
3. Screenshot after each
4. Parse the JSON content to verify preview alignment
5. Kill session

## How to verify

1. `cargo test` - all 25 tests must pass
2. `cargo run -- test-article.md` - scroll through the document with `j`/`k`/`G`/`gg`/`Ctrl+D`/`Ctrl+U` and verify the preview follows
3. Check code blocks, nested lists, blockquotes render correctly
4. No lag when typing or scrolling
5. Use `tu` for automated testing of scroll sync
