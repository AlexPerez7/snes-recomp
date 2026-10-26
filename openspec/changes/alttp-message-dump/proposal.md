## Why

Traducir ALttP al español (`idioma-espanol`) necesita antes una tubería de mensajes que se pueda comprobar: sacar los 397 textos del juego en un formato legible, poder sustituirlos desde un fichero y demostrar que esa sustitución, con el texto original, no cambia nada. Hoy el juego no ofrece ni lo uno ni lo otro, y al motor le faltan dos datos para hacerlo bien: el C del juego no sabe dónde está la carpeta de su perfil (donde irán sus ficheros de idioma) ni la de la ROM (donde debe caer el volcado, que lleva texto de Nintendo y no puede subirse), porque el juego arranca con el directorio de trabajo en la caché.

## What Changes

- **Motor:** `game_dir()` y `rom_dir()` en `snes_rt.h`; la opción `--game-dir` en el runtime sin vídeo y en el interactivo; `GameProfile` recuerda la carpeta de su perfil y `snes-port` la pasa al lanzar el juego. Los ficheros de esa carpeta que no son `.c` ni `.h` (los de idioma) no entran en la versión de la caché, así que editarlos no reconstruye nada.
- **Juego (`alttp-recomp`):**
  - Volcado de los 397 mensajes con `--game-opt dump_text=1` a `<carpeta de la ROM>/alttp_messages_en.txt`, en un formato de texto con las letras y signos conocidos tal cual y todo lo demás como `{hex}`.
  - Opción de juego `language` (`en`, `es`) y gancho `after` sobre el cargador de mensajes (`0E:C4E2`) que sobrescribe el búfer con el texto de `games/alttp_usa/lang/es.txt` cuando existe, recargándolo si el fichero cambió. Un mensaje sin traducir sale en inglés.
  - Comprobación de equivalencia en `scripts/check.ps1`.
- Sin letras acentuadas todavía: la fuente y las anchuras llegan en el cambio siguiente.

## Capabilities

### New Capabilities

- `game-paths`: carpetas del perfil y de la ROM que el runtime ofrece al C del juego.

### Modified Capabilities

Ninguna.

## Impact

- Motor: `runtime/snes_rt.h`, `runtime/snes_core.inc`, `runtime/headless.c`, `runtime/play.c`, `src/games.rs`, `src/port.rs` y sus pruebas.
- Juego: `games/alttp_usa.toml` (ganchos y opción), `games/alttp_usa/text.c`, `scripts/check.ps1`, README, `.gitignore`.
- Fuera de alcance: glifos con tilde, anchuras, comprobador de anchura de línea, nombres de comandos, menús y créditos, y la traducción misma.
