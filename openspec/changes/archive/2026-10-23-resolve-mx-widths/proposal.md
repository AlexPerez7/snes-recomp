# Proposal

## Why

El descubrimiento supone que una llamada no cambia los anchos M/X y que `PLP` los deja como estaban. Es falso a menudo en A Link to the Past: hay 254 sitios con `REP`/`SEP` justo antes de `RTS`/`RTL`, y 51 `PLP` que restauran anchos guardados por un `PHP`. El resultado son 538 direcciones alcanzadas con anchos distintos, de las cuales unas 61 cambian la longitud de la instrucción (inmediatos como `LDA #$80` frente a `LDA #$8D80`): el descubrimiento sigue las dos decodificaciones y la segunda es código fantasma. Hoy 126 de las 1.326 funciones alcanzadas por llamadas directas no se pueden traducir a C por esto, y parte del grafo (instrucciones, bloques, funciones) es inventada.

## What Changes

- El descubrimiento analiza cada función por contexto `(entrada, anchos de entrada)` y registra con qué anchos vuelve (`RTS`/`RTL`). Tras una llamada, el llamador continúa con los anchos con los que vuelve el llamado, no con los que tenía antes de llamar.
- Si un llamado no llega a retornar en el análisis, el llamador continúa con los anchos previos a la llamada (como hoy) y la llamada se informa como diagnóstico.
- El estado de cada camino lleva una pila abstracta de anchos para emparejar `PHP` con `PLP`, tolerando otras instrucciones de pila entre ambas. Un `PLP` que no se puede emparejar se informa y deja los anchos como estaban; `[[assume]]` sigue pudiendo corregirlo.
- `discover --widths` lista las direcciones en conflicto con los anchos vistos, los `PLP` sin emparejar y las llamadas sin retorno.
- Cambian las cifras de ALttP (instrucciones, bloques, funciones, conflictos) al desaparecer los caminos fantasma; se registran en el README y los cambios se contrastan a mano.
- Sin cambios incompatibles en la API: `Discovery` solo gana campos.

## Capabilities

### New Capabilities
- `register-width-tracking`: cómo el descubrimiento sigue los anchos M/X a través de llamadas, retornos y `PHP`/`PLP`, y cómo informa de lo que no puede resolver.

### Modified Capabilities

Ninguna: los requisitos de `indirect-jump-resolution`, `function-block-graph` y `c-code-generation` no cambian. Lo que cambia es que el generador encuentra menos conflictos.

## Impact

- `src/discover.rs`: estado de camino con contexto y pila de `PHP`/`PLP`, resúmenes de retorno y continuaciones pendientes, nuevos campos de `Discovery`.
- `src/main.rs`: opción `--widths` de `discover`.
- Tests de descubrimiento, y de grafo y generador donde dependan de las cifras de ALttP.
- README y CLAUDE.md: cifras nuevas y la opción.
- Fuera de alcance: especializar el código por anchos (emitir una variante por combinación), seguir `XCE` y `RTI`, y modelar manipulaciones directas del puntero de pila (`TCS`/`TXS`).
