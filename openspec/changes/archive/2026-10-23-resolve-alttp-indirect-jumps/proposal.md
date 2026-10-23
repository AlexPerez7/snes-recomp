## Why

El descubrimiento de código de A Link to the Past alcanza 16.577 instrucciones, pero deja 29 saltos indirectos sin resolver y solo hay 1 tabla configurada (`module_dispatch`). Mientras esos saltos no tengan destinos, el grafo de funciones y el generador de C tendrán huecos: el código alcanzable solo a través de ellos no se descubre.

## What Changes

- Inspeccionar en el ROM real cada uno de los 29 sitios indirectos (lista de `discover --unresolved`) y clasificarlos: tabla `indirect`, `split`, `inline`, o salto no resoluble estáticamente (p. ej. dirección calculada en RAM).
- Añadir a `games/alttp_usa.toml` una `[[table]]` por cada sitio resoluble, con comentario que describe el patrón y cómo se verificó a mano.
- Documentar en el propio TOML los sitios que no se pueden resolver estáticamente y el motivo.
- Medir antes/después con `discover` (instrucciones alcanzables y sitios sin resolver) y registrar el resultado.
- Los 29 sitios originales eran solo el principio: al resolverlos aparece código nuevo con más saltos indirectos, así que se itera hasta que `discover --unresolved` no lista ninguno.
- Los helpers de despacho `00:8781` (punteros de 16 bits) y `00:879C` (24 bits) reciben su tabla en línea tras cada `JSL` y la longitud varía en cada llamada. En vez de una tabla `inline` por llamada, se añade a la configuración `[[inline_helper]]` y el descubrimiento delimita la tabla de cada llamada (entradas que parecen punteros, hasta el primer destino situado en el mismo banco por encima de la tabla; los huecos `0000` se saltan).
- Se añade `[[not_code]]` para relleno o datos a los que apuntan las tablas y que no deben decodificarse.

## Capabilities

### New Capabilities
- `indirect-jump-resolution`: resolución de saltos y llamadas indirectas mediante tablas declaradas en la configuración por juego, y reporte de los sitios que quedan sin resolver.

### Modified Capabilities

(ninguna; `openspec/specs/` está vacío)

## Impact

- `games/alttp_usa.toml`: nuevas tablas y comentarios.
- `src/config.rs`, `src/discover.rs`: solo si hace falta un nuevo tipo de tabla.
- Tests: comprobación de que las tablas configuradas parsean y de que el número de sitios sin resolver disminuye.
- Fuera de alcance: grafo de funciones, generador de C y runtime; juegos distintos de ALttP.
