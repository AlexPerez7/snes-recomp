# launcher-language

## Por qué
El lanzador de Windows habla español o inglés según el idioma del sistema, pero el de Linux (`linux/launcher.c`) estaba escrito solo en español y ninguno de los dos deja cambiar de idioma a mano. Quien no lee español no podía usar el lanzador de Linux, y el README del juego pasa a ser principalmente inglés.

## Qué cambia
Primera plataforma: **Linux** (`linux/launcher.c`).
- Todos los textos del lanzador salen de una tabla `{inglés, español}` (`tr()`), incluidos los avisos de actualización, los mensajes de ROM y los títulos del selector de archivos (zenity y kdialog).
- Idioma inicial: el guardado en `~/.local/share/<juego>/lang.txt` si existe; si no, el del entorno (`LANGUAGE`, `LC_ALL`, `LC_MESSAGES`, `LANG`: español si empieza por `es`, inglés en cualquier otro caso).
- Un botón pequeño arriba a la derecha (un globo y el código del idioma actual, `EN` o `ES`) cambia de idioma al pulsarlo, o con la tecla **L**, y guarda la elección. Los mensajes que dependen del idioma (error de ROM, estado de la comprobación de versión) se vuelven a escribir al cambiar.

## Fuera de alcance
Windows (`launcher_win.rs`, que ya elige por el idioma del sistema) y Android; quedan para otros cambios con el mismo mecanismo (`lang.txt` / preferencia) y el mismo botón. El idioma del juego (`language=es`) y el del menú del motor no cambian.
