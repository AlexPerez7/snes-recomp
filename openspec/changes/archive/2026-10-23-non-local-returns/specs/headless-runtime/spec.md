## MODIFIED Requirements

### Requirement: Arranque y parada
El runtime SHALL arrancar la ejecución en el vector de reset leído de la ROM, y MUST detenerse tras el número de fotogramas pedido o en el primer salto indirecto sin resolver, función no soportada, discrepancia de anchos, retorno que ninguna llamada acepta, `STP`, `BRK` o `COP`, con un mensaje que indique la causa y la dirección.

#### Scenario: Fotogramas pedidos
- **WHEN** se ejecuta con `--frames 5` y el código no falla
- **THEN** termina tras 5 fotogramas con código de salida 0 y el informe lo indica

#### Scenario: Fallo del código
- **WHEN** la ejecución llega a `rt_unresolved`, `rt_unsupported` o `rt_flag_mismatch`
- **THEN** termina de inmediato con código de salida distinto de 0 y un mensaje con la causa y las direcciones implicadas

#### Scenario: Retorno perdido
- **WHEN** un `RTS` o `RTL` vuelve a una dirección que ninguna llamada activa espera y el retorno llega al código del runtime
- **THEN** termina con código de salida distinto de 0 y un mensaje con la causa, la dirección sacada de la pila y `S`

#### Scenario: ROM no encontrada
- **WHEN** la ruta de la ROM no existe o no tiene el tamaño de una LoROM válida
- **THEN** termina con error que lo indica antes de ejecutar nada
