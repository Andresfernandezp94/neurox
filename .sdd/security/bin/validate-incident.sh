#!/usr/bin/env bash
# security/bin/validate-incident.sh
# Valida que un incident postmortem tenga los campos REQUIRED.
# Uso: bash security/bin/validate-incident.sh <incident.md>

set -euo pipefail

if [ $# -ne 1 ]; then
  echo "ERROR: uso: bash security/bin/validate-incident.sh <incident.md>" >&2
  exit 1
fi

FILE="$1"
[ ! -f "$FILE" ] && { echo "ERROR: no existe $FILE" >&2; exit 1; }

for field in "Metadata" "Summary" "Timeline" "Impact" "Root Cause" "Lessons Learned" "Action Items"; do
  if ! grep -qE "^## ${field}" "$FILE"; then
    echo "ERROR: sección REQUIRED faltante: '## ${field}'" >&2
    exit 1
  fi
done

# Validar que Timeline tenga tabla con timestamp
if ! grep -qE "^\| Timestamp" "$FILE"; then
  echo "ERROR: Timeline no tiene tabla estándar (header esperado: '| Timestamp')" >&2
  exit 1
fi

# Validar que Action Items tenga tabla con Owner
if ! grep -qE "^\| .* \| .* \| Owner" "$FILE"; then
  echo "ERROR: Action Items no tiene tabla estándar con columna Owner" >&2
  exit 1
fi

# Validar que Root Cause tenga 5-whys o equivalente
if ! grep -qE "Why [0-9]" "$FILE" && ! grep -qE "Root cause" "$FILE"; then
  echo "WARN: Root Cause no tiene 5-whys ni sección 'Root cause'" >&2
fi

echo "✅ Incident postmortem válido: $FILE"
exit 0
