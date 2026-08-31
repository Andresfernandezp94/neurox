#!/usr/bin/env bash
# security/bin/close.sh
# Cierra un proceso: valida que todos los pasos estén done, resetea MANIFEST.

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

# --- Verificar que todos los pasos estén done ---
STATE_DIR="${SEC_DIR}/.state/${PROCESS}"
for i in $(seq 1 "$TOTAL_STEPS"); do
  STEP_FILE="${STATE_DIR}/step-$(printf '%02d' "$i")-done"
  if [ ! -f "$STEP_FILE" ]; then
    echo "ERROR: paso $i no está marcado como done" >&2
    echo "  Archivo esperado: $STEP_FILE" >&2
    echo "  Ejecutá: bash security/bin/advance-step.sh (estando en ese paso)" >&2
    exit 1
  fi
done

# --- Resetear MANIFEST ---
sed -i.bak \
  -e "s/^mode:.*/mode: null/" \
  -e "s/^process:.*/process: null/" \
  -e "s/^started_at:.*/started_at: null/" \
  -e "s/^current_step:.*/current_step: 0/" \
  -e "s/^last_validated:.*/last_validated: null/" \
  "$MANIFEST"

rm -f "$MANIFEST.bak"

# --- Output ---
cat <<EOF

OK. Proceso cerrado.

Process: $PROCESS
Archive: $SEC_DIR/archive/$PROCESS/

Recordatorios:
- Los archivos en archive/ son INMUTABLES (no modificar)
- Si hay findings que requieren acción, escalar a Andrés

EOF
