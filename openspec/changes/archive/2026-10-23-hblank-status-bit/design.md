## Context

El runtime mide el tiempo en bloques ejecutados (`tick_in_frame` de `frame_len`); no hay un reloj de píxeles. `$4212` ya derivaba el vblank de ese contador.

## Decisions

1. **H-blank derivado del mismo contador.** El fotograma se divide en 262 líneas de `frame_len / 262` bloques, y el bit 6 vale 1 en el último quinto de cada línea. Alternativa descartada: alternar el bit en cada lectura; da bucles de espera cortos pero un comportamiento que depende del número de lecturas, no del tiempo.
2. **Sin modelar la posición horizontal exacta ni `$2137`.** Solo importa que los bucles de espera terminen.

## Risks / Trade-offs

- [Con `--frame-blocks` pequeño las líneas miden menos de un bloque] → el bit sigue cambiando de forma determinista; solo se garantiza que se vea en 0 y en 1 a lo largo del fotograma.
