#!/usr/bin/env bash
# next-step.sh — Decirle al agente qué hacer a continuación.
# Uso: bash bin/next-step.sh
# Exit 0 = siempre.

set -euo pipefail

SDD="$(cd "$(dirname "$0")/.." && pwd)"
CHANGE_DIR="$SDD/changes"
STATE_FILE="$SDD/STATE.md"

GIT_BASE=$(git -C "$SDD/.." rev-parse --show-toplevel 2>/dev/null || echo "")

# 1. ¿Hay épicas activas?
active=$(ls -d "$CHANGE_DIR"/EP-* 2>/dev/null || true)

if [ -n "$active" ]; then
  echo "Épicas activas:"
  echo ""
  for dir in $active; do
    ep=$(basename "$dir")
    [ -f "$dir/proposal.md" ] || continue
    title=$(grep -m1 "^# " "$dir/proposal.md" | sed 's/^# //')
    status=$(grep -m1 "Status" "$dir/proposal.md" | sed 's/.*> \*\*Status\*\*: //' || echo "unknown")
    echo "  ▸ $ep"
    echo "    Título: $title"
    echo "    Status: $status"
    echo ""
  done
  echo "Próximos pasos:"
  echo "  1. Editá el proposal.md de la épica que querés trabajar"
  echo "  2. Cuando esté mergeado, corré: bash bin/close-epic.sh <EP-ID>"
  exit 0
fi

# 2. No hay épicas activas
echo "No hay épicas activas."
echo ""
echo "Próximas opciones:"
echo "  • Iniciar una nueva:       bash bin/start-epic.sh <slug-descriptivo>"
echo "  • Auditar seguridad:      bash security/bin/start.sh audit \"<scope>\""
echo "  • Threat model de feature: bash security/bin/start.sh threat-model \"<feature>\""
exit 0
