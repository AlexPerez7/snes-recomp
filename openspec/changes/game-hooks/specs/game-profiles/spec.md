## ADDED Requirements

### Requirement: Perfil de juego autocontenido
Un perfil de juego SHALL describirse por completo en un fichero `.toml`: una sección `[game]` con `id`, `name` y `crc32`, más las entradas, tablas y ganchos del juego. El programa MUST obtener la lista de juegos conocidos de los perfiles incrustados y no de constantes en el código.

#### Scenario: Perfil válido
- **WHEN** un `.toml` tiene `[game]` con `id`, `name` y `crc32`
- **THEN** el programa lo reconoce como el perfil de la ROM con ese CRC32

#### Scenario: Perfil sin identidad
- **WHEN** un `.toml` de perfil carece de `[game]` o de alguno de sus campos
- **THEN** el programa informa del error con el nombre del fichero y no lo usa

#### Scenario: CRC32 repetido
- **WHEN** dos perfiles cargados declaran el mismo `crc32`
- **THEN** el programa informa del conflicto con los dos ficheros y no construye nada

### Requirement: Perfil externo
`snes-port --game <perfil.toml>` SHALL cargar ese perfil, y el directorio hermano de igual nombre sin extensión si existe, además de los incrustados, y el perfil externo MUST prevalecer sobre un incrustado con el mismo `crc32`.

#### Scenario: Juego solo en un perfil externo
- **WHEN** se pasa una ROM cuyo CRC32 solo figura en el perfil dado con `--game`
- **THEN** se construye y se lanza con ese perfil

#### Scenario: Perfil externo sobre uno incrustado
- **WHEN** el perfil dado con `--game` tiene el `crc32` de ALttP
- **THEN** se usan sus entradas y ganchos y no los incrustados

#### Scenario: Perfil inexistente
- **WHEN** el fichero de `--game` no existe o no se puede leer
- **THEN** el programa termina con un error que nombra el fichero y no genera nada
