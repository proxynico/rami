# Project Closeout Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Align rami's public project surface with the shipped app, establish low-noise maintenance automation, protect `main`, and publish a verified `v0.1.2` release when purpose-specific Apple credentials are available.

**Architecture:** Keep Rust application behavior unchanged. Put all repository-file changes in one closeout pull request, merge it only after the full local and GitHub gates pass, then apply repository settings and release operations against the merged commit.

**Tech Stack:** Rust/Cargo, Markdown, YAML, GitHub Actions, Dependabot, `gh`, Apple `codesign`/`notarytool`/`stapler`, Homebrew Cask.

## Global Constraints

- Preserve the one-`NSStatusItem`, native-`NSMenu`, pressure-driven Accent contracts.
- Do not add features, refactor Rust code, change macOS 14+ / Apple Silicon support, or publish the crate.
- Use Rust 1.95.0 from `rust-toolchain.toml` and the existing Cargo lockfile.
- Keep the installed and running `/Applications/rami.app` untouched during repository verification.
- Never create or move tag `v0.1.2` until the notarized-artifact path is ready.
- Never reuse the broad local `gh` token as `HOMEBREW_TAP_TOKEN`.

---

### Task 1: Align public copy and durable project records

**Files:**
- Modify: `README.md`
- Modify: `Cargo.toml`
- Modify: `Casks/rami.rb`
- Modify: `BUILDING.md`
- Modify: `docs/agents/merge-flow.md`
- Modify: `docs/adr/0003-feature-complete.md`

**Interfaces:**
- Consumes: the shipped Memory, CPU, GPU, process-row, memory-history, diagnostics, settings, and single-status-item behavior documented by `CONTEXT.md` and the ADRs.
- Produces: consistent public copy for GitHub, Cargo metadata, the future Homebrew Cask, contributor verification, and the feature-complete decision.

- [ ] **Step 1: Update README product and release copy**

Describe rami as a Memory, CPU, and GPU menu-bar monitor; name the single gauge and bounded dropdown; remove the stale fixed-orange wording; retain the honest statement that public install is not live until the release succeeds.

- [ ] **Step 2: Update package and Cask metadata**

Change the Cargo and Cask descriptions from memory-only to system-monitor wording, add `cpu` and `gpu` Cargo keywords while staying within Cargo's five-keyword limit, and set the Cask template version to `0.1.2`.

- [ ] **Step 3: Make release instructions version-safe**

Replace the hard-coded `v0.1.1` tag example in `BUILDING.md` with `<version>` commands and state that the tag must match `Cargo.toml`.

- [ ] **Step 4: Record a reproducible runtime check**

Add a short `ps`-based before/after sampling procedure to `BUILDING.md` using PID, elapsed time, cumulative CPU time, `%CPU`, and RSS. Update ADR-0003 to cite that repository procedure and avoid unsupported exact figures from an inaccessible prior session.

- [ ] **Step 5: Correct the verification baseline**

Update `docs/agents/merge-flow.md` from 112 to 133 normal tests plus 4 ignored local-only smoke tests.

- [ ] **Step 6: Check copy consistency**

Run:

```sh
rg -n "tiny.*memory monitor|memory-only|orange jump|v0\.1\.1|112 tests" README.md Cargo.toml Casks/rami.rb BUILDING.md docs/agents docs/adr
git diff --check
```

Expected: no stale product/version/test copy; `git diff --check` exits 0.

### Task 2: Add maintenance automation and patch the lockfile

**Files:**
- Create: `.github/dependabot.yml`
- Modify: `Cargo.lock`

**Interfaces:**
- Consumes: Cargo as the only dependency ecosystem and SHA-pinned Actions in `.github/workflows/`.
- Produces: weekly, low-volume update pull requests for Cargo and GitHub Actions; no application API changes.

- [ ] **Step 1: Add Dependabot configuration**

Create version 2 configuration with weekly Monday checks for `cargo` and `github-actions`, rooted at `/`, timezone `Asia/Hong_Kong`, and an open-PR limit of 5 for each ecosystem.

- [ ] **Step 2: Apply only the observed compatible patch updates**

Run:

```sh
cargo update -p bitflags --precise 2.13.1
cargo update -p libc --precise 0.2.186
```

Expected: only `bitflags` and `libc` lockfile packages change.

- [ ] **Step 3: Verify dependency scope**

Run:

```sh
git diff -- Cargo.lock .github/dependabot.yml
cargo tree --duplicates
cargo audit
```

Expected: no duplicate packages and no RustSec vulnerabilities.

### Task 3: Verify and publish the closeout pull request

**Files:**
- Modify: `docs/superpowers/plans/2026-07-19-project-closeout.md` only to check completed steps if useful during execution.

**Interfaces:**
- Consumes: Tasks 1-2 repository diff.
- Produces: one reviewed, CI-green pull request merged into `main`.

- [ ] **Step 1: Run the complete local gate**

Run:

```sh
cargo fmt --all -- --check
cargo test --locked
cargo test --locked -- --ignored
cargo clippy --locked --all-targets -- -D warnings
cargo audit
./scripts/build-app.sh
codesign --verify --deep --strict rami.app
git diff --check
```

