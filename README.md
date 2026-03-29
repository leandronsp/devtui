# DevTUI

Terminal-based markdown editor with live preview. Vim keybindings. Built with Rust and ratatui.

## Install

```bash
cargo build --release
cp target/release/devtui ~/.local/bin/
```

## Usage

```bash
devtui
```

Split-pane layout: editor on the left, rendered markdown preview on the right. Everything updates in real-time as you type.

## Controls

| Key | Mode | Action |
|-----|------|--------|
| `i` | Normal | Insert mode |
| `Esc` | Insert | Normal mode |
| `hjkl` | Normal | Navigate |
| `w` / `b` | Normal | Word forward/backward |
| `0` / `$` | Normal | Line start/end |
| `G` / `gg` | Normal | Bottom/top |
| `o` / `O` | Normal | New line below/above |
| `x` | Normal | Delete char |
| `Ctrl+D` / `Ctrl+U` | Normal | Half-page down/up |
| `q` | Normal | Quit |

## Dependencies

- [ratatui](https://ratatui.rs/) — TUI framework
- [crossterm](https://github.com/crossterm-rs/crossterm) — Terminal events
- [pulldown-cmark](https://github.com/raphlinus/pulldown-cmark) — Markdown parsing
