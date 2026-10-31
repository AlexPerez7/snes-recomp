## 1. Valor por defecto

- [x] 1.1 Poner `FILTER_SHARP` como valor por defecto de `filter` en `opt_defs` (`runtime/play_cfg.h`)
- [x] 1.2 Actualizar la tabla del README y el comentario del `config.ini` generado si citan `nearest` como defecto
- [x] 1.3 Cambiar los tests de `src/headless.rs` que esperan `filter = nearest` (config por defecto, `--print-config`) y añadir uno con `filter = nearest` escrito que se respete

## 2. Factor de `sharp`

- [x] 2.1 En `present()` calcular `k = ceil(max(dst.w / pw, dst.h / lines))` entre 1 y 8
- [x] 2.2 Si el factor es entero exacto en ambos ejes, dibujar con vecino más cercano sin textura previa
- [x] 2.3 Prueba con `--present-shot`: `sharp` a 3x exactos es idéntico a `nearest`, y en `free` (p. ej. 1600x900) el borde de los píxeles no se emborrona

## 3. Cierre

- [x] 3.1 `cargo test` completo (con el juego real si hay ROM) y comprobar a mano en ventana `free` y pantalla completa
