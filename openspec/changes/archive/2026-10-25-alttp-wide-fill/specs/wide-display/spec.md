# Spec Delta

## ADDED Requirements

### Requirement: Ensanche visible para el C del juego
El runtime SHALL ofrecer al C del juego una función que devuelva el ensanche `E` vigente por lado, para que un juego adapte su comportamiento al ancho de la imagen. Con `E = 0` MUST devolver 0, y el valor MUST reflejar un cambio de `E` en cuanto este tiene efecto.

#### Scenario: Sin ensanche
- **WHEN** el C del juego consulta el ensanche y no se pidió ninguno
- **THEN** recibe 0

#### Scenario: Ensanche pedido
- **WHEN** se ejecuta con `--wide 64` y el C del juego consulta el ensanche
- **THEN** recibe 64

#### Scenario: Cambio en marcha
- **WHEN** el usuario cambia el ensanche de 32 a 72 desde el menú
- **THEN** la consulta devuelve 72 a partir del fotograma en que el cambio se aplica

### Requirement: Franjas en blanco pedidas por el juego
El runtime SHALL ofrecer al C del juego una función para fijar cuántos píxeles de la zona nueva de cada lado, contados desde el borde exterior de la imagen hacia el centro, se dibujan en negro en lugar de lo que dibujaría la PPU, y una forma de quitarlas. Sin franjas fijadas la imagen MUST ser la de siempre. Una franja MUST NOT superar `E` píxeles ni afectar a la imagen original de 256 píxeles.

#### Scenario: Franja a la izquierda
- **WHEN** el C del juego fija 24 píxeles de franja a la izquierda y `E = 32`
- **THEN** las columnas 0 a 23 de la imagen (las exteriores, contadas desde el borde izquierdo) son negras, las columnas 24 a 31 muestran lo que dibuja la PPU y la imagen original no cambia

#### Scenario: Franja mayor que E
- **WHEN** el C del juego fija 64 píxeles con `E = 32`
- **THEN** se dibujan 32 píxeles de franja, sin salir de la zona nueva

#### Scenario: Sin franjas
- **WHEN** nadie fija franjas
- **THEN** la zona nueva muestra lo que dibuja la PPU, como antes de este requisito
