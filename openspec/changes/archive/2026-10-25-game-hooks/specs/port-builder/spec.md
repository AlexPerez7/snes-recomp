## MODIFIED Requirements

### Requirement: Construir y lanzar con un comando
`snes-port <rom>` SHALL identificar la ROM por su CRC32 entre los perfiles cargados (incrustados o dados con `--game`), generar el C, compilarlo junto al C propio del perfil y lanzar el juego, y MUST aceptar la ROM arrastrada sobre el ejecutable. Si la ROM ya está construida en la caché con la misma versión del programa y el mismo contenido del perfil y de su C, SHALL lanzar el juego sin regenerar ni recompilar.

#### Scenario: Primera ejecución
- **WHEN** se ejecuta `snes-port alttp_usa.sfc` sin caché
- **THEN** genera, compila y lanza el juego, y deja el resultado en la caché del usuario

#### Scenario: Segunda ejecución
- **WHEN** se repite el comando con la misma ROM, la misma versión y el mismo perfil
- **THEN** el juego arranca sin ejecutar el generador ni el compilador

#### Scenario: Versión nueva del programa
- **WHEN** la caché la hizo otra versión del programa
- **THEN** se reconstruye antes de lanzar

#### Scenario: Perfil modificado
- **WHEN** cambia el `.toml` del perfil o un fichero de su directorio de C
- **THEN** se reconstruye antes de lanzar

### Requirement: ROM no reconocida
Si el CRC32 de la ROM no corresponde a ningún perfil cargado, el programa SHALL terminar con código de salida 2 sin generar nada, e indicar el CRC32 hallado y los juegos soportados, tomados de los `[game]` de los perfiles.

#### Scenario: ROM desconocida
- **WHEN** se pasa una ROM cuyo CRC32 no figura en ningún perfil
- **THEN** termina con código 2, muestra el CRC32 y no crea ficheros en la caché

#### Scenario: Cabecera de copiador
- **WHEN** la ROM tiene una cabecera de 512 bytes y el resto coincide con una conocida
- **THEN** se reconoce igualmente
