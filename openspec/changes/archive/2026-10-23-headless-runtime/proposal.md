## Why

`codegen --all` ya deja el C de todo ALttP compilando y enlazando, pero solo se ha ejecutado `reset` y los helpers, con un bus de prueba escrito dentro de un test. No se sabe qué pasa al correr el juego entero: dónde caen los 7 stubs, qué llamadas sin retorno importan, qué saltos quedan sin resolver y qué registros de hardware toca el código. Un runtime mínimo sin vídeo ni audio, que ejecute la ROM completa desde el vector de reset durante unos fotogramas, convierte esas incógnitas en una lista medida y marca por dónde seguir (PPU, DMA, APU o arreglos del recompilador).

## What Changes

- Nuevo runtime en C (`runtime/headless.c`, se incrusta en el binario como `snes_rt.h`) con:
  - bus LoROM real: ROM cargada desde un fichero en tiempo de ejecución, WRAM de 128 KiB con su espejo en `0000-1FFF`, SRAM y puerto de WRAM (`$2180-$2183`);
  - registros de hardware que guardan lo escrito y lo devuelven al leer, con los de estado (`$4210`, `$4212`) simulados, y un doble del procesador de audio que responde al protocolo de arranque;
  - modelo de fotogramas por número de bloques ejecutados: al final de cada fotograma entra el NMI si `$4200` lo habilita, y `WAI` avanza al siguiente fotograma;
  - informe final con fotogramas, NMIs, accesos por registro, escrituras a DMA/HDMA y la causa de parada.
- La ejecución arranca en el vector de reset leído de la ROM y termina tras `--frames N` fotogramas o en el primer `rt_unresolved`, `rt_unsupported`, `rt_flag_mismatch`, `STP`, `BRK` o `COP`, indicando dirección y código de salida.
- El C generado gana un punto de sondeo por bloque (`rt_tick`, desactivado salvo con `SNES_RT_TICK`) y `codegen --all` escribe además `snes_entry.c`, que permite llamar a una función por su dirección (para el reset y los vectores), y `snes_headless.c`.
- Verificación: tests con ROMs pequeñas que comprueban cada comportamiento, y una ejecución de ALttP completo cuyos resultados (fotogramas alcanzados, causa de parada, registros más usados) se anotan.

## Capabilities

### New Capabilities

- `headless-runtime`: ejecución del C generado sin vídeo ni audio, con bus LoROM, registros de hardware simulados, fotogramas con NMI e informe de la ejecución.

### Modified Capabilities

- `c-code-generation`: punto de sondeo opcional al entrar en cada bloque, despacho por dirección de entrada, y los ficheros de apoyo que escribe `--all`.

## Impact

- `runtime/headless.c` (nuevo), `runtime/snes_rt.h` (declaración de `rt_tick`).
- `src/codegen.rs`: `rt_tick` al inicio de cada bloque, `snes_entry.c` en `FullOutput`.
- `src/main.rs`: `--all` escribe `snes_entry.c` y `snes_headless.c`.
- `src/cctest.rs`: soporte de test para ficheros binarios y argumentos del programa.
- `CLAUDE.md`: cómo compilar y ejecutar el runtime sobre la salida de `--all`.
- Fuera de alcance: PPU, vídeo, APU/SPC700 real, DMA con transferencias reales, entrega de IRQ (solo se registran sus escrituras), temporización exacta de CPU o ciclos, entrada de mando, guardado de SRAM, arreglar las causas de parada que aparezcan (se miden y se apuntan), y que `snes-recomp` compile y ejecute el C por sí mismo.
