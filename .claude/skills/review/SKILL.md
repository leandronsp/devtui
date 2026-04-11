---
name: review
description: "[DevTUI] Multi-agent code review. Spawns parallel security, performance, and quality reviewers for Rust editor + blog engine, then a red team auditor, then presents findings for the user to judge. Accepts a prompt, GitHub issue URL, PRD file, or no args. Use when: review, code review, review this, check my changes, security review, quality check."
---

# Review

Multi-agent code review pipeline with three parallel specialized reviewers, red team audit, and user judgment. Rust-specialized for the DevTUI editor + blog engine.

## Usage

- `/review` - asks what to review
- `/review <prompt>` - review current changes against the prompt
- `/review <url>` - review against a GitHub issue/PR
- `/review <path>` - review against a PRD/spec file

## Workflow

### Phase 1: Understand the review target

**No arguments:** Ask: "What should I review? Describe it, paste an issue URL, or point me to a spec file."

**Prompt:** Use directly as review context.

**URL:** Fetch:
- `gh issue view <number> --json title,body --jq '.title + "\n\n" + .body'`
- `gh pr view <number> --json title,body --jq '.title + "\n\n" + .body'`

**File path:** Read it.

Store as `{review_context}`.

### Phase 2: Get the diff

```bash
git diff main...HEAD
git diff main...HEAD --stat
```

If empty, try `git diff HEAD~1`. If still empty, tell the user there's nothing to review and stop.

Store as `{diff}` and `{diff_stat}`.

### Phase 3: Scout

Launch the `scout` agent:

> Map the DevTUI areas touched by these changed files. Return: architecture (editor vs engine boundary), patterns, conventions, test structure, error handling, project rules from CLAUDE.md and .claude/rules/*.md.
>
> Changed files:
> {diff_stat}

Store as `{scout_context}`.

### Phase 4: Parallel review

Launch **3 agents in parallel** (single message, 3 Agent tool calls):

- **`security-reviewer`**: "Review this PR for security issues.\n\n## Review Context\n{review_context}\n\n## Codebase Context\n{scout_context}\n\n## Diff\n{diff}"
- **`performance-reviewer`**: "Review this PR for performance issues.\n\n## Review Context\n{review_context}\n\n## Codebase Context\n{scout_context}\n\n## Diff\n{diff}"
- **`quality-reviewer`**: "Review this PR for quality issues (design, testing, DDD, SOLID, Rust idioms, minimalism).\n\n## Review Context\n{review_context}\n\n## Codebase Context\n{scout_context}\n\n## Diff\n{diff}"

**CRITICAL**: All 3 in the **same message** so they run concurrently.

Store as `{security_report}`, `{performance_report}`, `{quality_report}`.

### Phase 5: Establish the main review

Synthesize the three reports (you, not a subagent):

1. Merge findings into unified tiers (Critical / High / Medium / Low / Positive)
2. Deduplicate same `file:line` across reviewers
3. Tag each finding: `[security]`, `[performance]`, `[quality]`
4. Present the **main review** to the user before the red team audit

```markdown
## Code Review: {branch name}

**Diff:** {files changed}, {insertions}+, {deletions}-
**Reviewers:** security, performance, quality

### Critical
- [{source}] **{title}** at `{file}:{line}`
  {description}
  **Test (RED first):** {failing test}
  **Fix:** {minimal fix}

### High / ### Medium / ### Low
- ...

### Good patterns
- ...
```

### Phase 6: Red team audit

Once the main review is established, launch the `review-auditor` agent to stress-test it:

> Audit these three code review reports against the actual DevTUI codebase and project rules (CLAUDE.md, .claude/rules/*.md). Verify findings against actual code, check for false positives, blind spots, contradictions, severity miscalibration.
>
> ## Review Context
> {review_context}
>
> ## Security Review
> {security_report}
>
> ## Performance Review
> {performance_report}
>
> ## Quality Review
> {quality_report}

Store as `{audit_report}`.

### Phase 7: Apply the audit

Refine the main review with the auditor's output:

1. Drop findings flagged as false positives
2. Apply severity adjustments
3. Mark high-confidence findings verified
4. Add blind spots as new findings
5. Re-deduplicate

Present the **audit-adjusted review**:

```markdown
## Code Review: {branch name} (audit-adjusted)

**Diff:** {files changed}, {insertions}+, {deletions}-
**Reviewers:** security, performance, quality + red team audit

### Critical
- [{source}] **{title}** at `{file}:{line}`
  {description}
  **Test (RED first):** {failing test}
  **Fix:** {minimal fix}

### High / ### Medium / ### Low
- ...

### Good patterns
- ...

### Audit notes
- {false positives removed and why}
- {severity adjustments made}
- {blind spots added}

**Verdict:** {Critical/High -> "Needs fixes" | Medium/Low only -> "Clean with suggestions" | Nothing -> "Ship it"}
```

### Phase 8: User choice

If verdict is **not** "Ship it":

> **What next?**
>
> **a)** Write full report to `docs/reviews/{branch-name}.md`
> **b)** Address findings (TDD, RED first, baby steps)

**Wait for the user to choose.**

**Option a:** Write the report file.

**Option b:** Prioritized fix list (critical → high → medium). One at a time. Every fix starts with a failing test. Run `cargo test` and `cargo clippy -- -D warnings` after each fix.

If **"Ship it"**: congratulate and stop.

## Principles

- TDD strictly. Every fix starts with RED first
- Baby steps. One fix at a time
- Rust-aware. Reviewers specialize on editor (PTY, TUI, preview) and engine (pipeline, SEO, minify) concerns
- Project-aware. Reviewers and auditor check `CLAUDE.md` and `.claude/rules/*.md`
- Adversarial audit. Red team kills bad findings and finds blind spots, it doesn't add noise

## Pipeline

```
/po -> GitHub issue -> /dev -> /review -> /commit -> /pr
```
