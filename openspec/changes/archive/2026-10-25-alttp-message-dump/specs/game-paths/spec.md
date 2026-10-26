## ADDED Requirements

### Requirement: Carpeta de la ROM
`snes_rt.h` SHALL ofrecer `const char *rom_dir(void)`, la carpeta de la ROM con la que se arrancó el programa, terminada en el separador de rutas (`./` si la ruta de la ROM no lleva carpeta). El valor MUST ser el mismo en el runtime sin vídeo y en el interactivo.

#### Scenario: ROM con carpeta
- **WHEN** se arranca con `roms/alttp_usa.sfc`
- **THEN** `rom_dir()` empieza por `roms/` y termina en un separador

#### Scenario: ROM sin carpeta
- **WHEN** se arranca con `alttp_usa.sfc`
- **THEN** `rom_dir()` devuelve `./`

### Requirement: Carpeta del perfil del juego
`snes_rt.h` SHALL ofrecer `const char *game_dir(void)`, la carpeta que el perfil del juego trae junto a su `.toml` (la que lleva su mismo nombre y guarda su C propio), terminada en el separador. El runtime MUST aceptarla con `--game-dir <carpeta>`, y `snes-port` SHALL pasarla al lanzar el juego cuando el perfil tiene esa carpeta. Sin `--game-dir`, `game_dir()` MUST devolver lo mismo que `rom_dir()`.

#### Scenario: Lanzado con snes-port
- **WHEN** `snes-port roms/x.sfc --game games/x.toml` lanza un juego cuyo perfil tiene la carpeta `games/x/`
- **THEN** el C del juego ve `game_dir()` igual a esa carpeta, aunque el programa corra con otro directorio de trabajo

#### Scenario: Programa lanzado sin snes-port
- **WHEN** se arranca el juego a mano sin `--game-dir`
- **THEN** `game_dir()` devuelve la carpeta de la ROM

#### Scenario: Perfil sin carpeta
- **WHEN** el perfil solo trae su `.toml`
- **THEN** `snes-port` no pasa `--game-dir` y `game_dir()` es la carpeta de la ROM

### Requirement: Los ficheros de datos no cambian la construcción
Los ficheros de la carpeta del perfil que no sean `*.c` ni `*.h` MUST NOT formar parte de la versión de la caché ni obligar a reconstruir el juego, de modo que se puedan editar con el juego ya construido.

#### Scenario: Editar un fichero de datos
- **WHEN** se modifica `games/x/lang/es.txt` con el juego ya construido
- **THEN** el siguiente arranque de `snes-port` no reconstruye y el juego lee el fichero nuevo
