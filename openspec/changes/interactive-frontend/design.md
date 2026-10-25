# Design

## Context

Hoy `runtime/headless.c` (1366 líneas) es un único fichero con el bus LoROM, los registros de hardware que «recuerdan lo escrito», DMA, el reloj de fotogramas (un fotograma = `frame_len` bloques, 20000 por defecto; la línea es `tick * 262 / frame_len`, el NMI cae en la 225), las fibras de contexto, los informes y `screenshot()`. Esta última dibuja el estado **final** de la PPU y reproduce el HDMA línea a línea sobre él. El procesador de sonido es un autómata de tres estados (`apu_write`) que solo contesta al handshake. El runtime se incrusta en el binario Rust y `codegen --all` lo escribe junto al C generado; el C generado llama a `rt_*` (ver `snes_rt.h`). El tiempo no es exacto al ciclo y no hace falta que lo sea: ALttP ya corre sin paradas con este modelo. Motivación y alcance: ver `proposal.md`.

## Goals / Non-Goals

**Goals:**
- Un solo dibujador de PPU, correcto línea a línea, compartido por la captura y por la ventana.
- Un solo núcleo de hardware (bus, DMA, PPU, SPC, reloj) con dos frontales finos: el actual sin vídeo y el nuevo interactivo.
- Jugar ALttP de principio a fin a 60 fps con sonido en una máquina de sobremesa corriente, usando el C generado de siempre.
- El runtime sin vídeo y `cargo test` no ganan ninguna dependencia nueva.

**Non-Goals:**
- Precisión de ciclos del 65816 o de la PPU; no se busca pasar pruebas de temporización de otros juegos.
- Entrelazado y alta resolución vertical, Super FX, SA-1 y demás coprocesadores, save states, filtros de escalado, netplay.
- Segundo juego: este cambio deja el motor listo, pero no lo prueba con otra ROM.

## Decisions

**1. Renderizar «por alcance» (catch-up), no con un registro de escrituras.** El dibujador lleva `ppu_line` (la próxima línea por dibujar). Antes de aplicar cualquier escritura a `$2100-$213F`, VRAM, CGRAM u OAM, y al cruzar cada límite de línea en `rt_poll`, llama a `ppu_catch_up(línea_actual)`, que dibuja las líneas pendientes con el estado vigente y ejecuta el HDMA de cada una antes de dibujarla. Así no hay que guardar un registro de escrituras ni reconstruir estado: el estado es el real en cada instante. *Alternativa:* guardar `(línea, registro, valor)` y reproducir al final; descartada porque duplica la lógica de memorias (VRAM/CGRAM/OAM también cambian) y la captura actual ya demuestra que el estado mutable por línea es más simple (`ppu_save/ppu_restore` desaparecen).

**2. El dibujador escribe en un búfer de fotograma y no sabe de SDL.** `ppu_catch_up` rellena `uint32_t fb[224][256]` (RGB). La captura PNG y la ventana leen ese búfer. Esto da reproducibilidad (los tests comparan PNG byte a byte) y mantiene el núcleo sin dependencias. Los modos de 512 de ancho se calculan a 512 y se promedian a 256 al escribir en `fb`.

**3. Dibujo por línea con una función por modo.** Un `render_line(y)` común hace las ventanas, el color math y el brillo, y llama a `bg_pixel(capa, x, y)` con tablas por modo (profundidad de bits, tamaño de tile, qué fondos, orden de prioridad). Se generaliza el actual `pixel_index` en lugar de reescribirlo, para conservar el modo 1 y 7 que ya están validados contra ALttP. Offset por tile (modos 2, 4, 6) lee la tabla de BG3 con la regla del hardware (bits 13-15 de la entrada eligen qué fondos se desplazan).

**4. Reloj por líneas.** `frame_len` sigue siendo la unidad (bloques), pero `rt_poll` cruza el límite de línea cuando `tick * 262 / frame_len` cambia, y entonces: dibuja la línea que acaba, ejecuta HDMA de la siguiente, avanza SPC/DSP, lleva a cabo la comprobación del IRQ y, en la 225, el NMI. El comportamiento observable de NMI e IRQ no cambia (mismas fracciones que hoy), de modo que ALttP sigue sin perder el hilo. El valor por defecto de `frame_len` se calibra para que el juego termine su trabajo antes del NMI y no sobre tiempo de CPU real.

