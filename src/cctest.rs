//! Test support: compile and run C sources with whatever compiler the machine has.
//! Tests that need a compiler skip themselves (and say so) when `find` returns `None`.

use std::path::{Path, PathBuf};
use std::process::Command;

pub enum Compiler {
    /// A GCC-compatible driver on the PATH (or named by `CC`).
    Gnu(String),
    /// MSVC, through the `vcvars64.bat` of a Visual Studio installation.
    Msvc(PathBuf),
}

fn works(cc: &str) -> bool {
    Command::new(cc).arg("--version").output().is_ok_and(|o| o.status.success())
}

#[cfg(windows)]
fn find_msvc() -> Option<PathBuf> {
    let vswhere = r"C:\Program Files (x86)\Microsoft Visual Studio\Installer\vswhere.exe";
    let out = Command::new(vswhere)
        .args(["-latest", "-products", "*", "-property", "installationPath"])
        .output()
        .ok()?;
    let root = String::from_utf8_lossy(&out.stdout).trim().to_string();
    let bat = PathBuf::from(root).join(r"VC\Auxiliary\Build\vcvars64.bat");
    bat.exists().then_some(bat)
}

#[cfg(not(windows))]
fn find_msvc() -> Option<PathBuf> {
    None
}

pub fn find() -> Option<Compiler> {
    if let Ok(cc) = std::env::var("CC") {
        if works(&cc) {
            return Some(Compiler::Gnu(cc));
        }
    }
    for cc in ["gcc", "clang", "cc"] {
        if works(cc) {
            return Some(Compiler::Gnu(cc.to_string()));
        }
    }
    find_msvc().map(Compiler::Msvc)
}

/// Declares a skipped test and returns early from the caller when there is no compiler.
#[macro_export]
macro_rules! need_compiler {
    () => {
        match $crate::cctest::find() {
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
        let dir = std::env::temp_dir().join(format!("snes-recomp-{}-{}", std::process::id(), name));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        for (file, text) in files {
            std::fs::write(dir.join(file), text).map_err(|e| e.to_string())?;
        }
        let sources: Vec<&str> = files.iter().map(|f| f.0).filter(|f| f.ends_with(".c")).collect();
        let result = match self {
            Compiler::Gnu(cc) => self.run_gnu(cc, &dir, &sources, defines, args),
            Compiler::Msvc(vcvars) => self.run_msvc(vcvars, &dir, &sources, defines, args),
        };
        let _ = std::fs::remove_dir_all(&dir);
        result
    }

    fn run_gnu(&self, cc: &str, dir: &Path, sources: &[&str], defines: &[&str], args: &[&str]) -> Result<String, String> {
        let mut cmd = Command::new(cc);
        cmd.current_dir(dir).args(["-std=c11", "-Wall", "-O1", "-o", "prog"]);
        for d in defines {
            cmd.arg(format!("-D{d}"));
        }
        let build = cmd.args(sources).output().map_err(|e| e.to_string())?;
        if !build.status.success() {
            return Err(String::from_utf8_lossy(&build.stderr).into_owned());
        }
        let run = Command::new(dir.join("prog")).current_dir(dir).args(args).output().map_err(|e| e.to_string())?;
        finish(run)
    }

    fn run_msvc(&self, vcvars: &Path, dir: &Path, sources: &[&str], defines: &[&str], args: &[&str]) -> Result<String, String> {
        let defs: String = defines.iter().map(|d| format!(" /D{d}")).collect();
        let bat = format!(
            "@echo off\r\ncall \"{}\" >nul 2>nul || exit /b 1\r\ncd /d \"{}\"\r\n\
             cl /nologo /MP /W3 /TC /std:c11{} /Fe:prog.exe {} >build.log 2>&1\r\n\
             if errorlevel 1 (type build.log & exit /b 1)\r\n.\\prog.exe{}\r\n",
            vcvars.display(),
            dir.display(),
            defs,
            sources.join(" "),
            args.iter().map(|a| format!(" \"{a}\"")).collect::<String>()
        );
        let script = dir.join("build.bat");
        std::fs::write(&script, bat).map_err(|e| e.to_string())?;
        let run = Command::new("cmd").arg("/C").arg(&script).output().map_err(|e| e.to_string())?;
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
