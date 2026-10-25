//! `snes-port <rom>`: builds the playable game from the user's own ROM, on the user's machine, and launches it.
//!
//! Everything derived from the ROM (the generated C, the objects, the executable) is written to the user's cache
//! and nowhere else; the ROM itself is never copied, and the executable reads it from where the user keeps it.
//! The cache holds one folder per ROM and version of the program (`<crc32>-<version>`), built in a temporary folder
//! and renamed when it is complete, so a build that fails or is interrupted leaves nothing half done.

use crate::crc32::crc32;
use crate::rom::Rom;
use crate::toolchain::{self, Compiler};
use crate::{codegen, config::Config, discover, download, games, graph};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

pub const EXIT_UNKNOWN_ROM: i32 = 2;
pub const EXIT_NO_TOOLCHAIN: i32 = 3;
pub const EXIT_BUILD_FAILED: i32 = 4;

/// Why the program stops: the code it exits with and what it tells the user.
#[derive(Debug)]
pub struct Failure {
    pub code: i32,
    pub message: String,
}

fn fail<T>(code: i32, message: impl Into<String>) -> Result<T, Failure> {
    Err(Failure { code, message: message.into() })
}

pub const USAGE: &str = "\
usage:
  snes-port <rom> [--cache <dir>] [--no-launch] [--fast] [--download-compiler] [-- <game options>]
  snes-port clean [<rom>] [--cache <dir>]

Builds the game from your own ROM, on this machine, the first time (a few minutes) and starts it; the next times
it starts at once. Nothing derived from the ROM leaves this machine: the generated C and the executable live in the
cache (%LOCALAPPDATA%\\snes-port on Windows) and the executable reads your ROM from where it is. You can also drag
the ROM onto snes-port.exe, or open it without arguments (double click) to choose the ROM in a file dialog.
--no-launch builds and does not start the game. --fast builds with fewer optimisations (quicker, slower game).
Everything after `--` goes to the game (see its options in the README).
clean deletes the cache of that ROM, or the whole cache; it never touches ROMs or saved games.
Needs a C compiler (Visual Studio Build Tools, clang or gcc) and the SDL2 development files (SDL2_DIR, or the sdl2
folder next to this program). Without a compiler it offers to download TinyCC (about 480 KiB, SHA-256 checked) into
the cache; --download-compiler agrees beforehand. With a compiler on the machine nothing is ever downloaded.";

// ---- the cache ----

/// The cache folder: `SNES_PORT_CACHE`, else `%LOCALAPPDATA%\snes-port` (or the usual place of the system).
pub fn default_cache_dir() -> PathBuf {
    if let Some(d) = std::env::var_os("SNES_PORT_CACHE") {
        return PathBuf::from(d);
    }
    let base = if cfg!(windows) {
        std::env::var_os("LOCALAPPDATA").map(PathBuf::from)
    } else if let Some(x) = std::env::var_os("XDG_CACHE_HOME") {
        Some(PathBuf::from(x))
    } else {
        std::env::var_os("HOME").map(|h| {
            let h = PathBuf::from(h);
            if cfg!(target_os = "macos") { h.join("Library").join("Caches") } else { h.join(".cache") }
        })
    };
    base.unwrap_or_else(std::env::temp_dir).join("snes-port")
}

/// A short tag of everything that decides what the build looks like: when any of `parts` changes, so does the tag.
pub fn version_tag(parts: &[&str]) -> String {
    let mut all = Vec::new();
    for p in parts {
        all.extend_from_slice(p.as_bytes());
        all.push(0);
    }
    format!("{:08x}", crc32(&all))
}

/// The tag of this program: its version, the runtimes and game configurations it embeds, and the source of the
/// generator, so that a new generator or runtime rebuilds what an older one left in the cache.
pub fn current_version(fast: bool) -> String {
    let configs: Vec<&str> = games::KNOWN.iter().map(|g| g.config).collect();
    let mut parts = vec![
        env!("CARGO_PKG_VERSION"),
        if fast { "fast" } else { "full" },
        codegen::RUNTIME_HEADER,
        codegen::RUNTIME_CORE,
        codegen::RUNTIME_PPU,
        codegen::RUNTIME_SPC,
        codegen::PLAY_RUNTIME,
        codegen::PLAY_CFG,
        include_str!("codegen.rs"),
        include_str!("discover.rs"),
        include_str!("graph.rs"),
        include_str!("disasm.rs"),
        include_str!("opcodes.rs"),
        include_str!("config.rs"),
    ];
    parts.extend(configs);
    version_tag(&parts)
}

pub fn cache_name(crc: u32, version: &str) -> String {
    format!("{crc:08x}-{version}")
}

/// A build in the cache.
pub struct Built {
    pub dir: PathBuf,
    /// False when it was already there and nothing ran.
    pub rebuilt: bool,
}

const COMPLETE: &str = ".complete";

