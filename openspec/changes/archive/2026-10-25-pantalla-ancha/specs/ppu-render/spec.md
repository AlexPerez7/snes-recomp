# Spec Delta

## MODIFIED Requirements

### Requirement: Captura de la imagen final
El runtime SHALL escribir con `--screenshot <fichero>`, al terminar la ejecución y también cuando se detiene por un fallo, un PNG de `256 + 2E` × 224 píxeles (256×224 cuando no se pide ensanche) con el último fotograma completo que dibujó la PPU (o, si no llegó a completarse ninguno, el estado de ese momento), y MUST no escribir nada si no se pide. El fichero SHALL ser un PNG válido de 8 bits por canal RGB que cualquier lector de imágenes pueda abrir.

#### Scenario: Captura pedida
- **WHEN** se ejecuta con `--screenshot s.png`
- **THEN** `s.png` existe, tiene la firma PNG y las dimensiones 256×224

#### Scenario: Captura con ensanche
- **WHEN** se ejecuta con `--screenshot s.png --wide 32`
- **THEN** `s.png` tiene las dimensiones 320×224

#### Scenario: Captura tras un fallo
- **WHEN** la ejecución se detiene por `rt_unresolved` con `--screenshot s.png`
- **THEN** `s.png` se escribe igualmente

#### Scenario: Sin opción
- **WHEN** no se pasa `--screenshot`
- **THEN** no se escribe ninguna imagen
