## Context

Observado en el código:
- `src/games.rs` define `GameProfile { id, name, crc32, config }` y una constante `ALTTP_USA` que incrusta `games/alttp_usa.toml` con `include_str!`; `KNOWN` y `identify(crc)` salen de ahí.
- `src/port.rs::generate` parsea `profile.config`, descubre, construye el grafo y llama a `codegen::Emitter::generate_all`, que devuelve el C por banco, `snes_funcs.h` y `snes_entry.c`. `compile` compila `rom_*.c snes_entry.c play\snes_play.c` (MSVC) o el equivalente para gcc/clang/TinyCC. `current_version` hace un CRC32 de los fuentes del motor y de `games::KNOWN.config`.
- `src/config.rs::RawConfig` usa `deny_unknown_fields`, así que cualquier sección nueva del `.toml` se declara ahí.
- El runtime (`runtime/snes_rt.h`) ya es la interfaz entre el C generado y el resto; `Cpu *c` es el estado de la CPU y hay funciones de bus.

Investigado en la tarea 5.1 (con `--trace-frames --watch` y el desensamblador): mientras hay un cuadro de mensaje (módulo `$10 = 0E`, submódulo 2) la función `0E:CA6C` corre una vez por fotograma; su contador `$1CD5` (se recarga desde `$1CD6`, que vale 3) cuenta los fotogramas hasta el siguiente carácter, y `$1CD9` es el índice del carácter. Con `$1CD5` a 1 el fotograma siguiente avanza un carácter.

## Goals / Non-Goals

**Goals:**
- Que un juego sea un perfil `.toml` más un directorio opcional de C propio, sin tocar el código Rust del motor para añadirlo.
- Ganchos `before`/`after`/`replace` sobre entradas de función, con C propio compilado junto al generado.
- Cargar un perfil externo con `--game`.
- Una mejora real de ALttP que ejercite todo lo anterior de punta a punta.

**Non-Goals:**
- Ganchos en mitad de una función o en direcciones que no son entrada de función.
- Dividir ya el repositorio en dos (viene después, como mover ficheros).
- Mejoras del renderizador (pantalla ancha, texturas).
- Un sistema de opciones por juego en el menú F1 (la mejora se desactiva con una opción simple de línea de comandos/INI existente).

## Decisions

1. **Identidad en el `.toml`.** `[game]` con `id`, `name`, `crc32` (cadena hexadecimal de 8 dígitos). `games.rs` deja de tener constantes: un `fn builtin() -> Vec<Profile>` parsea los `.toml` incrustados con `include_str!` (una lista de `(toml, directorio de C)` en un único sitio). Alternativa descartada: `build.rs` que recorra `games/`; añade complejidad y una lista explícita basta con un juego.
2. **Perfil = `.toml` + directorio hermano.** `games/alttp_usa.toml` y `games/alttp_usa/`. Un perfil externo sigue la misma regla junto a su `.toml`. Los ficheros C incrustados se listan con `include_str!` junto al `.toml`; los externos se leen del disco. Alternativa descartada: C dentro del propio `.toml`; no se puede compilar ni probar aparte.
3. **Ganchos como envoltorio de la función, no como parches posteriores.** `Emitter::with_hooks` valida que cada `addr` sea una función del grafo. Con ganchos, el cuerpo original se emite como `static void f_XX_YYYY_body(Cpu *c)` y la función pública `f_XX_YYYY` pasa a ser un envoltorio: `before`, cuerpo, `after`. Con `replace` el cuerpo no se traduce (así se puede reemplazar incluso una función que no se pueda traducir) y el envoltorio llama al gancho y hace el epílogo `rts`/`rtl` (`c->ret = rt_pull16(c)`, más `rt_pull8` para `rtl`). Los `call` se declaran en `snes_funcs.h`. Sin ganchos el texto generado no cambia. Alternativa descartada: insertar la llamada antes de cada `return;` del cuerpo, porque los retornos están repartidos por muchos puntos del generador (RTS, RTL, saltos de cola, despachos).
4. **Retornos no locales.** Como el envoltorio llama al cuerpo y luego a `after`, no toca la comprobación de `S`/`ret` que hace cada llamada: sigue ocurriendo en quien llama, después del envoltorio. Un `after` ve el estado de la CPU en el momento de volver, incluso tras una llamada de cola.
5. **Compilación.** `compile` añade `game\*.c` (copiados a `<tmp>/game/`) y `/I game`. El fallo por función ausente sale del enlazado; `port.rs` lo traduce a un mensaje que nombra el gancho (se comprueba antes de compilar buscando `void nombre(` en los `.c` del juego, para dar un error claro).
6. **Versión de la caché.** `current_version` incluye el texto del perfil activo y de su C, no solo los `KNOWN`. El nombre de la caché sigue siendo `<crc32>-<versión>`.
7. **`--game`.** Se admite también como `--game ruta.toml` repetible. Un perfil externo con el mismo `crc32` que uno incrustado lo reemplaza; dos externos con el mismo `crc32` es un error. Los mensajes de «ROM desconocida» listan todos los perfiles cargados.
8. **Interfaz del C del juego.** Se expone en `snes_rt.h` lo ya existente (`Cpu`, lectura/escritura de bus) más `wram_read8/wram_write8` y `joy1_state()` si no existen; el frontal interactivo y el sin vídeo implementan `joy1_state()` con el mismo valor que ya devuelven a `$4218-$4219`.
9. **Mejora de diálogos.** Gancho `before` sobre `0E:CA6C` que, con R pulsado (`joy1_state()`), pone `$1CD5` a 1 si era mayor: un carácter por fotograma, tres veces más rápido. Las pausas del propio texto (cuenta atrás de `$1CE0`) no se acortan. Desactivación con `--game-opt dialog_skip=0`: opción nueva del runtime (`game_opt()` en `snes_rt.h`, `--game-opt KEY=NUMBER` en `headless` y `snes_play`), solo por línea de comandos; no entra en `config.ini` ni en el menú F1.

## Risks / Trade-offs

- [Un gancho `after` ve el estado tras una llamada de cola, no el de la instrucción de salto] → es lo que significa «al volver la función»; documentado en el generador y cubierto por tests de ejecución.
- [Cambiar `games.rs` toca la identificación de ROM, que usa todo el constructor] → tests existentes de ROM desconocida y cabecera de copiador deben seguir pasando sin cambios.
- [El C del juego puede depender del frontal] → solo se le ofrece `snes_rt.h`; un test compila el mismo `hooks.c` con el runtime sin vídeo.
- [La rutina de texto de ALttP puede ser difícil de acelerar sin romper la sincronía con el NMI] → la tarea 5.1 la investiga primero y, si no es viable con ganchos de entrada de función, se recorta la demostración a lo que sí lo sea (p. ej. descartar el diálogo con el botón) y se anota en la spec.

## Migration Plan

Cambio interno sin datos que migrar. La caché existente queda obsoleta por el cambio de versión y se reconstruye sola. El `.toml` de ALttP gana una sección `[game]`; un `.toml` antiguo sin ella deja de ser un perfil, pero `discover`/`codegen --config` siguen aceptándolo porque ahí no se necesita identidad.

## Open Questions

- Si hace falta `kind = "before"` con salida anticipada (el gancho decide si se ejecuta el cuerpo); por ahora se usa `replace` cuando haga falta.
