# Spec Delta

## Purpose
Dividir el código descubierto de una ROM en bloques básicos y funciones con sus aristas de control, como base estructural para el generador de código.

## ADDED Requirements

### Requirement: Bloques básicos
El sistema SHALL dividir las instrucciones descubiertas en bloques básicos: secuencias sin entradas ni salidas intermedias, cuyo último elemento es un salto, un retorno o una caída hacia un líder.

#### Scenario: Salto condicional
- **WHEN** una instrucción es un salto condicional
- **THEN** termina su bloque, el destino y la instrucción siguiente son líderes de bloque, y el bloque tiene ambos como sucesores

#### Scenario: Destino en mitad de un bloque
- **WHEN** un salto apunta a una instrucción que está en mitad de una secuencia lineal
- **THEN** esa secuencia se divide en ese punto y el primer bloque cae al segundo

#### Scenario: Retorno
- **WHEN** un bloque termina en `RTS`, `RTL` o `RTI`
- **THEN** no tiene sucesores y se marca como retorno

### Requirement: Funciones
El sistema SHALL agrupar los bloques en funciones. Son entradas de función los puntos de entrada de la configuración, los destinos de llamadas (`JSR`/`JSL`) y los destinos de tablas de despacho.

#### Scenario: Llamada directa
- **WHEN** el código contiene `JSR $8010`
- **THEN** `00:8010` es la entrada de una función y el bloque llamador cae a la instrucción siguiente, no a la entrada

#### Scenario: Función con varios retornos
- **WHEN** una función alcanza dos instrucciones de retorno por caminos distintos
- **THEN** sus bloques incluyen ambos retornos y no se mezclan con los de otras funciones

#### Scenario: Salto a otra función
- **WHEN** un `JMP` o `JML` apunta a la entrada de otra función
- **THEN** se registra como llamada final (tail call) y los bloques de la otra función no se copian en la primera

### Requirement: Despacho indirecto en el grafo
El sistema SHALL incluir en el grafo los destinos de las tablas de saltos y de los helpers en línea resueltos por la configuración, como sucesores del sitio que los despacha.

#### Scenario: Tabla indirecta
- **WHEN** un `JMP (abs,X)` tiene una tabla configurada de dos destinos
- **THEN** el bloque del salto tiene esos dos destinos como sucesores y cada uno es entrada de función

#### Scenario: Helper en línea
- **WHEN** se llama a un `inline_helper`
- **THEN** los destinos de su tabla son entradas de función y el bloque llamador no cae a los bytes de la tabla

### Requirement: Diagnósticos del grafo
El sistema SHALL informar de las anomalías del grafo sin abortar: sucesores que no son una instrucción descubierta y bloques alcanzados desde más de una función.

#### Scenario: Sucesor no descubierto
- **WHEN** una arista apunta a una dirección sin instrucción descubierta
- **THEN** el diagnóstico indica el bloque origen y la dirección destino, y el resto del grafo se construye

#### Scenario: Bloque compartido
- **WHEN** un bloque es alcanzable por saltos desde dos funciones distintas
- **THEN** se informa y el bloque se asigna a una sola función de forma determinista (la de menor dirección de entrada)

### Requirement: Consulta por línea de comandos
El sistema SHALL ofrecer el subcomando `graph <rom>`, que muestra el resumen de totales y, dada una dirección, el detalle de la función que la contiene. La salida MUST ser determinista y ordenada por dirección.

#### Scenario: Resumen
- **WHEN** se ejecuta `graph` sobre una ROM
- **THEN** se imprimen los totales de funciones, bloques y diagnósticos

#### Scenario: Detalle de función
- **WHEN** se ejecuta `graph <rom> 00:8010` y esa dirección es una entrada de función
- **THEN** se listan sus bloques con rango de direcciones, sucesores y llamadas
