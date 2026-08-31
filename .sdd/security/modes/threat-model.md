# Mode: THREAT-MODEL — Threat model de una feature/componente

> **Este archivo lo lee `bin/next-step.sh` cuando el modo activo es `threat-model`.**
> NO leer manualmente — seguir el flujo de los scripts.

## Overview

- **Duración esperada**: 30-60 minutos (feature simple) / 2-4 horas (sistema complejo)
- **Output**: `security/archive/<YYYY-MM-DD-<scope>>/01-threat-model.md`
- **Cuándo usar**: ANTES de implementar una feature que toca auth/redatos/crypto/business logic
- **Pre-requisitos**: spec/feature description, conocimiento del sistema actual

## Steps

### Step 1: Context + Architecture (15 min)

**Qué hacer**: Documentar QUÉ se está modelando.

**Acciones concretas**:
1. Abrir `<archive-dir>/01-threat-model.md` (creado por `start.sh`)
2. Rellenar Metadata
3. Llenar "Context": qué feature, qué cambia, 1-3 párrafos
4. En "Architecture": ASCII art de los componentes y data flows
5. Llenar "Trust Boundaries": lista explícita de cada boundary
6. Llenar "Assets": qué se está protegiendo (credenciales, data, etc.)
7. **Validar**: `bash security/bin/validate-threat-model.sh <archive-dir>/01-threat-model.md`

**Exit condition**: validator exit 0

### Step 2: STRIDE Analysis (20-40 min)

**Qué hacer**: Para cada data flow, evaluar las 6 categorías STRIDE.

**Acciones concretas**:
1. Por cada data flow identificado en step 1, crear sección `### Data flow N: <descripción>`
2. Por cada data flow, completar tabla STRIDE (S/T/R/I/D/E)
3. Para cada categoría donde hay amenaza, escribir mitigación concreta
4. Si una categoría NO aplica, escribir "N/A" (no dejar vacío)

**Exit condition**: todas las data flows tienen tabla STRIDE completa

### Step 3: Identify Threats (15 min)

**Qué hacer**: Agregar las amenazas concretas encontradas.

**Acciones concretas**:
1. Crear tabla "Threats Identified" en la sección correspondiente
2. Por cada amenaza: ID (TM-NNN), descripción corta, STRIDE, severity sugerida, mitigación
3. Usar `bash security/bin/score-severity.sh "<description>"` para sugerir severity
4. Usar `bash security/bin/threat-classify.sh "<description>"` para STRIDE

**Exit condition**: tabla completa con al menos 1 amenaza (si hay 0 amenazas, replantear el scope)

### Step 4: Decisions (10 min)

**Qué hacer**: Documentar las decisiones de seguridad tomadas durante el threat model.

**Acciones concretas**:
1. Crear sección "Security Decisions"
2. Por cada decisión importante, escribir:
   - D-NN: decisión
   - Rationale: por qué se tomó
   - Alternative considered: qué se rechazó y por qué

**Exit condition**: al menos 1 decisión documentada (si no hay decisiones, no hacía falta threat model)

### Step 5: Validate + Close (5 min)

**Qué hacer**: Validar todo y cerrar.

**Acciones concretas**:
1. `bash security/bin/validate-threat-model.sh <archive-dir>/01-threat-model.md`
2. Si exit 0 → `bash security/bin/advance-step.sh`
3. Repetir hasta step 5 done
4. `bash security/bin/close.sh`
5. Commit el threat model al repo

**Exit condition**: MANIFEST reseteado

## Common pitfalls

- ❌ No documentar trust boundaries → amenaza implícita no mitigada
- ❌ "N/A" en STRIDE sin justificación → siempre explicar por qué
- ❌ Decisiones sin rationale → futuro dev no sabrá por qué se eligió
- ❌ Olvidar datos en tránsito vs. datos en reposo → tratar por separado
- ❌ Asumir "confiamos en X" sin verificar → cuestionar siempre

## Related docs

- `security/templates/threat-model.template.md`
- `security/lib/stride-explained.md`
- `security/lib/owasp-asvs-mini.md`
- Shostack's 4 Questions Framework: https://github.com/adamshostack/4QuestionFrame
