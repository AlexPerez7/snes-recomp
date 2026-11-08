# App Android de snes-port (`android-frontend`)

La app que un usuario instala, abre, elige su ROM una vez y juega. El teléfono genera el C del juego desde la ROM (biblioteca del generador, Rust) y lo compila en memoria con libtcc al arrancar (unos 0,6 s en un Galaxy S25 Ultra); el frontal es el de `runtime/play.c` sobre SDL2. **La ROM no va en el APK**: se elige con el selector de documentos, se copia a la carpeta privada de la app y se comprueba su CRC32. El único uso de Internet es el aviso opcional de versión nueva (apagado por defecto, ver abajo).

El identificador de la app (`snesPort.applicationId` en `gradle.properties`) es el del juego, `io.github.alexperez7.alttprecomp`; el paquete de Java (`io.github.alexperez7.snesport`) es interno y no se ve. El identificador no se puede cambiar una vez repartida una versión: Android lo contaría como otra app.

## Lo que hace falta en el PC

- Android SDK con **NDK 27.2.12479018**, **CMake 3.22.1**, plataforma 35 y build-tools 35; **JDK 17 o más** y **Gradle 8.9** (no hay wrapper en el repositorio).
- Rust con `rustup target add aarch64-linux-android`; la tarea `cargoGenerator` construye el generador como biblioteca estática y no necesita el enlazador del NDK.
- Fuentes hermanas sin configurar: **TinyCC** rama `mob` (`e23dabe`) en `../tinycc` y **SDL2 2.32.x** en `../SDL2-2.32.10` (rutas en `gradle.properties`). Las rutas del perfil del juego (`snesPort.gameProfile`) apuntan a `alttp-recomp`.

## Construir

```
export ANDROID_HOME=~/Android JAVA_HOME=~/Android/jdk
cd android
gradle assembleDebug      # app/build/outputs/apk/debug/app-debug.apk
gradle assembleRelease    # app/build/outputs/apk/release/app-release.apk
adb install -r app/build/outputs/apk/release/app-release.apk
```

Con `--offline` si no hay red. El análisis `lint` de release está desactivado porque descarga sus herramientas. `libmain.so`, `libSDL2.so` y `libtcc.so` salen alineadas a 16 KB.

## Firmar una versión para repartir

La clave **no está en el repositorio ni se sube nunca**; sin ella no hay actualizaciones sobre la versión instalada, así que hay que guardarla (y su contraseña) en más de un sitio.

```
keytool -genkeypair -v -keystore snes-port.jks -alias snes-port -keyalg RSA -keysize 4096 -validity 36500
```

En `~/.gradle/gradle.properties`:

```
snesPort.keystore=/ruta/a/snes-port.jks
snesPort.keystorePassword=...
snesPort.keyAlias=snes-port
snesPort.keyPassword=...
```

Con eso `gradle assembleRelease` firma el APK; sin ello lo firma con la clave de depuración y avisa (solo sirve para probar). Comprobar: `apksigner verify --print-certs app-release.apk`.

Antes de repartir un APK: `snes-recomp scan <rom> <carpeta>` sobre el APK descomprimido (fuera del repositorio) debe terminar sin hallazgos, y el APK debe llevar `NOTICE.txt`, la licencia de libtcc y la de SDL2 en `assets/licenses/`.

## Avisos de versión nueva y actualización desde la app (`android-updates`)

Con el ajuste **Buscar actualizaciones al abrir** (activo por defecto; se apaga en la pantalla de ajustes), la app consulta como mucho una vez al día la última release de GitHub (`snesPort.updateUrl` en `gradle.properties`; vacía, la app no lleva nada de esto) y, si su etiqueta es mayor que la `versionName` instalada, pregunta: **Descargar e instalar**, **Ahora no** u **Omitir esta versión**. Con el ajuste apagado no se conecta.

