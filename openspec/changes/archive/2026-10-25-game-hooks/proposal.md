# Proposal

## Why

Hoy un juego es solo un `games/<id>.toml` y una constante en `src/games.rs`: sirve para descubrir código, pero no permite añadirle nada propio. Para que ALttP tenga contenido propio (mejoras, ajustes, parches) hace falta que el motor ofrezca ganchos sobre las funciones recompiladas y que un perfil de juego pueda vivir fuera del código del motor. Con eso, separar más adelante el motor y cada juego en repositorios distintos es mover ficheros, no rediseñar.

## What Changes

- El perfil de un juego pasa a describirse entero en su `.toml`: una sección `[game]` con `id`, `name` y `crc32` sustituye a las constantes de `src/games.rs`; la lista de juegos conocidos sale de los `games/*.toml` incrustados.
- Nueva opción `snes-port --game <perfil.toml>` para cargar un perfil externo (y su directorio de C) además de los incrustados.
- Nueva sección `[[hook]]` en el `.toml`: `addr`, `kind` (`before`, `after`, `replace`), `call` (nombre de una función C del juego) y, para `replace`, `ret` (`rts` o `rtl`). El generador emite la llamada en la función recompilada de esa dirección.
- Un perfil puede tener un directorio hermano con el mismo nombre que el `.toml` (`games/alttp_usa/`) con código C propio (`*.c`, `*.h`) que `snes-port` compila junto al C generado. El C del juego es código original: puede incrustarse en el binario y distribuirse.
- La versión de la caché incluye el contenido del perfil y de su C, de modo que cambiarlos reconstruye.
- Mejora de demostración para ALttP, escrita solo con ganchos: mantener pulsado un botón durante un diálogo lo acelera.
- Sin cambios de comportamiento para quien no use ganchos: un perfil sin `[[hook]]` genera el mismo C que hoy.

## Capabilities

### New Capabilities
- `game-profiles`: un juego se describe por completo en su perfil `.toml` (identidad por CRC32, nombre, id), con perfiles incrustados y perfiles externos con `--game`.
- `game-hooks`: ganchos `before`/`after`/`replace` sobre funciones recompiladas y código C propio del juego compilado junto al generado.
- `alttp-dialog-skip`: la mejora de ALttP que acelera los diálogos mientras se mantiene un botón.

### Modified Capabilities
- `port-builder`: la identificación de la ROM usa los perfiles cargados (incrustados o `--game`) y la versión de la caché incluye perfil y C del juego.

## Impact

- `src/config.rs` (secciones `[game]` y `[[hook]]`), `src/games.rs` (perfiles leídos de los `.toml`), `src/codegen.rs` (emisión de los ganchos), `src/port.rs` y `src/main.rs` (`--game`, compilación del C del juego, versión de la caché).
- `runtime/snes_rt.h`: lo que el C del juego puede usar (estado de la CPU, lectura y escritura de bus y WRAM, mando 1).
- Ficheros nuevos `games/alttp_usa/*.c` y entradas `[[hook]]` en `games/alttp_usa.toml`.
- Sin dependencias nuevas. El C del juego no deriva de la ROM; el C generado sigue sin salir de la caché.
