# Tasks

## 1. Accesor de E en el motor

- [x] 1.1 Añadir `int ppu_wide(void)` a `runtime/snes_rt.h` y definirlo en `runtime/snes_ppu.inc` (devuelve `wide_x0`); verificar con un test en `src/headless.rs` que el C de prueba recibe 0 sin ensanche, 64 con `--wide 64` y el valor nuevo tras `ppu_set_wide`
- [x] 1.2 Documentar `ppu_wide` junto a `ppu_sprite_xoff` en el comentario de `snes_rt.h` y en CLAUDE.md; verificar que `cargo test` pasa

## 2. Investigar la ROM (resultado: notas en design.md)

- [x] 2.1 Confirmar con volcados de VRAM que en el borde del área la zona envuelta del tilemap muestra el lado opuesto, y anotar las variables de cámara y los límites del área
- [x] 2.2 Investigar la transición entre áreas y la cuadrícula Map16 y decidir entre precarga y franjas negras (decidido: franjas negras; la precarga queda como trabajo futuro en design.md)
- [ ] 2.3 Comprobar en la ROM que `$0604`/`$0606` son los límites izquierdo y derecho (no `$0600`/`$0602`, que son los verticales: ya comprobado, falta el borde real y un área de 1024 px) de la cámara del exterior (con áreas de 512 y de 1024 px) y que `$E2` es la X de la cámara; verificar con `--watch` y capturas `--wide 96` en el borde izquierdo y en el derecho de un área
- [ ] 2.4 Contrastar `$1B` como indicador de interior con una captura de exterior y otra de interior, y localizar una rutina que se ejecute cada fotograma en ambos casos para enganchar el cálculo de franjas; anotar ambas en design.md

## 3. Franjas negras en el motor

- [x] 3.1 Añadir `ppu_wide_blank(int left, int right)` y `ppu_wide_blank_clear()` a `runtime/snes_rt.h` y dibujarlas en `runtime/snes_ppu.inc` (negro en lugar de la PPU, limitadas a `E` y sin tocar los 256 px originales); verificar con tests en `src/headless.rs` de los escenarios de la spec (franja a la izquierda, franja mayor que `E`, sin franjas y con `E = 0` idéntico)
- [x] 3.2 Documentarlas en el comentario de `snes_rt.h`, en CLAUDE.md y en el README del motor si habla de `wide`; verificar que `cargo test` pasa

## 4. Exterior y salas en `alttp-recomp`

- [ ] 4.1 Escribir `games/alttp_usa/widefill.c` con un gancho (declarado en `games/alttp_usa.toml`) que cada fotograma calcule las franjas del exterior con los límites de 2.3 (`$0604`/`$0606`) y las del interior (completas) con `$1B`, y las fije con `ppu_wide_blank`; verificar con `--wide 72` en el borde izquierdo y derecho de un área del exterior, y en una sala y una cueva
- [ ] 4.2 Comprobar `E = 96`, una transición entre áreas y la entrada y salida de una cueva; verificar con capturas que no queda basura en ningún fotograma de la transición y medir el tiempo con `--stats`
- [ ] 4.3 Comprobar `E = 0`: verificar que la captura es idéntica byte a byte a la de antes del gancho

## 5. Cierre

- [ ] 5.1 Añadir a `scripts/check.ps1` de `alttp-recomp` una prueba con `--wide` (borde de exterior e interior) y documentar la opción en su README; verificar que el script pasa
- [ ] 5.2 Ejecutar `snes-recomp scan` sobre `alttp-recomp` antes de cualquier push y `cargo test` en el motor con `SNES_GAME_PROFILE` y `SNES_GAME_ROM`; verificar que ambos terminan sin fallos

## Workflow follow-up

- Archivar el cambio con `/opsx:archive alttp-wide-fill` tras la prueba manual con el mando en `snes_play.exe` con `wide = 72`.
- Cambio futuro: precarga real del exterior (opción A) y piezas 3 y 4.
