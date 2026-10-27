//! `snes-port.exe` is the program of the windows subsystem (no console on a double click, see `launcher_win`) with a
//! manifest for the look of the controls and the screen scaling; `snes-recomp.exe` stays a console program.

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=assets/snes-port.manifest");
    let target_windows_msvc = std::env::var("CARGO_CFG_TARGET_OS").is_ok_and(|v| v == "windows") && std::env::var("CARGO_CFG_TARGET_ENV").is_ok_and(|v| v == "msvc");
    if !target_windows_msvc {
        return;
    }
    let manifest = std::path::Path::new(&std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("assets").join("snes-port.manifest");
    println!("cargo:rustc-link-arg-bin=snes-port=/SUBSYSTEM:WINDOWS");
    println!("cargo:rustc-link-arg-bin=snes-port=/ENTRY:mainCRTStartup");
    println!("cargo:rustc-link-arg-bin=snes-port=/MANIFEST:EMBED");
    println!("cargo:rustc-link-arg-bin=snes-port=/MANIFESTINPUT:{}", manifest.display());
}
