## 1. Separar la lógica de la presentación

- [ ] 1.1 `trait Ui` (`status`, `error`, `confirm`, `ask_rom`) y su implementación de consola con los textos y códigos de salida de hoy
- [ ] 1.2 `build_port`, `run_port` y `portable_compiler` reciben el `Ui`; las pruebas existentes de `port.rs` pasan sin cambios con el de consola
- [ ] 1.3 Guardar y leer `<caché>/last-rom.txt` (solo tras lanzar con éxito); pruebas de la ruta presente, ausente y movida

## 2. Compilador del paquete

- [ ] 2.1 `toolchain::find`: local → `toolchain/tcc/tcc.exe` junto al ejecutable → nada; `SNES_PORT_NO_LOCAL_COMPILER=1` ignora el local
- [ ] 2.2 Prueba: con `SNES_PORT_NO_LOCAL_COMPILER=1`, una caché temporal y un TinyCC en `toolchain/`, el juego se construye sin abrir red (`network_requests() == 0`)
- [ ] 2.3 `scripts/release.ps1`: descargar TinyCC con el SHA-256 fijado de `download.rs`, copiarlo a `toolchain/` con su licencia y el aviso del código fuente; seguir fallando si el paquete lleva ROM o C generado
- [ ] 2.4 Medir con el TinyCC del paquete los fps del juego con ventana y anotar el resultado en `design.md`

## 3. Ventana del lanzador (Windows)

- [ ] 3.1 `src/launcher_win.rs`: ventana, bucle de mensajes y diálogo de abrir fichero (`.sfc`, `.smc`, carpeta inicial) por FFI directa
- [ ] 3.2 Hilo de trabajo con canal de estado; la ventana responde durante la construcción; consentimiento de descarga como petición con respuesta
- [ ] 3.3 Pantallas: primera vez (Elegir ROM…), siguientes (Jugar con la ROM recordada y Cambiar ROM…), construyendo, error con «Copiar detalle»
- [ ] 3.4 ROM no reconocida: CRC32 y juegos soportados en la ventana; permitir elegir otra
- [ ] 3.5 Cadenas `en` y `es` según `GetUserDefaultUILanguage`
- [ ] 3.6 Pruebas automáticas de lo que no es ventana: elección de idioma, textos de error iguales a los de consola, estado tras cancelar el diálogo

## 4. Subsistema de ventanas y terminal

- [ ] 4.1 `snes-port` con subsistema `windows`; con argumentos, `AttachConsole(ATTACH_PARENT_PROCESS)` y salida estándar/errores a esa consola
- [ ] 4.2 Sin argumentos abre el lanzador; con solo una ROM construye y lanza mostrando la ventana solo si hay construcción, descarga o error
- [ ] 4.3 Comprobar a mano desde `cmd` y PowerShell: mensajes, redirección a fichero, código de salida y `--no-launch`

## 5. Paquete y documentación

- [ ] 5.1 Probar el paquete completo en un Windows sin Visual Studio ni gcc (máquina virtual o equipo limpio): descomprimir, doble clic, elegir ROM, jugar; con y sin red
- [ ] 5.2 README del motor: tabla de estado, sección del constructor y de la descarga; `CLAUDE.md` con los módulos nuevos
- [ ] 5.3 `alttp-recomp`: `README.md` («Jugar» con el flujo de doble clic) y `plan.md` (punto 8b); `release.ps1` del juego comprueba que `toolchain/` llega al zip y que sigue sin ROM ni `.srm`
- [ ] 5.4 El lanzador se cierra al lanzar el juego y reaparece con el error si el juego falla al arrancar (decidido); comprobarlo con un juego que termina con código distinto de 0