Expected: 133 normal tests and all 4 live smoke tests pass; all other commands exit 0.

- [ ] **Step 2: Confirm the live app was not changed**

Compare SHA-256 hashes for `rami.app/Contents/MacOS/rami` and `/Applications/rami.app/Contents/MacOS/rami`. Do not install or relaunch during this task.

- [ ] **Step 3: Review the complete diff**

Review changes against `docs/superpowers/specs/2026-07-19-project-closeout-design.md`. Resolve all Critical and Important findings before publishing.

- [ ] **Step 4: Commit and publish**

Stage only the planned paths, commit with `Close out project maintenance surfaces`, push `agent/project-closeout`, and open a draft PR to `main` containing scope, rationale, impact, and exact checks.

- [ ] **Step 5: Verify GitHub CI and merge**

Mark the PR ready after review, wait for the `checks` job to pass on the PR head, squash-merge it, and confirm `origin/main` contains the merged commit with no open closeout PR.

### Task 4: Apply GitHub repository maintenance settings

**Files:**
- No repository files.

**Interfaces:**
- Consumes: merged closeout PR and existing `checks` CI job.
- Produces: public metadata matching the README, Dependabot alerts enabled, and a recoverable `main` ruleset.

- [ ] **Step 1: Update GitHub repository metadata**

Set the description to `A restrained macOS menu bar system monitor for Memory, CPU, and GPU.` Keep the homepage on the repository itself until a public release exists.

- [ ] **Step 2: Enable Dependabot security updates**

Enable vulnerability alerts and automated security fixes through the GitHub API, then read back `security_and_analysis` to confirm the state.

- [ ] **Step 3: Create the main ruleset**

Create one active branch ruleset targeting the default branch. Block branch deletion and non-fast-forward pushes, require the existing `checks` status check, and give the repository administrator role an `always` bypass so recovery remains possible.

- [ ] **Step 4: Verify settings**

Read back repository metadata, security settings, and rulesets. Confirm normal pull requests remain possible and direct destructive updates are blocked.

### Task 5: Prepare the Homebrew tap and release credentials

**Files:**
- External repository: `proxynico/homebrew-tap/Casks/rami.rb`

**Interfaces:**
- Consumes: merged `Casks/rami.rb` template and purpose-specific GitHub/Apple credentials.
- Produces: a public tap repository and configured rami release secrets without broad-token reuse.

- [ ] **Step 1: Create the tap repository if absent**

Create public `proxynico/homebrew-tap` with a README, add the merged rami Cask template under `Casks/rami.rb`, and push its default branch.

- [ ] **Step 2: Locate purpose-specific credentials**

Check the local signing identities and approved secret store for:

- a `Developer ID Application` certificate and exportable `.p12`;
- its `.p12` password;
- Apple notary Apple ID, team ID, and app-specific password;
- a fine-grained Homebrew tap token limited to `proxynico/homebrew-tap` contents write.

Do not print secret values. If any Apple credential is absent, stop Task 5 at this gate and report the exact missing items.

- [ ] **Step 3: Configure GitHub Actions secrets**

Set the seven secret names documented in `BUILDING.md` by piping values directly to `gh secret set`; never place secret values in command arguments, files inside the repository, logs, or the final report.

- [ ] **Step 4: Confirm names only**

Run `gh secret list -R proxynico/rami` and confirm all required names exist. Do not attempt to read secret values back.

### Task 6: Publish and verify `v0.1.2`

**Files:**
- No source changes.

**Interfaces:**
- Consumes: merged `main`, configured release secrets, public tap repository, and version `0.1.2` in `Cargo.toml`.
- Produces: notarized GitHub DMG and checksum assets, updated Homebrew Cask, and verified public installation surfaces.

- [ ] **Step 1: Run a manual release dry run**

Dispatch the `Release` workflow on `main`. Require the release job to build, Developer ID-sign, notarize, staple, pass Gatekeeper, and upload its manual-run artifact. Download the artifact and verify the DMG signature and checksum locally.

- [ ] **Step 2: Create and push the release tag**

Confirm `Cargo.toml` is `0.1.2`, `v0.1.2` does not exist, `main` is green, and the manual release passed. Create annotated tag `v0.1.2` on the merged `main` commit and push that exact tag.

- [ ] **Step 3: Verify the tag-triggered release**

Wait for both Release jobs to pass. Confirm GitHub Release `v0.1.2` contains `rami-0.1.2.dmg` and `rami-0.1.2.dmg.sha256`, the checksum matches, the DMG is notarized and stapled, and the tap Cask contains version `0.1.2` with the published checksum.

- [ ] **Step 4: Verify public consumption without disrupting the running app**

Download the installer script and release assets from their public URLs into a temporary directory. Verify installer resolution, checksum logic, DMG mount, bundle version, Developer ID signature, notarization ticket, and Gatekeeper assessment. Do not overwrite `/Applications/rami.app` while its current process is running.

- [ ] **Step 5: Final status check**

Confirm the repository homepage now safely points to the working latest Release, open issue and PR queues are empty, `main` CI is green, the release and tap URLs return successfully, and the configured checkout is clean on `main`.
