//! Games: each one is a profile, a `.toml` (see `config.rs`) with a `[game]` section that says which ROM it is
//! for, and optionally a folder next to it with the same name (`games/<id>/`) holding C code of the game that is
//! compiled together with the generated C.

use crate::config::Config;
use std::path::{Path, PathBuf};

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
    /// The folder next to the `.toml` with the same name, if it exists: C code and data files of the game.
    pub dir: Option<PathBuf>,
}

impl GameProfile {
    /// A profile from the text of its `.toml` and its C files; `origin` names it in the errors.
    pub fn from_text(config: &str, sources: Vec<Source>, origin: &str) -> Result<GameProfile, String> {
        let cfg = Config::parse(config).map_err(|e| format!("{origin}: {e}"))?;
        let Some(game) = cfg.game else {
            return Err(format!("{origin}: the profile has no [game] section (id, name and crc32)"));
        };
        Ok(GameProfile { id: game.id, name: game.name, crc32: game.crc32, config: config.to_string(), sources, origin: origin.to_string(), dir: None })
    }

    /// A profile from a `.toml` file and the `*.c`/`*.h` files of the folder with the same name next to it, if any.
    pub fn load(path: &Path) -> Result<GameProfile, String> {
        let config = std::fs::read_to_string(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        let mut sources = Vec::new();
        let mut folder = None;
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
            folder = Some(dir);
        }
        let mut profile = GameProfile::from_text(&config, sources, &path.display().to_string())?;
        profile.dir = folder;
        Ok(profile)
    }
}

/// The profiles in `dir`: every `*.toml` in it, with its folder of C code. A file that is not a valid profile is left
/// out and reported in the second value.
pub fn load_dir(dir: &Path) -> (Vec<GameProfile>, Vec<String>) {
    let mut profiles = Vec::new();
    let mut warnings = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else { return (profiles, warnings) };
    let mut files: Vec<PathBuf> = entries.flatten().map(|e| e.path()).filter(|p| p.is_file() && p.extension().is_some_and(|x| x == "toml")).collect();
    files.sort();
    for f in files {
        match GameProfile::load(&f) {
            Ok(p) => profiles.push(p),
            Err(e) => warnings.push(format!("warning: ignoring {e}")),
        }
    }
    (profiles, warnings)
}

/// The profiles in use: those of the folders where the program looks, with those given with `--game` on top. The
/// program carries none of its own.
#[derive(Debug, Clone, Default)]
pub struct Catalog {
    profiles: Vec<GameProfile>,
    warnings: Vec<String>,
}

impl Catalog {
    /// The profiles of `dirs` (highest priority first: a ROM found in an earlier folder ignores the later ones) and of
    /// `explicit`, which wins over all of them. Two profiles for the same ROM in one folder, or two explicit ones, are an
    /// error.
    pub fn discover(dirs: &[PathBuf], explicit: Vec<GameProfile>) -> Result<Catalog, String> {
        let clash = |a: &GameProfile, b: &GameProfile| format!("{} and {} are both for CRC32 {:08X}", a.origin, b.origin, b.crc32);
        let mut profiles: Vec<GameProfile> = Vec::new();
        for p in explicit {
            if let Some(o) = profiles.iter().find(|o| o.crc32 == p.crc32) {
                return Err(clash(o, &p));
            }
            profiles.push(p);
        }
        let mut warnings = Vec::new();
        for dir in dirs {
            let (found, w) = load_dir(dir);
            warnings.extend(w);
            let mut here: Vec<GameProfile> = Vec::new();
            for p in found {
                if let Some(o) = here.iter().find(|o| o.crc32 == p.crc32) {
                    return Err(clash(o, &p));
                }
                here.push(p);
            }
            for p in here {
                if !profiles.iter().any(|o| o.crc32 == p.crc32) {
                    profiles.push(p);
                }
            }
        }
        Ok(Catalog { profiles, warnings })
    }

    pub fn profiles(&self) -> &[GameProfile] {
        &self.profiles
    }

    /// What was left out while looking (files of the folders that are not valid profiles).
    pub fn warnings(&self) -> &[String] {
        &self.warnings
    }

    pub fn identify(&self, crc: u32) -> Option<&GameProfile> {
        self.profiles.iter().find(|g| g.crc32 == crc)
    }
}

/// The folders where profiles are looked for, highest priority first: `games/` of the current directory and `games/`
/// next to the executable.
pub fn search_dirs() -> Vec<PathBuf> {
    let mut dirs = vec![PathBuf::from("games")];
    if let Some(exe) = std::env::current_exe().ok().and_then(|e| e.parent().map(|p| p.join("games"))) {
        let same = |a: &Path, b: &Path| matches!((a.canonicalize(), b.canonicalize()), (Ok(x), Ok(y)) if x == y);
        if !same(&dirs[0], &exe) {
            dirs.push(exe);
        }
    }
    dirs
}

/// Paths of the `.toml` files given with `--game`, loaded.
pub fn load_external(paths: &[PathBuf]) -> Result<Vec<GameProfile>, String> {
    paths.iter().map(|p| GameProfile::load(p)).collect()
}

