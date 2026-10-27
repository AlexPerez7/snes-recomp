//! What the launcher window says and does, apart from drawing it: the texts in Spanish and English, the screen to show
//! and what a click leads to. The Win32 window (`launcher_win`) only draws a `Screen` and reports clicks, so all of this
//! can be tested without a window.

use crate::port;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lang {
    En,
    Es,
}

/// The language of the interface from a Windows language identifier (`GetUserDefaultUILanguage`): Spanish in any of
/// its variants (primary language 0x0A), English otherwise.
pub fn lang_from_langid(id: u16) -> Lang {
    if id & 0x3FF == 0x0A {
        Lang::Es
    } else {
        Lang::En
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    WindowTitle,
    PickPrompt,
    PickTitle,
    PickFilter,
    PickAction,
    ChangeAction,
    PlayAction,
    PlayWith,
    FirstBuild,
    Working,
    CopyDetail,
    Copied,
    Close,
    ChooseAnother,
    DownloadYes,
    DownloadNo,
    ErrorTitle,
    UnknownRomTitle,
    NoCompilerTitle,
    BuildFailedTitle,
    GameFailedTitle,
}

pub fn text(lang: Lang, key: Key) -> &'static str {
    use Key::*;
    match lang {
        Lang::En => match key {
            WindowTitle => "snes-port",
            PickPrompt => "Choose your ROM to play. It stays where it is: nothing is copied or sent anywhere.",
            PickTitle => "Choose your ROM",
            PickFilter => "SNES ROM (*.sfc;*.smc)\0*.sfc;*.smc\0All files (*.*)\0*.*\0\0",
            PickAction => "Choose ROM...",
            ChangeAction => "Change ROM...",
            PlayAction => "Play",
            PlayWith => "Ready to play with:",
            FirstBuild => "Getting the game ready for the first time. This can take a few minutes and only happens once.",
            Working => "Working...",
            CopyDetail => "Copy details",
            Copied => "Copied to the clipboard.",
            Close => "Close",
            ChooseAnother => "Choose another ROM...",
            DownloadYes => "Download",
            DownloadNo => "Not now",
            ErrorTitle => "Something went wrong",
            UnknownRomTitle => "ROM not recognised",
            NoCompilerTitle => "A C compiler is needed",
            BuildFailedTitle => "The game could not be built",
            GameFailedTitle => "The game stopped with an error",
        },
        Lang::Es => match key {
            WindowTitle => "snes-port",
            PickPrompt => "Elige tu ROM para jugar. Se queda donde está: no se copia ni se envía a ningún sitio.",
            PickTitle => "Elige tu ROM",
            PickFilter => "ROM de SNES (*.sfc;*.smc)\0*.sfc;*.smc\0Todos los archivos (*.*)\0*.*\0\0",
            PickAction => "Elegir ROM...",
            ChangeAction => "Cambiar ROM...",
            PlayAction => "Jugar",
            PlayWith => "Lista para jugar con:",
            FirstBuild => "Preparando el juego por primera vez. Puede tardar unos minutos y solo ocurre una vez.",
            Working => "Trabajando...",
            CopyDetail => "Copiar detalle",
            Copied => "Copiado al portapapeles.",
            Close => "Cerrar",
            ChooseAnother => "Elegir otra ROM...",
            DownloadYes => "Descargar",
            DownloadNo => "Ahora no",
            ErrorTitle => "Algo ha fallado",
            UnknownRomTitle => "ROM no reconocida",
            NoCompilerTitle => "Hace falta un compilador de C",
            BuildFailedTitle => "No se pudo construir el juego",
            GameFailedTitle => "El juego se cerró con un error",
        },
    }
}

/// Which button leads where.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Button {
    Primary,
    Secondary,
    Copy,
}

/// What the window shows. The text of an error is the one the console prints, unchanged.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Screen {
    /// First time, or the remembered ROM is gone.
    Pick,
    /// There is a remembered ROM: Play with it, or change it.
    Ready(PathBuf),
    /// Building or starting; `first` when the game is being built for the first time.
    Busy(String),
    /// A question with two answers (the compiler download).
    Ask(String),
    /// Why it stopped: the title and the detail, to read and copy.
    Failed { title: String, detail: String },
}

/// The screen to open with: the remembered ROM if it is still there, else the choice.
pub fn first_screen(cache: &Path) -> Screen {
    match port::recalled_rom(cache) {
        Some(rom) => Screen::Ready(rom),
        None => Screen::Pick,
    }
}

/// What to do after the ROM dialog: nothing when the person gave up, else build and play with the chosen ROM.
#[derive(Debug, PartialEq, Eq)]
pub enum Next {
    Stay,
    Start(PathBuf),
}

pub fn after_dialog(chosen: Option<PathBuf>) -> Next {
    chosen.map_or(Next::Stay, Next::Start)
}

/// The title for a failure, from the exit code it ends with.
pub fn failure_title(lang: Lang, code: i32) -> &'static str {
    match code {
        port::EXIT_UNKNOWN_ROM => text(lang, Key::UnknownRomTitle),
        port::EXIT_NO_TOOLCHAIN => text(lang, Key::NoCompilerTitle),
        port::EXIT_BUILD_FAILED => text(lang, Key::BuildFailedTitle),
        _ => text(lang, Key::ErrorTitle),
    }
}

