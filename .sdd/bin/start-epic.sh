#!/usr/bin/env bash
# start-epic.sh — Crear nueva épica con numeración correlativa.
# Uso: bash bin/start-epic.sh <slug-kebab-case>
# Exit 0 = éxito, exit 1 = error.

set -euo pipefail

if [ $# -ne 1 ]; then
  echo "Uso: bash bin/start-epic.sh <slug-kebab-case>"
  echo "  ej: bash bin/start-epic.sh multi-tenant-rate-limiting"
  exit 1
fi

SLUG="$1"
SDD="$(cd "$(dirname "$0")/.." && pwd)"
CHANGE_DIR="$SDD/changes"

# 1. Validar slug (kebab-case, max 60 chars, sin caracteres especiales)
if ! [[ "$SLUG" =~ ^[a-z0-9][a-z0-9-]{0,58}[a-z0-9]$ ]]; then
  echo "Error: slug debe ser kebab-case, max 60 chars, alphanumeric + guiones"
  echo "  Recibido: '$SLUG'"
  exit 1
fi

# 1b. EP-0001 v1.2: rechazar formato fecha (EP-YYYY-MM-DD-*).
# Los IDs de EP son **strict sequential numeric** desde 2026-08-29. Si
# necesitás fecha, usá un slug descriptivo (e.g. "tools-engine-integration")
# y el script le asignará el siguiente número correlativo.
if [[ "$SLUG" =~ ^[0-9]{4}-[0-9]{2}-[0-9]{2} ]]; then
  echo "Error: formato fecha no permitido en slugs de EP."
  echo "  Usá un slug descriptivo (ej: 'multi-agent-live-switch'). El script"
  echo "  asignará el siguiente número correlativo (EP-NNNN) automáticamente."
  echo "  Recibido: '$SLUG'"
  exit 1
fi

# 2. Calcular siguiente número correlativo.
# Busca el último EP-NNNN usado en los directorios changes/ y archived/.
# NO lee de STATE.md (puede tener números en texto descriptivo).
#
# EP-0001 v1.2: solo se consideran carpetas con formato `EP-NNNN-<slug>`.
# Las carpetas legacy fecha (EP-YYYY-MM-DD-*) ya fueron renumeradas en
# commit 6ff5e5e y siguientes.
LAST=0
for dir in "$CHANGE_DIR"/EP-* "$SDD/archived"/EP-*; do
  if [ -d "$dir" ]; then
    num=$(basename "$dir" | grep -oE '^EP-[0-9]{4}-[a-z]' | head -1 | grep -oE '[0-9]+' || echo "")
    [ -n "$num" ] && [ "$num" -gt "$LAST" ] && LAST="$num"
  fi
done

# IMPORTANTE: strip ceros a la izquierda (10#) antes de la aritmética,
# si no bash falla con "valor demasiado grande para la base".
NEXT=$((10#$LAST + 1))
EP_ID=$(printf "EP-%04d" "$NEXT")
EP_DIR="$CHANGE_DIR/$EP_ID-$SLUG"

# 3. Verificar que no exista
if [ -d "$EP_DIR" ]; then
  echo "Error: ya existe $EP_ID-$SLUG en $CHANGE_DIR"
  exit 1
fi

# 4. Crear estructura
mkdir -p "$EP_DIR/specs"
TODAY=$(date +%Y-%m-%d)

# 5. Generar proposal.md con placeholders estrictos
cat > "$EP_DIR/proposal.md" <<EOF
# $EP_ID — <título descriptivo>

> **Status**: draft:proposal
> **Created**: $TODAY
> **Updated**: $TODAY
> **Owner**: <@nombre>
> **Reviewers**: [@user1, @user2]

## Problema

[REQUIRED: 1-3 párrafos. Qué problema resolvemos y por qué importa.]

## Solución propuesta

[REQUIRED: Alto nivel. Cómo lo resolvemos.]

## Alternativas consideradas

[OPTIONAL: Lista de alternativas rechazadas y por qué.]

## Criterios de aceptación

[REQUIRED: Lista de condiciones que marcan esta épica como done.]

## Riesgos

[OPTIONAL: Lista de riesgos conocidos + mitigaciones.]
EOF

# 6. Generar design.md
cat > "$EP_DIR/design.md" <<EOF
# $EP_ID — Design

> **Status**: draft:design
> **Created**: $TODAY

## Decisiones técnicas

[REQUIRED: Lista de decisiones con trade-offs. Una por línea.]

## Contratos

[REQUIRED: APIs, schemas, eventos afectados.]

## Dependencias

[OPTIONAL: Otras épicas, librerías, infra.]
EOF

# 7. Generar tasks.md
cat > "$EP_DIR/tasks.md" <<EOF
# $EP_ID — Tasks

> **Status**: draft:tasks
> **Created**: $TODAY

## Plan por fases

### Fase 1 — Setup
- [ ] <task 1>

### Fase 2 — Implementación
- [ ] <task 2>

### Fase 3 — Tests
- [ ] <task 3>

### Fase 4 — Docs
- [ ] <task 4>

## Dependencias cruzadas

[OPTIONAL: Otras épicas o sistemas externos.]
EOF

# 8. Generar template de spec
mkdir -p "$EP_DIR/specs/$EP_ID-01-<repo>"
cat > "$EP_DIR/specs/$EP_ID-01-<repo>/requirements.md" <<EOF
# $EP_ID-01 — Spec para <repo>

> **Status**: draft:spec
> **Created**: $TODAY
> **Repo**: <repo>

## Contexto

Qué parte del codebase afecta esta spec.

## Requisitos

### R-001 — <título>

**Como** <rol>  
**Quiero** <acción>  
**Para** <beneficio>

**Criterios de aceptación**:
- [ ] <criterio 1>
- [ ] <criterio 2>

## Out of scope

[OPTIONAL: Qué NO entra en esta spec.]
EOF

# 9. Mensaje final
echo "✓ Épica creada: $EP_ID-$SLUG"
echo ""
echo "Archivos:"
ls -1 "$EP_DIR"
echo ""
echo "Próximos pasos:"
echo "  1. Editar $EP_DIR/proposal.md (rellenar placeholders [REQUIRED])"
echo "  2. Cuando esté aprobado, mergear PR"
echo "  3. Cuando esté implementado, corré: bash bin/close-epic.sh $EP_ID"