/// The folder `<cache>/<name>`, building it with `build` (which fills a temporary folder it is given) unless it is
/// already complete. A failed build leaves nothing; a successful one replaces older versions of the same ROM
/// (the folders that start with `prefix`).
pub fn ensure_built(cache: &Path, name: &str, prefix: &str, build: &mut dyn FnMut(&Path) -> Result<(), String>) -> Result<Built, String> {
    let dir = cache.join(name);
    if dir.join(COMPLETE).exists() {
        return Ok(Built { dir, rebuilt: false });
    }
    std::fs::create_dir_all(cache).map_err(|e| format!("{}: {e}", cache.display()))?;
    let tmp = cache.join(format!("{name}.tmp{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).map_err(|e| format!("{}: {e}", tmp.display()))?;
    if let Err(e) = build(&tmp) {
        let _ = std::fs::remove_dir_all(&tmp);
        return Err(e);
    }
    std::fs::write(tmp.join(COMPLETE), b"").map_err(|e| e.to_string())?;
    let _ = std::fs::remove_dir_all(&dir); // an incomplete build of this same version
    std::fs::rename(&tmp, &dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    if let Ok(entries) = std::fs::read_dir(cache) {
        for e in entries.flatten() {
            let n = e.file_name().to_string_lossy().into_owned();
            if n.starts_with(prefix) && n != name && e.path().is_dir() {
                let _ = std::fs::remove_dir_all(e.path());
            }
        }
    }
    Ok(Built { dir, rebuilt: true })
}

/// Deletes the cache of the ROM with this CRC32, or the whole cache. Returns what it deleted.
pub fn clean(cache: &Path, crc: Option<u32>) -> Result<Vec<PathBuf>, String> {
    let mut gone = Vec::new();
    let Ok(entries) = std::fs::read_dir(cache) else { return Ok(gone) };
    for e in entries.flatten() {
        let n = e.file_name().to_string_lossy().into_owned();
        let mine = match crc {
            Some(c) => n.starts_with(&format!("{c:08x}-")),
            None => true,
        };
        if !mine {
            continue;
        }
        let p = e.path();
        let r = if p.is_dir() { std::fs::remove_dir_all(&p) } else { std::fs::remove_file(&p) };
        r.map_err(|err| format!("{}: {err}", p.display()))?;
        gone.push(p);
    }
    if crc.is_none() {
        let _ = std::fs::remove_dir(cache);
    }
    Ok(gone)
}

// ---- generating and compiling ----

/// Generates the C of the game into `dir`.
fn generate(rom: &Rom, profile: &games::GameProfile, dir: &Path) -> Result<(), String> {
    let cfg = Config::parse(profile.config).map_err(|e| format!("games/{}.toml: {e}", profile.id))?;
    let roots = crate::default_roots(rom, &cfg);
    let d = discover::discover(rom, &cfg, &roots);
    let root_addrs: Vec<u32> = roots.iter().map(|r| r.0).collect();
    let g = graph::Graph::build(&d, &root_addrs);
    let out = codegen::Emitter::new(&d, &g).generate_all();
    crate::write_sources(&out, dir).map(|_| ())
}

pub fn exe_name() -> &'static str {
    if cfg!(windows) { "game.exe" } else { "game" }
}

/// The SDL2 development files (the folder with `include` and `lib`): `SDL2_DIR`, the `sdl2` folder next to the
/// program, or the one of the source tree.
pub fn find_sdl2() -> Option<PathBuf> {
    let ok = |p: &Path| p.join("include").join("SDL.h").exists();
    if let Ok(d) = std::env::var("SDL2_DIR") {
        let p = PathBuf::from(d);
        return ok(&p).then_some(p);
    }
    if let Some(exe_dir) = std::env::current_exe().ok().and_then(|e| e.parent().map(Path::to_path_buf)) {
        let p = exe_dir.join("sdl2");
        if ok(&p) {
            return Some(p);
        }
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("out").join("sdl2");
    std::fs::read_dir(root).ok()?.flatten().map(|e| e.path()).find(|p| ok(p))
}

fn tail(text: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    lines[lines.len().saturating_sub(30)..].join("\n")
}

/// Compiles the C in `dir` into `dir/game[.exe]` (and copies SDL2.dll next to it on Windows).
fn compile(compiler: &Compiler, dir: &Path, sdl: &Path, fast: bool) -> Result<(), String> {
    match compiler {
        Compiler::Msvc(vcvars) => {
            let env = toolchain::msvc_env(vcvars)?;
            let lib = sdl.join("lib").join("x64");
            let bat = format!(
                "@echo off\r\ncd /d \"{}\"\r\nif not exist obj mkdir obj\r\n\
                 cl /nologo /MP {} /TC /std:c11 /D_CRT_SECURE_NO_WARNINGS /DSNES_RT_TICK /I. /I\"{}\" rom_*.c snes_entry.c play\\snes_play.c \
                 /Fe:{} /Fo:obj\\ /link /LIBPATH:\"{}\" SDL2.lib >build.log 2>&1\r\nif errorlevel 1 (type build.log & exit /b 1)\r\n",
                dir.display(),
                if fast { "/O1" } else { "/O2" },
                sdl.join("include").display(),
                exe_name(),
                lib.display()
            );
            let script = dir.join("build.bat");
            std::fs::write(&script, bat).map_err(|e| e.to_string())?;
            let out = Command::new("cmd").arg("/C").arg(&script).envs(env.iter().map(|(k, v)| (k, v))).output().map_err(|e| e.to_string())?;
            if !out.status.success() {
                return Err(tail(&String::from_utf8_lossy(&out.stdout)));
            }
            std::fs::copy(lib.join("SDL2.dll"), dir.join("SDL2.dll")).map_err(|e| format!("SDL2.dll: {e}"))?;
            let _ = std::fs::remove_dir_all(dir.join("obj"));
            Ok(())
        }
        Compiler::Tcc(tcc) => {
            // TinyCC links against the import definitions of the DLL, not against SDL2.lib; its headers need two
            // empty macros and it has no globbing, so the sources are listed
            let dll = sdl.join("lib").join("x64").join("SDL2.dll");
            std::fs::copy(&dll, dir.join("SDL2.dll")).map_err(|e| format!("SDL2.dll: {e}"))?;
            let def = Command::new(tcc).current_dir(dir).args(["-impdef", "SDL2.dll"]).output().map_err(|e| e.to_string())?;
            if !def.status.success() {
                return Err(tail(&String::from_utf8_lossy(&def.stderr)));
            }
            let mut sources: Vec<String> = std::fs::read_dir(dir)
                .map_err(|e| e.to_string())?
                .flatten()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .filter(|n| n.ends_with(".c") && (n.starts_with("rom_") || n == "snes_entry.c"))
                .collect();
            sources.push("play/snes_play.c".to_string());
            let out = Command::new(tcc)
                .current_dir(dir)
                .args(["-DDECLSPEC=", "-DSDLCALL=", "-D_CRT_SECURE_NO_WARNINGS", "-DSNES_RT_TICK", "-I."])
                .arg(format!("-I{}", sdl.join("include").display()))
                .args(&sources)
                .args(["-L.", "-lSDL2", "-o"])
                .arg(exe_name())
                .output()
                .map_err(|e| e.to_string())?;
            if !out.status.success() {
                return Err(tail(&(String::from_utf8_lossy(&out.stdout).into_owned() + &String::from_utf8_lossy(&out.stderr))));
            }
            let _ = std::fs::remove_file(dir.join("SDL2.def"));
            Ok(())
        }
        Compiler::Gnu(cc) => {
            let sdl_flags = |what: &str| -> Vec<String> {
                Command::new("sdl2-config")
                    .arg(what)
                    .output()
                    .ok()
                    .filter(|o| o.status.success())
                    .map(|o| String::from_utf8_lossy(&o.stdout).split_whitespace().map(String::from).collect())
                    .unwrap_or_else(|| if what == "--libs" { vec!["-lSDL2".into()] } else { vec![] })
            };
            let cflags = sdl_flags("--cflags");
            let mut sources: Vec<PathBuf> = std::fs::read_dir(dir)
                .map_err(|e| e.to_string())?
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|x| x == "c") && p.file_name().is_some_and(|n| n.to_string_lossy().starts_with("rom_") || n == "snes_entry.c"))
                .collect();
            sources.push(dir.join("play").join("snes_play.c"));
            std::fs::create_dir_all(dir.join("obj")).map_err(|e| e.to_string())?;
            let jobs = std::thread::available_parallelism().map_or(2, |n| n.get());
            let next = std::sync::atomic::AtomicUsize::new(0);
            let errors = std::sync::Mutex::new(Vec::<String>::new());
            std::thread::scope(|s| {
                for _ in 0..jobs {
                    s.spawn(|| loop {
                        let i = next.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                        let Some(src) = sources.get(i) else { break };
                        let obj = dir.join("obj").join(format!("{i}.o"));
                        let out = Command::new(cc)
                            .current_dir(dir)
                            .args(["-std=c11", if fast { "-O1" } else { "-O2" }, "-DSNES_RT_TICK", "-I.", "-c"])
                            .args(&cflags)
                            .arg(src)
                            .arg("-o")
                            .arg(&obj)
                            .output();
                        match out {
                            Ok(o) if o.status.success() => {}
                            Ok(o) => errors.lock().unwrap().push(String::from_utf8_lossy(&o.stderr).into_owned()),
                            Err(e) => errors.lock().unwrap().push(e.to_string()),
                        }
                    });
                }
            });
            if let Some(e) = errors.lock().unwrap().first() {
                return Err(tail(e));
            }
            let objs: Vec<PathBuf> = (0..sources.len()).map(|i| dir.join("obj").join(format!("{i}.o"))).collect();
            let out = Command::new(cc).current_dir(dir).arg("-o").arg(exe_name()).args(&objs).args(sdl_flags("--libs")).arg("-lm").output().map_err(|e| e.to_string())?;
            if !out.status.success() {
                return Err(tail(&String::from_utf8_lossy(&out.stderr)));
            }
            let _ = std::fs::remove_dir_all(dir.join("obj"));
            Ok(())
        }
    }
}

// ---- nothing of the ROM in what is built ----

const WINDOW: usize = 32;
const MIN_DISTINCT: usize = 8;

fn window_hashes(data: &[u8], mut each: impl FnMut(usize, u64)) {
    const B: u64 = 0x100_0000_01B3;
    if data.len() < WINDOW {
        return;
    }
    let mut pow = 1u64;
    for _ in 1..WINDOW {
        pow = pow.wrapping_mul(B);
    }
    let mut h = 0u64;
    let mut counts = [0u16; 256];
    let mut distinct = 0usize;
    // For each stride (1: bytes, 2: 16-bit words, 4: 32-bit words): how many times the step between the values of
    // a position changes inside the window. 0 for some stride means a run or a ramp (`8A 8B 8C ...`, `00 3F 00 40 ...`).
    const STRIDES: [usize; 3] = [1, 2, 4];
    let bend = |k: usize, s: usize| (k >= 2 * s && data[k].wrapping_sub(data[k - s]) != data[k - s].wrapping_sub(data[k - 2 * s])) as usize;
    let mut bends = [0usize; 3];
    for (i, &b) in data.iter().enumerate() {
        for (j, &st) in STRIDES.iter().enumerate() {
            bends[j] += bend(i, st);
            if i >= WINDOW - 2 * st {
                bends[j] -= bend(i - (WINDOW - 2 * st), st);
            }
        }
        if i >= WINDOW {
            let old = data[i - WINDOW];
            h = h.wrapping_sub((old as u64 + 1).wrapping_mul(pow));
            counts[old as usize] -= 1;
            if counts[old as usize] == 0 {
                distinct -= 1;
            }
        }
        h = h.wrapping_mul(B).wrapping_add(b as u64 + 1);
        if counts[b as usize] == 0 {
            distinct += 1;
        }
        counts[b as usize] += 1;
        if i + 1 >= WINDOW && distinct >= MIN_DISTINCT && bends.iter().all(|&n| n > 0) {
            each(i + 1 - WINDOW, h);
        }
    }
}

/// An index of the windows of 32 consecutive bytes of the ROM. Windows that carry no information are left out:
/// those with fewer than 8 different values (runs of padding) and those that are a ramp of bytes or of 16 or 32-bit
/// words (`8A 8B 8C ...`), which any program holds as a table.
pub struct RomIndex<'a> {
    rom: &'a [u8],
    windows: HashMap<u64, Vec<usize>>,
}

