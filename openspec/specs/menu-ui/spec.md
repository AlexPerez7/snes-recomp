# menu-ui Specification

## Purpose

Aspecto y comportamiento visual del menú F1 del frontal: que se vea y se maneje como el menú de un juego de SNES, con tipografía y adornos propios del proyecto.

## Requirements

### Requirement: Tipografía propia
El menú SHALL dibujar todo su texto con una fuente de píxeles proporcional definida en el código del proyecto, con sombra, que cubra los caracteres ASCII imprimibles; MUST NOT usar la fuente de la ROM ni de ningún recurso de Nintendo.

#### Scenario: Todos los caracteres tienen glifo
- **WHEN** se recorre el rango ASCII 32 a 126
- **THEN** cada carácter tiene un glifo que cabe en su caja y una anchura mayor que cero

#### Scenario: Medida de texto
- **WHEN** se mide una cadena
- **THEN** su anchura es la suma de las anchuras de sus glifos más el espaciado, y coincide con lo que ocupa al dibujarla

### Requirement: Panel, pestañas y cursor
El menú SHALL mostrarse como un panel con marco decorado sobre la imagen oscurecida, con barra de título, una barra de pestañas (Imagen, Audio, Controles, Juego) con la activa resaltada, un cursor visible en la fila elegida y una línea de ayuda con las teclas y botones del mando.

#### Scenario: Cambiar de pestaña
- **WHEN** el usuario pulsa la tecla de pestaña siguiente (o R del mando)
- **THEN** se muestra la pestaña siguiente con su primera fila elegida y la barra la marca como activa

#### Scenario: Texto largo
- **WHEN** el nombre de un dispositivo de audio no cabe en su fila
- **THEN** se recorta con «...» sin salirse del panel

### Requirement: Filas con valor
Cada fila SHALL mostrar su etiqueta a la izquierda y su valor a la derecha: un selector con flechas, una barra rellena (volumen) o una acción; un valor que no se puede mover más en un sentido MUST mostrar su flecha atenuada.

#### Scenario: Volumen
- **WHEN** el volumen es 50
- **THEN** su barra está rellena a la mitad y el número 50 se muestra a su lado

### Requirement: Compatibilidad con las pruebas sin ventana
El menú SHALL poder dibujarse sin ventana para las capturas de prueba (`--menu-shot`, `--menu-page`), con salida determinista, y MUST seguir presentándose por el mismo camino que la imagen del juego (aspecto, filtros y pantalla completa).

#### Scenario: Captura determinista
- **WHEN** se genera dos veces la captura de la misma pestaña con el mismo estado
- **THEN** los dos PNG son idénticos byte a byte
