## Context

`discover` ya añade aristas `Dispatch` desde el sitio de la llamada a cada destino de la tabla en línea, y marca el sitio como sin caída. El helper (`00:8781`) extrae la dirección de retorno con `PLY`/`PLA`, lee el puntero indexado por A y termina con `JML [$0000]` (`exit`, `00:8799`). `helper_exits` solo recuerda qué direcciones no hay que informar como sin resolver. En el C, el bloque del sitio llama a `f_helper(c)` y, al no tener sucesor de caída, retorna; el `exit` cae en el `default` de un `switch` sin casos y llama a `rt_unresolved`.

## Goals / Non-Goals

**Goals:**
- El `exit` de cada helper despacha a los destinos reales de sus llamadas.
- El C de una llamada a un helper ejecuta el destino y deja la pila igual que la CPU.
- Verificado sobre el ROM real, sin conflictos de anchos nuevos.

**Non-Goals:**
- Precisión por sitio de llamada (qué destinos corresponden a qué llamada).
- Retornos manipulados, llamadas sin retorno, ROM completa, runtime.

## Decisions

1. **Unión de destinos en el `exit`.** El `exit` no sabe quién lo llamó, así que despacha sobre la unión de las tablas de todas las llamadas al helper. Alternativa: clonar el helper por sitio de llamada; se descarta por multiplicar código y cambiar el grafo. Alternativa: leer el destino con una pila de llamadas en ejecución; se descarta por añadir estado al runtime. El `switch` es más grande, pero el destino real siempre está dentro; si no, `rt_unresolved` lo detecta.
2. **`exit` ligado al helper.** `InlineHelper` pasa a tener `exit: Option<u32>`; `helper_exits` se deriva de esos valores para no tocar a los consumidores actuales. Hace falta para saber qué unión va a qué `exit`.
3. **Aristas añadidas tras el recorrido.** Durante el descenso se acumula `helper_targets: BTreeMap<u32, BTreeSet<u32>>` (clave: dirección del helper). Al terminar el descubrimiento se añaden las aristas `Dispatch` desde cada `exit` alcanzado. No se vuelve a encolar trabajo: los destinos ya se descubrieron desde el sitio de la llamada.
4. **Anchos de los destinos.** Los destinos se ejecutan tras `SEP #$30`, pero hoy se encolan con los anchos del sitio de la llamada. Es una discrepancia latente: `rt_check_flags` fallaría en ejecución si el llamador estaba en 16 bits. Se comprueba sobre el ROM: si algún destino se alcanza con anchos distintos de los del `exit`, se encolan con los anchos del `exit`. Para ello se aplaza la encolada de los destinos de un helper con `exit` hasta conocer los anchos en ese punto; si no hay `exit` declarado se mantiene el comportamiento actual.
5. **Emisión sin cambios de esquema.** El grafo trata una arista `Dispatch` sin caída como salto a una entrada; el emisor de `IndirectJump` ya genera `case ...: f_xx(c); return;`. Tras la llamada al destino, el helper retorna y el sitio de la llamada retorna (no tiene sucesor de caída). La pila es correcta porque el helper ya sacó la dirección de retorno del llamador y el `RTS`/`RTL` del destino saca la del llamador anterior.

## Risks / Trade-offs

- [Unión muy grande de destinos] → Aceptado; es un `switch` sobre enteros. Se mide el número de casos sobre ALttP.
- [Cambiar los anchos de los destinos altera el número de conflictos] → Se compara `discover --widths` antes y después; el resultado no debe empeorar y se anota.
- [Un destino cae en una función que comparte bloques con otras] → Ya cubierto por la decisión 10 de `c-codegen` (los bloques compartidos se emiten en cada función).
- [Tabla de destinos mal delimitada] → Sigue vigente la verificación manual de `inline_helper_targets`; este cambio no la modifica.
