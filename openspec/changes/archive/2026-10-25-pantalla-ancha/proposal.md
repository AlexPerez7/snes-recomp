# Proposal

## Why

La imagen del motor mide siempre 256×224, el tamaño de la SNES, y en una pantalla actual eso deja barras negras a los lados. Los fondos de la SNES se enrollan por su mapa y los sprites usan una coordenada que ya admite valores negativos, así que la PPU del motor puede dibujar un campo más ancho sin tocar el juego. Este cambio da al motor esa capacidad como una opción (`E` píxeles extra por lado, 0 por defecto); que cada juego tenga algo bueno que mostrar en la zona nueva se resuelve después, en el perfil de ese juego.

## What Changes

- El dibujador de la PPU (`snes_ppu.inc`) pasa de un ancho fijo a un ancho de línea de `256 + 2·E` píxeles, con `E` entre 0 y 96 en pasos de 8. Con `E = 0` la imagen, las capturas y el volcado son idénticos a los de hoy.
- En la zona nueva se muestra lo que dibuja la PPU: los fondos continúan por su mapa (que se enrolla) y los sprites se dibujan donde caigan. No se rellena ni se repite el borde.
- Las ventanas (`$2126-$2129`) se evalúan con la coordenada recortada a 0..255; el modo 7 y el HDMA siguen su propia aritmética, que ya admite coordenadas fuera de la imagen.
- Los sprites a la izquierda de la imagen salen solos (su X ya es negativa). Para los de la derecha, el runtime ofrece al C del juego un desplazamiento de X por sprite (`snes_rt.h`), que el perfil usará más adelante; sin él, un sprite con X de 256 a 511 sigue interpretándose como negativo, como en la consola.
- Se mantienen los límites de la consola por línea (32 sprites y 34 trozos de ocho píxeles), de modo que ensanchar no cambia qué sprites se pierden.
- `headless` acepta `--wide N` y las capturas (`--screenshot`) salen de `256 + 2N` píxeles de ancho.
- `snes_play` añade la opción `wide` (clave de `config.ini` y `--opt wide=N`, pestaña Image del menú F1) y adapta presentación, aspecto, escalado, efectos y capturas F10 al ancho real. Cambiarla con el juego en marcha actúa en el fotograma siguiente.
- No cambia el perfil de ningún juego: los arreglos de ALttP (mapas del exterior, activación de objetos, sprites por la derecha) quedan para cambios posteriores en `alttp-recomp`.

## Capabilities

### New Capabilities
- `wide-display`: campo de imagen ensanchado en `E` píxeles por lado (qué se dibuja en la zona nueva, ventanas recortadas, límites de sprites por línea, desplazamiento de X de los sprites para el C del juego y opción `--wide`).

### Modified Capabilities
- `ppu-render`: la captura deja de medir siempre 256×224; mide `256 + 2E` × 224 y con `E = 0` es la de siempre.
- `display-options`: nueva opción `wide`, su rango, su efecto sobre el rectángulo de la imagen y su aparición en el menú.

## Impact

- `runtime/snes_ppu.inc`: `SCREEN_W` deja de ser constante (máximo de memoria fijo y ancho activo variable), con `render_row`, `draw_sprites_row`, `win_area` y los búferes `spr_col`/`spr_pri`/`fb_*` adaptados.
- `runtime/snes_rt.h` y `runtime/snes_core.inc`: desplazamiento de X por sprite.
- `runtime/headless.c`: opción `--wide`.
- `runtime/play.c`, `runtime/play_cfg.h`: opción `wide`, textura, `view_rect`, captura y menú.
- Pruebas en `src/headless.rs` (capturas con `E = 0` iguales a las actuales, `E > 0` con ancho correcto y fondos continuados) y la tabla de opciones de `play_cfg.h`.
- Sin dependencias nuevas, sin cambios en el formato de los perfiles ni en la caché de `snes-port` más allá de la nueva versión de runtime.
