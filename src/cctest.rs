//! Test support: compile and run C sources with whatever compiler the machine has.
//! Tests that need a compiler skip themselves (and say so) when `find` returns `None`.

use std::path::{Path, PathBuf};
use std::process::Command;

pub use crate::toolchain::{find, Compiler};
use crate::toolchain::msvc_env;

/// The SDL2 development package for Visual C++ (the folder with `include` and `lib`): `SDL2_DIR`, or the first
/// `out/sdl2/SDL2-*` of the project. `None` skips the tests that need SDL2.
pub fn sdl2_dir() -> Option<std::path::PathBuf> {
    if let Ok(d) = std::env::var("SDL2_DIR") {
        let p = std::path::PathBuf::from(d);
        return p.join("include").join("SDL.h").exists().then_some(p);
    }
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("out").join("sdl2");
    std::fs::read_dir(root).ok()?.flatten().map(|e| e.path()).find(|p| p.join("include").join("SDL.h").exists())
}

/// A real game for the tests that need one: the profile named by `SNES_GAME_PROFILE` (its `.toml`, with the folder of C
/// next to it) and the ROM named by `SNES_GAME_ROM`.
pub struct RealGame {
    pub profile: crate::games::GameProfile,
    pub profile_path: PathBuf,
    pub rom_path: PathBuf,
    pub rom: crate::rom::Rom,
}

/// The real game, or `None` (the test is skipped) when the variables are not set, the files cannot be read or the
/// ROM is not the one the profile is for.
pub fn real_game() -> Option<RealGame> {
    let (Ok(profile_path), Ok(rom_path)) = (std::env::var("SNES_GAME_PROFILE"), std::env::var("SNES_GAME_ROM")) else {
        eprintln!("skipped: SNES_GAME_PROFILE and SNES_GAME_ROM name no real game");
        return None;
    };
    let (profile_path, rom_path) = (PathBuf::from(profile_path), PathBuf::from(rom_path));
    let profile = crate::games::GameProfile::load(&profile_path).map_err(|e| eprintln!("skipped: {e}")).ok()?;
    let rom = crate::rom::Rom::load(&rom_path).map_err(|e| eprintln!("skipped: {e}")).ok()?;
    if rom.crc32() != profile.crc32 {
        eprintln!("skipped: {} is not the ROM of the profile {} (CRC32 {:08X})", rom_path.display(), profile.id, profile.crc32);
        return None;
    }
    Some(RealGame { profile, profile_path, rom_path, rom })
}

/// `real_game` when its profile is `id` (for the tests that know addresses of one game).
pub fn real_game_id(id: &str) -> Option<RealGame> {
    let game = real_game()?;
    if game.profile.id != id {
        eprintln!("skipped: the real game is {}, the test is for {id}", game.profile.id);
        return None;
    }
    Some(game)
}

/// Declares a skipped test and returns early from the caller when there is no compiler.
#[macro_export]
macro_rules! need_compiler {
    () => {
        match $crate::toolchain::find() {
            Some(cc) => cc,
            None => {
                eprintln!("skipped: no C compiler found (set CC, or install gcc, clang or Visual Studio)");
                return;
            }
        }
    };
}

impl Compiler {
    /// Writes `files` (name, contents) into a fresh directory, compiles every `.c` among them
    /// with the given `-D` names, runs the program and returns its standard output.
    pub fn compile_run(&self, name: &str, files: &[(&str, &str)], defines: &[&str]) -> Result<String, String> {
        let bytes: Vec<(&str, &[u8])> = files.iter().map(|f| (f.0, f.1.as_bytes())).collect();
        self.compile_run_with(name, &bytes, defines, &[])
    }

    /// Like `compile_run`, but the files may be binary and the program gets `args`. The program runs
    /// in the directory that holds the files, so the arguments can name them.
    pub fn compile_run_with(&self, name: &str, files: &[(&str, &[u8])], defines: &[&str], args: &[&str]) -> Result<String, String> {
        self.compile_run_sdl(name, files, defines, args, None)
    }

    /// Like `compile_run_with`, also linking SDL2 from the development package at `sdl` (MSVC only; see `sdl2_dir`).
    pub fn compile_run_sdl(&self, name: &str, files: &[(&str, &[u8])], defines: &[&str], args: &[&str], sdl: Option<&Path>) -> Result<String, String> {
        self.compile_run_sdl_times(name, files, defines, args, sdl, 1)
    }

    /// Like `compile_run_sdl`, but runs the program `runs` times in the same folder (so it can keep files between runs)
    /// and returns the outputs one after another.
    pub fn compile_run_sdl_times(&self, name: &str, files: &[(&str, &[u8])], defines: &[&str], args: &[&str], sdl: Option<&Path>, runs: usize) -> Result<String, String> {
        let dir = std::env::temp_dir().join(format!("snes-recomp-{}-{}", std::process::id(), name));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        for (file, text) in files {
            std::fs::write(dir.join(file), text).map_err(|e| e.to_string())?;
        }
        let sources: Vec<&str> = files.iter().map(|f| f.0).filter(|f| f.ends_with(".c")).collect();
        let result = match self {
            Compiler::Gnu(cc) => self.run_gnu(cc, &dir, &sources, defines, args, runs),
            Compiler::Msvc(vcvars) => self.run_msvc(vcvars, &dir, &sources, defines, args, sdl, runs),
            Compiler::Tcc(_) => Err("TinyCC is only used by snes-port".to_string()),
        };
        let _ = std::fs::remove_dir_all(&dir);
        result
    }

