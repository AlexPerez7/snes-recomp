## Contexto

El estado de un juego recompilado está repartido: la memoria y el hardware son globales de `snes_core.inc`, pero las llamadas pendientes del 65816 son llamadas C reales (cada `JSR` es una función, cada retorno comprueba la pila emulada) y las interrupciones corren en contextos con pila nativa propia (fibras en Windows, `ucontext` en el resto). Una instantánea de datos no puede reanudar eso.

## Decisiones

1. **Punto de reanudación declarado por el perfil.** `[resume] addr` es el inicio de un **bloque** (no hace falta que sea una función) del bucle principal del juego, con la pila del 65816 en su base. El generador emite `rt_resume(c);` justo después de `rt_tick` en ese bloque. En ALttP es `00:8034` (`LDA $12 / BEQ`, la espera del NMI), un bloque de la función de reset `00:8000` alcanzado con `S = $01FF`; comprobado en el grafo: la función tiene 4 bloques, y los otros tres (`8038`, `8051` y el salto de vuelta) cierran el bucle con las llamadas por fotograma ya retornadas. Guardar y cargar ocurren **en ese mismo sitio, sobre la pila nativa viva**: como la pila nativa tiene siempre la misma forma ahí (el marco de la función de reset), cargar es restaurar los globales y los registros de `*c` y seguir; no hay que deshacer ninguna pila ni reentrar. Un estado cargado al arrancar espera al primer paso por el punto. Alternativas descartadas: copiar las pilas nativas (frágil, depende del compilador y de las fibras) y rebobinar reproduciendo entradas (carga lenta, no sobrevive a la sesión).
2. **El guardado se pide y se hace en el siguiente paso por el punto.** La tecla solo marca la petición; `rt_poll` no puede guardar en cualquier bloque. Si el juego tarda en pasar por el punto, la petición espera (con un límite de fotogramas) y se avisa.
3. **Qué se guarda.** Todo lo `static` de `snes_core.inc`, `snes_ppu.inc` y `snes_spc.inc` que cambia con la ejecución, en un volcado con cabecera: versión del formato, versión del motor y CRC32 de la ROM. Los datos derivados de la ROM (la propia ROM, el overlay) no se guardan. Los estados contienen WRAM y VRAM, es decir, **derivan del juego**: van en `.gitignore`, en `scan` y en la comprobación de `release.ps1` (extensión `.sst`).
4. **Carga.** `rt_resume` restaura los globales y los registros del `Cpu` y vuelve; el bloque sigue con el estado cargado. Se exige que `S` y el contexto activo coincidan con los del punto (solo el principal, sin NMI/IRQ en curso); si no, se rechaza el estado.
5. **Rebobinado.** Anillo de N estados (p. ej. uno cada 4 fotogramas, unos 10 s) comprimidos por diferencia con el anterior; mantener la tecla retrocede un estado por paso y sin sonido. Solo en puntos de reanudación, así que la granularidad es la del bucle principal.
6. **Determinismo.** Prueba: guardar, avanzar N fotogramas, anotar el hash de la imagen y del audio; cargar y repetir debe dar los mismos hashes. Cualquier desvío indica estado sin serializar.

## Riesgos

- Estado olvidado (un contador, el eco del DSP): la prueba de determinismo lo detecta, pero hay que escribirla con una secuencia larga de ALttP.
- Que otro juego no tenga un punto así (bucle principal dentro de una función con llamadas pendientes): entonces no podrá declarar `[resume]`.
- El reloj es proporcional por bloques: si `tick_in_frame` no es el mismo al reentrar, el audio puede derivar. Se guarda y se restaura tal cual.

## Pendiente de decidir al implementar

- Comprobar en ejecución (traza) que `S = $01FF` y que no hay otros contextos cada vez que se llega a `rt_resume`.
- Formato de compresión (RLE de diferencias u otro; preferir lo que no añada dependencias).
