## 1. Línea base

- [x] 1.1 Guardar la salida de `discover roms/alttp_usa.sfc --unresolved` (16.577 instrucciones, 29 sitios, 1 tabla) como referencia (guardada en `baseline.txt`)

## 2. Análisis de sitios

- [x] 2.1 Clasificar los sitios del banco 00 (00:8799, 00:87BD, 00:F8AE) desensamblando su entorno
- [x] 2.2 Clasificar los sitios del banco 01 (01:8939, 01:8980, 01:89B5, 01:89D9, 01:C30D, 01:C318)
- [x] 2.3 Clasificar los sitios del banco 02 (02:83C3, 02:87AC, 02:998A, 02:9ADF, 02:9AFD, 02:9C4E, 02:9D00, 02:9E8E, 02:9F28, 02:9FF0, 02:A47B)
- [x] 2.4 Clasificar el resto (07:8106, 08:837C, 09:A193, 09:F294, 09:F7A3, 0A:FE84, 0C:C0FF, 0C:EDCF, 0E:9885)

## 3. Configuración

- [x] 3.1 Añadir a `games/alttp_usa.toml` una `[[table]]` por sitio resoluble, con comentario del patrón y límite de `count`
- [x] 3.2 Documentar en el TOML los sitios no resolubles estáticamente y el motivo (ninguno quedó: los dos `JML [$00]` de los helpers se resuelven por llamada con `exit`)
- [x] 3.3 Ampliar `src/config.rs` y `src/discover.rs` con `[[inline_helper]]` (con `exit`) y `[[not_code]]`, con tests unitarios
- [x] 3.4 Declarar los helpers `00:8781` y `00:879C`, fijar `0C:C55E` a mano y marcar `0E:FB64` como no código

## 4. Verificación

- [x] 4.1 Repetir `discover --unresolved` hasta que no aparezcan sitios nuevos (3 rondas más la de los helpers: 0 sitios sin resolver)
- [x] 4.2 Contrastar a mano el final de cada tabla contra el ROM (cada tabla acaba donde empieza la rutina o los datos siguientes; las tablas de `module_dispatch` coinciden con 13 finales)
- [x] 4.3 Ejecutar `cargo test` y registrar el antes/después en el README
