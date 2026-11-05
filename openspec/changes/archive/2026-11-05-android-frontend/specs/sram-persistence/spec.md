# Spec Delta

## ADDED Requirements

### Requirement: Partidas en Android
En Android las partidas (`.srm`) y los estados SHALL guardarse en la carpeta de datos propia de la app, con el mismo formato que en escritorio.

#### Scenario: Partida portable
- **WHEN** se copia el `.srm` de la app a un PC
- **THEN** el juego de escritorio lo abre y continúa la misma partida
