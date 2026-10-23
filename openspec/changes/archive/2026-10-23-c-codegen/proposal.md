# Proposal

## Why

Ya existe el grafo de funciones y bloques básicos (2.488 funciones en A Link to the Past), pero nada lo traduce todavía a un programa ejecutable. El generador de C es el paso que convierte el análisis en código, y es el que permite validar de verdad el resto del recompilador: hasta que el C generado no se compile y se comporte como el 65816, los análisis previos solo se contrastan a ojo.

## What Changes

- Nuevo generador que traduce cada función del grafo a una función C: un bloque básico por etiqueta, `goto` para ramas y saltos, llamadas C para `JSR`/`JSL`, y `switch` sobre los destinos conocidos para los saltos y llamadas indirectos.
- Semántica de los 256 opcodes del 65816, con anchos de acumulador e índices según M/X conocidos en cada instrucción, y banderas N, V, Z y C actualizadas.
- Cabecera `snes_rt.h`, generada y embebida en el binario: estado de la CPU, funciones de bus (`bus_read8`/`bus_write8`…) y ayudas aritméticas. Es el único contacto del C con el resto del sistema, que se implementará después.
- El descubrimiento guarda los anchos M/X vigentes en cada instrucción (hoy se pierden) y señala las direcciones decodificadas con anchos distintos.
- Nuevo subcomando `codegen <rom> <addr> [--depth N] [-o <fichero>]` que emite la función en `<addr>` y sus llamadas hasta la profundidad pedida; las demás quedan solo declaradas.
- Alcance acordado: **rebanada vertical**. El emisor cubre todos los opcodes, pero se valida emitiendo unas pocas funciones reales (reset y sus llamadas), no la ROM completa.

## Capabilities

### New Capabilities
- `c-code-generation`: traducción de funciones y bloques básicos a código C que usa una interfaz de runtime fija, y su emisión desde la línea de comandos.

### Modified Capabilities

Ninguna: no cambian requisitos de `function-block-graph` ni de `indirect-jump-resolution`.

## Impact

- `src/discover.rs`: `Discovery` gana el mapa de anchos M/X por instrucción (aditivo).
- `src/codegen.rs` (nuevo), `runtime/snes_rt.h` (nuevo, embebido con `include_str!`) y `src/main.rs` (subcomando `codegen`).
- El C generado deriva del ROM: se escribe bajo `out/`, que ya está en `.gitignore`, y nunca se sube. No contiene datos del ROM más allá de los operandos de las instrucciones; toda lectura de memoria pasa por el bus.
- README y CLAUDE.md: roadmap y comandos.
- Fuera de alcance: runtime real (bus, DMA, PPU, APU, entrada), emisión de la ROM completa, optimizaciones (banderas perezosas, registros en variables locales), manejo de interrupciones y trucos con direcciones de retorno en la pila.
