#!/usr/bin/env bash
# Build GraphDraw.app and a DMG. Usage: build-dmg.sh <version> <release-dir> <arch-label>
# Unsigned: first launch needs right-click > Open (or xattr -d com.apple.quarantine).
set -euo pipefail
VERSION="$1"; BIN="$2"; ARCH="$3"
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
APP="$ROOT/dist/GraphDraw.app"
rm -rf "$APP"; mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
sed "s/__VERSION__/$VERSION/g" "$ROOT/packaging/macos/Info.plist" > "$APP/Contents/Info.plist"
cp "$BIN/graphdraw" "$BIN/graphdraw-cli" "$APP/Contents/MacOS/"
chmod +x "$APP/Contents/MacOS/"*

# Icons: the program icon and the one for .gdr files, as .icns from the PNG set.
for kind in app gdr; do
  ICONSET="$ROOT/dist/$kind.iconset"; rm -rf "$ICONSET"; mkdir -p "$ICONSET"
  for s in 16 32 128 256 512; do
    cp "$ROOT/assets/brand/$kind-$s.png" "$ICONSET/icon_${s}x${s}.png"
    d=$((s*2)); cp "$ROOT/assets/brand/$kind-$d.png" "$ICONSET/icon_${s}x${s}@2x.png"
  done
  name=graphdraw; [ "$kind" = gdr ] && name=gdr
  iconutil -c icns "$ICONSET" -o "$APP/Contents/Resources/$name.icns"
  rm -rf "$ICONSET"
done

# Ad-hoc signature so Gatekeeper reports a consistent bundle.
codesign --force --deep --sign - "$APP"

DMG="$ROOT/dist/graphdraw-$VERSION-macos-$ARCH.dmg"
rm -f "$DMG"
STAGE="$ROOT/dist/dmg-stage"; rm -rf "$STAGE"; mkdir -p "$STAGE"
cp -R "$APP" "$STAGE/"; ln -s /Applications "$STAGE/Applications"
hdiutil create -volname "GraphDraw" -srcfolder "$STAGE" -ov -format UDZO "$DMG"
rm -rf "$STAGE"
echo "built $DMG"
