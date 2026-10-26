## ADDED Requirements

### Requirement: Nombres legibles de teclas y botones
La pestaña Controles SHALL mostrar cada tecla y cada botón de mando con un nombre pensado para el jugador (por ejemplo «Flecha ↑», «Intro», «Mayús der.» con el menú en español y «Up Arrow», «Enter», «R Shift» en inglés) y MUST caer al nombre de SDL cuando no tenga uno propio. Lo que se escribe en `config.ini` MUST seguir siendo el nombre de SDL de siempre.

#### Scenario: Tecla con nombre propio
- **WHEN** el botón START está asignado a Return y el menú va en español
- **THEN** la pestaña muestra «Intro» y `config.ini` conserva `Return`

#### Scenario: Tecla sin nombre propio
- **WHEN** un botón está asignado a una tecla que la tabla no conoce
- **THEN** se muestra el nombre de SDL de esa tecla, recortado con «...» si no cabe

### Requirement: Teclado y mando en columnas separadas
La pestaña SHALL mostrar, para cada botón de la consola, el teclado y el mando en columnas distintas con su cabecera, sin pegarlos en una sola cadena. Varias asignaciones de un mismo botón MUST separarse con un carácter que la fuente dibuje, y un botón sin asignación en un dispositivo MUST mostrar un guion atenuado.

#### Scenario: Dos asignaciones
- **WHEN** el botón A tiene Space y Z en el teclado y el botón B del mando
- **THEN** la columna del teclado muestra las dos teclas separadas y la del mando muestra el botón, sin que se junten letras

#### Scenario: Jugador sin teclado
- **WHEN** se muestra el jugador 2, que no tiene teclas por defecto
- **THEN** la columna del teclado muestra el guion atenuado en cada fila

### Requirement: Nada se corta ni se solapa
Las etiquetas y los valores de la pestaña SHALL recortarse con «...» en frontera de carácter y MUST NOT salirse de su columna ni del panel, en inglés y en español.

#### Scenario: Fila del jugador
- **WHEN** se abre la pestaña en español
- **THEN** el selector del jugador se lee entero («Jugador 1») sin texto cortado

#### Scenario: Valor largo
- **WHEN** una tecla tiene un nombre más ancho que su columna
- **THEN** se recorta con «...» sin pisar la columna vecina

### Requirement: La edición sigue funcionando
Pulsar Intro sobre una fila SHALL pedir una tecla o un botón y asignarlo como hasta ahora, con el mismo aviso si ya está en uso; mientras espera, las celdas de teclado y mando de esa fila MUST mostrar «...», porque aún no se sabe qué dispositivo responderá.

#### Scenario: Reasignar
- **WHEN** el usuario elige la fila del botón X, pulsa Intro y luego la tecla E
- **THEN** la columna del teclado de esa fila muestra «E» y `config.ini` guarda `p1.key.X = E` (más lo que hubiera)
