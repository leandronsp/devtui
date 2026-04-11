---
name: commit
description: "[DevTUI] Git commit with conventional commits. Small, incremental, human-written messages. Runs cargo test + cargo clippy pre-commit. Never mentions AI, agents, or Claude. Use when: commit, commit this, save changes, git commit, stage and commit."
---

# Commit

Small, incremental git commits following conventional commits. For draft PRs, use `/pr`.

## Usage

- `/commit` - single-line commit for staged changes
- `/commit detailed` - multi-paragraph commit, preview before committing

## Commit Format

```
<type>(<scope>): <short description>
```

**Types:** `feat`, `fix`, `refactor`, `test`, `chore`, `docs`
**Scope (optional):** module or area — e.g. `feat(engine): ...`, `fix(editor): ...`, `refactor(minify): ...`

## Modes

### Quick (default)

Single-line commit message.

```bash
git commit -m "feat(engine): add tag filtering on index page"
```

### Detailed (`detailed` or when the change is significant)

Multi-paragraph for milestone features, non-obvious fixes, architectural changes.

Output the proposed message as plain text for review. **Do NOT run `git commit` until the user approves.**

```bash
git commit -m "fix(minify): preserve pre blocks during HTML minification

The minifier was stripping whitespace inside fenced code blocks,
breaking code samples in rendered articles. Now pre/script/style
blocks are extracted before minification and reinserted after."
```

## Commit Rules

1. **Stage explicitly** — `git add <files>`, never `git add -A` or `git add .`
2. **Concise** — present tense imperative ("add" not "added")
3. **Lowercase** after prefix
4. **No AI mentions** — never reference Claude, AI, agents, copilot, or assistants
5. **No Co-Authored-By** — never add Co-Authored-By trailers
6. **No emojis** in commit messages
7. **Human voice** — write like a developer wrote it by hand
8. **Small commits** — one logical change per commit. During TDD: commit after each RED-GREEN-REFACTOR cycle

## Pre-commit Checklist

Run the project's Rust checks before committing:

```bash
cargo test
cargo clippy -- -D warnings
git diff --staged
```

If tests fail or clippy complains, **fix the underlying issue**. Never use `--no-verify` to bypass hooks.

## Scope Reference

Scope matches the module or domain area. Common scopes:

- **Engine:** `engine`, `config`, `template`, `index`, `feed`, `seo`, `minify`, `links`, `markdown`, `analytics`, `build`
- **Editor:** `editor`, `preview`
- **Blog content:** `blog`, `theme`
- **Tooling:** `makefile`, `ci`, `deps`

Scope is optional — omit it for cross-cutting changes.

## Commit Examples

```
feat(engine): add tag filtering on index page
feat(editor): support split-pane resize with ctrl+arrow
fix(minify): preserve pre blocks during HTML minification
fix(preview): correct offset map drift on long documents
refactor(config): extract frontmatter parsing to own function
refactor(build): use writeln! instead of push_str(&format!())
test(seo): add sitemap integration test with missing fields
chore(deps): bump pulldown-cmark to 0.12
docs(claude): document incremental build mtime check
```

## Pipeline

```
/po -> GitHub issue -> /dev -> /review -> /commit -> /pr
```
