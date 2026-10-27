## Why

Quien solo quiere jugar descarga el paquete de un juego, ve un `.exe` y espera abrirlo, elegir su ROM y jugar. Hoy `snes-port.exe` es una aplicación de consola: al hacer doble clic abre una ventana negra con mensajes en inglés, lanza un selector de ROM a través de PowerShell, pregunta en la consola si descarga un compilador y, en la primera ejecución, puede pasar minutos construyendo sin más señal que texto. Funciona, pero no se parece a un lanzador y asusta a quien no usa terminales.

## What Changes

- Al abrir `snes-port.exe` sin argumentos (doble clic) aparece una ventana propia, sin consola: nombre del juego, un botón **Elegir ROM…** (diálogo de abrir fichero de Windows) y un estado. Al elegir la ROM se construye si hace falta y se lanza el juego.
- Recuerda la última ROM: la siguiente vez la ventana ofrece **Jugar** con esa ROM (y **Cambiar ROM…**), y si la ROM ya no está, vuelve a pedirla.
- Arrastrar la ROM sobre el `.exe` sigue arrancando sin pasar por el selector; solo muestra la ventana de estado cuando hay que construir, preguntar por el compilador o informar de un error.
- Todo lo que hoy es texto de consola pasa a la ventana: «ROM no reconocida» (con los juegos soportados), el progreso de la primera construcción («preparando el juego, solo la primera vez») con aviso de que puede tardar, el consentimiento para descargar el compilador y los errores (con una acción para copiar el detalle).
- El programa sigue funcionando en una terminal con los mismos argumentos y códigos de salida que hoy: se engancha a la consola del proceso que lo llama cuando existe.
- Textos en español o inglés según el idioma de Windows.
- El paquete del motor puede llevar TinyCC (LGPL, con su licencia y el aviso) para que quien no tiene compilador no tenga que descargar nada; el orden pasa a ser compilador local, TinyCC del paquete, descarga con consentimiento.
- Solo Windows. En otros sistemas `snes-port` sigue siendo de línea de comandos.

## Capabilities

### New Capabilities

- `port-launcher`: la ventana de lanzamiento: elegir y recordar la ROM, estado y errores en la ventana, consentimiento de la descarga, textos según el idioma.

### Modified Capabilities

- `port-builder`: compilador portátil incluido en el paquete antes de descargar; mensajes de progreso, error y consentimiento entregados a un receptor que puede ser la consola o la ventana.

## Impact

- `src/port.rs`: separar la lógica (elegir perfil, construir, lanzar) de cómo se muestran los mensajes y se pide confirmación; el selector por PowerShell (`pick_rom`) se sustituye por el diálogo propio. `src/toolchain.rs` y `src/download.rs`: comprobar un `tcc` junto al ejecutable antes de descargar.
- Módulo nuevo de ventana de Windows (`src/launcher_win.rs`) y subsistema de ventanas en el binario `snes-port`; sin dependencias nuevas si se usa FFI directo (ver `design.md`).
- `scripts/release.ps1` del motor: copiar TinyCC (con el hash comprobado y su licencia) al paquete; el `release.ps1` de `alttp-recomp` lo hereda al desempaquetar el motor.
- `alttp-recomp`: `README.md` (la sección «Jugar» pasa a ser: descomprimir, doble clic, elegir la ROM) y `plan.md`.
- Fuera de alcance: instalador MSI o firma de código, Linux y macOS, ajustes del juego dentro del lanzador (siguen en el menú F1 del juego), listas o carátulas de varios juegos.
