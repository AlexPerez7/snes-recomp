# display-options Specification

## Purpose

Ofrecer las opciones de presentación que se esperan de un port de PC: pantalla completa, aspecto, escalado, filtro y efectos de pantalla opcionales.

## Requirements

### Requirement: Pantalla completa
El ejecutable SHALL arrancar en pantalla completa si `fullscreen = true`, y F11 MUST alternar el modo en marcha sin perder el estado del juego, usando por defecto pantalla completa sin bordes a la resolución del escritorio.

#### Scenario: Alternar
- **WHEN** se pulsa F11 en modo ventana
- **THEN** pasa a pantalla completa y el juego continúa en el mismo fotograma

### Requirement: Relación de aspecto
El ejecutable SHALL ofrecer los aspectos `4:3` (píxel no cuadrado), `8:7` (píxel cuadrado) y `free`, y MUST conservar la proporción elegida al redimensionar la ventana con barras negras.

#### Scenario: 4:3 en pantalla 16:9
- **WHEN** la pantalla mide 1920x1080 y el aspecto es 4:3
- **THEN** la imagen ocupa 1440x1080 centrada con barras laterales

### Requirement: Escalado
El ejecutable SHALL ofrecer escalado `integer` (múltiplos enteros) y `fit` (relleno del área disponible), por defecto `integer`.

#### Scenario: Entero
- **WHEN** el área es 1000x900 con aspecto 8:7
- **THEN** la imagen se dibuja a 3x (768x672) centrada

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

### Requirement: Recorte de márgenes
El ejecutable SHALL permitir recortar hasta 8 líneas por arriba y por abajo (`overscan`), por defecto 0, mostrando 224 líneas.

#### Scenario: Recorte
- **WHEN** `overscan = 8`
- **THEN** la imagen mostrada tiene 208 líneas y se reescala manteniendo el aspecto

### Requirement: Efectos de pantalla opcionales
El ejecutable SHALL ofrecer `scanlines` (intensidad 0 a 100) y `grid`, desactivados por defecto, y MUST no cambiar la imagen cuando están desactivados.

#### Scenario: Scanlines
- **WHEN** `scanlines = 50`
- **THEN** una de cada dos líneas de pantalla se oscurece un 50 %

#### Scenario: Desactivados
- **WHEN** ningún efecto está activo
- **THEN** la imagen mostrada es la del búfer de fotograma sin alterar

### Requirement: Campo de imagen ancho
El ejecutable SHALL ofrecer la opción `wide` (`config.ini`, `--opt wide=N` y pestaña Image del menú F1), un múltiplo de 8 entre 0 y 96 que ensancha la imagen en ese número de píxeles por lado, por defecto 0. El rectángulo de la imagen, el aspecto, el escalado, los efectos y la captura F10 MUST usar el ancho `256 + 2·wide`, y cambiarla con el juego en marcha SHALL surtir efecto en el fotograma siguiente sin perder el estado.

#### Scenario: Valor por defecto
- **WHEN** `wide` no está en `config.ini`
- **THEN** la imagen mide 256×224 y todo se muestra como antes

#### Scenario: Aspecto con píxel cuadrado
- **WHEN** `wide = 72`, el aspecto es 8:7 y la ventana mide 1920x1080
- **THEN** la imagen es de 400×224 píxeles de juego, se dibuja a 4x (1600x896) centrada y conserva la proporción

#### Scenario: Cambio en marcha
- **WHEN** se cambia `wide` de 0 a 32 desde el menú F1
- **THEN** el fotograma siguiente se dibuja con 320 columnas y el juego sigue donde estaba

#### Scenario: Valor fuera de rango
- **WHEN** `config.ini` trae `wide = 500`
- **THEN** se ignora y se conserva el valor por defecto, como hace el resto de las opciones numéricas

#### Scenario: Valor que no es múltiplo de 8
- **WHEN** `config.ini` trae `wide = 69`
- **THEN** se usa 64, el múltiplo de 8 inferior
