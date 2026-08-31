#!/usr/bin/env bash
# neurox installer for Linux
# Usage: ./scripts/install.sh [--prefix ~/.local]
set -euo pipefail

PREFIX="${PREFIX:-$HOME/.local}"
SYSTEMD_USER_DIR="${SYSTEMD_USER_DIR:-$HOME/.config/systemd/user}"

# Choose paths that match what the runtime actually reads (XDG conventions).
# User install: ~/.local/bin + ~/.config/neurox + ~/.local/share/neurox
# System install: /usr/bin + /etc/neurox + /usr/share/neurox
# The systemd unit points at %h/.local/bin/neurox, so the user-local
# install must place binaries there for the service to find them.
case "$PREFIX" in
  /usr|/usr/local)
    BIN_DIR="$PREFIX/bin"
    CONFIG_DIR="/etc/neurox"
    DATA_DIR="$PREFIX/share/neurox"
    ;;
  *)
    BIN_DIR="$PREFIX/bin"
    CONFIG_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/neurox"
    DATA_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/neurox"
    ;;
esac

# Colors
RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; NC='\033[0m'

log() { printf "${GREEN}[+]${NC} %s\n" "$*" >&2; }
warn() { printf "${YELLOW}[!]${NC} %s\n" "$*" >&2; }
err() { printf "${RED}[x]${NC} %s\n" "$*" >&2; exit 1; }

# Sanity check binaries
for bin in neurox agent; do
  if [[ ! -x "target/release/$bin" && ! -x "release/$bin" ]]; then
    err "binary not found: $bin (run \`cargo build --release\` first)"
  fi
done

mkdir -p "$BIN_DIR" "$CONFIG_DIR" "$DATA_DIR"

# Install binaries (use sudo if installing to /usr)
if [[ "$PREFIX" == /usr* ]] && [[ $EUID -ne 0 ]]; then
  SUDO="sudo"
else
  SUDO=""
fi

for bin in neurox agent; do
  src="target/release/$bin"
  [[ -f "release/$bin" ]] && src="release/$bin"
  log "installing $bin"
  $SUDO install -Dm755 "$src" "$BIN_DIR/$bin"
done

# Install config example (only if no config exists)
if [[ ! -f "$CONFIG_DIR/config.yaml" ]]; then
  log "installing example config"
  install -Dm644 examples/agents.yaml "$CONFIG_DIR/config.yaml"
else
  warn "config exists at $CONFIG_DIR/config.yaml — not overwriting"
fi

# Install systemd user service (only if systemd available)
if command -v systemctl >/dev/null && [[ -d "$HOME/.config/systemd/user" || -w "$HOME/.config/systemd/user" ]]; then
  mkdir -p "$SYSTEMD_USER_DIR"
  log "installing systemd user service"
  install -Dm644 systemd/neurox.service "$SYSTEMD_USER_DIR/neurox.service"
  log "to enable: systemctl --user enable --now neurox.service"
else
  warn "systemctl not available — skipping systemd service install"
fi

log "install complete ✓"
log "run: neurox serve"
log "Admin UI: install the 'gui' plugin via 'neurox plugin install gui' (see EP-0023)"
