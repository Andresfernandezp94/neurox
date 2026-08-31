# Mode: AUDIT — Auditoría completa de seguridad

> **Este archivo lo lee `bin/next-step.sh` cuando el modo activo es `audit`.**
> NO leer manualmente — seguir el flujo de los scripts.

## Overview

- **Duración esperada**: 1-3 horas (L1) / 3-5 horas (L2) / 1-2 días (L3)
- **Output**: `security/archive/<YYYY-MM-DD-<scope>>/` con 7 archivos
- **Pre-requisitos**: acceso read-only al repo, `cargo`/`pnpm`/`rg` instalados

## Steps

### Step 1: Proposal (10 min)

**Qué hacer**: Definir QUÉ se va a auditar y con qué profundidad.

**Acciones concretas**:
1. Abrir `<archive-dir>/01-proposal.md` (creado por `start.sh`)
2. Rellenar los campos `[REQUIRED: ...]` siguiendo el template
3. Elegir Depth: L1 (1-2h, solo herramientas) / L2 (3-5h, + code review) / L3 (1-2d, + threat model)
4. Marcar las 6 layers que se cubrirán (secrets, deps, input, auth, networking, governance)
5. **Validar**: `bash security/bin/validate-proposal.sh <archive-dir>/01-proposal.md`
6. Si exit 0 → `bash security/bin/advance-step.sh`
7. Si exit 1 → corregir lo que el validator indique y volver a paso 5

**Exit condition**: validator exit 0

### Step 2: Design (20 min)

**Qué hacer**: Diagramar la arquitectura, hacer threat model conceptual, listar herramientas concretas.

**Acciones concretas**:
1. Abrir `<archive-dir>/02-design.md` (creado por `start.sh`)
2. ASCII art de los componentes y data flows
3. Por cada data flow, tabla STRIDE (S/T/R/I/D/E) con amenazas y mitigaciones
4. En "Tools to Use", poner **comandos bash exactos** (no "investigar X")
5. **Validar**: `bash security/bin/validate-design.sh <archive-dir>/02-design.md`
6. Si exit 0 → `bash security/bin/advance-step.sh`

**Exit condition**: validator exit 0

### Step 3: Execution — Static Analysis (45-90 min según Depth)

**Qué hacer**: Ejecutar las herramientas y recolectar los findings.

**Acciones concretas**:

Para cada tool en el design.md:
```bash
# 1. Secret detection
rg -i "(sk-[a-zA-Z0-9]{20,}|AKIA[0-9A-Z]{16}|AIza[0-9A-Za-z\-_]{35}|ghp_[a-zA-Z0-9]{36})" repos/ -g '!target' -g '!.git'

# 2. Dependency scan
cd repos/neuro-pro && cargo audit --no-fetch
cd ../neuro-pro-plugin-memory && cargo audit --no-fetch
# ... etc para cada repo Rust

# 3. Manual code review (L2/L3)
rg -i "Command::new\(.+/bin/sh\)" repos/
rg -i "sqlx::query\(format!" repos/
rg -i "bind.*0\.0\.0\.0" repos/
```

Por cada hallazgo encontrado:
1. Generar ID: `bash security/bin/new-finding.sh`
2. Sugerir severity: `bash security/bin/score-severity.sh "<descripción>"`
3. Sugerir STRIDE: `bash security/bin/threat-classify.sh "<descripción>"`
4. Crear `findings/F-NN-<slug>.md` desde `templates/finding.template.md`
5. Rellenar todos los campos REQUIRED
6. **Validar**: `bash security/bin/validate-finding.sh findings/F-NN-<slug>.md`
7. Si exit 0 → siguiente finding
8. Si exit 1 → corregir y re-validar

**Validar todos los findings al final**:
```bash
bash security/bin/validate-all-findings.sh <archive-dir>/findings/
```

**Exit condition**: todos los findings validados

### Step 4: Report (30 min)

**Qué hacer**: Compilar el reporte ejecutivo.

**Acciones concretas**:
1. Abrir `<archive-dir>/04-report.md` (creado por `start.sh`)
2. Completar Executive Summary (3-5 oraciones, sin jerga)
3. Llenar Findings Summary table con TODOS los findings
4. Llenar Findings by Severity (Critical/High/Medium/Low)
5. Llenar Findings by STRIDE Category (agregación)
6. Documentar "What Was NOT Found" (builds confidence)
7. Documentar "Out of Scope" (descartados, por qué)
8. Linkear todos los Detailed Findings a archivos `findings/F-*.md`
9. Priorizar Immediate Actions Required (Critical/High first)
10. **Validar**: `bash security/bin/validate-report.sh <archive-dir>/04-report.md`

**Exit condition**: validator exit 0

### Step 5: Review (15 min)

**Qué hacer**: Self-review del reporte completo.

**Acciones concretas**:
1. Releer el report.md completo
2. Verificar que cada finding tiene PoC ejecutable y reproducible
3. Verificar que la severidad es coherente con severity-matrix.md
4. Verificar que el executive summary es comprensible para un no-técnico
5. Si encontrás issues → volver al step correspondiente

**Exit condition**: auto-aprobación documentada

### Step 6: Stakeholder Communication (10 min)

**Qué hacer**: Comunicar hallazgos críticos a Andrés.

**Acciones concretas**:
1. Para cada Critical/High, escribir un mensaje directo al usuario:
   - Resumen de 1 línea
   - Vector de ataque
   - Mitigación inmediata sugerida
   - Link al finding completo
2. NO esperar a cerrar el proceso para comunicar — comunicar inmediatamente

**Exit condition**: comunicación enviada

### Step 7: Close (5 min)

**Qué hacer**: Cerrar el proceso formalmente.

**Acciones concretas**:
1. Verificar que todos los steps anteriores están done
2. `bash security/bin/advance-step.sh` (esto marca el step 7 como done)
3. `bash security/bin/close.sh` (esto resetea el MANIFEST)
4. Commit los archivos a git: `git add security/archive/<id>/ && git commit -m "docs(security): audit <id> complete"`
5. Push si hay remote configurado

**Exit condition**: MANIFEST reseteado, archivos commiteados

## Common pitfalls

- ❌ Inventar campos que no están en los templates → el validator rechaza
- ❌ Omitir el PoC o hacerlo no-ejecutable → el validator rechaza
- ❌ Severidad sin justificación → usar `score-severity.sh` primero
- ❌ Cerrar sin comunicar Critical/High → step 6 es obligatorio
- ❌ Modificar archivos en `archive/` después de cerrar → son inmutables

## Tips de eficiencia

- **Empezá por las tools automáticas** (cargo audit, rg patterns). Después hacé code review manual.
- **Agrupá findings similares** (ej: 4 repos sin secret scanning → 1 solo finding)
- **No reportes falsos positivos** — verificá el contexto antes de crear el archivo
- **Severity la define el PATRÓN, no tu opinión** — usá `score-severity.sh` primero

## Related docs

- `security/templates/audit-proposal.template.md`
- `security/templates/audit-design.template.md`
- `security/templates/audit-report.template.md`
- `security/templates/finding.template.md`
- `security/lib/severity-matrix.md`
- `security/lib/stride-explained.md`
