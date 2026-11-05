# Tasks

## 1. Base de la app

Proyecto en `android/` (2026-11-05): compila con `gradle assembleDebug` (Gradle 8.9), se instala en el S25 Ultra, `libmain.so` y `libSDL2.so` alineadas a 16 KB (`p_align` 0x4000) y el juego llega a la pantalla de título generado y compilado en el teléfono (generar 449 ms, compilar y enlazar 170 ms). `LauncherActivity` (selector de ROM con CRC32) y `GameActivity` (SDL) están escritas pero el selector aún no se ha probado a mano; la prueba se hizo dejando la ROM en `files/rom.sfc`. Sin sonido ni controles probados todavía.

- [x] 1.1 Proyecto Gradle definitivo (SDL2 `SDLActivity`, NDK 27 o 28, `arm64-v8a`, sin permiso de Internet, sin `debuggable` en release). Verificar: `gradlew assembleRelease` termina y el APK se instala con `adb install`.
- [x] 1.2 Libc para TinyCC: llevar `spikes/tcc_include/` a la app y la tabla de símbolos de la libc con `dlsym`. Verificar: el C del juego compila en el teléfono sin referencias sin resolver (como en la prueba).
- [x] 1.3 Biblioteca nativa con el runtime en modo biblioteca, la biblioteca del generador y libtcc; alinear a 16 KB. Verificar: `llvm-readelf` muestra `p_align` 0x4000 en los `LOAD` de cada `.so` y Android no muestra el aviso de compatibilidad.

## 2. Arranque de usuario

- [ ] 2.1 Selector de ROM con SAF, copia a `filesDir`, comprobación del CRC32 y mensaje para una ROM desconocida con los dos CRC32. Verificar: una ROM correcta arranca el juego, una ajena se rechaza.
- [ ] 2.2 ROM recordada con permiso persistente: la segunda vez se juega sin pedirla. Verificar: cerrar y abrir la app arranca directamente.
- [ ] 2.3 Generar y compilar en el arranque, con una pantalla de progreso. Verificar: registro con tiempos y memoria; hashes iguales a los del PC tras 3000 fotogramas.

## 3. Frontal

- [ ] 3.1 `play.c` en Android con `SDL_Renderer`: imagen y audio. Verificar: se ve y se oye el juego a 60 fps en el S25 Ultra.
- [ ] 3.2 Ciclo de vida: pausa y reanudación sin perder la partida, SRAM guardada al pausar, sin `exit()` en la salida normal. Verificar: segundo plano, giro de pantalla y volver.
- [ ] 3.3 Controles táctiles y mandos SDL. Verificar: se juega la intro y se llega a manejar a Link con los táctiles y con un mando Bluetooth.
- [ ] 3.4 Carpeta de partidas de la app (`filesDir/saves`). Verificar: el `.srm` se conserva entre ejecuciones y tiene el mismo formato que en escritorio (se abre en el PC).

## 4. Calidad y entrega

- [ ] 4.1 Medir el p99 por fotograma con la app completa, 3 repeticiones, en el S25 Ultra y en al menos un teléfono de gama media. Verificar: tabla en `design.md › Resultados` con los modelos.
- [ ] 4.2 GLES 3 si 4.1 lo pide; si no, dejar `SDL_Renderer`.
- [ ] 4.3 APK firmado, licencia y fuentes de libtcc dentro de la app, `scan` sin hallazgos. Verificar: el APK no contiene nada derivado de la ROM.
- [ ] 4.4 Documentar en `CLAUDE.md` y en el README cómo compilar y firmar el APK.

## Workflow follow-up

- En `alttp-recomp`: perfil, pruebas y workflow de GitHub Actions que sube el APK a las releases (cambio aparte).
- Archivar con `/opsx:archive` y actualizar en `CLAUDE.md` la línea «Último cambio archivado».
