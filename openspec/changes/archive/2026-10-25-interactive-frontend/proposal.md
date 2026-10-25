# Proposal

## Why

El C generado ya ejecuta A Link to the Past entero (intro, casa, overworld, castillo, mazmorras y Mundo Oscuro) sin paradas, pero solo en el runtime sin vídeo ni audio: la imagen es un PNG del estado final, el mando se simula con `--press` y el procesador de sonido es un simulacro de handshake. El objetivo del proyecto es poder jugar, así que hace falta un frontend interactivo: ventana en tiempo real, mando en vivo, imagen correcta línea a línea con todos los modos de la PPU, sonido y partidas guardadas.

## What Changes

- Nuevo ejecutable interactivo `snes_play` (mismo C generado, otro runtime) que abre una ventana SDL2, ejecuta la ROM a 60 fotogramas por segundo y muestra cada fotograma.
- La PPU se dibuja **línea a línea** durante el fotograma, registrando las escrituras a `$2100-$213F`, VRAM, CGRAM y OAM con su línea, de modo que los efectos de raster (IRQ, HDMA, cambios de scroll a mitad de pantalla) se vean como en hardware. Se completan los modos 0 y 2-6 (incluido BG4, mosaico, offset por tile y los modos de 512 de ancho/entrelazado), y se comparte el mismo dibujador con `--screenshot`.
- Entrada en vivo: teclado y mando SDL se traducen al joypad 1 (y 2) a través de los mismos registros `$4016-$4017` y `$4218-$421F`.
- Audio: se emulan la CPU SPC700 y el DSP (clean-room), con la subida real de programas y datos por los puertos `$2140-$2143`, y la salida se envía a SDL a 32 kHz. Sustituye al simulacro de `apu_write` en el runtime interactivo.
- SRAM persistente: la SRAM de la ROM se guarda en un fichero `.srm` junto a la ROM al salir y periódicamente, y se carga al arrancar, para conservar las partidas.
- Reloj más fiel: el fotograma deja de ser un número fijo de bloques; se reparte por líneas (262) para sincronizar NMI, IRQ, HDMA y el dibujo por línea.
- Se añaden herramientas de comprobación: volcado de la imagen y de la salida de audio a fichero (`--screenshot`, `--wav`) para verificar sin ventana.

## Capabilities

### New Capabilities
- `ppu-scanline-render`: imagen de la PPU calculada línea a línea con los registros vigentes en cada línea, para todos los modos de fondo (0-7), sprites, ventanas, mosaico, color math y HDMA.
- `live-video-window`: ventana SDL2 con el fotograma en pantalla, escalado entero, sincronización a 60 Hz y teclas de control (pantalla completa, salir, pausa, avance rápido).
- `live-input`: traducción de teclado y mandos físicos a los botones del joypad 1 y 2 en tiempo real.
- `spc700-audio`: CPU SPC700, DSP y los puertos de comunicación, con salida de audio en tiempo real y a fichero WAV.
- `sram-persistence`: carga y guardado de la SRAM de la cartucho en un fichero junto a la ROM.

### Modified Capabilities
- `ppu-render`: la captura usa el dibujador por línea; desaparecen los avisos de modos, ventanas y mosaico «no dibujados» salvo para lo que siga sin existir (modos de entrelazado de alta resolución en la captura).
- `joypad-input`: además de `--press`, el estado de los botones puede venir de una fuente en vivo; sin ella, el comportamiento actual no cambia.

## Impact

- Código nuevo en `runtime/` (dibujador de la PPU, SPC700/DSP, capa SDL, SRAM) y cambios en `runtime/headless.c` para compartir PPU y reloj; `codegen --all` escribirá también `snes_play.c` y los ficheros de runtime nuevos.
- Dependencia nueva solo para el ejecutable interactivo: SDL2 (binario de desarrollo para MSVC, fuera del repositorio; no se incrusta ni se redistribuye). El runtime sin vídeo y `cargo test` siguen sin necesitarla y los tests de SDL se omiten si falta.
- El ROM y el C generado siguen sin subirse; los `.srm` y los WAV de prueba van a `out/` o junto a la ROM, ya ignorados.
- Fuera de alcance: precisión de ciclos del 65816, el coprocesador de otros cartuchos, guardado de estados (save states), reescalado con filtros, soporte de otros juegos.
