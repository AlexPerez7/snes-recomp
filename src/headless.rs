//! Tests of `runtime/headless.c`: small ROMs are recompiled with `generate_all`, built together with the
//! runtime and run. They skip themselves when there is no C compiler.

use crate::codegen::{Emitter, HEADLESS_RUNTIME, PLAY_CFG, PLAY_RUNTIME, PLAY_UI, RUNTIME_CORE, RUNTIME_HEADER, RUNTIME_PPU, RUNTIME_SPC};
use crate::config::Config;
use crate::disasm::Flags;
use crate::discover::discover;
use crate::graph::Graph;

/// A 64 KiB LoROM image with `segments` at their addresses (as `rom::test_rom`).
fn image(segments: &[(u32, &[u8])]) -> Vec<u8> {
    let mut data = vec![0u8; 0x10000];
    for &(addr, bytes) in segments {
        let off = ((addr as usize >> 16) & 0x7F) << 15 | (addr as usize & 0x7FFF);
        data[off..off + bytes.len()].copy_from_slice(bytes);
    }
    data
}

/// Reset vector 00:8000 and native NMI vector 00:9000.
const VECTORS: [(u32, &[u8]); 2] = [(0xFFFC, &[0x00, 0x80]), (0xFFEA, &[0x00, 0x90])];

struct Run {
    code: i32,
    out: String,
}

impl Run {
    /// The report line of a hardware register, e.g. `$2110 reads=0 writes=1 last=AD`.
    fn reg(&self, addr: u32) -> Option<&str> {
        let key = format!("  ${addr:04X} ");
        self.out.lines().find(|l| l.starts_with(&key)).map(|l| l[key.len()..].trim())
    }

    /// The number in a report line such as `irqs: 5`.
    fn num(&self, label: &str) -> Option<u32> {
        let key = format!("{label}: ");
        self.out.lines().find_map(|l| l.strip_prefix(&key)).and_then(|n| n.trim().parse().ok())
    }

    fn has(&self, line: &str) -> bool {
        self.out.lines().any(|l| l == line)
    }
}

/// Recompile `segments` (with `roots` as entries), build them with the runtime and run with `args`.
/// `rom` replaces the image written as `rom.sfc` (`None` leaves the file out).
fn run(name: &str, segments: &[(u32, &[u8])], roots: &[u32], rom: Option<Vec<u8>>, args: &[&str]) -> Option<Run> {
    let cc = crate::cctest::find().or_else(|| {
        eprintln!("skipped {name}: no C compiler found");
        None
    })?;
    let mut all: Vec<(u32, &[u8])> = segments.to_vec();
    all.extend(VECTORS);
    let image_bytes = image(&all);
    let rom_obj = crate::rom::Rom::from_bytes(image_bytes.clone()).unwrap();
    let r: Vec<_> = roots.iter().map(|&a| (a, Flags::RESET)).collect();
    let d = discover(&rom_obj, &Config::default(), &r);
    let g = Graph::build(&d, roots);
    let out = Emitter::new(&d, &g).generate_all();
    let rom = rom.or(Some(image_bytes)).filter(|_| args.first() != Some(&"missing.sfc"));
    Some(build_and_run(&cc, name, &out, rom, args))
}

/// Build the generated C of `out` with the runtime, put `rom` next to it as `rom.sfc` and run it.
fn build_and_run(cc: &crate::cctest::Compiler, name: &str, out: &crate::codegen::FullOutput, rom: Option<Vec<u8>>, args: &[&str]) -> Run {
    let mut files: Vec<(String, Vec<u8>)> = vec![
        ("snes_rt.h".into(), RUNTIME_HEADER.as_bytes().to_vec()),
        ("snes_funcs.h".into(), out.header.clone().into_bytes()),
        ("snes_entry.c".into(), out.entry.clone().into_bytes()),
        ("headless.c".into(), HEADLESS_RUNTIME.as_bytes().to_vec()),
        ("snes_core.inc".into(), RUNTIME_CORE.as_bytes().to_vec()),
        ("snes_ppu.inc".into(), RUNTIME_PPU.as_bytes().to_vec()),
        ("snes_spc.inc".into(), RUNTIME_SPC.as_bytes().to_vec()),
    ];
    files.extend(out.banks.iter().map(|(b, t)| (format!("rom_{b:02X}.c"), t.clone().into_bytes())));
    files.extend(rom.map(|bytes| ("rom.sfc".to_string(), bytes)));
    let files: Vec<(&str, &[u8])> = files.iter().map(|(n, b)| (n.as_str(), b.as_slice())).collect();
    match cc.compile_run_with(name, &files, &["SNES_RT_TICK", "SNES_RT_CHECKS", "SNES_RT_TRACE"], args) {
        Ok(out) => Run { code: 0, out },
        Err(e) => {
            // The error reads "exit Some(N)", a newline, then the output.
            let code = e
                .strip_prefix("exit Some(")
                .and_then(|r| r.split(')').next())
                .and_then(|n| n.parse().ok())
                .unwrap_or_else(|| panic!("the build failed:\n{e}"));
            Run { code, out: e }
        }
    }
}

/// Like `run`, but builds the interactive frontend (`runtime/play.c`) with SDL2 and runs it with `args`
/// (which should include `--no-window`). Skipped, returning `None`, when SDL2 or Visual Studio is missing.
fn run_play(name: &str, segments: &[(u32, &[u8])], roots: &[u32], args: &[&str]) -> Option<Run> {
    run_play_with(name, segments, roots, args, &[])
}

/// Like `run_play`, with `extra` files (name, contents) next to the ROM, e.g. a `rom.keys`.
fn run_play_with(name: &str, segments: &[(u32, &[u8])], roots: &[u32], args: &[&str], extra: &[(&str, &[u8])]) -> Option<Run> {
    run_play_times(name, segments, roots, args, extra, 1)
}

