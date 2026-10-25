//! Pruebas del SPC700 y del DSP (`runtime/snes_spc.inc`): un programa en C incluye el fichero tal cual, carga
//! programas escritos a mano en la RAM de sonido y compara los registros con los valores calculados a mano.

use crate::codegen::RUNTIME_SPC;

const HEAD: &str = "#include <stdint.h>\n#include <stdio.h>\n#include <string.h>\n#include <stdlib.h>\n#include <math.h>\n\
static uint32_t frame_len = 1000;\n#include \"snes_spc.inc\"\n";

/// Compila y ejecuta `main_body` (el cuerpo de `main`) junto al runtime de sonido; `None` si no hay compilador.
fn run_c(name: &str, extra: &str, main_body: &str) -> Option<String> {
    let cc = crate::cctest::find()?;
    let main = format!("{HEAD}{extra}\nint main(void) {{\n{main_body}\n return 0;\n}}\n");
    let files = [("main.c", main.as_str()), ("snes_spc.inc", RUNTIME_SPC)];
    Some(cc.compile_run(name, &files, &[]).unwrap_or_else(|e| panic!("{e}")))
}

fn c_bytes(b: &[u8]) -> String {
    b.iter().map(|x| format!("0x{x:02X}")).collect::<Vec<_>>().join(",")
}

/// Ejecuta `prog` (cargado en $0200, sin ROM de arranque, SP=$EF) durante `steps` instrucciones, tras `init` (C),
/// y devuelve `A=.. X=.. Y=.. SP=.. PSW=.. PC=..` y los bytes $0030-$0033 de la RAM.
fn cpu(name: &str, prog: &[u8], steps: u32, init: &str) -> Option<String> {
    let body = format!(
        "static const uint8_t prog[] = {{{}}}; int i;\n spc_reset(); spc_f1 = 0; spc_sp = 0xEF;\n memcpy(aram + 0x200, prog, sizeof prog);\n spc_pc = 0x200;\n {init}\n for (i = 0; i < {steps}; i++) spc_step();\n \
         printf(\"A=%02X X=%02X Y=%02X SP=%02X PSW=%02X PC=%04X M=%02X%02X%02X%02X\\n\", spc_a, spc_x, spc_y, spc_sp, spc_psw, spc_pc, aram[0x30], aram[0x31], aram[0x32], aram[0x33]);",
        c_bytes(prog)
    );
    run_c(name, "", &body).map(|s| s.trim().to_string())
}

macro_rules! check {
    ($name:expr, $prog:expr, $steps:expr, $init:expr, $want:expr) => {
        if let Some(got) = cpu($name, $prog, $steps, $init) {
            assert_eq!(got, $want, "{}", $name);
        }
    };
}

#[test]
fn adc_and_sbc_set_the_flags_like_the_hardware() {
    // 50+50 = A0: overflow and negative. Then SETC ; SBC #1 = 9F, no borrow (C), negative.
    check!("spc-adc", &[0xE8, 0x50, 0x88, 0x50], 2, "", "A=A0 X=00 Y=00 SP=EF PSW=C0 PC=0204 M=00000000");
    check!("spc-sbc", &[0xE8, 0x50, 0x88, 0x50, 0x80, 0xA8, 0x01], 4, "", "A=9F X=00 Y=00 SP=EF PSW=81 PC=0207 M=00000000");
    // FF+01 = 00: carry, zero, half carry.
    check!("spc-adc-carry", &[0xE8, 0xFF, 0x88, 0x01], 2, "", "A=00 X=00 Y=00 SP=EF PSW=0B PC=0204 M=00000000");
}

#[test]
fn daa_and_das_adjust_decimal_results() {
    // 19 + 28 = 47 (BCD); 47 - 28 = 19.
    check!("spc-daa", &[0xE8, 0x19, 0x60, 0x88, 0x28, 0xDF], 4, "", "A=47 X=00 Y=00 SP=EF PSW=08 PC=0206 M=00000000");
    check!("spc-das", &[0xE8, 0x47, 0x80, 0xA8, 0x28, 0xBE], 4, "", "A=19 X=00 Y=00 SP=EF PSW=01 PC=0206 M=00000000");
}

#[test]
fn mul_and_div_work_on_ya() {
    // $12 * $34 = $03A8.
    check!("spc-mul", &[0x8D, 0x12, 0xE8, 0x34, 0xCF], 3, "", "A=A8 X=00 Y=03 SP=EF PSW=00 PC=0205 M=00000000");
    // $03A8 / $10 = $3A remainder 8.
    check!("spc-div", &[0x8D, 0x03, 0xE8, 0xA8, 0xCD, 0x10, 0x9E], 4, "", "A=3A X=10 Y=08 SP=EF PSW=08 PC=0207 M=00000000");
}

