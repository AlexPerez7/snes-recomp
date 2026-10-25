# Spec Delta

## MODIFIED Requirements

### Requirement: Captura de la imagen final
El runtime SHALL escribir con `--screenshot <fichero>`, al terminar la ejecución y también cuando se detiene por un fallo, un PNG de 256×224 píxeles con el último fotograma completo que dibujó la PPU (o, si no llegó a completarse ninguno, el estado de ese momento), y MUST no escribir nada si no se pide. El fichero SHALL ser un PNG válido de 8 bits por canal RGB que cualquier lector de imágenes pueda abrir.

#### Scenario: Captura pedida
- **WHEN** se ejecuta con `--screenshot s.png`
- **THEN** `s.png` existe, tiene la firma PNG y las dimensiones 256×224

#### Scenario: Captura tras un fallo
- **WHEN** la ejecución se detiene por `rt_unresolved` con `--screenshot s.png`
- **THEN** `s.png` se escribe igualmente

#### Scenario: Sin opción
- **WHEN** no se pasa `--screenshot`
- **THEN** no se escribe ninguna imagen

### Requirement: Límites declarados
El runtime SHALL avisar en el informe cuando la imagen use una función que no dibuja, que tras este cambio se reducen al entrelazado y a las resoluciones de alta resolución de `$2133`. MUST dibujar en esos casos lo que sí sabe dibujar y no inventar el resto. La captura SHALL usar el mismo dibujador por línea que el ejecutable interactivo y mostrar el último fotograma completo; si la ejecución se detiene antes de acabar el primero, SHALL dibujar las líneas que faltan con el estado de ese momento.

#### Scenario: Otro modo gráfico
- **WHEN** `$2105` selecciona el modo 0, 2, 3, 4, 5, 6 o 7
- **THEN** la captura dibuja las capas de ese modo y el informe no avisa de modo no dibujado

#### Scenario: Color math
- **WHEN** `$2131` tiene alguna capa activada y los bits 4-5 de `$2130` valen 0 o 3
- **THEN** el color math se aplica y el informe no avisa de que no se aplique

#### Scenario: Color math con ventana
- **WHEN** los bits 4-5 de `$2130` piden una región que depende de la ventana de color
- **THEN** la región se aplica y el informe no avisa

#### Scenario: Entrelazado
- **WHEN** `$2133` activa el entrelazado de imagen
- **THEN** el informe avisa de que el entrelazado no se aplica