impl<'a> RomIndex<'a> {
    pub fn new(rom: &'a [u8]) -> RomIndex<'a> {
        let mut windows: HashMap<u64, Vec<usize>> = HashMap::new();
        window_hashes(rom, |at, h| windows.entry(h).or_default().push(at));
        RomIndex { rom, windows }
    }

    /// Where `data` holds 32 consecutive bytes of the ROM: (offset in `data`, offset in the ROM).
    pub fn find_in(&self, data: &[u8]) -> Option<(usize, usize)> {
        let mut found = None;
        window_hashes(data, |at, h| {
            if found.is_some() {
                return;
            }
            if let Some(cands) = self.windows.get(&h) {
                if let Some(&r) = cands.iter().find(|&&r| self.rom[r..r + WINDOW] == data[at..at + WINDOW]) {
                    found = Some((at, r));
                }
            }
        });
        found
    }
}

/// The bytes written as `0xNN` literals in a C text, in order: the ROM could be hidden there as an array.
fn hex_literals(text: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let hex = |c: u8| (c as char).to_digit(16);
    let mut i = 0;
    while i + 3 < text.len() {
        if text[i] == b'0' && (text[i + 1] == b'x' || text[i + 1] == b'X') {
            if let (Some(a), Some(b)) = (hex(text[i + 2]), hex(text[i + 3])) {
                if text.get(i + 4).is_none_or(|&c| hex(c).is_none()) {
                    out.push((a * 16 + b) as u8);
                }
            }
            i += 4;
        } else {
            i += 1;
        }
    }
    out
}

/// Looks in every file under `dir` for 32 consecutive bytes of the ROM, raw or written as hex literals in the C.
/// Returns the file where it found them.
pub fn find_rom_bytes(rom: &[u8], dir: &Path) -> Option<PathBuf> {
    find_rom_bytes_at(rom, dir).map(|f| f.0)
}

/// Like `find_rom_bytes`, also saying where: (file, offset in the file, offset in the ROM).
pub fn find_rom_bytes_at(rom: &[u8], dir: &Path) -> Option<(PathBuf, usize, usize)> {
    let index = RomIndex::new(rom);
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&d) else { continue };
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
                continue;
            }
            let Ok(data) = std::fs::read(&p) else { continue };
            let text = p.extension().is_some_and(|x| x == "c" || x == "h" || x == "inc");
            if let Some((at, r)) = index.find_in(&data) {
                return Some((p, at, r));
            }
            if let Some((at, r)) = text.then(|| index.find_in(&hex_literals(&data))).flatten() {
                return Some((p, at, r));
            }
        }
    }
    None
}

