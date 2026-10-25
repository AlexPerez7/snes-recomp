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

## Open Questions

- Elección final del compilador portátil de último recurso (se resuelve midiendo rendimiento y tamaño en las tareas).
