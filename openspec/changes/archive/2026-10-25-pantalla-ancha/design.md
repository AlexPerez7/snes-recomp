# Design

## Context

La PPU del runtime (`runtime/snes_ppu.inc`) dibuja una línea por llamada a `render_row`, con `x` de 0 a `SCREEN_W` (256, constante). Los búferes `spr_col`/`spr_pri`/`fb_back`/`fb_front` y 17 puntos de `play.c` (textura, presentación, captura, menú) dependen de esa constante. `bg_pixel` ya enrolla con `& (width-1)`, `draw_sprites_row` ya trata la X de la OAM como con signo, y `m7_pixel` es aritmética afín. `play_cfg.h` rechaza (no recorta) los valores fuera de rango. Motivación y alcance: ver proposal.md; requisitos: specs.

## Goals / Non-Goals

**Goals:**
- Ancho de línea `W = 256 + 2E` en ejecución, sin recompilar el runtime, y `E = 0` idéntico a hoy byte a byte.
- Cambiar `E` en marcha (menú F1) sin reasignar memoria.

**Non-Goals:**
- Que ningún juego se vea bien en la zona nueva (mapas, activación de objetos, sprites por la derecha: perfiles de juego).
- Alto variable, interlazado, o cambios del reloj de la PPU: la línea sigue costando el mismo tiempo emulado.
- Modos hires con resolución propia mayor: se sigue mezclando a una columna por píxel.

## Decisions

**1. Búfer de ancho máximo y ancho activo `screen_w`.** `SCREEN_W` pasa a ser `SCREEN_W_MAX = 256 + 2·96 = 448` para dimensionar los búferes, y un `screen_w` activo y un `wide_x0 = E` gobiernan los bucles y el paso de fila de `fb_*`. Alternativa descartada: asignación dinámica al cambiar `E`; añade fallos de memoria y sincronización con el hilo de vídeo para ahorrar unos cientos de KiB. Con `E = 0` el paso de fila es 256 y el contenido igual.

**2. Coordenada de pantalla con signo.** `render_row` itera `x` de `-E` a `255+E`; el píxel se escribe en `out[(x+E)·3]`. Los fondos usan `x` directamente (`& (width-1)` ya admite negativos en complemento a dos). En los modos hires `xc = 2x+1` sigue valiendo. Arreglos puntuales con coordenadas negativas: `offset_per_tile` (el `col < 1` deja de proteger la columna 0 de la imagen original: hay que mantener "columna 0 de la pantalla original" y no la primera calculada) y el mosaico, que usa `%` y necesita módulo no negativo.

**3. Sprites: búfer por línea más ancho.** `spr_col`/`spr_pri` pasan a `[SCREEN_H][SCREEN_W_MAX]` indexadas con `sx + E`. Los límites (32 sprites, 34 trozos) se calculan igual que hoy contando sólo los trozos de la imagen original, de modo que el ensanche no altera qué sprites se pierden (spec: Límites de sprites por línea). Alternativa descartada: contar también los trozos de la zona nueva; sería más "puro", pero los sprites que perdiera dependerían del ensanche y el parpadeo cambiaría entre configuraciones.

**4. Desplazamiento de X por sprite en `snes_rt.h`.** Una tabla `int16_t` de 128 entradas, fijada con `ppu_sprite_xoff(i, dx)` y puesta a cero con `ppu_sprite_xoff_clear()`. `draw_sprites_row` suma la entrada a la X ya extendida con signo. Persiste entre fotogramas hasta que el juego la cambia (el juego decide cuándo, porque la OAM se reescribe por DMA cada fotograma). Alternativa: una OAM ampliada con X de 10 bits; obliga a interceptar DMA y la regla de ordenación de la consola, y es mucho más código.

**5. Ventanas con la `x` recortada.** `win_area(layer, x)` y la ventana de color se evalúan con `x` limitada a 0..255. El modo 7 sigue su aritmética (la matriz admite `x` fuera de rango y los mosaicos fuera del mapa ya tienen reglas) y el HDMA es por línea, por lo que ninguno de los dos necesita cambios.

**6. Captura y frontal.** `write_png` recibe el ancho activo. `headless.c` añade `--wide N` y valida `N`: múltiplo de 8, 0 a 96. En `play.c` la textura se crea para el máximo y se presenta el rectángulo `W×lines`; `view_rect` usa `W` en lugar de 256 en el cálculo del aspecto (y `256·7/6` para el 4:3 pasa a `W·7/6`). La opción `wide` en `opt_defs` con paso 8 y rango 0 a 96; el `OptDef` no tiene "múltiplo de", así que un valor intermedio se redondea hacia abajo al leerlo. El menú dibuja sobre un lienzo de 256×224, que se queda así (el menú no se ensancha).

**7. Caché de `snes-port`.** La versión de la caché ya incluye el CRC32 del runtime, de modo que el cambio invalida las compilaciones anteriores sin trabajo extra.

## Risks / Trade-offs

- [Coordenadas negativas en código escrito pensando en 0..255 (`offset_per_tile`, mosaico, índices de `spr_col`)] → pruebas con `E = 0` byte a byte contra las capturas actuales, y con `E > 0` comprobando que las columnas 0..255 de la captura son idénticas a las de `E = 0`.
- [Basura visible en la zona nueva con ALttP, hasta que el perfil lo arregle] → `E = 0` por defecto; el README lo avisa; es una consecuencia aceptada en la decisión de mostrar lo que dibuja la PPU.
- [Coste: más píxeles por línea, hasta un 75 % con `E = 96`] → medido en el volcado sin vídeo de ALttP (2000 fotogramas, MSVC `/O2`): 7,8 s con `E = 0` y 13,9 s con `E = 96`, un 78 % más, porque el dibujador por píxel domina el tiempo de un fotograma (unos 4 ms con `E = 0`, unos 7 ms con `E = 96`). Cabe en los 16,6 ms de 60 Hz, así que no se optimiza aquí; si hiciera falta, el sitio es `bg_pixel`/`tile_pixel`.
- [El hilo de vídeo lee `fb_front` mientras `screen_w` cambia] → `screen_w` se fija al empezar cada fotograma (`ppu_frame_end` lo adopta) y la presentación lee el ancho junto con el búfer.

## Migration Plan

Aditivo y apagado por defecto: sin `--wide` ni `wide`, nada cambia. Marcha atrás: poner `wide = 0` o revertir el cambio; no hay datos persistentes salvo la clave nueva de `config.ini`, que las versiones anteriores ignoran (conservan claves desconocidas).

## Open Questions

- Si merece la pena un preajuste "16:9" (`E = 72`, 400×224, 1,79:1) en el menú además del valor numérico. Se puede decidir al implementar la pestaña Image.
