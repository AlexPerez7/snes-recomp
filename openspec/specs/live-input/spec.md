# live-input Specification

## Purpose

Traducir teclado y mandos físicos a los botones del joypad de la consola en tiempo real, con la misma temporización de lectura que ya usa el runtime sin vídeo.

## Requirements

### Requirement: Teclado
El ejecutable interactivo SHALL asignar por defecto las flechas a la cruceta, X a B, Z a Y, S a A, A a X, Q a L, W a R, Enter a START y Right Shift a SELECT, y MUST permitir cambiar la asignación en un fichero de texto junto a la ROM.

#### Scenario: Pulsar una tecla
- **WHEN** se mantiene Enter durante tres fotogramas
- **THEN** la lectura automática de `$4218-$4219` devuelve START pulsado en esos fotogramas

#### Scenario: Asignación propia
- **WHEN** el fichero de asignación liga la tecla Space al botón A
- **THEN** pulsar Space pulsa A y la tecla S ya no lo hace

### Requirement: Mandos SDL
El ejecutable SHALL reconocer los mandos que SDL2 describa como game controller, asignar los dos primeros a los joypads 1 y 2, y aceptar su conexión y desconexión en caliente.

#### Scenario: Mando conectado en marcha
- **WHEN** se conecta un mando durante la partida
- **THEN** sus botones controlan el siguiente joypad libre sin reiniciar

### Requirement: Estado en el borde del fotograma
El estado de los botones SHALL fijarse al comienzo de cada fotograma, de modo que la lectura automática y la serie vean el mismo estado dentro del fotograma, y las direcciones opuestas pulsadas a la vez MUST no llegar juntas al juego.

#### Scenario: Izquierda y derecha
- **WHEN** se pulsan LEFT y RIGHT a la vez
- **THEN** el juego no lee ninguna de las dos