**5. Un solo SPC en el núcleo, activado por el frontal.** `runtime/spc.c` contiene la SPC700 y el DSP. El runtime sin vídeo puede seguir con el autómata (rápido, ya probado) mientras no se pida audio; con `--wav` o en `snes_play` se usa la emulación real. *Alternativa:* usar siempre la real; se descarta porque el autómata hace las 120 pruebas de regresión baratas y las suites de fuzz no necesitan sonido. Hay que vigilar que ambos satisfagan el mismo handshake `$AA/$BB`.

**6. Sincronización SPC.** El reloj de la SPC se avanza proporcionalmente por línea: 1,024 MHz / 60,1 Hz ≈ 17 040 ciclos por fotograma repartidos en 262 líneas; antes de leer o escribir `$2140-$2143` se pone al día. El DSP produce una muestra cada 32 ciclos SPC (533 por fotograma) en un búfer circular. En `snes_play`, el hilo de audio de SDL consume del búfer; el hilo principal marca el ritmo con el reloj de vídeo y comprueba que el búfer está entre 1 y 3 fotogramas. Se prefiere «el vídeo manda y el audio se adapta» (descartar/duplicar unas pocas muestras) a «el audio manda» porque la emulación puede tener ráfagas de lentitud.

**7. SDL2 solo en el frontal interactivo, fichero aparte.** `runtime/play.c` incluye `SDL.h`; el resto del runtime no. El build interactivo necesita el paquete de desarrollo de SDL2 para MSVC: la variable `SDL2_DIR` apunta a él, `SDL2.dll` se copia junto al `.exe`, y nada de SDL entra en el repositorio. `src/cctest.rs` omite las pruebas que lo necesiten si `SDL2_DIR` no está. *Alternativa:* cargar SDL con `LoadLibrary` para no necesitar cabeceras; descartada, complica sin ventaja real.

**8. Entrada como fuente de estado por fotograma.** El frontal rellena `live_pad[2]` (16 bits) al comienzo de cada fotograma; `joy_auto_read` y la lectura serie ya consultan el estado por fotograma, así que se les añade la OR con `live_pad`. Los pares opuestos se anulan allí, en un solo sitio.

**9. SRAM.** El tamaño sale de la cabecera (ya se calcula `sram_mask`). Se marca una bandera sucia en cada escritura a `$70-$7D:xxxx`; el hilo principal escribe a lo sumo una vez por segundo y al salir (fichero temporal + renombrar).

**10. Verificación sin ventana ni oído.** Todo lo observable se puede producir en modo `--no-window`: PNG por fotograma (`--screenshot`, y `--screenshot-every N`), WAV (`--wav`), `.srm`. Las pruebas de Rust compilan el C con ese modo, así que `cargo test` cubre PPU, SPC, DSP y SRAM sin SDL; solo el arranque de ventana real queda para comprobación manual.

## Risks / Trade-offs

- [Rendimiento del C generado a 60 fps con dibujo y sonido] → medir pronto (grupo 6); si falta, compilar con `/O2` y desactivar `SNES_RT_CHECKS`, dibujar solo líneas con cambios y precalcular tablas de planos de bits. Objetivo: ≥ 100 fps sin límite en la máquina de desarrollo.
- [Imprecisión de DSP y SPC audible] → no hay emulador de referencia permitido (reglas clean-room); se verifica con la documentación pública del hardware, pruebas con BRR sintético y escucha manual de la música del título y del overworld de ALttP. Se acepta una pequeña desviación de timbre en el primer corte.
- [El reloj por bloques no es el de la consola: efectos de raster sensibles pueden saltar de línea] → los efectos de ALttP (IRQ de ventana, HDMA de gradientes, modo 7 del mapa) ya se reproducen con el modelo actual; se compara con las capturas de hoy como línea base antes de tocar nada.
- [Hacer SDL obligatorio para jugar] → es una dependencia solo de ejecución interactiva; quien no la tenga conserva el runtime sin vídeo.
- [Refactor grande del runtime puede romper los tests actuales] → el grupo 1 solo reparte ficheros y exige la suite verde y las capturas idénticas antes de seguir.

## Migration Plan

Se hace por grupos, cada uno dejando `cargo test` verde y las capturas de referencia (modo 1 y 7 de ALttP) idénticas. `headless.exe` sigue existiendo con las mismas opciones. Si el frontal interactivo no sale a tiempo, los grupos de PPU y audio ya aportan valor en `--screenshot` y `--wav`. No hay migración de datos: el `.srm` es nuevo.

## Open Questions

- Si se prefiere WASAPI directo o miniaudio a SDL para el audio en Windows si SDL da latencia alta (se decide con medidas en el grupo 11).
