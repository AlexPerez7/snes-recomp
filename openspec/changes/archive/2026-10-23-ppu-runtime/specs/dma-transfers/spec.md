## Purpose

Transferir datos con el DMA de propósito general entre la memoria direccionable por el bus A y los registros `$21xx` del bus B, con el efecto que tiene en la consola, para que los gráficos y datos que el juego envía a la PPU lleguen a sus memorias.

## ADDED Requirements

### Requirement: Disparo de canales
El runtime SHALL ejecutar, al escribir una máscara en `$420B`, la transferencia de cada canal activado en orden creciente de canal, completa antes de continuar con la instrucción siguiente, y MUST dejar el contador del canal a 0 al terminar.

#### Scenario: Un canal
- **WHEN** el código configura el canal 0 para copiar 16 bytes y escribe `01` en `$420B`
- **THEN** los 16 bytes se transfieren antes de ejecutarse la instrucción siguiente y `$4305-$4306` valen 0

#### Scenario: Varios canales
- **WHEN** el código escribe `05` en `$420B`
- **THEN** se transfiere primero el canal 0 y después el 2

#### Scenario: Tamaño cero
- **WHEN** un canal tiene el contador a 0
- **THEN** transfiere 65536 bytes

### Requirement: Patrones de transferencia
El runtime SHALL transferir cada unidad según los bits 0-2 de `$43x0` con los desplazamientos de registro de destino de la consola: modo 0 un byte a `B`; modo 1 dos bytes a `B`, `B+1`; modo 2 dos bytes a `B`, `B`; modo 3 cuatro bytes a `B`, `B`, `B+1`, `B+1`; modo 4 cuatro bytes a `B`, `B+1`, `B+2`, `B+3`; los modos 5, 6 y 7 como los 1, 2 y 3.

#### Scenario: Modo 1 a VRAM
- **WHEN** un canal en modo 1 con destino `$18` copia cuatro bytes
- **THEN** el primero y el tercero se escriben en `$2118` y el segundo y el cuarto en `$2119`

#### Scenario: Modo 0 a CGRAM
- **WHEN** un canal en modo 0 con destino `$22` copia 512 bytes
- **THEN** los 512 bytes se escriben en `$2122` en orden

### Requirement: Dirección y avance de la dirección A
El runtime SHALL leer del bus A y escribir en el B cuando el bit 7 de `$43x0` es 0, y al revés cuando es 1, y MUST avanzar la dirección A (`$43x2-$43x3`, sin cruzar de banco) tras cada byte según los bits 3-4: incrementar, mantener o decrementar.

#### Scenario: Incremento
- **WHEN** un canal copia tres bytes desde `$7E:2000` con avance por incremento
- **THEN** lee `$2000`, `$2001`, `$2002` y la dirección A queda en `$2003`

#### Scenario: Dirección fija
- **WHEN** un canal con dirección A fija copia cuatro bytes
- **THEN** lee cuatro veces el mismo byte

#### Scenario: Del bus B al A
- **WHEN** un canal con el bit 7 activo lee `$2139`
- **THEN** guarda en memoria lo que devolvería esa lectura, aplicando los efectos del puerto

#### Scenario: Cambio de banco
- **WHEN** la dirección A pasa de `$FFFF`
- **THEN** da la vuelta a `$0000` dentro del mismo banco

### Requirement: Destino en la WRAM
El runtime SHALL tratar el destino `$80` (registro `$2180`) como escrituras al puerto de WRAM, con su dirección autoincrementada.

#### Scenario: Relleno de WRAM
- **WHEN** un canal en modo 0 con destino `$80` copia 256 bytes desde la ROM con `$2181-$2183` en `7E:4000`
- **THEN** `7E:4000-40FF` contiene esos bytes

### Requirement: Registro de transferencias
El runtime SHALL contar por canal cuántas transferencias y cuántos bytes realizó.

#### Scenario: Bytes por canal
- **WHEN** el canal 1 transfiere 64 bytes dos veces
- **THEN** el informe indica 2 transferencias y 128 bytes para el canal 1
