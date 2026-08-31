# Finding: CORS `Any` on daemon (loopback only, but still risky)

## Metadata

- ID: SEC-0001-2026-006
- Severity: Low
- CVSS-equivalent: 3.1
- STRIDE: T,I
- OWASP ASVS: v5.0.0-13.2.3
- NIST SSDF: PS.3.2
- Discovered: 2026-08-16
- Discovered by: default
- Status: Confirmed

## Location

- File: `daemon/core/src/router/mod.rs`
- Line: 1485-1488 (`CorsLayer::new().allow_origin(Any).allow_methods(Any).allow_headers(Any)`)
- Component: daemon
- Commit: current main

## Description

The daemon configures CORS with `allow_origin(Any)`, `allow_methods(Any)`,
and `allow_headers(Any)`. This means any browser origin can make
cross-origin requests to the daemon.

In the current deployment, all daemon endpoints bind to `127.0.0.1`,
which means cross-origin requests can only be made by a browser
running on the same machine. The `Origin` header in cross-origin
requests is still `null` or the requesting app's origin, but CORS
does not block the request — only the browser blocks the *response*
from being read by the JS. Without credentials (`allow_credentials`
not set), this is largely informational.

However:
- If the daemon is ever exposed via a Cloudflare tunnel or reverse
  proxy (with `bind 0.0.0.0` or a domain), any malicious site can
  fetch `/v1/*` endpoints from a victim's browser if they have a
  valid session token (e.g. via an XSS in the SPA).
- The current config implicitly trusts all origins, which is the
  opposite of zero-trust.

## Impact

Bounded today by loopback bind. Could become a vector for CSRF-like
attacks on the daemon if the bind is changed or if the SPA is
served from multiple origins in the future.

## Proof of Concept

```bash
# From any browser context (e.g. evil.com):
fetch("http://127.0.0.1:7878/v1/sessions", { credentials: "include" })
```

**Expected output when the bug is present**: the browser permits the
request (and blocks the response read in preflight cases). The
daemon does not enforce an Origin allowlist.

## Remediation

Restrict `allow_origin` to the SPA's actual origin
(`http://localhost:5173` in dev, `https://api.neurox.pro` in prod):

```diff
--- a/daemon/core/src/router/mod.rs
+++ b/daemon/core/src/router/mod.rs
-let cors = tower_http::cors::CorsLayer::new()
-    .allow_origin(tower_http::cors::Any)
-    .allow_methods(tower_http::cors::Any)
-    .allow_headers(tower_http::cors::Any);
+let cors = tower_http::cors::CorsLayer::new()
+    .allow_origin([
+        "http://localhost:5173".parse().unwrap(),     // vite dev
+        "https://api.neurox.pro".parse().unwrap(),    // CF Pages prod
+    ])
+    .allow_methods([axum::http::Method::GET, axum::http::Method::POST,
+                    axum::http::Method::PUT, axum::http::Method::DELETE,
+                    axum::http::Method::PATCH])
+    .allow_headers([axum::http::header::CONTENT_TYPE,
+                    axum::http::header::AUTHORIZATION])
+    .allow_credentials(true);
```

## References

- `daemon/core/src/router/mod.rs:1485-1488`
- tower-http `CorsLayer` docs
## Regression Test

```bash
# Cross-origin POST without preflight should be blocked by browser.
# Server-side: respond with Access-Control-Allow-Origin matching only configured origins.
curl -sI -X OPTIONS http://127.0.0.1:7878/v1/sessions \
  -H "Origin: https://evil.example" \
  -H "Access-Control-Request-Method: GET" | grep -i "access-control-allow-origin"
# Expected: NOT present (or specific origin, NOT "*")
```

