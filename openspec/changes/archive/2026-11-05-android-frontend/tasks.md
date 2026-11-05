# Tasks

## 1. Base de la app

Proyecto en `android/` (2026-11-05): compila con `gradle assembleDebug` (Gradle 8.9), se instala en el S25 Ultra, `libmain.so` y `libSDL2.so` alineadas a 16 KB (`p_align` 0x4000) y el juego llega a la pantalla de título generado y compilado en el teléfono (generar 449 ms, compilar y enlazar 170 ms). `LauncherActivity` (selector de ROM con CRC32) y `GameActivity` (SDL) están escritas pero el selector aún no se ha probado a mano; la prueba se hizo dejando la ROM en `files/rom.sfc`. Sin sonido ni controles probados todavía.

- [x] 1.1 Proyecto Gradle definitivo (SDL2 `SDLActivity`, NDK 27 o 28, `arm64-v8a`, sin permiso de Internet, sin `debuggable` en release). Verificar: `gradlew assembleRelease` termina y el APK se instala con `adb install`.
- [x] 1.2 Libc para TinyCC: llevar `spikes/tcc_include/` a la app y la tabla de símbolos de la libc con `dlsym`. Verificar: el C del juego compila en el teléfono sin referencias sin resolver (como en la prueba).
- [x] 1.3 Biblioteca nativa con el runtime en modo biblioteca, la biblioteca del generador y libtcc; alinear a 16 KB. Verificar: `llvm-readelf` muestra `p_align` 0x4000 en los `LOAD` de cada `.so` y Android no muestra el aviso de compatibilidad.

## 2. Arranque de usuario

- [x] 2.1 Selector de ROM con SAF, copia a `filesDir`, comprobación del CRC32 y mensaje para una ROM desconocida con los dos CRC32. Probado a mano en el S25 Ultra (2026-11-05, datos de la app borrados antes): una ROM aleatoria de 1 MiB se rechaza con «CRC32 29F0D5ED, se esperaba 777AAC2F»; la ROM buena arranca y llega a la pantalla de título.
- [x] 2.2 ROM recordada: la copia queda en `filesDir/rom.sfc` (no hace falta permiso persistente del SAF). Probado en el S25 Ultra: `force-stop` y abrir de nuevo arranca el juego sin pedir la ROM.
- [x] 2.3 Generar y compilar en el arranque, con una pantalla de progreso. Verificar: registro con tiempos y memoria; hashes iguales a los del PC tras 3000 fotogramas. Hecho (2026-11-05, S25 Ultra): `logcat` da «generar 432 ms, escribir 7 ms, compilar y enlazar 151 ms, memoria pico 270 MiB» (en otras ejecuciones 216 a 232 MiB); mientras tanto `GameActivity` muestra una pantalla negra con una rueda y «Generando el código del juego (1 de 2)...» y luego «Compilando el juego (2 de 2)...» (comprobado con una captura justo tras elegir la ROM), y se quita al abrir el juego o al fallar. Con `hash=3000` en `files/stats` (3000 fotogramas sin ventana, `language=0`) los cuatro hashes coinciden con los del PC (`snes-port ... -- --no-window --frames 3000 --hash-wram --game-opt language=0`): `wram f1d662c4`, `vram 96928d99`, `cgram fc9bb275`, `oam bbef2160`. `--hash-wram` imprime ahora los cuatro.

## 3. Frontal

