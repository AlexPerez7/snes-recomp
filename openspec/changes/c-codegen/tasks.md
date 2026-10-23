# Tasks

## 1. Anchos M/X por instrucción

- [x] 1.1 Añadir a `Discovery` los mapas `flags` y `flag_conflicts`, rellenados en `discover`; verificar con tests de `REP`/`SEP` y de una dirección alcanzada con dos combinaciones, y que `cargo test` sigue pasando
- [x] 1.2 Ejecutar el descubrimiento sobre ALttP, comprobar que sigue dando 82.108 instrucciones y 0 sin resolver, y anotar en el README cuántas direcciones tienen conflicto de anchos

## 2. Cabecera de runtime

- [x] 2.1 Crear `runtime/snes_rt.h` con `Cpu`, las funciones de bus, los ganchos (`rt_unresolved`, `rt_flag_mismatch`, `rt_wai`, `rt_stp`, `rt_brk`, `rt_cop`, `rt_wdm`) y la aritmética (`rt_adc*`, `rt_sbc*`, comparaciones, desplazamientos); verificar con un test de Rust que el texto embebido contiene cada símbolo que el emisor usa
- [x] 2.2 Si hay compilador de C disponible, compilar la cabecera sola con avisos activados y comprobar que no da errores; si no lo hay, la tarea se anota como omitida con el motivo

## 3. Emisor de instrucciones

- [x] 3.1 Crear `src/codegen.rs` con el cálculo de direcciones efectivas para todos los modos de direccionamiento; verificar con tests de texto por modo (directa, indexadas, indirectas, larga, pila relativa)
- [x] 3.2 Emitir carga, almacenamiento, transferencias y pila (`LDA`, `STA`, `TAX`, `PHA`, `PLP`, `XBA`, `XCE`…) con anchos según M/X; verificar con los escenarios de acumulador de 8 y 16 bits
- [x] 3.3 Emitir aritmética y lógica (`ADC`, `SBC`, `AND`, `ORA`, `EOR`, `CMP`, `CPX`, `CPY`, `BIT`, `INC`, `DEC`, desplazamientos y rotaciones, `TSB`, `TRB`); verificar con los escenarios de acarreo y modo decimal
- [x] 3.4 Emitir banderas y sistema (`CLC`, `SEC`, `REP`, `SEP`, `NOP`, `WAI`, `STP`, `BRK`, `COP`, `WDM`) y movimiento de bloques (`MVN`, `MVP`); verificar con tests de texto y que la tabla de opcodes completa se emite sin quedar ninguno sin cubrir (test que recorre los 256)

## 4. Funciones y flujo

- [x] 4.1 Emitir cada función del grafo con etiquetas por bloque, ramas condicionales y saltos con `goto`; verificar con los escenarios de nombre, firma, rama condicional y salida determinista
- [x] 4.2 Emitir llamadas con dirección de retorno en la pila, retornos y tail calls; verificar con los escenarios de llamada y tail call
- [x] 4.3 Emitir el despacho indirecto con `switch` y `rt_unresolved`, y la comprobación de anchos bajo `SNES_RT_CHECKS`; verificar con tests de tabla indirecta, helper en línea y anchos en conflicto (error sin salida parcial)

## 5. Línea de comandos

- [x] 5.1 Añadir el subcomando `codegen <rom> <addr> [--depth N] [-o <fichero>]` con declaraciones `extern` para lo no emitido y escritura de `snes_rt.h` junto al `-o`; verificar con los escenarios de profundidad y de dirección que no es entrada
- [x] 5.2 Generar la función de reset de ALttP con `--depth 1` en `out/`, revisarla a mano contra `disasm` en al menos 3 bloques (la inicialización, el bucle de espera y el bucle principal) y confirmar que `out/` no aparece en `git status`

## 6. Ejecución del C generado

- [x] 6.1 Escribir un arnés en C con bus de mentira (memoria plana y registro de escrituras) y tests de Rust que, si hay compilador, compilan y ejecutan funciones sintéticas (carga y suma de 8 y 16 bits, bucle con rama, `JSR`/`RTS`, tabla indirecta) comprobando registros y memoria finales; si no, se omiten avisando
- [x] 6.2 Compilar y ejecutar con Visual Studio (entorno x64) el C de la función de reset de ALttP con el arnés hasta el bucle de espera, comprobando las escrituras a registros de hardware que `disasm` predice; si no es posible, dejar anotado el motivo
- [x] 6.3 Actualizar README y CLAUDE.md (estructura, comando `codegen`, roadmap y la advertencia sobre el C derivado del ROM) y verificar que los comandos documentados funcionan

## Workflow follow-up

- Revisar el C generado de reset antes de archivar.
- Archivar el cambio con `/opsx:archive` cuando se cumplan los requisitos de revisión del proyecto.
