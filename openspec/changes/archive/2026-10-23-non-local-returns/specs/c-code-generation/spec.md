## ADDED Requirements

### Requirement: Retornos no locales
El sistema SHALL hacer que el flujo de control del C siga el de la CPU cuando un `RTS` o un `RTL` vuelve a una dirección distinta de la que espera la llamada nativa. Cada `RTS` y `RTL` MUST dejar en `Cpu` la dirección que sacó de la pila, y cada sitio de llamada MUST comparar, al volver la función llamada, el `S` y esa dirección con los que esperaba; si no coinciden, MUST retornar sin ejecutar nada más.

#### Scenario: Descartar un retorno
- **WHEN** una función llamada con `JSR` ejecuta `PLA`, `PLA` y `RTS`, y fue llamada desde otra función que a su vez fue llamada con `JSR`
- **THEN** la ejecución continúa en la instrucción siguiente a la llamada de la función exterior, y la función intermedia no ejecuta nada tras su llamada

#### Scenario: Pila coherente
- **WHEN** ningún código descarta direcciones de retorno
- **THEN** cada llamada continúa en la instrucción siguiente y `S` vale lo mismo antes y después de ella

#### Scenario: Dirección distinta con la misma pila
- **WHEN** una función saca su dirección de retorno y empuja otra en su lugar antes del `RTS`
- **THEN** el sitio de la llamada no continúa en la instrucción siguiente

## MODIFIED Requirements

### Requirement: Control de flujo
El sistema SHALL emitir cada bloque básico como una etiqueta, las ramas y saltos como `goto` entre etiquetas de la misma función, las llamadas como llamadas C que conservan en la pila emulada la dirección de retorno y comprueban al volver que el retorno es el esperado, y los retornos como la extracción de esa dirección, su anotación en `Cpu` y `return`.

#### Scenario: Rama condicional
- **WHEN** un bloque termina en `BNE`
- **THEN** el C salta a la etiqueta del destino si Z está inactiva y, si no, continúa con el bloque siguiente

#### Scenario: Llamada
- **WHEN** el código contiene `JSR $8010`
- **THEN** el C guarda `S`, empuja la dirección de retorno en la pila emulada, llama a `f_00_8010`, comprueba que el retorno es el esperado y continúa en la instrucción siguiente

#### Scenario: Tail call
- **WHEN** un `JMP` o `JML` apunta a otra función
- **THEN** el C llama a esa función y retorna a continuación, sin copiar sus bloques
