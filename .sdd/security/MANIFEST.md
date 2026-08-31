# MANIFEST.md — Security Process State Machine

> **Cualquier agente lee este archivo PRIMERO** para saber si hay un proceso activo.

## Current State

<!-- El script bin/start.sh actualiza este bloque automáticamente. NO editar a mano. -->

```yaml
mode: null
process: null
started_at: null
current_step: 0
total_steps: 7
last_validated: null
```

## Cómo se usa este archivo

### 1. Retomar un proceso interrumpido

```bash
bash security/bin/next-step.sh
```

El script lee este MANIFEST y `modes/<mode>.md`, te dice exactamente qué hacer.

### 2. Empezar un proceso nuevo

```bash
bash security/bin/start.sh <mode> "<scope-name>"
```

Modos disponibles:
- `audit` — auditoría completa de seguridad
- `threat-model` — threat model de una feature nueva
- `incident` — responder a un incidente de seguridad
- `disclosure` — manejar un reporte externo de vulnerabilidad

El script:
1. Valida que el mode sea válido
2. Crea directorio en `archive/<fecha>-<scope>/`
3. Inicializa `.state/<process>/`
4. Actualiza este MANIFEST
5. Te dice cuál es el paso 1

### 3. Validar un paso

```bash
bash security/bin/validate-step.sh
```

Verifica que el paso actual esté completo según el template. Si pasa (exit 0), avanza al siguiente. Si falla (exit 1), te dice qué falta.

### 4. Cerrar un proceso

```bash
bash security/bin/close.sh
```

Valida que TODOS los pasos estén done, commitea los archivos a `archive/`, y resetea el MANIFEST.

## Estados posibles

| Estado | Significado | Acción |
|---|---|---|
| `mode: null` | No hay proceso activo | Iniciar uno con `start.sh` |
| `mode: X, current_step: N` | Proceso activo, en paso N | Continuar con `next-step.sh` |
| `mode: X, current_step: total_steps` | Todos los pasos hechos | Cerrar con `close.sh` |
| `mode: X, last_validated: <old>` | Validación pendiente (> 1 día) | Re-validar |

## Schema de un process ID

`<process>` sigue el formato `YYYY-MM-DD-<scope>`:
- `YYYY-MM-DD` — fecha de inicio (ISO 8601)
- `<scope>` — kebab-case, max 50 chars
- Ejemplos válidos: `2026-08-08-neurox-runtime`, `2026-08-15-voice-plugin`, `2026-09-01-incident-ws-leak`

## Schema de un finding ID

`<finding-id>` sigue el formato `SEC-NNNN-YYYY-NNN`:
- `NNNN` — número secuencial de 4 dígitos (0001, 0002, ...)
- `YYYY` — año de descubrimiento
- `NNN` — número secuencial dentro del año (001, 002, ...)
- Ejemplos: `SEC-0001-2026-001`, `SEC-0001-2026-002`

Generar automáticamente con `bash security/bin/new-finding.sh`.

## Schema de severity

Severidad ∈ {`Critical`, `High`, `Medium`, `Low`}.

Para clasificar automáticamente: `bash security/bin/score-severity.sh "<description>"`

## Schema de STRIDE

Categoría ∈ {`S`, `T`, `R`, `I`, `D`, `E`}.

Para clasificar automáticamente: `bash security/bin/threat-classify.sh "<description>"`

## Anti-drift checks

Este MANIFEST es validado por `bin/validate-manifest.sh` antes de cada commit:

- ✅ El campo `mode` es uno de los 4 válidos
- ✅ El campo `process` matchea el regex `^\d{4}-\d{2}-\d{2}-[a-z0-9-]+$`
- ✅ Si `mode != null`, entonces `total_steps > 0`
- ✅ `current_step <= total_steps`
- ✅ Las fechas son ISO 8601 válidas

## Changelog del MANIFEST

| Fecha | Cambio |
|---|---|
| 2026-08-08 | Creación inicial con 4 modos (audit, threat-model, incident, disclosure) |
| 2026-08-17 | audit 2026-08-16-neurox-runtime: paso 3 cerrado manualmente (F-14 documenta el bug del validator) |
