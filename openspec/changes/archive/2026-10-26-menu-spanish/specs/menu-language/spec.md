## ADDED Requirements

### Requirement: El idioma del menú sigue la opción del juego
El menú del frontal SHALL mostrarse en español cuando la opción de juego del perfil con clave `language` tiene el valor `es`, y en inglés en cualquier otro caso: valor distinto, perfil sin esa opción o ejecución sin perfil. Cambiar el valor de esa opción desde el propio menú MUST cambiar el idioma del menú en el fotograma siguiente, sin reiniciar.

#### Scenario: Juego en español
- **WHEN** el perfil declara `language` con los valores `en` y `es` y `config.ini` tiene `game.language = es`
- **THEN** las pestañas, las filas, los avisos y la ayuda del menú salen en español

#### Scenario: Juego sin opción de idioma
- **WHEN** el perfil no declara ninguna opción `language`
- **THEN** el menú sale en inglés

#### Scenario: Cambiar el idioma desde el menú
- **WHEN** el usuario cambia `Language` de `en` a `es` en la pestaña Game
- **THEN** el menú se vuelve a dibujar en español sin cerrarlo

### Requirement: Se traduce lo que se dibuja, no lo que se guarda
La traducción SHALL afectar solo al texto que el menú dibuja (nombres de pestañas, etiquetas de filas, valores mostrados, avisos y ayuda). Las claves y los valores de `config.ini`, sus comentarios, la salida de consola y los argumentos de línea de comandos MUST NOT cambiar con el idioma, y un `config.ini` escrito con el menú en un idioma MUST leerse igual con el otro.

#### Scenario: Un valor mostrado y un valor guardado
- **WHEN** el menú está en español y el usuario elige el filtro «sharp»
- **THEN** `config.ini` contiene `filter = sharp`

#### Scenario: Mismo fichero con los dos idiomas
- **WHEN** se arranca con el mismo `config.ini` con `language = en` y con `language = es`
- **THEN** `--print-config` imprime lo mismo en los dos casos

### Requirement: Todo el texto en español cabe
Cada cadena del menú en español MUST caber en el sitio donde se dibuja (fila, valor, tab, aviso o ayuda) con el ancho que ocupa en pantalla; el texto que no quepa, como el nombre de un dispositivo de audio, SHALL recortarse con «...» sin partir un carácter de varios bytes.

#### Scenario: Ninguna cadena se sale
- **WHEN** se mide cada cadena de la tabla en español con la fuente del menú
- **THEN** su anchura no supera la del hueco en que se dibuja

#### Scenario: Recorte con tilde
- **WHEN** se recorta con «...» una cadena con tildes que no cabe
- **THEN** el resultado es UTF-8 válido y no queda un byte suelto de un carácter

### Requirement: Original y sin datos de ROM
La tabla de cadenas en español SHALL ser texto original del proyecto y MUST NOT incluir texto, gráficos ni datos de ninguna ROM; las cadenas sin traducción MUST mostrarse en inglés.

#### Scenario: Cadena sin traducir
- **WHEN** una cadena del menú no tiene entrada en español
- **THEN** se muestra su texto en inglés
