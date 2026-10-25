## Context

Observado en el repositorio (privado, `AlexPerez7/snes-recomp`, rama `main`, 49 commits con un único autor, sin ROMs, binarios ni imágenes en todo el historial):
- `src/games.rs` incrusta `games/alttp_usa.toml` y `games/alttp_usa/dialog.c` con `include_str!`; `Catalog::builtin()` los parsea y `with_external` añade los de `--game`. `port::build_port` y `current_version` ya trabajan con un `Catalog` y un `GameProfile` (config + fuentes C), no con constantes.
- `main.rs::load_config` busca `games/<id>.toml` en el directorio actual para una ROM conocida.
- Pruebas con ALttP real: `codegen.rs` (`run_alttp`, `alttp_full_rom_compiles_and_links`, `alttp_inline_helper_call_reaches_its_target`), `headless.rs` (`alttp_boots_and_takes_its_first_nmis`, `alttp_message_boxes_…`), `port.rs` (`real_rom()` en tres pruebas, una con TinyCC), `config.rs` (`bundled_alttp_config_parses`) y `games.rs` (`the_embedded_profile_is_alttp`). Todas se omiten solas sin ROM.
- `scripts/release.ps1` copia `games/` al zip y tiene una guarda contra ROMs y C generado. `outb.log` y `outbt.log` están rastreados (221 bytes cada uno, solo nombres de ficheros de salida del compilador). No hay `LICENSE` ni `CONTRIBUTING.md`; el README dice «Licencia: sin elegir todavía».
- El C propio de un juego (`dialog.c`) es código original; el `.toml` contiene direcciones, nombres y cuentas de tablas, no bytes de la ROM.

## Goals / Non-Goals

**Goals:**
- Dos repos públicos con responsabilidades claras: motor genérico y juego (perfil, C propio, documentación y paquete).
- Que un usuario sin Rust ni git consiga jugar bajándose un zip del juego y aportando su ROM.
- Que ningún commit publicado contenga ROMs, binarios ni bytes de la ROM, con una comprobación repetible.
- Dejar el motor sin conocimiento de ALttP (ni en el código ni en las pruebas).

**Non-Goals:**
- Un segundo juego, integración continua o releases automáticas en GitHub (se pueden añadir después).
- Reescribir el historial (se publica tal cual, decidido con el usuario); el correo de los commits queda visible.
- Dar al repo del juego su propio árbol `openspec/` (por ahora su comportamiento se documenta en README y se comprueba con un script).
- Empaquetar SDL2 ni un compilador para quien no lo tenga (sigue como hoy).

## Decisions

