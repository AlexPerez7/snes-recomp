## 1. Fuente y UTF-8

- [x] 1.1 En `runtime/play_ui.h`, añadir la decodificación UTF-8 (punto de código y longitud; `?` si es inválido o truncado) y usarla en `text_w`, `text_draw`, `text_fit` y `text_draw2x`
- [x] 1.2 Añadir los glifos `á é í ó ú ü ñ Á É Í Ó Ú Ü Ñ ¿ ¡` con el acento dentro de la altura de la fila; los glifos ASCII no cambian
- [x] 1.3 Que `text_fit` recorte con «...» en frontera de carácter

## 2. Tabla de cadenas y selección de idioma

- [x] 2.1 Crear `runtime/play_i18n.h` (sin SDL) con la tabla `{en, es}` y `tr()` con respaldo al inglés; que `codegen --all` lo escriba junto a los demás ficheros de `play/`
- [x] 2.2 En `play.c`, función que dice si el menú va en español: la opción de juego con clave `language` vale `es`; se consulta al dibujar
- [x] 2.3 Pasar por `tr()` los nombres de pestañas, `image_labels`, las etiquetas de filas, los valores mostrados, los avisos y las líneas de ayuda; no tocar `config.ini`, `--print-config` ni la consola

## 3. Pruebas

- [x] 3.1 En `src/headless.rs`: glifos del español dentro de su caja y con anchura > 0; UTF-8 inválido da `?`; recorte con tilde produce UTF-8 válido
- [x] 3.2 Prueba de anchura: cada cadena de la tabla cabe en su hueco
- [x] 3.3 `--menu-shot` con `language=es` da un hash distinto que con `en`; el de `en` no cambia respecto al anterior
- [x] 3.4 Cambiar `language` en la pestaña Game cambia el menú sin cerrarlo; `--print-config` idéntico con los dos idiomas
- [x] 3.5 `cargo test` completo con SDL2 (`SDL2_DIR`) sin regresiones

## 4. Documentación y cierre

- [x] 4.1 README y `CLAUDE.md`: el menú sigue la opción `language`; qué se traduce y qué no
- [x] 4.2 Probar a mano en `snes_play.exe` con ALttP en español: tabs, avisos, ayuda y nombres de dispositivo largos
- [x] 4.3 Actualizar la línea «Último cambio archivado» y la memoria al archivar
