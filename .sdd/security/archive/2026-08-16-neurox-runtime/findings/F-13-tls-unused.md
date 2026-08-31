# Finding: TLS support exists but is unused in production deployments

## Metadata

- ID: SEC-0001-2026-006
- Severity: Low
- STRIDE: T,I
- Discovered: 2026-08-16
- Discovered by: default
- Status: Confirmed (by design)

## Location

- File: `daemon/core/src/main.rs`
- Line: TLS bind via `axum_server::tls_rustls::RustlsConfig::from_pem_file`
- Component: daemon
- Commit: current main

## Description

The daemon has full TLS support (`rustls` integration with axum-server)
but the current local deployment uses plain HTTP on `127.0.0.1:7878`.
This is **by design** because the bind is loopback only — TLS would
add overhead without security benefit.

For production (`api.neurox.pro`), TLS is provided by Cloudflare
Pages/Workers (TLS termination at the edge), so the daemon-internal
TLS layer is not strictly required.

## Impact

- Local loopback: no impact.
- If the daemon is ever exposed directly (e.g. via SSH tunnel,
  reverse proxy, or `bind 0.0.0.0` for LAN access), all traffic is
  in plaintext, including JWT tokens in `Authorization: Bearer` headers.

## Proof of Concept

```bash
tcpdump -i lo -A -s 0 'tcp port 7878' | grep -i "Authorization: Bearer"
```

**Expected output when the bug is present**: Bearer tokens visible
in plaintext on the wire.

## Remediation

For local loopback: **no change** (acceptable per design).
For any future LAN/external exposure:

1. Enable the `rustls` config in the daemon (`tls.cert` and `tls.key`
   in `config.yaml`).
2. OR put a TLS-terminating reverse proxy (Caddy, nginx, traefik) in
   front.

## References

- `daemon/core/src/main.rs` — TLS bind path
- `daemon/core/src/config/mod.rs` — TLS config struct
## Regression Test

N/A — by-design choice. Document the decision in the daemon README.

