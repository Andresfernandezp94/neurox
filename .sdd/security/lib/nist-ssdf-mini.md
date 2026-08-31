# NIST SSDF Mini-Guide for neurox

> **Subset de NIST SP 800-218 (SSDF v1.1) relevante para neurox.**

## Qué es SSDF

Secure Software Development Framework — 4 grupos de prácticas para mitigar el riesgo de vulnerabilidades en software. Cada práctica tiene tasks con notional examples.

## Los 4 grupos

### PO — Prepare the Organization

Garantizar que la organización (gente, procesos, tech) esté preparada para desarrollo seguro.

| Practice | Task | Aplica a neurox |
|---|---|---|
| PO.1 | Define security requirements | ✅ Sí (security checklist) |
| PO.2 | Define roles and responsibilities | ✅ Sí (EVA + 12 subagentes) |
| PO.3 | Provide tools and resources | ✅ Sí (cargo audit, gitleaks, semgrep) |
| PO.4 | Define metrics | ✅ Sí (security metrics) |
| PO.5 | Implement secure environments | ✅ Sí (containers, CI/CD) |

### PS — Protect the Software

Proteger todos los componentes del software contra tampering y acceso no autorizado.

| Practice | Task | Aplica a neurox |
|---|---|---|
| PS.1 | Protect code from unauthorized access | ✅ Sí (branch protection, signed commits) |
| PS.2 | Provide a mechanism for verifying software | ✅ Sí (SHA256 en registry) |
| PS.3 | Archive and protect software | ✅ Sí (git history) |

### PW — Produce Well-Secured Software

Producir software con mínimas vulnerabilidades.

| Practice | Task | Aplica a neurox |
|---|---|---|
| PW.1 | Design software to meet security requirements | ✅ Sí (threat modeling) |
| PW.2 | Review code | ✅ Sí (sixbell-reviewer) |
| PW.3 | Test software | ✅ Sí (cargo test, integration tests) |
| PW.4 | Configure compilation, interpreter, build | ✅ Sí (CI builds) |
| PW.5 | Provide a mechanism for verifying software | ✅ Sí (signing) |
| PW.6 | Configure and harden | ✅ Sí (default configs seguros) |
| PW.7 | Review and analyze human-readable code | ✅ Sí (semgrep, code review) |
| PW.8 | Test executable code | ✅ Sí (cargo test, e2e) |

### RV — Respond to Vulnerabilities

Identificar vulnerabilidades residuales y responder apropiadamente.

| Practice | Task | Aplica a neurox |
|---|---|---|
| RV.1 | Identify and confirm vulnerabilities | ✅ Sí (auditorías) |
| RV.2 | Assess, prioritize, and assign | ✅ Sí (severity matrix) |
| RV.3 | Analyze and remediate | ✅ Sí (developer fixes) |
| RV.4 | Analyze root cause | ✅ Sí (post-mortems) |

## Mapeo neurox → SSDF

| Proceso neurox | SSDF Practice |
|---|---|
| Auditoría de seguridad | RV.1 |
| Threat model de feature | PW.1 |
| Code review con sixbell-reviewer | PW.7 |
| `cargo audit` en CI | RV.1 |
| Security checklist pre-PR | PO.1 |
| Branch protection | PS.1 |
| Post-mortem de incidente | RV.4 |
| Disclosure policy | RV.2, RV.3 |

## Schema (para usar en templates)

```yaml
nist_ssdf: PW.7.2   # grupo.practice.task
```

Si el finding no aplica a ningún SSDF, dejar el campo vacío u omitirlo.

## Cómo encontrar el SSDF ID correcto

1. Identificá el grupo (PO/PS/PW/RV)
2. Identificá la practice (PO.1, PW.7, etc.)
3. Identificá el task (PW.7.2, etc.)
4. Si no hay task específico, usar solo el practice (PW.7)

## Referencias

- NIST SP 800-218 (SSDF v1.1): https://csrc.nist.gov/publications/detail/sp/800-218/final
- SSDF Practices table: https://csrc.nist.gov/Projects/ssdf
