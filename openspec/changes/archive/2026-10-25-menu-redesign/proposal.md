# Proposal

## Why

El menú F1 actual es funcional pero pobre: una fuente 5x7 de pantalla de depuración, texto plano sobre la imagen oscurecida, sin marco, sin cursor reconocible y con pocas opciones. Se probó con éxito y el usuario pidió que parezca un menú nativo de un juego, con el aire de los menús de *A Link to the Past*, y con más opciones.

## What Changes

- Fuente de píxeles propia, dibujada para este proyecto (mayúsculas y minúsculas, proporcional, con sombra), con el carácter de los menús de ALttP. No se copia ni se extrae la fuente de Nintendo ni ningún gráfico de la ROM.
- Menú con diseño: panel con marco decorado y esquinas, barra de título, pestañas (Imagen, Audio, Controles, Juego), cursor dibujado por código (flecha/mano) con animación suave, valores con flechas `◄ ►`, barra para el volumen y una línea de ayuda con las teclas y los botones del mando.
- Nueva pestaña **Juego**: continuar, guardar la partida ahora, restaurar valores por defecto y salir. Y una opción nueva, `fps`, para mostrar el contador de fotogramas.
- «Restaurar valores por defecto» también en las pestañas de Imagen y Audio.
- Se mantienen el manejo por teclado y mando, las opciones existentes, el remapeo de controles, el guardado al cerrar y las capturas de prueba `--menu-shot`/`--menu-page`.
- **BREAKING**: `--menu-page N` pasa a numerar las pestañas del menú nuevo (Imagen 0, Audio 1, Controles 2, Juego 3); las secuencias de `--menu-keys` cambian porque la navegación pasa a ser por pestañas (izquierda/derecha en la barra de pestañas).

## Capabilities

### New Capabilities
- `menu-ui`: aspecto y comportamiento visual del menú: fuente propia, panel, pestañas, cursor, ayuda de teclas y presentación sobre la imagen.

### Modified Capabilities
- `port-config`: el requisito «Menú en pantalla» incluye la pestaña Juego y restaurar valores por defecto; se añade el contador de fotogramas como opción.

## Impact

- `runtime/play.c` (menú, dibujo, `--menu-*`), nuevo `runtime/play_ui.h` sin dependencia de SDL (fuente, medida de texto, marco, barras) que `codegen --all` escribe en `play/` y compilan los tests solo; `runtime/play_cfg.h` (opción `fps`).
- `src/codegen.rs`, `src/main.rs` (escribir el fichero nuevo), `src/headless.rs` (tests), `README.md`, `CLAUDE.md`.
- Sin dependencias nuevas ni ficheros de fuente: todo va en el código.
