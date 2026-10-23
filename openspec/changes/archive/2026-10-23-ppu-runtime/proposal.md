## Why

El runtime sin vídeo ni audio ya ejecuta ALttP durante 5000 fotogramas sin detenerse, pero el juego no hace nada observable: el DMA solo se cuenta y los registros de PPU solo guardan el último byte escrito. Los gráficos del juego viajan por DMA a VRAM, CGRAM y OAM, así que sin esas memorias no hay forma de saber si el código recompilado produce datos correctos (tiles, paletas, sprites) ni de montar luego un renderizador. Este cambio da al runtime esas tres memorias y el DMA que las llena, y permite volcarlas a ficheros para inspeccionarlas.

## What Changes

- El DMA de propósito general (`$420B`) pasa a transferir de verdad: los ocho canales con sus registros (`$43x0-$43x6`), los patrones de los modos 0 a 7, dirección A→B y B→A, dirección A fija, incrementando o decrementando, y el destino en los registros `$21xx` o en el puerto de WRAM.
- Nuevos puertos de PPU con memoria real: VRAM (`$2115-$2119`, lectura por `$2139/$213A` con precarga), CGRAM (`$2121`, `$2122`, `$213B`) y OAM (`$2102-$2104`, `$2138`), con sus reglas de incremento, de byte alto/bajo y de la tabla alta de OAM.
- Nueva opción `--dump <dir>`: al terminar escribe `vram.bin`, `cgram.bin`, `oam.bin` y `ppu_regs.bin` (el último valor de `$2100-$213F`), y el informe indica cuántos bytes transfirió el DMA por canal.
- HDMA sigue sin ejecutarse (solo se cuenta); ver fuera de alcance.
- Verificación: tests con ROMs pequeñas para cada modo de DMA y cada puerto, y una ejecución de ALttP cuyas memorias volcadas se comprueban (VRAM, paleta y OAM no vacías tras unos fotogramas) y se anotan.

## Capabilities

### New Capabilities

- `dma-transfers`: transferencia de datos del DMA de propósito general entre el bus A y los registros del bus B, con todos sus modos.
- `ppu-memory`: VRAM, CGRAM y OAM de la PPU accesibles por sus puertos, y volcado de su contenido.

### Modified Capabilities

- `headless-runtime`: el informe incluye los bytes transferidos por el DMA y el runtime admite `--dump`.

## Impact

- `runtime/headless.c`: DMA, puertos de PPU, volcado y opción `--dump`.
- `src/headless.rs`: tests nuevos de DMA y PPU; la prueba de ALttP comprueba las memorias volcadas.
- `CLAUDE.md`: la línea de ejecución documenta `--dump`.
- Fuera de alcance: HDMA, renderizado (fondos, sprites, modos, mosaico, ventanas), temporización de la PPU (vblank real, acceso a VRAM solo en vblank, DMA que consume ciclos), registros de multiplicación y división del hardware, la APU real, el mando y que el binario de Rust ejecute el C.
