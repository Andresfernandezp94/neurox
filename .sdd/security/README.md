# Security Process — neurox

**Entry point para CUALQUIER agente (humano o IA) que necesite ejecutar el proceso de seguridad.**

## Estás aquí

Estás en el directorio `security/` del SoT de neurox. Este directorio define **un proceso agent-agnostic, robusto, resumable y validable** para seguridad.

## Cómo empezar (5 segundos)

```bash
# ¿Tenés un proceso activo?
bash security/bin/next-step.sh

# ¿No hay proceso activo? Iniciar uno:
bash security/bin/start.sh audit "neurox-runtime"     # auditoría completa
bash security/bin/start.sh threat-model "feature-X"    # threat model de feature
bash security/bin/start.sh incident "ws-bind-leak"     # responder a incidente
bash security/bin/start.sh disclosure "sec-0001-2026"  # manejar reporte de vuln
```

Eso es todo. El script te dice qué hacer paso a paso.

## Mapa del directorio

| Carpeta/Archivo | Propósito | Cuándo leer |
|---|---|---|
| `README.md` | Este archivo | Al llegar |
| `MANIFEST.md` | State machine del proceso activo | Al retomar |
| `process.md` | Flujo narrativo completo (referencia) | Para entender el "por qué" |
| `modes/` | 4 modos (audit / threat-model / incident / disclosure) | Al iniciar cualquier modo |
| `templates/` | Templates con placeholders estrictos | Al generar un artefacto |
| `bin/` | Scripts bash (validators + tools + state mgmt) | SIEMPRE se ejecutan, no se leen |
| `lib/` | Reference docs (STRIDE, ASVS-mini, SSDF-mini, etc.) | Cuando un script lo indique |
| `archive/` | Auditorías/incidentes cerrados (inmutable) | Después de cerrar un proceso |
| `.state/` | State machine persistente (gitignored) | Lo manejan los scripts, no se edita |

## Principios (NO NEGOCIABLES)

1. **Cualquier agente puede ejecutarlo** — comandos exactos, no opciones abiertas
2. **Resumable** — si te interrumpís, `next-step.sh` te dice dónde estás
3. **Validable** — cada output pasa por un script, exit 0 = OK, exit 1 = corregir
4. **Anti-drift** — templates con placeholders `[REQUIRED: ...]` fuerzan estructura
5. **Idempotente** — re-ejecutar un paso da el mismo resultado
6. **Sin expertise previa** — las reference docs explican conceptos cuando se necesitan

## Lo que NO hace este proceso

- No modifica código de los repos (eso lo hace `sixbell-developer`)
- No hace deploys (eso lo hace `sixbell-deployer`)
- No inventa frameworks — cita OWASP/NIST/CISA sin redefinirlos
- No decide prioridades finales — eso lo hace Andrés

## Quick reference: severidad (de `lib/severity-matrix.md`)

| Severidad | CVSS | SLA | Ejemplo |
|---|---|---|---|
| Critical | 9.0-10.0 | < 24h | RCE, secret expuesto |
| High | 7.0-8.9 | < 7 días | Auth bypass, SQL injection |
| Medium | 4.0-6.9 | < 30 días | XSS stored, info disclosure |
| Low | 0.1-3.9 | < 90 días | Missing headers, verbose errors |

Para clasificar automáticamente: `bash security/bin/score-severity.sh "<descripción del bug>"`

## Quick reference: STRIDE

| Letra | Categoría | Pregunta clave |
|---|---|---|
| S | Spoofing | ¿Se puede suplantar la identidad? |
| T | Tampering | ¿Se puede modificar data sin autorización? |
| R | Repudiation | ¿Se puede negar una acción sin evidencia? |
| I | Information Disclosure | ¿Se filtra data que no debería? |
| D | Denial of Service | ¿Se puede tirar el servicio? |
| E | Elevation of Privilege | ¿Se puede escalar permisos? |

Para clasificar automáticamente: `bash security/bin/threat-classify.sh "<descripción>"`

## Workflow típico (audit)

```
[Andrés] "Auditá neurox por seguridad"
   ↓
[Cualquier agente] bash security/bin/start.sh audit "neurox-runtime"
   ↓
[Agente] bash security/bin/next-step.sh → te dice el paso 1
   ↓
[Agente] ejecuta paso 1 → bash security/bin/validate-step.sh
   ↓
[Agente] repite hasta terminar
   ↓
[Agente] bash security/bin/close.sh → commit al repo
   ↓
[Andrés] revisa el reporte en security/archive/<fecha>-<scope>/
```

## Cómo extender

Si necesitás un nuevo modo (ej: "compliance check"):
1. Crear `modes/compliance.md` siguiendo el patrón de los otros modos
2. Crear templates específicos en `templates/`
3. Crear validators en `bin/`
4. Actualizar `MANIFEST.md` y este README
5. Commit + PR

## Reglas de contribución

- Cada script en `bin/` debe ser **idempotente** y retornar exit 0/1
- Cada template debe tener placeholders `[REQUIRED: ...]` o `[OPTIONAL: ...]`
- Cada modo debe referenciar scripts, no dar instrucciones abiertas
- Cada reference doc en `lib/` debe ser **independiente** (no asume contexto previo)
