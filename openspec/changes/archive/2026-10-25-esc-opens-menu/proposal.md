## Why

En el frontal interactivo `Esc` cierra el juego al instante, y lo normal es que abra el menú. Una pulsación accidental acaba la partida (la SRAM se guarda, pero se pierde la sesión), mientras que el menú solo se abre con `F1`, una tecla poco conocida. Se quiere que `Esc` sea la tecla del menú y que salir sea una elección deliberada.

## What Changes

- `Esc`, con el menú cerrado, **abre el menú** en lugar de salir; `F1` sigue abriéndolo (segundo atajo). Con el menú abierto, `Esc` lo cierra como hoy (y guarda), y sigue cancelando las confirmaciones y la captura de teclas.
- La fila **Quit** de la pestaña Game (hoy sale al instante) pide **confirmación**, como «Restaurar todo»: Intro sale, `Esc` cancela; en el mando, A sale y B cancela.
- La **X de la ventana** sigue cerrando el juego sin preguntar.
- Documentación (README, `CLAUDE.md`) y pruebas del menú puestas al día.
- No cambia el mando: no tiene un botón que abra el menú, y este cambio no añade uno.

## Capabilities

### New Capabilities

Ninguna.

### Modified Capabilities

- `live-video-window`: requisito «Teclas de control»: `Escape` abre el menú en lugar de salir.
- `port-config`: requisito «Menú en pantalla»: el menú se abre con `Escape` o `F1`, y «salir» pide confirmación.

## Impact

- `runtime/play.c` (manejo de teclas, fila Quit, confirmación) y sus pruebas en `src/headless.rs` (`menu-quit` y las que abren el menú).
- `README.md`, `CLAUDE.md` (tabla de teclas y descripción del menú).
- Los juegos que usan `snes-port` lo reciben con la siguiente versión del motor; su caché cambia de versión porque cambia el runtime.
