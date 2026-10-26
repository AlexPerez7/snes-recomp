# menu-text-translation Specification

## Purpose

Traducir el texto fijo de los menús de un juego (en ALttP, la selección de partida y el registro de nombre) desde un fichero editable de la carpeta del perfil, sin tocar la ROM, su CRC32 ni las partidas guardadas, con el mismo hueco que el original y avisos legibles cuando un texto no cabe.

## Requirements
### Requirement: Volcado del texto de los menús
Un juego que ofrezca esta capacidad SHALL poder volcar el texto fijo de sus menús traducibles a un fichero de texto en la carpeta de la ROM, con una línea `clave = TEXTO` por cadena y la longitud máxima de su hueco. El fichero MUST NOT escribirse en la carpeta del perfil ni en el repositorio, porque deriva de la ROM.

#### Scenario: Volcado de ALttP
- **WHEN** se ejecuta el juego con `--game-opt dump_menu=1` y la ROM de ALttP (USA)
- **THEN** se escribe `alttp_menu_en.txt` junto a la ROM con las cadenas de la selección de partida y del registro de nombre, y el programa sale sin dejar `.srm` ni `config.ini`

#### Scenario: El volcado no se versiona
- **WHEN** se ejecuta `scan` sobre las carpetas que se van a subir
- **THEN** no encuentra el fichero del volcado ni bytes de la ROM

### Requirement: Sustitución desde un fichero de idioma
Con la opción de juego `language` en `es`, el juego SHALL leer `<carpeta del perfil>/lang/es_menu.txt` y mostrar en las pantallas afectadas el texto de cada clave que el fichero contenga. Una clave ausente MUST mostrarse en inglés. El fichero de la ROM, su CRC32 y el código generado MUST NOT cambiar.

#### Scenario: Una cadena traducida
- **WHEN** `es_menu.txt` define `select.copy` y se abre la selección de partida
- **THEN** la pantalla muestra ese texto en lugar de «COPY PLAYER» y el resto sigue en inglés

#### Scenario: Fichero ausente
- **WHEN** `language` es `es` y no existe `es_menu.txt`
- **THEN** las pantallas salen en inglés y se avisa una vez por `stderr`

#### Scenario: Con el texto original no cambia nada
- **WHEN** `es_menu.txt` es igual al volcado
- **THEN** las capturas de la selección de partida y del registro de nombre son idénticas a las de `language = en`

### Requirement: El texto respeta su hueco y la fuente
Cada cadena SHALL ocupar exactamente el hueco que el juego le da: la dirección de VRAM y la longitud de sus registros MUST conservarse. Un texto más corto MUST completarse con espacios. Un texto más largo que su hueco, o con un carácter que la fuente no tiene, MUST rechazarse con un aviso que nombre la clave y la línea, dejando esa cadena en inglés.

#### Scenario: Texto más corto
- **WHEN** una clave con hueco de 12 tiles recibe un texto de 9 caracteres
- **THEN** se dibujan los 9 caracteres seguidos de 3 espacios, sin afectar a las cadenas vecinas

#### Scenario: Texto demasiado largo
- **WHEN** el texto supera la longitud del hueco
- **THEN** se avisa con la clave y el número de línea, y esa cadena sigue en inglés

#### Scenario: Carácter sin glifo
- **WHEN** el texto contiene «ñ» o una letra con tilde
- **THEN** se avisa con la clave y el número de línea, y esa cadena sigue en inglés

### Requirement: Las partidas guardadas no cambian
Traducir los menús MUST NOT modificar los códigos de tile de las letras ni la tabla de caracteres del registro de nombre, de modo que un nombre guardado se vea igual con cualquier idioma y la SRAM resultante sea byte a byte la misma.

#### Scenario: Mismo nombre en dos idiomas
- **WHEN** se registra el nombre «LINK» con `language = en` y con `language = es`
- **THEN** los bytes del nombre en la SRAM son idénticos

### Requirement: Edición con el juego en marcha
El fichero de idioma SHALL volver a leerse cuando cambie su fecha de modificación, y el cambio MUST verse la siguiente vez que se dibuje la pantalla afectada, sin reiniciar. Una clave que se quita del fichero MUST volver a mostrarse en inglés.

#### Scenario: Editar y volver a abrir
- **WHEN** se cambia una línea de `es_menu.txt` con el juego abierto y se vuelve a la selección de partida
- **THEN** la pantalla muestra el texto nuevo

#### Scenario: Quitar una clave
- **WHEN** se borra una línea de `es_menu.txt` con el juego abierto y se vuelve a la pantalla
- **THEN** esa cadena se muestra en inglés
