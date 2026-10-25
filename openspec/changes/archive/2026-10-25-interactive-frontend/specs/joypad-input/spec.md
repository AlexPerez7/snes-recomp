# Spec Delta

## ADDED Requirements

### Requirement: Fuente de botones en vivo
El runtime SHALL admitir que el estado de los botones de los joypads 1 y 2 lo dé una fuente en vivo, consultada al comienzo de cada fotograma. Con fuente en vivo, `--press` MUST seguir siendo válido y sus botones se suman a los de la fuente. Sin fuente en vivo, el comportamiento MUST ser el anterior.

#### Scenario: Fuente y --press juntas
- **WHEN** la fuente mantiene RIGHT y `--press A@10-12` está activa
- **THEN** en el fotograma 11 el juego lee RIGHT y A pulsados

#### Scenario: Sin fuente en vivo
- **WHEN** se ejecuta el runtime sin vídeo con `--press START@3-4`
- **THEN** el comportamiento es el de antes de este cambio