// ---- the command ----

pub struct Options {
    pub rom: PathBuf,
    pub cache: PathBuf,
    pub launch: bool,
    pub fast: bool,
    /// Capture the game's output instead of passing it through (tests).
    pub capture: bool,
    pub game_args: Vec<String>,
}

/// What a run of the command did.
pub struct Outcome {
    pub built: Built,
    /// The exit code of the game and what it printed, if it was started.
    pub game: Option<(i32, String)>,
}

/// Identifies the ROM, builds the game unless it is in the cache, and starts it. `compiler` and `sdl` are what the
/// machine offers (`None`: not found).
pub fn build_port(opts: &Options, compiler: Option<Compiler>, sdl: Option<PathBuf>) -> Result<Outcome, Failure> {
    let rom = match Rom::load(&opts.rom) {
        Ok(r) => r,
        Err(e) => return fail(1, e),
    };
    let crc = rom.crc32();
    let Some(profile) = games::identify(crc) else {
        let mut m = format!("error: unknown ROM: CRC32 {crc:08X}\nSupported games:\n");
        for g in games::KNOWN {
            m.push_str(&format!("  {} (CRC32 {:08X})\n", g.name, g.crc32));
        }
        return fail(EXIT_UNKNOWN_ROM, m);
    };
    let version = current_version(opts.fast);
    let name = cache_name(crc, &version);
    let prefix = format!("{crc:08x}-");
    let mut build = |tmp: &Path| -> Result<(), String> {
        let Some(compiler) = compiler.as_ref() else {
            return Err("no compiler".to_string());
        };
        let Some(sdl) = sdl.as_ref() else {
            return Err("no sdl".to_string());
        };
        eprintln!("Building {} for the first time: this takes a few minutes and happens only once.", profile.name);
        generate(&rom, profile, tmp)?;
        compile(compiler, tmp, sdl, opts.fast)?;
        if !tmp.join(exe_name()).exists() {
            return Err("the compiler produced no executable".to_string());
        }
        if let Some((p, at, r)) = find_rom_bytes_at(rom.bytes(), tmp) {
            return Err(format!("internal error: bytes of the ROM found in {} (offset {at:#x}, ROM offset {r:#x}); build discarded", p.display()));
        }
        Ok(())
    };
    // with the build in the cache there is no need for the toolchain: check it only when something must be built
    let already = cache_has(&opts.cache, &name);
    if !already {
        if compiler.is_none() {
            return fail(
                EXIT_NO_TOOLCHAIN,
                "error: no C compiler found. Install Visual Studio Build Tools (\"Desktop development with C++\"), clang or gcc and run this again.",
            );
        }
        if sdl.is_none() {
            return fail(EXIT_NO_TOOLCHAIN, "error: SDL2 development files not found: set SDL2_DIR to the folder with include and lib (SDL2-devel-x.y.z-VC.zip), or put it in an sdl2 folder next to this program.");
        }
    }
    let built = match ensure_built(&opts.cache, &name, &prefix, &mut build) {
        Ok(b) => b,
        Err(e) => return fail(EXIT_BUILD_FAILED, format!("error: the build failed:\n{e}")),
    };
    if !opts.launch {
        return Ok(Outcome { built, game: None });
    }
    let mut cmd = Command::new(built.dir.join(exe_name()));
    cmd.current_dir(&built.dir).arg(&opts.rom).args(&opts.game_args);
    let game = if opts.capture {
        match cmd.output() {
            Ok(o) => (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned() + &String::from_utf8_lossy(&o.stderr)),
            Err(e) => return fail(EXIT_BUILD_FAILED, format!("error: cannot start the game: {e}")),
        }
    } else {
        match cmd.status() {
            Ok(s) => (s.code().unwrap_or(-1), String::new()),
            Err(e) => return fail(EXIT_BUILD_FAILED, format!("error: cannot start the game: {e}")),
        }
    };
    Ok(Outcome { built, game: Some(game) })
}

