## ADDED Requirements

### Requirement: Superposición de bytes sobre la ROM en memoria
`snes_rt.h` SHALL ofrecer `rom_overlay(uint32_t addr, const uint8_t *data, size_t n)` para que el C del juego sustituya bytes de la copia de la ROM que lee el código recompilado. `addr` es una dirección LoROM `BBAAAA` y MUST seguir la misma correspondencia que las lecturas del bus. Tras la llamada, toda lectura de esas direcciones SHALL devolver los bytes superpuestos. El fichero de la ROM, su CRC32 y el código generado MUST NOT cambiar.

#### Scenario: Lectura superpuesta
- **WHEN** el C del juego llama `rom_overlay(0x0E8000, glyph, 16)` y el código recompilado lee `0E:8004`
- **THEN** obtiene `glyph[4]`

#### Scenario: La ROM del disco no cambia
- **WHEN** se ejecuta con una superposición activa
- **THEN** el fichero de la ROM conserva su CRC32 y el `scan` de lo construido no encuentra bytes de la ROM

#### Scenario: Fuera de la ROM
- **WHEN** `addr + n` sobrepasa el tamaño de la ROM
- **THEN** no se escribe nada y la llamada devuelve error (valor distinto de 0)

#### Scenario: Superposiciones que se pisan
- **WHEN** dos llamadas cubren el mismo byte
- **THEN** vale la última

### Requirement: Sin efecto si no se usa
Un juego que no llama a `rom_overlay` MUST ejecutarse exactamente como antes de este cambio, con el mismo coste de lectura de la ROM.

#### Scenario: Sin llamadas
- **WHEN** el C del juego no llama a `rom_overlay`
- **THEN** el informe y las capturas del runtime sin vídeo coinciden con los de antes
