# Audit Report: Neurox runtime security & integrity

## Metadata

- Audit ID: 2026-08-16-neurox-runtime
- Completed: 2026-08-16
- Performed by: default (agent, MiniMax-M3, agent_id="default")

## Executive Summary

The neurox runtime has a strong security posture for a single-user,
loopback-bound deployment (JWT HS256, per-endpoint Bearer auth on
admin endpoints, sandbox config for paths, rate limit on memoryd).
However, **two CRITICAL findings** must be addressed immediately:

1. **API tokens are hardcoded in the public repo** (`neurox-*-dev-2026`
   in 5 systemd units + docs + Taskfile). Although named "dev", they
   are the same tokens used in production. If the daemon is ever
   exposed beyond loopback, these tokens grant full plugin access.
2. **The bootstrap admin password was written to journald** at first
   run (`password: 9ty58S9m@dcfAkqU`) and is permanently recoverable
   via `journalctl`. Admin RBAC privileges are equivalent to full
   takeover.

Plus 1 HIGH (WS `/v1/events` and `/v1/commands` have no auth) and
several MEDIUM/LOW findings around file permissions, missing rate
limits on most services, and a shell tool that can run arbitrary
`/bin/sh -c` commands (gated by approval).

**TL;DR**: Rotate the admin password NOW and migrate tokens to
`EnvironmentFile=` before any exposure beyond `127.0.0.1`.

## Methodology

L2 audit following the 6-layer framework in `security/process.md`.
All 6 layers covered: secrets, deps (best-effort), input validation,
auth/authz, networking, governance. Tools: `rg`, `ss`, `sqlite3
PRAGMA integrity_check`, `git status`, manual code review. No SAST
tool (`semgrep`/`gitleaks`) was available in this env; manual `rg`
patterns were used instead. See `02-design.md` for the architecture
diagram, threat model, and exact commands.

## Findings Summary

| ID | Severity | CVSS | STRIDE | Title | Status |
|---|---|---|---|---|---|
| SEC-0001-2026-001 | Critical | 9.1 | S,I | Hardcoded dev API tokens in public repo (systemd units) | Confirmed |
| SEC-0001-2026-002 | Critical | 9.8 | I,R | First-run admin password leaked to journald via stderr | Confirmed |
| SEC-0001-2026-003 | High | 7.5 | S,I | WebSocket `/v1/events` and `/v1/commands` have NO auth | Confirmed |
| SEC-0001-2026-004 | Medium | 5.5 | I | SQLite databases world-readable (mode 0644) | Confirmed |
| SEC-0001-2026-005 | Medium | 5.5 | I | `config.yaml` and backups world-readable | Confirmed |
| SEC-0001-2026-006 | Medium | 5.4 | E,T | shell tool executes arbitrary commands via `/bin/sh -c` | Confirmed |
| SEC-0001-2026-007 | Medium | 6.0 | E,T | subprocess `spec.command` no binary allowlist | Confirmed |
| SEC-0001-2026-008 | Low | 3.1 | T,I | CORS `Any` on daemon (loopback-bounded) | Confirmed |
| SEC-0001-2026-009 | Medium | 4.3 | I | memoryd `/admin/storage/stats` and `/admin/embeddings/config` public | Confirmed |
| SEC-0001-2026-010 | Low | 2.0 | I | Embedding cache theoretical inversion risk | Confirmed |
| SEC-0001-2026-011 | Low | 1.5 | I | GGUF models world-readable (1.5 GB) | Confirmed |
| SEC-0001-2026-012 | Low | 3.7 | D | No rate limiting on daemon/llmd/voiced/clickup-d/playwright-d | Confirmed |
| SEC-0001-2026-013 | Low | 2.0 | T,I | TLS support exists but unused (loopback by design) | Confirmed |
| SEC-0001-2026-014 | Medium | 4.3 | R | Security framework `validate-finding.sh` has structural bug | Confirmed |

## Findings by Severity

### Critical (2)