#[test]
fn bit_operations_and_the_carry_bit_instructions() {
    // SET1 $30.3 ; MOV A,$30 -> 08 ; CLR1 $30.3 ; MOV A,$30 -> 00 (Z)
    check!("spc-set1", &[0x62, 0x30, 0xE4, 0x30], 2, "", "A=08 X=00 Y=00 SP=EF PSW=00 PC=0204 M=08000000");
    check!("spc-clr1", &[0x62, 0x30, 0x72, 0x30, 0xE4, 0x30], 3, "", "A=00 X=00 Y=00 SP=EF PSW=02 PC=0206 M=00000000");
    // $30 = 04: MOV1 C,$0030.2 -> C=1 ; NOT1 $0030.2 -> $30 = 00 ; OR1 C,/$0030.2 -> C stays 1 ; AND1 C,$0030.2 -> C=0
    check!(
        "spc-bits",
        &[0x8F, 0x04, 0x30, 0xAA, 0x30, 0x40, 0xEA, 0x30, 0x40, 0x2A, 0x30, 0x40, 0x4A, 0x30, 0x40],
        5,
        "",
        "A=00 X=00 Y=00 SP=EF PSW=00 PC=020F M=00000000"
    );
    // BBS: $30 bit 0 set -> the branch skips the INC.
    check!("spc-bbs", &[0x8F, 0x01, 0x30, 0x03, 0x30, 0x01, 0xBC, 0xBC], 2, "", "A=00 X=00 Y=00 SP=EF PSW=00 PC=0207 M=01000000");
}

#[test]
fn jumps_loops_the_stack_and_calls() {
    // X=5, A=0; loop: CLRC ; ADC A,#3 ; DEC X ; BNE loop  -> A=15, X=0 after 2 + 5*4 steps
    check!(
        "spc-loop",
        &[0xCD, 0x05, 0xE8, 0x00, 0x60, 0x88, 0x03, 0x1D, 0xD0, 0xFA],
        22,
        "",
        "A=0F X=00 Y=00 SP=EF PSW=02 PC=020A M=00000000"
    );
    // PUSH / POP and CALL / RET
    check!("spc-push", &[0xE8, 0x77, 0x2D, 0xE8, 0x00, 0xAE], 4, "", "A=77 X=00 Y=00 SP=EF PSW=02 PC=0206 M=00000000");
    let mut prog = vec![0x3F, 0x10, 0x02, 0xCD, 0x01]; // CALL $0210 ; MOV X,#1
    prog.resize(0x10, 0);
    prog.extend([0xE8, 0x99, 0x6F]); // MOV A,#$99 ; RET
    check!("spc-call", &prog, 4, "", "A=99 X=01 Y=00 SP=EF PSW=00 PC=0205 M=00000000");
    // DBNZ Y: Y=3 counts down; the loop body INCs X each time.
    check!("spc-dbnz", &[0x8D, 0x03, 0x3D, 0xFE, 0xFD], 7, "", "A=00 X=03 Y=00 SP=EF PSW=00 PC=0205 M=00000000");
}

#[test]
fn sixteen_bit_operations() {
    // $30/$31 = $01FF: MOVW YA,$30 ; INCW $30 -> $0200 ; ADDW YA,$30 -> $03FF
    check!(
        "spc-movw",
        &[0xBA, 0x30, 0x3A, 0x30, 0x7A, 0x30],
        3,
        "aram[0x30] = 0xFF; aram[0x31] = 0x01;",
        "A=FF X=00 Y=03 SP=EF PSW=00 PC=0206 M=00020000"
    );
    // CMPW YA,$30 with YA < value: borrow (C=0), negative.
    check!("spc-cmpw", &[0xBA, 0x30, 0x5A, 0x32], 2, "aram[0x30] = 0x00; aram[0x31] = 0x01; aram[0x32] = 0x01; aram[0x33] = 0x01;", "A=00 X=00 Y=01 SP=EF PSW=80 PC=0204 M=00010101");
}

