# headless-runtime Specification

## Purpose

Ejecutar el C generado a partir de una ROM sin vídeo ni audio: bus de memoria LoROM, registros de hardware simulados, fotogramas con NMI e informe de lo que hizo el código, para saber hasta dónde llega la recompilación antes de implementar el hardware real.

## Requirements

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

#### Scenario: Estado de H-blank
- **WHEN** el código lee `$4212` repetidamente durante un fotograma
- **THEN** el bit 6 vale 0 al empezar el fotograma y 1 en el tramo final de cada una de las 262 líneas del fotograma, de modo que un bucle que espera ese bit termina

#### Scenario: Procesador de audio
- **WHEN** el código lee los puertos `$2140-$2143` antes del arranque del audio
- **THEN** obtiene `AA` y `BB` en los dos primeros, y tras escribir `CC` en `$2140` cada puerto devuelve lo último que el código escribió en él

### Requirement: Fotogramas y NMI
El runtime SHALL dividir la ejecución en fotogramas de un número fijo de bloques ejecutados y, al terminar cada uno, entregar el NMI si el bit 7 de `$4200` está activo, llamando al vector de NMI de la ROM con una pila coherente. Además SHALL entregar la IRQ cuando se habilite en los bits 4-5 de `$4200` y se alcance la línea de `$4209/$420A`, ignorando la posición horizontal.

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
- **WHEN** el código escribe `A1` en `$4200` y `FF` en `$4209` con la bandera I a 0 y sigue ejecutando bloques
- **THEN** el manejador del vector de IRQ de la ROM se ejecuta una vez por fotograma, después del NMI de ese fotograma, y su `RTI` devuelve el control al punto interrumpido con la pila y los registros como estaban

#### Scenario: Lectura de TIMEUP
- **WHEN** ha llegado una IRQ y el código lee `$4211`
- **THEN** el bit 7 vale 1 en esa lectura y 0 en la siguiente

#### Scenario: IRQ enmascarada
- **WHEN** la IRQ se dispara con la bandera I a 1
- **THEN** no se entrega hasta que la bandera pasa a 0, y no se pierde

#### Scenario: IRQ deshabilitada
- **WHEN** los bits 4-5 de `$4200` son 0
- **THEN** no se entrega ninguna IRQ

### Requirement: Arranque y parada
El runtime SHALL arrancar la ejecución en el vector de reset leído de la ROM, y MUST detenerse tras el número de fotogramas pedido o en el primer salto indirecto sin resolver, función no soportada, discrepancia de anchos, retorno que ninguna llamada acepta, `STP`, `BRK` o `COP`, con un mensaje que indique la causa y la dirección.

#### Scenario: Fotogramas pedidos
- **WHEN** se ejecuta con `--frames 5` y el código no falla
- **THEN** termina tras 5 fotogramas con código de salida 0 y el informe lo indica

#### Scenario: Fallo del código
- **WHEN** la ejecución llega a `rt_unresolved`, `rt_unsupported` o `rt_flag_mismatch`
- **THEN** termina de inmediato con código de salida distinto de 0 y un mensaje con la causa y las direcciones implicadas

#### Scenario: Retorno perdido
- **WHEN** un `RTS` o `RTL` vuelve a una dirección que ninguna llamada activa espera y el retorno llega al código del runtime
- **THEN** termina con código de salida distinto de 0 y un mensaje con la causa, la dirección sacada de la pila y `S`

#### Scenario: ROM no encontrada
- **WHEN** la ruta de la ROM no existe o no tiene el tamaño de una LoROM válida
- **THEN** termina con error que lo indica antes de ejecutar nada

### Requirement: Informe de la ejecución
El runtime SHALL imprimir al terminar un informe con los fotogramas ejecutados, los NMIs entregados, la causa de parada y los registros de hardware accedidos con su número de lecturas y escrituras, y MUST listar aparte los canales de DMA y HDMA activados, con los bytes que transfirió cada canal de DMA.

#### Scenario: Registros accedidos
- **WHEN** termina una ejecución en la que el código escribió 3 veces `$2100`
- **THEN** el informe muestra `$2100` con 3 escrituras

#### Scenario: DMA activado
- **WHEN** el código escribe un bit en `$420B` o `$420C`
- **THEN** el informe lista los canales activados y el número de veces de cada uno

#### Scenario: Bytes transferidos
- **WHEN** el canal 0 transfiere 512 bytes en total
- **THEN** el informe indica 512 bytes para el canal 0
