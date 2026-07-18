# rami — agent guide

macOS menu bar system monitor (memory, CPU, GPU) in Rust + objc2. Single
status item; everything renders in the NSMenu dropdown. Read `CONTEXT.md`
for the domain vocabulary and `docs/adr/` for standing decisions before
changing behavior.

Verify with `cargo fmt`, `cargo test`, and
`cargo clippy --all-targets -- -D warnings`; build the app bundle with
`./scripts/build-app.sh` (see BUILDING.md). The ignored integration tests
(`cargo test -- --ignored`) are local-only.

## Agent skills

### Issue tracker

GitHub Issues on `proxynico/rami` via the `gh` CLI. See `docs/agents/issue-tracker.md`.

### Triage labels

The five canonical triage labels, unmodified (`needs-triage`, `needs-info`,
`ready-for-agent`, `ready-for-human`, `wontfix`). See `docs/agents/triage-labels.md`.

### Domain docs

Single-context: `CONTEXT.md` + `docs/adr/` at the root. See `docs/agents/domain.md`.