- [x] 3.1 `play.c` en Android con `SDL_Renderer`: imagen y audio. Probado por adb en el S25 Ultra (2026-11-05): 600 fotogramas cada 9,98 s (60,1 fps) durante 4 minutos; la pista de audio de la app (OpenSL ES, 32 kHz estéreo) está activa, con volumen 1,0 y 0 *underruns* en `dumpsys media.audio_flinger`. Que se oiga de verdad no se puede comprobar por adb.
- [x] 3.2 Ciclo de vida. Probado por adb en el S25 Ultra: con HOME el audio pasa a pausado y el juego se detiene (sin registros de fotogramas); al volver sigue el mismo proceso y la partida donde estaba; girar a horizontal y volver a vertical no reinicia nada (mismo PID, los táctiles cambian de disposición). Sin `exit()` en la salida normal. La SRAM la escribe el propio juego al guardar, y `SDL_APP_WILLENTERBACKGROUND` llama a `srm_save` (no se ha podido ver un guardado que solo ocurra al pausar, porque el juego ya escribe al guardar).
- [~] 3.3 Controles táctiles y mandos SDL. **Táctiles hechos (2026-11-05)**: `runtime/play_touch.h` (diseño por orientación, pruebas de pulsación, `touch.ini`, arte pixel art dorado y verde con la Trifuerza en la cruceta y los cuatro botones en los colores de la consola) y su integración en `play.c`: dibujo sobre la imagen con `SDL_Renderer`, manejo del menú con el táctil, botón Atrás de Android, pestaña **Touch** en los ajustes (tamaño, opacidad, vibración, editar y restaurar la disposición) y editor para arrastrar y escalar cada control. Comprobado en el S25 Ultra con toques de `adb`: abrir el menú, navegar hasta la pestaña Touch, editar (arrastrar y escalar el botón A), guardar y recuperar la disposición al reiniciar, y START en el título lleva a «PLAYER SELECT». Test en `src/headless.rs`. Descubierto y corregido: con contextos sobre hilos, el contexto GL de SDL se quedaba atado a un hilo y la pantalla dejaba de actualizarse (`EGL_BAD_ACCESS`); el hilo saliente lo suelta (`snes_rt_ctx_release`). **Falta**: probar a mano con dedos (varios a la vez, diagonales de la cruceta, vibración) y con un mando Bluetooth (los táctiles se ocultan). Verificar: se juega la intro y se llega a manejar a Link con los táctiles y con un mando Bluetooth.

Horizontal (2026-11-05, S25 Ultra, 20:9): la imagen salía cortada arriba y abajo (la app dibujaba 1080 de alto, pero los 84 px bajo la barra de estado y los 42 de la barra de navegación tapaban parte) y SELECT, START, Y y MENU caían sobre el texto. Corregido: en Android la ventana va a pantalla completa (barras ocultas, el sistema las muestra al deslizar desde el borde) y la disposición horizontal por defecto deja la imagen sola en el centro, con todos los controles en las bandas laterales (cruceta y SELECT a la izquierda; A, B, X, Y y START a la derecha; L, R y MENU en las esquinas). Comprobado con capturas en el selector de jugador y en la casa de Link, en horizontal y vertical. En un móvil 16:9 las bandas serían más estrechas (hay unos 120 px por lado con la imagen a 4:3) y los controles se montarían un poco sobre la imagen; sin probar. Quien ya tenga un `touch.ini` guardado conserva sus posiciones.

16:9 horizontal probado con `adb shell wm size 1080x1920` (pantalla de 1920x1080, 2026-11-05): la imagen queda de 1234 px entre bandas de 343 px, y la cruceta (387 px de ancho a tamaño 100) se mete unos 110 px en el borde izquierdo de la imagen y Y unos 47 px en el derecho; SELECT, START, MENU, L y R quedan fuera. Es usable (tapa los bordes), pero molesta en el juego. Posibles arreglos: reducir los controles según la proporción (p. ej. ×0,85 a 16:9), o que las posiciones por defecto salgan de la banda libre y no de una fracción de la ventana. Sin hacer.
- [x] 3.4 Carpeta de partidas de la app (`filesDir/saves`). Probado: se registra un jugador, `rom.srm` (8192 bytes, SRAM cruda como en escritorio) queda en `files/saves`, y tras `force-stop` y abrir de nuevo el jugador sigue en la ranura 1. Comprobado en el PC (2026-11-05): el `rom.srm` copiado del teléfono como `alttp_usa.srm` se carga tal cual en `snes-port` (`--saves-dir`), la ranura 1 existe y START la abre en la casa de Link; el fichero queda intacto.

## 4. Calidad y entrega

