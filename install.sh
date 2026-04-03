#!/bin/sh
set -e

REPO="sarthakagrawal927/port-whisperer"
INSTALL_DIR="${INSTALL_DIR:-/usr/local/bin}"

# Detect architecture
ARCH=$(uname -m)
case "$ARCH" in
  x86_64)  TARGET="x86_64-apple-darwin" ;;
  arm64)   TARGET="aarch64-apple-darwin" ;;
  aarch64) TARGET="aarch64-apple-darwin" ;;
  *)       echo "Unsupported architecture: $ARCH"; exit 1 ;;
esac

# Detect OS
OS=$(uname -s)
if [ "$OS" != "Darwin" ]; then
  echo "port-whisperer only supports macOS (got $OS)"
  exit 1
fi

# Get latest release tag
LATEST=$(curl -sL "https://api.github.com/repos/$REPO/releases/latest" | grep '"tag_name"' | cut -d'"' -f4)
if [ -z "$LATEST" ]; then
  echo "No releases found. Install from source instead:"
  echo "  cargo install --git https://github.com/$REPO"
  exit 1
fi

ASSET="ports-${TARGET}.tar.gz"
URL="https://github.com/$REPO/releases/download/$LATEST/$ASSET"

echo "Installing port-whisperer $LATEST ($TARGET)..."

# Download and extract
TMPDIR=$(mktemp -d)
curl -sL "$URL" -o "$TMPDIR/$ASSET"
tar xzf "$TMPDIR/$ASSET" -C "$TMPDIR"

# Install
if [ -w "$INSTALL_DIR" ]; then
  mv "$TMPDIR/ports" "$INSTALL_DIR/ports"
else
  sudo mv "$TMPDIR/ports" "$INSTALL_DIR/ports"
fi

rm -rf "$TMPDIR"

echo "Installed ports to $INSTALL_DIR/ports"
echo "Run 'ports' to get started."
