# snes-recomp

A static recompiler for SNES (65816) ROMs, written in Rust from scratch.
First target: *The Legend of Zelda: A Link to the Past* (USA, CRC32 `777AAC2F`).

You supply your own legally obtained ROM. No ROM data or assets are included or distributed.

License: not chosen yet (add a `LICENSE` before publishing).

## Status

- [x] ROM loader (LoROM, copier header stripping, CRC32, header and vectors)
- [x] 65816 opcode table and decoder with M/X width tracking
- [x] Linear disassembler
- [x] Recursive-descent code discovery (direct jumps, branches, calls)
- [x] Indirect jump / jump-table resolution (per-game config; ALttP: 0 unresolved, 82,108 instructions reachable from 16,577)
- [ ] Function and basic-block graph
- [ ] C code generator
- [ ] Runtime: bus, DMA/HDMA, PPU, APU, input

## Usage

```
cargo run --release -- info roms/game.sfc
cargo run --release -- disasm roms/game.sfc 00:8000 40
cargo run --release -- discover roms/game.sfc
cargo test
```

Put your ROM in `roms/` (git-ignored).
