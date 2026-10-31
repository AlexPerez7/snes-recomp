//! The C of a game, made in memory from the bytes of the ROM and the game's profile, without touching the disk.
//!
//! `snes-port` and `codegen --all` write what this returns; a build on another device (Android) gets it through the
//! C interface of `ffi`. Every platform goes through here, so a better generator reaches all of them alike.
//! What it returns derives from the ROM: it is written only to the user's cache or `out/`, never published.

use crate::codegen::{self, FullOutput};
use crate::config::Config;
use crate::disasm::Flags;
use crate::games::{GameProfile, Source};
use crate::rom::Rom;
use crate::{discover, graph};

/// The generated C of a game, ready to be written or compiled.
pub struct Generated {
    /// `snes_funcs.h`, `snes_entry.c`, `rom_BB.c` per bank and, under `game/`, the C of the profile (relative names
    /// with `/`).
    pub sources: Vec<Source>,
    /// The whole output of the generator, for the summary (functions and stubs).
    pub output: FullOutput,
}

impl Generated {
    /// Lines of generated C (`snes_funcs.h` and the banks), for the summary.
    pub fn lines(&self) -> usize {
        self.output.header.lines().count() + self.output.banks.values().map(|t| t.lines().count()).sum::<usize>()
    }
}

/// The config's entries, or the reset vector if there are none.
pub fn default_roots(rom: &Rom, cfg: &Config) -> Vec<(u32, Flags)> {
    if cfg.entries.is_empty() {
        vec![(rom.vectors().emu_reset as u32, Flags::RESET)]
    } else {
        cfg.entries.iter().map(|e| (e.addr, e.flags)).collect()
    }
}

/// The C of the game from `rom` with a config, plus the game's own C (`game`, put under `game/`). No check that the
/// config is the ROM's: `codegen --all --config` uses it for any ROM.
pub fn generate_with_config(rom: &Rom, cfg: &Config, game: &[Source]) -> Result<Generated, String> {
    let roots = default_roots(rom, cfg);
    let d = discover::discover(rom, cfg, &roots);
    let root_addrs: Vec<u32> = roots.iter().map(|r| r.0).collect();
    let g = graph::Graph::build(&d, &root_addrs);
    let output = codegen::Emitter::new(&d, &g).with_hooks(&cfg.hooks)?.generate_all();
    let mut sources = vec![
        Source { name: "snes_funcs.h".into(), text: output.header.clone() },
        Source { name: "snes_entry.c".into(), text: output.entry.clone() },
    ];
    for (bank, text) in &output.banks {
        sources.push(Source { name: format!("rom_{bank:02X}.c"), text: text.clone() });
    }
    for s in game {
        sources.push(Source { name: format!("game/{}", s.name), text: s.text.clone() });
    }
    Ok(Generated { sources, output })
}

/// The C of the game whose profile is `profile`, from the bytes of its ROM (a copier header is stripped). Fails, with
/// no sources, when the ROM is not the profile's or a hook calls a function that no `.c` of the profile defines.
pub fn generate_sources(rom: &[u8], profile: &GameProfile) -> Result<Generated, String> {
    let rom = Rom::from_bytes(rom.to_vec())?;
    if rom.crc32() != profile.crc32 {
        return Err(format!(
            "{}: the ROM has CRC32 {:08X}, the profile is for CRC32 {:08X} ({})",
            profile.origin,
            rom.crc32(),
            profile.crc32,
            profile.name
        ));
    }
    let cfg = Config::parse(&profile.config).map_err(|e| format!("{}: {e}", profile.origin))?;
    for h in &cfg.hooks {
        let defined = profile.sources.iter().any(|s| s.name.ends_with(".c") && s.text.contains(&format!("void {}(", h.call)));
        if !defined {
            return Err(format!(
                "{}: the hook at {:06X} calls `{}`, which no .c file of the game defines (void {}(Cpu *c))",
                profile.origin, h.addr, h.call, h.call
            ));
        }
    }
    generate_with_config(&rom, &cfg, &profile.sources).map_err(|e| format!("{}: {e}", profile.origin))
}

