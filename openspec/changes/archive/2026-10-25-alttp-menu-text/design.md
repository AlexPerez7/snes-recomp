## Context

Observado en la ROM de ALttP (USA) con las herramientas del motor (capturas y volcados de VRAM del runtime sin vídeo):

- La selección de partida y el registro de nombre dibujan su texto en BG3 (2 bpp), con letras de 8x16: dos tiles apilados, el de arriba `N` y el de abajo `N+0x10`. `A`–`P` son `00`–`0F`, `Q`–`Z` son `20`–`29`, `I` es `AF` y el espacio es `A9` (atributo `18`). La fuente no tiene acentos ni «ñ».
- El texto está **sin comprimir** en el banco `0C` como registros de escritura a VRAM: dirección de VRAM (2 bytes, big endian), longitud en bytes (2 bytes, big endian), palabras de tilemap (little endian) y una lista que acaba en `$FF`. Cada cadena son dos registros: la fila de arriba y la de abajo, a `+0x20` palabras. Encontrados: `0C:E2A0–E4xx` (selección de partida, «COPY/ERASE PLAYER», «SELECT»), `0C:D430` (variante), `0C:E9C7` («REGISTER») y `0C:EBA5`, `0C:ED25`, `0C:ED41` («END»). «PLAYER SELECT» y «REGISTER YOUR NAME» llevan espacios más anchos y están partidos en varios registros.
- La rejilla de letras del registro de nombre son tiles sueltos con los mismos códigos de la fuente; el nombre se guarda en la SRAM como esos códigos.
- El formato exacto de cada registro: dirección de VRAM (2 bytes, big endian), **longitud menos uno** (2 bytes, big endian), palabras `(tile, 0x18)`; una lista acaba en `$FF`. Varias pantallas comparten una lista de registros seguidos (`0C:E456` para borrar, `0C:E63C` para copiar), y el código la copia a WRAM con un bucle `LDA abs,X / STA $1002,X` (por ejemplo `0C:D147`, función `f_0C_D13F`); un NMI la sube a VRAM. Como el bucle lee la ROM por el bus, `rom_overlay` basta: no hace falta un gancho por pantalla.
- Existen `rom_overlay`, `game_dir()`, `rom_dir()`, `game_opt()` y los ganchos `before/after/replace`; `language` ya es una opción del perfil y `text.c` ya lee `lang/es.txt` con recarga por fecha. El menú de pausa usa otra fuente (8x8) y su tabla no está localizada: queda fuera.

## Goals / Non-Goals

**Goals:**
- Sustituir el texto de las dos pantallas desde un fichero, con las mismas garantías que `es.txt` (mismo largo, fallo visible, recarga en marcha).
- Una prueba automática de que el volcado, vuelto a cargar, no cambia nada.

**Non-Goals:** reubicar registros para cambiar su longitud, tiles nuevos con acento o «ñ», el menú de pausa, y cualquier cambio de los códigos que ve la SRAM.

## Decisions

- **Inventario fijo en C, no un escáner.** `menutext.c` lleva una tabla de claves con la dirección de cada registro y la longitud de su hueco, y la comprueba contra la ROM al arrancar (que el registro empiece con la dirección de VRAM y la longitud esperadas, y acabe en `$FF`). Si no coincide, no se superpone nada. Alternativa descartada: buscar los registros por patrón en cada arranque; es más frágil y la ROM no cambia.
- **Una cadena = sus registros de arriba y de abajo.** El texto se convierte a tiles con la tabla de letras (`A`–`Z`, `0`–`9`, espacio y los signos que la fuente tenga) y se escribe el par `N` / `N+0x10` en cada registro, sin tocar la dirección ni la longitud. Se rellena con `A9` hasta el hueco.
- **Longitud fija.** Un texto más largo que el hueco se rechaza (aviso con clave y línea). Cambiar la longitud exigiría reubicar el registro y cambiar quién lo apunta, con ganchos por pantalla; se deja para el cambio de la fuente con acentos, cuando se vea cuánto aprieta.
- **Superposición con restauración.** Antes de superponer, `menutext.c` guarda los bytes originales (`bus_read8`) y, al recargar el fichero, vuelve a escribir los originales de las claves que ya no están: así quitar una línea devuelve el inglés sin reiniciar. Si hiciera falta, un requisito de motor para deshacer una superposición entra en este cambio.
- **Cuándo se aplica.** Al arrancar, y cuando cambia la fecha del fichero (como mucho una vez por segundo), comprobado desde el gancho por fotograma que el perfil ya tiene en `00:80B5`. Las pantallas copian los registros al abrirse, así que el cambio se ve la siguiente vez que se abren. No hay gancho por pantalla.
- **Fichero y volcado.** `lang/es_menu.txt` junto a `es.txt`; el volcado `alttp_menu_en.txt` va a la carpeta de la ROM (ignorado, como el de los mensajes). Una línea `clave = TEXTO`; líneas con `#` son comentarios; la longitud máxima va en un comentario del volcado.
- **Compatibilidad de partidas.** No se escribe sobre la rejilla de letras ni sobre la tabla de caracteres del nombre: solo los textos fijos (títulos, «COPY», «ERASE», «END»…).

