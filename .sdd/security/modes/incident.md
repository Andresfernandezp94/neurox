# Mode: INCIDENT — Responder a un incidente de seguridad

> **Este archivo lo lee `bin/next-step.sh` cuando el modo activo es `incident`.**
> NO leer manualmente — seguir el flujo de los scripts.

## Overview

- **Duración esperada**: 1-4 horas para respuesta inicial, postmortem en 1-2 semanas
- **Output**: `security/archive/<YYYY-MM-DD-<incident-name>>/01-incident.md`
- **Cuándo usar**: cuando algo salió mal y necesitás documentarlo
- **Principio**: BLAMELESS postmortem. Foco en el "qué" y el "por qué", no en el "quién"

## Steps

### Step 1: Contain + Initial Postmortem (durante el incidente)

**Qué hacer**: Mientras se resuelve el incidente, documentar el postmortem EN VIVO.

**Acciones concretas**:
1. Abrir `<archive-dir>/01-incident.md` (creado por `start.sh`)
2. Rellenar Metadata básico (fecha, severity estimado)
3. Llenar Timeline EN VIVO a medida que pasan cosas:
   ```
   | Timestamp (UTC) | Actor | Event |
   | 2026-08-08T10:00Z | user | Detected unusual behavior |
   | 2026-08-08T10:05Z | dev | Started investigation |
   | 2026-08-08T10:15Z | dev | Identified root cause |
   | 2026-08-08T10:30Z | dev | Applied fix |
   ```
4. Documentar el Impact tan pronto como se conoce
5. **NO** saltar a Root Cause antes de tener Timeline completo

**Exit condition**: postmortem "borrador vivo" creado (NO validar todavía — eso es al final)

### Step 2: Detailed Timeline (después del incidente, +24h)

**Qué hacer**: Completar el timeline con data de logs/monitoring.

**Acciones concretas**:
1. Revisar logs (CloudWatch, journald, application logs)
2. Correlacionar timestamps con eventos del timeline del step 1
3. Agregar eventos que faltaron (especialmente: cuándo empezó realmente el incidente)
4. Marcar claramente: detección, contención, resolución, recovery

**Exit condition**: timeline lo más exacto posible

### Step 3: Root Cause Analysis (después del incidente, +1-3 días)

**Qué hacer**: Hacer 5-whys para llegar a la causa raíz sistémica.

**Acciones concretas**:
1. Empezar por el síntoma visible (ej: "el servicio estuvo caído 2h")
2. Por qué → respuesta 1 → por qué → respuesta 2 → ...
3. Continuar hasta llegar a una causa SISTÉMICA (no individual)
4. La causa sistémica debe ser algo que se pueda cambiar con un proceso/tool/training
5. Documentar también los "Contributing Factors" (cosas que empeoraron el incidente)

**Exit condition**: 5-whys completo + causa sistémica identificada

### Step 4: Lessons + Action Items (después, +3-7 días)

**Qué hacer**: Documentar qué aprendimos y qué vamos a cambiar.

**Acciones concretas**:
1. En "Lessons Learned", escribir 1-3 oraciones por lección (máx 5 lecciones)
2. En "Action Items", crear tabla con:
   - Action (concreta, accionable)
   - Priority (Critical/High/Medium/Low)
   - Owner (nombre)
   - Deadline (YYYY-MM-DD)
   - Status (Open/InProgress/Done)
3. Cada action item debe ser algo que SE PUEDE HACER (no "mejorar la seguridad")

**Exit condition**: tabla de action items con al menos 1 owner + 1 deadline

### Step 5: Review (después, +5-10 días)

**Qué hacer**: Review por pares antes de publicar.

**Acciones concretas**:
1. Compartir el postmortem con al menos 1 reviewer (humano o agente)
2. Validar:
   - Blameless (no apunta a individuos)
   - Causa sistémica (no solo síntoma)
   - Action items tienen owners reales
3. **Validar**: `bash security/bin/validate-incident.sh <archive-dir>/01-incident.md`

**Exit condition**: validator exit 0 + al menos 1 review sign-off

### Step 6: Publish + Close (después, +10-14 días)

**Qué hacer**: Publicar el postmortem y cerrar el proceso.

**Acciones concretas**:
1. Marcar el postmortem como Reviewed/Published
2. Si hay learnings que aplican a otras auditorías → referenciarlos en auditorías futuras
3. `bash security/bin/advance-step.sh` para cada step
4. `bash security/bin/close.sh`
5. Commit al repo

**Exit condition**: MANIFEST reseteado

## Common pitfalls

- ❌ **Asignar culpa a individuos** — usar nombres propios solo como "actor del evento", nunca como "responsable"
- ❌ **Saltar el 5-whys** — quedarse en el síntoma es facilismo
- ❌ **Action items sin owner** — sin owner no se hacen
- ❌ **Action items vagos** ("mejorar monitoring") — ser concreto ("agregar alerta de latencia > 5s en Grafana")
- ❌ **No publicar** — el valor del postmortem es que otros aprendan

## Lessons learned NO son blame

✅ "Aprendimos que nuestro proceso de code review no incluye security review de nuevos endpoints WS"
❌ "Juan debería haber revisado el WS upgrade"

✅ "Aprendimos que el bind a 0.0.0.0 estaba documentado como loopback pero el flag CLI lo cambió"
❌ "El operador se equivocó al configurar el bind"

## Related docs

- `security/templates/incident-postmortem.template.md`
- Google SRE Chapter 15: https://sre.google/sre-book/postmortem-culture/
- Etsy Morgue: https://github.com/etsy/morgue
