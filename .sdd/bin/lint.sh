#!/usr/bin/env bash
# lint.sh — SOT consistency validator (EP-0007 R-010).
# Uso: bash bin/lint.sh [--strict]
# Exit 0 = OK, 1 = error, 2 = warning (only with --strict treat warnings as errors).
#
# Implementa 9 checks sobre .sdd/ del workspace:
#   1. Frontmatter de todas las EPs tiene **Status:** + **ID:** + **Type:**
#   2. Sin refs rotas a directorios viejos (metodologias/, auditorias/, epicas/, fixes/, releases/)
#   3. Sin refs rotas a archivos viejos (constitution.md, gov-structure.md con lowercase)
#   4. Numeración correlativa (último ID + 1 = próximo)
#   5. STATE.md activa == changes/EP-*/
#   6. REGISTRO.md archivadas == archived/EP-*/
#   7. Naming UPPERCASE para governance docs del SOT root
#   8. changes/.gitkeep existe (no se pierde el dir)
#   9. Sin CJK en código del SOT (solo warning, no aborta)
#
# El script imprime el resultado de cada check. Si un check falla, el
# exit code es 1 (error). Si solo warnings, el exit code es 0 a menos
# que se pase --strict.

set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
SDD="$(cd "$SCRIPT_DIR/.." && pwd)"

STRICT=0
[ "${1:-}" = "--strict" ] && STRICT=1

errors=0
warnings=0

# Helper: print error and increment
err() {
  echo "  ✗ $*"
  errors=$((errors + 1))
}

warn() {
  echo "  ⚠ $*"
  warnings=$((warnings + 1))
}

ok() {
  echo "  ✓ $*"
}

echo "=== .sdd/ lint (R-010 de EP-0007) ==="
echo ""

