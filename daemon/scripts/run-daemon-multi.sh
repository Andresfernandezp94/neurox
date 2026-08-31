#!/usr/bin/env bash
# Arranque del daemon con dos in-process agents paralelos (admin + user).
# Cada uno corre como su propio proceso `agent` con --id y --identity-dir
# separados. Hoy Phase 1 los expone pero el daemon sigue ruteando todo a
# un solo `agent`; Phase 2 va a spawnear N `agent` segun `in_process`
# y rutear por agent_id.

set -euo pipefail

WORKDIR="$(cd "$(dirname "$0")/.." && pwd)"
cd "$WORKDIR"

export MINIMAX_API_KEY="${MINIMAX_API_KEY:-test-key}"
export MINIMAX_BASE_URL="${MINIMAX_BASE_URL:-https://api.minimaxi.chat/v1}"
export MINIMAX_MODEL="${MINIMAX_MODEL:-MiniMax-M3}"

# Dos identity dirs separados (uno por agente).
ADMIN_IDENTITY_DIR="${ADMIN_IDENTITY_DIR:-$HOME/.local/share/neurox/identity/}"
USER_IDENTITY_DIR="${USER_IDENTITY_DIR:-$HOME/.local/share/neurox-legacy/}"

cat > /tmp/neurox-multi.yaml <<EOF
bind_addr: "127.0.0.1:7878"
log_level: "info"

# EP-2026-08-15: lista de agentes in-process. Cada entry es una
# instancia del daemon con --id y --identity-dir distintos.
in_process:
  - id: admin
    identity_dir: ${ADMIN_IDENTITY_DIR}
  - id: user
    identity_dir: ${USER_IDENTITY_DIR}
EOF

echo "[run-daemon-multi] starting daemon + 2 agent processes..."
echo "[run-daemon-multi] admin -> ${ADMIN_IDENTITY_DIR}"
echo "[run-daemon-multi] user  -> ${USER_IDENTITY_DIR}"

exec "$WORKDIR/target/debug/neurox" \
  --config /tmp/neurox-multi.yaml \
  serve
