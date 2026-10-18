#!/usr/bin/env bash
# Gera o instalador Windows: target/package/GraphDraw-<versão>-setup.exe
# (mais o .sha256). Precisa de: cargo (alvo GNU), a ponte libcdr já
# compilada em target/cdr-bridge (native/cdr-bridge/build.sh) e o makensis
# (MSYS2: pacman -S mingw-w64-x86_64-nsis).
set -euo pipefail

repo="$(cd "$(dirname "$0")/.." && pwd)"
cd "$repo"
version="$(grep -m1 '^version' Cargo.toml | sed -E 's/.*"(.*)".*/\1/')"
stage="$repo/target/package/stage"
out="$repo/target/package/GraphDraw-$version-setup.exe"

[[ -f target/cdr-bridge/gd-cdr-bridge.exe ]] || {
  echo "falta target/cdr-bridge/gd-cdr-bridge.exe (rode native/cdr-bridge/build.sh)" >&2
  exit 1
}
cargo build --release -p graphdraw-app -p graphdraw-cli -p graphdraw-thumbs

rm -rf "$stage"
mkdir -p "$stage/cdr-bridge"
cp target/release/graphdraw-app.exe "$stage/GraphDraw.exe"
cp target/release/graphdraw.exe "$stage/graphdraw-cli.exe"
cp target/release/graphdraw_thumbs.dll "$stage/graphdraw_thumbs.dll"
cp target/cdr-bridge/* "$stage/cdr-bridge/"
cp LICENSE "$stage/LICENSE.txt"
cp THIRD_PARTY.md "$stage/THIRD_PARTY.md"
cp crates/graphdraw-app/assets/brand/app.ico "$stage/app.ico"

win() { cygpath -w "$1" 2>/dev/null || echo "$1"; }
makensis -V2 -DVERSION="$version" -DSTAGE="$(win "$stage")" -DOUTFILE="$(win "$out")" \
  "$(win "$repo/installer/windows/graphdraw.nsi")"
(cd "$(dirname "$out")" && sha256sum "$(basename "$out")" > "$(basename "$out").sha256")
echo "ok: $out"
