# Spec Delta

## Purpose

Permitir que el runtime dibuje un campo de imagen más ancho que los 256 píxeles de la SNES, con un número configurable de píxeles extra por lado, sin alterar la imagen original cuando no se usa y sin depender de ningún juego concreto.

## ADDED Requirements

### Requirement: Ancho de imagen configurable
El runtime SHALL dibujar líneas de `256 + 2·E` píxeles, donde `E` es el ensanche por lado, un múltiplo de 8 entre 0 y 96, con 0 por defecto. Con `E = 0` la imagen, las capturas y los volcados MUST ser idénticos píxel a píxel a los del runtime sin esta capacidad.

#### Scenario: Sin ensanche
- **WHEN** se ejecuta sin pedir ensanche
- **THEN** la captura mide 256×224 y es idéntica a la de antes de este cambio

#### Scenario: Ensanche pedido
- **WHEN** se ejecuta con `--wide 64`
- **THEN** la captura mide 384×224 y la imagen original de 256 píxeles ocupa las columnas 64 a 319

#### Scenario: Valor no válido
- **WHEN** se pide un ensanche que no es múltiplo de 8 o que sale de 0 a 96
- **THEN** el runtime rechaza el valor con un mensaje y no ejecuta nada

### Requirement: Contenido de la zona nueva
En las columnas añadidas, el runtime SHALL mostrar lo que la PPU dibujaría en esas coordenadas con el estado de sus registros y memorias: los fondos MUST continuar por su mapa con el enrollado habitual, y los sprites SHALL dibujarse donde caigan. El runtime MUST NOT rellenar ni repetir el borde de la imagen original.

#### Scenario: Fondo que continúa
- **WHEN** un fondo de 32 teselas de ancho tiene un patrón distinto en cada columna y `E = 32`
- **THEN** a la izquierda de la columna 0 se ven las teselas 31, 30… del mapa y a la derecha de la 255 las teselas 0, 1…

#### Scenario: Sprite a la izquierda
- **WHEN** un sprite tiene X igual a -16 en la OAM y `E = 32`
- **THEN** se dibuja completo, 16 píxeles antes del borde izquierdo de la imagen original

#### Scenario: Color de fondo
- **WHEN** ninguna capa aporta un píxel en la zona nueva
- **THEN** se dibuja el color 0 de la CGRAM, como en el resto de la imagen

### Requirement: Ventanas con coordenada recortada
Las ventanas (`$2126-$2129`) y la ventana de color SHALL evaluarse en la zona nueva con la coordenada recortada a 0..255, de modo que el borde de la imagen original se prolonga hacia fuera.

#### Scenario: Ventana que cubre el borde
- **WHEN** una ventana cubre las columnas 0 a 40 y `E = 16`
- **THEN** las columnas añadidas a la izquierda quedan dentro de la ventana

#### Scenario: Ventana que no llega al borde
- **WHEN** una ventana cubre las columnas 100 a 200
- **THEN** ninguna columna añadida queda dentro de la ventana

### Requirement: Límites de sprites por línea
El runtime SHALL conservar los límites de la consola por línea (32 sprites y 34 trozos de 8 píxeles) con cualquier ensanche, y MUST contar los trozos que caen en la zona nueva. El ensanche no hace que se pierdan sprites distintos de los que se perderían con `E = 0` en la imagen original.

#### Scenario: Línea cargada
- **WHEN** una línea tiene más de 34 trozos y `E = 64`
- **THEN** los trozos que se descartan son los mismos que habría con `E = 0`, y los que caen fuera de la imagen original no ocupan lugar

### Requirement: Desplazamiento de X de los sprites
El runtime SHALL ofrecer al C del juego una función para fijar, por índice de sprite (0 a 127), un desplazamiento de X con signo que se suma a la X de la OAM al dibujar, y otra para quitarlos todos. Sin desplazamientos fijados, la X de un sprite de 256 a 511 MUST interpretarse como negativa, como en la consola.

#### Scenario: Sprite a la derecha
- **WHEN** el C del juego fija un desplazamiento de +256 para un sprite con X igual a 0 en la OAM y `E = 32`
- **THEN** el sprite se dibuja en la columna 256 de la imagen original, dentro de la zona nueva de la derecha

#### Scenario: Sin desplazamientos
- **WHEN** nadie fija desplazamientos
- **THEN** un sprite con X igual a 300 se dibuja en la columna -212, como antes
