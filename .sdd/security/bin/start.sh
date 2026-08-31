#!/usr/bin/env bash
# security/bin/start.sh
# Inicia un proceso de seguridad (audit, threat-model, incident, disclosure).
# Crea el directorio, inicializa state, actualiza MANIFEST.
#
# Uso: bash security/bin/start.sh <mode> "<scope-name>"
# Ejemplo: bash security/bin/start.sh audit "neurox-runtime"

set -euo pipefail

# --- Validación de input ---
if [ $# -ne 2 ]; then
  echo "ERROR: uso incorrecto" >&2
  echo "  bash security/bin/start.sh <mode> <scope>" >&2
  echo "  modes: audit | threat-model | incident | disclosure" >&2
  echo "  scope: kebab-case, max 50 chars" >&2
  exit 1
fi

MODE="$1"
SCOPE="$2"

# Validar mode
case "$MODE" in
  audit|threat-model|incident|disclosure) ;;
  *)
    echo "ERROR: mode inválido '$MODE'. Válidos: audit | threat-model | incident | disclosure" >&2
    exit 1
    ;;
esac

# Validar scope
if ! [[ "$SCOPE" =~ ^[a-z0-9-]+$ ]]; then
  echo "ERROR: scope debe ser kebab-case (solo a-z, 0-9, -)" >&2
  exit 1
fi
if [ ${#SCOPE} -gt 50 ]; then
  echo "ERROR: scope demasiado largo (max 50 chars)" >&2
  exit 1
fi

# --- Paths ---
SEC_DIR="$(cd "$(dirname "$0")/.." && pwd)"
NEUROX_DIR="$(cd "$SEC_DIR/../.." && pwd)"
DATE=$(date +%Y-%m-%d)
PROCESS_ID="${DATE}-${SCOPE}"
PROCESS_DIR="${SEC_DIR}/archive/${PROCESS_ID}"
STATE_DIR="${SEC_DIR}/.state/${PROCESS_ID}"
MANIFEST="${SEC_DIR}/MANIFEST.md"

# --- Verificar que no hay proceso activo ---
if grep -q "mode: [a-z]" "$MANIFEST" 2>/dev/null; then
  CURRENT_MODE=$(grep -E "^mode:" "$MANIFEST" | head -1 | awk '{print $2}')
  CURRENT_PROC=$(grep -E "^process:" "$MANIFEST" | head -1 | awk '{print $2}')
  if [ "$CURRENT_MODE" != "null" ] && [ -n "$CURRENT_MODE" ]; then
    echo "ERROR: ya hay un proceso activo: $CURRENT_MODE / $CURRENT_PROC" >&2
    echo "  Cerralo primero con: bash security/bin/close.sh" >&2
    echo "  O continuá con: bash security/bin/next-step.sh" >&2
    exit 1
  fi
fi

# --- Verificar que el proceso no existe ya ---
if [ -d "$PROCESS_DIR" ]; then
  echo "ERROR: el proceso '$PROCESS_ID' ya existe en $PROCESS_DIR" >&2
  echo "  Si querés continuarlo, usá: bash security/bin/next-step.sh" >&2
  exit 1
fi

# --- Determinar total_steps según el modo ---
case "$MODE" in
  audit)         TOTAL_STEPS=7 ;;
  threat-model)  TOTAL_STEPS=5 ;;
  incident)      TOTAL_STEPS=6 ;;
  disclosure)    TOTAL_STEPS=5 ;;
esac

# --- Crear directorios ---
mkdir -p "$PROCESS_DIR" "$STATE_DIR"

# --- Copiar templates según el modo ---
case "$MODE" in
  audit)
    cp "$SEC_DIR/templates/audit-proposal.template.md" "$PROCESS_DIR/01-proposal.md"
    cp "$SEC_DIR/templates/audit-design.template.md" "$PROCESS_DIR/02-design.md"
    cp "$SEC_DIR/templates/audit-report.template.md" "$PROCESS_DIR/04-report.md"
    mkdir -p "$PROCESS_DIR/findings"
    ;;
  threat-model)
    cp "$SEC_DIR/templates/threat-model.template.md" "$PROCESS_DIR/01-threat-model.md"
    ;;
  incident)
    cp "$SEC_DIR/templates/incident-postmortem.template.md" "$PROCESS_DIR/01-incident.md"
    ;;
  disclosure)
    cp "$SEC_DIR/templates/finding.template.md" "$PROCESS_DIR/01-finding.md"
    ;;
esac

# --- Actualizar MANIFEST ---
TIMESTAMP=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
cat > "$MANIFEST.tmp" <<EOF
# MANIFEST.md — Security Process State Machine

> **Cualquier agente lee este archivo PRIMERO** para saber si hay un proceso activo.

## Current State

<!-- El script bin/start.sh actualiza este bloque automáticamente. NO editar a mano. -->

\`\`\`yaml
mode: $MODE
process: $PROCESS_ID
started_at: $TIMESTAMP
current_step: 1
total_steps: $TOTAL_STEPS
last_validated: null
\`\`\`
EOF

# Append el resto del MANIFEST (después del bloque yaml)
awk '
  /^## Current State/ { in_state=1; next }
  /^```yaml$/ { in_yaml=1; next }
  /^```$/ && in_yaml { in_yaml=0; next }
  !in_state || !in_yaml { print }
' "$MANIFEST" >> "$MANIFEST.tmp"

mv "$MANIFEST.tmp" "$MANIFEST"

# --- Marcar paso 1 como in-progress (NO done) ---
touch "$STATE_DIR/.started"

# --- Output al usuario ---
cat <<EOF

OK. Proceso '$MODE' iniciado.

Process ID: $PROCESS_ID
Working dir: $PROCESS_DIR
State dir: $STATE_DIR
Total steps: $TOTAL_STEPS
Started: $TIMESTAMP

Archivos creados:
$(ls -1 "$PROCESS_DIR" | sed 's/^/  /')

Próximo paso:
  $ bash security/bin/next-step.sh

EOF
