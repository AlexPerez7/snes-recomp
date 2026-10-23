## Context

`runtime/headless.c` guarda cada escritura a `$2100-$43FF` en un array `hw[]` y cuenta accesos; `$420B` solo incrementa `dma_starts[]`. Los registros de PPU que el código lee (`$2138-$213F`) devuelven el último byte escrito en ellos, lo que no tiene sentido para los puertos de lectura. ALttP, tras 5000 fotogramas, dispara DMA por los canales 0 a 4 cada fotograma (2508 veces el canal 0) y escribe `$2115-$2119`, `$2121-$2122` y `$2102-$2104`: es el tráfico que llena VRAM, CGRAM y OAM. No hay ciclos ni línea de exploración, así que no hay temporización de acceso que modelar.

## Goals / Non-Goals

**Goals:**
- Que lo que el juego envía a la PPU quede en memorias reales con las reglas de los puertos, y poder volcarlas.
- DMA completo en sus modos, porque es el camino de casi todos los datos gráficos.

**Non-Goals:**
- HDMA, renderizado, temporización de la PPU, restricciones de acceso fuera de vblank, ciclos de DMA, registros de multiplicación y división, APU real. Ver `proposal.md`.

## Decisions

1. **El DMA se ejecuta de forma síncrona en la escritura a `$420B`.** Es lo que ve el programa: la CPU se detiene durante el DMA. No hay que intercalar NMI ni sondeo (`rt_tick` no se llama dentro del C del runtime). Alternativa: ejecutarlo al final del bloque; descartada por añadir un estado pendiente sin ganar nada.
2. **El DMA usa las mismas funciones `bus_read8`/`bus_write8` que la CPU.** El lado A pasa por `bus_read8`/`bus_write8`; el lado B por la dirección `$2100 + B`. Así los efectos de los puertos (VRAM con incremento, `$2180` con su dirección) se aplican por el mismo camino y no hay una segunda implementación. Efecto secundario: el DMA también cuenta como acceso en el informe (lecturas y escrituras por registro); eso es lo que ocurre en la consola de cara al registro, y se distingue de los accesos de la CPU solo por los contadores propios de DMA. Alternativa: un camino aparte sin contadores; descartada para no duplicar los puertos.
3. **Tabla de patrones por modo.** Un array de desplazamientos por modo (`{0}`, `{0,1}`, `{0,0}`, `{0,0,1,1}`, `{0,1,2,3}`; los modos 5-7 alias de 1-3), recorrido por unidad hasta agotar el contador. Si el contador no es múltiplo de la unidad, la última unidad se corta, como en la consola.
4. **Dirección A de 16 bits dentro del banco.** `$43x2-$43x3` avanza y da la vuelta sin tocar `$43x4`. El contador `$43x5-$43x6` queda en 0 al terminar, y la dirección A final queda escrita en sus registros (los juegos la releen a veces).
5. **VRAM como array de 32 K palabras de 16 bits** con dirección de palabra de 15 bits. Una escritura en `$2118` cambia el byte bajo y una en `$2119` el alto; el avance ocurre según el bit 7 de `$2115`. La precarga de lectura se hace al fijar la dirección (`$2116/7`) y tras cada lectura que avanza. La traducción de direcciones de los bits 2-3 de `$2115` (usada por pocos juegos) no se implementa; se anota y, si el juego la activa, el informe lo avisa (ver Riesgos).
6. **CGRAM de 512 bytes con un latch.** La escritura del byte bajo guarda el latch; la del alto escribe el color y avanza el índice. Fijar `$2121` pone el índice y descarta el latch. El bit 7 del byte alto se fuerza a 0. La lectura (`$213B`) sigue el mismo orden.
7. **OAM de 544 bytes con dirección de byte interna.** `$2102/3` fijan la dirección de palabra (se multiplica por 2). En la tabla baja se confirma por pares con un latch, como la CGRAM; en la alta (dirección ≥ 512) se escribe directo y la dirección da la vuelta cada 32 bytes. El bit de prioridad de `$2103` y la reiniciación de la dirección al terminar el vblank no se modelan.
8. **Volcado con `--dump <dir>`.** Se escribe en `report()`, así que también ocurre al detenerse por un fallo. Es un directorio ya existente. Los ficheros derivan de la ROM, así que se escriben donde diga el usuario (en la práctica `out/`, ya ignorado); el runtime no los sube a ningún sitio.
9. **Lecturas de PPU.** `$2138`, `$2139`, `$213A` y `$213B` pasan a devolver el dato del puerto; los demás registros de `$2134-$213F` siguen devolviendo el último valor escrito (multiplicación `$2134-$2136` y contadores `$213C-$213F` quedan fuera de alcance).

## Risks / Trade-offs

- [El juego usa la traducción de direcciones de VRAM] → ALttP lo hace en pocas pantallas; si `$2115` tiene los bits 2-3 distintos de 0, el informe lo indica y la prueba de ALttP lo mide. Si ocurre, se añade en un cambio posterior.
- [El DMA sin ciclos reordena accesos respecto a la consola] → no es observable desde el código recompilado, que no tiene ciclos; aceptable para este runtime.
- [Una pantalla de ALttP escribe VRAM fuera de vblank y en la consola se ignoraría] → no se modela la restricción; el informe no distingue entonces escrituras válidas de ignoradas. Se anota como límite.
- [Un canal con contador 0 transfiere 65 536 bytes] → es el comportamiento real; hay que tenerlo en cuenta al leer el informe de bytes por canal.
