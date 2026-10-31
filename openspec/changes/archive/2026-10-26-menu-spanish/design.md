## Contexto

El menú del frontal (`runtime/play.c`) dibuja con la fuente propia de `runtime/play_ui.h` (5x7 engrosada, proporcional, solo ASCII 32–126, filas de 11 px). Las etiquetas, los avisos y la ayuda son literales C repartidos por `play.c`; los nombres de los valores salen de `opt_defs` (`names` = `"a|b|c"`) en `play_cfg.h`, que se usan también para leer y escribir `config.ini`. Las opciones de juego del perfil (`snes_game_opts`, valor en `gopt_eff[i]`) ya existen y `language` es una de ellas en ALttP.

## Decisiones

1. **El idioma sale de la opción de juego `language`.** Se busca en `snes_game_opts` la opción con clave `language` y se mira si el nombre del valor vigente es `es`. No hay opción propia del frontal: así no hay dos ajustes que se contradigan y el cambio en la pestaña Game se ve al instante, porque se consulta al dibujar. Alternativa descartada: una clave `menu_lang` en `config.ini`; añade una opción más y deja el menú en un idioma distinto al del juego.
2. **Tabla de cadenas dentro del motor.** Una función `tr(const char *en)` devuelve la traducción si el idioma es español y la cadena tiene entrada, y el original si no. La tabla es un array de pares `{en, es}` en un `.h` sin SDL (`runtime/play_i18n.h`, probable con las pruebas que ya compilan `play_cfg.h` solo). Buscar por el texto en inglés evita tocar la estructura del menú y da el respaldo en inglés gratis. Alternativa descartada: índices de enumeración; obligan a reescribir cada punto de uso y no tienen respaldo.
3. **Se traduce al dibujar, no al guardar.** Las claves y los valores de `config.ini` siguen usando `names` tal cual. Para mostrar el valor se pasa `tr(nombre)` solo en el punto de dibujo. `--print-config`, los comentarios del fichero y la consola no llaman a `tr`.
4. **UTF-8 en la fuente.** Los textos del menú son UTF-8 (el fichero fuente es UTF-8). `play_ui.h` gana una función de decodificación que devuelve el punto de código y su longitud en bytes; `text_w`, `text_draw`, `text_fit` y `text_draw2x` la usan. Un byte inválido o truncado se trata como `?`. El recorte con «...» corta siempre en frontera de carácter.
5. **Glifos nuevos con acento por encima.** Se añaden `á é í ó ú ü ñ Á É Í Ó Ú Ü Ñ ¿ ¡`. Las letras acentuadas usan la misma forma que la letra base y un acento de 1 o 2 filas por encima de la caja 5x7; la caja del glifo pasa a tener una fila extra arriba para ello (o se reutiliza el hueco que ya hay entre filas de 11 px, a comprobar al implementar). El engrosado y la sombra se aplican igual. Una prueba comprueba que ningún píxel cae fuera de la altura de la fila.
6. **Las cadenas largas se miden en pruebas.** Una prueba recorre la tabla y mide cada traducción con `text_w` contra el hueco donde se dibuja (etiqueta de fila, columna de valor, nombre de pestaña, línea de aviso, línea de ayuda). Lo que se desborde se acorta en la tabla, no en el código.

## Riesgos

- El español es más largo que el inglés: la ayuda del pie y los avisos son lo más justo. Mitigación: la prueba de anchura y textos compactos («Intro: salir, Esc: cancelar»).
- Cambiar el alto del glifo puede mover el dibujo del menú en inglés y romper los hashes de `--menu-shot`. Mitigación: que el inglés quede píxel a píxel igual (los glifos ASCII no cambian) y comprobarlo con las pruebas existentes.
- Los valores guardados (`nearest`, `sharp`...) se muestran traducidos pero se escriben en inglés; es intencionado y se documenta.

## Pendiente de decidir al implementar

- Si `play_i18n.h` es un fichero propio o va dentro de `play_cfg.h` (depende de cuánto crezca la tabla).
