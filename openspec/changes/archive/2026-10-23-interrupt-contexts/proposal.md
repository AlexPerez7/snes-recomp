## Why

ALttP se queda en el módulo 0, submódulo 3, porque su intro no es un único hilo de ejecución. Con `$012A != 0`, el NMI (`00:822D`, acaba en `00:82C9-82D7`) y el IRQ (`00:82D8`, rama `00:831F-8332`) intercambian la pila con `$1F0A` y terminan en `RTI`, de modo que cada interrupción devuelve el control al *otro* contexto, el que dejó guardado la interrupción anterior. La investigación de `intro-stall-trace` mostró que el submódulo 3 no espera ningún registro: el contexto que hace avanzar `$1F00` y la máquina de estados de la Trifuerza no llega a ejecutarse.

El runtime tiene dos carencias que lo impiden. No entrega IRQ (el requisito vigente dice que no se entrega ninguna). Y el C generado traduce `RTI` como un `return` nativo (`rt_rti` solo saca de la pila emulada el estado y la dirección), así que la pila emulada y la nativa se desincronizan en cuanto un manejador cambia `S`. Sin resolver ambas cosas, la intro, y probablemente más código de ALttP, no puede avanzar.

## What Changes

- El runtime entrega la IRQ de la SNES: se habilita con los bits 4-5 de `$4200`, se dispara en la línea indicada por `$4209/$420A` (IRQ vertical y combinada; la posición horizontal se ignora), se anuncia en el bit 7 de `$4211` y esa lectura la limpia. Respeta la bandera I y llega después del NMI del mismo fotograma.
- Un `RTI` que no devuelve al contexto que se interrumpió cambia de contexto de ejecución. Cada contexto es una pila nativa propia (fibras en Windows, `ucontext` en POSIX). El runtime lo detecta comparando el `S` del `RTI` con el que tenía el manejador al entrar: reanuda el contexto suspendido cuyo `S` coincide o, si no lo hay, arranca uno nuevo en la dirección que lee de la trama de pila.
- `rt_rti` pasa a ser un gancho del runtime (hoy es una función `static inline` del header) para poder hacer ese cambio sin que el generador cambie su salida.
- Se comprueba contra la intro de ALttP: el submódulo 3 deja de ser terminal. Si la intro tropieza con algo más que no es de este cambio, se anota qué.

## Capabilities

### New Capabilities

- `execution-contexts`: varios contextos de ejecución del 65816 (cada uno con su pila nativa) y cambio entre ellos cuando un manejador de interrupción termina en `RTI` sobre otra pila.

### Modified Capabilities

- `headless-runtime`: el requisito de fotogramas y NMI pasa a incluir la entrega de la IRQ.

## Impact

- `runtime/snes_rt.h`: `rt_rti` deja de ser inline y lo define el runtime.
- `runtime/headless.c`: temporización y estado de la IRQ, `$4211`, contextos y cambio de contexto.
- `src/headless.rs`: se sustituye el test `irq_enable_does_not_deliver_anything_and_is_reported` y se añaden tests de IRQ y de cambio de contexto con ROMs pequeñas; el test de ALttP comprueba que `$11` pasa de 3.
- `src/codegen.rs`: solo si los runtimes de prueba de `codegen` necesitan el nuevo `rt_rti`.
- Fuera de alcance: IRQ horizontal exacta y posición H/V al ciclo, `WAI`/`STP` en contextos distintos del principal, más de unos pocos contextos simultáneos y cualquier otra mejora de la intro.
