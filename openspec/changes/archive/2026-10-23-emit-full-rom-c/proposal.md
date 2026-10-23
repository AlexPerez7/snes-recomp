## Why

`codegen` genera hoy una función y sus llamadas hasta cierta profundidad, y falla entero con el primer error. Con los helpers de despacho resueltos (`resolve-inline-helper-exits`) no se sabe cuánto de ALttP se puede traducir y compilar de verdad: los 232 conflictos de anchos, los 437 bloques compartidos y las llamadas sin retorno solo se han probado en rebanadas pequeñas. Emitir la ROM completa y compilarla mide ese hueco y es el paso previo a ejecutar más allá de `reset`.

## What Changes

- Nueva opción `codegen <rom> --all -o <dir>` que emite todas las funciones del grafo (unas 2.500 en ALttP), sin profundidad ni dirección.
- La salida se reparte en un fichero `.c` por banco de la entrada de cada función, más un `snes_funcs.h` con las declaraciones y `snes_rt.h`, para que el compilador no reciba un único fichero enorme.
- La generación es de mejor esfuerzo: una función que no se puede traducir (conflicto de anchos que cambia el C, instrucción sin traducción, salto a algo que no es código) no aborta el resto. Se emite como stub que llama al nuevo gancho `rt_unsupported`, y se informa de cada una con su causa.
- Resumen final por la salida de error: funciones totales, generadas y con stub, agrupadas por causa, y líneas de C.
- El gancho `rt_unsupported(c, entry)` entra en `snes_rt.h` y en el runtime simulado de los tests.
- Verificación sobre ALttP: el C completo compila y enlaza con Visual Studio contra el runtime simulado, y se anotan los números medidos.

## Capabilities

### New Capabilities

Ninguna.

### Modified Capabilities

- `c-code-generation`: el subcomando gana el modo de ROM completa, la salida dividida por banco, el stub con informe para las funciones no generables y el gancho `rt_unsupported` en la interfaz con el runtime.

## Impact

- `src/codegen.rs`: generación de todas las funciones con recogida de errores por función; el modo con dirección y profundidad no cambia.
- `src/main.rs`: opción `--all`, escritura de varios ficheros y resumen; texto de uso.
- `runtime/snes_rt.h`: declaración de `rt_unsupported`.
- `src/cctest.rs`: soporte de test para compilar varios ficheros y enlazar sin ejecutar.
- `CLAUDE.md`: el comando nuevo.
- Fuera de alcance: runtime real (PPU, DMA, APU), ejecutar el juego completo, resolver los conflictos de anchos o las llamadas sin retorno que aparezcan (se miden y se apuntan para cambios posteriores), optimizar el C, especializar por anchos.
