#!/usr/bin/env bash
# bin/summary.sh — One-page summary of the neurox workspace + .sdd/ state.
# Usage: bash bin/summary.sh
# Exit 0 always.

set -eu

SDD="$(cd "$(dirname "$0")/.." && pwd)"
WORKSPACE="$(cd "$SDD/.." && pwd)"
DATE=$(date +%Y-%m-%d)

# --- Helpers ---
# glob_count <pattern> — count entries matching a glob. 0 if no matches.
# Uses -d so the glob matches the entry itself (not its contents).
glob_count() {
  # shellcheck disable=SC2086
  ls -1d $1 2>/dev/null | wc -l | tr -d ' '
}

# ok <path> — print ✓ if exists, ✗ otherwise.
ok() {
  if [ -e "$1" ]; then echo "✓"; else echo "✗"; fi
}

# --- Header ---
echo "═══════════════════════════════════════════════════════════════"
echo "  neurox — workspace summary"
echo "═══════════════════════════════════════════════════════════════"
echo "  Path:    $WORKSPACE"
echo "  SOT:     $SDD"
echo "  Date:    $DATE"
echo ""

# --- Submodules ---
echo "── Submodules ─────────────────────────────────────────────────"
if [ -d "$WORKSPACE/.git" ]; then
  git -C "$WORKSPACE" submodule status 2>/dev/null | \
    awk 'NF>=2 {printf "  %-40s %s\n", $2, $1}'
else
  echo "  (not a git repo)"
fi
echo ""

# --- Git ---
echo "── Git ────────────────────────────────────────────────────────"
if [ -d "$WORKSPACE/.git" ]; then
  BRANCH=$(git -C "$WORKSPACE" rev-parse --abbrev-ref HEAD 2>/dev/null || echo "detached")
  MOD=$(git -C "$WORKSPACE" status --porcelain 2>/dev/null | wc -l | tr -d ' ')
  echo "  Branch:    $BRANCH"
  echo "  Modified:  $MOD file(s)"
else
  echo "  (not a git repo)"
fi
echo ""

# --- SOT state ---
echo "── SOT state ──────────────────────────────────────────────────"
echo "  Active epics:    $(glob_count "$SDD/changes/EP-*")"
echo "  Archived epics:  $(glob_count "$SDD/archived/EP-*")"
echo "  ADRs:            $(glob_count "$SDD/decisions/[0-9]*.md")"
echo "  Templates:       $(glob_count "$SDD/templates/*.md")"
echo "  Methodologies:   $(glob_count "$SDD/methodologies/method-*.md")"
if [ -f "$SDD/security/MANIFEST.md" ]; then
  MODE=$(awk '/^mode:/ {print $2; exit}' "$SDD/security/MANIFEST.md")
  if [ -z "$MODE" ] || [ "$MODE" = "null" ]; then
    SEC="idle"
  else
    STEP=$(awk '/^current_step:/ {print $2; exit}' "$SDD/security/MANIFEST.md")
    TOTAL=$(awk '/^total_steps:/ {print $2; exit}' "$SDD/security/MANIFEST.md")
    PROC=$(awk '/^process:/ {print $2; exit}' "$SDD/security/MANIFEST.md")
    SEC="$MODE · $PROC · step $STEP/$TOTAL"
  fi
else
  SEC="MANIFEST.md missing"
fi
echo "  Security:        $SEC"
echo ""

# --- SOT files ---
echo "── SOT files ──────────────────────────────────────────────────"
for f in CONSTITUTION.md GLOSSARY.md INDEXING.md CONVENTION.md STATE.md README.md; do
  printf "  %s  %s\n" "$(ok "$SDD/$f")" "$f"
done
for d in methodologies security decisions templates research bin; do
  printf "  %s  %s/\n" "$(ok "$SDD/$d")" "$d"
done
echo ""

# --- Reading order ---
echo "── New here? Read in this order ───────────────────────────────"
echo "  1. README.md"
echo "  2. CONSTITUTION.md"
echo "  3. GLOSSARY.md"
echo "  4. methodologies/method-01-workflow.md"
echo "  5. methodologies/method-07-states-and-steps.md"
echo "═══════════════════════════════════════════════════════════════"