Medición (2026-11-05): con un fichero `files/stats` en la carpeta de la app (`adb shell run-as io.github.alexperez7.alttprecomp touch files/stats`; con el texto `unpaced` dentro, sin esperar entre fotogramas; `hash=N` corre N fotogramas sin ventana e imprime el hash de la WRAM; `perf=cpus=auto` fija el hilo a los núcleos más rápidos, que lee de `cpuinfo_max_freq`: en el S25 Ultra da `0xC0`) el runtime imprime en logcat (`adb logcat -s snesport`) cada 600 fotogramas el tiempo de trabajo por fotograma (p50, p99 y máximo), repartido en `emu` (CPU, PPU y SPC700 hasta el final del fotograma), `tail` (imagen para la pantalla, eventos, cola de audio, capa táctil) y `pres` (la llamada de presentación, que puede esperar a la sincronización vertical). S25 Ultra, intro del juego (fotogramas 601 a 1200), 3 repeticiones de cada modo, vsync apagado:

| Modo | emu p50 | emu p99 | pres p50 | tail p99 | total p99 |
|---|---|---|---|---|---|
| Con espera a 60 Hz | 10,1 ms | 14,8 a 16,3 ms | 0,8 ms | 0,5 ms | 16,4 a 17,3 ms |
| Sin espera (`unpaced`) | 2,8 a 3,5 ms | 6,1 a 7,4 ms | 0,3 a 0,4 ms | 0,1 ms | 6,6 a 7,9 ms |

- **La presentación y la capa táctil casi no cuestan** (menos de 1 ms y 0,5 ms): el trabajo está en la emulación.
- **El tiempo de la emulación depende de la frecuencia del procesador.** Con la espera a 60 Hz el sistema baja los relojes (los núcleos Performance a 2,4 a 2,7 GHz y los Prime parados a 1 GHz, `SDLThread` al 74 % de un núcleo) y cada fotograma tarda 3 veces más que a pleno reloj. Sin espera sale 2,8 a 3,5 ms de mediana, en línea con la prueba (media 2,6 ms, p99 5,67 ms; aquí p99 6,1 a 7,4 ms con otro recorrido y los táctiles).
- **Consecuencia para 4.1:** el coste real se mide con `unpaced`; la fluidez real, con la espera. En el S25 Ultra la espera mantiene 60,1 fps, pero el p99 del trabajo (16,4 a 17,3 ms) roza el presupuesto de 16,64 ms, y con un móvil más lento y relojes bajados sería peor. Falta probar si fijar un mínimo de reloj o un hilo de más prioridad lo arregla antes de dar la cifra para gama media.

Pruebas para bajar el coste con la espera a 60 Hz (S25 Ultra, mismos fotogramas, 2 repeticiones cada una; se activan con `perf=...` dentro de `files/stats`, ver `perf_init` en `runtime/play.c`; gobernador `walt`, los núcleos Prime 6 y 7 pasan el rato a 1,0 a 2,4 GHz de 4,47 GHz):

| Prueba | emu p50 | emu p99 |
|---|---|---|
| Base (sin nada) | 10,1 ms | 14,8 a 15,6 ms |
| Prioridad del hilo (`prio`, nice -20) | 10,3 ms | 15,2 a 15,6 ms (sin efecto) |
| Sesión de ADPF (`hint=4`, `8`, `12`) | 10,1 a 10,3 ms | 14,1 a 14,6 ms (casi sin efecto) |
| Hilo fijado a los núcleos Prime (`cpus=0xC0`) | 8,9 ms | 11,6 a 11,7 ms |
| Prime + espera en bucle los últimos 4 ms | 6,7 ms | 8,8 ms |
| Prime + espera en bucle los últimos 8 ms | 4,3 ms | 5,1 a 5,2 ms |
| Prime + espera en bucle los últimos 12 ms | 3,0 a 3,4 ms | 3,2 a 3,7 ms |
| Prime + espera en bucle todo el rato (`spin`) | 2,8 a 3,0 ms | 3,0 a 3,2 ms |

Repetición (2026-11-05, 3 ejecuciones por modo, mismos fotogramas, pantalla despierta):

| Modo | emu p50 | emu p99 | total p99 |
|---|---|---|---|
| Base | 10,1 a 10,2 ms | 13,8 a 14,3 ms | 14,8 a 15,3 ms |
| `cpus=auto` (núcleos `0xC0`) | 8,9 a 9,0 ms | 11,7 ms | 12,5 a 12,7 ms |
| `cpus=auto,hint=8` | 8,7 a 8,8 ms | 11,7 ms | 12,6 a 12,7 ms |

