# Proposal

## Why

El descubrimiento de código devuelve hoy un mapa plano de instrucciones alcanzables (82.108 en A Link to the Past) sin estructura: no se sabe dónde empieza y termina cada rutina ni cómo se conectan sus bloques. El generador de C necesita funciones y bloques básicos como unidad de emisión, así que este es el paso previo obligatorio del roadmap.

## What Changes

- El descubrimiento registra además las aristas de control que ya calcula y hoy descarta: destinos de saltos y llamadas directos, destinos de tablas resueltas y de helpers en línea.
- Nuevo módulo que, a partir de ese resultado, parte el código en **bloques básicos** (líder, instrucciones, sucesores) y los agrupa en **funciones** (entrada, bloques, llamadas, retornos).
- Nuevo subcomando `graph` que lista funciones y bloques de una ROM, con un resumen de totales y diagnósticos.
- Diagnósticos explícitos: sucesores que no son una instrucción descubierta y bloques compartidos entre funciones.
- Sin cambios incompatibles: la salida de `discover` no cambia.

## Capabilities

### New Capabilities
- `function-block-graph`: partición del código descubierto en bloques básicos y funciones, con sus aristas, y su consulta desde la línea de comandos.

### Modified Capabilities

Ninguna: los requisitos de `indirect-jump-resolution` no cambian.

## Impact

- `src/discover.rs`: `Discovery` gana campos con las aristas (aditivo).
- `src/graph.rs` (nuevo) y `src/main.rs` (subcomando `graph`, uso en `USAGE`).
- `README.md` y `CLAUDE.md`: marcar el punto del roadmap y documentar el comando.
- Sin dependencias nuevas. Fuera de alcance: generación de C, análisis de registros/pila, detección de bucles y convenciones de llamada.
