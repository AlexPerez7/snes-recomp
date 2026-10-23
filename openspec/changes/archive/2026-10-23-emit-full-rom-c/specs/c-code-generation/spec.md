## ADDED Requirements

### Requirement: Generación de la ROM completa
El sistema SHALL ofrecer `codegen <rom> --all -o <dir>`, que emite una función C por cada función del grafo sin pedir dirección ni profundidad, y escribe el resultado en el directorio `<dir>`.

#### Scenario: Todas las funciones
- **WHEN** se ejecuta con `--all` sobre una ROM cuyo grafo tiene N funciones
- **THEN** cada una de las N está definida exactamente una vez en los ficheros generados

#### Scenario: Sin directorio de salida
- **WHEN** se ejecuta con `--all` y sin `-o`
- **THEN** el comando termina con error que lo indica, sin escribir nada

#### Scenario: Salida determinista
- **WHEN** se ejecuta dos veces con `--all` sobre la misma ROM y configuración
- **THEN** los ficheros generados son idénticos

### Requirement: Salida dividida por banco
El sistema SHALL repartir el C de `--all` en un fichero por banco de la entrada de cada función, escribir un `snes_funcs.h` con la declaración de todas las funciones y escribir `snes_rt.h` junto a ellos.

#### Scenario: Reparto por banco
- **WHEN** hay funciones con entrada en los bancos `00` y `02`
- **THEN** existen un fichero para cada uno de esos bancos y ninguno para bancos sin funciones, y cada función está en el de su banco

#### Scenario: Declaraciones compartidas
- **WHEN** una función de un banco llama a otra de otro banco
- **THEN** el fichero de la primera incluye `snes_funcs.h`, que declara a la segunda

### Requirement: Funciones que no se pueden generar
El sistema SHALL generar con `--all` todas las funciones que pueda aunque otras fallen, sustituir cada una de las que no puede traducir por una función con la misma firma que llama a `rt_unsupported` con su dirección de entrada, e informar de cada una con su causa.

#### Scenario: Una función falla
- **WHEN** una función tiene anchos en conflicto que cambian su C y otra no
- **THEN** la primera se emite como stub, la segunda con su código completo y el comando termina sin error

#### Scenario: Informe de causas
- **WHEN** termina `--all`
- **THEN** se informa del total de funciones, cuántas se generaron y cuántas son stub, y de cada stub su dirección y el motivo del fallo

#### Scenario: Stub ejecutado
- **WHEN** el C compilado ejecuta un stub
- **THEN** se llama a `rt_unsupported` con la dirección de entrada de esa función

### Requirement: Gancho de funciones no soportadas
El sistema SHALL declarar `rt_unsupported(Cpu *c, uint32_t entry)` en `snes_rt.h` como el gancho que el runtime implementa para las funciones que no se pudieron generar.

#### Scenario: Cabecera
- **WHEN** se escribe `snes_rt.h`
- **THEN** declara `rt_unsupported` y el C generado no usa ningún otro símbolo de runtime que la cabecera no defina
