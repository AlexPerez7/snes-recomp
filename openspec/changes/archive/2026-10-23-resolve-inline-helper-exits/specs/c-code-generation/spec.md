## MODIFIED Requirements

### Requirement: Despacho indirecto
El sistema SHALL emitir los saltos y llamadas indirectos calculando el destino en ejecución como lo hace la CPU y despachando con un `switch` sobre los destinos conocidos del grafo. Esto incluye el `exit` de un helper de despacho en línea: tras llamar a la función del destino, el helper MUST retornar, y el sitio de la llamada MUST retornar sin ejecutar nada más.

#### Scenario: Destino conocido
- **WHEN** el destino calculado en ejecución es uno de los de la tabla
- **THEN** se llama a la función de ese destino

#### Scenario: Destino desconocido
- **WHEN** el destino calculado no está entre los conocidos
- **THEN** el C llama a `rt_unresolved` con la dirección del sitio y la del destino

#### Scenario: Salida de un helper en línea
- **WHEN** se ejecuta una llamada a un helper en línea y el `exit` calcula un destino de la tabla
- **THEN** se ejecuta la función de ese destino, la pila queda con el estado que tendría en la CPU tras el salto y el control vuelve al llamador del sitio de la llamada, no a la instrucción siguiente
