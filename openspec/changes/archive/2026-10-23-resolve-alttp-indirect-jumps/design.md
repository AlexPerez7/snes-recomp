## Context

`src/config.rs` ya soporta tres tipos de tabla (`indirect`, `split`, `inline`) y `src/discover.rs` los consume. Solo hay una tabla configurada (`module_dispatch`, tipo `split`). `discover --unresolved` lista 29 sitios, repartidos desde el banco 00 hasta el 0E (varios en los bancos 01 y 02). El trabajo es principalmente de análisis sobre el ROM real, no de código.

## Goals / Non-Goals

**Goals:**
- Reducir al mínimo los sitios sin resolver en ALttP con tablas verificadas a mano.
- Dejar documentado cada sitio: patrón, tabla o motivo de no resolución.

**Non-Goals:**
- Resolver saltos dependientes de datos en RAM mediante análisis de valores.
- Grafo de funciones, generación de C, runtime.

## Decisions

- **Trabajar por sitio con `disasm`**: para cada sitio, desensamblar el entorno (`disasm <rom> BB:AAAA n`), identificar el patrón (`JMP (a,X)`, `JSR (a,X)`, `JML [a]`, dispatcher con `RTS`, punteros en línea tras un `JSR`) y localizar la tabla. Alternativa descartada: heurística automática de detección de tablas; es más código y menos fiable que la verificación manual exigida por el proyecto.
- **Recuento de entradas**: se fija el `count` por el límite real de la tabla (siguiente dato no puntero, o rango de valores posible del índice), no por exceso, para no descubrir basura como código.
- **Ampliar el descubrimiento solo bajo demanda**: si un patrón no encaja, se añade el tipo mínimo con test unitario; si no, no se toca el código Rust.
- **Helpers en línea con `[[inline_helper]]`**: hay 213 llamadas a `00:8781`/`00:879C` en el ROM (51 alcanzables hoy) y cada una lleva su tabla detrás del `JSL`. Una tabla `inline` por llamada exigiría fijar 51+ longitudes a mano, y al ampliar el descubrimiento aparecerían más. Se delimita automáticamente: se leen entradas mientras parezcan punteros (≥ `$8000`, banco dentro del ROM), se saltan las `0000` y se corta en el destino más bajo del mismo banco por encima de la tabla. Una tabla `inline` explícita en el sitio tiene prioridad cuando la heurística falla (caso `0C:C55E`, con datos tras la tabla que parecen un puntero).
- **`exit` del helper**: el `JML [$00]` final de cada helper se declara para no listarlo como sin resolver.
- **`[[not_code]]`**: `0E:F664` apunta a relleno `$FF` (`0E:FB64`); marcarlo evita decodificar basura hasta el final del banco.
- **Medición**: registrar instrucciones alcanzables y sitios sin resolver antes y después de cada lote de tablas.

## Risks / Trade-offs

- [Una tabla con `count` demasiado grande descubre datos como código] → verificar el último destino a mano y comprobar que el desensamblado tiene sentido.
- [Sitios que dependen de RAM no son resolubles] → documentarlos y dejarlos como pendientes, sin inventar destinos.
- [El código nuevo descubierto expone más saltos indirectos] → iterar con `discover --unresolved` hasta estabilizar.
- [La heurística de longitud puede sobrar o faltar entradas en tablas en línea] → 38 de las 51 llamadas alcanzables terminan por el límite del primer destino superior (exacto); las 13 restantes se revisaron a mano y una (`0C:C55E`) se fijó con tabla explícita.
