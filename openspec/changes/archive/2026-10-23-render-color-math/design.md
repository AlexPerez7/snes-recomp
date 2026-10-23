## Context

`screenshot()` en `runtime/headless.c` calcula por píxel un índice de CGRAM con `pixel_index` y le aplica el brillo. No hay color math. El diagnóstico (ver `tasks.md` del cambio `render-color-math-hdma`, descartado y sustituido por este) mostró que ALttP lo usa en la pantalla de título con `$2130`=`02`, `$2131`=`22`.

## Goals / Non-Goals

**Goals:**
- Color math correcto para el color fijo y la subpantalla, con las regiones `siempre` y `nunca`.

**Non-Goals:**
- HDMA, ventanas, mosaico y modos distintos del 1.
- Explicar el forced blank de ALttP tras el fotograma ~1054.

## Decisions

1. **`pixel_index` devuelve color y capa.** Para decidir si la capa está elegida en `$2131` hace falta saber qué capa aportó el píxel (BG1, BG2, BG3, sprite, fondo). Se cambia para devolver el índice de CGRAM y un identificador de capa.
2. **La subpantalla reutiliza la misma ruta** con la máscara de `$212D` en lugar de la de `$212C`. Sin píxel de subpantalla se usa el color fijo.
3. **Cálculo en 5 bits por canal.** Se opera sobre los valores de 15 bits de la CGRAM y solo después se pasa a 8 bits con el brillo.
4. **Sprites con paleta 0-3 (colores 128-191) no se mezclan**, como en el hardware: solo los de paleta 4-7 participan; se sigue la regla estándar del bit de capa de objetos.

## Risks / Trade-offs

- [La suma con la subpantalla tiene casos límite en el hardware (mitad solo con subpantalla)] → se implementa la regla de que el color fijo no se divide por 2 cuando es el operando por defecto del fondo, y se cubre con un test.
- [Sin HDMA, el `$2132` por fotograma será el último valor] → aceptado; es el límite declarado del cambio.
