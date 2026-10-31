## MODIFIED Requirements

### Requirement: Menú en pantalla
Escape o F1 SHALL abrir un menú que se maneje con el teclado y el mando, pause la emulación y permita cambiar las opciones de imagen, controles y audio, aplicándolas al instante y guardándolas al salir del menú. El menú SHALL incluir una pestaña Juego con las acciones continuar, guardar la partida ahora, restaurar valores por defecto y salir, y las pestañas de imagen y audio SHALL permitir restaurar sus valores por defecto. La acción de salir MUST pedir confirmación antes de terminar el programa: Intro (A en el mando) confirma y Escape (B en el mando) cancela dejando el menú abierto.

#### Scenario: Cambiar el filtro
- **WHEN** el usuario cambia el filtro a `bilinear` en el menú y cierra el menú
- **THEN** la imagen usa el filtro nuevo y `config.ini` contiene `filter = bilinear`

#### Scenario: Guardar ahora
- **WHEN** el usuario elige «Guardar ahora» en la pestaña Juego con la SRAM modificada
- **THEN** `<rom>.srm` contiene la SRAM actual sin esperar al intervalo de guardado

#### Scenario: Restaurar valores por defecto
- **WHEN** el usuario cambia el escalado y elige «Restaurar» en la pestaña Imagen
- **THEN** las opciones de imagen vuelven a sus valores por defecto y las demás pestañas no cambian

#### Scenario: Abrir con Escape o con F1
- **WHEN** se pulsa Escape (o F1) con el menú cerrado
- **THEN** el menú se abre en la pestaña y la fila en que se dejó

#### Scenario: Salir pide confirmación
- **WHEN** el usuario elige «Salir» en la pestaña Juego
- **THEN** el menú muestra un aviso y el programa sigue en marcha hasta que el usuario confirme

#### Scenario: Confirmar la salida
- **WHEN** el usuario elige «Salir» y pulsa Intro
- **THEN** el programa termina con código 0, guardando `config.ini` y la SRAM

#### Scenario: Cancelar la salida
- **WHEN** el usuario elige «Salir» y pulsa Escape
- **THEN** el aviso desaparece, el menú sigue abierto y el programa sigue en marcha
