# DevTUI

Terminal markdown editor with live preview + multi-site static blog engine.

## Editor

Embeds vim via PTY with real-time markdown preview. Left pane is full vim. Right pane renders the markdown as you type.

```bash
cargo build --release
./target/release/devtui mypost.md
```

Position sync uses vim's `titlestring` (zero file I/O). Content sync debounced via `CursorHold`. Mode detected from the terminal title.

All vim keybindings work. `Ctrl+D`/`Ctrl+U` for half-page scroll, `G`/`gg` for top/bottom, `:w` to save, `:q` to quit.

## Blog Engine

Multi-site static blog generator. See [docs/BLOG_ENGINE.md](docs/BLOG_ENGINE.md) for full documentation.

## Development

```bash
make test                     # cargo tests + bats engine tests
cargo test                    # 26 preview rendering tests
bats engine/tests/            # 49 engine tests (unit + integration)
cargo clippy -- -D warnings   # lint
```

## Dependencies

### Editor

- [ratatui](https://ratatui.rs/) + [crossterm](https://github.com/crossterm-rs/crossterm) -- TUI framework
- [portable-pty](https://docs.rs/portable-pty) + [vt100](https://docs.rs/vt100) + [tui-term](https://docs.rs/tui-term) -- vim PTY embedding
- [pulldown-cmark](https://github.com/raphlinus/pulldown-cmark) -- markdown parsing

### Engine

- [pandoc](https://pandoc.org/) -- markdown to HTML
- [dasel](https://github.com/TomWright/dasel) -- TOML config parsing
- [jq](https://jqlang.github.io/jq/) -- JSON processing
- **python3** -- minification, text processing
- **xmllint** -- feed.xml validation
- **rsync** -- static asset copying and deploy

### Testing

- [bats](https://github.com/bats-core/bats-core) -- engine shell tests

### Install (macOS)

```bash
brew install vim pandoc dasel jq bats-core
# python3 and xmllint are pre-installed on macOS
```

## License

AGPL-3.0
