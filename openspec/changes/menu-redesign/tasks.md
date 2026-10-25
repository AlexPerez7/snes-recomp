## 1. Fuente y dibujo base

- [x] 1.1 `runtime/play_ui.h` sin SDL: fuente proporcional propia (ASCII 32–126, flechas), `text_width`, `draw_text` con sombra y recorte con «...». Verificación: tests en C compilando el `.h` solo (todos los glifos caben, anchura = suma, recorte respeta el ancho) y captura ampliada de un alfabeto completo en `out/`.
- [x] 1.2 Primitivas del marco: panel con degradado, borde doble, esquinas, línea de título, barra de relleno, cursor. Verificación: test que dibuja en un búfer con guardas y comprueba que no escribe fuera, y captura revisada a ojo.
- [x] 1.3 `codegen --all` escribe `play/play_ui.h`; `build_play.bat` y `snes-port` lo compilan. Verificación: `cargo test` de codegen y construcción real con MSVC y con TinyCC.

## 2. Menú nuevo

- [x] 2.1 Reescribir `menu_draw` con título, pestañas, filas (selector, barra, acción), cursor animado y línea de ayuda. Verificación: capturas de las cuatro pestañas con `--menu-shot --menu-page N`, revisadas a ojo y guardadas en `out/shots/`.
- [x] 2.2 Navegación por pestañas (L/R del mando, Q/W del teclado) conservando remapeo y confirmación de conflictos. Verificación: tests con `--menu-keys` actualizados (cambiar pestaña, cambiar un valor, remapear y resolver un conflicto).
- [x] 2.3 Recorte de textos largos (dispositivo de audio, teclas asignadas) y flechas atenuadas en los extremos. Verificación: captura con un nombre de dispositivo largo simulado.

## 3. Opciones nuevas

- [x] 3.1 Opción `fps` en `play_cfg.h` y contador dibujado en la esquina. Verificación: test de la tabla de opciones y captura con `fps = on`; con `off` la salida del presentador es idéntica al búfer.
- [x] 3.2 Pestaña Juego: continuar, guardar ahora, restaurar valores por defecto (con confirmación) y salir. Verificación: test que fuerza el volcado de la SRAM y comprueba `<rom>.srm`, y test de restaurar que deja `config.ini` con los valores por defecto.
- [x] 3.3 Fila «Restaurar» en las pestañas de Imagen y Audio. Verificación: test que cambia tres opciones, restaura una pestaña y comprueba que solo esa vuelve a sus valores.

## 4. Cierre

- [x] 4.1 Determinismo de capturas y compatibilidad de presentación (aspecto, filtros, pantalla completa). Verificación: dos capturas idénticas byte a byte y captura del menú con `sharp` y `fit`.
- [x] 4.2 Documentar el menú nuevo, `fps` y los cambios de `--menu-keys`/`--menu-page` en `README.md` y `CLAUDE.md`. Verificación: los comandos documentados se ejecutan tal cual.
- [ ] 4.3 Prueba a mano del usuario: abrir F1 con teclado y mando, recorrer las pestañas y valorar el aspecto. Verificación: nota con el resultado en `out/play-notes.md`.
