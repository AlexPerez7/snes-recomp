## MODIFIED Requirements

### Requirement: Tipografía propia
El menú SHALL dibujar todo su texto con una fuente de píxeles proporcional definida en el código del proyecto, con sombra, que cubra los caracteres ASCII imprimibles y los del español (`á é í ó ú ü ñ ¿ ¡` y sus mayúsculas); el texto del menú MUST tratarse como UTF-8, de modo que medir, dibujar y recortar una cadena no parta un carácter de varios bytes. MUST NOT usar la fuente de la ROM ni de ningún recurso de Nintendo.

#### Scenario: Todos los caracteres tienen glifo
- **WHEN** se recorre el rango ASCII 32 a 126 y los caracteres del español
- **THEN** cada carácter tiene un glifo que cabe en su caja y una anchura mayor que cero

#### Scenario: Medida de texto
- **WHEN** se mide una cadena
- **THEN** su anchura es la suma de las anchuras de sus glifos más el espaciado, y coincide con lo que ocupa al dibujarla

#### Scenario: Letra con tilde
- **WHEN** se dibuja «ó» o «Ñ»
- **THEN** el acento o la tilde queda dentro de la altura de la fila y no pisa la fila de arriba

#### Scenario: UTF-8 inválido
- **WHEN** una cadena trae un byte de continuación suelto o termina a mitad de un carácter
- **THEN** se dibuja un `?` en su lugar y la medida coincide con lo dibujado
