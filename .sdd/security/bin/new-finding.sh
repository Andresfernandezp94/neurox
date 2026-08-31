#!/usr/bin/env bash
# security/bin/new-finding.sh
# Genera el siguiente ID disponible para un nuevo finding.
# Uso: bash security/bin/new-finding.sh
# Output: SEC-NNNN-YYYY-NNN (ejemplo: SEC-0003-2026-001)
#         SEC-NNNN es el # de archivo (001, 002, ...)
#         YYYY-NNN es año + # global del año

set -euo pipefail

SEC_DIR="$(cd "$(dirname "$0")/.." && pwd)"
ARCHIVE_DIR="${SEC_DIR}/archive"

YEAR=$(date +%Y)
NEXT_GLOBAL=0
# Buscar el max NNN ya usado este año en TODOS los archives
MAX_NNN=0
if [ -d "$ARCHIVE_DIR" ]; then
  while IFS= read -r -d '' file; do
    if grep -qE "^- ID: SEC-[0-9]{4}-${YEAR}-[0-9]{3}" "$file" 2>/dev/null; then
      NNN=$(grep -E "^- ID: SEC-[0-9]{4}-${YEAR}-[0-9]{3}" "$file" | head -1 | sed -E "s/^.*-${YEAR}-([0-9]{3}).*/\1/")
      NNN=$((10#$NNN))
      if [ "$NNN" -gt "$MAX_NNN" ]; then MAX_NNN=$NNN; fi
    fi
  done < <(find "$ARCHIVE_DIR" -name "F-*.md" -print0 2>/dev/null)
fi
NEXT_GLOBAL=$((MAX_NNN + 1))
NEXT_GLOBAL_FMT=$(printf "%03d" "$NEXT_GLOBAL")

# El NNNN (per-file counter) se mantiene en 001 hasta que el usuario lo cambie
# (cada finding nuevo dentro de un mismo archivo debería empezar en 001 y
#  el validador le advertirá si hay duplicados)
NEXT_FILE=1
NEXT_FILE_FMT=$(printf "%03d" "$NEXT_FILE")

echo "SEC-${NEXT_FILE_FMT}-${YEAR}-${NEXT_GLOBAL_FMT}"
