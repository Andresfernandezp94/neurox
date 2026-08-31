#!/usr/bin/env bash
# security/bin/validate-proposal.sh
# Valida que un audit proposal tenga los campos REQUIRED.
# Uso: bash security/bin/validate-proposal.sh <proposal.md>

set -euo pipefail

if [ $# -ne 1 ]; then
  echo "ERROR: uso: bash security/bin/validate-proposal.sh <proposal.md>" >&2
  exit 1
fi

FILE="$1"
[ ! -f "$FILE" ] && { echo "ERROR: no existe $FILE" >&2; exit 1; }

# Validar campos REQUIRED
for field in "Audit ID" "Started" "Proposed by" "Status" "Scope" "Depth" "Methodology" "Tools" "Success Criteria"; do
  if ! grep -qE "^${field}:" "$FILE"; then
    echo "ERROR: campo REQUIRED faltante: '$field:'" >&2
    exit 1
  fi
done

# Validar que Depth sea L1, L2 o L3
DEPTH=$(grep -E "^- Depth:" "$FILE" | head -1 | sed 's/^[^:]*:[[:space:]]*//')
case "$DEPTH" in
  L1|L2|L3) ;;
  *) echo "ERROR: Depth debe ser L1, L2 o L3 (got: '$DEPTH')" >&2; exit 1 ;;
esac

# Validar que el Audit ID tenga formato correcto
AUDIT_ID=$(grep -E "^- Audit ID:" "$FILE" | head -1 | sed 's/^[^:]*:[[:space:]]*//')
if ! [[ "$AUDIT_ID" =~ ^[0-9]{4}-[0-9]{2}-[0-9]{2}-[a-z0-9-]+$ ]]; then
  echo "ERROR: Audit ID con formato inválido: '$AUDIT_ID'" >&2
  exit 1
fi

echo "✅ Proposal válido: $FILE"
exit 0
