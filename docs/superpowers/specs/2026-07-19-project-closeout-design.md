# Project Closeout Design

> **Status: repository closeout shipped (PR #36); public release still gated.**
> Docs, Dependabot, ruleset, and tap scaffolding are done. Tagging `v0.1.2`
> waits on purpose-specific Apple signing/notary secrets. Kept for history.

## Goal

Close the gap between rami's healthy, feature-complete implementation and its
unfinished public release and maintenance surfaces. Keep the shipped app and
its feature boundary unchanged.

## Scope

The repository change is one closeout pull request. It will:

- update the README, crate metadata, GitHub-facing copy, Cask template, build
  guide, and merge guide to describe the shipped Memory, CPU, and GPU monitor;
- replace stale release and test examples with version-safe or current text;
- make the runtime-cost statement in ADR-0003 reproducible from repository
  instructions instead of relying on an inaccessible prior session;
- add weekly Dependabot checks for Cargo and pinned GitHub Actions;
- update only the compatible `bitflags` and `libc` lockfile entries currently
  reported by `cargo update --dry-run --locked`.

The change will not alter Rust application behavior, add features, refactor the
architecture, change the minimum macOS version, or publish the crate.

## External GitHub Work

After the pull request passes the complete repository gate and review:

1. Merge it to `main`.
2. Add a light `main` ruleset that blocks deletion and force-pushes and requires
   the existing `checks` status check, while retaining maintainer bypass for
   recovery.
3. Enable Dependabot security updates if GitHub permits it for the repository.
4. Create the public `proxynico/homebrew-tap` repository with a `Casks/`
   directory ready for the release workflow.
5. Configure the release secrets only from purpose-specific Apple Developer and
   Homebrew credentials. Do not reuse the broad local `gh` token as the tap
   secret.
6. Run the Release workflow manually, verify the signed and notarized DMG, then
   tag `v0.1.2` and verify the GitHub Release, checksum, installer, and Homebrew
   Cask from clean public URLs.

Creating a Developer ID certificate or app-specific Apple password requires the
account holder's Apple authorization. If no existing purpose-specific
credentials are available, stop the release at that exact gate without tagging
or publishing a partial release. All other closeout work should still finish.

## Verification

The repository pull request must pass:

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

The installed app must remain untouched during repository verification. Its
binary hash may be compared with the fresh bundle, but the running process must
not be terminated merely to complete documentation and maintenance work.

The release gate additionally requires successful `codesign`, `notarytool`,
`stapler`, Gatekeeper assessment, checksum verification, installer execution,
and Homebrew Cask resolution against the published `v0.1.2` assets.

## Error Handling and Rollback

- Repository changes remain in a pull request until CI and review pass.
- Never create or move the `v0.1.2` tag before the notarized artifact path is
  ready; tags drive publication and are not a dry-run surface.
- If Apple credentials are unavailable, report the missing credential names and
  leave GitHub Releases unchanged.
- If the branch ruleset blocks the documented maintainer flow, disable or amend
  only that new ruleset; do not weaken repository-wide Actions permissions.
- If the Homebrew tap update fails after a valid GitHub Release, publish the Cask
  manually from the release DMG checksum rather than changing the app artifact.
