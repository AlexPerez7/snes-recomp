## 1. Motor sin perfiles incrustados

- [x] 1.1 `src/games.rs`: quitar `builtin_texts`/`builtin()`; `Catalog::discover` con carpetas de búsqueda (junto al ejecutable y directorio actual, el actual prevalece) más `--game`; un `.toml` roto en una carpeta se informa y se omite
- [x] 1.2 `src/port.rs` y `src/main.rs`: usar `discover` en `catalog()`, `needs_no_build` y `load_config` (`discover`/`codegen` sin `--config` usan el catálogo en vez de `games/<id>.toml` del directorio actual)
- [x] 1.3 Mensaje de «ROM no reconocida» sin ningún perfil: dónde busca y cómo conseguir uno; actualizar el texto de `USAGE`
- [x] 1.4 Tests: perfil junto al ejecutable, directorio actual sobre el del ejecutable, `--game` sobre carpetas, fichero roto en `games/`, sin perfiles (código 2 y mensaje)

## 2. Pruebas con juego real desde fuera

- [x] 2.1 Auxiliar de pruebas `real_game()` con `SNES_GAME_PROFILE` y `SNES_GAME_ROM` (se omite si faltan o si el CRC32 de la ROM no es el del perfil)
- [x] 2.2 Migrar `codegen.rs`, `headless.rs` (`alttp_boots_…`), `port.rs` (`real_rom()`, e2e y TinyCC) y `config.rs` al auxiliar; las que usan direcciones de ALttP comprueban `id == "alttp_usa"`
- [x] 2.3 Quitar de `games.rs` `the_embedded_profile_is_alttp` y la prueba `alttp_message_boxes_…` de `headless.rs` (pasa a `check.ps1` del juego)
- [x] 2.4 `cargo test` en verde sin variables y con `SNES_GAME_PROFILE`/`SNES_GAME_ROM` apuntando al perfil y a `roms/alttp_usa.sfc`

## 3. Higiene del repo del motor

- [x] 3.1 Subcomando `snes-recomp scan <rom> <carpeta>` (reutiliza `find_rom_bytes_at`; código distinto de cero y nombre del fichero si encuentra bytes) con tests: carpeta limpia, bytes en bruto, literales hex
- [x] 3.2 `git rm --cached outb.log outbt.log`; `.gitignore` con `*.log`, `.srm`, `/out/`, ROMs y `/target`
- [x] 3.3 `scan` sobre el árbol actual y sobre cada uno de los commits (`git archive`) con `roms/alttp_usa.sfc`; si algo aparece, parar y avisar
- [x] 3.4 `scripts/release.ps1`: el zip del motor ya no copia `games/`; mantener la guarda

## 4. Documentación de proyecto del motor

- [x] 4.1 `LICENSE` MIT
- [x] 4.2 `README.md` reescrito para visitantes: qué es, aviso legal arriba, requisitos, uso con perfiles (`games/`, `--game`), estructura, enlace a `alttp-recomp`; quitar «Licencia: sin elegir todavía» y las menciones a `games/*.toml` incluidos
- [x] 4.3 `CONTRIBUTING.md`: flujo OpenSpec, clean-room, ROMs nunca, pruebas con y sin ROM (env vars), cómo proponer un juego nuevo
- [x] 4.4 `CLAUDE.md`: estructura nueva (sin `games/`, búsqueda de perfiles, env vars de las pruebas, `scan`)

## 5. Repo del juego `alttp-recomp` (en local, sin publicar)

- [x] 5.1 Carpeta hermana `alttp-recomp` con `git init`; mover `games/alttp_usa.toml` y `games/alttp_usa/dialog.c` (estructura `games/` para que el paquete y `snes-port` los encuentren); eliminarlos del motor
- [x] 5.2 `LICENSE` MIT, `.gitignore`, `README.md` (qué incluye, aviso legal, cómo jugar con el paquete o con `--game`, tecla R y `--game-opt dialog_skip=0`, limitaciones conocidas) y `CONTRIBUTING.md`
- [x] 5.3 `scripts/check.ps1`: genera con el CLI del motor, compila con `cl`, corre `headless.exe` tres veces y comprueba `$1CD9` a los 2040 fotogramas (5, al menos el doble, 5); se omite con un aviso si falta la ROM o el compilador
- [x] 5.4 `scripts/release.ps1 -Engine <zip o carpeta>`: arma `alttp-recomp-<v>-win64.zip` con `snes-port`, SDL2, README y `games/`; guarda contra ROMs y C generado
- [x] 5.5 Verificar: `check.ps1` y `release.ps1` contra el motor local; `scan` del árbol del juego; arrastrar la ROM sobre el `snes-port.exe` del zip desde una carpeta limpia construye y arranca

## 6. Publicación (requiere confirmación del usuario)

- [x] 6.1 Antes de nada: `cargo test` del motor, `scan` de ambos árboles y del historial, `release.ps1` de ambos, `git status` limpio en ambos
- [x] 6.2 Pedir confirmación explícita al usuario (nombres, MIT, historial con su nombre y correo, visibilidad pública); no continuar sin ella
- [x] 6.3 Commit de los cambios del motor y push; crear `AlexPerez7/alttp-recomp` público y subirlo; cambiar `AlexPerez7/snes-recomp` a público; descripción y temas de ambos
- [x] 6.4 Comprobar desde fuera (`gh repo view`, clonar en una carpeta temporal) que ambos son públicos y que el clon no contiene ROMs ni `out/`; actualizar `CLAUDE.md` y la memoria con las URLs
