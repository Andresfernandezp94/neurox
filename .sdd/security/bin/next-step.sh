#!/usr/bin/env bash
# security/bin/next-step.sh
# Lee el MANIFEST, identifica el modo y paso actual, e imprime qué hacer.
#
# Uso: bash security/bin/next-step.sh

set -euo pipefail

SEC_DIR="$(cd "$(dirname "$0")/.." && pwd)"
MANIFEST="${SEC_DIR}/MANIFEST.md"

# --- Leer estado del MANIFEST ---
get_state() {
  local key="$1"
  grep -E "^${key}:" "$MANIFEST" | head -1 | sed "s/^${key}:[[:space:]]*//" | tr -d ' '
}

MODE=$(get_state mode)
PROCESS=$(get_state process)
CURRENT_STEP=$(get_state current_step)
TOTAL_STEPS=$(get_state total_steps)

# --- Si no hay proceso activo ---
if [ -z "$MODE" ] || [ "$MODE" = "null" ]; then
  cat <<EOF

No hay proceso de seguridad activo.

Para iniciar uno:
  bash security/bin/start.sh audit "<scope-name>"           # auditoría completa
  bash security/bin/start.sh threat-model "<feature-name>"  # threat model
  bash security/bin/start.sh incident "<incident-name>"     # incidente
  bash security/bin/start.sh disclosure "<vuln-id>"          # reporte externo

EOF
  exit 0
fi

# --- Determinar archivo del paso actual ---
STEP_FILE="${SEC_DIR}/modes/${MODE}.md"

# Si MODE es null/vacío, mostrar mensaje de "no hay proceso"
if [ -z "$MODE" ] || [ "$MODE" = "null" ] || [ ! -f "$STEP_FILE" ]; then
  cat <<EOF

No hay proceso de seguridad activo (o el modo '$MODE' no es válido).

Para iniciar uno:
  bash security/bin/start.sh audit "<scope-name>"           # auditoría completa
  bash security/bin/start.sh threat-model "<feature-name>"  # threat model
  bash security/bin/start.sh incident "<incident-name>"     # incidente
  bash security/bin/start.sh disclosure "<vuln-id>"          # reporte externo

EOF
  exit 0
fi

# --- Si ya terminó ---
if [ "$CURRENT_STEP" -gt "$TOTAL_STEPS" ]; then
  cat <<EOF

Proceso COMPLETO.

Mode: $MODE
Process: $PROCESS
Steps: $CURRENT_STEP / $TOTAL_STEPS

Para cerrar el proceso:
  bash security/bin/close.sh

EOF
  exit 0
fi

# --- Buscar la sección del paso actual en el archivo de modo ---
STEP_HEADER="## Step $CURRENT_STEP"
STEP_SECTION=$(awk -v hdr="$STEP_HEADER" '
  $0 ~ hdr { found=1 }
  found { print }
  found && /^## Step / && NR > 1 && $0 !~ hdr { exit }
' "$STEP_FILE")

if [ -z "$STEP_SECTION" ]; then
  echo "ERROR: no se encontró '## Step $CURRENT_STEP' en $STEP_FILE" >&2
  exit 1
fi

# --- Output ---
cat <<EOF

═══════════════════════════════════════════════════════════
ESTADO: $MODE / $PROCESS
PASO: $CURRENT_STEP / $TOTAL_STEPS
═══════════════════════════════════════════════════════════

$STEP_SECTION

EOF

# --- Mostrar también el archivo del proceso si existe ---
PROCESS_DIR="${SEC_DIR}/archive/${PROCESS}"
if [ -d "$PROCESS_DIR" ]; then
  echo "═══════════════════════════════════════════════════════════"
  echo "ARCHIVOS DEL PROCESO:"
  echo "═══════════════════════════════════════════════════════════"
  ls -la "$PROCESS_DIR" | tail -n +2 | sed 's/^/  /'
fi

cat <<EOF

Para validar el paso actual:
  bash security/bin/validate-step.sh

Para saltar al siguiente paso (después de validar):
  bash security/bin/advance-step.sh

EOF
