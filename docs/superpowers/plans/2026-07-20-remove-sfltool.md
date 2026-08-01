# Remove Runtime `sfltool` Use Implementation Plan

> **Status: shipped (2026-07).** Landed as `fix: stop invoking sfltool at runtime`
> on `main`. Kept for history; do not re-execute.

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Stop Rami from invoking `sfltool` during normal app use while preserving launch-at-login control through `SMAppService`.

**Architecture:** Make the existing Service Management adapter the sole source of launch-at-login state. Delete the external Background Task Management dump path and guard the boundary with a source-level regression test.

**Tech Stack:** Rust, objc2, macOS ServiceManagement.

## Global Constraints

- Keep `SMAppService` registration, unregistration, and status mapping intact.
- Do not add dependencies or explicit `any` types.
- Do not retain any runtime `sfltool` invocation.
- Verify with `cargo fmt`, `cargo test`, `cargo clippy --all-targets -- -D warnings`, and `./scripts/build-app.sh`.

---

### Task 1: Remove the diagnostic subprocess

**Files:**
- Modify: `src/login_item.rs`
- Modify: `src/diagnostics.rs`
- Create: `tests/no_diagnostic_subprocess.rs`

**Interfaces:**
- Consumes: `SMAppService::status`, `SMAppService::register_and_return_error`, and `SMAppService::unregister_and_return_error`.
- Produces: `LaunchAtLoginController::status() -> LaunchAtLoginStatus` and `LaunchAtLoginController::toggle() -> Result<LaunchAtLoginStatus, Retained<NSError>>` without an external command.

- [x] **Step 1: Write the failing regression test**

```rust
#[test]
fn login_item_controller_does_not_invoke_the_btm_diagnostic() {
    let forbidden_command = ["sfl", "tool"].concat();
    let source = include_str!("../src/login_item.rs");

    assert!(
        !source.contains(&forbidden_command),
        "the launch-at-login adapter must not invoke the BTM diagnostic"
    );
}
```

- [x] **Step 2: Run the test and verify the current code fails it**

Run: `cargo test --test no_diagnostic_subprocess`

Expected: FAIL with `the launch-at-login adapter must not invoke the BTM diagnostic`.

- [x] **Step 3: Remove the external status path**

Keep the controller limited to the supported framework API:

```rust
pub struct LaunchAtLoginController {
    service: Retained<SMAppService>,
}

impl LaunchAtLoginController {
    pub fn new() -> Self {
        Self {
            service: SMAppService::main_app_service(),
        }
    }

    pub fn status(&self) -> LaunchAtLoginStatus {
        self.service.status().into()
    }
}
```

Delete `EnabledExternal`, its menu behavior, the cache, background thread, URL
encoding, dump parsing, `Command::new`, and their obsolete unit tests. Preserve
`current_app_bundle_path` and its helper because diagnostics also use them. Keep
the existing `Disabled`, `Enabled`, `RequiresApproval`, and `Unavailable`
behavior. Remove the obsolete external-state label from diagnostics and use the
supported `Enabled` state in its report fixture.

- [x] **Step 4: Run focused and full verification**

Run:

```bash
cargo test --test no_diagnostic_subprocess
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
./scripts/build-app.sh
codesign --verify --deep --strict rami.app
```

Expected: all commands exit 0; the app bundle satisfies its designated requirement.

- [x] **Step 5: Commit the implementation**

```bash
git add src/login_item.rs src/diagnostics.rs tests/no_diagnostic_subprocess.rs
git commit -m "fix: stop invoking sfltool at runtime"
```
