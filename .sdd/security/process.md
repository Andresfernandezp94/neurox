# Security Process — neurox (narrative reference)

> **Este archivo es REFERENCIA — NO se ejecuta.**
> Para ejecutar el proceso, usá los scripts de `bin/` que apuntan a `modes/<mode>.md`.

## Propósito

Este documento describe el "por qué" del proceso de seguridad. Los scripts y templates en otros archivos son el "cómo".

## Principios fundamentales

### 1. Defense-in-depth en 4 fases

```
   PREPARAR  →  PROTEGER  →  PRODUCIR  →  RESPONDER
       │            │             │             │
 NIST PO/1-5    NIST PS/1-3   NIST PW/1-8   NIST RV/1-4
 OWASP SAMM     OWASP SAMM   OWASP SAMM    OWASP SAMM
```

El proceso cubre las 4 fases con diferentes modos:

| Fase | Modo del proceso | Cuándo se usa |
|---|---|---|
| Preparar | threat-model | Antes de implementar una feature |
| Proteger | (en auditoría) | Verificar que el código está protegido |
| Producir | audit | Auditar el código ya escrito |
| Responder | incident + disclosure | Cuando algo salió mal o alguien reporta algo |

### 2. Blameless culture (Google SRE)

El modo `incident` aplica **postmortems blameless**:
- Foco en el "qué pasó" y "por qué pasó", no en "quién tuvo la culpa"
- Asumimos que todos tenían buenas intenciones con la info que tenían
- El objetivo es APRENDER, no castigar

### 3. Reproducibilidad

Cada finding tiene:
- Comandos exactos para reproducir
- Output esperado
- Fix concreto
- Test de regresión

Esto permite que **cualquier agente** pueda validar el fix sin tener que contactar al descubridor original.

### 4. Anti-drift

El proceso resiste el drift gracias a 4 mecanismos:

| Mecanismo | Cómo funciona |
|---|---|
| Templates con placeholders `[REQUIRED]` | El validator rechaza outputs incompletos |
| State machine persistente (`.state/`) | Cada step debe estar done antes del siguiente |
| Validators binarios (exit 0/1) | No hay "casi bien", hay válido o inválido |
| Reference docs en `lib/` | No reinventamos OWASP/NIST, los citamos |

### 5. Agent-agnostic

El proceso está diseñado para que **cualquier agente** (humano o IA, con o sin experiencia en seguridad) pueda ejecutarlo:
- Comandos exactos, no "investiga"
- Decisiones con opciones pre-cargadas, no open-ended
- Outputs validables mecánicamente
- Reference docs para conceptos nuevos

## Framework de referencia

El proceso se basa en frameworks reconocidos de la industria:

| Framework | Uso | Dónde se referencia |
|---|---|---|
| **OWASP ASVS v5.0** | Requisitos de verificación | `lib/owasp-asvs-mini.md` |
| **NIST SSDF v1.1** | Prácticas de desarrollo seguro | `lib/nist-ssdf-mini.md` |
| **OWASP SAMM v2** | Modelo de madurez | (referencia en este archivo) |
| **STRIDE** | Modelo de amenazas | `lib/stride-explained.md` |
| **CVSS 3.1** | Scoring de severidad | `lib/severity-matrix.md` |
| **CISA CVD** | Coordinated Vulnerability Disclosure | `lib/disclosure-policy.md` |
| **Google SRE Ch.15** | Blameless postmortem | `modes/incident.md` |
| **Shostack's 4 Questions** | Threat modeling methodology-neutral | `modes/threat-model.md` |

No redefinimos estos frameworks — los **citamos** cuando se necesitan.

## Los 4 modos (resumen)

### audit
Auditoría completa de un repo/sistema. Genera un reporte con findings.
- 7 steps
- Duración: 1-3 horas (L1) / 1-2 días (L3)
- Output: `archive/<date>-<scope>/{01-proposal,02-design,04-report,findings/F-NN-*}`

### threat-model
Análisis de seguridad de una feature ANTES de implementarla.
- 5 steps
- Duración: 30-60 minutos
- Output: `archive/<date>-<scope>/01-threat-model.md`

### incident
Respuesta a un incidente + postmortem blameless.
- 6 steps
- Duración: 1-14 días (según complejidad)
- Output: `archive/<date>-<incident-name>/01-incident.md`

### disclosure
Manejo de un reporte externo de vulnerabilidad.
- 5 steps
- Duración: 1-90 días (según SLA)
- Output: `archive/<date>-<slug>/01-finding.md` + GitHub Advisory

## Cómo se relacionan con OpenSpec

neurox usa metodología OpenSpec (proposal → design → tasks → code). La seguridad se integra:

```
OpenSpec proposal.md  ← agrega sección "Security Implications" (STRIDE summary)
OpenSpec design.md    ← agrega sección "Security Design Decisions" + threat model link
OpenSpec tasks.md     ← agrega tasks de seguridad (audit, tests de regresión, etc.)
```

Y el modo `audit` puede dispararse desde:
- Trigger 1: programado (trimestral)
- Trigger 2: post-release (validar que el release no introdujo vulns)
- Trigger 3: pre-merge de cambios sensibles (auth, networking, crypto)
- Trigger 4: después de un incidente (entender la superficie)

## Cómo se relacionan con los subagentes

| Subagente | Rol en seguridad |
|---|---|
| EVA (primary) | Orquesta — puede delegar |
| sixbell-researcher | Lee código, NO audita — pasa findings a sixbell-developer |
| sixbell-developer | Implementa fixes |
| sixbell-reviewer | Code review (incluyendo security review) |
| sixbell-security | Auditor dedicado (puede ser invocado para auditorías grandes) |
| sixbell-doc-agent | Genera docs (incluyendo advisories) |

**Importante**: cualquier agente puede ejecutar el proceso, no solo sixbell-security.

## Cuándo NO usar este proceso

- **Auditorías de compliance externas** (SOC2, ISO 27001): este proceso es para security engineering, no compliance
- **Penetration testing de caja negra**: este proceso es de caja blanca (lee el código)
- **Bug bounty programs externos**: usar plataformas dedicadas (HackerOne, Bugcrowd)
- **Auditorías regulatorias** (PCI-DSS, HIPAA): tienen frameworks propios

## Changelog

| Fecha | Cambio |
|---|---|
| 2026-08-08 | Creación inicial con 4 modos (audit, threat-model, incident, disclosure) |

## Referencias externas

- OWASP ASVS: https://owasp.org/www-project-application-security-verification-standard/
- OWASP SAMM: https://owaspsamm.org/
- NIST SSDF: https://csrc.nist.gov/Projects/ssdf
- Google SRE Book Ch.15: https://sre.google/sre-book/postmortem-culture/
- CISA CVD: https://www.cisa.gov/resources-tools/programs/coordinated-vulnerability-disclosure-program
- STRIDE: https://en.wikipedia.org/wiki/STRIDE_(security)
- CVSS 3.1: https://www.first.org/cvss/specification-document
- Threat Modeling Manifesto: https://www.threatmodelingmanifesto.org/
