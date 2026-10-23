## Context

`call_lines` empuja la dirección de retorno en la pila emulada y llama a `f_BB_AAAA(c)`; el retorno es solo el `return` de C. `RTS`/`RTL` sacan la dirección y la descartan. Mientras el código respete la disciplina de la pila, pila emulada y pila nativa coinciden. El código que la rompe a propósito (descartar un retorno con `PLA`/`PLA`) deja las dos desalineadas. Observado en ALttP (módulo `$14`, fotograma 6889): `06:E41E` descarta el retorno de su `JSR` y su `RTS` en `06:E475` vuelve al llamador de `06:E41A`; el C ejecuta además el `RTS` en `06:E41D`, y el `S` queda 2 bytes más alto por cada pasada.

## Goals / Non-Goals

**Goals:**
- Que el flujo de control del C siga el de la CPU cuando un `RTS`/`RTL` vuelve a un llamador más lejano que el nativo.
- Fallar con un mensaje claro, no seguir con la pila desequilibrada, si el retorno no corresponde a ninguna llamada activa.
- Que ALttP pase el fotograma 6889.

**Non-Goals:**
- Retornos a direcciones que no son un sitio de llamada activo (`PEA`/`PHA` seguido de `RTS` como salto calculado). Se detectan y se paran; resolverlos es otro cambio.
- `RTI` y los contextos de ejecución.
- Cambiar el modo de las llamadas inline helpers, que ya se resuelven por configuración.

## Decisions

1. **Comprobación en el sitio de la llamada, no una pila sombra.** Una pila sombra de marcos nativos costaría un empuje y un desempuje por llamada y habría que mantenerla coherente con los cambios de contexto. En cambio el sitio de la llamada ya conoce lo que espera: el `S` anterior al empuje (lo guarda en una variable local antes de empujar) y la dirección de retorno que empujó (constante). Tras la llamada compara `c->s == s0 && c->ret == RET`.
2. **`RTS`/`RTL` dejan la dirección en `c->ret`** (16 bits, la sacada de la pila). El bit de banco de `RTL` no se compara: en un `JSL` el banco de retorno es el del sitio y sería redundante con `S` y `ret`.
3. **El retorno se propaga con `return` de C.** El sitio que no coincide ejecuta `return;` sin tocar `S` ni `ret`. Cada nivel nativo hace lo mismo hasta el que coincide. Los tail calls ya hacen `f(c); return;` y propagan sin cambios.
4. **`S` y la dirección juntos.** Solo `S` identificaría el marco, pero dos retornos al mismo `S` con distinta dirección (`PLA`/`PLA`/`PEA`/`RTS`) deben detectarse; con ambos, un retorno que no coincide con ninguna llamada acaba en el runtime en vez de continuar en un sitio equivocado.
5. **El runtime detecta el retorno perdido.** Cuando una función de nivel superior (el vector de reset, un manejador de interrupción, el cuerpo de un contexto) vuelve por un `RTS`/`RTL`, el runtime llama a `rt_lost_return(c)` y se detiene. Se distingue de un retorno normal porque `RTS`/`RTL` de nivel superior no tienen llamador que lo acepte: los puntos de entrada del runtime marcan `c->ret = RT_NO_RET` antes de llamar y comprueban al volver que no ha cambiado a un valor de retorno.
6. **Coste aceptado.** Un almacenamiento por `RTS`/`RTL` y una comparación por llamada. Si el compilador de C inlinea poco, la bajada es mínima porque la comparación casi nunca falla.

## Risks / Trade-offs

- [`RTS`/`RTL` dentro de un helper en línea que sale por `exit`] → ya tienen su manejo estático; se verifica con los tests existentes de helpers.
- [El runtime compara `c->ret` con un centinela y un `RTS` real puede sacar justo ese valor] → el centinela es `0xFFFFFFFF` en un campo de 32 bits, fuera del rango de 16 bits de una dirección.
- [Una llamada con retorno no local desde dentro de un contexto distinto al que la hizo] → no se contempla: los contextos solo cambian en `RTI`, y un `RTS` no los cruza.
- [Más código C por llamada] → unas líneas; el generado completo de ALttP crece unos cientos de KiB.

## Open Questions

- Qué otros retornos no locales hay en ALttP además del de `06:E491`: lo irá diciendo la ejecución más allá del fotograma 6889.
