# Tasks

## 1. Ancho de línea variable en la PPU

- [x] 1.1 Sustituir `SCREEN_W` por `SCREEN_W_MAX` (448) para los búferes y un ancho activo `screen_w` y un `wide_x0` para los bucles y el paso de fila de `fb_back`/`fb_front`/`spr_*`; verificar que `cargo test` sigue pasando y que con `E = 0` la captura de ALttP (fotograma 2000, `--screenshot`) es idéntica byte a byte a la de antes del cambio
- [x] 1.2 Hacer que `render_row` itere `x` de `-E` a `255+E` y escriba en `out[(x+E)·3]`, y que `ppu_frame_end` adopte el ancho nuevo al empezar cada fotograma; verificar con una prueba en `src/headless.rs` (fondo con patrón distinto por columna, `E = 32`) que a la izquierda de la columna 0 se ven las teselas 31, 30… del mapa
- [x] 1.3 Corregir `offset_per_tile` y el mosaico para `x` negativo (columna 0 de la imagen original en vez de la primera calculada; módulo no negativo); verificar con pruebas de modo 2/4 y de mosaico que las columnas 0..255 son iguales con `E = 0` y con `E = 32`
- [x] 1.4 Evaluar `win_area` y la ventana de color con `x` recortada a 0..255; verificar con las dos situaciones de la spec (ventana que cubre el borde, ventana que no llega a él)

## 2. Sprites en la zona nueva

- [x] 2.1 Ampliar `spr_col`/`spr_pri` al ancho máximo (índice `sx + E`) y dibujar sprites con `sx` desde `-E` hasta `255+E`, contando para los límites (32 sprites, 34 trozos) sólo los trozos de la imagen original; verificar con la prueba de la línea cargada que se pierden los mismos trozos con `E = 0` y con `E = 64`
- [x] 2.2 Añadir en `snes_rt.h`/`snes_core.inc` `ppu_sprite_xoff(i, dx)` y `ppu_sprite_xoff_clear()` y sumarlo en `draw_sprites_row`; verificar con las pruebas de la spec (sprite con X 0 y desplazamiento +256 aparece en la columna 256; sin desplazamientos X 300 sigue en -212)
- [x] 2.3 Probar a mano que un sprite con X igual a -16 se ve completo a la izquierda con `E = 32` (captura de una ROM de prueba mínima o de ALttP con `--poke`), y anotar en la nota de memoria el resultado

## 3. Frontal sin vídeo

- [x] 3.1 Añadir `--wide N` a `headless.c` (múltiplo de 8, 0 a 96; rechaza el resto con mensaje y salida distinta de 0) y hacer que `write_png`/`--screenshot` usen el ancho activo; verificar las tres escenas de la spec (sin ensanche 256×224, `--wide 64` 384×224 con la imagen original en las columnas 64 a 319, valor no válido) con pruebas en `src/headless.rs`
- [x] 3.2 Comprobar con ALttP real que `--wide 0` y `--wide 64` dan las mismas columnas 64..319 y 0..255 respectivamente (comparación de las capturas del fotograma 2000), y medir con `--stats` el coste de `E = 96`; anotar el resultado en design.md si supera el 15 % de tiempo por fotograma

## 4. Frontal interactivo

- [x] 4.1 Añadir la opción `wide` a `opt_defs` (clave, rango 0 a 96, paso 8, por defecto 0) y su lectura de `config.ini`/`--opt` con redondeo hacia abajo a múltiplo de 8; verificar con las pruebas de `play_cfg.h` los escenarios de valor por defecto, fuera de rango y no múltiplo de 8
- [x] 4.2 Adaptar `play.c` (textura al máximo, presentación `W×lines`, `view_rect` y 4:3 con `W`, overlays de scanlines y rejilla, captura F10, ventana inicial) y la pestaña Image del menú; verificar con `--no-window --present-shot` y `--present-size` los números del escenario 8:7 a 1920x1080 con `wide = 72` (400×224 a 4x, 1600x896) y con `wide = 0` que la imagen es idéntica a la actual
- [x] 4.3 Cambiar `wide` con el menú en marcha (`--menu-keys` con `--print-config`) y verificar que el fotograma siguiente sale con el ancho nuevo y que el juego sigue en el mismo punto

## 5. Documentación e integración

- [x] 5.1 Documentar `--wide`, la opción `wide`, la zona nueva (lo que dibuja la PPU, con avisos de huecos) y las funciones de desplazamiento de sprites en `CLAUDE.md` y en el README del motor; verificar que los comandos documentados se ejecutan tal cual
- [x] 5.2 Ejecutar `cargo test` completo, con el juego real (`SNES_GAME_PROFILE`, `SNES_GAME_ROM`) y sin él, y la construcción de `snes-port` desde cero con MSVC; verificar que todo pasa y que `scan` de la carpeta de publicación no encuentra bytes de la ROM
- [x] 5.3 Probar a mano en la ventana con ALttP y `wide = 72`: el juego arranca, la imagen original está centrada, la zona nueva muestra lo que dibuja la PPU y volver a `wide = 0` deja la imagen de siempre

## Workflow follow-up

- Archivar el cambio con `/opsx:archive pantalla-ancha` cuando 5.3 esté probada a mano.
- Después, en `alttp-recomp`, abrir cambios propios para arreglar mapas del exterior, activación de objetos y sprites por la derecha usando `ppu_sprite_xoff`.
