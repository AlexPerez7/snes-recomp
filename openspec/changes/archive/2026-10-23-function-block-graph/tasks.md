# Tasks

## 1. Aristas en el descubrimiento

- [x] 1.1 Añadir `Edge`/`EdgeKind` y el campo `edges` a `Discovery`, registrando ramas, saltos, llamadas, destinos de tablas y de helpers en línea; verificar con tests nuevos en `src/discover.rs` y que los existentes siguen pasando (`cargo test`)
- [x] 1.2 Verificar que `discover` sobre la ROM de ALttP sigue dando 82.108 instrucciones y 0 sin resolver

## 2. Bloques básicos

- [x] 2.1 Crear `src/graph.rs` con la partición en bloques por líderes; verificar con tests de rama condicional, destino en mitad de secuencia y retorno
- [x] 2.2 Cubrir la caída sobre tablas en línea y los sucesores no descubiertos (diagnóstico) con tests

## 3. Funciones

- [x] 3.1 Agrupar bloques en funciones (llamadas, tail calls, varios retornos, tablas como entradas); verificar con los escenarios de la spec como tests
- [x] 3.2 Asignación determinista de bloques compartidos con diagnóstico; verificar con un test de dos funciones que comparten cola

## 4. Línea de comandos y documentación

- [x] 4.1 Añadir el subcomando `graph` (resumen y detalle por dirección) en `src/main.rs` y en `USAGE`; verificar ejecutándolo sobre ALttP
- [x] 4.2 Contrastar a mano con `disasm` el detalle de 3 funciones reales de ALttP (reset, NMI y un manejador de tabla) y registrar los totales en el README
- [x] 4.3 Actualizar README y CLAUDE.md (estructura, comandos y roadmap) y verificar que los comandos documentados funcionan
