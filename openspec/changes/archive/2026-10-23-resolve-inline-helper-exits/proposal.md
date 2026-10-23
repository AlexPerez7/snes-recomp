## Why

Los helpers de despacho en línea de ALttP (`00:8781` y `00:879C`) terminan en un `JML [$0000]` que salta al destino elegido por la tabla que sigue a cada llamada. El descubrimiento ya conoce esos destinos (aristas `Dispatch` desde el sitio de la llamada), pero el salto interno del helper no tiene destinos: el C generado ejecuta el helper, cae en `rt_unresolved` y, si no, el sitio de la llamada retorna sin haber ejecutado el destino. Es el bloqueo principal para que el C generado funcione más allá de la rebanada de `reset` (decisión 11 de `c-codegen`).

## What Changes

- `[[inline_helper]]` recibe el campo `exit` asociado a su helper concreto (hoy `exit` solo se guarda como un conjunto de direcciones sin saber a qué helper pertenece).
- El descubrimiento acumula, por helper, la unión de los destinos de todas sus llamadas y añade aristas `Dispatch` desde la instrucción `exit` hacia esa unión. El `exit` deja de ser un salto sin destinos.
- El generador emite en el `exit` un `switch` sobre esos destinos (`f_xx(c); return;` en cada caso) y `rt_unresolved` en `default`. Al volver el destino, el helper retorna y el sitio de la llamada retorna también, con lo que la pila queda como en la CPU real (el helper ya extrajo la dirección de retorno).
- Los destinos de la tabla se descubren con los anchos M/X vigentes en el `exit` (tras `SEP #$30`), no con los del sitio de la llamada, que es lo que ven en ejecución.
- Verificación contra el ROM real: ningún `exit` queda sin destinos y `discover --widths` no gana conflictos por este cambio.

## Capabilities

### New Capabilities

Ninguna.

### Modified Capabilities

- `indirect-jump-resolution`: el `exit` de un helper en línea pasa de «no se informa» a tener como destinos la unión de los de sus llamadas.
- `c-code-generation`: el despacho del `exit` de un helper en línea sigue el mismo esquema de `switch` que el resto de saltos indirectos.

## Impact

- `src/config.rs`: `InlineHelper` guarda su `exit`; `helper_exits` se deriva de ahí.
- `src/discover.rs`: acumulación de destinos por helper, aristas desde el `exit`, anchos de los destinos.
- `src/graph.rs`: sin cambios previstos (las aristas `Dispatch` sin caída ya se tratan como saltos).
- `src/codegen.rs`: sin cambios previstos en el emisor de `IndirectJump` si el bloque del `exit` ya recibe sus destinos del grafo; test nuevo que ejecuta el C.
- `games/alttp_usa.toml`: sin cambios si `exit` ya está declarado en ambos helpers (lo está).
- Fuera de alcance: retornos manipulados y las 3 llamadas sin retorno, emisión de la ROM completa, especializar por anchos, runtime real.
