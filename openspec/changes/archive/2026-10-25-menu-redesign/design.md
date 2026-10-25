## Context

El menú se dibuja por software en un búfer de 256x224 (`fb_menu`) que luego pasa por el mismo `present()` que la imagen del juego, así que hereda aspecto, filtros y pantalla completa. Hoy usa la fuente 5x7 (`font5x7`), oscurece la imagen a un tercio y escribe líneas de texto. La lógica (páginas, selección, remapeo, confirmación de conflictos) está en `play.c` y se prueba con `--menu-keys`/`--menu-shot`.

## Goals / Non-Goals

**Goals:**
- Que el menú parezca parte de un juego de SNES de la época: marco, pestañas, cursor, tipografía con personalidad.
- Más opciones útiles sin tocar la lógica de remapeo.
- Que lo puramente gráfico (medida de texto, glifos, rectángulos) se pueda probar sin SDL.

**Non-Goals:**
- Reutilizar fuentes o gráficos de Nintendo, ni copiarlos de la ROM. La fuente y los adornos se dibujan aquí.
- Reiniciar la consola desde el menú (el runtime con contextos de ejecución no lo permite de forma limpia; queda fuera).
- Ratón o pantallas táctiles.

## Decisions

**1. Misma resolución, 256x224.** El menú se compone en el búfer de la imagen. Alternativa: dibujar a la resolución de la ventana con SDL; se descarta porque rompería las capturas deterministas y la coherencia con los filtros.

**2. Fuente propia, proporcional, 8 px de alto con sombra.** Cada glifo es una tabla de filas de bits con su ancho (de 3 a 8 px), con verticales de 2 px y remates marcados, para que se lea «de aventura» y no «de consola de depuración». Se dibuja con un píxel de sombra oscura abajo a la derecha. Cubre ASCII 32–126 más flechas (`◄ ►`). Vive en `runtime/play_ui.h` junto a `text_width()` para poder centrar y alinear. Alternativa: fuente OFL incluida (el usuario eligió la propia).

**3. Paleta y marco propios.** Panel azul noche con degradado vertical sutil, borde doble (claro por fuera, oscuro por dentro), esquinas con adorno de 5x5 y una línea dorada bajo el título. Sin triforce ni iconos reconocibles de la saga: formas geométricas genéricas.

**4. Pestañas en lugar de lista de páginas.** Barra superior con cuatro pestañas; L/R del mando y Q/W del teclado cambian de pestaña (izquierda/derecha cambia el valor de la fila), arriba/abajo recorre las filas. Alternativa: mantener páginas anidadas; se descarta por más pulsaciones.

**5. Filas con tres tipos de valor.** Selector (`◄ valor ►`), barra (volumen, con relleno) y acción (texto centrado). Cada fila se describe con una pequeña tabla (etiqueta, tipo, opción) para no repetir código de dibujo. La pestaña de Controles lista los 12 botones de un jugador con un selector de jugador arriba.

**6. Cursor.** Una flecha de 8x8 dibujada por código a la izquierda de la fila elegida, con un balanceo de 1 px cada ~20 fotogramas (el menú anima con un contador propio, ya que la emulación está en pausa). Fila elegida resaltada con una barra más clara.

**7. Pestaña Juego.** Continuar (cierra el menú), Guardar ahora (fuerza el volcado de la SRAM a `.srm`), Restaurar valores por defecto (de todo, pidiendo confirmación) y Salir. En las pestañas Imagen y Audio, una fila «Restaurar» que solo afecta a esa pestaña. La opción `fps` (mostrar contador) va en la pestaña Imagen.

**8. Pruebas.** `play_ui.h` compila solo y se prueba en C: cada glifo cabe en su caja, `text_width` suma anchos, el marco no escribe fuera del búfer. Las capturas de cada pestaña se guardan en `out/` y se revisan a ojo (tarea de verificación manual explícita). Las secuencias `--menu-keys` de los tests existentes se actualizan.

## Risks / Trade-offs

- [Estética subjetiva] → se itera con capturas ampliadas y el usuario decide al final; la fuente es datos, cambiarla es barato.
- [Texto largo en 256 px] → nombres de dispositivo de audio y teclas asignadas se recortan con «...» según `text_width`.
- [Ruptura de `--menu-keys`] → solo afecta a pruebas; se documenta en el README.

## Open Questions

- Ninguna que bloquee: si a 1x la fuente no se lee bien, se dobla la escala del texto de los títulos.
