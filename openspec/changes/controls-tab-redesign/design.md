## Context

`menu_draw` (en `runtime/play.c`) dibuja la pestaña Controles con la misma `menu_row` de las demás: etiqueta a la izquierda y un único texto a la derecha (`kind` 3, `VAL_W + 20` = 112 px). El texto sale de `bind_text`, que concatena los nombres de teclado y, tras ` | `, los de mando; estos vienen de `SDL_GetScancodeName` y `SDL_GameControllerGetStringForButton`. El panel mide 236 px y la fuente es proporcional de 5x7 engrosada.

## Goals / Non-Goals

**Goals:**
- Nombres legibles y traducibles; teclado y mando separados; nada cortado de forma fea.
- No cambiar el formato de `config.ini` ni su contenido.
- Mantener las pruebas sin ventana (`--menu-shot`) deterministas.

**Non-Goals:**
- Cambiar cómo se capturan las teclas (`menu_capture`) ni el reemplazo de asignaciones.
- Ejes analógicos o tipos de mando.

## Decisions

1. **Tabla de nombres de presentación, aparte de `bind_name`.** `bind_name` se sigue usando para escribir `config.ini` (nombres de SDL). Una función nueva `bind_label` devuelve el nombre mostrado: busca el código en una tabla `{código, en, es}` (flechas, Intro, Espacio, Mayús der./izq., Ctrl, Alt, Tab, Retroceso, Supr, Insert, Inicio, Fin, RePág, AvPág, F1-F12, teclado numérico) y, si no está, cae al nombre de SDL. Las letras y los números ya son legibles. Alternativa descartada: reemplazar `bind_name`, porque rompería la lectura del INI.
2. **Dos columnas.** Cada fila muestra `etiqueta | teclado | mando`. Anchuras repartidas en los 212 px de la fila (etiqueta ~50, teclado ~80, mando ~72, a medir al implementar) con `text_fit` en cada una. Los dos jugadores usan el mismo diseño; el jugador 2 no trae teclas por defecto y su columna queda vacía con un «–» atenuado. Alternativa descartada: una columna con cambio por tecla (Tab) entre teclado y mando, porque oculta información.
3. **Cabeceras.** Una línea fija con «Teclado» y «Mando» sobre las columnas, atenuada, que no es una fila elegible. Si no cabe la altura (13 filas de 11 px desde `ROW_Y` = 52 llegan a 195 y el aviso está en 194), se baja `ROW_Y` solo en esta pestaña o se reduce el alto de las filas a 10; se decide midiendo en la captura.
4. **Varias asignaciones.** Se separan con « · » si el glifo existe, o con «/» si no; nunca con la coma pegada, que es lo que ahora se pierde. Si no caben, se recorta con «...».
5. **Etiquetas.** Pasan por `text_fit` y por `tr()`. Los botones de la consola mantienen su letra (B, Y, A, X, L, R, Select, Start) y la cruceta se traduce en español (Arriba, Abajo, Izquierda, Derecha). La fila del jugador usa «Jugador N» como selector con flechas, sin la etiqueta «Controles de».
6. **Glifos.** Se añaden `← ↑ → ↓` a `ext_glyphs` y, si falta, el separador. La prueba de glifos existente (caja y anchura) los cubre.

## Risks / Trade-offs

- [Los nombres largos de teclas poco comunes no caben en 80 px] → recorte con «...» y respaldo al nombre de SDL; se prueba con la tabla completa.
- [La altura del panel es justa] → medir en la captura y ajustar `ROW_Y`/`ROW_H` solo en esta pestaña, sin tocar las demás.
- [Cambiar la presentación puede romper los hashes de `--menu-shot` de otras pestañas] → las demás pestañas no se tocan; sus hashes deben quedar igual.
