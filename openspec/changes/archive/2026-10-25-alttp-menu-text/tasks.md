## 1. Inventario en la ROM

- [x] 1.1 Listar todos los registros de tilemap de texto de la selección de partida y del registro de nombre (dirección, longitud, cadena) leyendo el banco `0C`, y confirmarlos con capturas de cada pantalla
- [x] 1.2 Localizar la rutina que abre cada pantalla y comprobar con `graph`/`disasm` que copia esos registros a VRAM sin otra copia del texto
- [x] 1.3 Anotar en `design.md` las claves, sus direcciones y sus huecos, y cualquier cadena que no sea un registro

## 2. Texto del menú (repositorio `alttp-recomp`)

- [x] 2.1 `games/alttp_usa/menutext.c`: tabla de claves, comprobación de cada registro contra la ROM, conversión de texto a tiles (letras, cifras, espacio y signos de la fuente) y relleno con espacios
- [x] 2.2 Volcado con `--game-opt dump_menu=1` a `<rom_dir>/alttp_menu_en.txt` (una línea `clave = TEXTO`, longitud máxima en comentario), sin dejar `.srm` ni `config.ini`
- [x] 2.3 Lectura de `lang/es_menu.txt` con `language = es`: recarga por fecha, restauración de los bytes originales de las claves quitadas, avisos con clave y línea por texto largo o carácter sin glifo, y aviso único si falta el fichero
- [x] 2.4 Ganchos en `games/alttp_usa.toml` para aplicar la superposición al abrir cada pantalla
- [x] 2.5 `.gitignore`, README del juego y `CLAUDE.md` (formato de `es_menu.txt`, límites y qué no se traduce)

## 3. Verificación

- [x] 3.1 `scripts/check.ps1`: número de claves del volcado, capturas idénticas con `es_menu.txt` igual al volcado, diferencia acotada al cambiar una clave (pocos bytes del mapa de BG3) y WRAM idéntica a los 2040 fotogramas con los menús traducidos (la SRAM no se puede comparar con el runtime sin vídeo, que no la guarda; el nombre pasa por la WRAM)
- [x] 3.2 `cargo test` del motor y `scan` de las dos carpetas antes de subir nada
- [x] 3.3 Prueba manual con `play.bat`: cambiar una cadena de cada pantalla desde `es_menu.txt`, verla, editarla con el juego abierto y quitarla