# Check 1: frontmatter compliance
echo "Check 1: frontmatter de EPs archivadas tiene ID + Type..."
ep_count=0
ep_missing=0
for ep_dir in "$SDD"/archived/EP-* "$SDD"/changes/EP-*; do
  [ -d "$ep_dir" ] || continue
  ep_count=$((ep_count + 1))
  ep_name=$(basename "$ep_dir")
  for f in proposal.md design.md tasks.md; do
    [ -f "$ep_dir/$f" ] || continue
    if ! grep -qF "**Status**:" "$ep_dir/$f"; then
      err "$ep_name/$f: missing **Status:**"
    fi
    if ! grep -qF "**ID**:" "$ep_dir/$f"; then
      err "$ep_name/$f: missing **ID:**"
    fi
    if ! grep -qF "**Type**:" "$ep_dir/$f"; then
      err "$ep_name/$f: missing **Type:**"
    fi
    ep_missing=$((ep_missing + 1))
  done
  for f in "$ep_dir"/specs/*/requirements.md; do
    [ -f "$f" ] || continue
    if ! grep -qF "**Status**:" "$f"; then
      err "$(basename $(dirname $(dirname $f)))/$(basename $(dirname $f))/requirements.md: missing **Status:**"
    fi
  done
done
[ "$ep_count" -gt 0 ] && echo "  ($ep_count EPs escaneadas)" || err "no EPs found"
ok "check 1 done"

# Check 2: sin refs rotas a directorios viejos
# Filtra refs que documentan el cambio histórico (legacy/URL/code/ejemplos).
echo ""
echo "Check 2: sin refs rotas a directorios viejos..."
old_dirs=(metodologias auditorias epicas fixes releases misc notes)
for old in "${old_dirs[@]}"; do
  if [ -d "$SDD/$old" ]; then
    err "directorio legacy existe: $old/ (debería haber sido removido en v1.1)"
    continue
  fi
  # Buscar refs activas: excluir líneas con marcadores legacy/URL/ejemplos
  # y placeholders literales (EP-NNNN-titulo, fix-NNN-slug) en plantillas.
  active_lines=$(grep -rEn "/$old/" "$SDD" 2>/dev/null \
    | grep -v "\.git/" \
    | grep -v "/bin/lint.sh" \
    | grep -viE "\(legacy|\(borrado|legacy —|legacy\)|no usado|Removido del plan|migrad|migraci|://|github\.com" \
    | grep -vE 'rg -[lnE] ' \
    | grep -vE 'EP-NNNN-titulo|fix-NNN-slug|EP-NNNN-[a-z]+|<slug>' \
    || true)
  if [ -n "$active_lines" ]; then
    files=$(echo "$active_lines" | cut -d: -f1 | sort -u | head -3 | tr '\n' ' ')
    err "refs activas a $old/ en: $files"
  fi
done
ok "check 2 done"

# Check 3: sin refs rotas a archivos viejos (lowercase).
# Excluye refs dentro de backticks (código inline / ejemplos / instrucciones git mv).
echo ""
echo "Check 3: sin refs rotas a archivos viejos (lowercase)..."
for old in "constitution.md" "gov-structure.md"; do
  active_files=$(python3 -c "
import os, re, sys
target = sys.argv[1]
sdd = sys.argv[2]
hits = []
for root, dirs, fnames in os.walk(sdd):
    if '.git' in root or '/bin' in root:
        continue
    for fn in fnames:
        if not (fn.endswith('.md') or fn.endswith('.sh')):
            continue
        p = os.path.join(root, fn)
        try:
            with open(p, encoding='utf-8') as fh:
                for line in fh:
                    if target in line:
                        # Strip inline code (backticks) and check if ref remains
                        cleaned = re.sub(r'\`[^\`]*\`', '', line)
                        if target in cleaned:
                            hits.append(p)
                            break
        except (UnicodeDecodeError, OSError):
            pass
for h in sorted(set(hits)):
    print(h)
" "$old" "$SDD" 2>/dev/null)
  if [ -n "$active_files" ]; then
    err "refs activas a $old en: $(echo "$active_files" | head -3 | tr '\n' ' ')"
  fi
done
ok "check 3 done"

# Check 4: numeración correlativa
echo ""
echo "Check 4: numeración correlativa (último + 1 = próximo)..."
last_id=0
for d in "$SDD"/changes/EP-* "$SDD"/archived/EP-*; do
  [ -d "$d" ] || continue
  num=$(basename "$d" | grep -oE '^EP-[0-9]{4}' | grep -oE '[0-9]+' || echo "")
  if [ -n "$num" ] && [ "$num" -gt "$last_id" ]; then
    last_id=$num
  fi
done
expected_next=$((10#$last_id + 1))
state_next=$(grep -oE "próxima libre: \`EP-([0-9]+)\`" "$SDD/STATE.md" 2>/dev/null | grep -oE '[0-9]+' || echo "")
if [ -z "$state_next" ]; then
  warn "STATE.md no tiene 'próxima libre: EP-XXXX' (revisar formato)"
elif [ "$state_next" -ne "$expected_next" ]; then
  err "STATE.md dice 'próxima libre: EP-$state_next' pero el siguiente real es EP-$expected_next (numeración rota)"
else
  ok "STATE.md: próxima libre = EP-$expected_next (consistente)"
fi

# Check 5: STATE.md activa == changes/EP-*/
echo ""
echo "Check 5: STATE.md activa == changes/EP-*/..."
state_active=$(grep -A100 "## Épicas activas" "$SDD/STATE.md" | grep -E "^\| EP-[0-9]{4} " | grep -oE 'EP-[0-9]{4}' | sort -u)
changes_active=$(ls -d "$SDD"/changes/EP-* 2>/dev/null | xargs -n1 basename 2>/dev/null | grep -oE 'EP-[0-9]{4}' | sort -u)
if [ -z "$state_active" ] && [ -z "$changes_active" ]; then
  ok "0 EPs activas (consistente)"
else
  for ep in $state_active; do
    if ! echo "$changes_active" | grep -q "$ep"; then
      err "STATE.md lista $ep como activa pero no está en changes/"
    fi
  done
  for ep in $changes_active; do
    if ! echo "$state_active" | grep -q "$ep"; then
      err "$ep está en changes/ pero no en STATE.md activas"
    fi
  done
  [ "$state_active" = "$changes_active" ] && ok "STATE.md y changes/ están en sync"
fi

# Check 6: REGISTRO.md archivadas == archived/EP-*/
echo ""
echo "Check 6: REGISTRO.md archivadas == archived/EP-*/..."
reg_archived=$(grep -oE "^\| EP-[0-9]{4} " "$SDD/archived/REGISTRO.md" 2>/dev/null | grep -oE 'EP-[0-9]{4}' | sort -u)
dir_archived=$(ls -d "$SDD"/archived/EP-* 2>/dev/null | xargs -n1 basename 2>/dev/null | grep -oE 'EP-[0-9]{4}' | sort -u)
if [ "$reg_archived" = "$dir_archived" ]; then
  ok "REGISTRO.md y archived/ están en sync ($(echo "$reg_archived" | wc -l) EPs)"
else
  for ep in $reg_archived; do
    if ! echo "$dir_archived" | grep -q "$ep"; then
      err "REGISTRO.md lista $ep archivada pero no existe folder"
    fi
  done
  for ep in $dir_archived; do
    if ! echo "$reg_archived" | grep -q "$ep"; then
      err "archived/$ep-* existe pero no está en REGISTRO.md"
    fi
  done
fi

