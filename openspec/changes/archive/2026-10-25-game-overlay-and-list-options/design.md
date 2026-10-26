## Context

- **Opciones de juego hoy:** `game_opt(key, def)` (`runtime/snes_core.inc`) lee hasta 16 pares clave/número que llegan solo por `--game-opt` (`parse_game_opt`). No pasan por `config.ini` ni por el menú. La pestaña «Game» de F1 (`runtime/play.c`, `TAB_GAME`) tiene solo filas de sistema (Resume, Save game now, Restore all defaults, Quit).
- **Opciones del ejecutable:** `play_cfg.h` define `opt_defs[O_COUNT]` (enum fijo), un INI que conserva claves desconocidas (`ini_get/ini_set`) y reglas para valores fuera de rango (se ignoran). `play.c` aplica la línea de comandos solo en esa ejecución.
- **ROM:** `rom_byte()` lee `rom[(bank << 15 | a & 0x7FFF) % rom_size]` y `rom` es un búfer de `malloc` cargado del disco, escribible. El runtime sin vídeo (`headless.c`) y el interactivo (`play.c`) comparten `snes_core.inc`.
- **Generador:** `codegen --all` ya escribe junto al C generado `play/play_cfg.h`, `play/play_ui.h` y `build_play.bat`; el perfil se carga en el generador, no en el runtime.

## Goals / Non-Goals

**Goals:**
- Opciones de juego de tipo lista, declaradas en el perfil, persistidas en `config.ini`, visibles en el menú y legibles con `game_opt`.
- Superposición de bytes sobre la ROM en memoria desde el C del juego.
- Compatibilidad total: sin `[[option]]` ni `rom_overlay`, nada cambia (ni C generado, ni informe, ni imagen).

**Non-Goals:** opciones numéricas o de texto libre, descubrir opciones en tiempo de ejecución, traducción, fuente y cargador de mensajes.

## Decisions

- **Las opciones llegan al runtime dentro de `snes_entry.c`**, que `codegen --all` ya escribe desde el perfil y que entra en toda compilación (runtime sin vídeo, interactivo, `snes-port` y pruebas): tabla `snes_game_opts[]` (`GameOptDef` de `snes_rt.h`: clave, etiqueta, nombres y valor por defecto) y `snes_game_opt_count`. Sin opciones hay una tabla vacía, así que el runtime siempre encuentra los símbolos. Se prefirió a un `play/game_opts.h` nuevo porque no obliga a tocar ninguna lista de ficheros. Alternativa descartada: que `snes-port` pase las opciones por la línea de comandos, porque `snes_play.exe` también se usa suelto con `config.ini`.
- **Valor = índice en `names`; el C del juego recibe el índice por `game_opt`.** Sin cambiar su firma. El runtime guarda, por opción declarada, el valor del fichero y el vigente en la ejecución (como `Config.file` y `Config.eff` del resto de opciones): `--game-opt` toca solo el vigente, el menú toca los dos. `--game-opt` acepta nombre o número para las declaradas y solo número para el resto (el almacén antiguo de 16 claves numéricas sigue igual). Un perfil declara como máximo 16 opciones, y el nombre de cada valor es de letras, cifras, `_`, `-` o `.`, y la etiqueta es ASCII imprimible (la fuente del menú no tiene más).
- **Clave en el INI: `game.<key>` con el nombre**, no el número, para que reordenar `names` no cambie las partidas de nadie. Un nombre desconocido se ignora como el resto de valores inválidos.
- **Menú:** las filas de opciones del juego van arriba de la pestaña «Game», con el estilo de las filas de lista de «Image» (izquierda/derecha cambian, sin tope porque la lista da la vuelta). `menu_count`, `menu_change` y el dibujo de la pestaña suman `GAME_OPT_COUNT` al desplazamiento de las filas de sistema; «Restore all defaults» también restaura las del juego. Con 0 opciones el código es el de antes.
- **Efecto inmediato:** el menú escribe en `game_opts[]` al cambiar, y el C del juego lo lee cada vez que lo necesita (`game_opt` es barato).
- **`rom_overlay(addr, data, n)`** escribe en `rom` en las posiciones que `rom_byte()` calcularía para `addr`. Devuelve 0 si fue bien y distinto de 0 si algún byte cae fuera de la imagen (en ese caso no escribe nada). No añade ramas al camino de lectura, que sigue siendo `rom[...]`, así que el coste de lectura es el mismo. Alternativa descartada: tabla de intervalos consultada en cada lectura, que penalizaría a todos los juegos.
- **La superposición vive en memoria y no hay forma de volcarla a disco:** el fichero de la ROM, `--dump` de WRAM/VRAM y la comprobación de que ningún byte de ROM acaba en lo construido no cambian. Los bytes superpuestos los aporta el C del juego, y el `scan` los trata como cualquier otro dato propio.
- **Cuándo se llama:** el C del juego la llama desde un gancho, de forma idempotente (p. ej. en el primer fotograma). El motor no añade ningún gancho de arranque en este cambio; si hace falta uno general, se propondrá aparte.

## Risks / Trade-offs

- **`rom_overlay` pisa datos que el análisis estático ya usó** (el recompilador analiza la ROM original) → se documenta que solo es válido sobre datos, no sobre código; no se comprueba por el motor.
- **Persistir por nombre** exige que los nombres sean estables → la validación del perfil los exige únicos; renombrar uno es romper la configuración guardada, y queda anotado en el formato.
- **Menú con muchas opciones** puede no caber → fuera de alcance; el menú ya recorta y el perfil de ALttP traerá una.
