## ADDED Requirements

### Requirement: Shader CRT opcional
El ejecutable SHALL ofrecer la opción `shader` con los valores `off` y `crt`, por defecto `off`. Con `crt` la imagen MUST presentarse con OpenGL 3.3 mediante una pasada de fragmentos que reúne el muestreo nítido, la curvatura, el bloom, el haz de las scanlines y la máscara de fósforo, con las fuerzas de `crt_mask`, `crt_curve`, `crt_bloom` y `scanlines` (todas de 0 a 100). El aspecto, el escalado, el recorte y la pantalla ancha SHALL seguir fijando el rectángulo donde se dibuja la imagen.

#### Scenario: Valor por defecto
- **WHEN** `shader` no está en `config.ini`
- **THEN** la imagen se presenta con el camino del renderizador de SDL, sin cambios

#### Scenario: Sin efectos
- **WHEN** `shader = crt`, `scanlines`, `crt_mask`, `crt_curve` y `crt_bloom` valen 0 y la imagen se dibuja con un factor entero exacto
- **THEN** la imagen es idéntica byte a byte a la que da `nearest`

#### Scenario: Máscara y haz
- **WHEN** `shader = crt` con `crt_mask` y `scanlines` mayores que 0
- **THEN** la imagen muestra el patrón de aperturas y el oscurecimiento entre líneas, y el resto de opciones de imagen siguen aplicándose

#### Scenario: Curvatura
- **WHEN** `crt_curve` es mayor que 0
- **THEN** los bordes de la imagen se curvan y lo que queda fuera del tubo se dibuja en negro

### Requirement: Respaldo del shader
Si no hay OpenGL 3.3, el contexto no se crea o el shader no compila ni enlaza, el ejecutable MUST avisar una sola vez por la salida de errores y seguir con el camino del renderizador de SDL, sin dejar de arrancar. `config.ini` SHALL conservar el valor pedido de `shader`.

#### Scenario: Sin OpenGL 3.3
- **WHEN** `shader = crt` en un equipo sin OpenGL 3.3
- **THEN** el juego arranca con el camino del renderizador, hay un solo aviso y `config.ini` sigue diciendo `shader = crt`

### Requirement: Cambiar de shader en marcha
Cambiar `shader` desde el menú F1 SHALL recrear la ventana, conservando su posición y el modo de pantalla completa, sin alterar el estado del juego ni el audio.

#### Scenario: Activar el shader jugando
- **WHEN** se cambia `shader` de `off` a `crt` con el juego en marcha
- **THEN** la ventana se vuelve a crear, el juego continúa en el mismo fotograma y la imagen se presenta con el shader
