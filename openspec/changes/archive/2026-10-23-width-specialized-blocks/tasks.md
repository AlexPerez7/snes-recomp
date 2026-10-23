## 1. Latch H/V del runtime

- [x] 1.1 Añadir tests en `src/headless.rs` del latch H/V ya implementado en `runtime/headless.c` (`$2137` captura, `$213D` byte bajo y bit 0 alternando, `$213F` reinicia y devuelve el bit 6 solo con captura pendiente, y un bucle de espera de línea que termina) y ajustar el runtime si fallan

## 2. Variantes por anchos

- [x] 2.1 `src/discover.rs`: conservar las aristas entre variantes `(dirección, anchos)` junto a `flag_conflicts`, con tests de un programa con `SEP`/`REP` que alcanza un bloque con dos anchos
- [x] 2.2 `src/graph.rs`: sin cambios; el grafo sigue siendo por dirección y las copias por anchos se calculan en `codegen` a partir de `Discovery::transitions` (ver design.md); verificado por los tests de `codegen` y por que el C de ALttP no cambia fuera de las funciones con conflicto
- [x] 2.3 `src/codegen.rs`: emitir una copia por variante con etiquetas `L_<dir>_m<M>x<X>`, resolver los saltos a la variante de destino y comprobar los anchos de cada copia; mantener el stub con el mensaje de anchos cuando falte una arista; verificar con tests que compilan y ejecutan un bloque `INC abs` alcanzado con M=8 y con M=16, y que el C de un programa sin conflictos no cambia

## 3. ALttP y documentación

- [x] 3.1 Regenerar ALttP y comprobar que las 7 funciones por anchos (entre ellas `01:D81B`) se traducen; ejecutar con las pulsaciones que eligen la partida (`START@1100-1110`, `A@1300-1305`, `A@1500-1504`, `DOWN@1540-1543`, `DOWN@1560-1563`, `DOWN@1580-1583`, `RIGHT@1600-1603`, `RIGHT@1620-1623`, `A@1640-1643`, `A@2000-2004`), capturar la pantalla tras la elección y anotar en este fichero qué se ve o qué falla después, sin ampliar el alcance
- [x] 3.2 Actualizar `CLAUDE.md` (variantes por anchos y latch H/V) y la memoria de proyecto; `cargo test` completo en verde

Verificación 2.3/3.1: de las 7 funciones de ALttP con stub por anchos pasan a traducirse 5 (entre ellas `01:D81B`); quedan `01:CA17` y `08:A8E3`, donde un inmediato cambia de longitud según M (`AND #$FF` de 2 o 3 bytes) y el flujo de instrucciones es otro, así que no hay copia posible y siguen siendo stub. Con la secuencia de pulsaciones de la tarea, tras elegir la partida la captura muestra la habitación de Link con el HUD (corazones, rupias); el juego corre hasta el fotograma 2011 y para con `LOST_RETURN to BFDF` en `06:9271`: un despacho `PHA`/`RTS` (`LDA $9283,Y` con Y=$E6, sprite 0 de tipo $73) lee una dirección fuera de la tabla. Es un fallo posterior, fuera de este cambio.
