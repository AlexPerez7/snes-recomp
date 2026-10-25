# ppu-render Specification

## Purpose

Dibujar la imagen que la PPU mostraría con el estado final de sus registros y memorias, y guardarla como PNG, para comprobar a ojo que el código recompilado prepara los gráficos correctos sin necesitar todavía un renderizador completo.

## Requirements

### Requirement: Captura de la imagen final
El runtime SHALL escribir con `--screenshot <fichero>`, al terminar la ejecución y también cuando se detiene por un fallo, un PNG de `256 + 2E` × 224 píxeles (256×224 cuando no se pide ensanche) con el último fotograma completo que dibujó la PPU (o, si no llegó a completarse ninguno, el estado de ese momento), y MUST no escribir nada si no se pide. El fichero SHALL ser un PNG válido de 8 bits por canal RGB que cualquier lector de imágenes pueda abrir.

#### Scenario: Captura pedida
- **WHEN** se ejecuta con `--screenshot s.png`
- **THEN** `s.png` existe, tiene la firma PNG y las dimensiones 256×224

#### Scenario: Captura con ensanche
- **WHEN** se ejecuta con `--screenshot s.png --wide 32`
- **THEN** `s.png` tiene las dimensiones 320×224

#### Scenario: Captura tras un fallo
- **WHEN** la ejecución se detiene por `rt_unresolved` con `--screenshot s.png`
- **THEN** `s.png` se escribe igualmente

#### Scenario: Sin opción
- **WHEN** no se pasa `--screenshot`
- **THEN** no se escribe ninguna imagen

### Requirement: Pantalla y brillo
El runtime SHALL dibujar el color de fondo (el color 0 de la CGRAM) donde ninguna capa aporta un píxel, MUST dibujar solo las capas activadas en la pantalla principal (`$212C`), y SHALL aplicar el brillo de `$2100` (bits 0-3, de 0 negro a 15 pleno) y el forced blank (bit 7), que deja la imagen en negro.

#### Scenario: Color de fondo
- **WHEN** ninguna capa está activada y el color 0 de la CGRAM es rojo
- **THEN** todos los píxeles son rojos

#### Scenario: Forced blank
- **WHEN** `$2100` tiene el bit 7 a 1
- **THEN** todos los píxeles son negros, sea cual sea el contenido de las memorias

#### Scenario: Brillo
- **WHEN** `$2100` vale `07` y un píxel tiene el canal verde al máximo
- **THEN** su verde es aproximadamente la mitad del máximo

#### Scenario: Capa desactivada
- **WHEN** un fondo tiene datos pero su bit de `$212C` está a 0
- **THEN** no se dibuja

### Requirement: Fondos del modo 1
El runtime SHALL dibujar en el modo 1 los fondos BG1 y BG2 a 4 bits por píxel y BG3 a 2 bits por píxel, leyendo cada uno su tilemap (base y tamaño 32×32, 64×32, 32×64 o 64×64 de `$2107-$2109`) y sus tiles (base de `$210B-$210C`), con tiles de 8×8 o de 16×16 según los bits 4-6 de `$2105`, y para cada entrada del tilemap el número de tile, la paleta, el giro horizontal y el giro vertical. El color 0 de cada paleta MUST ser transparente.

#### Scenario: Un tile
- **WHEN** el tilemap de BG1 tiene en la posición (0,0) un tile cuyo píxel (2,3) usa el color 5 de la paleta 1
- **THEN** el píxel (2,3) de la imagen tiene el color `16 + 5` de la CGRAM

#### Scenario: Giros
- **WHEN** la misma entrada tiene el giro horizontal activo
- **THEN** el píxel (2,3) del tile aparece en la columna (7−2)

#### Scenario: Tiles de 16×16
- **WHEN** BG1 usa tiles de 16×16 y la entrada es el tile `t`
- **THEN** se dibujan los tiles `t`, `t+1`, `t+16` y `t+17` como un cuadrado de 16×16, con los giros intercambiando la posición de los cuatro

#### Scenario: Tilemap grande
- **WHEN** BG1 es de 64×64 y se mira el píxel (300, 300)
- **THEN** se lee el cuarto del tilemap que corresponde, en el orden de la consola

#### Scenario: Transparencia
- **WHEN** un píxel de un tile vale 0
- **THEN** se ve la capa de debajo

### Requirement: Desplazamiento de los fondos
El runtime SHALL seguir los registros de desplazamiento `$210D-$2114`, que se escriben dos veces, con el latch de la consola, de modo que cada fondo se dibuje desplazado en horizontal y en vertical de 10 bits.

#### Scenario: Escritura doble
- **WHEN** el código escribe `$34` y luego `$01` en `$210D`
- **THEN** el desplazamiento horizontal de BG1 vale `$134`

#### Scenario: Desplazamiento
- **WHEN** BG1 tiene el desplazamiento horizontal 8
- **THEN** el píxel (0,0) de la imagen es el píxel (8,0) del fondo

### Requirement: Sprites
El runtime SHALL dibujar los 128 sprites de la OAM con el par de tamaños elegido en los bits 5-7 de `$2101`, la base de nombres y la separación de la segunda tabla de `$2101`, 4 bits por píxel con paletas desde el color 128, giros, la coordenada X de 9 bits con signo y la Y desplazada una línea, y los sprites de índice menor MUST quedar encima de los de índice mayor con la misma prioridad.

