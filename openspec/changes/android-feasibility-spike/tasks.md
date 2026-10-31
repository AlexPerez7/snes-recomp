# Tasks

## 1. Generación en memoria (`in-memory-generation`)

- [x] 1.1 Añadir `[lib]` al crate (`src/lib.rs` reexporta los módulos que hoy declara `main.rs`; los binarios usan la biblioteca). Verificar: `cargo build` y `cargo test` pasan sin cambios en la salida.
- [x] 1.2 Implementar `generate_sources(rom, profile)`, que devuelve la lista de fuentes (rom_*.c, snes_funcs.h, snes_entry.c, game/*) o un error, sin tocar el disco. Incluye la comprobación del CRC32 y de los ganchos. Verificar: tests unitarios con una ROM sintética (fuentes esperadas, CRC ajeno, gancho sin función) y un test que corre con el directorio actual de solo lectura.
- [x] 1.3 Hacer que `port.rs::generate` y `codegen --all` usen `generate_sources` y solo escriban el resultado. Verificar: con el juego real, los SHA-256 de los 32 ficheros que escribe `codegen --all` coinciden con los de antes del cambio (comprobación única), y un test de integración (`tests/codegen_all.rs`, con `SNES_GAME_PROFILE` y `SNES_GAME_ROM`) compara byte a byte lo escrito con lo que devuelve la biblioteca.
- [x] 1.4 Añadir la interfaz `extern "C"` (`snes_gen_generate`, `snes_gen_error`, `snes_gen_count`, `snes_gen_name`, `snes_gen_text`, `snes_gen_free`) como `cdylib`, con el perfil recibido como texto del `.toml` más sus ficheros C. Verificar: un test de Rust la llama a través de la ABI C y obtiene las mismas fuentes que 1.2, y `nm -D` del `.so` de Linux muestra los seis símbolos.
- [ ] 1.5 Compilar la biblioteca para `aarch64-linux-android` y documentar el comando en `spikes/android/README.md`. Como `staticlib` no necesita el enlazador del NDK: hecho aquí, y el `.a` exporta los seis símbolos (`llvm-nm`). La app la enlaza dentro de `libspike.so` con CMake (tarea `cargoGenerator` de Gradle). Verificar: falta la parte dinámica, que necesita el NDK (en este entorno no se puede descargar); se cierra con 4.1, cuando `libspike.so` exista y `llvm-nm -D` muestre `Java_io_github_alexperez7_snesspike_Native_generate`.

## 2. Runtime en modo biblioteca (`runtime-library`)

- [x] 2.1 Sustituir la llamada directa a `snes_call_entry` por un puntero registrado (`snes_rt_register_entry`). `snes_entry.c` lo inicializa en la construcción de una pieza. Sin registro, el runtime termina con un error que nombra el registro que falta. Verificar: los tests de `src/headless.rs` y `codegen.rs` pasan, y un test nuevo comprueba el error sin módulo.
- [x] 2.2 Añadir la macro `SNES_RT_LIBRARY` (sin `main`, funciones de `snes_rt.h` exportadas) y una entrada de prueba que carga el módulo del juego compilado aparte. Verificar: el runtime compila como biblioteca sin los `rom_*.c` y sin símbolos del juego sin resolver (test con el compilador disponible).
- [x] 2.3 Test de equivalencia con el juego real: 1200 fotogramas con `--press START@1100-1110` en la construcción de una pieza y en la de biblioteca más módulo, comparando `--dump` byte a byte. Verificar: el test pasa (`src/rtlib.rs`, gcc o clang en Linux), y se omite solo si falta el juego o el compilador. Los objetos del juego se compilan una sola vez en paralelo: unos 80 s.
- [x] 2.4 Pasar `scan` con la ROM real sobre la biblioteca del runtime construida. Verificar: no encuentra fragmentos de la ROM (lo hace el test de 2.3).
- [x] 2.5 Actualizar `CLAUDE.md` (sección Estructura) con el modo biblioteca y el registro del punto de entrada. Verificar: el comando documentado compila tal como está escrito.
- [x] 2.6 Contextos de ejecución sobre hilos (`SNES_RT_THREAD_CONTEXTS`, automático en Android, porque bionic no tiene `makecontext` ni `swapcontext`). Verificar: el test de 2.3 compara también una tercera construcción con hilos y los volcados son idénticos.
- [x] 2.7 Tabla de símbolos del runtime en modo biblioteca (`snes_rt_symbols`) para `tcc_add_symbol`. Verificar: un test comprueba que la tabla tiene exactamente las funciones no `inline` de `snes_rt.h`.
- [x] 2.8 `--frame-times FICHERO` en el runtime sin vídeo. Verificar: el test pequeño de `src/rtlib.rs` comprueba una línea por fotograma y el código 2 si el fichero no se puede crear.

## 3. Medición en el PC

- [x] 3.1 Construir el runtime sin vídeo con el C del juego compilado por gcc/clang -O2 y por TinyCC, y medir el tiempo por fotograma (media y p99) en 3000 fotogramas con un recorrido fijo (sin entradas: intro y demostración). Verificar: la tabla queda en `design.md` › Resultados, con la máquina, el compilador y la versión de cada uno (`spikes/bench_pc.sh`).
- [x] 3.2 Programa de prueba con libtcc en memoria (x86_64): compila las fuentes devueltas por `generate_sources`, registra los símbolos del runtime, enlaza con `tcc_relocate` y ejecuta los mismos 3000 fotogramas. Verificar: los volcados `--dump` coinciden con los de 3.1, y se anotan el tiempo de compilación y la memoria pico.
- [x] 3.3 Compilar todo el C del juego con `tcc` cruzado a arm64 (sin ejecutar). Verificar: compila sin errores, o se lista cada error con su fichero y función en `design.md` › Resultados.
- [x] 3.4 Primera decisión: si TinyCC en el PC deja menos margen del que exige la tabla del punto 7 de `design.md` (escalado al teléfono), anotar el paso al plan B y saltar al grupo 5. Verificar: la decisión y su razón quedan escritas en Resultados.

## 4. App de prueba en el S25 Ultra (`spikes/android/`, no se publica)

- [ ] 4.1 Proyecto Android mínimo (Gradle, NDK, solo `arm64-v8a`, targetSdk 35, sin permiso de Internet) con el runtime en modo biblioteca compilado con clang -O2, libtcc compilado para arm64 y el generador. El proyecto ya está escrito (`spikes/android/`, ver su README). Su JNI (`spike_jni.c`) y `Native.java` se probaron en Linux: generar, medir en memoria y plan B, con los mismos hashes que el PC. Verificar: `gradlew assembleDebug` termina en el PC del usuario y la app se instala con `adb install`.
- [ ] 4.2 Leer la ROM con el selector de documentos (SAF), comprobar el CRC32, generar las fuentes con la biblioteca y guardarlas en la caché de la app. Verificar: el registro (`adb logcat`) muestra el CRC32, el número de fuentes y el tiempo de generación.
- [ ] 4.3 Compilar en memoria con libtcc, registrar el punto de entrada y ejecutar 3000 fotogramas sin presentar imagen, registrando el tiempo por fotograma (media y p99) y la memoria pico. Repetir 3 veces, y además fijando el hilo a un núcleo secundario con `sched_setaffinity`. Verificar: las medidas aparecen en `logcat` y se copian a Resultados.
- [ ] 4.4 Probar el plan B: que tcc escriba un `.so` en `filesDir` y cargarlo con `dlopen`, y anotar si Android lo permite y el tiempo de arranque con el `.so` en caché. Verificar: el resultado (funciona o el error exacto) queda en Resultados.
- [ ] 4.5 Comprobar que el estado tras los 3000 fotogramas en el teléfono coincide con el del PC (hash de WRAM y VRAM en `logcat`). Verificar: los hashes coinciden con los de 3.1.
- [x] 4.6 Pasar `scan` con la ROM sobre `spikes/` (fuentes) antes de subir nada. Verificar: no hay fragmentos de la ROM y en el árbol no hay ningún `.sfc`, `.apk` ni C generado (`.gitignore` excluye además `*.apk` y lo construido por Gradle).

## 5. Decisión

- [ ] 5.1 Completar `design.md` › Resultados con todas las tablas y aplicar los criterios del punto 7: camino on-device, plan B (paquete compilado en el PC más `dlopen`) o solo APK personal. Verificar: la decisión está escrita con las medidas que la justifican, y el usuario la revisó.
- [ ] 5.2 Proponer en una nota en Resultados el siguiente cambio según la decisión (por ejemplo, el frontal Android del motor y el APK de `alttp-recomp`). Verificar: la nota nombra el cambio y su repositorio.

## Workflow follow-up

- Archivar el cambio con `/opsx:archive` cuando la decisión esté tomada y revisada.
- Actualizar en `CLAUDE.md` la línea «Último cambio archivado».
