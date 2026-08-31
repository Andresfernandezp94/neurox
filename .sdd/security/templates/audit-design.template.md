# Audit Design: [REQUIRED: same scope as proposal]

## Metadata

- Audit ID: [REQUIRED: YYYY-MM-DD-<scope>]
- Mode: audit
- Step: 2 (Design)

## Architecture Under Review

[REQUIRED: describe the components, their relationships, and the trust boundaries. Use ASCII art if it helps.]

```
[Component A] ── HTTP ──> [Component B]
     │                       │
     │ WebSocket             │ SQLite
     ▼                       ▼
[Browser]               [memory.db]
```

## Threat Model (STRIDE per data flow)

[REQUIRED: for each data flow identified above, list the STRIDE threats that apply. Use the format:]

### Data flow: [Component A] → [Component B] via HTTP

| STRIDE | Amenaza existe? | Mitigación actual |
|---|---|---|
| S | [Sí/No] | [cómo se mitiga hoy, o "no mitigado"] |
| T | [Sí/No] | [...] |
| R | [Sí/No] | [...] |
| I | [Sí/No] | [...] |
| D | [Sí/No] | [...] |
| E | [Sí/No] | [...] |

[Repeat for each data flow]

## Tools to Use (concrete commands)

[REQUIRED: list each tool with the EXACT command to run. No "investigar" — be precise.]

### Secret detection

```bash
# Search for hardcoded secrets in code
rg -i "(sk-[a-zA-Z0-9]{20,}|AKIA[0-9A-Z]{16}|AIza[0-9A-Za-z\-_]{35}|ghp_[a-zA-Z0-9]{36})" --type-not log \
   repos/ -g '!target' -g '!.git'

# Check file permissions of config files
ls -la ~/.config/neuro-pro/
```

### Dependency scan

```bash
# For each Rust repo
for repo in repos/neuro-pro repos/neuro-pro-plugin-*; do
  cd "$repo" && cargo audit --no-fetch 2>&1 | tee "/tmp/audit-$repo.log"
done

# For the admin (TypeScript)
cd repos/neuro-pro/clients/admin && pnpm audit --prod
```

### SAST

```bash
# Semgrep with default ruleset
semgrep --config auto repos/neuro-pro/core/src/

# Manual grep for common patterns
rg "Command::new\(.+/bin/sh\)" repos/neuro-pro/core/src/
rg "tokio::process::Command" repos/neuro-pro/core/src/
```

### Networking audit

```bash
# Open ports + bind addresses
ss -tlnp | grep -E "7878|9998|9999|9100"

# systemd unit bind
grep -E "ExecStart.*--bind" ~/.config/systemd/user/*.service
```

### Auth audit

```bash
# Find all HTTP routes (axum pattern)
rg "axum::Router::new" repos/neuro-pro/core/src/router/
rg "route\(.*post|get|put|delete\)" repos/neuro-pro/core/src/router/

# Find WS handlers
rg "WebSocketUpgrade|on_upgrade" repos/neuro-pro/core/src/router/

# Find AuthLayer usages
rg "AuthLayer::new|with_auth" repos/neuro-pro/core/src/router/
```

## Findings Structure

[REQUIRED: where findings will be stored]

```
archive/<audit-id>/
├── findings/
│   ├── F-01-<slug>.md   ← use templates/finding.template.md
│   ├── F-02-<slug>.md
│   └── ...
```

Each finding must have all REQUIRED fields from the template.

## Output

- `archive/<audit-id>/report.md` — executive summary + findings table
- `archive/<audit-id>/findings/*.md` — detailed findings (one per file)
- `archive/<audit-id>/executive-summary.md` — 1-page for stakeholders (optional)

## Risk Tolerance

[REQUIRED: what findings will trigger immediate escalation to Andrés vs. backlog]

**Immediate escalation (Critical/High)**:
- RCE vector
- Secret exposed in repo
- Auth bypass

**Backlog (Medium/Low)**:
- Missing headers
- Verbose errors
- Style nits

## Validation

Before moving to step 3 (Execution), run:
```bash
bash security/bin/validate-proposal.sh archive/<audit-id>/01-proposal.md
bash security/bin/validate-design.sh archive/<audit-id>/02-design.md
```

Both must exit 0 before proceeding.

## References

- security/templates/audit-proposal.template.md — back to proposal
- security/process.md — overall process