#[test]
fn memory_forms_and_shifts() {
    // MOV $30,#$81 ; ASL $30 -> $02, C=1 ; ROR $30 -> $81 (C goes in), C=0 ; XCN A
    check!("spc-asl", &[0x8F, 0x81, 0x30, 0x0B, 0x30, 0x6B, 0x30], 3, "", "A=00 X=00 Y=00 SP=EF PSW=80 PC=0207 M=81000000");
    // ADC dd,ds : $30 = $10 + $32 ($05) ; AND (X),(Y)
    check!("spc-adc-dd", &[0x60, 0x89, 0x32, 0x30], 2, "aram[0x30] = 0x10; aram[0x32] = 0x05;", "A=00 X=00 Y=00 SP=EF PSW=00 PC=0204 M=15000500");
    // MOV A,[$32]+Y with $32/$33 = $0300 and Y=2
    check!("spc-indy", &[0x8D, 0x02, 0xF7, 0x32], 2, "aram[0x32] = 0x00; aram[0x33] = 0x03; aram[0x302] = 0x5A;", "A=5A X=00 Y=02 SP=EF PSW=00 PC=0204 M=00000003");
    // TSET1 $0030 with A=$0F and [$30]=$F0: Z from A-[..], memory becomes $FF
    check!("spc-tset1", &[0xE8, 0x0F, 0x0E, 0x30, 0x00], 2, "aram[0x30] = 0xF0;", "A=0F X=00 Y=00 SP=EF PSW=00 PC=0205 M=FF000000");
}

#[test]
fn the_timers_count_in_their_own_rates() {
    // T0 (target 2, 128 cycles per tick): wait for $FD to be non zero, store it at $40.
    // T2 (target 4, 16 cycles per tick): accumulate $FF at $41.
    let prog_t0: &[u8] = &[0x8F, 0x02, 0xFA, 0x8F, 0x01, 0xF1, 0xE4, 0xFD, 0xF0, 0xFC, 0xC4, 0x40, 0x2F, 0xFE];
    let body = format!(
        "static const uint8_t p[] = {{{}}};\n spc_reset(); spc_f1 = 0; memcpy(aram + 0x200, p, sizeof p); spc_pc = 0x200; spc_advance(100);\n printf(\"%02X\\n\", aram[0x40]);",
        c_bytes(prog_t0)
    );
    if let Some(out) = run_c("spc-t0", "", &body) {
        assert_eq!(out.trim(), "01");
    }
    let prog_t2: &[u8] = &[0x8F, 0x04, 0xFC, 0x8F, 0x04, 0xF1, 0xE4, 0xFF, 0x60, 0x84, 0x41, 0xC4, 0x41, 0x2F, 0xF7];
    let body = format!(
        "static const uint8_t p[] = {{{}}};\n spc_reset(); spc_f1 = 0; memcpy(aram + 0x200, p, sizeof p); spc_pc = 0x200; spc_advance(120);\n printf(\"%d\\n\", aram[0x41]);",
        c_bytes(prog_t2)
    );
    if let Some(out) = run_c("spc-t2", "", &body) {
        // 120 ticks are about 2044 cycles; T2 ticks every 16 cycles and the target is 4: one count per 64 cycles
        let n: i32 = out.trim().parse().unwrap();
        assert!((29..=33).contains(&n), "T2 counted {n}");
    }
}

