#!/bin/bash
# Cargopit Universal Installer
# Works on: Arch, Debian/Ubuntu, Fedora-based (incl. Nobara), openSUSE
set -euo pipefail

SCRIPT_VERSION="1.1.0"
INSTALL_DIR="${CARGOPIT_INSTALL_DIR:-${XDG_DATA_HOME:-$HOME/.local/share}/cargopit}"
CONFIG_DIR="${XDG_CONFIG_HOME:-$HOME/.config}"
BIN_DIR="${HOME}/.local/bin"
SIMAPI_PREFIX="${SIMAPI_PREFIX:-/usr/local}"
BRIDGE_RELEASE_URL="https://github.com/Spacefreak18/simshmbridge/releases/download/0.1.0/compatbinaries.zip"
CARGOPIT_GITHUB_REPO="M4X1K02/cargopit"
CARGOPIT_GIT_URL="https://github.com/${CARGOPIT_GITHUB_REPO}.git"
CARGOPIT_RAW_MASTER_URL="https://raw.githubusercontent.com/${CARGOPIT_GITHUB_REPO}/master"
CARGOPIT_RELEASES_URL="https://github.com/${CARGOPIT_GITHUB_REPO}/releases"
AUR_INSTALL_UNAVAILABLE_MSG="AUR install is not offered yet; use --from-source"
# Keep in sync with tui/Cargo.toml rust-version. Cargo.lock v4 needs cargo
# 1.78+; ratatui's darling/instability crates need rustc 1.88+.
TUI_MIN_RUSTC_MAJOR=1
TUI_MIN_RUSTC_MINOR=88
RUSTUP_INIT_URL="https://sh.rustup.rs"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

BUILD_BRIDGES=0
SKIP_BRIDGES=0
ALLOW_ROOT=0
DEPS_ONLY=0
DETECT_ONLY=0
FORCE_NATIVE=0
DO_DISTROBOX=0

DISTRO_ID=""
DISTRO_LIKE=""
DISTRO_VERSION=""
DISTRO_VARIANT=""
DISTRO_FAMILY="unknown"
DISTRO_IMMUTABLE=0

SCRIPT_DIR=""
LOCAL_SRC=""
CARGOPIT_SRC=""
SIMD_BIN=""
CARGOPIT_BIN=""
CARGOPIT_TUI_BIN=""

if [ -n "${BASH_SOURCE[0]:-}" ] && [ -f "${BASH_SOURCE[0]}" ]; then
    SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
    if [ -f "$SCRIPT_DIR/CMakeLists.txt" ] && [ -d "$SCRIPT_DIR/src/cargopit" ]; then
        LOCAL_SRC="$SCRIPT_DIR"
    fi
fi

log_info()    { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[SUCCESS]${NC} $1"; }
log_warn()    { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error()   { echo -e "${RED}[ERROR]${NC} $1"; }

print_header() {
    echo ""
    echo "╔══════════════════════════════════════════════════════════════════╗"
    echo "║          Cargopit Universal Installer v${SCRIPT_VERSION}              ║"
    echo "╚══════════════════════════════════════════════════════════════════╝"
    echo ""
}

usage() {
    cat <<EOF
Usage: $(basename "${BASH_SOURCE[0]:-install.sh}") [options]

Options:
  --from-source     Build simapi, simd, and cargopit from source (default)
  --distrobox       Print (and run, if distrobox exists) immutable-distro setup
  --build-bridges   Cross-compile simshmbridge with mingw instead of prebuilts
  --skip-bridges    Do not download or build simshmbridge compatibility EXEs
  --deps-only       Install build dependencies and exit
  --detect-only     Print distro detection results and exit
  --force-native    Ignore immutable-distro detection and install on the host
  --allow-root      Allow running as root (containers / CI)
  -h, --help        Show this help

Environment:
  CARGOPIT_INSTALL_DIR   Install prefix (default: ~/.local/share/cargopit)
  SIMAPI_PREFIX           simapi install prefix (default: /usr/local)
EOF
}

run_root() {
    if [ "$(id -u)" -eq 0 ]; then
        "$@"
    else
        sudo "$@"
    fi
}

have_cmd() {
    command -v "$1" >/dev/null 2>&1
}

prepend_rustup_bin() {
    local cargo_bin="${CARGO_HOME:-$HOME/.cargo}/bin"
    case ":$PATH:" in
        *":$cargo_bin:"*) ;;
        *) export PATH="$cargo_bin:$PATH" ;;
    esac
}

rustc_major_minor() {
    local version
    if ! have_cmd rustc; then
        printf '%s\n' "0.0"
        return 0
    fi
    version="$(rustc --version | awk '{print $2}')"
    printf '%s\n' "${version%.*}"
}

tui_rustc_is_new_enough() {
    local mm major minor
    mm="$(rustc_major_minor)"
    major="${mm%%.*}"
    minor="${mm#*.}"
    if [ "$major" -gt "$TUI_MIN_RUSTC_MAJOR" ]; then
        return 0
    fi
    if [ "$major" -eq "$TUI_MIN_RUSTC_MAJOR" ] && [ "$minor" -ge "$TUI_MIN_RUSTC_MINOR" ]; then
        return 0
    fi
    return 1
}

