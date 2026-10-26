## MODIFIED Requirements

### Requirement: Filtro
El ejecutable SHALL ofrecer los filtros `nearest`, `bilinear` y `sharp` (bilinear con nitidez por píxel para escalado no entero), por defecto `sharp`. Cuando la imagen se dibuja con un factor entero exacto en ambos ejes, `sharp` MUST producir la misma imagen que `nearest`. Para un factor no entero, `sharp` SHALL ampliar antes con vecino más cercano por el menor factor entero que no sea inferior al mayor de los factores de ancho y alto, y reducir después con bilinear, también con `aspect = free`.

#### Scenario: Valor por defecto
- **WHEN** `filter` no está en `config.ini`
- **THEN** el filtro vigente es `sharp`

#### Scenario: Factor entero
- **WHEN** el filtro es `sharp` y la imagen se dibuja a 3x exactos
- **THEN** la imagen es idéntica a la que da `nearest`

#### Scenario: Escalado no entero con `sharp`
- **WHEN** el escalado es `fit` y el filtro `sharp`
- **THEN** los bordes de los píxeles no se muestran borrosos ni con ondulaciones

#### Scenario: Aspecto libre
- **WHEN** el aspecto es `free`, la ventana mide 1600x900 y el filtro es `sharp`
- **THEN** la textura previa se amplía con un factor que cubre tanto el factor horizontal como el vertical

#### Scenario: Clave ya escrita
- **WHEN** `config.ini` trae `filter = nearest`
- **THEN** se respeta ese valor
