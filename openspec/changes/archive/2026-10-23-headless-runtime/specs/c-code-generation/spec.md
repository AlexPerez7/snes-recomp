## ADDED Requirements

### Requirement: Punto de sondeo por bloque
El sistema SHALL emitir `rt_tick(c);` como primera línea de cada bloque, y `snes_rt.h` MUST definirlo como una expresión vacía salvo cuando se compila con `SNES_RT_TICK`, en cuyo caso llama a un gancho del runtime con el estado de la CPU.

#### Scenario: Cada bloque
- **WHEN** se genera una función con tres bloques
- **THEN** cada uno empieza con `rt_tick(c);`

#### Scenario: Sin SNES_RT_TICK
- **WHEN** el C se compila sin `SNES_RT_TICK`
- **THEN** `rt_tick` no genera ninguna llamada ni exige ningún símbolo del runtime

#### Scenario: Con SNES_RT_TICK
- **WHEN** el C se compila con `SNES_RT_TICK`
- **THEN** cada bloque ejecutado llama al gancho del runtime, también al volver a un bloque por un salto hacia atrás

### Requirement: Despacho por dirección de entrada
El sistema SHALL escribir con `--all` un `snes_entry.c` con una función que, dada una dirección de 24 bits, llama a la función generada de esa entrada y avisa con su valor de retorno de si la dirección no es la entrada de ninguna.

#### Scenario: Entrada conocida
- **WHEN** se pide el despacho de la dirección de una función generada
- **THEN** se ejecuta esa función y el retorno indica éxito

#### Scenario: Dirección desconocida
- **WHEN** se pide el despacho de una dirección que no es entrada de función
- **THEN** no se ejecuta nada y el retorno indica fallo

### Requirement: Ficheros de apoyo del runtime
El sistema SHALL escribir con `--all`, junto al C generado, el runtime sin hardware (`snes_headless.c`), de modo que los ficheros del directorio compilen juntos con un compilador de C sin más dependencias, y MUST NOT incrustar la ROM en ninguno de ellos.

#### Scenario: Directorio completo
- **WHEN** se ejecuta `codegen --all -o <dir>`
- **THEN** el directorio contiene `snes_entry.c` y `snes_headless.c` además de los ficheros por banco y las cabeceras

#### Scenario: Sin datos de la ROM
- **WHEN** se examinan `snes_entry.c` y `snes_headless.c`
- **THEN** no contienen bytes de la ROM, solo direcciones de funciones y código
