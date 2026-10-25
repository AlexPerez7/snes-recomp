# Spec Delta

## Purpose

Guardar las preferencias del usuario del port (imagen, controles, audio) en un fichero legible y editable, aplicarlas al arrancar y poder cambiarlas desde dentro del juego.

## ADDED Requirements

### Requirement: Fichero de configuración
El ejecutable interactivo SHALL leer `config.ini` al arrancar y, si no existe, crearlo con los valores por defecto. Las claves desconocidas MUST conservarse al reescribir y una clave inválida MUST ignorarse con un aviso, usando su valor por defecto.

#### Scenario: Primera ejecución
- **WHEN** no existe `config.ini`
- **THEN** se crea con todas las opciones y sus valores por defecto, y el juego arranca con ellos

#### Scenario: Valor inválido
- **WHEN** `scale = banana`
- **THEN** se avisa, se usa el valor por defecto y el resto de la configuración se aplica

#### Scenario: Clave desconocida
- **WHEN** el fichero tiene una clave que este programa no conoce
- **THEN** sigue ahí tras guardar la configuración desde el menú

### Requirement: Precedencia
Las opciones de la línea de comandos SHALL prevalecer sobre el fichero durante esa ejecución sin modificarlo.

#### Scenario: Pantalla completa por línea de comandos
- **WHEN** el fichero tiene `fullscreen = false` y se ejecuta con `--fullscreen`
- **THEN** arranca en pantalla completa y el fichero conserva `false`

### Requirement: Menú en pantalla
F1 SHALL abrir un menú que se maneje con el teclado y el mando, pause la emulación y permita cambiar las opciones de imagen, controles y audio, aplicándolas al instante y guardándolas al salir del menú.

#### Scenario: Cambiar el filtro
- **WHEN** el usuario cambia el filtro a `bilinear` en el menú y cierra el menú
- **THEN** la imagen usa el filtro nuevo y `config.ini` contiene `filter = bilinear`

### Requirement: Remapeo de controles
El menú SHALL permitir asignar un botón de la consola a una tecla o a un botón del mando pulsándolo, por jugador, y MUST avisar si la tecla ya está usada por otro botón.

#### Scenario: Asignar A
- **WHEN** el usuario elige «A» y pulsa la tecla Space
- **THEN** Space pulsa A y se guarda en la configuración

#### Scenario: Conflicto
- **WHEN** el usuario asigna una tecla ya usada
- **THEN** se avisa y se ofrece reemplazar o cancelar

### Requirement: Audio
La configuración SHALL incluir volumen maestro (0 a 100) y dispositivo de salida, aplicables al instante.

#### Scenario: Volumen
- **WHEN** el volumen maestro se pone a 50
- **THEN** la salida de audio tiene la mitad de amplitud que a 100
