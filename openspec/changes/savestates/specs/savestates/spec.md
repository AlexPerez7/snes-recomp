## ADDED Requirements

### Requirement: Punto de reanudación del perfil
Un perfil SHALL poder declarar `[resume]` con la dirección de un bloque del bucle principal del juego, con la pila del 65816 en su base. Solo los juegos con ese punto MUST ofrecer save states y rebobinado.

#### Scenario: Perfil sin punto de reanudación
- **WHEN** el perfil no tiene `[resume]`
- **THEN** guardar, cargar y rebobinar no están disponibles y el menú lo indica

#### Scenario: Dirección que no es inicio de bloque
- **WHEN** la dirección de `[resume]` no es el inicio de un bloque del grafo
- **THEN** la construcción falla con un error que nombra la dirección

### Requirement: Guardar solo en el punto de reanudación
El runtime SHALL atender una petición de guardado la siguiente vez que la ejecución pase por el punto de reanudación, sin interrupción en curso. Si no pasa en un número razonable de fotogramas, MUST avisar y descartar la petición.

#### Scenario: Petición normal
- **WHEN** el jugador pide guardar durante el juego
- **THEN** el estado se escribe en la ranura en cuanto el juego pasa por el punto

### Requirement: Estado completo y cargar reanuda
Un estado guardado SHALL incluir CPU, WRAM, SRAM, VRAM, CGRAM, OAM, registros de la PPU, DMA/HDMA, temporizadores, entrada, SPC700 y DSP. Cargarlo MUST restaurar todo en el punto de reanudación y seguir desde ahí; si `S`, el contexto activo o una interrupción en curso no coinciden con los de ese punto, MUST rechazar el estado.

#### Scenario: Determinismo
- **WHEN** se guarda, se avanzan N fotogramas, se carga y se vuelven a avanzar N
- **THEN** la imagen y el audio son idénticos en las dos pasadas

### Requirement: Estados ajenos rechazados
Un estado MUST llevar versión del formato, versión del motor y CRC32 de la ROM, y el runtime SHALL rechazar sin modificar el juego el que no coincida.

#### Scenario: Estado de otra ROM
- **WHEN** se carga un estado cuyo CRC32 no es el de la ROM en uso
- **THEN** se muestra un aviso y el juego sigue como estaba

### Requirement: Rebobinado
El frontal SHALL mantener un anillo de estados recientes y retroceder por él mientras se mantenga una tecla, sin sonido durante el retroceso.

#### Scenario: Retroceder
- **WHEN** el jugador mantiene la tecla de rebobinado
- **THEN** la imagen vuelve a estados anteriores y, al soltarla, el juego continúa desde el último alcanzado

### Requirement: Los estados no se publican
Los ficheros de estado (`.sst`) derivan de la ROM y MUST estar en `.gitignore`, ser detectados por `scan` y rechazados por `release.ps1`.

#### Scenario: Paquete con un estado
- **WHEN** `release.ps1` encuentra un `.sst`
- **THEN** falla y no crea el zip
