# c-code-generation Specification

## Purpose
Traducir las funciones y bloques básicos de una ROM a código C compilable que reproduce el comportamiento del 65816 y se comunica con el resto del sistema únicamente a través de una interfaz de runtime fija.

## Requirements

### Requirement: Una función C por función del grafo
El sistema SHALL emitir una función C por cada función del grafo, con nombre derivado de su dirección de 24 bits, que recibe el estado de la CPU y no devuelve valor.

#### Scenario: Nombre y firma
- **WHEN** se emite la función con entrada en `00:8010`
- **THEN** el C contiene `void f_00_8010(Cpu *c)`

#### Scenario: Salida determinista
- **WHEN** se genera el mismo código dos veces con la misma ROM y configuración
- **THEN** los dos ficheros son idénticos byte a byte

### Requirement: Semántica de instrucciones
El sistema SHALL traducir los 256 opcodes del 65816 a C que actualice registros, memoria y banderas N, V, Z y C como lo hace la CPU, usando el ancho de 8 o 16 bits que M y X fijan en esa instrucción.

#### Scenario: Acumulador de 16 bits
- **WHEN** `LDA #$1234` se ejecuta con M=0
- **THEN** el acumulador queda en `$1234` y Z y N reflejan el valor de 16 bits

#### Scenario: Acumulador de 8 bits
- **WHEN** `LDA #$80` se ejecuta con M=1
- **THEN** se carga el byte bajo, el alto del acumulador se conserva y N queda activa

#### Scenario: Acarreo en la suma
- **WHEN** `ADC #$01` se ejecuta con M=1, A=`$FF` y C=0
- **THEN** A queda en `$00`, C y Z quedan activas

#### Scenario: Modo decimal
- **WHEN** `ADC` o `SBC` se ejecutan con la bandera D activa
- **THEN** el resultado sigue la aritmética BCD del 65816

### Requirement: Anchos M/X por instrucción
El sistema SHALL conocer los anchos M y X vigentes en cada instrucción descubierta, y MUST informar de las direcciones alcanzadas con anchos distintos en lugar de elegir una al azar, salvo cuando el C de esa instrucción y su longitud son los mismos con todos los anchos vistos.

#### Scenario: Anchos conocidos
- **WHEN** una instrucción se alcanza solo con M=0, X=1
- **THEN** el generador usa esos anchos para ella

#### Scenario: Anchos en conflicto
- **WHEN** una dirección se alcanza con anchos distintos y el C o la longitud de su instrucción dependen de la diferencia
- **THEN** la generación de una función que la contiene falla con un error que indica la dirección y los anchos vistos, sin producir C parcial

#### Scenario: Anchos en conflicto que no cambian el C
- **WHEN** una dirección se alcanza con anchos distintos pero su instrucción produce el mismo C y tiene la misma longitud con todos ellos, como un `SEP #$30` o un `STA $10` cuando solo difiere X
- **THEN** la función que la contiene se genera sin error

#### Scenario: Comprobación en ejecución
- **WHEN** el C se compila con `SNES_RT_CHECKS`
- **THEN** cada bloque comprueba al entrar que M y X de la CPU coinciden con los supuestos y avisa a través de `rt_flag_mismatch` si no

#### Scenario: Comprobación en ejecución con anchos variables
- **WHEN** el C se compila con `SNES_RT_CHECKS` y el bloque empieza en una dirección alcanzada con valores distintos de M (o de X)
- **THEN** el bloque no comprueba M (o X) al entrar y sigue comprobando el otro ancho

### Requirement: Control de flujo
El sistema SHALL emitir cada bloque básico como una etiqueta, las ramas y saltos como `goto` entre etiquetas de la misma función, las llamadas como llamadas C que conservan en la pila emulada la dirección de retorno y comprueban al volver que el retorno es el esperado, y los retornos como la extracción de esa dirección, su anotación en `Cpu` y `return`.

#### Scenario: Rama condicional
- **WHEN** un bloque termina en `BNE`
- **THEN** el C salta a la etiqueta del destino si Z está inactiva y, si no, continúa con el bloque siguiente

