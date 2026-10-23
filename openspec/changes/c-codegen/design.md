# Design

## Context

El grafo (`graph::Graph`) da funciones, bloques, sucesores (`Fall`, `Branch`, `Jump`, `Dispatch`), llamadas y tail calls. Ver `proposal.md` para el alcance acordado: rebanada vertical y cabecera de runtime propia.

Tres hechos del estado actual condicionan el diseño:
- `Discovery.instrs` guarda una sola decodificación por dirección y **no guarda los anchos M/X** con los que se decodificó; el generador los necesita para cada instrucción.
- El seguimiento de M/X solo entiende `REP`/`SEP`; `PLP`, `RTI` y `XCE` no se siguen, así que tras ellos los anchos supuestos pueden ser falsos.
- En este equipo no hay compilador de C en el `PATH` (hay Visual Studio 2022 instalado, sin entorno cargado). La verificación no puede depender de uno.

## Goals / Non-Goals

**Goals:**
- C correcto antes que rápido: banderas calculadas en cada instrucción, sin optimizar.
- Emisor dirigido por tabla (mnemónico y modo de direccionamiento), con pruebas de texto por familia de opcodes.
- Verificar el C generado ejecutándolo contra un bus de mentira cuando haya compilador.

**Non-Goals:**
- Runtime real, interrupciones, emisión de la ROM completa, banderas perezosas, registros en locales.
- Reproducir trucos con direcciones de retorno en la pila (`PEA` + `RTS` como salto, retornos manipulados): el retorno extrae la dirección y vuelve a quien llamó.

## Decisions

1. **Guardar los anchos en `Discovery`.** Se añade `flags: BTreeMap<u32, Flags>` con los anchos vigentes *antes* de cada instrucción, y `flag_conflicts: BTreeMap<u32, BTreeSet<Flags>>` para las direcciones alcanzadas con más de una combinación. Alternativa: decodificar de nuevo en el generador; se descarta porque duplicaría el seguimiento de M/X. Alternativa: clave `(dirección, anchos)` en `instrs`; se descarta por romper el grafo, que indexa por dirección.
2. **Estado de CPU explícito.** `Cpu { uint16_t a, x, y, s, d; uint8_t db, pb; uint8_t c, z, v, n, dec, irq, e; uint8_t m, x8; }`, con las banderas en bytes separados (se reconstruye P al empujarlo). Alternativa: P empaquetado con máscaras; se descarta por ilegible y propenso a errores en esta primera versión.
3. **M/X estáticos, con comprobación.** El C usa el ancho conocido por instrucción; `REP`/`SEP`/`XCE`/`PLP`/`RTI` además actualizan `c->m`/`c->x8` en ejecución. Con `SNES_RT_CHECKS`, cada bloque verifica al entrar que coinciden con lo supuesto. Así un fallo del seguimiento de M/X tras `PLP` se ve en ejecución en vez de corromper datos en silencio.
4. **Aritmética en `static inline` de `snes_rt.h`.** `rt_adc8/16`, `rt_sbc8/16` (con modo decimal), comparaciones y desplazamientos viven en la cabecera; el generador solo emite llamadas. Reduce el texto generado y concentra la lógica delicada donde se prueba una vez.
5. **Bus de 24 bits con acceso por tamaño.** `bus_read8/16/24` y `bus_write8/16` reciben la dirección completa; los accesos de 16 bits se definen en la cabecera sobre `bus_read8` para respetar el comportamiento de página directa y bancos. El runtime real podrá sustituirlos por versiones más rápidas.
6. **Pila emulada fiel.** `JSR`/`JSL` empujan la dirección de retorno real en `S` y llaman a la función C; `RTS`/`RTL` extraen y vuelven. Mantiene el valor de `S` exacto para el código que lo lee o ajusta, a cambio de no soportar retornos manipulados (no objetivo).
7. **Despacho con `switch`.** El destino se calcula como la CPU (`JMP (abs,X)`, `JML [dp]`, `JSR (abs,X)`…) y se despacha sobre los destinos de las aristas `Dispatch` del sitio; `default` llama a `rt_unresolved`. Para saltos sin retorno el caso termina con `return`.
8. **`codegen` por entrada con profundidad.** Emite la función pedida y sus llamadas hasta `--depth`; el resto se declara `extern`. Permite validar unas pocas funciones reales sin exigir que compile toda la ROM. Los anchos en conflicto dentro de lo emitido son un error.
9. **Cabecera embebida.** `runtime/snes_rt.h` se incluye con `include_str!` y `codegen` la escribe junto al `.c` cuando se usa `-o`; así el binario no depende de ficheros externos.

10. **Los bloques compartidos se emiten en cada función que los alcanza.** El grafo asigna cada bloque a una sola función, pero un `goto` no puede entrar en otra función de C; el emisor recorre por su cuenta los bloques alcanzables desde la entrada (sin cruzar otras entradas) y duplica el código compartido. Alternativa: convertir la cola compartida en función aparte; se descarta porque cambiaría el grafo y la semántica de pila.
11. **Helpers en línea.** El sitio de la llamada llama al helper y retorna, sin caer en la tabla. Los destinos de esa tabla son entradas de función, pero el salto interno del helper (`exit`) no conoce el conjunto de destinos de todos sus llamadores y acaba en `rt_unresolved`. Resolverlo bien queda para un cambio posterior.
12. **Pruebas que ejecutan el C.** `src/cctest.rs` busca `CC`, `gcc`, `clang`, `cc` o Visual Studio (con `vswhere` y `vcvars64.bat`) y compila con avisos activados y `SNES_RT_CHECKS`. Un test contra la ROM real recompila el reset de ALttP con profundidad 2 y lo ejecuta con un bus LoROM y un APU de mentira hasta el bucle de espera del NMI.

## Risks / Trade-offs

- [Anchos mal seguidos tras `PLP`/`RTI`/`XCE`] → `SNES_RT_CHECKS` los detecta en ejecución; se anotan en la verificación sobre ALttP cuántas direcciones tienen conflicto.
- [Helpers en línea sin destinos en su propio salto] → Documentado en la decisión 11; el reset de ALttP no los ejecuta antes del bucle de espera.
- [Sin compilador local, el C podría no compilar] → Las pruebas de texto se ejecutan siempre; las de compilación y ejecución buscan `CC`, `cc`, `gcc`, `clang` o `cl` y, si no hay, se omiten avisando. Antes de dar por buena la verificación se compila una vez con Visual Studio (entorno x64 cargado) o se deja anotado que no se pudo.
- [Rendimiento: banderas y pila exactas en cada instrucción] → Aceptado; la optimización es un cambio posterior.
- [Retornos manipulados en ALttP] → Fuera de alcance; el generador no los detecta y cualquier caso aparecerá al ejecutar.
- [El C generado deriva del ROM] → Va a `out/` (ignorado por Git) y la documentación lo recuerda.
