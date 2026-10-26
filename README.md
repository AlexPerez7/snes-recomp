# snes-recomp

Recompilador estático de ROMs de SNES (CPU 65816) escrito en Rust desde cero. Traduce la ROM a C, que se compila junto a un runtime propio (bus, DMA/HDMA, PPU, SPC700 y DSP, entrada y partidas guardadas) para obtener un ejecutable nativo.

> **Aviso legal.** Este repositorio no contiene ninguna ROM ni recurso de ningún juego, ni lo tendrá. Cada usuario debe aportar su propia copia obtenida legalmente. El C generado deriva de la ROM, así que va a `out/` (ignorado por git) y no debe subirse ni distribuirse. Proyecto independiente, sin relación con Nintendo ni con ningún titular de derechos de los juegos.

Este repositorio es el **motor**: recompilador, runtime y `snes-port`. No incrusta ningún juego; cada juego es un *perfil* (`games/<id>.toml` y, si lo necesita, C propio) que vive en su propio repositorio. El primero es *The Legend of Zelda: A Link to the Past* (USA, CRC32 `777AAC2F`): [`alttp-recomp`](https://github.com/AlexPerez7/alttp-recomp), que ya se juega en una ventana con imagen, sonido, teclado/mando y partidas guardadas.

Licencia: [MIT](LICENSE) (solo el código de este repositorio; no da derechos sobre ningún juego). Para contribuir, mira [CONTRIBUTING.md](CONTRIBUTING.md).

## Cómo funciona

```
ROM ──► carga y cabecera ──► descubrimiento de código ──► grafo de funciones y bloques ──► generador de C
                              (descenso recursivo,         (aristas, tail calls,           (una función por función,
                               tablas de saltos,            diagnósticos)                   una etiqueta por bloque)
                               anchos M/X)                                                          │
                                                                                                    ▼
                                         runtime (C): bus LoROM · DMA/HDMA · PPU · SPC700+DSP · entrada · SRAM ──► ejecutable
```

- **Descubrimiento:** sigue el código desde los vectores y las entradas del perfil del juego (`games/<id>.toml`), resuelve saltos indirectos con tablas comprobadas a mano contra la ROM y sigue los anchos de los registros M/X por contexto de función.
- **Generación:** cada función del grafo es una función C; un bloque alcanzado con anchos distintos se emite una vez por combinación. Los retornos no locales (descartar una dirección con `PLA`/`PLA` antes de `RTS`) funcionan porque cada llamada comprueba la pila y la dirección de retorno al volver.
- **Runtime:** el C generado habla con el runtime a través de `runtime/snes_rt.h`. El runtime se reparte en un núcleo de hardware, la PPU, el sonido y dos frontales (uno sin vídeo para pruebas y otro interactivo con SDL2).

## Estado

| Área | Estado |
|---|---|
| Carga de ROM (LoROM, cabecera de copiador, CRC32, vectores) | hecho |
| Decodificador 65816 y desensamblador, con seguimiento de M/X | hecho |
| Descubrimiento de código y tablas de saltos | hecho (ALttP: 0 saltos sin resolver) |
| Grafo de funciones y bloques | hecho |
| Generador de C de toda la ROM | hecho: con el perfil de ALttP genera 4 176 de 4 178 funciones; 2 quedan como *stub* (`01:CA17`, `08:A8E3`) y no se ejecutan |
| Bus, DMA y HDMA | hecho |
| PPU por línea | hecho: modos 0 a 7, sprites, ventanas, mosaico, color math, HDMA, modo 7 |
| SPC700 y DSP | hecho: 256 instrucciones, temporizadores, ROM de arranque, voces BRR, ADSR/GAIN, ruido, eco |
| Frontal interactivo (SDL2) | hecho: ventana, pantalla completa, teclado y mandos, audio, `.srm` |
| Constructor de port desde la ROM del usuario (`snes-port`) | hecho: caché por CRC32, MSVC/gcc/clang o TinyCC descargado con consentimiento |
| Opciones de port (menú F1, `config.ini`, filtros) | hecho |
| Perfiles de juego (entradas, tablas, ganchos y C propio) | hecho: viven en el repositorio de cada juego |
| Pantalla ancha (`wide`, hasta 96 px por lado) | hecho en el motor, con API para el C del juego; el relleno específico de ALttP (franjas negras donde no hay mapa) está en curso en el cambio `alttp-wide-fill` y la precarga real del exterior queda como trabajo futuro |

Validación: más de 200 pruebas (algunas compilan y ejecutan el C generado; las que necesitan compilador, SDL2 o la ROM se omiten solas si faltan) y suites de *fuzz* de 30 000 fotogramas por mazmorras, overworld y Mundo Oscuro sin paradas ni *stubs* ejecutados.

Limitaciones conocidas: la interpolación del DSP usa un núcleo en forma de campana, cercano a la gaussiana del hardware pero no su tabla exacta; la PPU no emula el entrelazado; los comportamientos dependientes de ciclos exactos no se reproducen (el reloj es proporcional por bloques).

## Requisitos

- Rust estable (`cargo`).
- Un compilador de C: `gcc`, `clang` o Visual Studio Build Tools (en Windows, desde "x64 Native Tools" o tras `vcvars64.bat`).
- Para el frontal interactivo: el paquete de desarrollo de SDL2 para Visual C++ (`SDL2-devel-x.y.z-VC.zip`), descomprimido en `out/sdl2` o apuntado por `SDL2_DIR`.
- Tu ROM en `roms/` (ignorada por git), por ejemplo `roms/alttp_usa.sfc`.
- El **perfil del juego**, que sale del repositorio de ese juego (para ALttP, `alttp-recomp`): su carpeta `games/` va junto a `snes-port.exe` o en el directorio desde el que lo ejecutas, o se pasa con `--game <perfil.toml>`. Sin perfil, `snes-port` dice dónde busca y no construye nada.

## Uso

En los ejemplos, `games/alttp_usa.toml` es el perfil del repositorio `alttp-recomp`; si lo tienes en `games/` no hace falta `--game`.

### Análisis

```
cargo run --release -- info roms/alttp_usa.sfc                 # cabecera, CRC y vectores
cargo run --release -- disasm roms/alttp_usa.sfc 00:8000 40
cargo run --release -- discover roms/alttp_usa.sfc --game games/alttp_usa.toml   # --unresolved, --widths
cargo run --release -- graph roms/alttp_usa.sfc [00:8000] --game games/alttp_usa.toml   # resumen, o la función que contiene la dirección
cargo run --release -- codegen roms/alttp_usa.sfc 00:8000 --depth 1 -o out/reset.c --game games/alttp_usa.toml
cargo run --release -- codegen roms/alttp_usa.sfc --all -o out/rom --game games/alttp_usa.toml   # C de toda la ROM + runtime
cargo run --release -- scan roms/alttp_usa.sfc <carpeta>        # falla si algo de la carpeta lleva bytes de la ROM (antes de publicar)
cargo test
```

### Jugar con `snes-port` (lo más simple)

`snes-port` construye el juego en tu máquina a partir de **tu** ROM y lo arranca. No se distribuye ningún juego: el ejecutable se genera aquí, la primera vez (unas decenas de segundos con MSVC; algo más con gcc), y las siguientes arranca al instante. Lo más cómodo es el paquete del repositorio del juego (`alttp-recomp-<versión>-win64.zip`), que ya trae `snes-port.exe`, SDL2 y el perfil.

```
snes-port roms\alttp_usa.sfc [--game games\alttp_usa.toml]   # construye (si hace falta) y juega; también vale arrastrar la ROM sobre snes-port.exe, o abrirlo con doble clic para elegirla en una ventana (recuerda la última carpeta)
snes-port roms\alttp_usa.sfc -- --scale 3 --mute   # lo que va tras -- se pasa al juego
snes-port roms\alttp_usa.sfc --no-launch           # solo construye
snes-port clean [roms\alttp_usa.sfc]               # borra la caché de esa ROM (o toda); nunca toca ROMs ni partidas
```

- **Qué se queda dónde.** El C generado y el ejecutable viven en la caché (`%LOCALAPPDATA%\snes-port`, o `--cache <carpeta>` / `SNES_PORT_CACHE`), en una carpeta `<crc32>-<versión>`; cambiar el runtime reconstruye solo. El ejecutable lee la ROM del sitio donde está, y la partida guardada (`.srm`) queda junto a ella. Tras construir se comprueba que ningún byte de la ROM ha acabado en lo construido.
- **Compilador.** Usa `CC`, gcc, clang o Visual Studio, y los ficheros de desarrollo de SDL2 (`SDL2_DIR`, o la carpeta `sdl2` junto a `snes-port.exe`, que el paquete de `scripts\release.ps1` ya incluye). Con uno local **nunca se descarga nada**. Sin compilador pregunta si descargar TinyCC (480 KiB, desde `download.savannah.gnu.org`, con SHA-256 fijado en el programa; si no coincide se descarta); `--download-compiler` responde que sí de antemano. Es lo único que usa la red.
- **Perfiles.** Busca `*.toml` (con su carpeta hermana de C, si la hay) en `games/` del directorio actual y en `games/` junto al ejecutable; el del directorio actual manda si coinciden, y `--game` (repetible) manda sobre ambos. Un `.toml` roto se avisa y se omite; dos perfiles para el mismo CRC32 en un mismo sitio son un error.
- **Paquete del motor.** `powershell -File scripts\release.ps1` crea `out\release\snes-port-<versión>-win64.zip` con el programa, este README, la licencia y SDL2 (sin ningún juego). Falla (y no crea el zip) si encuentra `.sfc`, `.smc`, `.srm` o C generado. Cada juego arma el suyo con su propio `release.ps1`.
- **Códigos de salida.** 2: ROM no reconocida (se listan las admitidas, o dónde se buscan los perfiles si no hay ninguno); 3: falta compilador o SDL2; 4: la construcción falló.

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
- F1 abre un menú con aspecto de menú de juego (tipografía y marco propios) que pausa la emulación, aplica los cambios al instante y los guarda al cerrarlo. Cuatro pestañas: **Image** (pantalla completa, VSync, aspecto, escalado, filtro, recorte, scanlines, rejilla, contador de fps), **Audio** (volumen y dispositivo de salida), **Controls** (jugador y botón: se pulsa la tecla o el botón del mando que se quiere asignar; si ya está en uso avisa y ofrece reemplazar con Intro o cancelar con Esc) y **Game** (continuar, guardar la partida ahora, restaurar todo a los valores por defecto, salir). Cada pestaña de opciones tiene su «Restore defaults». Teclado: flechas para moverse y cambiar el valor, Intro elige, Q/W (o RePág/AvPág) cambian de pestaña, Esc o F1 cierran. Mando: cruceta, A elige, B cierra, L/R cambian de pestaña.

| Clave | Valores (por defecto el primero) | Efecto |
|---|---|---|
| `fullscreen` | `false`, `true` | pantalla completa sin bordes (F11 la alterna) |
| `vsync` | `false`, `true` | sincronía vertical del renderizador |
| `aspect` | `8:7`, `4:3`, `free` | píxel cuadrado, píxel 7/6 más ancho como en un televisor, o la imagen llena toda la ventana |
| `scale` | `3` (1 a 8) | tamaño inicial de la ventana |
| `scaling` | `integer`, `fit` | múltiplos enteros, o relleno del área disponible |
| `filter` | `nearest`, `bilinear`, `sharp` | `sharp` (por defecto) escala por un factor entero y suaviza solo el resto; con factor entero es idéntico a `nearest` |
| `overscan` | `0` (0 a 8) | líneas recortadas arriba y abajo |
| `wide` | `0` (0 a 96, de 8 en 8) | píxeles de imagen extra a cada lado de los 256 de la consola; la zona nueva muestra lo que dibuja la PPU (los fondos continúan por su mapa), así que puede verse vacía o con restos; el C de un juego puede leer el valor (`ppu_wide()`) y tapar en negro lo que no tenga cargado (`ppu_wide_blank()`) |
| `scanlines` | `0` (0 a 100) | oscurece una de cada dos líneas de pantalla en ese porcentaje |
| `grid` | `false`, `true` | rejilla entre los píxeles de la consola |
| `fps` | `false`, `true` | muestra los fotogramas por segundo en la esquina de la imagen |
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

Opciones útiles: `--press BOTONES@A-B` (mando), `--poke ADDR=BYTES@N` (escribir en WRAM), `--dump DIR` (VRAM, CGRAM, OAM, WRAM), `--wide N` (captura ensanchada en N columnas por lado, múltiplo de 8 hasta 96), y con `/DSNES_RT_TRACE` las trazas `--trace-frames`, `--trace-reads` y `--watch`. `snes_play --no-window` ofrece lo mismo con el frontal de SDL2.

## Estructura

| Ruta | Contenido |
|---|---|
| `src/rom.rs`, `src/games.rs` | carga de ROM, mapeo LoROM, búsqueda e identificación de perfiles por CRC32 |
| `src/opcodes.rs`, `src/disasm.rs` | tabla de opcodes y decodificador |
| `src/discover.rs`, `src/graph.rs` | descubrimiento de código, bloques y funciones |
| `src/codegen.rs` | generador de C |
| `src/config.rs` | formato de los perfiles de juego (`games/<id>.toml`): entradas, tablas de saltos y ganchos |
| `src/port.rs`, `src/toolchain.rs`, `src/download.rs` | `snes-port`: caché, compilación y descarga opcional de TinyCC |
| `runtime/snes_rt.h` | interfaz entre el C generado y el runtime |
| `runtime/snes_core.inc`, `snes_ppu.inc`, `snes_spc.inc` | núcleo de hardware, PPU y sonido |
| `runtime/headless.c`, `runtime/play.c`, `runtime/play_cfg.h` | frontales sin vídeo e interactivo, y la lógica de opciones sin SDL (INI, imagen, efectos, volumen, teclas) |
| `src/headless.rs`, `src/spctest.rs`, `src/cctest.rs` | pruebas del runtime y utilidades de compilación |
| `openspec/` | especificaciones y cambios (spec-driven) |

## Desarrollo

El proyecto sigue un flujo con OpenSpec (`/opsx:explore`, `/opsx:propose`, `/opsx:apply`, `/opsx:archive`); el contexto está en `openspec/config.yaml`. Reglas: código propio y *clean-room* (no se copia código de otros recompiladores), tablas de saltos comprobadas a mano contra la ROM real, y nada derivado de la ROM se sube al repositorio. Detalles y cómo probar con un juego real (`SNES_GAME_PROFILE`, `SNES_GAME_ROM`) en [CONTRIBUTING.md](CONTRIBUTING.md).
