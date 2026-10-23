## Context

`discover.rs` ya analiza cada función por separado para cada combinación de anchos de entrada y recorre caminos `(dirección, anchos)`, pero al terminar solo guarda un `Flags` por dirección (`flags`) y el conjunto de los vistos (`flag_conflicts`). `graph.rs` agrupa por dirección y `codegen.rs` (`check_widths`) rechaza la función si una instrucción con conflicto cambia de C o de longitud. En ALttP son 7 funciones; en `01:D81B` el bloque `01:D815` se alcanza con M=X=8 (tras `SEP #$30` en `D7EA`/`D807`) y con M=X=16 (rama de `D7C8`), y `INC $042C` cambia de ancho. `D81B` empieza con `REP #$30`, así que el conflicto se acaba ahí: los anchos de cada copia son únicos y calculables.

## Goals / Non-Goals

**Goals:**
- Emitir C correcto para bloques alcanzados con anchos distintos.
- No cambiar el C de las funciones que hoy se generan (mismas etiquetas, mismo texto), para que el resto de tests y la ROM sigan igual.
- Latch H/V del runtime sin vídeo, con tests.

**Non-Goals:**
- Especializar por anchos los bloques cuyo C no cambia.
- Resolver conflictos que dependen de datos (saltos indirectos sin asociar a anchos).
- Cambios en el renderizador.

## Decisions

1. **Las variantes salen del descubrimiento, no se recalculan en el generador.** El descubrimiento ya recorre `(dirección, anchos)`; además de `flag_conflicts` registra en `Discovery::transitions` las aristas entre variantes `((instrucción, anchos) → (siguiente, anchos))`: continuación, ramas, saltos y retornos de llamadas. El grafo de bloques no cambia; `codegen` recorre las variantes desde la entrada. Recalcular el efecto de `REP`/`SEP`/`PLP`/retornos en `codegen` duplicaría lógica que ya tiene `discover.rs`. Alternativa descartada: reanalizar cada variante en el generador.
2. **Solo se duplican los bloques con conflicto que cambia el C.** Para una función se parte de las variantes alcanzables desde su entrada; los bloques sin conflicto relevante y sus etiquetas quedan como hoy. Una copia se llama `L_<dir>_m<M>x<X>` (anchos en bits, 8 o 16) solo cuando existe más de una; así el C sin conflictos no cambia.
3. **Un salto va a la variante con los anchos con los que se sale.** El destino de cada rama/salto/continuación se resuelve con la arista de variante registrada, no con los anchos de la dirección. Con anchos únicos coincide con la etiqueta actual.
4. **`rt_check_flags` comprueba los anchos de la copia** (sin el `2` de «variable» cuando la variante es única), de modo que `SNES_RT_CHECKS` verifica las copias de verdad.
5. **Si falta la arista de una variante** (salto indirecto sin asociar) la función sigue siendo stub con el mensaje actual, para no emitir C erróneo.
6. **Latch H/V:** línea = `tick_in_frame * 262 / frame_len` y columna 0-339 con el resto, como el modelo de `$4212`; solo `$2137` captura, `$213C/$213D` alternan byte bajo/bit 0 y `$213F` devuelve el bit 6 y reinicia.

## Risks / Trade-offs

- [Crecimiento del C por las copias] → solo se copian bloques con conflicto que cambia el C, ~7 funciones.
- [Aristas de variante incompletas por la heurística de `PLP`/retornos] → el stub sigue siendo la salida segura y los tests comprueban el caso contrario con programas pequeños.
- [Las líneas del contador H/V no coinciden con el hardware porque el vblank del runtime empieza en 9/10 del fotograma] → basta con que el contador avance de forma monótona; se anota para cuando se modele el hardware con más precisión.

- [Un inmediato cambia de longitud con M o X] → el flujo de instrucciones de cada variante es otro y no cabe en un grafo por dirección; esas funciones (`01:CA17`, `08:A8E3` en ALttP) siguen siendo stub con el mensaje de anchos.
- [Una función entrada con varios anchos] → el prólogo elige la copia con `c->m`/`c->xf` en ejecución; la primera encontrada es la de por defecto.
