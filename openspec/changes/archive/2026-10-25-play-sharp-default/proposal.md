## Why

Con `aspect = free` (y con `scaling = fit`) la imagen se estira a un tamaño que no es múltiplo entero del fotograma, y el filtro por defecto, `nearest`, hace que unos píxeles de juego ocupen más píxeles de pantalla que otros: bordes irregulares y ondulaciones en el movimiento. El filtro `sharp` ya existe y lo evita, pero no es el valor por defecto y su cálculo del factor entero mira solo el ancho, así que con `free` (factores distintos en X e Y) no queda bien en el eje vertical.

## What Changes

- El filtro por defecto pasa de `nearest` a `sharp`. Con escalado entero (factor exacto en ambos ejes) `sharp` MUST dar el mismo resultado que `nearest`, byte por byte, y no gastar el paso intermedio.
- `sharp` calcula el factor entero previo con el mayor de los dos factores (ancho y alto), de modo que con `aspect = free` ningún eje se reduce desde una textura demasiado pequeña.
- Se actualizan la tabla del README, los valores por defecto del `config.ini` generado y los tests que esperan `filter = nearest`.
- Los usuarios con `filter` ya escrito en su `config.ini` no cambian; solo cambia el valor cuando la clave falta.

## Capabilities

### New Capabilities

Ninguna.

### Modified Capabilities

- `display-options`: el requisito «Filtro» cambia el valor por defecto a `sharp` y exige que `sharp` coincida con `nearest` en factores enteros y que use el mayor factor de los dos ejes.

## Impact

- `runtime/play_cfg.h` (valor por defecto), `runtime/play.c` (`present()`: factor previo y atajo en factor entero), `README.md` y los tests de `src/headless.rs` que comprueban el `config.ini` por defecto y `--present-shot`.
- Fuera de alcance: resolución interna mayor de la PPU (solo ganaría el modo 7) y shaders propios.
