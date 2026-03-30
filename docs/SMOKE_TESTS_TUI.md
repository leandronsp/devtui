# CMS Smoke Tests (TU CLI)

Smoke tests for the terminal blog CMS using `tu` (terminal-use CLI).
Run against `acme-alchemist` blog with 3 test articles.

## Setup

```bash
cargo build --bin devtui
tu run --name devtui-cms --size 120x40 --cwd /path/to/devtui -- ./target/debug/devtui --cms blogs/acme-alchemist
tu wait --name devtui-cms --text "Articles" --timeout 5000
```

## Test 1: Article list loads with imported articles

```bash
tu screenshot --name devtui-cms
```

**Expected:**
- Header shows "DevTUI CMS" and "3 articles"
- Table has columns: STATUS, PIN, LANG, DATE, TITLE
- All 3 articles show as PUB (published)
- First article is selected ("> " marker)
- Status bar shows hotkey hints

## Test 2: j/k navigation

```bash
tu press --name devtui-cms j
tu screenshot --name devtui-cms
```

**Expected:**
- Selection moves to second row
- "> " marker on row 2

```bash
tu press --name devtui-cms k
tu screenshot --name devtui-cms
```

**Expected:**
- Selection moves back to first row

## Test 3: Search filter

```bash
tu press --name devtui-cms /
tu type --name devtui-cms "Gen"
tu screenshot --name devtui-cms
```

**Expected:**
- Status bar shows "/ Gen_" (search input active)
- Table filtered to 1 article (GenServer Demystified)
- Header shows "1 articles"

```bash
tu press --name devtui-cms Escape
tu screenshot --name devtui-cms
```

**Expected:**
- Search cleared, all 3 articles visible again

## Test 4: Unpublish action

```bash
tu press --name devtui-cms g         # go to first article
tu press --name devtui-cms p         # toggle publish
tu screenshot --name devtui-cms
```

**Expected:**
- Flash message "Unpublished" in header
- Article status changes to DRF (draft)
- Article moves to bottom of list (drafts sort after published)

## Test 5: Re-publish action

```bash
# Navigate to the draft article
tu press --name devtui-cms j
tu press --name devtui-cms j
tu press --name devtui-cms p
tu screenshot --name devtui-cms
```

**Expected:**
- Flash message "Published" in header
- Article status changes back to PUB

## Test 6: Pin action

```bash
tu press --name devtui-cms g         # go to first article
tu press --name devtui-cms j         # second article
tu press --name devtui-cms i         # pin
tu screenshot --name devtui-cms
```

**Expected:**
- Flash message "Pinned" in header
- Pinned article shows "*" in PIN column
- Pinned article sorts to top of list

## Test 7: Pin is exclusive (only one pinned at a time)

```bash
tu press --name devtui-cms j         # move to another article
tu press --name devtui-cms i         # pin a different one
tu screenshot --name devtui-cms
```

**Expected:**
- New article has "*" in PIN column
- Previous article no longer has "*"

## Test 8: Help overlay

```bash
tu type --name devtui-cms '?'
tu screenshot --name devtui-cms
```

**Expected:**
- Centered popup with "Keyboard Shortcuts" title
- Lists all keybindings: j/k, Enter, n, p, i, d, /, Esc, q
- "Press any key to close" at bottom

```bash
tu press --name devtui-cms Escape
tu screenshot --name devtui-cms
```

**Expected:**
- Help overlay dismissed, article list visible

## Test 9: Delete with confirmation

```bash
tu press --name devtui-cms g         # go to first article
tu press --name devtui-cms d         # delete
tu screenshot --name devtui-cms
```

**Expected:**
- Centered "Confirm Delete" popup
- Shows article title
- "y confirm  n cancel" prompt

```bash
tu press --name devtui-cms n         # cancel
tu screenshot --name devtui-cms
```

**Expected:**
- Popup dismissed, article still in list

## Test 10: Open editor (list to editor transition)

```bash
tu press --name devtui-cms g
tu press --name devtui-cms Enter
tu wait --name devtui-cms --text "EDITOR" --timeout 3000
tu screenshot --name devtui-cms
```

**Expected:**
- Dual pane: EDITOR (vim) on left, PREVIEW on right
- Article content visible in both panes
- Status bar shows filename and "DevTUI"
- Mode badge shows "NORMAL"

## Test 11: Return to list from editor

```bash
tu press --name devtui-cms Escape
tu type --name devtui-cms ':q'
tu press --name devtui-cms Enter
tu wait --name devtui-cms --text "Articles" --timeout 3000
tu screenshot --name devtui-cms
```

**Expected:**
- Back to article list
- All articles still present
- State preserved (pins, publish status)

## Test 12: Quit CMS

```bash
tu press --name devtui-cms q
tu status --name devtui-cms
```

**Expected:**
- Process exited with code 0
- `"alive": false, "exit_code": 0`

## Cleanup

```bash
tu kill --name devtui-cms
rm -f blogs/acme-alchemist/devtui.db
```
