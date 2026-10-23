## Purpose

Ejecutar el C generado a partir de una ROM sin vídeo ni audio: bus de memoria LoROM, registros de hardware simulados, fotogramas con NMI e informe de lo que hizo el código, para saber hasta dónde llega la recompilación antes de implementar el hardware real.

## ADDED Requirements

### Requirement: Mapa de memoria LoROM
El runtime SHALL resolver los accesos del bus como una SNES LoROM: ROM leída de un fichero indicado en la línea de comandos, WRAM de 128 KiB, SRAM y espejos, de modo que lo que el código escribe se lea igual después.

#### Scenario: ROM en cualquier espejo
- **WHEN** el código lee `$8000` de los bancos `00`, `40` y `80`
- **THEN** obtiene el mismo byte de la ROM en los tres

#### Scenario: WRAM y su espejo
- **WHEN** el código escribe en `7E:0010` y lee `00:0010`, o escribe en `00:1FFF` y lee `7E:1FFF`
- **THEN** lee lo que escribió

#### Scenario: Puerto de WRAM
- **WHEN** el código fija `$2181-$2183` y escribe bytes en `$2180`
- **THEN** los bytes quedan en WRAM desde esa dirección, que avanza en cada escritura

#### Scenario: Escritura en la ROM
- **WHEN** el código escribe en una dirección de ROM
- **THEN** la ROM no cambia

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

#### Scenario: Procesador de audio
- **WHEN** el código lee los puertos `$2140-$2143` antes del arranque del audio
- **THEN** obtiene `AA` y `BB` en los dos primeros, y tras escribir `CC` en `$2140` cada puerto devuelve lo último que el código escribió en él

### Requirement: Fotogramas y NMI
El runtime SHALL dividir la ejecución en fotogramas de un número fijo de bloques ejecutados y, al terminar cada uno, entregar el NMI si el bit 7 de `$4200` está activo, llamando al vector de NMI de la ROM con una pila coherente.

#### Scenario: NMI habilitado
- **WHEN** el código escribe `81` en `$4200` y sigue ejecutando bloques durante varios fotogramas
- **THEN** el manejador de NMI de la ROM se ejecuta una vez por fotograma y su `RTI` devuelve el control al punto interrumpido con la pila y los registros como estaban

#### Scenario: NMI deshabilitado
- **WHEN** `$4200` tiene el bit 7 a 0 al terminar un fotograma
- **THEN** no se entrega ningún NMI

#### Scenario: Espera de interrupción
- **WHEN** el código ejecuta `WAI`
- **THEN** la ejecución avanza al final del fotograma actual y entrega el NMI si está habilitado

#### Scenario: IRQ
- **WHEN** el código habilita las IRQ en `$4200`
- **THEN** no se entrega ninguna IRQ y sus escrituras quedan en el informe

### Requirement: Arranque y parada
El runtime SHALL arrancar la ejecución en el vector de reset leído de la ROM, y MUST detenerse tras el número de fotogramas pedido o en el primer salto indirecto sin resolver, función no soportada, discrepancia de anchos, `STP`, `BRK` o `COP`, con un mensaje que indique la causa y la dirección.

#### Scenario: Fotogramas pedidos
- **WHEN** se ejecuta con `--frames 5` y el código no falla
- **THEN** termina tras 5 fotogramas con código de salida 0 y el informe lo indica

#### Scenario: Fallo del código
- **WHEN** la ejecución llega a `rt_unresolved`, `rt_unsupported` o `rt_flag_mismatch`
- **THEN** termina de inmediato con código de salida distinto de 0 y un mensaje con la causa y las direcciones implicadas

#### Scenario: ROM no encontrada
- **WHEN** la ruta de la ROM no existe o no tiene el tamaño de una LoROM válida
- **THEN** termina con error que lo indica antes de ejecutar nada

### Requirement: Informe de la ejecución
El runtime SHALL imprimir al terminar un informe con los fotogramas ejecutados, los NMIs entregados, la causa de parada y los registros de hardware accedidos con su número de lecturas y escrituras, y MUST listar aparte los canales de DMA y HDMA activados.

#### Scenario: Registros accedidos
- **WHEN** termina una ejecución en la que el código escribió 3 veces `$2100`
- **THEN** el informe muestra `$2100` con 3 escrituras

#### Scenario: DMA activado
- **WHEN** el código escribe un bit en `$420B` o `$420C`
- **THEN** el informe lista los canales activados y el número de veces de cada uno