install_rustup_stable() {
    local cargo_env="${CARGO_HOME:-$HOME/.cargo}/env"
    log_info "Installing rustup (need rustc ${TUI_MIN_RUSTC_MAJOR}.${TUI_MIN_RUSTC_MINOR}+ for cargopit-tui)"
    if ! have_cmd curl; then
        log_error "curl is required to install rustup"
        exit 1
    fi
    export RUSTUP_INIT_SKIP_PATH_CHECK=yes
    curl --proto '=https' --tlsv1.2 -sSf "$RUSTUP_INIT_URL" | sh -s -- -y --profile minimal
    if [ -f "$cargo_env" ]; then
        # shellcheck disable=SC1090
        . "$cargo_env"
    fi
    prepend_rustup_bin
}

ensure_tui_rust_toolchain() {
    prepend_rustup_bin
    if tui_rustc_is_new_enough; then
        log_info "Using $(rustc --version) for cargopit-tui"
        return 0
    fi
    if have_cmd rustup; then
        log_info "Updating rustup stable (need rustc ${TUI_MIN_RUSTC_MAJOR}.${TUI_MIN_RUSTC_MINOR}+)"
        rustup toolchain install stable
        rustup default stable
        prepend_rustup_bin
    else
        install_rustup_stable
    fi
    if ! tui_rustc_is_new_enough; then
        log_error "rustc ${TUI_MIN_RUSTC_MAJOR}.${TUI_MIN_RUSTC_MINOR}+ is required for cargopit-tui"
        exit 1
    fi
    log_success "Using $(rustc --version) for cargopit-tui"
}

ensure_writable_dir() {
    local dir="$1"
    mkdir -p "$dir" 2>/dev/null || true
    if [ ! -d "$dir" ] || [ ! -w "$dir" ]; then
        log_error "Cannot write to $dir"
        ls -ld "$dir" 2>/dev/null || true
        log_info "If a previous sudo run created this path, fix ownership:"
        echo "    sudo chown -R \"\$USER:\$USER\" \"$dir\""
        return 1
    fi
}

read_os_release() {
    DISTRO_ID="unknown"
    DISTRO_LIKE=""
    DISTRO_VERSION=""
    DISTRO_VARIANT=""
    if [ -f /etc/os-release ]; then
        # shellcheck disable=SC1091
        . /etc/os-release
        DISTRO_ID="${ID:-unknown}"
        DISTRO_LIKE="${ID_LIKE:-}"
        DISTRO_VERSION="${VERSION_ID:-}"
        DISTRO_VARIANT="${VARIANT_ID:-}"
    elif [ -f /etc/arch-release ]; then
        DISTRO_ID="arch"
    fi
}

detect_distro() {
    read_os_release

    local id_lc like_lc variant_lc
    id_lc="$(echo "$DISTRO_ID" | tr '[:upper:]' '[:lower:]')"
    like_lc="$(echo "$DISTRO_LIKE" | tr '[:upper:]' '[:lower:]')"
    variant_lc="$(echo "$DISTRO_VARIANT" | tr '[:upper:]' '[:lower:]')"

    DISTRO_IMMUTABLE=0
    if [ -e /run/ostree-booted ] || [ -d /ostree ]; then
        DISTRO_IMMUTABLE=1
    fi
    case "$id_lc" in
        bazzite|silverblue|kinoite|bluefin|aurora|steamos) DISTRO_IMMUTABLE=1 ;;
    esac
    case "$variant_lc" in
        silverblue|kinoite|sericea|bluefin|aurora|bazzite) DISTRO_IMMUTABLE=1 ;;
    esac

    DISTRO_FAMILY="unknown"
    case "$id_lc" in
        arch|manjaro|endeavouros|garuda|cachyos|archcraft|arcolinux)
            DISTRO_FAMILY="arch" ;;
        fedora|nobara|rhel|centos|rocky|almalinux|ol|ultramarine)
            DISTRO_FAMILY="fedora" ;;
        debian|ubuntu|linuxmint|pop|elementary|zorin|neon|kali|raspbian)
            DISTRO_FAMILY="debian" ;;
        opensuse*|suse|sles)
            DISTRO_FAMILY="opensuse" ;;
        bazzite|silverblue|kinoite|bluefin|aurora)
            DISTRO_FAMILY="fedora" ;;
        steamos)
            DISTRO_FAMILY="arch" ;;
    esac

    if [ "$DISTRO_FAMILY" = "unknown" ]; then
        case "$like_lc" in
            *arch*) DISTRO_FAMILY="arch" ;;
            *fedora*|*rhel*) DISTRO_FAMILY="fedora" ;;
            *debian*|*ubuntu*) DISTRO_FAMILY="debian" ;;
            *suse*) DISTRO_FAMILY="opensuse" ;;
        esac
    fi
}

