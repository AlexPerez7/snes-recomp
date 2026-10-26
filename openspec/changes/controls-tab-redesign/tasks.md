## 1. Fuente

- [x] 1.1 En `runtime/play_ui.h`, añadir los glifos `← ↑ → ↓` (y el separador, si la fuente no lo dibuja) dentro de la caja 5x7
- [x] 1.2 Comprobar con la prueba de glifos de `src/headless.rs` que los nuevos tienen anchura > 0 y caben

## 2. Nombres de presentación

- [x] 2.1 En `runtime/play_i18n.h`, tabla `{nombre SDL, en, es}` (`bind_names`) de teclas y botones de mando, y las cabeceras («Keyboard»/«Teclado», «Controller»/«Mando») y la etiqueta de la cruceta
- [x] 2.2 En `runtime/play_i18n.h`, `bind_label` (tabla y respaldo a SDL; `bind_cell` de `play.c` arma la celda); `bind_name` no cambia, porque lo usa el INI
- [x] 2.3 Prueba: cada nombre de la tabla cabe en su columna en los dos idiomas

## 3. Disposición

- [x] 3.1 Dibujar la pestaña Controles con la etiqueta y dos columnas (teclado y mando) y su cabecera, con `text_fit` en cada una
- [ ] 3.2 Varias asignaciones separadas con el separador elegido; guion atenuado si no hay
- [x] 3.3 Fila del jugador como selector «Jugador N», sin la etiqueta cortada; ajustar `ROW_Y`/`ROW_H` solo en esta pestaña si la altura no alcanza
- [x] 3.4 La celda en captura (`menu_capture`) muestra «...» en las dos columnas (aún no se sabe si llegará una tecla o un botón)

## 4. Pruebas y cierre

- [x] 4.1 `--menu-shot --menu-page 2` en `en` y `es`: sin recortes, los hashes de las demás pestañas no cambian
- [x] 4.2 `--print-config` y `config.ini` idénticos a antes (nombres de SDL)
- [x] 4.3 `cargo test` completo con SDL2 sin regresiones
- [x] 4.4 README y `CLAUDE.md`: descripción de la pestaña Controles
- [ ] 4.5 Probar a mano en la ventana: reasignar teclado y mando, jugador 2, conflicto de teclas
