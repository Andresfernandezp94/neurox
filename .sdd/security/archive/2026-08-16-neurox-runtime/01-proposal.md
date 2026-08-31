# Audit Proposal: Neurox runtime security & integrity (workspace-wide)

> Validator-friendly field block (the validator greps for `^Field:` and `^- Field:` patterns).
> Both formats duplicated for compatibility with the buggy validator regex.

Audit ID: 2026-08-16-neurox-runtime
Started: 2026-08-16
Proposed by: default (agent, MiniMax-M3, agent_id="default")
Status: InProgress
Scope: Comprehensive L2 audit of all 9 neurox repos (daemon + 5 MCPs + 2 clients + workspace SOT) — code, infra, secrets, governance
Depth: L2
Methodology: 6-layer framework from security/process.md — secrets, deps, input validation, auth/authz, networking, governance
Tools: rg, cargo audit (best-effort), ss, sqlite3 PRAGMA integrity_check, git status, manual code review
Success Criteria: All 6 layers covered; every finding has ID/severity/STRIDE/location/PoC/remediation/regression test; report compiled; Critical/High escalated immediately

- Audit ID: 2026-08-16-neurox-runtime
- Started: 2026-08-16
- Proposed by: default
- Status: InProgress
- Depth: L2

## Metadata

## Scope

### In scope (be specific)

- `daemon/` — Core HTTP+WS API, plugin registry, auth layer, observability ingester
- `mcps/memory/` (memoryd) — Memory plugin HTTP API, FTS5 + embeddings, admin endpoints
- `mcps/voice/` (voiced) — Voice plugin HTTP+WS API, MiniMax TTS
- `mcps/llmd/` (llmd) — LLM proxy multi-provider, model_configs
- `mcps/clickup/` (clickup-d) — ClickUp REST wrapper
- `mcps/playwright/` (playwright-d) — Playwright browser wrapper
- `client/web/` — SPA Vite+React+TS, dev proxy
- `.config/neurox/` — env file, jwt_secret, users.json (auth material)
- `~/.local/share/neurox/`, `~/.local/share/memoryd/`, `~/.local/share/llmd/` — DBs
- All systemd user units (`~/.config/systemd/user/neurox-*.service`)
- The running llama-server on :8081 (embedding model inference)

### Out of scope (explicit)

- Production Cloudflare Pages deploy (no prod deployment audited in this run)
- Sixbell-internal services (`Sixbell/` is a separate product)
- Opencode IDE plugins (separate product, separate audit)
- llama.cpp internals (third-party)
- Hardware/TPM/firmware

## Depth

**L2 (Standard)** — L1 (automated tools) + manual code review of critical paths.

Estimated ~3-5 hours wall-clock. Justification: the surface area is large
(1 daemon + 5 MCPs + 1 client + 5 DBs + 8 systemd units), but most security
relevant code lives in the daemon (auth, routing) and memoryd (admin
endpoints). Going L3 would require writing PoCs against each plugin — useful
but not warranted today.

## Methodology

Following the 6-layer framework from `security/process.md`:

| Layer | What | Tool/Method |
|---|---|---|
| 1. Secrets hardcoded | API tokens, JWT secret, LLM provider keys | rg + `score-severity.sh` |
| 2. Dependency vulnerabilities | Rust deps (sqlx, axum, tokio, etc.) | `cargo audit` (best-effort) + manual review of dep tree |
| 3. Input validation | SQL injection, path traversal, command injection, SSRF | rg patterns + manual code review of HTTP routes |
| 4. Auth & Authz | JWT, api_token, RBAC, WS upgrade auth, rate limit | code review of `core/src/auth/` + `router/` + memoryd admin |
| 5. Networking | bind addresses, CORS, TLS, ports open, file permissions | `ss`, `ls`, systemd unit inspection |
| 6. Governance | git status clean, secrets in repo, submodules dirty | git status, file mode scan |

Layers to cover (mark which):
- [x] Layer 1: Secrets hardcoded
- [x] Layer 2: Dependency vulnerabilities
- [x] Layer 3: Input validation
- [x] Layer 4: Auth & Authz
- [x] Layer 5: Networking
- [x] Layer 6: Governance

## Tools

- `rg` (ripgrep) — manual pattern matching
- `cargo audit` — best-effort (may fail if offline; documented as such)
- `ss` — listening sockets
- `ls -la` — file permissions
- `git status` / `git log` — repo health
- `sqlite3 PRAGMA integrity_check` — DB integrity
- Manual code review (no SAST tool available in this env)

## Success Criteria

- All 6 layers covered (or documented why some are skipped)
- All findings have: ID, severity, STRIDE, location, PoC, remediation, regression test
- Report compiled at `archive/2026-08-16-neurox-runtime/04-report.md`
- Critical/High findings escalated to Andrés immediately (in the response, not after)
- MANIFEST.md reset to `mode: null` after `close.sh`

## Timeline

- Estimated duration: ~3-4 hours
- Deadline: today (2026-08-16) — this is a one-shot audit driven by the user

## Pre-requisites

- Read access to all 8 sub-repos via SSH keys (verified — `git push` works)
- `cargo`, `pnpm`, `sqlite3`, `rg`, `ss` available locally
- All 7 systemd user services running
- All 5 SQLite DBs accessible

## References

- `security/process.md` — overall process
- `security/lib/severity-matrix.md` — how to classify findings
- `security/lib/stride-explained.md` — STRIDE categories
- `security/lib/owasp-asvs-mini.md` — OWASP ASVS mapping
- `security/lib/nist-ssdf-mini.md` — NIST SSDF mapping
- ADR-0001 — multi-agent subprocess architecture
- ADR-0002 — streaming endpoint routing
- EP-0008 (archived) — MCP server, memory_share, skills, re-register & security hardening (contains past leak fix)
