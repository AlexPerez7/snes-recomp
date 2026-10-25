# Tasks

## 1. Repartir el runtime en módulos

- [x] 1.1 Guardar capturas de referencia de ALttP (título, selección de partida, overworld, mazmorra, mapa del modo 7) con el runtime actual en `out/ref/`. Verificación: los PNG existen y sirven de línea base.
- [x] 1.2 Separar `runtime/headless.c` en `snes_hw.c` (bus, registros, DMA, reloj, contextos), `snes_ppu.c` (memorias y dibujador) y `headless.c` (opciones, informe y captura); embeberlos y escribirlos con `codegen --all`. Verificación: `cargo test` verde y las capturas de 1.1 idénticas byte a byte.
- [x] 1.3 Documentar el reparto en `CLAUDE.md` (estructura y comandos). Verificación: la sección «Estructura» nombra los ficheros nuevos.

## 2. PPU línea a línea

- [x] 2.1 Sustituir `screenshot()` por `ppu_catch_up(línea)` sobre un búfer de fotograma de 256x224, llamado en cada escritura a la PPU y al cruzar de línea. Verificación: test que cambia `$210D` en la línea 100 y comprueba dos scrolls en la misma imagen.
- [x] 2.2 Ejecutar el HDMA al principio de cada línea durante la ejecución (no como repetición posterior) y quitar `ppu_save/ppu_restore`. Los dos tests de HDMA siguen pasando; `--screenshot` pasa a ser el último fotograma completo dibujado en vivo (cambia solo unos píxeles animados en título y selección de partida; las referencias de `out/ref/` se regeneraron).
- [x] 2.3 Guardar el fotograma ya dibujado al llegar a la línea 224 y exponerlo al frontal. Verificación: test que compara dos ejecuciones iguales byte a byte (requisito de reproducibilidad).

## 3. Modos de fondo y sprites

- [x] 3.1 Generalizar el cálculo de píxel de fondo por tablas de modo (profundidad, fondos activos, prioridades) y añadir los modos 0 y 3 y BG4. Verificación: tests con VRAM sintética para el modo 0 (cuatro fondos de 2 bits) y el modo 3 (8 bits).
- [x] 3.2 Añadir los modos 2, 4 y 6 con offset por tile (bits 13-15 de BG3). Verificación: test modo 2 con columnas desplazadas por tabla.
- [x] 3.3 Añadir los modos 5 y 6 de 512 de ancho con mezcla a 256. Verificación: test que dibuja tiles de 8 píxeles de ancho en el modo 5 y los reduce por promedio.
- [x] 3.4 Completar sprites: tamaños de `$2101`, giros, prioridades y límite de 32 sprites/34 tiles por línea. Verificación: test con 33 sprites en una línea que dibuja 32.
- [x] 3.5 Revisar ventanas, mosaico y color math en todos los modos y actualizar los avisos del informe (solo entrelazado y alta resolución). Verificación: el informe no avisa en ninguno de los modos 0-7 y avisa con `$2133` bit 0.
- [x] 3.6 Actualizar el spec `ppu-render` con la sincronización a los cambios y la documentación de `--screenshot`. Verificación: `openspec validate interactive-frontend --strict` pasa.

## 4. Reloj por líneas

- [x] 4.1 (hecho con el dibujo perezoso del grupo 2: las filas vencen por tick con la misma rejilla de 262 líneas que el IRQ y el NMI, que no cambian) Hacer que el dibujo, el HDMA y el IRQ compartan la rejilla de líneas, manteniendo las fracciones de NMI (225) e IRQ actuales. Verificación: las suites de IRQ/NMI de `src/headless.rs` y la intro de ALttP siguen verdes y las capturas de 1.1 idénticas.
- [x] 4.2 Medir y fijar `frame_len` por defecto. Medido: ALttP gasta los 20000 bloques de cada fotograma (espera el NMI girando en un bucle) y el runtime con `SNES_RT_CHECKS` ejecuta 3,3 millones de bloques por segundo (167 fotogramas por segundo); se mantiene 20000 porque es el valor con el que todas las pruebas de juego son estables, y la optimización de velocidad queda en 5.3 (sin `SNES_RT_CHECKS`, `/O2`, y detección de bucles de espera).

## 5. Frontal SDL2

