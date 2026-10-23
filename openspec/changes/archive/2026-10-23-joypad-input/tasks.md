## 1. Botones y lectura automática

- [x] 1.1 Analizar `--press BOTONES@A-B` (repetible, hasta 32) en `runtime/headless.c` y guardar una tabla `(máscara, desde, hasta)`; un botón desconocido, un rango invertido o la falta de `@` termina con código 2; verificar con tests de las opciones válidas y de las tres inválidas
- [x] 1.2 Cargar `$4218-$421B` al empezar el vblank, antes del NMI, cuando `$4200` tiene el bit 0 activo, y conservarlos si no; devolver el bit 0 de `$4212` a 1 durante las 3 primeras líneas del vblank; verificar con tests del bit de START en `$4219`, de la ausencia de pulsaciones sin la opción, de `$4200` sin el bit 0 y del bit de ocupado

## 2. Lectura serie

- [x] 2.1 Implementar `$4016`: captura al bajar el bit 0 tras un 1, desplazamiento de 16 bits en orden B Y SELECT START UP DOWN LEFT RIGHT A X L R, uno tras el decimosexto, y B mientras el latch está a 1; verificar con tests del orden de doce bits con START y A pulsados y de las lecturas posteriores a la decimosexta

## 3. ALttP y documentación

- [x] 3.1 Regenerar ALttP, buscar una ventana de `--press START@A-B` en la pantalla de título (fotograma ~1054 en adelante) con la que el juego salga del modo atracción, capturar la pantalla resultante y anotar la ventana y lo que se ve en este fichero, sin ampliar el alcance
- [x] 3.2 Actualizar `CLAUDE.md` (la opción `--press`) y la memoria de proyecto; `cargo test` completo en verde

Verificación 3.1: `--frames 3000 --press START@1100-1110` (y también `@1500-1510`) sale del modo atracción; la captura muestra la pantalla «PLAYER SELECT» (tres ranuras vacías, COPY PLAYER / ERASE PLAYER).
