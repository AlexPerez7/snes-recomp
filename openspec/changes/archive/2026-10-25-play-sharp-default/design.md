## Context

`present()` en `runtime/play.c` ya implementa `sharp`: amplía la textura con vecino más cercano por `k = ceil(dst.w / pw)` (tope 8) hacia una textura de destino y la reduce con bilinear. El factor solo mira el ancho; con `aspect = free` el factor vertical (`dst.h / lines`) puede ser mayor, y entonces la reducción vertical parte de menos muestras de las necesarias. El valor por defecto de `filter` está en `opt_defs` (`runtime/play_cfg.h`).

## Goals / Non-Goals

**Goals:**
- Que quien juega en `free` o `fit` vea píxeles de tamaño uniforme sin tocar la configuración.
- Que `sharp` no cueste nada ni cambie un solo byte cuando el factor ya es entero.

**Non-Goals:**
- Render interno a mayor resolución o shaders; el modo 7 es lo único que ganaría y es un cambio grande.
- Cambiar configuraciones ya guardadas.

## Decisions

- **Factor previo = `ceil(max(dst.w / pw, dst.h / lines))`**, entre 1 y 8. Alternativa: factores separados por eje con una textura `pw*kx` por `lines*ky`; se descarta porque el coste de memoria no compensa y el máximo común basta para que ninguna reducción sea mayor que la de un eje.
- **Atajo en factor entero:** si `dst.w % pw == 0` y `dst.h % lines == 0`, se usa el camino de `nearest` directo (sin textura previa). Garantiza la identidad con `nearest` y ahorra una pasada por fotograma.
- **Valor por defecto vía `opt_defs`:** solo cambia `FILTER_SHARP` como `def`; `settings_defaults` y la escritura del `config.ini` ya lo toman de ahí. Las claves existentes mandan, así que no hay migración.
- Los tests que fijan `filter = nearest` por defecto pasan a esperar `sharp`.

## Risks / Trade-offs

- Un poco más de GPU por fotograma en factores no enteros (una textura previa) → ya es el coste actual de `sharp`; sigue acotado por el tope 8.
- El renderizador por software de `--present-shot` puede diferir ligeramente del de GPU → la comprobación de identidad se hace en factor entero, donde ambos coinciden.
