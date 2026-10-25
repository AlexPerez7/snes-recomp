//! Known games: each one is a profile, a `.toml` (see `config.rs`) with a `[game]` section that says which ROM it is
//! for, and optionally a folder next to it with the same name (`games/<id>/`) holding C code of the game that is
//! compiled together with the generated C.

use crate::config::Config;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// A file of C code of a game.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Source {
    pub name: String,
    pub text: String,
}

#[derive(Debug, Clone)]
pub struct GameProfile {
    pub id: String,
    pub name: String,
    pub crc32: u32,
    /// The text of the profile's `.toml`.
    pub config: String,
    /// The `*.c` and `*.h` files of the profile's folder.
    pub sources: Vec<Source>,
    /// Where it came from, for messages: `games/<id>.toml` or the path given to `--game`.
    pub origin: String,
}

impl GameProfile {
    /// A profile from the text of its `.toml` and its C files; `origin` names it in the errors.
    pub fn from_text(config: &str, sources: Vec<Source>, origin: &str) -> Result<GameProfile, String> {
        let cfg = Config::parse(config).map_err(|e| format!("{origin}: {e}"))?;
        let Some(game) = cfg.game else {
            return Err(format!("{origin}: the profile has no [game] section (id, name and crc32)"));
        };
        Ok(GameProfile { id: game.id, name: game.name, crc32: game.crc32, config: config.to_string(), sources, origin: origin.to_string() })
    }

    /// A profile from a `.toml` file and the `*.c`/`*.h` files of the folder with the same name next to it, if any.
    pub fn load(path: &Path) -> Result<GameProfile, String> {
        let config = std::fs::read_to_string(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        let mut sources = Vec::new();
        if let Some(dir) = path.file_stem().map(|s| path.with_file_name(s)).filter(|d| d.is_dir()) {
            let entries = std::fs::read_dir(&dir).map_err(|e| format!("cannot read {}: {e}", dir.display()))?;
            for entry in entries {
                let p = entry.map_err(|e| format!("cannot read {}: {e}", dir.display()))?.path();
                let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                if p.is_file() && (name.ends_with(".c") || name.ends_with(".h")) {
                    let text = std::fs::read_to_string(&p).map_err(|e| format!("cannot read {}: {e}", p.display()))?;
                    sources.push(Source { name, text });
                }
            }
            sources.sort_by(|a, b| a.name.cmp(&b.name));
        }
        GameProfile::from_text(&config, sources, &path.display().to_string())
    }
}

/// The profiles that ship inside the program: the text of each `.toml` and of the C files of its folder.
fn builtin_texts() -> Vec<(&'static str, &'static str, Vec<Source>)> {
    let source = |name: &str, text: &str| Source { name: name.to_string(), text: text.to_string() };
    vec![("games/alttp_usa.toml", include_str!("../games/alttp_usa.toml"), vec![source("dialog.c", include_str!("../games/alttp_usa/dialog.c"))])]
}

/// The profiles in use: the embedded ones, with those given with `--game` on top.
#[derive(Debug, Clone)]
pub struct Catalog {
    profiles: Vec<GameProfile>,
}

impl Catalog {
    /// The embedded profiles only.
    pub fn builtin() -> Result<Catalog, String> {
        let mut profiles: Vec<GameProfile> = Vec::new();
        for (origin, text, sources) in builtin_texts() {
            let p = GameProfile::from_text(text, sources, origin)?;
            if let Some(o) = profiles.iter().find(|o| o.crc32 == p.crc32) {
                return Err(format!("{} and {} are both for CRC32 {:08X}", o.origin, p.origin, p.crc32));
            }
            profiles.push(p);
        }
        Ok(Catalog { profiles })
    }

    /// Adds `external` profiles: one replaces an embedded profile for the same ROM, but two of them for the same ROM
    /// are an error.
    pub fn with_external(self, external: Vec<GameProfile>) -> Result<Catalog, String> {
        let mut profiles: Vec<GameProfile> = Vec::new();
        for p in external {
            if let Some(o) = profiles.iter().find(|o| o.crc32 == p.crc32) {
                return Err(format!("{} and {} are both for CRC32 {:08X}", o.origin, p.origin, p.crc32));
            }
            profiles.push(p);
        }
        let embedded: Vec<GameProfile> = self.profiles.into_iter().filter(|e| !profiles.iter().any(|p| p.crc32 == e.crc32)).collect();
        profiles.extend(embedded);
        Ok(Catalog { profiles })
    }

