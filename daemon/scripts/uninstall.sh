#!/usr/bin/env bash
# neurox uninstaller for Linux
set -euo pipefail

PREFIX="${PREFIX:-$HOME/.local}"
SYSTEMD_USER_DIR="${SYSTEMD_USER_DIR:-$HOME/.config/systemd/user}"
CONFIG_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/neurox"
DATA_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/neurox"
BIN_DIR="$PREFIX/bin"

GREEN='\033[0;32m'; YELLOW='\033[1;33m'; NC='\033[0m'
log() { printf "${GREEN}[+]${NC} %s\n" "$*" >&2; }
warn() { printf "${YELLOW}[!]${NC} %s\n" "$*" >&2; }

if [[ "$PREFIX" == /usr* ]] && [[ $EUID -ne 0 ]]; then SUDO="sudo"; else SUDO=""; fi

# Stop service if running
if command -v systemctl >/dev/null; then
  if systemctl --user is-active neurox.service >/dev/null 2>&1; then
    log "stopping neurox.service"
    systemctl --user disable --now neurox.service || true
  fi
fi

# Remove binaries
for bin in neurox agent; do
  if [[ -f "$BIN_DIR/$bin" ]]; then
    log "removing $BIN_DIR/$bin"
    $SUDO rm -f "$BIN_DIR/$bin"
  fi
done

# Remove systemd service
if [[ -f "$SYSTEMD_USER_DIR/neurox.service" ]]; then
  log "removing systemd service"
  rm -f "$SYSTEMD_USER_DIR/neurox.service"
  command -v systemctl >/dev/null && systemctl --user daemon-reload || true
fi

# Preserve user data (config + db) — only remove if --purge
if [[ "${1:-}" == "--purge" ]]; then
  warn "purging config and data"
  rm -rf "$CONFIG_DIR" "$DATA_DIR"
else
  warn "preserving config at $CONFIG_DIR"
  warn "preserving data at $DATA_DIR"
  warn "pass --purge to remove"
fi

log "uninstall complete ✓"
