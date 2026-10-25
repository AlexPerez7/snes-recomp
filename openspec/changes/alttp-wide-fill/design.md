# Design

## Context

Ver proposal.md. `wide-display` obliga al motor a no rellenar ni repetir el borde, así que el relleno es cosa del C del juego, que dispone de `wram_read8/wram_write8`, `game_opt()` y ganchos `before/after/replace`. Hoy `games/alttp_usa/dialog.c` es el único C propio.

Hipótesis a verificar en la ROM antes de implementar (no comprobadas):

- El exterior de ALttP guarda su tilemap en VRAM como 64×64 teselas (512 px de ancho) y se muestran 256 px.

- Los interiores tienen tilemaps de cuarto sin continuación fuera de los límites del cuarto.

Observado el 2026-10-25 (captura con `--wide 96` caminando por las áreas 2B/2C): la cámara (`$E2`) se queda clavada en el borde del área (`0x0800`) y el juego **no carga columnas al desplazarse**. Con la cámara en mitad del área, los `E` píxeles extra caen dentro de lo ya cargado y se ven bien. En el borde del área caen en la zona del tilemap que envuelve y muestran el lado opuesto de la misma área (cortes visibles a x≈96 y x≈352 con `E = 96`). El contenido del área vecina solo se carga en la transición entre áreas. 

Confirmado con volcados (área 2C, cámara en el borde izquierdo): `$E2` = `0x0800` es la X absoluta de la cámara (el área 2C empieza en `0x0800`; cada área mide 512×512 px y su columna es `área & 7`). El terreno está en BG2 (`$2108` = `$03`: base `$0000`, 64×64 teselas) y BG1 (`$2107` = `$13`: base `$1000`, 64×64) es una capa casi vacía (tesela 511). El tilemap de BG2 contiene el área completa y nada más: sus columnas 52 a 63, que son las que la PPU enseña a la izquierda de la 0, tienen el borde derecho del propio área. `$8A` es el área actual y `$1B` distingue interior (≠0) de exterior (0).


Investigación de la transición entre áreas (2.3, en curso; área 2C hacia 2B, módulo `09`):

- La transición va por submódulos de `$11`: en el fotograma 7355 (sub 03) cambia `$8A` al área vecina y el bloque `02:F449` (rutina `02:F3xx-F4xx`) rellena de golpe un búfer en `$1104-$17FF` con 13 columnas del vecino. El NMI (`00:8D3A`) las sube por DMA, pares de 64 bytes a `VRAM $0400+c` y `$0C00+c` (las dos pantallas derechas del tilemap de 64×64), con `c` de 30 a 18, es decir las columnas 62 a 50: exactamente la franja que `wide` enseña a la izquierda de la columna 0.
- Después, con la cámara moviéndose 8 px por fotograma (`02:C025`, que suma `$BEBA,Y` a `$E2`), sube dos columnas cada 2 fotogramas (`c` = 16, 14, 12…).
- La rutina convierte Map16 a teselas leyendo la cuadrícula Map16 del área desde WRAM por puntero largo (banco `7E`) y la tabla de teselas de `0F:8000` (8 bytes por Map16).
- **Comprobado: el Map16 del vecino no está en WRAM antes de la transición.** La cuadrícula Map16 es de 64×64 entradas de 16 bits en `7E:2000-3FFF` (1024×1024 px, la mitad del área actual y su lado ya cargado). Entre el fotograma 7354 (sub 02) y el 7358 (sub 06) el contenido anterior se desplaza 32 columnas Map16 (coincidencia del 100 % en las 2048 celdas solapadas) y la mitad nueva se descomprime desde la ROM. Es la misma cuadrícula que usa la lógica del juego, así que escribir en ella el vecino rompería lo que ve el juego (colisiones) si no se guarda y se restaura.
- Pendiente de comprobar: qué rutinas descomprimen la mitad nueva y con qué variables, y qué variables (`$84`, `$86`, `$0418`, `$0410`, `$0E`) parametrizan la construcción de columnas.
- Las herramientas de traza usadas (DMA y escrituras a VRAM con su bloque) están en una copia del runtime generado en `out/`, no en el motor.

## Goals / Non-Goals

**Goals:**
- Que la zona nueva nunca muestre basura en ALttP: contenido real donde hay mapa cargado y negro donde no.
- Sin efecto con `E = 0`.

**Non-Goals:**
- Cargar contenido del área vecina (precarga real): queda como trabajo futuro, con lo averiguado arriba.
- Enemigos, objetos y sprites fuera de los 256 px (cambio posterior).
- Que el motor rellene o repita bordes por su cuenta.

## Decisions

