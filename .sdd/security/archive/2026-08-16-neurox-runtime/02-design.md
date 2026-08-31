# Audit Design: Neurox runtime security & integrity

## Metadata

Audit ID: 2026-08-16-neurox-runtime
Mode: audit
Step: 2 (Design)

## Architecture Under Review

```
                          Browser (SPA)
                              │
                              │ HTTPS (Cloudflare Pages) / HTTP (loopback)
                              ▼
            ┌─── neurox-web (port 8787, systemd) ───┐
            │     serves SPA bundle (static)         │
            └────────────────────────────────────────┘

                          User (LAN/localhost)
                              │
                              │ HTTP + WS
                              ▼
            ┌─────────── neurox daemon (7878) ──────────────────┐
            │  - HTTP/WS API                                    │
            │  - JWT HS256 / api_token                          │
            │  - Plugin registry (/v1/mcps)                     │
            │  - Auth middleware + RBAC (admin/operator/viewer) │
            │  - Observability ingester → logs.sqlite          │
            └─┬─────────────┬────────────┬──────────────┬───────┘
              │             │            │              │
              │             │            │              │
   ┌──────────▼──┐  ┌───────▼─────┐ ┌────▼────┐ ┌──────▼───────┐
   │  memoryd    │  │  voiced     │ │  llmd   │ │ playwright-d │
   │  (9999)     │  │  (9998)     │ │ (9997)  │ │  (9995)      │
   │  +admin/*   │  │  WS /voice  │ │  LLM    │ │  browser     │
   │  SQLite L2  │  │  MiniMax    │ │  proxy  │ │  automation  │
   └─────────────┘  └─────────────┘ └─────────┘ └──────────────┘
                                                         ┌──────────────┐
                                                         │ clickup-d    │
                                                         │ (9996) CT    │
                                                         └──────────────┘

            ┌─── llama-server (8081) ───┐
            │  embeddinggemma-300m GGUF │
            │  OpenAI-compat /v1/embed  │
            │  stateless inference      │
            └───────────────────────────┘
```

### Trust boundaries

