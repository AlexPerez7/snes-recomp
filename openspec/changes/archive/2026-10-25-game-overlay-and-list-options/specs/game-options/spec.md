## ADDED Requirements

### Requirement: Opciones de juego de tipo lista
Un perfil SHALL poder declarar opciones propias del juego, cada una con `key`, `label`, `names` (lista de al menos dos nombres) y `default` (uno de los nombres). El ejecutable interactivo MUST ofrecerlas en `config.ini` como `game.<key> = <nombre>` y en el menú F1, y el C del juego SHALL leerlas con `game_opt(key, def)`, que devuelve el índice del nombre elegido en `names`.

#### Scenario: Valor por defecto
- **WHEN** el perfil declara `key = "language"` con `names = ["en", "es"]` y `default = "en"`, y `config.ini` no trae `game.language`
- **THEN** `game_opt("language", 7)` devuelve 0 y el `config.ini` escrito en la primera ejecución contiene `game.language = en`

#### Scenario: Valor guardado
- **WHEN** `config.ini` trae `game.language = es`
- **THEN** `game_opt("language", 7)` devuelve 1

#### Scenario: Nombre desconocido
- **WHEN** `config.ini` trae `game.language = fr`, que no está en `names`
- **THEN** se ignora, se usa el valor por defecto y el resto del fichero se conserva intacto

#### Scenario: La línea de comandos manda
- **WHEN** se ejecuta con `--game-opt language=0` y `config.ini` dice `es`
- **THEN** `game_opt("language", 7)` devuelve 0 en esa ejecución y `config.ini` no cambia

### Requirement: Fila de la opción en el menú
El menú F1 SHALL mostrar cada opción de juego como una fila de lista (izquierda y derecha cambian el valor, como `filter`) en la pestaña «Game», antes de las filas de sistema, y MUST guardar el valor al cerrar el menú. Cambiar una opción con el juego en marcha SHALL surtir efecto sin reiniciar.

#### Scenario: Cambiar desde el menú
- **WHEN** se abre F1, se va a la fila `Language` y se pulsa derecha
- **THEN** la fila muestra `es`, y al cerrar el menú `config.ini` contiene `game.language = es`

#### Scenario: Juego sin opciones
- **WHEN** el perfil no declara `[[option]]`
- **THEN** la pestaña «Game» tiene exactamente las filas de sistema de antes y `config.ini` no gana ninguna clave `game.*`

### Requirement: Opciones en el runtime sin vídeo
El runtime sin vídeo SHALL aceptar `--game-opt CLAVE=NOMBRE` o `CLAVE=NUMERO` para las opciones declaradas, de modo que las pruebas y capturas sin ventana usen las mismas opciones que el juego interactivo.

#### Scenario: Nombre en la línea de comandos
- **WHEN** se ejecuta `--game-opt language=es`
- **THEN** `game_opt("language", 7)` devuelve 1
