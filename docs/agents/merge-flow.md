# Merge Flow

How work moves from a triaged issue to `main` in this repo. Agreed 2026-07-18
during the codebase-audit pass that produced issues #17–#25.

## One PR per issue

No batching. Each issue gets its own branch and its own PR, even when several
land in the same area. Issues that bundle sub-items (e.g. a cleanup ticket with
four independent items) may be split into multiple PRs, never merged into one
with another issue.

## Who implements what

| Work type | Implementer | Notes |
| --- | --- | --- |
| Mechanical, fully specified | Codex (`gpt-5.6-terra`) | Escalate to `gpt-5.6-sol` if terra's output misses the bar |
| Judgment, design, UI | Claude | Anything where the seam or the visual call is the hard part |

Parallel Codex runs use `isolation: 'worktree'` so edits don't collide in the
shared checkout. Codex never commits — it produces a diff, Claude reviews it.

The `ready-for-agent` / `ready-for-human` labels (see `triage-labels.md`) carry
this split on the tracker.

## Verification gate

Claude reviews everything before merge, regardless of who wrote it, and runs:

```sh
cargo fmt
cargo test
cargo clippy --all-targets -- -D warnings
RAMI_INSTALL=1 ./scripts/build-app.sh    # then spot-check the rebuilt app
```

All four must pass. The rebuilt-app check is not optional for anything with a
runtime surface — see the warning below.

**Baseline at time of writing:** 112 tests pass (4 ignored, local-only);
clippy and fmt clean.

## Rebuilt-app check: read this

`scripts/build-app.sh` builds the repo-local bundle by default. Install and
relaunch are opt-in with `RAMI_INSTALL=1`, so a bare invocation leaves an
already-running app stale — meaning a spot check can inspect an old binary.

The script now warns when a bare development build detects a running `rami`.
Use `RAMI_INSTALL=1 ./scripts/build-app.sh` for visual verification, then
confirm its output reports the terminated PID(s), binary mtime, and relaunch.

## Review split

Nico audits **the tracker, not the PRs**. Issue hygiene, sequencing, and scope
are his surface; diff-level review is Claude's. Escalate a PR to him only when it
changes agreed scope, contradicts an ADR, or the fix turned out far larger than
the ticket implied.

Time-to-fix is an architecture signal. If a "small" ticket takes hours or touches
many files, say so in the PR — don't just merge it.

## Sequencing

Blocking edges are native GitHub issue dependencies, not prose. Query them:

```sh
gh api repos/proxynico/rami/issues/<n> --jq .issue_dependencies_summary.blocked_by
```

A ticket is workable when that returns `0`. Don't start a blocked ticket because
the blocker "looks nearly done".

## Process routing

This repo follows the engineering-process skill set. For the audit queue
specifically:

- Issues #17–#25 were authored from an audit, not filed by outside reporters —
  **do not run `/triage` on them**. Triage is for issues you didn't create.
- They are already ticket-shaped with acceptance criteria, so `/to-spec` and
  `/to-tickets` do not apply. Go straight to `/implement`, fresh context per
  ticket.
- Exception: #24 (extract `RefreshEngine`) is design work, not a specified
  defect. Run `/codebase-design` on the seam before implementing it.

## macOS / tooling notes

Session-specific knowledge that cost time to discover:

- rami's status item is **`menu bar 1`**, not `menu bar 2`, in the accessibility
  tree. Its accessibility label carries the live reading, which makes it
  scriptable without OCR.
- `codex exec --sandbox danger-full-access` (the `codex-computer-use` skill) is
  blocked by the permission classifier in some environments. Working fallback for
  driving the menu: pre-schedule a background `screencapture` / Escape, then run
  the blocking AppleScript click on
  `menu bar item 1 of menu bar 1 of process "rami"`.
- Warning and Critical accent states are unreachable without exhausting real
  memory. Issue #22 adds `RAMI_FORCE_PRESSURE` for this; use it once merged.
- Screenshots taken during UI runs may capture unrelated windows. Check a capture
  before attaching it anywhere outside the repo.
