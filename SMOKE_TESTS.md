# Smoke Tests

Browser smoke tests for the blog using `agent-browser` CLI.

## Setup

```bash
make blog.serve  # builds and serves on localhost:8000
```

## Tests

### 1. Index page loads with articles

```bash
agent-browser open http://localhost:8000
agent-browser screenshot /tmp/qa-test-1.png
```

Verify: page title "leandronsp.com", subtitle visible, 3 articles listed.

### 2. Articles in reverse chronological order

From the index screenshot, verify order:
- 2026-03-29 The Art of Writing Clean Code
- 2026-03-28 Why I Live in the Terminal
- 2026-03-27 GenServer Demystified

### 3. Article page renders correctly

```bash
agent-browser click "a[href='2026-03-27-elixir-genserver.html']"
agent-browser screenshot /tmp/qa-test-2.png
```

Verify: title "GenServer Demystified", date "2026-03-27", body text visible, monospace heading font.

### 4. Syntax highlighting works

```bash
agent-browser eval "window.scrollBy(0, 600)"
agent-browser screenshot /tmp/qa-test-3.png
```

Verify: Elixir code block with colored keywords (`def`, `do`, `end`), atoms, module names. Not plain monochrome text.

### 5. Footer back link works

```bash
agent-browser eval "window.scrollTo(0, document.body.scrollHeight)"
agent-browser screenshot /tmp/qa-test-4.png
agent-browser click "footer a"
agent-browser screenshot /tmp/qa-test-5.png
```

Verify: "back" link visible at bottom. Clicking it returns to index page.

### 6. Header nav link works

From any article page:

```bash
agent-browser click "header nav a"
agent-browser screenshot /tmp/qa-test-6.png
```

Verify: returns to index page.

## Adding new tests

When adding posts or changing templates, re-run all tests. When adding new features (RSS, tags, etc.), add a new numbered test section here.
