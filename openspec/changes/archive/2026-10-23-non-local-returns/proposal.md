# Proposal

## Why

El C generado emite `RTS`/`RTL` como `(void)rt_pull16(c); return;`: vuelve siempre un solo nivel nativo, sea cual sea la dirección sacada de la pila. El 65816 permite descartar una dirección de retorno (`PLA`/`PLA` o `PLX`/`PLX` antes del `RTS`) para volver al llamador del llamador, y ALttP lo usa: en `06:E491` (`PLA; PLA; SEC; BRA $E45F … RTS`), dentro de `JSR $E41E` llamado desde `JSR $E41A` llamado desde `JSL $E416`. En el C, tras ese `RTS` se ejecuta además el `RTS` que el hardware se saltó, `S` queda dos bytes más alto cada vez y la ejecución diverge: en el fotograma 6889 el despachador de módulos llega con `DB`=01 y `S`=`0200` y `JML [$0003]` salta a `0A0908`, un destino sin resolver. Es la causa de que ALttP deje de avanzar tras la demostración de la intro.

## What Changes

- Cada `RTS` y `RTL` deja la dirección que ha sacado de la pila en `Cpu` (campo nuevo) además de devolver.
- Tras cada llamada (`JSR`, `JSL`, directa o indirecta), el sitio de la llamada compara el `S` y la dirección con los que esperaba: si coinciden, continúa en la instrucción siguiente como hasta ahora; si no, retorna a su vez sin ejecutar nada más, de modo que el retorno se propaga hasta la llamada cuyo `S` y dirección coinciden.
- Si el retorno sube hasta el código del runtime sin que ninguna llamada lo acepte, queda registrado en `Cpu` y el runtime sin vídeo lo detecta y detiene la ejecución con la causa y la dirección, en lugar de seguir con la pila desequilibrada.
- Los tail calls (`JMP`/`JML` a otra función) y las salidas de helpers en línea no cambian: ya retornan sin ejecutar nada más y el retorno se propaga a través de ellos.
- `RTI` queda fuera: sus cambios de contexto ya los resuelve `rt_rti`.

## Capabilities

### New Capabilities

### Modified Capabilities
- `c-code-generation`: «Control de flujo» (los retornos y las llamadas comprueban la dirección y la pila) y un requisito nuevo para los retornos no locales.
- `headless-runtime`: «Arranque y parada» añade la parada por un retorno que ninguna llamada acepta.

## Impact

- `src/codegen.rs`: emisión de `JSR`/`JSL`, `RTS`/`RTL`; tests que compilan y ejecutan C.
- `runtime/snes_rt.h`: campo nuevo de `Cpu` y gancho `rt_lost_return`; los runtimes de prueba de `codegen` y `runtime/headless.c` lo implementan.
- Coste: un almacenamiento por retorno y una comparación por llamada; el C regenerado de ALttP crece algo.
- `CLAUDE.md`: mención del mecanismo.
- Sin cambios en `games/*.toml`.
