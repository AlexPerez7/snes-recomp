#!/usr/bin/env bash
# Construye el lanzador de Linux (`linux-launcher`) y arma la carpeta autónoma out/linux/<nombre>/ para un juego:
#   ejecutable (lanzador + frontal + generador), lib/libtcc.so, lib/libSDL2-2.0.so.0 y assets/ (cabeceras para TinyCC, perfil,
#   C propio del juego, licencias).
#   linux/build.sh <perfil.toml> [nombre]     p. ej. linux/build.sh ../alttp-recomp/games/alttp_usa.toml alttp-recomp
# Hace falta (solo para construir): gcc, libsdl2-dev, Rust y las fuentes de TinyCC rama mob en ../tinycc (o TINYCC_DIR).
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
repo="$(dirname "$here")"
profile="$(readlink -f "${1:?uso: linux/build.sh <perfil.toml> [nombre]}")"
name="${2:-snes-port}"
tinycc="${TINYCC_DIR:-$repo/../tinycc}"
[ -f "$tinycc/libtcc.c" ] || { echo "faltan las fuentes de TinyCC en $tinycc (TINYCC_DIR)"; exit 1; }
[ ! -e "$tinycc/config.h" ] || { echo "$tinycc/config.h es de ./configure: usa una copia sin configurar"; exit 1; }
out="$repo/out/linux/$name"
rm -rf "$out"; mkdir -p "$out/lib" "$out/assets/tcc_include" "$out/assets/game/c" "$out/assets/licenses"

(cd "$repo" && cargo rustc --release --lib --crate-type staticlib)
gen="$repo/target/release/libsnes_recomp.a"

gcc -O2 -w -fPIC -shared -DONE_SOURCE=1 -DTCC_TARGET_X86_64=1 -D_GNU_SOURCE -I"$here/tcc_config" -I"$tinycc" \
    "$tinycc/libtcc.c" -o "$out/lib/libtcc.so" -ldl -lm -lpthread
gcc -O2 -std=gnu11 -w -D_GNU_SOURCE -DSNES_RT_LIBRARY -DSNES_RT_TICK -I"$tinycc" -I"$repo/runtime" $(sdl2-config --cflags) \
    "$here/launcher.c" "$repo/runtime/play.c" "$gen" -o "$out/$name" \
    -L"$out/lib" -ltcc $(sdl2-config --libs) -lm -ldl -lpthread \
    -Wl,-rpath,'$ORIGIN/lib' -Wl,--enable-new-dtags
sdl="$(ldd "$out/$name" | awk '/libSDL2-2.0.so.0/ {print $3}')"
cp -L "$sdl" "$out/lib/libSDL2-2.0.so.0"

cp "$tinycc"/include/*.h "$out/assets/tcc_include/"
cp -r "$repo"/spikes/tcc_include/* "$out/assets/tcc_include/"
cp "$repo/runtime/snes_rt.h" "$out/assets/tcc_include/"
cp "$profile" "$out/assets/game/profile.toml"
gdir="${profile%.toml}"
for f in "$gdir"/*.c "$gdir"/*.h; do [ -f "$f" ] && cp "$f" "$out/assets/game/c/"; done
[ -d "$gdir/lang" ] && cp -r "$gdir/lang" "$out/assets/game/c/lang"
cp "$tinycc/COPYING" "$out/assets/licenses/libtcc-LGPL-2.1.txt"
[ -f /usr/share/doc/libsdl2-2.0-0/copyright ] && cp /usr/share/doc/libsdl2-2.0-0/copyright "$out/assets/licenses/SDL2-zlib.txt" || true
cp "$repo/android/NOTICE.txt" "$out/assets/licenses/NOTICE.txt"
echo "listo: $out/$name"
