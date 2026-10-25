# Design

## Context

Hoy `codegen --all` escribe el C en un directorio y el usuario compila a mano con `cl` desde una consola de Visual Studio. `src/cctest.rs` ya sabe localizar `CC`, gcc, clang, cc o Visual Studio (`msvc_env` con `vcvars64.bat`), pero solo para tests. El C generado es una traducción del código de la ROM; los datos (gráficos, mapas, música) no se incrustan: el runtime lee la ROM del disco. Los `games/*.toml` contienen direcciones y tablas de salto, no bytes de la ROM. Ver `proposal.md`.

## Goals / Non-Goals

**Goals:**
- Un único paso para el usuario final: su ROM entra, un juego jugable sale.
- Que lo distribuido no contenga nada de la ROM ni derivado de ella, y que sea fácil de comprobar.
- Arranque rápido tras la primera construcción.

**Non-Goals:**
- Ocultar o cifrar el C generado: simplemente nunca sale de la máquina del usuario.
- Un instalador gráfico o actualizador automático.
- Compilar sin compilador (un intérprete de respaldo en lugar de C): se evalúa luego si la descarga del compilador resulta incómoda.

## Decisions

**1. Compilar en la máquina del usuario, no distribuir binarios precompilados.** Un ejecutable generado contiene el código traducido del juego, es decir, derivado de la ROM. Construirlo localmente es lo que mantiene separado lo distribuible. *Alternativa:* distribuir un emulador/intérprete genérico que cargue la ROM; legalmente más limpio pero pierde el sentido del proyecto (recompilación estática) y su rendimiento.

**2. Caché indexada por `crc32/versión`.** Carpeta `<cache>/<crc32>-<versión>/` con `rom_*.c`, objetos y `game.exe`. La versión incluye el hash del runtime y de `games/<id>.toml` embebidos, de modo que cualquier cambio en el generador reconstruye. Se escribe en un directorio temporal y se renombra al terminar para no dejar cachés a medias.

**3. Detección de compilador en capas.** Primero los locales (reutiliza `cctest.rs`, que pasa a un módulo común). Si no hay ninguno, descarga de un compilador portátil con URL y SHA-256 fijados en el código, a `<cache>/toolchain/`. *Alternativa:* TinyCC embebido (compila 430 000 líneas en segundos pero optimiza poco); se mide en las tareas y se decide si basta para 60 fps como compilador de último recurso.

**4. Compilación paralela por banco.** El C está partido en 23 ficheros (`rom_BB.c`); se compilan en paralelo y se enlazan, con `/O2` o `-O2`. En MSVC, el tiempo actual completo ronda minutos, así que se muestra progreso y se informa de que solo ocurre la primera vez.

**5. La ROM no se copia.** El ejecutable generado lleva la ruta de la ROM (o la recibe por argumento); `snes-port` la pasa al lanzarlo. El `.srm` va junto a la ROM del usuario, no en la caché, para que `clean` no pueda borrar partidas.

**6. Comprobación automatizada de lo distribuible.** El script de empaquetado recorre el árbol del paquete y falla si encuentra extensiones de ROM o C generado. Un test busca secuencias de 32 bytes de la ROM dentro de los ficheros de la caché de prueba.

## Risks / Trade-offs

- [Primera construcción lenta (minutos)] → compilar en paralelo, mostrar progreso, cachear; permitir `-O1` en un modo rápido para desarrollo.
- [Usuario sin compilador ni red] → mensaje claro con los pasos para instalarlo a mano; fuera de alcance un respaldo sin compilador.
- [Que la ruta de la ROM cambie] → el ejecutable la pide de nuevo por argumento, no la guarda en la caché.
- [Riesgo jurídico residual: las leyes de cada país difieren] → el diseño evita distribuir material protegido, pero esto no es asesoría legal; se recomienda a quien publique el proyecto que lo revise por su cuenta.

## Migration Plan

`codegen --all` y la compilación manual siguen funcionando como hoy. El subcomando nuevo se añade sin romper nada; el paquete de distribución se produce con un script de release y se prueba en una máquina limpia (sin ROM, sin caché).

## Measurements (task 3.1)

Hecho el 2026-10-25 en la máquina de desarrollo (Windows 11, MSVC 2022 Build Tools, TinyCC 0.9.27). Código: `codegen --all` de ALttP, 427 370 líneas de C en 23 ficheros. Tiempo de construcción del runtime sin vídeo (`snes_headless.c`, mismo código del juego); fps medidos sin límite de ritmo en 1200 fotogramas del título y en 7000 del overworld.

| Compilador | Construcción | fps título | fps overworld |
|---|---|---|---|
| MSVC `/O2 /MP` | 40 s | 234 | 234 |
| MSVC `/O1 /MP` | 20 s | 227 | – |
| MSVC `/Od /MP` | 2 s | 143 | – |
| TinyCC 0.9.27 (un solo hilo) | 0,7 s | 118 | 121 |

El frontal con SDL2 y TinyCC: 115 fps con `--no-window` (MSVC `/O2`: 244 fps) y 60 fps sostenidos con ventana real. Hacen falta tres ajustes: definir `DECLSPEC=` y `SDLCALL=` vacíos (las cabeceras de SDL2 usan una sintaxis que TinyCC no entiende), generar `SDL2.def` con `tcc -impdef SDL2.dll` y no usar `commdlg.h` (que TinyCC no trae: el diálogo de elegir ROM se omite, y el constructor siempre pasa la ROM por argumento).

**Decisión:** TinyCC 0.9.27 es el compilador portátil de último recurso. Compila en menos de un segundo y da casi el doble del ritmo necesario (60 fps), aunque la mitad que MSVC `/O2`; en un equipo mucho más lento que el de desarrollo podría quedarse corto, y entonces conviene instalar un compilador de verdad (el programa lo dice). Se descarga de `https://download.savannah.gnu.org/releases/tinycc/tcc-0.9.27-win64-bin.zip` (489 586 bytes, SHA-256 `34a721949a2583fdff725312da092fa0f5f1f284b702e6f811c6954714faabb2`, calculado al descargarlo por primera vez desde la fuente oficial) a `<caché>/toolchain/`. Los compiladores del sistema siguen teniendo prioridad.

## Open Questions

- (Resuelta) Compilador portátil de último recurso: TinyCC, ver «Measurements».
