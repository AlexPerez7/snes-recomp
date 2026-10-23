## Context

`runtime/headless.c` guarda lo escrito en `$2100-$43FF` y lo devuelve al leer, salvo los registros de estado (`$4210-$4212`). Los fotogramas se cuentan por bloques ejecutados y el vblank empieza al final de cada fotograma (`tick_in_frame`), donde ya se entrega el NMI. ALttP lee el mando en `00:83D1`: `STZ $4016` y después `$4218`/`$4219`; no usa la lectura serie.

## Goals / Non-Goals

**Goals:**
- Pulsar botones reproduciblemente desde la línea de comandos y que ALttP los vea.
- Modelar lo mínimo del hardware real que el código de los juegos suele asumir: lectura automática tras el vblank, bit de ocupado y lectura serie.

**Non-Goals:**
- Mando 2, ratón, multitap, pistola.
- Teclado o ventana: el runtime sigue sin vídeo ni audio.
- Temporización exacta de la lectura automática en ciclos.

## Decisions

1. **Los botones dependen del fotograma, no del tiempo.** Una tabla de pulsaciones `(máscara, desde, hasta)` se consulta con el número de fotograma actual. Es reproducible y no cambia con la velocidad de la máquina.
2. **La lectura automática se aplica al empezar el vblank.** Se hace donde ya se entrega el NMI, antes de él, así que el manejador del NMI ve los botones del fotograma que termina. Alternativa descartada: calcularla en cada lectura de `$4218`; un registro que cambia en mitad de un fotograma no se parece al hardware.
3. **El bit de ocupado se deriva del contador de bloques**, como el vblank: vale 1 desde el comienzo del vblank durante 3 de 262 líneas del fotograma (`3 * frame_len / 262` bloques).
4. **`$4016` con su propio estado:** un latch, un desplazador de 16 bits y la máscara capturada. La escritura de `$4016` bit 0 de 1 a 0 captura; mientras está a 1, la lectura devuelve B. Tras los 16 bits devuelve 1, como el hardware.
5. **Formato de `--press`:** `BOTONES@A-B` con `+` entre botones. Es el mismo estilo que `--trace-frames A-B` y se analiza a mano en C; hasta 32 opciones.
6. **Palabras de botón en mayúsculas**, para no confundir `A` (botón) con una dirección o un fotograma.

## Risks / Trade-offs

- [ALttP puede necesitar más de un fotograma pulsado, o soltar y volver a pulsar] → los rangos de `--press` lo permiten y el ajuste se hace al verificar con la ROM.
- [El modo atracción puede ignorar `START` hasta cierto punto del título] → la tarea de verificación prueba varias ventanas y anota la que funciona.
- [`$4219` de otros dispositivos y `$4017`] → devuelven 0 y no se contemplan.