- **Tapar en negro en vez de precargar (opción B).** Precargar el vecino exige descomprimir su mapa de la ROM, mover la cuadrícula Map16 compartida con la lógica y restaurarla, con riesgo de romper colisiones. Tapar cumple el objetivo de no mostrar basura con una fracción del coste. Alternativa: opción A (precarga real), reservada para un cambio posterior.
- **Las franjas las dibuja el motor, las decide el juego.** Un API `ppu_wide_blank(left, right)` en `snes_rt.h` (ver delta de `wide-display`) permite al C del juego pedir cuántos píxeles de cada lado se dibujan en negro. Alternativa descartada: que el C del juego sobrescriba columnas del tilemap en VRAM, porque esas columnas contienen datos reales del otro borde del área (se verían al volver la cámara) y habría que guardarlas y restaurarlas a cada fotograma.
- **Exterior: franjas según los límites de scroll del propio juego.** `$E2` es la X de la cámara y el juego guarda los límites de la zona de scroll horizontal en `$0604` (izquierdo) y `$0606` (derecho); `$0600`/`$0602` son los verticales (comprobado: `$E8` = `$0602` en el borde inferior). Se actualizan al cruzar a otra área (área `0x2C`: 2048..2304; al pasar a `0x2B`: 1536..1792, para áreas de 512 px). La franja izquierda es `max(0, E - (E2 - límite_izquierdo))` y la derecha `max(0, E - (límite_derecho - E2))`. A comprobar en la ROM (tarea 2.3) antes de implementar, también con áreas grandes.
- **Salas: franjas completas a ambos lados** como primera versión, por seguridad. Más adelante se puede aprovechar el contenido real cuando la cámara de la sala no está en un borde.
- **`E` se lee con un accesor nuevo en `snes_rt.h`** (`ppu_wide()`), no con `game_opt`, porque `E` cambia en marcha desde el menú y `game_opt` es una opción fija de la ejecución.
- **Exterior o interior** por `$1B` (0 exterior, distinto de 0 interior), a contrastar con capturas (tarea 2.2).

## Risks / Trade-offs

- [Los límites `$0604/$0606` pueden no valer fuera de transiciones, o no reflejar el borde real] → verificarlos con capturas a ambos lados y con `--poke`; si no sirven, usar la X del área (`$8A & 7` por 512) con el tamaño del área.
- [Franja visible como banda negra en el borde del área, menos vistosa que una precarga] → es el compromiso aceptado; la opción A lo cierra.
- [Transiciones exterior-interior o de área dejan una franja mal puesta durante unos fotogramas] → recalcular las franjas cada fotograma y comprobar con capturas durante una transición.
- [El gancho que fija las franjas debe correr cada fotograma, también durante mensajes y transiciones] → engancharlo a una rutina que corra siempre (a localizar), no solo a la de movimiento.

## Future work

- **Precarga real del exterior (opción A):** descomprimir el vecino con las rutinas del juego, construir las columnas con la rutina de `02:F3xx-F4xx`, subirlas al tilemap y guardar y restaurar la cuadrícula Map16 de `7E:2000-3FFF`. Datos de partida: la sección sobre la transición entre áreas.
- Piezas 3 y 4 (activar enemigos y objetos fuera de los 256 px y sprites a la derecha con `ppu_sprite_xoff`).

## Resultados de la comprobación en la ROM (tareas 2.3, 2.4 y 4.1)

- Límites horizontales: `$0604` (mínimo) y `$0606` (máximo) de la X de la cámara; `$0600`/`$0602` son los verticales. `$E2` es la X de la cámara (ya comprobado en `0x2C`: 2048..2304; en `0x2B`: 1536..1792).
- Durante la transición entre áreas (`$10 = 9`, `$11 = 6`) los límites aún son los del área anterior y la cámara los rebasa, así que el lado del que se sale queda negro entero hasta que el juego actualiza los límites; no queda basura.
- La franja se cuenta desde el borde exterior hacia la imagen (lo que hay más allá del final del mapa), no desde la imagen: se corrigió `ppu_wide_blank` en el motor al probar el caso parcial.
- Interior: `$1B != 0` ⇒ franjas completas; se probó en la casa de Link. Salida de la casa a `0x2C` sin basura en ningún fotograma.
- Gancho por fotograma: `after` en `00:80B5` (el enrutador de módulos que llama el bucle principal una vez por fotograma tras esperar al NMI). En los módulos menores que 6 (título, selección de partida, nombre) se quitan las franjas.
- `E = 0`: capturas idénticas byte a byte con el gancho y con un gancho vacío (fotogramas 5300, 5730, 6700 y 6850).
- Sin medir: tiempo con `--stats`, cuevas (comparten la ruta de `$1B != 0` con la casa) y un área de 1024 px.
