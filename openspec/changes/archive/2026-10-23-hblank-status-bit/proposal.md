# Proposal

## Why

Tras el fotograma ~1054 ALttP se quedaba en forced blank y las capturas salían negras. La causa era el runtime sin vídeo: su manejador de IRQ espera con `BIT $4212 / BVC` a que se active el bit 6 (H-blank), que `$4212` nunca devolvía, así que el bucle no terminaba y el juego no avanzaba. Con el bit modelado, la intro sale del forced blank y muestra sus pantallas de historia.

## What Changes

- La lectura de `$4212` devuelve también el bit 6 (H-blank): vale 1 en el último tramo de cada una de las 262 líneas en que se divide el fotograma, y 0 al empezar el fotograma.
- El bit 7 (vblank) y el resto de bits no cambian.

## Capabilities

### New Capabilities

### Modified Capabilities
- `headless-runtime`: el requisito «Registros de hardware simulados» añade el estado de H-blank.

## Impact

- `runtime/headless.c`: lectura de `$4212`.
- `src/headless.rs`: test del bit 6 y el de vblank deja de depender de él.
- Sin cambios en el generador de C.
