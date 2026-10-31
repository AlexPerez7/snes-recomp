# port-launcher Specification

## Purpose

Que quien solo quiere jugar pueda abrir el programa con doble clic, elegir su ROM en una ventana y jugar, sin consola ni pasos técnicos, y que los avisos y errores se vean en esa ventana.

## ADDED Requirements

### Requirement: Ventana de lanzamiento sin argumentos
Al ejecutarse sin argumentos en Windows, `snes-port` SHALL mostrar una ventana propia y MUST NOT abrir una consola. La ventana SHALL mostrar el nombre del juego de los perfiles cargados y una acción para elegir la ROM mediante el diálogo de abrir fichero de Windows, que MUST empezar en la carpeta de la última ROM usada y filtrar `.sfc` y `.smc`.

#### Scenario: Doble clic
- **WHEN** el usuario hace doble clic en `snes-port.exe`
- **THEN** aparece la ventana de lanzamiento y no aparece ninguna ventana de consola

#### Scenario: Elegir la ROM
- **WHEN** el usuario elige una ROM reconocida en el diálogo
- **THEN** el programa construye el juego si hace falta y lo lanza con esa ROM

#### Scenario: Cancelar el diálogo
- **WHEN** el usuario cierra el diálogo sin elegir
- **THEN** la ventana sigue abierta y no se construye ni se lanza nada

### Requirement: Recordar la última ROM
El programa SHALL guardar en la caché la ruta de la última ROM lanzada con éxito. Al abrirse sin argumentos con esa ROM aún presente, la ventana SHALL ofrecer **Jugar** con ella como acción principal y una acción para cambiarla. Si el fichero ya no existe, MUST comportarse como la primera vez.

#### Scenario: Segunda vez
- **WHEN** el usuario abre el programa de nuevo y la ROM recordada existe
- **THEN** la ventana muestra el nombre de su fichero y **Jugar** la lanza sin abrir el diálogo

#### Scenario: ROM movida
- **WHEN** la ROM recordada ya no existe
- **THEN** la ventana pide elegir una, sin error

### Requirement: Estado de la construcción en la ventana
Mientras construye, el programa SHALL mostrar en la ventana qué hace (comprobando, preparando el juego por primera vez, lanzando) e indicar que la primera construcción puede tardar y solo ocurre una vez. La ventana MUST seguir respondiendo durante la construcción.

#### Scenario: Primera construcción
- **WHEN** la caché no tiene el juego
- **THEN** la ventana indica que está preparando el juego por primera vez y que puede tardar, y se puede mover y minimizar

### Requirement: Errores en la ventana
Los errores (ROM no reconocida, falta de compilador o de SDL2, construcción fallida) MUST mostrarse en la ventana con el mismo contenido que hoy en la consola, incluidos los juegos soportados cuando la ROM no se reconoce, y con una acción para copiar el detalle completo.

#### Scenario: ROM no reconocida
- **WHEN** el usuario elige una ROM cuyo CRC32 no está en ningún perfil
- **THEN** la ventana muestra el CRC32 hallado y los juegos soportados, y permite elegir otra

#### Scenario: Fallo de construcción
- **WHEN** la construcción falla
- **THEN** la ventana muestra el error y ofrece copiar el detalle al portapapeles

### Requirement: Consentimiento de descarga en la ventana
Cuando no haya ningún compilador utilizable, el programa SHALL pedir en la ventana el consentimiento para descargar el compilador portátil, diciendo qué se descarga, de dónde, su tamaño y que no se envía nada de la ROM. MUST NOT descargar nada sin una respuesta afirmativa explícita, salvo con `--download-compiler`.

#### Scenario: Aceptar
- **WHEN** el usuario acepta la descarga
- **THEN** se descarga con el hash verificado y la construcción continúa

#### Scenario: Rechazar
- **WHEN** el usuario rechaza la descarga
- **THEN** no se abre ninguna conexión de red y la ventana explica qué compilador instalar

### Requirement: Arrastrar la ROM sobre el ejecutable
Con una ROM como único argumento, el programa SHALL construir y lanzar sin mostrar el selector, y SHALL mostrar la ventana de estado solo si hay que construir, preguntar por el compilador o informar de un error.

#### Scenario: ROM arrastrada y ya construida
- **WHEN** se arrastra una ROM ya construida en la caché
- **THEN** el juego se lanza sin mostrar ninguna ventana del lanzador

### Requirement: Uso desde terminal sin cambios
Con argumentos que no sean solo una ROM, `snes-port` SHALL comportarse como hasta ahora: escribir en la consola del proceso que lo lanzó y devolver los mismos códigos de salida, sin abrir la ventana del lanzador.

#### Scenario: Línea de comandos
- **WHEN** se ejecuta `snes-port alttp_usa.sfc --no-launch` en una terminal
- **THEN** los mensajes salen en esa terminal y el código de salida es el de siempre

### Requirement: Idioma de la ventana
Los textos de la ventana SHALL estar en español cuando el idioma de interfaz de Windows sea el español y en inglés en cualquier otro caso. Los nombres de juego y los mensajes que vienen de los perfiles no se traducen.

#### Scenario: Windows en español
- **WHEN** el idioma de interfaz de Windows es el español
- **THEN** los botones y los avisos de la ventana están en español