print_detect() {
    echo "DISTRO_ID=$DISTRO_ID"
    echo "DISTRO_LIKE=$DISTRO_LIKE"
    echo "DISTRO_VERSION=$DISTRO_VERSION"
    echo "DISTRO_VARIANT=$DISTRO_VARIANT"
    echo "DISTRO_FAMILY=$DISTRO_FAMILY"
    echo "DISTRO_IMMUTABLE=$DISTRO_IMMUTABLE"
}

manual_dep_hint() {
    cat <<EOF
Required build packages (names vary by distro):
  git cmake gcc make pkg-config
  libuv argtable libserialport libconfig hidapi lua libxdg-basedir libxml2 libpulse
  yder (simd), cargo/rustc ${TUI_MIN_RUSTC_MAJOR}.${TUI_MIN_RUSTC_MINOR}+ (or rustup)
  optional: mingw-w64 (only with --build-bridges), python3 (tests)

Arch:    pacman -S --needed git cmake base-devel libuv argtable libserialport libconfig hidapi lua54 libpulse pkgconf libxdg-basedir libxml2 rust python yder
Fedora:  dnf install git cmake gcc gcc-c++ make libuv-devel argtable-devel libserialport-devel libconfig-devel hidapi-devel lua-devel libxdg-basedir-devel libxml2-devel pulseaudio-libs-devel pkgconf-pkg-config cargo python3
Debian:  apt install build-essential git cmake libuv1-dev libargtable2-dev libserialport-dev libconfig-dev libhidapi-dev liblua5.4-dev libxdg-basedir-dev libxml2-dev libpulse-dev pkg-config cargo python3
EOF
}

print_immutable_help() {
    log_error "Detected immutable distro: $DISTRO_ID (family=$DISTRO_FAMILY)"
    echo ""
    echo "Do not install with the host package manager (rpm-ostree layering is a last resort)."
    echo "Use distrobox so the stack lives in a mutable container that shares \$HOME:"
    echo ""
    echo "    distrobox create --name cargopit --image archlinux:latest"
    echo "    distrobox enter cargopit"
    echo "    curl -fsSL ${CARGOPIT_RAW_MASTER_URL}/install.sh -o install.sh"
    echo "    bash install.sh --from-source"
    echo ""
    echo "Docs: https://spacefreak18.github.io/simapi/"
    echo "Override with --force-native if you really want to install on the host."
}

install_yder_from_source() {
    if pkg-config --exists yder 2>/dev/null; then
        log_info "yder already available"
        return 0
    fi

    log_warn "yder is not in the distro repos; building orcania + yder from source"
    local src="$INSTALL_DIR/src-deps"
    mkdir -p "$src"

    if [ ! -d "$src/orcania" ]; then
        git clone --depth 1 https://github.com/babelouest/orcania.git "$src/orcania"
    fi
    mkdir -p "$src/orcania/build"
    cmake -S "$src/orcania" -B "$src/orcania/build"
    cmake --build "$src/orcania/build" -j"$(nproc)"
    run_root cmake --install "$src/orcania/build"

    if [ ! -d "$src/yder" ]; then
        git clone --depth 1 https://github.com/babelouest/yder.git "$src/yder"
    fi
    mkdir -p "$src/yder/build"
    cmake -S "$src/yder" -B "$src/yder/build" -DWITH_JOURNALD=off
    cmake --build "$src/yder/build" -j"$(nproc)"
    run_root cmake --install "$src/yder/build"
    if [ -d /usr/local/lib64 ]; then
        echo "/usr/local/lib64" | run_root tee /etc/ld.so.conf.d/usr-local-lib64.conf >/dev/null
    fi
    run_root ldconfig 2>/dev/null || true
    export PKG_CONFIG_PATH="/usr/local/lib/pkgconfig:/usr/local/lib64/pkgconfig:${PKG_CONFIG_PATH:-}"
    log_success "yder installed to /usr/local"
}

install_deps_arch() {
    local deps=(
        git cmake make gcc pkgconf python curl unzip rust
        libuv argtable libserialport libconfig hidapi lua54
        libpulse libxdg-basedir libxml2 yder procps-ng
    )
    if [ "$BUILD_BRIDGES" -eq 1 ]; then
        deps+=(mingw-w64-gcc)
    fi
    log_info "Installing Arch packages: ${deps[*]}"
    run_root pacman -Sy --needed --noconfirm "${deps[@]}"
}

install_deps_fedora() {
    local deps=(
        git cmake gcc gcc-c++ make pkgconf-pkg-config python3 curl unzip ca-certificates cargo
        libuv-devel argtable-devel libserialport-devel libconfig-devel
        hidapi-devel lua-devel libxdg-basedir-devel libxml2-devel
        pulseaudio-libs-devel procps-ng-devel
    )
    if [ "$BUILD_BRIDGES" -eq 1 ]; then
        deps+=(mingw64-gcc)
    fi
    log_info "Installing Fedora packages: ${deps[*]}"
    run_root dnf install -y "${deps[@]}"

    if run_root dnf install -y yder-devel; then
        log_success "yder-devel installed from repos"
    else
        install_yder_from_source
    fi
}

