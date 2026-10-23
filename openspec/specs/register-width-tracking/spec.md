# register-width-tracking Specification

## Purpose
Seguir los anchos del acumulador y de los índices (M/X) a través de llamadas, retornos y `PHP`/`PLP`, para decodificar cada instrucción con los anchos con los que realmente se ejecuta e informar de lo que no se puede determinar.

## Requirements

### Requirement: Anchos tras una llamada
El sistema SHALL continuar el análisis tras una llamada con los anchos con los que vuelve el código llamado, no con los que tenía el llamador antes de llamar.

#### Scenario: El llamado cambia los anchos
- **WHEN** una rutina termina con `SEP #$20` seguido de `RTS`, y se la llama con M=16
- **THEN** la instrucción siguiente a la llamada se decodifica con M=8

#### Scenario: El llamado los conserva
- **WHEN** una rutina vuelve con los mismos anchos con los que entró
- **THEN** el llamador continúa con los anchos que tenía antes de la llamada

#### Scenario: Retornos con anchos distintos
- **WHEN** una rutina tiene dos retornos que dejan anchos distintos
- **THEN** la instrucción siguiente a la llamada se analiza con cada combinación y, si difieren, se informa como conflicto

### Requirement: Contexto de cada función
El sistema SHALL analizar el código de una función por separado para cada combinación de anchos de entrada, y MUST atribuir a la función llamadora los retornos de los saltos de despacho y de los saltos a otra función.

#### Scenario: Dos llamadores con anchos distintos
- **WHEN** una rutina se llama una vez con M=8 y otra con M=16
- **THEN** cada llamada recibe los anchos de retorno de su propio contexto

#### Scenario: Salto de despacho
- **WHEN** una tabla de saltos lleva a un manejador que termina en `RTL`
- **THEN** ese retorno cuenta como retorno de la función que ejecutó el salto

### Requirement: Llamadas sin retorno
El sistema SHALL continuar tras una llamada cuyo llamado no llega a retornar en el análisis con los anchos previos a la llamada, y MUST informar de esa llamada, salvo que esté declarada sin retorno en la configuración.

#### Scenario: Llamado que no retorna
- **WHEN** el llamado termina en un bucle infinito
- **THEN** el código tras la llamada se sigue descubriendo con los anchos previos y la llamada aparece en el informe

#### Scenario: Llamado declarado sin retorno
- **WHEN** la configuración declara `no_return` para el llamado
- **THEN** no se continúa tras la llamada y no se informa de ella

### Requirement: Pila de anchos de PHP y PLP
El sistema SHALL emparejar `PLP` con el `PHP` correspondiente del mismo camino y restaurar los anchos guardados, ignorando las demás instrucciones de pila que haya entre ambos.

#### Scenario: Par PHP y PLP
- **WHEN** una rutina hace `PHP`, cambia los anchos con `REP` y después hace `PLP`
- **THEN** la instrucción siguiente al `PLP` se decodifica con los anchos de antes del `PHP`

#### Scenario: Otras instrucciones de pila entre ambos
- **WHEN** entre `PHP` y `PLP` hay un `PHA` y su `PLA`
- **THEN** el `PLP` sigue emparejándose con el `PHP`

#### Scenario: PLP sin PHP
- **WHEN** un `PLP` no tiene `PHP` en su camino
- **THEN** los anchos no cambian y el `PLP` aparece en el informe

### Requirement: Anchos forzados por configuración
El sistema SHALL dar prioridad a `[[assume]]` sobre cualquier anchos calculado en esa dirección.

#### Scenario: Corrección manual
- **WHEN** hay un `[[assume]]` en la dirección posterior a un `PLP` sin emparejar
- **THEN** esa dirección se decodifica con los anchos declarados

### Requirement: Informe de anchos
El sistema SHALL listar, con `discover --widths`, las direcciones alcanzadas con anchos distintos junto con los anchos vistos, los `PLP` sin emparejar y las llamadas sin retorno, ordenados por dirección.

#### Scenario: Conflicto
- **WHEN** una dirección se alcanza con M=8 y con M=16
- **THEN** el informe la muestra con las dos combinaciones

#### Scenario: Sin incidencias
- **WHEN** no hay conflictos, ni `PLP` sin emparejar, ni llamadas sin retorno
- **THEN** el informe termina con un resumen de ceros
