## ADDED Requirements

### Requirement: Ganchos sobre funciones recompiladas
El perfil SHALL poder declarar `[[hook]]` con `addr` (entrada de una función del grafo), `kind` (`before`, `after` o `replace`) y `call` (nombre de una función C del juego con la firma `void nombre(Cpu *c)`). El generador MUST emitir en la función de esa dirección una llamada a `call`: antes de su cuerpo con `before`, cuando la función vuelve (por cualquiera de sus retornos) con `after`, y en lugar del cuerpo con `replace`.

#### Scenario: Gancho before
- **WHEN** un `[[hook]]` `before` apunta a una función
- **THEN** `call` se ejecuta con el estado de la CPU de la entrada, antes de la primera instrucción de la función

#### Scenario: Gancho after
- **WHEN** un `[[hook]]` `after` apunta a una función con dos instrucciones de retorno y se llama una vez por cada camino
- **THEN** `call` se ejecuta una vez en cada llamada, con el estado de la CPU de ese retorno

#### Scenario: Gancho replace
- **WHEN** un `[[hook]]` `replace` apunta a una función
- **THEN** el cuerpo original no se emite, se llama a `call` y la función vuelve a quien la llamó con el tipo de retorno (`rts` o `rtl`) indicado por el campo `ret` del gancho

#### Scenario: Dirección que no es una función
- **WHEN** el `addr` de un `[[hook]]` no es la entrada de ninguna función del grafo
- **THEN** el generador informa del error con la dirección y no genera nada

#### Scenario: Sin ganchos
- **WHEN** el perfil no declara ningún `[[hook]]`
- **THEN** el C generado es idéntico al que se generaba antes de este cambio

### Requirement: Código C propio del juego
Si existe un directorio hermano del perfil con el mismo nombre sin extensión, `snes-port` SHALL copiar sus `*.c` y `*.h` al directorio de construcción y compilar los `.c` junto al C generado, con acceso a `snes_rt.h`. Ese código MUST poder incrustarse en el binario y distribuirse, y el programa MUST seguir sin incluir bytes de la ROM en lo construido.

#### Scenario: C del juego compilado
- **WHEN** el directorio del perfil contiene `hooks.c` con la función citada por un gancho
- **THEN** el ejecutable construido enlaza esa función y el gancho la llama

#### Scenario: Función de gancho ausente
- **WHEN** un `[[hook]]` cita una función que ningún `.c` del juego define
- **THEN** la construcción falla con un error que nombra el gancho y la función

#### Scenario: Cambio en el C del juego
- **WHEN** se edita un `.c` del directorio del perfil
- **THEN** la siguiente ejecución reconstruye y no reutiliza la caché anterior

### Requirement: Interfaz del C del juego con el runtime
`snes_rt.h` SHALL ofrecer al C del juego el estado de la CPU y funciones para leer y escribir el bus y la WRAM y para leer el estado del mando 1, sin exigirle conocer detalles del frontal (SDL o sin vídeo).

#### Scenario: Leer y escribir WRAM
- **WHEN** una función de gancho escribe un byte en WRAM con la interfaz del runtime
- **THEN** el código recompilado que lee esa dirección ve el valor escrito

#### Scenario: Mismo gancho en ambos frontales
- **WHEN** el mismo `hooks.c` se compila con el runtime sin vídeo y con el frontal interactivo
- **THEN** compila y se comporta igual en los dos
