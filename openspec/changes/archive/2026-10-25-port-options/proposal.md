# Proposal

## Why

Un port jugable en PC no es solo «que corra»: se espera pantalla completa, elegir la resolución y el aspecto, remapear controles, ajustar el volumen y recordar todo eso entre sesiones. `interactive-frontend` da lo mínimo (ventana, teclas fijas, escalado entero). Este cambio añade las opciones que cualquier port de PC trae de serie, sin tocar la lógica del juego.

## What Changes

- Fichero de configuración de usuario (`config.ini` junto a la ROM o en la carpeta de datos del usuario) con valores por defecto, que se crea solo y se relee al arrancar; las opciones de línea de comandos lo sobrescriben.
- Opciones de imagen: pantalla completa (ventana sin bordes o exclusiva), vsync, relación de aspecto (4:3 píxel no cuadrado, 8:7 píxel cuadrado, ajuste libre), escalado entero o libre, filtro `nearest`/`bilinear`/nitidez por píxel, y recorte de márgenes superior e inferior (overscan).
- Shaders de postproceso opcionales y sencillos (scanlines, rejilla, corrección de color), activables por configuración.
- Menú en pantalla accesible con una tecla (F1) para cambiar esas opciones sin salir, que se guardan al confirmarlas.
- Controles: remapeo interactivo de teclado y mando por jugador, zonas muertas y botón de turbo; guardado en la misma configuración.
- Audio: volumen maestro, volumen de efectos de pantalla (clic de menú) y selección de dispositivo de salida.
- Sustituye el fichero `<rom>.keys` de `interactive-frontend` por esta configuración.

## Capabilities

### New Capabilities
- `port-config`: fichero de configuración del port, valores por defecto, precedencia entre fichero y línea de comandos, y menú en pantalla para editarlo.
- `display-options`: modos de pantalla, aspecto, escalado, filtros y shaders opcionales de la presentación.

### Modified Capabilities

## Impact

- `runtime/play.c`: lectura/escritura de configuración, menú, escalado, shaders (SDL2 con renderizador de texturas; los shaders de píxel van por SDL_gpu o una implementación en CPU de scanlines, a decidir en el diseño).
- Sustituye partes de `live-input` (la asignación se mueve a la configuración) y dependen de `interactive-frontend`.
- Sin dependencias nuevas obligatorias; parser INI propio y pequeño.
- Fuera de alcance: mejoras que cambian el juego (pantalla ancha, 60+ fps interpolados, calidad de vida), que irán en cambios aparte sobre un mecanismo de parches.
