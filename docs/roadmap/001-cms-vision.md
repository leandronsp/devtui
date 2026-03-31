# CMS Vision

DevTUI evolves from a static blog generator into a full TUI blog CMS, replacing Curupira (Phoenix LiveView CMS).

## Why

- Curupira is web-based, being phased out
- DevTUI already has the engine (build/deploy) and editor (vim+preview)
- Missing: content management layer
- Goal: terminal-native workflow, zero browser dependency

## Architecture

- **SQLite** for article metadata (status, pinned, tags, language, dates)
- **Two views**: article list (k9s-style) + editor (existing vim+preview)
- **Filesystem stays source of truth** for the engine. Writes to symlinked posts dir (e.g. `blogs/leandronsp.com/posts/`)
- **k9s-style UX**: j/k navigation, `/` for search, hotkeys for actions, zero mouse

## Views

### List View

Table of articles sorted by creation date. Inspired by k9s.

```
 Articles (77)                                    /search...
 ──────────────────────────────────────────────────────────────
 STATUS   PIN  LANG  DATE        TITLE
 ●        📌   en    2026-02-19  Taming non-determinism: from logic gates to LLMs
 ●             en    2025-11-14  Understanding Recursion Fundamentals
 ●             en    2025-10-22  Arrays in x86 Assembly
 ○             pt    2026-03-01  Path to Vibe Engineering
 ○             pt    2026-03-21  dsadsasda
 ──────────────────────────────────────────────────────────────
 <p> publish  <d> delete  <i> pin  <Enter> edit  <n> new  </> search
```

Features:
- j/k to navigate, Enter to open editor
- `/` to search by title (like k9s)
- `p` publish/unpublish toggle
- `d` delete with confirmation
- `i` pin/unpin (one at a time)
- `n` new article
- Status indicators: `●` published, `○` draft
- Language flag, pin indicator, tags

### Editor View (enhance existing)

Current vim+preview split stays. Add above the editor:

- Tag bar: add/remove tags (like Curupira badges with x)
- Language selector
- Publish/unpublish action
- Image upload from filesystem

## Features

### Content Management

| Feature | How it works |
|---|---|
| **Publish** | Sets status=published, published_at=now, writes .md to posts dir |
| **Unpublish** | Sets status=draft, removes .md from posts dir |
| **Pin** | Marks article as pinned (one at a time), engine shows it first in index |
| **Delete** | Removes from DB and filesystem, with y/n confirmation |
| **Drafts** | Articles with status=draft, not exported to posts dir |
| **Search** | Filter list by title, instant, like k9s `/` |
| **Tags** | Managed in editor UI, stored in DB and frontmatter |
| **Language** | Selector in editor, stored in DB and frontmatter |
| **Image upload** | Pick from filesystem, copy to images dir, insert markdown at cursor |

### What's NOT needed

- No profile/bio management (blog.toml handles it)
- No pagination (for now)
- No auto-save (vim `:w` is the save mechanism)
- No web UI (pure TUI)
- No themes for the TUI (terminal only)

## Data Model (SQLite)

```sql
CREATE TABLE articles (
  id INTEGER PRIMARY KEY,
  title TEXT NOT NULL,
  slug TEXT NOT NULL UNIQUE,
  content TEXT NOT NULL,
  status TEXT NOT NULL DEFAULT 'draft',  -- 'draft' or 'published'
  language TEXT NOT NULL DEFAULT 'en',
  pinned INTEGER NOT NULL DEFAULT 0,
  published_at TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE article_tags (
  article_id INTEGER NOT NULL REFERENCES articles(id),
  tag TEXT NOT NULL,
  PRIMARY KEY (article_id, tag)
);
```

## Flow

```
New article (n)
  → Opens editor with empty buffer
  → User writes in vim, previews on the right
  → :w saves to /tmp, background sync updates DB
  → User sets tags, language in TUI bar
  → User presses publish hotkey
  → Engine writes .md with frontmatter to posts dir
  → make blog.build picks it up

Existing article (Enter on list)
  → Loads content from DB into vim via PTY
  → Same edit flow
  → On publish, overwrites .md in posts dir

Unpublish (p on published article)
  → Sets status=draft in DB
  → Removes .md from posts dir
  → Next build excludes it

Delete (d on any article)
  → Confirmation prompt (y/n)
  → Removes from DB
  → Removes .md from posts dir if exists
```

## Curupira Feature Parity

| Feature | Curupira | DevTUI Target |
|---|---|---|
| Editor | Web textarea, split-view | Vim PTY, split-view |
| Preview | LiveView real-time | ratatui real-time |
| Publish/Unpublish | Button | `p` hotkey |
| Pin | Button | `i` hotkey |
| Delete | Button + confirm | `d` + y/n |
| Search | ILIKE query | Instant filter on list |
| Tags | Badge UI with x | TUI bar with add/remove |
| Language | Dropdown with flag | Selector in editor |
| Image upload | Drag-drop, paste | File picker, copy to images/ |
| List view | Cards with pagination | Table with j/k navigation |
| Database | PostgreSQL | SQLite |
| Dark/light | Toggle | Terminal only |
| dev.to import | Mix task | Not planned (already imported) |
| Auto-save | 5s debounce | Not needed (vim :w) |
| Keyboard shortcuts | Ctrl+B/I/E/K | Vim native |

## Reference

- Curupira codebase: `../curupira`
  - Schema: `lib/curupira/blog/article.ex`
  - List view: `lib/curupira_web/live/article_live/index.ex`
  - Editor: `lib/curupira_web/live/article_live/form.ex`
  - Export: `lib/curupira/export/markdown_export.ex`
- k9s: https://k9scli.io/ (UX inspiration)
