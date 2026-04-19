# rami

Tiny macOS menu bar RAM monitor for Nico's M1 Pro MacBook.

## What v1 does

RAM-only v1 intentionally:

- shows RAM percentage in the menu bar
- uses a plain dropdown for RAM used / total
- includes Refresh and Quit
- stays RAM-only for v1 on purpose

## Dev

```sh
cargo test
cargo run
```

## Build the .app

```sh
./scripts/build-app.sh
open rami.app
```
