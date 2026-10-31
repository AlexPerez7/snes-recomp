# Spec Delta

## ADDED Requirements

### Requirement: Tiempo por fotograma
Con `--frame-times FICHERO`, el runtime sin vídeo SHALL escribir una línea por fotograma con los microsegundos que tardó (CPU, PPU y sonido juntos), medidos con el reloj de C11. Si no puede crear el fichero, MUST terminar con el código 2 antes de ejecutar nada.

#### Scenario: Una línea por fotograma
- **WHEN** se ejecuta con `--frames 3 --frame-times ft.txt`
- **THEN** `ft.txt` tiene tres líneas, cada una con un número entero de microsegundos

#### Scenario: Fichero imposible
- **WHEN** `--frame-times` nombra un fichero en una carpeta que no existe
- **THEN** el runtime termina con código 2 y un mensaje que nombra el fichero
