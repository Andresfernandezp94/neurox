#!/usr/bin/env bash
# security/bin/validate-design.sh
# Valida que un audit design tenga los campos REQUIRED.
# Uso: bash security/bin/validate-design.sh <design.md>

set -euo pipefail

if [ $# -ne 1 ]; then
  echo "ERROR: uso: bash security/bin/validate-design.sh <design.md>" >&2
  exit 1
fi

FILE="$1"
[ ! -f "$FILE" ] && { echo "ERROR: no existe $FILE" >&2; exit 1; }

for field in "Audit ID" "Mode" "Step" "Architecture Under Review" "Threat Model" "Tools to Use" "Findings Structure" "Output" "Validation"; do
  if ! grep -qE "^${field}:" "$FILE" && ! grep -qE "^## ${field}" "$FILE"; then
    echo "ERROR: campo/sección REQUIRED faltante: '$field'" >&2
    exit 1
  fi
done

# Verificar que Threat Model tenga al menos una "Data flow" table
if ! grep -qE "^### Data flow" "$FILE"; then
  echo "ERROR: Threat Model no tiene ninguna sección '### Data flow'" >&2
  exit 1
fi

# Verificar que Tools to Use tenga al menos 1 comando concreto (bash block)
if ! grep -qE '^```bash' "$FILE"; then
  echo "ERROR: Tools to Use no tiene comandos concretos (sin code blocks bash)" >&2
  exit 1
fi

echo "✅ Design válido: $FILE"
exit 0
