## Why

La pestaña Controles (Mandos en español) del menú F1 es la menos legible del menú. Con ALttP en español se ve: la fila «Controles de» queda cortada, cada botón muestra los nombres crudos de SDL pegados en una sola cadena (`Up,dpup`, `Left|dpleft`, `Right Shift`, `Return`) y el separador se pierde, de modo que se lee «Updpup». Teclado y mando comparten el mismo hueco de 112 px, así que lo largo se recorta con «...», y nada de eso se traduce. Reasignar un botón exige adivinar qué es cada cosa.

## What Changes

- Los botones se muestran con nombres propios y legibles en lugar de los de SDL: teclas como «Flecha ↑», «Intro», «Mayús der.», «Espacio», y botones de mando como «A», «LB», «Cruceta ↑», «Select», con respaldo al nombre de SDL para las teclas raras. En inglés: «Up Arrow», «Enter», «R Shift».
- Teclado y mando se dibujan en dos columnas con su cabecera («Teclado» y «Mando»), en lugar de una cadena con `,` y ` | `; varias asignaciones de un mismo botón se separan con un carácter que la fuente sí dibuja.
- La etiqueta de cada fila es el botón de la consola (con su letra y, en español, su sentido: «Arriba», «Abajo», «Izquierda», «Derecha»; los de letras se quedan como son), y la fila del jugador no se corta.
- El texto que no cabe se recorta siempre con «...», también las etiquetas, y el panel no se sale ni se pisa con otra columna.
- Los nombres se traducen con la misma regla que el resto del menú: solo al dibujar, nunca lo que va a `config.ini` (`p1.key.A = Space`).
- La fuente gana las flechas `← ↑ → ↓` y los signos que falten (separador).
- Pruebas con captura de la pestaña en inglés y en español, sin recortes.

## Capabilities

### New Capabilities

- `menu-controls`: cómo se muestran y se editan las asignaciones de teclas y botones en la pestaña Controles del menú.

### Modified Capabilities

(ninguna: `menu-language` y `menu-ui` ya cubren el idioma y la tipografía; los requisitos nuevos viven en `menu-controls`)

## Impact

- `runtime/play.c` (`bind_name`, `bind_text`, `menu_row`, la rama `TAB_CONTROLS` de `menu_draw`), `runtime/play_ui.h` (flechas y separador), `runtime/play_i18n.h` (nombres de teclas, botones y cabeceras), `src/headless.rs` (pruebas).
- README y `CLAUDE.md`: descripción de la pestaña.
- `config.ini` no cambia de formato ni de contenido.
- Los juegos que usan `snes-port` reciben el cambio con la siguiente versión del motor.
- Fuera de alcance: remapeo de ejes analógicos, perfiles por mando, detección del tipo de mando (Xbox/PlayStation), otros idiomas.
