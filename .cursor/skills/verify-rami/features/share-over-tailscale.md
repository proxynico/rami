# Share over Tailscale

Settings → Share over Tailscale makes rami answer on `127.0.0.1:47960` with this Mac's shared reading. It is off by default and persists as `shareOverTailscale`.

## Sub-features

- `share-on` starts a listener on `127.0.0.1:47960` only.
- `share-reading` answers any HTTP request with `rami-share 1`, then `key=value` lines: `name`, the memory snapshot, `cpu_user_percent` and `cpu_system_percent` from the second request on, and `gpu_*` when the driver reports them.
- `share-off` stops the listener and frees the port.
- `share-tailnet` reaches the same answer through the Mac's Tailscale address once `tailscale serve --bg --tcp 47960 tcp://localhost:47960` is set.

## How to get to it (user POV)

- Click the gauge, then `Settings`, then `Share over Tailscale`.
- From another tailnet device: `curl http://<mac>:47960/`.

## Driving it with control-rami

Preconditions:

- `control-rami doctor` prints `ok=yes`.
- `curl -s -m 3 http://127.0.0.1:47960/` fails before you turn sharing on.

- **Turn on.** Run `control-rami click-settings --item "Share over Tailscale"`. `defaults read com.nicomontero.rami shareOverTailscale` is `1`, and `lsof -nP -iTCP:47960 -sTCP:LISTEN` shows rami on `127.0.0.1:47960` only.
- **Read.** Run `curl -s -m 3 http://127.0.0.1:47960/` twice, 5 s apart, into `.cursor/skills/verify-rami/artifacts/share/local-1.txt` and `local-2.txt`. Both start with `rami-share 1`; the second has `cpu_user_percent`.
- **Compare.** Run `capture-open-menu` into `artifacts/share/menu.ax.txt`. `pressure_percent` and `swap_used_bytes` match the dump's Pressure and Swap within a few seconds' drift.
- **Tailnet.** When `tailscale serve status` lists `tcp://localhost:47960`, `curl -s -m 3 http://<tailscale-ip>:47960/` returns the same shape.
- **Turn off.** Run the same `click-settings` again. The key is `0` and `curl` to port 47960 fails.

## Gotchas

- `tailscale serve` outlives rami. It is the user's one-time setup; leave it in place after a run.
- Cleanup restores defaults, so the next launch starts with sharing off.
