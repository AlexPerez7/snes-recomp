//! `snes-port.exe` is the program of the windows subsystem (no console on a double click, see `launcher_win`) with a
//! manifest for the look of the controls and the screen scaling; `snes-recomp.exe` stays a console program. Both carry the icon
//! of `assets/snes-port.ico` as resource 1 (written here as a `.res` file, so no tool or crate is needed).

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=assets/snes-port.manifest");
    println!("cargo:rerun-if-changed=assets/snes-port.ico");
    let target_windows_msvc = std::env::var("CARGO_CFG_TARGET_OS").is_ok_and(|v| v == "windows") && std::env::var("CARGO_CFG_TARGET_ENV").is_ok_and(|v| v == "msvc");
    if !target_windows_msvc {
        return;
    }
    let manifest = std::path::Path::new(&std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("assets").join("snes-port.manifest");
    let res = std::path::Path::new(&std::env::var("OUT_DIR").unwrap()).join("icon.res");
    let ico = std::fs::read(std::path::Path::new(&std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("assets").join("snes-port.ico")).expect("assets/snes-port.ico");
    std::fs::write(&res, icon_res(&ico)).expect("icon.res");
    println!("cargo:rustc-link-arg-bins={}", res.display());
    println!("cargo:rustc-link-arg-bin=snes-port=/SUBSYSTEM:WINDOWS");
    println!("cargo:rustc-link-arg-bin=snes-port=/ENTRY:mainCRTStartup");
    println!("cargo:rustc-link-arg-bin=snes-port=/MANIFEST:EMBED");
    println!("cargo:rustc-link-arg-bin=snes-port=/MANIFESTINPUT:{}", manifest.display());
}

/// One entry of a `.res` file: type and name are numeric ids, the data is padded to 4 bytes.
fn res_entry(kind: u16, id: u16, data: &[u8]) -> Vec<u8> {
    let mut v = Vec::new();
    v.extend((data.len() as u32).to_le_bytes());
    v.extend(32u32.to_le_bytes()); // header size
    v.extend([0xFF, 0xFF]);
    v.extend(kind.to_le_bytes());
    v.extend([0xFF, 0xFF]);
    v.extend(id.to_le_bytes());
    v.extend([0u8; 4]); // data version
    v.extend(0x1030u16.to_le_bytes()); // MOVEABLE | PURE | DISCARDABLE
    v.extend(0x0409u16.to_le_bytes()); // language
    v.extend([0u8; 8]); // version, characteristics
    v.extend(data);
    while v.len() % 4 != 0 {
        v.push(0);
    }
    v
}

/// The icon file as resources: one RT_ICON per image (ids 1..) and the RT_GROUP_ICON (id 1) that lists them.
fn icon_res(ico: &[u8]) -> Vec<u8> {
    let u16_at = |o: usize| u16::from_le_bytes([ico[o], ico[o + 1]]);
    let u32_at = |o: usize| u32::from_le_bytes([ico[o], ico[o + 1], ico[o + 2], ico[o + 3]]);
    let count = u16_at(4) as usize;
    let mut out = res_entry(0, 0, &[]); // the empty entry every .res starts with
    let mut group = vec![0, 0, 1, 0];
    group.extend((count as u16).to_le_bytes());
    for i in 0..count {
        let e = 6 + 16 * i;
        let (size, offset) = (u32_at(e + 8) as usize, u32_at(e + 12) as usize);
        out.extend(res_entry(3, i as u16 + 1, &ico[offset..offset + size]));
        group.extend(&ico[e..e + 12]);
        group.extend((i as u16 + 1).to_le_bytes());
    }
    out.extend(res_entry(14, 1, &group));
    out
}
