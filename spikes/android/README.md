# App de prueba de factibilidad en Android (`android-feasibility-spike`)

App mínima, **de uso personal y que no se publica**, para medir en el teléfono si el camino «el teléfono genera el C desde la ROM del usuario y lo compila con libtcc» sirve para un APK publicable. Los criterios y los resultados están en `openspec/changes/android-feasibility-spike/design.md`.

Qué hace, con botones:

1. **Elegir ROM**: la copia al almacenamiento privado de la app (`files/rom.sfc`). Nunca sale del teléfono, y la app no tiene permiso de Internet.
2. **Generar el C**: la biblioteca del generador (Rust, `src/generate.rs`) produce el C del juego con el perfil incluido en la app. Comprueba el CRC32 y deja las fuentes en `files/src`.
3. **Medir en memoria**: libtcc compila ese C en memoria, lo une al runtime (`libsnesrt.so`, compilado con clang -O2) y ejecuta 3000 fotogramas sin vídeo.
4. **Lo mismo fijando el hilo al núcleo 0**: en el Snapdragon 8 Elite, los núcleos 0 a 5 son los del clúster más lento. Es una aproximación a un teléfono de gama media.
5. **Plan B**: tcc escribe `files/run_so/game.so` y se carga con `dlopen`. Sirve para saber si Android lo permite.

Cada medición se ejecuta en un proceso aparte (`:bench`), porque el runtime termina con `exit()`. La app vuelve sola a la pantalla principal y **Ver resultados** muestra cada `result.txt`. Lo mismo sale por `adb logcat -s snesspike`.

## Lo que hace falta en el PC

- Android Studio, o los command-line tools, con el **NDK 27.2.12479018** y **CMake 3.22.1** (SDK Manager › SDK Tools).
- Rust con el target de Android: `rustup target add aarch64-linux-android`. La compilación ejecuta `cargo rustc ... --crate-type staticlib`, que no necesita el enlazador del NDK.
- **TinyCC** de la rama `mob`, clonado **sin ejecutar `./configure`**, porque su `config.h` sería el del PC. La prueba se hizo con el commit `e23dabe`:
  ```
  cd C:\ruta\donde\estan\snes-recomp\..
  git clone https://github.com/TinyCC/tinycc.git
  ```
  Así queda como hermano de `snes-recomp` y `alttp-recomp`, que es lo que espera `gradle.properties`. Si no, cambie `snesSpike.tinyccDir`.
- El perfil del juego en `..\alttp-recomp\games\alttp_usa.toml` (`snesSpike.gameProfile`). Se incluye en la app; es código propio y no deriva de la ROM.

## Compilar e instalar

Desde Android Studio: **File › Open** sobre `spikes/android`. Si falta el *Gradle wrapper*, Android Studio lo crea y ofrece actualizar el plugin de Android si hay uno más nuevo; aceptarlo no cambia nada de la prueba. Después: **Run** con el teléfono conectado por USB y la depuración USB activada.

Desde la consola, con Gradle instalado:

```
cd spikes\android
gradle wrapper
gradlew assembleDebug
adb install -r app\build\outputs\apk\debug\app-debug.apk
adb logcat -s snesspike
```

La variante *debug* compila el código nativo con `-O2`, así que mide lo mismo que una *release*.

## Qué anotar

Por cada medición, copie a `design.md › Resultados` lo que muestra **Ver resultados**:

- `compilar_ms`, `enlazar_ms` y `memoria_pico_mib`;
- `media_ms`, `p99_ms` y `max_ms` (tiempo por fotograma, sin presentar imagen);
- los cuatro `fnv`, que deben coincidir con los del PC (3000 fotogramas, sin entradas):

  | Fichero | FNV-1a 64 en el PC |
  |---|---|
  | wram.bin | `3a6f2313886e5104` |
  | vram.bin | `1b065eac555a61f9` |
  | cgram.bin | `22537cc0decddbf5` |
  | oam.bin | `00984b3d23dfa740` |

  Estos hashes dependen del generador y del runtime: si alguno de los dos cambia, hay que volver a calcularlos en el PC con `spikes/bench_pc.sh`.

- el mensaje de **Generar el C** (tiempo y memoria).

Si un paso falla, el mensaje aparece en pantalla y el detalle en `logcat`. Por ejemplo, `tcc_relocate falló` significa que Android denegó la memoria ejecutable, y `ERROR plan B: dlopen(...)` que no deja cargar el `.so`.

## En el PC (sin teléfono)

- `spikes/bench_pc.sh <rom> <perfil.toml> <prefijo de TinyCC instalado>`: tiempo por fotograma con gcc -O2, todo con TinyCC, y runtime con gcc más el C del juego con libtcc en memoria (`spikes/tcc_host.c`).
- `spikes/tcc_host.c` es el mismo camino que `spike_jni.c`, pero desde la línea de comandos. Con `SNES_TCC_INCLUDE` usa solo las cabeceras que llevaría el teléfono (las de TinyCC, `spikes/tcc_include/stdint.h` y `snes_rt.h`), sin `libtcc1.a`.

Nada de esta carpeta deriva de la ROM. Antes de subir algo, `cargo run --release -- scan roms/alttp_usa.sfc spikes`.