#### Scenario: Un sprite pequeño
- **WHEN** el sprite 0 es de 8×8 en (10, 20) con un píxel de color 3 de la paleta 2 en su posición (1,1)
- **THEN** el píxel (11, 22) de la imagen tiene el color `128 + 2*16 + 3` de la CGRAM

#### Scenario: X negativa
- **WHEN** un sprite tiene la X de 9 bits igual a 508
- **THEN** se dibuja cortado por el borde izquierdo de la pantalla

#### Scenario: Orden entre sprites
- **WHEN** dos sprites de la misma prioridad se solapan
- **THEN** se ve el de índice menor

#### Scenario: Tamaño grande
- **WHEN** el bit de tamaño de un sprite está activo y el par de tamaños es 8×8 / 32×32
- **THEN** se dibuja de 32×32, con las filas de tiles separadas 16 números de tile

### Requirement: Prioridad entre capas
El runtime SHALL componer las capas del modo 1, de delante hacia atrás, en este orden: con el bit 3 de `$2105` a 0, sprites de prioridad 3, BG1 y BG2 de prioridad alta, sprites de 2, BG1 y BG2 de prioridad baja, sprites de 1, BG3 de prioridad alta, sprites de 0, BG3 de prioridad baja; y con ese bit a 1, el BG3 de prioridad alta pasa al frente de todos.

#### Scenario: Sprite detrás de un fondo
- **WHEN** un sprite de prioridad 1 coincide con un píxel de BG1 de prioridad baja
- **THEN** se ve el de BG1

#### Scenario: BG3 al frente
- **WHEN** el bit 3 de `$2105` es 1 y un tile de BG3 de prioridad alta coincide con un sprite de prioridad 3
- **THEN** se ve el de BG3

### Requirement: Límites declarados
El runtime SHALL avisar en el informe cuando la imagen use una función que no dibuja, que tras este cambio se reducen al entrelazado y a las resoluciones de alta resolución de `$2133`. MUST dibujar en esos casos lo que sí sabe dibujar y no inventar el resto. La captura SHALL usar el mismo dibujador por línea que el ejecutable interactivo y mostrar el último fotograma completo; si la ejecución se detiene antes de acabar el primero, SHALL dibujar las líneas que faltan con el estado de ese momento.

#### Scenario: Otro modo gráfico
- **WHEN** `$2105` selecciona el modo 0, 2, 3, 4, 5, 6 o 7
- **THEN** la captura dibuja las capas de ese modo y el informe no avisa de modo no dibujado

#### Scenario: Color math
- **WHEN** `$2131` tiene alguna capa activada y los bits 4-5 de `$2130` valen 0 o 3
- **THEN** el color math se aplica y el informe no avisa de que no se aplique

#### Scenario: Color math con ventana
- **WHEN** los bits 4-5 de `$2130` piden una región que depende de la ventana de color
- **THEN** la región se aplica y el informe no avisa

#### Scenario: Entrelazado
- **WHEN** `$2133` activa el entrelazado de imagen
- **THEN** el informe avisa de que el entrelazado no se aplica

### Requirement: Color math
El runtime SHALL aplicar el color math de `$2130-$2132` a cada píxel de la pantalla principal: tomar como segundo operando el color fijo de `$2132`, o la subpantalla (capas de `$212D`) si el bit 1 de `$2130` está activo y esta aporta un píxel; sumar o restar (bit 7 de `$2131`) por canal de color con saturación a 0 y 31; y dividir por 2 si el bit 6 de `$2131` está activo. SHALL aplicarlo solo a las capas elegidas en `$2131` (bits 0-5, con el color de fondo en el bit 5), y la región de `$2130` bits 4-5 (siempre aplicar y nunca aplicar) MUST respetarse; el color math opera sobre los 5 bits de la CGRAM y el brillo se aplica después.

#### Scenario: Suma del color fijo
- **WHEN** `$2131` es `21`, el color fijo es rojo pleno y un píxel de BG1 es verde pleno
- **THEN** el píxel resultante es amarillo

#### Scenario: Resta
- **WHEN** `$2131` es `A1` y el color fijo tiene el rojo a 31 sobre un píxel blanco de BG1
- **THEN** el píxel resultante tiene rojo 0 y verde y azul a 31

#### Scenario: Mitad
- **WHEN** el bit 6 de `$2131` está activo
- **THEN** el resultado se divide por 2 antes del brillo

#### Scenario: Capa no elegida
- **WHEN** un píxel de BG2 se dibuja y el bit de BG2 de `$2131` está a 0
- **THEN** conserva su color sin mezcla

#### Scenario: Subpantalla
- **WHEN** el bit 1 de `$2130` está activo y BG3 está solo en `$212D`
- **THEN** el segundo operando es el píxel de BG3 de esa posición, o el color fijo si la subpantalla no aporta píxel

#### Scenario: Nunca
- **WHEN** los bits 4-5 de `$2130` valen 3 (color math prohibido siempre)
- **THEN** ningún píxel se mezcla
