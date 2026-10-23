## ADDED Requirements

### Requirement: Latch de los contadores H/V
El runtime SHALL capturar los contadores horizontal y vertical al leer `$2137`, a partir del progreso del fotograma en curso (línea de 0 a 261 y columna de 0 a 339). Cada lectura de `$213C` (H) y `$213D` (V) MUST devolver primero el byte bajo y después el bit 0 del contador capturado, alternando. Una lectura de `$213F` SHALL devolver el bit 6 a 1 si hay captura pendiente, reiniciar la alternancia de `$213C`/`$213D` y borrar la captura pendiente.

#### Scenario: Línea del contador vertical
- **WHEN** el código lee `$2137` y después `$213D` dos veces en mitad de un fotograma
- **THEN** la primera lectura devuelve el byte bajo de la línea actual y la segunda su bit 8, que vale 0 antes de la línea 256

#### Scenario: Espera de línea
- **WHEN** el código repite `$2137`, `$213D` hasta que la línea alcanza un valor
- **THEN** el bucle termina dentro del mismo fotograma, porque el contador avanza con los bloques ejecutados

#### Scenario: Reinicio con $213F
- **WHEN** el código lee `$213D` una vez, lee `$213F` y vuelve a leer `$213D`
- **THEN** la lectura posterior devuelve de nuevo el byte bajo, y `$213F` devolvió el bit 6 a 1 solo la primera vez
