## 1. Color math

- [x] 1.1 Cambiar `pixel_index` para que devuelva el índice de CGRAM y la capa que aportó el píxel, y añadir la composición de la subpantalla con la máscara de `$212D`; los tests de renderizado existentes siguen en verde
- [x] 1.2 Aplicar suma, resta, mitad, selección de capas de `$2131`, color fijo, subpantalla y regiones de `$2130` bits 4-5 con valor 0 o 3, en 5 bits por canal antes del brillo; verificar con tests de suma del color fijo, resta, mitad, capa no elegida, subpantalla y `nunca`
- [x] 1.3 Cambiar los avisos de `screenshot()`: quitar el del color math y avisar de las regiones que dependen de ventanas; actualizar `what_is_not_drawn_is_reported` y verificar el aviso nuevo

## 2. ALttP y documentación

- [x] 2.1 Regenerar ALttP, capturar el fotograma 1054 y comprobar a ojo que la pantalla de título se mezcla como se espera
- [x] 2.2 Actualizar `CLAUDE.md` (`--screenshot`) y la memoria de proyecto con el hallazgo del forced blank; `cargo test` completo en verde

## Hallazgos del diagnóstico (heredados del cambio descartado)

- Las capturas negras posteriores al fotograma ~1054 no se deben al renderizador: `$2100`=`80` (forced blank) del 1100 al 2500; en el 1054 vale `0F` y la captura muestra la pantalla de título.
- Al final de la ejecución `$420C`=`00`, las ventanas están a `00` y el color math está activo (`$2130`=`02`, `$2131`=`22`, `$2132` cambia 3 veces por fotograma).
- La causa del forced blank sostenido (`$10`=`14`) queda sin explicar; es un asunto de ejecución para otro cambio.
