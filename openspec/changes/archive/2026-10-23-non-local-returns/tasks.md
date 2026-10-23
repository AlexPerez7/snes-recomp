## 1. Generador y cabecera

- [x] 1.1 Añadir a `Cpu` en `runtime/snes_rt.h` el campo `uint32_t ret` y la constante `RT_NO_RET` (`0xFFFFFFFF`), y declarar el gancho `rt_lost_return(Cpu *c)`; los runtimes de prueba de `codegen` y `cctest` lo definen (parada con mensaje)
- [x] 1.2 Emitir en `src/codegen.rs` el `RTS` como sacar la dirección, guardarla en `c->ret` y `return`, y el `RTL` igual con el banco sacado y descartado; verificar con un test que lee `c->ret` tras un `RTS` y un `RTL`
- [x] 1.3 Emitir las llamadas (`JSR`/`JSL` directos e indirectos) con `uint16_t s0 = c->s` antes del empuje y, tras la llamada, `if (c->s != s0 || c->ret != 0x<RET>) return;`; verificar con tests de llamada normal (la ejecución continúa) y de `PLA`/`PLA`/`RTS` en dos niveles (la ejecución continúa en el llamador exterior y la función intermedia no ejecuta lo que sigue a su llamada)
- [x] 1.4 Verificar con tests que no cambian los tail calls ni las salidas de helpers en línea, y que una dirección distinta con la misma pila (sacar y empujar otra antes del `RTS`) no continúa en la instrucción siguiente

## 2. Runtime sin vídeo

- [x] 2.1 En `runtime/headless.c`, poner `c->ret = RT_NO_RET` antes de llamar a una función de nivel superior (reset, manejadores de interrupción, cuerpo de un contexto) y llamar a `rt_lost_return` si al volver ya no vale `RT_NO_RET`, parando con código distinto de 0, causa, dirección y `S`; verificar con un test que ejecuta un `RTS` de nivel superior con una dirección de la pila que ninguna llamada espera
- [x] 2.2 Verificar que los tests existentes del runtime (NMI, IRQ, contextos) siguen en verde

## 3. ALttP y documentación

- [x] 3.1 Regenerar ALttP y comprobar que el fotograma 6889 ya no falla (`$10`/`$11` y `S` coherentes en el despachador) y anotar en este fichero hasta qué fotograma llega ahora y la causa de la siguiente parada, si la hay, sin ampliar el alcance
- [x] 3.2 Actualizar `CLAUDE.md` y la memoria de proyecto; `cargo test` completo en verde

## Resultado

Con el retorno no local, ALttP supera el fotograma 6889 y ejecuta 20000 fotogramas sin parar. Se ve la demostración del modo atracción (Link en el castillo, fotograma 6900) y el ciclo vuelve a las pantallas de historia de la intro (fotograma 20000). Sin mandos conectados el juego no sale de ese ciclo; lo siguiente es simular la entrada del mando para llegar a la selección de partida. El runtime sin vídeo trata ahora un `RTS` de nivel superior (reset, manejador o contexto) como retorno perdido (salida 14), en lugar de «reset returned»/«context returned», que quedan para el resto de casos.
