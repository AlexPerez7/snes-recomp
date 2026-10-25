## 1. Perfiles autocontenidos

- [x] 1.1 Añadir la sección `[game]` (`id`, `name`, `crc32`) a `RawConfig` en `src/config.rs` y documentarla al inicio del fichero; `Config::parse` sigue aceptando `.toml` sin ella para `discover`/`codegen --config`
- [x] 1.2 Reescribir `src/games.rs`: perfiles leídos de los `.toml` incrustados (lista única con `include_str!`), `identify(crc)` y detección de CRC32 repetido; añadir `[game]` a `games/alttp_usa.toml`
- [x] 1.3 Adaptar `src/port.rs` (`generate`, mensaje de ROM desconocida, `current_version`) a los perfiles nuevos; los tests existentes de ROM desconocida y cabecera de copiador pasan sin cambios
- [x] 1.4 Tests: perfil sin `[game]`, con campos que faltan, y CRC32 repetido

## 2. Perfiles externos

- [x] 2.1 Opción `--game <perfil.toml>` (repetible) en `snes-port`: leer el `.toml` y el directorio hermano, el externo prevalece sobre el incrustado con el mismo `crc32`
- [x] 2.2 Errores claros: fichero inexistente o ilegible, dos externos con el mismo `crc32`; el listado de juegos soportados incluye los externos
- [x] 2.3 Tests de `--game`: juego solo externo, externo sobre incrustado, fichero inexistente

## 3. Ganchos en el generador

- [x] 3.1 Sección `[[hook]]` en `src/config.rs` (`addr`, `kind`, `call`, `ret` obligatorio con `replace`) con validación de valores
- [x] 3.2 `Emitter` (`src/codegen.rs`) recibe los ganchos, valida que cada `addr` sea función del grafo y emite `before`, `after` (al volver la función) y `replace` (con epílogo `rts`/`rtl`); declaración de los `call` en `snes_funcs.h`
- [x] 3.3 Test: sin ganchos el C generado es idéntico al anterior
- [x] 3.4 Tests que compilan y ejecutan el C (usar nombres de caso únicos): `before` ve el estado de entrada, `after` se ejecuta en cada retorno, `replace` sustituye el cuerpo y vuelve bien, dirección que no es función da error

## 4. C propio del juego y runtime

- [x] 4.1 Copiar `*.c`/`*.h` del directorio del perfil a `<construcción>/game/` y compilarlos con el resto en `compile` (MSVC, gcc/clang y TinyCC), con `/I game`
- [x] 4.2 Comprobar antes de compilar que cada `call` de un gancho está definido en algún `.c` del juego y fallar con un mensaje que nombre el gancho
- [x] 4.3 Incluir perfil y C del juego en `current_version`; test de que editar un `.c` cambia la versión
- [x] 4.4 Exponer en `snes_rt.h` lectura/escritura de WRAM y `joy1_state()` (implementada en `headless.c`/`snes_core.inc` y en `play.c`) y test de que el mismo `hooks.c` compila con ambos frontales
- [x] 4.5 Test de que los bytes de la ROM siguen sin aparecer en lo construido con un perfil con C propio

## 5. Mejora de demostración: diálogos de ALttP

- [x] 5.1 Localizar con `--trace-frames`, `--watch` y `disasm` la rutina que pinta el texto de los diálogos (`$10 = 0E`) y un botón libre durante ellos; fijar el botón definitivo y, si no es viable acelerar con ganchos de entrada de función, recortar la demostración y ajustar `alttp-dialog-skip`
- [x] 5.2 Escribir `games/alttp_usa/dialog.c` y los `[[hook]]` en `games/alttp_usa.toml`
- [x] 5.3 Opción para desactivarla: `--game-opt dialog_skip=0`, con `game_opt()` en el runtime
- [x] 5.4 Prueba con la ROM real (se omite sola si falta): mismo diálogo con y sin el botón (`--press`), medir fotogramas hasta que el texto está completo y comprobar que sin pulsar no cambia

## 6. Cierre

- [x] 6.1 Actualizar `CLAUDE.md` (estructura: perfiles, ganchos, directorio del juego, `--game`) y la documentación de `src/config.rs`
- [x] 6.2 `cargo test` completo en verde y `scripts\release.ps1` sigue pasando (el C propio del juego no es material derivado de la ROM)
- [x] 6.3 Prueba a mano: jugar con ventana, mantener el botón durante un diálogo y comprobar que acelera
