# DevTUI

Terminal markdown editor with live preview + static blog generator.

## Editor

Embeds vim via PTY with real-time markdown preview. Left pane is full vim. Right pane renders the markdown as you type.

```bash
cargo build --release
./target/release/devtui mypost.md
```

Position sync uses vim's `titlestring` (zero file I/O). Content sync debounced via `CursorHold`. Mode detected from the terminal title.

All vim keybindings work. `Ctrl+D`/`Ctrl+U` for half-page scroll, `G`/`gg` for top/bottom, `:w` to save, `:q` to quit.

## Blog

Static site generator for [leandronsp.com](https://leandronsp.com). Converts markdown to HTML via pandoc.

```bash
make blog.build   # build all posts to blog/
make blog.serve   # build and serve on localhost:8000
make blog.clean   # remove blog/
```

### Adding a post

Create `posts/YYYY-MM-DD-slug.md` with frontmatter:

```yaml
---
title: Post Title
date: 2026-03-29
description: Short description
---
```

Code blocks get syntax highlighting for 140+ languages automatically.

### Themes

Dark (default) and light (solarized pastel). Toggle in the top-right corner, persists in localStorage.

## Development

```bash
cargo test                    # 26 preview rendering tests
cargo clippy -- -D warnings   # lint
```

## Dependencies

- [ratatui](https://ratatui.rs/) + [crossterm](https://github.com/crossterm-rs/crossterm) — TUI framework
- [portable-pty](https://docs.rs/portable-pty) + [vt100](https://docs.rs/vt100) + [tui-term](https://docs.rs/tui-term) — Vim PTY embedding
- [pulldown-cmark](https://github.com/raphlinus/pulldown-cmark) — Markdown parsing
- [pandoc](https://pandoc.org/) — Blog HTML generation
