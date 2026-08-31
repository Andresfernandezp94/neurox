# OWASP ASVS Mini-Guide for neurox

> **Subset de OWASP ASVS v5.0 relevante para neurox.**
> ASVS completo: https://owasp.org/www-project-application-security-verification-standard/

## Qué es ASVS

Application Security Verification Standard (v5.0) — lista de 286 requisitos organizados en 14 capítulos, con niveles 1-3 de profundidad.

**Para neurox usamos nivel 1 (base) + nivel 2 selectivo**.

## Los 14 capítulos (versión mini)

| Cap | Título | Aplica a neurox |
|---|---|---|
| V1 | Encoding and Sanitization | ✅ Sí |
| V2 | Validation and Business Logic | ✅ Sí |
| V3 | Web Frontend Security | ❌ No (Rust server-side) |
| V4 | API and Web Service | ✅ Sí (HTTP + WS) |
| V5 | File Handling | ⚠️ Solo para `read`/`write` tools |
| V6 | Communication | ✅ Sí (HTTP/WS en producción) |
| V7 | Cryptography | ✅ Sí (API keys, tokens) |
| V8 | Error Handling and Logging | ✅ Sí |
| V9 | Data Protection | ✅ Sí (multi-tenant) |
| V10 | Configuration | ✅ Sí (systemd units, configs) |
| V11 | Business Logic | ✅ Sí (approval flow) |
| V12 | Session Management | ✅ Sí (WS sessions, JWTs) |
| V13 | Identity and Access Control | ✅ Sí (AuthLayer, RBAC) |
| V14 | Concurrency | ⚠️ Solo si hay race conditions |

## Requisitos clave para neurox (nivel 1)

### V4 — API and Web Service

- `v5.0.0-4.1.1` — Verify the API has authentication
- `v5.0.0-4.1.2` — Verify the API has authorization checks
- `v5.0.0-4.2.1` — Verify that all API endpoints are documented
- `v5.0.0-4.3.1` — Verify that all API endpoints validate input

### V7 — Cryptography

- `v5.0.0-7.1.1` — Verify that cryptographic operations use approved algorithms
- `v5.0.0-7.2.1` — Verify that secrets are stored securely (not in code)
- `v5.0.0-7.3.1` — Verify that secrets are rotated regularly

### V9 — Data Protection

- `v5.0.0-9.1.1` — Verify that sensitive data is encrypted at rest
- `v5.0.0-9.2.1` — Verify that sensitive data is encrypted in transit
- `v5.0.0-9.3.1` — Verify that PII is handled according to regulations

### V11 — Business Logic

- `v5.0.0-11.1.1` — Verify that the application enforces business rules
- `v5.0.0-11.2.1` — Verify that workflows cannot be bypassed
- `v5.0.0-11.3.1` — Verify that critical operations require approval

### V13 — Identity and Access Control

- `v5.0.0-13.1.1` — Verify that authentication uses secure mechanisms
- `v5.0.0-13.2.1` — Verify that authorization is enforced on every request
- `v5.0.0-13.3.1` — Verify that principle of least privilege is applied

## Schema (para usar en templates)

```yaml
owasp_asvs: v5.0.0-13.2.1   # capítulo.sección.requisito
```

Si el finding no aplica a ningún ASVS, dejar el campo vacío u omitirlo.

## Cómo encontrar el ASVS ID correcto

1. Identificá qué categoría describe mejor el problema (auth, crypto, business logic, etc.)
2. Buscá en el capítulo correspondiente del ASVS completo (CSV descargable)
3. Copiá el ID con el formato `v5.0.0-X.Y.Z`

## Mapeo neurox → ASVS (referencia rápida)

| Componente neurox | ASVS cap relevantes |
|---|---|
| `AuthLayer` (HTTP) | V13.1, V13.2 |
| WS endpoints (sin auth) | V13.2 (falla), V13.3 (falla) |
| Bearer token comparison | V7.1, V13.1 |
| API keys en env vars | V7.2, V7.3 |
| SQLite (memory plugin) | V9.1 (falla — sin cifrado) |
| HTTP sin TLS | V9.2 (falla) |
| `requires_approval: true` en bash tool | V11.3 |
| Path traversal mitigation | V1.2, V1.3 |
| SQL parametrizado | V1.2 (cumple) |

## Referencias

- ASVS 5.0 completo (PDF): https://github.com/OWASP/ASVS/raw/v5.0.0/5.0/OWASP_Application_Security_Verification_Standard_5.0.0_en.pdf
- ASVS 5.0 CSV: https://github.com/OWASP/ASVS/raw/v5.0.0/5.0/docs_en/OWASP_Application_Security_Verification_Standard_5.0.0_en.csv
