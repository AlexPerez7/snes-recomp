## MODIFIED Requirements

### Requirement: Registros de hardware simulados
El runtime SHALL guardar lo escrito en los registros de `$2100-$21FF` y `$4200-$43FF` y devolverlo al leer, salvo los de estado, y MUST contar cada acceso por registro.

#### Scenario: Registro de escritura
- **WHEN** el código escribe un valor en un registro de PPU o de DMA y lo lee después
- **THEN** obtiene el valor escrito, y el acceso queda contado

#### Scenario: Estado de NMI
- **WHEN** el código lee `$4210` tras un NMI
- **THEN** el bit 7 está a 1 en esa primera lectura y a 0 en la siguiente

#### Scenario: Estado de vblank
- **WHEN** el código lee `$4212` en un instante del fotograma
- **THEN** el bit 7 vale 1 solo durante el tramo final del fotograma

#### Scenario: Estado de H-blank
- **WHEN** el código lee `$4212` repetidamente durante un fotograma
- **THEN** el bit 6 vale 0 al empezar el fotograma y 1 en el tramo final de cada una de las 262 líneas del fotograma, de modo que un bucle que espera ese bit termina

#### Scenario: Procesador de audio
- **WHEN** el código lee los puertos `$2140-$2143` antes del arranque del audio
- **THEN** obtiene `AA` y `BB` en los dos primeros, y tras escribir `CC` en `$2140` cada puerto devuelve lo último que el código escribió en él
