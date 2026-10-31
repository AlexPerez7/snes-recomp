## Why

Un juego no puede hoy ofrecer ajustes propios al jugador (p. ej. el idioma de un texto traducido): `game_opt` solo acepta números por `--game-opt` en la línea de comandos, no se guardan en `config.ini` ni aparecen en el menú F1 (la pestaña «Game» es la de sistema: Resume, Save game, Restore, Quit). Además el C del juego no puede sustituir datos que el código recompilado lee de la ROM (una fuente con acentos, tablas de anchuras), porque la ROM se lee tal cual. Las dos carencias bloquean la traducción propia al español de ALttP (`idioma-espanol`) y sirven a cualquier juego futuro.

## What Changes

- El perfil de juego puede declarar `[[option]]` (clave, etiqueta, nombres de la lista y valor por defecto). El motor los guarda en `config.ini` como `game.<clave> = <nombre>`, los muestra en el menú F1 y se los pasa al C del juego mediante el `game_opt(clave, def)` existente (como el índice de la lista). `--game-opt` sigue valiendo y manda en esa ejecución.
- Nueva API en `snes_rt.h`: `rom_overlay(addr, datos, n)` para que el C del juego superponga bytes sobre la copia de la ROM **en memoria**. El fichero de la ROM, su CRC y el C generado no cambian, y los bytes superpuestos son del propio juego, no de la ROM.
- Los perfiles sin `[[option]]` y sin llamadas a `rom_overlay` se comportan exactamente como hoy.

## Capabilities

### New Capabilities

- `game-options`: opciones de juego declaradas por el perfil, de tipo lista, con persistencia en `config.ini`, fila en el menú F1 y lectura desde el C del juego.
- `rom-overlay`: superposición de bytes en la imagen de ROM en memoria desde el C del juego.

### Modified Capabilities

- `game-profiles`: el formato del perfil admite `[[option]]`.

## Impact

- `src/config.rs` (formato `[[option]]` y su validación), `src/codegen.rs` (`generate_all` escribe la tabla de opciones en `snes_entry.c`), `runtime/snes_rt.h` y `runtime/snes_core.inc` (`rom_overlay`), `runtime/play.c` y `runtime/play_cfg.h` (INI, menú), README y pruebas de `src/headless.rs`.
- Fuera de alcance: la traducción en sí, la fuente y el cargador de mensajes (cambios posteriores en `alttp-recomp`), opciones numéricas o de texto libre, y cualquier cambio en `snes-port` más allá de recompilar con la tabla nueva de `snes_entry.c` (la versión de la caché ya incluye el runtime).
