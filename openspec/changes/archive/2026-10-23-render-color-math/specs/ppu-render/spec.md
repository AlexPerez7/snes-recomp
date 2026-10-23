## ADDED Requirements

### Requirement: Color math
El runtime SHALL aplicar el color math de `$2130-$2132` a cada píxel de la pantalla principal: tomar como segundo operando el color fijo de `$2132`, o la subpantalla (capas de `$212D`) si el bit 1 de `$2130` está activo y esta aporta un píxel; sumar o restar (bit 7 de `$2131`) por canal de color con saturación a 0 y 31; y dividir por 2 si el bit 6 de `$2131` está activo. SHALL aplicarlo solo a las capas elegidas en `$2131` (bits 0-5, con el color de fondo en el bit 5), y la región de `$2130` bits 4-5 (siempre aplicar y nunca aplicar) MUST respetarse; el color math opera sobre los 5 bits de la CGRAM y el brillo se aplica después.

#### Scenario: Suma del color fijo
- **WHEN** `$2131` es `21`, el color fijo es rojo pleno y un píxel de BG1 es verde pleno
- **THEN** el píxel resultante es amarillo

#### Scenario: Resta
- **WHEN** `$2131` es `A1` y el color fijo tiene el rojo a 31 sobre un píxel blanco de BG1
- **THEN** el píxel resultante tiene rojo 0 y verde y azul a 31

#### Scenario: Mitad
- **WHEN** el bit 6 de `$2131` está activo
- **THEN** el resultado se divide por 2 antes del brillo

#### Scenario: Capa no elegida
- **WHEN** un píxel de BG2 se dibuja y el bit de BG2 de `$2131` está a 0
- **THEN** conserva su color sin mezcla

#### Scenario: Subpantalla
- **WHEN** el bit 1 de `$2130` está activo y BG3 está solo en `$212D`
- **THEN** el segundo operando es el píxel de BG3 de esa posición, o el color fijo si la subpantalla no aporta píxel

#### Scenario: Nunca
- **WHEN** los bits 4-5 de `$2130` valen 3 (color math prohibido siempre)
- **THEN** ningún píxel se mezcla

## MODIFIED Requirements

### Requirement: Límites declarados
El runtime SHALL avisar en el informe cuando la imagen use una función que no dibuja: un modo gráfico distinto del 1, ventanas, mosaico, o una región de color math que depende de ventanas. MUST dibujar en esos casos las capas que sí sabe dibujar y no inventar el resto.

#### Scenario: Otro modo gráfico
- **WHEN** `$2105` selecciona el modo 7
- **THEN** el informe indica que el modo 7 no se dibuja y la imagen contiene solo el color de fondo y los sprites

#### Scenario: Color math
- **WHEN** `$2131` tiene alguna capa activada y los bits 4-5 de `$2130` valen 0 o 3
- **THEN** el color math se aplica y el informe no avisa de que no se aplique

#### Scenario: Color math con ventana
- **WHEN** los bits 4-5 de `$2130` piden una región que depende de la ventana de color
- **THEN** el informe avisa de que esa región no se aplica
