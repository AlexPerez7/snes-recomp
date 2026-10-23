## Why

ALttP se queda en el módulo 0, submódulo 3 (`$10 = 00`, `$11 = 03`) desde el fotograma ~60 hasta más de 4000: el contador de fotogramas avanza, los NMI se entregan y en `$0000-$1FFF` casi nada cambia. El juego espera algo que nuestro hardware simulado no da, o hay un fallo del código recompilado. Hoy no hay forma de saber qué: el runtime cuenta accesos por registro pero no dice quién los hace ni cuándo, y no puede vigilar una variable de WRAM. Sin eso, cada hipótesis (la APU, el mando, un temporizador) obliga a parchear a mano el C generado, como hubo que hacer en la investigación anterior. Un trazado mínimo y permanente del runtime convierte la pregunta en una lectura y deja la herramienta para los atascos siguientes.

## What Changes

- El C generado, compilado con `SNES_RT_TRACE`, deja disponible la dirección del bloque en curso para el runtime (sin él no cuesta nada ni exige ningún símbolo, de modo que los runtimes de los tests no cambian).
- Nueva opción `--trace-frames A-B`: ventana de fotogramas (contados desde 0) en la que el runtime imprime la traza; fuera de ella, o sin la opción, no imprime nada.
- Nueva opción `--trace-reads`: en la ventana, cada lectura de un registro de hardware (`$2100-$43FF`) se anota con el bloque que la hizo, el registro y el valor devuelto, agrupando las repeticiones idénticas de cada fotograma en una línea con su contador.
- Nueva opción `--watch ADDR` (repetible): en la ventana, cada escritura que cambie un byte de WRAM vigilado se anota con el bloque, el valor antiguo y el nuevo.
- Se usa para averiguar qué espera el submódulo 3 del módulo 0 de ALttP. Si la causa es una simplificación del hardware simulado (la APU, el mando, el estado de un registro), se corrige en este cambio con una prueba que la cubre; si es un fallo del código recompilado, se documenta con la dirección y se deja para un cambio aparte.

## Capabilities

### New Capabilities

- `execution-trace`: trazado opcional de las lecturas de hardware y de las escrituras a WRAM vigiladas durante una ventana de fotogramas, con el bloque que las provoca.

### Modified Capabilities

(ninguna; si la investigación obliga a cambiar el hardware simulado, se añade el requisito correspondiente a `headless-runtime` como delta de este cambio)

## Impact

- `runtime/snes_rt.h`: `rt_check_flags` guarda la dirección con `SNES_RT_TRACE`.
- `runtime/headless.c`: ventana de fotogramas, trazas de lecturas y vigilancia de WRAM, opciones nuevas.
- `src/headless.rs`: tests de las trazas con ROMs pequeñas; `build_and_run` define `SNES_RT_TRACE`.
- `CLAUDE.md`: documenta las opciones de traza.
- Fuera de alcance: trazar escrituras a hardware, trazado de instrucciones o de todos los bloques, un depurador interactivo, y cualquier cambio de hardware simulado que no sea la causa del atasco.
