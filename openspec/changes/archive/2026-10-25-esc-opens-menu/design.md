## Context

Observado en `runtime/play.c`:

- `handle_events` trata `Esc` con el menú cerrado como `stop(0, "quit")`; con el menú abierto, todas las teclas van a `menu_key`, donde `Esc` y `F1` llaman a `menu_close` (que guarda `config.ini`) y `Esc` también cancela la confirmación de reasignación, la de «restaurar todo» y la captura de una tecla.
- Solo `F1` abre el menú (`ui_key`), y el modo de pruebas (`--menu-keys`) reutiliza `ui_key`; casi todas las pruebas de `src/headless.rs` empiezan con `F1`.
- La pestaña Game ya termina con las filas Resume, Save, Restore y **Quit**; Quit llama a `stop(0, "quit from the menu")` sin preguntar. «Restore all defaults» ya usa el patrón de confirmación `menu_ask` (Intro confirma, `Esc` cancela; el mando manda A como Intro y B como `Esc` a `menu_key`).
- Cerrar la ventana llega como `SDL_QUIT` y llama a `stop(0, "window closed")`.

## Goals / Non-Goals

**Goals:**
- `Esc` abre el menú y no cierra el juego; `F1` sigue siendo atajo.
- Salir queda como una acción deliberada con confirmación, con el teclado y con el mando.

**Non-Goals:** un botón del mando para abrir el menú, cambiar la lista de filas del menú, o preguntar al cerrar la ventana (la X sigue directa).

## Decisions

- **Una sola función para abrir.** El código de `F1` que reinicia el estado del menú (`menu_capture`, `menu_confirm`, `menu_ask`, mensaje, tick) pasa a `menu_open_now()`; `ui_key` la llama para `F1` y para `Esc`. `handle_events` deja de tener el caso `ESCAPE: stop` y manda `Esc` por `ui_key` igual que `F1`. Alternativa descartada: un atajo de ajustes para elegir la tecla; más superficie sin necesidad.
- **Confirmación de Quit con el mismo mecanismo.** `menu_ask` pasa de 0/1 a 0/1/2 (`ASK_RESTORE`, `ASK_QUIT`): Intro ejecuta lo que corresponda (`restore_all()` o `stop(0, "quit from the menu")`), `Esc` cancela. El mensaje es «Enter quits the game, Esc cancels». El mando ya manda A como Intro y B como `Esc` mientras haya una confirmación, así que no cambia. Alternativa descartada: pedir confirmación en cada salida, incluida la X de la ventana; la X es ya una acción deliberada.
- **`Esc` con el menú abierto no cambia.** Cierra y guarda, y sigue cancelando confirmaciones y capturas; el pie del menú ya dice «Esc: close».
- **Pruebas.** `--menu-keys` ya acepta `Escape`: los scripts con `F1,…` siguen valiendo y se añade una variante que abre con `Escape`. `menu-quit` se actualiza (la fila ya no sale con un solo Intro) y se añade la cancelación.
- **Documentación.** La tabla de teclas del README (`Esc` pasa de «salir» a «menú», `F1` queda como «menú»), la frase del menú y `CLAUDE.md`.

## Risks / Trade-offs

- [Quien tenga el hábito de Esc para salir lo verá abrir el menú] → Es lo pedido; salir es dos acciones (Game → Quit → Intro) y la X sigue funcionando. El README lo dice.
- [Pantalla completa sin ratón: la X no está a mano] → Quit en el menú cubre ese caso, y `Alt+F4` sigue funcionando.
- [`Esc` mientras el juego está pausado con `P`] → Abre el menú como siempre, y al cerrarlo vuelve a la pausa (el bucle ya espera mientras `paused || menu_open`).
- [El juego ve `Esc` como entrada] → No: `Esc` no estaba asignado a ningún botón de la consola y solo cambia lo que hace el frontal.