- **SEC-0001-2026-001** — Hardcoded dev API tokens in public repo. Affects
  5 systemd units + docs + Taskfile.yml. Rotate immediately; move
  tokens to `EnvironmentFile=` with mode 0600. See F-01.
- **SEC-0001-2026-002** — First-run admin password leaked to journald.
  The password `9ty58S9m@dcfAkqU` is permanently recoverable from
  `journalctl`. Rotate admin password NOW. See F-02.

### High (1)

- **SEC-0001-2026-003** — WebSocket `/v1/events` and `/v1/commands` have
  no auth. Apply `JwtAuthLayer` to WS handlers. See F-03.

### Medium (6)

- **SEC-0001-2026-004** — All 5 SQLite DBs are world-readable (0644).
  `chmod 600` + enforce in code. See F-04.
- **SEC-0001-2026-005** — `config.yaml` + 3 backups world-readable.
  `chmod 600` + delete `.bak-*`. See F-05.
- **SEC-0001-2026-006** — Shell tool uses `/bin/sh -c <arbitrary>`.
  Replace with explicit argv + binary allowlist. See F-06.
- **SEC-0001-2026-007** — Daemon subprocess `spec.command` not
  allowlisted. Restrict to `daemon/agents/<id>/` paths. See F-07.
- **SEC-0001-2026-009** — memoryd public admin endpoints leak store
  size + embedding endpoint. Move to Bearer-protected. See F-09.
- **SEC-0001-2026-014** — Security framework validator has
  `sed '1d;$d'` → bash expands `$d` → empty → `sed '1d;d'` = delete
  everything. Patch in next release. See F-14.

### Low (5)

- **SEC-0001-2026-008** — CORS `Any` (loopback bounded). Restrict
  origins. See F-08.
- **SEC-0001-2026-010** — Embedding cache theoretical inversion.
  Watchlist. See F-10.
- **SEC-0001-2026-011** — GGUF models world-readable. `chmod 600`. See F-11.
- **SEC-0001-2026-012** — No rate limiting on most services. Add
  tower-governor. See F-12.
- **SEC-0001-2026-013** — TLS unused (loopback by design). Acceptable. See F-13.

## Findings by STRIDE Category

| STRIDE | # findings | Notable examples |
|---|---|---|
| **S** (Spoofing) | 2 | F-01 (hardcoded tokens), F-03 (WS no auth) |
| **T** (Tampering) | 4 | F-06, F-07, F-08, F-13 |
| **R** (Repudiation) | 2 | F-02 (password to journald), F-14 (validator bug) |
| **I** (Info Disclosure) | 8 | F-01, F-02, F-03, F-04, F-05, F-09, F-10, F-11 |
| **D** (DoS) | 1 | F-12 (no rate limit) |
| **E** (Elevation) | 2 | F-06 (shell tool), F-07 (subprocess spec) |

**Observation**: **Information Disclosure is the dominant category**
(8 of 14 findings). Several are local-only (file modes) but two
(F-01, F-02) are CRITICAL and exploitable. The framework has a good
foundation but is leaking operational details through multiple paths.

## What Was NOT Found (positive)

- ✅ **No SQL injection**. All `sqlx::query` calls in the codebase use
  prepared statements. No `format!` inside `sqlx::query`.
- ✅ **No path traversal**. `SandboxConfig::resolve_paths` resolves all
  user paths under `workspace_root`. No `fs::read(query.?)` patterns.
- ✅ **No `0.0.0.0` bindings**. All 8 services bind to `127.0.0.1`
  only (`ss -tlnp` confirms).
- ✅ **JWT secret is auto-generated and 0600** — not hardcoded.
- ✅ **SQLite integrity check passed on all 5 DBs** (`PRAGMA
  integrity_check` returns `ok`).
- ✅ **MINIMAX_API_KEY is read from env, never logged**
  (`voiced` source confirms).
- ✅ **memoryd has rate limiting** (`RateLimiter::new(...)` in main.rs).
- ✅ **TLS support exists** (`axum_server::tls_rustls`) — just not
  activated in this deployment (loopback by design).
