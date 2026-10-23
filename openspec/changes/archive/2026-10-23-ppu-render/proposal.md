## Why

El runtime sin vídeo ya llena VRAM, CGRAM y OAM con los gráficos de ALttP (41 K de VRAM, 14 colores y 61 sprites a los 600 fotogramas), pero no sabemos qué pantalla representan ni si los datos son correctos: un tile mal copiado o una paleta desplazada solo se ve dibujándolos. Un renderizador mínimo que vuelque la imagen a un PNG permite comprobar a ojo que el código recompilado produce lo que produciría la consola, y es la base del renderizador completo.

## What Changes

- Nueva opción `--screenshot <fichero.png>`: al terminar la ejecución (también cuando se detiene por un fallo) el runtime dibuja el estado final de la PPU en una imagen de 256×224 y la escribe como PNG.
- Render del modo gráfico 1, el que usa ALttP en casi todas sus pantallas: BG1 y BG2 a 4 bits por píxel, BG3 a 2 bits, tilemaps de 32×32 hasta 64×64, tiles de 8×8 o 16×16, giros horizontal y vertical, paleta y prioridad por tile, y desplazamiento por fondo.
- Render de sprites desde la OAM: los ocho pares de tamaños, tabla de nombres, giros, paleta, prioridad y la coordenada X de 9 bits.
- Composición por prioridad con el orden del modo 1 (incluido el bit de prioridad del BG3), selección de capas de `$212C`, color de fondo, brillo de `$2100` y forced blank.
- Los registros de desplazamiento de fondo (`$210D-$2114`, de escritura doble) pasan a seguirse con su latch.
- El PNG se escribe sin dependencias, con bloques sin comprimir (código propio con su CRC32 y Adler-32).
- Lo que el renderizador no soporta se declara en el informe (otros modos gráficos, por ejemplo el modo 7), en lugar de dibujar algo incorrecto en silencio.
- Verificación: tests que decodifican el PNG de ROMs pequeñas y comprueban píxeles concretos, y una captura de ALttP que se mira a ojo y se anota.

## Capabilities

### New Capabilities

- `ppu-render`: dibujo de la imagen final de la PPU (fondos del modo 1, sprites, prioridad, brillo) y su escritura como PNG.

### Modified Capabilities

(ninguna)

## Impact

- `runtime/headless.c`: seguimiento de los registros de scroll, renderizador, escritor de PNG y opción `--screenshot`.
- `src/headless.rs`: tests de render con un decodificador mínimo de PNG sin comprimir, y la captura de ALttP.
- `CLAUDE.md`: la línea de ejecución documenta `--screenshot`.
- Fuera de alcance: modos 0, 2 a 7 (en particular el modo 7, que ALttP usa en el mapa y en la introducción), ventanas, mosaico, subpantalla y color math, entrelazado, alta resolución, el límite de 32 sprites por línea, y efectos por línea (HDMA, IRQ): se dibuja el estado de los registros al final de la ejecución, no línea a línea.
