# Proposal

## Why

La app de Android se reparte como APK en las releases de GitHub, sin tienda. Sin un aviso dentro de la app, quien la instala no se entera de que hay una versión nueva. Quien la instala sí puede actualizar sobre la versión anterior (el identificador y la firma son los mismos), pero tiene que acordarse de mirar la página de releases.

## What Changes

- **Comprobación de versión nueva, apagada por defecto.** Un ajuste «Buscar actualizaciones al abrir» (desactivado hasta que el usuario lo active) y un botón «Buscar ahora». Al abrir la app con el ajuste activo, consulta la última release de GitHub, compara su etiqueta con la `versionName` instalada y, si es mayor, muestra un diálogo con **Descargar** (abre la página de la release en el navegador), **Ahora no** y **Omitir esta versión**.
- **Como mucho una consulta al día** (el resultado se guarda), con 3 s de espera máxima, y los fallos de red no se muestran en la comprobación automática: la app sigue y se abre el juego.
- **Permiso de Internet.** La app lo declara (antes no lo tenía). No se conecta a ningún sitio mientras el ajuste esté apagado, ni envía nada más que la petición HTTPS a la API de GitHub; el servidor ve la dirección IP.
- **Pantalla de ajustes accesible con la ROM ya elegida:** un atajo de la app (mantener pulsado el icono › «Ajustes y actualizaciones») abre la pantalla de inicio con el ajuste, el botón, la versión y las licencias. Abrir la app normalmente sigue yendo directo al juego.
- **La dirección es del juego, no del motor:** `snesPort.updateUrl` en `gradle.properties`; vacía, la app no tiene nada de esto.

Fuera de alcance: descargar e instalar el APK desde la app (necesita el permiso de instalar paquetes desconocidos), comprobar la firma o la huella del APK descargado, notificaciones en segundo plano, y publicar las releases (workflow en `alttp-recomp`).

## Capabilities

### New Capabilities
- `android-updates`: aviso opcional de versión nueva en la app de Android.

### Modified Capabilities
- `android-app`: declara el permiso de Internet y tiene una pantalla de ajustes accesible con la ROM elegida.

## Impact

- `android/`: `LauncherActivity`, una clase nueva para la comprobación, el manifiesto (permiso, atajo), `build.gradle` y `gradle.properties` (`snesPort.updateUrl`).
- `alttp-recomp`: el workflow de releases debe etiquetar cada versión como `vX.Y` y la `versionName` del APK debe coincidir; cambio aparte.
