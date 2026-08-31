#!/usr/bin/env bash
# security/bin/validate-finding.sh
# Valida que un finding tenga todos los campos REQUIRED del template.
# Uso: bash security/bin/validate-finding.sh <finding-file.md>
# Retorna exit 0 si válido, exit 1 si falta algo.

set -euo pipefail

if [ $# -ne 1 ]; then
  echo "ERROR: uso: bash security/bin/validate-finding.sh <finding-file.md>" >&2
  exit 1
fi

FILE="$1"

if [ ! -f "$FILE" ]; then
  echo "ERROR: archivo no existe: $FILE" >&2
  exit 1
fi

# --- Cargar valores del finding ---
get_value() {
  local pattern="$1"
  grep -E "$pattern" "$FILE" | head -1 | sed 's/^[^:]*:[[:space:]]*//'
}

ID=$(get_value '^- ID:')
SEVERITY=$(get_value '^- Severity:')
CVSS=$(get_value '^- CVSS-equivalent:')
STRIDE=$(get_value '^- STRIDE:')
DISCOVERED=$(get_value '^- Discovered:')
DISCOVERED_BY=$(get_value '^- Discovered by:')
STATUS=$(get_value '^- Status:')
FILE_PATH=$(get_value '^- File:')
LINE=$(get_value '^- Line:')
COMPONENT=$(get_value '^- Component:')
DESCRIPTION=$(awk '/^## Description/,/^## /' "$FILE" | sed '1d;$d' | tr -d '[:space:]')
IMPACT=$(awk '/^## Impact/,/^## /' "$FILE" | sed '1d;$d' | tr -d '[:space:]')
POC=$(awk '/^## Proof of Concept/,/^## /' "$FILE" | sed '1d;$d' | tr -d '[:space:]')
REMEDIATION=$(awk '/^## Remediation/,/^## /' "$FILE" | sed '1d;$d' | tr -d '[:space:]')
REGRESSION=$(awk '/^## Regression Test/,/^## /' "$FILE" | sed '1d;$d' | tr -d '[:space:]')

ERRORS=()

# --- Validar ID ---
if ! [[ "$ID" =~ ^SEC-[0-9]{4}-[0-9]{4}-[0-9]{3}$ ]]; then
  ERRORS+=("ID inválido o vacío: '$ID' (formato esperado: SEC-NNNN-YYYY-NNN)")
fi

# --- Validar Severity ---
case "$SEVERITY" in
  Critical|High|Medium|Low) ;;
  *) ERRORS+=("Severity inválido: '$SEVERITY' (válidos: Critical|High|Medium|Low)") ;;
esac

# --- Validar CVSS ---
if ! [[ "$CVSS" =~ ^[0-9]+\.[0-9]+$ ]] || (( $(echo "$CVSS < 0.0" | bc -l 2>/dev/null || echo 0) )) || (( $(echo "$CVSS > 10.0" | bc -l 2>/dev/null || echo 0) )); then
  ERRORS+=("CVSS inválido: '$CVSS' (esperado: 0.0-10.0)")
fi

# --- Validar STRIDE ---
for letter in $(echo "$STRIDE" | tr ',' ' '); do
  case "$letter" in
    S|T|R|I|D|E) ;;
    *) ERRORS+=("STRIDE contiene letra inválida: '$letter' (válidas: S|T|R|I|D|E)") ;;
  esac
done

# --- Validar Date ---
if ! [[ "$DISCOVERED" =~ ^[0-9]{4}-[0-9]{2}-[0-9]{2}$ ]]; then
  ERRORS+=("Discovered inválido: '$DISCOVERED' (formato esperado: YYYY-MM-DD)")
fi

# --- Validar campos no vacíos ---
[ -z "$DISCOVERED_BY" ] && ERRORS+=("Discovered by vacío")
[ -z "$FILE_PATH" ] && ERRORS+=("File vacío")
[ -z "$LINE" ] && ERRORS+=("Line vacío")
[ -z "$COMPONENT" ] && ERRORS+=("Component vacío")
[ -z "$DESCRIPTION" ] && ERRORS+=("Description vacío (necesita contenido)")
[ -z "$IMPACT" ] && ERRORS+=("Impact vacío (necesita contenido)")
[ -z "$POC" ] && ERRORS+=("Proof of Concept vacío (necesita comando ejecutable)")
[ -z "$REMEDIATION" ] && ERRORS+=("Remediation vacío (necesita contenido)")
[ -z "$REGRESSION" ] && ERRORS+=("Regression Test vacío (necesita cómo verificar el fix)")

# --- Validar que el PoC tenga un code block bash ---
if ! grep -q '^```bash' "$FILE"; then
  ERRORS+=("Proof of Concept no tiene code block bash")
fi

# --- Output ---
if [ ${#ERRORS[@]} -eq 0 ]; then
  echo "✅ Finding válido: $FILE"
  echo "   ID: $ID | Severity: $SEVERITY | CVSS: $CVSS | STRIDE: $STRIDE"
  exit 0
else
  echo "❌ Finding INVÁLIDO: $FILE" >&2
  echo "" >&2
  echo "Errores encontrados:" >&2
  for err in "${ERRORS[@]}"; do
    echo "  - $err" >&2
  done
  echo "" >&2
  echo "Ver el template: security/templates/finding.template.md" >&2
  exit 1
fi
