# Proposal

## Why

El proyecto ya tiene una frontera clara entre el motor (recompilador, runtime y `snes-port`) y lo que es de un juego (perfil `.toml`, tablas de saltos, C propio con ganchos), pero vive en un único repositorio privado, sin licencia ni guía para colaborar. Publicarlo permite que otras personas lo usen y aporten; separar el juego del motor deja el motor genérico (un segundo juego sería otro repo, no otro `if`) y evita mezclar en un mismo historial lo que es de ALttP con lo que no.

## What Changes

- **Dos repositorios públicos:** `snes-recomp` (el motor, conserva el historial actual de 49 commits) y `alttp-recomp` (nuevo: `alttp_usa.toml`, `alttp_usa/dialog.c`, README del juego, scripts de comprobación y de paquete).
- **BREAKING (interno):** el motor deja de incrustar perfiles de juego. `snes-port` busca perfiles en `games/` junto al ejecutable y en `games/` del directorio actual, además de `--game`. Sin perfil para la ROM, el error explica dónde conseguir uno.
- El motor deja de llevar `games/` y la parte de ALttP de sus pruebas: las pruebas con una ROM real reciben el perfil y la ROM por `SNES_GAME_PROFILE` y `SNES_GAME_ROM` y se omiten si faltan; la prueba de mensajes de ALttP y su spec pasan al repo del juego.
- Archivos de proyecto abierto: `LICENSE` (MIT), `README.md` reescrito para quien llega de fuera (qué es, aviso legal, cómo usarlo con un perfil), `CONTRIBUTING.md` (flujo OpenSpec, reglas clean-room, nunca ROMs).
- Higiene antes de publicar: dejar de rastrear `outb.log` y `outbt.log`, ampliar `.gitignore`, y comprobar que ningún commit del historial contiene bytes de la ROM (nuevo subcomando `snes-recomp scan <rom> <carpeta>`).
- Paquete del juego: `alttp-recomp` arma un zip con el `snes-port` del motor más su perfil en `games/`, de modo que el usuario solo necesita su ROM.
- Hacer públicos los repos (cambiar la visibilidad del actual y crear el del juego) solo tras pedir confirmación en ese momento.

## Capabilities

### New Capabilities
- `project-publication`: lo que un repositorio de este proyecto debe cumplir para publicarse: licencia, aviso legal, guía de contribución, y ausencia de ROMs, binarios y material derivado de la ROM en el árbol y en el historial.

### Modified Capabilities
- `game-profiles`: los perfiles ya no vienen incrustados; se descubren en carpetas `games/` y con `--game`.
- `port-builder`: el mensaje de ROM no reconocida cubre el caso sin ningún perfil cargado.
- `alttp-dialog-skip`: se retira de este repositorio (pasa al repo del juego).

## Impact

- `src/games.rs` (sin perfiles incrustados, búsqueda en carpetas), `src/port.rs`, `src/main.rs` (`load_config` usa el catálogo; subcomando `scan`), pruebas que usan la ROM real en `src/codegen.rs`, `src/headless.rs`, `src/port.rs` y `src/config.rs`.
- `scripts/release.ps1` (el zip del motor ya no lleva `games/`), `.gitignore`, `README.md`, `CLAUDE.md`, nuevos `LICENSE` y `CONTRIBUTING.md`.
- Fuera del repo: crear `AlexPerez7/alttp-recomp`, cambiar la visibilidad de `AlexPerez7/snes-recomp`, descripción y temas de ambos.
- Sin dependencias nuevas. El historial publicado lleva el nombre y el correo reales de los commits.