| Boundary | Crossing data | Auth |
|---|---|---|
| Browser → neurox-web | static SPA | none (public) |
| Browser → daemon (via Vite proxy in dev) | REST/WS /v1/* | JWT/api_token |
| User → daemon directly | REST/WS /v1/* | JWT/api_token |
| Daemon → memoryd | /admin/* (some public, some with Bearer) | per-endpoint |
| Daemon → voiced | /voice/* + /voice/ws | api_token shared |
| Daemon → llmd | (none — llmd is queried by daemon directly) | n/a (loopback) |
| memoryd → llama-server | HTTP POST /v1/embeddings | none (loopback, internal) |
| Browser → voiced (direct, via Vite proxy) | /voice/ws | none — anyone on LAN can speak to voice MCP |

## Threat Model (STRIDE per data flow)

### Data flow: Browser → daemon (`/v1/*`)

| STRIDE | Amenaza? | Mitigación actual |
|---|---|---|
| **S** (Spoofing) | Sí | JWT HS256 + api_token. Riesgo: tokens dev visibles en systemd units. |
| **T** (Tampering) | Sí | JWT firmado (HS256). api_token: texto plano en header. |
| **R** (Repudiation) | Parcial | Hay logs JSONL por componente + observability ingester. Pero logs.sqlite está vacía — el ingester no está activo o no ingiere. |
| **I** (Info disclosure) | Sí | CORS `Any` (loopback only). Stack traces verbose? Por verificar. |
| **D** (DoS) | Parcial | Sin rate limiting explícito. axum no incluye rate limit por default. |
| **E** (Elevation) | Parcial | RBAC implementado (admin/operator/viewer) pero cobertura por endpoint no auditada. |

### Data flow: Daemon → memoryd (`/admin/*`)

| STRIDE | Amenaza? | Mitigación actual |
|---|---|---|
| **S** | Sí | Bearer token en /admin/* protegidos. Algunos /admin/* son públicos por diseño (`/admin/info`, `/admin/storage/stats`, `/admin/health`). |
| **T** | Sí | Mismo Bearer. |
| **R** | No relevante | memoryd loguea a JSONL. |
| **I** | Sí | `/admin/embeddings/config` filtraba la API key real (EP-0008 lo arregló en 2026-08-10). |
| **D** | Parcial | Sin rate limit. Memoryd puede saturarse con requests de búsqueda pesados. |
| **E** | Sí | Endpoints `/admin/*` son sensibles — un Bearer comprometido da control total sobre memories. |

### Data flow: Browser → voiced (direct via Vite proxy `/voice/ws`)

| STRIDE | Amenaza? | Mitigación actual |
|---|---|---|
| **S** | **Sí** | **WS no requiere auth.** Vite proxy `/voice/ws → ws://127.0.0.1:9998` está abierto a cualquiera en LAN (si vite dev está en `host: true`). |
| **T** | Sí | Sin auth = cualquiera puede enviar texto al voice MCP. |
| **R** | Parcial | voiced loguea eventos. |
| **I** | Sí | Voz del usuario viaja al MCP sin cifrado en LAN (loopback, pero si vite dev está expuesto a LAN...). |
| **D** | Sí | WS abierto = cualquiera puede saturar la TTS pipeline (CPU/MiniMax quota). |
| **E** | Sí | Si voiced reenvía mensajes al daemon brain, podría inyectar comandos sin pasar por el filtro del daemon. |

### Data flow: memoryd → llama-server (`/v1/embeddings`)

| STRIDE | Amenaza? | Mitigación actual |
|---|---|---|
| **S** | No | Loopback only. |
| **T** | Sí | Texto de la memoria va al modelo de embeddings — si alguien compromete llama-server puede exfiltrar contenidos. |
| **R** | No | n/a |
| **I** | Sí | Texto va plano a la inferencia (no cifrado in-transit — pero loopback only). |
| **D** | Sí | Sin rate limit; un recall masivo puede saturar llama-server (CPU-bound). |
| **E** | No relevante | llama-server no tiene roles. |

### Data flow: Daemon → SQLite (5 DBs)

| STRIDE | Amenaza? | Mitigación actual |
|---|---|---|
| **S** | Parcial | File permissions de los DBs (a verificar). |
| **T** | Sí | sqlx + queries parametrizadas (a verificar). |
| **R** | Parcial | No hay WORM audit log. `memory_audit` table sí existe pero no se usa. |
| **I** | Sí | DBs sin cifrado en disco. Contienen embeddings (vector BLOBs) y memories. |
| **D** | Parcial | SQLite locks — si una query pesada bloquea, puede DoS. |
| **E** | Parcial | File permissions (a verificar). |

## Tools to Use (concrete commands)

### Secret detection

```bash
# 1. Buscar API tokens hardcoded (sk-, AKIA, AIza, ghp_)
rg -i "(sk-[a-zA-Z0-9]{20,}|AKIA[0-9A-Z]{16}|AIza[0-9A-Za-z\-_]{35}|ghp_[a-zA-Z0-9]{36})" \
   /home/andres_fernandez/Proyectos/neurox/ -g '!target' -g '!.git' -g '!node_modules'

# 2. Buscar tokens específicos de neurox (formato neurox-*-dev-2026)
rg "neurox-[a-z]+-dev-2026" /home/andres_fernandez/Proyectos/neurox/ -g '!target' -g '!.git'

# 3. Buscar referencias a API keys de providers LLM
rg -i "(MINIMAX_API_KEY|OPENAI_API_KEY|ANTHROPIC_API_KEY)" /home/andres_fernandez/Proyectos/neurox/ -g '!target'

# 4. File permissions de archivos sensibles
ls -la /home/andres_fernandez/.config/neurox/
ls -la /home/andres_fernandez/.local/share/neurox/
ls -la /home/andres_fernandez/.local/share/memoryd/
```

### Dependency scan

```bash
# Para cada repo Rust
for repo in /home/andres_fernandez/Proyectos/neurox/{daemon,mcps/memory,mcps/voice,mcps/llmd}; do
  echo "=== $repo ==="
  (cd "$repo" && timeout 60 cargo audit --no-fetch 2>&1 | head -50)
done
```

### SAST (manual patterns)

```bash
# Command injection
rg "Command::new\(" /home/andres_fernandez/Proyectos/neurox/daemon/ /home/andres_fernandez/Proyectos/neurox/mcps/ -g '!target'

# SQL injection (format!() en queries sqlx)
rg "sqlx::query\(format!|sqlx::query_as\(format!" /home/andres_fernandez/Proyectos/neurox/ -g '!target'

# Path traversal
rg "PathBuf::from\(.*req\)|fs::read\(.*query|fs::write\(.*query" /home/andres_fernandez/Proyectos/neurox/ -g '!target'

# Bind addresses
rg "0\.0\.0\.0" /home/andres_fernandez/Proyectos/neurox/ -g '!target'

# unwrap() en código de producción
rg "\.unwrap\(\)" /home/andres_fernandez/Proyectos/neurox/daemon/core/src /home/andres_fernandez/Proyectos/neurox/mcps/memory/memoryd/src 2>/dev/null | /usr/bin/wc -l
```

### Networking audit

```bash
# Puertos abiertos
ss -tlnp | grep -E ":(7878|8787|8081|9995|9996|9997|9998|9999) "

# systemd unit bind
/usr/bin/grep -E "ExecStart.*--(bind|port)" /home/andres_fernandez/.config/systemd/user/neurox-*.service

# CORS
rg -i "cors|Access-Control" /home/andres_fernandez/Proyectos/neurox/daemon/core/src /home/andres_fernandez/Proyectos/neurox/client/web/src -g '!target' 2>/dev/null

# TLS
rg -i "rustls|tls|https" /home/andres_fernandez/Proyectos/neurox/daemon/core/src -g '!target' 2>/dev/null
```

### Auth audit

```bash
# JWT secret + api_token usage
/usr/bin/grep -r "jwt_secret\|JWT_SECRET\|api_token\|ApiToken" /home/andres_fernandez/Proyectos/neurox/daemon/core/src --include='*.rs' 2>/dev/null

# RBAC roles
/usr/bin/grep -r "admin\|operator\|viewer" /home/andres_fernandez/Proyectos/neurox/daemon/core/src/router --include='*.rs' 2>/dev/null

# WS auth
/usr/bin/grep -r "WebSocketUpgrade\|on_upgrade" /home/andres_fernandez/Proyectos/neurox/daemon/core/src --include='*.rs' 2>/dev/null

# memoryd admin endpoints
/usr/bin/grep -r "/admin/" /home/andres_fernandez/Proyectos/neurox/mcps/memory/memoryd/src --include='*.rs' 2>/dev/null
```

### Integrity checks

```bash
# SQLite PRAGMA integrity_check en todas las DBs
for db in /home/andres_fernandez/.local/share/neurox/neurox.db \
          /home/andres_fernandez/.local/share/neurox/logs.sqlite \
          /home/andres_fernandez/.local/share/memoryd/memory.db \
          /home/andres_fernandez/.local/share/memoryd/workspaces/workspace-1.db \
          /home/andres_fernandez/.local/share/llmd/llmd.db; do
  echo "=== $db ==="
  sqlite3 "$db" "PRAGMA integrity_check;"
done

# File permissions on DBs
ls -la /home/andres_fernandez/.local/share/neurox/ /home/andres_fernandez/.local/share/memoryd/ /home/andres_fernandez/.local/share/llmd/

# Git status (debe estar limpio en todos los repos después de la sesión previa)
for repo in /home/andres_fernandez/Proyectos/neurox/{,daemon,client/cli,client/web,mcps/memory,mcps/voice,mcps/llmd,mcps/clickup,mcps/playwright}; do
  echo "=== $repo ==="
  git -C "$repo" status --short --branch 2>&1 | head -3
done

# Symlinks health (72 totales, 0 rotos esperado)
find /home/andres_fernandez/Proyectos/neurox -name '.sdd' -type d 2>/dev/null | while read sdd; do
  find "$sdd" -maxdepth 1 -type l 2>/dev/null | while read link; do
    if [ ! -e "$link" ]; then
      echo "ROTO: $link → $(readlink "$link")"
    fi
  done
done
```

### Governance

```bash
# ¿Hay secrets commiteados?
/usr/bin/grep -rE "(BEGIN.*PRIVATE KEY|api[_-]?key.*=.*['\"])" /home/andres_fernandez/Proyectos/neurox/ -g '!target' -g '!.git' --include='*.{rs,ts,tsx,js,json,yaml,yml,toml,md}' 2>/dev/null | /usr/bin/head -20

# ¿Branch protection en GitHub? (no auditable desde CLI — flag como 'manual check')
# ¿CI/CD con secret scanning? (no auditable desde CLI — flag como 'manual check')
```

## Findings Structure

```
archive/2026-08-16-neurox-runtime/
├── findings/
│   ├── F-01-<slug>.md
│   ├── F-02-<slug>.md
│   └── ...
```

Each finding follows `templates/finding.template.md` with all REQUIRED fields.

## Output

- `archive/2026-08-16-neurox-runtime/04-report.md` — executive summary + findings table
- `archive/2026-08-16-neurox-runtime/findings/*.md` — detailed findings (one per file)
- `archive/2026-08-16-neurox-runtime/executive-summary.md` — 1-page for stakeholders

## Risk Tolerance

**Immediate escalation (Critical/High)** — communicated to user in the response, not deferred:
- RCE vector
- Secret exposed in repo or in default config
- Auth bypass (any path)
- WS endpoint without auth on bind 0.0.0.0

**Backlog (Medium/Low)** — documented in report, not escalated:
- Missing security headers
- Verbose errors
- Style nits
- Documentation gaps

## Validation

```bash
bash security/bin/validate-design.sh archive/2026-08-16-neurox-runtime/02-design.md
```

Must exit 0 before proceeding.

## References

- `security/templates/audit-proposal.template.md`
- `security/templates/audit-design.template.md`
- `security/templates/finding.template.md`
- `security/lib/severity-matrix.md`
- `security/lib/stride-explained.md`
- `security/lib/owasp-asvs-mini.md`
- `security/lib/nist-ssdf-mini.md`