- ✅ **Embedding api_key scrubbed from `/admin/info`** post-EP-0008.
- ✅ **Workspace SOT is structurally clean**: 72 symlinks OK, 0 broken,
  all 9 repos with the same 6-LOCAL + 9-symlink layout.

## Detailed Findings

- [F-01](findings/F-01-hardcoded-dev-api-tokens.md) — Critical (9.1)
- [F-02](findings/F-02-first-run-password-stderr.md) — Critical (9.8)
- [F-03](findings/F-03-ws-events-no-auth.md) — High (7.5)
- [F-04](findings/F-04-dbs-world-readable.md) — Medium (5.5)
- [F-05](findings/F-05-config-yaml-world-readable.md) — Medium (5.5)
- [F-06](findings/F-06-shell-tool-arbitrary-cmd.md) — Medium (5.4)
- [F-07](findings/F-07-subprocess-spec-no-allowlist.md) — Medium (6.0)
- [F-08](findings/F-08-cors-any.md) — Low (3.1)
- [F-09](findings/F-09-memoryd-public-admin-endpoints.md) — Medium (4.3)
- [F-10](findings/F-10-embedding-cache-inversion.md) — Low (2.0)
- [F-11](findings/F-11-models-world-readable.md) — Low (1.5)
- [F-12](findings/F-12-no-rate-limiting.md) — Low (3.7)
- [F-13](findings/F-13-tls-unused.md) — Low (2.0)
- [F-14](findings/F-14-framework-validator-bug.md) — Medium (4.3)

## Out of Scope (considered, not findings)

- **Embedding model prompt injection** (e.g. crafting a text that
  produces a malicious embedding vector): the embedding server
  (`llama-server` on :8081) is stateless inference; it doesn't
  *execute* anything based on the vector. The vector is stored in
  `embedding_cache` (F-10) but never "executes". Theoretical only.
- **CORS `Any` risk**: bounded by loopback bind; only relevant if
  bind changes. Documented as F-08 (Low).
- **TLS absent in loopback**: by design (no value added; Cloudflare
  provides TLS at the edge in prod). Documented as F-13 (Low).

## Immediate Actions Required

1. **SEC-0001-2026-002** — **Rotate admin password NOW**
   - Mitigation: `PATCH /v1/users/me/password` or delete and re-bootstrap
   - Deadline: <24h
2. **SEC-0001-2026-001** — **Rotate all `neurox-*-dev-2026` tokens**
   - Mitigation: move to `EnvironmentFile=` (mode 0600), generate
     fresh `openssl rand -base64 32` tokens
   - Deadline: <24h
3. **SEC-0001-2026-003** — **Add auth to WS endpoints**
   - Mitigation: apply `JwtAuthLayer` in `daemon/core/src/router/ws.rs`
   - Deadline: <7d

## Recommendations (Roadmap)

1. **(Now) Password rotation + token migration** — see Immediate Actions.
2. **(This week) File permission hardening** — `chmod 600` on all DBs,
   config.yaml, models; delete stale backups.
3. **(This month) WS auth + shell tool allowlist** — F-03 + F-06.
4. **(This quarter) Framework patch** — fix F-14 validator; add
   rate limiting (F-12); restrict CORS (F-08); close memoryd
   public endpoints (F-09).
5. **(Backlog) Embedding cache encryption** — F-10.

## Metrics

- Total findings: 14
- Critical: 2
- High: 1
- Medium: 6
- Low: 5
- Coverage: 6/6 layers (100%)
- Time spent: ~3 hours (within L2 estimate)
- Files created: 14 findings + 1 report + 1 proposal + 1 design

## References

- `security/templates/finding.template.md` — finding template used
- `security/lib/severity-matrix.md` — severity classification
- `security/lib/stride-explained.md` — STRIDE categories
- `security/lib/owasp-asvs-mini.md` — ASVS mapping reference
- `security/lib/nist-ssdf-mini.md` — SSDF mapping reference

## Changelog

| Date | Change |
|---|---|
| 2026-08-16 | Initial report |
