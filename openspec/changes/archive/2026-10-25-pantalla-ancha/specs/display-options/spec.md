# Spec Delta

## ADDED Requirements

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