install_deps_debian() {
    export DEBIAN_FRONTEND=noninteractive
    log_info "Updating apt package lists..."
    run_root apt-get update

    local deps=(
        build-essential git cmake pkg-config python3 curl unzip ca-certificates cargo
        libuv1-dev libargtable2-dev libserialport-dev libconfig-dev
        libhidapi-dev libxdg-basedir-dev libxml2-dev libpulse-dev
    )
    if [ "$BUILD_BRIDGES" -eq 1 ]; then
        deps+=(mingw-w64)
    fi
    log_info "Installing Debian/Ubuntu packages: ${deps[*]}"
    run_root apt-get install -y "${deps[@]}"

    if ! run_root apt-get install -y libproc2-dev; then
        log_warn "libproc2-dev not available, trying libprocps-dev"
        run_root apt-get install -y libprocps-dev
    fi

    if ! run_root apt-get install -y liblua5.4-dev lua5.4; then
        log_warn "lua 5.4 not available, trying lua 5.3"
        run_root apt-get install -y liblua5.3-dev lua5.3
    fi

    if ! run_root apt-get install -y libyder-dev; then
        install_yder_from_source
    fi
}

install_deps_opensuse() {
    local deps=(
        git cmake gcc gcc-c++ make pkg-config python3 curl unzip cargo
        libuv-devel argtable-devel libserialport-devel libconfig-devel
        hidapi-devel lua-devel libxdg-basedir-devel libxml2-devel
        libpulse-devel procps-devel
    )
    if [ "$BUILD_BRIDGES" -eq 1 ]; then
        deps+=(mingw64-gcc)
    fi
    log_info "Installing openSUSE packages: ${deps[*]}"
    run_root zypper install -y "${deps[@]}"
    if ! run_root zypper install -y libyder-devel; then
        install_yder_from_source
    fi
}

install_dependencies() {
    log_info "Installing dependencies for $DISTRO_ID (family=$DISTRO_FAMILY)..."
    case "$DISTRO_FAMILY" in
        arch) install_deps_arch ;;
        fedora) install_deps_fedora ;;
        debian) install_deps_debian ;;
        opensuse) install_deps_opensuse ;;
        *)
            log_error "Unsupported distribution: $DISTRO_ID (ID_LIKE=$DISTRO_LIKE)"
            manual_dep_hint
            exit 1
            ;;
    esac
    log_success "Dependencies installed"
}

check_requirements() {
    local missing=()
    local cmd
    for cmd in git cmake make gcc; do
        if ! have_cmd "$cmd"; then
            missing+=("$cmd")
        fi
    done
    if [ "${#missing[@]}" -ne 0 ]; then
        log_error "Missing required commands after dependency install: ${missing[*]}"
        exit 1
    fi
}

git_clone_or_update() {
    local url="$1"
    local dest="$2"
    local with_submodules="${3:-0}"

    if [ -d "$dest/.git" ]; then
        log_info "Updating $(basename "$dest")..."
        git -C "$dest" pull --ff-only || log_warn "git pull failed in $dest; using existing tree"
    else
        log_info "Cloning $url"
        git clone "$url" "$dest"
    fi
    if [ "$with_submodules" = "1" ]; then
        git -C "$dest" submodule sync --recursive
        git -C "$dest" submodule update --init --recursive
    fi
}

prepare_sources() {
    mkdir -p "$INSTALL_DIR"
    cd "$INSTALL_DIR"

    if [ -n "$LOCAL_SRC" ]; then
        log_info "Using local cargopit source: $LOCAL_SRC"
        mkdir -p "$INSTALL_DIR/cargopit"
        tar -C "$LOCAL_SRC" --exclude='./build' --exclude='./.git' -cf - . \
            | tar -C "$INSTALL_DIR/cargopit" -xf -
        CARGOPIT_SRC="$INSTALL_DIR/cargopit"
    else
        git_clone_or_update "$CARGOPIT_GIT_URL" "$INSTALL_DIR/cargopit" 1
        CARGOPIT_SRC="$INSTALL_DIR/cargopit"
    fi

    local simapi_submodule="$CARGOPIT_SRC/src/cargopit/simulatorapi/simapi"
    if [ ! -f "$simapi_submodule/simapi/simdata.h" ]; then
        log_error "simapi submodule is missing under $CARGOPIT_SRC"
        log_info "Run: git submodule update --init --recursive"
        exit 1
    fi
    if [ ! -f "$simapi_submodule/simd/CMakeLists.txt" ]; then
        log_error "simapi submodule does not include simd ($simapi_submodule/simd)"
        exit 1
    fi

    # simd must come from the same simapi tree cargopit compiles against so both
    # share one SimData layout for /dev/shm/SIMAPI.DAT (do not clone simapi master).
    log_info "Using pinned simapi submodule for simd"
    rm -rf "$INSTALL_DIR/simapi"
    mkdir -p "$INSTALL_DIR/simapi"
    tar -C "$simapi_submodule" --exclude='./.git' --exclude='./build' --exclude='./simd/build' -cf - . \
        | tar -C "$INSTALL_DIR/simapi" -xf -
    if [ -e "$simapi_submodule/.git" ]; then
        log_info "simapi pin: $(git -C "$simapi_submodule" rev-parse --short HEAD 2>/dev/null || echo unknown)"
    fi
    log_success "Sources ready"
}

