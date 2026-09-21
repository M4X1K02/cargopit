#!/usr/bin/env bash
# Cloud Agent bootstrap for cargopit.
#
# Idempotent: safe to run repeatedly and against a cached/partially prepared
# tree. It installs system build dependencies, initialises the required
# submodules, ensures a current Rust toolchain, then configures and builds the
# C CLI, the Rust TUI, and the automated test suite. Mirrors
# .github/workflows/pr-build.yaml so local builds match CI.
set -euo pipefail

cd "$(dirname "$0")/.."

# --- System build dependencies (C CLI + test suite) -------------------------
# Same package list as the pr-build.yaml CI leg. apt-get install is a no-op
# for packages that are already present, so this stays idempotent.
SUDO=""
if [ "$(id -u)" -ne 0 ]; then
    SUDO="sudo"
fi

export DEBIAN_FRONTEND=noninteractive
$SUDO apt-get update
$SUDO apt-get install -y --no-install-recommends \
    cmake pkg-config build-essential \
    libuv1-dev libargtable2-dev libserialport-dev libconfig-dev \
    libhidapi-dev liblua5.4-dev libxdg-basedir-dev libxml2-dev \
    libpulse-dev libproc2-dev

# --- Submodules -------------------------------------------------------------
# simapi (shared-memory headers/mappers) is required to configure and build;
# an empty submodule fails CMake with a missing CMakeLists.txt error. The Rust
# TUI's build.rs also compiles a native view against simapi's simdata.h.
git submodule update --init --recursive

# --- Rust toolchain (for the cargopit-tui build) ----------------------------
# CMake builds cargopit-tui via cargo (BUILD_TUI defaults ON and hard-errors
# when cargo is missing), and tui/Cargo.toml requires rust-version 1.88+. The
# base image's preinstalled toolchain can be older, so make an up-to-date
# stable toolchain the default before invoking CMake. rustup's home is
# world-writable on the base image, so no sudo is needed.
export RUSTUP_HOME="${RUSTUP_HOME:-/usr/local/rustup}"
export CARGO_HOME="${CARGO_HOME:-/usr/local/cargo}"
export PATH="$CARGO_HOME/bin:$PATH"
rustup toolchain install stable --profile minimal
rustup default stable

# --- Configure + build (C CLI + tests + Rust TUI) ---------------------------
# gcc/g++ are pinned explicitly: the base image's default cc/c++ resolve to
# clang, which selects a gcc-14 toolchain whose libstdc++ dev files are not
# installed and then fails to link with "cannot find -lstdc++". The project's
# own packaging CI configures with gcc/g++ for the same reason.
cmake -B build -DENABLE_TESTS=ON -DCMAKE_BUILD_TYPE=Debug \
    -DCMAKE_C_COMPILER=gcc -DCMAKE_CXX_COMPILER=g++
cmake --build build --config Debug -j"$(nproc)"
