## 1. Configuración

- [x] 1.1 Guardar `exit` dentro de `InlineHelper` en `src/config.rs` y derivar `helper_exits` de él; ajustar el test `parses_inline_helpers`

## 2. Descubrimiento

- [x] 2.1 Medir antes: guardar la salida de `discover --widths` y el número de destinos de las tablas de los dos helpers de ALttP
- [x] 2.2 Acumular en `discover` los destinos por helper y añadir, al terminar, aristas `Dispatch` desde cada `exit` alcanzado
- [x] 2.3 Descubrir los destinos con los anchos vigentes en el `exit` cuando el helper lo declara; comprobar sobre ALttP si cambian y comparar los conflictos con 2.1
- [x] 2.4 Tests en `src/discover.rs`: `exit` con unión de dos llamadas, `exit` sin destinos si no hay llamadas, y destinos con los anchos del `exit`

## 3. Generador

- [x] 3.1 Comprobar con `graph` y `codegen` que el bloque del `exit` recibe los destinos y emite el `switch` con `return;`; ajustar el emisor solo si hace falta
- [x] 3.2 Test que ejecuta el C: llamada a un helper con dos destinos, el elegido se ejecuta, la pila queda en el valor esperado y el control vuelve al llamador del sitio de la llamada
- [x] 3.3 Test de texto: el `exit` tiene un `case` por destino y `rt_unresolved` en `default`

## 4. Verificación sobre ALttP

- [x] 4.1 `discover` informa 0 sitios sin resolver y ningún `exit` queda sin destinos
- [x] 4.2 `codegen` de una función que llame a `00:8781` (profundidad 2) compila con Visual Studio y se ejecuta sin `UNRESOLVED` en el helper
- [x] 4.3 `cargo test` en verde; actualizar `CLAUDE.md` y los comentarios de `src/config.rs` si cambia el formato de `exit`
