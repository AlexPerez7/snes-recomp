## 1. Formato del perfil

- [x] 1.1 En `src/config.rs`, parsear `[[option]]` (`key`, `label`, `names`, `default`), validarlas (clave de hasta 31 caracteres, `names` de al menos dos nombres distintos, `default` dentro de `names`, claves únicas) y documentarlas al inicio del fichero
- [x] 1.2 Tests de `config.rs`: opción válida, `default` fuera de la lista, clave repetida, `names` con una sola entrada o repetidos, y que un perfil sin `[[option]]` se cargue como antes

## 2. Opciones en el runtime

- [x] 2.1 En `codegen --all`, escribir la tabla de opciones del perfil (vacía si no hay) en `snes_entry.c`, que ven los dos runtimes
- [x] 2.2 Cargar las opciones del `config.ini` (`game.<key> = <nombre>`) en el almacén de `game_opt`, con valor por defecto, nombre desconocido ignorado y claves ajenas conservadas, y escribir `game.<key>` en el `config.ini` de la primera ejecución
- [x] 2.3 `--game-opt CLAVE=NOMBRE|NUMERO` en `snes_play` y en el runtime sin vídeo, que manda solo en esa ejecución; ajustar el límite del almacén
- [x] 2.4 Tests de `src/headless.rs`: valor por defecto, valor guardado, nombre desconocido, línea de comandos sobre INI y nombre por línea de comandos en el runtime sin vídeo

## 3. Menú F1

- [x] 3.1 Filas de opciones del juego al principio de la pestaña «Game» (`menu_count`, `menu_change`, dibujo), guardado al cerrar y restauración con «Restore all defaults»
- [x] 3.2 Tests con `--menu-keys` y `--print-config`: cambiar una opción desde el menú la guarda, y un perfil sin opciones deja la pestaña exactamente como estaba

## 4. Superposición de la ROM

- [x] 4.1 Declarar `rom_overlay` en `runtime/snes_rt.h` e implementarla en `runtime/snes_core.inc` con la misma correspondencia de direcciones que `rom_byte()` y comprobación de límites
- [x] 4.2 Tests: lectura superpuesta desde el código recompilado, error y sin escritura si se sale de la ROM, la última llamada gana, el fichero de la ROM y su CRC32 no cambian, y un juego sin llamadas da el mismo informe que antes

## 5. Documentación y cierre

- [x] 5.1 README (tabla de opciones y sección del C del juego), `CLAUDE.md` (estructura y `snes_rt.h`) y `config.ini` de ejemplo
- [x] 5.2 `cargo test` completo y comprobación a mano del menú con un perfil de prueba con una opción `language`
