## MODIFIED Requirements

### Requirement: Anchos M/X por instrucción
El sistema SHALL conocer los anchos M y X vigentes en cada instrucción descubierta. Cuando una dirección se alcanza con anchos distintos y el C o la longitud de su instrucción dependen de la diferencia, el sistema MUST emitir una copia del bloque por combinación de anchos (capacidad `width-specialized-blocks`) en lugar de elegir una al azar, y solo falla si el conflicto no se puede separar en variantes.

#### Scenario: Anchos conocidos
- **WHEN** una instrucción se alcanza solo con M=0, X=1
- **THEN** el generador usa esos anchos para ella

#### Scenario: Anchos en conflicto
- **WHEN** una dirección se alcanza con anchos distintos y el C o la longitud de su instrucción dependen de la diferencia
- **THEN** la función que la contiene se genera con una copia del bloque por cada combinación de anchos alcanzada

#### Scenario: Anchos en conflicto que no cambian el C
- **WHEN** una dirección se alcanza con anchos distintos pero su instrucción produce el mismo C y tiene la misma longitud con todos ellos, como un `SEP #$30` o un `STA $10` cuando solo difiere X
- **THEN** la función que la contiene se genera sin duplicar el bloque y sin error

#### Scenario: Conflicto que no se separa
- **WHEN** el conflicto de anchos de una función no se puede separar en variantes
- **THEN** la generación falla para esa función con un error que indica la dirección y los anchos vistos, sin producir C parcial

#### Scenario: Comprobación en ejecución
- **WHEN** el C se compila con `SNES_RT_CHECKS`
- **THEN** cada bloque comprueba al entrar que M y X de la CPU coinciden con los supuestos y avisa a través de `rt_flag_mismatch` si no

#### Scenario: Comprobación en ejecución con anchos variables
- **WHEN** el C se compila con `SNES_RT_CHECKS` y el bloque empieza en una dirección alcanzada con valores distintos de M (o de X) cuyo C no cambia
- **THEN** el bloque no comprueba M (o X) al entrar y sigue comprobando el otro ancho
