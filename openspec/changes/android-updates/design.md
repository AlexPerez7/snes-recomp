# Design

## Context

La app no tenía permiso de Internet y lo anunciaba como una garantía. El usuario ha elegido el nivel 2 de tres posibles: avisar de una versión nueva y abrir la página de descarga, con la comprobación apagada por defecto. Los otros niveles quedan descartados por ahora: el 1 (solo un enlace) no sabe si hay versión nueva, y el 3 (descargar e instalar dentro de la app) necesita el permiso de instalar paquetes desconocidos y más cosas que pueden fallar.

## Decisions

**1. Sin librerías.** `HttpURLConnection` y `org.json` (que ya trae Android), con 3 s de espera de conexión y de lectura. La consulta es `GET https://api.github.com/repos/<dueño>/<repo>/releases/latest` y solo se leen `tag_name` y `html_url`.

**2. Comparación de versiones numérica** por partes (`v0.10` > `v0.9`), quitando la `v` inicial y tratando como 0 lo que no sea número. Una etiqueta que no se entienda se ignora. No hay comparación de `versionCode`, porque el servidor no lo publica.

**3. Una consulta al día.** El resultado (etiqueta, enlace y hora) se guarda en las preferencias de la app. Si hay una versión nueva ya conocida y no omitida, el diálogo sale sin esperar a la red. «Omitir esta versión» guarda esa etiqueta; una etiqueta distinta vuelve a avisar.

**4. Abrir normal va directo al juego.** El diálogo y la espera solo ocurren con el ajuste activo. Los ajustes se llegan con un atajo estático de Android (mantener pulsado el icono), que abre `LauncherActivity` con una acción propia y muestra la pantalla de inicio aunque ya haya ROM.

**5. La dirección viene del perfil de compilación** (`snesPort.updateUrl` → recurso de cadena). Vacía, no se muestran ajuste ni botón y no se consulta nada. En las compilaciones de depuración se puede sustituir con un fichero `files/update_url` para probar sin GitHub.

## Risks / Trade-offs

- [El permiso de Internet cambia lo que la app promete] → se dice en la descripción de la release y la comprobación nunca ocurre sin que el usuario la active.
- [La API de GitHub limita a 60 consultas por hora y dirección sin autenticar] → una consulta al día lo deja muy lejos del límite; ante un 403 o 404 la comprobación automática no dice nada.
- [Una etiqueta mal puesta puede avisar de una versión «nueva» que no lo es] → el workflow de `alttp-recomp` debe poner `vX.Y` y la misma `versionName`; es un cambio aparte.
- [Abrir el navegador no instala nada] → es lo que se quería: el usuario descarga el APK y Android comprueba que la firma es la misma.
