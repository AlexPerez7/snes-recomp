## Why

El menú del frontal (Esc o F1) está escrito solo en inglés, aunque el jugador haya elegido `language = es` para el juego: «Image», «Fullscreen», «Restore defaults», «Quit». Con ALttP ya traducible, el menú es lo que queda en inglés. El menú se dibuja con una fuente propia del motor, sin nada de la ROM, así que se puede traducir y dotar de tildes sin depender de la fuente del juego.

## What Changes

- El menú se muestra en español cuando una opción de juego del perfil con clave `language` vale `es`; en cualquier otro caso (sin esa opción, otro valor) sigue en inglés. Cambiar la opción desde la pestaña Game actualiza el menú al instante.
- Tabla de cadenas en español dentro del motor (original, sin datos de ROM), con las etiquetas, los mensajes de aviso, la ayuda del pie y los nombres de las pestañas y de los valores que se muestran.
- La fuente del menú gana los caracteres del español (`á é í ó ú ü ñ ¿ ¡` y las mayúsculas que haga falta) y el texto del menú se decodifica como UTF-8: medida, dibujo y recorte con «...» no parten un carácter.
- **Solo cambia lo que se dibuja**: las claves y los valores de `config.ini` (`nearest`, `sharp`, `false|true`…), los comentarios del fichero, la salida de consola y `snes-port` siguen en inglés, para que los ficheros existentes se sigan leyendo igual.
- Pruebas: cada cadena en español cabe en su sitio, el menú con `es` se dibuja distinto que con `en`, y los textos del español se miden y se recortan bien.

## Capabilities

### New Capabilities

- `menu-language`: idioma del texto del menú del frontal, de dónde sale, qué se traduce y qué no, y que todo cabe.

### Modified Capabilities

- `menu-ui`: requisito «Tipografía propia»: la fuente cubre también los caracteres del español y el texto se trata como UTF-8.

## Impact

- `runtime/play_ui.h` (glifos nuevos, decodificación UTF-8 en medida, dibujo y recorte), `runtime/play.c` (tabla de cadenas y su uso en las filas, pestañas, avisos y ayuda), `runtime/play_cfg.h` (nombres de valores que se muestran), `src/headless.rs` (pruebas).
- README y `CLAUDE.md`: el menú sigue la opción `language` del juego.
- Los juegos que usan `snes-port` reciben el cambio con la siguiente versión del motor; su caché se reconstruye una vez.
- Fuera de alcance: la fuente del juego (tiles de la ROM), el menú de pausa de ALttP, otros idiomas, una opción de idioma propia del frontal y la traducción de `config.ini` o de la consola.
