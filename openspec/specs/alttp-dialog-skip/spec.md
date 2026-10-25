# alttp-dialog-skip Specification

## Purpose

Mejora propia de A Link to the Past, escrita solo con ganchos y C del juego, que acelera la aparición del texto de los cuadros de mensaje mientras se mantiene pulsado un botón; demuestra de punta a punta el mecanismo de ganchos.

## Requirements

### Requirement: Diálogos acelerados con un botón
El perfil de ALttP SHALL incluir, solo mediante ganchos y C propio, una mejora que acelere la aparición del texto de los diálogos mientras el jugador mantiene pulsado R (no se ha observado que el juego lo use durante los textos), y MUST dejar el comportamiento original cuando el botón no está pulsado.

#### Scenario: Botón pulsado durante un diálogo
- **WHEN** aparece un diálogo y se mantiene pulsado el botón durante él
- **THEN** el texto avanza un carácter por fotograma en vez de uno cada tres, y a igual número de fotogramas se ven más caracteres que sin pulsarlo

#### Scenario: Sin pulsar
- **WHEN** aparece el mismo diálogo sin pulsar el botón
- **THEN** el texto avanza al ritmo original del juego

#### Scenario: Fuera de los diálogos
- **WHEN** se mantiene pulsado el botón fuera de un diálogo
- **THEN** el juego se comporta como sin la mejora

### Requirement: Mejora desactivable
El juego SHALL poder ejecutarse sin la mejora, de modo que quede disponible el comportamiento original de ALttP.

#### Scenario: Sin la mejora
- **WHEN** se ejecuta con `--game-opt dialog_skip=0`
- **THEN** pulsar el botón durante un diálogo no cambia su velocidad