Las repeticiones coinciden entre sí (±0,1 ms), y la lectura anterior de 5,2 ms con `cpus=auto,hint=8` fue un valor aislado (la primera ejecución justo tras despertar el teléfono), no se repite. Conclusión: fijar el hilo a los núcleos rápidos baja 1,2 ms la mediana y 2 ms el p99; ADPF encima no añade nada medible.

**Decisión (2026-11-05): fijar el hilo a los núcleos rápidos (`cpus=auto`) va activado por defecto en Android**; `perf=none` en `files/stats` lo apaga (comprobado con `Cpus_allowed_list`: 6-7 por defecto, 0-7 con `none`). Prioridad, ADPF y la espera en bucle siguen apagados. Falta probarlo en un móvil más lento y medir batería y temperatura; si va peor en otro modelo, el interruptor es `perf=none`.

- **Ni la prioridad ni ADPF suben los relojes**; ADPF se crea (el sistema lo registra) pero no cambia la frecuencia.
- **Fijar el hilo a los núcleos más rápidos da lo más barato**: p99 de 15 a 11,7 ms, sin coste de batería conocido. Hay que calcular la máscara en cada teléfono (por `cpuinfo_max_freq`); `0xC0` es del S25 Ultra.
- **Lo que de verdad sube los relojes es que el hilo parezca ocupado**: esperar en bucle da el coste a pleno reloj (p99 3 ms), pero gasta un núcleo Prime entero y calienta. Sin medir el consumo (pendiente) no es una opción por defecto; la mejor relación está entre los últimos 4 y 8 ms.
- **Lo que falta:** medir batería y temperatura de cada opción y repetir en un móvil más lento antes de elegir. Por ahora todo sigue desactivado por defecto.

- [ ] 4.1 (**pendiente, se archiva sin hacer: hace falta un móvil de gama media y medir batería y temperatura; abrir un cambio nuevo cuando haya hardware**) Medir el p99 por fotograma con la app completa, 3 repeticiones, en el S25 Ultra y en al menos un teléfono de gama media. Verificar: tabla en `design.md › Resultados` con los modelos.
- [ ] 4.2 (**sin hacer: 4.1 no lo ha pedido; se queda `SDL_Renderer`**) GLES 3 si 4.1 lo pide; si no, dejar `SDL_Renderer`.
- [x] 4.3 APK firmado, licencia y fuentes de libtcc dentro de la app, `scan` sin hallazgos. Verificar: el APK no contiene nada derivado de la ROM. **Hecho (2026-11-05):** `gradle assembleRelease` da un APK de 1,6 MB, sin permiso de Internet, sin `debuggable`, con `libmain.so`, `libSDL2.so` y `libtcc.so` (aparte, LGPL) alineadas a 16 KB, y `assets/licenses/` con `NOTICE.txt`, la LGPL 2.1 de libtcc y la zlib de SDL2; el botón **Licencias** de la pantalla de elegir ROM los muestra; `snes-recomp scan` sobre el APK descomprimido termina sin hallazgos; instalado en el S25 Ultra arranca y genera y compila (409 ms y 145 ms, 216 MiB). La firma lee la clave de `~/.gradle/gradle.properties` (no se sube); **sin ella firma con la clave de depuración**, y así está firmado el APK probado. **Cerrado (2026-11-05):** la clave de reparto existe (copia de seguridad guardada por el autor), el APK de release va firmado con ella con id `io.github.alexperez7.alttprecomp`, está publicado en la release `v0.1` de `alttp-recomp` e instalado y arrancado en el S25 Ultra. Desde `android-updates` el APK lleva el permiso de Internet (aviso de versión, apagado por defecto).
- [x] 4.4 Documentar en `CLAUDE.md` y en el README cómo compilar y firmar el APK. Hecho: `android/README.md` (requisitos, construir, firmar, licencias, medir) y una línea en `CLAUDE.md`.

## Workflow follow-up

- En `alttp-recomp`: perfil, pruebas y workflow de GitHub Actions que sube el APK a las releases (cambio aparte).
- Archivar con `/opsx:archive` y actualizar en `CLAUDE.md` la línea «Último cambio archivado».