/// The catalog of a run: the usual folders and the `--game` files.
pub fn catalog(explicit: &[PathBuf]) -> Result<Catalog, String> {
    Catalog::discover(&search_dirs(), load_external(explicit)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn toml(id: &str, crc: &str) -> String {
        format!("[game]\nid=\"{id}\"\nname=\"Game {id}\"\ncrc32=\"{crc}\"\n")
    }

    fn folder(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("snes-recomp-games-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn profile(id: &str, crc: &str, origin: &str) -> GameProfile {
        GameProfile::from_text(&toml(id, crc), vec![], origin).unwrap()
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
    fn the_program_knows_no_game_by_itself() {
        let c = Catalog::discover(&[], vec![]).unwrap();
        assert!(c.profiles().is_empty() && c.identify(0x777A_AC2F).is_none());
    }

    #[test]
    fn profiles_are_found_in_the_folders_and_the_first_folder_wins() {
        let (first, second) = (folder("first"), folder("second"));
        std::fs::write(first.join("a.toml"), toml("from_first", "0000ABCD")).unwrap();
        std::fs::write(second.join("a.toml"), toml("from_second", "0000ABCD")).unwrap();
        std::fs::write(second.join("b.toml"), toml("only_second", "0000BEEF")).unwrap();
        std::fs::write(second.join("notes.txt"), "not a profile").unwrap();
        let c = Catalog::discover(&[first.clone(), second.clone(), std::env::temp_dir().join("snes-recomp-games-absent")], vec![]).unwrap();
        assert_eq!(c.identify(0xABCD).unwrap().id, "from_first");
        assert_eq!(c.identify(0xBEEF).unwrap().id, "only_second");
        assert_eq!(c.profiles().len(), 2);
        assert!(c.warnings().is_empty());
        let _ = (std::fs::remove_dir_all(&first), std::fs::remove_dir_all(&second));
    }

    #[test]
    fn a_broken_file_in_a_folder_is_reported_and_the_others_still_load() {
        let dir = folder("broken");
        std::fs::write(dir.join("bad.toml"), "[[entry]]\naddr=\"00:8000\"").unwrap();
        std::fs::write(dir.join("good.toml"), toml("good", "0000ABCD")).unwrap();
        let c = Catalog::discover(&[dir.clone()], vec![]).unwrap();
        assert_eq!(c.profiles().len(), 1);
        assert_eq!(c.warnings().len(), 1);
        assert!(c.warnings()[0].contains("bad.toml"), "{:?}", c.warnings());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn two_profiles_for_one_rom_in_one_place_are_an_error_and_game_beats_the_folders() {
        let dir = folder("clash");
        std::fs::write(dir.join("a.toml"), toml("a", "0000ABCD")).unwrap();
        std::fs::write(dir.join("b.toml"), toml("b", "0000ABCD")).unwrap();
        let e = Catalog::discover(&[dir.clone()], vec![]).unwrap_err();
        assert!(e.contains("a.toml") && e.contains("b.toml") && e.contains("0000ABCD"), "{e}");
        let e = Catalog::discover(&[], vec![profile("x", "0000ABCD", "x.toml"), profile("y", "0000ABCD", "y.toml")]).unwrap_err();
        assert!(e.contains("x.toml") && e.contains("y.toml"), "{e}");
        std::fs::remove_file(dir.join("b.toml")).unwrap();
        let c = Catalog::discover(&[dir.clone()], vec![profile("mine", "0000ABCD", "mine.toml")]).unwrap();
        assert_eq!(c.identify(0xABCD).unwrap().id, "mine");
        assert_eq!(c.profiles().len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_search_folders_are_games_here_and_next_to_the_executable() {
        let dirs = search_dirs();
        assert_eq!(dirs[0], PathBuf::from("games"));
        assert!(dirs.iter().all(|d| d.ends_with("games")));
    }

    #[test]
    fn a_profile_loads_the_c_files_of_the_folder_next_to_it() {
        let dir = folder("c-files");
        std::fs::create_dir_all(dir.join("g")).unwrap();
        std::fs::write(dir.join("g.toml"), toml("g", "0000ABCD")).unwrap();
        std::fs::write(dir.join("g").join("b.c"), "int b;").unwrap();
        std::fs::write(dir.join("g").join("a.h"), "int a;").unwrap();
        std::fs::write(dir.join("g").join("notes.txt"), "x").unwrap();
        let p = GameProfile::load(&dir.join("g.toml")).unwrap();
        let names: Vec<&str> = p.sources.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, ["a.h", "b.c"]);
        assert_eq!(p.dir.as_deref(), Some(dir.join("g").as_path()), "the profile remembers its folder");
        std::fs::write(dir.join("h.toml"), toml("h", "0000ABCE")).unwrap();
        assert_eq!(GameProfile::load(&dir.join("h.toml")).unwrap().dir, None, "no folder, no dir");
        assert!(GameProfile::load(&dir.join("missing.toml")).unwrap_err().contains("missing.toml"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
