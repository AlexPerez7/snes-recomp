#!/usr/bin/env bash
# bench_pc.sh - prueba de factibilidad (android-feasibility-spike), grupo 3: tiempo por fotograma del runtime sin vídeo
# con el C del juego compilado por gcc -O2 y por TinyCC, y compilación en memoria con libtcc (tcc_host.c).
#
#   spikes/bench_pc.sh <rom> <perfil.toml> <prefijo de TinyCC instalado> [fotogramas] [repeticiones]
#
# Todo lo que deriva de la ROM queda en out/bench (ignorado por git). Linux con gcc; TinyCC compilado desde sus fuentes
# (./configure --prefix=P && make && make install). Imprime una tabla en Markdown para design.md › Resultados.
set -euo pipefail
ROM=$(realpath "$1"); PROFILE=$(realpath "$2"); TCC=$(realpath "$3")
FRAMES=${4:-3000}; RUNS=${5:-3}
HERE=$(cd "$(dirname "$0")" && pwd); ROOT=$(dirname "$HERE")
OUT=$ROOT/out/bench; SRC=$OUT/src
rm -rf "$OUT"; mkdir -p "$OUT"
cd "$ROOT"
cargo build --release -q --bin snes-recomp
./target/release/snes-recomp codegen "$ROM" --all -o "$SRC" --game "$PROFILE" 2>/dev/null
GAME_C=$(cd "$SRC" && ls rom_*.c snes_entry.c game/*.c 2>/dev/null)
DEFS="-DSNES_RT_TICK"
cd "$OUT"

# 1) gcc -O2 en una pieza (objetos en paralelo)
mkdir -p obj
for f in $GAME_C; do echo "$f"; done | xargs -P "$(nproc)" -I{} sh -c 'gcc -std=c11 -O2 '"$DEFS"' -I src -c src/{} -o obj/$(echo {} | tr / _).o'
gcc -std=c11 -O2 $DEFS -I src -o gcc_one src/snes_headless.c obj/*.o -lm
# 2) TinyCC en una pieza
T0=$(date +%s.%N)
(cd src && "$TCC/bin/tcc" -std=c11 $DEFS -I . -o ../tcc_one snes_headless.c $GAME_C -lm)
T1=$(date +%s.%N)
# 3) runtime con gcc -O2 como biblioteca + C del juego en memoria con libtcc
gcc -std=c11 -O2 $DEFS -DSNES_RT_LIBRARY -shared -fPIC -Wl,--no-undefined -I src -o libsnesrt.so src/snes_headless.c -lm
gcc -std=c11 -O2 -I "$TCC/include" -o tcc_host "$HERE/tcc_host.c" -L. -lsnesrt -Wl,-rpath,'$ORIGIN' "$TCC/lib/libtcc.a" -ldl -lm -lpthread

stats() { # media y p99 en ms de un fichero de --frame-times (sin el primer fotograma, que incluye el arranque)
    tail -n +2 "$1" | sort -n | awk '{a[NR]=$1; s+=$1} END {i=int(NR*0.99); if(i<1)i=1; printf "%.3f %.3f", s/NR/1000, a[i]/1000}'
}
run() { # nombre, comando...
    local name=$1; shift
    for r in $(seq 1 "$RUNS"); do
        mkdir -p "dump_$name"
        "$@" "$ROM" --frames "$FRAMES" --frame-times "ft_${name}_$r.txt" --dump "dump_$name" > "log_${name}_$r.txt" 2>&1 || { echo "$name falló"; cat "log_${name}_$r.txt"; exit 1; }
    done
}
run gcc_one ./gcc_one
run tcc_one ./tcc_one
run tcc_mem ./tcc_host src "$TCC/lib/tcc"

for f in wram.bin vram.bin cgram.bin oam.bin; do
    cmp -s "dump_gcc_one/$f" "dump_tcc_one/$f" && cmp -s "dump_gcc_one/$f" "dump_tcc_mem/$f" || { echo "AVISO: $f difiere entre construcciones"; }
done

echo
echo "Máquina: $(grep -m1 'model name' /proc/cpuinfo | cut -d: -f2 | xargs), $(nproc) núcleos; $(gcc --version | head -1); $("$TCC/bin/tcc" -v)"
echo "Recorrido: $FRAMES fotogramas sin entradas (intro y demostración), $RUNS repeticiones"
echo
echo "| Construcción | Media (ms/fotograma) | p99 (ms) |"
echo "|---|---|---|"
for name in gcc_one tcc_one tcc_mem; do
    for r in $(seq 1 "$RUNS"); do stats "ft_${name}_$r.txt"; echo; done | awk -v n="$name" '{m+=$1; p+=$2} END {printf "| %s | %.3f | %.3f |\n", n, m/NR, p/NR}'
done
echo
printf 'TinyCC (una pieza, desde la línea de comandos): %.1f s de compilación y enlace\n' "$(echo "$T1 - $T0" | bc)"
grep -h '^tcc:' log_tcc_mem_*.txt | head -"$RUNS"
