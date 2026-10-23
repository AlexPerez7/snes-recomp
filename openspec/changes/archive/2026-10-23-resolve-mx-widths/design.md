## Context

Ver `proposal.md` - Why. Hoy `discover` recorre el código por descenso recursivo con un trabajo pendiente `(dirección, Flags)` y un conjunto `seen`. `next_flags` solo entiende `REP`/`SEP`; las llamadas (`JSR`/`JSL`) siguen en la instrucción siguiente con los mismos anchos, y `PLP`, `RTI` y `XCE` no cambian nada. `Discovery.flags` guarda un `Flags` por dirección y `flag_conflicts` los demás; `[[assume]]` fuerza anchos en una dirección. El grafo y el generador consumen `Discovery` sin conocer cómo se obtuvieron los anchos.

## Goals / Non-Goals

**Goals:**
- Decodificar cada instrucción con los anchos con los que se ejecuta en los casos habituales de ALttP (rutinas que terminan con `REP`/`SEP`, pares `PHP`/`PLP`).
- Que el resultado siga siendo determinista y que termine siempre.
- Sin cambios en el formato de `games/*.toml` ni en la API de `graph` y `codegen`.

**Non-Goals:**
- Resolver polimorfismo real (una rutina cuyo código depende de los anchos de entrada y que se llama con ambos): se informa y se corrige con `[[assume]]`.
- Seguir `XCE`/`RTI` ni la aritmética de la pila.

## Decisions

### D1. Resúmenes de retorno por contexto `(entrada, anchos)`
Cada vez que se alcanza una entrada de función (destino de una llamada) con unos anchos, se abre un contexto `(entrada, Flags)`. El descubrimiento propaga a cada camino el contexto de la función que lo contiene. Al llegar a un `RTS`/`RTL` se añade el `Flags` actual al conjunto de retornos de ese contexto: `returns: BTreeMap<Ctx, BTreeSet<Flags>>`.

Una llamada no continúa de inmediato: registra una continuación `(ctx llamado) -> [(dirección de retorno, contexto del llamador, pila)]`. Cuando el contexto llamado añade un retorno nuevo, cada continuación suscrita arranca un trabajo `(retorno, flags nuevos)`; si el contexto ya tenía retornos al suscribirse, se disparan en ese momento. Es un punto fijo monótono (los conjuntos solo crecen y son finitos: 4 combinaciones de anchos por entrada), así que termina.

*Alternativa descartada*: C, especializar por `(dirección, Flags)` y duplicar el código. Resuelve más, pero multiplica funciones C y toca grafo y generador; queda para más adelante si los residuos lo piden.

### D2. Qué es una función para el contexto
Un camino hereda el contexto de la entrada que lo originó. `JSR`/`JSL` y las entradas de la configuración abren un contexto nuevo; `JMP`/`JML` y los saltos de despacho continúan en el contexto actual, de modo que un retorno alcanzado por ellos se atribuye a quien saltó (cola de llamada). Es lo que hace el procesador: el `RTS` vuelve al llamador original.

### D3. Llamadas sin retorno
Al terminar el punto fijo, toda continuación cuyo contexto llamado no tenga retornos se resuelve con los anchos previos a la llamada y se anota en `calls_without_return`. Se hace después del punto fijo, no antes, para no activar caminos con anchos erróneos mientras un retorno aún puede aparecer. Las que la configuración declara `no_return` no continúan y no se anotan. Esa segunda pasada puede abrir contextos nuevos, así que se itera hasta que no cambie nada.

### D4. Pila de PHP/PLP
El estado del camino pasa de `Flags` a `(Flags, Vec<Item>)` con `Item = Flags | Other`. `PHP` apila los anchos actuales; `PHA`, `PHX`, `PHY`, `PHB`, `PHD`, `PHK` y `PEA`/`PEI`/`PER` apilan `Other` (con el número de bytes que no importa: se cuenta una entrada por instrucción). `PLP` desapila: si el tope es `Flags`, los restaura; si es `Other` o la pila está vacía, deja los anchos y anota el `PLP` en `untracked_plp`. `PLA`/`PLX`/`PLY`/`PLB`/`PLD` desapilan una entrada. La pila se limita a 16 entradas (al desbordarse se descarta la más antigua) para que el espacio de estados sea finito. La pila forma parte de la clave de `seen`.

Las llamadas no tocan la pila del llamador: el llamado empieza con la pila vacía y su retorno se continúa con la pila del llamador, lo que supone que el llamado la deja equilibrada, como hacen las rutinas bien formadas.

*Alternativa descartada*: un solo registro de "anchos guardados". Falla con `PHP` anidados.

### D5. `assume` y diagnósticos
`assume` se aplica al calcular los anchos de una dirección, antes de decodificar, y gana a todo. Los conflictos residuales se siguen registrando en `flag_conflicts`. Los campos nuevos de `Discovery` son `calls_without_return: BTreeSet<u32>` (dirección de la llamada) y `untracked_plp: BTreeSet<u32>`. `discover --widths` los imprime junto con los conflictos.

## Risks / Trade-offs

- [Más contextos que hoy: el trabajo crece hasta 4 veces por entrada] → Las entradas son ~1.300 y cada contexto solo se recorre una vez por `seen`; se mide el tiempo en la tarea de ALttP.
- [Una llamada con retorno mal resuelto activa código fantasma] → Los diagnósticos lo hacen visible y `[[assume]]` lo corrige.
- [Rutinas que manipulan la pila a mano (`PLA` para descartar la dirección de retorno, `PEA`+`RTS` como salto)] → La pila abstracta se desincroniza y el `PLP` posterior se informa como sin emparejar; no empeora el estado actual.
- [Polimorfismo real queda como conflicto] → Aceptado; se cuantifica en la medición y, si es relevante, motiva el cambio C.

## Migration Plan

Cambio interno al descubrimiento. Las cifras de ALttP cambian; se actualizan en README y se contrastan a mano. Si la medición empeora (más instrucciones inalcanzadas de las esperadas), se revierte el commit; no hay datos persistidos que migrar.

## Open Questions

- ¿Basta el límite de 16 entradas de pila? Se comprueba en la medición con ALttP (profundidad máxima observada).
