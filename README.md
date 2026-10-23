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
- [x] Function and basic-block graph (ALttP: 2,488 functions, 16,030 blocks, 1,485 tail calls, 0 missing targets, 440 shared blocks reported)
- [x] C code generator, first slice: all 256 opcodes, calls, tail calls and indirect dispatch against a fixed runtime interface (`runtime/snes_rt.h`). ALttP's reset plus its callees compile and run to the NMI wait loop; M/X widths are now tracked across calls, returns and `PHP`/`PLP`: 232 addresses (down from 538) are still reached with different widths and make any function containing them fail to generate (95 of the 1,326 directly called functions, down from 126). `discover --widths` lists them. ALttP now discovers 82,025 instructions in 15,989 blocks and 2,488 functions. Whole-ROM emission is not done yet
- [ ] Runtime: bus, DMA/HDMA, PPU, APU, input

## Usage

```
cargo run --release -- info roms/game.sfc
cargo run --release -- disasm roms/game.sfc 00:8000 40
cargo run --release -- discover roms/game.sfc
cargo run --release -- graph roms/game.sfc            # totals and diagnostics
cargo run --release -- graph roms/game.sfc 00:8000    # blocks of the function containing the address
cargo run --release -- codegen roms/game.sfc 00:8000 --depth 1 -o out/reset.c   # C for a function and its callees
cargo test
```

Put your ROM in `roms/` (git-ignored).

The C written by `codegen` is derived from the ROM, so it goes under `out/` (git-ignored) and must not be committed or distributed. Tests that compile and run the generated C look for `CC`, `gcc`, `clang`, `cc` or Visual Studio (through `vcvars64.bat`) and skip themselves when none is found; the test against the real game also needs `roms/alttp_usa.sfc`.