/// The files of the runtime that go next to the generated C (`snes_rt.h`, the runtime without video, the frontend
/// and `build_play.bat`). They are the program's own code: nothing in them comes from the ROM.
pub fn runtime_files() -> Vec<Source> {
    let f = |name: &str, text: &str| Source { name: name.to_string(), text: text.to_string() };
    vec![
        f("snes_rt.h", codegen::RUNTIME_HEADER),
        f("snes_headless.c", codegen::HEADLESS_RUNTIME),
        f("snes_core.inc", codegen::RUNTIME_CORE),
        f("snes_ppu.inc", codegen::RUNTIME_PPU),
        f("snes_spc.inc", codegen::RUNTIME_SPC),
        f("play/snes_play.c", codegen::PLAY_RUNTIME),
        f("play/play_cfg.h", codegen::PLAY_CFG),
        f("play/play_ui.h", codegen::PLAY_UI),
        f("build_play.bat", &codegen::BUILD_PLAY_BAT.replace('\r', "").replace('\n', "\r\n")), // cmd.exe wants CRLF
    ]
}

/// Writes `files` into `dir`, making the folders their names need.
pub fn write_files(files: &[Source], dir: &std::path::Path) -> Result<(), String> {
    for s in files {
        let path = dir.join(&s.name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
        }
        std::fs::write(&path, &s.text).map_err(|e| format!("{}: {e}", path.display()))?;
    }
    Ok(())
}

/// Writes the runtime files and the generated C into `dir`. Returns the number of lines of generated C.
pub fn write_sources(g: &Generated, dir: &std::path::Path) -> Result<usize, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    write_files(&runtime_files(), dir)?;
    write_files(&g.sources, dir)?;
    Ok(g.lines())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rom::test_rom;

    /// A ROM whose reset vector points at a `JSR $8010; STP` with an `RTS` at $8010.
    fn small_rom() -> Rom {
        test_rom(&[(0x8000, &[0x20, 0x10, 0x80, 0xDB]), (0x8010, &[0x60]), (0xFFFC, &[0x00, 0x80])])
    }

    fn profile_for(rom: &Rom, extra: &str, sources: Vec<Source>) -> GameProfile {
        let toml = format!("[game]\nid = \"t\"\nname = \"Test\"\ncrc32 = \"{:08X}\"\n{extra}", rom.crc32());
        GameProfile::from_text(&toml, sources, "games/t.toml").unwrap()
    }

    #[test]
    fn the_sources_are_the_generated_c_and_the_game_c() {
        let rom = small_rom();
        let game = vec![Source { name: "extra.c".into(), text: "int x;\n".into() }];
        let g = generate_sources(rom.bytes(), &profile_for(&rom, "", game)).unwrap();
        let names: Vec<&str> = g.sources.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, ["snes_funcs.h", "snes_entry.c", "rom_00.c", "game/extra.c"]);
        assert!(g.sources[2].text.contains("f_00_8010"), "{}", g.sources[2].text);
        assert!(g.sources[1].text.contains("case 0x008000"));
        assert_eq!(g.sources[3].text, "int x;\n");
        assert!(g.lines() > 0);
    }

    #[test]
    fn a_rom_of_another_game_gives_an_error_and_no_sources() {
        let rom = small_rom();
        let mut p = profile_for(&rom, "", vec![]);
        p.crc32 ^= 1;
        let e = generate_sources(rom.bytes(), &p).err().expect("the CRC32 does not match");
        assert!(e.contains(&format!("{:08X}", rom.crc32())) && e.contains(&format!("{:08X}", p.crc32)), "{e}");
    }

    #[test]
    fn a_hook_without_its_function_gives_the_error_of_snes_port() {
        let rom = small_rom();
        let p = profile_for(&rom, "[[hook]]\naddr = \"00:8010\"\nkind = \"before\"\ncall = \"my_hook\"\n", vec![]);
        let e = generate_sources(rom.bytes(), &p).err().expect("the hook has no function");
        assert_eq!(e, "games/t.toml: the hook at 008010 calls `my_hook`, which no .c file of the game defines (void my_hook(Cpu *c))");
    }

    #[test]
    fn generating_writes_nothing() {
        let list = |d: &std::path::Path| {
            let mut v: Vec<_> = std::fs::read_dir(d).map(|r| r.flatten().map(|e| e.file_name()).collect()).unwrap_or_default();
            v.sort();
            v
        };
        let here = std::env::current_dir().unwrap();
        let before = list(&here);
        let rom = small_rom();
        let g = generate_sources(rom.bytes(), &profile_for(&rom, "", vec![])).unwrap();
        assert!(!g.sources.is_empty());
        assert_eq!(list(&here), before, "generation must not create files");
    }
}

