#!/usr/bin/env bash
# security/bin/validate-step.sh
# Valida que el paso actual esté completo según el template.
# Retorna exit 0 si válido, exit 1 si falta algo.
#
# Uso: bash security/bin/validate-step.sh
# Asume que hay un proceso activo (verificado por next-step.sh).

set -euo pipefail

SEC_DIR="$(cd "$(dirname "$0")/.." && pwd)"
MANIFEST="${SEC_DIR}/MANIFEST.md"

get_state() {
  local key="$1"
  grep -E "^${key}:" "$MANIFEST" | head -1 | sed "s/^${key}:[[:space:]]*//" | tr -d ' '
}

MODE=$(get_state mode)
PROCESS=$(get_state process)
CURRENT_STEP=$(get_state current_step)
TOTAL_STEPS=$(get_state total_steps)

if [ -z "$MODE" ] || [ "$MODE" = "null" ]; then
  echo "ERROR: no hay proceso activo" >&2
  exit 1
fi

PROCESS_DIR="${SEC_DIR}/archive/${PROCESS}"

# --- Mapear step → archivo esperado ---
# Esto depende del modo. Por ahora hardcoded por modo.
validate_audit() {
  local step="$1"
  case "$step" in
    1) [ -f "$PROCESS_DIR/01-proposal.md" ] && \
       bash "${SEC_DIR}/bin/validate-proposal.sh" "$PROCESS_DIR/01-proposal.md" ;;
    2) [ -f "$PROCESS_DIR/02-design.md" ] && \
       bash "${SEC_DIR}/bin/validate-design.sh" "$PROCESS_DIR/02-design.md" ;;
    3) [ -d "$PROCESS_DIR/findings" ] && \
       [ "$(ls -1 "$PROCESS_DIR/findings" | wc -l)" -gt 0 ] || { echo "No findings yet"; exit 1; } && \
       bash "${SEC_DIR}/bin/validate-all-findings.sh" "$PROCESS_DIR/findings/" ;;
    4) [ -f "$PROCESS_DIR/04-report.md" ] && \
       bash "${SEC_DIR}/bin/validate-report.sh" "$PROCESS_DIR/04-report.md" ;;
    5|6|7) echo "Step $step: manual validation (review by human)"; return 0 ;;
    *) echo "Unknown step $step for audit mode"; exit 1 ;;
  esac
}

validate_threat_model() {
  local step="$1"
  case "$step" in
    1) [ -f "$PROCESS_DIR/01-threat-model.md" ] && \
       bash "${SEC_DIR}/bin/validate-threat-model.sh" "$PROCESS_DIR/01-threat-model.md" ;;
    2|3|4|5) echo "Step $step: manual validation"; return 0 ;;
    *) echo "Unknown step $step"; exit 1 ;;
  esac
}

validate_incident() {
  local step="$1"
  case "$step" in
    1) [ -f "$PROCESS_DIR/01-incident.md" ] && \
       bash "${SEC_DIR}/bin/validate-incident.sh" "$PROCESS_DIR/01-incident.md" ;;
    2|3|4|5|6) echo "Step $step: manual validation"; return 0 ;;
    *) echo "Unknown step $step"; exit 1 ;;
  esac
}

validate_disclosure() {
  local step="$1"
  case "$step" in
    1) [ -f "$PROCESS_DIR/01-finding.md" ] && \
       bash "${SEC_DIR}/bin/validate-finding.sh" "$PROCESS_DIR/01-finding.md" ;;
    2|3|4|5) echo "Step $step: manual validation"; return 0 ;;
    *) echo "Unknown step $step"; exit 1 ;;
  esac
}

# --- Dispatch ---
case "$MODE" in
  audit)         validate_audit "$CURRENT_STEP" ;;
  threat-model)  validate_threat_model "$CURRENT_STEP" ;;
  incident)      validate_incident "$CURRENT_STEP" ;;
  disclosure)    validate_disclosure "$CURRENT_STEP" ;;
esac
