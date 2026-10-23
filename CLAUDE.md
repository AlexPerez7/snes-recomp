# snes-recomp

Recompilador estático de ROMs de SNES (CPU 65816) escrito en Rust desde cero. Primer objetivo: *The Legend of Zelda: A Link to the Past* (USA, CRC32 `777AAC2F`).

## Comandos

```
cargo test                                          # tests unitarios
cargo run --release -- info roms/alttp_usa.sfc      # cabecera, CRC y vectores
cargo run --release -- disasm roms/alttp_usa.sfc 00:8000 40
cargo run --release -- discover roms/alttp_usa.sfc  # añade --unresolved para ver saltos indirectos sin tabla, o --widths para ver conflictos M/X, PLP sin emparejar y llamadas sin retorno
cargo run --release -- graph roms/alttp_usa.sfc [00:8000]  # resumen, o detalle de la función que contiene la dirección
cargo run --release -- codegen roms/alttp_usa.sfc 00:8000 --depth 1 -o out/reset.c  # C de una función y sus llamadas
cargo run --release -- codegen roms/alttp_usa.sfc --all -o out/rom        # C de toda la ROM: rom_BB.c por banco, snes_funcs.h y snes_rt.h; las funciones no traducibles quedan como stub (rt_unsupported) y se listan
# Ejecutar la ROM completa sin vídeo ni audio (con Visual Studio: desde "x64 Native Tools" o tras vcvars64.bat):
#   cl /MP /TC /std:c11 /DSNES_RT_TICK /DSNES_RT_CHECKS out\rom\*.c /Fe:out\headless.exe /Fo:out\rom\
#   out\headless.exe roms\alttp_usa.sfc --frames 120 [--frame-blocks 20000] [--dump out\dump] [--screenshot out\shot.png]   # informe de registros, DMA y causa de parada; --dump (carpeta existente) escribe vram.bin, cgram.bin, oam.bin y ppu_regs.bin; --screenshot dibuja el estado final de la PPU (modo 1 y sprites) en un PNG de 256x224
```

## Estructura

- `src/rom.rs`: carga de ROM, mapeo LoROM, cabecera y vectores.
- `src/opcodes.rs`, `src/disasm.rs`: tabla de opcodes y decodificador, con seguimiento de los anchos M/X.
- `src/discover.rs`: descubrimiento de código por descenso recursivo, guiado por la configuración. Sigue los anchos M/X por contexto de función (entrada, anchos), a través de llamadas, retornos y pares `PHP`/`PLP`.
- `src/graph.rs`: bloques básicos y funciones a partir del descubrimiento (aristas, tail calls, diagnósticos).
- `src/codegen.rs`: generador de C (una función por función del grafo, una etiqueta por bloque) y pruebas que compilan y ejecutan el C. Solo rechaza conflictos de anchos M/X que cambian el C o la longitud de la instrucción. `generate_all` emite todas las funciones y deja las que no puede traducir como stub.
- `runtime/snes_rt.h`: interfaz del C generado con el runtime (estado de la CPU, bus, ganchos, aritmética). Se incrusta en el binario.
- `runtime/headless.c`: runtime sin vídeo ni audio (bus LoROM, registros que guardan lo escrito, DMA real, VRAM/CGRAM/OAM con volcado `--dump`, captura `--screenshot` del modo 1 y los sprites, fotogramas por bloques, NMI, informe). `codegen --all` lo escribe como `snes_headless.c`, junto a `snes_entry.c`; la ROM se lee del disco, nunca se incrusta. Tests en `src/headless.rs`.
- `src/cctest.rs`: solo para tests; busca `CC`/gcc/clang/cc o Visual Studio y compila C.
- `src/config.rs`: formato de `games/<id>.toml` (documentado al inicio del fichero).
- `src/games.rs`: identificación de ROMs conocidas por CRC32.
- `games/alttp_usa.toml`: entradas y tablas de saltos de A Link to the Past.

## Reglas

- El código es propio (clean-room): no copiar código de otros recompiladores.
- El ROM lo aporta el usuario. Nunca se sube ni se distribuye: `roms/` y `*.sfc`/`*.smc` están en `.gitignore`.
- Las tablas de saltos que se añadan a `games/*.toml` se comprueban a mano contra el ROM real.
- El C generado deriva del ROM: va a `out/` (ignorado) y nunca se sube.
- Los tests que necesitan compilador o el ROM se omiten solos si faltan; en esta máquina se usa Visual Studio Build Tools.
- Los cambios se hacen con OpenSpec: `/opsx:explore`, `/opsx:propose`, `/opsx:apply`, `/opsx:archive`. Contexto del proyecto en `openspec/config.yaml`.
- Artefactos de OpenSpec y documentación, en español.
