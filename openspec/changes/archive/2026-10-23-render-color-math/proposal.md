# Proposal

## Why

La pantalla de título de ALttP (fotograma ~1054) usa color math (`$2130`=`02`, `$2131`=`22`, `$2132` cambiando cada fotograma), que `--screenshot` ignora: la captura sale sin la mezcla que el juego pretende. El diagnóstico del cambio anterior mostró además que las capturas negras posteriores no son culpa del renderizador (el juego mantiene el forced blank) y que ni el HDMA ni las ventanas se usan al final de la ejecución, así que quedan fuera.

## What Changes

- El renderizador aplica el color math de `$2130-$2132`: suma o resta del color fijo (`$2132`) o de la subpantalla (`$212D`), con la mitad opcional, sobre las capas elegidas en `$2131`.
- Las regiones de `$2130` (bits 4-5) que dependen de ventanas siguen sin aplicarse y se avisa de ello; 0 (siempre) y 3 (nunca) sí.
- El aviso «colour math is not applied» desaparece; los de ventanas y mosaico se mantienen.

## Capabilities

### New Capabilities

### Modified Capabilities
- `ppu-render`: nuevo requisito de color math y el requisito «Límites declarados» deja de avisar de él.

## Impact

- `runtime/headless.c`: `screenshot` y `pixel_index`.
- `src/headless.rs`: tests de color math y del aviso.
- `CLAUDE.md`: descripción de `--screenshot`.
- Fuera de alcance, para otros cambios: HDMA por línea, ventanas y la causa del forced blank tras el fotograma ~1054.
