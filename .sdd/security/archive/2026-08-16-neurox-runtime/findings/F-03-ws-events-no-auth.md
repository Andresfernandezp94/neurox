# Finding: WebSocket `/v1/events` has NO authentication

## Metadata

- ID: SEC-0001-2026-003
- Severity: High
- CVSS-equivalent: 7.5
- STRIDE: S,I
- OWASP ASVS: v5.0.0-7.4.1
- NIST SSDF: PS.3.2
- Discovered: 2026-08-16
- Discovered by: default
- Status: Confirmed

## Location

- File: `daemon/core/src/router/ws.rs`
- Line: 19-32 (`pub async fn ws_events`)
- Component: daemon (WebSocket event stream)
- Commit: current main

## Description

The WebSocket endpoint `/v1/events` accepts upgrades from **any** client
without authentication. The handler reads only `user_agent` and
`ConnectInfo(addr)`; it does NOT verify a Bearer token, JWT, or
session_id cookie before calling `ws.on_upgrade(...)`.

Same applies to `/v1/commands` (visible in `daemon/core/src/router/mod.rs:1488`).

In contrast, the legacy `AuthLayer` (api_token) and `JwtAuthLayer` ARE
applied to the REST routes, but the WS upgrade happens before those
layers can act on the request.

## Impact

Any actor who can connect to the daemon (currently loopback only —
`127.0.0.1:7878`) can:

1. Subscribe to the global event stream and observe:
   - Session creation/deletion
   - Agent spawn events (`session_agents` spawn PIDs are logged as events)
   - System monitor stats (every 5s, EP-0003)
   - Capability toggles
   - Voice call state changes
2. (If `/v1/commands` is similarly unprotected) send arbitrary
   commands to running sessions, including the embedded `agent` agent.

The blast radius is bounded by loopback binding, but is a **clear
information disclosure + potential command injection** if the bind
ever changes.

## Proof of Concept

```bash
# Connect without any credentials:
websocat ws://127.0.0.1:7878/v1/events
```

**Expected output when the bug is present**:
A continuous stream of JSON events:
```json
{"type":"system_stat","ts":"2026-08-16T21:00:00Z","cpu":{...}}
{"type":"session_created","session_id":"...","agent_id":"admin"}
```

## Remediation

Apply the same `JwtAuthLayer`/`AuthLayer` to the WS endpoints. For
WebSocketUpgrade, the auth check has to happen inside the handler
**before** calling `on_upgrade` (or by rejecting the upgrade with
`401 Unauthorized`).

```diff
--- a/daemon/core/src/router/ws.rs
+++ b/daemon/core/src/router/ws.rs
 pub async fn ws_events(
+    headers: HeaderMap,  // existing
     ws: WebSocketUpgrade,
     State(state): State<Arc<AppState>>,
     ConnectInfo(addr): ConnectInfo<SocketAddr>,
-) -> impl IntoResponse {
-    let user_agent = headers
-        .get("user-agent")
-        ...
+) -> impl IntoResponse {
+    // Verify Bearer/JWT before upgrading
+    let token = headers.get("authorization")
+        .and_then(|v| v.to_str().ok())
+        .and_then(|s| s.strip_prefix("Bearer "));
+    if !crate::auth::handlers::verify_token(state.as_ref(), token) {
+        return (axum::http::StatusCode::UNAUTHORIZED, "Bearer required").into_response();
+    }
+    let user_agent = headers.get("user-agent")...
```

And in `daemon/core/src/router/mod.rs`, route the WS through the auth
layer:
```diff
-        .route("/v1/events", get(ws::ws_events))
+        .route("/v1/events", get(ws::ws_events))
+        // apply AuthLayer/JwtAuthLayer to WS the same as REST
```

## References

- `daemon/core/src/router/ws.rs:19`
- `daemon/core/src/router/mod.rs:1488` — route registration
- `daemon/core/src/auth/tokens.rs` — has the verify function we can reuse
## Regression Test

```bash
# Without auth → should be 401 (or close the WS)
websocat -n1 ws://127.0.0.1:7878/v1/events 2>&1 | head -3
[ "$?" = "0" ] && { echo "FAIL: WS accepted without auth"; exit 1; }
echo "OK: WS requires auth"
```

