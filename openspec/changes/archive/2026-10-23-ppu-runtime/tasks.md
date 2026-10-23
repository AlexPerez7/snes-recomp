## 1. DMA

- [x] 1.1 Implementar en `runtime/headless.c` la transferencia de `$420B` (canales en orden, patrones de los modos 0 a 7, bit de dirección, avance de la dirección A, contador a 0, destino `$2180`) con contadores de transferencias y bytes por canal; verificar con tests de `src/headless.rs` sobre ROMs pequeñas que cubren un escenario de la spec cada uno (varios canales, tamaño 0, modo 1, avance fijo/decreciente, B→A, vuelta de banco, relleno de WRAM)
- [x] 1.2 Mostrar en el informe las transferencias y los bytes por canal; verificar con un test que el canal 1 con dos transferencias de 64 bytes informa 2 y 128

## 2. Memorias de la PPU

- [x] 2.1 Añadir VRAM con `$2115-$2119` y las lecturas `$2139/$213A` con precarga; verificar con tests de escritura secuencial, incremento por el byte bajo, paso de 32 y lectura con precarga
- [x] 2.2 Añadir CGRAM (`$2121`, `$2122`, `$213B`) con su latch y el bit 7 del byte alto; verificar con tests de paleta completa, color a medias y bit alto
- [x] 2.3 Añadir OAM (`$2102-$2104`, `$2138`) con la tabla baja por pares y la alta directa; verificar con tests de tabla baja, tabla alta y lectura
- [x] 2.4 Avisar en el informe si `$2115` activa la traducción de direcciones (bits 2-3 distintos de 0); verificar con un test que lo escribe

## 3. Volcado

- [x] 3.1 Implementar `--dump <dir>` (`vram.bin`, `cgram.bin`, `oam.bin`, `ppu_regs.bin`) en el informe final, también en las paradas por fallo, y su uso en la ayuda; verificar con tests de volcado normal, tras `rt_unresolved` y sin la opción

## 4. ALttP

- [x] 4.1 Ampliar la prueba de ALttP (`alttp_boots_and_takes_its_first_nmis` o una nueva) para volcar las memorias tras unos fotogramas y comprobar que la VRAM, la CGRAM y la OAM no están vacías; verificar con Visual Studio
- [x] 4.2 Anotar en este fichero las mediciones: bytes de DMA por canal y destino, estado de las tres memorias tras un número de fotogramas, si `$2115` usa la traducción de direcciones y qué pantalla se espera ver
- [x] 4.3 Documentar `--dump` en `CLAUDE.md`; verificar que el comando documentado funciona tal cual
- [x] 4.4 `cargo test` completo en verde

## Mediciones (ALttP USA, 600 fotogramas de 20.000 bloques, Visual Studio Build Tools)

- Sin fallos: 600 fotogramas, código de salida 0. La ejecución tarda menos de un segundo.
- DMA por canal (600 fotogramas): canal 0 a `$2104` (OAM, modo 0), 308 transferencias y 165.632 bytes, es decir, la OAM entera casi cada fotograma tras el arranque; canal 1 a `$2122` (CGRAM, modo 0), 13 transferencias y 22.400 bytes; canales 2, 3 y 4 a `$2118` (VRAM, modo 1), una transferencia pequeña cada uno (320, 64 y 128 bytes). A los 120 fotogramas: canal 0 con 68 transferencias, canal 1 con las mismas 13.
- Memorias tras 600 fotogramas: VRAM con 41.316 bytes no nulos de 65.536, CGRAM con 14 colores no nulos y OAM con 324 bytes no nulos y 61 sprites en pantalla (Y menor que 224). Los primeros colores de la paleta son 0.
- Registros: `$2100 = 0F` (brillo máximo, sin forzar blanking), `$2105 = 09` (modo 1 con prioridad del BG3), `$2115 = 80` (incremento tras el byte alto, sin traducción de direcciones: el aviso no aparece).
- Lo esperable en pantalla: una escena de modo 1 con los fondos de la VRAM y 61 sprites; no se ha renderizado nada, así que no se sabe cuál. Es el punto de partida del renderizador.
- Con 30 fotogramas, los de arranque, la VRAM tiene solo unos cientos de bytes: la subida de gráficos viene después de la del audio. La prueba de ALttP usa 120 fotogramas por defecto.
- `$4200` deja de valer `81` al cabo de los fotogramas (el juego añade otros bits), así que la prueba comprueba los bits 7 y 0.

