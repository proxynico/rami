# Remove Runtime `sfltool` Use

> **Status: shipped (2026-07).** Design accepted and implemented on `main`.
> Kept for history; do not reopen without a new ADR-level reason.

## Problem

Rami runs `sfltool dumpbtm` when `SMAppService` reports launch at login as
disabled. It starts that diagnostic subprocess when the app opens and repeats
the check every five minutes. On the current macOS release, this can present an
administrator password prompt attributed to `sfltool`.

Apple treats `sfltool dumpbtm` as a diagnostic command for inspecting
Background Task Management state. Normal app startup must not invoke it.

## Design

Use `SMAppService.mainAppService().status` as the only launch-at-login status
source. Keep the existing register and unregister actions unchanged. Remove the
`EnabledExternal` status and all external-dump parsing, path encoding, caching,
threading, and subprocess code that only supported it.

The Settings item will report and control registrations created through Rami.
It will no longer detect the uncommon case where the user separately adds Rami
as a legacy login item through System Settings. That loss is preferable to a
recurring password prompt and removes reliance on a diagnostic interface.

## Testing

Add a source-level regression test that scans the production Rust sources and
fails if `sfltool` appears. Keep the existing status mapping and menu-state unit
tests for `Disabled`, `Enabled`, `RequiresApproval`, and `Unavailable`. Run the
repository's required formatting, test, clippy, and app-bundle checks.