/// The C interface of the generator, for a build on another device (the Android app calls it through JNI). A result
/// holds either an error or the list of sources; every string is NUL-terminated UTF-8 and lives until `snes_gen_free`.
///
/// ```c
/// typedef struct SnesGenResult SnesGenResult;
/// SnesGenResult *snes_gen_generate(const uint8_t *rom, size_t rom_len, const char *profile_toml,
///                                  const char *const *game_names, const char *const *game_texts, size_t game_count);
/// const char *snes_gen_error(const SnesGenResult *r);      /* NULL when it went well */
/// size_t snes_gen_count(const SnesGenResult *r);
/// const char *snes_gen_name(const SnesGenResult *r, size_t i);   /* "rom_00.c", "game/dialog.c", ... */
/// const char *snes_gen_text(const SnesGenResult *r, size_t i);
/// void snes_gen_free(SnesGenResult *r);
/// ```
pub mod ffi {
    use super::generate_sources;
    use crate::games::{GameProfile, Source};
    use std::ffi::{c_char, CStr, CString};

    pub struct SnesGenResult {
        error: Option<CString>,
        files: Vec<(CString, CString)>,
    }

    fn c_string(s: &str) -> CString {
        CString::new(s.replace('\0', "")).expect("no NUL left")
    }

    /// # Safety
    /// `ptr` is NULL or a valid NUL-terminated string.
    unsafe fn text<'a>(ptr: *const c_char, what: &str) -> Result<&'a str, String> {
        if ptr.is_null() {
            return Err(format!("{what} is NULL"));
        }
        CStr::from_ptr(ptr).to_str().map_err(|_| format!("{what} is not UTF-8"))
    }

    unsafe fn run(rom: *const u8, rom_len: usize, toml: *const c_char, names: *const *const c_char, texts: *const *const c_char, count: usize) -> Result<Vec<Source>, String> {
        if rom.is_null() {
            return Err("the ROM is NULL".into());
        }
        let rom = std::slice::from_raw_parts(rom, rom_len);
        let toml = text(toml, "the profile")?;
        let mut sources = Vec::with_capacity(count);
        for i in 0..count {
            if names.is_null() || texts.is_null() {
                return Err("the C files of the game are NULL".into());
            }
            let name = text(*names.add(i), "the name of a C file of the game")?;
            let body = text(*texts.add(i), "a C file of the game")?;
            sources.push(Source { name: name.to_string(), text: body.to_string() });
        }
        let profile = GameProfile::from_text(toml, sources, "profile")?;
        Ok(generate_sources(rom, &profile)?.sources)
    }

    /// Generates the C of the game; never returns NULL. See the module for the interface.
    ///
    /// # Safety
    /// `rom` points at `rom_len` bytes, `profile_toml` is a NUL-terminated string, and `game_names`/`game_texts`
    /// point at `game_count` NUL-terminated strings each (they may be NULL when `game_count` is 0).
    #[no_mangle]
    pub unsafe extern "C" fn snes_gen_generate(
        rom: *const u8,
        rom_len: usize,
        profile_toml: *const c_char,
        game_names: *const *const c_char,
        game_texts: *const *const c_char,
        game_count: usize,
    ) -> *mut SnesGenResult {
        let result = std::panic::catch_unwind(|| run(rom, rom_len, profile_toml, game_names, game_texts, game_count))
            .unwrap_or_else(|_| Err("the generator failed (panic)".into()));
        let r = match result {
            Ok(files) => SnesGenResult { error: None, files: files.iter().map(|s| (c_string(&s.name), c_string(&s.text))).collect() },
            Err(e) => SnesGenResult { error: Some(c_string(&e)), files: Vec::new() },
        };
        Box::into_raw(Box::new(r))
    }

