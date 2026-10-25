# Spec Delta

## Purpose

Configuración del frontal interactivo y su menú en pantalla.

## MODIFIED Requirements

### Requirement: Menú en pantalla
F1 SHALL abrir un menú que se maneje con el teclado y el mando, pause la emulación y permita cambiar las opciones de imagen, controles y audio, aplicándolas al instante y guardándolas al salir del menú. El menú SHALL incluir una pestaña Juego con las acciones continuar, guardar la partida ahora, restaurar valores por defecto y salir, y las pestañas de imagen y audio SHALL permitir restaurar sus valores por defecto.

#### Scenario: Cambiar el filtro
- **WHEN** el usuario cambia el filtro a `bilinear` en el menú y cierra el menú
- **THEN** la imagen usa el filtro nuevo y `config.ini` contiene `filter = bilinear`

#### Scenario: Guardar ahora
- **WHEN** el usuario elige «Guardar ahora» en la pestaña Juego con la SRAM modificada
- **THEN** `<rom>.srm` contiene la SRAM actual sin esperar al intervalo de guardado

#### Scenario: Restaurar valores por defecto
- **WHEN** el usuario cambia el escalado y elige «Restaurar» en la pestaña Imagen
- **THEN** las opciones de imagen vuelven a sus valores por defecto y las demás pestañas no cambian

## ADDED Requirements

### Requirement: Contador de fotogramas
La configuración SHALL incluir la opción `fps` (`on`/`off`, por defecto `off`) que muestra en una esquina de la imagen los fotogramas por segundo, y MUST poder cambiarse desde el menú.

#### Scenario: Mostrar el contador
- **WHEN** `fps = on` en `config.ini`
- **THEN** la imagen muestra el contador sobre el juego y, con `off`, la imagen es idéntica al búfer de la consola
