## Context

`Emitter::generate(entry, depth)` recorre las funciones desde una entrada con una cola por niveles y devuelve `Result<String, String>`: el primer error de cualquier función aborta todo. `Emitter::function(entry)` ya genera una sola función y devuelve su texto y las funciones a las que se refiere, así que sirve tal cual para el modo completo. Los bloques compartidos entre funciones ya se emiten en cada una (decisión 10 de `c-codegen`) y los conflictos de anchos solo rechazan la función cuando cambian el C o la longitud (`relax-width-conflicts`). Sobre ALttP el grafo tiene 2.488 funciones, 15.989 bloques y 82.025 instrucciones. Ver `proposal.md` para la motivación.

## Goals / Non-Goals

**Goals:**
- Un solo comando que deje en un directorio todo el C de la ROM, con las funciones que se puedan traducir y un informe claro de las que no.
- Comprobar que ese C compila y enlaza con Visual Studio, y dejar medido cuánto del juego queda en stub y por qué.

**Non-Goals:**
- Ejecutar el juego completo o implementar el runtime de hardware.
- Arreglar las causas de los stubs que aparezcan: se miden y se apuntan para cambios posteriores.
- Optimizar el C, ni reducir su tamaño, ni compilación incremental.

## Decisions

1. **Mejor esfuerzo con stub, no aborto.** `generate_all` llama a `function(entry)` para cada función del grafo; si devuelve `Err`, emite `void f_xx(Cpu *c) { rt_unsupported(c, 0x...); }` y guarda la causa. Alternativa: abortar como `generate` y pedir arreglarlo todo antes; se descarta porque el objetivo es medir, y un solo fallo ocultaría el resto. Alternativa: omitir la función sin definir; se descarta porque el enlace fallaría y el fallo en ejecución sería un símbolo sin resolver en vez de un aviso con dirección.
2. **Gancho propio `rt_unsupported`, no `rt_unresolved`.** Un stub no es un salto indirecto sin destino; son fallos distintos con distinto arreglo (código del emisor frente a configuración del juego). Se declara en `snes_rt.h` y en el runtime simulado de los tests.
3. **Un fichero por banco de la entrada.** ALttP son 82.000 instrucciones; un único `.c` daría un fichero de decenas de megabytes y compilaciones lentas sin posibilidad de paralelizar. El banco es una frontera estable y barata de calcular, y deja el reparto legible (`rom_00.c`, `rom_02.c`...). Alternativa: trozos de N funciones; se descarta por inestable (un cambio mueve todos los trozos). Un bloque compartido que vive en otro banco se emite igualmente en cada función que lo alcanza, como hasta ahora.
4. **Declaraciones en `snes_funcs.h`.** Contiene `void f_xx(Cpu *c);` de todas las funciones, ordenadas, e incluye `snes_rt.h`. Cada `rom_BB.c` solo incluye ese. Así no se calcula por banco qué hay que declarar.
5. **Agrupar causas por texto del error.** `function` devuelve `String`; el informe clasifica por subcadenas conocidas («different register widths», «has no translation», «which is not code», «unsupported indirect») y deja el resto como «otras». Alternativa: tipar el error con un enum; se descarta ahora por alcance, y se hará si los números muestran que merece la pena.
6. **Salida determinista.** Todo se recorre en orden de dirección (`BTreeMap`/`BTreeSet`) y no se escribe fecha ni ruta absoluta en los ficheros.
7. **Verificación por compilación y enlace, no por ejecución.** El test genera ALttP completo en un directorio temporal, compila cada banco a objeto con el compilador encontrado por `cctest`, y enlaza con el runtime simulado y un `main` vacío. Eso prueba sintaxis, tipos y símbolos. Se omite solo si faltan ROM o compilador, como el resto de pruebas con ROM.

## Risks / Trade-offs

- [El C completo tarda mucho en compilar con MSVC] → Se mide en la primera tarea; se compila por banco en paralelo con `/MP` y sin optimización (`/Od`). Si pasa de un minuto, el test se marca `#[ignore]` y el comando para lanzarlo se documenta en `CLAUDE.md`.
- [Muchas funciones acaban en stub y el resultado «compila» engaña] → El resumen muestra el porcentaje de stubs y sus causas; la verificación apunta los números en las tareas, no solo el éxito de la compilación.
- [Un stub se llama desde código válido y oculta un fallo hasta ejecutar] → Es el comportamiento buscado: falla con la dirección en `rt_unsupported`, no con un símbolo sin resolver.
- [Cambiar `generate` rompe tests existentes] → El modo con dirección y profundidad no se toca; `generate_all` es un método nuevo que comparte `function`.