fn cache_has(cache: &Path, name: &str) -> bool {
    cache.join(name).join(COMPLETE).exists()
}

/// True when the game of `opts.rom` is already built (or the ROM is not one that can be built), so no compiler is needed.
fn needs_no_build(opts: &Options) -> bool {
    let Ok(rom) = Rom::load(&opts.rom) else { return true };
    let crc = rom.crc32();
    games::identify(crc).is_none() || cache_has(&opts.cache, &cache_name(crc, &current_version(opts.fast)))
}

/// The last resort when the machine has no C compiler: the portable one in the cache, or, if the user agrees
/// (`--download-compiler`, or `y` at the question), downloaded into it with its SHA-256 checked.
fn portable_compiler(cache: &Path, yes: bool, input: &mut dyn std::io::BufRead) -> Option<Compiler> {
    use download::TINYCC;
    if let Some(exe) = download::installed(cache, &TINYCC) {
        return Some(Compiler::Tcc(exe));
    }
    eprintln!("No C compiler found on this machine.");
    eprintln!(
        "{} ({} KiB) can be downloaded from {} and kept in {}. Nothing about your ROM is sent; it is the only download.",
        TINYCC.name,
        TINYCC.size / 1024,
        TINYCC.url,
        download::toolchain_dir(cache).display()
    );
    if !yes && !download::confirm("Download it now?", input) {
        return None;
    }
    match download::install(cache, &TINYCC) {
        Ok(exe) => Some(Compiler::Tcc(exe)),
        Err(e) => {
            eprintln!("error: {e}");
            None
        }
    }
}