build_simapi() {
    log_info "Building simapi..."
    local gcc_major
    gcc_major="$(gcc -dumpfullversion -dumpversion 2>/dev/null | cut -d. -f1 || echo 0)"
    if [ "${gcc_major:-0}" -gt 0 ] && [ "$gcc_major" -lt 13 ]; then
        log_warn "GCC $gcc_major may fail to compile current simapi (C23 typed enums need GCC 13+)."
        log_info "On Debian 12 use the .deb from GitHub Releases, or Ubuntu 24.04 / Debian testing."
    fi
    mkdir -p "$INSTALL_DIR/simapi/build"
    cmake -S "$INSTALL_DIR/simapi" -B "$INSTALL_DIR/simapi/build" -DCMAKE_INSTALL_PREFIX="$SIMAPI_PREFIX"
    cmake --build "$INSTALL_DIR/simapi/build" -j"$(nproc)"
    run_root cmake --install "$INSTALL_DIR/simapi/build"
    run_root ldconfig 2>/dev/null || true

    if [ ! -f "$SIMAPI_PREFIX/include/simdata.h" ]; then
        log_error "simapi headers were not installed to $SIMAPI_PREFIX/include/simdata.h"
        log_info "simd will fail to compile without them"
        exit 1
    fi
    log_success "simapi installed ($SIMAPI_PREFIX/include/simdata.h)"
}

build_simd() {
    log_info "Building simd..."
    rm -rf "$INSTALL_DIR/simapi/simd/build"
    mkdir -p "$INSTALL_DIR/simapi/simd/build"
    cmake -S "$INSTALL_DIR/simapi/simd" -B "$INSTALL_DIR/simapi/simd/build" \
        -DCMAKE_PREFIX_PATH="$SIMAPI_PREFIX" \
        -DCMAKE_INCLUDE_PATH="$SIMAPI_PREFIX/include" \
        -DCMAKE_LIBRARY_PATH="$SIMAPI_PREFIX/lib;$SIMAPI_PREFIX/lib64" \
        -DCMAKE_C_FLAGS="-I$SIMAPI_PREFIX/include" \
        -DCMAKE_EXE_LINKER_FLAGS="-L$SIMAPI_PREFIX/lib -L$SIMAPI_PREFIX/lib64 -Wl,-rpath,$SIMAPI_PREFIX/lib -Wl,-rpath,$SIMAPI_PREFIX/lib64"
    cmake --build "$INSTALL_DIR/simapi/simd/build" -j"$(nproc)"
    SIMD_BIN="$INSTALL_DIR/simapi/simd/build/simd"
    if [ ! -x "$SIMD_BIN" ]; then
        log_error "simd binary was not produced"
        exit 1
    fi
    log_success "simd built"
}

build_cargopit() {
    log_info "Building cargopit..."
    mkdir -p "$CARGOPIT_SRC/build"
    cmake -S "$CARGOPIT_SRC" -B "$CARGOPIT_SRC/build"
    cmake --build "$CARGOPIT_SRC/build" -j"$(nproc)"
    CARGOPIT_BIN="$CARGOPIT_SRC/build/cargopit"
    if [ ! -x "$CARGOPIT_BIN" ]; then
        log_error "cargopit binary was not produced"
        exit 1
    fi
    if [ -x "$CARGOPIT_SRC/build/tui/release/cargopit-tui" ]; then
        CARGOPIT_TUI_BIN="$CARGOPIT_SRC/build/tui/release/cargopit-tui"
    elif [ -x "$CARGOPIT_SRC/build/tui/debug/cargopit-tui" ]; then
        CARGOPIT_TUI_BIN="$CARGOPIT_SRC/build/tui/debug/cargopit-tui"
    else
        log_error "cargopit-tui binary was not produced (install cargo/rustc)"
        exit 1
    fi
    log_success "cargopit built"
}

install_prebuilt_bridges() {
    log_info "Downloading prebuilt simshmbridge compatibility binaries..."
    if ! have_cmd unzip; then
        log_warn "unzip not found; skip bridge download (install unzip or pass --build-bridges)"
        return 0
    fi
    mkdir -p "$INSTALL_DIR/simshmbridge/assets"
    local zip="$INSTALL_DIR/compatbinaries.zip"
    if curl -fsSL -o "$zip" "$BRIDGE_RELEASE_URL"; then
        unzip -o "$zip" -d "$INSTALL_DIR/simshmbridge/assets"
        log_success "Bridge EXEs extracted to $INSTALL_DIR/simshmbridge/assets"
        echo "    Set Steam launch option, for example:"
        echo "    SIMD_BRIDGE_EXE=$INSTALL_DIR/simshmbridge/assets/acbridge.exe %command%"
        echo "    See: https://spacefreak18.github.io/simapi/simd_usage"
    else
        log_warn "Could not download $BRIDGE_RELEASE_URL"
        log_info "UDP-only titles still work. Get EXEs from https://github.com/spacefreak18/simshmbridge/releases"
    fi
}

