# Design

## Context

La motivación está en `proposal.md`. Para el diseño importa lo siguiente:

- **Frontera entre runtime y C generado.** El C generado solo llama al runtime a través de `snes_rt.h`: `bus_read8`/`bus_write8`, `rt_*`, `joy1_state`, `ppu_*` y `game_opt`, más 53 funciones `static inline` que se compilan dentro del propio código generado. En sentido contrario, el runtime solo llama al código generado en un punto: `snes_call_entry` (`runtime/snes_core.inc:713`). Hoy ese símbolo se resuelve al enlazar con `snes_entry.c`.
- **Tamaño del C generado.** `codegen --all` produce unos 13 MB de C para ALttP (`out/rom`), y el C propio del perfil (`game/*.c`) se compila junto a él.
- **Generación hoy.** `port.rs::generate` toma un `Rom` y un `GameProfile`, y escribe en disco con `write_sources` y `write_game_sources`. `generate_all()` ya produce todo el texto en memoria (`FullOutput`), así que lo único atado al disco es la escritura.
- **TinyCC ya está probado como compilador del C generado.** En el PC compila los `rom_*.c` (`Compiler::Tcc` y el test con `SNES_TCC_EXE`), así que el C que emitimos no necesita cambios para él.
- **El crate solo tiene binarios** (`snes-recomp` y `snes-port`, ambos sobre `src/main.rs`). No hay ningún `[lib]`.
- **Android.** Desde targetSdk 29, una app no puede ejecutar (`execve`) ficheros de su propio directorio de datos. La memoria ejecutable anónima (`mmap` con `PROT_EXEC`, la que usan los JIT) sí está permitida a las apps normales. Que `dlopen` funcione sobre un `.so` del directorio de datos no está garantizado por la política de Android, y por eso se mide.
- **Dispositivo de prueba.** Galaxy S25 Ultra (Snapdragon 8 Elite), un teléfono de gama alta. Un teléfono de gama media puede ser entre 2,5 y 3 veces más lento por núcleo, y los criterios de decisión lo tienen en cuenta.

## Goals / Non-Goals

**Goals:**
- Responder con números si el camino «el teléfono genera y compila con libtcc» sirve para un APK publicable.
- Dejar hechas las dos piezas que necesitan tanto ese camino como el plan B: el runtime separable y la generación en memoria.
- Que el PC no cambie: mismo ejecutable y mismos tests.

**Non-Goals:**
- Frontal SDL en Android, imagen en pantalla, audio por el altavoz, controles táctiles y ciclo de vida. La prueba corre sin presentar imagen; el audio se genera (SPC700 y DSP corren) pero se descarta.
- Firma de release, CI y publicación. La app de la prueba nunca se sube a una release.
- Optimizar el C generado para TinyCC. Si hace falta, será un cambio aparte, informado por estas medidas.

## Decisions

**1. El punto de entrada se registra con un puntero, no con un símbolo resuelto al enlazar.**
`snes_core.inc` llama a través de `int (*snes_entry_fn)(Cpu *, uint32_t)`. En la construcción de una pieza, `snes_entry.c` lo inicializa de forma estática, así el PC no cambia. En modo biblioteca, el módulo del juego lo registra con `snes_rt_register_entry()`. Los ganchos del perfil siguen dentro del módulo del juego, porque son C que el generador referencia.
- *Alternativa descartada:* que el runtime busque `snes_call_entry` con `tcc_get_symbol` o `dlsym`. Ataría el runtime a un cargador concreto. Con el registro, el mismo runtime sirve con libtcc en memoria, con `dlopen` (plan B) y con el enlace de una pieza.

**2. Modo biblioteca con una macro (`SNES_RT_LIBRARY`).**
Con la macro definida, el núcleo, la PPU y la SPC se compilan sin `main` y exportan las funciones de `snes_rt.h` con visibilidad pública. En la prueba se compilan con clang -O2 del NDK. Sin la macro, todo queda como hoy.
- *Alternativa descartada:* compilar todo con libtcc en el teléfono. La PPU y el DSP son la parte más pesada de cada fotograma, y compilarlos con TinyCC perdería justamente el margen que queremos medir.

