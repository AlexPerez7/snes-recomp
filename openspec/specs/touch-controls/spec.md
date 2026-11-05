# touch-controls Specification

## Purpose

Controles táctiles en pantalla y mandos físicos para el frontal en Android.

## Requirements

### Requirement: Controles táctiles
La app SHALL mostrar en pantalla la cruceta, A, B, X, Y, L, R, Start y Select y SHALL alimentar con ellos el mando del jugador 1 como si fueran un mando físico.

#### Scenario: Jugar con el táctil
- **WHEN** el usuario toca la cruceta y los botones
- **THEN** el juego recibe esas pulsaciones en `joy1_state`

### Requirement: Mandos físicos
Cuando se use un mando SDL (Bluetooth o USB), la app SHALL ocultar los controles táctiles y SHALL volver a mostrarlos al tocar la pantalla.

#### Scenario: Mando conectado
- **WHEN** el usuario pulsa un botón de un mando conectado
- **THEN** los controles táctiles se ocultan y el mando maneja al jugador 1
