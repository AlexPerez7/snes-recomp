## 1. Bloque en curso y ventana

- [x] 1.1 Hacer que `rt_check_flags` guarde la dirección del bloque en `rt_block` con `SNES_RT_TRACE` (también sin `SNES_RT_CHECKS`) en `runtime/snes_rt.h`, definir `rt_block` en `runtime/headless.c` y añadir `SNES_RT_TRACE` a las definiciones de `build_and_run`; verificar con un test del bloque en curso (una lectura de registro hecha desde `00:8010`) y otro que compila y enlaza sin `SNES_RT_TRACE` contra un runtime sin el símbolo
- [x] 1.2 Añadir la opción `--trace-frames A-B` y su ayuda; verificar con tests de ventana (solo los fotogramas de la ventana) y de que sin ventana no se imprime nada

## 2. Trazas

- [x] 2.1 Implementar `--trace-reads` con la agrupación por fotograma, bloque, registro y valor y sin las lecturas del DMA; verificar con tests de una lectura, de 50 repeticiones agrupadas y de lecturas del DMA no anotadas
- [x] 2.2 Implementar `--watch ADDR` (repetible, direcciones `0000-1FFF`, `7E/7F:xxxx` y offsets) con valor antiguo y nuevo, sin anotar escrituras que no cambian el byte; verificar con tests de cambio de valor, escritura sin cambio, espejo de WRAM y escritura por DMA

## 3. Diagnóstico de ALttP

- [x] 3.1 Ejecutar ALttP con la traza (ventana de unos fotogramas ya en el atasco, `--trace-reads`, `--watch` sobre `$10` y `$11`), identificar el bloque y el registro o la variable que el submódulo 3 espera, y anotar el hallazgo en este fichero
- [x] 3.2 Si la causa es una simplificación del hardware simulado: corregirla con un delta de `headless-runtime` y una prueba que la cubre, y comprobar que ALttP sale del submódulo 3; si es del código recompilado: anotar la dirección y el motivo y cerrar sin corregir (no se corrige: ver el hallazgo)
- [x] 3.3 Documentar `--trace-frames`, `--trace-reads` y `--watch` en `CLAUDE.md`; verificar que el comando documentado funciona tal cual
- [x] 3.4 `cargo test` completo en verde

## Hallazgo (3.1 y 3.2)

El submódulo 3 no espera ningún registro de hardware. Con `--trace-frames 88-95 --trace-reads` las únicas lecturas son las del NMI (`00:80C9` lee `$4210`; `00:80E1` y `00:8108` leen `$2140`/`$2141` y los comparan con `$0133`/`$0131`, el protocolo normal de comandos de música; `00:83D1` lee `$4218/$4219` cada dos fotogramas). Vigilando `$10-$5FF` y `$1E00-$1F1F`, lo único que cambia por fotograma es `$1A`, `$12` y la animación de sprites (`$1E0A`, `$1E28-$1E4A`).

La causa está en cómo el juego se estructura, no en un registro:

- El submódulo 3 es `0C:C404` (tabla de `0C:C139`, entrada 3), la animación de la Trifuerza. Su máquina de estados (`0C:C448`, estado en `$1E00`, contador en `$1E01`) solo corre si `$1F00 = 0` (`0C:C43B`). `0C:C39F` deja `$1F00 = 1` al inicializar (fotograma 54), y en los submódulos 3 y 4 nada lo vuelve a poner a 0 (solo lo hacen `0C:C2B2` y `0C:C2D8`, de los submódulos 6 y 8). El estado `$1E00` queda a 0 para siempre.
- Con `$012A != 0` (lo pone `0C:C435` en cada fotograma) el NMI toma la rama `00:822D` y termina en `00:82C9-82D7` intercambiando la pila (`TSC`, `LDA $1F0A`, `TCS`, `STX $1F0A`, `PLB…PLA`, `RTI`). El IRQ (`00:82D8`, vertical en la línea `$4209 = $FF`, `$4200 = $A1`) hace lo mismo en `00:831F-8332`. Son dos contextos de ejecución que se turnan: el NMI entra en el contexto que dejó guardado el IRQ y viceversa.
- El runtime no entrega IRQ, y aunque lo hiciera, el C generado no puede cambiar de contexto: `RTI` es un `return` de C (`rt_rti`) y las direcciones de retorno viven en la pila nativa, no en la emulada. El `TCS` solo deja el puntero de pila apuntando a la pila del otro contexto, y a partir de ahí los `PHA`/`PLA`/`PHP`/`PLP` del hilo principal usan la pila equivocada.

No se corrige aquí: hace falta modelar los dos contextos (p. ej. con corrutinas o hilos nativos) y entregar el IRQ, que es un cambio de diseño del generador y del runtime, no una simplificación de un registro. Se deja para un cambio aparte.
