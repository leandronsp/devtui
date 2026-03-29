# DevTUI Known Issues

Use `test-article.md` to reproduce all issues below.

## P0: Preview scroll sync is broken

The preview pane does not align with the editor. Multiple approaches have been tried and all failed:

### Problem 1: Preview is ahead of editor (middle of document)
- Editor shows "## Formatting" section, preview shows "## Lists Galore" already
- The `content.lines().skip(source_line)` approach renders from the wrong starting point
- Root cause: `line('w0')` written by CursorHold autocmd is stale or not yet updated when rendering happens

### Problem 2: Preview cuts off last lines (bottom of document)
- Editor shows "- Understand before changing" as last content line
- Preview stops at "- Remove before adding", missing the last 2 items
- Root cause: markdown rendering adds extra lines (blank lines after headings, lists), so the rendered output is taller than the source. The preview viewport runs out of space.

### Problem 3: Preview completely out of sync on small terminal (zoom)
- Image 46 shows editor at "## Final Thoughts" but preview shows "Deep nesting" section (much earlier)
- Worse on smaller viewports because the discrepancy accumulates

### Approaches tried and why they failed
1. **Source-to-rendered offset mapping**: `render_with_offsets()` maps source line N to rendered line M. Works in theory but the mapping was always slightly off, causing drift that accumulates over the document.
2. **String matching first visible line**: Read the first line of the vt100 screen, find it in the source. Failed because of duplicate lines (e.g., "das" appears 10+ times).
3. **Slice and render**: `content.lines().skip(source_line)` then render. The preview starts at the right place but the CursorHold position is stale (100ms delay), causing the preview to lag behind or jump ahead.
4. **Snap to bottom**: Detect when editor is near the end and snap preview to max_scroll. Didn't reliably detect "near the end".

### Suggested approach
The real fix: use `CursorMoved` (not CursorHold) for position, but DON'T use `writefile()` (causes lag). Instead:
- Option A: Have vim set the terminal title to include `line('w0')` via `set titlestring` and parse it from the PTY.
- Option B: Use a named pipe (FIFO) instead of a regular file. `writefile()` to a FIFO doesn't fsync, so no lag.
- Option C: Use vim's `--servername` and `--remote-expr` to query position from a separate thread.
- Option D: Abandon file-based position sync entirely. Parse the vt100 screen content and match against source using a fuzzy/sliding-window approach instead of single-line matching.

## P1: Code blocks render on single line

- Editor shows a multi-line code block (elixir defmodule)
- Preview renders it all on one line: `defmodule Blog do  def list_articles do  Repo.all(Article)  endend`
- Root cause: `Event::Text` in code blocks contains the entire block as one string with `\n`. The renderer pushes it as a single span, and ratatui renders it inline.
- Fix: Split code block text on `\n` and push each line as a separate `Line`.

## P2: Lists have excessive spacing

- After nested lists, there are extra blank lines in the preview
- Each `TagEnd::List` at depth 0 adds a blank line, but nested lists cause multiple blank lines to accumulate
- Fix: Only add one blank line after the outermost list ends. Track and deduplicate consecutive blank lines.

## P3: Blockquote styling inconsistent

- Multi-line blockquotes show `> ` prefix correctly on soft breaks
- But the styling (gray/italic) may not carry through all lines consistently
- Minor visual issue, lower priority

## Test file

All issues are reproducible with `cargo run -- test-article.md`. The file covers:
- H1, H2, H3, H4 headings
- Bold, italic, strikethrough, inline code
- Fenced code blocks with language
- Blockquotes (single and multi-line)
- Simple lists, nested 2-level, nested 3-level, deeply nested 4-level
- Links, horizontal rules
- Paragraphs separated by blank lines
