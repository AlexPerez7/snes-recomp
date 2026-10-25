# Design

## Context

`interactive-frontend` presenta el búfer de 256x224 con una textura SDL escalada por entero y teclas fijas, y guarda `<rom>.keys` para el remapeo. Aquí se añaden las opciones esperadas de un port. Ver `proposal.md`.

## Goals / Non-Goals

**Goals:**
- Opciones de imagen, controles y audio persistentes, editables a mano y desde un menú.
- Que desactivadas, todas dejen la imagen idéntica a la del búfer (para poder comparar con capturas).

**Non-Goals:**
- Cambios en la lógica del juego (pantalla ancha, interpolación, calidad de vida).
- Interfaz con ratón o un lanzador gráfico.

## Decisions

**1. INI propio de claves planas.** Pocas claves, lectura trivial, comentarios y claves desconocidas conservados. *Alternativa:* TOML o JSON con biblioteca; descartada para no añadir dependencias al runtime en C.

**2. Un `Settings` único con valores por defecto en una tabla.** La misma tabla genera el INI por defecto, valida valores y rellena el menú, así que añadir una opción es una línea.

**3. Presentación por SDL con escalado en el renderizador.** `nearest` y `bilinear` usan `SDL_SetTextureScaleMode`; `sharp` se hace escalando primero a un múltiplo entero con `nearest` y luego a tamaño final con `bilinear`. Scanlines y grid se dibujan como una textura superpuesta de píxeles de pantalla, sin shaders de GPU. *Alternativa:* shaders GLSL/HLSL; descartada de momento por portabilidad y tamaño, y ampliable luego.

**4. Menú pintado con el propio búfer de fotograma.** Usa una fuente de bitmap incrustada de 8x8 (propia, sin copiar de otras), de modo que no depende de SDL_ttf.

**5. Menú y emulación.** El menú pausa la emulación y el audio, y reutiliza el estado de entrada de `live-input`; la configuración de teclas pasa de `<rom>.keys` a `config.ini` (se migra si el fichero antiguo existe).

## Risks / Trade-offs

- [`sharp` poco fino en escalas muy bajas] → comprobar visualmente con capturas a 1,5x y 2,5x; si no convence, documentarlo.
- [Menú pintado a mano es poco vistoso] → es deliberadamente simple; un menú más rico queda para un cambio posterior.
- [Dos fuentes de verdad para teclas (`.keys` y INI)] → se migra y se elimina `.keys` en el mismo cambio.

## Migration Plan

Si existe `<rom>.keys`, se importa a `config.ini` la primera vez y se deja de leer. Sin migración para el resto.

## Open Questions

- Si merece la pena un backend de shaders de GPU para filtros CRT completos (se decide después, no cambia los specs).
