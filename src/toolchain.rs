//! The C compiler of the machine: `CC`, gcc, clang, cc or a Visual Studio installation (through `vcvars64.bat`).
//! The port builder uses it to build the game; the tests use it to compile and run the generated C.

use std::path::PathBuf;
use std::process::Command;

pub enum Compiler {
    /// A GCC-compatible driver on the PATH (or named by `CC`).
    Gnu(String),
    /// MSVC, through the `vcvars64.bat` of a Visual Studio installation.
    Msvc(PathBuf),
    /// The TinyCC downloaded into the cache (`download::install`); `find` never returns it.
    Tcc(PathBuf),
}

pub(crate) fn works(cc: &str) -> bool {
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

/// The environment `vcvars64.bat` sets up, computed once: running that script in parallel from several
/// tests is racy, so each build reuses the variables instead of calling it again.
#[cfg(windows)]
pub(crate) fn msvc_env(vcvars: &std::path::Path) -> Result<&'static Vec<(String, String)>, String> {
    static ENV: std::sync::OnceLock<Result<Vec<(String, String)>, String>> = std::sync::OnceLock::new();
    ENV.get_or_init(|| {
        use std::os::windows::process::CommandExt;
        let out = Command::new("cmd")
            .raw_arg(format!("/S /C \"call \"{}\" >nul 2>nul && set\"", vcvars.display()))
            .output()
            .map_err(|e| e.to_string())?;
        if !out.status.success() {
            return Err("vcvars64.bat failed".to_string());
        }
        let text = String::from_utf8_lossy(&out.stdout);
        Ok(text.lines().filter_map(|l| l.split_once('=')).filter(|(k, _)| !k.is_empty()).map(|(k, v)| (k.to_string(), v.trim_end().to_string())).collect())
    })
    .as_ref()
    .map_err(|e| e.clone())
}


#[cfg(not(windows))]
pub(crate) fn msvc_env(_vcvars: &std::path::Path) -> Result<&'static Vec<(String, String)>, String> {
    Err("MSVC is only available on Windows".to_string())
}