build_simshmbridge_from_source() {
    log_info "Building simshmbridge from source (mingw)..."
    git_clone_or_update https://github.com/spacefreak18/simshmbridge.git "$INSTALL_DIR/simshmbridge" 1
    make -C "$INSTALL_DIR/simshmbridge" clean || true
    make -C "$INSTALL_DIR/simshmbridge" -j"$(nproc)"
    log_success "simshmbridge built"
}

install_bridges() {
    if [ "$SKIP_BRIDGES" -eq 1 ]; then
        log_info "Skipping simshmbridge (--skip-bridges)"
        return 0
    fi
    if [ "$BUILD_BRIDGES" -eq 1 ]; then
        build_simshmbridge_from_source
    else
        install_prebuilt_bridges
    fi
}

setup_configs() {
    log_info "Setting up configuration files..."
    mkdir -p "$CONFIG_DIR/simd" "$CONFIG_DIR/cargopit"

    if [ ! -f "$CONFIG_DIR/simd/simd.config" ]; then
        if [ -f "$INSTALL_DIR/simapi/simd/conf/simd.config" ]; then
            cp "$INSTALL_DIR/simapi/simd/conf/simd.config" "$CONFIG_DIR/simd/simd.config"
            log_success "Created $CONFIG_DIR/simd/simd.config"
        else
            log_warn "simd example config not found; create $CONFIG_DIR/simd/simd.config from simapi docs"
        fi
    else
        log_info "simd config already exists, skipping"
    fi

    local example_src=""
    if [ -f "$CARGOPIT_SRC/conf/cargopit.config" ]; then
        example_src="$CARGOPIT_SRC/conf/cargopit.config"
    elif [ -f "$INSTALL_DIR/cargopit/conf/cargopit.config" ]; then
        example_src="$INSTALL_DIR/cargopit/conf/cargopit.config"
    fi
    if [ -n "$example_src" ]; then
        cp "$example_src" "$CONFIG_DIR/cargopit/cargopit.config.example"
    fi

    if [ ! -f "$CONFIG_DIR/cargopit/cargopit.config" ]; then
        cat > "$CONFIG_DIR/cargopit/cargopit.config" << 'EOF'
// Starter config — add only devices you actually have.
// Full examples: ~/.config/cargopit/cargopit.config.example
// Device docs: https://spacefreak18.github.io/simapi/
configs = (
    {
        sim = "default";
        car = "default";
        devices = (
        // Serial wheel / Arduino example
        /*
        {
            device       = "Serial";
            type         = "Wheel";
            subtype      = "MozaR5";
            baud         = 115200;
            devpath      = "/dev/ttyACM0";
        },
        */
        // Bass shaker (use `pactl list sinks` for devid)
        /*
        {
            device       = "Sound";
            effect       = "Engine";
            devid        = "alsa_output.your_device_here";
            pan          = 0;
            fps          = 60;
            threshold    = 0.2;
            channels     = 2;
            volume       = 70;
            modulation   = "frequency";
            frequency    = 17;
            frequencyMax = 37;
        },
        */
        );
    }
);
EOF
        log_success "Created $CONFIG_DIR/cargopit/cargopit.config"
    else
        log_info "cargopit config already exists, skipping"
    fi
}

resolve_binaries() {
    if [ -z "${SIMD_BIN}" ] && [ -x "$INSTALL_DIR/simapi/simd/build/simd" ]; then
        SIMD_BIN="$INSTALL_DIR/simapi/simd/build/simd"
    fi
    if [ -z "${SIMD_BIN}" ] && have_cmd simd; then
        SIMD_BIN="$(command -v simd)"
    fi
    if [ -z "${CARGOPIT_BIN}" ] && [ -x "$INSTALL_DIR/cargopit/build/cargopit" ]; then
        CARGOPIT_BIN="$INSTALL_DIR/cargopit/build/cargopit"
    fi
    if [ -z "${CARGOPIT_BIN}" ] && have_cmd cargopit; then
        CARGOPIT_BIN="$(command -v cargopit)"
    fi
}

