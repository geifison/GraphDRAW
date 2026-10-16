#!/usr/bin/env bash
# Compila o gd-cdr-bridge no ambiente MSYS2 UCRT64 (Windows) ou em Linux com
# libcdr-dev instalado. Saída: <repo>/target/cdr-bridge/ com o executável e,
# no Windows, as DLLs necessárias copiadas ao lado.
#
# Windows:  C:\msys64\usr\bin\bash.exe -lc "cd '<repo>' && MSYSTEM=UCRT64 native/cdr-bridge/build.sh"
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$here/../.." && pwd)"
out="$repo/target/cdr-bridge"
mkdir -p "$out"

exe="gd-cdr-bridge"
case "$(uname -s)" in
  MINGW*|MSYS*) exe="gd-cdr-bridge.exe" ;;
esac

# shellcheck disable=SC2046
g++ -std=c++17 -O2 -Wall -Wextra -o "$out/$exe" "$here/gd_cdr_bridge.cpp" \
  $(pkg-config --cflags --libs libcdr-0.1 librevenge-0.0 librevenge-stream-0.0)

if [[ "$exe" == *.exe ]]; then
  # Copia as DLLs do MSYS2 de que o executável depende (exceto as do sistema).
  ldd "$out/$exe" | awk '{print $3}' | grep -i '/ucrt64/' | while read -r dll; do
    cp -u "$dll" "$out/"
  done
fi

echo "ok: $out/$exe"