# Check 7: naming UPPERCASE para governance docs
echo ""
echo "Check 7: naming UPPERCASE para governance docs del SOT root..."
for gov in "constitution.md" "glossary.md" "gov-structure.md" "index.md"; do
  if [ -f "$SDD/$gov" ] && [ ! -L "$SDD/$gov" ]; then
    err "$gov está en lowercase (v1.3 dice UPPERCASE para governance)"
  fi
done
ok "check 7 done"

# Check 8: changes/.gitkeep existe
echo ""
echo "Check 8: changes/.gitkeep existe..."
if [ -d "$SDD/changes" ]; then
  if [ ! -f "$SDD/changes/.gitkeep" ]; then
    warn "changes/.gitkeep no existe (el dir se perderá cuando esté vacío en otra rama)"
  else
    ok "changes/.gitkeep existe"
  fi
else
  ok "changes/ no existe (no aplica)"
fi

# Check 8b: DOCS-INDEX.md existe (catálogo canónico del SOT, v1.4)
echo ""
echo "Check 8b: DOCS-INDEX.md existe (catálogo canónico v1.4)..."
if [ ! -f "$SDD/DOCS-INDEX.md" ]; then
  err "DOCS-INDEX.md no existe en SOT root (creado en EP-0008 T-15, R-009)"
else
  ok "DOCS-INDEX.md existe"
fi

