## Why

La ventana del juego ya tiene escalado, filtros (`nearest`, `bilinear`, `sharp`), scanlines y rejilla, pero todo ello sale del `SDL_Renderer`, que no admite shaders: los efectos son una textura superpuesta con mezcla. Eso no permite lo que se espera de un port de SNES: máscara de fósforo real, haz de scanline que depende de la luminosidad, curvatura suave del tubo ni resplandor (bloom). Quien juega en un monitor moderno quiere poder acercarse al aspecto de un CRT sin tocar el juego.

## What Changes

- Una opción nueva `shader` con los valores `off` (por defecto, el camino actual sin cambios) y `crt`.
- Con `shader = crt` la ventana presenta la imagen con OpenGL 3.3 (cargado con `SDL_GL_GetProcAddress`, sin dependencias nuevas): una pasada de fragmentos a la resolución de salida que reúne el filtrado nítido (`sharp`), las scanlines con haz, la máscara de fósforo, la curvatura y el bloom.
- Opciones nuevas del efecto: `crt_mask` (0-100), `crt_curve` (0-100) y `crt_bloom` (0-100); `scanlines` (0-100) pasa a gobernar la fuerza del haz. Aspecto, escalado, recorte (`overscan`) y pantalla ancha (`wide`) siguen mandando sobre el rectángulo donde se dibuja.
- Las filas nuevas aparecen en la pestaña Image del menú F1, con su traducción al español.
- Respaldo: si no hay OpenGL 3.3, el contexto o el shader no se crean, se avisa una vez por consola y se sigue con el camino actual del `SDL_Renderer`. El juego nunca deja de arrancar por esto.
- El cambio de `shader` en marcha recrea la ventana (el contexto de OpenGL se fija al crearla) sin tocar el estado del juego.

## Capabilities

### New Capabilities

Ninguna.

### Modified Capabilities

- `display-options`: el shader CRT, sus opciones y el respaldo al renderizador.

## Impact

- `runtime/play.c`: ruta de presentación con OpenGL (`present_gl`), creación y recreación de la ventana, respaldo; `runtime/play_cfg.h`: opciones nuevas; `runtime/play_i18n.h` y `runtime/play_ui.h`: filas y textos del menú (con las pruebas que miden cada cadena contra su hueco).
- El código del shader (GLSL) va como cadena en un fichero nuevo, `runtime/play_gl.h`, escrito desde cero (clean-room).
- `build_play.bat` y la compilación de `snes-port` no necesitan bibliotecas nuevas: SDL2 carga `opengl32`/`libGL` por su cuenta.
- `README.md` (tabla de opciones) y el `plan.md` de `alttp-recomp`.
- Fuera de alcance: shaders GLSL cargados de una carpeta del usuario, Direct3D/Metal/Vulkan, varias pasadas encadenadas, simulación de vídeo compuesto o NTSC, HDR, y el modo 7 en alta resolución (punto 13 del plan).
