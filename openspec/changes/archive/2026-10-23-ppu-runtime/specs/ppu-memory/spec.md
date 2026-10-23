## Purpose

Dar a la PPU sus tres memorias (VRAM, CGRAM y OAM) accesibles por los puertos de la consola, y poder volcarlas, para inspeccionar los gráficos que el código recompilado prepara sin necesitar aún un renderizador.

## ADDED Requirements

### Requirement: VRAM
El runtime SHALL mantener una VRAM de 32 K palabras accesible por `$2116-$2117` (dirección de palabra), `$2118-$2119` (escritura del byte bajo y del alto) y `$2139-$213A` (lectura), con el modo de incremento de `$2115`: el bit 7 elige si el avance ocurre tras el acceso al byte alto (1) o al bajo (0), y los bits 0-1 el paso de 1, 32 o 128 palabras.

#### Scenario: Escritura secuencial
- **WHEN** el código fija `$2115 = 80`, `$2116/7 = 1000` y escribe pares en `$2118/$2119`
- **THEN** las palabras quedan en las direcciones `$1000, $1001, …`

#### Scenario: Incremento por el byte bajo
- **WHEN** `$2115 = 00` y el código escribe solo en `$2118`
- **THEN** la dirección avanza en cada escritura y el byte alto de cada palabra queda sin tocar

#### Scenario: Paso de 32
- **WHEN** `$2115 = 81` y se escriben dos palabras
- **THEN** la segunda se escribe 32 palabras después de la primera

#### Scenario: Lectura con precarga
- **WHEN** el código fija la dirección en `$2116/7` y lee `$2139`
- **THEN** la primera lectura devuelve el valor precargado al fijar la dirección y las siguientes devuelven palabras consecutivas

### Requirement: CGRAM
El runtime SHALL mantener una CGRAM de 256 colores de 15 bits accesible por `$2121` (índice de color), `$2122` (escritura de dos bytes por color: primero el bajo, luego el alto) y `$213B` (lectura con el mismo orden), con el índice avanzando tras cada color completo.

#### Scenario: Paleta completa
- **WHEN** el código fija `$2121 = 00` y escribe 512 bytes en `$2122`
- **THEN** la CGRAM contiene esos 512 bytes en orden

#### Scenario: Color a medias
- **WHEN** el código escribe un solo byte en `$2122` y fija `$2121` otra vez
- **THEN** el byte pendiente se descarta y la escritura siguiente es la de un byte bajo

#### Scenario: Bit alto
- **WHEN** el código escribe `FF` como byte alto de un color
- **THEN** el bit 7 se guarda como 0

### Requirement: OAM
El runtime SHALL mantener una OAM de 544 bytes (512 de la tabla baja y 32 de la alta) accesible por `$2102-$2103` (dirección de palabra), `$2104` (escritura) y `$2138` (lectura), con la escritura en la tabla baja por pares de bytes, que se confirman al escribir el segundo, y directa en la tabla alta, y la dirección avanzando por byte.

#### Scenario: Tabla baja
- **WHEN** el código fija `$2102/3 = 0000` y escribe cuatro bytes en `$2104`
- **THEN** los cuatro bytes quedan en `OAM[0..4]`

#### Scenario: Tabla alta
- **WHEN** el código fija la dirección de palabra a `$0100` y escribe un byte en `$2104`
- **THEN** el byte queda en `OAM[512]`

#### Scenario: Lectura
- **WHEN** el código fija la dirección y lee `$2138` dos veces
- **THEN** obtiene los dos primeros bytes de OAM desde esa dirección

### Requirement: Volcado de las memorias
El runtime SHALL escribir con `--dump <dir>`, al terminar la ejecución y también cuando se detiene por un fallo, los ficheros `vram.bin` (64 KiB), `cgram.bin` (512 bytes), `oam.bin` (544 bytes) y `ppu_regs.bin` (el último valor escrito en cada registro `$2100-$213F`).

#### Scenario: Volcado tras una ejecución normal
- **WHEN** se ejecuta con `--dump d` y el código escribió en VRAM, CGRAM y OAM
- **THEN** los cuatro ficheros existen en `d` con los tamaños indicados y reflejan esas escrituras

#### Scenario: Volcado tras un fallo
- **WHEN** la ejecución se detiene por `rt_unresolved`
- **THEN** los ficheros se escriben igualmente antes de salir

#### Scenario: Sin opción
- **WHEN** no se pasa `--dump`
- **THEN** no se escribe ningún fichero
