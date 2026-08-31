# Severity Matrix — neurox

> **Cómo clasificar la severidad de un finding.**
> Para clasificar automáticamente: `bash security/bin/score-severity.sh "<description>"`

## Tabla principal (CVSS-aligned)

| Severidad | CVSS | SLA de remediación | Descripción típica |
|---|---|---|---|
| `Critical` | 9.0 - 10.0 | < 24 horas | RCE, secret expuesto en repo, auth bypass completo |
| `High` | 7.0 - 8.9 | < 7 días | SQL injection, XSS stored, privilege escalation, hardcoded secrets |
| `Medium` | 4.0 - 6.9 | < 30 días | XSS reflected, info disclosure parcial, missing rate limit, CSRF |
| `Low` | 0.1 - 3.9 | < 90 días | Verbose errors, missing security headers, info disclosure menor |

## Cómo calcular el CVSS score (3 pasos)

### Paso 1: ¿Cuál es el **Impacto**?

| Impacto | Score base |
|---|---|
| **Confidentiality**: data sensible expuesta | +0.0 a +3.9 |
| **Integrity**: data modificada sin autorización | +0.0 a +3.9 |
| **Availability**: servicio caído | +0.0 a +3.9 |

### Paso 2: ¿Cuál es el **Vector de ataque**?

| Vector | Multiplicador |
|---|---|
| **Network** (atacante remoto, sin acceso) | 1.0 |
| **Adjacent** (misma red) | 0.8 |
| **Local** (ya tiene cuenta en el sistema) | 0.6 |
| **Physical** (acceso físico) | 0.4 |

### Paso 3: ¿Qué tan **complejo** es explotarlo?

| Complejidad | Multiplicador |
|---|---|
| **Low** (no requiere autenticación, código público) | 1.0 |
| **High** (requiere auth, condiciones especiales) | 0.6 |

## Ejemplos resueltos

| Ejemplo | Impacto | Vector | Complejidad | CVSS | Severidad |
|---|---|---|---|---|---|
| Bash command ejecutable sin approval + WS abierto a LAN | All 3 (9.0+) | Network (1.0) | Low (1.0) | **9.5** | **Critical** |
| API key hardcoded en código commiteado | Confidentiality (3.9) | Network (1.0) | Low (1.0) | **7.5** | **High** |
| SQL injection en endpoint interno | All 3 (9.0+) | Adjacent (0.8) | High (0.6) | **7.0** | **High** |
| XSS stored en chat privado | Integrity (2.5) | Network (1.0) | Low (1.0) | **5.4** | **Medium** |
| Missing rate limit en endpoint de auth | Availability (2.5) | Network (1.0) | Low (1.0) | **4.5** | **Medium** |
| Server header expone versión exacta | Confidentiality (1.0) | Network (1.0) | Low (1.0) | **2.0** | **Low** |

## Reglas especiales para neurox

| Finding | Severidad mínima |
|---|---|
| WS endpoint sin auth | High |
| HTTP endpoint sin auth (en bind 0.0.0.0) | High |
| Secret en `.env` commiteado al repo | High |
| Secret en `~/.config/` con permisos 644 | Medium |
| `cargo audit` falla con CVE medium | Medium |
| `cargo audit` falla con CVE low | Low |
| Bash tool sin `requires_approval` | Critical |
| Tool con `requires_approval` pero sin validación de input | High |

## Cuándo **escalar** la severidad

- Si el componente afectado está expuesto a internet → subir 1 nivel
- Si no hay mitigación posible sin breaking change → bajar 1 nivel (pero documentar)
- Si hay evidencia de explotación activa → Critical sin importar CVSS
- Si afecta a múltiples tenants (multi-tenancy) → subir 1 nivel

## Cuándo **bajar** la severidad

- Solo accesible localmente (sin red) → bajar 1 nivel
- Requiere credenciales de admin → bajar 1 nivel
- Ya está mitigado parcialmente (workaround conocido) → mantener o bajar 1 nivel
- Es un theoretical issue sin PoC práctico → bajar 1 nivel y marcar como `theoretical`

## Cuándo **duplicar** el severity

NUNCA. Si hay duda entre dos niveles, asignar el más bajo y documentar por qué.

## Cuándo **NO** reportar (no es un finding)

- Self-XSS (solo afecta al usuario que lo dispara)
- CSRF en endpoints sin state-changing operations (read-only)
- Missing headers en endpoints internos
- Verbose errors en desarrollo (no en producción)
- Theoretical race conditions sin PoC concreto

## Schema (para usar en templates)

```yaml
severity: Critical  # o High | Medium | Low
cvss: 9.5          # 0.0 - 10.0
```

El validator exige:
- `severity` ∈ {`Critical`, `High`, `Medium`, `Low`}
- `cvss` ∈ [0.0, 10.0]

## Referencias

- CVSS 3.1 spec: https://www.first.org/cvss/specification-document
- NIST CVSS calculator: https://nvd.nist.gov/vuln-metrics/cvss/v3-calculator