create_launcher_scripts() {
    log_info "Creating launcher scripts in $BIN_DIR..."
    ensure_writable_dir "$BIN_DIR"
    resolve_binaries

    cat > "$BIN_DIR/start-simd" << EOF
#!/bin/bash
export LD_LIBRARY_PATH="\${LD_LIBRARY_PATH:-}:$SIMAPI_PREFIX/lib:$SIMAPI_PREFIX/lib64"
BIN="${SIMD_BIN:-simd}"
if [ ! -x "\$BIN" ]; then
    echo "simd not found at \$BIN" >&2
    exit 1
fi
exec "\$BIN" "\$@"
EOF
    chmod +x "$BIN_DIR/start-simd"

    cat > "$BIN_DIR/start-cargopit" << EOF
#!/bin/bash
BIN="${CARGOPIT_BIN:-cargopit}"
if [ ! -x "\$BIN" ]; then
    echo "cargopit not found at \$BIN" >&2
    exit 1
fi
exec "\$BIN" play "\$@"
EOF
    chmod +x "$BIN_DIR/start-cargopit"

    cat > "$BIN_DIR/test-cargopit" << EOF
#!/bin/bash
BIN="${CARGOPIT_BIN:-cargopit}"
if [ ! -x "\$BIN" ]; then
    echo "cargopit not found at \$BIN" >&2
    exit 1
fi
exec "\$BIN" test -vv "\$@"
EOF
    chmod +x "$BIN_DIR/test-cargopit"

    if [ -z "${CARGOPIT_TUI_BIN:-}" ]; then
        if [ -x "${CARGOPIT_SRC:-}/build/tui/release/cargopit-tui" ]; then
            CARGOPIT_TUI_BIN="$CARGOPIT_SRC/build/tui/release/cargopit-tui"
        elif [ -x "${CARGOPIT_SRC:-}/build/tui/debug/cargopit-tui" ]; then
            CARGOPIT_TUI_BIN="$CARGOPIT_SRC/build/tui/debug/cargopit-tui"
        fi
    fi
    if [ -n "${CARGOPIT_TUI_BIN:-}" ] && [ -x "$CARGOPIT_TUI_BIN" ]; then
        cp "$CARGOPIT_TUI_BIN" "$BIN_DIR/cargopit-tui"
        chmod +x "$BIN_DIR/cargopit-tui"
        log_success "Installed cargopit-tui from $CARGOPIT_TUI_BIN"
    else
        log_warn "cargopit-tui was not built (need cargo during cmake)"
    fi

    if [[ ":$PATH:" != *":$BIN_DIR:"* ]]; then
        log_warn "$BIN_DIR is not in PATH. Add this to your shell config:"
        echo "    export PATH=\"\$HOME/.local/bin:\$PATH\""
    fi
    log_success "Launcher scripts created"
}

setup_systemd_services() {
    local systemd_dir="$CONFIG_DIR/systemd/user"
    log_info "Creating systemd user service..."
    resolve_binaries

    if [ -z "${SIMD_BIN}" ]; then
        log_warn "simd binary not found; skipping systemd unit"
        return 0
    fi
    if ! ensure_writable_dir "$systemd_dir"; then
        log_warn "Skipping systemd unit (directory not writable)"
        return 0
    fi

    cat > "$systemd_dir/simd.service" << EOF
[Unit]
Description=Sim Telemetry Daemon
Documentation=https://spacefreak18.github.io/simapi/
After=default.target

[Service]
Type=simple
Environment=LD_LIBRARY_PATH=$SIMAPI_PREFIX/lib:$SIMAPI_PREFIX/lib64
ExecStart=$SIMD_BIN
Restart=on-failure
RestartSec=5

[Install]
WantedBy=default.target
EOF

    log_success "Wrote $systemd_dir/simd.service"
    if have_cmd systemctl; then
        systemctl --user daemon-reload 2>/dev/null || true
        if systemctl --user enable --now simd.service 2>/dev/null; then
            log_success "Enabled simd.service (starts at login)"
        else
            log_warn "Could not enable simd.service; cargopit will start simd when you run it"
        fi
    fi
}

install_udev_rules() {
    local rules=""
    if [ -f "${CARGOPIT_SRC:-}/udev/69-cargopit.rules" ]; then
        rules="$CARGOPIT_SRC/udev/69-cargopit.rules"
    elif [ -f "$INSTALL_DIR/cargopit/udev/69-cargopit.rules" ]; then
        rules="$INSTALL_DIR/cargopit/udev/69-cargopit.rules"
    fi
    if [ -z "$rules" ]; then
        log_warn "No udev rules found in the source tree"
        return 0
    fi
    if run_root mkdir -p /etc/udev/rules.d && run_root cp "$rules" /etc/udev/rules.d/69-cargopit.rules; then
        run_root udevadm control --reload-rules 2>/dev/null || true
        log_success "Installed udev rules (some Arduino matches are serial-specific; edit if needed)"
        log_info "Serial/HID access usually needs group membership:"
        echo "    sudo usermod -aG input,dialout,uucp \$USER"
        echo "    (log out and back in after changing groups)"
    else
        log_warn "Could not install udev rules. Copy $rules to /etc/udev/rules.d/"
    fi
}

verify_install() {
    log_info "Verifying installation..."
    resolve_binaries
    local ok=1
    if [ -n "${CARGOPIT_BIN}" ] && [ -x "$CARGOPIT_BIN" ]; then
        log_success "cargopit: $CARGOPIT_BIN"
    else
        log_error "cargopit binary missing"
        ok=0
    fi
    if [ -n "${SIMD_BIN}" ] && [ -x "$SIMD_BIN" ]; then
        log_success "simd: $SIMD_BIN"
    else
        log_error "simd binary missing"
        ok=0
    fi
    if [ -x "$BIN_DIR/cargopit-tui" ]; then
        log_success "tui: $BIN_DIR/cargopit-tui"
    else
        log_warn "cargopit-tui was not installed"
    fi
    if [ "$ok" -ne 1 ]; then
        exit 1
    fi
}

