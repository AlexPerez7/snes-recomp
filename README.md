# snes-recomp

Recompilador estático de ROMs de SNES (CPU 65816) escrito en Rust desde cero. Traduce la ROM a C, que se compila junto a un runtime propio (bus, DMA/HDMA, PPU, SPC700 y DSP, entrada y partidas guardadas) para obtener un ejecutable nativo.

Primer objetivo: *The Legend of Zelda: A Link to the Past* (USA, CRC32 `777AAC2F`), que ya se juega en una ventana con imagen, sonido, teclado/mando y partidas guardadas.

> **Aviso legal.** Este repositorio no contiene ninguna ROM ni recurso del juego. Cada usuario debe aportar su propia copia obtenida legalmente. El C generado deriva de la ROM, así que va a `out/` (ignorado por git) y no debe subirse ni distribuirse.

Licencia: sin elegir todavía (añadir un `LICENSE` antes de abrir el proyecto a terceros).

## Cómo funciona

```
ROM ──► carga y cabecera ──► descubrimiento de código ──► grafo de funciones y bloques ──► generador de C
                              (descenso recursivo,         (aristas, tail calls,           (una función por función,
                               tablas de saltos,            diagnósticos)                   una etiqueta por bloque)
                               anchos M/X)                                                          │
                                                                                                    ▼
                                         runtime (C): bus LoROM · DMA/HDMA · PPU · SPC700+DSP · entrada · SRAM ──► ejecutable
```

- **Descubrimiento:** sigue el código desde los vectores y las entradas de `games/<id>.toml`, resuelve saltos indirectos con tablas comprobadas a mano contra la ROM y sigue los anchos de los registros M/X por contexto de función.
- **Generación:** cada función del grafo es una función C; un bloque alcanzado con anchos distintos se emite una vez por combinación. Los retornos no locales (descartar una dirección con `PLA`/`PLA` antes de `RTS`) funcionan porque cada llamada comprueba la pila y la dirección de retorno al volver.
- **Runtime:** el C generado habla con el runtime a través de `runtime/snes_rt.h`. El runtime se reparte en un núcleo de hardware, la PPU, el sonido y dos frontales (uno sin vídeo para pruebas y otro interactivo con SDL2).

## Estado

| Área | Estado |
|---|---|
| Carga de ROM (LoROM, cabecera de copiador, CRC32, vectores) | hecho |
| Decodificador 65816 y desensamblador, con seguimiento de M/X | hecho |
| Descubrimiento de código y tablas de saltos | hecho (ALttP: 0 saltos sin resolver) |
| Grafo de funciones y bloques | hecho |
| Generador de C de toda la ROM | hecho: ALttP genera 4 176 de 4 178 funciones; 2 quedan como *stub* (`01:CA17`, `08:A8E3`) y no se ejecutan |
| Bus, DMA y HDMA | hecho |
| PPU por línea | hecho: modos 0 a 7, sprites, ventanas, mosaico, color math, HDMA, modo 7 |
| SPC700 y DSP | hecho: 256 instrucciones, temporizadores, ROM de arranque, voces BRR, ADSR/GAIN, ruido, eco |
| Frontal interactivo (SDL2) | hecho: ventana, pantalla completa, teclado y mandos, audio, `.srm` |
| Constructor de port desde la ROM del usuario | propuesto (`openspec/changes/rom-to-port-builder`) |
| Opciones de port (menú, `config.ini`, filtros) | propuesto (`openspec/changes/port-options`) |

Validación: más de 200 pruebas (algunas compilan y ejecutan el C generado; las que necesitan compilador, SDL2 o la ROM se omiten solas si faltan) y suites de *fuzz* de 30 000 fotogramas por mazmorras, overworld y Mundo Oscuro sin paradas ni *stubs* ejecutados.

Limitaciones conocidas: la interpolación del DSP usa un núcleo en forma de campana, cercano a la gaussiana del hardware pero no su tabla exacta; la PPU no emula el entrelazado; los comportamientos dependientes de ciclos exactos no se reproducen (el reloj es proporcional por bloques).

## Requisitos

- Rust estable (`cargo`).
- Un compilador de C: `gcc`, `clang` o Visual Studio Build Tools (en Windows, desde "x64 Native Tools" o tras `vcvars64.bat`).
- Para el frontal interactivo: el paquete de desarrollo de SDL2 para Visual C++ (`SDL2-devel-x.y.z-VC.zip`), descomprimido en `out/sdl2` o apuntado por `SDL2_DIR`.
- Tu ROM en `roms/` (ignorada por git), por ejemplo `roms/alttp_usa.sfc`.

