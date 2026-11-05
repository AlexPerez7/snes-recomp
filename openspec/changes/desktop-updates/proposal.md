# desktop-updates

## Por qué
Windows y Linux se reparten como paquetes descargables; sin aviso, quien lo instaló no sabe que hay una versión nueva ni cómo ponerla.

## Qué cambia
`src/update.rs`: el lanzador (Win32 en Windows, SDL en `linux/launcher.c`) pregunta a la API de GitHub por la última release del repositorio que dice `update.info` (`repo=`, `version=`, junto al ejecutable; sin él no hay nada que comprobar). **Apagado por defecto** (casilla «Buscar actualizaciones (usa Internet)»; guarda `update.txt`); como mucho una consulta al día. Si la etiqueta (`vX.Y`, comparación numérica) es mayor: Actualizar / Ahora no / Omitir esta versión. Actualizar descarga el paquete de la plataforma (`*-win64.zip` o `*-linux-x86_64.tar.gz`) y el `SHA256SUMS` de la misma release; si el hash no coincide o falta, no instala; si coincide, extrae con `tar` y copia los archivos encima (lo que no está en el paquete, como `saves\`, se conserva; un archivo en uso se aparta como `.old`, que se borra al arrancar) y reinicia el lanzador. Solo se usan enlaces HTTPS (y 127.0.0.1 para probar; `SNES_PORT_UPDATE_API` cambia la dirección de la API).

## Qué no hace
No firma las releases: la confianza descansa en el repositorio de GitHub (el hash viene de la misma release; protege de descargas corruptas, no de una cuenta comprometida). No elimina archivos viejos del paquete anterior. No actualiza Android (tiene su propio aviso, que abre la página).
