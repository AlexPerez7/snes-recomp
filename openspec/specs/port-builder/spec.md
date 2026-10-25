# port-builder Specification

## Purpose

Construir en la máquina del usuario, a partir de su propia ROM, el ejecutable jugable del juego, sin que nada derivado de la ROM salga de esa máquina ni forme parte de lo que se distribuye.

## Requirements

### Requirement: Construir y lanzar con un comando
`snes-port <rom>` SHALL identificar la ROM por su CRC32, generar el C, compilarlo y lanzar el juego, y MUST aceptar la ROM arrastrada sobre el ejecutable. Si la ROM ya está construida en la caché con la misma versión del programa, SHALL lanzar el juego sin regenerar ni recompilar.

#### Scenario: Primera ejecución
- **WHEN** se ejecuta `snes-port alttp_usa.sfc` sin caché
- **THEN** genera, compila y lanza el juego, y deja el resultado en la caché del usuario

#### Scenario: Segunda ejecución
- **WHEN** se repite el comando con la misma ROM y la misma versión
- **THEN** el juego arranca sin ejecutar el generador ni el compilador

#### Scenario: Versión nueva del programa
- **WHEN** la caché la hizo otra versión del programa
- **THEN** se reconstruye antes de lanzar

### Requirement: ROM no reconocida
Si el CRC32 de la ROM no corresponde a un juego soportado, el programa SHALL terminar con código de salida 2 sin generar nada, e indicar el CRC32 hallado y los juegos soportados.

#### Scenario: ROM desconocida
- **WHEN** se pasa una ROM cuyo CRC32 no figura en la lista
- **THEN** termina con código 2, muestra el CRC32 y no crea ficheros en la caché

#### Scenario: Cabecera de copiador
- **WHEN** la ROM tiene una cabecera de 512 bytes y el resto coincide con una conocida
- **THEN** se reconoce igualmente

### Requirement: Todo lo derivado queda en local
El programa MUST escribir lo derivado de la ROM (C generado, objetos, ejecutable, partidas) solo en la caché del usuario y junto a la ROM, MUST NOT incluir bytes de la ROM en ningún fichero generado, y MUST NOT enviar la ROM ni nada derivado de ella por la red. El ejecutable generado SHALL leer la ROM del disco al arrancar.

#### Scenario: Sin ROM incrustada
- **WHEN** se construye el port
- **THEN** ninguna secuencia de 32 bytes consecutivos de la ROM aparece en el ejecutable ni en los ficheros de la caché

#### Scenario: Sin red
- **WHEN** hay un compilador local y se construye el port
- **THEN** el programa no abre ninguna conexión de red

### Requirement: Compilador
El programa SHALL usar el primer compilador de C11 que encuentre (MSVC, clang o gcc) y, si no hay ninguno, SHALL ofrecer descargar un compilador portátil a la caché verificando su SHA-256, y MUST explicar con un mensaje claro qué falta cuando no pueda continuar.

#### Scenario: Sin compilador
- **WHEN** la máquina no tiene compilador y el usuario acepta la descarga
- **THEN** el compilador se descarga con hash verificado y la construcción continúa

#### Scenario: Hash incorrecto
- **WHEN** el fichero descargado no coincide con el SHA-256 esperado
- **THEN** se descarta, no se ejecuta y se informa del error

### Requirement: Limpieza
`snes-port clean` SHALL borrar la caché del usuario completa, o la de una ROM con `snes-port clean <rom>`, sin tocar la ROM ni los ficheros `.srm`.

#### Scenario: Borrar caché
- **WHEN** se ejecuta `snes-port clean`
- **THEN** la caché desaparece y la ROM y las partidas guardadas siguen intactas

### Requirement: Paquete distribuible sin material protegido
El paquete de distribución SHALL contener solo el programa, el runtime, las configuraciones `games/*.toml`, la documentación y las licencias, y MUST NOT contener ROM, C generado ni nada extraído de ella.

#### Scenario: Comprobación del paquete
- **WHEN** se ejecuta el script de empaquetado
- **THEN** falla si encuentra ficheros `.sfc`, `.smc`, `.srm` o C generado en el árbol del paquete
