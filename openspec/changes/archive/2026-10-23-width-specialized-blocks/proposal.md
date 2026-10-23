# Proposal

## Why

Siete funciones de ALttP siguen siendo stubs porque algún bloque se alcanza con anchos M/X distintos y su C depende de la diferencia (p. ej. `01:D815`: `INC $042C` es de 8 bits con M=8 y de 16 con M=16). Una de ellas, `01:D81B`, se ejecuta al elegir la partida en PLAYER SELECT y detiene el juego con `rt_unsupported`. Elegir un ancho sería incorrecto, así que hace falta emitir cada bloque con los anchos con los que de verdad se ejecuta.

## What Changes

- El descubrimiento conserva, para cada dirección con anchos en conflicto, las variantes `(dirección, anchos)` que ya recorre y las aristas entre ellas.
- El generador de C emite una copia del bloque por cada variante cuyo C o longitud difiere, con etiquetas que incluyen los anchos, y cada salto va a la copia que corresponde a los anchos con los que se llega. Las funciones sin conflictos que cambian el C se generan exactamente igual que hoy.
- Una función ya no se rechaza por «different register widths» salvo que el conflicto no se pueda separar en variantes; en ese caso sigue siendo stub, con el mismo mensaje.
- El runtime sin vídeo simula el latch de los contadores H/V: `$2137` captura, `$213C`/`$213D` devuelven el byte bajo y luego el bit 0, y `$213F` los reinicia y devuelve el bit 6 del latch. Ya está escrito en `runtime/headless.c` sin tests; ALttP lo necesita para salir de un bucle de espera de línea (`00:F3A3`).

## Capabilities

### New Capabilities
- `width-specialized-blocks`: copias de un bloque por combinación de anchos M/X con la que se alcanza, y los saltos entre ellas.

### Modified Capabilities
- `c-code-generation`: el requisito «Anchos M/X por instrucción» deja de fallar en conflictos que se pueden separar en variantes.
- `headless-runtime`: nuevo requisito del latch de contadores H/V.

## Impact

- `src/discover.rs` (variantes y aristas por anchos), `src/graph.rs` (bloques por variante), `src/codegen.rs` (etiquetas, saltos, comprobación de anchos).
- `runtime/headless.c` (latch H/V) y tests en `src/codegen.rs`, `src/discover.rs` y `src/headless.rs`.
- ALttP: las 7 funciones con stub por anchos pasan a traducirse; el juego avanza tras elegir la partida.
- Fuera de alcance: ventanas, HDMA y mosaico del renderizador; conflictos de anchos que no cambian el C (siguen sin duplicar bloques).
