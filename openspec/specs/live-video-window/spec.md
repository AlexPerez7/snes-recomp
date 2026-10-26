# live-video-window Specification

## Purpose

Mostrar en una ventana, en tiempo real, los fotogramas que produce la PPU, y dar las teclas básicas de control, para poder jugar la ROM recompilada.

## Requirements

### Requirement: Ejecutable interactivo
`codegen --all` SHALL escribir, además del runtime sin vídeo, los ficheros necesarios para construir un ejecutable interactivo que se invoca como `snes_play <rom>`. El ejecutable MUST abrir una ventana SDL2 con el título del juego y terminar con código 2 si la ROM no se puede leer.

#### Scenario: Arranque
- **WHEN** se ejecuta `snes_play roms/alttp_usa.sfc`
- **THEN** aparece una ventana que muestra la imagen del juego desde el primer fotograma

#### Scenario: ROM ausente
- **WHEN** se ejecuta `snes_play falta.sfc`
- **THEN** termina con código de salida 2 y un mensaje de error

### Requirement: Presentación a 60 Hz
El ejecutable SHALL presentar un fotograma por cada fotograma de la consola (unos 60,1 Hz) y MUST esperar si va adelantado respecto al reloj real, sin acumular retraso si va atrasado más de tres fotogramas.

#### Scenario: Máquina rápida
- **WHEN** la emulación de un fotograma tarda menos de 16 ms
- **THEN** la ventana se actualiza a unos 60 fotogramas por segundo y no más

### Requirement: Escalado
La ventana SHALL mostrar la imagen de 256x224 escalada a un múltiplo entero y conservar la proporción al redimensionarla, con las barras negras necesarias.

#### Scenario: Ventana grande
- **WHEN** la ventana mide 1280x1000
- **THEN** la imagen se dibuja a 4x centrada y el resto es negro

### Requirement: Teclas de control
El ejecutable SHALL ofrecer F11 para pantalla completa, Escape para abrir el menú (con el menú abierto, Escape lo cierra), P para pausar, Tab (mantenida) para avance rápido y F10 para guardar una captura PNG del fotograma actual. Escape MUST NOT cerrar el programa. Cerrar la ventana SHALL seguir terminando el programa.

#### Scenario: Pausa
- **WHEN** se pulsa P
- **THEN** la emulación se detiene y la ventana conserva el último fotograma hasta pulsar P otra vez

#### Scenario: Escape abre el menú
- **WHEN** se pulsa Escape con el menú cerrado
- **THEN** el menú se abre, la emulación se pausa y el programa sigue en marcha

#### Scenario: Escape no sale
- **WHEN** se pulsa Escape dos veces seguidas, una para abrir y otra para cerrar el menú
- **THEN** el juego sigue en marcha tras la segunda y la configuración se ha guardado

#### Scenario: Cerrar la ventana
- **WHEN** el usuario cierra la ventana
- **THEN** el programa termina sin pedir confirmación, guardando la SRAM como al salir por el menú

### Requirement: Modo sin ventana verificable
El ejecutable SHALL admitir `--frames N`, `--screenshot FICHERO` y `--no-window` para ejecutar sin abrir ventana, de modo que las pruebas automáticas comprueben la imagen sin pantalla.

#### Scenario: Prueba automática
- **WHEN** se ejecuta con `--no-window --frames 300 --screenshot s.png`
- **THEN** escribe `s.png` y termina con código 0 sin abrir ventana