    pub fn profiles(&self) -> &[GameProfile] {
        &self.profiles
    }

    pub fn identify(&self, crc: u32) -> Option<&GameProfile> {
        self.profiles.iter().find(|g| g.crc32 == crc)
    }
}

/// The embedded profiles, parsed once.
pub fn builtin() -> &'static Catalog {
    static CATALOG: OnceLock<Catalog> = OnceLock::new();
    CATALOG.get_or_init(|| Catalog::builtin().unwrap_or_else(|e| panic!("bad embedded game profile: {e}")))
}

/// The embedded profile for the ROM with this CRC32.
pub fn identify(crc: u32) -> Option<&'static GameProfile> {
    builtin().identify(crc)
}

/// Paths of the `.toml` files given with `--game`, loaded.
pub fn load_external(paths: &[PathBuf]) -> Result<Vec<GameProfile>, String> {
    paths.iter().map(|p| GameProfile::load(p)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn toml(id: &str, crc: &str) -> String {
        format!("[game]\nid=\"{id}\"\nname=\"Game {id}\"\ncrc32=\"{crc}\"\n")
    }

    #[test]
    fn the_embedded_profile_is_alttp() {
        let p = identify(0x777A_AC2F).unwrap();
        assert_eq!(p.id, "alttp_usa");
        assert!(p.name.contains("A Link to the Past"));
        assert!(identify(0x1234_5678).is_none());
    }

    #[test]
    fn a_profile_needs_a_complete_game_section() {
        let e = GameProfile::from_text("[[entry]]\naddr=\"00:8000\"", vec![], "x.toml").unwrap_err();
        assert!(e.contains("x.toml") && e.contains("[game]"), "{e}");
        let e = GameProfile::from_text("[game]\nid=\"g\"\nname=\"n\"", vec![], "y.toml").unwrap_err();
        assert!(e.contains("y.toml"), "{e}");
        assert!(GameProfile::from_text(&toml("g", "0000ABCD"), vec![], "z.toml").is_ok());
    }

    #[test]
    fn two_external_profiles_for_one_rom_are_an_error_but_one_replaces_an_embedded_one() {
        let a = GameProfile::from_text(&toml("a", "0000ABCD"), vec![], "a.toml").unwrap();
        let b = GameProfile::from_text(&toml("b", "0000ABCD"), vec![], "b.toml").unwrap();
        let e = Catalog::builtin().unwrap().with_external(vec![a.clone(), b]).unwrap_err();
        assert!(e.contains("a.toml") && e.contains("b.toml") && e.contains("0000ABCD"), "{e}");
        let over = GameProfile::from_text(&toml("mine", "777AAC2F"), vec![], "mine.toml").unwrap();
        let c = Catalog::builtin().unwrap().with_external(vec![a, over]).unwrap();
        assert_eq!(c.identify(0x777A_AC2F).unwrap().id, "mine");
        assert_eq!(c.identify(0xABCD).unwrap().id, "a");
        assert_eq!(c.profiles().len(), 2);
    }

    #[test]
    fn a_profile_loads_the_c_files_of_the_folder_next_to_it() {
        let dir = std::env::temp_dir().join(format!("snes-recomp-games-{}-folder", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("g")).unwrap();
        std::fs::write(dir.join("g.toml"), toml("g", "0000ABCD")).unwrap();
        std::fs::write(dir.join("g").join("b.c"), "int b;").unwrap();
        std::fs::write(dir.join("g").join("a.h"), "int a;").unwrap();
        std::fs::write(dir.join("g").join("notes.txt"), "x").unwrap();
        let p = GameProfile::load(&dir.join("g.toml")).unwrap();
        let names: Vec<&str> = p.sources.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, ["a.h", "b.c"]);
        assert!(GameProfile::load(&dir.join("missing.toml")).unwrap_err().contains("missing.toml"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
