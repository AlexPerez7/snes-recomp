## Purpose

Emitir cada bloque con los anchos M/X con los que realmente se ejecuta cuando una misma dirección se alcanza con anchos distintos y su C depende de la diferencia, en lugar de rechazar la función.

## ADDED Requirements

### Requirement: Variantes de bloque por anchos
Cuando una dirección se alcanza con varias combinaciones de anchos y el C o la longitud de alguna de sus instrucciones depende de la diferencia, el sistema SHALL emitir una copia del bloque por cada combinación alcanzada, con las instrucciones generadas con esos anchos. Cada salto, rama o continuación que llegue a esa dirección MUST ir a la copia que corresponde a los anchos con los que se llega.

#### Scenario: Dos anchos en un mismo bloque
- **WHEN** un bloque con `INC abs` se alcanza con M=8 por una rama y con M=16 por otra
- **THEN** el C contiene dos copias del bloque, una con incremento de 8 bits y otra de 16, y cada rama salta a la suya

#### Scenario: Las copias siguen su propio camino
- **WHEN** una copia con M=8 cae en un bloque que después ejecuta `REP #$30`
- **THEN** su continuación va a la copia de ese bloque para los anchos que `REP` deja, no a otra

#### Scenario: Conflicto que no cambia el C
- **WHEN** una dirección se alcanza con anchos distintos pero su C y su longitud son iguales con todos
- **THEN** no se duplica el bloque y el C generado es el mismo que antes del cambio

### Requirement: Etiquetas y comprobaciones por variante
El sistema SHALL dar a cada copia una etiqueta distinta que incluya sus anchos, y la comprobación `rt_check_flags` de cada copia MUST validar los anchos de esa copia.

#### Scenario: Comprobación de la copia
- **WHEN** se compila con `SNES_RT_CHECKS` y se ejecuta la copia M=16 de un bloque con variantes
- **THEN** `rt_check_flags` espera M=16 y no falla

### Requirement: Conflictos que no se separan
El sistema SHALL seguir generando un stub, con el mensaje de anchos distintos de siempre, para una función cuyo conflicto no se pueda separar en variantes.

#### Scenario: Función con salto indirecto ambiguo
- **WHEN** una función no puede asociar un destino a los anchos con los que se llega
- **THEN** queda como stub y se lista con la dirección y los anchos vistos

#### Scenario: Inmediato de otra longitud
- **WHEN** una instrucción con operando inmediato se alcanza con anchos que le dan longitudes distintas, como `AND #$FF` con M=8 y `AND #$00FF` con M=16
- **THEN** la función queda como stub, porque las dos variantes no ejecutan las mismas instrucciones a partir de ahí