- [x] 5.1 Añadir `runtime/play.c` (SDL2: ventana, textura, escalado entero, pacing a 60,1 Hz, teclas F11/Esc/P/Tab/F10) y la generación de `snes_play.c` y de un `build_play.bat` con `SDL2_DIR`. Verificación: `snes_play roms/alttp_usa.sfc` abre ventana y muestra la intro (comprobación manual con captura de pantalla).
- [x] 5.2 Opciones `--no-window`, `--frames`, `--screenshot` en `snes_play`. Verificación: test de Rust que compila `snes_play` en modo sin ventana y compara la captura con el runtime sin vídeo; se omite si no hay SDL2.
- [x] 5.3 Medir fotogramas por segundo sin límite en la intro y en el overworld y optimizar si no se alcanzan los 100 fps (`/O2`, tablas de planos, sin `SNES_RT_CHECKS`). Verificación: informe de fps en el log de la comprobación.

## 6. Entrada en vivo

- [x] 6.1 Hacer que el joypad 1 y 2 sumen `live_pad` al estado de `--press`, con anulación de direcciones opuestas. Verificación: tests de `joypad-input` (fuente y `--press` juntas; LEFT+RIGHT no llega).
- [x] 6.2 Teclado con asignación por defecto y fichero `<rom>.keys` para cambiarla. Verificación: test de la lectura del fichero (asignación propia) y manual: Enter pasa del título a la selección de partida.
- [x] 6.3 Mandos SDL (game controller) con conexión en caliente. Verificación: manual con un mando; el código falla sin romper si no hay mando.

## 7. Partidas guardadas

- [x] 7.1 Cargar `<rom>.srm` al arrancar con la comprobación de tamaño y el aviso. Verificación: test con `.srm` de tamaño correcto e incorrecto.
- [x] 7.2 Guardar el `.srm` con bandera sucia (máx. 1/s), al salir y con escritura atómica. Verificación: test que guarda, reinicia y recupera la partida; y que sin cambios no reescribe.

## 8. SPC700

- [x] 8.1 Implementar la SPC700 (registros, banderas, modos de direccionamiento, todas las instrucciones, RAM de 64 KiB, ROM de arranque) y los puertos `$F4-$F7`. Verificación: pruebas con programas de ensamblador manual por familia de instrucciones (ADC/SBC, DAA/DAS, MUL/DIV, bit ops, saltos, pila).
- [x] 8.2 Temporizadores 0-2 y registros `$F0-$FF`. Verificación: test con programa que cuenta ticks de T0 y T2.
- [x] 8.3 Conectar `$2140-$2143` al SPC real con el reloj proporcional por línea y el handshake de ALttP (`$AA/$BB`, subida de bloques). Verificación: ALttP sube su programa de sonido y llega al título sin quedarse esperando; el runtime sin vídeo con SPC real corre 3000 fotogramas.

## 9. DSP y audio

- [x] 9.1 Decodificación BRR con filtros y bucles. Verificación: test con bloque BRR sintético y valores calculados a mano.
- [x] 9.2 Voces: pitch, ADSR, GAIN, ruido, modulación de pitch, KON/KOFF, panorama y volumen maestro. Verificación: tests por voz con envolvente y periodo comprobables.
- [x] 9.3 Eco con búfer y filtro FIR de 8 coeficientes. Verificación: test que reproduce un impulso y comprueba el retardo y la realimentación.
- [x] 9.4 `--wav FICHERO` (16 bits, 32 kHz estéreo). Verificación: ejecutar 1200 fotogramas del título de ALttP con `--no-window` y comprobar WAV de ~20 s no silencioso.
- [x] 9.5 Salida de audio por SDL con búfer circular y ajuste de ritmo (vídeo manda). Verificación: manual: la música del título suena sin cortes ni deriva en 10 minutos; el audio se silencia al pausar.

## 10. Prueba de juego y cierre

- [x] 10.1 Partida real: arrancar con teclado, elegir partida, llegar a Hyrule, entrar en una mazmorra y salir al Mundo Oscuro, guardar y reiniciar con el `.srm`. Verificación: lista de comprobación con capturas y notas de fallos en `out/play-notes.md`.
- [x] 10.2 Corregir los fallos hallados en 10.1 (imagen, sonido o juego). Verificación: cada fallo cerrado con un test o con captura antes/después.
- [x] 10.3 Repetir las suites de fuzz (mazmorras, overworld y Mundo Oscuro) con el runtime nuevo. Verificación: 30 ejecuciones de 30000 fotogramas sin paradas ni stubs ejecutados.
- [x] 10.4 Actualizar `CLAUDE.md` con los comandos de `snes_play`, `--no-window`, `--wav` y el `.srm`. Verificación: los comandos documentados se ejecutan tal cual.

## Workflow follow-up

- Archivar con `/opsx:archive` cuando el grupo 10 esté completo.