/// The screen for a failure: the title by kind and the message exactly as the console prints it.
pub fn failed(lang: Lang, code: i32, message: &str) -> Screen {
    Screen::Failed { title: failure_title(lang, code).to_string(), detail: message.to_string() }
}

/// The screen for a game that started and stopped with an error code.
pub fn game_failed(lang: Lang, code: i32) -> Screen {
    let detail = match lang {
        Lang::En => format!("The game ended with exit code {code}."),
        Lang::Es => format!("El juego terminó con el código de salida {code}."),
    };
    Screen::Failed { title: text(lang, Key::GameFailedTitle).to_string(), detail }
}

/// A long path shortened from the left (`...snes-port toolchain`) so that the window does not cut it at the edge.
pub fn shorten_path(path: &str, max: usize) -> String {
    let n = path.chars().count();
    if n <= max {
        return path.to_string();
    }
    let tail: String = path.chars().skip(n - (max - 3)).collect();
    format!("...{tail}")
}

/// Where the ROM of a launch comes from, as a file name for the window (the folder is not shown).
pub fn rom_label(rom: &Path) -> String {
    rom.file_name().map_or_else(|| rom.display().to_string(), |n| n.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("snes-port-launcher-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn spanish_windows_in_any_variant_gets_spanish_and_the_rest_english() {
        for id in [0x0C0A, 0x040A, 0x080A, 0x2C0A, 0x540A] {
            assert_eq!(lang_from_langid(id), Lang::Es, "{id:#06x}");
        }
        for id in [0x0409, 0x0809, 0x0407, 0x040C, 0x0416, 0x0000] {
            assert_eq!(lang_from_langid(id), Lang::En, "{id:#06x}");
        }
    }

    #[test]
    fn every_text_exists_in_both_languages_and_they_differ() {
        use Key::*;
        let all = [
            WindowTitle, PickPrompt, PickTitle, PickFilter, PickAction, ChangeAction, PlayAction, PlayWith, FirstBuild, Working, CopyDetail,
            Copied, Close, ChooseAnother, DownloadYes, DownloadNo, ErrorTitle, UnknownRomTitle, NoCompilerTitle, BuildFailedTitle, GameFailedTitle,
        ];
        for k in all {
            let (en, es) = (text(Lang::En, k), text(Lang::Es, k));
            assert!(!en.is_empty() && !es.is_empty(), "{k:?}");
            if k != WindowTitle {
                assert_ne!(en, es, "{k:?} is not translated");
            }
        }
        // the file dialog filter is a list of pairs ended by an empty string
        for l in [Lang::En, Lang::Es] {
            let f = text(l, PickFilter);
            assert!(f.ends_with("\0\0") && f.contains("*.sfc") && f.contains("*.smc"));
        }
    }

    #[test]
    fn the_window_says_what_the_console_says() {
        let msg = "error: unknown ROM: CRC32 DEADBEEF\nSupported games:\n  A (CRC32 00000001)\n";
        match failed(Lang::Es, port::EXIT_UNKNOWN_ROM, msg) {
            Screen::Failed { detail, title } => {
                assert_eq!(detail, msg, "the detail is the console message, untouched");
                assert_eq!(title, text(Lang::Es, Key::UnknownRomTitle));
            }
            other => panic!("{other:?}"),
        }
        let kinds = [(port::EXIT_NO_TOOLCHAIN, Key::NoCompilerTitle), (port::EXIT_BUILD_FAILED, Key::BuildFailedTitle), (1, Key::ErrorTitle)];
        for (code, key) in kinds {
            assert_eq!(failure_title(Lang::En, code), text(Lang::En, key));
        }
    }

    #[test]
    fn giving_up_the_dialog_changes_nothing() {
        assert_eq!(after_dialog(None), Next::Stay);
        assert_eq!(after_dialog(Some(PathBuf::from("a.sfc"))), Next::Start(PathBuf::from("a.sfc")));
    }

    #[test]
    fn the_first_screen_offers_the_remembered_rom_only_while_it_is_there() {
        let dir = temp("first");
        let cache = dir.join("cache");
        assert_eq!(first_screen(&cache), Screen::Pick);
        let rom = dir.join("zelda.sfc");
        std::fs::write(&rom, b"rom").unwrap();
        port::remember_rom(&cache, &rom);
        assert_eq!(first_screen(&cache), Screen::Ready(rom.clone()));
        assert_eq!(rom_label(&rom), "zelda.sfc");
        std::fs::remove_file(&rom).unwrap();
        assert_eq!(first_screen(&cache), Screen::Pick, "a moved ROM is asked for again");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_long_path_is_shortened_from_the_left_and_a_short_one_is_left_alone() {
        assert_eq!(shorten_path(r"C:\a\b", 20), r"C:\a\b");
        let long = r"C:\Users\someone\AppData\Local\snes-port\toolchain";
        let s = shorten_path(long, 30);
        assert_eq!(s.chars().count(), 30);
        assert!(s.starts_with("...") && s.ends_with(r"snes-port\toolchain"), "{s}");
    }

    #[test]
    fn a_game_that_stops_with_an_error_shows_its_code() {
        match game_failed(Lang::En, 7) {
            Screen::Failed { detail, .. } => assert!(detail.contains('7')),
            other => panic!("{other:?}"),
        }
    }
}
