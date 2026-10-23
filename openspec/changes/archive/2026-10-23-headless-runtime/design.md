## Context

El C generado solo toca el mundo exterior por `snes_rt.h`: `bus_read8`/`bus_write8` y los ganchos (`rt_unresolved`, `rt_unsupported`, `rt_flag_mismatch`, `rt_wai`, `rt_stp`, `rt_brk`, `rt_cop`). Hoy hay dos implementaciones, ambas dentro de tests: `MOCK_RUNTIME` (memoria plana de 16 MiB) y `ALTTP_BUS` (LoROM con WRAM, un doble del audio y la detección del bucle de espera del NMI). El código recompilado no lleva contador de programa ni ciclos, así que no hay forma natural de interrumpirlo: un NMI solo puede entrar entre bloques. `cctest` compila con MSVC o GCC/Clang y ejecuta el programa, pero solo escribe ficheros de texto y no pasa argumentos. Ver `proposal.md` para la motivación.

## Goals / Non-Goals

**Goals:**
- Un runtime en C que ejecute la ROM completa generada desde el vector de reset, reutilizable por los tests y por el usuario.
- Entregar el NMI con el mismo efecto observable que en la CPU (pila, registros, `RTI`), sin tocar la semántica de las instrucciones.
- Un informe que diga qué hace el código con el hardware y por qué se detiene.

**Non-Goals:**
- Fidelidad de temporización (ciclos, línea de exploración, HDMA), PPU, APU real, DMA con transferencia, IRQ, mando, SRAM persistente.
- Que el binario de Rust compile y ejecute el C; `cctest` sigue siendo solo para tests.

## Decisions

1. **Los fotogramas se miden en bloques ejecutados.** El punto de sondeo `rt_tick(c)` va al inicio de cada bloque, así que un bucle de espera (`goto` hacia atrás) también avanza el reloj. Alternativas: contar accesos al bus, descartada porque interrumpiría a mitad de una instrucción con un estado parcial; llamar al NMI solo en `WAI` y en lecturas de `$4210`/`$4212`, descartada porque ALttP espera con un bucle sobre WRAM (`$12`), no con esos registros. El coste (una llamada por bloque) solo existe con `SNES_RT_TICK`; sin él `rt_tick` es `((void)0)`, así que los tests existentes y su C no cambian salvo por esa línea.
2. **NMI entre bloques con una pila de verdad.** Al terminar un fotograma, `rt_tick` empuja banco de programa (nativo), dirección de retorno ficticia y `P`, pone `irq=1`, `dec=0` y llama al manejador por dirección; su `RTI` (`rt_rti`) deshace exactamente eso. El manejador de ALttP guarda y restaura los registros que usa, igual que con la CPU real. Como no hay PC, la dirección de retorno apilada es un valor fijo que nadie lee. Se asume que el código no inspecciona la dirección de retorno de una interrupción; si lo hace, se descubrirá en la ejecución. Antes de llamar al manejador el runtime fija M=X=8 sin tocar los registros, porque el recompilador analiza el manejador desde su entrada con esos anchos (su `[[entry]]`); el `P` apilado conserva los anchos reales y el `RTI` los restaura. Vale mientras el manejador fije sus anchos en las primeras instrucciones, como hace el de ALttP (`REP #$30`).
3. **Despacho por dirección generado.** `snes_entry.c` contiene un `switch` sobre las ~2.500 entradas. Hace falta para llamar al reset y a los vectores de NMI, que el runtime lee de la ROM en tiempo de ejecución y no conoce el generador. Alternativa: tabla de punteros a función; equivalente, y se prefiere el `switch` por ser el esquema ya usado para los saltos indirectos.
4. **ROM leída del disco, nunca incrustada.** `snes_headless.c` y `snes_entry.c` no contienen datos de la ROM, de modo que pueden vivir en `out/` sin violar la regla de no distribuirla. El runtime recibe la ruta como argumento y valida el tamaño. Para los tests, `cctest` aprende a escribir ficheros binarios y a pasar argumentos al programa.
5. **Registros de hardware: guardar, devolver y contar.** Un array de 0x300 bytes para `$2100-$43FF` y dos contadores por dirección. Solo tienen comportamiento propio `$4210` (el bit 7 se borra al leer), `$4212` (vblank durante el último 10 % del fotograma, bit 0 de autolectura de mando siempre 0), `$2140-$2143` (doble del audio de `ALTTP_BUS`, que ya arrancó el juego) y el puerto de WRAM `$2180-$2183`. Alternativa: devolver cero a todo lo no escrito; se descarta porque ocultaría registros de solo lectura que el juego sondea.
6. **Parada en el primer fallo.** Los ganchos de fallo imprimen el motivo y salen con un código por causa (`unresolved` 5, `flag_mismatch` 6, `stp/brk/cop` 7, `unsupported` 9; `ALTTP_BUS` ya usa 5-7). Tras ellos el estado de la CPU no es fiable, así que no se intenta seguir. El informe se imprime también en ese caso.
7. **`runtime/headless.c` incrustado con `include_str!`.** Igual que `snes_rt.h`. El `main` está dentro, de modo que compilar el directorio generado basta para obtener un programa.
8. **Sin cambios de semántica en las instrucciones.** El único cambio del generador es la línea `rt_tick(c);` por bloque y el fichero `snes_entry.c`; los tests que comparan cuerpos de función se ajustan a la línea nueva.

## Risks / Trade-offs

- [Un fotograma de N bloques no se parece al de la consola y el juego depende de la temporización] → N es un parámetro; se anota qué valor deja avanzar a ALttP y se documenta que no es un reloj fiel.
- [Dos aperturas del fotograma de ALttP (NMI mientras el manejador anterior sigue en curso)] → No se anidan los NMI: si el manejador no ha terminado al acabar el siguiente fotograma, se pospone al siguiente `rt_tick` fuera del manejador.
- [El juego espera algo de un registro que solo devuelve lo escrito y se cuelga] → Se detecta por un máximo de bloques sin avanzar de fotograma (la parada informa del último bloque), y el informe muestra qué registros sondeaba.
- [El runtime en C duplica a `ALTTP_BUS` de los tests] → El test de reset de ALttP pasa al runtime nuevo. `ALTTP_BUS` se conserva solo para `alttp_inline_helper_call_reaches_its_target`, que entra por una función cualquiera con la memoria preparada, algo que el runtime nuevo no ofrece; eliminarlo exigiría añadirle `--entry` y `--poke`, y queda para cuando haga falta.
- [Rendimiento de `rt_tick` por bloque] → Aceptable para un runtime de exploración; no es un objetivo optimizarlo.
