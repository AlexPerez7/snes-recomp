## Context

`runtime/headless.c` guarda ya VRAM, CGRAM y OAM, y el último valor de cada registro de `$2100-$21FF` en `hw[]`. Los registros de desplazamiento (`$210D-$2114`) son de escritura doble con un latch compartido, así que su último valor escrito no basta. ALttP, a los 600 fotogramas, deja `$2100 = 0F`, `$2105 = 09` (modo 1 con prioridad del BG3), 41 K de VRAM no nula y 61 sprites en pantalla. El runtime no tiene líneas de exploración: el estado de la PPU es único para todo el fotograma.

## Goals / Non-Goals

**Goals:**
- Una imagen del estado final que permita comprobar a ojo y con tests de píxeles que los datos y registros son correctos.
- El modo 1 completo en lo que ALttP usa casi siempre (fondos, sprites, prioridad, brillo).
- Que lo no soportado se declare.

**Non-Goals:**
- Fidelidad de temporización o de línea: ver `proposal.md` (HDMA, IRQ, modo 7, ventanas, color math, mosaico, límite de sprites por línea).
- Salida en vivo o animada; solo una captura al terminar.

## Decisions

1. **Se dibuja una vez, al terminar, desde el estado de `hw[]` y las memorias.** No hay estado de línea. Alternativa: capturar en cada fotograma; descartada porque el valor está en poder ver el estado tras N fotogramas, y repetirlo es pasar `--frames N` distinto.
2. **Composición por píxel.** Para cada píxel de la imagen se recorre la lista ordenada de capas (según el bit 3 de `$2105`) y gana el primer píxel no transparente; si ninguno, el color 0. Es simple y exacto para el modo 1; el coste (224×256 píxeles × 10 capas) es despreciable para una captura. Alternativa: rasterizar por capas y mezclar; descartada por duplicar la lógica de prioridad.
3. **Sprites rasterizados antes en un búfer.** Se recorren los sprites de índice 127 a 0 escribiendo en un búfer de 256×224 con color y prioridad, de modo que el de índice menor queda encima. Durante la composición, la entrada de sprite para la prioridad *n* se toma del búfer solo si su prioridad es *n*. Esto representa que un sprite oculto por otro de la misma prioridad no deja ver el que está debajo, aunque este tenga otra prioridad: es el comportamiento de la consola, que decide por orden en la línea antes de mezclar.
4. **Scroll con latch propio.** Cada escritura a `$210D-$2114` actualiza el desplazamiento de su fondo con el latch compartido: horizontal `(v<<8) | (latch & ~7) | ((hofs>>8) & 7)`, vertical `(v<<8) | latch`, y después `latch = v`. Los desplazamientos se guardan enteros de 10 bits.
5. **Tiles de 16×16 como cuatro de 8×8.** Se calcula el tile de 8×8 dentro del de 16×16 con los giros aplicados a la posición, y el número de tile suma 0, 1, 16 o 17. Para el tilemap, el cuadrante se elige según el tamaño del mapa: pantallas de 32×32 palabras en orden `[0, 1; 2, 3]` según los bits de tamaño (ancho +0x400, alto +0x400 o +0x800).
6. **Colores.** Cada color BGR555 pasa a RGB de 8 bits con `c8 = (c5 << 3) | (c5 >> 2)`. El brillo `b` de `$2100` se aplica como `c8 * b / 15`. Con forced blank la imagen es toda negra.
7. **PNG sin compresión.** Una cabecera IHDR de color RGB 8 bits, un IDAT con zlib en bloques «stored» de hasta 65 535 bytes (con su Adler-32), y el CRC32 de cada trozo, todo en unas decenas de líneas de C. Alternativa: usar zlib o stb; descartada por la regla de no añadir dependencias ni copiar código de otros proyectos. La imagen pesa 172 KB, aceptable para una captura.
8. **Avisos de lo no soportado.** Al terminar, el informe lista el modo gráfico si no es el 1, y si `$212E-$2133` indican ventanas o color math activos, o `$2106` mosaico. El renderizador dibuja los sprites y el fondo de color en todos los casos y los fondos solo en el modo 1.
9. **Verificación de los tests con un decodificador mínimo.** Como el PNG no está comprimido, el test lo decodifica concatenando los bloques stored (sin filtros: se escribe el filtro 0 en todas las líneas) y compara píxeles concretos. Las ROMs pequeñas preparan VRAM, CGRAM y OAM con instrucciones normales.

## Risks / Trade-offs

- [El estado final puede no ser una pantalla «completa»: ALttP deja efectos de HDMA a mitad de línea y escribe registros con valores propios del siguiente fotograma] → la captura es de lo que dejan los registros al terminar; la comparación es a ojo y se anota. Si una pantalla sale mal por esto, no es un fallo del renderizador.
- [El orden de composición de sprites contra fondos tiene casos finos] → se cubre con tests de las combinaciones de la spec; el resto se valida a ojo con ALttP.
- [El modo 7, que ALttP usa en el mapa del mundo y la introducción, queda sin dibujar] → se declara; es el siguiente cambio natural.
- [Un PNG sin comprimir es grande] → es una captura puntual; no hay otra salida.
