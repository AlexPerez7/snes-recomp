## 1. Frontal (`runtime/play.c`)

- [x] 1.1 `menu_open_now()` con el reinicio del estado del menú; `ui_key` la llama con `F1` y con `Esc`, y `handle_events` manda `Esc` por `ui_key` y quita el caso que salía del programa
- [x] 1.2 `menu_ask` con dos clases (restaurar, salir): la fila Quit pone el aviso «Enter quits the game, Esc cancels»; Intro sale con `stop(0, "quit from the menu")`, `Esc` cancela y deja el menú abierto
- [x] 1.3 Con el mando, A confirma la salida y B la cancela (comprobar que `menu_pad` ya lo hace)

## 2. Pruebas (`src/headless.rs`)

- [x] 2.1 Un script que abre con `Escape` (y otro con `F1`) llega a las mismas filas y cambia un ajuste igual
- [x] 2.2 `Escape` con el menú cerrado no termina el programa: el juego corre los fotogramas pedidos y sale con código 0 por agotarlos
- [x] 2.3 `menu-quit`: Intro sobre Quit pide confirmación y el programa sigue; Intro otra vez termina; `Escape` cancela y el menú sigue abierto; la captura del menú con el aviso es distinta de la de sin aviso
- [x] 2.4 Las pruebas existentes del menú siguen pasando sin cambios

## 3. Documentación y cierre

- [x] 3.1 README (tabla de teclas y descripción del menú) y `CLAUDE.md` (tabla de teclas)
- [x] 3.2 `cargo test` completo y, si hay compilador y SDL2, `build_play.bat` para ver `snes_play.exe` arrancar
- [x] 3.3 Prueba manual con `play.bat`: `Esc` abre el menú, `Esc` lo cierra, Game → Quit pide confirmación y la X cierra sin preguntar
