# android-app Specification

## Purpose

Ejecutar el frontal en Android con la ROM que elige el usuario, sin que la ROM salga del teléfono ni vaya en el APK.

## Requirements

### Requirement: Arranque con la ROM del usuario
La app SHALL pedir la ROM con el selector de documentos de Android la primera vez, comprobar su CRC32 contra los perfiles que lleva y, si coincide, generar el C del juego, compilarlo en el dispositivo y arrancar el juego sin más pasos del usuario. El APK SHALL NOT contener la ROM ni nada derivado de ella.

#### Scenario: Primera vez con una ROM correcta
- **WHEN** el usuario abre la app por primera vez y elige una ROM con CRC32 conocido
- **THEN** la app prepara el juego y lo ejecuta sin más preguntas

#### Scenario: ROM desconocida
- **WHEN** el usuario elige un fichero cuyo CRC32 no coincide con ningún perfil
- **THEN** la app lo rechaza con un mensaje que nombra el CRC32 recibido y los esperados

#### Scenario: ROM recordada
- **WHEN** el usuario vuelve a abrir la app
- **THEN** el juego arranca directamente con la ROM elegida antes, sin volver a pedirla

### Requirement: Ciclo de vida
La app SHALL poder pasar a segundo plano y volver sin perder la partida, y SHALL guardar la SRAM al pausarse.

#### Scenario: Segundo plano
- **WHEN** el usuario pasa la app a segundo plano y vuelve más tarde
- **THEN** la partida continúa donde estaba y la SRAM guardada es la del último momento de juego
