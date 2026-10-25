# ppu-scanline-render Specification

## Purpose

Calcular la imagen de la PPU línea a línea con los registros, el scroll y las memorias vigentes en cada línea, para todos los modos gráficos, de modo que los efectos de raster de los juegos se vean como en la consola.

## Requirements

### Requirement: Registro de escrituras por línea
El runtime SHALL anotar con la línea de pantalla en curso cada escritura a `$2100-$213F` y los cambios de VRAM, CGRAM y OAM, y SHALL dibujar cada línea con el estado resultante de las escrituras anteriores a ella. Las escrituras hechas en el vblank MUST aplicarse desde la primera línea del fotograma siguiente.

#### Scenario: Scroll cambiado a mitad de pantalla
- **WHEN** un manejador de IRQ cambia `$210D` en la línea 100
- **THEN** las líneas 0 a 99 se dibujan con el scroll anterior y desde la 100 con el nuevo

#### Scenario: Escritura en el vblank
- **WHEN** el juego cambia el brillo `$2100` durante el vblank
- **THEN** la línea 0 del fotograma siguiente usa el brillo nuevo

### Requirement: HDMA por línea
El runtime SHALL ejecutar las transferencias HDMA de los canales de `$420C` al principio de cada línea visible, con tablas directas e indirectas y modos de repetición, y el efecto MUST verse en esa misma línea.

#### Scenario: Ventana por HDMA
- **WHEN** el canal 7 escribe `$2126/$2127` con valores distintos por línea
- **THEN** la ventana 1 tiene en cada línea el borde que dicta la tabla

### Requirement: Modos de fondo 0 a 6
El dibujador SHALL soportar los modos de fondo 0 a 6 con sus profundidades de color, número de fondos y prioridades propias, incluidos BG4, el offset por tile de los modos 2, 4 y 6, los fondos de 16x16 y las resoluciones de 512 de ancho de los modos 5 y 6, y SHALL soportar el modo 7 con matriz, centro y scroll por línea.

#### Scenario: Modo 0 con cuatro fondos
- **WHEN** `$2105` selecciona el modo 0 con los cuatro fondos activados y datos distintos en cada uno
- **THEN** la imagen muestra los cuatro con 2 bits por píxel y su paleta propia

#### Scenario: Offset por tile
- **WHEN** el modo 2 tiene en BG3 una tabla de offsets distinta por columna
- **THEN** cada columna de BG1 y BG2 se desplaza según esa tabla

#### Scenario: Modo de 512 de ancho
- **WHEN** `$2105` selecciona el modo 5 o 6
- **THEN** la imagen horizontal se calcula a 512 píxeles y se reduce a 256 para presentarla

### Requirement: Sprites completos
El dibujador SHALL dibujar los 128 sprites con los tamaños de `$2101`, el giro horizontal y vertical, las paletas, las prioridades y el límite de 32 sprites y 34 tiles por línea.

#### Scenario: Límite por línea
- **WHEN** 33 sprites coinciden en una línea
- **THEN** solo se dibujan los 32 de menor índice en esa línea

### Requirement: Ventanas, mosaico y color math
El dibujador SHALL aplicar las dos ventanas con sus operadores lógicos a cada fondo, a los sprites y al color math, el mosaico por fondo, y el color math sobre pantalla principal y subpantalla con medio brillo y color fijo.

#### Scenario: Mosaico
- **WHEN** `$2106` activa el mosaico de tamaño 4 en BG1
- **THEN** BG1 se dibuja en bloques de 4x4 píxeles

### Requirement: Reproducibilidad
El dibujador MUST producir la misma imagen para la misma secuencia de escrituras y líneas, sin depender del reloj real, para poder compararla en pruebas.

#### Scenario: Dos ejecuciones
- **WHEN** se ejecutan 600 fotogramas dos veces con las mismas opciones
- **THEN** los PNG del fotograma final son idénticos byte a byte
