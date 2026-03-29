# CLAUDE.md

## Project Overview

DevTUI is a terminal-based markdown editor with live preview, built in Rust with ratatui. Vim keybindings, split-pane layout (editor left, rendered preview right), real-time rendering. Think dev.to editor but 100% in the terminal.

## Architecture

- **`src/main.rs`** — Entry point, terminal init/restore, event loop
- **ratatui** — TUI framework for rendering
- **crossterm** — Terminal event handling (keyboard input)
- **pulldown-cmark** — Markdown parsing to styled terminal output

## Development Commands

```bash
cargo run                    # run the editor
cargo build                  # build
cargo build --release        # release build
cargo test                   # run tests
cargo clippy -- -D warnings  # lint
```

## Key Design Decisions

- **Vim-first**: Normal/Insert mode with standard vim motions (hjkl, w, b, 0, $, G, gg, etc.)
- **Real-time preview**: Every keystroke re-renders the markdown preview in the right pane
- **No file I/O required**: Editor works on an in-memory buffer. File open/save is optional
- **Scroll sync**: Editor and preview scroll together
- **Minimal dependencies**: Only ratatui, crossterm, pulldown-cmark. No bloat.

## Controls

- `i/a/A/I/o/O` — Enter insert mode
- `Esc` — Back to normal mode
- `hjkl` — Navigation
- `w/b` — Word forward/backward
- `0/$` — Line start/end
- `G/gg` — Bottom/top of document
- `x` — Delete char
- `Ctrl+D/Ctrl+U` — Half-page down/up
- `q` — Quit (normal mode)
- `Ctrl+C` — Force quit