**3. Las funciones `static inline` de `snes_rt.h` no cambian.**
TinyCC no las inlinea, pero tampoco cambia la semántica, y la diferencia queda reflejada en la medición del PC (tcc frente a gcc -O2). Si el rendimiento no alcanza, la primera palanca es esta, y se estudiará en un cambio aparte.

**4. API de generación: `generate::generate_sources(rom: &[u8], profile: &GameProfile) -> Result<Generated, String>`** (las fuentes y el resumen del generador).
Va en un módulo de la biblioteca nueva (`src/lib.rs` declara los módulos que antes declaraba `main.rs`). `port.rs` y `codegen --all` la usan y solo escriben el resultado (`generate::write_sources`). Para Android, una capa `extern "C"` mínima (`snes_gen_generate`, `snes_gen_error`, `snes_gen_count`, `snes_gen_name`, `snes_gen_text`, `snes_gen_free`) entrega los pares nombre/texto o un mensaje de error. Esa capa compila como `cdylib` con el target `aarch64-linux-android`. El perfil llega como texto del `.toml` más los ficheros de su C, porque en el teléfono no hay catálogo de carpetas.
- *Alternativa descartada:* llamar al ejecutable `snes-recomp` desde la app. Android no deja ejecutar binarios del directorio de datos, y empaquetarlo como «lib» para ejecutarlo sería un truco frágil.

**5. Compilación en el teléfono: libtcc en memoria, con los símbolos pasados uno por uno.**
`tcc_new`, después `tcc_set_options("-nostdinc -nostdlib")` (antes del tipo de salida, que es donde libtcc agrega sus rutas por defecto), `tcc_set_output_type(TCC_OUTPUT_MEMORY)`, las cabeceras de la app como `sysinclude`, `tcc_add_symbol` por cada entrada de `snes_rt_symbols()`, un `tcc_add_file` por fuente, `tcc_relocate` y por último `tcc_get_symbol("snes_rt_module_init")`, que registra el punto de entrada.
- Con `-nostdlib`, libtcc no busca por sí mismo los símbolos del runtime. Además, en Android cada biblioteca de la app vive en su propio espacio de nombres. Por eso el runtime, en modo biblioteca, da una tabla de símbolos (spec `runtime-library`).
- El dispositivo no tiene cabeceras del sistema. Lo que necesita el C del juego cabe en tres cosas: el `include/` de TinyCC (con `tccdefs.h`), una `stdint.h` mínima propia para LP64 (`spikes/tcc_include/stdint.h`) y `snes_rt.h`. No hace falta `libtcc1.a`: el C del juego solo llama al runtime.
- libtcc se compila con el NDK como un único fuente (`libtcc.c` con `ONE_SOURCE`), con un `config.h` propio de cuatro líneas (`CONFIG_TCC_PREDEFS 0`: lee `tccdefs.h` de sus cabeceras), y viaja como biblioteca aparte (LGPL 2.1).
- *Alternativa medida en paralelo:* que tcc escriba un `.so` en `filesDir` (que depende de `libsnesrt.so`) y cargarlo con `dlopen`. Si funciona en el S25 con targetSdk 35, el resultado se podría cachear entre arranques. Si no, cada arranque recompila en memoria, y el tiempo de compilación pasa a contar en cada inicio.

**5 bis. Contextos de ejecución sobre hilos en Android.**
bionic no tiene `makecontext` ni `swapcontext`, y el runtime los usa (fibras en Windows) para los contextos que cambia la intro de ALttP. En Android, o con `SNES_RT_THREAD_CONTEXTS`, cada contexto corre en un hilo propio. Un mutex y una variable de condición hacen que solo uno corra a la vez. Los contextos nunca terminan y son 5 como mucho, así que no hace falta reciclar hilos.
- *Alternativa descartada:* un cambio de contexto escrito en ensamblador para arm64. Sería más rápido, pero es más código propio que mantener. Se cambia de contexto pocas veces por fotograma, así que el costo de los hilos no se nota.

