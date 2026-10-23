# Proposal

## Why

Sin mando, ALttP recorre en bucle su modo atracción (intro, título, historia, demo del castillo) y nunca llega a la selección de partida: el juego espera un `START` que el runtime sin vídeo no puede dar, porque `$4218-$421B` y `$4016` solo devuelven lo último que se escribió (cero). Para seguir comprobando que la ROM recompilada funciona más allá del modo atracción hace falta poder pulsar botones de forma reproducible.

## What Changes

- El runtime simula el mando 1: lectura automática (`$4218-$4219`, activada por el bit 0 de `$4200`, con el bit 0 de `$4212` a 1 unas líneas al empezar el vblank) y lectura serie manual por `$4016` con el latch de su bit 0.
- Nueva opción `--press BOTONES@A-B` (repetible): mantiene pulsados los botones durante los fotogramas A a B, ambos incluidos. Los botones son `B Y SELECT START UP DOWN LEFT RIGHT A X L R`, unidos con `+`. Sin la opción, el mando no pulsa nada.
- Una opción inválida termina con error de uso, como las de traza.
- El mando 2 y los accesorios (`$4017`, `$4219` de otros dispositivos) quedan fuera: devuelven 0.

## Capabilities

### New Capabilities
- `joypad-input`: botones pulsados por fotograma, lectura automática y lectura serie del mando 1, y la opción `--press`.

### Modified Capabilities

## Impact

- `runtime/headless.c`: lectura de `$4016`, `$4212`, `$4218-$421B`, actualización al empezar el vblank, análisis de `--press`.
- `src/headless.rs`: tests con programas pequeños.
- `CLAUDE.md`: la opción `--press`.
- ALttP: una captura tras pulsar `START` en la pantalla de título muestra la selección de partida.
