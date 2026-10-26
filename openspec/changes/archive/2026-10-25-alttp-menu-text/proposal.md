## Why

Los mensajes de ALttP ya se pueden traducir con `lang/es.txt` (`alttp-message-dump`), pero las pantallas de selección de partida y de registro de nombre siguen en inglés porque no usan el sistema de mensajes: su texto son listas de tiles de 8x16 que el juego copia a VRAM. Se ha comprobado que están **sin comprimir** en el banco `0C` y con un formato regular, así que se pueden sustituir sin tocar la ROM del disco.

## What Changes

- **Juego (`alttp-recomp`):**
  - Volcado de los textos de esas dos pantallas con `--game-opt dump_menu=1` a `<carpeta de la ROM>/alttp_menu_en.txt`: una línea `clave = TEXTO` por cadena (por ejemplo `select.title`, `select.copy`, `name.title`, `name.end`), con la longitud máxima en un comentario. Deriva de la ROM: no se sube.
  - Con `language = es`, lectura de `games/alttp_usa/lang/es_menu.txt` y superposición en memoria (`rom_overlay`) de los registros de tilemap de esas cadenas, conservando la dirección de VRAM y la longitud de cada registro. Una clave sin traducir queda en inglés; un texto más largo que su hueco se rechaza con un aviso y queda en inglés; uno más corto se rellena con espacios.
  - El texto se limita a las letras, cifras y signos que la fuente ya tiene (A–Z, 0–9 y los signos presentes). Sin acentos ni «ñ» todavía.
  - Comprobación de equivalencia y de edición en `scripts/check.ps1`.
- **Motor:** sin cambios previstos; usa `rom_overlay`, `game_dir()`, `game_opt()` y los ganchos que ya existen. Si la superposición necesitara poder deshacerse, se añade en este cambio con su requisito.
- **No se tocan** los códigos de tile de las letras ni la rejilla de letras del registro de nombre: el nombre del jugador se guarda en la SRAM como esos códigos y no debe cambiar (compatibilidad de partidas).

## Capabilities

### New Capabilities

- `menu-text-translation`: traducción del texto fijo de los menús de un juego desde un fichero editable, sin tocar la ROM ni sus partidas guardadas (primer uso: selección de partida y registro de nombre de ALttP).

### Modified Capabilities

Ninguna.

## Impact

- Juego: `games/alttp_usa/menutext.c` (nuevo), `games/alttp_usa.toml` (ganchos), `scripts/check.ps1`, README, `.gitignore`.
- Motor: ninguno previsto.
- Fuera de alcance: menú de pausa, mapa y objetos (su texto aún no está localizado en la ROM), glifos con tilde y «ñ», cambiar la longitud de un hueco (reubicar registros), el texto gráfico («Game Over», títulos) y la traducción misma.
