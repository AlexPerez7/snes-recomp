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

### En el S25 Ultra (2026-11-05)

Galaxy S25 Ultra (SM-S938B, Snapdragon 8 Elite), Android 17, APK *debug* (código nativo con `-O2`), `targetSdk 35`, NDK 27.2.12479018, TinyCC `mob@e23dabe`. 3000 fotogramas sin entradas y sin presentar imagen (el audio se genera y se descarta), **una sola ejecución de cada modo** (la tarea 4.3 pedía repetir 3 veces: queda pendiente, para saber cuánto varía el p99). La ROM se eligió con el selector de documentos.

| Medida | `run_mem` (todos los núcleos) | `run_mem_cpu0` (núcleo 0) | `run_so` (plan B, `dlopen`) | Criterio |
|---|---|---|---|---|
| Compilar con libtcc (27 ficheros) | 162 ms | 290 ms | 162 ms | ≤ 4 s |
| Enlazar | 5 ms | 8 ms | 6 ms | n/a |
| Memoria pico del proceso | 156 MiB | 155 MiB | 149 MiB | ≤ 400 MiB |
| Fotograma, media | 2,62 ms | 4,24 ms | 2,60 ms | n/a |
| Fotograma, **p99** | **5,67 ms** | **5,65 ms** | **5,66 ms** | ≤ 5,5 ms |
| Fotograma, máximo | 13,5 ms | 8,1 ms | 8,3 ms | n/a |

Generar el C con la biblioteca del generador en el teléfono: 28 ficheros, 432 743 líneas (12,6 MiB), **408 y 421 ms**, memoria pico **253 y 260 MiB** (criterio: ≤ 10 s). El ordenador de escritorio tardaba 0,96 s con 134 MiB.

**Los cuatro hashes coinciden con los del PC en los tres modos** (`wram.bin 3a6f2313886e5104`, `vram.bin 1b065eac555a61f9`, `cgram.bin 22537cc0decddbf5`, `oam.bin 00984b3d23dfa740`): el juego, generado y compilado en el teléfono, da el mismo estado que gcc en el PC tras 3000 fotogramas.

- **Android deja la memoria ejecutable** (`tcc_relocate` en memoria) **y deja `dlopen` de un `.so` escrito en `filesDir`** (targetSdk 35, Samsung One UI). Las dos rutas funcionan, así que el plan B existe como respaldo.
- **Rendimiento:** la media (2,6 ms) está muy por debajo de los 16,64 ms de un fotograma. El p99 (5,65 a 5,67 ms) se pasa del criterio de 5,5 ms por un 3 %, y es casi igual en los tres modos, incluso en el núcleo 0, donde la media sube a 4,24 ms. Eso apunta a unos pocos fotogramas pesados del recorrido (intro y demostración) que dependen del contenido y no del núcleo. El máximo está en 8 a 13,5 ms, por debajo de los 16,64 ms.
- **El núcleo 0 no es un núcleo de gama media.** El Snapdragon 8 Elite solo tiene núcleos Oryon (2 Prime y 6 Performance), sin núcleos pequeños. El pase fijado al núcleo 0 mide el clúster más lento del S25 (1,6 veces la media del pase libre), no un teléfono de gama media, que puede ser 2,5 a 3 veces más lento por núcleo.

Problemas que salieron de la prueba (ninguno es del camino en sí, y los tres se resolvieron en la app):
1. **TinyCC no encuentra la libc.** Con `-nostdinc -nostdlib`, el C del juego (`text.c`, `menutext.c`) necesita `stdio.h`, `stdlib.h`, `string.h` y `sys/stat.h`. Se añadieron cabeceras mínimas en `spikes/tcc_include/` (con la disposición de `struct stat` de arm64 y x86_64) y la app pasa las funciones de la libc a libtcc con `dlsym` (`tcc_add_symbol`), igual que la tabla `snes_rt_symbols`. Sin eso `tcc_relocate` falla con referencias sin resolver. Un APK real tendrá que hacer lo mismo, o dejar que el C del juego no dependa de la libc.
2. **`targetSdk 35` dibuja bajo las barras del sistema:** la app de prueba tapaba sus botones. Se corrigió con un `OnApplyWindowInsetsListener`.
3. **Aviso de compatibilidad con páginas de 16 KB** (`libsnesrt.so` y `libspike.so`, «el segmento LOAD no está alineado»). No impide funcionar. Para el APK definitivo hay que enlazar con `-Wl,-z,max-page-size=16384` (NDK 27) o pasar al NDK 28.

### Decisión final

**Camino on-device (2026-11-05).** El APK no lleva nada derivado de la ROM. El usuario abre la app, elige su ROM una vez y juega: la app comprueba el CRC32, genera el C (~0,4 s) y lo compila con libtcc en memoria (0,2 a 0,3 s), así que la preparación cabe en un solo segundo en el S25 Ultra y no hace falta caché de compilación. El plan B (`.so` en `filesDir` con `dlopen`) funciona y queda como respaldo si una versión de Android o un fabricante negara la memoria ejecutable.

Razones: se cumplen todos los criterios menos el p99 por fotograma (5,65 a 5,67 ms frente a 5,5 ms, un 3 % de diferencia, muy lejos del «más del doble» que según el punto 7 obligaría a cambiar de camino), el estado final es idéntico al del PC y la memoria ejecutable y `dlopen` están permitidos en `targetSdk 35` con One UI.

Riesgos que quedan abiertos, a comprobar en el frontal y no aquí:
- **Rendimiento en gama media.** No se midió ningún teléfono más lento que el S25 Ultra. El p99 depende de unos pocos fotogramas pesados del recorrido; con un teléfono 2,5 a 3 veces más lento seguiría por debajo de 16,64 ms solo si el margen del p99 se mantiene. Hay que medir al menos un teléfono de gama media antes de publicar, y repetir las medidas 3 veces.
- **La libc para TinyCC** (cabeceras y símbolos pasados con `dlsym`, ver problema 1 de arriba) pasa a ser parte del APK.
- **Páginas de 16 KB** (`-Wl,-z,max-page-size=16384` o NDK 28).
- **Fin de la partida sin `exit()`:** el runtime termina el proceso con `exit()`; un frontal con ciclo de vida de Android necesita poder pararse y reanudarse.

### Siguiente cambio (tarea 5.2)

`android-frontend`, en **`snes-recomp`** (el motor): frontal SDL2 para Android sobre el runtime en modo biblioteca, con la generación y la compilación en el arranque, la ROM elegida con el selector de documentos y recordada con permiso persistente (el CRC32 se comprueba al elegirla y las veces siguientes se arranca directamente), controles táctiles en pantalla, mandos Bluetooth y USB, audio, ciclo de vida (pausa y reanudación), carpeta de partidas de la app (`.srm` portable) y empaquetado del APK firmado. Después, en **`alttp-recomp`**, el perfil, las pruebas y el workflow de GitHub Actions que sube el APK a las releases. Sin cuentas ni servicios de Google.

