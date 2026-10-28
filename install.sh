#!/usr/bin/env bash
set -euo pipefail

REPO="eyeblech/kava"
INSTALL_DIR="${KAVA_INSTALL_DIR:-$HOME/.local/bin}"

echo "❂ Installing KAVA — Cosmic Audio Kaleidoscope..."
mkdir -p "$INSTALL_DIR"

OS="$(uname -s)"
ARCH="$(uname -m)"

case "$OS" in
    Linux)  OS_TAG="unknown-linux-gnu" ;;
    Darwin) OS_TAG="apple-darwin" ;;
    *)      OS_TAG="" ;;
esac

case "$ARCH" in
    x86_64|amd64) ARCH_TAG="x86_64" ;;
    aarch64|arm64) ARCH_TAG="aarch64" ;;
    *) ARCH_TAG="" ;;
esac

# 1. Try downloading prebuilt release binary from GitHub Releases first
if [ -n "$OS_TAG" ] && [ -n "$ARCH_TAG" ] && command -v curl >/dev/null 2>&1; then
    TARGET="${ARCH_TAG}-${OS_TAG}"
    URL="https://github.com/${REPO}/releases/latest/download/kava-${TARGET}.tar.gz"
    TMP_DIR="$(mktemp -d)"
    if curl -fsSL "$URL" -o "$TMP_DIR/kava.tar.gz" 2>/dev/null; then
        tar -xzf "$TMP_DIR/kava.tar.gz" -C "$TMP_DIR"
        install -m 755 "$TMP_DIR/kava" "$INSTALL_DIR/kava"
        rm -rf "$TMP_DIR"
        echo "✦ Installed prebuilt kava binary to $INSTALL_DIR/kava"
        echo "✦ Run 'kava' in your terminal!"
        exit 0
    fi
    rm -rf "$TMP_DIR"
fi

# 2. Fallback: Build from source with Cargo (zero external crate dependencies, builds in ~10s)
if command -v cargo >/dev/null 2>&1; then
    echo "✦ Building KAVA from source via Cargo (zero external dependencies)..."
    TMP_SRC="$(mktemp -d)"
    git clone --depth 1 "https://github.com/${REPO}.git" "$TMP_SRC/kava"
    (cd "$TMP_SRC/kava" && cargo build --release)
    install -m 755 "$TMP_SRC/kava/target/release/kava" "$INSTALL_DIR/kava"
    rm -rf "$TMP_SRC"
    echo "✦ Installed kava to $INSTALL_DIR/kava"
    echo "✦ Make sure $INSTALL_DIR is in your PATH, then run: kava"
    exit 0
fi

echo "Error: Could not find a prebuilt binary or 'cargo' to compile from source." >&2
echo "Install Rust from https://rustup.rs and re-run this script." >&2
exit 1
