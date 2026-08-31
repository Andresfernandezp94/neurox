# Mode: DISCLOSURE — Manejar un reporte externo de vulnerabilidad

> **Este archivo lo lee `bin/next-step.sh` cuando el modo activo es `disclosure`.**
> NO leer manualmente — seguir el flujo de los scripts.

## Overview

- **Duración esperada**: 1-3 horas por step, 90 días total hasta disclosure
- **Output**: `security/archive/<YYYY-MM-DD-<slug>>/` con finding + advisory
- **Cuándo usar**: cuando alguien reporta una vuln externamente (reporter externo)
- **Policy base**: `security/lib/disclosure-policy.md` (leer primero)

## Steps

### Step 1: Initial Triage (mismo día del reporte)

**Qué hacer**: Validar el reporte y estimar severity.

**Acciones concretas**:
1. Abrir `<archive-dir>/01-finding.md` (creado por `start.sh`)
2. Rellenar Metadata:
   - ID: `bash security/bin/new-finding.sh`
   - Severity estimado: `bash security/bin/score-severity.sh "<description>"`
   - STRIDE: `bash security/bin/threat-classify.sh "<description>"`
3. Llenar Location (archivo:línea donde está el bug)
4. Llenar Description (resumir el reporte externo)
5. Copiar el PoC del reporte externo
6. **Validar**: `bash security/bin/validate-finding.sh <archive-dir>/01-finding.md`
7. Enviar email de acknowledge al reporter (ver template en disclosure-policy.md)
   - **SLA**: Critical < 24h, High < 3 días, Medium < 7 días, Low < 14 días

**Exit condition**: finding creado + acknowledge enviado

### Step 2: Reproduction + Confirmation (1-7 días)

**Qué hacer**: Reproducir el bug localmente.

**Acciones concretas**:
1. Clonar el repo en la versión reportada
2. Aplicar el PoC del reporter
3. **Si NO se reproduce**: responder al reporter explicando, pedir más info
4. **Si SÍ se reproduce**: confirmar el finding está bien
5. Actualizar Impact en el finding con tu propio análisis
6. Decidir: ¿es válido? ¿es duplicado de un finding existente? ¿es theoretical?

**Casos**:
- ✅ **Válido**: continuar a step 3
- ❌ **No válido / False positive**: responder al reporter + cerrar proceso con `close.sh`
- 🔄 **Duplicado**: mergear con el finding existente, notificar al reporter

**Exit condition**: confirmado válido (o cerrado si false positive)

### Step 3: Coordinate Fix (7-90 días)

**Qué hacer**: Coordinar con sixbell-developer para crear el fix.

**Acciones concretas**:
1. Compartir el finding con sixbell-developer
2. Esperar el PR con el fix
3. Validar que el fix:
   - Resuelve el issue
   - No introduce regresiones
   - Tiene test de regresión
4. **Coordinar fecha de disclosure con el reporter**:
   - Default: 90 días desde el reporte
   - Si el reporter quiere más/menos tiempo: acordar
   - Si hay evidencia de exploitation activa: disclosure inmediata

**Exit condition**: PR con fix merged + fecha de disclosure acordada

### Step 4: Pre-Disclosure (7 días antes)

**Qué hacer**: Notificar al reporter y preparar advisory.

**Acciones concretas**:
1. Email al reporter: "Fix merged, disclosure en 7 días (fecha X)"
2. Si el reporter quiere crédito: confirmar nombre/handle
3. Preparar el advisory público (template en disclosure-policy.md)
4. Preparar el mensaje de release notes
5. (Opcional) Crear GitHub Security Advisory en draft

**Exit condition**: advisory listo en draft

### Step 5: Public Disclosure + Close (fecha X)

**Qué hacer**: Publicar y cerrar.

**Acciones concretas**:
1. Publicar el advisory (GitHub Security Advisories → Publish)
2. Merge del fix → tag de release → publicar release notes
3. Si el reporter quiere crédito: agregar al advisory + CHANGELOG
4. Email final al reporter agradeciendo
5. Mover el finding a `security/archive/<date>-<slug>/`
6. `bash security/bin/advance-step.sh` para cada step
7. `bash security/bin/close.sh`
8. Commit al repo

**Exit condition**: MANIFEST reseteado

## Disclosure timeline default (basado en Google Project Zero + ZDI)

```
Día 0:    Reporte recibido
Día 1-7:  Acknowledge + triage
Día 7-30: Trabajo en fix
Día 60:   Pre-disclosure al reporter (7 días antes)
Día 67:   Disclosure pública
Día 90:   Max deadline (si no hay fix, disclosure unilateral)
```

## Excepciones al timeline

| Situación | Disclosure |
|---|---|
| Explotación activa observada | Inmediata (sin esperar fix) |
| Reporter no responde por 60 días | Unilateral |
| Reporter pide disclosure inmediata | Sin esperar 90 días |
| Vendor confirma que NO va a fix | Disclosure a los 45 días |

## Common pitfalls

- ❌ No enviar acknowledge dentro del SLA → reporter pierde confianza
- ❌ Discutir detalles técnicos en canales inseguros (Discord, Twitter)
- ❌ Prometer bounty si no hay programa de bounty
- ❌ Disclose sin coordinar con el reporter → rompe la confianza
- ❌ Cerrar como false positive sin verificar bien → reputación

## Related docs

- `security/templates/finding.template.md`
- `security/lib/disclosure-policy.md` — policy completo
- ISO/IEC 29147 — Vulnerability Disclosure
- ISO/IEC 30111 — Vulnerability Handling
- CISA CVD: https://www.cisa.gov/resources-tools/programs/coordinated-vulnerability-disclosure-program
