# Spec Delta

## Purpose

Ofrecer las opciones de presentación que se esperan de un port de PC: pantalla completa, aspecto, escalado, filtro y efectos de pantalla opcionales.

## ADDED Requirements

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
El ejecutable SHALL ofrecer los filtros `nearest`, `bilinear` y `sharp` (bilinear con nitidez por píxel para escalado no entero), por defecto `nearest`.

#### Scenario: Escalado no entero con `sharp`
- **WHEN** el escalado es `fit` y el filtro `sharp`
- **THEN** los bordes de los píxeles no se muestran borrosos ni con ondulaciones

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
