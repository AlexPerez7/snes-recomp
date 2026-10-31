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

**4. API de generación: `generate_sources(rom: &[u8], profile: &GameProfile) -> Result<Vec<Source>, String>`.**
Va en un módulo de la biblioteca nueva (`src/lib.rs` reexporta los módulos que hoy declara `main.rs`). `port.rs` y `codegen --all` la usan y solo escriben el resultado. Para Android, una capa `extern "C"` mínima (`snes_gen_generate`, `snes_gen_free`) entrega un arreglo de pares nombre/texto y un mensaje de error. Esa capa compila como `cdylib` con el target `aarch64-linux-android`. El perfil llega como texto del `.toml` más los ficheros de su C, porque en el teléfono no hay catálogo de carpetas.
- *Alternativa descartada:* llamar al ejecutable `snes-recomp` desde la app. Android no deja ejecutar binarios del directorio de datos, y empaquetarlo como «lib» para ejecutarlo sería un truco frágil.

**5. Compilación en el teléfono: libtcc en memoria.**
`tcc_new`, después `tcc_set_output_type(TCC_OUTPUT_MEMORY)`, un `tcc_compile_string` por fuente (los `#include` de `snes_rt.h` y `snes_funcs.h` se resuelven con un directorio de inclusión que la app prepara en su caché), `tcc_add_symbol` para cada función del runtime, `tcc_relocate` y por último `tcc_get_symbol("snes_rt_module_init")`, que registra el punto de entrada. libtcc se compila para arm64 con el NDK y viaja como `libtcc.so` aparte (LGPL 2.1).
- *Alternativa medida en paralelo:* que tcc escriba un `.so` en `filesDir` y cargarlo con `dlopen`. Si funciona en el S25 con targetSdk 35, el resultado se podría cachear entre arranques. Si no, cada arranque recompila en memoria, y el tiempo de compilación pasa a contar en cada inicio.

**6. Primero se mide en el PC, después en el teléfono.**
En Linux/Windows: el runtime sin vídeo con el C compilado por tcc frente a gcc/clang -O2 (`--frames 3000` con un recorrido fijo de `--press`), y el tiempo y la memoria pico de libtcc compilando las fuentes en memoria. Si en el PC TinyCC ya no deja margen, el teléfono no lo va a salvar y se pasa directamente al plan B sin construir el APK de la prueba.

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
- [El S25 Ultra oculta problemas de la gama media] → Los criterios exigen un margen de 3 veces. Además, se repite la medición fijando el hilo a un solo núcleo que no sea de los principales, con `sched_setaffinity`, como aproximación.
- [Licencia de libtcc (LGPL 2.1)] → Se enlaza como `.so` aparte, nunca de forma estática. El APK de la prueba no se publica. La entrega de la licencia y las fuentes queda para el APK definitivo en `alttp-recomp`.
- [Que la generación en el teléfono use mucha memoria por el descubrimiento y el grafo] → Se mide junto con la compilación. Como el C se cachea por versión, ese costo solo se paga una vez.
- [Que el refactor del runtime rompa el PC] → La construcción de una pieza sigue siendo la de siempre. El test de equivalencia del spec `runtime-library` compara los volcados `--dump` de las dos formas.

## Migration Plan

No hay migración para los usuarios: en el PC el ejecutable y la caché no cambian, salvo que `current_version` cambia porque cambia el runtime, y eso dispara una reconstrucción automática como en cualquier versión nueva. Para deshacerlo, basta revertir el cambio. La app de la prueba se puede borrar sin efectos.

## Open Questions

- La versión exacta de TinyCC a fijar: la misma que descarga `download.rs` o la rama `mob`, según cuál funcione mejor en arm64. Se decide al compilar libtcc, y no cambia ni el enfoque ni las tareas.

## Resultados

<!-- Se completa al aplicar el cambio: tablas de medidas en el PC y en el S25 Ultra, y la decisión según el punto 7. -->
