# Design

## Context

La decisión viene de `android-feasibility-spike` (ver su `design.md › Resultados`): camino on-device, con el plan B (`.so` y `dlopen`) como respaldo. En el S25 Ultra, generar el C tarda unos 0,4 s y compilarlo de 0,16 a 0,29 s, así que no hace falta caché de compilación. Lo que falta para un producto es un frontal real. Hoy `runtime/play.c` asume escritorio: ventana SDL con contexto OpenGL 3.3 core, `main` con argumentos, `exit()` y `atexit` para terminar, la ROM como ruta de fichero y `config.ini` junto a la ROM.

## Goals / Non-Goals

**Goals:**
- Que el usuario instale el APK, elija la ROM una vez y juegue.
- Que el estado final sea idéntico al del PC (los hashes de la prueba).
- Fluidez razonable en gama media, medida y no supuesta.

**Non-Goals:**
- Nube, Play Store, iOS, shaders nuevos y ajustes finos de interfaz.

## Decisions

Las decisiones se completan al escribir las tareas; las ya tomadas son:

**1. El frontal es el de `play.c`, no uno nuevo.** El código de PPU, audio, entrada y menú ya está en `runtime/play*.h`. Se adapta con `#ifdef __ANDROID__` donde haga falta (contexto, terminación, ruta de la ROM) y se añaden los controles táctiles como una capa aparte.
- *Alternativa descartada:* una app nativa con su propia interfaz de vídeo y audio. Duplicaría lo que ya funciona y se desviaría del PC.

**2. La app es una Activity de SDL2** (`SDLActivity`) con una biblioteca nativa que lleva el runtime, el generador y libtcc. La interfaz de selección de ROM (SAF) es Java mínimo y entrega el contenido por descriptor de fichero, sin permisos de almacenamiento.

**3. Terminación.** El runtime hoy termina el proceso con `exit()`. En Android, «salir» pasa por `SDL_QUIT` y por el ciclo de vida: al pausar se guarda la SRAM y se detiene el audio, y al reanudar se continúa. El `exit()` queda para errores fatales.

**4. Imagen.** Primero `SDL_Renderer` (ya existe en `play.c`) para tener algo en pantalla; después GLES 3 con el mismo `play_gl.h` si el rendimiento lo pide. Se decide con medidas, no antes.

**5. Controles táctiles** dibujados por el propio frontal con las mismas texturas que el menú (sin recursos de imagen), con zonas de toque que alimentan `joy1_state` como un mando más.

## Risks / Trade-offs

- [El p99 en gama media no se ha medido] → medición obligatoria antes de publicar, con al menos un teléfono más lento. Si falla, la primera palanca es el C para TinyCC (`static inline` de `snes_rt.h`) y el coste de la PPU por línea.
- [`exit()` y las variables globales del runtime no admiten pausar y reanudar] → las pruebas de ciclo de vida (poner la app en segundo plano, girar, recibir una llamada) son parte de las tareas.
- [Licencia de libtcc (LGPL 2.1)] → `.so` aparte, con licencia y enlace a las fuentes dentro de la app y en las releases.
- [Google Play y políticas de memoria ejecutable] → no se publica en Play Store; solo APK en GitHub.

## Open Questions

- Ubicación del proyecto Android definitivo (¿`android/` en este repositorio?) y su relación con `spikes/android/`.
- Cómo se empaqueta el perfil de cada juego (hoy solo ALttP) y su C propio dentro del APK.
- Si hace falta una segunda ABI (`x86_64` para el emulador); la prueba solo usó `arm64-v8a`.