## Uso

### Análisis

```
cargo run --release -- info roms/alttp_usa.sfc                 # cabecera, CRC y vectores
cargo run --release -- disasm roms/alttp_usa.sfc 00:8000 40
cargo run --release -- discover roms/alttp_usa.sfc             # --unresolved, --widths
cargo run --release -- graph roms/alttp_usa.sfc [00:8000]      # resumen, o la función que contiene la dirección
cargo run --release -- codegen roms/alttp_usa.sfc 00:8000 --depth 1 -o out/reset.c
cargo run --release -- codegen roms/alttp_usa.sfc --all -o out/rom   # C de toda la ROM + runtime
cargo test
```

### Jugar con `snes-port` (lo más simple)

`snes-port` construye el juego en tu máquina a partir de **tu** ROM y lo arranca. No se distribuye ningún juego: el ejecutable se genera aquí, la primera vez (unas decenas de segundos con MSVC; algo más con gcc), y las siguientes arranca al instante.

```
snes-port roms\alttp_usa.sfc                       # construye (si hace falta) y juega; también vale arrastrar la ROM sobre snes-port.exe
snes-port roms\alttp_usa.sfc -- --scale 3 --mute   # lo que va tras -- se pasa al juego
snes-port roms\alttp_usa.sfc --no-launch           # solo construye
snes-port clean [roms\alttp_usa.sfc]               # borra la caché de esa ROM (o toda); nunca toca ROMs ni partidas
```

- **Qué se queda dónde.** El C generado y el ejecutable viven en la caché (`%LOCALAPPDATA%\snes-port`, o `--cache <carpeta>` / `SNES_PORT_CACHE`), en una carpeta `<crc32>-<versión>`; cambiar el runtime reconstruye solo. El ejecutable lee la ROM del sitio donde está, y la partida guardada (`.srm`) queda junto a ella. Tras construir se comprueba que ningún byte de la ROM ha acabado en lo construido.
- **Compilador.** Usa `CC`, gcc, clang o Visual Studio, y los ficheros de desarrollo de SDL2 (`SDL2_DIR`, o la carpeta `sdl2` junto a `snes-port.exe`, que el paquete de `scripts\release.ps1` ya incluye). Con uno local **nunca se descarga nada**. Sin compilador pregunta si descargar TinyCC (480 KiB, desde `download.savannah.gnu.org`, con SHA-256 fijado en el programa; si no coincide se descarta); `--download-compiler` responde que sí de antemano. Es lo único que usa la red.
- **Paquete para repartir.** `powershell -File scripts\release.ps1` crea `out\release\snes-port-<versión>-win64.zip` con el programa, `games/*.toml`, este README y SDL2. Falla (y no crea el zip) si encuentra `.sfc`, `.smc`, `.srm` o C generado.
- **Códigos de salida.** 2: ROM no reconocida (se listan las admitidas); 3: falta compilador o SDL2; 4: la construcción falló.

### Jugar a mano


`codegen --all` escribe en `out/rom` los bancos en C, el runtime, `play/snes_play.c` y `build_play.bat`:

```
cd out\rom
set SDL2_DIR=...\SDL2-2.30.9
build_play.bat
snes_play.exe ..\..\roms\alttp_usa.sfc [--scale 3] [--fullscreen] [--mute] [--wav salida.wav]
```

Sin argumento de ROM (o con doble clic en el exe) se abre el diálogo de Windows para elegirla.

| Tecla | Acción |
|---|---|
| Flechas | cruceta |
| X / Z / S / A | B / Y / A / X |
| Q / W | L / R |
| Intro / Mayús derecha | START / SELECT |
| F1 | menú de opciones |
| F11 | pantalla completa |
| P | pausa (silencia el audio) |
| Tab (mantener) | avance rápido |
| F10 | captura PNG |
| Esc | salir |

- Cualquier mando SDL funciona, también conectado en caliente: el primero es el jugador 1 y el segundo el jugador 2. `--print-keys` lista el teclado del jugador 1.
- Las opciones viven en `config.ini` junto a la ROM: se crea con los valores por defecto la primera vez, conserva comentarios y claves que no conoce, y se relee al arrancar. La línea de comandos manda sobre el fichero solo en esa ejecución (`--scale N`, `--fullscreen`, `--opt CLAVE=VALOR`, repetible). Un `<rom>.keys` antiguo se importa una vez, si aún no hay `config.ini`.
- F1 abre un menú (teclado o mando: cruceta, A confirma, B vuelve) que pausa la emulación, aplica los cambios al instante y los guarda al cerrarlo. En «Controls» se elige jugador y botón y se pulsa la tecla o el botón del mando que se quiere asignar; si ya está en uso avisa y ofrece reemplazar (Intro) o cancelar (Esc).

