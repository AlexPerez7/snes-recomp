# Spec Delta

## MODIFIED Requirements

### Requirement: Anchos M/X por instrucción
El sistema SHALL conocer los anchos M y X vigentes en cada instrucción descubierta, y MUST informar de las direcciones alcanzadas con anchos distintos en lugar de elegir una al azar, salvo cuando el C de esa instrucción y su longitud son los mismos con todos los anchos vistos.

#### Scenario: Anchos conocidos
- **WHEN** una instrucción se alcanza solo con M=0, X=1
- **THEN** el generador usa esos anchos para ella

#### Scenario: Anchos en conflicto
- **WHEN** una dirección se alcanza con anchos distintos y el C o la longitud de su instrucción dependen de la diferencia
- **THEN** la generación de una función que la contiene falla con un error que indica la dirección y los anchos vistos, sin producir C parcial

#### Scenario: Anchos en conflicto que no cambian el C
- **WHEN** una dirección se alcanza con anchos distintos pero su instrucción produce el mismo C y tiene la misma longitud con todos ellos, como un `SEP #$30` o un `STA $10` cuando solo difiere X
- **THEN** la función que la contiene se genera sin error

#### Scenario: Comprobación en ejecución
- **WHEN** el C se compila con `SNES_RT_CHECKS`
- **THEN** cada bloque comprueba al entrar que M y X de la CPU coinciden con los supuestos y avisa a través de `rt_flag_mismatch` si no

#### Scenario: Comprobación en ejecución con anchos variables
- **WHEN** el C se compila con `SNES_RT_CHECKS` y el bloque empieza en una dirección alcanzada con valores distintos de M (o de X)
- **THEN** el bloque no comprueba M (o X) al entrar y sigue comprobando el otro ancho
