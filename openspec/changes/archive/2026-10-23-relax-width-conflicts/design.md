## Context

Ver `proposal.md` - Why. `Emitter::check_widths` (`src/codegen.rs`) recorre las instrucciones de los bloques de una función y falla si alguna está en `Discovery.flag_conflicts`. Cada instrucción se emite con `instr_lines(ins, d.flags[addr], ..)`, donde `d.flags` guarda la primera combinación vista; `instr_lines` solo depende de los anchos a través de `(m8, x8)`. Cada bloque abre con `rt_check_flags(c, dir, m, x)`, que en compilaciones con `SNES_RT_CHECKS` compara con `c->m` y `c->xf`. `Discovery` solo guarda una instrucción decodificada por dirección (la de los primeros anchos).

## Goals / Non-Goals

**Goals:**
- Que un conflicto de anchos solo sea error cuando de verdad hay dos programas distintos que emitir.
- No perder la red de seguridad de las compilaciones con comprobaciones.
- Cero cambios de comportamiento para las funciones que hoy ya se generan.

**Non-Goals:**
- Especializar el código por combinación de anchos.
- Tocar el descubrimiento, el grafo o el informe `discover --widths`.

## Decisions

### D1. Sensibilidad definida por el C emitido, no por una lista de mnemónicos
Una dirección en conflicto es sensible si, para el conjunto de anchos visto, ocurre algo de esto: (a) su modo de operando es inmediato de M (`ImmM`) y M difiere, o inmediato de X (`ImmX`) y X difiere, porque cambia la longitud y el operando; (b) las líneas de `instr_lines` no son idénticas para todas las combinaciones del conjunto. Se calcula generando el C para cada combinación y comparando el texto.

Así no hay que mantener a mano qué instrucciones dependen de M o de X, y cualquier cambio futuro de `instr_lines` se refleja solo. `instr_lines` se invoca sin destinos de llamada indirecta: la sensibilidad se decide sobre la instrucción, no sobre el despacho, que no depende de los anchos.

*Alternativa descartada*: tabla de mnemónicos sensibles a M, a X o a ambos. Es más rápida de escribir pero duplica conocimiento que ya está en `instr_lines`, y un opcode mal clasificado se traduciría en C incorrecto sin aviso.

### D2. Comprobación de entrada de bloque con ancho «no comprobado»
Las direcciones de inicio de bloque con conflicto inofensivo se alcanzan con anchos distintos, así que la comprobación actual fallaría en la ruta que no es la primera. `rt_check_flags` pasa a aceptar `2` como «no comprobar» para M o para X; el generador pasa `2` en el ancho que varía en la dirección de inicio del bloque y el valor fijo en el otro. Con eso el bloque conserva la comprobación de todo lo que sí es cierto.

Las instrucciones posteriores del bloque se emiten con sus propios anchos de `d.flags`: si siguen en conflicto sensible, D1 ya habrá rechazado la función; si un `SEP`/`REP` los unifica, son únicos desde ahí.

*Alternativa descartada*: omitir la comprobación del bloque entero. Pierde la detección de fallos reales en el ancho que no varía.

### D3. Bloqueos restantes
Para cada uno de los cinco bloqueos sensibles se mira el código con `disasm`/`graph`: si el ancho correcto es el mismo que el de la primera combinación en todas las rutas que el juego recorre, se fija con `[[assume]]` con un comentario que lo explique, igual que `02:87AF`; si hay polimorfismo real (como la entrada de `01:D81B`, a la que se llega con M=8 y M=16 y ejecuta `INC` de 8 y de 16 bits), se deja informado y se anota en el README.

## Risks / Trade-offs

- [Un conflicto inofensivo para el C puede ocultar una divergencia real de estado que el resto de la función asume] → La sensibilidad se evalúa en cada dirección en conflicto; las instrucciones que sí dependen del ancho aparecerían como conflicto sensible. Se verifica con las pruebas de ejecución de ALttP y con la comprobación de bloques en compilaciones con `SNES_RT_CHECKS`.
- [Comparar C como texto es frágil si `instr_lines` incluye algo que no depende de los anchos pero cambia entre llamadas, por ejemplo etiquetas] → La comparación usa la misma instrucción y los mismos destinos vacíos; un test cubre un `STA` con X variable y un `LDA #imm` con M variable.
- [`[[assume]]` incorrecto hace C incorrecto sin error] → Cada uno se comprueba a mano contra el ROM y lleva un comentario que lo justifica.

## Migration Plan

Cambio interno del generador y de la cabecera de runtime, que se incrusta en el binario. No hay datos persistidos. Si las pruebas de ejecución de ALttP empeoran, se revierte el commit.
