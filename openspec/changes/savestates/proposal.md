## Why

El motor solo guarda la SRAM (`.srm`). Falta lo que más se echa de menos al jugar un port: guardar y cargar el estado en cualquier momento, y rebobinar. No basta con volcar WRAM, VRAM y la PPU: en el C generado cada `JSR` es una llamada nativa y cada contexto de interrupción corre sobre su propia pila nativa, así que parte del estado vive en la pila de C y no se puede serializar de forma portable.

## What Changes

- Un punto de reanudación declarado por el perfil del juego (`[resume]` en el `.toml`: inicio de un bloque del bucle principal, comprobado a mano contra la ROM; el generador emite `rt_resume(c)` ahí). Solo se puede guardar cuando la ejecución pasa por él, con la pila del 65816 poco profunda y sin NMI/IRQ en curso.
- Serialización del estado de hardware en ese punto: CPU, WRAM, SRAM, VRAM, CGRAM, OAM, registros de la PPU, DMA/HDMA, contadores de tiempo y entrada, SPC700 (RAM, registros, puertos, temporizadores) y DSP (voces, eco, ruido).
- Al cargar: en ese mismo punto se restauran los globales y los registros del `Cpu` y el juego sigue; la pila nativa tiene siempre la misma forma ahí, así que no se deshace nada.
- Ranuras de save state (varias por ROM, junto a la ROM), teclas para guardar y cargar, y rebobinado: un anillo de estados comprimidos, uno cada pocos fotogramas, que se recorre hacia atrás mientras se mantiene una tecla.
- Un perfil sin `[resume]` no ofrece save states ni rebobinado, y el menú lo dice.
- Pruebas: guardar, avanzar N fotogramas, cargar y repetir da el mismo fotograma y el mismo audio; un estado de otra ROM o versión se rechaza.

## Capabilities

### New Capabilities

- `savestates`: punto de reanudación, formato del estado, ranuras, carga y rebobinado.

### Modified Capabilities

- `game-profiles`: nueva sección opcional `[resume]`.
- `menu-ui`: teclas y filas de save states y rebobinado.

## Impact

- `runtime/snes_core.inc` (serializar/restaurar, reentrada), `runtime/snes_spc.inc` y `runtime/snes_ppu.inc` (su estado), `runtime/play.c` y `play_cfg.h` (teclas, ranuras, rebobinado), `src/config.rs` y `src/codegen.rs` (`[resume]`), `src/headless.rs` (pruebas).
- `alttp-recomp`: declarar `[resume]` en `games/alttp_usa.toml`, tras comprobar la dirección en la ROM.
- Fuera de alcance: guardar en mitad de una interrupción o en cualquier punto, estados entre versiones distintas del motor, netplay.
