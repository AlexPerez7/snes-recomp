## MODIFIED Requirements

### Requirement: Fotogramas y NMI
El runtime SHALL dividir la ejecución en fotogramas de un número fijo de bloques ejecutados y, al terminar cada uno, entregar el NMI si el bit 7 de `$4200` está activo, llamando al vector de NMI de la ROM con una pila coherente. Además SHALL entregar la IRQ cuando se habilite en los bits 4-5 de `$4200` y se alcance la línea de `$4209/$420A`, ignorando la posición horizontal.

#### Scenario: NMI habilitado
- **WHEN** el código escribe `81` en `$4200` y sigue ejecutando bloques durante varios fotogramas
- **THEN** el manejador de NMI de la ROM se ejecuta una vez por fotograma y su `RTI` devuelve el control al punto interrumpido con la pila y los registros como estaban

#### Scenario: NMI deshabilitado
- **WHEN** `$4200` tiene el bit 7 a 0 al terminar un fotograma
- **THEN** no se entrega ningún NMI

#### Scenario: Espera de interrupción
- **WHEN** el código ejecuta `WAI`
- **THEN** la ejecución avanza al final del fotograma actual y entrega el NMI si está habilitado

#### Scenario: IRQ
- **WHEN** el código escribe `A1` en `$4200` y `FF` en `$4209` con la bandera I a 0 y sigue ejecutando bloques
- **THEN** el manejador del vector de IRQ de la ROM se ejecuta una vez por fotograma, después del NMI de ese fotograma, y su `RTI` devuelve el control al punto interrumpido con la pila y los registros como estaban

#### Scenario: Lectura de TIMEUP
- **WHEN** ha llegado una IRQ y el código lee `$4211`
- **THEN** el bit 7 vale 1 en esa lectura y 0 en la siguiente

#### Scenario: IRQ enmascarada
- **WHEN** la IRQ se dispara con la bandera I a 1
- **THEN** no se entrega hasta que la bandera pasa a 0, y no se pierde

#### Scenario: IRQ deshabilitada
- **WHEN** los bits 4-5 de `$4200` son 0
- **THEN** no se entrega ninguna IRQ
