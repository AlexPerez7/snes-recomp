## 1. Generación de todas las funciones

- [x] 1.1 Declarar `rt_unsupported(Cpu *c, uint32_t entry)` en `runtime/snes_rt.h` y en `MOCK_RUNTIME` de `src/cctest.rs`; verificar con `cargo test header_defines_every_symbol_the_emitter_uses`
- [x] 1.2 Añadir `Emitter::generate_all` en `src/codegen.rs`: una función por entrada del grafo, stub con `rt_unsupported` si `function` falla, resultado con C por banco, `snes_funcs.h` y la lista de fallos con su causa; verificar con tests sobre ROMs pequeñas: una función que falla se emite como stub y la otra completa, cada función está una sola vez, el reparto por banco y la salida idéntica en dos ejecuciones
- [x] 1.3 Clasificar las causas de los stubs (anchos, sin traducción, salto a no código, indirecto no soportado, otras) y verificar con un test que cada mensaje conocido cae en su grupo

## 2. Línea de comandos

- [x] 2.1 Añadir `--all` a `codegen` en `src/main.rs`: sin dirección, exige `-o <dir>`, escribe `rom_BB.c`, `snes_funcs.h` y `snes_rt.h`, imprime por stderr el resumen (totales, stubs por causa, cada stub con dirección y motivo, líneas de C); actualizar `USAGE`; verificar ejecutando `cargo run --release -- codegen roms/alttp_usa.sfc --all -o out/rom` y comprobando los ficheros y el resumen
- [x] 2.2 Verificar que `--all` sin `-o` termina con error sin escribir nada, y que el modo con dirección y `--depth` no cambia (los tests de `codegen` existentes siguen en verde)

## 3. Verificación sobre ALttP

- [x] 3.1 Test (se omite sin ROM o compilador) que genera ALttP completo en un directorio temporal, compila cada banco a objeto y enlaza con el runtime simulado y un `main` vacío; verificar que pasa con Visual Studio y anotar en este fichero el tiempo de compilación y el tamaño del C
- [x] 3.2 Anotar en este fichero el número de funciones totales, generadas y con stub, y sus causas; verificar que el resumen del comando y los números anotados coinciden
- [x] 3.3 Si la compilación completa pasa de un minuto, marcar el test de 3.1 con `#[ignore]` y documentar el comando que lo lanza; si no, dejarlo activo
- [x] 3.4 Actualizar `CLAUDE.md` con el comando `codegen --all` y la estructura de la salida; verificar que el comando documentado funciona tal cual
- [x] 3.5 `cargo test` en verde, incluido `alttp_reset_runs_to_the_nmi_wait_loop`

## Mediciones (ALttP USA, Visual Studio Build Tools, `/MP`, `SNES_RT_CHECKS`)

- Funciones: 2.488 en total, 2.481 generadas y 7 en stub (0,3 %). Las 7 son conflictos de anchos que cambian el C: `01:CA17`, `01:D81B`, `07:CF12`, `07:E026`, `08:A80D`, `08:A80F`, `08:A8E3`.
- Tamaño: unas 237.000 líneas de C (240.000 con cabeceras) en 19 ficheros `rom_BB.c`, cerca de 7 MB.
- Tiempos: generar el C, 0,4 s; compilar y enlazar todo con el runtime simulado, 3 s. No hace falta `#[ignore]` en el test (tarea 3.3).
