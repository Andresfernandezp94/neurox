#!/usr/bin/env bash
# security/bin/validate-threat-model.sh
# Valida que un threat model tenga los campos REQUIRED.
# Uso: bash security/bin/validate-threat-model.sh <threat-model.md>

set -euo pipefail

if [ $# -ne 1 ]; then
  echo "ERROR: uso: bash security/bin/validate-threat-model.sh <threat-model.md>" >&2
  exit 1
fi

FILE="$1"
[ ! -f "$FILE" ] && { echo "ERROR: no existe $FILE" >&2; exit 1; }

for field in "Metadata" "Context" "Architecture" "Trust Boundaries" "Assets" "STRIDE Analysis" "Threats Identified"; do
  if ! grep -qE "^## ${field}" "$FILE"; then
    echo "ERROR: sección REQUIRED faltante: '## ${field}'" >&2
    exit 1
  fi
done

# Validar que STRIDE Analysis tenga al menos 1 data flow
if ! grep -qE "^### Data flow" "$FILE"; then
  echo "ERROR: STRIDE Analysis no tiene ninguna sección '### Data flow'" >&2
  exit 1
fi

# Validar que cada Data flow tenga las 6 categorías STRIDE
for flow_header in $(grep -E "^### Data flow" "$FILE"); do
  for category in "S " "T " "R " "I " "D " "E "; do
    # (heurística: que aparezca la letra como valor en la tabla)
    if ! awk "/^### Data flow/,/^### Data flow|^## /" "$FILE" | grep -qE "^\| $category"; then
      echo "WARN: STRIDE Analysis no menciona categoría '$category' en alguna data flow" >&2
    fi
  done
done

echo "✅ Threat Model válido: $FILE"
exit 0
