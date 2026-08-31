#!/usr/bin/env bash
# list-status.sh — Listar todas las épicas con status.
# Uso: bash bin/list-status.sh
# Exit 0 = siempre.

set -euo pipefail

SDD="$(cd "$(dirname "$0")/.." && pwd)"

echo "=== ÉPICAS ACTIVAS ==="
found_active=0
if [ -d "$SDD/changes" ]; then
  for dir in "$SDD/changes"/EP-*; do
    [ -d "$dir" ] || continue
    found_active=1
    ep=$(basename "$dir")
    # Buscar SOLO la línea de frontmatter `> **Status**: ...` (no `## Status`
    # H2 u otras ocurrencias) para evitar output tipo "## Status".
    status=$(grep -m 1 -E '^>\s*\*\*Status\*\*:\s*.+' "$dir/proposal.md" 2>/dev/null \
             | sed -E 's/^>\s*\*\*Status\*\*:\s*//' \
             || echo "unknown")
    echo "  $ep | $status"
  done
fi
[ "$found_active" -eq 0 ] && echo "  (ninguna)"

echo ""
echo "=== ÉPICAS ARCHIVADAS ==="
total_archived=0
if [ -d "$SDD/archived" ]; then
  for dir in "$SDD/archived"/EP-*; do
    [ -d "$dir" ] || continue
    ep=$(basename "$dir")
    # Buscar SOLO la línea de frontmatter `> **Status**: ...` (no `## Status`
    # H2 u otras ocurrencias) para evitar output tipo "## Status".
    status=$(grep -m 1 -E '^>\s*\*\*Status\*\*:\s*.+' "$dir/proposal.md" 2>/dev/null \
             | sed -E 's/^>\s*\*\*Status\*\*:\s*//' \
             || echo "unknown")
    total_archived=$((total_archived + 1))
    echo "  $ep | $status"
  done
fi
echo "  Total: $total_archived"

echo ""
echo "=== SEGURIDAD ==="
if [ -f "$SDD/security/MANIFEST.md" ]; then
  mode=$(grep -m1 "^mode:" "$SDD/security/MANIFEST.md" | sed 's/mode: //' || echo "null")
  echo "  Proceso activo: $mode"
  if [ "$mode" != "null" ]; then
    step=$(grep -m1 "^current_step:" "$SDD/security/MANIFEST.md" | sed 's/current_step: //' || echo "0")
    total=$(grep -m1 "^total_steps:" "$SDD/security/MANIFEST.md" | sed 's/total_steps: //' || echo "0")
    echo "  Step: $step / $total"
  fi
else
  echo "  (security/MANIFEST.md no encontrado)"
fi