**Descargar e instalar** (`UpdateInstaller`): si la release trae un APK y el `SHA256SUMS`, la app los baja con una barra de progreso (con **Cancelar**), comprueba el SHA-256 del APK contra el `SHA256SUMS` y, solo si coincide, se lo entrega al instalador de Android (`PackageInstaller`), que enseña su confirmación; al terminar, la app se cierra y se abre de nuevo ya actualizada. La primera vez Android pide permitir a la app instalar aplicaciones (permiso `REQUEST_INSTALL_PACKAGES`; la app abre esos ajustes). Android no deja instalar sin que el usuario confirme, y solo acepta el APK si lleva **la misma firma** que la app instalada, así que una release ajena no puede reemplazarla. Si la release no trae APK, el botón abre su página.

La app sale en el idioma del teléfono (inglés por defecto y español: `res/values` y `res/values-es`); no hay selector propio dentro de la app. El ajuste y el botón **Buscar actualizaciones ahora** están en la pantalla de ajustes (mantener pulsado el icono › *Ajustes y actualizaciones*, o la propia pantalla de elegir ROM); abrir la app normalmente va directo al juego, salvo que haya una versión nueva.

Para que funcione, cada release de `alttp-recomp` se etiqueta `vX.Y`, lleva el APK (`*.apk`) y `SHA256SUMS`, y la `versionName` del APK (en `app/build.gradle`, hoy `0.2` con `versionCode 2`) debe subir con ella. En depuración, un fichero `files/update_url` sustituye la dirección y un servidor local con `adb reverse` permite probar sin GitHub.

**Probado sin móvil:** la interpretación del `SHA256SUMS` y que el APK 0.2 lleva la misma firma que el 0.1 (Android lo acepta como actualización). **Sin probar en un teléfono:** la descarga, el permiso y el instalador; ver `tasks.md` del cambio.

## Licencias

libtcc (TinyCC) es LGPL 2.1 y va como `libtcc.so` aparte, para poder sustituirla. Los avisos y el enlace a las fuentes están en `NOTICE.txt` y se ven en la app con el botón **Licencias** de la pantalla de elegir ROM.

## Medir en el teléfono

Un fichero `files/stats` en la carpeta de la app activa la medición (`adb shell run-as io.github.alexperez7.alttprecomp sh -c 'echo ... > files/stats'`; solo en el APK de depuración). Su contenido, separado por comas o espacios:

- `unpaced`: sin esperar entre fotogramas (coste a pleno reloj).
- `hash=N`: N fotogramas sin ventana y el hash de la WRAM, para compararlo con `snes-port ... -- --no-window --frames N --hash-wram --game-opt language=0` en el PC.
- `perf=none`, `perf=prio`, `perf=hint=MS`, `perf=spin`, `perf=cpus=0xMASK`: ajustes del hilo; por defecto va fijado a los núcleos más rápidos (`cpus=auto`).

La salida (`work: p50, p99, max` cada 600 fotogramas, desglosado en `emu`, `tail` y `pres`) va a `adb logcat -s snesport`. Los resultados y su análisis están en `openspec/changes/android-frontend/tasks.md`.

## Estados guardados

En un juego con `[resume]` en su perfil (ALttP lo tiene) la pantalla táctil trae tres placas: **SAVE** y **LOAD** guardan y cargan el estado de la ranura actual, **SLOT** abre el selector de ranuras (una cuadrícula de 3×3 con la miniatura y la fecha de cada estado; se elige tocando una ranura o con la cruceta y A, y se cierra con B, tocando fuera o con SLOT otra vez; el juego queda en pausa mientras está abierto) y **REWIND**, mantenida, rebobina. Son los mismos controles que F5, F7, F4 y Retroceso en el PC (estados en `saves/`, formato de `snes_state.inc`) y se colocan y escalan en el editor de la pestaña Touch como el resto. En un juego sin `[resume]` no aparecen. Probado con las pruebas del motor (posiciones y pulsaciones) y a mano el 2026-11-07 en un Galaxy S25 Ultra: guardar, cargar y rebobinar funcionan y el selector de ranuras también (cuadrícula, miniaturas, elegir ranura). La miniatura se guarda junto al `.sst`, en un `.sN.thumb`; los estados guardados antes de esta versión salen sin miniatura hasta que se vuelven a guardar.

## Ciclo de vida

Al pasar a segundo plano el juego se detiene, se guarda la SRAM y el audio se pausa; al volver sigue donde estaba. Cerrar la app de verdad mata el proceso (SDL no admite un segundo `SDL_main`).
