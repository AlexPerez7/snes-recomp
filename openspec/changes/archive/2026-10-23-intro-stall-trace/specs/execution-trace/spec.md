## Purpose

Saber qué bloque del código recompilado hace cada lectura de hardware y cada escritura a una variable de WRAM vigilada durante una ventana de fotogramas, para diagnosticar por qué el juego espera algo o no avanza, sin parchear el C generado.

## ADDED Requirements

### Requirement: Bloque en curso
El C generado compilado con `SNES_RT_TRACE` SHALL hacer disponible al runtime la dirección de 24 bits del bloque que se está ejecutando, y MUST no exigir ningún símbolo al runtime ni cambiar su comportamiento cuando `SNES_RT_TRACE` no está definido.

#### Scenario: Con traza
- **WHEN** el código ejecuta un bloque que empieza en `00:8010` y lee un registro
- **THEN** el runtime sabe que la lectura la hizo el bloque `00:8010`

#### Scenario: Sin traza
- **WHEN** el C se compila sin `SNES_RT_TRACE` contra un runtime que no define el símbolo del bloque en curso
- **THEN** compila y enlaza

### Requirement: Ventana de fotogramas
El runtime SHALL imprimir trazas solo con `--trace-frames A-B` y solo en los fotogramas de `A` a `B` ambos incluidos (el primero es el 0), y MUST no imprimir ninguna traza fuera de esa ventana ni cuando la opción no se pasa.

#### Scenario: Dentro de la ventana
- **WHEN** se ejecuta con `--trace-frames 2-3` y `--trace-reads` y el código lee `$4210` en cada fotograma
- **THEN** la traza contiene lecturas de los fotogramas 2 y 3 y de ningún otro

#### Scenario: Sin ventana
- **WHEN** se pasa `--trace-reads` sin `--trace-frames`
- **THEN** no se imprime ninguna traza

### Requirement: Traza de lecturas de hardware
El runtime SHALL anotar con `--trace-reads`, dentro de la ventana, cada lectura de un registro de `$2100-$43FF` con el fotograma, el bloque que la hizo, el registro y el valor devuelto, y MUST agrupar en una sola línea con su número de repeticiones las lecturas idénticas (mismo bloque, registro y valor) de un mismo fotograma.

#### Scenario: Una lectura
- **WHEN** el bloque `00:8010` lee `$4210` y obtiene `80` en el fotograma 2
- **THEN** la traza contiene una línea con el fotograma 2, el bloque `00:8010`, el registro `$4210` y el valor `80`

#### Scenario: Repeticiones
- **WHEN** el mismo bloque lee 50 veces `$2140` y obtiene siempre `00` en un fotograma
- **THEN** la traza tiene una sola línea para ellas con el contador 50

#### Scenario: Lecturas del DMA
- **WHEN** un canal de DMA lee registros del bus B
- **THEN** esas lecturas no se anotan como lecturas del bloque en curso

### Requirement: Vigilancia de WRAM
El runtime SHALL vigilar con `--watch ADDR`, repetible, los bytes de WRAM indicados (una dirección `0000-1FFF`, `7E:0000-7F:FFFF` o un offset de 17 bits) y MUST anotar dentro de la ventana, cuando una escritura cambia el valor de un byte vigilado, el fotograma, el bloque, la dirección, el valor antiguo y el nuevo, ignorando las escrituras que no lo cambian.

#### Scenario: Cambio de valor
- **WHEN** el bloque `00:8010` escribe `03` en el byte vigilado `$0011`, que valía `02`
- **THEN** la traza contiene el bloque `00:8010`, la dirección `$0011`, el valor antiguo `02` y el nuevo `03`

#### Scenario: Escritura sin cambio
- **WHEN** el código escribe en un byte vigilado el mismo valor que ya tenía
- **THEN** no se anota

#### Scenario: Espejo de WRAM
- **WHEN** se vigila `$0011` y el código escribe en `7E:0011`
- **THEN** la escritura se anota, porque es el mismo byte

#### Scenario: Escritura por DMA
- **WHEN** un DMA a WRAM cambia un byte vigilado
- **THEN** se anota con el bloque que disparó el DMA