/// `snes-port ...` as the user types it; returns the exit code.
pub fn main(args: &[String]) -> i32 {
    let mut rom: Option<PathBuf> = None;
    let mut cache: Option<PathBuf> = None;
    let (mut launch, mut fast, mut clean_cmd, mut download_yes) = (true, false, false, false);
    let mut game_args = Vec::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--" => {
                game_args.extend(it.by_ref().cloned());
                break;
            }
            "--cache" => match it.next() {
                Some(d) => cache = Some(PathBuf::from(d)),
                None => {
                    eprintln!("{USAGE}");
                    return 1;
                }
            },
            "--no-launch" => launch = false,
            "--fast" => fast = true,
            "--download-compiler" => download_yes = true,
            "clean" if rom.is_none() && !clean_cmd => clean_cmd = true,
            "-h" | "--help" => {
                println!("{USAGE}");
                return 0;
            }
            s if !s.starts_with("--") && rom.is_none() => rom = Some(PathBuf::from(s)),
            _ => {
                eprintln!("{USAGE}");
                return 1;
            }
        }
    }
    // absolute: the build runs scripts from inside its folders
    let cache = cache.unwrap_or_else(default_cache_dir);
    let cache = std::path::absolute(&cache).unwrap_or(cache);
    if clean_cmd {
        let crc = match &rom {
            Some(p) => match Rom::load(p) {
                Ok(r) => Some(r.crc32()),
                Err(e) => {
                    eprintln!("error: {e}");
                    return 1;
                }
            },
            None => None,
        };
        return match clean(&cache, crc) {
            Ok(gone) => {
                println!("deleted {} item(s) from {}", gone.len(), cache.display());
                0
            }
            Err(e) => {
                eprintln!("error: {e}");
                1
            }
        };
    }
    // started without a ROM (double click): the launcher asks for it, remembers the folder, and keeps the window open if
    // something goes wrong so the message can be read
    let launcher = rom.is_none() && args.is_empty();
    let rom = match rom {
        Some(r) => r,
        None if launcher => match pick_rom(&cache) {
            Some(r) => r,
            None => return 0,
        },
        None => {
            eprintln!("{USAGE}");
            return 1;
        }
    };
    let code = run_port(rom, cache, launch, fast, download_yes, game_args);
    if launcher && code != 0 {
        eprintln!("
Press Enter to close.");
        let _ = std::io::stdin().read_line(&mut String::new());
    }
    code
}

/// The ROM chosen in the Windows "open file" dialog (through PowerShell, which every Windows has), starting in the
/// folder of the last one; `None` if cancelled or there is no dialog.
fn pick_rom(cache: &Path) -> Option<PathBuf> {
    let last = cache.join("last-rom-dir.txt");
    let start = std::fs::read_to_string(&last).unwrap_or_default().trim().to_string();
    println!("snes-port: choose your ROM in the window that opens (A Link to the Past, USA).");
    let script = "Add-Type -AssemblyName System.Windows.Forms;         $d = New-Object System.Windows.Forms.OpenFileDialog;         $d.Title = 'snes-port: choose your ROM';         $d.Filter = 'SNES ROM (*.sfc;*.smc)|*.sfc;*.smc|All files (*.*)|*.*';         if ($env:SNES_PORT_START -and (Test-Path $env:SNES_PORT_START)) { $d.InitialDirectory = $env:SNES_PORT_START };         if ($d.ShowDialog() -eq 'OK') { [Console]::Out.Write($d.FileName) }";
    let out = Command::new("powershell").args(["-NoProfile", "-STA", "-Command", script]).env("SNES_PORT_START", start).output().ok()?;
    let chosen = PathBuf::from(String::from_utf8_lossy(&out.stdout).trim());
    if chosen.as_os_str().is_empty() {
        return None;
    }
    if let (Some(dir), Ok(())) = (chosen.parent(), std::fs::create_dir_all(cache)) {
        let _ = std::fs::write(&last, dir.to_string_lossy().as_bytes());
    }
    Some(chosen)
}

