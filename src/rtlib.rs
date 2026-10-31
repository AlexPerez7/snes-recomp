//! Tests of the runtime as a library of its own (`SNES_RT_LIBRARY`, spec `runtime-library`): the runtime without
//! video is built as a shared library with nothing of the game in it, the generated C as another one, and a small
//! loader joins them at run time with `dlopen`, the way a build on another device would. They need gcc or clang on
//! Linux (shared libraries and `dlopen`), and are skipped elsewhere.

use crate::codegen::{HEADLESS_RUNTIME, RUNTIME_CORE, RUNTIME_HEADER, RUNTIME_PPU, RUNTIME_SPC};
use crate::games::Source;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The flags both builds share, so that they only differ in how the game joins the runtime.
const FLAGS: &[&str] = &["-std=c11", "-O2", "-I.", "-DSNES_RT_TICK", "-DSNES_RT_CHECKS"];

/// Loads the module of the game named by `argv[1]` (unless it is `-`), registers it and runs the runtime with the
/// remaining arguments.
const LOADER: &str = r#"#include <dlfcn.h>
#include <stdio.h>
int snes_headless_main(int argc, char **argv);
int main(int argc, char **argv) {
    if (argv[1][0] != '-') {
        void *game = dlopen(argv[1], RTLD_NOW | RTLD_GLOBAL);
        void (*init)(void);
        if (!game) { fprintf(stderr, "dlopen: %s\n", dlerror()); return 20; }
        *(void **)&init = dlsym(game, "snes_rt_module_init");
        if (!init) { fprintf(stderr, "no snes_rt_module_init\n"); return 21; }
        init();
    }
    argv[1] = argv[0];
    return snes_headless_main(argc - 1, argv + 1);
}
"#;

fn gnu() -> Option<String> {
    match crate::cctest::find() {
        Some(crate::cctest::Compiler::Gnu(cc)) if cfg!(target_os = "linux") => Some(cc),
        _ => {
            eprintln!("skipped: the runtime as a library is tested with gcc or clang on Linux");
            None
        }
    }
}

fn temp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("snes-rtlib-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn cc(cc: &str, dir: &Path, args: &[&str]) {
    let out = Command::new(cc).current_dir(dir).args(FLAGS).args(args).output().unwrap();
    assert!(out.status.success(), "{cc} {args:?}\n{}", String::from_utf8_lossy(&out.stderr));
}

/// Writes the runtime and the game's sources into `dir`.
fn write_all(dir: &Path, game: &[Source]) {
    let runtime = [
        ("snes_rt.h", RUNTIME_HEADER),
        ("headless.c", HEADLESS_RUNTIME),
        ("snes_core.inc", RUNTIME_CORE),
        ("snes_ppu.inc", RUNTIME_PPU),
        ("snes_spc.inc", RUNTIME_SPC),
    ];
    for (n, t) in runtime {
        std::fs::write(dir.join(n), t).unwrap();
    }
    crate::generate::write_files(game, dir).unwrap();
}

/// Compiles every `.c` of the game to a position-independent object, several at a time; the same objects go into
/// the module and into the build in one piece. Returns the object names.
fn game_objects(cc_: &str, dir: &Path, game: &[Source]) -> Vec<String> {
    let sources: Vec<&str> = game.iter().map(|s| s.name.as_str()).filter(|n| n.ends_with(".c")).collect();
    let objects: Vec<String> = sources.iter().map(|s| s.replace('/', "_").replace(".c", ".o")).collect();
    let next = std::sync::atomic::AtomicUsize::new(0);
    let threads = std::thread::available_parallelism().map_or(2, |n| n.get());
    std::thread::scope(|scope| {
        for _ in 0..threads {
            scope.spawn(|| loop {
                let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                let Some(src) = sources.get(i) else { break };
                cc(cc_, dir, &["-c", "-fPIC", "-o", &objects[i], src]);
            });
        }
    });
    objects
}

