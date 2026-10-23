//! Tests of `runtime/headless.c`: small ROMs are recompiled with `generate_all`, built together with the
//! runtime and run. They skip themselves when there is no C compiler.

use crate::codegen::{Emitter, HEADLESS_RUNTIME, RUNTIME_HEADER};
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
    ];
    files.extend(out.banks.iter().map(|(b, t)| (format!("rom_{b:02X}.c"), t.clone().into_bytes())));
    files.extend(rom.map(|bytes| ("rom.sfc".to_string(), bytes)));
    let files: Vec<(&str, &[u8])> = files.iter().map(|(n, b)| (n.as_str(), b.as_slice())).collect();
    match cc.compile_run_with(name, &files, &["SNES_RT_TICK", "SNES_RT_CHECKS"], args) {
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
        0x8D, 0x34, 0x21, // STA $2134
        0x80, 0xFE,
    ];
    let Some(r) = run("hl-hvbjoy", &[(0x8000, vblank)], &[0x8000], None, &args) else { return };
    assert_eq!(r.code, 0, "{}", r.out);
    assert_eq!(r.reg(0x2133), Some("reads=0 writes=1 last=00"), "{}", r.out);
    assert_eq!(r.reg(0x2134), Some("reads=0 writes=1 last=80"), "{}", r.out);
}

#[test]
fn audio_ports_answer_the_boot_handshake_and_then_echo() {
    let code: &[u8] = &[
        0xAD, 0x40, 0x21, 0x8D, 0x50, 0x21, // $2140 -> $2150 (AA)
        0xAD, 0x41, 0x21, 0x8D, 0x51, 0x21, // $2141 -> $2151 (BB)
        0xA9, 0xCC, 0x8D, 0x40, 0x21, // CC to port 0 starts the transfer
        0xA9, 0x12, 0x8D, 0x41, 0x21, 0xA9, 0x34, 0x8D, 0x42, 0x21,
        0xAD, 0x40, 0x21, 0x8D, 0x52, 0x21, // -> CC
        0xAD, 0x41, 0x21, 0x8D, 0x53, 0x21, // -> 12
        0xAD, 0x42, 0x21, 0x8D, 0x54, 0x21, // -> 34
        0x80, 0xFE,
    ];
    let Some(r) = run("hl-apu", &[(0x8000, code)], &[0x8000], None, &["rom.sfc", "--frames", "1", "--frame-blocks", "20"]) else { return };
    assert_eq!(r.code, 0, "{}", r.out);
    for (reg, want) in [(0x2150, "AA"), (0x2151, "BB"), (0x2152, "CC"), (0x2153, "12"), (0x2154, "34")] {
        assert!(r.reg(reg).is_some_and(|l| l.ends_with(&format!("last={want}"))), "${reg:04X}: {:?}\n{}", r.reg(reg), r.out);
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

#[test]
fn irq_enable_does_not_deliver_anything_and_is_reported() {
    // $4200 = $30 (IRQ on H and V): nothing runs, the write shows in the report
    let main: &[u8] = &[0x18, 0xFB, 0xA9, 0x30, 0x8D, 0x00, 0x42, 0x80, 0xFE];
    let segs: [(u32, &[u8]); 2] = [(0x8000, main), (0x9000, NMI_HANDLER)];
    let Some(r) = run("hl-irq", &segs, &[0x8000, 0x9000], None, &["rom.sfc", "--frames", "3", "--frame-blocks", "50"]) else { return };
    assert_eq!(r.code, 0, "{}", r.out);
    assert!(r.has("nmis: 0"), "{}", r.out);
    assert_eq!(r.reg(0x4200), Some("reads=0 writes=1 last=30"), "{}", r.out);
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

    // A reset that returns
    let Some(r) = run("hl-returns", &[(0x8000, &[0x60])], &[0x8000], None, &["rom.sfc"]) else { return };
    assert_eq!(r.code, 11, "{}", r.out);
    assert!(r.out.contains("stop: reset returned"), "{}", r.out);
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
    let Ok(bytes) = std::fs::read("roms/alttp_usa.sfc") else {
        eprintln!("skipped: roms/alttp_usa.sfc not found");
        return;
    };
    let rom = crate::rom::Rom::from_bytes(bytes.clone()).unwrap();
    let cfg = Config::load("games/alttp_usa.toml").unwrap();
    let roots: Vec<(u32, Flags)> = cfg.entries.iter().map(|e| (e.addr, e.flags)).collect();
    let d = discover(&rom, &cfg, &roots);
    let addrs: Vec<u32> = roots.iter().map(|r| r.0).collect();
    let g = Graph::build(&d, &addrs);
    let out = Emitter::new(&d, &g).generate_all();
    let frames = std::env::var("ALTTP_FRAMES").unwrap_or_else(|_| "30".into());
    let blocks = std::env::var("ALTTP_FRAME_BLOCKS").unwrap_or_else(|_| "20000".into());
    let r = build_and_run(&cc, "alttp-headless", &out, Some(bytes), &["rom.sfc", "--frames", &frames, "--frame-blocks", &blocks]);
    eprintln!("exit code {}\n{}", r.code, r.out);
    assert!(r.out.contains("== report =="), "{}", r.out);
    // The reset code forces blank, the audio upload goes through the handshake, and NMI gets enabled.
    assert!(r.reg(0x2100).is_some(), "{}", r.out);
    assert!(r.reg(0x2140).is_some_and(|l| l.contains("reads=") && !l.contains("reads=0 ")), "{}", r.out);
    assert!(r.reg(0x4200).is_some_and(|l| l.ends_with("last=81")), "{}", r.out);
    let nmis: u32 = r.out.lines().find_map(|l| l.strip_prefix("nmis: ")).and_then(|n| n.parse().ok()).expect("nmis in the report");
    assert!(nmis >= 10, "only {nmis} NMIs\n{}", r.out);
    assert!(r.reg(0x420B).is_some(), "the NMI handler starts DMA\n{}", r.out);
}
