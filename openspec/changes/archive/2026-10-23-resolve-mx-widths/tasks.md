## 1. Contextos y resúmenes de retorno

- [x] 1.1 En `src/discover.rs`, añadir el contexto `(entrada, Flags)` al trabajo pendiente y a `seen`, y abrir uno nuevo en cada `JSR`/`JSL` y en cada entrada de la configuración
- [x] 1.2 Registrar en `RTS`/`RTL` los anchos de retorno del contexto y suscribir las continuaciones de las llamadas, disparándolas hasta alcanzar el punto fijo
- [x] 1.3 Atribuir al contexto actual los retornos alcanzados por `JMP`/`JML` y por saltos de despacho
- [x] 1.4 Resolver tras el punto fijo las llamadas sin retorno con los anchos previos, respetando `no_return`, y repetir hasta que no cambie nada
- [x] 1.5 Tests unitarios: llamado que cambia anchos, llamado con dos anchos de entrada, retornos con anchos distintos, llamado que no retorna, `no_return`, llamada indirecta con tabla, salto de despacho y `inline_helper`

## 2. Pila de PHP/PLP

- [x] 2.1 Añadir la pila abstracta (`Flags` | `Other`, máximo 16) al estado del camino y a la clave de `seen`
- [x] 2.2 Implementar `PHP`/`PLP` y el apilado de `Other` en el resto de instrucciones de pila, sin que las llamadas toquen la pila del llamador
- [x] 2.3 Tests unitarios: par simple, pares anidados, `PHA`/`PLA` entre ambos, `PLP` sin `PHP` y `assume` tras un `PLP` sin emparejar

## 3. Informe de anchos

- [x] 3.1 Añadir a `Discovery` los campos `calls_without_return` y `untracked_plp`
- [x] 3.2 Añadir `discover --widths` en `src/main.rs`, con conflictos, `PLP` sin emparejar y llamadas sin retorno ordenados por dirección, y resumen de totales
- [x] 3.3 Actualizar el resumen de `graph` para que siga mostrando los conflictos de anchos

## 4. Medición con ALttP

- [x] 4.1 Guardar antes de empezar la salida de `discover` actual como línea base fuera del repositorio
- [x] 4.2 Comparar conjuntos de instrucciones antes y después y revisar las que desaparecen (código fantasma) y las que aparecen
- [x] 4.3 Repetir el barrido de las 1.326 entradas con `codegen` a profundidad 0 y clasificar los conflictos que queden
- [x] 4.4 Comprobar a mano con `disasm` `00:893D` (`LDA #$80` con M=8), `01:D815` y tres pares `PHP`/`PLP`
- [x] 4.5 Comprobar que el test `alttp_reset_runs_to_the_nmi_wait_loop` sigue pasando e intentar la profundidad 3
- [x] 4.6 Anotar el tiempo de `discover` y la profundidad máxima de pila observada, y decidir si el límite de 16 basta

## 5. Documentación

- [x] 5.1 Actualizar README y CLAUDE.md con `discover --widths` y las cifras nuevas de conflictos, instrucciones, bloques y funciones
- [x] 5.2 Ejecutar `cargo test` completo y `openspec validate resolve-mx-widths --type change --strict`
