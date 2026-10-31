# Tasks

## 1. Generación en memoria (`in-memory-generation`)

- [ ] 1.1 Añadir `[lib]` al crate (`src/lib.rs` reexporta los módulos que hoy declara `main.rs`; los binarios usan la biblioteca). Verificar: `cargo build` y `cargo test` pasan sin cambios en la salida.
- [ ] 1.2 Implementar `generate_sources(rom, profile)`, que devuelve la lista de fuentes (rom_*.c, snes_funcs.h, snes_entry.c, game/*) o un error, sin tocar el disco. Incluye la comprobación del CRC32 y de los ganchos. Verificar: tests unitarios con una ROM sintética (fuentes esperadas, CRC ajeno, gancho sin función) y un test que corre con el directorio actual de solo lectura.
- [ ] 1.3 Hacer que `port.rs::generate` y `codegen --all` usen `generate_sources` y solo escriban el resultado. Verificar: con el juego real (`SNES_GAME_PROFILE`, `SNES_GAME_ROM`), un test compara byte a byte lo que escribe `codegen --all` con lo que se escribía antes del cambio, guardado como hash por fichero.
- [ ] 1.4 Añadir la interfaz `extern "C"` (`snes_gen_generate`, `snes_gen_free`) como `cdylib`, con el perfil recibido como texto del `.toml` más sus ficheros C. Verificar: un test de Rust la llama a través de la ABI C y obtiene las mismas fuentes que 1.2.
- [ ] 1.5 Compilar el `cdylib` para `aarch64-linux-android` con el enlazador del NDK y documentar el comando en `spikes/android/README.md`. Verificar: el `.so` existe y `llvm-nm -D` muestra `snes_gen_generate`.

## 2. Runtime en modo biblioteca (`runtime-library`)

- [ ] 2.1 Sustituir la llamada directa a `snes_call_entry` por un puntero registrado (`snes_rt_register_entry`). `snes_entry.c` lo inicializa en la construcción de una pieza. Sin registro, el runtime termina con un error que nombra el registro que falta. Verificar: los tests de `src/headless.rs` y `codegen.rs` pasan, y un test nuevo comprueba el error sin módulo.
- [ ] 2.2 Añadir la macro `SNES_RT_LIBRARY` (sin `main`, funciones de `snes_rt.h` exportadas) y una entrada de prueba que carga el módulo del juego compilado aparte. Verificar: el runtime compila como biblioteca sin los `rom_*.c` y sin símbolos del juego sin resolver (test con el compilador disponible).
- [ ] 2.3 Test de equivalencia con el juego real: 1200 fotogramas con `--press START@1100-1110` en la construcción de una pieza y en la de biblioteca más módulo, comparando `--dump` byte a byte. Verificar: el test pasa, y se omite solo si falta el juego o el compilador.
- [ ] 2.4 Pasar `scan` con la ROM real sobre la biblioteca del runtime construida. Verificar: no encuentra fragmentos de la ROM.
- [ ] 2.5 Actualizar `CLAUDE.md` (sección Estructura) con el modo biblioteca y el registro del punto de entrada. Verificar: el comando documentado compila tal como está escrito.

## 3. Medición en el PC

- [ ] 3.1 Construir el runtime sin vídeo con el C del juego compilado por gcc/clang -O2 y por TinyCC, y medir el tiempo por fotograma (media y p99) en 3000 fotogramas con un recorrido fijo de `--press`. Verificar: la tabla queda en `design.md` › Resultados, con la máquina, el compilador y la versión de cada uno.
- [ ] 3.2 Programa de prueba con libtcc en memoria (x86_64): compila las fuentes devueltas por `generate_sources`, registra los símbolos del runtime, enlaza con `tcc_relocate` y ejecuta los mismos 3000 fotogramas. Verificar: los volcados `--dump` coinciden con los de 3.1, y se anotan el tiempo de compilación y la memoria pico.
- [ ] 3.3 Compilar todo el C del juego con `tcc` cruzado a arm64 (sin ejecutar). Verificar: compila sin errores, o se lista cada error con su fichero y función en `design.md` › Resultados.
- [ ] 3.4 Primera decisión: si TinyCC en el PC deja menos margen del que exige la tabla del punto 7 de `design.md` (escalado al teléfono), anotar el paso al plan B y saltar al grupo 5. Verificar: la decisión y su razón quedan escritas en Resultados.

## 4. App de prueba en el S25 Ultra (`spikes/android/`, no se publica)

- [ ] 4.1 Proyecto Android mínimo (Gradle, NDK, solo `arm64-v8a`, targetSdk 35, sin permiso de Internet) con el runtime en modo biblioteca compilado con clang -O2, `libtcc.so` compilada para arm64 y el `.so` del generador. Verificar: `./gradlew assembleDebug` termina en el PC del usuario y la app se instala con `adb install`.
- [ ] 4.2 Leer la ROM con el selector de documentos (SAF), comprobar el CRC32, generar las fuentes con la biblioteca y guardarlas en la caché de la app. Verificar: el registro (`adb logcat`) muestra el CRC32, el número de fuentes y el tiempo de generación.
- [ ] 4.3 Compilar en memoria con libtcc, registrar el punto de entrada y ejecutar 3000 fotogramas sin presentar imagen, registrando el tiempo por fotograma (media y p99) y la memoria pico. Repetir 3 veces, y además fijando el hilo a un núcleo secundario con `sched_setaffinity`. Verificar: las medidas aparecen en `logcat` y se copian a Resultados.
- [ ] 4.4 Probar el plan B: que tcc escriba un `.so` en `filesDir` y cargarlo con `dlopen`, y anotar si Android lo permite y el tiempo de arranque con el `.so` en caché. Verificar: el resultado (funciona o el error exacto) queda en Resultados.
- [ ] 4.5 Comprobar que el estado tras los 3000 fotogramas en el teléfono coincide con el del PC (hash de WRAM y VRAM en `logcat`). Verificar: los hashes coinciden con los de 3.1.
- [ ] 4.6 Pasar `scan` con la ROM sobre `spikes/android/` (fuentes) antes de subir nada. Verificar: no hay fragmentos de la ROM y en el árbol no hay ningún `.sfc`, `.apk` ni C generado.

## 5. Decisión

- [ ] 5.1 Completar `design.md` › Resultados con todas las tablas y aplicar los criterios del punto 7: camino on-device, plan B (paquete compilado en el PC más `dlopen`) o solo APK personal. Verificar: la decisión está escrita con las medidas que la justifican, y el usuario la revisó.
- [ ] 5.2 Proponer en una nota en Resultados el siguiente cambio según la decisión (por ejemplo, el frontal Android del motor y el APK de `alttp-recomp`). Verificar: la nota nombra el cambio y su repositorio.

## Workflow follow-up

- Archivar el cambio con `/opsx:archive` cuando la decisión esté tomada y revisada.
- Actualizar en `CLAUDE.md` la línea «Último cambio archivado».
