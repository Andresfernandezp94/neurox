# Finding: memoryd `/admin/info`, `/admin/storage/stats` and `/admin/health` are public

## Metadata

- ID: SEC-0001-2026-006
- Severity: Medium
- CVSS-equivalent: 4.3
- STRIDE: I
- OWASP ASVS: v5.0.0-13.1.3
- NIST SSDF: PS.3.2
- Discovered: 2026-08-16
- Discovered by: default
- Status: Confirmed

## Location

- File: `mcps/memory/memoryd/src/http/middleware/bearer_auth.rs`
- Line: 33 (`public_paths: Arc::new(vec!["/health"])`)
- File: `mcps/memory/memoryd/src/main.rs`
- Lines: 64-75 (`/admin/info`, `/admin/storage/stats` registered as public)
- Component: memoryd
- Commit: current main

## Description

The bearer auth middleware in memoryd only protects some `/admin/*`
endpoints. The following are intentionally public (no Bearer required):

- `GET /health` — liveness probe
- `GET /admin/info` — server version, build, uptime
- `GET /admin/storage/stats` — total entries per scope
- `GET /admin/embeddings/config` — endpoints, model, dim (api_key
  scrubbed post-EP-0008)

The `/admin/info` and `/admin/storage/stats` are public by design
(per the Taskfile comments in `mcps/memory/Taskfile.yml:228-231`).
However, the choice means:

- Any local caller (or anyone on LAN if the bind changes) can probe
  the size of your memory store (`SELECT count(*) FROM memories`).
- The embeddings endpoint host is leaked (so an attacker knows which
  model is being used and can target prompt-injection via crafted
  text that becomes a malicious embedding — see also F-09).

This is **intentional design** (the comment says "GET /admin/info
(public)"), but it widens the reconnaissance surface.

## Impact

- **Information disclosure**: model name, embedding endpoint, total
  memory count, DB version, uptime.
- **Reconnaissance**: an attacker on loopback can size up the
  deployment before attempting privilege escalation.

## Proof of Concept

```bash
# Without any Bearer token:
curl -s http://127.0.0.1:9999/admin/info
curl -s http://127.0.0.1:9999/admin/storage/stats
```

**Expected output when the bug is present**:
```json
{"version":"...","db_size_bytes":1531904,"workspaces":["workspace-1"], ...}
{"total_memories":196,"by_scope":{"agent":{"default":17,"default":179}}}
```

## Remediation

Document the design decision explicitly in the ADR / constitution,
and decide per-endpoint which public endpoints leak the least
information. At minimum:

1. **`/admin/storage/stats`** should require Bearer — it leaks the
   *size* of the memory store, which is reconnaissance.
2. **`/admin/info`** is OK to keep public (it's just version + uptime).
3. **`/admin/embeddings/config`** should require Bearer — the model +
   endpoint host is enough to plan a prompt-injection attack.

```diff
--- a/mcps/memory/memoryd/src/main.rs
+++ b/mcps/memory/memoryd/src/main.rs
 let public_paths = vec![
     "/health",
     "/admin/info",
-    "/admin/storage/stats",
-    "/admin/embeddings/config",
+    // /admin/storage/stats and /admin/embeddings/config moved to protected.
 ];
```

## References

- `mcps/memory/memoryd/src/http/middleware/bearer_auth.rs:33`
- `mcps/memory/memoryd/src/main.rs:64-75`
- `mcps/memory/memoryd/src/http/admin.rs` — handler definitions
- EP-0008 (memory_share + admin hardening) — past fix
## Regression Test

```bash
# /admin/storage/stats without Bearer should be 401
curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:9999/admin/storage/stats
[ "$(...)" = "401" ] || { echo "FAIL"; exit 1; }
```

