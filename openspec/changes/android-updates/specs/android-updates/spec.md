## ADDED Requirements

### Requirement: La comprobación está apagada por defecto
La app SHALL NOT conectarse a ningún servidor mientras el ajuste «Buscar actualizaciones al abrir» esté desactivado, y MUST traerlo desactivado en una instalación nueva. Con `snesPort.updateUrl` vacío, la app MUST NOT mostrar ajuste ni botón de actualizaciones.

#### Scenario: Instalación nueva
- **WHEN** la app se abre por primera vez
- **THEN** el ajuste está desactivado y no se hace ninguna petición de red

#### Scenario: Sin dirección de actualizaciones
- **WHEN** la app se compila con `snesPort.updateUrl` vacío
- **THEN** la pantalla de ajustes no muestra nada de actualizaciones

### Requirement: Aviso de versión nueva
Con el ajuste activo, la app SHALL consultar la última release a lo sumo una vez cada 24 horas y, si su etiqueta es una versión mayor que la instalada, MUST mostrar un diálogo con **Descargar**, **Ahora no** y **Omitir esta versión**. **Descargar** SHALL abrir en el navegador la página de la release; **Omitir** MUST guardar esa etiqueta para no volver a avisar de ella, y una etiqueta distinta MUST volver a avisar.

#### Scenario: Hay una versión mayor
- **WHEN** la versión instalada es 0.1 y la última release es `v0.2`
- **THEN** se muestra el diálogo con la versión nueva y los tres botones

#### Scenario: La versión es la misma o menor
- **WHEN** la última release es `v0.1` o `v0.0.9`
- **THEN** no se muestra nada y se abre el juego

#### Scenario: Versión omitida
- **WHEN** el usuario pulsa «Omitir esta versión» ante `v0.2` y vuelve a abrir la app
- **THEN** no se avisa de `v0.2`, y sí de `v0.3` cuando aparezca

### Requirement: Un fallo de red no estorba
En la comprobación automática, la app MUST esperar como mucho 3 segundos, MUST abrir el juego pase lo que pase con la red y MUST NOT mostrar errores de conexión, de la API ni de formato. El botón «Buscar ahora» SHALL decir en pantalla qué ha pasado (hay versión nueva, estás al día, no se pudo comprobar).

#### Scenario: Sin red
- **WHEN** el ajuste está activo y no hay conexión
- **THEN** el juego se abre en cuanto se agota la espera, sin ningún mensaje de error

#### Scenario: Botón sin red
- **WHEN** el usuario pulsa «Buscar ahora» sin conexión
- **THEN** la pantalla dice que no se pudo comprobar
