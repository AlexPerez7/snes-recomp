## 1. Generador

- [x] 1.1 Declarar `rt_tick` en `runtime/snes_rt.h` (vacío salvo con `SNES_RT_TICK`, que llama a `rt_poll(Cpu *c)`) y emitir `rt_tick(c);` como primera línea de cada bloque en `src/codegen.rs`; ajustar los tests que comparan cuerpos de función; verificar con un test que cada bloque lo lleva y que compilar sin `SNES_RT_TICK` no exige `rt_poll`, y con `cargo test` en verde
- [x] 1.2 Generar `snes_entry.c` en `generate_all` (función `snes_call_entry(Cpu *c, uint32_t addr)` con un `switch` sobre las entradas, que devuelve 0 si la dirección no es una); verificar con un test sobre una ROM pequeña (entrada conocida se ejecuta, desconocida devuelve 0) y que el texto no contiene bytes de la ROM
- [x] 1.3 Ampliar `src/cctest.rs` para escribir ficheros binarios y pasar argumentos al programa; verificar con un test del propio `cctest` que el programa recibe un fichero y un argumento

## 2. Runtime

- [x] 2.1 Crear `runtime/headless.c` con el bus LoROM (ROM del fichero, espejos, WRAM y su espejo, SRAM, puerto `$2180-$2183`) y la carga validada de la ROM; verificar con un test que ejecuta una ROM pequeña que lee la ROM por tres espejos, escribe y relee WRAM, y que una ruta inexistente termina con error antes de ejecutar nada
- [x] 2.2 Añadir los registros de hardware (almacén, contadores, `$4210`, `$4212`, doble del audio) y el informe de accesos y de canales de DMA/HDMA; verificar con tests que cubren cada escenario de la spec (valor guardado y devuelto, `$4210` se borra al leer, `$4212` solo en vblank, protocolo del audio, lista de DMA)
- [x] 2.3 Añadir los fotogramas, `rt_poll`, la entrega del NMI con su pila, `WAI`, el arranque por el vector de reset vía `snes_call_entry` y la parada con códigos de salida por causa; verificar con una ROM pequeña que habilita el NMI y cuenta en WRAM (el manejador se ejecuta una vez por fotograma, `RTI` deja pila y registros como estaban, sin NMI si `$4200` no lo habilita, `WAI` avanza de fotograma, `--frames N` termina con 0, `rt_unresolved`/`rt_unsupported` terminan con código distinto y mensaje)
- [x] 2.4 Incrustar `runtime/headless.c` con `include_str!` y escribirlo con `--all` como `snes_headless.c` junto a `snes_entry.c` en `src/main.rs`; verificar ejecutando `codegen --all -o out/rom` y comprobando los ficheros nuevos

## 3. ALttP completo

- [x] 3.1 Test (se omite sin ROM o compilador) que genera ALttP completo, compila todo con `SNES_RT_TICK` y `SNES_RT_CHECKS`, lo ejecuta con la ROM real y unos pocos fotogramas y comprueba que termina con un informe; verificar que pasa con Visual Studio
- [x] 3.2 Ajustar la longitud del fotograma hasta que el juego avance y anotar en este fichero las mediciones: fotogramas alcanzados, causa y dirección de la parada, registros de hardware más usados y canales de DMA activados
- [x] 3.3 Reescribir `alttp_reset_runs_to_the_nmi_wait_loop` sobre el runtime nuevo (queda como `alttp_boots_and_takes_its_first_nmis`); `ALTTP_BUS` se conserva porque `alttp_inline_helper_call_reaches_its_target` entra por una función cualquiera con la memoria preparada, algo que el runtime nuevo no hace; verificar con `cargo test` en verde
- [x] 3.4 Documentar en `CLAUDE.md` cómo compilar y ejecutar el directorio de `--all` con el runtime; verificar que el comando documentado funciona tal cual
- [x] 3.5 `cargo test` completo en verde

## Mediciones (ALttP USA, Visual Studio Build Tools, `SNES_RT_TICK` y `SNES_RT_CHECKS`)

- Compilar y enlazar el C completo con el runtime: unos 2 s. Una ejecución de 30 fotogramas: 0,14 s.
- Arranque: el reset pasa por la subida del programa de audio (unas 54.000 lecturas y escrituras de `$2140`, 14 fotogramas de 20.000 bloques), activa el NMI (`$4200` = `81`) y entra en su bucle principal.
- Con 30 fotogramas la ejecución termina bien (código 0), con 17 NMIs. Con más, se detiene en `FLAG_MISMATCH 00:E63C` tras 50 fotogramas y 37 NMIs (la CPU llega con M=8 X=8 y el bloque supone M=16 X=8); es la misma dirección con fotogramas de 5.000, 20.000 y 100.000 bloques, así que depende de la lógica del juego y no del reloj. Es un fallo del análisis de anchos (bloque compartido `00:E63C`, de los 437 que comparten funciones): cambio posterior.
- Registros más usados (por fotograma): DMA con los canales 0 a 4, `$4210`, el mando automático (`$4218-$4219`, `$4016`) y las escrituras de PPU (`$2100-$2107`, `$2115-$2119`, `$2121-$2122`).
- Hallazgos de la config de ALttP que se corrigieron aquí porque sin ellos no había NMI que medir: el vector de NMI nativo `00:80C9` no era entrada (solo lo era el de emulación `00:822C`), y el `JMP ($8C7E,X)` del NMI (`00:8C7B`) no tenía tabla. Se añadieron la entrada `nmi_native` y una tabla de 25 entradas (acaba en `00:8CB0`), comprobadas a mano contra el ROM. `discover --widths` pasa de 232 a 234 conflictos; las llamadas sin retorno siguen en 3.
- Los NMI entran con M=X=8 aunque el código interrumpido tuviera otros anchos: el recompilador analiza el manejador desde su entrada con 8/8 y el manejador de ALttP empieza con `REP #$30`.
