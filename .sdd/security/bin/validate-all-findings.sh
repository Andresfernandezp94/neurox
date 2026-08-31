#!/usr/bin/env bash
# security/bin/validate-all-findings.sh
# Valida TODOS los findings en un directorio.
# Uso: bash security/bin/validate-all-findings.sh <findings-dir/>

set -euo pipefail

if [ $# -ne 1 ]; then
  echo "ERROR: uso: bash security/bin/validate-all-findings.sh <findings-dir/>" >&2
  exit 1
fi

DIR="$1"

if [ ! -d "$DIR" ]; then
  echo "ERROR: directorio no existe: $DIR" >&2
  exit 1
fi

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
TOTAL=0
FAILED=0
PASSED=0

for file in "$DIR"/F-*.md; do
  if [ ! -f "$file" ]; then continue; fi
  TOTAL=$((TOTAL + 1))
  if bash "$SCRIPT_DIR/validate-finding.sh" "$file" > /dev/null 2>&1; then
    PASSED=$((PASSED + 1))
    echo "  ✅ $(basename "$file")"
  else
    FAILED=$((FAILED + 1))
    echo "  ❌ $(basename "$file")"
    bash "$SCRIPT_DIR/validate-finding.sh" "$file" 2>&1 | sed 's/^/      /'
  fi
done

echo ""
echo "Total: $TOTAL | Passed: $PASSED | Failed: $FAILED"

if [ "$FAILED" -gt 0 ]; then
  exit 1
fi
exit 0
