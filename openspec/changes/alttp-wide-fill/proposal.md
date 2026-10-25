# Proposal

## Why

La pantalla ancha (`wide-display`) ya dibuja `256 + 2·E` píxeles, pero en *A Link to the Past* la zona nueva solo muestra lo que casualmente hay en memoria: en el exterior el mapa se corta donde acaba lo cargado y en las salas se repiten cuartos. Para que `wide` sea usable en el juego, la zona nueva debe mostrar contenido coherente.

## What Changes

Alcance reducido tras la investigación (ver design.md): el mapa del área vecina no está en memoria hasta la transición, así que precargarlo exige descomprimirlo desde la ROM. Este cambio no lo hace; en su lugar **tapa en negro** lo que no hay.

- **Motor**: una función para que el C del juego fije franjas negras en la zona nueva (`ppu_wide_blank`), además de `ppu_wide()` para leer `E`.
- **Exterior**: el C del juego pone las franjas cuando la cámara está a menos de `E` px del borde de su zona de scroll, de modo que la zona nueva solo se ve cuando hay mapa cargado.
- **Salas**: franjas completas a ambos lados, salvo lo que se compruebe que se puede mostrar.
- Todo el código del juego va a `alttp-recomp` (`games/alttp_usa/*.c` y ganchos en `games/alttp_usa.toml`).
- Con `E = 0` el comportamiento no cambia.
- **Futuro, cambio aparte**: migrar a la precarga real del exterior (descomprimir el vecino, construir columnas, restaurar la cuadrícula Map16) y las piezas 3 y 4 (enemigos, objetos y sprites a la derecha con `ppu_sprite_xoff`).

## Capabilities

### New Capabilities
Ninguna. El comportamiento del relleno es del perfil de ALttP (repo `alttp-recomp`), no del motor, y no tiene spec aquí.

### Modified Capabilities
- `wide-display`: dos requisitos nuevos, consultar el ensanche `E` vigente y fijar franjas negras en la zona nueva. El motor sigue sin rellenar por su cuenta: solo obedece al juego.

## Impact

- `alttp-recomp`: `games/alttp_usa/` (C nuevo) y `games/alttp_usa.toml` (ganchos); su `scripts/check.ps1` puede ganar una prueba con `--wide`.
- Motor: `snes_rt.h` (`ppu_wide`, `ppu_wide_blank`), `snes_ppu.inc` (dibujo de las franjas) y tests en `src/headless.rs`.
- Coste: sin carga extra; las franjas se dibujan en lugar de lo que dibujaría la PPU.
