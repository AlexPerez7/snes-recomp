## 1. Punto de reanudación

- [x] 1.1 Localizar en la ROM de ALttP el bucle principal (espera de NMI) y comprobar a mano que la pila del 65816 está en su base y que no hay una interrupción en curso
- [x] 1.2 `[resume]` en `src/config.rs` (inicio de un bloque) y que el generador emita `rt_resume(c)` tras `rt_tick`; error claro si la dirección no es el inicio de un bloque
- [ ] 1.3 Documentar la sección en `game-profiles` y en el README

## 2. Estado

- [ ] 2.1 Inventario de todo el estado mutable de `snes_core.inc`, `snes_ppu.inc` y `snes_spc.inc` y lista de lo que no se guarda (con motivo)
- [ ] 2.2 Funciones `state_save(buf)` / `state_load(buf)` con cabecera (formato, motor, CRC32 de la ROM) y rechazo de estados ajenos
- [ ] 2.3 `rt_resume`: atender petición de guardado o carga, comprobando `S`, contexto principal y sin NMI/IRQ en curso

## 3. Frontal

- [ ] 3.1 Teclas de guardar y cargar, ranuras `<rom>.sNN.sst` junto a la ROM, aviso en pantalla
- [ ] 3.2 Rebobinado: anillo de estados con diferencias y tecla de retroceso
- [ ] 3.3 Menú: filas y teclas en Controls y en Game; sin `[resume]` se indica que no está disponible
- [ ] 3.4 `.sst` en `.gitignore`, en `scan` y en la comprobación de `release.ps1`

## 4. Pruebas

- [ ] 4.1 Determinismo: guardar, N fotogramas, cargar, N fotogramas; mismos hashes de imagen y audio
- [ ] 4.2 Estado de otra ROM o de otra versión: rechazado sin tocar el juego
- [ ] 4.3 Rebobinar K pasos vuelve al hash esperado
- [ ] 4.4 `cargo test` completo sin regresiones y fuzz de ALttP con guardar/cargar aleatorio

## 5. Cierre

- [ ] 5.1 Probar a mano en ALttP (mazmorra, overworld, con música y en un menú)
- [ ] 5.2 Declarar `[resume]` en `alttp-recomp` y actualizar su `CLAUDE.md`
- [ ] 5.3 Archivar y actualizar la memoria
