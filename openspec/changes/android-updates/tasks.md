# Tasks

## 1. Comprobación

- [x] 1.1 Clase de comprobación (`HttpURLConnection`, `org.json`, 3 s), comparación numérica de versiones y caché de 24 h en las preferencias. Verificar: pruebas con etiquetas `v0.2`, `v0.10`, `v0.1`, ilegible, 404 y sin red. Hecho y probado en el S25 Ultra con un servidor local: `v0.2` y `v0.10` frente a `0.1` avisan (la comparación es numérica: 10 > 1), `v0.1` y `v0.0.9` no, `nightly` y una dirección sin servidor se ignoran y abren el juego.
- [x] 1.2 Dirección `snesPort.updateUrl` como recurso; vacía oculta todo. En depuración, `files/update_url` la sustituye. Hecho: `snesPort.updateUrl` va a un recurso de cadena; en depuración `files/update_url` la sustituye. Sin probar con la dirección vacía (la pantalla de ajustes debería no mostrar nada de actualizaciones).
- [x] 1.3 Permiso de Internet, y de red solo para esto. Verificar con `aapt2 dump permissions`. Hecho: `aapt2 dump permissions` del APK de release da `VIBRATE` e `INTERNET`; en release no hay `networkSecurityConfig` (solo HTTPS), el HTTP a localhost es solo de depuración.

## 2. Interfaz

- [x] 2.1 Diálogo de versión nueva (Descargar, Ahora no, Omitir) al abrir con el ajuste activo, sin retrasar el juego más de 3 s. Hecho: el diálogo sale con la espera a la red como mucho 3 s; «Descargar» abrió la app de GitHub del teléfono, «Omitir» guarda la etiqueta (una etiqueta distinta vuelve a avisar), «Ahora no» y cancelar abren el juego.
- [x] 2.2 Pantalla de ajustes: interruptor (apagado por defecto), «Buscar ahora» con resultado en texto, versión instalada y licencias; accesible con el atajo de la app aunque haya ROM. Hecho: pantalla de inicio con interruptor (apagado en una instalación nueva), «Buscar actualizaciones ahora» («Estás al día», «No se pudo comprobar» y el diálogo), versión, «Jugar», «Cambiar ROM» y licencias; el atajo estático del icono («Ajustes y actualizaciones») está registrado en el sistema, pero no se ha probado a mantener pulsado el icono con el dedo; con la acción `io.github.alexperez7.snesport.SETTINGS` la pantalla se abre.
- [x] 2.3 Abrir normal con el ajuste apagado va directo al juego, sin red. Verificar sin ninguna petición (`dumpsys netstats` o logcat). Comprobado con un servidor local que registra las peticiones: 0 con el ajuste apagado y 1 con el ajuste activo.

## 3. Entrega

- [x] 3.1 Documentar en `android/README.md` (permiso, etiquetas `vX.Y`, versionName) y en `CLAUDE.md`. Hecho: `android/README.md` y una línea en `CLAUDE.md`.
- [ ] 3.2 En `alttp-recomp` (cambio aparte): el workflow etiqueta `vX.Y` y la `versionName` coincide.

## Pendiente tras la implementación

- La API real de GitHub devuelve 404 para `alttp-recomp` porque aún no hay ninguna release: probar con una real cuando se publique la primera.
- Mantener pulsado el icono (atajo) con el dedo.