**6. Primero se mide en el PC, después en el teléfono.**
En Linux/Windows: el runtime sin vídeo con el C compilado por tcc frente a gcc/clang -O2 (`--frames 3000` sin entradas: la intro y la demostración del juego, siempre el mismo recorrido; el tiempo de cada fotograma sale con `--frame-times`), y el tiempo y la memoria pico de libtcc compilando las fuentes en memoria. Si en el PC TinyCC ya no deja margen, el teléfono no lo va a salvar y se pasa directamente al plan B sin construir el APK de la prueba.

**7. Criterios de decisión, fijados antes de medir (en el S25 Ultra, sobre la media de 3 corridas).**

| Medida | Camino on-device viable si… | Razón |
|---|---|---|
| Tiempo de emulación por fotograma (CPU + PPU + SPC, sin presentar), p99 | ≤ 5,5 ms | Un tercio de 16,64 ms, para que un teléfono 3 veces más lento todavía llegue a 60 fps |
| Compilar con libtcc todas las fuentes en memoria | ≤ 4 s si se recompila en cada arranque; ≤ 20 s si el `.so` se cachea | Arranque aceptable |
| Memoria pico del proceso durante la compilación | ≤ 400 MB | Que no lo mate el sistema en gama media |
| Generación con la biblioteca en el teléfono | ≤ 10 s, una vez por versión (se cachea el C) | Primer arranque aceptable |

- Si se cumplen todos, se sigue con el camino on-device.
- Si falla solo el rendimiento por fotograma, se prueba antes una mejora del C para TinyCC. Si el margen que falta es más del doble, se pasa al plan B.
- Si falla la memoria ejecutable, se pasa al plan B, siempre que `dlopen` desde `filesDir` funcione.
- Si fallan las dos cosas, ningún APK publicable puede llevar el juego. Queda solo el APK personal armado en el PC.

**8. Dónde vive cada cosa.**
- La app de la prueba vive en `snes-recomp/spikes/android/`: es investigación del motor y la regla del proyecto dice que la investigación sale de aquí. No incluye la ROM, que se elige en el teléfono con el selector de documentos, ni C generado.
- El APK definitivo y su release viven en `alttp-recomp`, en cambios posteriores, que consumirán el runtime en modo biblioteca, la biblioteca del generador y una plantilla Android del motor.

## Risks / Trade-offs

- [TinyCC para arm64 está menos probado que para x86_64 y podría fallar con algún C generado] → Primero se compila todo el C con `tcc` cruzado a arm64 en el PC. Cualquier error se reporta con el fichero y la función, y se decide si se corrige el generador o se descarta TinyCC.
- [Samsung (One UI) podría restringir la memoria ejecutable más que el Android de Google] → Es justamente una de las cosas que mide la prueba. Si falla, el plan B no la necesita.
- [El hilo Java tiene poca pila: 1 MiB por defecto en Android, frente a los 8 MiB del hilo principal en el PC] → La app ejecuta el juego en un hilo creado con 64 MiB de pila. Los contextos sobre hilos usan 1 MiB, igual que con `ucontext`.
- [El runtime termina con `exit()`] → Para la prueba basta con ejecutar en un proceso aparte (`:bench`). El frontal definitivo tendrá que terminar sin `exit()`, pero eso queda fuera de este cambio.
- [El S25 Ultra oculta problemas de la gama media] → Los criterios exigen un margen de 3 veces. Además, se repite la medición fijando el hilo a un solo núcleo que no sea de los principales, con `sched_setaffinity`, como aproximación.
- [Licencia de libtcc (LGPL 2.1)] → Se enlaza como `.so` aparte, nunca de forma estática. El APK de la prueba no se publica. La entrega de la licencia y las fuentes queda para el APK definitivo en `alttp-recomp`.
- [Que la generación en el teléfono use mucha memoria por el descubrimiento y el grafo] → Se mide junto con la compilación. Como el C se cachea por versión, ese costo solo se paga una vez.
- [Que el refactor del runtime rompa el PC] → La construcción de una pieza sigue siendo la de siempre. El test de equivalencia del spec `runtime-library` compara los volcados `--dump` de las dos formas.

## Migration Plan

