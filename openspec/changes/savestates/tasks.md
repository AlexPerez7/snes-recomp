## 1. Punto de reanudación

- [x] 1.1 Localizar en la ROM de ALttP el bucle principal (espera de NMI) y comprobar a mano que la pila del 65816 está en su base y que no hay una interrupción en curso
- [x] 1.2 `[resume]` en `src/config.rs` (inicio de un bloque) y que el generador emita `rt_resume(c)` tras `rt_tick`; error claro si la dirección no es el inicio de un bloque
- [ ] 1.3 Documentar la sección en `game-profiles` y en el README

## 2. Estado

- [x] 2.1 Inventario de todo el estado mutable de `snes_core.inc`, `snes_ppu.inc` y `snes_spc.inc` y lista de lo que no se guarda (con motivo)
- [x] 2.2 `runtime/snes_state.inc`: una sola lista de campos (`state_io`) para contar, guardar y cargar; cabecera con formato y CRC32 de la ROM; rechazo de estados ajenos o de otra pila
- [x] 2.3 `rt_resume`: atender petición de guardado o carga; si no es un punto seguro la petición espera al siguiente paso
- [x] 2.4 `--state-save FILE@N` y `--state-load FILE@N` en el frontal sin ventana
- [x] 4.1 Determinismo con ALttP en juego (guardar en 3500, cargar en 3600 con otras entradas): WRAM, VRAM, CGRAM, OAM, registros de la PPU e imagen idénticos; sin cargar, la WRAM difiere (falta convertirlo en prueba automática y comparar el audio)

## 3. Frontal

- [x] 3.1 Teclas de guardar y cargar (F5/F7, ranura con F3/F4), ranuras `<rom>.sN.sst` junto a la ROM, aviso en pantalla
- [x] 3.2 Rebobinado: anillo de 150 estados con diferencias XOR y tecla Retroceso
- [x] 3.3 Sin `[resume]` el aviso dice que el juego no admite estados; las filas del menú quedan fuera de este cambio (teclas fijas)
- [x] 3.4 `.sst` en `.gitignore`, en `scan` y en la comprobación de `release.ps1`

## 4. Pruebas

- [ ] 4.1 Determinismo: guardar, N fotogramas, cargar, N fotogramas; mismos hashes de imagen y audio
- [ ] 4.2 Estado de otra ROM o de otra versión: rechazado sin tocar el juego (la comprobación existe en `state_do_load`; falta la prueba)
- [x] 4.3 Rebobinado: cada diferencia del anillo se comprueba deshaciéndola (prueba `a_state_saved_in_the_main_loop...`); falta una prueba de que K pasos vuelven al hash esperado
- [ ] 4.4 `cargo test` completo sin regresiones y fuzz de ALttP con guardar/cargar aleatorio

## 5. Cierre

- [ ] 5.1 Probar a mano en ALttP (mazmorra, overworld, con música y en un menú)
- [ ] 5.2 Declarar `[resume]` en `alttp-recomp` y actualizar su `CLAUDE.md`
- [ ] 5.3 Archivar y actualizar la memoria
