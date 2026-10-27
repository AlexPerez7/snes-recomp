## Context

Ver `proposal.md` para el motivo. Estado actual (leído en `src/port.rs`, `src/toolchain.rs`, `src/download.rs`):

- `snes-port` es el mismo `main.rs` que `snes-recomp`, arrancado como `port`. Es un binario de consola (subsistema `console`).
- Sin argumentos llama a `pick_rom`, que lanza PowerShell con un `OpenFileDialog` de WinForms, y guarda la carpeta en `<caché>/last-rom-dir.txt`. Si algo falla, espera a que se pulse Intro.
- `run_port` busca un compilador con `toolchain::find`, y si no hay, `portable_compiler` pregunta por la consola y descarga TinyCC 0.9.27 (480 KiB, SHA-256 fijado) a `<caché>/toolchain`.
- `build_port` imprime con `eprintln!` y devuelve `Result<Outcome, Failure>` con código de salida (2 ROM no reconocida, 3 falta compilador o SDL2, 4 construcción fallida). El juego se lanza como proceso hijo desde la carpeta de la caché.
- El motor no tiene dependencias fuera de `serde` y `toml`, y la medición del cambio `rom-to-port-builder` dio con TinyCC unos 118 fps sin vídeo y 60 fps con ventana, la mitad que MSVC.
- Máquina de desarrollo: Windows 11 Home, que no trae Windows Sandbox.

## Goals / Non-Goals

**Goals:**
- Doble clic, elegir la ROM, jugar, sin consola y sin tener que instalar nada.
- Los mismos códigos de salida y mensajes en terminal que hoy.
- La lógica del lanzador comprobable con pruebas automáticas aunque la ventana no lo sea.

**Non-Goals:**
- Un lanzador con aspecto de aplicación completa (listas de juegos, carátulas, ajustes): el menú F1 del juego ya tiene los ajustes.
- Linux y macOS (punto 7 del plan de `alttp-recomp`), instalador y firma de código.
- Cambiar cómo se genera o compila el juego.

## Decisions

**1. Ventana de Win32 con FFI directa, sin dependencias nuevas.** Unas quince funciones de `user32`, `comdlg32` y `comctl32` (crear ventana, bucle de mensajes, `GetOpenFileNameW`, `MessageBoxW`/botones, portapapeles) declaradas con `extern "system"` en `src/launcher_win.rs`. *Alternativas:* la caja `windows-sys` (menos `unsafe` escrito a mano, pero rompe la regla de dependencias mínimas y alarga la compilación); una ventana SDL2 dentro de `snes-port` (SDL2 solo está en el juego construido, no en `snes-port`, y no da diálogo de ficheros); seguir con PowerShell/WinForms (tarda en arrancar, depende de la política de ejecución y no puede mostrar progreso mientras el hilo principal construye).

**2. Hilo de trabajo y hilo de ventana.** La construcción corre en otro hilo y manda mensajes de estado por un canal; el hilo de la ventana los dibuja (`PostMessage`). El consentimiento de descarga es una petición que el hilo de trabajo envía y espera como respuesta. Así la ventana no se «cuelga» mientras compila.

**3. Receptor de mensajes (`trait Ui`).** `build_port`, `run_port` y `portable_compiler` pasan a recibir un `&dyn Ui` con `status`, `error`, `confirm` y `ask_rom`; hay una implementación de consola (la de hoy, para terminal y pruebas) y otra de ventana. Los textos y los códigos de salida no cambian. Las pruebas existentes usan la de consola.

**4. Un solo binario, subsistema de ventanas y enganche a la consola.** `snes-port.exe` pasa a ser del subsistema `windows` (sin consola al hacer doble clic). Si se ejecuta con argumentos, llama a `AttachConsole(ATTACH_PARENT_PROCESS)` y reasocia la salida estándar y de errores, de modo que desde una terminal todo sale como hoy. *Limitación conocida:* en `cmd` o PowerShell el programa de subsistema `windows` no hace esperar al intérprete; se mitiga con el código de salida y con `snes-recomp port ...`, que sigue siendo de consola. *Alternativa:* dos ejecutables (`snes-port.exe` de ventana y `snes-port-cli.exe`); más claro, pero más cosas en el paquete y el README ya enseña `snes-port`.

**5. Recordar la ROM.** `<caché>/last-rom.txt` con la ruta (y se sigue escribiendo `last-rom-dir.txt` para el diálogo). Se guarda solo tras lanzar con éxito. Nada de esto sale de la máquina.

**6. TinyCC en el paquete.** `scripts/release.ps1` toma el zip de TinyCC (misma URL y SHA-256 que `download.rs`), lo verifica y copia `tcc/` a `toolchain/` junto al ejecutable, con `COPYING` (LGPL) y una nota de dónde está el código fuente. `toolchain::find` mira primero lo local (MSVC, gcc, clang), después `toolchain/tcc/tcc.exe` junto al ejecutable, y solo entonces `portable_compiler` ofrece descargar. Con eso el flujo de descomprimir, doble clic y jugar no pide red. *Alternativa:* no incluirlo y mantener la descarga con consentimiento; se conserva como último recurso para quien tenga un paquete sin `toolchain/`.

**7. Idioma.** Dos tablas de cadenas (`en`, `es`) elegidas con `GetUserDefaultUILanguage`. Los mensajes de los perfiles y los nombres de juego no se traducen.

**8. Probar sin un Windows limpio.** Una variable `SNES_PORT_NO_LOCAL_COMPILER=1` hace que `toolchain::find` ignore MSVC, gcc y clang, y con una caché temporal (`--cache`) reproduce la máquina sin compilador, tanto con el TinyCC del paquete como con la descarga. La prueba en un Windows realmente limpio queda como lista de comprobación manual (máquina virtual o un equipo sin herramientas de desarrollo), porque Windows Sandbox no está disponible en Home.

## Risks / Trade-offs

- **Rendimiento de TinyCC**: medido el 2026-10-27 en la máquina de desarrollo con el runtime actual (ALttP, 1 200 fotogramas del título y la intro, sin ventana, sin audio): TinyCC 16,2 ms por fotograma (unos 62 fps) frente a 6,9 ms con MSVC (unos 145 fps). Es 2,3 veces más lento y apenas rebasa los 60 fps sin contar el vídeo ni el audio de una sesión real; la medida de `rom-to-port-builder` (118 fps) ya no se cumple con el runtime actual. En un equipo más lento no llegaría. Mitigación: medir con ventana real antes de dar por buena la vía TinyCC, buscar qué parte del runtime pesa más para TinyCC (o un compilador portátil que optimice, como el de w64devkit o clang) y, si va corto, sugerir en la ventana instalar Build Tools (sin bloquear).
- **Antivirus**: un ejecutable que descarga un compilador y compila C al arrancar puede dar falsos positivos, sobre todo sin firma. Mitigación: documentarlo en el README del juego; la firma de código queda fuera de este cambio.
- **FFI manual**: `unsafe` y errores de ciclo de vida de ventanas. Mitigación: módulo pequeño, aislado, con el trabajo real fuera de él.
- **Enganchar la consola**: la salida desde terminal con un binario de ventanas tiene aristas (redirecciones, paginación). Mitigación: probar `> fichero`, `| more` y el código de salida.
- **Licencia**: redistribuir TinyCC exige conservar su licencia y ofrecer el código fuente; `release.ps1` lo comprobará y el paquete llevará el aviso.

## Decisions (cerradas con el usuario)

- Al lanzar el juego, la ventana del lanzador se cierra y el juego la reemplaza; si el juego falla al arrancar, el lanzador reaparece con el error.
