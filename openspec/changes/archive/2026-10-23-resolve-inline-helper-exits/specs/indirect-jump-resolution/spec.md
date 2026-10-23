## MODIFIED Requirements

### Requirement: Helpers de despacho en línea
El sistema SHALL permitir declarar con `[[inline_helper]]` rutinas que reciben una tabla de punteros inmediatamente después de cada llamada y que nunca vuelven al llamador, y MUST delimitar la tabla de cada llamada sin que la configuración indique su longitud. Cuando el helper declara `exit`, el sistema MUST tratar esa instrucción como un salto cuyos destinos son la unión de los destinos de las tablas de todas las llamadas al helper.

#### Scenario: Tabla delimitada por el primer destino superior
- **WHEN** se alcanza una llamada a un helper y alguna entrada apunta al mismo banco por encima de la tabla
- **THEN** la tabla termina antes de la dirección del destino más bajo de ese tipo, y todos sus destinos se descubren

#### Scenario: Entrada sin usar
- **WHEN** una entrada de la tabla vale `0000`
- **THEN** se omite como destino y la tabla continúa

#### Scenario: Fin por entrada inválida
- **WHEN** una entrada es menor que `$8000` (distinta de `0000`) o su banco queda fuera del ROM
- **THEN** la tabla termina ahí

#### Scenario: Salto interno del helper
- **WHEN** el helper declara `exit` con la dirección de su propio salto indirecto
- **THEN** ese salto no se informa como sin resolver y tiene una arista `Dispatch` a cada destino de las tablas de las llamadas al helper

#### Scenario: Varias llamadas al mismo helper
- **WHEN** dos llamadas al helper tienen tablas distintas
- **THEN** el `exit` tiene como destinos la unión de ambas

#### Scenario: Anchos de los destinos
- **WHEN** el `exit` del helper deja M y X en un ancho distinto del que tenía el sitio de la llamada
- **THEN** los destinos de la tabla se descubren con los anchos vigentes en el `exit`
