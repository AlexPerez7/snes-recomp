## MODIFIED Requirements

### Requirement: ROM no reconocida
Si el CRC32 de la ROM no corresponde a ningún perfil cargado, el programa SHALL terminar con código de salida 2 sin generar nada, e indicar el CRC32 hallado y los juegos soportados por los perfiles cargados. Si no hay ningún perfil cargado, el mensaje MUST decir dónde busca perfiles (`games/` junto al ejecutable, `games/` del directorio actual y `--game`) y que un perfil se obtiene del repositorio del juego.

#### Scenario: ROM desconocida
- **WHEN** se pasa una ROM cuyo CRC32 no figura en ningún perfil
- **THEN** termina con código 2, muestra el CRC32 y no crea ficheros en la caché

#### Scenario: Cabecera de copiador
- **WHEN** la ROM tiene una cabecera de 512 bytes y el resto coincide con una conocida
- **THEN** se reconoce igualmente

#### Scenario: Sin ningún perfil
- **WHEN** no hay carpetas `games/` con perfiles ni `--game`
- **THEN** termina con código 2 y el mensaje cita las carpetas donde busca y cómo conseguir un perfil
