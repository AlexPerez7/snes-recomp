## Purpose
Resolver los saltos y llamadas indirectas del código de un juego mediante tablas y helpers declarados en su configuración, para que el descubrimiento de código alcance todos los destinos.

## ADDED Requirements

### Requirement: Tablas de saltos por juego
El sistema SHALL resolver los saltos y llamadas indirectas declarados en `games/<id>.toml` mediante tablas de tipo `indirect`, `split` o `inline`, añadiendo todos sus destinos al descubrimiento de código.

#### Scenario: Tabla configurada
- **WHEN** un sitio indirecto tiene una `[[table]]` en la configuración
- **THEN** cada destino de la tabla se descubre como código y el sitio no aparece como sin resolver

#### Scenario: Destino fuera del ROM
- **WHEN** un destino calculado de una tabla no cae en el ROM mapeado
- **THEN** el sistema MUST informar del error con el sitio y la entrada de tabla, sin abortar el resto del análisis

### Requirement: Helpers de despacho en línea
El sistema SHALL permitir declarar con `[[inline_helper]]` rutinas que reciben una tabla de punteros inmediatamente después de cada llamada y que nunca vuelven al llamador, y MUST delimitar la tabla de cada llamada sin que la configuración indique su longitud.

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
- **THEN** ese salto no se informa como sin resolver

### Requirement: Datos que no se decodifican
El sistema SHALL permitir marcar con `[[not_code]]` direcciones a las que apuntan las tablas pero que contienen relleno o datos, y el descubrimiento MUST NOT decodificarlas.

#### Scenario: Destino marcado como no código
- **WHEN** un destino de tabla coincide con una dirección `not_code`
- **THEN** no se añade ninguna instrucción desde esa dirección

### Requirement: Reporte de sitios sin resolver
El sistema SHALL listar con `discover --unresolved` cada salto indirecto alcanzable que no tenga tabla configurada, e indicar el total al final.

#### Scenario: Sitios pendientes
- **WHEN** se ejecuta `discover --unresolved` sobre un juego con saltos indirectos sin tabla
- **THEN** se imprime una línea por sitio y un resumen con el número de sitios sin resolver

### Requirement: Tablas de ALttP verificadas
Cada tabla añadida a `games/alttp_usa.toml` MUST estar contrastada a mano con el ROM real y llevar un comentario que describa el patrón de código que la usa.

#### Scenario: Sitio no resoluble estáticamente
- **WHEN** un sitio indirecto depende de un valor calculado en tiempo de ejecución sin tabla fija en el ROM
- **THEN** queda documentado en el TOML con el motivo y permanece como sin resolver
