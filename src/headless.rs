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
    assert_eq!(r.code, 12, "{}", r.out);
    assert!(r.out.contains("the context started at 00A000 returned"), "{}", r.out);
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
    let frames = std::env::var("ALTTP_FRAMES").unwrap_or_else(|_| "200".into());
    let blocks = std::env::var("ALTTP_FRAME_BLOCKS").unwrap_or_else(|_| "20000".into());
    let dump = std::env::temp_dir().join(format!("snes-recomp-alttp-dump-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dump);
    std::fs::create_dir_all(&dump).unwrap();
    let dump_arg = dump.to_string_lossy().into_owned();
    let shot_arg = dump.join("shot.png").to_string_lossy().into_owned();
    let r = build_and_run(&cc, "alttp-headless", &out, Some(bytes), &["rom.sfc", "--frames", &frames, "--frame-blocks", &blocks, "--dump", &dump_arg, "--screenshot", &shot_arg, "--trace-frames", "0-100000", "--watch", "11"]);
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
    let mem = Mem { vram: read("vram.bin"), cgram: read("cgram.bin"), oam: read("oam.bin"), regs: read("ppu_regs.bin") };
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
    let code = [st(0x00, 0x2121), st(0x21, 0x2122), st(0x22, 0x2122), st(0x07, 0x2105), vec![0x6C, 0x00, 0x00]].concat();
    let Some((r, m, dir)) = run_dump("hl-dump-fail", &code, &[], true) else { return };
    assert_eq!(r.code, 5, "{}", r.out);
    assert_eq!((m.vram.len(), m.cgram.len(), m.oam.len(), m.regs.len()), (0x10000, 512, 544, 0x40));
    assert_eq!(&m.cgram[0..2], &[0x21, 0x22]);
    assert_eq!(m.regs[5], 0x07);
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
    let dir = std::env::temp_dir().join(format!("snes-recomp-shot-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("s.png").to_string_lossy().into_owned();
    let r = run(name, &[(0x8000, code)], &[0x8000], None, &["rom.sfc", "--frames", "1", "--frame-blocks", "20", "--screenshot", &path])?;
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
    let Some(img) = math_case("sub", 0x01, GREEN, BLUE, extra.clone()) else { return };
    assert_eq!(img.rgb(0, 0), rgb(0x7FE0));
    // ...and the fixed colour stands in, not halved, when nothing is on the subscreen.
    let extra = vec![st(0x00, 0x212D), st(0x02, 0x2130), st(0x61, 0x2131), fixed_red];
    let Some(img) = math_case("sub-empty", 0x01, GREEN, BLUE, extra) else { return };
    assert_eq!(img.rgb(0, 0), rgb(YELLOW));
}

#[test]
fn what_is_not_drawn_is_reported() {
    let code = prog(&[st(0x0F, 0x2100), st(0x07, 0x2105), cgram_set(0, &[RED]), st(0x11, 0x212C)]);
    let Some((r, img)) = shot("rd-mode7", &code) else { return };
    assert!(r.out.contains("warning: graphics mode 7 is not drawn"), "{}", r.out);
    assert_eq!(img.unwrap().rgb(4, 4), rgb(RED));

    let code = prog(&[st(0x01, 0x2105), st(0x01, 0x2131), st(0x01, 0x212E), st(0x10, 0x2106)]);
    let Some((r, _)) = shot("rd-math", &code) else { return };
    assert!(!r.out.contains("colour math"), "{}", r.out);
    assert!(r.out.contains("warning: windows are not applied"), "{}", r.out);
    assert!(!r.out.contains("graphics mode"), "{}", r.out);

    let code = prog(&[st(0x01, 0x2105), st(0x01, 0x2131), st(0x10, 0x2130)]);
    let Some((r, _)) = shot("rd-math-window", &code) else { return };
    assert!(r.out.contains("warning: colour math inside or outside the colour window is not applied"), "{}", r.out);
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