    fn run_gnu(&self, cc: &str, dir: &Path, sources: &[&str], defines: &[&str], args: &[&str], runs: usize) -> Result<String, String> {
        let mut cmd = Command::new(cc);
        cmd.current_dir(dir).args(["-std=c11", "-Wall", "-O1", "-o", "prog"]);
        for d in defines {
            cmd.arg(format!("-D{d}"));
        }
        let build = cmd.args(sources).arg("-lm").output().map_err(|e| e.to_string())?;
        if !build.status.success() {
            return Err(String::from_utf8_lossy(&build.stderr).into_owned());
        }
        let mut all = String::new();
        for _ in 0..runs {
            let run = Command::new(dir.join("prog")).current_dir(dir).args(args).output().map_err(|e| e.to_string())?;
            all.push_str(&finish(run)?);
        }
        Ok(all)
    }

    fn run_msvc(&self, vcvars: &Path, dir: &Path, sources: &[&str], defines: &[&str], args: &[&str], sdl: Option<&Path>, runs: usize) -> Result<String, String> {
        let env = msvc_env(vcvars)?;
        let mut defs: String = defines.iter().map(|d| format!(" /D{d}")).collect();
        let mut link = String::new();
        if let Some(sdl) = sdl {
            defs.push_str(&format!(" /I\"{}\"", sdl.join("include").display()));
            link = format!(" /link /LIBPATH:\"{}\" SDL2.lib", sdl.join("lib").join("x64").display());
            std::fs::copy(sdl.join("lib").join("x64").join("SDL2.dll"), dir.join("SDL2.dll")).map_err(|e| e.to_string())?;
        }
        let bat = format!(
            "@echo off\r\ncd /d \"{}\"\r\n\
             cl /nologo /MP /W3 /TC /std:c11{} /Fe:prog.exe {}{} >build.log 2>&1\r\n\
             if errorlevel 1 (type build.log & exit /b 1)\r\n{}",
            dir.display(),
            defs,
            sources.join(" "),
            link,
            format!(".\\prog.exe{}\r\nif errorlevel 1 exit /b %errorlevel%\r\n", args.iter().map(|a| format!(" \"{a}\"")).collect::<String>()).repeat(runs)
        );
        let script = dir.join("build.bat");
        std::fs::write(&script, bat).map_err(|e| e.to_string())?;
        let run = Command::new("cmd").arg("/C").arg(&script).envs(env.iter().map(|(k, v)| (k, v))).output().map_err(|e| e.to_string())?;
        finish(run)
    }
}

fn finish(out: std::process::Output) -> Result<String, String> {
    let stdout = String::from_utf8_lossy(&out.stdout).replace("\r\n", "\n");
    if out.status.success() {
        Ok(stdout)
    } else {
        Err(format!("exit {:?}\n{}{}", out.status.code(), stdout, String::from_utf8_lossy(&out.stderr)))
    }
}

/// Bus and hooks for a flat 16 MiB memory; `main.c` supplies `main`.
pub const MOCK_RUNTIME: &str = r#"
#include "snes_rt.h"
#include <stdio.h>
#include <stdlib.h>

static uint8_t mem[0x1000000];
uint8_t bus_read8(uint32_t a) { return mem[a & 0xFFFFFF]; }
void bus_write8(uint32_t a, uint8_t v) { mem[a & 0xFFFFFF] = v; }
void rt_unresolved(Cpu *c, uint32_t site, uint32_t target) {
    (void)c;
    printf("UNRESOLVED site=%06X target=%06X\n", (unsigned)site, (unsigned)target);
}
void rt_flag_mismatch(Cpu *c, uint32_t addr) { (void)c; printf("FLAG_MISMATCH %06X\n", (unsigned)addr); }
void rt_unsupported(Cpu *c, uint32_t entry) { (void)c; printf("UNSUPPORTED %06X\n", (unsigned)entry); }
void rt_wai(Cpu *c) { (void)c; printf("WAI\n"); }
void rt_rti(Cpu *c) { rt_rti_pop(c); }
void rt_lost_return(Cpu *c) { (void)c; printf("LOST_RETURN\n"); }
void rt_stp(Cpu *c) { (void)c; printf("STP\n"); }
void rt_brk(Cpu *c) { (void)c; printf("BRK\n"); }
void rt_cop(Cpu *c) { (void)c; printf("COP\n"); }
void rt_wdm(Cpu *c) { (void)c; }
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_program_gets_binary_files_and_arguments() {
        let Some(cc) = find() else {
            eprintln!("skipped: no C compiler found");
            return;
        };
        let main = br#"#include <stdio.h>
int main(int argc, char **argv) {
    FILE *f = fopen(argv[1], "rb");
    int a = fgetc(f), b = fgetc(f), c = fgetc(f);
    printf("%d %s %02X %02X %02X\n", argc, argv[2], a, b, c);
    return 0;
}
"#;
        let files: [(&str, &[u8]); 2] = [("main.c", main), ("data.bin", &[0x00, 0xFF, 0x7E])];
        let out = cc.compile_run_with("cctest-args", &files, &[], &["data.bin", "hello"]).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(out, "3 hello 00 FF 7E\n");
    }
}