No hay migración para los usuarios: en el PC el ejecutable y la caché no cambian, salvo que `current_version` cambia porque cambia el runtime, y eso dispara una reconstrucción automática como en cualquier versión nueva. Para deshacerlo, basta revertir el cambio. La app de la prueba se puede borrar sin efectos.

## Open Questions

- Resuelta: la versión de TinyCC es la rama `mob` (se probó con `e23dabe`). La 0.9.27 que descarga `download.rs` es solo un binario para Windows x64, y la `mob` trae soporte de Android en su `configure` y en `tccdefs.h`.

## Resultados

### En el PC (31-10-2026)

Máquina virtual Intel Xeon a 2,1 GHz con 4 núcleos (compartida, con ruido de ±10 %); gcc 13.3; TinyCC 0.9.28rc `mob@e23dabe`. Recorrido: 3000 fotogramas sin entradas, 3 repeticiones (`spikes/bench_pc.sh`). Tiempo por fotograma sin contar el primero, que incluye el arranque.

| Construcción | Media (ms/fotograma) | p99 (ms) |
|---|---|---|
| Todo con gcc -O2, en una pieza | 9,04 | 15,40 |
| Todo con TinyCC, en una pieza | 15,83 | 31,27 |
| Runtime con gcc -O2 (biblioteca) + C del juego con libtcc en memoria | 7,56 | 15,35 |

Tres repeticiones más, alternando las dos construcciones relevantes: gcc 7,74 / 8,26 / 8,45 ms y libtcc en memoria 7,76 / 8,18 / 8,00 ms (p99 de 14,6 a 16,4 ms en ambas). **Quedan dentro del ruido.**

- **Lo que cuesta es el runtime (PPU por línea, SPC700 y DSP), no la CPU traducida.** Compilar el C del juego con TinyCC no se nota si el runtime va con un compilador optimizador. Esto confirma la decisión 2.
- **Compilación en memoria con libtcc:** 22 ficheros (12,5 MiB de C) en **0,26 a 0,45 s**, más **12 a 18 ms** de enlace; memoria pico del proceso **28 MiB**.
- **Modo «dispositivo»** (`-nostdinc -nostdlib`, solo las cabeceras de TinyCC, la `stdint.h` mínima y `snes_rt.h`, tabla de símbolos y libtcc compilado desde un fuente con el `config.h` de la app): mismo tiempo de compilación y **mismo estado final** (WRAM, VRAM, CGRAM, OAM y registros) que gcc en una pieza.
- **TinyCC para arm64** (compilador cruzado `arm64-tcc`): los 22 ficheros compilan sin errores en 0,43 s. Sus objetos solo dejan sin resolver funciones del runtime.
- **JNI de la app, probado en Linux** (el mismo `spike_jni.c` y `Native.java`, compilados para x86_64 con un JDK): generar con la biblioteca del generador da 23 ficheros (431 605 líneas, 12,5 MiB) en **0,96 s** con **134 MiB** de memoria pico, y rechaza una ROM ajena nombrando los dos CRC32. Medir en memoria y con el plan B (`.so` y `dlopen`) funciona, con los mismos hashes que el PC.
- **Contextos sobre hilos:** el test de equivalencia (1200 fotogramas, la intro cambia de contexto) da los mismos volcados con `ucontext` y con hilos.

**Decisión del punto 3.4: se sigue al teléfono, no al plan B.** En el PC, TinyCC no le resta margen al C del juego, así que el plan B (C del juego compilado con clang en el PC) no ganaría rendimiento. Solo serviría si Android negara la memoria ejecutable. Lo que queda por saber en el teléfono es si el S25 Ultra cumple el p99 de 5,5 ms. Este PC hace unos 8 ms de media y 15 ms de p99; un núcleo del S25 debería ser entre 2 y 3 veces más rápido. Ese margen depende del runtime, y afecta igual a cualquier camino, incluido un APK armado en el PC.

### En el S25 Ultra

<!-- Pendiente: grupo 4 de tasks.md, con la app de spikes/android (ver su README). Anotar aquí lo que muestra «Ver resultados» para run_mem, run_mem_cpu0 y run_so, y el mensaje de «Generar el C». -->

### Decisión final

<!-- Pendiente de las medidas del teléfono (tarea 5.1). -->
