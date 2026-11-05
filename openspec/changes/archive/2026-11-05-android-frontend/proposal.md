# Proposal

## Why

La prueba `android-feasibility-spike` (2026-11-05, Galaxy S25 Ultra) demostró que el teléfono puede generar el C del juego desde la ROM del usuario y compilarlo con libtcc en menos de un segundo, con el mismo estado final que el PC, y que Android permite tanto la memoria ejecutable como `dlopen` desde `filesDir`. Falta el producto: una app que un usuario normal instale, abra, elija su ROM una vez y juegue, sin consola ni cuentas ni servicios de Google, repartida como APK en las releases de GitHub.

## What Changes

- **Frontal Android**: el frontal de `runtime/play.c` (SDL2) funciona en Android con el runtime en modo biblioteca, sin `exit()` como forma normal de terminar, con pausa y reanudación del ciclo de vida de Android y con el audio de SDL.
- **Arranque de usuario**: la app pide la ROM con el selector de documentos, comprueba el CRC32 y, si es una ROM conocida (perfil incluido en el APK), genera el C (`snes_gen_*`), lo compila en memoria con libtcc y arranca. La ROM elegida se recuerda con permiso persistente y las veces siguientes se juega directamente. Una ROM desconocida se rechaza con un mensaje que nombra el CRC32.
- **Libc para TinyCC**: cabeceras mínimas (`spikes/tcc_include/`, pasan a ser parte de la app) y símbolos de la libc entregados a libtcc con `dlsym`, como hizo la prueba.
- **Imagen**: OpenGL ES 3 (el desktop usa GL 3.3 core) con el camino de `SDL_Renderer` como respaldo; relación de aspecto y pantalla ancha según la proporción del móvil.
- **Controles táctiles** en pantalla (cruceta, A/B/X/Y, L/R, Start/Select) con tamaño y posición ajustables y vibración opcional; los mandos SDL (Bluetooth y USB) siguen funcionando y ocultan los táctiles al usarse.
- **Partidas**: carpeta propia de la app (`filesDir/saves`), `.srm` y estados como en escritorio.
- **Empaquetado**: proyecto Gradle definitivo, APK firmado y alineado a 16 KB (`-Wl,-z,max-page-size=16384` o NDK 28), sin permiso de Internet.
- **Medición en gama media**: medir el p99 por fotograma en al menos un teléfono más lento que el S25 Ultra y repetir las medidas 3 veces antes de publicar.

Fuera de alcance: sincronización de partidas en la nube (descartada), Play Store, iOS, y los ajustes finos del motor (shaders, rebobinado en pantalla táctil) que irán en cambios posteriores. El workflow de release y el perfil del juego viven en `alttp-recomp`, en un cambio aparte.

## Capabilities

### New Capabilities
- `android-app`: la app Android que genera, compila y ejecuta el juego a partir de la ROM del usuario, con ciclo de vida y ROM recordada.
- `touch-controls`: controles táctiles en pantalla y su convivencia con los mandos.

### Modified Capabilities
- `live-video-window`: también presenta con OpenGL ES 3 y con `SDL_Renderer` cuando no hay GL 3.3 core.
- `sram-persistence`: en Android las partidas van a la carpeta de la app.

## Impact

- `runtime/play.c`, `play_gl.h` y `play_cfg.h`: contexto GLES, terminación sin `exit()`, eventos de ciclo de vida y de toque.
- `spikes/android/` pasa a un proyecto definitivo (nombre y ubicación a decidir en el diseño) que consume la biblioteca del generador y el runtime en modo biblioteca; la app de la prueba se conserva como referencia.
- Dependencias en la máquina de quien compila el APK: Android SDK y NDK, Rust con `aarch64-linux-android`, fuentes de TinyCC (LGPL 2.1, como `.so` aparte; hay que entregar licencia y fuentes) y SDL2 para Android.
- `alttp-recomp`: sin cambios en este cambio.
