## Purpose

Simular el mando 1 de la consola en el runtime sin vídeo, con botones pulsados por rangos de fotogramas desde la línea de comandos, para que el código recompilado pueda avanzar más allá de las pantallas que esperan una pulsación y las pruebas sean reproducibles.

## ADDED Requirements

### Requirement: Opción para pulsar botones
El runtime SHALL aceptar la opción repetible `--press BOTONES@A-B`, que mantiene pulsados los botones durante los fotogramas A a B, ambos incluidos y contados desde 0. Los botones SHALL ser `B`, `Y`, `SELECT`, `START`, `UP`, `DOWN`, `LEFT`, `RIGHT`, `A`, `X`, `L` y `R`, unidos con `+`. Sin la opción, el mando MUST no pulsar nada, y una opción mal formada MUST terminar con error de uso antes de ejecutar nada.

#### Scenario: Un botón en un rango
- **WHEN** se ejecuta con `--press START@3-4`
- **THEN** START está pulsado en los fotogramas 3 y 4 y suelto en el resto

#### Scenario: Varios botones y varias opciones
- **WHEN** se ejecuta con `--press A+RIGHT@10-12 --press B@11-20`
- **THEN** en el fotograma 11 están pulsados A, RIGHT y B, y en el 15 solo B

#### Scenario: Sin opción
- **WHEN** no se pasa `--press`
- **THEN** ninguna lectura del mando devuelve un botón pulsado

#### Scenario: Opción inválida
- **WHEN** se pasa `--press FIRE@1-2`, `--press START@5-2` o `--press START`
- **THEN** termina con código de salida 2 antes de ejecutar nada

### Requirement: Lectura automática
Con el bit 0 de `$4200` activo, el runtime SHALL cargar al empezar el vblank de cada fotograma los botones pulsados en ese fotograma en `$4218-$4219`, con `$4218` = `A X L R 0 0 0 0` (del bit 7 al 0) y `$4219` = `B Y SELECT START UP DOWN LEFT RIGHT`, y `$421A-$421B` (mando 2) a 0. Con ese bit a 0, los registros MUST conservar su valor anterior. El bit 0 de `$4212` SHALL valer 1 durante las primeras 3 de las 262 líneas del fotograma que siguen al comienzo del vblank y 0 fuera de ellas.

#### Scenario: Botón pulsado
- **WHEN** START está pulsado en un fotograma con `$4200` = `81` y el código lee `$4219` tras el comienzo de su vblank
- **THEN** obtiene el bit 4 a 1

#### Scenario: Sin lectura automática
- **WHEN** `$4200` tiene el bit 0 a 0
- **THEN** `$4218-$4219` no cambian aunque haya botones pulsados

#### Scenario: Ocupado
- **WHEN** el código lee `$4212` justo al empezar el vblank y una vez a mitad de fotograma
- **THEN** el bit 0 vale 1 en la primera lectura y 0 en la segunda

### Requirement: Lectura serie manual
El runtime SHALL, al escribir 1 y después 0 en el bit 0 de `$4016`, capturar los botones pulsados en ese momento, y cada lectura de `$4016` SHALL devolver en el bit 0 el siguiente botón en el orden `B Y SELECT START UP DOWN LEFT RIGHT A X L R`, seguido de cuatro ceros y después unos. Mientras el latch esté a 1, la lectura MUST devolver siempre el botón B.

#### Scenario: Orden de los botones
- **WHEN** START y A están pulsados y el código hace el latch y lee `$4016` doce veces
- **THEN** los bits leídos son `0 0 0 1 0 0 0 0 1 0 0 0`

#### Scenario: Tras los 16 bits
- **WHEN** el código lee `$4016` más de 16 veces tras un latch
- **THEN** las lecturas posteriores a la decimosexta devuelven 1
