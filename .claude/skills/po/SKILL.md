---
name: po
description: Product Owner & Technical Analyst - scans codebase, creates GitHub issues with PRD and implementation tasks. Use when: PRD, feature, issue, create issue, amend issue, what should we build, plan feature, design feature, scope, requirements, user story, SPEC, technical spec, break down, task breakdown.
---

# Product Owner & Technical Analyst

**Scan the codebase, write a PRD with technical breakdown, and open a GitHub issue.**

## Usage

- `/po <prompt>` - Create a GitHub issue from the given feature description
- `/po <prompt> --priority <P0|P1|P2|P3>` - Create issue with explicit priority
- `/po` - Ask user what to build, then create the issue
- `/po amend <issue_number> <summary>` - Append a revision to an existing issue (called by `/dev` when the implementer's research refines the PRD)

If no `--priority` is given, ask the user which priority to assign.

## Workflow

1. **Scan the codebase** to understand current state:
   - Read `src/engine/mod.rs` and `src/editor/mod.rs` to see existing modules
   - Explore relevant source files for what's already implemented
   - Identify gaps, dependencies, and integration points
2. **Write the PRD + technical breakdown** as the issue body (no local file)
3. **Create GitHub issue** with conventional title and PRD body

## Issue Title

Conventional format:

```
feat(<module>): <short description>
```

Examples:
- `feat(engine): add tag filtering on index page`
- `feat(editor): support split-pane resize`
- `fix(minify): preserve pre blocks during HTML minification`
- `refactor(config): extract frontmatter parsing to own function`

Rules:
- Lowercase after prefix
- Present tense imperative ("add", not "added")
- Under 70 characters
- Module name matches `src/engine/*.rs`, `src/editor/*.rs`, or domain area

## Issue Body (PRD + Technical Breakdown)

```markdown
## Overview
What we're building and why.

## Problem Statement
What user problem does this solve?

## User Stories
- As a [user], I want [goal] so that [benefit]

## Current State
What already exists in the codebase relevant to this feature.

## Requirements

### Functional
- [ ] FR-1: Description

### Non-Functional
- [ ] NFR-1: Performance/quality requirement

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
**Test**: `assert_eq!(...)` - what proves this works
**Depends on**: (none or previous task)

#### Task 2: [description]
**Size**: XS/S/M/L
**Test**: `assert_eq!(...)` - what proves this works
**Depends on**: Task 1

### Error Handling
Module-specific error enum with `Display` impl.

### Edge Cases
| Case | Handling |
|------|----------|
| Missing config field | Use default value |

## Acceptance Criteria
Given/When/Then scenarios.

## Out of Scope
What we're explicitly NOT doing.
```

## Task Sizing

| Size | Complexity | Example |
|------|------------|---------|
| XS | Single function, trivial | Module skeleton |
| S | Single function, some logic | `frontmatter()` with new field |
| M | Multiple functions, moderate | New template variable |
| L | Module with integration | New engine module |
| XL | Cross-module feature | Full editor feature with preview |

## Creating the Issue

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

Replace `"P2: medium"` with the chosen priority label (`P0: critical`, `P1: high`, `P2: medium`, `P3: low`).

If the `prd` label doesn't exist, create it first:

```bash
gh label create prd --description "Product Requirements Document" --color "0075ca"
```

Report the issue URL to the user when done.

## Amending an Issue

When called with `/po amend <issue_number> <summary>`, the PO appends a revision to the existing issue. **The original PRD is immutable** - never edit or overwrite it.

### Amend Workflow

1. **Fetch the current issue** using `gh issue view <number> --json title,body`
2. **Read the summary** provided by the implementer
3. **Research the codebase** if needed to validate the proposed changes
4. **Append a revision block** to the issue body:

```bash
gh issue edit <number> --body "$(cat <<'EOF'
<original body unchanged>

---

## Revision <N> - <date>

**Source:** Implementer research

### Changes
- <what changed and why>

### Updated Requirements
- [ ] FR-X: <new or modified requirement>

### Updated Tasks
- Task N: <new or modified task>

### Removed/Deferred
- ~~FR-Y: <requirement removed or moved to out of scope>~~
EOF
)"
```

### Amend Rules

- **Never modify the original PRD text** - append only
- **Increment revision number** (Revision 1, Revision 2, ...)
- **Each revision is self-contained** - reader can understand what changed without diffing
- **Link back to the source** - who requested the change and why
- The PO may push back on the amendment if it contradicts product goals - surface disagreement to the user

## Pipeline

```
/po -> GitHub issue -> /dev -> /review -> /commit -> /pr
         ^                |
         |                | (PRD feedback)
         +--- /po amend --+
```
