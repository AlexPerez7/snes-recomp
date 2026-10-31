# Proposal

## Why

Queremos llevar *A Link to the Past* a Android como un APK que se pueda publicar en las releases de GitHub del repositorio `alttp-recomp`. Como en el PC, el usuario aporta su ROM y el juego se arma en su dispositivo, por lo que el APK no contiene nada derivado de la ROM. Para eso el teléfono tiene que generar el C y compilarlo él mismo (libtcc), algo que todavía no se ha probado. Antes de invertir en el frontal móvil (controles táctiles, ciclo de vida, empaquetado) necesitamos saber si este camino es viable en tiempo de preparación, memoria y fotogramas por segundo. Si no lo es, hay que activar el plan B: un paquete de juego compilado en el PC y cargado por el mismo APK.

## What Changes

- **Runtime separable del código generado**: el runtime (bus, DMA, PPU, SPC700/DSP, frontal) puede compilarse como biblioteca propia, sin el C generado. Las dos partes se unen solo a través de `snes_rt.h` y de un punto de entrada que el código generado registra al arrancar (hoy `snes_call_entry` se resuelve al enlazar). En el PC sigue funcionando igual que ahora y el ejecutable resultante no cambia.
- **Generación en memoria**: una API del generador que recibe los bytes de la ROM y el perfil del juego, y devuelve las fuentes C (nombre y texto) sin escribir en disco. `snes-port` y `codegen --all` pasan a usarla. El crate expone una biblioteca (`[lib]`) que compila también para `aarch64-linux-android`.
- **Prueba de factibilidad en Android (no se publica)**: una app mínima de uso personal, en `spikes/android/`, que en el teléfono del usuario (Samsung Galaxy S25 Ultra):
  - lee la ROM con el selector de documentos;
  - genera el C con la biblioteca del generador;
  - lo compila en memoria con libtcc para arm64;
  - lo enlaza contra el runtime compilado con clang -O2;
  - ejecuta fotogramas sin presentar imagen y registra las medidas.

  También prueba si Android deja cargar con `dlopen` un `.so` desde el directorio de datos de la app, que es lo que necesita el plan B.
- **Medición previa en el PC**: tiempo por fotograma del runtime sin vídeo con el C generado compilado por TinyCC frente a gcc/clang -O2, y tiempo y memoria que necesita libtcc para compilar todo el C del juego.
- **Informe de decisión**: los resultados se registran en `design.md` junto con la decisión (camino on-device, plan B o ninguno), según criterios fijados de antemano que dejan margen para teléfonos de gama media.

Fuera de alcance: controles táctiles, frontal SDL en Android, ciclo de vida, firma de release, CI y cualquier publicación. El APK final, su perfil y su workflow de release vivirán en `alttp-recomp`, en cambios posteriores.

## Capabilities

### New Capabilities
- `runtime-library`: el runtime se compila y enlaza aparte del código generado. La unión se hace por la interfaz de `snes_rt.h` y por un registro del punto de entrada en tiempo de ejecución, de modo que el código del juego pueda venir de otro módulo (compilado en memoria o cargado como biblioteca).
- `in-memory-generation`: el generador está disponible como biblioteca que produce las fuentes C de un juego a partir de los bytes de la ROM y un perfil, sin tocar el disco, y compila para Android arm64.

### Modified Capabilities
<!-- Ninguna: `port-builder` y `c-code-generation` mantienen su comportamiento observable; solo cambian por dentro. -->

## Impact

- `runtime/snes_core.inc` y `runtime/snes_rt.h`: el despacho por dirección pasa por un puntero registrado; macro de compilación para el modo biblioteca.
- `src/port.rs` y `src/main.rs` (`generate`, `write_sources`): pasan a usar la API en memoria.
- `Cargo.toml`: se añade un objetivo `[lib]` (rlib y cdylib).
- Nuevo directorio `spikes/android/` con el proyecto Android de la prueba (Gradle, NDK y SDL2 no hacen falta todavía). No contiene nada de la ROM y, por precaución, se pasa `scan` antes de subirlo.
- Dependencias de la prueba, solo en la máquina del usuario: Android SDK y NDK, target de Rust `aarch64-linux-android` y fuentes de TinyCC (LGPL 2.1, como `.so` aparte).
- Sin cambios en el repositorio `alttp-recomp` en este cambio.
