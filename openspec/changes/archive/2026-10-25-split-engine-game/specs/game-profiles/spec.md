## MODIFIED Requirements

### Requirement: Perfil de juego autocontenido
Un perfil de juego SHALL describirse por completo en un fichero `.toml`: una sección `[game]` con `id`, `name` y `crc32`, más las entradas, tablas y ganchos del juego. El programa MUST NOT incrustar ningún perfil de juego: la lista de juegos conocidos sale de los perfiles que encuentra en las carpetas de búsqueda y en `--game`.

#### Scenario: Perfil válido
- **WHEN** un `.toml` tiene `[game]` con `id`, `name` y `crc32`
- **THEN** el programa lo reconoce como el perfil de la ROM con ese CRC32

#### Scenario: Perfil sin identidad
- **WHEN** un `.toml` de perfil carece de `[game]` o de alguno de sus campos
- **THEN** el programa informa del error con el nombre del fichero y no lo usa

#### Scenario: CRC32 repetido
- **WHEN** dos perfiles cargados declaran el mismo `crc32`
- **THEN** el programa informa del conflicto con los dos ficheros y no construye nada

#### Scenario: Sin perfiles incrustados
- **WHEN** se ejecuta `snes-port` sin carpetas `games/` ni `--game`
- **THEN** no hay ningún juego conocido, aunque la ROM sea una que el proyecto soporta en otro repositorio

### Requirement: Perfil externo
`snes-port --game <perfil.toml>` SHALL cargar ese perfil, y el directorio hermano de igual nombre sin extensión si existe, además de los encontrados en las carpetas de búsqueda, y el perfil dado con `--game` MUST prevalecer sobre uno encontrado en una carpeta con el mismo `crc32`.

#### Scenario: Juego solo en un perfil externo
- **WHEN** se pasa una ROM cuyo CRC32 solo figura en el perfil dado con `--game`
- **THEN** se construye y se lanza con ese perfil

#### Scenario: Perfil externo sobre uno incrustado
- **WHEN** el perfil dado con `--game` y uno de una carpeta `games/` (el caso que antes era «incrustado») tienen el mismo `crc32`
- **THEN** se usan las entradas y ganchos del dado con `--game`

#### Scenario: Perfil inexistente
- **WHEN** el fichero de `--game` no existe o no se puede leer
- **THEN** el programa termina con un error que nombra el fichero y no genera nada

## ADDED Requirements

### Requirement: Búsqueda de perfiles en carpetas
`snes-port` SHALL buscar perfiles (`*.toml` con su carpeta hermana de C) en `games/` junto al ejecutable y en `games/` del directorio actual, y MUST ignorar una carpeta que no exista. Si el mismo `crc32` aparece en las dos carpetas, el del directorio actual SHALL prevalecer.

#### Scenario: Perfil junto al ejecutable
- **WHEN** `games/alttp_usa.toml` está junto a `snes-port.exe` y se pasa la ROM correspondiente sin `--game`
- **THEN** se construye y se lanza con ese perfil

#### Scenario: Directorio actual sobre el del ejecutable
- **WHEN** las dos carpetas tienen un perfil con el mismo `crc32`
- **THEN** se usa el del directorio actual

#### Scenario: Un fichero roto en la carpeta
- **WHEN** un `.toml` de `games/` no es un perfil válido
- **THEN** el programa informa de él con su nombre y sigue con los demás perfiles, sin usar el roto

#### Scenario: Los comandos de análisis
- **WHEN** se ejecuta `snes-recomp discover` o `codegen` con una ROM cuyo perfil está en una carpeta de búsqueda y sin `--config`
- **THEN** se usa ese perfil, como `snes-port`
