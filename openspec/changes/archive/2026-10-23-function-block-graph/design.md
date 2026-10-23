# Design

## Context

`discover::discover` hace descenso recursivo y devuelve `Discovery { instrs, unresolved }`, con las instrucciones indexadas por dirección. Las aristas (ramas, saltos, llamadas, destinos de tablas) se calculan en el bucle pero se empujan a la cola de trabajo y se pierden. `instrs` no distingue anchos M/X por dirección: guarda la primera decodificación vista. Ver `proposal.md` para la motivación.

## Goals / Non-Goals

**Goals:**
- Un grafo determinista de bloques y funciones derivado únicamente de `Discovery` (sin releer la ROM).
- Que el análisis de A Link to the Past termine sin pánicos y con diagnósticos legibles.

**Non-Goals:**
- Análisis de pila, registros, bucles o convenciones de llamada.
- Distinguir una misma dirección decodificada con distintos anchos M/X (se hereda la limitación de `instrs`).

## Decisions

1. **Registrar aristas en `Discovery`, no re-derivarlas.** Se añade `edges: BTreeMap<u32, Vec<Edge>>` (por dirección de sitio) con `Edge { to, kind }`, `kind` ∈ {Branch, Jump, Call, Dispatch}. Alternativa: recalcular desde `Instr::flow()` en el módulo nuevo; se descarta porque duplicaría la lógica de tablas, helpers en línea y `no_return`, y divergiría de ella.
2. **Bloques por líderes.** Líderes: entradas de función, destinos de ramas/saltos/despachos y la instrucción siguiente a un terminador o salto condicional. Una pasada ordenada por dirección corta las secuencias. La caída se toma de la arista registrada, no de `addr + len`, para saltar las tablas en línea.
3. **Funciones por alcanzabilidad intraprocedural.** Desde cada entrada se siguen los sucesores de bloque (ramas, saltos, caídas) sin cruzar llamadas ni entradas de otras funciones; un salto a otra entrada es `TailCall`. Alternativa: dominadores; se descarta por excesiva para este paso.
4. **Los destinos de tablas son entradas de función.** Suelen acabar en `RTS` devolviendo al llamador del despachador; tratarlos como funciones propias evita fusionar decenas de manejadores en una. El sitio de despacho los referencia como sucesores `Dispatch`.
5. **Bloques compartidos.** Si dos funciones alcanzan el mismo bloque, se asigna a la de menor entrada y se emite diagnóstico; el generador decidirá después si duplicar.
6. **`BTreeMap` y direcciones `u32` de 24 bits**, como el resto del código, para salida determinista.

## Risks / Trade-offs

- [Direcciones decodificadas con otros anchos M/X] → Se hereda de `instrs`; el diagnóstico de sucesor no descubierto lo hará visible.
- [Funciones sobredimensionadas por código compartido en ALttP] → Diagnóstico de bloques compartidos y regla determinista de asignación.
- [Tamaño: ~82k instrucciones] → Todo es lineal sobre `BTreeMap`; se comprueba el tiempo en `--release`.