| Clave | Valores (por defecto el primero) | Efecto |
|---|---|---|
| `fullscreen` | `false`, `true` | pantalla completa sin bordes (F11 la alterna) |
| `vsync` | `false`, `true` | sincronía vertical del renderizador |
| `aspect` | `8:7`, `4:3`, `free` | píxel cuadrado, píxel 7/6 más ancho como en un televisor, o la imagen llena toda la ventana |
| `scale` | `3` (1 a 8) | tamaño inicial de la ventana |
| `scaling` | `integer`, `fit` | múltiplos enteros, o relleno del área disponible |
| `filter` | `nearest`, `bilinear`, `sharp` | `sharp` escala por un factor entero y suaviza solo el resto |
| `overscan` | `0` (0 a 8) | líneas recortadas arriba y abajo |
| `scanlines` | `0` (0 a 100) | oscurece una de cada dos líneas de pantalla en ese porcentaje |
| `grid` | `false`, `true` | rejilla entre los píxeles de la consola |
| `volume` | `100` (0 a 100) | volumen maestro |
| `audio_device` | vacío = el del sistema | nombre del dispositivo de salida |
| `p1.key.A`, `p2.pad.B`… | nombres de tecla o de botón de SDL separados por comas | asignación por jugador (`key` teclado, `pad` mando) y botón (`B Y SELECT START UP DOWN LEFT RIGHT A X L R`) |

- Para ver el resultado de las opciones de imagen sin ventana: `snes_play <rom> --no-window --frames 700 --opt filter=sharp --present-shot out\p.png --present-size 640x560`. Con efectos desactivados y el tamaño de la propia imagen, la salida es idéntica al búfer byte a byte (lo dice por consola).
- La SRAM se guarda en `<rom>.srm` (cada segundo si cambió y al salir) y se recarga al arrancar.
- `--stats` muestra ritmo, cola de audio y parones.

### Pruebas sin ventana

El runtime sin vídeo ejecuta la ROM completa y sirve para depurar y automatizar:

```
cl /MP /TC /std:c11 /DSNES_RT_TICK /DSNES_RT_CHECKS out\rom\*.c /Fe:out\headless.exe /Fo:out\rom\
out\headless.exe roms\alttp_usa.sfc --frames 1200 --screenshot out\shot.png --wav out\audio.wav
```

Opciones útiles: `--press BOTONES@A-B` (mando), `--poke ADDR=BYTES@N` (escribir en WRAM), `--dump DIR` (VRAM, CGRAM, OAM, WRAM), y con `/DSNES_RT_TRACE` las trazas `--trace-frames`, `--trace-reads` y `--watch`. `snes_play --no-window` ofrece lo mismo con el frontal de SDL2.

## Estructura

| Ruta | Contenido |
|---|---|
| `src/rom.rs`, `src/games.rs` | carga de ROM, mapeo LoROM, identificación por CRC32 |
| `src/opcodes.rs`, `src/disasm.rs` | tabla de opcodes y decodificador |
| `src/discover.rs`, `src/graph.rs` | descubrimiento de código, bloques y funciones |
| `src/codegen.rs` | generador de C |
| `src/config.rs`, `games/*.toml` | configuración por juego: entradas y tablas de saltos |
| `runtime/snes_rt.h` | interfaz entre el C generado y el runtime |
| `runtime/snes_core.inc`, `snes_ppu.inc`, `snes_spc.inc` | núcleo de hardware, PPU y sonido |
| `runtime/headless.c`, `runtime/play.c`, `runtime/play_cfg.h` | frontales sin vídeo e interactivo, y la lógica de opciones sin SDL (INI, imagen, efectos, volumen, teclas) |
| `src/headless.rs`, `src/spctest.rs`, `src/cctest.rs` | pruebas del runtime y utilidades de compilación |
| `openspec/` | especificaciones y cambios (spec-driven) |

## Desarrollo

El proyecto sigue un flujo con OpenSpec (`/opsx:explore`, `/opsx:propose`, `/opsx:apply`, `/opsx:archive`); el contexto está en `openspec/config.yaml`. Reglas: código propio y *clean-room* (no se copia código de otros recompiladores), tablas de saltos comprobadas a mano contra la ROM real, y nada derivado de la ROM se sube al repositorio.