#### Scenario: Llamada
- **WHEN** el código contiene `JSR $8010`
- **THEN** el C guarda `S`, empuja la dirección de retorno en la pila emulada, llama a `f_00_8010`, comprueba que el retorno es el esperado y continúa en la instrucción siguiente

#### Scenario: Tail call
- **WHEN** un `JMP` o `JML` apunta a otra función
- **THEN** el C llama a esa función y retorna a continuación, sin copiar sus bloques

### Requirement: Despacho indirecto
El sistema SHALL emitir los saltos y llamadas indirectos calculando el destino en ejecución como lo hace la CPU y despachando con un `switch` sobre los destinos conocidos del grafo. Esto incluye el `exit` de un helper de despacho en línea: tras llamar a la función del destino, el helper MUST retornar, y el sitio de la llamada MUST retornar sin ejecutar nada más.

#### Scenario: Destino conocido
- **WHEN** el destino calculado en ejecución es uno de los de la tabla
- **THEN** se llama a la función de ese destino

#### Scenario: Destino desconocido
- **WHEN** el destino calculado no está entre los conocidos
- **THEN** el C llama a `rt_unresolved` con la dirección del sitio y la del destino

#### Scenario: Salida de un helper en línea
- **WHEN** se ejecuta una llamada a un helper en línea y el `exit` calcula un destino de la tabla
- **THEN** se ejecuta la función de ese destino, la pila queda con el estado que tendría en la CPU tras el salto y el control vuelve al llamador del sitio de la llamada, no a la instrucción siguiente

### Requirement: Interfaz con el runtime
El sistema SHALL generar C cuyo único contacto con el resto del sistema sea la cabecera `snes_rt.h`, que define el estado de la CPU y declara las funciones de bus y los ganchos de runtime; todo acceso a memoria MUST pasar por el bus.

#### Scenario: Acceso a memoria
- **WHEN** una instrucción lee o escribe memoria
- **THEN** el C llama a las funciones de bus con la dirección de 24 bits efectiva y no accede a arrays de memoria

#### Scenario: Instrucciones de sistema
- **WHEN** el código contiene `WAI`, `STP`, `BRK`, `COP` o `WDM`
- **THEN** el C llama al gancho de runtime correspondiente en lugar de resolverlo por su cuenta

### Requirement: Subcomando de generación
El sistema SHALL ofrecer `codegen <rom> <addr>` que emite la función en `<addr>` y las funciones a las que llama hasta `--depth` niveles (1 por defecto), declara sin definir las demás, y escribe en la salida estándar o en el fichero de `-o`.

#### Scenario: Profundidad
- **WHEN** se ejecuta con `--depth 1` y la función llama a otra
- **THEN** se emiten las dos y las llamadas de la segunda quedan como declaraciones

#### Scenario: Dirección que no es entrada
- **WHEN** `<addr>` no es la entrada de una función del grafo
- **THEN** el comando termina con error que lo indica, sin escribir salida

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

### Requirement: Retornos no locales
El sistema SHALL hacer que el flujo de control del C siga el de la CPU cuando un `RTS` o un `RTL` vuelve a una dirección distinta de la que espera la llamada nativa. Cada `RTS` y `RTL` MUST dejar en `Cpu` la dirección que sacó de la pila, y cada sitio de llamada MUST comparar, al volver la función llamada, el `S` y esa dirección con los que esperaba; si no coinciden, MUST retornar sin ejecutar nada más.

#### Scenario: Descartar un retorno
- **WHEN** una función llamada con `JSR` ejecuta `PLA`, `PLA` y `RTS`, y fue llamada desde otra función que a su vez fue llamada con `JSR`
- **THEN** la ejecución continúa en la instrucción siguiente a la llamada de la función exterior, y la función intermedia no ejecuta nada tras su llamada

#### Scenario: Pila coherente
- **WHEN** ningún código descarta direcciones de retorno
- **THEN** cada llamada continúa en la instrucción siguiente y `S` vale lo mismo antes y después de ella

#### Scenario: Dirección distinta con la misma pila
- **WHEN** una función saca su dirección de retorno y empuja otra en su lugar antes del `RTS`
- **THEN** el sitio de la llamada no continúa en la instrucción siguiente