    /// # Safety
    /// `r` comes from `snes_gen_generate` and was not freed.
    #[no_mangle]
    pub unsafe extern "C" fn snes_gen_error(r: *const SnesGenResult) -> *const c_char {
        (&(*r).error).as_ref().map_or(std::ptr::null(), |e| e.as_ptr())
    }

    /// # Safety
    /// `r` comes from `snes_gen_generate` and was not freed.
    #[no_mangle]
    pub unsafe extern "C" fn snes_gen_count(r: *const SnesGenResult) -> usize {
        (&(*r).files).len()
    }

    /// # Safety
    /// `r` comes from `snes_gen_generate` and was not freed; NULL when `i` is out of range.
    #[no_mangle]
    pub unsafe extern "C" fn snes_gen_name(r: *const SnesGenResult, i: usize) -> *const c_char {
        (&(*r).files).get(i).map_or(std::ptr::null(), |f| f.0.as_ptr())
    }

    /// # Safety
    /// `r` comes from `snes_gen_generate` and was not freed; NULL when `i` is out of range.
    #[no_mangle]
    pub unsafe extern "C" fn snes_gen_text(r: *const SnesGenResult, i: usize) -> *const c_char {
        (&(*r).files).get(i).map_or(std::ptr::null(), |f| f.1.as_ptr())
    }

    /// # Safety
    /// `r` comes from `snes_gen_generate` (or is NULL) and is freed only once.
    #[no_mangle]
    pub unsafe extern "C" fn snes_gen_free(r: *mut SnesGenResult) {
        if !r.is_null() {
            drop(Box::from_raw(r));
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::generate::generate_sources;
        use crate::rom::test_rom;

        #[test]
        fn the_c_interface_gives_the_same_sources_and_reports_errors() {
            let rom = test_rom(&[(0x8000, &[0x20, 0x10, 0x80, 0xDB]), (0x8010, &[0x60]), (0xFFFC, &[0x00, 0x80])]);
            let toml = format!("[game]\nid = \"t\"\nname = \"Test\"\ncrc32 = \"{:08X}\"\n", rom.crc32());
            let ctoml = CString::new(toml.clone()).unwrap();
            let (n, t) = (CString::new("extra.c").unwrap(), CString::new("int x;\n").unwrap());
            let (names, texts) = ([n.as_ptr()], [t.as_ptr()]);
            let expected = generate_sources(rom.bytes(), &GameProfile::from_text(&toml, vec![Source { name: "extra.c".into(), text: "int x;\n".into() }], "p").unwrap()).unwrap();
            unsafe {
                let r = snes_gen_generate(rom.bytes().as_ptr(), rom.len(), ctoml.as_ptr(), names.as_ptr(), texts.as_ptr(), 1);
                assert!(snes_gen_error(r).is_null());
                assert_eq!(snes_gen_count(r), expected.sources.len());
                for (i, s) in expected.sources.iter().enumerate() {
                    assert_eq!(CStr::from_ptr(snes_gen_name(r, i)).to_str().unwrap(), s.name);
                    assert_eq!(CStr::from_ptr(snes_gen_text(r, i)).to_str().unwrap(), s.text);
                }
                assert!(snes_gen_name(r, 99).is_null());
                snes_gen_free(r);
                // another ROM: an error and no sources
                let other = [0u8; 0x8000];
                let r = snes_gen_generate(other.as_ptr(), other.len(), ctoml.as_ptr(), std::ptr::null(), std::ptr::null(), 0);
                let e = CStr::from_ptr(snes_gen_error(r)).to_str().unwrap().to_string();
                assert!(e.contains("CRC32"), "{e}");
                assert_eq!(snes_gen_count(r), 0);
                snes_gen_free(r);
            }
        }
    }
}
