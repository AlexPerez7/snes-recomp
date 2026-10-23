## Context

`intro-stall-trace` localizó el atasco de la intro de ALttP. Con `$012A != 0`, tanto el NMI (`00:82C7-82D7`) como el IRQ (`00:831F-8332`) hacen `TSC; TAX; LDA $1F0A; TCS; STX $1F0A; PLB; PLD; PLY; PLX; PLA; RTI`: guardan su `S` en `$1F0A`, cargan el que dejó la otra interrupción y devuelven a *ese* contexto. El IRQ vertical está en la línea `$FF` (`$4209`, `$4200 = $A1`). Son dos hilos cooperativos que se alternan entre vblank y el resto del fotograma.

Estado actual del runtime: `deliver_nmi` empuja la trama (PBR si es nativo, PC, P), pone M/X a 8 bits y llama a `snes_call_entry(c, vector)`; el manejador compilado termina en `rt_rti` + `return`, y `deliver_nmi` devuelve a `rt_poll`. `rt_rti` solo saca P, PC (y PBR) de la pila emulada. Ese modelo es correcto mientras el manejador devuelva con la pila con la que entró, y falla si no.

## Goals / Non-Goals

**Goals:**
- Que NMI e IRQ puedan intercambiar contextos como en el hardware, con el código recompilado sin cambios.
- Entregar la IRQ con la temporización mínima que ALttP necesita.
- Que el caso de un solo contexto siga siendo el de ahora, con el mismo coste.

**Non-Goals:**
- Exactitud al ciclo, IRQ horizontal exacta, `WAI`/`STP` repartidos entre contextos, más de unos pocos contextos.

## Decisions

1. **Un contexto es una pila nativa.** Fibras de Windows (`ConvertThreadToFiber`, `CreateFiber`, `SwitchToFiber`) y `ucontext` (`getcontext`, `makecontext`, `swapcontext`) en POSIX, detrás de tres funciones (`ctx_init`, `ctx_create(fn, arg)`, `ctx_switch(from, to)`) en `headless.c`. Alternativas descartadas: hilos con semáforos (funcionan en todas partes pero son más pesados y se hacen más difíciles de depurar) y reescribir el generador para que cada función guarde su continuación (cambio enorme, y la salida de `codegen` dejaría de ser legible). El generador no cambia.
2. **Un `RTI` es un cambio de contexto si su `S` no es el de entrada del manejador.** Al entregar una interrupción el runtime anota `entry_s` (la `S` tras empujar la trama). En `rt_rti`, tras los `PLB…PLA` del código, `c->s` ya apunta a la trama que se va a sacar. Si `c->s == entry_s` es un `RTI` normal. Si no, el código ha cambiado de pila.
3. **Contextos suspendidos, por `S`.** Un contexto que cambia de pila queda suspendido dentro de su `rt_rti` y se registra con la `S` de su trama (el valor que `STX $1F0A` dejó para el otro). Cuando otro `RTI` llega con esa misma `S`, se reanuda el contexto: la `S` del `RTI` es la clave. Se admiten hasta 4 suspendidos a la vez; más es un error con mensaje claro.
4. **Si no hay contexto con esa `S`, se crea uno.** El primer cambio de ALttP carga una pila que el juego preparó antes con una trama fabricada, no una que haya dejado una interrupción. Se saca la trama de la pila emulada (P, PC, PBR) y se crea un contexto cuya función de entrada pone P y llama a `snes_call_entry(pbr:pc)`. Si no es una función se detiene la ejecución con el mismo mensaje que para los vectores. Si la función retorna (un contexto que sale por `RTS`/`RTL` sin `RTI`), se detiene con una causa propia, porque no hay a dónde volver.
5. **Quien sigue ejecutando es quien se reanudó.** Al cambiar, el estado de la CPU (`Cpu c`, que es único y compartido por los contextos) no se toca: lo que el manejador dejó en los registros y la pila emulados es lo que ve el contexto reanudado. Esa es la semántica del hardware.
6. **`rt_rti` pasa a gancho.** Se declara en `snes_rt.h` (`void rt_rti(Cpu *c)`) y lo define `headless.c`; la implementación actual (sacar P, PC y PBR) pasa al runtime y se amplía con lo anterior. Los runtimes de prueba de `codegen` que usan `rt_rti` definen la versión simple.
7. **IRQ.** Modelo: `$4200` bits 4-5 distintos de cero, línea `V = $4209 | ($420A & 1) << 8`. Para que llegue después del NMI, la posición dentro del fotograma se cuenta desde el NMI: línea 225 es el inicio (`tick_in_frame = 0` tras el NMI), así que `irq_at = frame_len * ((V - 225 + 262) % 262) / 262` bloques. El modo H solo (bit 4 sin bit 5) se trata como una vez por fotograma en la posición V = 0 al no tener una posición vertical que cumplir; es una aproximación que se documenta. La IRQ se marca pendiente al alcanzar `irq_at`, y `$4211` bit 7 la refleja y la limpia al leer. Se entrega entre bloques si I = 0 y no hay otro manejador activo. El NMI tiene prioridad y puede entrar dentro de una IRQ, como en el hardware.
8. **La entrega de IRQ usa el mismo camino que el NMI:** empujar la trama, I = 1, M/X a 8 bits y llamar al vector de IRQ (`FFEE` nativo, `FFFE` en emulación). Las dos entregas comparten una función.
9. **Verificación.** Tests con ROMs pequeñas: IRQ vertical una vez por fotograma tras el NMI; `$4211`; I = 1; contextos que se intercambian (NMI e IRQ con el patrón de ALttP reducido, comprobando el orden de ejecución con escrituras a registros); contexto nuevo desde una trama fabricada; dirección que no es función; contexto que retorna. En ALttP, el test pasa a exigir que `$11` sea mayor que 3 en algún momento de 300 fotogramas, o, si la intro tropieza con otra cosa, que se documente.

## Risks / Trade-offs

- [El `S` como clave puede coincidir sin querer con la de otro contexto] → es el mismo criterio con el que el código original distingue sus contextos; se detecta una colisión entre suspendidos y se aborta con mensaje.
- [Las fibras de Windows y `ucontext` no se prueban igual] → una sola implementación de las tres funciones por plataforma, con los mismos tests; en esta máquina se prueba Windows y la rama POSIX se compila solo si hay compilador POSIX (si no, queda sin probar y se anota).
- [El tamaño de pila nativa] → el C generado puede anidar bastantes llamadas; cada contexto lleva 1 MiB de pila, un número que se documenta y se puede cambiar.
- [ALttP puede tropezar con otra cosa después de la intro] → es esperable. El criterio de cierre es que el submódulo 3 deje de ser terminal, no que el juego llegue al título; lo siguiente se anota.
- [Aproximar la IRQ horizontal] → se documenta; ALttP usa la vertical.
