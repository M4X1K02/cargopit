#!/usr/bin/env bash
# Cloud Agent bootstrap for cargopit (monocoque fork).
#
# Idempotent: safe to run repeatedly and against a cached/partially prepared
# tree. It installs system build dependencies, initialises the required
# submodules, then configures and builds the CLI plus the automated test
# suite. Mirrors .github/workflows/pr-build.yaml so local builds match CI.
set -euo pipefail

cd "$(dirname "$0")/.."

# --- System build dependencies (CLI + test suite) ---------------------------
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
# an empty submodule fails CMake with a missing CMakeLists.txt error.
git submodule update --init --recursive

# --- Configure + build ------------------------------------------------------
# gcc/g++ are pinned explicitly: the base image's default cc/c++ resolve to
# clang, which selects a gcc-14 toolchain whose libstdc++ dev files are not
# installed and then fails to link with "cannot find -lstdc++". The project's
# own packaging CI configures with gcc/g++ for the same reason.
cmake -B build -DENABLE_TESTS=ON -DCMAKE_BUILD_TYPE=Debug \
    -DCMAKE_C_COMPILER=gcc -DCMAKE_CXX_COMPILER=g++
cmake --build build --config Debug -j"$(nproc)"
