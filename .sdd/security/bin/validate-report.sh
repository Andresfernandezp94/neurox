#!/usr/bin/env bash
# security/bin/validate-report.sh
# Valida que un audit report tenga los campos REQUIRED.
# Uso: bash security/bin/validate-report.sh <report.md>

set -euo pipefail

if [ $# -ne 1 ]; then
  echo "ERROR: uso: bash security/bin/validate-report.sh <report.md>" >&2
  exit 1
fi

FILE="$1"
[ ! -f "$FILE" ] && { echo "ERROR: no existe $FILE" >&2; exit 1; }

for field in "Metadata" "Executive Summary" "Methodology" "Findings Summary" "Detailed Findings" "Recommendations"; do
  if ! grep -qE "^## ${field}" "$FILE"; then
    echo "ERROR: sección REQUIRED faltante: '## ${field}'" >&2
    exit 1
  fi
done

# Validar que el Findings Summary tenga una tabla con header
if ! grep -qE "^\| ID \| Severity" "$FILE"; then
  echo "ERROR: Findings Summary no tiene tabla estándar (header esperado: '| ID | Severity |')" >&2
  exit 1
fi

# Validar que Detailed Findings tenga al menos un link a un archivo
if ! grep -qE "\]\(findings/F-" "$FILE"; then
  echo "WARN: Detailed Findings no tiene links a archivos findings/F-*.md" >&2
fi

echo "✅ Report válido: $FILE"
exit 0