1. **El motor no incrusta perfiles; los busca.** `games::Catalog` pasa a construirse con `Catalog::discover(search_dirs, extra)`: lee cada `*.toml` de `games/` junto al ejecutable y de `games/` del directorio actual (el actual prevalece), y luego aplica `--game` (que prevalece sobre ambos). Un `.toml` roto en una carpeta se informa con su nombre y se omite; en `--game` es un error. `main.rs::load_config` usa el mismo catálogo. Alternativas descartadas: submódulo git del juego dentro del motor (acopla los dos repos y reintroduce el contenido del juego en el motor) y variable de entorno (invisible para el usuario).
2. **Pruebas con juego real por variables de entorno.** Un auxiliar `#[cfg(test)] real_game() -> Option<RealGame { profile, rom }>` lee `SNES_GAME_PROFILE` y `SNES_GAME_ROM`; sin ellas (o con una ROM que no coincide con el CRC32 del perfil) la prueba se omite. Las pruebas que usan direcciones concretas de ALttP (`alttp_inline_helper_call_reaches_its_target`, `alttp_boots_…`) comprueban además que el perfil sea el de ALttP (`id == "alttp_usa"`) y se omiten si no. El motor sigue probándose en CI local con ROMs sintéticas, que es la mayoría.
3. **La prueba de mensajes pasa al repo del juego como script.** `alttp-recomp/scripts/check.ps1` usa el CLI del motor (`snes-recomp codegen <rom> --all -o <dir> --config alttp_usa.toml`, que ya aplica ganchos y escribe `game/`), compila con `cl` (como `out/b2.bat`), ejecuta `headless.exe` tres veces con `--dump` y compara `$1CD9` en WRAM a los 2040 fotogramas (5 sin R, más de 10 con R, 5 con `--game-opt dialog_skip=0`). Alternativa descartada: dejar la prueba en Rust del motor con un `if` por juego.
4. **Subcomando `snes-recomp scan <rom> <carpeta>`.** Reutiliza `port::find_rom_bytes_at` (el mismo detector que la construcción: 32 bytes en bruto o como literales hex). Se usa para comprobar el árbol de trabajo y cada commit del historial (`git archive <c> | tar -x`). Alternativa descartada: script ad hoc que duplique la lógica.
5. **Historial.** `snes-recomp` conserva los 49 commits; antes de publicar se quitan `outb.log`/`outbt.log` con un commit normal (sus versiones antiguas solo contienen nombres de ficheros). `alttp-recomp` nace con un historial nuevo con los ficheros movidos; `games/alttp_usa.toml` y su carpeta salen del motor en el mismo cambio. No se reescribe nada con `filter-repo`.
6. **Paquete del juego.** `alttp-recomp/scripts/release.ps1 -Engine <zip o carpeta de snes-recomp>` descomprime el paquete del motor (que ya lleva `snes-port.exe`, `snes-recomp.exe`, SDL2 y README), añade `games/alttp_usa.toml` y `games/alttp_usa/`, aplica la misma guarda de ROMs/C generado y crea `alttp-recomp-<v>-win64.zip`. Como `snes-port` busca `games/` junto al ejecutable, el usuario solo arrastra su ROM sobre `snes-port.exe`. `release.ps1` del motor deja de copiar `games/`.
7. **Documentación de proyecto.** MIT, con el nombre del autor del repo y el año; README del motor reescrito para visitantes (qué es, aviso legal arriba, requisitos, uso con `--game`, estructura); `CONTRIBUTING.md` en español (el proyecto trabaja en español): flujo OpenSpec, clean-room, ROMs nunca, cómo ejecutar las pruebas (con y sin ROM) y cómo proponer un juego nuevo. El README del juego explica el paquete, la tecla R, `--game-opt dialog_skip=0` y la lista de limitaciones conocidas.
8. **Publicar es un paso aparte y confirmado.** Orden: (a) todo el trabajo en local y en el repo privado (cambios del motor, repo nuevo del juego en una carpeta hermana, sin crear nada remoto); (b) verificaciones (`cargo test`, `scan` del árbol y del historial, `release.ps1` de ambos); (c) pedir confirmación al usuario; (d) `gh repo create AlexPerez7/alttp-recomp --public --source … --push` y `gh repo edit AlexPerez7/snes-recomp --visibility public --accept-visibility-change-consequences`, descripción y temas. Publicar no se puede deshacer del todo (copias, cachés, forks), por eso (c).

## Risks / Trade-offs

- [Algo derivado de la ROM llega a un repo público] → `scan` sobre el árbol y los 49 commits antes de publicar, la guarda ya existente de `release.ps1`, y confirmación explícita antes de cambiar la visibilidad. Si `scan` encuentra algo, se detiene todo.
- [Un perfil de juego es material «derivado» en sentido amplio] → solo lleva direcciones, nombres y tamaños de tabla, como cualquier documentación de la ROM; el README lo explica y no incluye datos del juego.
- [Las pruebas de ALttP del motor pierden cobertura automática] → las env vars y `scripts/check.ps1` del juego las mantienen ejecutables a mano; se documentan en CONTRIBUTING de ambos repos.
- [El historial publicado lleva nombre y correo reales] → decisión consciente del usuario; se vuelve a avisar en la confirmación previa a publicar.
- [Dos repos que evolucionan a la vez] → el paquete del juego declara la versión mínima del motor; el perfil usa solo `[[hook]]`, `[game]` y `--game-opt`, que son estables desde `game-hooks`.

## Migration Plan

1. Cambios del motor en una rama; `cargo test` en verde con y sin las env vars.
2. Crear `alttp-recomp` en local (carpeta hermana, `git init`), mover perfil y C, escribir README/CONTRIBUTING/LICENSE y los scripts.
3. Verificar: `check.ps1` y `release.ps1` del juego contra el motor local; `scan` de ambos árboles y del historial del motor.
4. Confirmar con el usuario y publicar (paso 8). Si algo falla antes de publicar no hay nada que revertir fuera de la máquina.
5. Rollback tras publicar: volver el repo del motor a privado con `gh repo edit … --visibility private` (no retira copias ya hechas).

## Open Questions

- Si el repo del juego debe tener su propio `openspec/` (por ahora no).
- Si el zip del juego debe publicarse como release de GitHub en esta tanda (por ahora solo se prepara el script).
