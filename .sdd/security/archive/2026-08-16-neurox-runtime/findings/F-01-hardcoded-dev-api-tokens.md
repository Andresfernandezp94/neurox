# Finding: Hardcoded dev API tokens committed to public repo (systemd units)

## Metadata

- ID: SEC-0001-2026-001
- Severity: Critical
- CVSS-equivalent: 9.1
- STRIDE: S,I
- OWASP ASVS: v5.0.0-2.10.4
- NIST SSDF: PS.3.2
- Discovered: 2026-08-16
- Discovered by: default
- Status: Confirmed

## Location

- File: `mcps/memory/systemd/neurox-memory.service`
- Line: 16
- Component: workspace (all MCP systemd units + daemon docs + Taskfile.yml)
- Commit: `b0eb4ed` (workspace) and various per-repo commits

Also found at:
- `mcps/voice/systemd/neurox-voice.service` line 16
- `mcps/clickup/systemd/neurox-clickup.service` line 16
- `mcps/playwright/systemd/neurox-playwright.service` line 16
- `daemon/docs/integrations/memory.md` (multiple lines)
- `mcps/memory/Taskfile.yml` line 238

## Description

The repository `Andresfernandezp94/neurox-*` is public on GitHub. Five (5)
hardcoded `--api-token neurox-*-dev-2026` values are committed in systemd
unit files and task definitions. These tokens are real authentication
material shared between the daemon and each MCP plugin at runtime. Any
attacker who reads the public repo can authenticate to a running
deployment that uses these tokens.

## Impact

If a deployment uses these "dev" tokens in production (which the current
local dev does, and the README/Taskfile encourages), any actor who
reads the public repo can:

- Authenticate to the daemon as a recognized plugin (memory, voice,
  clickup, playwright) and exercise its full API surface.
- Read, modify, and delete memories (via memoryd).
- Initiate voice calls (via voiced).
- Drive the browser (via playwright-d).
- Read ClickUp data (via clickup-d).

The blast radius is bounded by the fact that all services bind to
`127.0.0.1`. The tokens become exploitable only if: (a) the service
bind is changed to `0.0.0.0`, (b) the daemon is exposed via a reverse
proxy, or (c) the attacker has local shell access. None of those are
the default, but all are realistic.

The naming "dev" is misleading — the tokens are the same in dev and
the deployment that ships to `mcp.neurox.pro` (per the README).

## Proof of Concept

```bash
# 1. Anyone can extract the tokens from the public repo
rg "neurox-.*-dev-2026" \
   /home/andres_fernandez/Proyectos/neurox/{mcps/*,daemon}/ \
   -g '!target' -g '!.git'

# 2. The token is the same used by the daemon's legacy auth path
#    (AuthLayer in router/mod.rs uses the api_token from config)
TOKEN="neurox-memory-dev-2026"

# 3. With network access (e.g. exposed via Cloudflare Tunnel or LAN):
curl -s http://localhost:7878/v1/sessions \
  -H "Authorization: Bearer $TOKEN"
# → 200 OK with session list
```

**Expected output when the bug is present**:
```json
[
  {"session_id": "...", "agent_id": "admin", "started_at": "..."}
]
```

## Remediation

1. **Rotate all tokens immediately** for the affected deployments.
2. **Move tokens out of source control**: read from `EnvironmentFile=`
   pointing at `~/.config/<plugin>/env` (mode 0600).
3. **Use real secrets in production** (e.g. `openssl rand -base64 32`)
   per-plugin, with rotation cadence.
4. **Add a CI check** that fails on `rg "neurox-.*-dev-2026"` in
   systemd/Taskfile/docs.

```diff
--- a/mcps/memory/systemd/neurox-memory.service
+++ b/mcps/memory/systemd/neurox-memory.service
 [Service]
-ExecStart=%h/.local/bin/memoryd --daemon-url http://127.0.0.1:7878 --api-token neurox-memory-dev-2026 ...
+EnvironmentFile=%h/.config/memoryd/env
+ExecStart=%h/.local/bin/memoryd --daemon-url http://127.0.0.1:7878 --api-token ${MEMORYD_API_TOKEN} ...
```

## References

- `daemon/docs/integrations/memory.md` — token documented as the canonical
  way to authenticate
- `mcps/memory/docs/postmortems/2026-08-11-memory-mcp-401-and-deploy-env-var-breakage.md`
  — past incident caused by token mismanagement
## Regression Test

```bash
# Add to CI: any PR that adds/keeps the literal token string fails.
cd /home/andres_fernandez/Proyectos/neurox
rg "neurox-.*-dev-2026" --type-not log \
   -g '!target' -g '!.git' \
   -g '!**/postmortems/**' \
   -g '!**/CHANGELOG.md' \
   && { echo "FAIL: hardcoded dev token"; exit 1; } \
   || echo "OK: no hardcoded dev tokens"
```

**Expected output after fix**: `OK: no hardcoded dev tokens`

