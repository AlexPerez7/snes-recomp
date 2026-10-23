# Proposal

## Why

Tras `resolve-mx-widths` quedan 232 direcciones de ALttP alcanzadas con anchos M/X distintos, y el generador de C rechaza cualquier función que contenga una (95 de las 1.326 funciones llamadas directamente). Clasificadas, 225 de esas direcciones son inofensivas: la instrucción no depende del ancho que difiere (110 `SEP`, 40 `REP`, `RTS`, `NOP`, ramas, o un `STA` cuando solo difiere X) y el C que se emitiría es idéntico con cualquiera de los anchos. Solo 7 direcciones, en 5 funciones, cambian de verdad el C o la longitud de la instrucción. El generador es mucho más estricto de lo necesario.

## What Changes

- El generador de C solo rechaza una función por anchos en conflicto cuando, en alguna dirección en conflicto, el C emitido o la longitud de la instrucción serían distintos según los anchos vistos. Un conflicto en una instrucción cuyo C no depende de ellos (típicamente un `SEP`/`REP` que los normaliza) se admite.
- La comprobación de ejecución `rt_check_flags` al entrar en un bloque no vuelve a comparar los anchos que varían en esa dirección; sigue comprobando los demás.
- Se repite el barrido de las 1.326 entradas y se documenta el resultado.
- Se estudian a mano los bloqueos restantes (`01:CA17`, `01:D81B`, `07:CF12`, `07:E026`, `08:A80D`): cada uno se corrige con `[[assume]]` en `games/alttp_usa.toml` si el ancho correcto se puede comprobar contra el ROM, o se deja informado como polimorfismo real.
- Sin cambios en el descubrimiento, en el grafo ni en el formato de `games/*.toml`.

## Capabilities

### New Capabilities

Ninguna.

### Modified Capabilities

- `c-code-generation`: cambia el requisito «Anchos M/X por instrucción»: el conflicto de anchos deja de ser un error cuando no afecta al C emitido, y la comprobación en ejecución ignora los anchos que varían en el bloque.

## Impact

- `src/codegen.rs`: `check_widths` y la emisión de `rt_check_flags`; tests de generación.
- `runtime/snes_rt.h`: `rt_check_flags` admite «no comprobar» para un ancho.
- `games/alttp_usa.toml`: posibles `[[assume]]` nuevos.
- README y CLAUDE.md: cifras del barrido.
- Fuera de alcance: especializar el código por anchos (emitir una variante por combinación), que seguiría siendo la vía para el polimorfismo real.