# Check 10: agent names no canónicos (EP-0009, G13)
# El único agent_id válido es 'default'. Detecta: adan, eva (case-sensitive).
# Excluye: archived/, security/archive/, changes/ (EP documenta el rename),
# research/ y methodologies/ (mención histórica narrativa),
# SOT root docs (definen la regla), lint.sh mismo, build outputs.
echo ""
echo "Check 10: agent names no canónicos (adan|eva) en código/docs..."
CHECK10_HITS=$(python3 -c "
import os, re, sys
sdd = '$SDD'
exclude_paths = {
    'archived',
    'security/archive',
    'changes',
    'research',
    'methodologies',
    'CONSTITUTION.md',
    'INDEXING.md',
    'GLOSSARY.md',
    'GOVERNANCE.md',
    'DOCS-INDEX.md',
    'HANDOVER.md',
    'STATE.md',
    'bin/lint.sh',
    # ADRs históricos con banner pre-rename (EP-0009 A2)
    'decisions/ADR-0001-multi-agent-subprocess-architecture.md',
    'decisions/ADR-0002-streaming-endpoint-routing.md',
    # Decisions cross-repo históricas
    'decisions/README.md',
}
exclude_dirs = {'.git', 'archived', 'dist', 'target', 'node_modules'}
hits = []
for root, dirs, files in os.walk(sdd):
    rel_root = root.replace(sdd + '/', '') if root != sdd else ''
    if any(rel_root == p or rel_root.startswith(p + '/') for p in exclude_paths):
        dirs[:] = []
        continue
    dirs[:] = [d for d in dirs if d not in exclude_dirs]
    for fn in files:
        rel = (rel_root + '/' + fn) if rel_root else fn
        if rel in exclude_paths:
            continue
        if not (fn.endswith('.md') or fn.endswith('.rs') or fn.endswith('.ts') or fn.endswith('.tsx') or fn.endswith('.js') or fn.endswith('.json') or fn.endswith('.yaml') or fn.endswith('.yml') or fn.endswith('.toml') or fn.endswith('.sh')):
            continue
        p = os.path.join(root, fn)
        try:
            with open(p, encoding='utf-8') as f:
                for i, line in enumerate(f, 1):
                    if re.search(r'\baden\b', line) or re.search(r'\beva\b', line):
                        hits.append(f'{rel}:{i}')
        except (UnicodeDecodeError, OSError):
            pass
for h in hits[:20]:
    print(h)
if len(hits) > 20:
    print(f'... and {len(hits) - 20} more')
" 2>/dev/null)
if [ -n "$CHECK10_HITS" ]; then
  err "agent names no canónicos encontrados:\n  $CHECK10_HITS"
else
  ok "check 10 done"
fi

# Check 11: default_agent o neurox_default como agent id (EP-0009, G13)
# Excluye: archived/, security/archive/, changes/, SOT docs que documentan la regla
echo ""
echo "Check 11: default_agent/neurox_default como agent id..."
CHECK11_HITS=$(python3 -c "
import os, re, sys
sdd = '$SDD'
exclude_paths = {
    'archived',
    'security/archive',
    'changes',
    'CONSTITUTION.md',
    'INDEXING.md',
    'GOVERNANCE.md',
    'DOCS-INDEX.md',
}
exclude_dirs = {'.git', 'archived', 'dist', 'target', 'node_modules'}
hits = []
for root, dirs, files in os.walk(sdd):
    rel_root = root.replace(sdd + '/', '') if root != sdd else ''
    if any(rel_root == p or rel_root.startswith(p + '/') for p in exclude_paths):
        dirs[:] = []
        continue
    dirs[:] = [d for d in dirs if d not in exclude_dirs]
    for fn in files:
        rel = (rel_root + '/' + fn) if rel_root else fn
        if rel in exclude_paths:
            continue
        if not (fn.endswith('.md') or fn.endswith('.rs') or fn.endswith('.ts') or fn.endswith('.tsx') or fn.endswith('.js') or fn.endswith('.json') or fn.endswith('.yaml') or fn.endswith('.yml') or fn.endswith('.toml')):
            continue
        p = os.path.join(root, fn)
        try:
            with open(p, encoding='utf-8') as f:
                content = f.read()
            if re.search(r'agent_id[\"\s:]+[\"\']default_agent[\"\']', content):
                hits.append(rel + ': agent_id default_agent')
            if re.search(r'agent_id[\"\s:]+[\"\']neurox_default[\"\']', content):
                hits.append(rel + ': agent_id neurox_default')
        except (UnicodeDecodeError, OSError):
            pass
for h in hits[:10]:
    print(h)
" 2>/dev/null)
if [ -n "$CHECK11_HITS" ]; then
  err "agent_id no canónicos:\n  $CHECK11_HITS"
else
  ok "check 11 done"
fi

# Check 12: MiAgente placeholder (EP-0009)
# Excluye: skills/ (template didáctico), SOT docs que documentan la regla
echo ""
echo "Check 12: MiAgente placeholder..."
CHECK12_HITS=$(python3 -c "
import os, re, sys
sdd = '$SDD'
exclude_paths = {
    'archived',
    'security/archive',
    'changes',
    'CONSTITUTION.md',
    'INDEXING.md',
    'methodologies/method-08-doc-update-mandatory.md',
}
exclude_dirs = {'.git', 'archived', 'dist', 'target', 'node_modules'}
hits = []
for root, dirs, files in os.walk(sdd):
    rel_root = root.replace(sdd + '/', '') if root != sdd else ''
    # Excluir subárboles completos si rel_root empieza con un exclude_paths
    if any(rel_root == p or rel_root.startswith(p + '/') for p in exclude_paths):
        dirs[:] = []
        continue
    dirs[:] = [d for d in dirs if d not in exclude_dirs]
    if '/skills/' in root:
        continue
    for fn in files:
        rel = (rel_root + '/' + fn) if rel_root else fn
        if rel in exclude_paths:
            continue
        if not (fn.endswith('.md') or fn.endswith('.ts') or fn.endswith('.tsx') or fn.endswith('.js') or fn.endswith('.json') or fn.endswith('.yaml')):
            continue
        p = os.path.join(root, fn)
        try:
            with open(p, encoding='utf-8') as f:
                for i, line in enumerate(f, 1):
                    if re.search(r'\bMiAgente\b', line):
                        hits.append(f'{rel}:{i}')
        except (UnicodeDecodeError, OSError):
            pass
for h in hits[:10]:
    print(h)
" 2>/dev/null)
if [ -n "$CHECK12_HITS" ]; then
  err "MiAgente placeholder encontrado:\n  $CHECK12_HITS"
else
  ok "check 12 done"
fi

# Check 9: sin CJK en código del SOT
echo ""
echo "Check 9: sin CJK en código del SOT (warning)..."
cjk_files=$(find "$SDD" -name "*.md" -o -name "*.sh" 2>/dev/null | xargs -I{} python3 -c "
import sys
try:
    with open('{}', 'r', encoding='utf-8') as f:
        for i, line in enumerate(f, 1):
            for c in line:
                if 0x4E00 <= ord(c) <= 0x9FFF or 0x3400 <= ord(c) <= 0x4DBF or 0x3040 <= ord(c) <= 0x309F or 0xAC00 <= ord(c) <= 0xD7AF:
                    print('{}:{}: {}'.format('{}', i, line.strip()[:80]))
                    sys.exit(0)
except (UnicodeDecodeError, FileNotFoundError):
    pass
" 2>/dev/null)
if [ -n "$cjk_files" ]; then
  warn "CJK encontrado en: $cjk_files"
else
  ok "sin CJK en el SOT"
fi

# Resumen
echo ""
echo "=== Resumen ==="
echo "  Errores: $errors"
echo "  Warnings: $warnings"
echo ""

if [ "$errors" -gt 0 ]; then
  echo "✗ LINT FAILED ($errors errores)"
  exit 1
elif [ "$warnings" -gt 0 ] && [ "$STRICT" -eq 1 ]; then
  echo "⚠ LINT OK con warnings (--strict los trata como errors)"
  exit 1
else
  echo "✓ LINT PASSED"
  exit 0
fi