fn run_port(rom: PathBuf, cache: PathBuf, launch: bool, fast: bool, download_yes: bool, game_args: Vec<String>) -> i32 {
    let opts = Options { rom, cache, launch, fast, capture: false, game_args };
    let mut compiler = toolchain::find();
    if compiler.is_none() && !needs_no_build(&opts) {
        compiler = portable_compiler(&opts.cache, download_yes, &mut std::io::stdin().lock());
    }
    match build_port(&opts, compiler, find_sdl2()) {
        Ok(o) => o.game.map_or(0, |(code, _)| code),
        Err(f) => {
            eprintln!("{}", f.message);
            f.code
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    fn temp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("snes-port-test-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn the_second_run_does_not_build_and_a_new_version_does() {
        let cache = temp("cache");
        let calls = Cell::new(0);
        let mut build = |dir: &Path| -> Result<(), String> {
            calls.set(calls.get() + 1);
            std::fs::write(dir.join(exe_name()), b"exe").map_err(|e| e.to_string())
        };
        let a = ensure_built(&cache, "777aac2f-aaaa", "777aac2f-", &mut build).unwrap();
        assert!(a.rebuilt && calls.get() == 1 && a.dir.join(exe_name()).exists());
        let b = ensure_built(&cache, "777aac2f-aaaa", "777aac2f-", &mut build).unwrap();
        assert!(!b.rebuilt && calls.get() == 1, "the second run builds nothing");
        // another version (a different runtime): rebuilt, and the old one goes away
        let other = ensure_built(&cache, "777aac2f-bbbb", "777aac2f-", &mut build).unwrap();
        assert!(other.rebuilt && calls.get() == 2);
        assert!(!cache.join("777aac2f-aaaa").exists() && cache.join("777aac2f-bbbb").exists());
        // another ROM's cache is not touched
        ensure_built(&cache, "11111111-bbbb", "11111111-", &mut build).unwrap();
        ensure_built(&cache, "777aac2f-cccc", "777aac2f-", &mut build).unwrap();
        assert!(cache.join("11111111-bbbb").exists());
        // no temporary folders left behind
        assert!(std::fs::read_dir(&cache).unwrap().flatten().all(|e| !e.file_name().to_string_lossy().contains(".tmp")));
        let _ = std::fs::remove_dir_all(&cache);
    }

    #[test]
    fn a_failed_build_leaves_nothing_in_the_cache() {
        let cache = temp("failed");
        let mut build = |dir: &Path| -> Result<(), String> {
            std::fs::write(dir.join("half.c"), b"x").unwrap();
            Err("boom".to_string())
        };
        let r = ensure_built(&cache, "777aac2f-aaaa", "777aac2f-", &mut build);
        assert_eq!(r.err().as_deref(), Some("boom"));
        assert_eq!(std::fs::read_dir(&cache).unwrap().count(), 0);
        // an interrupted build (a folder without the mark) is not taken for a build
        std::fs::create_dir_all(cache.join("777aac2f-bbbb")).unwrap();
        let mut ok = |dir: &Path| -> Result<(), String> { std::fs::write(dir.join(exe_name()), b"exe").map_err(|e| e.to_string()) };
        assert!(ensure_built(&cache, "777aac2f-bbbb", "777aac2f-", &mut ok).unwrap().rebuilt);
        let _ = std::fs::remove_dir_all(&cache);
    }

    #[test]
    fn the_version_changes_with_the_runtime_the_config_or_the_options() {
        let base = version_tag(&["0.1.0", "runtime v1", "config v1"]);
        assert_eq!(base, version_tag(&["0.1.0", "runtime v1", "config v1"]));
        assert_ne!(base, version_tag(&["0.1.0", "runtime v2", "config v1"]));
        assert_ne!(base, version_tag(&["0.1.0", "runtime v1", "config v2"]));
        assert_ne!(version_tag(&["ab", "c"]), version_tag(&["a", "bc"]), "the parts are told apart");
        assert_ne!(current_version(true), current_version(false), "a fast build is another build");
        assert_eq!(current_version(false).len(), 8);
    }

    /// 256 distinct-looking bytes per kilobyte: a stand-in ROM.
    fn fake_rom() -> Vec<u8> {
        (0..0x8000u32).map(|i| (i.wrapping_mul(2654435761) >> 13) as u8).collect()
    }

    #[test]
    fn rom_bytes_are_found_raw_or_as_hex_and_padding_is_not_taken_for_them() {
        let rom = fake_rom();
        let dir = temp("scan");
        std::fs::write(dir.join("clean.bin"), vec![0u8; 5000]).unwrap();
        std::fs::write(dir.join("padding.bin"), vec![0xFFu8; 5000]).unwrap();
        std::fs::write(dir.join("a.c"), "int x = 0x12; /* 0xAB */ int y[] = {0x01, 0x02};").unwrap();
        assert!(find_rom_bytes(&rom, &dir).is_none());
        let mut with = vec![1u8, 2, 3];
        with.extend_from_slice(&rom[0x1234..0x1234 + 32]);
        std::fs::write(dir.join("exe.bin"), &with).unwrap();
        assert_eq!(find_rom_bytes(&rom, &dir), Some(dir.join("exe.bin")));
        std::fs::remove_file(dir.join("exe.bin")).unwrap();
        let hex: String = rom[0x4000..0x4000 + 40].iter().map(|b| format!("0x{b:02X}, ")).collect();
        std::fs::write(dir.join("table.c"), format!("const unsigned char t[] = {{ {hex} }};")).unwrap();
        assert_eq!(find_rom_bytes(&rom, &dir), Some(dir.join("table.c")));
        // a ramp of the ROM in a file is not the ROM: any program has tables like that
        let mut ramp_rom = fake_rom();
        for (i, b) in ramp_rom[0x2000..0x2040].iter_mut().enumerate() {
            *b = 0x80u8.wrapping_add(i as u8);
        }
        std::fs::write(dir.join("table.c"), (0..64u8).map(|i| 0x80u8.wrapping_add(i)).collect::<Vec<u8>>()).unwrap();
        assert!(find_rom_bytes(&ramp_rom, &dir).is_none());
        // 31 bytes are not enough
        std::fs::write(dir.join("table.c"), &with[..34]).unwrap();
        assert!(find_rom_bytes(&rom, &dir).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn real_rom() -> Option<PathBuf> {
        let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("roms").join("alttp_usa.sfc");
        p.exists().then_some(p)
    }

    fn opts(rom: &Path, cache: &Path) -> Options {
        Options { rom: rom.to_path_buf(), cache: cache.to_path_buf(), launch: true, fast: false, capture: true, game_args: vec![] }
    }

    #[test]
    fn an_unknown_rom_stops_with_code_2_shows_its_crc32_and_creates_nothing() {
        let dir = temp("unknown");
        let rom: Vec<u8> = (0..0x8000u32).map(|i| (i.wrapping_mul(40503) >> 7) as u8).collect();
        std::fs::write(dir.join("mystery.sfc"), &rom).unwrap();
        let cache = dir.join("cache");
        let f = build_port(&opts(&dir.join("mystery.sfc"), &cache), toolchain::find(), find_sdl2()).err().expect("an unknown ROM must fail");
        assert_eq!(f.code, EXIT_UNKNOWN_ROM);
        assert!(f.message.contains(&format!("{:08X}", crc32(&rom))), "{}", f.message);
        assert!(f.message.contains("A Link to the Past") && f.message.contains("777AAC2F"), "the supported games are listed
{}", f.message);
        assert!(!cache.exists(), "nothing is created in the cache");
        // a file that is not a ROM at all
        std::fs::write(dir.join("tiny.sfc"), b"nope").unwrap();
        assert_eq!(build_port(&opts(&dir.join("tiny.sfc"), &cache), None, None).err().unwrap().code, 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn without_a_compiler_or_sdl2_it_explains_what_is_missing_and_creates_nothing() {
        let Some(real) = real_rom() else {
            eprintln!("skipped: no ROM in roms/");
            return;
        };
        let dir = temp("nocc");
        // a copier header does not stop the ROM from being recognised (it gets as far as asking for the compiler)
        let mut headered = vec![0u8; 512];
        headered.extend(std::fs::read(&real).unwrap());
        std::fs::write(dir.join("headered.smc"), &headered).unwrap();
        let cache = dir.join("cache");
        let f = build_port(&opts(&dir.join("headered.smc"), &cache), None, None).err().expect("no compiler");
        assert_eq!(f.code, EXIT_NO_TOOLCHAIN);
        assert!(f.message.contains("no C compiler"), "{}", f.message);
        assert!(!cache.exists(), "nothing is created in the cache");
        if let Some(cc) = toolchain::find() {
            let f = build_port(&opts(&dir.join("headered.smc"), &cache), Some(cc), None).err().expect("no SDL2");
            assert_eq!(f.code, EXIT_NO_TOOLCHAIN);
            assert!(f.message.contains("SDL2"), "{}", f.message);
            assert!(!cache.exists());
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn it_builds_and_runs_the_game_from_the_rom_and_keeps_everything_local() {
        let Some(real) = real_rom() else {
            eprintln!("skipped: no ROM in roms/");
            return;
        };
        let (Some(cc), Some(sdl)) = (toolchain::find(), find_sdl2()) else {
            eprintln!("skipped: no C compiler or no SDL2 development files");
            return;
        };
        let dir = temp("e2e");
        let rom_path = dir.join("alttp.sfc"); // a copy, so the saved game of the test is not the player's
        std::fs::copy(&real, &rom_path).unwrap();
        let rom_bytes = std::fs::read(&rom_path).unwrap();
        let cache = dir.join("cache");
        let requests = download::network_requests();
        let mut o = opts(&rom_path, &cache);
        o.game_args = ["--no-window", "--frames", "300", "--mute"].map(String::from).to_vec();
        // first run: builds, and the game runs its 300 frames
        let first = build_port(&o, Some(cc), Some(sdl.clone())).unwrap_or_else(|f| panic!("{}", f.message));
        assert!(first.built.rebuilt);
        let (code, out) = first.game.expect("the game was started");
        assert_eq!(code, 0, "{out}");
        assert!(out.contains("frames: 300"), "{out}");
        // nothing of the ROM in what was built (2.1)
        assert!(find_rom_bytes(&rom_bytes, &cache).is_none());
        assert!(first.built.dir.join(exe_name()).exists());
        // second run: nothing is generated or compiled
        let t = std::time::Instant::now();
        let second = build_port(&o, toolchain::find(), Some(sdl)).unwrap_or_else(|f| panic!("{}", f.message));
        assert!(!second.built.rebuilt && second.game.is_some_and(|g| g.0 == 0));
        assert!(t.elapsed().as_secs() < 30, "the second run starts at once");
        // with a compiler on the machine, neither run touched the network (2.3)
        assert_eq!(download::network_requests(), requests);
        // the saved game stays next to the ROM and `clean` does not touch it or the ROM (2.2)
        std::fs::write(dir.join("alttp.srm"), b"my saved game").unwrap();
        let gone = clean(&cache, Some(crc32(&rom_bytes))).unwrap();
        assert_eq!(gone.len(), 1);
        assert!(!first.built.dir.exists());
        assert_eq!(std::fs::read(dir.join("alttp.srm")).unwrap(), b"my saved game");
        assert_eq!(std::fs::read(&rom_path).unwrap(), rom_bytes);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The portable compiler path (3.2), when `SNES_TCC_EXE` names a TinyCC (the download itself is tested in `download`).
    #[test]
    fn it_builds_the_game_with_tinycc() {
        let (Some(real), Some(sdl), Ok(tcc)) = (real_rom(), find_sdl2(), std::env::var("SNES_TCC_EXE")) else {
            eprintln!("skipped: needs the ROM, SDL2 and SNES_TCC_EXE");
            return;
        };
        let dir = temp("tcc");
        let rom_path = dir.join("alttp.sfc");
        std::fs::copy(&real, &rom_path).unwrap();
        let mut o = opts(&rom_path, &dir.join("cache"));
        o.game_args = ["--no-window", "--frames", "200", "--mute"].map(String::from).to_vec();
        let r = build_port(&o, Some(Compiler::Tcc(PathBuf::from(tcc))), Some(sdl)).unwrap_or_else(|f| panic!("{}", f.message));
        let (code, out) = r.game.expect("started");
        assert_eq!(code, 0, "{out}");
        assert!(out.contains("frames: 200"), "{out}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
