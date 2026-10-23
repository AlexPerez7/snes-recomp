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
El sistema SHALL emitir cada bloque básico como una etiqueta, las ramas y saltos como `goto` entre etiquetas de la misma función, las llamadas como llamadas C que conservan en la pila emulada la dirección de retorno, y los retornos como la extracción de esa dirección seguida de `return`.

#### Scenario: Rama condicional
- **WHEN** un bloque termina en `BNE`
- **THEN** el C salta a la etiqueta del destino si Z está inactiva y, si no, continúa con el bloque siguiente

#### Scenario: Llamada
- **WHEN** el código contiene `JSR $8010`
- **THEN** el C empuja la dirección de retorno en la pila emulada, llama a `f_00_8010` y continúa en la instrucción siguiente

#### Scenario: Tail call
- **WHEN** un `JMP` o `JML` apunta a otra función
- **THEN** el C llama a esa función y retorna a continuación, sin copiar sus bloques

### Requirement: Despacho indirecto
El sistema SHALL emitir los saltos y llamadas indirectos calculando el destino en ejecución como lo hace la CPU y despachando con un `switch` sobre los destinos conocidos del grafo.

#### Scenario: Destino conocido
- **WHEN** el destino calculado en ejecución es uno de los de la tabla
- **THEN** se llama a la función de ese destino

#### Scenario: Destino desconocido
- **WHEN** el destino calculado no está entre los conocidos
- **THEN** el C llama a `rt_unresolved` con la dirección del sitio y la del destino

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
