# snes-recomp

Recompilador estático de ROMs de SNES (CPU 65816) escrito en Rust desde cero. Primer objetivo: *The Legend of Zelda: A Link to the Past* (USA, CRC32 `777AAC2F`).

## Comandos

```
cargo test                                          # tests unitarios
cargo run --release -- info roms/alttp_usa.sfc      # cabecera, CRC y vectores
cargo run --release -- disasm roms/alttp_usa.sfc 00:8000 40
cargo run --release -- discover roms/alttp_usa.sfc  # añade --unresolved para ver saltos indirectos sin tabla
cargo run --release -- graph roms/alttp_usa.sfc [00:8000]  # resumen, o detalle de la función que contiene la dirección
```

## Estructura

- `src/rom.rs`: carga de ROM, mapeo LoROM, cabecera y vectores.
- `src/opcodes.rs`, `src/disasm.rs`: tabla de opcodes y decodificador, con seguimiento de los anchos M/X.
- `src/discover.rs`: descubrimiento de código por descenso recursivo, guiado por la configuración.
- `src/graph.rs`: bloques básicos y funciones a partir del descubrimiento (aristas, tail calls, diagnósticos).
- `src/config.rs`: formato de `games/<id>.toml` (documentado al inicio del fichero).
- `src/games.rs`: identificación de ROMs conocidas por CRC32.
- `games/alttp_usa.toml`: entradas y tablas de saltos de A Link to the Past.

## Reglas

- El código es propio (clean-room): no copiar código de otros recompiladores.
- El ROM lo aporta el usuario. Nunca se sube ni se distribuye: `roms/` y `*.sfc`/`*.smc` están en `.gitignore`.
- Las tablas de saltos que se añadan a `games/*.toml` se comprueban a mano contra el ROM real.
- Los cambios se hacen con OpenSpec: `/opsx:explore`, `/opsx:propose`, `/opsx:apply`, `/opsx:archive`. Contexto del proyecto en `openspec/config.yaml`.
- Artefactos de OpenSpec y documentación, en español.
