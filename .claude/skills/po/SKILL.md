---
name: po
description: "[DevTUI] Product Owner & Technical Analyst. Scans the Rust codebase, writes a PRD with technical breakdown, then publishes to docs/, GitHub, or Linear. Supports /po amend for PRD revisions from the implementer. Use when: PRD, feature, issue, create issue, amend issue, plan feature, scope, requirements, user story, technical spec, break down, task breakdown."
---

# Product Owner & Technical Analyst

**Scan the DevTUI codebase, write a PRD with technical breakdown, and publish it to your chosen target (local docs, GitHub, or Linear).**

## Usage

- `/po <prompt>` - Create a PRD from the given feature description
- `/po <prompt> --priority <P0|P1|P2|P3>` - With explicit priority (GitHub/Linear only)
- `/po` - Ask user what to build, then create the PRD
- `/po amend <issue_number> <summary>` - Append a revision to an existing GitHub issue (called by `/dev` when implementer research refines the PRD)

## Workflow

### Phase 1: Understand

If no prompt, ask: "What do you want to build? Describe it like you'd explain to a stakeholder."

**Wait for the user's response.**

Parse for: what (feature), who benefits, why (pain point / gap).

### Phase 2: Scan the codebase

1. Read `src/engine/mod.rs` and `src/editor/mod.rs` to see existing modules
2. Explore relevant source files for what's already implemented
3. Identify gaps, dependencies, integration points
4. Read `CLAUDE.md`, `.claude/rules/*.md`, `README.md` for domain language, conventions, and roadmap items

### Phase 3: Write the PRD + Technical Breakdown

Write from a product perspective first, then add a technical breakdown. The audience is both the user (PM/stakeholder hat) and the implementer.

```markdown
# PRD: {feature title}

**Date:** {YYYY-MM-DD}
**Status:** Draft

## Overview

What we're building and why. One paragraph.

## Problem Statement

What user/developer problem does this solve? Who feels it? How often?

## User Stories

- As a [user], I want [goal] so that [benefit]

## Current State

What already exists in the DevTUI codebase relevant to this feature. Reference specific modules (`src/engine/config.rs`, `src/editor/preview.rs`, etc.) discovered during the scan.

## Requirements

### Functional
- [ ] FR-1: Description in user-facing terms
- [ ] FR-2: ...

### Non-Functional
- [ ] NFR-1: Performance / quality requirement

### Out of Scope
- What we're explicitly NOT doing and why

## Technical Approach

### Affected Files

| File | Changes |
|------|---------|
| `src/engine/config.rs` | Add new config field parsing |
| `src/engine/build.rs` | Integration test for new feature |

### New Types & Functions

```rust
pub fn new_function(config: &BlogConfig) -> String {
    // ...
}
```

### Implementation Tasks

Each task = one testable behavior increment, ordered by dependency.

#### Task 1: [description]
**Size**: XS/S/M/L
**Test**: `assert_eq!(...)` — what proves this works
**Depends on**: (none or previous task)

#### Task 2: [description]
**Size**: XS/S/M/L
**Test**: `assert_eq!(...)`
**Depends on**: Task 1

### Error Handling

Module-specific error enum with `Display` + `Error` impls.

### Edge Cases

| Case | Handling |
|------|----------|
| Missing config field | Use default value |
| Malformed frontmatter | Error with `ConfigError::Parse` |

## Acceptance Criteria

- Given {context}, when {action}, then {expected result}
- Given {context}, when {edge case}, then {graceful handling}
```

## Task Sizing

| Size | Complexity | Example |
|------|------------|---------|
| XS | Single function, trivial | Module skeleton |
| S | Single function, some logic | `frontmatter()` with new field |
| M | Multiple functions, moderate | New template variable |
| L | Module with integration | New engine module |
| XL | Cross-module feature | Full editor feature with preview |

### Phase 4: Preview and Publish

Show the full PRD to the user and ask:

> **Where should I publish this?**
>
> **a)** Write to `docs/prd/{YYYY-MM-DD}-{slug}.md` in the project
> **b)** Create a GitHub issue (uses `gh` CLI)
> **c)** Create a Linear issue (uses `lineark` CLI)

**Wait for the user to choose.**

---

### Option A: Write to docs/

```bash
mkdir -p docs/prd
```

Write to `docs/prd/{YYYY-MM-DD}-{slug}.md` where `{slug}` is kebab-case (max 50 chars).

Confirm the file path to the user.

### Option B: GitHub Issue

If priority not given, ask: P0 (critical), P1 (high), P2 (medium), P3 (low).

```bash
gh issue create \
  --title "feat(<module>): <description>" \
  --body "$(cat <<'EOF'
<PRD content>
EOF
)" \
  --label "prd" \
  --label "P2: medium"
```

If the `prd` label doesn't exist:

```bash
gh label create prd --description "Product Requirements Document" --color "0075ca"
```

Report the issue URL.

#### Issue Title Format

```
<type>(<module>): <short description>
```

Examples:
- `feat(engine): add tag filtering on index page`
- `feat(editor): support split-pane resize`
- `fix(minify): preserve pre blocks during HTML minification`
- `refactor(config): extract frontmatter parsing to own function`

Rules: lowercase after prefix, present tense imperative, under 70 chars, module matches `src/engine/*.rs` or `src/editor/*.rs`.

### Option C: Linear Issue

Ask for team key if ambiguous.

```bash
lineark issues create "prd: {feature title}" \
  --team {TEAM_KEY} \
  --description "{PRD content}" \
  -p {0-4 mapping to none/urgent/high/medium/low}
```

Report the issue identifier and URL.

---

## Amending an Issue

When called with `/po amend <issue_number> <summary>`, the PO appends a revision to the existing GitHub issue. **The original PRD is immutable** — never edit or overwrite it.

### Amend Workflow

1. **Fetch** the issue: `gh issue view <number> --json title,body`
2. **Read the summary** from the implementer
3. **Research the codebase** to validate proposed changes
4. **Append a revision block**:

```bash
gh issue edit <number> --body "$(cat <<'EOF'
<original body unchanged>

---

## Revision <N> - <YYYY-MM-DD>

**Source:** Implementer research

### Changes
- what changed and why

### Updated Requirements
- [ ] FR-X: new or modified requirement

### Updated Tasks
- Task N: new or modified task

### Removed/Deferred
- ~~FR-Y: requirement removed or moved to out of scope~~
EOF
)"
```

### Amend Rules

- **Never modify the original PRD text** — append only
- **Increment revision number** (Revision 1, Revision 2, ...)
- **Each revision is self-contained** — reader can understand what changed without diffing
- **Link back to the source** — who requested the change and why
- **Push back when needed**: if the amendment contradicts product goals, surface the disagreement to the user instead of blindly applying it

## Writing Style

- Write for humans, not machines
- Domain language from the project (`Post`, `BlogConfig`, `Buffer`, `Cursor`, `Mode`), not generic product jargon
- Short sentences. No filler. Every paragraph earns its place
- Concrete over abstract. "User sees the preview pane update within 200ms of `:w`" not "improve perceived performance"
- Never use em dashes. Use periods or restructure

## Pipeline

```
/po -> GitHub issue -> /dev -> /review -> /commit -> /pr
         ^                |
         |                | (PRD feedback)
         +--- /po amend --+
```
