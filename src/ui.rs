//! How `snes-port` talks to the person: progress, errors, the question about the compiler download and the choice of
//! the ROM. The builder only knows this trait; the console implements it here and the launcher window will implement
//! it too, so the same messages and exit codes reach either.

use std::path::{Path, PathBuf};

/// What is said when there is no C compiler and a portable one could be downloaded.
pub struct DownloadNotice {
    pub name: String,
    pub kib: u64,
    pub url: String,
    pub dir: PathBuf,
}

impl DownloadNotice {
    /// The sentence the console prints: what, from where, where it is kept and that nothing of the ROM is sent.
    pub fn text(&self) -> String {
        format!(
            "{} ({} KiB) can be downloaded from {} and kept in {}. Nothing about your ROM is sent; it is the only download.",
            self.name,
            self.kib,
            self.url,
            self.dir.display()
        )
    }
}

pub trait Ui {
    /// Something the program is doing or a warning that does not stop it.
    fn status(&self, text: &str);
    /// Why the program stops.
    fn error(&self, text: &str);
    /// Asks a yes/no question; anything but an explicit yes is a no.
    fn confirm(&self, question: &str) -> bool;
    /// Lets the person choose a ROM, starting in `start_dir` (empty if unknown); `None` if they give up.
    fn ask_rom(&self, start_dir: &Path) -> Option<PathBuf>;
    /// Tells that there is no compiler and asks whether to download the portable one. The console says it in two lines
    /// and asks; the launcher window says it in its language.
    fn offer_download(&self, notice: &DownloadNotice) -> bool {
        self.status("No C compiler found on this machine.");
        self.status(&notice.text());
        self.confirm("Download it now?")
    }
    /// The game is about to start (and the program waits for it to end): the launcher window gets out of the way.
    fn launching(&self) {}
}

/// The terminal: messages to stderr, answers from stdin and the ROM from the Windows file dialog.
pub struct ConsoleUi;

impl Ui for ConsoleUi {
    fn status(&self, text: &str) {
        eprintln!("{text}");
    }

    fn error(&self, text: &str) {
        eprintln!("{text}");
    }

    fn confirm(&self, question: &str) -> bool {
        crate::download::confirm(question, &mut std::io::stdin().lock())
    }

    fn ask_rom(&self, start_dir: &Path) -> Option<PathBuf> {
        println!("snes-port: choose your ROM in the window that opens (A Link to the Past, USA).");
        let script = "Add-Type -AssemblyName System.Windows.Forms;         $d = New-Object System.Windows.Forms.OpenFileDialog;         $d.Title = 'snes-port: choose your ROM';         $d.Filter = 'SNES ROM (*.sfc;*.smc)|*.sfc;*.smc|All files (*.*)|*.*';         if ($env:SNES_PORT_START -and (Test-Path $env:SNES_PORT_START)) { $d.InitialDirectory = $env:SNES_PORT_START };         if ($d.ShowDialog() -eq 'OK') { [Console]::Out.Write($d.FileName) }";
        let out = std::process::Command::new("powershell").args(["-NoProfile", "-STA", "-Command", script]).env("SNES_PORT_START", start_dir).output().ok()?;
        let chosen = PathBuf::from(String::from_utf8_lossy(&out.stdout).trim());
        (!chosen.as_os_str().is_empty()).then_some(chosen)
    }
}

/// A `Ui` for tests: records what it is told and answers the question as told.
#[cfg(test)]
pub struct TestUi {
    pub said: std::cell::RefCell<Vec<String>>,
    pub asked: std::cell::RefCell<Vec<String>>,
    pub answer: bool,
    pub rom: Option<PathBuf>,
}

#[cfg(test)]
impl TestUi {
    pub fn new(answer: bool) -> Self {
        TestUi { said: Default::default(), asked: Default::default(), answer, rom: None }
    }
}

#[cfg(test)]
impl Ui for TestUi {
    fn status(&self, text: &str) {
        self.said.borrow_mut().push(text.to_string());
    }

    fn error(&self, text: &str) {
        self.said.borrow_mut().push(text.to_string());
    }

    fn confirm(&self, question: &str) -> bool {
        self.asked.borrow_mut().push(question.to_string());
        self.answer
    }

    fn ask_rom(&self, _start_dir: &Path) -> Option<PathBuf> {
        self.rom.clone()
    }
}
