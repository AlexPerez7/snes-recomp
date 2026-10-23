## 1. IRQ

- [x] 1.1 Implementar la IRQ en `runtime/headless.c`: línea de `$4209/$420A`, posición contada desde el NMI, estado pendiente, bit 7 de `$4211` que se limpia al leer, entrega entre bloques con la bandera I a 0 y el vector `FFEE`/`FFFE`, compartiendo con el NMI la función que empuja la trama; verificar con tests de IRQ vertical una vez por fotograma tras el NMI, lectura de `$4211`, bandera I a 1 sin pérdida y bits 4-5 a 0 sin entrega
- [x] 1.2 Sustituir el test `irq_enable_does_not_deliver_anything_and_is_reported` por los anteriores y actualizar el comentario de cabecera de `headless.c`

## 2. Contextos de ejecución

- [x] 2.1 Añadir `ctx_init`, `ctx_create` y `ctx_switch` (fibras en Windows, `ucontext` en POSIX) en `headless.c`, con pila de 1 MiB por contexto y un máximo de 4 suspendidos
- [x] 2.2 Hacer de `rt_rti` un gancho (`snes_rt.h` lo declara, `headless.c` lo define, los runtimes de prueba de `codegen` definen la versión simple) y anotar la `S` de entrada en cada entrega de interrupción; verificar que un `RTI` normal se comporta como antes (los tests de NMI siguen en verde)
- [x] 2.3 Implementar el cambio de contexto en `rt_rti`: reanudar el suspendido con la misma `S`, o crear uno nuevo desde la trama fabricada (con el error de dirección que no es función y la parada si un contexto retorna); verificar con tests del intercambio de pila entre NMI e IRQ, del contexto nuevo, de la dirección que no es función y de los registros que pasan intactos

## 3. ALttP

- [x] 3.1 Regenerar y ejecutar ALttP: comprobar que el submódulo 3 deja de ser terminal (`$11` supera 3) y actualizar el test de ALttP; si la intro tropieza con otra cosa, localizarla con `--trace-reads`/`--watch` y anotar el hallazgo en este fichero sin ampliar el alcance
- [x] 3.2 Documentar en `CLAUDE.md` la IRQ y los contextos, y la memoria de proyecto; `cargo test` completo en verde

## Resultado

ALttP supera el submódulo 3: `$11` pasa de 3 a 4 en el fotograma 134 y la intro sigue (`$11` = 5 en el 376, 6 en el 438, 7 en el 480, 8 en el 798) hasta que en el fotograma 1054 `$10` pasa a `$14` y `$11` a 0. El runtime hace unos 1400 cambios de contexto en 1500 fotogramas. Hizo falta una entrada nueva en `games/alttp_usa.toml` (`09:F81D`, el hilo de la intro): solo la alcanza una trama `RTI` que el juego guarda en la ROM (`09:F818`: P=30, PC=F81D, PBR=09), así que el descubrimiento no podía verla. Las capturas de 300 fotogramas muestran el texto de copyright; las de 1070 y 1500 salen negras (el juego usa mezcla de colores y HDMA, que el renderizador no implementa), un asunto aparte.
