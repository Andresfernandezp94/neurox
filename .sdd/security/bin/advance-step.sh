#!/usr/bin/env bash
# security/bin/advance-step.sh
# Marca el paso actual como done y avanza al siguiente.
# Actualiza el MANIFEST.
#
# Uso: bash security/bin/advance-step.sh

set -euo pipefail

SEC_DIR="$(cd "$(dirname "$0")/.." && pwd)"
MANIFEST="${SEC_DIR}/MANIFEST.md"

get_state() {
  local key="$1"
  grep -E "^${key}:" "$MANIFEST" | head -1 | sed "s/^${key}:[[:space:]]*//" | tr -d ' '
}

MODE=$(get_state mode)
PROCESS=$(get_state process)
CURRENT_STEP=$(get_state current_step)
TOTAL_STEPS=$(get_state total_steps)

if [ -z "$MODE" ] || [ "$MODE" = "null" ]; then
  echo "ERROR: no hay proceso activo" >&2
  exit 1
fi

# --- Validar antes de avanzar ---
bash "${SEC_DIR}/bin/validate-step.sh" || {
  echo "ERROR: validate-step.sh falló. Corregí los issues antes de avanzar." >&2
  exit 1
}

# --- Marcar paso como done ---
STATE_DIR="${SEC_DIR}/.state/${PROCESS}"
touch "${STATE_DIR}/step-$(printf '%02d' "$CURRENT_STEP")-done"

# --- Actualizar current_step en MANIFEST ---
NEW_STEP=$((CURRENT_STEP + 1))
TIMESTAMP=$(date -u +"%Y-%m-%dT%H:%M:%SZ")

# Reemplazar la línea current_step y last_validated
sed -i.bak \
  -e "s/^current_step:.*/current_step: $NEW_STEP/" \
  -e "s/^last_validated:.*/last_validated: $TIMESTAMP/" \
  "$MANIFEST"

rm -f "$MANIFEST.bak"

# --- Output ---
echo "OK. Paso $CURRENT_STEP marcado como done."
echo ""
if [ "$NEW_STEP" -gt "$TOTAL_STEPS" ]; then
  echo "Proceso completo. Para cerrar:"
  echo "  bash security/bin/close.sh"
else
  echo "Próximo paso (${NEW_STEP}/${TOTAL_STEPS}):"
  echo "  bash security/bin/next-step.sh"
fi