/// The runtime as `libsnesrt.so` (with no undefined symbol left: nothing of the game) and the loader `run_lib`.
fn build_library(cc_: &str, dir: &Path) {
    cc(cc_, dir, &["-shared", "-fPIC", "-DSNES_RT_LIBRARY", "-Wl,--no-undefined", "-o", "libsnesrt.so", "headless.c", "-lm"]);
    std::fs::write(dir.join("loader.c"), LOADER).unwrap();
    cc(cc_, dir, &["-o", "run_lib", "loader.c", "-L.", "-lsnesrt", "-Wl,-rpath,$ORIGIN", "-ldl"]);
}

/// The code of the game as `libgame.so`, its runtime symbols left for the loader to resolve.
fn build_module(cc_: &str, dir: &Path, objects: &[String]) {
    let mut args = vec!["-shared", "-o", "libgame.so"];
    args.extend(objects.iter().map(String::as_str));
    cc(cc_, dir, &args);
}

/// The runtime and the game linked in one piece, as `build_play.bat` and `snes-port` do: `run_one`.
fn build_one_piece(cc_: &str, dir: &Path, objects: &[String]) {
    let mut args = vec!["-o", "run_one", "headless.c"];
    args.extend(objects.iter().map(String::as_str));
    args.push("-lm");
    cc(cc_, dir, &args);
}

fn run(dir: &Path, exe: &str, args: &[&str]) -> (i32, String) {
    let out = Command::new(dir.join(exe)).current_dir(dir).args(args).output().unwrap();
    (out.status.code().unwrap_or(-1), format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr)))
}

/// A tiny game: reset at 00:8000 writes $42 to $10 and loops on WAI, so the run ends by frames.
fn tiny_game() -> (crate::rom::Rom, Vec<Source>) {
    let code: &[u8] = &[0xA9, 0x42, 0x85, 0x10, 0xCB, 0x80, 0xFD]; // LDA #$42; STA $10; WAI; BRA -3
    let rom = crate::rom::test_rom(&[(0x8000, code), (0xFFFC, &[0x00, 0x80])]);
    let toml = format!("[game]\nid = \"t\"\nname = \"Tiny\"\ncrc32 = \"{:08X}\"\n", rom.crc32());
    let profile = crate::games::GameProfile::from_text(&toml, vec![], "t.toml").unwrap();
    let g = crate::generate::generate_sources(rom.bytes(), &profile).unwrap_or_else(|e| panic!("{e}"));
    (rom, g.sources)
}

