# Spec Delta

## Purpose

Permitir que el runtime (bus, DMA, PPU, SPC700/DSP y frontal) se compile y distribuya como una pieza propia, sin nada derivado de la ROM, y que el código del juego llegue después como otro módulo (compilado en el dispositivo o cargado como biblioteca).

## ADDED Requirements

### Requirement: Runtime compilable sin el código generado
El runtime SHALL poder compilarse y enlazarse como biblioteca sin ningún fichero C generado a partir de la ROM. Lo que se construya en ese modo MUST NOT contener bytes de la ROM ni símbolos de funciones del juego.

#### Scenario: Biblioteca sin juego
- **WHEN** se compila el runtime en modo biblioteca sin los `rom_*.c` ni `snes_entry.c`
- **THEN** la compilación termina sin símbolos sin resolver que pertenezcan al código generado

#### Scenario: Sin bytes de la ROM
- **WHEN** se pasa `scan` con la ROM real sobre la biblioteca construida
- **THEN** no se encuentra ningún fragmento de la ROM

### Requirement: El código del juego se une en tiempo de ejecución
El código generado SHALL depender del runtime solo a través de las funciones declaradas en `snes_rt.h`. El runtime SHALL obtener el despacho por dirección de un punto de entrada que el módulo del juego registra antes de ejecutar el vector de reset. Si se ejecuta sin un módulo registrado, el runtime MUST terminar con un error que lo indique, sin ejecutar código del juego.

#### Scenario: Módulo registrado
- **WHEN** un módulo con el C generado se carga, registra su punto de entrada y se arranca el juego
- **THEN** el runtime ejecuta las funciones del juego a través de ese punto de entrada

#### Scenario: Sin módulo
- **WHEN** se arranca el runtime en modo biblioteca sin que ningún módulo haya registrado el punto de entrada
- **THEN** termina con un error que nombra el registro que falta y no ejecuta el reset

### Requirement: El modo biblioteca no cambia el comportamiento
El juego ejecutado con el runtime en modo biblioteca y el módulo del juego unido en tiempo de ejecución SHALL comportarse igual que el ejecutable de un solo enlace. Con la misma ROM, las mismas entradas y el mismo número de fotogramas, MUST producir el mismo estado de WRAM, VRAM, CGRAM y OAM.

#### Scenario: Mismo resultado
- **WHEN** se ejecutan 1200 fotogramas sin vídeo con `--press START@1100-1110` en las dos formas de construcción
- **THEN** los volcados `--dump` de ambas son idénticos byte a byte

#### Scenario: Construcción de PC sin cambios
- **WHEN** `snes-port` o `build_play.bat` construyen el juego como hasta ahora
- **THEN** el ejecutable se enlaza en una sola pieza y todos los tests existentes pasan
