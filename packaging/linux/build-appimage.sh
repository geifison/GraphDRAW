#!/usr/bin/env bash
# Build an AppImage. Usage: build-appimage.sh <version> <release-dir>
set -euo pipefail
VERSION="$1"; BIN="$2"
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
DIR="$ROOT/dist/AppDir"; rm -rf "$DIR"
mkdir -p "$DIR/usr/bin" "$DIR/usr/share/applications" "$DIR/usr/share/icons/hicolor/256x256/apps"
cp "$BIN/graphdraw" "$BIN/graphdraw-cli" "$DIR/usr/bin/"
cp "$ROOT/packaging/linux/graphdraw.desktop" "$DIR/usr/share/applications/"
cp "$ROOT/assets/icon/graphdraw-256.png" "$DIR/usr/share/icons/hicolor/256x256/apps/graphdraw.png"
TOOL="$ROOT/dist/linuxdeploy.AppImage"
[ -f "$TOOL" ] || curl -sSL -o "$TOOL" https://github.com/linuxdeploy/linuxdeploy/releases/download/continuous/linuxdeploy-x86_64.AppImage
chmod +x "$TOOL"
cd "$ROOT/dist"
VERSION="$VERSION" OUTPUT="graphdraw-$VERSION-linux-x86_64.AppImage" "$TOOL" --appimage-extract-and-run \
  --appdir "$DIR" --desktop-file "$DIR/usr/share/applications/graphdraw.desktop" \
  --icon-file "$DIR/usr/share/icons/hicolor/256x256/apps/graphdraw.png" --output appimage
rm -rf "$DIR"
