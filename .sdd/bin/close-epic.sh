#!/usr/bin/env bash
# close-epic.sh — Cerrar una épica moviéndola de changes/ a archived/.
# Uso: bash bin/close-epic.sh <EP-NNNN>
# Exit 0 = éxito, exit 1 = error.
#
# Comportamiento atómico (R-004 de EP-0007):
#   1. Verifica que EP-NNNN existe en changes/
#   2. Verifica que tiene al menos proposal.md (no se cierra una EP vacía)
#   3. Mueve el directorio changes/EP-NNNN-* a archived/EP-NNNN-*
#   4. Actualiza REGISTRO.md agregando una fila
#   5. Actualiza STATE.md:
#      - Remueve EP-NNNN de la tabla "Épicas activas"
#      - Actualiza "Última épica numerada" (max ID en archived/)
#      - Actualiza "Próxima libre" (max + 1)
#   6. Si STATE.md no existe, falla con error claro (no abort silencioso)

set -euo pipefail

if [ $# -ne 1 ]; then
  echo "Uso: bash bin/close-epic.sh <EP-NNNN>"
  echo "  ej: bash bin/close-epic.sh EP-0007"
  exit 1
fi

EP_ID="$1"
SDD="$(cd "$(dirname "$0")/.." && pwd)"
CHANGE_DIR="$SDD/changes"
ARCHIVE_DIR="$SDD/archived"
REGISTRO="$ARCHIVE_DIR/REGISTRO.md"
STATE_FILE="$SDD/STATE.md"

# 1. Validar formato del ID
if ! [[ "$EP_ID" =~ ^EP-[0-9]+$ ]]; then
  echo "Error: id debe tener formato EP-NNNN (ej. EP-0007). Recibido: '$EP_ID'"
  exit 1
fi

# 2. Encontrar el directorio de la EP en changes/
EP_SRC=$(find "$CHANGE_DIR" -maxdepth 1 -type d -name "${EP_ID}-*" | head -1)
if [ -z "$EP_SRC" ]; then
  echo "Error: no se encontró $EP_ID-* en $CHANGE_DIR"
  echo "  (¿ya está archivada? Verificá en $ARCHIVE_DIR)"
  exit 1
fi

EP_SLUG=$(basename "$EP_SRC")
EP_BASENAME="${EP_SLUG#${EP_ID}-}"   # ej. "auth-users-roles"

# 3. Verificar que tenga al menos proposal.md
if [ ! -f "$EP_SRC/proposal.md" ]; then
  echo "Error: $EP_SRC/proposal.md no existe. Una EP sin proposal no se puede cerrar."
  exit 1
fi

# 4. Mover a archived/
EP_DST="$ARCHIVE_DIR/$EP_SLUG"
if [ -d "$EP_DST" ]; then
  echo "Error: ya existe $EP_DST (¿corrupto? mover manualmente)"
  exit 1
fi

# Extraer título del proposal.md (primera línea `# ` que NO sea `## `).
# Antes filtraba `!/^# EP-/` pero eso excluía el título mismo (e.g.,
# `# EP-0007 — SOT governance hardening`). El fix es excluir `## ` (h2).
TITLE=$(awk '/^# / && !/^## / {print; exit}' "$EP_SRC/proposal.md" | head -1)
TITLE="${TITLE:-<sin título>}"

# Extraer fecha del frontmatter (Updated: YYYY-MM-DD). Si el proposal
# no tiene la línea `> **Updated**: YYYY-MM-DD` en el frontmatter, caemos
# a la fecha de hoy. Usamos `|| true` en los greps para que `set -euo
# pipefail` no aborte cuando no hay match (antes abortaba con exit 1).
CLOSED_DATE=$( (grep -oE 'Updated\*\*: [0-9]{4}-[0-9]{2}-[0-9]{2}' "$EP_SRC/proposal.md" || true) \
             | (grep -oE '[0-9]{4}-[0-9]{2}-[0-9]{2}' || true) \
             | head -1 )
CLOSED_DATE="${CLOSED_DATE:-$(date +%Y-%m-%d)}"

echo "Cerrando $EP_ID-$EP_BASENAME..."
echo "  Título: $TITLE"
echo "  Fecha: $CLOSED_DATE"

mkdir -p "$ARCHIVE_DIR"
mv "$EP_SRC" "$EP_DST"
echo "  Movido: $EP_SRC → $EP_DST"

# 5. Actualizar REGISTRO.md — agregar fila si no existe
if [ -f "$REGISTRO" ]; then
  # Buscar SOLO en filas de la tabla (`^| EP-NNNN |`) — antes buscaba
  # `$EP_ID` en cualquier parte, lo cual matcheaba refs en blockquotes
  # (e.g., "Próxima libre: `EP-0007`") y skipeaba el insert indebidamente.
  if ! grep -qE "^\| $EP_ID \|" "$REGISTRO"; then
    # Insertar antes de la sección "## Próxima libre" si existe, si no al final
    if grep -q "^## Próxima libre" "$REGISTRO"; then
      # Encontrar la línea de la sección
      SEP_LINE=$(grep -n "^## Próxima libre" "$REGISTRO" | cut -d: -f1)
      # Construir nueva fila: | EP-NNNN | [link](EP-NNNN-slug/) | título | fecha | — |
      NEW_ROW="| $EP_ID | [$EP_ID-$EP_BASENAME]($EP_SLUG/) | $TITLE | $CLOSED_DATE | — |"
      # Insertar antes de la sección (en la sección "## Entradas")
      awk -v row="$NEW_ROW" -v sep="$SEP_LINE" '
        /^## Entradas$/ { print; print row; next }
        { print }
      ' "$REGISTRO" > "$REGISTRO.tmp"
      mv "$REGISTRO.tmp" "$REGISTRO"
      echo "  REGISTRO.md actualizado con nueva fila"
    else
      # Fallback: append al final
      NEW_ROW="| $EP_ID | [$EP_ID-$EP_BASENAME]($EP_SLUG/) | $TITLE | $CLOSED_DATE | — |"
      echo "" >> "$REGISTRO"
      echo "$NEW_ROW" >> "$REGISTRO"
      echo "  REGISTRO.md: fila appendeada al final"
    fi
  else
    echo "  REGISTRO.md ya contenía $EP_ID (no se duplicó)"
  fi
fi

echo ""
echo "✓ $EP_ID-$EP_BASENAME cerrado y archivado."
echo ""

# 6. Actualizar STATE.md (atómico con el move + REGISTRO update)
if [ ! -f "$STATE_FILE" ]; then
  echo "Error: $STATE_FILE no existe. close-epic.sh no aborta silenciosamente"
  echo "  — el SOT requiere STATE.md para mantener la coherencia."
  echo "  Si acabás de crear un SOT nuevo, corré 'touch STATE.md' antes de cerrar"
  echo "  tu primera EP."
  exit 1
fi

# 6a. Remover EP-NNNN de la tabla "Épicas activas" (si está)
if grep -q "| $EP_ID |" "$STATE_FILE"; then
  # Borrar la línea que contiene "| EP-NNNN |" (sed -i /regex/d)
  sed -i "/| $EP_ID |/d" "$STATE_FILE"
  echo "  STATE.md: $EP_ID removida de 'Épicas activas'"
fi

# 6b. Actualizar "Última épica numerada" con el max ID real
# IMPORTANTE: strip ceros a la izquierda (10#) para que bash arithmetic no
# explote con "0008" (los IDs vienen zero-padded a 4 dígitos).
LAST_ID=$(find "$ARCHIVE_DIR" -maxdepth 1 -type d -name 'EP-[0-9]*-*' \
  | sed 's|.*/EP-\([0-9]\{4\}\)-.*|\1|' | sort -n | tail -1)
if [ -z "$LAST_ID" ]; then
  LAST_ID="0"
fi
LAST_ID_DEC=$((10#$LAST_ID))   # strip ceros a la izquierda
# IMPORTANTE: NEXT_ID es SOLO el número (sin "EP-" prefix). El script
# python3 lo agrega. Antes de este fix, NEXT_ID era "EP-0009" y python
# escribía "EP-EP-0009" (doble prefijo). El bug fue introducido cuando
# arreglé el bash arithmetic (T-4) sin notar que NEXT_ID ya tenía "EP-".
NEXT_ID=$(printf "%04d" $((LAST_ID_DEC + 1)))

# 6c. Reemplazar las líneas de "Última" y "Próxima libre" en STATE.md
# El formato actual es: `## Última épica numerada\n\n\`EP-XXXX\` (próxima libre: ...)`
if grep -q "^## Última épica numerada" "$STATE_FILE"; then
  # Reemplazar el bloque que empieza en "## Última épica numerada".
  # El texto actual de STATE.md tiene `EP-XXXX` (con backticks), y el
  # patrón con groups es frágil (conflicto de backticks + EP- + 4 dígitos).
  # Usamos un callback que reescribe los 2 IDs explícitamente.
  # IMPORTANTE: el pattern es un Python raw string (r"..."). Dentro de
  # raw strings NO se escapan meta-chars. Por eso `r"\(" ` es
  # literalmente "\(" (backslash + paren), NO un grupo de captura.
  # El bug que tuve antes era ese. La solución: usar el meta-char
  # directamente, sin backslash: r"(...)" — no r"\(...)".
  python3 - "$LAST_ID" "$NEXT_ID" "$STATE_FILE" <<'PYEOF'
import re, sys
last_id, next_id, state_file = sys.argv[1], sys.argv[2], sys.argv[3]
with open(state_file) as f:
    text = f.read()
def replace_ids(m):
    return f"`EP-{last_id}` (próxima libre: `EP-{next_id}"
new = re.sub(
    r"`EP-\d{4}` \(próxima libre: `EP-\d{4}",
    replace_ids,
    text,
    count=1,
)
if new == text:
    print(f'  ⚠ STATE.md no se modificó (regex no matcheó). Aplicar manualmente:')
    print(f'    `EP-{last_id}` (próxima libre: `EP-{next_id}`...)')
else:
    with open(state_file, 'w') as f:
        f.write(new)
    print(f'  STATE.md: "Última épica numerada" → EP-{last_id} ("Próxima libre" → EP-{next_id})')
PYEOF
else
  echo "  Advertencia: STATE.md no tiene sección '## Última épica numerada' (no se actualizó)"
  echo "  Aplicar manualmente el nuevo last_id=$LAST_ID next_id=$NEXT_ID"
fi

echo ""
echo "Próximos pasos:"
echo "  1. Si hay ADRs asociadas, verificá que decisions/README.md las liste"
echo "  2. Commit: docs(ssd): archive $EP_ID-$EP_BASENAME"
echo "  3. Si querés liberar al daemon, purgá las sesiones idle del spec"
