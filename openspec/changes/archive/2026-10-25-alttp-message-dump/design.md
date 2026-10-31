## Context

Observado en la ROM de ALttP (USA) con las herramientas del motor (no hay que volver a investigarlo):

- Hay 397 mensajes consecutivos en `1C:8000` (hasta un byte `$80`, que salta a `0E:DF40`) y terminan en `$FF`. Cada mensaje acaba en `$7F`. Los bytes `$88+` son palabras de diccionario (tabla en `0E:C703`), `<$67` letras y signos, `$67–$7E` comandos de uno o dos bytes (longitudes en `0E:D36B`).
- La tabla de punteros no está en la ROM: la construye el juego al arrancar (`0E:D3EB`, escribe `$7F71C0 + 3·id`, recorriendo los mensajes).
- El cargador `0E:C4E2` (id en `$1CF0`) expande el mensaje a `$7F1200` y es la única vía de entrada. Sus llamadores (`0E:C4B4`, `CD6D`, `CED6`, `CF53`, `CFC7`) ponen a cero `$1CD9` después de llamarlo. Los demás lectores del búfer solo leen bytes de parámetro de comandos, no letras.
- Tras `$7F1200` hay unos 3.500 bytes libres antes del uso intensivo de `$7F2000` (mapa); el mensaje inglés más largo ocupa 1.872 ya expandido.
- `game_opt`, `[[option]]`, `rom_overlay` y los ganchos `before/after/replace` ya existen. `replace` no emite el cuerpo original; `before` y `after` sí. El juego se lanza con el directorio de trabajo en la caché (`port.rs`), y la versión de la caché solo incluye los `*.c`/`*.h` de la carpeta del perfil.

## Goals / Non-Goals

**Goals:**
- Volcar los mensajes a texto legible y reconstruir exactamente el mismo búfer desde ese texto.
- Poder sustituir mensajes desde `games/alttp_usa/lang/es.txt`, parcial y editable sin reconstruir.
- Una prueba automática de que sustituir el inglés por sí mismo no cambia nada.

**Non-Goals:** letras con tilde, anchuras, límite de anchura de línea, nombres simbólicos de comandos, menús y créditos, y la traducción.

## Decisions

- **Gancho `after` sobre `0E:C4E2`, no `replace`.** El original corre siempre y el C solo pisa el búfer si hay traducción. Así un mensaje sin traducir sale en inglés y el volcado puede llamar al cargador original. No se tocan `$1CD9` ni `$1CDD`: los llamadores reinician el primero. Si la prueba de equivalencia mostrara una diferencia, el contador que falte se fija en el gancho.
- **Volcado: gancho `after` sobre `0E:D3EB`, activo con `--game-opt dump_text=1`.** Con la tabla de punteros ya completa, para `id = 0, 1, …` (hasta que el byte al que apunta el puntero sea `$FF`, leído con `bus_read8`) se copia el `Cpu` con `dp = 0`, se escribe `$1CF0`, se llama a la función pública del cargador (`f_0E_C4E2`) y se lee `$7F1200` hasta el `$7F` final, recorriendo los comandos con su longitud. Al terminar escribe el fichero y sale con `exit(0)`. Es un modo de extracción: no se restaura el estado del juego.
- **Fichero del volcado: `<rom_dir>/alttp_messages_en.txt`.** La carpeta de la ROM (`roms/`) está en `.gitignore`. Lleva una línea de aviso (`# Generado de tu ROM; no lo subas ni lo compartas`) y no se versiona.
- **Formato de texto.** Una cabecera `@NNN` (decimal, tres cifras) abre cada mensaje; las líneas que empiezan por `#` son comentarios. El texto son letras, cifras, espacio (`$59`) y `! ? . , '` por su carácter; cualquier otro byte va como `{xx}` (un byte) o `{xxyy}` (comando y parámetro). Los saltos de línea del fichero se ignoran, y el volcado escribe uno después de cada `{73}`, `{75}` o `{76}` para que el traductor vea el cuadro. Los espacios cuentan (no hay sangría). Un carácter o un `{…}` desconocido es un error con el número de línea.
- **Solo se nombran los signos verificados** por frecuencia y contexto en los 397 mensajes (`!` `$3E`, `?` `$3F`, `.` `$41`, `,` `$42`, `'` `$51`). El resto, incluidos los comandos, queda en hex; nombrarlos después es compatible hacia atrás porque el lector aceptará ambas formas.
- **Opción `language`** (`en`, `es`, por defecto `en`), declarada con `[[option]]` en el perfil. Con `es` el gancho carga `game_dir()/lang/es.txt`. Si el fichero no existe o falla, avisa una vez por `stderr` y deja el inglés.
- **Recarga:** el gancho mira la fecha de modificación del fichero (`stat`) cada vez que se abre un cuadro y lo vuelve a leer si cambió. Un error de lectura conserva la última versión buena y lo avisa.
- **Límite de tamaño:** un mensaje traducido de más de 3.500 bytes se rechaza (queda el inglés) con aviso. Al aplicar hay que comprobar con `--watch` que nada más usa `$7F1300–$7F1FBF`.
- **Rutas del motor:** `rom_dir()` y `game_dir()` (ver la spec `game-paths`). `GameProfile` guarda la carpeta hermana de su `.toml` cuando existe y `snes-port` pasa `--game-dir`; los datos (`lang/`) no entran en la versión de la caché.

## Verification (en `scripts/check.ps1`, con la ROM del usuario)

1. Volcar el inglés: debe haber 397 mensajes y el SHA-256 del fichero debe coincidir con una constante guardada (un hash del texto derivado, no bytes de la ROM).
2. Copiar el volcado a una carpeta temporal como `lang/es.txt`, volver a volcar con `language=es` y `--game-dir` apuntando a ella: los dos ficheros son idénticos.
3. Cambiar una línea de esa copia: el volcado difiere solo en ese mensaje (prueba de que el gancho actúa).
4. Con `es.txt` igual al inglés, la captura del primer cuadro de texto (a los 2.040 fotogramas, como la prueba de `dialog_skip`) es idéntica byte a byte con `language=en` y con `language=es`; esto cubre `$1CD9` y `$1CDD`.

## Risks / Trade-offs

- **Contadores `$1CDD` sin restaurar** → lo cubre la comprobación 4; si falla, se fijan en el gancho.
- **Espacio libre tras `$7F1200` por verificar** → el límite de 3.500 bytes y la comprobación con `--watch`.
- **Nombres de comandos sin conocer** → el hex es exacto aunque feo; se nombrarán cuando haya forma de dibujar y comprobar cada uno.
- **El volcado lleva texto de Nintendo** → solo se escribe junto a la ROM del usuario, que git ya ignora, y el aviso del fichero lo recuerda.
- **`exit(0)` desde un gancho** salta la limpieza del runtime → es un modo de extracción que no escribe `.srm` ni `config.ini` (se comprueba al aplicar).
