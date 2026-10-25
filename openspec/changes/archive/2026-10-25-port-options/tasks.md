# Tasks

## 1. Configuración

- [x] 1.1 Tabla de opciones con valores por defecto, parser/escritor INI que conserve claves desconocidas y comentarios. Verificación: tests de lectura, valor inválido y conservación de claves.
- [x] 1.2 Precedencia de la línea de comandos sobre el fichero sin reescribirlo. Verificación: test con `--fullscreen` y `fullscreen = false`.
- [x] 1.3 Migración de `<rom>.keys` a `config.ini`. Verificación: test que importa un `.keys` de ejemplo.

## 2. Opciones de imagen

- [x] 2.1 Pantalla completa sin bordes y F11, con `fullscreen` en la configuración. Verificación: manual (captura antes y después) y el juego sigue en el mismo fotograma.
- [x] 2.2 Aspecto 4:3, 8:7 y free, y escalado `integer` y `fit`, con las barras negras. Verificación: test con función pura que, dadas dimensiones de área y aspecto, devuelve el rectángulo (casos de los escenarios).
- [x] 2.3 Filtros `nearest`, `bilinear` y `sharp`. Verificación: capturas a 1,5x y 2,5x comparadas a ojo, guardadas en `out/`.
- [x] 2.4 Recorte de márgenes (`overscan`). Verificación: la imagen mostrada tiene 208 líneas con `overscan = 8`.
- [x] 2.5 Efectos `scanlines` y `grid`, sin alterar la imagen cuando están desactivados. Verificación: con efectos desactivados, la salida del presentador es idéntica byte a byte al búfer en modo `--no-window`.

## 3. Menú y controles

- [x] 3.1 Fuente de bitmap 8x8 propia y menú F1 con las opciones de imagen y audio, que pausa la emulación. Verificación: captura del menú en `out/` y cambio de filtro persistido en `config.ini`.
- [x] 3.2 Remapeo interactivo de teclado y mando por jugador, con aviso de conflicto. Verificación: manual con teclado y test de la función de asignación.
- [ ] 3.3 Volumen maestro y selección de dispositivo de salida. Verificación: test que comprueba la amplitud al 50 % y manual con dos dispositivos.

## 4. Cierre

- [x] 4.1 Documentar la configuración y el menú en `CLAUDE.md` y `README.md`. Verificación: las claves documentadas coinciden con las del `config.ini` por defecto.

## Workflow follow-up

- Archivar con `/opsx:archive` cuando se complete; se aplica después de `interactive-frontend`.