#[test]
fn the_runtime_builds_without_the_game_and_runs_it_once_the_module_registers() {
    let Some(cc_) = gnu() else { return };
    let dir = temp("tiny");
    let (rom, game) = tiny_game();
    write_all(&dir, &game);
    std::fs::write(dir.join("rom.sfc"), rom.bytes()).unwrap();
    build_library(&cc_, &dir); // -Wl,--no-undefined: no symbol of the game is missing from it
    let lib = std::fs::read(dir.join("libsnesrt.so")).unwrap();
    assert!(!lib.windows(9).any(|w| w == b"f_00_8000"), "the library holds a function of the game");
    let objects = game_objects(&cc_, &dir, &game);
    build_module(&cc_, &dir, &objects);
    let (code, out) = run(&dir, "run_lib", &["./libgame.so", "rom.sfc", "--frames", "3", "--dump", ".", "--frame-times", "ft.txt"]);
    assert_eq!(code, 0, "{out}");
    let times = std::fs::read_to_string(dir.join("ft.txt")).unwrap();
    assert_eq!(times.lines().count(), 3, "one line per frame: {times}");
    assert!(times.lines().all(|l| l.parse::<u64>().is_ok()), "{times}");
    let (code, out) = run(&dir, "run_lib", &["./libgame.so", "rom.sfc", "--frames", "3", "--frame-times", "no/such/dir/ft.txt"]);
    assert!(code == 2 && out.contains("no/such/dir/ft.txt"), "{code}: {out}");
    assert_eq!(std::fs::read(dir.join("wram.bin")).unwrap()[0x10], 0x42, "{out}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn without_a_module_the_runtime_stops_with_code_15_and_runs_nothing() {
    let Some(cc_) = gnu() else { return };
    let dir = temp("nomodule");
    let (rom, game) = tiny_game();
    write_all(&dir, &game);
    std::fs::write(dir.join("rom.sfc"), rom.bytes()).unwrap();
    build_library(&cc_, &dir);
    let (code, out) = run(&dir, "run_lib", &["-", "rom.sfc", "--frames", "3", "--dump", "."]);
    assert_eq!(code, 15, "{out}");
    assert!(out.contains("snes_rt_register_entry") && out.contains("no game module registered"), "{out}");
    assert_eq!(std::fs::read(dir.join("wram.bin")).unwrap()[0x10], 0, "the reset ran: {out}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// The real game, 1200 frames with START pressed at 1100 (the intro switches execution contexts): the library, the
/// build in one piece and the build with the contexts on threads (`SNES_RT_THREAD_CONTEXTS`, as on Android) leave the
/// same WRAM, VRAM, CGRAM, OAM and registers. Skipped without the game or gcc/clang on Linux.
#[test]
fn the_real_game_runs_the_same_with_the_runtime_as_a_library() {
    let Some(game) = crate::cctest::real_game() else { return };
    let Some(cc_) = gnu() else { return };
    let dir = temp("real");
    let g = crate::generate::generate_sources(game.rom.bytes(), &game.profile).unwrap();
    write_all(&dir, &g.sources);
    std::fs::copy(&game.rom_path, dir.join("rom.sfc")).unwrap();
    let objects = game_objects(&cc_, &dir, &g.sources);
    build_one_piece(&cc_, &dir, &objects);
    build_library(&cc_, &dir);
    build_module(&cc_, &dir, &objects);
    // the library holds no byte of the ROM (2.4)
    crate::port::scan(game.rom.bytes(), &{
        let only = dir.join("lib-only");
        std::fs::create_dir_all(&only).unwrap();
        std::fs::copy(dir.join("libsnesrt.so"), only.join("libsnesrt.so")).unwrap();
        only
    })
    .unwrap_or_else(|e| panic!("{e}"));
    // the contexts on threads (as on Android, which has no ucontext) instead of ucontext
    let mut args = vec!["-DSNES_RT_THREAD_CONTEXTS", "-o", "run_thr", "headless.c"];
    args.extend(objects.iter().map(String::as_str));
    args.extend(["-lm", "-lpthread"]);
    cc(&cc_, &dir, &args);
    for (exe, sub, pre) in [("run_one", "one", vec![]), ("run_lib", "lib", vec!["./libgame.so"]), ("run_thr", "thr", vec![])] {
        std::fs::create_dir_all(dir.join(sub)).unwrap();
        let mut args = pre;
        args.extend(["rom.sfc", "--frames", "1200", "--press", "START@1100-1110", "--dump", sub]);
        let (code, out) = run(&dir, exe, &args);
        assert_eq!(code, 0, "{exe}: {out}");
    }
    for f in ["wram.bin", "vram.bin", "cgram.bin", "oam.bin", "ppu_regs.bin"] {
        let read = |sub: &str| std::fs::read(dir.join(sub).join(f)).unwrap();
        assert!(read("one") == read("lib"), "{f} differs between the build in one piece and the library");
        assert!(read("one") == read("thr"), "{f} differs between ucontext and the contexts on threads");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// Every function of `snes_rt.h` that is not inline is in the table the runtime gives a host that links the code of
/// the game by hand (`snes_rt_symbols`), and nothing else is.
#[test]
fn the_symbol_table_of_the_runtime_lists_every_function_of_snes_rt_h() {
    let mut declared: Vec<String> = RUNTIME_HEADER
        .lines()
        .filter(|l| !l.starts_with("static") && !l.starts_with(' ') && !l.starts_with('#') && l.ends_with(");") && l.contains('('))
        .filter_map(|l| l.split('(').next()?.split_whitespace().last().map(|n| n.trim_start_matches('*').to_string()))
        .filter(|n| n != "snes_rt_module_init") // defined by the module of the game, not by the runtime
        .collect();
    declared.sort();
    let table = &RUNTIME_CORE[RUNTIME_CORE.find("static const SnesRtSymbol rt_symbols[]").unwrap()..];
    let table = &table[..table.find("};").unwrap()];
    let mut listed: Vec<String> = table.split("RT_SYM(").skip(1).map(|p| p.split(')').next().unwrap().to_string()).collect();
    listed.sort();
    assert_eq!(listed, declared);
    assert!(table.contains("\"rt_block\""), "rt_block (SNES_RT_TRACE) is in the table");
}
