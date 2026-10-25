# Spec Delta

## Purpose

Conservar las partidas guardadas del juego entre ejecuciones guardando la SRAM de la cartucho en un fichero junto a la ROM.

## ADDED Requirements

### Requirement: Carga al arrancar
El ejecutable interactivo SHALL leer `<rom>.srm` si existe y copiarlo a la SRAM antes de ejecutar el primer fotograma, y MUST ignorarlo con un aviso si su tamaño no coincide con el de la SRAM de la cabecera.

#### Scenario: Fichero existente
- **WHEN** existe `alttp_usa.srm` del tamaño correcto
- **THEN** el juego ve en la selección de partida los datos guardados

#### Scenario: Tamaño distinto
- **WHEN** el fichero tiene un tamaño que no es el de la SRAM
- **THEN** se ignora, se avisa y no se sobrescribe hasta que el juego guarde

### Requirement: Guardado
El ejecutable SHALL escribir el `.srm` cuando el juego modifica la SRAM (como mucho una vez por segundo) y al salir, y MUST escribirlo en un fichero temporal y renombrarlo para no dejarlo a medias.

#### Scenario: Guardar partida
- **WHEN** el juego escribe su partida en la SRAM y se cierra la ventana
- **THEN** el `.srm` contiene la partida y el siguiente arranque la ofrece

#### Scenario: SRAM sin cambios
- **WHEN** la SRAM no cambia durante la ejecución
- **THEN** el `.srm` no se reescribe
