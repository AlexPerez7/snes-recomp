# Tasks

## 1. Base de la app

Proyecto en `android/` (2026-11-05): compila con `gradle assembleDebug` (Gradle 8.9), se instala en el S25 Ultra, `libmain.so` y `libSDL2.so` alineadas a 16 KB (`p_align` 0x4000) y el juego llega a la pantalla de título generado y compilado en el teléfono (generar 449 ms, compilar y enlazar 170 ms). `LauncherActivity` (selector de ROM con CRC32) y `GameActivity` (SDL) están escritas pero el selector aún no se ha probado a mano; la prueba se hizo dejando la ROM en `files/rom.sfc`. Sin sonido ni controles probados todavía.

- [x] 1.1 Proyecto Gradle definitivo (SDL2 `SDLActivity`, NDK 27 o 28, `arm64-v8a`, sin permiso de Internet, sin `debuggable` en release). Verificar: `gradlew assembleRelease` termina y el APK se instala con `adb install`.
- [x] 1.2 Libc para TinyCC: llevar `spikes/tcc_include/` a la app y la tabla de símbolos de la libc con `dlsym`. Verificar: el C del juego compila en el teléfono sin referencias sin resolver (como en la prueba).
- [x] 1.3 Biblioteca nativa con el runtime en modo biblioteca, la biblioteca del generador y libtcc; alinear a 16 KB. Verificar: `llvm-readelf` muestra `p_align` 0x4000 en los `LOAD` de cada `.so` y Android no muestra el aviso de compatibilidad.

## 2. Arranque de usuario

- [x] 2.1 Selector de ROM con SAF, copia a `filesDir`, comprobación del CRC32 y mensaje para una ROM desconocida con los dos CRC32. Probado a mano en el S25 Ultra (2026-11-05, datos de la app borrados antes): una ROM aleatoria de 1 MiB se rechaza con «CRC32 29F0D5ED, se esperaba 777AAC2F»; la ROM buena arranca y llega a la pantalla de título.
- [x] 2.2 ROM recordada: la copia queda en `filesDir/rom.sfc` (no hace falta permiso persistente del SAF). Probado en el S25 Ultra: `force-stop` y abrir de nuevo arranca el juego sin pedir la ROM.
- [ ] 2.3 Generar y compilar en el arranque, con una pantalla de progreso. Verificar: registro con tiempos y memoria; hashes iguales a los del PC tras 3000 fotogramas.

## 3. Frontal

- [x] 3.1 `play.c` en Android con `SDL_Renderer`: imagen y audio. Probado por adb en el S25 Ultra (2026-11-05): 600 fotogramas cada 9,98 s (60,1 fps) durante 4 minutos; la pista de audio de la app (OpenSL ES, 32 kHz estéreo) está activa, con volumen 1,0 y 0 *underruns* en `dumpsys media.audio_flinger`. Que se oiga de verdad no se puede comprobar por adb.
- [x] 3.2 Ciclo de vida. Probado por adb en el S25 Ultra: con HOME el audio pasa a pausado y el juego se detiene (sin registros de fotogramas); al volver sigue el mismo proceso y la partida donde estaba; girar a horizontal y volver a vertical no reinicia nada (mismo PID, los táctiles cambian de disposición). Sin `exit()` en la salida normal. La SRAM la escribe el propio juego al guardar, y `SDL_APP_WILLENTERBACKGROUND` llama a `srm_save` (no se ha podido ver un guardado que solo ocurra al pausar, porque el juego ya escribe al guardar).
- [~] 3.3 Controles táctiles y mandos SDL. **Táctiles hechos (2026-11-05)**: `runtime/play_touch.h` (diseño por orientación, pruebas de pulsación, `touch.ini`, arte pixel art dorado y verde con la Trifuerza en la cruceta y los cuatro botones en los colores de la consola) y su integración en `play.c`: dibujo sobre la imagen con `SDL_Renderer`, manejo del menú con el táctil, botón Atrás de Android, pestaña **Touch** en los ajustes (tamaño, opacidad, vibración, editar y restaurar la disposición) y editor para arrastrar y escalar cada control. Comprobado en el S25 Ultra con toques de `adb`: abrir el menú, navegar hasta la pestaña Touch, editar (arrastrar y escalar el botón A), guardar y recuperar la disposición al reiniciar, y START en el título lleva a «PLAYER SELECT». Test en `src/headless.rs`. Descubierto y corregido: con contextos sobre hilos, el contexto GL de SDL se quedaba atado a un hilo y la pantalla dejaba de actualizarse (`EGL_BAD_ACCESS`); el hilo saliente lo suelta (`snes_rt_ctx_release`). **Falta**: probar a mano con dedos (varios a la vez, diagonales de la cruceta, vibración) y con un mando Bluetooth (los táctiles se ocultan). Verificar: se juega la intro y se llega a manejar a Link con los táctiles y con un mando Bluetooth.
- [x] 3.4 Carpeta de partidas de la app (`filesDir/saves`). Probado: se registra un jugador, `rom.srm` (8192 bytes, SRAM cruda como en escritorio) queda en `files/saves`, y tras `force-stop` y abrir de nuevo el jugador sigue en la ranura 1. Falta abrir ese `.srm` en el PC.

## 4. Calidad y entrega

Medición (2026-11-05): con un fichero `files/stats` en la carpeta de la app (`adb shell run-as io.github.alexperez7.snesport touch files/stats`) el runtime imprime en logcat (`adb logcat -s snesport`) cada 600 fotogramas el tiempo de trabajo por fotograma (`work: p50, p99, max`). Primera lectura en el S25 Ultra con los táctiles y la pantalla encendida: p50 10,5 ms, p99 15,4 ms, que no es comparable con los 5,65 ms de la prueba (aquí cuenta también la presentación del fotograma, que puede esperar a la sincronización vertical); hay que separar emulación y presentación antes de dar la cifra de 4.1.

- [ ] 4.1 Medir el p99 por fotograma con la app completa, 3 repeticiones, en el S25 Ultra y en al menos un teléfono de gama media. Verificar: tabla en `design.md › Resultados` con los modelos.
- [ ] 4.2 GLES 3 si 4.1 lo pide; si no, dejar `SDL_Renderer`.
- [ ] 4.3 APK firmado, licencia y fuentes de libtcc dentro de la app, `scan` sin hallazgos. Verificar: el APK no contiene nada derivado de la ROM.
- [ ] 4.4 Documentar en `CLAUDE.md` y en el README cómo compilar y firmar el APK.

## Workflow follow-up

- En `alttp-recomp`: perfil, pruebas y workflow de GitHub Actions que sube el APK a las releases (cambio aparte).
- Archivar con `/opsx:archive` y actualizar en `CLAUDE.md` la línea «Último cambio archivado».
