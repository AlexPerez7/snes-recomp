## Purpose

Permitir que el código recompilado cambie de contexto de ejecución cuando el original lo hace con un manejador de interrupción que intercambia la pila y termina en `RTI`, algo que el `return` nativo del C generado no puede expresar.

## ADDED Requirements

### Requirement: Contextos con pila nativa propia
El runtime SHALL ejecutar cada contexto del 65816 sobre su propia pila nativa, de modo que un contexto suspendido en mitad de una interrupción conserve intactas sus llamadas nativas pendientes mientras otro corre.

#### Scenario: Contexto único
- **WHEN** el código no cambia nunca de pila en un manejador
- **THEN** solo existe el contexto principal y las interrupciones se atienden sobre él, como hasta ahora

### Requirement: Cambio de contexto en RTI
El runtime SHALL detectar que un `RTI` devuelve a un contexto distinto del interrumpido, cuando el `S` con el que se ejecuta difiere del que el manejador tenía al entrar. En ese caso MUST suspender el contexto actual dentro del `RTI` y reanudar el contexto suspendido cuyo `S` coincide.

#### Scenario: RTI normal
- **WHEN** un manejador termina con el mismo `S` con el que entró
- **THEN** el `RTI` devuelve el control al código interrumpido, sin cambio de contexto

#### Scenario: Intercambio de pila entre NMI e IRQ
- **WHEN** el NMI guarda su `S`, carga el que dejó la IRQ y hace `RTI`, y después la IRQ hace lo contrario
- **THEN** tras el `RTI` del NMI se ejecuta el contexto que la IRQ dejó suspendido, y tras el de la IRQ vuelve el contexto que el NMI dejó suspendido, cada uno en el punto que interrumpió

### Requirement: Contexto nuevo desde una trama fabricada
Si no hay ningún contexto suspendido con el `S` de un `RTI` que cambia de pila, el runtime MUST crear uno nuevo que empiece en la dirección leída de la trama de pila de ese `RTI`, con los anchos que indica el byte de estado de la trama.

#### Scenario: Contexto nuevo
- **WHEN** un manejador carga una pila con una trama de interrupción fabricada por el código y hace `RTI`
- **THEN** empieza a ejecutarse un contexto nuevo en la dirección de esa trama

#### Scenario: Dirección que no es función
- **WHEN** la dirección de la trama no es la entrada de una función del C generado
- **THEN** la ejecución se detiene con la causa y la dirección, como ante cualquier vector que no es función

### Requirement: Estado de la CPU entre contextos
El runtime MUST conservar al cambiar de contexto los registros, la pila emulada y la memoria exactamente como los dejó el código del manejador, sin guardar ni restaurar nada por su cuenta.

#### Scenario: Registros
- **WHEN** el manejador carga registros desde la pila del otro contexto y hace `RTI`
- **THEN** el contexto reanudado ve esos registros
