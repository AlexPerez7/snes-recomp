## 1. Rutas en el motor

- [x] 1.1 `GameProfile` guarda la carpeta de su perfil (la hermana del `.toml` con su nombre, si existe) y tests en `src/games.rs`
- [x] 1.2 `rom_dir()` y `game_dir()` en `runtime/snes_rt.h` y `runtime/snes_core.inc`, y `--game-dir` en `runtime/headless.c` y `runtime/play.c`; sin él, `game_dir()` es `rom_dir()`
- [x] 1.3 `snes-port` pasa `--game-dir` al lanzar el juego cuando el perfil tiene carpeta
- [x] 1.4 Tests de `src/headless.rs` y `src/port.rs`: ROM con y sin carpeta, `--game-dir`, la caída a la carpeta de la ROM, que `snes-port` lo pasa aunque el directorio de trabajo sea otro, y que un fichero que no es `.c`/`.h` en la carpeta del perfil no cambia la versión de la caché
- [x] 1.5 README y `CLAUDE.md`: las dos funciones y `--game-dir`

## 2. Texto de ALttP (repositorio `alttp-recomp`)

- [x] 2.1 `games/alttp_usa/text.c`: tabla de caracteres, longitudes de los comandos, y paso de búfer a texto y de texto a búfer con errores con número de línea
- [x] 2.2 Volcado con `--game-opt dump_text=1` (gancho `after` en `0E:D3EB`): llama al cargador original por cada id, escribe `<rom_dir>/alttp_messages_en.txt` y sale, sin dejar `.srm` ni `config.ini`
- [x] 2.3 Opción `language` (`en`, `es`) en `games/alttp_usa.toml` y gancho `after` en `0E:C4E2` que pisa el búfer con `game_dir()/lang/es.txt`: recarga por fecha, mensaje sin traducir en inglés, límite de 3.500 bytes, avisos por `stderr`
- [x] 2.4 Comprobar (en el C generado: solo `$7F1200` y `$7F1FC0` en adelante, y el volcado de 1.872 como máximo) que nada más usa `$7F1300–$7F1FBF`; ajustar el límite si hace falta
- [x] 2.5 `.gitignore` y README del juego: formato de `es.txt`, cómo volcar el inglés y que el volcado no se sube

## 3. Verificación

- [x] 3.1 `scripts/check.ps1`: volcado (397 mensajes y huella SHA-256 constante), ida y vuelta idéntica, diferencia al cambiar una línea y captura idéntica del primer cuadro con `language=en` y `es`
- [x] 3.2 `cargo test` del motor completo y `scan` de las dos carpetas antes de subir nada
- [ ] 3.3 Prueba manual con `play.bat`: `game.language = es` y un `es.txt` con un solo mensaje cambiado se ve en el juego, y al editarlo se actualiza sin reiniciar