## Verification (en `scripts/check.ps1`, con la ROM del usuario)

- El volcado tiene el número esperado de claves.
- Con `es_menu.txt` igual al volcado, la captura de la selección de partida y la del registro de nombre son idénticas a las de `language = en`.
- Cambiar una clave cambia solo esa cadena (diferencia de captura acotada a su rectángulo).
- Registrar «LINK» en inglés y en español deja la misma SRAM.

## Risks / Trade-offs

- [Los huecos son estrechos: «SELECCIONAR JUGADOR» no cabe en «PLAYER SELECT»] → Textos cortos («ELEGIR PARTIDA», «COPIAR», «BORRAR»); si aprieta, el cambio de la fuente permite reubicar.
- [Alguna pantalla repite una cadena en otro banco que no se ha encontrado] → El volcado y la prueba por captura lo muestran; se añade la clave que falte.
- [Las direcciones se comprueban contra esta ROM y otra revisión podría diferir] → El perfil ya fija el CRC32; con otro CRC el gancho no se activa.
- [Pantallas que dibujan letras sueltas desde un código y no desde un registro (puede haber alguna)] → Fuera del alcance si resulta ser así; se anota como pendiente.

## Inventario (tarea 1; «tiles» = longitud del hueco; las filas de abajo van 0x20 palabras más abajo, a `N+0x10`)

```
clave             arriba/abajo       VRAM(arriba)  tiles  texto original
select.title      0C:E2A8 / E2C8     6065          14     PLAYER  SELECT
select.copy       0C:E2E8 / E304     62C6          12     COPY  PLAYER
select.erase      0C:E320 / E33C     6306          12     ERASE PLAYER
select.copy_ok    0C:D246 / D258     62C6           7     COPY OK
select.erase_this 0C:D436 / D45C     62C6          17     ERASE THIS PLAYER
erase.title       0C:E456 / E472     6065          12     ERASE PLAYER
erase.which       0C:E48E / E4C2     6104          24     WHICH PLAYER DO YOU WANT
erase.to_erase    0C:E4F6 / E50E     6144          10     TO ERASE ?
erase.quit        0C:E526 / E532     6306           4     QUIT
copy.title        0C:E63C / E658     6065          12     COPY  PLAYER
copy.quit         0C:E674 / E680     6306           4     QUIT
copy.which        0C:E68D / E6A7     6104          6*    WHICH?   (* entre dos tiles de marco, 85 y 86)
copy.to_which     0C:E73A / E754     6151          9*    TO WHICH? (* entre marcos)
name.title        0C:E9C1 / E9F1     60A4          22     " REGISTER  YOUR  NAME " (con espacios de margen)
name.end          0C:EB95 / EBAB     62CC           3*    END (tras 6 tiles de cursor y marco)
name.end2         0C:ED15 / ED47     66C4           3+3*  END ... END (segunda página)
```

No son texto y no se tocan: los marcos y casillas de partida (`E359`…`E42B`, `E53F`…`E611`, `E6C1`…, `E76E`…) y las rejillas de letras del registro de nombre (`EA21`–`EB57`, `EBC1`–`ECDF`; sus códigos son los del nombre en la SRAM). Los espacios son el tile `A9` (abajo `B9`) y los signos `?` es `6F`/`7F`; el resto de signos de la fuente se confirma al implementar. Esta lista sale de buscar registros en toda la ROM; solo se encontraron en el banco `0C`.

## Open Questions

- ¿Qué signos de la fuente (guion, coma, punto, `?`, `!`) tienen tile? Se comprueba al implementar la conversión; solo cambia la lista de caracteres admitidos.
