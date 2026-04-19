# rami

Tiny macOS menu bar RAM monitor for Nico's M1 Pro MacBook.

## Scope

V1 shows:

- RAM percentage in the menu bar
- RAM used and total in a native dropdown
- Refresh
- Quit

CPU temperature is intentionally deferred until a clean Apple Silicon path is proven.

## Local build

```sh
./scripts/build-app.sh
open rami.app
```

`rami.app` is an accessory app, so it should stay out of the Dock while the live RAM value remains visible in the menu bar.
