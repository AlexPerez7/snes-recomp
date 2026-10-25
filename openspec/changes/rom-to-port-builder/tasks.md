# Tasks

## 1. Orquestación de la construcción

- [ ] 1.1 Mover la detección de compilador de `src/cctest.rs` a un módulo común (`src/toolchain.rs`) usado por tests y por el builder. Verificación: `cargo test` sigue verde.
- [ ] 1.2 Subcomando `snes-port <rom>`: identifica por CRC32 (cabecera de copiador incluida), genera el C en un directorio temporal de la caché y compila en paralelo por banco. Verificación: test con la ROM real que produce un `game.exe` que corre 300 fotogramas (se omite si falta ROM o compilador).
- [ ] 1.3 Caché `<crc32>-<versión>` con escritura atómica y reconstrucción por cambio de versión. Verificación: test que comprueba que la segunda ejecución no invoca al generador y que cambiar el hash del runtime reconstruye.
- [ ] 1.4 Códigos de salida y mensajes para ROM desconocida y sin compilador. Verificación: tests de ambos casos (código 2, CRC32 mostrado, nada creado en la caché).

## 2. Garantías de lo derivado

- [ ] 2.1 Comprobar que el ejecutable y los ficheros de la caché no contienen secuencias de 32 bytes de la ROM. Verificación: test que escanea la caché de prueba.
- [ ] 2.2 Que el `.srm` vaya junto a la ROM y `snes-port clean [rom]` borre solo la caché. Verificación: test que crea un `.srm`, ejecuta `clean` y comprueba que el `.srm` y la ROM siguen.
- [ ] 2.3 Garantizar que no hay conexiones de red cuando hay compilador local (la descarga solo existe en la ruta sin compilador). Verificación: revisión de código más test que ejecuta con la red del proceso inhabilitada en la ruta local.

## 3. Compilador de último recurso

- [ ] 3.1 Medir TinyCC frente a clang/MSVC en tiempo de construcción y fps del juego. Verificación: tabla de medidas en `design.md` y decisión registrada.
- [ ] 3.2 Descarga con SHA-256 fijado, consentimiento explícito y descarte si el hash no coincide. Verificación: test con un servidor local que sirve un fichero alterado y comprueba el rechazo.

## 4. Distribución

- [ ] 4.1 Script `release` que arma el zip (programa, runtime, `games/*.toml`, docs, licencias) y falla si encuentra `.sfc`, `.smc`, `.srm` o C generado. Verificación: ejecutarlo con un fichero de prueba prohibido y ver el fallo; sin él, el zip se crea.
- [ ] 4.2 Prueba en una máquina o carpeta limpia (sin caché ni ROM en el árbol) siguiendo solo el README. Verificación: nota con los pasos y el resultado en `out/release-notes.md`.
- [ ] 4.3 Documentar el uso y el modelo «trae tu ROM» en `README.md` y `CLAUDE.md`. Verificación: los comandos documentados se ejecutan tal cual.

## Workflow follow-up

- Archivar con `/opsx:archive` tras completar el grupo 4; depende de `interactive-frontend` para lanzar la versión jugable.
