## MODIFIED Requirements

### Requirement: Informe de la ejecución
El runtime SHALL imprimir al terminar un informe con los fotogramas ejecutados, los NMIs entregados, la causa de parada y los registros de hardware accedidos con su número de lecturas y escrituras, y MUST listar aparte los canales de DMA y HDMA activados, con los bytes que transfirió cada canal de DMA.

#### Scenario: Registros accedidos
- **WHEN** termina una ejecución en la que el código escribió 3 veces `$2100`
- **THEN** el informe muestra `$2100` con 3 escrituras

#### Scenario: DMA activado
- **WHEN** el código escribe un bit en `$420B` o `$420C`
- **THEN** el informe lista los canales activados y el número de veces de cada uno

#### Scenario: Bytes transferidos
- **WHEN** el canal 0 transfiere 512 bytes en total
- **THEN** el informe indica 512 bytes para el canal 0
