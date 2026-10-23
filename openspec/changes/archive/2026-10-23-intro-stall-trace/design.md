## Context

En la investigación de las letras con barras hubo que parchear a mano el C generado de ALttP (añadir `fprintf` a funciones concretas, un histograma de bloques en `rt_poll` y un `g_block` en `rt_check_flags`) para saber quién escribía un búfer y qué leía el juego. Ese andamiaje desapareció al regenerar `out/`. Las mediciones que dio: casi todo el tiempo se va en el bucle de espera del NMI (`00:8030`), el juego lee `$2140`/`$2141` una vez por fotograma tras la subida del audio y `$4218/$4219` cada dos fotogramas, y `$10/$11` se quedan en `00/03`. Las hipótesis (la APU, el mando, un contador) no se pueden confirmar sin conocer el bloque que lee y el valor que obtiene.

## Goals / Non-Goals

**Goals:**
- Saber, para una ventana de fotogramas, qué bloques leen qué registros de hardware y con qué valores, y quién cambia ciertas variables de WRAM.
- Que no cueste nada ni cambie nada cuando no se pide, incluidos los runtimes de los tests de `codegen`.

**Non-Goals:**
- Un depurador o un trazado de instrucciones; trazar escrituras a hardware; cambiar el hardware simulado salvo que sea la causa del atasco.

## Decisions

1. **La dirección del bloque sale de `rt_check_flags`.** Esa llamada ya está al principio de cada bloque con su dirección y se compila a nada sin `SNES_RT_CHECKS`. Con `SNES_RT_TRACE` guarda la dirección en una variable que declara el propio `snes_rt.h` (`static` no sirve: es una por unidad de traducción) como `extern uint32_t rt_block;`; el runtime la define. Sin `SNES_RT_TRACE` nada cambia. Alternativa: pasar la dirección a `rt_tick`; descartada porque cambia el texto de todos los bloques y los tests que lo comparan. El valor es el del último bloque *iniciado*, así que tras volver de una función la lectura se atribuye al bloque de la llamada o al de retorno según dónde empiece el siguiente; para este diagnóstico basta (la lectura ocurre dentro del bloque en curso, que es el último iniciado).
2. **`rt_check_flags` guarda también con `SNES_RT_CHECKS` apagado.** El macro vacío del caso sin comprobaciones se sustituye, solo con `SNES_RT_TRACE`, por una función inline que guarda la dirección. Así la traza no depende de las comprobaciones de anchos.
3. **Ventana y traza viven en el runtime.** El fotograma actual ya existe (`frames`). `trace_on()` es verdadero si hay ventana y `frames` está dentro. Las lecturas se agrupan en una tabla pequeña por fotograma (bloque, registro, valor) con contador y se vuelcan al cambiar de fotograma y al terminar; una tabla de unos cientos de entradas basta porque en un fotograma suele haber pocas combinaciones distintas.
4. **Las lecturas del DMA no se atribuyen al bloque.** El DMA lee el bus B con las mismas funciones que la CPU (decisión de `ppu-runtime`), así que se baja un indicador `dma_active` (ya existía en la depuración) para no anotarlas: no las hizo el código, las hizo el DMA.
5. **`--watch` guarda direcciones como offsets de WRAM de 17 bits.** `0000-1FFF` se interpreta como el espejo bajo (offset igual) y `7E/7F:xxxx` como `(banco & 1) << 16 | xxxx`, de modo que `$0011` y `7E:0011` son el mismo byte. La comprobación es un array de banderas de 128 KiB; la escritura compara el valor antiguo con el nuevo antes de anotar. Los DMA a WRAM pasan por `bus_write8`, así que se anotan con el bloque en curso (el que disparó el DMA).
6. **Salida en la salida estándar, con prefijo.** Las líneas llevan un prefijo (`trace read`, `trace write`) para filtrarlas fácilmente con `Select-String` o `grep`, y van antes del informe.
7. **La investigación es parte del cambio.** Las tareas incluyen usar la herramienta sobre ALttP y anotar qué espera el submódulo 3. La corrección del hardware simulado, si es esa la causa, es una tarea condicionada: se añade y se prueba aquí; si la causa es del recompilador, se anota la dirección y el cambio se cierra sin corregirla.

## Risks / Trade-offs

- [La traza imprime demasiado y oculta lo importante] → ventana corta y agrupación de repeticiones; el uso previsto es unos pocos fotogramas.
- [La atribución al «último bloque iniciado» es imprecisa tras una llamada] → suficiente para el diagnóstico; se anota como límite.
- [La causa no está en el hardware simulado] → el cambio se cierra igualmente con la herramienta y el hallazgo documentado; no se corrige nada que no se haya demostrado como causa.
- [`rt_block` global obliga a los runtimes que usen `SNES_RT_TRACE` a definirlo] → solo el runtime sin vídeo lo usa; los demás no definen `SNES_RT_TRACE`.
