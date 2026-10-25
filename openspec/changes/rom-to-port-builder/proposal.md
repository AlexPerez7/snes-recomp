# Proposal

## Why

Para que el port se pueda compartir sin distribuir material con derechos de terceros, el programa que se reparte no debe contener ni la ROM ni código derivado de ella. Hoy el flujo es de desarrollador: ejecutar `codegen`, compilar el C con Visual Studio y lanzar el resultado a mano. Hace falta un único programa que cada usuario ejecute con **su** ROM y que construya el port en **su** máquina, de forma que todo lo derivado de la ROM nazca y se quede en local.

## What Changes

- Nuevo comando `snes-port <rom>` (y arrastrar la ROM sobre el ejecutable): identifica la ROM por CRC32, genera el C, lo compila y arranca el juego. No hace falta conocer `cargo`, `codegen` ni Visual Studio.
- Caché por usuario (`%LOCALAPPDATA%\snes-port\<crc32>\` o el equivalente del sistema) con el C generado y el ejecutable compilado, indexada por CRC32 y por versión del programa; la segunda vez arranca al instante y se reconstruye sola si el programa cambia.
- Selección del compilador: usa el que haya en la máquina (MSVC, clang, gcc) y, si no hay ninguno, un compilador portátil que el propio programa descarga con verificación de hash a la caché del usuario. Nada del compilador ni de la ROM va en el repositorio.
- Rechazo claro de ROMs no reconocidas, con el CRC32 hallado y la lista de juegos soportados.
- Garantías de privacidad y legales visibles: el programa nunca sube ni copia la ROM, nunca incluye bytes de la ROM en el ejecutable generado, y borra la caché con `snes-port clean`.
- Distribución: un paquete (zip) con `snes-port.exe`, el runtime y los `games/*.toml`, sin ROM ni C generado, más documentación de uso.

## Capabilities

### New Capabilities
- `port-builder`: construcción local, en un solo paso, del ejecutable jugable a partir de la ROM del usuario, con caché, selección de compilador y garantías de que nada derivado de la ROM sale de la máquina.

### Modified Capabilities

## Impact

- `src/main.rs`: nuevo subcomando y orquestación (hoy `codegen --all` y la compilación son pasos separados); nuevo módulo para caché y compilador (reutiliza la detección de `src/cctest.rs`).
- Dependencias: descarga HTTPS con verificación SHA-256 solo para el compilador portátil; opcional si hay compilador local.
- Empaquetado: script de release que arma el zip sin ROM; `.gitignore` ya cubre ROM, `.srm` y `out/`.
- Depende de `interactive-frontend` para el ejecutable jugable; hasta entonces construye y lanza el runtime sin vídeo.
- Fuera de alcance: asesoría jurídica (el diseño minimiza el riesgo, pero el usuario decide), instaladores gráficos, otros sistemas operativos más allá de que el diseño no los impida.
