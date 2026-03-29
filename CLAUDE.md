# CLAUDE.md

## Project Overview

DevTUI is two things:

1. **Terminal markdown editor** with live preview. Embeds vim via PTY (left pane) with real-time rendered markdown preview (right pane). Built in Rust with ratatui.
2. **Static blog generator** for leandronsp.com. Converts markdown posts to HTML via pandoc with dark/light theme toggle.

## Architecture

### Editor (src/)

- **`src/main.rs`** — PTY setup, vim spawn, event loop, scroll sync, mode detection
- **`src/preview.rs`** — Markdown to ratatui Lines rendering + 26 unit tests
- **portable-pty** — Spawns vim in a pseudo-terminal
- **vt100** — Parses vim's ANSI output to reconstruct screen state
- **tui-term** — Renders vt100 screen as a ratatui widget
- **pulldown-cmark** — Markdown parsing to styled terminal output
- **ratatui** — TUI framework
- **crossterm** — Terminal event handling

### Blog (posts/, templates/, style.css)

- **Makefile** — Build system (`blog.build`, `blog.serve`, `blog.clean`)
- **pandoc** — Converts markdown to HTML with syntax highlighting
- **posts/** — Markdown source files with YAML frontmatter
- **templates/** — HTML templates for articles and index
- **style.css** — Dark (default) and light (solarized) themes

## Development Commands

```bash
# Editor
cargo run -- test-article.md  # run editor with a file
cargo build --release          # release build
cargo test                     # run 26 preview tests
cargo clippy -- -D warnings    # lint

# Blog
make help                      # show all targets
make blog.build                # convert posts to HTML
make blog.serve                # build and serve on localhost:8000
make blog.clean                # remove generated files
```

## How the Editor Works

### Vim via PTY

vim runs inside a pseudo-terminal (`vim -u NONE -N`). All keystrokes pass through. Full vim. Not a reimplementation.

### Position Sync (zero I/O)

vim's `titlestring=%{line('w0')}:%{mode()}` encodes the first visible line and current mode in the terminal title. The vt100 parser reads `screen.title()` every frame. No file I/O for position.

### Content Sync (debounced I/O)

vim writes buffer to `/tmp/devtui-content` on `CursorHold` (fires after 150ms idle) and `TextChanged`. A background thread polls this file every 100ms.

### Preview Rendering

`render_with_offsets()` parses the full document once, returns rendered lines + a source-to-rendered offset map. The preview uses `Paragraph::scroll()` to scroll to the correct position. Re-renders only when content changes.

### Mode Detection

Parsed from titlestring. Vim's `mode()` returns: `n` (normal), `i` (insert), `v/V` (visual), `R` (replace), `c` (command). Displayed as colored badge in the editor title bar.

## How the Blog Works

### Adding a Post

Create a file in `posts/` with YAML frontmatter:

```markdown
---
title: My Post Title
date: 2026-03-29
description: Short description for the index page
---

Content here. Supports all markdown including fenced code blocks
with syntax highlighting (140+ languages via pandoc/skylighting).
```

Run `make blog.build`. The filename becomes the URL slug.

### Themes

Dark theme (default) uses GitHub-dark colors. Light theme uses solarized pastel. Toggle persists in localStorage. Theme button shows "light" or "dark" indicating what you'll switch to.

## Key Gotchas

- **`-c` with `|` inside autocmd**: The pipe is interpreted as part of the autocmd command in vim, not as a command separator. Use separate `-c` flags.
- **`vim -u NONE`** needs explicit config: tabstop, expandtab, noswapfile, etc. Without config, tab is 8 spaces.
- **`writefile()` is synchronous with fsync**. Calling it on every keystroke causes visible lag/blink. Use CursorHold (debounced) instead of TextChanged for expensive I/O.
- **Code blocks from pulldown-cmark**: `Event::Text` inside `Tag::CodeBlock` delivers the entire block as one string with `\n`. Must split on `\n` and push each line separately.
- **Rendered lines != source lines**: Headings add blank lines, lists add spacing. The offset map tracks this but drifts over long documents.
- **`screen.title()`** gives real-time position from vim's titlestring via OSC escape sequences through the PTY. Zero file I/O.
- **`shortmess=aFIoOstTWcCS`** must be set via `--cmd` (before file load) to suppress the initial file info message.
- **`noshowmode noshowcmd`** hides vim's INSERT/command display since we detect mode from titlestring.

## Controls

All vim keybindings work. These are the most common:

- `i/a/A/I/o/O` — Enter insert mode
- `Esc` — Back to normal mode
- `hjkl` — Navigation
- `w/b` — Word forward/backward
- `G/gg` — Bottom/top of document
- `Ctrl+D/Ctrl+U` — Half-page down/up
- `:w` — Save (message auto-cleared)
- `:q` / `:wq` — Quit