#[test]
fn the_boot_rom_answers_aa_bb_takes_an_upload_and_runs_it() {
    let prog: &[u8] = &[0x8F, 0x42, 0xF4, 0x2F, 0xFE]; // MOV $F4,#$42 ; BRA $
    let body = format!(
        "static const uint8_t p[] = {{{}}}; int i, k;\n spc_reset();\n spc_advance(200);\n printf(\"%02X %02X\\n\", spc_out[0], spc_out[1]);\n \
         spc_in[2] = 0x00; spc_in[3] = 0x03; spc_in[1] = 1; spc_in[0] = 0xCC;\n \
         for (k = 0; k < 100 && spc_out[0] != 0xCC; k++) spc_advance(10);\n printf(\"ready %02X\\n\", spc_out[0]);\n \
         for (i = 0; i < (int)sizeof p; i++) {{ spc_in[1] = p[i]; spc_in[0] = (uint8_t)i; for (k = 0; k < 100 && spc_out[0] != i; k++) spc_advance(10); }}\n \
         spc_in[2] = 0x00; spc_in[3] = 0x03; spc_in[1] = 0; spc_in[0] = (uint8_t)(sizeof p + 2);\n \
         for (k = 0; k < 100 && spc_out[0] != 0x42; k++) spc_advance(10);\n printf(\"run %02X\\n\", spc_out[0]);",
        c_bytes(prog)
    );
    if let Some(out) = run_c("spc-boot", "", &body) {
        assert_eq!(out, "AA BB
ready CC
run 42
");
    }
}

// ---- DSP ----

/// C shared by the DSP tests: a captured output, the sample directory at $0100 and a square wave of BRR at $0200
/// (8 samples of +14336, 8 of -14336, looping), with the master volume up and the echo writes off.
const DSP_C: &str = r#"
static int16_t cl[2048], cr[2048]; static int ncap;
static void cap(int16_t l, int16_t r) { if (ncap < 2048) { cl[ncap] = l; cr[ncap] = r; ncap++; } }
static void square(void) {
    int i;
    spc_reset();
    audio_hook = cap;
    aram[0x100] = 0x00; aram[0x101] = 0x02; aram[0x102] = 0x00; aram[0x103] = 0x02;
    aram[0x200] = 0xC3;
    for (i = 0; i < 4; i++) { aram[0x201 + i] = 0x77; aram[0x205 + i] = 0x99; }
    dsp_write(0x5D, 0x01); dsp_write(0x6C, 0x20);
    dsp_write(0x0C, 0x7F); dsp_write(0x1C, 0x7F);
    dsp_write(0x00, 0x7F); dsp_write(0x01, 0x7F);
    dsp_write(0x04, 0x00); dsp_write(0x05, 0x00); dsp_write(0x07, 0x7F);
}
static void run(int n) { while (n-- > 0) dsp_sample(); }
"#;

fn dsp(name: &str, body: &str) -> Option<String> {
    run_c(name, DSP_C, body).map(|s| s.trim().to_string())
}

#[test]
fn brr_blocks_decode_with_their_range_filter_and_end_flags() {
    // Range 12, filter 0: nibble 7 is +14336 and nibble 9 (-7) is -14336; the block has loop and end flags.
    let body = r#"
        Voice v; memset(&v, 0, sizeof v);
        spc_reset();
        aram[0x200] = 0xC3;
        { int i; for (i = 0; i < 4; i++) { aram[0x201 + i] = 0x77; aram[0x205 + i] = 0x99; } }
        v.addr = 0x200; brr_next(&v, 0);
        printf("%d %d %d end=%d loop=%d next=%04X\n", v.blk[0], v.blk[7], v.blk[8], v.blk_end != 0, v.blk_loop != 0, v.addr);
        /* range 4, nibble 1: filters 1, 2 and 3 over the same data */
        { int f; for (f = 1; f <= 3; f++) {
            int i; memset(&v, 0, sizeof v);
            aram[0x300] = (uint8_t)(0x40 | f << 2);
            for (i = 1; i < 9; i++) aram[0x300 + i] = 0x11;
            v.addr = 0x300; brr_next(&v, 0);
            printf("f%d %d %d %d\n", f, v.blk[0], v.blk[1], v.blk[2]);
        } }
        /* range above 12: the sample is 0 or -2048 */
        memset(&v, 0, sizeof v);
        aram[0x400] = 0xD0; aram[0x401] = 0x18;
        v.addr = 0x400; brr_next(&v, 0);
        printf("big %d %d\n", v.blk[0], v.blk[1]);
    "#;
    if let Some(out) = dsp("dsp-brr", body) {
        assert_eq!(out, "14336 14336 -14336 end=1 loop=1 next=0209\nf1 8 16 23\nf2 8 23 43\nf3 8 22 40\nbig 0 -2048", "{out}");
    }
}

#[test]
fn a_voice_plays_the_sample_at_the_period_its_pitch_gives() {
    // Pitch $1000 = one sample per output sample: the 16-sample square wave has period 16; pitch $0800, period 32.
    let body = r#"
        int i, last = -1, d[8], nd = 0, mx = -99999, mn = 99999, p;
        for (p = 0x1000; p >= 0x800; p -= 0x800) {
            ncap = 0; last = -1; nd = 0; mx = -99999; mn = 99999;
            square();
            dsp_write(0x02, (uint8_t)p); dsp_write(0x03, (uint8_t)(p >> 8));
            dsp_write(0x4C, 1);
            run(400);
            for (i = 60; i < 400; i++) {
                if (cl[i] > mx) mx = cl[i];
                if (cl[i] < mn) mn = cl[i];
                if (cl[i - 1] < 0 && cl[i] >= 0) { if (last >= 0 && nd < 8) d[nd++] = i - last; last = i; }
            }
            printf("pitch %04X: crossings %d first period %d max>13700 %d min<-13700 %d endx %02X right==left %d\n", p, nd, d[0], mx > 13700 && mx < 14300, mn < -13700 && mn > -14300, endx, cl[100] == cr[100]);
        }
    "#;
    if let Some(out) = dsp("dsp-voice", body) {
        let lines: Vec<&str> = out.lines().collect();
        assert!(lines[0].starts_with("pitch 1000: crossings ") && lines[0].contains("first period 16 max>13700 1 min<-13700 1 endx 01 right==left 1"), "{out}");
        assert!(lines[1].starts_with("pitch 0800: crossings ") && lines[1].contains("first period 32 max>13700 1 min<-13700 1"), "{out}");
    }
}

#[test]
fn the_envelope_attacks_sustains_and_releases() {
    // ADSR with the fastest attack, sustain level 7 and no sustain decay: $7F in ENVX; KOFF takes 8 per sample.
    let body = r#"
        square();
        dsp_write(0x02, 0x00); dsp_write(0x03, 0x10);
        dsp_write(0x05, 0xFF); dsp_write(0x06, 0xE0);
        dsp_write(0x4C, 1);
        run(30);
        printf("sustain envx %02X\n", dsp_reg[0x08]);
        dsp_write(0x5C, 1);
        run(10);
        printf("release envx %02X\n", dsp_reg[0x08]);
        run(400);
        printf("off envx %02X active %d\n", dsp_reg[0x08], voice[0].active);
        /* GAIN: linear increase from a direct level of 0 (mode 2, rate 31 every sample: +32) */
        square();
        dsp_write(0x02, 0x00); dsp_write(0x03, 0x10);
        dsp_write(0x05, 0x00); dsp_write(0x07, 0xFF);
        dsp_write(0x4C, 1);
        run(5 + 10);
        printf("gain envx %02X\n", dsp_reg[0x08]);
    "#;
    if let Some(out) = dsp("dsp-env", body) {
        // gain: key-on delay of 5 samples, then 10 steps of +32 = 320 -> ENVX = 320 >> 4 = 20 = 0x14
        assert_eq!(out, "sustain envx 7F\nrelease envx 7A\noff envx 00 active 0\ngain envx 14", "{out}");
    }
}

#[test]
fn noise_replaces_the_sample_and_the_flags_mute() {
    let body = r#"
        int i, distinct = 0, seen[64] = {0}, n = 0;
        square();
        dsp_write(0x02, 0x00); dsp_write(0x03, 0x10);
        dsp_write(0x3D, 1); dsp_write(0x6C, 0x20 | 0x1F);
        dsp_write(0x4C, 1);
        run(80);
        for (i = 20; i < 80; i++) { int k, f = 0; for (k = 0; k < n; k++) if (seen[k] == cl[i]) f = 1; if (!f && n < 64) seen[n++] = cl[i]; }
        distinct = n;
        printf("noise distinct>20 %d\n", distinct > 20);
        dsp_write(0x6C, 0x60);
        ncap = 0; run(5);
        printf("muted %d\n", cl[0] == 0 && cl[4] == 0);
    "#;
    if let Some(out) = dsp("dsp-noise", body) {
        assert_eq!(out, "noise distinct>20 1\nmuted 1", "{out}");
    }
}

#[test]
fn an_impulse_comes_back_from_the_echo_after_its_delay_with_the_feedback() {
    // Only the newest FIR tap ($7F), no main output, EVOL $7F, feedback $40, delay 1 (512 samples).
    let body = r#"
        int i, quiet = 1;
        spc_reset();
        audio_hook = cap;
        dsp_write(0x6C, 0x00);
        dsp_write(0x6D, 0x30); dsp_write(0x7D, 0x01);
        dsp_write(0x7F, 0x7F);
        dsp_write(0x0C, 0x00); dsp_write(0x1C, 0x00);
        dsp_write(0x2C, 0x7F); dsp_write(0x3C, 0x7F);
        dsp_write(0x0D, 0x40);
        run(1);                       /* the first sample moves the buffer to its real length */
        aram[0x3000] = 0x00; aram[0x3001] = 0x10;
        ncap = 0;
        run(1100);
        for (i = 1; i < 512; i++) if (cl[i] != 0) quiet = 0;
        printf("%d %d quiet=%d %d\n", cl[0], cl[512], quiet, cl[1024]);
    "#;
    if let Some(out) = dsp("dsp-echo", body) {
        // $1000 -> history $0800 -> FIR $0800*127>>6 = 4064 -> output 4064*127>>7 = 4032;
        // written back with feedback: 4064*64>>7 = 2032 -> next lap: 1016 -> 2016 -> 2000
        assert!(out.starts_with("4032 2000 quiet=1 "), "{out}");
    }
}