print_next_steps() {
    echo ""
    echo "╔══════════════════════════════════════════════════════════════════╗"
    echo "║                    Installation Complete!                        ║"
    echo "╚══════════════════════════════════════════════════════════════════╝"
    echo ""
    echo "Install dir:  $INSTALL_DIR"
    echo "Config:       $CONFIG_DIR/simd  $CONFIG_DIR/cargopit"
    echo "Launchers:    $BIN_DIR"
    echo ""
    echo "This installer does not configure Steam, Pulse/PipeWire sinks, or devices."
    echo ""
    echo "Launchers:"
    echo "  start-simd"
    echo "  start-cargopit"
    echo "  test-cargopit"
    echo "  cargopit-tui"
    echo ""
    echo "Confirm telemetry after the session is live:"
    echo "  hexdump /dev/shm/SIMAPI.DAT | head"
    echo "  hexdump /dev/shm/acpmf_physics | head     # AC / ACC"
    echo ""
    echo "Start:         start-cargopit   (or cargopit-tui)"
    echo "simd is started automatically if it is not already running."
    echo "You will only be asked to act if simd is not installed."
    echo ""
    echo "Edit devices:  cargopit-tui"
    echo "Examples:      $CONFIG_DIR/cargopit/cargopit.config.example"
    echo "Test devices:  test-cargopit"
    echo "TUI:           cargopit-tui"
    echo ""
    echo "Game setup:    https://spacefreak18.github.io/simapi/simd_usage"
    echo "Docs:          https://spacefreak18.github.io/simapi/"
    echo "Packages:      $CARGOPIT_RELEASES_URL"
    echo ""
}

run_distrobox() {
    if have_cmd distrobox; then
        log_info "Creating Arch distrobox 'cargopit' (if needed)..."
        distrobox create --name cargopit --image archlinux:latest --yes || true
        local script_arg="bash -c 'curl -fsSL ${CARGOPIT_RAW_MASTER_URL}/install.sh | bash -s -- --from-source'"
        if [ -n "$SCRIPT_DIR" ] && [ -f "$SCRIPT_DIR/install.sh" ]; then
            script_arg="bash \"$SCRIPT_DIR/install.sh\" --from-source"
        fi
        log_info "Running installer inside distrobox..."
        # shellcheck disable=SC2086
        distrobox enter cargopit -- $script_arg
        return 0
    fi
    print_immutable_help
    exit 1
}

parse_args() {
    while [ $# -gt 0 ]; do
        case "$1" in
            --from-source) ;;
            --aur)
                log_error "$AUR_INSTALL_UNAVAILABLE_MSG"
                exit 1
                ;;
            --distrobox) DO_DISTROBOX=1 ;;
            --build-bridges) BUILD_BRIDGES=1 ;;
            --skip-bridges) SKIP_BRIDGES=1 ;;
            --deps-only) DEPS_ONLY=1 ;;
            --detect-only) DETECT_ONLY=1 ;;
            --force-native) FORCE_NATIVE=1 ;;
            --allow-root) ALLOW_ROOT=1 ;;
            -h|--help) usage; exit 0 ;;
            *)
                log_error "Unknown option: $1"
                usage
                exit 1
                ;;
        esac
        shift
    done
}

main() {
    parse_args "$@"
    detect_distro

    if [ "$DETECT_ONLY" -eq 1 ]; then
        print_detect
        exit 0
    fi

    print_header
    log_info "Detected $DISTRO_ID $DISTRO_VERSION (family=$DISTRO_FAMILY, immutable=$DISTRO_IMMUTABLE)"

    if [ "$DO_DISTROBOX" -eq 1 ] || { [ "$DISTRO_IMMUTABLE" -eq 1 ] && [ "$FORCE_NATIVE" -eq 0 ]; }; then
        if [ "$DO_DISTROBOX" -eq 1 ]; then
            run_distrobox
            exit $?
        fi
        print_immutable_help
        exit 1
    fi

    if [ "$(id -u)" -eq 0 ] && [ "$ALLOW_ROOT" -eq 0 ]; then
        log_error "Do not run as root. Use a normal user with sudo, or pass --allow-root for containers."
        exit 1
    fi

    install_dependencies
    check_requirements
    ensure_tui_rust_toolchain

    if [ "$DEPS_ONLY" -eq 1 ]; then
        log_success "Dependencies only; done"
        exit 0
    fi

    log_info "Building from source (this may take a few minutes)..."
    prepare_sources
    build_simapi
    build_simd
    build_cargopit
    install_bridges
    setup_configs
    create_launcher_scripts
    setup_systemd_services
    install_udev_rules
    verify_install
    print_next_steps
}

main "$@"
