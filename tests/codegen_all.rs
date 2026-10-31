//! `codegen --all` writes exactly what the library generates in memory (`generate::generate_sources`), for the real
//! game. Skipped unless `SNES_GAME_PROFILE` and `SNES_GAME_ROM` name a profile and its ROM.

use snes_recomp::{games::GameProfile, generate, rom::Rom};
use std::path::PathBuf;

#[test]
fn codegen_all_writes_what_the_library_generates() {
    let (Ok(profile_path), Ok(rom_path)) = (std::env::var("SNES_GAME_PROFILE"), std::env::var("SNES_GAME_ROM")) else {
        eprintln!("skipped: SNES_GAME_PROFILE and SNES_GAME_ROM name no real game");
        return;
    };
    let (Ok(profile), Ok(rom)) = (GameProfile::load(&PathBuf::from(&profile_path)), Rom::load(&rom_path)) else {
        eprintln!("skipped: cannot read the profile or the ROM");
        return;
    };
    if rom.crc32() != profile.crc32 {
        eprintln!("skipped: the ROM is not the one of the profile");
        return;
    }
    let g = generate::generate_sources(rom.bytes(), &profile).unwrap();
    let dir = std::env::temp_dir().join(format!("snes-codegen-all-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_snes-recomp"))
        .args(["codegen", &rom_path, "--all", "-o", &dir.to_string_lossy(), "--game", &profile_path])
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    for s in g.sources.iter().chain(generate::runtime_files().iter()) {
        let on_disk = std::fs::read(dir.join(&s.name)).unwrap_or_else(|e| panic!("{}: {e}", s.name));
        assert!(on_disk == s.text.as_bytes(), "{} differs from what codegen --all wrote", s.name);
    }
    let _ = std::fs::remove_dir_all(&dir);
}
