# Tasks

## 1. Conflictos inofensivos en el generador

- [x] 1.1 En `src/codegen.rs`, sustituir el rechazo de todo conflicto en `check_widths` por la sensibilidad de D1 (inmediato con ancho distinto, o `instr_lines` distinto entre las combinaciones); verificación: `cargo test` pasa con los tests actuales de conflicto
- [x] 1.2 Tests unitarios de generación: `SEP #$30` alcanzado con dos anchos se genera, `STA $10` con solo X distinto se genera, `LDA #imm` con M distinto sigue fallando y el error nombra la dirección y los anchos

## 2. Comprobación en ejecución

- [x] 2.1 En `runtime/snes_rt.h`, hacer que `rt_check_flags` trate `2` como «no comprobar» para M o X; verificación: el test `runtime_arithmetic_stack_and_flags_behave` y el de compilaciones con comprobaciones siguen pasando
- [x] 2.2 En `src/codegen.rs`, emitir `2` en el ancho que varía en la dirección de inicio de cada bloque; test unitario sobre el texto generado de un bloque con M variable (`rt_check_flags(c, 0x..., 2, 1)`) y de uno sin conflicto (valores fijos como hoy)
- [x] 2.3 Test de ejecución: un programa pequeño compilado con `SNES_RT_CHECKS` cuya subrutina vuelve con dos anchos y luego normaliza con `SEP`, que se ejecuta por las dos rutas sin llamar a `rt_flag_mismatch`

## 3. Medición con ALttP

- [x] 3.1 Repetir el barrido de las 1.326 entradas con `codegen` a profundidad 0 y comprobar que los fallos bajan de 95 a los bloqueos sensibles previstos (unos 5)
- [x] 3.2 Comprobar que `alttp_reset_runs_to_the_nmi_wait_loop` sigue pasando, que `02:87AF` sigue necesitando (o no) su `[[assume]]`, y probar la profundidad 3
- [x] 3.3 Estudiar a mano `01:CA17`, `01:D81B`, `07:CF12`, `07:E026` y `08:A80D` con `disasm` y `graph`: añadir un `[[assume]]` comentado en `games/alttp_usa.toml` donde el ancho correcto se compruebe contra el ROM, y anotar el resto como polimorfismo real
- [x] 3.4 Repetir el barrido tras los `[[assume]]` y registrar los fallos que quedan

## 4. Documentación y cierre

- [x] 4.1 Actualizar README y CLAUDE.md con las cifras del barrido y la regla de qué conflictos de anchos admite el generador
- [x] 4.2 Ejecutar `cargo test` completo y `openspec validate relax-width-conflicts --type change --strict`
