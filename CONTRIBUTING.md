# Contribuir

Gracias por el interés. Este repositorio es el **motor** (recompilador, runtime y `snes-port`); cada juego vive en su propio repositorio con su perfil (`games/<id>.toml`) y su C propio. El de *A Link to the Past* es `alttp-recomp`.

## Reglas que no se negocian

- **Nunca ROMs.** Ni en commits, ni en issues, ni en pull requests, ni enlaces a ellas. `.gitignore` ya excluye `*.sfc`, `*.smc`, `*.srm`, `roms/` y `out/`.
- **Nada derivado de la ROM.** El C generado, volcados de VRAM/WRAM, capturas con recursos del juego o tablas copiadas bytes a bytes de la ROM no se suben. Antes de abrir un PR: `snes-recomp scan <rom> <carpeta>` falla si encuentra bytes de la ROM en lo que vas a publicar.
- **Clean-room.** El código es propio: no se copia código de otros recompiladores ni emuladores. Si te inspiras en documentación pública del hardware (65816, PPU, SPC700), basta con citarla en el comentario.
- **Tablas de saltos comprobadas a mano** contra la ROM real antes de añadirlas a un perfil.

## Flujo de trabajo (OpenSpec)

Los cambios se hacen con [OpenSpec](https://github.com/Fission-AI/OpenSpec): primero se propone y luego se implementa.

1. `/opsx:explore` para pensar el problema (opcional).
2. `/opsx:propose <nombre>` crea `openspec/changes/<nombre>/` con propuesta, especificaciones (deltas), diseño y tareas.
3. `/opsx:apply` implementa las tareas y las va marcando.
4. `/opsx:archive` sincroniza las especificaciones y archiva el cambio.

Los artefactos de OpenSpec y la documentación se escriben en español. Un cambio pequeño (un error, una errata) puede ir directo en un PR sin propuesta; si toca el comportamiento descrito en `openspec/specs/`, hay que proponerlo.

## Compilar y probar

```
cargo build --release
cargo test
```

Necesitas Rust estable y, para las pruebas que compilan el C generado, un compilador (gcc, clang o Visual Studio Build Tools). Las pruebas que necesitan compilador, SDL2 o un juego real **se omiten solas** si faltan, así que `cargo test` pasa sin ROM. Las del runtime como biblioteca (`src/rtlib.rs`: bibliotecas compartidas y `dlopen`) solo corren con gcc o clang en Linux; en Windows se omiten.

Para correr también las pruebas con un juego real, apunta a su perfil y a tu ROM:

```
# PowerShell
$env:SNES_GAME_PROFILE = "..\alttp-recomp\games\alttp_usa.toml"
$env:SNES_GAME_ROM = "roms\alttp_usa.sfc"
cargo test
```

Si el CRC32 de la ROM no es el del perfil, esas pruebas se omiten. Las que usan direcciones de un juego concreto también comprueban el `id` del perfil.

## Qué tocar y dónde

| Quieres... | Mira |
|---|---|
| arreglar el traductor de instrucciones o el descubrimiento | `src/codegen.rs`, `src/discover.rs`, `src/opcodes.rs` |
| mejorar vídeo, audio o entrada | `runtime/snes_ppu.inc`, `runtime/snes_spc.inc`, `runtime/play.c` |
| cambiar cómo se construye el port | `src/port.rs`, `src/toolchain.rs` |
| cambiar el formato de los perfiles | `src/config.rs`, `src/games.rs` (y la spec `game-profiles`) |

Un error de la PPU o del sonido se prueba mejor con una prueba que compile un trozo de C y compare el resultado (hay muchas en `src/headless.rs` y `src/spctest.rs`) que con una ROM.

## Proponer un juego nuevo

El motor no incrusta ningún juego. Para añadir uno:

1. Crea un repositorio `<juego>-recomp` con `games/<id>.toml` (`[game]` con `id`, `name` y `crc32`, más `[[entry]]` y `[[table]]`) y, si quieres ganchos propios, `games/<id>/*.c`. Mira `alttp-recomp` como ejemplo. El C propio incluye `snes_rt.h` y dispone de `wram_read8/wram_write8`, `joy1_state()`, `game_opt()`, `ppu_sprite_xoff()` para colocar sprites en las columnas extra de la pantalla ancha, `ppu_wide()` para leer el ensanche vigente y `ppu_wide_blank()` para tapar en negro lo que el juego no tenga cargado ahí.
2. Pruébalo con `snes-recomp discover <rom> --game games/<id>.toml --unresolved` hasta que no queden saltos indirectos sin tabla.
3. Si el juego necesita algo que el motor no ofrece (un registro, un modo de la PPU, un chip de la cartucha), abre aquí un issue o una propuesta con ese hueco concreto.

## Commits y PR

- Mensajes en español, en imperativo y con el *qué* y el *por qué* en la primera línea.
- Un PR por cambio de OpenSpec, con `cargo test` en verde.
- Describe cómo lo has probado y, si hay ROM de por medio, con qué juego (sin adjuntar nada de él).
