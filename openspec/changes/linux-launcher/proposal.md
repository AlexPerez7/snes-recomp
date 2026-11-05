# linux-launcher

## Por qué
En Windows y Android el usuario descarga, abre, elige su ROM y juega sin instalar nada. En Linux hacía falta terminal, gcc y libsdl2-dev, y no había ventana de lanzador (la de Windows es Win32).

## Qué cambia
`linux/` (`launcher.c`, `build.sh`, `tcc_config/`): un ejecutable único para Linux x86_64, equivalente al `launcher.c` de Android. Ventana SDL2 con **Elegir ROM** (zenity/kdialog si existen, o arrastrar el archivo) y **Jugar**; comprueba el CRC32 del perfil (acepta cabecera de 512 bytes); recuerda la ruta de la ROM en `~/.local/share/<juego>`; genera el C con la biblioteca del generador, lo compila en memoria con libtcc (cabeceras mínimas de `assets/tcc_include`, sin las del sistema) y llama al frontal de `play.c` en modo `SNES_RT_LIBRARY`. `build.sh <perfil.toml> [nombre]` deja `out/linux/<nombre>/` con el ejecutable, `lib/libtcc.so`, `lib/libSDL2-2.0.so.0` y `assets/`. `--play` salta la pantalla de inicio; `--stats` imprime los tiempos.

## Fuera de alcance
Instalador, icono de escritorio, actualizaciones, aarch64, versiones de glibc anteriores a la 2.38 (sale de compilar en esta máquina).
