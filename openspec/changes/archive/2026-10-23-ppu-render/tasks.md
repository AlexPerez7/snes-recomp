## 1. Base

- [x] 1.1 Escribir en `runtime/headless.c` el escritor de PNG (IHDR, IDAT en bloques stored con Adler-32, IEND, CRC32) y la opción `--screenshot <fichero>` que, por ahora, vuelca una imagen del color de fondo, también tras un fallo; añadir a `src/headless.rs` un decodificador mínimo de PNG sin comprimir; verificar con tests de la captura pedida, tras un fallo y sin la opción
- [x] 1.2 Aplicar la pantalla (color de fondo de la CGRAM, `$212C`, brillo, forced blank); verificar con tests de color de fondo, forced blank, brillo a medias y capa desactivada

## 2. Fondos

- [x] 2.1 Seguir los registros de desplazamiento `$210D-$2114` con el latch de la consola; verificar con un test de la escritura doble que lee el desplazamiento de vuelta (sobre el dibujo del fondo de la tarea siguiente)
- [x] 2.2 Dibujar BG1 y BG2 a 4 bits y BG3 a 2 bits del modo 1: tilemaps de los cuatro tamaños, tiles de 8×8 y 16×16, giros, paleta y transparencia, y desplazamiento; verificar con tests de un tile, giros, tiles de 16×16, tilemap grande, transparencia y desplazamiento
- [x] 2.3 Componer los fondos por prioridad con el orden del modo 1 y el bit 3 de `$2105`; verificar con tests de dos fondos de distinta prioridad y del BG3 al frente

## 3. Sprites

- [x] 3.1 Dibujar los sprites de la OAM (tamaños, nombres, giros, paleta, X de 9 bits, orden por índice) y mezclarlos por prioridad con los fondos; verificar con tests de sprite pequeño, X negativa, orden entre sprites, tamaño grande y sprite detrás de un fondo

## 4. Límites y ALttP

- [x] 4.1 Avisar en el informe de los modos gráficos distintos del 1, las ventanas, el mosaico y el color math; verificar con tests del modo 7 y del color math
- [x] 4.2 Capturar ALttP con `--screenshot` tras unos fotogramas, mirar la imagen y comprobar en una prueba que no es de un solo color; anotar en este fichero qué se ve en 120 y 600 fotogramas, qué falta respecto a la consola y por qué
- [x] 4.3 Documentar `--screenshot` en `CLAUDE.md`; verificar que el comando documentado funciona tal cual
- [x] 4.4 `cargo test` completo en verde

## Mediciones (ALttP USA, capturas a 120, 600 y 1500 fotogramas de 20.000 bloques)

- El renderizador funciona sobre el juego real: el texto «© 1991,1992 Nintendo» sale nítido y en su sitio, formado por 13 sprites de 8×8 de la primera tabla de nombres (paleta 5, `$2101 = 02`); a los 120 fotogramas solo ese texto y dos sprites en las esquinas inferiores, a los 600 y 1500 aparecen además líneas de letras doradas formadas por sprites de 16×16.
- El fondo es negro en todas las capturas (modo 1, `$2105 = 09`, `$212C = 10`: solo se activan los sprites); el único aviso es el de color math (`$2131` distinto de 0), que no se aplica.
- **Hallazgo sin resolver**: los sprites de 16×16 de la segunda tabla de nombres (los que forman las letras doradas, `attr` con el bit 0 a 1, tabla en la palabra `$5000`) se dibujan como letras con barras verticales. La traza de OAM y de VRAM confirma que el renderizador dibuja lo que hay: en esa zona de VRAM los datos de los tiles ya tienen ese aspecto. Hay dos explicaciones posibles que este cambio no distingue: que sean gráficos que el juego aún no ha terminado de subir (la subida de ALttP es incremental) o que los datos estén mal por un fallo del código recompilado en la descompresión de gráficos. Queda para investigarlo en un cambio aparte, por ejemplo comparando los bytes de VRAM con los descomprimidos a mano de la ROM.
- Lo que falta respecto a la consola, por orden de efecto: modo 7 (mapa del mundo y parte de la introducción), HDMA y efectos por línea (degradados, ondas), color math y subpantalla, ventanas, mosaico y el límite de 32 sprites por línea.
