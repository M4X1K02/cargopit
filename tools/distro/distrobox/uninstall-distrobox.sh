#!/bin/bash
# Cargopit Distrobox Uninstaller
# Removes the distrobox-based cargopit installation.

set -euo pipefail

CONTAINER_NAME="cargopit"
LEGACY_CONTAINER_NAME="simracing"
LEGACY_INSTALL_DIR="${HOME}/.local/share/simracing"
DEFAULT_INSTALL_DIR="${CARGOPIT_INSTALL_DIR:-${XDG_DATA_HOME:-$HOME/.local/share}/cargopit}"
BIN_DIR="$HOME/.local/bin"
CONFIG_DIR="$HOME/.config/cargopit"
UDEV_RULE="/etc/udev/rules.d/99-moza-serial.rules"

# Color output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

distrobox_container_exists() {
    local name="$1"
    distrobox list 2>/dev/null | grep -q "$name" \
        || distrobox list --root 2>/dev/null | grep -q "$name"
}

remove_container_if_present() {
    local name="$1"
    if ! distrobox_container_exists "$name"; then
        log_info "Container '$name' not found, skipping"
        return
    fi
    log_info "Removing distrobox container '$name'..."
    distrobox rm --root --force "$name"
    log_success "Container '$name' removed"
}

print_header() {
    echo ""
    echo "╔══════════════════════════════════════════════════════════════════╗"
    echo "║                Cargopit Distrobox Uninstaller                   ║"
    echo "╚══════════════════════════════════════════════════════════════════╝"
    echo ""
}

print_header

log_warn "This will remove:"
echo "  - Distrobox container(s) '$CONTAINER_NAME' and/or '$LEGACY_CONTAINER_NAME' (if present)"
echo "  - Wrapper scripts ($BIN_DIR/start-simd, start-cargopit, test-cargopit, cargopit-tui)"
echo "  - simshmbridge source under $DEFAULT_INSTALL_DIR and/or $LEGACY_INSTALL_DIR (if present)"
echo ""
echo "This will NOT remove:"
echo "  - Configuration files ($CONFIG_DIR)"
echo "  - Convenience scripts (simracing-launch, configure-moza, etc.)"
echo ""

read -rp "Continue with uninstallation? [y/N]: " confirm
if [[ ! "$confirm" =~ ^[Yy]$ ]]; then
    log_info "Uninstallation cancelled"
    exit 0
fi

echo ""

# Remove wrapper scripts
log_info "Removing wrapper scripts..."
rm -f "$BIN_DIR/start-simd"
rm -f "$BIN_DIR/start-cargopit"
rm -f "$BIN_DIR/test-cargopit"
rm -f "$BIN_DIR/cargopit-tui"
log_success "Wrapper scripts removed"

for data_dir in "$DEFAULT_INSTALL_DIR" "$LEGACY_INSTALL_DIR"; do
    if [ -d "$data_dir" ]; then
        log_info "Removing data directory $data_dir..."
        rm -rf "$data_dir"
        log_success "Removed $data_dir"
    fi
done

remove_container_if_present "$CONTAINER_NAME"
remove_container_if_present "$LEGACY_CONTAINER_NAME"

# Optionally remove udev rule
if [ -f "$UDEV_RULE" ]; then
    echo ""
    read -rp "Remove ModemManager udev rule ($UDEV_RULE)? [y/N]: " remove_udev
    if [[ "$remove_udev" =~ ^[Yy]$ ]]; then
        sudo rm -f "$UDEV_RULE"
        sudo udevadm control --reload-rules
        sudo udevadm trigger
        log_success "udev rule removed"
    else
        log_info "Keeping udev rule"
    fi
fi

echo ""
log_success "Uninstallation complete!"
echo ""
log_info "Config files preserved at $CONFIG_DIR"
echo "  To remove those too: rm -rf $CONFIG_DIR"
echo ""
