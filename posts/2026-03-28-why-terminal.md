---
title: Why I Live in the Terminal
date: 2026-03-28
description: A case for terminal-first development and why GUIs slow me down
---

I spend 90% of my working hours inside a terminal. Not because I'm nostalgic. Because it's faster.

## The Speed Argument

Every GUI interaction involves context switching. You move your hand to the mouse, your eyes scan for the button, you click, you wait for the visual feedback. In the terminal, you type a command and get a result. No scanning, no waiting.

```bash
# Find all TODO comments in the project
rg "TODO" --type rust

# Run tests for a specific module
cargo test preview::tests

# Deploy in one command
make deploy
```

Three commands, three seconds. The GUI equivalent involves opening menus, clicking through dialogs, and waiting for progress bars.

## The Composability Argument

Unix pipes are the original microservices architecture. Every tool does one thing. You compose them.

```bash
# Count lines of Rust code
find . -name "*.rs" | xargs wc -l | tail -1

# Find the largest files
du -sh * | sort -rh | head -10

# Watch logs in real time, filtered
tail -f /var/log/app.log | grep --line-buffered "ERROR"
```

No GUI gives you this level of composability. Every graphical tool is a walled garden.

## The Tools

My daily stack:

- **Neovim** for editing. LSP, treesitter, telescope. Everything a modern IDE offers, but faster.
- **tmux** for session management. Split panes, persistent sessions, scriptable layouts.
- **ripgrep** for searching. Orders of magnitude faster than any IDE search.
- **git** on the command line. No abstraction layer hiding what's actually happening.
- **fzf** for fuzzy finding anything. Files, git branches, command history.

## The Aesthetic

There's something honest about a terminal. No rounded corners hiding complexity. No animations distracting you from the work. Just text, a cursor, and your thoughts.

The terminal doesn't pretend to be simple. It *is* simple. And that simplicity is a feature.