/// Like `run_play_with`, running the program `runs` times in a row in the same folder; the output is all of them.
fn run_play_times(name: &str, segments: &[(u32, &[u8])], roots: &[u32], args: &[&str], extra: &[(&str, &[u8])], runs: usize) -> Option<Run> {
    let cc = crate::cctest::find()?;
    let Some(sdl) = crate::cctest::sdl2_dir() else {
        eprintln!("skipped {name}: no SDL2 development package (set SDL2_DIR or unpack it in out/sdl2)");
        return None;
    };
    let mut all: Vec<(u32, &[u8])> = segments.to_vec();
    all.extend(VECTORS);
    let image_bytes = image(&all);
    let rom_obj = crate::rom::Rom::from_bytes(image_bytes.clone()).unwrap();
    let r: Vec<_> = roots.iter().map(|&a| (a, Flags::RESET)).collect();
    let d = discover(&rom_obj, &Config::default(), &r);
    let g = Graph::build(&d, roots);
    let out = Emitter::new(&d, &g).generate_all();
    let mut files: Vec<(String, Vec<u8>)> = vec![
        ("snes_rt.h".into(), RUNTIME_HEADER.as_bytes().to_vec()),
        ("snes_funcs.h".into(), out.header.clone().into_bytes()),
        ("snes_entry.c".into(), out.entry.clone().into_bytes()),
        ("snes_play.c".into(), PLAY_RUNTIME.as_bytes().to_vec()),
        ("play_cfg.h".into(), PLAY_CFG.as_bytes().to_vec()),
        ("play_ui.h".into(), PLAY_UI.as_bytes().to_vec()),
        ("snes_core.inc".into(), RUNTIME_CORE.as_bytes().to_vec()),
        ("snes_ppu.inc".into(), RUNTIME_PPU.as_bytes().to_vec()),
        ("snes_spc.inc".into(), RUNTIME_SPC.as_bytes().to_vec()),
        ("rom.sfc".into(), image_bytes),
    ];
    files.extend(out.banks.iter().map(|(b, t)| (format!("rom_{b:02X}.c"), t.clone().into_bytes())));
    files.extend(extra.iter().map(|(n, b)| (n.to_string(), b.to_vec())));
    let files: Vec<(&str, &[u8])> = files.iter().map(|(n, b)| (n.as_str(), b.as_slice())).collect();
    Some(match cc.compile_run_sdl_times(name, &files, &["SNES_RT_TICK"], args, Some(&sdl), runs) {
        Ok(out) => Run { code: 0, out },
        Err(e) => {
            let code = e.strip_prefix("exit Some(").and_then(|r| r.split(')').next()).and_then(|n| n.parse().ok()).unwrap_or_else(|| panic!("the build failed:
{e}"));
            Run { code, out: e }
        }
    })
}

/// Builds `segments` with the hooks of `cfg` and the game's C code `game_c` (a `hooks.c` next to the generated files)
/// against the headless runtime, or, with `play`, against the interactive frontend, and runs it with `args`.
fn run_hooked(name: &str, play: bool, segments: &[(u32, &[u8])], roots: &[u32], cfg: &str, game_c: &str, args: &[&str]) -> Option<Run> {
    let cc = crate::cctest::find()?;
    let sdl = if play { Some(crate::cctest::sdl2_dir().or_else(|| { eprintln!("skipped {name}: no SDL2 development package"); None })?) } else { None };
    let mut all: Vec<(u32, &[u8])> = segments.to_vec();
    all.extend(VECTORS);
    let image_bytes = image(&all);
    let rom_obj = crate::rom::Rom::from_bytes(image_bytes.clone()).unwrap();
    let cfg = Config::parse(cfg).unwrap();
    let r: Vec<_> = roots.iter().map(|&a| (a, Flags::RESET)).collect();
    let d = discover(&rom_obj, &cfg, &r);
    let g = Graph::build(&d, roots);
    let out = Emitter::new(&d, &g).with_hooks(&cfg.hooks).unwrap().generate_all();
    let mut files: Vec<(String, Vec<u8>)> = vec![
        ("snes_rt.h".into(), RUNTIME_HEADER.as_bytes().to_vec()),
        ("snes_funcs.h".into(), out.header.clone().into_bytes()),
        ("snes_entry.c".into(), out.entry.clone().into_bytes()),
        ("snes_core.inc".into(), RUNTIME_CORE.as_bytes().to_vec()),
        ("snes_ppu.inc".into(), RUNTIME_PPU.as_bytes().to_vec()),
        ("snes_spc.inc".into(), RUNTIME_SPC.as_bytes().to_vec()),
        ("rom.sfc".into(), image_bytes),
        ("hooks.c".into(), game_c.as_bytes().to_vec()),
    ];
    if play {
        files.push(("snes_play.c".into(), PLAY_RUNTIME.as_bytes().to_vec()));
        files.push(("play_cfg.h".into(), PLAY_CFG.as_bytes().to_vec()));
        files.push(("play_ui.h".into(), PLAY_UI.as_bytes().to_vec()));
    } else {
        files.push(("headless.c".into(), HEADLESS_RUNTIME.as_bytes().to_vec()));
    }
    files.extend(out.banks.iter().map(|(b, t)| (format!("rom_{b:02X}.c"), t.clone().into_bytes())));
    let files: Vec<(&str, &[u8])> = files.iter().map(|(n, b)| (n.as_str(), b.as_slice())).collect();
    Some(match cc.compile_run_sdl_times(name, &files, &["SNES_RT_TICK"], args, sdl.as_deref(), 1) {
        Ok(out) => Run { code: 0, out },
        Err(e) => {
            let code = e.strip_prefix("exit Some(").and_then(|r| r.split(')').next()).and_then(|n| n.parse().ok()).unwrap_or_else(|| panic!("the build failed:\n{e}"));
            Run { code, out: e }
        }
    })
}

/// LDA #$05 ; STA $10 ; JSR $8100 ; BRA to itself    and at 00:8100 an RTS.
const HOOKED_ROM: [(u32, &[u8]); 2] = [(0x8000, &[0xA9, 0x05, 0x85, 0x10, 0x20, 0x00, 0x81, 0x80, 0xFE]), (0x8100, &[0x60])];
const HOOK_CFG: &str = "[[hook]]\naddr=\"00:8100\"\nkind=\"before\"\ncall=\"game_hook\"";
/// Writes into registers the report shows: WRAM $10 plus $20 (through the WRAM helpers) and the buttons of pad 1.
const HOOK_C: &str = "#include \"snes_rt.h\"\nvoid game_hook(Cpu *c) {\n    (void)c;\n    wram_write8(0x11, (uint8_t)(wram_read8(0x10) + 0x20));\n    bus_write8(0x2105, wram_read8(0x11));\n    bus_write8(0x2106, (uint8_t)(joy1_state() >> 8));\n}\n";

#[test]
fn the_c_code_of_a_game_reaches_wram_and_the_pad_through_the_headless_runtime() {
    let args = ["rom.sfc", "--frames", "2", "--frame-blocks", "50", "--press", "START@0-100"];
    let Some(r) = run_hooked("hook-headless", false, &HOOKED_ROM, &[0x8000, 0x8100], HOOK_CFG, HOOK_C, &args) else { return };
    assert_eq!(r.reg(0x2105), Some("reads=0 writes=1 last=25"), "{}", r.out);
    assert_eq!(r.reg(0x2106), Some("reads=0 writes=1 last=10"), "{}", r.out);
}

#[test]
fn the_same_c_code_of_a_game_builds_and_runs_with_the_interactive_frontend() {
    let args = ["rom.sfc", "--no-window", "--frames", "2", "--frame-blocks", "50", "--press", "START@0-100", "--stats"];
    let Some(r) = run_hooked("hook-play", true, &HOOKED_ROM, &[0x8000, 0x8100], HOOK_CFG, HOOK_C, &args) else { return };
    assert_eq!(r.code, 0, "{}", r.out);
}

#[test]
fn bus_maps_rom_wram_and_the_wram_port_like_a_lorom() {
    let code: &[u8] = &[
        0xAD, 0x00, 0x80, 0x8D, 0x10, 0x21, // LDA $8000 -> $2110 (first ROM byte)
        0xAF, 0x00, 0x80, 0x40, 0x8D, 0x11, 0x21, // LDA $408000 -> $2111
        0xAF, 0x00, 0x80, 0x80, 0x8D, 0x12, 0x21, // LDA $808000 -> $2112
        0xA9, 0x5A, 0x8F, 0x20, 0x00, 0x7E, // LDA #$5A ; STA $7E0020
        0xAD, 0x20, 0x00, 0x8D, 0x13, 0x21, // LDA $0020 -> $2113
        0xA9, 0x6B, 0x8D, 0xFF, 0x1F, // LDA #$6B ; STA $1FFF
        0xAF, 0xFF, 0x1F, 0x7E, 0x8D, 0x14, 0x21, // LDA $7E1FFF -> $2114
        0xA9, 0x11, 0x8D, 0x00, 0x80, // LDA #$11 ; STA $8000 (the ROM ignores it)
        0xAD, 0x00, 0x80, 0x8D, 0x15, 0x21, // LDA $8000 -> $2115
        0xA9, 0x00, 0x8D, 0x81, 0x21, // WRAM port address $000100
        0xA9, 0x01, 0x8D, 0x82, 0x21, //
        0xA9, 0x00, 0x8D, 0x83, 0x21, //
        0xA9, 0x77, 0x8D, 0x80, 0x21, 0xA9, 0x88, 0x8D, 0x80, 0x21, // two bytes through $2180
        0xAD, 0x00, 0x01, 0x8D, 0x16, 0x21, // LDA $0100 -> $2116
        0xAD, 0x01, 0x01, 0x8D, 0x17, 0x21, // LDA $0101 -> $2117
        0x80, 0xFE, // loop
    ];
    let Some(r) = run("hl-bus", &[(0x8000, code)], &[0x8000], None, &["rom.sfc", "--frames", "2", "--frame-blocks", "50"]) else { return };
    assert_eq!(r.code, 0, "{}", r.out);
    for (reg, want) in [(0x2110, "AD"), (0x2111, "AD"), (0x2112, "AD"), (0x2113, "5A"), (0x2114, "6B"), (0x2115, "AD"), (0x2116, "77"), (0x2117, "88")] {
        let line = r.reg(reg).unwrap_or_else(|| panic!("${reg:04X} not in the report:\n{}", r.out));
        assert!(line.ends_with(&format!("last={want}")), "${reg:04X}: {line}");
    }
}

#[test]
fn a_missing_or_malformed_rom_stops_before_running_anything() {
    let code: &[u8] = &[0xA9, 0x01, 0x8D, 0x10, 0x21, 0x80, 0xFE];
    let Some(r) = run("hl-norom", &[(0x8000, code)], &[0x8000], None, &["missing.sfc"]) else { return };
    assert_eq!(r.code, 2, "{}", r.out);
    assert!(r.out.contains("error: cannot read missing.sfc") && !r.out.contains("== report =="), "{}", r.out);

    let Some(r) = run("hl-badrom", &[(0x8000, code)], &[0x8000], Some(vec![0u8; 100]), &["rom.sfc"]) else { return };
    assert_eq!(r.code, 2, "{}", r.out);
    assert!(r.out.contains("not the size of a LoROM image") && !r.out.contains("== report =="), "{}", r.out);
}

#[test]
fn hardware_registers_remember_what_is_written_and_count_accesses() {
    let code: &[u8] = &[
        0xA9, 0x33, 0x8D, 0x05, 0x21, // LDA #$33 ; STA $2105
        0xAD, 0x05, 0x21, 0x8D, 0x20, 0x21, // LDA $2105 -> $2120
        0x80, 0xFE,
    ];
    let Some(r) = run("hl-regs", &[(0x8000, code)], &[0x8000], None, &["rom.sfc", "--frames", "1", "--frame-blocks", "20"]) else { return };
    assert_eq!(r.code, 0, "{}", r.out);
    assert_eq!(r.reg(0x2105), Some("reads=1 writes=1 last=33"), "{}", r.out);
    assert_eq!(r.reg(0x2120), Some("reads=0 writes=1 last=33"), "{}", r.out);
}

#[test]
fn nmi_flag_is_read_once_and_vblank_only_ends_the_frame() {
    // Wait for $4210 bit 7 (first read after the frame ends), then read it again.
    let nmi: &[u8] = &[
        0xAD, 0x10, 0x42, 0x10, 0xFB, // LDA $4210 ; BPL $8000
        0x8D, 0x31, 0x21, // STA $2131
        0xAD, 0x10, 0x42, 0x8D, 0x32, 0x21, // LDA $4210 -> $2132
        0x80, 0xFE,
    ];
    let args = ["rom.sfc", "--frames", "3", "--frame-blocks", "50"];
    let Some(r) = run("hl-rdnmi", &[(0x8000, nmi)], &[0x8000], None, &args) else { return };
    assert_eq!(r.code, 0, "{}", r.out);
    assert_eq!(r.reg(0x2131), Some("reads=0 writes=1 last=82"), "{}", r.out);
    assert_eq!(r.reg(0x2132), Some("reads=0 writes=1 last=02"), "{}", r.out);

    // $4212 reads 0 at the start of a frame and bit 7 near its end.
    let vblank: &[u8] = &[
        0xAD, 0x12, 0x42, 0x8D, 0x33, 0x21, // LDA $4212 -> $2133
        0xAD, 0x12, 0x42, 0x10, 0xFB, // LDA $4212 ; BPL
        0x29, 0x80, 0x8D, 0x34, 0x21, // AND #$80 ; STA $2134
        0x80, 0xFE,
    ];
    let Some(r) = run("hl-hvbjoy", &[(0x8000, vblank)], &[0x8000], None, &args) else { return };
    assert_eq!(r.code, 0, "{}", r.out);
    assert_eq!(r.reg(0x2133), Some("reads=0 writes=1 last=00"), "{}", r.out);
    assert_eq!(r.reg(0x2134), Some("reads=0 writes=1 last=80"), "{}", r.out);

    // Bit 6 (horizontal blank) is 0 at the start of a frame and becomes 1 before the frame ends, many times a frame.
    let hblank: &[u8] = &[
        0xAD, 0x12, 0x42, 0x29, 0x40, 0x8D, 0x35, 0x21, // LDA $4212 ; AND #$40 -> $2135
        0xAD, 0x12, 0x42, 0x29, 0x40, 0xF0, 0xF9, // LDA $4212 ; AND #$40 ; BEQ
        0x8D, 0x36, 0x21, // STA $2136
        0x80, 0xFE,
    ];
    let Some(r) = run("hl-hblank", &[(0x8000, hblank)], &[0x8000], None, &args) else { return };
    assert_eq!(r.code, 0, "{}", r.out);
    assert_eq!(r.reg(0x2135), Some("reads=0 writes=1 last=00"), "{}", r.out);
    assert_eq!(r.reg(0x2136), Some("reads=0 writes=1 last=40"), "{}", r.out);
}

/// `LDA $2140 ; CMP #v ; BNE -7`: waits for port 0 of the sound CPU to show `v`.
fn wait_port0(v: u8) -> Vec<u8> {
    vec![0xAD, 0x40, 0x21, 0xC9, v, 0xD0, 0xF9]
}

#[test]
fn the_sound_cpu_answers_the_boot_handshake_takes_an_upload_and_runs_it() {
    // The main CPU waits for $AA/$BB, uploads `MOV $F4,#$42 ; BRA $` to $0300 with the boot ROM protocol, and
    // starts it: port 0 then shows $42.
    let spc_prog = [0x8F, 0x42, 0xF4, 0x2F, 0xFE];
    let mut parts = vec![wait_port0(0xAA), mv(0x2140, 0x2150), mv(0x2141, 0x2151)];
    parts.extend([st(0x00, 0x2142), st(0x03, 0x2143), st(0x01, 0x2141), st(0xCC, 0x2140), wait_port0(0xCC)]);
    for (i, b) in spc_prog.iter().enumerate() {
        parts.extend([st(*b, 0x2141), st(i as u8, 0x2140), wait_port0(i as u8)]);
    }
    parts.extend([st(0x00, 0x2141), st(0x00, 0x2142), st(0x03, 0x2143), st(spc_prog.len() as u8 + 2, 0x2140), wait_port0(0x42), mv(0x2140, 0x2152)]);
    let code = prog(&parts);
    let Some(r) = run("hl-apu", &[(0x8000, code.as_slice())], &[0x8000], None, &["rom.sfc", "--frames", "3", "--frame-blocks", "200"]) else { return };
    assert_eq!(r.code, 0, "{}", r.out);
    for (reg, want) in [(0x2150, "AA"), (0x2151, "BB"), (0x2152, "42")] {
        assert!(r.reg(reg).is_some_and(|l| l.ends_with(&format!("last={want}"))), "${reg:04X}: {:?}
{}", r.reg(reg), r.out);
    }
}

#[test]
fn dma_and_hdma_channels_that_are_started_are_listed() {
    let code: &[u8] = &[
        0xA9, 0x05, 0x8D, 0x0B, 0x42, // channels 0 and 2
        0xA9, 0x01, 0x8D, 0x0B, 0x42, // channel 0 again
        0xA9, 0x80, 0x8D, 0x0C, 0x42, // HDMA channel 7
        0x80, 0xFE,
    ];
    let Some(r) = run("hl-dma", &[(0x8000, code)], &[0x8000], None, &["rom.sfc", "--frames", "1", "--frame-blocks", "20"]) else { return };
    assert_eq!(r.code, 0, "{}", r.out);
    assert!(r.has("dma ch0: 2") && r.has("dma ch2: 1") && r.has("hdma ch7: 1"), "{}", r.out);
    assert!(!r.out.contains("dma ch1:"), "{}", r.out);
}

/// Native mode, NMI enabled (unless `enable` is false), A=$5A kept by the main loop, which also
/// records the stack pointer; the handler counts in $10 and reports it in $2160.
fn nmi_program(enable: bool) -> Vec<u8> {
    let mut v = vec![0x18, 0xFB]; // CLC ; XCE
    v.extend(if enable { [0xA9, 0x81, 0x8D, 0x00, 0x42] } else { [0xEA; 5] });
    v.extend([0xA9, 0x5A]); // LDA #$5A
    v.extend([0x8D, 0x61, 0x21, 0xBA, 0x8E, 0x62, 0x21, 0x80, 0xF7]); // loop: STA $2161 ; TSX ; STX $2162 ; BRA loop
    v
}

const NMI_HANDLER: &[u8] = &[0x48, 0xE6, 0x10, 0xA5, 0x10, 0x8D, 0x60, 0x21, 0x68, 0x40]; // PHA INC LDA STA PLA RTI

#[test]
fn nmi_runs_the_handler_once_per_frame_and_returns_to_the_interrupted_code() {
    let main = nmi_program(true);
    let segs: [(u32, &[u8]); 2] = [(0x8000, &main), (0x9000, NMI_HANDLER)];
    let Some(r) = run("hl-nmi", &segs, &[0x8000, 0x9000], None, &["rom.sfc", "--frames", "5", "--frame-blocks", "100"]) else { return };
    assert_eq!(r.code, 0, "{}", r.out);
    assert!(r.has("stop: frames reached") && r.has("frames: 5") && r.has("nmis: 5"), "{}", r.out);
    assert_eq!(r.reg(0x2160).map(|l| l.split(" last=").nth(1).unwrap().to_string()), Some("05".into()), "{}", r.out);
    assert!(r.reg(0x2160).is_some_and(|l| l.contains("writes=5")), "{}", r.out);
    // A survives every NMI and the stack is back where it was whenever the loop reads it.
    assert!(r.reg(0x2161).is_some_and(|l| l.ends_with("last=5A")), "{}", r.out);
    assert!(r.reg(0x2162).is_some_and(|l| l.ends_with("last=FF")), "{}", r.out);
}

#[test]
fn no_nmi_is_delivered_when_it_is_not_enabled() {
    let main = nmi_program(false);
    let segs: [(u32, &[u8]); 2] = [(0x8000, &main), (0x9000, NMI_HANDLER)];
    let Some(r) = run("hl-nonmi", &segs, &[0x8000, 0x9000], None, &["rom.sfc", "--frames", "4", "--frame-blocks", "100"]) else { return };
    assert_eq!(r.code, 0, "{}", r.out);
    assert!(r.has("frames: 4") && r.has("nmis: 0"), "{}", r.out);
    assert!(r.reg(0x2160).is_none(), "{}", r.out);
}

#[test]
fn wai_jumps_to_the_end_of_the_frame() {
    // CLC ; XCE ; LDA #$81 ; STA $4200 ; loop: WAI ; BRA loop  -- with frames of a million blocks
    let main: &[u8] = &[0x18, 0xFB, 0xA9, 0x81, 0x8D, 0x00, 0x42, 0xCB, 0x80, 0xFD];
    let segs: [(u32, &[u8]); 2] = [(0x8000, main), (0x9000, NMI_HANDLER)];
    let Some(r) = run("hl-wai", &segs, &[0x8000, 0x9000], None, &["rom.sfc", "--frames", "3", "--frame-blocks", "1000000"]) else { return };
    assert_eq!(r.code, 0, "{}", r.out);
    assert!(r.has("frames: 3") && r.has("nmis: 3"), "{}", r.out);
}

/// IRQ handler at 00:A000 (vectors $FFEE and $FFFE): it reads $4211 to acknowledge the IRQ, counts in
/// $11 and reports the count in $2163.
const IRQ_HANDLER: &[u8] = &[0x48, 0xAD, 0x11, 0x42, 0xE6, 0x11, 0xA5, 0x11, 0x8D, 0x63, 0x21, 0x68, 0x40];
const IRQ_VECTORS: [(u32, &[u8]); 2] = [(0xFFEE, &[0x00, 0xA0]), (0xFFFE, &[0x00, 0xA0])];

#[test]
fn a_vertical_irq_comes_once_per_frame_after_the_nmi_and_returns() {
    // NMI and V-IRQ at line 255, interrupts on; the loop keeps A and records the stack pointer
    let main: &[u8] = &[
        0x18, 0xFB, // CLC ; XCE
        0xA9, 0xA1, 0x8D, 0x00, 0x42, // $4200 = $A1
        0xA9, 0xFF, 0x8D, 0x09, 0x42, // $4209 = $FF
        0x58, // CLI
        0xA9, 0x5A, 0x8D, 0x61, 0x21, 0xBA, 0x8E, 0x62, 0x21, 0x80, 0xF7,
    ];
    let segs: [(u32, &[u8]); 5] = [(0x8000, main), (0x9000, NMI_HANDLER), (0xA000, IRQ_HANDLER), IRQ_VECTORS[0], IRQ_VECTORS[1]];
    let Some(r) = run("hl-irq", &segs, &[0x8000, 0x9000, 0xA000], None, &["rom.sfc", "--frames", "5", "--frame-blocks", "100"]) else { return };
    assert_eq!(r.code, 0, "{}", r.out);
    assert_eq!((r.num("nmis"), r.num("irqs")), (Some(5), Some(5)), "{}", r.out);
    assert!(r.reg(0x2163).is_some_and(|l| l.ends_with("last=05")), "{}", r.out);
    assert!(r.reg(0x2161).is_some_and(|l| l.ends_with("last=5A")), "{}", r.out);
    assert!(r.reg(0x2162).is_some_and(|l| l.ends_with("last=FF")), "{}", r.out);
    assert_eq!(r.num("context switches"), Some(0), "{}", r.out);
}

#[test]
fn no_irq_comes_when_it_is_not_enabled() {
    let main = nmi_program(true); // $4200 = $81: NMI only
    let segs: [(u32, &[u8]); 5] = [(0x8000, &main), (0x9000, NMI_HANDLER), (0xA000, IRQ_HANDLER), IRQ_VECTORS[0], IRQ_VECTORS[1]];
    let Some(r) = run("hl-noirq", &segs, &[0x8000, 0x9000, 0xA000], None, &["rom.sfc", "--frames", "4", "--frame-blocks", "100"]) else { return };
    assert_eq!(r.code, 0, "{}", r.out);
    assert_eq!((r.num("nmis"), r.num("irqs")), (Some(4), Some(0)), "{}", r.out);
}

#[test]
fn timeup_is_read_once_and_a_masked_irq_waits_for_the_flag() {
    // Reset state: I = 1. Wait for the IRQ flag in $4211, record it, read again.
    let flag: &[u8] = &[
        0xA9, 0x20, 0x8D, 0x00, 0x42, 0xA9, 0x10, 0x8D, 0x09, 0x42, // V-IRQ at line 16
        0xAD, 0x11, 0x42, 0xF0, 0xFB, // spin until $4211 is not 0
        0x8D, 0x65, 0x21, 0xAD, 0x11, 0x42, 0x8D, 0x66, 0x21, 0x80, 0xFE,
    ];
    let segs: [(u32, &[u8]); 4] = [(0x8000, flag), (0xA000, IRQ_HANDLER), IRQ_VECTORS[0], IRQ_VECTORS[1]];
    let Some(r) = run("hl-timeup", &segs, &[0x8000, 0xA000], None, &["rom.sfc", "--frames", "1", "--frame-blocks", "100"]) else { return };
    assert_eq!(r.code, 0, "{}", r.out);
    assert!(r.reg(0x2165).is_some_and(|l| l.ends_with("last=80")), "{}", r.out);
    assert!(r.reg(0x2166).is_some_and(|l| l.ends_with("last=00")), "{}", r.out);
    assert_eq!(r.num("irqs"), Some(0), "masked by I\n{}", r.out);

    // The same IRQ, held back by I = 1 for a few hundred blocks, arrives as soon as I is cleared.
    // The handler reports $12 on its first run: 0 would mean it came early.
    let late: &[u8] = &[
        0xA9, 0x20, 0x8D, 0x00, 0x42, 0xA9, 0x10, 0x8D, 0x09, 0x42,
        0xA2, 0xFF, 0xCA, 0xD0, 0xFD, // wait about 255 blocks
        0xA9, 0x01, 0x85, 0x12, 0x58, // $12 = 1 ; CLI
        0x80, 0xFE,
    ];
    let first: &[u8] = &[0xAD, 0x11, 0x42, 0xA5, 0x13, 0xD0, 0x07, 0xA5, 0x12, 0x8D, 0x63, 0x21, 0xE6, 0x13, 0x40];
    let segs: [(u32, &[u8]); 4] = [(0x8000, late), (0xA000, first), IRQ_VECTORS[0], IRQ_VECTORS[1]];
    let Some(r) = run("hl-irq-late", &segs, &[0x8000, 0xA000], None, &["rom.sfc", "--frames", "3", "--frame-blocks", "100"]) else { return };
    assert_eq!(r.code, 0, "{}", r.out);
    assert!(r.num("irqs").is_some_and(|n| n >= 1), "{}", r.out);
    assert!(r.reg(0x2163).is_some_and(|l| l.ends_with("writes=1 last=01")), "{}", r.out);
}

/// Two contexts that take turns the way the ALttP intro does: NMI and IRQ share a handler that swaps
/// the stack with the one saved in $20 and returns into it. Context 2 starts from a frame built by
/// hand (at $01D5, with X = $5678 and A = $1234 under it) that points at `pc`.
fn swap_program(pc: u16) -> Vec<Vec<u8>> {
    let w = |v: u16, a: u16| vec![0xA9, v as u8, (v >> 8) as u8, 0x8D, a as u8, (a >> 8) as u8];
    let mut main = vec![0x18, 0xFB, 0xC2, 0x30, 0xA9, 0xD0, 0x01, 0x85, 0x20]; // CLC ; XCE ; REP #$30 ; $20 = $01D0
    main.extend(w(0x5678, 0x01D1)); // X
    main.extend(w(0x1234, 0x01D3)); // A
    main.extend(w(0x0030, 0x01D5)); // P = 8-bit registers, I = 0 ; PC low = 0
    main.extend(w(pc >> 8, 0x01D7)); // PC high ; bank 0
    main.extend([0xE2, 0x30, 0xA9, 0xA1, 0x8D, 0x00, 0x42, 0xA9, 0xFF, 0x8D, 0x09, 0x42, 0x58]);
    main.extend([0xE6, 0x30, 0xA5, 0x30, 0x8D, 0x70, 0x21, 0x80, 0xF7]); // context 1: count in $2170
    // PHA ... : REP #$30 ; PHA ; PHX ; TSC ; TAX ; LDA $20 ; TCS ; STX $20 ; PLX ; PLA ; RTI
    let swap = vec![0xC2, 0x30, 0x48, 0xDA, 0x3B, 0xAA, 0xA5, 0x20, 0x1B, 0x86, 0x20, 0xFA, 0x68, 0x40];
    // Context 2: report A and X, then count in $2171
    let two = vec![0x8D, 0x72, 0x21, 0x8E, 0x73, 0x21, 0xE6, 0x31, 0xA5, 0x31, 0x8D, 0x71, 0x21, 0x80, 0xF7];
    vec![main, swap, two]
}

fn run_swap(name: &str, pc: u16, two: Option<&[u8]>) -> Option<Run> {
    let p = swap_program(pc);
    let two = two.unwrap_or(&p[2]);
    let segs: [(u32, &[u8]); 4] = [(0x8000, &p[0]), (0x9000, &p[1]), (0xA000, two), (0xFFEE, &[0x00, 0x90])];
    run(name, &segs, &[0x8000, 0x9000, 0xA000], None, &["rom.sfc", "--frames", "6", "--frame-blocks", "200"])
}

#[test]
fn nmi_and_irq_swap_stacks_and_the_contexts_take_turns() {
    let Some(r) = run_swap("hl-swap", 0xA000, None) else { return };
    assert_eq!(r.code, 0, "{}", r.out);
    assert!(r.num("nmis").is_some_and(|n| n >= 5) && r.num("irqs").is_some_and(|n| n >= 5), "{}", r.out);
    assert!(r.num("context switches").is_some_and(|n| n >= 8), "{}", r.out);
    // Both contexts ran, and the second saw the registers of the frame the code built
    assert!(r.reg(0x2170).is_some(), "context 1 never ran\n{}", r.out);
    assert!(r.reg(0x2171).is_some(), "context 2 never ran\n{}", r.out);
    assert!(r.reg(0x2172).is_some_and(|l| l.ends_with("last=34")), "A\n{}", r.out);
    assert!(r.reg(0x2173).is_some_and(|l| l.ends_with("last=78")), "X\n{}", r.out);
}

#[test]
fn a_context_cannot_start_where_there_is_no_function_or_return() {
    let Some(r) = run_swap("hl-swap-nofn", 0xB000, None) else { return };
    assert_eq!(r.code, 10, "{}", r.out);
    assert!(r.out.contains("context entry 00B000 is not the entry of a function"), "{}", r.out);

    let Some(r) = run_swap("hl-swap-ret", 0xA000, Some(&[0x60])) else { return };
    // Its RTS goes to an address no call expects.
    assert_eq!(r.code, 14, "{}", r.out);
    assert!(r.out.contains("LOST_RETURN to"), "{}", r.out);
}

#[test]
fn failures_stop_the_run_with_the_cause_and_a_distinct_exit_code() {
    // JMP ($8100) with no table configured
    let Some(r) = run("hl-unresolved", &[(0x8000, &[0x6C, 0x00, 0x81])], &[0x8000], None, &["rom.sfc"]) else { return };
    assert_eq!(r.code, 5, "{}", r.out);
    assert!(r.out.contains("UNRESOLVED site=008000 target=") && r.out.contains("stop: unresolved indirect jump"), "{}", r.out);

    // A reset function that cannot be translated (widths in conflict) is a stub
    let conflict: &[u8] = &[0xF0, 0x02, 0xC2, 0x20, 0xA9, 0x12, 0x00, 0x60];
    let Some(r) = run("hl-unsupported", &[(0x8000, conflict)], &[0x8000], None, &["rom.sfc"]) else { return };
    assert_eq!(r.code, 9, "{}", r.out);
    assert!(r.out.contains("UNSUPPORTED 008000") && r.out.contains("stop: unsupported function"), "{}", r.out);

    // STP
    let Some(r) = run("hl-stp", &[(0x8000, &[0xDB])], &[0x8000], None, &["rom.sfc"]) else { return };
    assert_eq!(r.code, 7, "{}", r.out);
    assert!(r.out.contains("stop: STP"), "{}", r.out);

    // A reset that returns with RTS: nothing accepts that return, and the report says where it went.
    let Some(r) = run("hl-returns", &[(0x8000, &[0x60])], &[0x8000], None, &["rom.sfc"]) else { return };
    assert_eq!(r.code, 14, "{}", r.out);
    assert!(r.out.contains("LOST_RETURN to 0000 with S=0201"), "{}", r.out);
    assert!(r.out.contains("stop: return to an address no call expects"), "{}", r.out);
}

/// A Link to the Past, recompiled whole and run with the real ROM through its boot (the audio upload
/// takes the first 14 frames) and its first NMIs. Where the run stops later is not asserted: it is a
/// finding about the recompiler, listed in the notes of the change. `ALTTP_FRAMES` and
/// `ALTTP_FRAME_BLOCKS` change the run to explore it. Skipped without the ROM or a compiler.
#[test]
fn alttp_boots_and_takes_its_first_nmis() {
    let Some(cc) = crate::cctest::find() else {
        eprintln!("skipped: no C compiler found");
        return;
    };
    let Some(game) = crate::cctest::real_game_id("alttp_usa") else { return };
    let bytes = std::fs::read(&game.rom_path).unwrap();
    let rom = crate::rom::Rom::from_bytes(bytes.clone()).unwrap();
    let cfg = Config::parse(&game.profile.config).unwrap();
    let roots: Vec<(u32, Flags)> = cfg.entries.iter().map(|e| (e.addr, e.flags)).collect();
    let d = discover(&rom, &cfg, &roots);
    let addrs: Vec<u32> = roots.iter().map(|r| r.0).collect();
    let g = Graph::build(&d, &addrs);
    let out = Emitter::new(&d, &g).generate_all();
    let frames = std::env::var("ALTTP_FRAMES").unwrap_or_else(|_| "260".into());
    let blocks = std::env::var("ALTTP_FRAME_BLOCKS").unwrap_or_else(|_| "20000".into());
    let dump = std::env::temp_dir().join(format!("snes-recomp-alttp-dump-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dump);
    std::fs::create_dir_all(&dump).unwrap();
    let dump_arg = dump.to_string_lossy().into_owned();
    let shot_arg = dump.join("shot.png").to_string_lossy().into_owned();
    let wav_arg = dump.join("out.wav").to_string_lossy().into_owned();
    let r = build_and_run(&cc, "alttp-headless", &out, Some(bytes), &["rom.sfc", "--frames", &frames, "--frame-blocks", &blocks, "--dump", &dump_arg, "--screenshot", &shot_arg, "--trace-frames", "0-100000", "--watch", "11", "--wav", &wav_arg]);
    let wav = std::fs::read(dump.join("out.wav")).unwrap_or_default();
    let shot = std::fs::read(dump.join("shot.png")).ok().map(|b| decode_png(&b));
    eprintln!("exit code {}\n{}", r.code, r.out);
    let read = |f: &str| std::fs::read(dump.join(f)).unwrap_or_default();
    let (vram, cgram, oam) = (read("vram.bin"), read("cgram.bin"), read("oam.bin"));
    let _ = std::fs::remove_dir_all(&dump);
    let filled = |b: &[u8]| b.iter().filter(|&&x| x != 0).count();
    // The game has uploaded graphics, a palette and sprites by now.
    assert!(filled(&vram) > 1000 && filled(&cgram) > 8 && filled(&oam) > 16, "vram {} cgram {} oam {}", filled(&vram), filled(&cgram), filled(&oam));
    assert!(!r.out.contains("warning: VRAM address translation"), "{}", r.out);
    // The picture is not a single colour: the game draws text and sprites on the black backdrop.
    let shot = shot.expect("the screenshot was written");
    let distinct: std::collections::HashSet<[u8; 3]> = (0..shot.w * shot.h).map(|i| [shot.px[i * 3], shot.px[i * 3 + 1], shot.px[i * 3 + 2]]).collect();
    assert!(distinct.len() >= 3, "only {} colours in the screenshot", distinct.len());
    assert!(r.out.contains("== report =="), "{}", r.out);
    // The audio: a WAV of 16 bit stereo at 32 kHz, one stereo sample per 1/32000 s of the run, with sound in it
    // (the game uploads its sound program through the real boot ROM and the driver then plays).
    let n_frames: f64 = frames.parse().unwrap();
    let samples = n_frames / 60.0988 * 32000.0;
    assert!(wav.len() > 44 && &wav[0..4] == b"RIFF" && &wav[8..16] == b"WAVEfmt ", "no WAV header ({} bytes)", wav.len());
    assert_eq!(u16::from_le_bytes([wav[22], wav[23]]), 2, "stereo");
    assert_eq!(u32::from_le_bytes([wav[24], wav[25], wav[26], wav[27]]), 32000, "32 kHz");
    assert_eq!(u16::from_le_bytes([wav[34], wav[35]]), 16, "16 bits");
    let data = u32::from_le_bytes([wav[40], wav[41], wav[42], wav[43]]) as usize;
    assert_eq!(data, wav.len() - 44, "the data size of the header");
    assert!(((data / 4) as f64 - samples).abs() < 600.0, "{} samples, expected about {samples}", data / 4);
    assert!(wav[44..].chunks(2).any(|b| b != [0, 0]), "the audio is silent");
    // The reset code forces blank, the audio upload goes through the handshake, and NMI gets enabled.
    assert!(r.reg(0x2100).is_some(), "{}", r.out);
    assert!(r.reg(0x2140).is_some_and(|l| l.contains("reads=") && !l.contains("reads=0 ")), "{}", r.out);
    // The game keeps NMI on, and later adds the auto joypad read and the IRQ bits.
    let nmi_on = r.reg(0x4200).and_then(|l| l.rsplit("last=").next()).and_then(|v| u8::from_str_radix(v, 16).ok()).is_some_and(|v| v & 0x81 == 0x81);
    assert!(nmi_on, "{}", r.out);
    let nmis: u32 = r.out.lines().find_map(|l| l.strip_prefix("nmis: ")).and_then(|n| n.parse().ok()).expect("nmis in the report");
    assert!(nmis >= 10, "only {nmis} NMIs\n{}", r.out);
    assert!(r.reg(0x420B).is_some(), "the NMI handler starts DMA\n{}", r.out);
    // The intro runs on two contexts that NMI and IRQ swap; submodule 3 (the triforce) gives way to 4.
    assert!(r.num("irqs").is_some_and(|n| n > 0) && r.num("context switches").is_some_and(|n| n > 0), "{}", r.out);
    assert!(r.out.contains("wram 00011: 03 -> 04"), "submodule 3 never ends\n{}", r.out);
}

// ---- DMA and PPU memories ----

/// `LDA #v ; STA addr`.
fn st(v: u8, addr: u16) -> Vec<u8> {
    vec![0xA9, v, 0x8D, addr as u8, (addr >> 8) as u8]
}

/// `LDA from ; STA to`.
fn mv(from: u16, to: u16) -> Vec<u8> {
    vec![0xAD, from as u8, (from >> 8) as u8, 0x8D, to as u8, (to >> 8) as u8]
}

/// Channel `ch` set up with control `ctl`, B-bus register `$21<b>`, A address `00:a` and `count`.
fn channel(ch: u16, ctl: u8, b: u8, a: u16, count: u16) -> Vec<u8> {
    let base = 0x4300 + ch * 16;
    [st(ctl, base), st(b, base + 1), st(a as u8, base + 2), st((a >> 8) as u8, base + 3), st(0, base + 4), st(count as u8, base + 5), st((count >> 8) as u8, base + 6)].concat()
}

/// The program `parts` followed by a loop.
fn prog(parts: &[Vec<u8>]) -> Vec<u8> {
    let mut code = parts.concat();
    code.extend([0x80, 0xFE]);
    code
}

#[derive(Default)]
struct Mem {
    vram: Vec<u8>,
    cgram: Vec<u8>,
    oam: Vec<u8>,
    regs: Vec<u8>,
    wram: Vec<u8>,
}

/// Run `code` at 00:8000 (with `data` in the ROM) for one short frame and read the dumped memories.
fn run_dump(name: &str, code: &[u8], data: &[(u32, &[u8])], dump: bool) -> Option<(Run, Mem, std::path::PathBuf)> {
    let dir = std::env::temp_dir().join(format!("snes-recomp-dump-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.to_string_lossy().into_owned();
    let mut args = vec!["rom.sfc", "--frames", "1", "--frame-blocks", "20"];
    if dump {
        args.extend(["--dump", &path]);
    }
    let mut segments = vec![(0x8000, code)];
    segments.extend_from_slice(data);
    let r = run(name, &segments, &[0x8000], None, &args);
    let read = |f: &str| std::fs::read(dir.join(f)).unwrap_or_default();
    let mem = Mem { vram: read("vram.bin"), cgram: read("cgram.bin"), oam: read("oam.bin"), regs: read("ppu_regs.bin"), wram: read("wram.bin") };
    r.map(|r| (r, mem, dir))
}

const DATA: [u8; 16] = [0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1A, 0x1B, 0x1C, 0x1D, 0x1E, 0x1F, 0x20];

#[test]
fn dma_copies_to_vram_cgram_and_the_registers_with_each_pattern() {
    let code = prog(&[
        st(0x80, 0x2115),
        // mode 1 to VRAM word 0: 16 bytes
        channel(0, 0x01, 0x18, 0x8100, 16),
        st(0x01, 0x420B),
        mv(0x4305, 0x2110), // the counter is back to 0
        mv(0x4302, 0x2111), // the A address moved on by 16
        // mode 3 to VRAM word $0100: b0,b1 -> $2118 ; b2,b3 -> $2119
        st(0x00, 0x2116),
        st(0x01, 0x2117),
        channel(0, 0x03, 0x18, 0x8100, 4),
        st(0x01, 0x420B),
        // mode 0 and mode 2 to CGRAM
        st(0x00, 0x2121),
        channel(1, 0x00, 0x22, 0x8100, 4),
        st(0x02, 0x420B),
        channel(1, 0x02, 0x22, 0x8104, 4),
        st(0x02, 0x420B),
        // mode 4 to $2105-$2108
        channel(2, 0x04, 0x05, 0x8108, 4),
        st(0x04, 0x420B),
    ]);
    let Some((r, m, dir)) = run_dump("hl-dma-modes", &code, &[(0x8100, &DATA)], true) else { return };
    let _ = std::fs::remove_dir_all(dir);
    assert_eq!(r.code, 0, "{}", r.out);
    assert_eq!(&m.vram[0..16], &DATA, "mode 1");
    assert_eq!(&m.vram[0x200..0x204], &[0x12, 0x13, 0x00, 0x14], "mode 3");
    assert_eq!(&m.cgram[0..8], &[0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18], "modes 0 and 2");
    for (reg, want) in [(0x2105, "19"), (0x2106, "1A"), (0x2107, "1B"), (0x2108, "1C")] {
        assert!(r.reg(reg).is_some_and(|l| l.ends_with(&format!("last={want}"))), "${reg:04X}\n{}", r.out);
    }
    assert!(r.reg(0x2110).is_some_and(|l| l.ends_with("last=00")), "{}", r.out);
    assert!(r.reg(0x2111).is_some_and(|l| l.ends_with("last=10")), "{}", r.out);
    assert!(r.has("dma ch0: 2") && r.has("dma ch0 bytes: 20"), "{}", r.out);
}

#[test]
fn dma_runs_channels_in_order_and_fills_wram_through_the_port() {
    let code = prog(&[
        st(0x00, 0x2181),
        st(0x01, 0x2182),
        st(0x00, 0x2183), // port at $000100
        channel(0, 0x00, 0x80, 0x8100, 2),
        channel(2, 0x00, 0x80, 0x8104, 2),
        st(0x05, 0x420B), // channels 0 then 2
        mv(0x0100, 0x2110),
        mv(0x0101, 0x2111),
        mv(0x0102, 0x2112),
        mv(0x0103, 0x2113),
        // a zero count moves 65536 bytes (the whole of WRAM bank 7E from $0000)
        st(0x00, 0x2181),
        st(0x00, 0x2182),
        channel(3, 0x00, 0x80, 0x8000, 0),
        st(0x08, 0x420B),
    ]);
    let Some((r, _, dir)) = run_dump("hl-dma-wram", &code, &[(0x8100, &DATA)], false) else { return };
    let _ = std::fs::remove_dir_all(dir);
    assert_eq!(r.code, 0, "{}", r.out);
    for (reg, want) in [(0x2110, "11"), (0x2111, "12"), (0x2112, "15"), (0x2113, "16")] {
        assert!(r.reg(reg).is_some_and(|l| l.ends_with(&format!("last={want}"))), "${reg:04X}\n{}", r.out);
    }
    assert!(r.has("dma ch3 bytes: 65536"), "{}", r.out);
}

#[test]
fn dma_address_modes_directions_and_bank_wrap() {
    let code = prog(&[
        // decrement from $8103: reads $8103, $8102, $8101 and ends at $8100
        channel(0, 0x10, 0x05, 0x8103, 3),
        st(0x01, 0x420B),
        mv(0x4302, 0x2110),
        mv(0x4303, 0x2111),
        // fixed: four reads of $8102
        channel(1, 0x08, 0x06, 0x8102, 4),
        st(0x02, 0x420B),
        // B to A: two VRAM bytes read through $2139/$213A into WRAM $0200
        st(0x80, 0x2115),
        st(0x00, 0x2116),
        st(0x00, 0x2117),
        st(0xC1, 0x2118),
        st(0xC2, 0x2119),
        st(0x00, 0x2116), // back to word 0
        channel(2, 0x81, 0x39, 0x0200, 2),
        st(0x04, 0x420B),
        mv(0x0200, 0x2112),
        mv(0x0201, 0x2113),
        // the A address wraps inside the bank: $FFFF then $0000 (WRAM $0000 holds $77)
        st(0x77, 0x0000),
        channel(3, 0x00, 0x07, 0xFFFF, 2),
        st(0x08, 0x420B),
        mv(0x4332, 0x2114),
        mv(0x4333, 0x2116),
    ]);
    let Some((r, _, dir)) = run_dump("hl-dma-addr", &code, &[(0x8100, &DATA)], false) else { return };
    let _ = std::fs::remove_dir_all(dir);
    assert_eq!(r.code, 0, "{}", r.out);
    let last = |reg: u32, want: &str| assert!(r.reg(reg).is_some_and(|l| l.ends_with(&format!("last={want}"))), "${reg:04X} want {want}\n{}", r.out);
    last(0x2105, "12"); // third byte read going down is $8101
    last(0x2110, "00");
    last(0x2111, "81"); // the address stopped at $8100
    last(0x2106, "13"); // fixed on $8102
    last(0x2112, "C1");
    last(0x2113, "C2");
    last(0x2107, "77"); // $0000 of the same bank
    last(0x2114, "01");
    last(0x2116, "00");
}

#[test]
fn dma_bytes_are_added_per_channel() {
    let code = prog(&[
        channel(1, 0x00, 0x05, 0x8100, 64),
        st(0x02, 0x420B),
        channel(1, 0x00, 0x05, 0x8100, 64),
        st(0x02, 0x420B),
    ]);
    let Some((r, _, dir)) = run_dump("hl-dma-bytes", &code, &[(0x8100, &DATA)], false) else { return };
    let _ = std::fs::remove_dir_all(dir);
    assert!(r.has("dma ch1: 2") && r.has("dma ch1 bytes: 128"), "{}", r.out);
}

#[test]
fn vram_ports_follow_the_increment_mode() {
    let code = prog(&[
        // increment after the high byte: words $1000 and $1001
        st(0x80, 0x2115),
        st(0x00, 0x2116),
        st(0x10, 0x2117),
        st(0xA1, 0x2118),
        st(0xA2, 0x2119),
        st(0xB1, 0x2118),
        st(0xB2, 0x2119),
        // increment after the low byte: the high bytes stay as they were
        st(0x00, 0x2115),
        st(0x00, 0x2116),
        st(0x00, 0x2117),
        st(0xC1, 0x2118),
        st(0xC2, 0x2118),
        // step of 32 words
        st(0x81, 0x2115),
        st(0x00, 0x2116),
        st(0x20, 0x2117),
        st(0xD1, 0x2118),
        st(0xD2, 0x2119),
        st(0xE1, 0x2118),
        st(0xE2, 0x2119),
    ]);
    let Some((r, m, dir)) = run_dump("hl-vram", &code, &[], true) else { return };
    let _ = std::fs::remove_dir_all(dir);
    assert_eq!(r.code, 0, "{}", r.out);
    assert_eq!(&m.vram[0x2000..0x2004], &[0xA1, 0xA2, 0xB1, 0xB2]);
    assert_eq!(&m.vram[0..4], &[0xC1, 0x00, 0xC2, 0x00]);
    assert_eq!(&m.vram[0x4000..0x4002], &[0xD1, 0xD2]);
    assert_eq!(&m.vram[0x4040..0x4042], &[0xE1, 0xE2]);
}

#[test]
fn vram_reads_come_from_the_prefetch() {
    let code = prog(&[
        st(0x80, 0x2115),
        st(0x00, 0x2116),
        st(0x00, 0x2117),
        st(0x11, 0x2118),
        st(0x22, 0x2119),
        st(0x33, 0x2118),
        st(0x44, 0x2119),
        st(0x00, 0x2116),
        st(0x00, 0x2117), // prefetches word 0
        mv(0x2139, 0x2110),
        mv(0x213A, 0x2111),
        mv(0x2139, 0x2112),
        mv(0x213A, 0x2113),
    ]);
    let Some((r, _, dir)) = run_dump("hl-vram-read", &code, &[], false) else { return };
    let _ = std::fs::remove_dir_all(dir);
    for (reg, want) in [(0x2110, "11"), (0x2111, "22"), (0x2112, "33"), (0x2113, "44")] {
        assert!(r.reg(reg).is_some_and(|l| l.ends_with(&format!("last={want}"))), "${reg:04X}\n{}", r.out);
    }
}

#[test]
fn cgram_takes_two_bytes_per_colour_and_drops_a_half_colour() {
    let code = prog(&[
        st(0x00, 0x2121),
        st(0x01, 0x2122),
        st(0x02, 0x2122),
        st(0x03, 0x2122),
        st(0x04, 0x2122),
        st(0x05, 0x2122),
        st(0x06, 0x2122),
        st(0x07, 0x2122),
        st(0xFF, 0x2122), // the top bit of the high byte is dropped
        st(0xAA, 0x2122), // half a colour...
        st(0x10, 0x2121), // ...dropped by setting the index again
        st(0xBB, 0x2122),
        st(0xCC, 0x2122),
        st(0x00, 0x2121),
        mv(0x213B, 0x2110),
        mv(0x213B, 0x2111),
        mv(0x213B, 0x2112),
        mv(0x213B, 0x2113),
    ]);
    let Some((r, m, dir)) = run_dump("hl-cgram", &code, &[], true) else { return };
    let _ = std::fs::remove_dir_all(dir);
    assert_eq!(r.code, 0, "{}", r.out);
    assert_eq!(&m.cgram[0..8], &[0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x7F]);
    assert_eq!(m.cgram[8], 0);
    assert_eq!(&m.cgram[0x20..0x22], &[0xBB, 0x4C]); // the top bit of the high byte is dropped
    for (reg, want) in [(0x2110, "01"), (0x2111, "02"), (0x2112, "03"), (0x2113, "04")] {
        assert!(r.reg(reg).is_some_and(|l| l.ends_with(&format!("last={want}"))), "${reg:04X}\n{}", r.out);
    }
}

#[test]
fn oam_has_a_paired_low_table_and_a_direct_high_table() {
    let code = prog(&[
        st(0x00, 0x2102),
        st(0x00, 0x2103),
        st(0x01, 0x2104),
        st(0x02, 0x2104),
        st(0x03, 0x2104),
        st(0x04, 0x2104),
        st(0x00, 0x2102),
        st(0x01, 0x2103), // word $100: the high table
        st(0x5A, 0x2104),
        st(0x00, 0x2102),
        st(0x00, 0x2103),
        mv(0x2138, 0x2110),
        mv(0x2138, 0x2111),
    ]);
    let Some((r, m, dir)) = run_dump("hl-oam", &code, &[], true) else { return };
    let _ = std::fs::remove_dir_all(dir);
    assert_eq!(r.code, 0, "{}", r.out);
    assert_eq!(m.oam.len(), 544);
    assert_eq!(&m.oam[0..4], &[1, 2, 3, 4]);
    assert_eq!(m.oam[512], 0x5A);
    assert!(r.reg(0x2110).is_some_and(|l| l.ends_with("last=01")) && r.reg(0x2111).is_some_and(|l| l.ends_with("last=02")), "{}", r.out);
}

#[test]
fn dump_writes_the_memories_also_after_a_failure_and_only_when_asked() {
    // Writes the colour 0 and the register $2105, then jumps through a pointer nothing resolves.
    let code = [st(0x00, 0x2121), st(0x21, 0x2122), st(0x22, 0x2122), st(0x07, 0x2105), st(0x5A, 0x0020), vec![0x6C, 0x00, 0x00]].concat();
    let Some((r, m, dir)) = run_dump("hl-dump-fail", &code, &[], true) else { return };
    assert_eq!(r.code, 5, "{}", r.out);
    assert_eq!((m.vram.len(), m.cgram.len(), m.oam.len(), m.regs.len()), (0x10000, 512, 544, 0x40));
    assert_eq!(&m.cgram[0..2], &[0x21, 0x22]);
    assert_eq!(m.regs[5], 0x07);
    assert_eq!((m.wram.len(), m.wram[0x20]), (0x20000, 0x5A));
    let _ = std::fs::remove_dir_all(dir);

    let Some((r, m, dir)) = run_dump("hl-dump-off", &code, &[], false) else { return };
    assert_eq!(r.code, 5, "{}", r.out);
    assert!(m.vram.is_empty() && std::fs::read_dir(&dir).unwrap().next().is_none());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn vram_address_translation_is_reported_when_requested() {
    let code = prog(&[st(0x84, 0x2115)]);
    let Some((r, _, dir)) = run_dump("hl-vram-translate", &code, &[], false) else { return };
    let _ = std::fs::remove_dir_all(dir);
    assert!(r.out.contains("warning: VRAM address translation ($2115=84) is not modelled"), "{}", r.out);
}

// ---- Screenshot ----

/// A decoded PNG: RGB, 8 bits per channel.
struct Img {
    w: usize,
    h: usize,
    px: Vec<u8>,
}

impl Img {
    fn rgb(&self, x: usize, y: usize) -> [u8; 3] {
        let i = (y * self.w + x) * 3;
        [self.px[i], self.px[i + 1], self.px[i + 2]]
    }
}

/// Decode a PNG made of stored zlib blocks and filter 0 only, which is all the runtime writes.
fn decode_png(b: &[u8]) -> Img {
    assert_eq!(&b[..8], &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A], "PNG signature");
    let (mut pos, mut w, mut h, mut z) = (8, 0usize, 0usize, Vec::new());
    let mut seen_end = false;
    while pos < b.len() {
        let n = u32::from_be_bytes(b[pos..pos + 4].try_into().unwrap()) as usize;
        let kind = &b[pos + 4..pos + 8];
        let data = &b[pos + 8..pos + 8 + n];
        match kind {
            b"IHDR" => {
                w = u32::from_be_bytes(data[0..4].try_into().unwrap()) as usize;
                h = u32::from_be_bytes(data[4..8].try_into().unwrap()) as usize;
                assert_eq!(&data[8..], &[8, 2, 0, 0, 0]);
            }
            b"IDAT" => z.extend_from_slice(data),
            b"IEND" => seen_end = true,
            _ => {}
        }
        pos += 12 + n;
    }
    assert!(seen_end, "IEND");
    let mut raw = Vec::new();
    let mut p = 2; // zlib header
    loop {
        let last = z[p] & 1 == 1;
        let len = u16::from_le_bytes([z[p + 1], z[p + 2]]) as usize;
        assert_eq!(u16::from_le_bytes([z[p + 3], z[p + 4]]), !(len as u16), "stored block length check");
        raw.extend_from_slice(&z[p + 5..p + 5 + len]);
        p += 5 + len;
        if last {
            break;
        }
    }
    assert_eq!(raw.len(), (1 + w * 3) * h);
    let mut px = Vec::new();
    for row in raw.chunks(1 + w * 3) {
        assert_eq!(row[0], 0, "filter");
        px.extend_from_slice(&row[1..]);
    }
    Img { w, h, px }
}

/// `LDA #v ; STA addr` sequences that fill VRAM from word address `addr`.
fn vram_words(addr: u16, words: &[u16]) -> Vec<u8> {
    let mut v = [st(0x80, 0x2115), st(addr as u8, 0x2116), st((addr >> 8) as u8, 0x2117)].concat();
    for w in words {
        v.extend(st(*w as u8, 0x2118));
        v.extend(st((*w >> 8) as u8, 0x2119));
    }
    v
}

fn cgram_set(idx: u8, colors: &[u16]) -> Vec<u8> {
    let mut v = st(idx, 0x2121);
    for c in colors {
        v.extend(st(*c as u8, 0x2122));
        v.extend(st((*c >> 8) as u8, 0x2122));
    }
    v
}

fn oam_set(word: u16, bytes: &[u8]) -> Vec<u8> {
    let mut v = [st(word as u8, 0x2102), st((word >> 8) as u8, 0x2103)].concat();
    for b in bytes {
        v.extend(st(*b, 0x2104));
    }
    v
}

/// A 4 bits per pixel tile (16 words) with the given `(x, y, colour)` pixels.
fn tile4(pixels: &[(usize, usize, u8)]) -> Vec<u16> {
    let mut t = vec![0u16; 16];
    for &(x, y, c) in pixels {
        let bit = 0x80u16 >> x;
        for plane in 0..4 {
            if c >> plane & 1 == 1 {
                t[(plane / 2) * 8 + y] |= bit << ((plane % 2) * 8);
            }
        }
    }
    t
}

/// A 2 bits per pixel tile (8 words).
fn tile2(pixels: &[(usize, usize, u8)]) -> Vec<u16> {
    let mut t = vec![0u16; 8];
    for &(x, y, c) in pixels {
        let bit = 0x80u16 >> x;
        for plane in 0..2 {
            if c >> plane & 1 == 1 {
                t[y] |= bit << (plane * 8);
            }
        }
    }
    t
}

const RED: u16 = 0x001F;
const GREEN: u16 = 0x03E0;
const BLUE: u16 = 0x7C00;
const YELLOW: u16 = 0x03FF;

/// The colour a BGR555 value shows at full brightness.
fn rgb(c: u16) -> [u8; 3] {
    let e = |v: u16| ((v << 3) | (v >> 2)) as u8;
    [e(c & 31), e(c >> 5 & 31), e(c >> 10 & 31)]
}

/// Run `code` at 00:8000 for one short frame asking for a screenshot; returns the run and the decoded image.
fn shot(name: &str, code: &[u8]) -> Option<(Run, Option<Img>)> {
    shot_with(name, code, &[])
}

/// `shot` with more options for the runtime (e.g. `--wide 32`).
fn shot_with(name: &str, code: &[u8], extra: &[&str]) -> Option<(Run, Option<Img>)> {
    let dir = std::env::temp_dir().join(format!("snes-recomp-shot-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("s.png").to_string_lossy().into_owned();
    let mut args = vec!["rom.sfc", "--frames", "1", "--frame-blocks", "20", "--screenshot", path.as_str()];
    args.extend_from_slice(extra);
    let r = run(name, &[(0x8000, code)], &[0x8000], None, &args)?;
    let img = std::fs::read(dir.join("s.png")).ok().map(|b| decode_png(&b));
    let _ = std::fs::remove_dir_all(dir);
    Some((r, img))
}

/// Mode 1 with the layers in `main`, full brightness, BG1 map at word $1000 and BG2 at $1400, BG3 map at $1800
/// with its tiles at word $2000, BG1/BG2 tiles at word 0 and sprite tiles at word $2000.
fn ppu_base(main: u8) -> Vec<u8> {
    [
        st(0x0F, 0x2100),
        st(0x01, 0x2105),
        st(0x10, 0x2107),
        st(0x14, 0x2108),
        st(0x18, 0x2109),
        st(0x00, 0x210B),
        st(0x02, 0x210C),
        st(0x01, 0x2101),
        st(main, 0x212C),
    ]
    .concat()
}

#[test]
fn the_screenshot_is_a_png_of_the_backdrop_when_nothing_else_is_drawn() {
    let code = prog(&[st(0x0F, 0x2100), cgram_set(0, &[RED]), st(0x01, 0x212C)]);
    let Some((r, img)) = shot("rd-backdrop", &code) else { return };
    assert_eq!(r.code, 0, "{}", r.out);
    let img = img.expect("the screenshot exists");
    assert_eq!((img.w, img.h), (256, 224));
    assert!(img.px.chunks(3).all(|p| p == rgb(RED)));

    // The screenshot is also written after a failure.
    let code = [st(0x0F, 0x2100), cgram_set(0, &[BLUE]), vec![0x6C, 0x00, 0x00]].concat();
    let Some((r, img)) = shot("rd-fail", &code) else { return };
    assert_eq!(r.code, 5, "{}", r.out);
    assert_eq!(img.expect("written after a failure").rgb(3, 3), rgb(BLUE));

    // And nothing is written without the option.
    let code = prog(&[cgram_set(0, &[RED])]);
    let Some((_, m, dir)) = run_dump("rd-off", &code, &[], false) else { return };
    assert!(m.vram.is_empty() && std::fs::read_dir(&dir).unwrap().next().is_none());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn brightness_forced_blank_and_disabled_layers() {
    let tile = vram_words(16, &tile4(&[(0, 0, 1)]));
    let map = vram_words(0x1000, &[0x0001]);
    let pal = cgram_set(0, &[RED, GREEN]);

    // Brightness 7 of 15 scales every channel.
    let code = prog(&[ppu_base(0x01), tile.clone(), map.clone(), pal.clone(), st(0x07, 0x2100)]);
    let Some((_, img)) = shot("rd-bright", &code) else { return };
    let img = img.unwrap();
    assert_eq!(img.rgb(0, 0), [0, (255u32 * 7 / 15) as u8, 0]);
    assert_eq!(img.rgb(5, 5), [(255u32 * 7 / 15) as u8, 0, 0]);

    // Forced blank is black whatever the memories hold.
    let code = prog(&[ppu_base(0x01), tile.clone(), map.clone(), pal.clone(), st(0x8F, 0x2100)]);
    let Some((_, img)) = shot("rd-blank", &code) else { return };
    assert!(img.unwrap().px.iter().all(|&b| b == 0));

    // A background that is not on the main screen is not drawn.
    let code = prog(&[ppu_base(0x02), tile, map, pal]);
    let Some((_, img)) = shot("rd-off-layer", &code) else { return };
    assert_eq!(img.unwrap().rgb(0, 0), rgb(RED));
}

#[test]
fn a_background_tile_has_its_pixel_flips_and_transparency() {
    // Tile 1 has colour 5 at (2,3); entry (0,0) of BG1 is tile 1 with palette 1.
    let tile = vram_words(16, &tile4(&[(2, 3, 5)]));
    let pal = cgram_set(0, &[RED]);
    let green = cgram_set(16 + 5, &[GREEN]);
    for (name, entry, at) in [("plain", 0x0401u16, (2, 3)), ("hflip", 0x4401, (5, 3)), ("vflip", 0x8401, (2, 4)), ("both", 0xC401, (5, 4))] {
        let code = prog(&[ppu_base(0x01), tile.clone(), vram_words(0x1000, &[entry]), pal.clone(), green.clone()]);
        let Some((r, img)) = shot(&format!("rd-tile-{name}"), &code) else { return };
        assert_eq!(r.code, 0, "{}", r.out);
        let img = img.unwrap();
        assert_eq!(img.rgb(at.0, at.1), rgb(GREEN), "{name}");
        // Every other pixel of the tile is transparent: the backdrop shows.
        assert_eq!(img.rgb(0, 0), rgb(RED), "{name}");
        assert_eq!(img.rgb(at.0 + 1, at.1), rgb(RED), "{name}");
    }
}

#[test]
fn sixteen_pixel_tiles_use_four_tiles_and_swap_them_when_flipped() {
    // Tile 17 (the bottom right quarter of tile 0) has colour 1 at (1,1).
    let tile = vram_words(17 * 16, &tile4(&[(1, 1, 1)]));
    let pal = cgram_set(0, &[RED, GREEN]);
    for (name, entry, at) in [("plain", 0x0000u16, (9, 9)), ("hflip", 0x4000, (6, 9)), ("vflip", 0x8000, (9, 6))] {
        let code = prog(&[ppu_base(0x01), st(0x11, 0x2105), tile.clone(), vram_words(0x1000, &[entry]), pal.clone()]);
        let Some((_, img)) = shot(&format!("rd-big-{name}"), &code) else { return };
        let img = img.unwrap();
        assert_eq!(img.rgb(at.0, at.1), rgb(GREEN), "{name}");
        assert_eq!(img.rgb(1, 1), rgb(RED), "{name}");
    }
}

#[test]
fn scroll_registers_take_two_writes_and_move_the_background() {
    // Tile 1 has colour 1 at (0,0); the map puts it at tile column 1 and, in the 64x64 map, at (32,32).
    let tile = vram_words(16, &tile4(&[(0, 0, 1)]));
    let pal = cgram_set(0, &[RED, GREEN]);
    // Horizontal scroll of 8: the tile at column 1 reaches the left edge.
    let code = prog(&[ppu_base(0x01), tile.clone(), vram_words(0x1001, &[0x0001]), pal.clone(), st(0x08, 0x210D), st(0x00, 0x210D)]);
    let Some((_, img)) = shot("rd-scroll-h", &code) else { return };
    let img = img.unwrap();
    assert_eq!(img.rgb(0, 0), rgb(GREEN));
    assert_eq!(img.rgb(8, 0), rgb(RED));

    // A 64x64 map scrolled by (256, 256) shows its fourth screen: entry (0,0) is at word $1000 + 3*$400.
    let code = prog(&[
        ppu_base(0x01),
        st(0x13, 0x2107),
        tile,
        vram_words(0x1000 + 3 * 0x400, &[0x0001]),
        pal,
        st(0x00, 0x210D),
        st(0x01, 0x210D),
        st(0x00, 0x210E),
        st(0x01, 0x210E),
    ]);
    let Some((_, img)) = shot("rd-scroll-big", &code) else { return };
    assert_eq!(img.unwrap().rgb(0, 0), rgb(GREEN));
}

#[test]
fn the_interactive_frontend_without_a_window_draws_the_same_picture_as_the_headless_runtime() {
    let mut parts = scroll_scene();
    parts.extend([st(0x04, 0x210D), st(0x00, 0x210D)]);
    let code = prog(&parts);
    let dir = std::env::temp_dir().join(format!("snes-recomp-play-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("p.png").to_string_lossy().into_owned();
    let Some(r) = run_play("play-nowindow", &[(0x8000, &code)], &[0x8000], &["rom.sfc", "--no-window", "--frames", "1", "--frame-blocks", "20", "--screenshot", &path]) else { return };
    assert_eq!(r.code, 0, "{}", r.out);
    let play = std::fs::read(dir.join("p.png")).expect("no screenshot from snes_play");
    let _ = std::fs::remove_dir_all(&dir);
    let Some((_, head)) = shot("play-vs-headless", &code) else { return };
    let play = decode_png(&play);
    let head = head.unwrap();
    assert!(play.px == head.px, "the two runtimes drew different pictures");
}

#[test]
fn the_interactive_frontend_rejects_a_missing_rom_and_bad_options() {
    let code = prog(&[st(0x00, 0x2100)]);
    let Some(r) = run_play("play-missing", &[(0x8000, &code)], &[0x8000], &["missing.sfc", "--no-window"]) else { return };
    assert_eq!(r.code, 2, "{}", r.out);
    let Some(r) = run_play("play-badopt", &[(0x8000, &code)], &[0x8000], &["rom.sfc", "--no-window", "--bogus"]) else { return };
    assert_eq!(r.code, 2, "{}", r.out);
}

#[test]
fn a_live_source_and_press_add_up_in_the_automatic_read() {
    // $4200 = 1, then each vblank: LDA $4219 -> $2134 ; LDA $4218 -> $2135 (as in the --press test)
    let code = prog(&[st(0x01, 0x4200), vec![0xAD, 0x12, 0x42, 0x10, 0xFB], mv(0x4219, 0x2134), mv(0x4218, 0x2135), vec![0x80, 0xED]]);
    let seg = [(0x8000, code.as_slice())];
    let base = ["rom.sfc", "--no-window", "--frames", "4", "--frame-blocks", "50"];
    // RIGHT is held by the source, A by --press: both show.
    let args: Vec<&str> = base.iter().copied().chain(["--hold", "RIGHT", "--press", "A@0-9"]).collect();
    let Some(r) = run_play("play-hold", &seg, &[0x8000], &args) else { return };
    assert_eq!(r.code, 0, "{}", r.out);
    assert!(r.reg(0x2134).is_some_and(|l| l.ends_with("last=01")), "RIGHT (bit 0) in $4219
{}", r.out);
    assert!(r.reg(0x2135).is_some_and(|l| l.ends_with("last=80")), "A (bit 7) in $4218
{}", r.out);
    // Without --hold only --press counts; a wrong name is a usage error.
    let args: Vec<&str> = base.iter().copied().chain(["--press", "A@0-9"]).collect();
    let Some(r) = run_play("play-nohold", &seg, &[0x8000], &args) else { return };
    assert!(r.reg(0x2134).is_some_and(|l| l.ends_with("last=00")), "{}", r.out);
    let args: Vec<&str> = base.iter().copied().chain(["--hold", "FIRE"]).collect();
    let Some(r) = run_play("play-badhold", &seg, &[0x8000], &args) else { return };
    assert_eq!(r.code, 2, "{}", r.out);
}

#[test]
fn a_keys_file_next_to_the_rom_changes_the_keyboard_map() {
    let seg = [(0x8000, [0x80u8, 0xFE].as_slice())];
    let keys = b"# my keys
Space = A
F = Q
NoSuchKey = B
X = FIRE
";
    let Some(r) = run_play_with("play-keys", &seg, &[0x8000], &["rom.sfc", "--print-keys"], &[("rom.keys", keys)]) else { return };
    assert_eq!(r.code, 0, "{}", r.out);
    let lines: Vec<&str> = r.out.lines().collect();
    assert!(lines.contains(&"Space = A"), "{}", r.out);
    assert!(!lines.contains(&"S = A"), "A loses its default key
{}", r.out);
    assert!(lines.contains(&"X = B") && lines.contains(&"Up = UP"), "the other defaults stay
{}", r.out);
    // `F = Q` is not a button name, so L keeps its default; the two bad lines are ignored (warnings go to stderr)
    assert!(lines.contains(&"Q = L") && !lines.iter().any(|l| l.starts_with("F =")), "{}", r.out);
    assert_eq!(lines.len(), 12, "12 bindings in all\n{}", r.out);
}

/// `LDA $70:0000 ; STA $2134` (what the game finds in the SRAM).
fn read_sram() -> Vec<u8> {
    vec![0xAF, 0x00, 0x00, 0x70, 0x8D, 0x34, 0x21]
}

#[test]
fn the_sram_is_saved_to_a_srm_file_and_found_again() {
    // First run: read $70:0000 (shown in $2134), then write $5A there. The second run finds the $5A.
    let code = prog(&[read_sram(), vec![0xA9, 0x5A, 0x8F, 0x00, 0x00, 0x70]]);
    let seg = [(0x8000, code.as_slice())];
    let args = ["rom.sfc", "--no-window", "--frames", "2", "--frame-blocks", "50"];
    let Some(r) = run_play_times("play-srm", &seg, &[0x8000], &args, &[], 2) else { return };
    assert_eq!(r.code, 0, "{}", r.out);
    let seen: Vec<&str> = r.out.lines().filter(|l| l.starts_with("  $2134 ")).map(|l| l.rsplit("last=").next().unwrap()).collect();
    assert_eq!(seen, ["00", "5A"], "{}", r.out);
    assert_eq!(r.out.matches("sram: saved").count(), 1, "saved once, in the run that changed it
{}", r.out);
    assert_eq!(r.out.matches("sram: loaded").count(), 1, "loaded by the second run only
{}", r.out);
}

#[test]
fn a_srm_of_the_wrong_size_is_ignored_and_one_without_changes_is_not_rewritten() {
    let seg_read = [(0x8000, prog(&[read_sram()]))];
    let seg = [(seg_read[0].0, seg_read[0].1.as_slice())];
    let args = ["rom.sfc", "--no-window", "--frames", "2", "--frame-blocks", "50"];
    let mut good = vec![0u8; 0x8000];
    good[0] = 0x33;
    let Some(r) = run_play_times("play-srm-ok", &seg, &[0x8000], &args, &[("rom.srm", &good)], 2) else { return };
    assert_eq!(r.out.matches("last=33").count(), 2, "{}", r.out);
    assert!(!r.out.contains("sram: saved"), "no change, no rewrite
{}", r.out);
    let Some(r) = run_play_times("play-srm-bad", &seg, &[0x8000], &args, &[("rom.srm", &[0x44u8; 100])], 1) else { return };
    assert_eq!(r.code, 0, "{}", r.out);
    assert!(r.reg(0x2134).is_some_and(|l| l.ends_with("last=00")), "the short file is not used
{}", r.out);
    assert!(!r.out.contains("sram: loaded"), "{}", r.out);
}

fn tile8(pixels: &[(usize, usize, u8)]) -> Vec<u16> {
    let mut t = vec![0u16; 32];
    for &(x, y, c) in pixels {
        let bit = 0x80u16 >> x;
        for plane in 0..8 {
            if c >> plane & 1 == 1 {
                t[(plane / 2) * 8 + y] |= bit << ((plane % 2) * 8);
            }
        }
    }
    t
}

#[test]
fn mode_0_has_four_backgrounds_of_two_bits_with_a_palette_region_each() {
    // Tile 1 of BG1 and BG3, tile 2 of BG2 and tile 1 of BG4 each have colour 1 at a different pixel of the first row.
    let code = prog(&[
        ppu_base(0x0F),
        st(0x00, 0x2105),
        st(0x1C, 0x210A),
        st(0x32, 0x210C),
        vram_words(8, &tile2(&[(0, 0, 1)])),
        vram_words(16, &tile2(&[(1, 0, 1)])),
        vram_words(0x2008, &tile2(&[(2, 0, 1)])),
        vram_words(0x3008, &tile2(&[(3, 0, 1)])),
        vram_words(0x1000, &[1]),
        vram_words(0x1400, &[2]),
        vram_words(0x1800, &[1]),
        vram_words(0x1C00, &[1]),
        cgram_set(1, &[RED]),
        cgram_set(33, &[GREEN]),
        cgram_set(65, &[BLUE]),
        cgram_set(97, &[YELLOW]),
    ]);
    let Some((r, img)) = shot("rd-mode0", &code) else { return };
    assert!(!r.out.contains("graphics mode"), "{}", r.out);
    let img = img.unwrap();
    assert_eq!(img.rgb(0, 0), rgb(RED));
    assert_eq!(img.rgb(1, 0), rgb(GREEN));
    assert_eq!(img.rgb(2, 0), rgb(BLUE));
    assert_eq!(img.rgb(3, 0), rgb(YELLOW));
    assert_eq!(img.rgb(4, 0), rgb(0));
}

#[test]
fn mode_3_has_a_background_of_eight_bits_in_front_of_one_of_four() {
    // BG1 (8 bits, colour $C8 at pixel 0) and BG2 (4 bits, palette 0 colour 3 at pixel 1) in the first row.
    let code = prog(&[
        ppu_base(0x03),
        st(0x03, 0x2105),
        st(0x40, 0x210B),
        vram_words(32, &tile8(&[(0, 0, 0xC8)])),
        vram_words(0x4010, &tile4(&[(1, 0, 3)])),
        vram_words(0x1000, &[1]),
        vram_words(0x1400, &[1]),
        cgram_set(0xC8, &[RED]),
        cgram_set(3, &[GREEN]),
    ]);
    let Some((r, img)) = shot("rd-mode3", &code) else { return };
    assert!(!r.out.contains("graphics mode"), "{}", r.out);
    let img = img.unwrap();
    assert_eq!(img.rgb(0, 0), rgb(RED));
    assert_eq!(img.rgb(1, 0), rgb(GREEN));
}

#[test]
fn mode_2_scrolls_a_column_of_eight_pixels_by_the_entry_of_the_third_background() {
    // BG1 shows tile 1 (colour 1 at its top left pixel) in map column 1, so at x = 8. BG3's first entry gives the
    // column starting at x = 8 a horizontal offset of 8 for BG1 (bit 13), which moves it onto the empty column 2.
    let tile = vram_words(16, &tile4(&[(0, 0, 1)]));
    let pal = cgram_set(0, &[RED, GREEN]);
    let plain = prog(&[ppu_base(0x01), st(0x02, 0x2105), tile.clone(), vram_words(0x1001, &[1]), pal.clone()]);
    let Some((r, img)) = shot("rd-mode2-plain", &plain) else { return };
    assert!(!r.out.contains("graphics mode"), "{}", r.out);
    assert_eq!(img.unwrap().rgb(8, 0), rgb(GREEN));
    let shifted = prog(&[ppu_base(0x01), st(0x02, 0x2105), tile, vram_words(0x1001, &[1]), vram_words(0x1800, &[0x2008]), pal]);
    let Some((_, img)) = shot("rd-mode2-offset", &shifted) else { return };
    let img = img.unwrap();
    assert_eq!(img.rgb(8, 0), rgb(RED));
    // The entry does not apply to BG2 (bit 14 is clear), and the first column (x < 8) is never shifted.
    assert_eq!(img.rgb(0, 0), rgb(RED));
}

#[test]
fn mode_5_blends_the_two_columns_of_each_pixel_with_the_character_pair_of_an_entry() {
    // Entry 1 of BG1 (4 bits) is a pair: character 1 on the left 8 pixels of 512 and character 2 on the right 8. Both
    // have colour 1 at their top left pixel, so columns 0 and 8 of the 512 are set. The main screen (right column of each
    // pair) has none of them, the subscreen (left column) does: a picture pixel is the blend of colour and backdrop.
    let code = prog(&[
        ppu_base(0x01),
        st(0x05, 0x2105),
        st(0x01, 0x212D),
        vram_words(16, &tile4(&[(0, 0, 1)])),
        vram_words(32, &tile4(&[(0, 0, 1)])),
        vram_words(0x1000, &[1]),
        cgram_set(0, &[0]),
        cgram_set(1, &[0x001F]),
    ]);
    let Some((r, img)) = shot("rd-mode5", &code) else { return };
    assert!(!r.out.contains("graphics mode"), "{}", r.out);
    let img = img.unwrap();
    // Column 0 of the 512 is the left one of picture pixel 0 (on the subscreen): half red. Pixel 4 (column 8, character 2) too.
    let half = img.rgb(0, 0);
    assert!(half[0] > 100 && half[0] < 160 && half[1] == 0, "{half:?}");
    assert_eq!(img.rgb(4, 0), half);
    assert_eq!(img.rgb(1, 0), rgb(0));
}

#[test]
fn only_the_first_32_sprites_of_a_line_are_drawn() {
    // Sprites 0-31 are 8x8, one every 8 pixels on one line (palette 2); sprite 32 (palette 3) sits on X = 0 as well.
    // First sprite 0 is moved off the line, so 32 sprites touch it and all are drawn; then it is moved back and sprite 32 is left out.
    let mut oam = Vec::new();
    for i in 0..32u16 {
        oam.extend([(i * 8) as u8, 20, 1, 0x04]);
    }
    oam.extend([0, 20, 1, 0x06]);
    oam[1] = 100;
    let code = prog(&[
        ppu_base(0x10),
        sprite_tile(1, &[(0, 0, 3)]),
        cgram_set(128 + 2 * 16 + 3, &[GREEN]),
        cgram_set(128 + 3 * 16 + 3, &[BLUE]),
        cgram_set(0, &[RED]),
        oam_set(0, &oam),
    ]);
    let Some((r, img)) = shot("rd-spr-limit", &code) else { return };
    assert_eq!(r.code, 0, "{}", r.out);
    let img = img.unwrap();
    // The line holds sprites 1..32: all drawn, and sprite 32 (palette 3) is in front of nothing at X = 0.
    assert_eq!(img.rgb(8, 21), rgb(GREEN));
    assert_eq!(img.rgb(0, 21), rgb(BLUE));
    // Now sprite 0 is back: 33 sprites touch the line and the one with the highest index is left out.
    oam[1] = 20;
    let code = prog(&[
        ppu_base(0x10),
        sprite_tile(1, &[(0, 0, 3)]),
        cgram_set(128 + 2 * 16 + 3, &[GREEN]),
        cgram_set(128 + 3 * 16 + 3, &[BLUE]),
        cgram_set(0, &[RED]),
        oam_set(0, &oam),
    ]);
    let Some((_, img)) = shot("rd-spr-limit2", &code) else { return };
    let img = img.unwrap();
    assert_eq!(img.rgb(0, 21), rgb(GREEN));
    assert_eq!(img.rgb(8, 21), rgb(GREEN));
}

#[test]
fn no_graphics_mode_is_reported_as_not_drawn() {
    for mode in 0..8u8 {
        let code = prog(&[st(0x0F, 0x2100), st(mode, 0x2105), st(0x11, 0x212C), st(0x01, 0x212E), st(0x11, 0x2106)]);
        let Some((r, _)) = shot(&format!("rd-warn-mode{mode}"), &code) else { return };
        assert!(!r.out.contains("warning:"), "mode {mode}: {}", r.out);
    }
}

/// Like `shot` but with 2620 blocks per frame (ten per line) and `frames` frames, to time writes within the frame.
fn timed_shot(name: &str, code: &[u8], frames: &str) -> Option<(Run, Img)> {
    let dir = std::env::temp_dir().join(format!("snes-recomp-shot-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("s.png").to_string_lossy().into_owned();
    let r = run(name, &[(0x8000, code)], &[0x8000], None, &["rom.sfc", "--frames", frames, "--frame-blocks", "2620", "--screenshot", &path])?;
    let img = std::fs::read(dir.join("s.png")).ok().map(|b| decode_png(&b));
    let _ = std::fs::remove_dir_all(dir);
    Some((r, img.expect("no screenshot")))
}

/// BG1 with tile 1 (colour 1 at its top left pixel) at tile rows 5 and 17, so lines 40 and 136 show it every 8 pixels.
fn scroll_scene() -> Vec<Vec<u8>> {
    let mut row5 = vec![0u16; 32];
    row5.iter_mut().for_each(|w| *w = 1);
    vec![
        ppu_base(0x01),
        vram_words(16, &tile4(&[(0, 0, 1)])),
        vram_words(0x1000 + 5 * 32, &row5),
        vram_words(0x1000 + 17 * 32, &row5),
        cgram_set(0, &[RED, GREEN]),
    ]
}

#[test]
fn a_scroll_written_in_the_middle_of_the_frame_only_moves_the_lines_below() {
    // After a delay of about 1280 blocks (line 90 of a frame of 2620 blocks, ten per line) the horizontal scroll becomes 4.
    let mut parts = scroll_scene();
    parts.push(vec![0x18, 0xFB, 0xC2, 0x10, 0xA2, 0x00, 0x05, 0xCA, 0xD0, 0xFD]); // CLC XCE ; REP #$10 ; LDX #$0500 ; DEX ; BNE
    parts.extend([st(0x04, 0x210D), st(0x00, 0x210D)]);
    let Some((r, img)) = timed_shot("rd-midscroll", &prog(&parts), "1") else { return };
    assert_eq!(r.code, 0, "{}", r.out);
    // Line 40 was drawn before the write: the tile starts at x = 0. Line 136 after it: at x = 4.
    assert_eq!((img.rgb(0, 40), img.rgb(4, 40)), (rgb(GREEN), rgb(RED)));
    assert_eq!((img.rgb(0, 136), img.rgb(4, 136)), (rgb(RED), rgb(GREEN)));
}

#[test]
fn a_write_in_the_vblank_applies_from_the_first_line_of_the_next_frame() {
    // The scroll is written right at the start, in the vertical blank of frame 1: every line has it.
    let mut parts = scroll_scene();
    parts.extend([st(0x04, 0x210D), st(0x00, 0x210D)]);
    let Some((_, img)) = timed_shot("rd-vblankscroll", &prog(&parts), "1") else { return };
    assert_eq!((img.rgb(0, 40), img.rgb(4, 40)), (rgb(RED), rgb(GREEN)));
    assert_eq!((img.rgb(0, 136), img.rgb(4, 136)), (rgb(RED), rgb(GREEN)));
}

#[test]
fn the_same_run_gives_the_same_picture_byte_for_byte() {
    let mut parts = scroll_scene();
    parts.extend([vec![0x18, 0xFB, 0xC2, 0x10, 0xA2, 0x00, 0x05, 0xCA, 0xD0, 0xFD], st(0x04, 0x210D), st(0x00, 0x210D)]);
    let code = prog(&parts);
    let (Some((_, a)), Some((_, b))) = (timed_shot("rd-repro-a", &code, "3"), timed_shot("rd-repro-b", &code, "3")) else { return };
    assert!(a.px == b.px, "the two runs drew different pictures");
}

#[test]
fn tile_priority_orders_the_backgrounds_and_the_bit_of_bg3() {
    // BG1 and BG2 both have a tile with colour 1 at (0,0): BG1 uses palette 0, BG2 palette 1; BG3 (2 bits)
    // has colour 1 of palette 2.
    let t4 = [vram_words(16, &tile4(&[(0, 0, 1)])), vram_words(32, &tile4(&[(0, 0, 1)]))].concat();
    let t2 = vram_words(0x2000 + 8, &tile2(&[(0, 0, 1)]));
    let pal = cgram_set(0, &[0, RED]);
    let pal2 = [cgram_set(16 + 1, &[GREEN]), cgram_set(2 * 4 + 1, &[BLUE])].concat();
    let case = |name: &str, bg1: u16, bg2: u16, bg3: u16, extra: Vec<u8>| -> Option<[u8; 3]> {
        let code = prog(&[
            ppu_base(0x07),
            t4.clone(),
            t2.clone(),
            vram_words(0x1000, &[bg1]),
            vram_words(0x1400, &[bg2]),
            vram_words(0x1800, &[bg3]),
            pal.clone(),
            pal2.clone(),
            extra,
        ]);
        let (_, img) = shot(&format!("rd-pri-{name}"), &code)?;
        Some(img.unwrap().rgb(0, 0))
    };
    let (e1, e2, e3) = (0x0001u16, 0x0401 | 0x0000, 0x0801);
    // BG2 with high priority covers BG1 with low priority; and the reverse.
    let Some(c) = case("bg2-front", e1, e2 | 0x2000, 0, vec![]) else { return };
    assert_eq!(c, rgb(GREEN));
    let Some(c) = case("bg1-front", e1 | 0x2000, e2, 0, vec![]) else { return };
    assert_eq!(c, rgb(RED));
    // With equal priority BG1 is in front of BG2, and both are in front of BG3 of low priority.
    let Some(c) = case("equal", e1, e2, e3, vec![]) else { return };
    assert_eq!(c, rgb(RED));
    // BG3 with high priority is behind BG1 and BG2 of high priority, and in front of them with the bit set.
    let Some(c) = case("bg3-high", e1 | 0x2000, e2, e3 | 0x2000, vec![]) else { return };
    assert_eq!(c, rgb(RED));
    let Some(c) = case("bg3-bit", e1 | 0x2000, e2, e3 | 0x2000, st(0x09, 0x2105)) else { return };
    assert_eq!(c, rgb(BLUE));
}

/// Sprite tiles live at word $2000 (name base 1); `tile` is the tile number, `px` its pixels.
fn sprite_tile(tile: u16, px: &[(usize, usize, u8)]) -> Vec<u8> {
    vram_words(0x2000 + tile * 16, &tile4(px))
}

#[test]
fn sprites_have_palette_flip_position_size_and_order() {
    let pal = cgram_set(128 + 2 * 16 + 3, &[GREEN]);
    let pal3 = cgram_set(128 + 3 * 16 + 3, &[BLUE]);
    let back = cgram_set(0, &[RED]);
    let one = |name: &str, oam: &[u8], hi: &[u8], extra: Vec<u8>| -> Option<Img> {
        let code = prog(&[
            ppu_base(0x10),
            sprite_tile(1, &[(1, 1, 3)]),
            sprite_tile(0x12, &[(0, 0, 3)]),
            pal.clone(),
            pal3.clone(),
            back.clone(),
            oam_set(0, oam),
            oam_set(0x100, hi),
            extra,
        ]);
        let (r, img) = shot(&format!("rd-spr-{name}"), &code)?;
        assert_eq!(r.code, 0, "{}", r.out);
        img
    };
    // Sprite 0: 8x8 at (10,20), tile 1, palette 2. The pixel (1,1) shows at (11, 22): Y is one line late.
    let Some(img) = one("small", &[10, 20, 1, 0x04], &[], vec![]) else { return };
    assert_eq!(img.rgb(11, 22), rgb(GREEN));
    assert_eq!(img.rgb(10, 20), rgb(RED));
    // Flipped both ways: the pixel (1,1) moves to (6,6).
    let Some(img) = one("flip", &[10, 20, 1, 0x04 | 0xC0], &[], vec![]) else { return };
    assert_eq!(img.rgb(16, 27), rgb(GREEN));
    // X = 508 (9 bits): the sprite starts four pixels left of the screen, so pixel (1,1) is at (-3, ..) and is cut.
    let Some(img) = one("negative", &[0xFC, 20, 1, 0x04], &[1], vec![]) else { return };
    assert_eq!(img.rgb(0, 22), rgb(RED));
    assert_eq!(img.rgb(1, 22), rgb(RED));
    // Two sprites at the same place and priority: the lower index (palette 2) wins.
    let Some(img) = one("order", &[10, 20, 1, 0x04, 10, 20, 1, 0x06], &[], vec![]) else { return };
    assert_eq!(img.rgb(11, 22), rgb(GREEN));
    // A large sprite (32x32 with size select 1): tile 1 + (1,1) is tile $12, shown at (+8,+8).
    let Some(img) = one("large", &[10, 20, 1, 0x04], &[2], st(0x21, 0x2101)) else { return };
    assert_eq!(img.rgb(10 + 8, 21 + 8), rgb(GREEN));
}

#[test]
fn sprites_mix_with_the_backgrounds_by_priority() {
    // BG1 tile 1 colour 1 (green) at (0,0), low priority; a sprite of priority 1 at the same place (blue).
    let case = |name: &str, spr_pri: u8| -> Option<[u8; 3]> {
        let code = prog(&[
            ppu_base(0x11),
            vram_words(16, &tile4(&[(0, 0, 1)])),
            vram_words(0x1000, &[0x0001]),
            sprite_tile(1, &[(0, 0, 1)]),
            cgram_set(0, &[0, GREEN]),
            cgram_set(128 + 1, &[BLUE]),
            oam_set(0, &[0, 255, 1, spr_pri << 4]),
            // the other 127 sprites are off screen
            st(0x00, 0x2102),
        ]);
        let (_, img) = shot(&format!("rd-mix-{name}"), &code)?;
        Some(img.unwrap().rgb(0, 0))
    };
    // Sprite Y = 255 puts the sprite on line 0; BG1 pixel (0,0) is low priority.
    let Some(c) = case("s1", 1) else { return };
    assert_eq!(c, rgb(GREEN), "priority 1 is behind BG1");
    let Some(c) = case("s2", 2) else { return };
    assert_eq!(c, rgb(BLUE), "priority 2 is in front of low priority BG1");
}

/// BG1 with a single pixel at (0,0) of colour `c`, BG3 (palette 2) with one of colour `c3` in the same place, and `extra`.
fn math_case(name: &str, main: u8, c: u16, c3: u16, extra: Vec<Vec<u8>>) -> Option<Img> {
    let code = prog(&[
        ppu_base(main),
        vram_words(16, &tile4(&[(0, 0, 1)])),
        vram_words(0x2000 + 8, &tile2(&[(0, 0, 1)])),
        vram_words(0x1000, &[0x0001]),
        vram_words(0x1800, &[0x0801]),
        cgram_set(0, &[0x0000, c]),
        cgram_set(2 * 4 + 1, &[c3]),
        extra.concat(),
    ]);
    let (r, img) = shot(&format!("rd-math-{name}"), &code)?;
    assert_eq!(r.code, 0, "{}", r.out);
    img
}

#[test]
fn colour_math_adds_subtracts_and_halves_the_fixed_colour() {
    let fixed_red = st(0x20 | 31, 0x2132);
    // Green plus red is yellow; the backdrop, which is not chosen, is untouched.
    let Some(img) = math_case("add", 0x01, GREEN, 0, vec![st(0x01, 0x2131), fixed_red.clone()]) else { return };
    assert_eq!(img.rgb(0, 0), rgb(YELLOW));
    assert_eq!(img.rgb(5, 5), rgb(0));

    // White minus red keeps green and blue.
    let Some(img) = math_case("sub", 0x01, 0x7FFF, 0, vec![st(0xA1, 0x2131), fixed_red.clone()]) else { return };
    assert_eq!(img.rgb(0, 0), rgb(0x7FE0));

    // Half of the sum, per channel.
    let Some(img) = math_case("half", 0x01, GREEN, 0, vec![st(0x61, 0x2131), fixed_red.clone()]) else { return };
    assert_eq!(img.rgb(0, 0), rgb(15 | 15 << 5));

    // The sum saturates.
    let Some(img) = math_case("sat", 0x01, YELLOW, 0, vec![st(0x21, 0x2131), fixed_red.clone()]) else { return };
    assert_eq!(img.rgb(0, 0), rgb(YELLOW));

    // The fixed colour is built from several writes, each choosing its channels.
    let both = [st(0x20 | 31, 0x2132), st(0x40 | 7, 0x2132)].concat();
    let Some(img) = math_case("fixed-parts", 0x01, 0, 0, vec![st(0x21, 0x2131), both]) else { return };
    assert_eq!(img.rgb(0, 0), rgb(31 | 7 << 5));
}

#[test]
fn colour_math_only_touches_the_chosen_layers_and_regions() {
    let fixed_red = st(0x20 | 31, 0x2132);
    // BG2 is chosen, BG1 is not.
    let Some(img) = math_case("other-layer", 0x01, GREEN, 0, vec![st(0x22, 0x2131), fixed_red.clone()]) else { return };
    assert_eq!(img.rgb(0, 0), rgb(GREEN));
    // The backdrop bit mixes the backdrop.
    let Some(img) = math_case("backdrop", 0x01, GREEN, 0, vec![st(0x20, 0x2131), fixed_red.clone()]) else { return };
    assert_eq!(img.rgb(5, 5), rgb(RED));
    assert_eq!(img.rgb(0, 0), rgb(GREEN));
    // Bits 4-5 of $2130 set to 3 prevent the maths everywhere.
    let Some(img) = math_case("never", 0x01, GREEN, 0, vec![st(0x21, 0x2131), st(0x30, 0x2130), fixed_red]) else { return };
    assert_eq!(img.rgb(0, 0), rgb(GREEN));
}

#[test]
fn colour_math_can_use_the_subscreen() {
    let fixed_red = st(0x20 | 31, 0x2132);
    // BG3 is only on the subscreen: green plus blue is cyan where BG3 has a pixel...
    let extra = vec![st(0x04, 0x212D), st(0x02, 0x2130), st(0x21, 0x2131), fixed_red.clone()];
    let Some(img) = math_case("subscreen", 0x01, GREEN, BLUE, extra.clone()) else { return };
    assert_eq!(img.rgb(0, 0), rgb(0x7FE0));
    // ...and the fixed colour stands in, not halved, when nothing is on the subscreen.
    let extra = vec![st(0x00, 0x212D), st(0x02, 0x2130), st(0x61, 0x2131), fixed_red];
    let Some(img) = math_case("sub-empty", 0x01, GREEN, BLUE, extra) else { return };
    assert_eq!(img.rgb(0, 0), rgb(YELLOW));
}

#[test]
fn what_is_not_drawn_is_reported() {
    let code = prog(&[st(0x0F, 0x2100), st(0x05, 0x2105), st(0x01, 0x2133), cgram_set(0, &[RED]), st(0x11, 0x212C)]);
    let Some((r, img)) = shot("rd-interlace", &code) else { return };
    assert!(r.out.contains("warning: interlace ($2133=01) is not applied"), "{}", r.out);
    assert_eq!(img.unwrap().rgb(4, 4), rgb(RED));

    let code = prog(&[st(0x01, 0x2105), st(0x01, 0x2131), st(0x01, 0x212E), st(0x10, 0x2106)]);
    let Some((r, _)) = shot("rd-math", &code) else { return };
    assert!(!r.out.contains("colour math"), "{}", r.out);
    assert!(!r.out.contains("windows"), "windows are drawn now: {}", r.out);
    assert!(!r.out.contains("graphics mode"), "{}", r.out);

    let code = prog(&[st(0x01, 0x2105), st(0x01, 0x2131), st(0x10, 0x2130)]);
    let Some((r, _)) = shot("rd-math-window", &code) else { return };
    assert!(!r.out.contains("colour window"), "the colour window is applied now: {}", r.out);
}

// ---- Joypad ----

/// Reads `$4016` once and keeps its bit 0 in `$2170 + i`.
fn read_serial(i: u16) -> Vec<u8> {
    [vec![0xAD, 0x16, 0x40, 0x29, 0x01], vec![0x8D, (0x2170 + i) as u8, ((0x2170 + i) >> 8) as u8]].concat()
}

/// The bits `$2170..` hold after the program: `last=0X` of each register.
fn bits(r: &Run, n: u16) -> Vec<u8> {
    (0..n).map(|i| r.reg(0x2170 + i as u32).and_then(|l| l.rsplit("last=").next()).map_or(255, |v| u8::from_str_radix(v, 16).unwrap())).collect()
}

#[test]
fn press_options_are_validated() {
    let code: &[u8] = &[0x80, 0xFE];
    for good in ["START@3-4", "A+RIGHT@10-12", "UP+DOWN+LEFT+RIGHT+SELECT+B+Y+X+L+R@0-0"] {
        let Some(r) = run("hl-press-ok", &[(0x8000, code)], &[0x8000], None, &["rom.sfc", "--press", good]) else { return };
        assert_eq!(r.code, 0, "{good}
{}", r.out);
    }
    for bad in ["FIRE@1-2", "START@5-2", "START", "START@a-b", "START@1", "@1-2", "START+@1-2", "start@1-2"] {
        let Some(r) = run("hl-press-bad", &[(0x8000, code)], &[0x8000], None, &["rom.sfc", "--press", bad]) else { return };
        assert_eq!(r.code, 2, "{bad}
{}", r.out);
    }
}

#[test]
fn the_automatic_read_gives_the_pressed_buttons_at_vblank() {
    // $4200 = 1, then each vblank: LDA $4219 -> $2134 ; LDA $4218 -> $2135
    let code = prog(&[st(0x01, 0x4200), vec![0xAD, 0x12, 0x42, 0x10, 0xFB], mv(0x4219, 0x2134), mv(0x4218, 0x2135), vec![0x80, 0xED]]);
    // The BRA above goes back to the wait loop; the final prog() BRA is never reached.
    let args = ["rom.sfc", "--frames", "4", "--frame-blocks", "50", "--press", "START+RIGHT@0-9", "--press", "A+R@0-9"];
    let Some(r) = run("hl-joy-auto", &[(0x8000, &code)], &[0x8000], None, &args) else { return };
    assert_eq!(r.code, 0, "{}", r.out);
    assert!(r.reg(0x2134).is_some_and(|l| l.ends_with("last=11")), "START (bit 4) and RIGHT (bit 0) in $4219
{}", r.out);
    assert!(r.reg(0x2135).is_some_and(|l| l.ends_with("last=90")), "A (bit 7) and R (bit 4) in $4218
{}", r.out);

    // Nothing is held without the option.
    let args = ["rom.sfc", "--frames", "4", "--frame-blocks", "50"];
    let Some(r) = run("hl-joy-none", &[(0x8000, &code)], &[0x8000], None, &args) else { return };
    assert!(r.reg(0x2134).is_some_and(|l| l.ends_with("last=00")) && r.reg(0x2135).is_some_and(|l| l.ends_with("last=00")), "{}", r.out);

    // The buttons are those of the frame being read: held only in frame 2, they show in the vblank of frame 2.
    let args = ["rom.sfc", "--frames", "5", "--frame-blocks", "50", "--press", "START@2-2"];
    let Some(r) = run("hl-joy-frame", &[(0x8000, &code)], &[0x8000], None, &args) else { return };
    assert!(r.reg(0x2134).is_some_and(|l| l.ends_with("last=00")), "released again by the last frame
{}", r.out);
    assert_eq!(r.reg(0x2134).map(|l| l.contains("writes=")), Some(true));
}

#[test]
fn the_automatic_read_needs_the_enable_bit_and_keeps_the_last_value() {
    // $4200 is never written: the registers stay at 0 although START is held.
    let code = prog(&[vec![0xAD, 0x12, 0x42, 0x10, 0xFB], mv(0x4219, 0x2134), vec![0x80, 0xF3]]);
    let args = ["rom.sfc", "--frames", "3", "--frame-blocks", "50", "--press", "START@0-9"];
    let Some(r) = run("hl-joy-off", &[(0x8000, &code)], &[0x8000], None, &args) else { return };
    assert!(r.reg(0x2134).is_some_and(|l| l.ends_with("last=00")), "{}", r.out);

    // Enabled during the first frames and disabled later (writing $4200 = 0 in the second wait): the last
    // value read is kept.
    let code = prog(&[
        st(0x01, 0x4200),
        vec![0xAD, 0x12, 0x42, 0x10, 0xFB],
        st(0x00, 0x4200),
        vec![0xAD, 0x12, 0x42, 0x30, 0xFB], // wait for vblank to end
        vec![0xAD, 0x12, 0x42, 0x10, 0xFB], // and for the next one
        mv(0x4219, 0x2134),
    ]);
    let args = ["rom.sfc", "--frames", "4", "--frame-blocks", "50", "--press", "START@0-0"];
    let Some(r) = run("hl-joy-keep", &[(0x8000, &code)], &[0x8000], None, &args) else { return };
    assert!(r.reg(0x2134).is_some_and(|l| l.ends_with("last=10")), "{}", r.out);
}

#[test]
fn bit_0_of_hvbjoy_is_set_only_at_the_start_of_vblank() {
    // Right after reset: LDA $4212 ; AND #$01 -> $2135. Then wait for vblank and do the same -> $2136.
    let code = prog(&[
        vec![0xAD, 0x12, 0x42, 0x29, 0x01, 0x8D, 0x35, 0x21],
        vec![0xAD, 0x12, 0x42, 0x10, 0xFB, 0x29, 0x01, 0x8D, 0x36, 0x21],
    ]);
    let args = ["rom.sfc", "--frames", "2", "--frame-blocks", "50"];
    let Some(r) = run("hl-busy", &[(0x8000, &code)], &[0x8000], None, &args) else { return };
    assert!(r.reg(0x2135).is_some_and(|l| l.ends_with("last=00")), "{}", r.out);
    assert!(r.reg(0x2136).is_some_and(|l| l.ends_with("last=01")), "{}", r.out);
}

#[test]
fn mode_7_multiplication_reads_back_the_signed_product_of_a_and_the_last_b_byte() {
    // A = $0300 (768), B = $FE00 (last byte written -2): the product -1536 is $FFFA00 at $2134-$2136.
    let wr = |v: u8, to: u16| vec![0xA9, v, 0x8D, to as u8, (to >> 8) as u8];
    let ld = |r: u16, to: u16| vec![0xAD, r as u8, (r >> 8) as u8, 0x8D, to as u8, (to >> 8) as u8];
    let code = prog(&[
        wr(0x00, 0x211B),
        wr(0x03, 0x211B),
        wr(0x00, 0x211C),
        wr(0xFE, 0x211C),
        ld(0x2134, 0x2170),
        ld(0x2135, 0x2171),
        ld(0x2136, 0x2172),
    ]);
    let args = ["rom.sfc", "--frames", "1", "--frame-blocks", "50"];
    let Some(r) = run("hl-m7-mul", &[(0x8000, &code)], &[0x8000], None, &args) else { return };
    assert_eq!(r.code, 0, "{}", r.out);
    assert_eq!(bits(&r, 3), [0x00, 0xFA, 0xFF], "{}", r.out);
}

#[test]
fn the_hv_latch_gives_the_line_low_byte_then_bit_0_and_resets_with_2137() {
    // $2137 ; $213D -> $2170 ; $213D -> $2171 ; $213F (bit 6) -> $2172 ; $213F -> $2173 ; $213D -> $2174
    let ld = |r: u16, to: u16| vec![0xAD, r as u8, (r >> 8) as u8, 0x8D, to as u8, (to >> 8) as u8];
    let code = prog(&[
        vec![0xAD, 0x37, 0x21],
        ld(0x213D, 0x2170),
        ld(0x213D, 0x2171),
        [vec![0xAD, 0x3F, 0x21, 0x29, 0x40], vec![0x8D, 0x72, 0x21]].concat(),
        [vec![0xAD, 0x3F, 0x21, 0x29, 0x40], vec![0x8D, 0x73, 0x21]].concat(),
        ld(0x213D, 0x2174),
    ]);
    let args = ["rom.sfc", "--frames", "1", "--frame-blocks", "2620"];
    let Some(r) = run("hl-hv", &[(0x8000, &code)], &[0x8000], None, &args) else { return };
    assert_eq!(r.code, 0, "{}", r.out);
    let b = bits(&r, 5);
    assert!(b[0] <= 1, "the program runs in line 0 of 262 (10 blocks per line): {b:?}
{}", r.out);
    assert_eq!(b[1], 0, "bit 0 of the V counter: {b:?}");
    assert_eq!(b[2], 0x40, "latch pending");
    assert_eq!(b[3], 0, "latch cleared by the previous $213F");
    assert!(b[4] <= 1, "alternation restarted: low byte again {b:?}");
}

#[test]
fn a_line_wait_through_the_hv_counter_ends() {
    // $2137 ; LDA $213D ; CMP #$80 ; BCC back ; then STA $2170 (marker)
    let code = prog(&[vec![0xAD, 0x37, 0x21, 0xAD, 0x3D, 0x21, 0xC9, 0x80, 0x90, 0xF6], st(0x55, 0x2170)]);
    let args = ["rom.sfc", "--frames", "1", "--frame-blocks", "2620"];
    let Some(r) = run("hl-hv-wait", &[(0x8000, &code)], &[0x8000], None, &args) else { return };
    assert!(r.reg(0x2170).is_some_and(|l| l.ends_with("last=55")), "{}", r.out);
}

#[test]
fn the_serial_read_gives_the_buttons_in_order_and_then_ones() {
    // Latch ($4016 = 1 then 0), then 18 reads.
    let mut parts = vec![st(1, 0x4016), st(0, 0x4016)];
    parts.extend((0..18).map(read_serial));
    let code = prog(&parts);
    let args = ["rom.sfc", "--frames", "1", "--frame-blocks", "400", "--press", "START+A@0-9"];
    let Some(r) = run("hl-joy-serial", &[(0x8000, &code)], &[0x8000], None, &args) else { return };
    assert_eq!(r.code, 0, "{}", r.out);
    // B Y SELECT START UP DOWN LEFT RIGHT A X L R, four zeros, then ones.
    assert_eq!(bits(&r, 18), [0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 1, 1], "{}", r.out);

    // While the latch is 1 the read is always the B button; after it goes to 0 the register shifts.
    let mut parts = vec![st(1, 0x4016), read_serial(0), read_serial(1), st(0, 0x4016), read_serial(2), read_serial(3)];
    parts.push(vec![]);
    let code = prog(&parts);
    let args = ["rom.sfc", "--frames", "1", "--frame-blocks", "400", "--press", "B@0-9"];
    let Some(r) = run("hl-joy-latch", &[(0x8000, &code)], &[0x8000], None, &args) else { return };
    assert_eq!(bits(&r, 4), [1, 1, 1, 0], "{}", r.out);
}

// ---- Execution trace ----

/// The lines of the run that start with `trace `.
fn trace_lines(r: &Run) -> Vec<&str> {
    r.out.lines().filter(|l| l.starts_with("trace ")).collect()
}

#[test]
fn trace_reads_names_the_block_register_and_value() {
    let code = prog(&[st(0x33, 0x2105), mv(0x2105, 0x2120)]);
    let args = ["rom.sfc", "--frames", "2", "--frame-blocks", "50", "--trace-frames", "0-1", "--trace-reads"];
    let Some(r) = run("hl-trace-read", &[(0x8000, &code)], &[0x8000], None, &args) else { return };
    assert_eq!(r.code, 0, "{}", r.out);
    assert_eq!(trace_lines(&r), ["trace read frame 0 block 00:8000 reg $2105 = 33 x1"], "{}", r.out);
}

#[test]
fn trace_only_prints_inside_the_window_and_groups_repeats() {
    // LDA $2105 ; BRA back: the same block reads the same value over and over.
    let code: &[u8] = &[0xAD, 0x05, 0x21, 0x80, 0xFB];
    let args = ["rom.sfc", "--frames", "5", "--frame-blocks", "50", "--trace-frames", "2-3", "--trace-reads"];
    let Some(r) = run("hl-trace-window", &[(0x8000, code)], &[0x8000], None, &args) else { return };
    assert_eq!(r.code, 0, "{}", r.out);
    let lines = trace_lines(&r);
    assert_eq!(lines.len(), 2, "{}", r.out);
    for (line, frame) in lines.iter().zip([2, 3]) {
        assert!(line.starts_with(&format!("trace read frame {frame} block 00:8000 reg $2105 = 00 x")), "{line}");
        let n: u32 = line.rsplit('x').next().unwrap().parse().unwrap();
        assert!(n >= 10, "{line}");
    }

    // Without a window nothing is printed.
    let args = ["rom.sfc", "--frames", "5", "--frame-blocks", "50", "--trace-reads"];
    let Some(r) = run("hl-trace-nowin", &[(0x8000, code)], &[0x8000], None, &args) else { return };
    assert_eq!(r.code, 0, "{}", r.out);
    assert!(trace_lines(&r).is_empty(), "{}", r.out);
}

#[test]
fn trace_does_not_attribute_dma_reads_to_the_block() {
    // A channel from the B bus to WRAM reads $2105 itself.
    let code = prog(&[channel(0, 0x80, 0x05, 0x0100, 4), st(0x01, 0x420B)]);
    let args = ["rom.sfc", "--frames", "1", "--frame-blocks", "50", "--trace-frames", "0-0", "--trace-reads"];
    let Some(r) = run("hl-trace-dma", &[(0x8000, &code)], &[0x8000], None, &args) else { return };
    assert_eq!(r.code, 0, "{}", r.out);
    assert!(r.reg(0x420B).is_some() && trace_lines(&r).is_empty(), "{}", r.out);
}

#[test]
fn watch_reports_changes_of_a_wram_byte_and_its_mirror() {
    let code = prog(&[
        st(0x02, 0x0011),
        st(0x03, 0x0011),
        st(0x03, 0x0011), // same value: not reported
        vec![0xA9, 0x05, 0x8F, 0x11, 0x00, 0x7E], // LDA #$05 ; STA $7E0011
    ]);
    for addr in ["0011", "7E:0011", "00011"] {
        let args = ["rom.sfc", "--frames", "1", "--frame-blocks", "50", "--trace-frames", "0-0", "--watch", addr];
        let Some(r) = run("hl-watch", &[(0x8000, &code)], &[0x8000], None, &args) else { return };
        assert_eq!(r.code, 0, "{}", r.out);
        let want = [
            "trace write frame 0 block 00:8000 wram 00011: 00 -> 02",
            "trace write frame 0 block 00:8000 wram 00011: 02 -> 03",
            "trace write frame 0 block 00:8000 wram 00011: 03 -> 05",
        ];
        assert_eq!(trace_lines(&r), want, "--watch {addr}
{}", r.out);
    }
}

#[test]
fn watch_sees_dma_writes_with_the_block_that_started_them() {
    let code = prog(&[
        st(0x11, 0x2181),
        st(0x00, 0x2182),
        st(0x00, 0x2183), // port at $000011
        channel(0, 0x00, 0x80, 0x8100, 1),
        st(0x01, 0x420B),
    ]);
    let args = ["rom.sfc", "--frames", "1", "--frame-blocks", "50", "--trace-frames", "0-0", "--watch", "11"];
    let Some(r) = run("hl-watch-dma", &[(0x8000, &code), (0x8100, &DATA)], &[0x8000], None, &args) else { return };
    assert_eq!(r.code, 0, "{}", r.out);
    assert_eq!(trace_lines(&r), ["trace write frame 0 block 00:8000 wram 00011: 00 -> 11"], "{}", r.out);
}

#[test]
fn trace_options_are_validated() {
    let code: &[u8] = &[0x80, 0xFE];
    for bad in [["--trace-frames", "5"], ["--trace-frames", "5-2"], ["--watch", "20000"], ["--watch", "7D:0011"]] {
        let args = ["rom.sfc", bad[0], bad[1]];
        let Some(r) = run("hl-trace-bad", &[(0x8000, code)], &[0x8000], None, &args) else { return };
        assert_eq!(r.code, 2, "{bad:?}
{}", r.out);
    }
}

#[test]
fn poke_writes_wram_bytes_when_the_frame_starts() {
    // NOP ; LDA $0020 ; STA $2134 ; LDA $7F0001 ; STA $2135 ; BRA to the start
    let code: &[u8] = &[0xEA, 0xAD, 0x20, 0x00, 0x8D, 0x34, 0x21, 0xAF, 0x01, 0x00, 0x7F, 0x8D, 0x35, 0x21, 0x80, 0xF1];
    let before = ["rom.sfc", "--frames", "2", "--frame-blocks", "50", "--poke", "20=5A@5", "--poke", "7F:0000=1122@5"];
    let Some(r) = run("hl-poke-late", &[(0x8000, code)], &[0x8000], None, &before) else { return };
    assert_eq!(r.code, 0, "{}", r.out);
    assert!(r.reg(0x2134).is_some_and(|l| l.ends_with("last=00")) && r.reg(0x2135).is_some_and(|l| l.ends_with("last=00")), "{}", r.out);
    let after = ["rom.sfc", "--frames", "6", "--frame-blocks", "50", "--poke", "20=5A@5", "--poke", "7F:0000=1122@5"];
    let Some(r) = run("hl-poke", &[(0x8000, code)], &[0x8000], None, &after) else { return };
    assert_eq!(r.code, 0, "{}", r.out);
    assert!(r.reg(0x2134).is_some_and(|l| l.ends_with("last=5A")), "{}", r.out);
    assert!(r.reg(0x2135).is_some_and(|l| l.ends_with("last=22")), "second byte goes to the next address: {}", r.out);
    for bad in ["20=5A", "20=5@3", "20=@3", "20@3", "=5A@3", "7D:0000=5A@3", "20000=5A@3", "1FFFF=5A11@3", "20=5A@x", "20=5A@3x"] {
        let Some(r) = run("hl-poke-bad", &[(0x8000, code)], &[0x8000], None, &["rom.sfc", "--poke", bad]) else { return };
        assert_eq!(r.code, 2, "{bad}\n{}", r.out);
    }
}

/// Bytes stored from WRAM `$0300` on (the code writes them before enabling HDMA).
fn wram_bytes(at: u16, bytes: &[u8]) -> Vec<u8> {
    bytes.iter().enumerate().flat_map(|(i, &b)| st(b, at + i as u16)).collect()
}

/// HDMA channel 0 from WRAM `$0300` to register `$21xx` with control byte `ctl`.
fn hdma_channel0(ctl: u8, reg: u8) -> Vec<u8> {
    [st(ctl, 0x4300), st(reg, 0x4301), st(0x00, 0x4302), st(0x03, 0x4303), st(0x00, 0x4304), st(0x00, 0x4307), st(0x01, 0x420C)].concat()
}

#[test]
fn hdma_changes_a_register_per_group_of_lines() {
    // Backdrop only, the colour math adds the fixed colour to it; HDMA sets the fixed colour for 10 lines each:
    // red, none, green. A zero header ends the table.
    let table = wram_bytes(0x0300, &[0x0A, 0x20 | 31, 0x0A, 0x20, 0x0A, 0x40 | 31, 0x00]);
    let code = prog(&[ppu_base(0x00), st(0x20, 0x2131), table, hdma_channel0(0x00, 0x32)]);
    let Some((r, img)) = shot("rd-hdma-direct", &code) else { return };
    assert_eq!(r.code, 0, "{}", r.out);
    assert!(r.out.contains("hdma line transfers:"), "{}", r.out);
    let img = img.unwrap();
    assert_eq!(img.rgb(5, 0), rgb(RED));
    assert_eq!(img.rgb(5, 9), rgb(RED));
    assert_eq!(img.rgb(5, 10), rgb(0));
    assert_eq!(img.rgb(5, 19), rgb(0));
    assert_eq!(img.rgb(5, 20), rgb(GREEN));
    // After the last group the register keeps its value.
    assert_eq!(img.rgb(5, 100), rgb(GREEN));
}

#[test]
fn hdma_indirect_tables_repeat_one_value_per_line() {
    // Indirect: header $83 (repeat, 3 lines) and a pointer to $0310, where there is one byte per line.
    let table = [wram_bytes(0x0300, &[0x83, 0x10, 0x03, 0x00]), wram_bytes(0x0310, &[0x20 | 31, 0x20, 0x40 | 31])].concat();
    let code = prog(&[ppu_base(0x00), st(0x20, 0x2131), table, hdma_channel0(0x40, 0x32)]);
    let Some((r, img)) = shot("rd-hdma-indirect", &code) else { return };
    assert_eq!(r.code, 0, "{}", r.out);
    let img = img.unwrap();
    assert_eq!(img.rgb(5, 0), rgb(RED));
    assert_eq!(img.rgb(5, 1), rgb(0));
    assert_eq!(img.rgb(5, 2), rgb(GREEN));
    assert_eq!(img.rgb(5, 3), rgb(GREEN));
}

#[test]
fn hdma_can_scroll_a_background_line_by_line() {
    // BG1 has a one pixel wide bar at the left of tile 1, used by map entries 0 and 1 (bars at x = 0 and 8).
    // HDMA writes BG1HOFS (two bytes, mode 2 to $210D): 0 for line 0, then 3 for the rest of the screen.
    let bar: Vec<(usize, usize, u8)> = (0..8).map(|y| (0, y, 1)).collect();
    let table = wram_bytes(0x0300, &[0x01, 0x00, 0x00, 0x01, 0x03, 0x00, 0x00]);
    let code = prog(&[
        ppu_base(0x01),
        vram_words(16, &tile4(&bar)),
        vram_words(0x1000, &[0x0001, 0x0001]),
        cgram_set(0, &[0, GREEN]),
        table,
        hdma_channel0(0x02, 0x0D),
    ]);
    let Some((r, img)) = shot("rd-hdma-scroll", &code) else { return };
    assert_eq!(r.code, 0, "{}", r.out);
    let img = img.unwrap();
    assert_eq!((img.rgb(0, 0), img.rgb(8, 0), img.rgb(5, 0)), (rgb(GREEN), rgb(GREEN), rgb(0)), "line 0 has no scroll");
    assert_eq!((img.rgb(0, 1), img.rgb(8, 1), img.rgb(5, 1)), (rgb(0), rgb(0), rgb(GREEN)), "from line 1 the bars move 3 pixels left");
    assert_eq!(img.rgb(5, 7), rgb(GREEN), "the map row 0 covers lines 0-7");
}

/// BG1 with four solid green tiles (x 0-31, lines 0-7), full brightness, and `extra` set up before the screenshot.
fn window_case(name: &str, extra: Vec<Vec<u8>>) -> Option<Img> {
    let solid: Vec<(usize, usize, u8)> = (0..8).flat_map(|y| (0..8).map(move |x| (x, y, 1))).collect();
    let code = prog(&[
        ppu_base(0x01),
        vram_words(16, &tile4(&solid)),
        vram_words(0x1000, &[1, 1, 1, 1]),
        cgram_set(0, &[0, GREEN]),
        extra.concat(),
    ]);
    let (r, img) = shot(&format!("rd-win-{name}"), &code)?;
    assert_eq!(r.code, 0, "{}", r.out);
    img
}

/// Window 1 = [l1, r1], window 2 = [l2, r2].
fn windows(l1: u8, r1: u8, l2: u8, r2: u8) -> Vec<u8> {
    [st(l1, 0x2126), st(r1, 0x2127), st(l2, 0x2128), st(r2, 0x2129)].concat()
}

/// The x positions 0-31 at line 0 where BG1 shows (green).
fn shown(img: &Img) -> Vec<usize> {
    (0..32).filter(|&x| img.rgb(x, 0) == rgb(GREEN)).collect()
}

#[test]
fn a_window_hides_a_layer_inside_it_or_outside_when_inverted() {
    // W1 = [8,15] on BG1, enabled for the main screen.
    let Some(img) = window_case("inside", vec![windows(8, 15, 0, 0), st(0x02, 0x2123), st(0x01, 0x212E)]) else { return };
    assert_eq!(shown(&img), (0..8).chain(16..32).collect::<Vec<_>>());
    // Inverted: the layer shows only inside.
    let Some(img) = window_case("inverted", vec![windows(8, 15, 0, 0), st(0x03, 0x2123), st(0x01, 0x212E)]) else { return };
    assert_eq!(shown(&img), (8..16).collect::<Vec<_>>());
    // The window is defined but the layer is not in $212E: nothing is hidden.
    let Some(img) = window_case("unmasked", vec![windows(8, 15, 0, 0), st(0x02, 0x2123)]) else { return };
    assert_eq!(shown(&img), (0..32).collect::<Vec<_>>());
    // $212E on, but the window not enabled for the layer: nothing is hidden.
    let Some(img) = window_case("not-enabled", vec![windows(8, 15, 0, 0), st(0x01, 0x212E)]) else { return };
    assert_eq!(shown(&img), (0..32).collect::<Vec<_>>());
    // A window whose left is past its right is empty.
    let Some(img) = window_case("empty", vec![windows(20, 10, 0, 0), st(0x02, 0x2123), st(0x01, 0x212E)]) else { return };
    assert_eq!(shown(&img), (0..32).collect::<Vec<_>>());
}

#[test]
fn two_windows_combine_with_the_chosen_logic() {
    // W1 = [0,15], W2 = [8,23], both enabled for BG1: $2123 = 0x0A. The logic is in $212A bits 0-1.
    let case = |logic: u8, name: &str| {
        let img = window_case(name, vec![windows(0, 15, 8, 23), st(0x0A, 0x2123), st(logic, 0x212A), st(0x01, 0x212E)])?;
        Some(shown(&img))
    };
    let Some(or) = case(0, "or") else { return };
    assert_eq!(or, (24..32).collect::<Vec<_>>(), "OR hides 0-23");
    let Some(and) = case(1, "and") else { return };
    assert_eq!(and, (0..8).chain(16..32).collect::<Vec<_>>(), "AND hides 8-15");
    let Some(xor) = case(2, "xor") else { return };
    assert_eq!(xor, (8..16).chain(24..32).collect::<Vec<_>>(), "XOR hides 0-7 and 16-23");
    let Some(xnor) = case(3, "xnor") else { return };
    assert_eq!(xnor, (0..8).chain(16..24).collect::<Vec<_>>(), "XNOR hides 8-15 and 24-31");
}

#[test]
fn the_colour_window_chooses_where_colour_math_applies_and_where_the_main_screen_goes_black() {
    // The backdrop gets the fixed red added; the colour window is W1 = [8,15] ($2125 bits 4-5 enable it).
    let math = |sel: u8, name: &str| {
        let extra = vec![windows(8, 15, 0, 0), st(0x20, 0x2125), st(0x20, 0x2131), st(0x20 | 31, 0x2132), st(sel, 0x2130)];
        let img = window_case(name, extra)?;
        // Line 20 has no BG1 tile, so the backdrop shows there.
        Some((img.rgb(4, 20), img.rgb(12, 20)))
    };
    let Some(r) = math(0x10, "math-inside") else { return };
    assert_eq!(r, (rgb(0), rgb(RED)), "math only inside the window");
    let Some(r) = math(0x20, "math-outside") else { return };
    assert_eq!(r, (rgb(RED), rgb(0)), "math only outside the window");
    let Some(r) = math(0x00, "math-always") else { return };
    assert_eq!(r, (rgb(RED), rgb(RED)));
    // Forcing the main screen to black outside the window: BG1 keeps its green only inside.
    let extra = vec![windows(8, 15, 0, 0), st(0x20, 0x2125), st(0x40, 0x2130)];
    let Some(img) = window_case("black-outside", extra) else { return };
    assert_eq!(shown(&img), (8..16).collect::<Vec<_>>());
}

#[test]
fn mosaic_repeats_the_top_left_pixel_of_each_block() {
    // BG1 tile 1 has one pixel, (0,1). Mosaic of size 4 ($2106 = $31): blocks of 4x4, the vertical ones start at line 1,
    // so lines 1-4 all show row 1 and the horizontal blocks repeat the pixel at x = 0 over x = 0-3.
    let case = |reg: u8, name: &str| {
        let code = prog(&[
            ppu_base(0x01),
            vram_words(16, &tile4(&[(0, 1, 1)])),
            vram_words(0x1000, &[0x0001]),
            cgram_set(0, &[0, GREEN]),
            st(reg, 0x2106),
        ]);
        let (r, img) = shot(name, &code)?;
        assert_eq!(r.code, 0, "{}", r.out);
        assert!(!r.out.contains("mosaic"), "{}", r.out);
        img
    };
    let Some(img) = case(0x31, "rd-mosaic-on") else { return };
    for (x, y, green) in [(0, 1, true), (3, 1, true), (3, 4, true), (4, 1, false), (0, 5, false), (0, 0, false)] {
        assert_eq!(img.rgb(x, y) == rgb(GREEN), green, "pixel ({x},{y})");
    }
    // Size bits set but BG1 not enabled for the mosaic: the single pixel is alone.
    let Some(img) = case(0x30, "rd-mosaic-off") else { return };
    assert_eq!((img.rgb(0, 1) == rgb(GREEN), img.rgb(1, 1) == rgb(GREEN), img.rgb(0, 2) == rgb(GREEN)), (true, false, false));
}

/// Mode 7 with BG1 shown: the matrix a, b, c, d (8.8 fixed), `extra` writes, one tile (number 1) whose pixel (1,1) is
/// colour 1 (green) at map cell (0,0), tile 0 with the same pixel, and `$211A` = `sel`.
fn mode7_case(name: &str, sel: u8, matrix: [u16; 4], extra: Vec<Vec<u8>>) -> Option<Img> {
    let two = |v: u16, reg: u16| [st(v as u8, reg), st((v >> 8) as u8, reg)].concat();
    // VRAM word w: low byte = map entry w, high byte = pixel data of tile (w / 64) at position w % 64.
    let code = prog(&[
        st(0x0F, 0x2100),
        st(0x07, 0x2105),
        st(0x01, 0x212C),
        st(sel, 0x211A),
        two(matrix[0], 0x211B),
        two(matrix[1], 0x211C),
        two(matrix[2], 0x211D),
        two(matrix[3], 0x211E),
        cgram_set(0, &[0, GREEN]),
        vram_words(0, &[0x0001]),
        vram_words(64 + 9, &[0x0100]),
        vram_words(9, &[0x0100]),
        extra.concat(),
    ]);
    let (r, img) = shot(&format!("rd-m7-{name}"), &code)?;
    assert_eq!(r.code, 0, "{}", r.out);
    assert!(!r.out.contains("graphics mode"), "{}", r.out);
    img
}

#[test]
fn mode_7_draws_the_background_through_the_matrix() {
    let green = |img: &Img, x: usize, y: usize| img.rgb(x, y) == rgb(GREEN);
    // Identity: texture pixel (1,1) shows at screen (1,1).
    let Some(img) = mode7_case("identity", 0, [0x100, 0, 0, 0x100], vec![]) else { return };
    assert!(green(&img, 1, 1) && !green(&img, 0, 0) && !green(&img, 2, 2) && !green(&img, 2, 1));
    // Half step: every texture pixel covers 2x2 screen pixels.
    let Some(img) = mode7_case("zoom", 0, [0x80, 0, 0, 0x80], vec![]) else { return };
    assert!(green(&img, 2, 2) && green(&img, 3, 3) && green(&img, 2, 3) && !green(&img, 1, 1) && !green(&img, 4, 4));
    // Flip X on the screen: x is 255 - x, so the texture pixel 1 is at screen x = 254.
    let Some(img) = mode7_case("flip-x", 1, [0x100, 0, 0, 0x100], vec![]) else { return };
    assert!(green(&img, 254, 1) && !green(&img, 1, 1));
    // The scroll moves the picture: BG1HOFS = 1 shifts it one pixel left, so the pixel shows at x = 0.
    let scroll = vec![st(0x01, 0x210D), st(0x00, 0x210D)];
    let Some(img) = mode7_case("scroll", 0, [0x100, 0, 0, 0x100], scroll) else { return };
    assert!(green(&img, 0, 1) && !green(&img, 1, 1));
    // Outside the 1024x1024 area (BG1HOFS = -64 puts screen x = 1 at texture x = -63): bit 7 of $211A makes it
    // transparent; otherwise the map wraps (tile 0 there, with its own green pixel at (1,1)) or tile 0 fills it (bit 6).
    let left = vec![st(0xC0, 0x210D), st(0x1F, 0x210D)];
    for (sel, name, want) in [(0x00, "wrap", true), (0x80, "transparent", false), (0xC0, "tile0", true)] {
        let Some(img) = mode7_case(name, sel, [0x100, 0, 0, 0x100], left.clone()) else { return };
        assert_eq!(green(&img, 1, 1), want, "$211A = {sel:02X}");
    }
}

#[test]
fn mode_7_shares_the_priority_with_sprites() {
    // BG1 green pixel at (1,1), and sprite 0 (blue pixel at its (0,0)) at (1,0): the sprite goes over BG1 only with priority >= 2.
    let case = |pri: u8, name: &str| {
        let code = prog(&[
            st(0x0F, 0x2100),
            st(0x07, 0x2105),
            st(0x11, 0x212C),
            [st(0x00, 0x211B), st(0x01, 0x211B), st(0x00, 0x211C), st(0x00, 0x211C), st(0x00, 0x211D), st(0x00, 0x211D), st(0x00, 0x211E), st(0x01, 0x211E)].concat(),
            cgram_set(0, &[0, GREEN]),
            vram_words(0, &[0x0001]),
            vram_words(64 + 9, &[0x0100]),
            st(0x01, 0x2101),
            vram_words(0x2000 + 16, &tile4(&[(0, 0, 1)])),
            cgram_set(128 + 1, &[BLUE]),
            oam_set(0, &[1, 0, 1, pri << 4]),
        ]);
        let (r, img) = shot(name, &code)?;
        assert_eq!(r.code, 0, "{}", r.out);
        img.map(|i| i.rgb(1, 1))
    };
    let Some(low) = case(1, "rd-m7-spr1") else { return };
    assert_eq!(low, rgb(GREEN), "sprites of priority 1 are behind BG1");
    let Some(high) = case(2, "rd-m7-spr2") else { return };
    assert_eq!(high, rgb(BLUE), "sprites of priority 2 are in front of BG1");
}

#[test]
fn hardware_multiply_and_divide_leave_their_results_in_the_registers() {
    let code = prog(&[
        st(0x25, 0x4202), st(0x37, 0x4203), mv(0x4216, 0x10), mv(0x4217, 0x11), // 37 * 55 = $07F3
        st(0x34, 0x4204), st(0x12, 0x4205), st(7, 0x4206), // $1234 / 7 = 665 rest 5
        mv(0x4214, 0x12), mv(0x4215, 0x13), mv(0x4216, 0x14), mv(0x4217, 0x15),
        st(0, 0x4206), // by zero: quotient $FFFF, remainder = dividend
        mv(0x4214, 0x16), mv(0x4215, 0x17), mv(0x4216, 0x18), mv(0x4217, 0x19),
    ]);
    let Some((r, mem, _)) = run_dump("hl-muldiv", &code, &[], true) else { return };
    assert_eq!(r.code, 0, "{}", r.out);
    assert_eq!(&mem.wram[0x10..0x1A], &[0xF3, 0x07, 0x99, 0x02, 0x05, 0x00, 0xFF, 0xFF, 0x34, 0x12], "{}", r.out);
}

/// The C program that checks `runtime/play_cfg.h` (options, INI, layout, effects, volume, key map) and prints `ok` or
/// the first thing that failed.
const PLAY_CFG_TEST: &str = r##"
#include "play_cfg.h"
#define CHECK(c) do { if (!(c)) { printf("FAIL line %d: %s\n", __LINE__, #c); return 1; } } while (0)

int main(void) {
    Settings s; Ini ini = {0}, back = {0}; Config cfg; char buf[64]; Rect r; int i, n;
    FILE *f;

    /* defaults, an invalid value, an unknown key and the comments */
    f = fopen("t.ini", "w");
    fputs("# my config\nscale = banana\nfilter = bilinear\nmystery = 7\nvolume = 50\nfullscreen = false\n", f);
    fclose(f);
    CHECK(ini_load(&ini, "t.ini") == 1);
    CHECK(cfg_apply(&s, &ini) == 1);
    CHECK(s.v[O_SCALE] == 3);                 /* the default, after the warning */
    CHECK(s.v[O_FILTER] == FILTER_BILINEAR && s.v[O_VOLUME] == 50 && s.v[O_FULLSCREEN] == 0);
    CHECK(s.v[O_ASPECT] == ASPECT_87 && s.v[O_SCALING] == SCALING_INTEGER && s.v[O_OVERSCAN] == 0 && s.v[O_SCANLINES] == 0);

    /* saving keeps the comments and the unknown keys, and writes every option */
    s.v[O_FILTER] = FILTER_SHARP;
    cfg_store(&s, &ini);
    CHECK(ini_save(&ini, "t.ini") == 1);
    CHECK(ini_load(&back, "t.ini") == 1);
    CHECK(!strcmp(back.line[0], "# my config"));
    CHECK(ini_get(&back, "mystery", buf, sizeof buf) && !strcmp(buf, "7"));
    CHECK(ini_get(&back, "filter", buf, sizeof buf) && !strcmp(buf, "sharp"));
    for (i = 0; i < O_COUNT; i++) CHECK(ini_find(&back, opt_defs[i].key) >= 0);
    CHECK(cfg_apply(&s, &back) == 0 && s.v[O_FILTER] == FILTER_SHARP);

    /* the defaults file written for a first run reads back as the defaults */
    ini_free(&ini);
    settings_defaults(&s);
    cfg_store(&s, &ini);
    CHECK(cfg_apply(&s, &ini) == 0 && s.v[O_SCALE] == 3 && s.v[O_VOLUME] == 100);

    /* wide: 0 by default, a multiple of 8 (rounded down), and out of 0-96 is invalid */
    settings_defaults(&s);
    CHECK(s.v[O_WIDE] == 0);
    for (i = 0; i < 4; i++) {
        static const char *const text[4] = {"72", "69", "500", "7x"};
        static const int want[4] = {72, 64, 0, 0}, bad[4] = {0, 0, 1, 1};
        int v = -1;
        CHECK(opt_parse(&opt_defs[O_WIDE], text[i], &v) == !bad[i]);
        if (!bad[i]) CHECK(v == want[i]);
    }
    ini_free(&ini);
    ini_add(&ini, "wide = 500");
    CHECK(cfg_apply(&s, &ini) == 1 && s.v[O_WIDE] == 0);

    /* the command line wins over the file for this run, and the file keeps its value */
    settings_defaults(&s);
    config_init(&cfg, &s);
    config_override(&cfg, O_FULLSCREEN, 1);
    CHECK(cfg.eff.v[O_FULLSCREEN] == 1 && cfg.file.v[O_FULLSCREEN] == 0);
    cfg_store(&cfg.file, &ini);
    CHECK(ini_get(&ini, "fullscreen", buf, sizeof buf) && !strcmp(buf, "false"));
    config_user_set(&cfg, O_FULLSCREEN, 1);
    CHECK(cfg.file.v[O_FULLSCREEN] == 1 && !(cfg.cli & 1u << O_FULLSCREEN));

    /* the old .keys file becomes keys of the INI */
    f = fopen("t.keys", "w");
    fputs("# my keys\nSpace = A\nReturn = START\nZ = A\nbogus line\nQ = NOPE\n", f);
    fclose(f);
    ini_free(&ini);
    n = keys_migrate(&ini, "t.keys");
    CHECK(n == 3);
    CHECK(ini_get(&ini, "p1.key.A", buf, sizeof buf) && !strcmp(buf, "Space, Z"));
    CHECK(ini_get(&ini, "p1.key.START", buf, sizeof buf) && !strcmp(buf, "Return"));
    CHECK(keys_migrate(&ini, "missing.keys") == -1);

    /* layout */
    r = view_rect(1920, 1080, ASPECT_43, SCALING_FIT, 224, 256);
    CHECK(r.w == 1440 && r.h == 1080 && r.x == 240 && r.y == 0);
    r = view_rect(1000, 900, ASPECT_87, SCALING_INTEGER, 224, 256);
    CHECK(r.w == 768 && r.h == 672 && r.x == 116 && r.y == 114);
    r = view_rect(1000, 900, ASPECT_87, SCALING_INTEGER, 208, 256);
    CHECK(r.w == 768 && r.h == 624);
    r = view_rect(1000, 900, ASPECT_87, SCALING_FIT, 224, 256);
    CHECK(r.w == 1000 && r.h == 875 && r.y == 12);
    r = view_rect(200, 100, ASPECT_87, SCALING_INTEGER, 224, 256);  /* not even 1x fits: fitted */
    CHECK(r.h == 100 && r.w == 114);
    r = view_rect(1920, 1080, ASPECT_87, SCALING_INTEGER, 224, 400);  /* wide = 72: 4x of 400 x 224 */
    CHECK(r.w == 1600 && r.h == 896 && r.x == 160 && r.y == 92);
    r = view_rect(800, 600, ASPECT_FREE, SCALING_INTEGER, 224, 256);
    CHECK(r.x == 0 && r.y == 0 && r.w == 800 && r.h == 600);

    /* effects */
    {
        uint8_t px[8 * 4 * 4];
        memset(px, 0xAA, sizeof px);
        CHECK(overlay_build(px, 8, 4, 4, 2, 0, 0) == 0 && px[0] == 0xAA && px[sizeof px - 1] == 0xAA);
        CHECK(overlay_build(px, 8, 4, 4, 2, 50, 0) == 1);
        CHECK(px[(0 * 8 + 3) * 4 + 3] == 0 && px[(1 * 8 + 3) * 4 + 3] == 127 && px[(2 * 8 + 3) * 4 + 3] == 0 && px[(3 * 8 + 3) * 4 + 3] == 127);
        CHECK(overlay_build(px, 8, 4, 4, 2, 0, 1) == 1);
        CHECK(px[(1 * 8 + 1) * 4 + 3] == 0 && px[(1 * 8 + 2) * 4 + 3] == 100 && px[(2 * 8 + 1) * 4 + 3] == 100 && px[(1 * 8 + 0) * 4 + 3] == 100);
    }

    /* volume */
    CHECK(vol_apply(10000, 50) == 5000 && vol_apply(-10000, 50) == -5000 && vol_apply(12345, 100) == 12345 && vol_apply(12345, 0) == 0);

    /* key map */
    {
        Bind b[8]; int nb = 0;
        CHECK(bind_assign(b, &nb, 8, BIND_KEY, 44, 0, 0x0080, 0) == 0);        /* Space -> A */
        CHECK(bind_assign(b, &nb, 8, BIND_KEY, 40, 0, 0x1000, 0) == 0);        /* Return -> START */
        CHECK(bind_find(b, nb, BIND_KEY, 44) >= 0 && b[bind_find(b, nb, BIND_KEY, 44)].button == 0x0080);
        CHECK(bind_assign(b, &nb, 8, BIND_KEY, 44, 0, 0x8000, 0) == 1 && nb == 2); /* Space is A's: asks */
        CHECK(bind_assign(b, &nb, 8, BIND_KEY, 44, 0, 0x8000, 1) == 0 && nb == 2); /* replaced: B has it, A has none */
        CHECK(b[bind_find(b, nb, BIND_KEY, 44)].button == 0x8000);
        CHECK(bind_assign(b, &nb, 8, BIND_KEY, 45, 0, 0x8000, 0) == 0 && nb == 2 && bind_find(b, nb, BIND_KEY, 44) < 0); /* B moves to 45 */
        CHECK(bind_assign(b, &nb, 8, BIND_PAD, 44, 1, 0x0080, 0) == 0);        /* another device or player does not clash */
        CHECK(bind_assign(b, &nb, 8, BIND_KEY, 45, 0, 0x8000, 0) == 0 && nb == 3); /* the same again changes nothing */
    }
    printf("ok\n");
    return 0;
}
"##;

#[test]
fn the_port_options_logic_reads_saves_lays_out_and_assigns() {
    let cc = crate::need_compiler!();
    match cc.compile_run("play-cfg", &[("play_cfg.h", PLAY_CFG), ("t.c", PLAY_CFG_TEST)], &[]) {
        Ok(out) => assert!(out.trim_end().ends_with("ok"), "{out}"),
        Err(e) => panic!("{e}"),
    }
}

/// The C program that checks `runtime/play_ui.h` (font, text measure and fit, panel pieces) against guard bytes. With an
/// argument it also writes the alphabet and a panel as a PPM picture, to look at.
const PLAY_UI_TEST: &str = r##"
#include "play_ui.h"
#define CHECK(c) do { if (!(c)) { printf("FAIL line %d: %s\n", __LINE__, #c); return 1; } } while (0)
#define W 64
#define H 40
#define GUARD 256

static int guards_ok(const uint8_t *buf, size_t n) {
    size_t i;
    for (i = 0; i < GUARD; i++) if (buf[i] != 0xA5 || buf[GUARD + n + i] != 0xA5) return 0;
    return 1;
}

int main(int argc, char **argv) {
    static uint8_t mem[GUARD + W * H * 3 + GUARD];
    Canvas c; int ch, sum, i, lo, hi; char out[64];
    memset(mem, 0xA5, sizeof mem);
    memset(mem + GUARD, 0, W * H * 3);
    c.px = mem + GUARD; c.w = W; c.h = H;

    /* every printable character has a shape, fits the grid and moves the pen at least 2 pixels */
    for (ch = 32; ch <= 126; ch++) {
        glyph_ink(ch, &lo, &hi);
        CHECK(glyph_adv(ch) >= 2);
        if (ch != ' ') CHECK(hi >= lo && hi - lo + 1 <= 6);
    }
    CHECK(glyph_adv(' ') == 3);
    CHECK(glyph_adv('i') < glyph_adv('W') && glyph_adv('l') < glyph_adv('m')); /* proportional */
    /* the measure is the sum of the advances and what is drawn does not go past it */
    for (sum = 0, i = 0; "Hello, Link!"[i]; i++) sum += glyph_adv((unsigned char)"Hello, Link!"[i]);
    CHECK(text_w("Hello, Link!") == sum && text_w("") == 0);
    CHECK(text_w2x("Hi") == 2 * text_w("Hi"));
    memset(c.px, 0, W * H * 3);
    text_draw(&c, 2, 2, "Hello", UI_TEXT, 0);
    { int maxx = -1, x, y; for (y = 0; y < H; y++) for (x = 0; x < W; x++) if (c.px[(y * W + x) * 3]) if (x > maxx) maxx = x;
      CHECK(maxx >= 2 && maxx < 2 + text_w("Hello")); }

    /* fitting: short text is kept, long text ends in "..." and fits, tiny widths still terminate */
    text_fit(out, sizeof out, "Speakers", 100); CHECK(!strcmp(out, "Speakers"));
    text_fit(out, sizeof out, "Realtek High Definition Audio (Front)", 90);
    CHECK(text_w(out) <= 90 && strlen(out) > 3 && !strcmp(out + strlen(out) - 3, "..."));
    text_fit(out, sizeof out, "Realtek High Definition Audio (Front)", 1);
    CHECK(strlen(out) <= 3);
    text_fit(out, 8, "abcdefghijklmnop", 1000); CHECK(strlen(out) < 8);

    /* nothing is written outside the canvas, however the pieces are placed */
    for (i = -40; i < 80; i += 7) {
        text_draw(&c, i, i / 2, "Zelda: A Link", UI_TEXT, 1);
        text_draw2x(&c, i, 3 - i, "Menu", UI_GOLD, UI_SHADOW);
        ui_panel(&c, i, i / 3, 50, 30);
        ui_rule(&c, i, 5, 40);
        ui_bar(&c, i, 9, 30, 6, 50);
        ui_cursor(&c, i, i / 4);
        ui_arrow(&c, i, 20, -1, UI_TEXT); ui_arrow(&c, i + 8, 20, 1, UI_DIM);
        text_center(&c, i, 30, 30, "Mid", UI_TEXT); text_right(&c, i + 40, 12, "Right", UI_TEXT);
    }
    CHECK(guards_ok(mem, W * H * 3));

    /* the panel paints its corners gold and the bar fills in proportion */
    memset(c.px, 0, W * H * 3);
    ui_panel(&c, 0, 0, 40, 30);
    CHECK(c.px[(1 * W + 1) * 3] == 0xF8 && c.px[(1 * W + 1) * 3 + 1] == 0xD8);
    memset(c.px, 0, W * H * 3);
    ui_bar(&c, 0, 0, 52, 6, 50);
    { int x, filled = 0; for (x = 1; x < 51; x++) if (c.px[(2 * W + x) * 3 + 1] > 0x80) filled++; CHECK(filled == 25); }

    if (argc > 1) { /* a picture of the alphabet and a panel, for the eyes */
        enum { PW = 256, PH = 120 };
        static uint8_t pic[PW * PH * 3];
        Canvas p; FILE *f; int x, y;
        p.px = pic; p.w = PW; p.h = PH;
        for (i = 0; i < PW * PH; i++) { pic[i * 3] = 0x30; pic[i * 3 + 1] = 0x60; pic[i * 3 + 2] = 0x30; }
        ui_panel(&p, 4, 4, 248, 112);
        text_draw2x(&p, 14, 10, "OPTIONS", UI_GOLD, UI_SHADOW);
        ui_rule(&p, 12, 28, 232);
        text_draw(&p, 12, 34, "ABCDEFGHIJKLMNOPQRSTUVWXYZ", UI_TEXT, 1);
        text_draw(&p, 12, 46, "abcdefghijklmnopqrstuvwxyz", UI_TEXT, 1);
        text_draw(&p, 12, 58, "0123456789 .,:;!?'\"-+/()[]%&", UI_TEXT, 1);
        ui_cursor(&p, 12, 74); text_draw(&p, 24, 72, "Volume", UI_GOLD, 1); ui_bar(&p, 120, 73, 80, 8, 60);
        text_draw(&p, 24, 84, "Filter", UI_TEXT, 1); ui_arrow(&p, 120, 85, -1, UI_TEXT); text_draw(&p, 128, 84, "bilinear", UI_GOOD, 1); ui_arrow(&p, 180, 85, 1, UI_DIM);
        text_draw(&p, 24, 96, "Realtek High Definition", UI_DIM, 1);
        f = fopen(argv[1], "wb");
        fprintf(f, "P6\n%d %d\n255\n", PW, PH);
        for (y = 0; y < PH; y++) for (x = 0; x < PW; x++) fwrite(pic + (y * PW + x) * 3, 1, 3, f);
        fclose(f);
    }
    printf("ok\n");
    return 0;
}
"##;

#[test]
fn the_menu_font_and_panel_pieces_measure_fit_and_stay_inside_the_canvas() {
    let cc = crate::need_compiler!();
    match cc.compile_run("play-ui", &[("play_ui.h", PLAY_UI), ("t.c", PLAY_UI_TEST)], &[]) {
        Ok(out) => assert!(out.trim_end().ends_with("ok"), "{out}"),
        Err(e) => panic!("{e}"),
    }
}

/// Runs the interactive frontend on a program that only loops, without a window, for `frames` frames with `args` and
/// the files `extra` next to the ROM (a `config.ini`, for one); `--print-config` is added, so the output ends with the
/// lines of the config file as it was left.
fn run_menu(name: &str, args: &[&str], extra: &[(&str, &[u8])]) -> Option<Run> {
    let seg = [(0x8000, [0x80u8, 0xFE].as_slice())];
    let mut all = vec!["rom.sfc", "--no-window", "--frames", "40", "--frame-blocks", "20", "--print-config"];
    all.extend_from_slice(args);
    run_play_with(name, &seg, &[0x8000], &all, extra)
}

/// The lines of the config file in the output of `run_menu`.
fn config_lines(r: &Run) -> Vec<&str> {
    r.out.lines().collect()
}

#[test]
fn the_menu_changes_the_filter_and_saves_it_and_a_first_run_writes_the_defaults() {
    // F1 opens the menu on Image; four Downs reach `filter`; Right makes it bilinear; Escape closes and saves
    let keys = "F1,Down,Down,Down,Down,Right,Escape";
    let Some(r) = run_menu("menu-filter", &["--menu-keys", keys], &[]) else { return };
    assert_eq!(r.code, 0, "{}", r.out);
    let l = config_lines(&r);
    assert!(l.contains(&"filter = bilinear"), "{}", r.out);
    assert!(l.contains(&"scale = 3") && l.contains(&"volume = 100") && l.contains(&"fullscreen = false") && l.contains(&"scaling = integer"), "every option with its default\n{}", r.out);
    assert!(l.iter().any(|x| x.starts_with("# Options of snes_play")), "{}", r.out);
    // closing the menu without changing anything still writes the defaults on the first run
    let Some(r) = run_menu("menu-defaults", &["--menu-keys", "F1,Escape"], &[]) else { return };
    assert!(config_lines(&r).contains(&"filter = nearest"), "{}", r.out);
}

#[test]
fn the_command_line_wins_over_the_config_file_without_changing_it() {
    let ini = b"# mine\nfullscreen = false\nmystery = 7\nfilter = sharp\n";
    let Some(r) = run_menu("menu-precedence", &["--opt", "fullscreen=true", "--opt", "filter=bilinear", "--menu-keys", "F1,Escape"], &[("config.ini", ini)]) else { return };
    assert_eq!(r.code, 0, "{}", r.out);
    let l = config_lines(&r);
    assert!(l.contains(&"fullscreen = false") && l.contains(&"filter = sharp"), "the file keeps its values\n{}", r.out);
    assert!(l.contains(&"# mine") && l.contains(&"mystery = 7"), "comments and unknown keys stay\n{}", r.out);
    let Some(r) = run_menu("menu-badopt", &["--opt", "filter=banana"], &[]) else { return };
    assert_eq!(r.code, 2, "{}", r.out);
}

#[test]
fn the_menu_remaps_a_key_and_asks_before_taking_one_that_is_in_use() {
    // Controls (W twice from Image), Player 1 is already chosen, button A is the ninth line after it, Enter starts the capture
    let to_a = "F1,W,W,Down,Down,Down,Down,Down,Down,Down,Down,Down,Return";
    let Some(r) = run_menu("menu-remap", &["--menu-keys", &format!("{to_a},Space,Escape")], &[]) else { return };
    assert_eq!(r.code, 0, "{}", r.out);
    let l = config_lines(&r);
    assert!(l.contains(&"p1.key.A = Space"), "{}", r.out);
    assert!(l.contains(&"p1.key.START = Return") && l.contains(&"p1.key.B = X"), "{}", r.out);
    // Return is START's: Escape cancels and leaves things as they were
    let Some(r) = run_menu("menu-conflict", &["--menu-keys", &format!("{to_a},Return,Escape,Escape")], &[]) else { return };
    let l = config_lines(&r);
    assert!(l.contains(&"p1.key.A = S") && l.contains(&"p1.key.START = Return"), "{}", r.out);
    // Enter replaces: A gets Return and START is left without a key
    let Some(r) = run_menu("menu-replace", &["--menu-keys", &format!("{to_a},Return,Return,Escape")], &[]) else { return };
    let l = config_lines(&r);
    assert!(l.contains(&"p1.key.A = Return") && l.contains(&"p1.key.START = "), "{}", r.out);
}

#[test]
fn the_game_tab_saves_now_restores_everything_and_quits() {
    // Game is the fourth tab: W three times from Image. Down once is "Save game now".
    let code = [0xA9, 0x5A, 0x8F, 0x00, 0x00, 0x70, 0x80, 0xFE];
    let seg = [(0x8000, code.as_slice())];
    let all = ["rom.sfc", "--no-window", "--frames", "40", "--frame-blocks", "20", "--print-config", "--menu-keys", "F1,W,W,W,Down,Return,Escape"];
    let Some(r) = run_play_with("menu-save-now", &seg, &[0x8000], &all, &[]) else { return };
    assert_eq!(r.code, 0, "{}", r.out);
    let saved = r.out.find("sram: saved").expect(&r.out);
    let config = r.out.find("# Options of snes_play").expect(&r.out);
    assert!(saved < config, "the saved game is written when the menu asks, not when the program ends
{}", r.out);
    // Restore all defaults (Down twice, Enter, Enter to confirm) over a config that changed things
    let ini = b"filter = sharp
volume = 50
scanlines = 40
p1.key.A = Space
";
    let Some(r) = run_menu("menu-restore-all", &["--menu-keys", "F1,W,W,W,Down,Down,Return,Return,Escape"], &[("config.ini", ini)]) else { return };
    let l = config_lines(&r);
    assert!(l.contains(&"filter = nearest") && l.contains(&"volume = 100") && l.contains(&"scanlines = 0") && l.contains(&"p1.key.A = S"), "{}", r.out);
    // Escape instead of the second Enter cancels the restoring
    let Some(r) = run_menu("menu-restore-cancel", &["--menu-keys", "F1,W,W,W,Down,Down,Return,Escape,Escape"], &[("config.ini", ini)]) else { return };
    let l = config_lines(&r);
    assert!(l.contains(&"filter = sharp") && l.contains(&"volume = 50") && l.contains(&"p1.key.A = Space"), "{}", r.out);
    // Quit (the fourth row) ends the program without closing the menu by hand
    let Some(r) = run_menu("menu-quit", &["--menu-keys", "F1,W,W,W,Down,Down,Down,Return"], &[]) else { return };
    assert_eq!(r.code, 0, "{}", r.out);
}

#[test]
fn restoring_a_tab_only_changes_that_tab() {
    let ini = b"filter = sharp
scanlines = 40
volume = 50
";
    // Image: ten rows of options, then "Restore defaults" (ten Downs); Enter; Escape
    let keys = "F1,Down,Down,Down,Down,Down,Down,Down,Down,Down,Down,Return,Escape";
    let Some(r) = run_menu("menu-restore-image", &["--menu-keys", keys], &[("config.ini", ini)]) else { return };
    let l = config_lines(&r);
    assert!(l.contains(&"filter = nearest") && l.contains(&"scanlines = 0") && l.contains(&"volume = 50"), "{}", r.out);
    // Audio: Restore is the third row of the second tab
    let Some(r) = run_menu("menu-restore-audio", &["--menu-keys", "F1,W,Down,Down,Return,Escape"], &[("config.ini", ini)]) else { return };
    let l = config_lines(&r);
    assert!(l.contains(&"volume = 100") && l.contains(&"filter = sharp") && l.contains(&"scanlines = 40"), "{}", r.out);
}

#[test]
fn the_fps_counter_changes_the_picture_only_when_it_is_on() {
    let seg = [(0x8000, [0x80u8, 0xFE].as_slice())];
    let base = ["rom.sfc", "--no-window", "--frames", "5", "--frame-blocks", "20", "--present-shot", "p.png", "--present-size", "256x224"];
    let Some(r) = run_play("play-fps-off", &seg, &[0x8000], &base) else { return };
    assert!(r.out.contains("present-shot: identical to the picture"), "{}", r.out);
    let args: Vec<&str> = base.iter().copied().chain(["--opt", "fps=true"]).collect();
    let Some(r) = run_play("play-fps-on", &seg, &[0x8000], &args) else { return };
    assert!(r.out.contains("present-shot: differs from the picture"), "{}", r.out);
}

#[test]
fn the_menu_picture_is_the_same_every_time_and_goes_through_the_presenter() {
    let seg = [(0x8000, [0x80u8, 0xFE].as_slice())];
    let base = ["rom.sfc", "--no-window", "--frames", "5", "--frame-blocks", "20", "--menu-shot", "m.png", "--menu-page", "2"];
    let hash = |out: &str| out.lines().find(|l| l.starts_with("menu-shot: ")).map(str::to_string);
    let Some(a) = run_play("play-menushot-a", &seg, &[0x8000], &base) else { return };
    let Some(b) = run_play("play-menushot-b", &seg, &[0x8000], &base) else { return };
    assert!(hash(&a.out).is_some() && hash(&a.out) == hash(&b.out), "{}
{}", a.out, b.out);
    let other: Vec<&str> = base.iter().copied().take(base.len() - 1).chain(["3"]).collect();
    let Some(c) = run_play("play-menushot-c", &seg, &[0x8000], &other) else { return };
    assert!(hash(&c.out).is_some() && hash(&c.out) != hash(&a.out), "another tab looks different
{}", c.out);
    // with a filter and the whole window area the menu is presented like the game picture
    let args: Vec<&str> = base.iter().copied().chain(["--present-shot", "p.png", "--present-size", "600x450", "--opt", "filter=sharp", "--opt", "scaling=fit"]).collect();
    let Some(r) = run_play("play-menushot-sharp", &seg, &[0x8000], &args) else { return };
    assert_eq!(r.code, 0, "{}", r.out);
    assert!(r.out.contains("present-shot: 600x450"), "{}", r.out);
}

#[test]
fn the_picture_is_shown_unchanged_without_effects_and_cropped_by_overscan() {
    let seg = [(0x8000, [0x80u8, 0xFE].as_slice())];
    let base = ["rom.sfc", "--no-window", "--frames", "5", "--frame-blocks", "20", "--present-shot", "p.png"];
    let args: Vec<&str> = base.iter().copied().chain(["--present-size", "256x224"]).collect();
    let Some(r) = run_play("play-present", &seg, &[0x8000], &args) else { return };
    assert_eq!(r.code, 0, "{}", r.out);
    assert!(r.out.contains("present-shot: identical to the picture"), "{}", r.out);
    let args: Vec<&str> = base.iter().copied().chain(["--present-size", "256x208", "--opt", "overscan=8"]).collect();
    let Some(r) = run_play("play-overscan", &seg, &[0x8000], &args) else { return };
    assert!(r.out.contains("present-shot: identical to the picture"), "208 lines of the picture\n{}", r.out);
}

#[test]
fn the_documented_options_are_the_ones_of_the_default_config() {
    let Some(r) = run_menu("menu-docs", &["--menu-keys", "F1,Escape"], &[]) else { return };
    assert_eq!(r.code, 0, "{}", r.out);
    let readme = include_str!("../README.md");
    let claude = include_str!("../CLAUDE.md");
    let mut keys = 0;
    for line in config_lines(&r) {
        let Some((key, _)) = line.split_once(" = ").or_else(|| line.strip_suffix(" =").map(|k| (k, ""))) else { continue };
        if line.starts_with('#') {
            continue;
        }
        keys += 1;
        // the key map is documented by its pattern (`p1.key.A`, `p2.pad.B`); the options one by one
        let documented = match key.split('.').collect::<Vec<_>>().as_slice() {
            [p, kind @ ("key" | "pad"), _] if p.starts_with('p') => readme.contains(&format!("`{p}.{kind}.")) || readme.contains("`p1.key.A`, `p2.pad.B`"),
            _ => readme.contains(&format!("| `{key}` |")),
        };
        assert!(documented, "the option `{key}` is not in the README table");
        if !key.contains('.') && key != "audio_device" {
            assert!(claude.contains(key), "the option `{key}` is not in CLAUDE.md");
        }
    }
    assert!(keys > 10 + 48, "the default file lists the options and the key map ({keys} keys)\n{}", r.out);
}

// ---- Wide field (--wide N) ----

/// The same scene at E = 0 and at E = `e`: both pictures, after checking that the columns `e..e+256` of the wide one are the narrow one.
fn wide_pair(name: &str, code: &[u8], e: usize) -> Option<(Img, Img)> {
    let (r, narrow) = shot(&format!("{name}-n"), code)?;
    assert_eq!(r.code, 0, "{}", r.out);
    let (r, wide) = shot_with(&format!("{name}-w"), code, &["--wide", &e.to_string()])?;
    assert_eq!(r.code, 0, "{}", r.out);
    let (narrow, wide) = (narrow.unwrap(), wide.unwrap());
    assert_eq!((narrow.w, wide.w, wide.h), (256, 256 + 2 * e, 224));
    for y in 0..224 {
        for x in 0..256 {
            assert_eq!(wide.rgb(x + e, y), narrow.rgb(x, y), "the picture of the console changed at ({x},{y}) with --wide {e}");
        }
    }
    Some((narrow, wide))
}

#[test]
fn a_wide_screenshot_has_the_columns_asked_for_and_keeps_the_picture_in_the_middle() {
    let code = prog(&[ppu_base(0x01), vram_words(16, &tile4(&[(0, 0, 1)])), vram_words(0x1000, &[0x0001]), cgram_set(0, &[RED, GREEN])]);
    for e in [8usize, 64, 96] {
        let Some((_, wide)) = wide_pair(&format!("wd-size-{e}"), &code, e) else { return };
        // Only one map column holds a tile, so the free columns right of the picture show the backdrop.
        assert_eq!(wide.rgb(wide.w - 1, 100), rgb(RED));
    }
}

#[test]
fn a_bad_wide_value_is_refused_before_running_anything() {
    let code = prog(&[st(0x0F, 0x2100)]);
    for bad in ["5", "104", "-8", "x", ""] {
        let Some(r) = run("wd-bad", &[(0x8000, &code)], &[0x8000], None, &["rom.sfc", "--wide", bad]) else { return };
        assert_eq!(r.code, 2, "--wide {bad:?}: {}", r.out);
        assert!(r.out.contains("--wide"), "{}", r.out);
    }
}

#[test]
fn backgrounds_go_on_through_their_map_in_the_extra_columns() {
    // Tile 1 has colour 1 at (0,0). It sits at the first and at the last column of the 32x32 map of BG1.
    let tile = vram_words(16, &tile4(&[(0, 0, 1)]));
    let map = [vram_words(0x1000, &[0x0001]), vram_words(0x1000 + 31, &[0x0001])].concat();
    let code = prog(&[ppu_base(0x01), tile, map, cgram_set(0, &[RED, GREEN])]);
    let e = 32;
    let Some((_, wide)) = wide_pair("wd-bg", &code, e) else { return };
    assert_eq!(wide.rgb(e, 0), rgb(GREEN), "column 0 of the picture");
    assert_eq!(wide.rgb(e - 8, 0), rgb(GREEN), "the last map column shows left of column 0");
    assert_eq!(wide.rgb(e - 7, 0), rgb(RED));
    assert_eq!(wide.rgb(e + 256, 0), rgb(GREEN), "the first map column shows right of column 255");
    assert_eq!(wide.rgb(e + 257, 0), rgb(RED));
}

#[test]
fn a_mosaic_and_an_offset_per_tile_background_do_not_change_the_picture_of_the_console() {
    // Mosaic 4 and a scroll that is not a multiple of 8.
    let tile = vram_words(16, &tile4(&[(0, 0, 1), (3, 2, 1), (7, 7, 1)]));
    let map = vram_words(0x1000, &(0..64u16).map(|i| if i % 3 == 0 { 0x0001 } else { 0 }).collect::<Vec<_>>());
    let code = prog(&[ppu_base(0x01), st(0x31, 0x2106), tile, map, cgram_set(0, &[RED, GREEN]), st(0x05, 0x210D), st(0x00, 0x210D)]);
    wide_pair("wd-mosaic", &code, 32);
    // Mode 2 with offset per tile: BG3's map moves the columns of BG1.
    let tile = vram_words(16, &tile4(&[(0, 0, 1)]));
    let code = prog(&[
        ppu_base(0x01),
        st(0x02, 0x2105),
        tile,
        vram_words(0x1000, &[0x0001, 0x0001, 0x0001]),
        vram_words(0x1800, &[0x2008, 0x2010, 0x2018]),
        cgram_set(0, &[RED, GREEN]),
        st(0x03, 0x210D),
        st(0x00, 0x210D),
    ]);
    wide_pair("wd-opt", &code, 32);
}

#[test]
fn windows_in_the_extra_columns_take_the_window_of_the_edge_of_the_picture() {
    // BG1 is solid green; its window ($2126/$2127) hides it where the window is.
    let solid = vram_words(0, &tile4(&(0..8).flat_map(|y| (0..8).map(move |x| (x, y, 1u8))).collect::<Vec<_>>()));
    let scene = |l: u8, r: u8| {
        prog(&[ppu_base(0x01), solid.clone(), cgram_set(0, &[RED, GREEN]), st(0x02, 0x2123), st(0x01, 0x212E), st(l, 0x2126), st(r, 0x2127)])
    };
    let e = 16;
    // Window over columns 0-40: the extra columns on the left are inside it, the ones on the right are not.
    let Some((_, wide)) = wide_pair("wd-win-edge", &scene(0, 40), e) else { return };
    assert_eq!(wide.rgb(0, 50), rgb(RED));
    assert_eq!(wide.rgb(e - 1, 50), rgb(RED));
    assert_eq!(wide.rgb(e + 41, 50), rgb(GREEN));
    assert_eq!(wide.rgb(wide.w - 1, 50), rgb(GREEN));
    // Window over columns 100-200: no extra column is inside it.
    let Some((_, wide)) = wide_pair("wd-win-mid", &scene(100, 200), e) else { return };
    assert_eq!(wide.rgb(0, 50), rgb(GREEN));
    assert_eq!(wide.rgb(wide.w - 1, 50), rgb(GREEN));
    assert_eq!(wide.rgb(e + 150, 50), rgb(RED));
}

#[test]
fn a_sprite_left_of_the_picture_is_drawn_whole_in_the_extra_columns() {
    // Sprite 0: 8x8 at X = -16 (9 bits: 496), Y = 20, tile 1 with colour 3 at (0,0) and (7,0), palette 2.
    let code = prog(&[
        ppu_base(0x10),
        sprite_tile(1, &[(0, 0, 3), (7, 0, 3)]),
        cgram_set(128 + 2 * 16 + 3, &[GREEN]),
        cgram_set(0, &[RED]),
        oam_set(0, &[0xF0, 20, 1, 0x04]),
        oam_set(0x100, &[1]),
    ]);
    let e = 32;
    let Some((_, wide)) = wide_pair("wd-spr-left", &code, e) else { return };
    assert_eq!(wide.rgb(e - 16, 21), rgb(GREEN));
    assert_eq!(wide.rgb(e - 9, 21), rgb(GREEN));
    assert_eq!(wide.rgb(e - 17, 21), rgb(RED));
    assert_eq!(wide.rgb(e - 8, 21), rgb(RED));
}

#[test]
fn a_wider_field_loses_the_same_sprite_slices_as_the_picture_of_the_console() {
    // Ten 32x32 sprites 24 pixels apart put 40 slices on a line (the console keeps 34); the eleventh starts in the extra columns.
    let mut oam = Vec::new();
    for i in 0..10u16 {
        oam.extend([(i * 24) as u8, 20, 1, 0x04]);
    }
    oam.extend([0xE8, 20, 1, 0x06]); // X = 488 = -24
    let tiles: Vec<u8> = (1..5).flat_map(|t| sprite_tile(t, &[(0, 0, 3)])).collect();
    let code = prog(&[
        ppu_base(0x10),
        st(0xA1, 0x2101),
        tiles,
        cgram_set(128 + 2 * 16 + 3, &[GREEN]),
        cgram_set(128 + 3 * 16 + 3, &[BLUE]),
        cgram_set(0, &[RED]),
        oam_set(0, &oam),
        oam_set(0x100, &[0, 0, 0x10]),
    ]);
    let Some((narrow, wide)) = wide_pair("wd-spr-limit", &code, 64) else { return };
    let shown = |img: &Img, e: usize| (0..256).filter(|&x| img.rgb(x + e, 21) != rgb(RED)).count();
    assert_eq!(shown(&narrow, 0), shown(&wide, 64));
    assert!(shown(&narrow, 0) < 44, "some slices must have been dropped");
}

#[test]
fn the_c_code_of_a_game_moves_a_sprite_into_the_extra_columns_on_the_right() {
    // Sprite 0 is at X = 0 in the OAM; the hook of the game puts it 256 pixels to the right. Sprite 1 has X = 300 (reads as -212).
    let scene = [
        ppu_base(0x10),
        sprite_tile(1, &[(0, 0, 3)]),
        cgram_set(128 + 2 * 16 + 3, &[GREEN]),
        cgram_set(0, &[RED]),
        oam_set(0, &[0, 20, 1, 0x04, 0x2C, 40, 1, 0x04]),
        oam_set(0x100, &[0b0100]),
    ]
    .concat();
    let with_call = |call: bool| {
        let mut c = scene.clone();
        if call {
            c.extend([0x20, 0x00, 0x81]);
        }
        c.extend([0x80, 0xFE]);
        c
    };
    let hook = "#include \"snes_rt.h\"\nvoid game_hook(Cpu *c) {\n    (void)c;\n    ppu_sprite_xoff(1, 99);\n    ppu_sprite_xoff_clear();\n    ppu_sprite_xoff(0, 256);\n}\n";
    let e = 32;
    let dir = std::env::temp_dir().join(format!("snes-recomp-xoff-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut images = Vec::new();
    for (name, call) in [("with", true), ("without", false)] {
        let path = dir.join(format!("{name}.png")).to_string_lossy().into_owned();
        let code = with_call(call);
        let args = ["rom.sfc", "--frames", "2", "--frame-blocks", "50", "--wide", "32", "--screenshot", path.as_str()];
        let segs: [(u32, &[u8]); 2] = [(0x8000, &code), (0x8100, &[0x60])];
        let Some(r) = run_hooked(&format!("xoff-{name}"), false, &segs, &[0x8000, 0x8100], HOOK_CFG, hook, &args) else { return };
        assert_eq!(r.code, 0, "{}", r.out);
        images.push(decode_png(&std::fs::read(&path).unwrap()));
    }
    let _ = std::fs::remove_dir_all(&dir);
    let (with, without) = (&images[0], &images[1]);
    // With the offset, sprite 0 shows at column 256 of the picture: the first extra column on the right.
    assert_eq!(with.rgb(e + 256, 21), rgb(GREEN));
    assert_eq!(with.rgb(e, 21), rgb(RED), "sprite 0 left its place in the OAM");
    // Without any, nothing changes: sprite 0 is at X = 0 and the one with X = 300 is off to the left, not at 300 - 256.
    assert_eq!(without.rgb(e, 21), rgb(GREEN));
    assert_eq!(without.rgb(e + 256, 21), rgb(RED));
    assert_eq!(without.rgb(e + 44, 41), rgb(RED));
    assert_eq!(with.rgb(e + 44, 41), rgb(RED));
}
