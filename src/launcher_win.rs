//! The launcher window (Windows): a small Win32 window, written against the system libraries directly so that the
//! program gains no dependency. What it says and what a click leads to is in `launcher`; here is only the drawing, the
//! message loop, the file dialog and the clipboard.
//!
//! The window lives on a thread of its own with its message loop. The build runs on another thread and reaches the
//! window with posted messages (a `Screen` to show); a question (the compiler download) waits for its answer on a
//! channel. The window gets out of the way when the game starts and comes back with the error if the game fails.

use crate::launcher::{self, Button, Key, Lang, Screen};
use crate::port;
use crate::ui::{DownloadNotice, Ui};
use std::ffi::c_void;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicIsize, Ordering};
use std::sync::{mpsc, Mutex, OnceLock};

type Hwnd = isize;

const CLASS: &str = "SnesPortLauncher";
const WM_CREATE: u32 = 0x0001;
const WM_DESTROY: u32 = 0x0002;
const WM_CLOSE: u32 = 0x0010;
const WM_SETFONT: u32 = 0x0030;
const WM_COMMAND: u32 = 0x0111;
const WM_CTLCOLORSTATIC: u32 = 0x0138;
const WM_APP_SCREEN: u32 = 0x8001;
const WM_APP_HIDE: u32 = 0x8002;
const WM_APP_SHOW: u32 = 0x8003;
const WM_APP_QUIT: u32 = 0x8004;

const WS_CAPTION: u32 = 0x00C0_0000;
const WS_SYSMENU: u32 = 0x0008_0000;
const WS_MINIMIZEBOX: u32 = 0x0002_0000;
const WS_CHILD: u32 = 0x4000_0000;
const WS_VISIBLE: u32 = 0x1000_0000;
const WS_TABSTOP: u32 = 0x0001_0000;
const WS_VSCROLL: u32 = 0x0020_0000;
const WS_BORDER: u32 = 0x0080_0000;
const BS_DEFPUSHBUTTON: u32 = 1;
const ES_MULTILINE: u32 = 0x0004;
const ES_AUTOVSCROLL: u32 = 0x0040;
const ES_READONLY: u32 = 0x0800;
const SW_HIDE: i32 = 0;
const SW_SHOW: i32 = 5;
const COLOR_WINDOW: isize = 5;
const IDC_ARROW: usize = 32512;
const OFN_PATHMUSTEXIST: u32 = 0x0000_0800;
const OFN_FILEMUSTEXIST: u32 = 0x0000_1000;
const OFN_NOCHANGEDIR: u32 = 0x0000_0008;
const OFN_EXPLORER: u32 = 0x0008_0000;
const CF_UNICODETEXT: u32 = 13;
const GMEM_MOVEABLE: u32 = 0x0002;
const LOGPIXELSX: i32 = 88;
const ATTACH_PARENT_PROCESS: u32 = 0xFFFF_FFFF;
const STD_INPUT_HANDLE: u32 = 0xFFFF_FFF6;
const STD_OUTPUT_HANDLE: u32 = 0xFFFF_FFF5;
const STD_ERROR_HANDLE: u32 = 0xFFFF_FFF4;
const ID_PRIMARY: i32 = 1;
const ID_SECONDARY: i32 = 2;
const ID_COPY: i32 = 3;

const WM_PAINT: u32 = 0x000F;
const WM_ERASEBKGND: u32 = 0x0014;
const WM_DRAWITEM: u32 = 0x002B;
const BS_OWNERDRAW: u32 = 0x0B;
const ODS_SELECTED: u32 = 0x0001;
const ODS_FOCUS: u32 = 0x0010;
const DT_CENTER: u32 = 0x0001;
const DT_VCENTER: u32 = 0x0004;
const DT_SINGLELINE: u32 = 0x0020;
const DT_END_ELLIPSIS: u32 = 0x8000;
const DT_NOPREFIX: u32 = 0x0800;
const IMAGE_ICON: u32 = 1;
const DI_NORMAL: u32 = 3;

// The look: a dark header with the icon over a light page, one accent colour (the cyan of the icon). COLORREF is 0x00BBGGRR.
const HEADER: u32 = 0x003B_2214;
const HEADER_ERROR: u32 = 0x0022_1E5A;
const STRIPE: u32 = 0x00E6_D84F;
const STRIPE_ERROR: u32 = 0x0058_58E0;
const PAGE: u32 = 0x00FA_F7F5;
const INK: u32 = 0x0033_291F;
const HEADER_TITLE: u32 = 0x00FF_FFFF;
const HEADER_SUB: u32 = 0x00C8_B39F;
const ACCENT: u32 = 0x00A8_8C1B;
const ACCENT_PRESSED: u32 = 0x0088_7016;
const HEADER_H: i32 = 84;
const STRIPE_H: i32 = 3;
const CLIENT_W: i32 = 540;
const CLIENT_H: i32 = 340;

#[repr(C)]
struct Point {
    x: i32,
    y: i32,
}

#[repr(C)]
struct Msg {
    hwnd: Hwnd,
    message: u32,
    wparam: usize,
    lparam: isize,
    time: u32,
    pt: Point,
    private: u32,
}

#[repr(C)]
struct Rect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

#[repr(C)]
struct PaintStruct {
    hdc: isize,
    erase: i32,
    paint: Rect,
    restore: i32,
    update: i32,
    reserved: [u8; 32],
}

#[repr(C)]
struct DrawItem {
    ctl_type: u32,
    ctl_id: u32,
    item_id: u32,
    action: u32,
    state: u32,
    item: Hwnd,
    dc: isize,
    rc: Rect,
    data: usize,
}

#[repr(C)]
struct WndClassW {
    style: u32,
    proc_: unsafe extern "system" fn(Hwnd, u32, usize, isize) -> isize,
    cls_extra: i32,
    wnd_extra: i32,
    instance: isize,
    icon: isize,
    cursor: isize,
    background: isize,
    menu: *const u16,
    class_name: *const u16,
}

#[repr(C)]
struct OpenFileNameW {
    size: u32,
    owner: Hwnd,
    instance: isize,
    filter: *const u16,
    custom_filter: *mut u16,
    max_custom: u32,
    filter_index: u32,
    file: *mut u16,
    max_file: u32,
    file_title: *mut u16,
    max_file_title: u32,
    initial_dir: *const u16,
    title: *const u16,
    flags: u32,
    file_offset: u16,
    file_extension: u16,
    def_ext: *const u16,
    cust_data: isize,
    hook: isize,
    template: *const u16,
    reserved: *mut c_void,
    reserved_dw: u32,
    flags_ex: u32,
}

#[link(name = "user32")]
extern "system" {
    fn RegisterClassW(c: *const WndClassW) -> u16;
    fn CreateWindowExW(ex: u32, class: *const u16, title: *const u16, style: u32, x: i32, y: i32, w: i32, h: i32, parent: Hwnd, menu: isize, inst: isize, param: *mut c_void) -> Hwnd;
    fn DefWindowProcW(h: Hwnd, m: u32, w: usize, l: isize) -> isize;
    fn GetMessageW(m: *mut Msg, h: Hwnd, min: u32, max: u32) -> i32;
    fn TranslateMessage(m: *const Msg) -> i32;
    fn DispatchMessageW(m: *const Msg) -> isize;
    fn IsDialogMessageW(h: Hwnd, m: *mut Msg) -> i32;
    fn PostMessageW(h: Hwnd, m: u32, w: usize, l: isize) -> i32;
    fn SendMessageW(h: Hwnd, m: u32, w: usize, l: isize) -> isize;
    fn PostQuitMessage(code: i32);
    fn ShowWindow(h: Hwnd, cmd: i32) -> i32;
    fn SetWindowTextW(h: Hwnd, text: *const u16) -> i32;
    fn DestroyWindow(h: Hwnd) -> i32;
    fn LoadCursorW(inst: isize, name: usize) -> isize;
    fn LoadIconW(inst: isize, name: usize) -> isize;
    fn AdjustWindowRect(r: *mut Rect, style: u32, menu: i32) -> i32;
    fn GetSystemMetrics(i: i32) -> i32;
    fn SetForegroundWindow(h: Hwnd) -> i32;
    fn OpenClipboard(h: Hwnd) -> i32;
    fn EmptyClipboard() -> i32;
    fn SetClipboardData(fmt: u32, h: isize) -> isize;
    fn CloseClipboard() -> i32;
    fn GetDC(h: Hwnd) -> isize;
    fn ReleaseDC(h: Hwnd, dc: isize) -> i32;
    fn GetSysColorBrush(index: i32) -> isize;
    fn InvalidateRect(h: Hwnd, r: *const Rect, erase: i32) -> i32;
    fn UpdateWindow(h: Hwnd) -> i32;
    fn BeginPaint(h: Hwnd, ps: *mut PaintStruct) -> isize;
    fn EndPaint(h: Hwnd, ps: *const PaintStruct) -> i32;
    fn GetClientRect(h: Hwnd, r: *mut Rect) -> i32;
    fn FillRect(dc: isize, r: *const Rect, brush: isize) -> i32;
    fn DrawTextW(dc: isize, text: *const u16, n: i32, r: *mut Rect, format: u32) -> i32;
    fn DrawIconEx(dc: isize, x: i32, y: i32, icon: isize, cx: i32, cy: i32, step: u32, brush: isize, flags: u32) -> i32;
    fn DrawFocusRect(dc: isize, r: *const Rect) -> i32;
    fn LoadImageW(inst: isize, name: usize, kind: u32, cx: i32, cy: i32, flags: u32) -> isize;
    fn GetWindowTextW(h: Hwnd, buf: *mut u16, n: i32) -> i32;
}

#[link(name = "gdi32")]
extern "system" {
    fn CreateFontW(h: i32, w: i32, esc: i32, orient: i32, weight: i32, italic: u32, underline: u32, strike: u32, charset: u32, out: u32, clip: u32, quality: u32, pitch: u32, face: *const u16) -> isize;
    fn GetDeviceCaps(dc: isize, index: i32) -> i32;
    fn SetBkMode(dc: isize, mode: i32) -> i32;
    fn SetTextColor(dc: isize, color: u32) -> u32;
    fn CreateSolidBrush(color: u32) -> isize;
    fn CreatePen(style: i32, width: i32, color: u32) -> isize;
    fn SelectObject(dc: isize, obj: isize) -> isize;
    fn DeleteObject(obj: isize) -> i32;
    fn RoundRect(dc: isize, l: i32, t: i32, r: i32, b: i32, w: i32, h: i32) -> i32;
}

#[link(name = "comdlg32")]
extern "system" {
    fn GetOpenFileNameW(o: *mut OpenFileNameW) -> i32;
}

#[link(name = "kernel32")]
extern "system" {
    fn GetModuleHandleW(name: *const u16) -> isize;
    fn GlobalAlloc(flags: u32, bytes: usize) -> isize;
    fn GlobalLock(h: isize) -> *mut c_void;
    fn GlobalUnlock(h: isize) -> i32;
    fn AttachConsole(pid: u32) -> i32;
    fn GetConsoleWindow() -> isize;
    fn GetStdHandle(n: u32) -> isize;
    fn SetStdHandle(n: u32, h: isize) -> i32;
    fn CreateFileW(name: *const u16, access: u32, share: u32, sec: *mut c_void, disp: u32, flags: u32, template: isize) -> isize;
    fn GetUserDefaultUILanguage() -> u16;
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Edit controls want `\r\n`.
fn crlf(s: &str) -> String {
    s.replace("\r\n", "\n").replace('\n', "\r\n")
}

// ---- the console of the terminal that started the program ----

/// A program of the windows subsystem has no console of its own. When it is started from a terminal it takes that
/// one (and its standard handles, unless the terminal redirected them), so that the messages and the exit code are
/// what they were. Returns true when there is a console to write to: false when started by a double click or a
/// drop of the ROM on the program.
pub fn attach_console() -> bool {
    unsafe {
        if GetConsoleWindow() == 0 && AttachConsole(ATTACH_PARENT_PROCESS) == 0 {
            return false;
        }
        for (std_handle, file, access) in [(STD_INPUT_HANDLE, "CONIN$", 0x8000_0000u32), (STD_OUTPUT_HANDLE, "CONOUT$", 0x4000_0000), (STD_ERROR_HANDLE, "CONOUT$", 0x4000_0000)] {
            let h = GetStdHandle(std_handle);
            if h == 0 || h == -1 {
                // 3 = OPEN_EXISTING, 3 = FILE_SHARE_READ | FILE_SHARE_WRITE
                let f = CreateFileW(wide(file).as_ptr(), access, 3, std::ptr::null_mut(), 3, 0, 0);
                if f != -1 {
                    SetStdHandle(std_handle, f);
                }
            }
        }
        true
    }
}

pub fn ui_language() -> Lang {
    launcher::lang_from_langid(unsafe { GetUserDefaultUILanguage() })
}

// ---- state shared by the threads ----

struct Gui {
    lang: Lang,
    cache: PathBuf,
    /// The window offers to choose a ROM (started without one), as opposed to a ROM given to the program.
    picks_rom: bool,
    hwnd: AtomicIsize,
    answer: Mutex<Option<mpsc::Sender<bool>>>,
    screen: Mutex<Screen>,
    thread: Mutex<Option<std::thread::JoinHandle<()>>>,
}

static GUI: OnceLock<Gui> = OnceLock::new();
static EXIT: AtomicI32 = AtomicI32::new(0);

fn gui() -> &'static Gui {
    GUI.get().expect("launcher window not initialised")
}

struct Controls {
    body: Hwnd,
    detail: Hwnd,
    primary: Hwnd,
    secondary: Hwnd,
    copy: Hwnd,
}

static CONTROLS: OnceLock<Controls> = OnceLock::new();

/// The fonts, brush and icon the window paints with (made once, when it is created).
struct Look {
    title: isize,
    sub: isize,
    button: isize,
    page: isize,
    icon: isize,
}

static LOOK: OnceLock<Look> = OnceLock::new();

fn dpi() -> i32 {
    unsafe {
        let dc = GetDC(0);
        let d = if dc != 0 { GetDeviceCaps(dc, LOGPIXELSX) } else { 96 };
        if dc != 0 {
            ReleaseDC(0, dc);
        }
        d.max(96)
    }
}

fn scale(v: i32) -> i32 {
    v * dpi() / 96
}

fn font(points: i32, weight: i32) -> isize {
    unsafe { CreateFontW(-(points * dpi() / 72), 0, 0, 0, weight, 0, 0, 0, 1, 0, 0, 5, 0, wide("Segoe UI").as_ptr()) }
}

// ---- the window ----

unsafe fn child(parent: Hwnd, class: &str, text: &str, style: u32, id: i32, x: i32, y: i32, w: i32, h: i32, f: isize) -> Hwnd {
    let inst = GetModuleHandleW(std::ptr::null());
    let c = CreateWindowExW(0, wide(class).as_ptr(), wide(text).as_ptr(), WS_CHILD | style, scale(x), scale(y), scale(w), scale(h), parent, id as isize, inst, std::ptr::null_mut());
    SendMessageW(c, WM_SETFONT, f as usize, 1);
    c
}

unsafe fn create_controls(hwnd: Hwnd) {
    let normal = font(10, 400);
    let inst = GetModuleHandleW(std::ptr::null());
    let _ = LOOK.set(Look {
        title: font(17, 600),
        sub: font(10, 400),
        button: font(10, 600),
        page: CreateSolidBrush(PAGE),
        icon: LoadImageW(inst, 1, IMAGE_ICON, scale(56), scale(56), 0),
    });
    let top = HEADER_H + STRIPE_H + 20;
    let body = child(hwnd, "STATIC", "", WS_VISIBLE, 0, 24, top, 492, 150, normal);
    let detail = child(hwnd, "EDIT", "", WS_BORDER | WS_VSCROLL | ES_MULTILINE | ES_AUTOVSCROLL | ES_READONLY | WS_TABSTOP, 0, 24, top, 492, 150, normal);
    let copy = child(hwnd, "BUTTON", "", WS_TABSTOP, ID_COPY, 24, 286, 150, 34, normal);
    let secondary = child(hwnd, "BUTTON", "", WS_TABSTOP, ID_SECONDARY, 366, 286, 150, 34, normal);
    let primary = child(hwnd, "BUTTON", "", WS_TABSTOP | BS_OWNERDRAW | BS_DEFPUSHBUTTON, ID_PRIMARY, 204, 286, 150, 34, normal);
    let _ = CONTROLS.set(Controls { body, detail, primary, secondary, copy });
}

fn set_text(h: Hwnd, s: &str) {
    unsafe { SetWindowTextW(h, wide(s).as_ptr()) };
}

fn show_control(h: Hwnd, visible: bool) {
    unsafe { ShowWindow(h, if visible { SW_SHOW } else { SW_HIDE }) };
}

/// The names of the games the profiles at hand know, for the window to say what it can play.
fn supported_games() -> String {
    match crate::games::catalog(&[]) {
        Ok(c) => c.profiles().iter().map(|g| g.name.clone()).collect::<Vec<_>>().join(", "),
        Err(_) => String::new(),
    }
}

/// Draws a screen: the texts, the buttons that apply and which of them is the main one.
fn draw(screen: &Screen) {
    let g = gui();
    let c = CONTROLS.get().expect("controls");
    let t = |k: Key| launcher::text(g.lang, k);
    let games = supported_games();
    let (body, detail, primary, secondary, copy): (String, Option<String>, Option<&str>, Option<&str>, bool) = match screen {
        Screen::Pick => (
            if games.is_empty() { t(Key::PickPrompt).to_string() } else { format!("{games}\n\n{}", t(Key::PickPrompt)) },
            None,
            Some(t(Key::PickAction)),
            None,
            false,
        ),
        Screen::Ready(rom) => (
            format!("{}\n\n{}", t(Key::PlayWith), launcher::rom_label(rom)),
            None,
            Some(t(Key::PlayAction)),
            Some(t(Key::ChangeAction)),
            false,
        ),
        Screen::Busy(text) => (text.clone(), None, None, None, false),
        Screen::Ask(text) => (text.clone(), None, Some(t(Key::DownloadYes)), Some(t(Key::DownloadNo)), false),
        Screen::Failed { detail, .. } => (String::new(), Some(detail.clone()), if g.picks_rom { Some(t(Key::ChooseAnother)) } else { None }, Some(t(Key::Close)), true),
    };
    set_text(c.body, &body);
    show_control(c.body, detail.is_none());
    show_control(c.detail, detail.is_some());
    if let Some(d) = &detail {
        set_text(c.detail, &crlf(d));
    }
    for (h, label) in [(c.primary, primary), (c.secondary, secondary)] {
        show_control(h, label.is_some());
        if let Some(l) = label {
            set_text(h, l);
        }
    }
    show_control(c.copy, copy);
    if copy {
        set_text(c.copy, t(Key::CopyDetail));
    }
    *g.screen.lock().unwrap() = screen.clone();
    // what was hidden or shown is painted now, not at the next occasion
    let h = g.hwnd.load(Ordering::SeqCst);
    if h != 0 {
        unsafe {
            InvalidateRect(h, std::ptr::null(), 1);
            UpdateWindow(h);
        }
    }
}

fn post_screen(screen: Screen) {
    let h = gui().hwnd.load(Ordering::SeqCst);
    let raw = Box::into_raw(Box::new(screen));
    if h == 0 || unsafe { PostMessageW(h, WM_APP_SCREEN, 0, raw as isize) } == 0 {
        drop(unsafe { Box::from_raw(raw) });
    }
}

fn post(msg: u32) {
    let h = gui().hwnd.load(Ordering::SeqCst);
    if h != 0 {
        unsafe { PostMessageW(h, msg, 0, 0) };
    }
}

fn copy_to_clipboard(owner: Hwnd, text: &str) -> bool {
    unsafe {
        if OpenClipboard(owner) == 0 {
            return false;
        }
        EmptyClipboard();
        let data = wide(&crlf(text));
        let bytes = data.len() * 2;
        let mem = GlobalAlloc(GMEM_MOVEABLE, bytes);
        let mut ok = false;
        if mem != 0 {
            let p = GlobalLock(mem) as *mut u16;
            if !p.is_null() {
                std::ptr::copy_nonoverlapping(data.as_ptr(), p, data.len());
                GlobalUnlock(mem);
                ok = SetClipboardData(CF_UNICODETEXT, mem) != 0;
            }
        }
        CloseClipboard();
        ok
    }
}

/// The ROM dialog; `None` when the person closes it without choosing.
fn pick_rom(owner: Hwnd) -> Option<PathBuf> {
    let g = gui();
    let start = port::start_dir(&g.cache);
    let mut file = vec![0u16; 32768];
    let filter: Vec<u16> = launcher::text(g.lang, Key::PickFilter).encode_utf16().collect();
    let title = wide(launcher::text(g.lang, Key::PickTitle));
    let dir = wide(&start.to_string_lossy());
    let mut o = OpenFileNameW {
        size: std::mem::size_of::<OpenFileNameW>() as u32,
        owner,
        instance: 0,
        filter: filter.as_ptr(),
        custom_filter: std::ptr::null_mut(),
        max_custom: 0,
        filter_index: 1,
        file: file.as_mut_ptr(),
        max_file: file.len() as u32,
        file_title: std::ptr::null_mut(),
        max_file_title: 0,
        initial_dir: if start.as_os_str().is_empty() { std::ptr::null() } else { dir.as_ptr() },
        title: title.as_ptr(),
        flags: OFN_PATHMUSTEXIST | OFN_FILEMUSTEXIST | OFN_NOCHANGEDIR | OFN_EXPLORER,
        file_offset: 0,
        file_extension: 0,
        def_ext: std::ptr::null(),
        cust_data: 0,
        hook: 0,
        template: std::ptr::null(),
        reserved: std::ptr::null_mut(),
        reserved_dw: 0,
        flags_ex: 0,
    };
    if unsafe { GetOpenFileNameW(&mut o) } == 0 {
        return None;
    }
    let len = file.iter().position(|&c| c == 0).unwrap_or(file.len());
    Some(PathBuf::from(String::from_utf16_lossy(&file[..len])))
}

/// Starts the build and the game for `rom` on a thread of its own.
fn start(rom: PathBuf) {
    draw(&Screen::Busy(launcher::text(gui().lang, Key::Working).to_string()));
    std::thread::spawn(move || {
        let ui = WindowUi::new();
        let code = port::run_port(rom, gui().cache.clone(), true, false, false, Vec::new(), Vec::new(), &ui);
        finish(code, &ui);
    });
}

/// What follows the end of a run: leave when it went well, else show why it did not.
fn finish(code: i32, ui: &WindowUi) {
    if code == 0 {
        EXIT.store(0, Ordering::SeqCst);
        post(WM_APP_QUIT);
        return;
    }
    let screen = if ui.launched.load(Ordering::SeqCst) { launcher::game_failed(gui().lang, code) } else { launcher::failed(gui().lang, code, &ui.errors.lock().unwrap().join("\n")) };
    EXIT.store(code, Ordering::SeqCst);
    post_screen(screen);
    post(WM_APP_SHOW);
}

unsafe fn on_click(hwnd: Hwnd, id: i32) {
    let button = match id {
        ID_PRIMARY => Button::Primary,
        ID_SECONDARY => Button::Secondary,
        ID_COPY => Button::Copy,
        _ => return,
    };
    let screen = gui().screen.lock().unwrap().clone();
    match (&screen, button) {
        (Screen::Pick, Button::Primary) | (Screen::Ready(_), Button::Secondary) | (Screen::Failed { .. }, Button::Primary) => {
            if let launcher::Next::Start(rom) = launcher::after_dialog(pick_rom(hwnd)) {
                start(rom);
            }
        }
        (Screen::Ready(rom), Button::Primary) => start(rom.clone()),
        (Screen::Ask(_), b) => {
            if let Some(tx) = gui().answer.lock().unwrap().take() {
                let _ = tx.send(b == Button::Primary);
            }
            draw(&Screen::Busy(launcher::text(gui().lang, Key::Working).to_string()));
        }
        (Screen::Failed { .. }, Button::Secondary) => {
            DestroyWindow(hwnd);
        }
        (Screen::Failed { detail, .. }, Button::Copy) => {
            if copy_to_clipboard(hwnd, detail) {
                if let Some(c) = CONTROLS.get() {
                    set_text(c.copy, launcher::text(gui().lang, Key::Copied));
                }
            }
        }
        _ => {}
    }
}

/// What the header says for a screen: the title, the line under it and whether it is an error.
fn header_texts(lang: Lang, screen: &Screen) -> (String, &'static str, bool) {
    match screen {
        Screen::Failed { title, .. } => (title.clone(), launcher::text(lang, Key::WindowTitle), true),
        _ => (launcher::text(lang, Key::WindowTitle).to_string(), launcher::text(lang, Key::Tagline), false),
    }
}

unsafe fn text_at(dc: isize, text: &str, font: isize, color: u32, rect: &mut Rect, format: u32) {
    let old = SelectObject(dc, font);
    SetTextColor(dc, color);
    let w = wide(text);
    DrawTextW(dc, w.as_ptr(), -1, rect, format | DT_SINGLELINE | DT_NOPREFIX | DT_END_ELLIPSIS);
    SelectObject(dc, old);
}

/// The header: the dark band with the icon, the title and the line under it, and the stripe of the accent colour below.
unsafe fn paint(hwnd: Hwnd) {
    let mut ps = std::mem::zeroed::<PaintStruct>();
    let dc = BeginPaint(hwnd, &mut ps);
    if let Some(look) = LOOK.get() {
        let g = gui();
        let (title, sub, error) = header_texts(g.lang, &g.screen.lock().unwrap());
        let band = Rect { left: 0, top: 0, right: scale(CLIENT_W), bottom: scale(HEADER_H) };
        let brush = CreateSolidBrush(if error { HEADER_ERROR } else { HEADER });
        FillRect(dc, &band, brush);
        DeleteObject(brush);
        let stripe = Rect { left: 0, top: scale(HEADER_H), right: scale(CLIENT_W), bottom: scale(HEADER_H + STRIPE_H) };
        let brush = CreateSolidBrush(if error { STRIPE_ERROR } else { STRIPE });
        FillRect(dc, &stripe, brush);
        DeleteObject(brush);
        if look.icon != 0 {
            DrawIconEx(dc, scale(24), scale(14), look.icon, scale(56), scale(56), 0, 0, DI_NORMAL);
        }
        SetBkMode(dc, 1);
        let mut r = Rect { left: scale(96), top: scale(14), right: scale(CLIENT_W - 24), bottom: scale(46) };
        text_at(dc, &title, look.title, HEADER_TITLE, &mut r, 0);
        let mut r = Rect { left: scale(96), top: scale(46), right: scale(CLIENT_W - 24), bottom: scale(72) };
        text_at(dc, sub, look.sub, HEADER_SUB, &mut r, 0);
    }
    EndPaint(hwnd, &ps);
}

/// The main button: filled with the accent colour, a little darker while pressed, with a dotted rectangle when it has the focus.
unsafe fn draw_button(di: &DrawItem) {
    let Some(look) = LOOK.get() else { return };
    let rc = Rect { left: di.rc.left, top: di.rc.top, right: di.rc.right, bottom: di.rc.bottom };
    FillRect(di.dc, &rc, look.page);
    let color = if di.state & ODS_SELECTED != 0 { ACCENT_PRESSED } else { ACCENT };
    let (brush, pen) = (CreateSolidBrush(color), CreatePen(0, 1, color));
    let (old_brush, old_pen) = (SelectObject(di.dc, brush), SelectObject(di.dc, pen));
    RoundRect(di.dc, rc.left, rc.top, rc.right, rc.bottom, scale(8), scale(8));
    SelectObject(di.dc, old_brush);
    SelectObject(di.dc, old_pen);
    DeleteObject(brush);
    DeleteObject(pen);
    let mut label = [0u16; 128];
    let n = GetWindowTextW(di.item, label.as_mut_ptr(), label.len() as i32).max(0) as usize;
    let text = String::from_utf16_lossy(&label[..n]);
    let mut r = Rect { left: rc.left, top: rc.top, right: rc.right, bottom: rc.bottom };
    SetBkMode(di.dc, 1);
    text_at(di.dc, &text, look.button, HEADER_TITLE, &mut r, DT_CENTER | DT_VCENTER);
    if di.state & ODS_FOCUS != 0 {
        let f = Rect { left: rc.left + scale(4), top: rc.top + scale(4), right: rc.right - scale(4), bottom: rc.bottom - scale(4) };
        DrawFocusRect(di.dc, &f);
    }
}

unsafe extern "system" fn wndproc(hwnd: Hwnd, msg: u32, wp: usize, lp: isize) -> isize {
    match msg {
        WM_CREATE => {
            create_controls(hwnd);
            0
        }
        WM_COMMAND => {
            on_click(hwnd, (wp & 0xFFFF) as i32);
            0
        }
        WM_ERASEBKGND => {
            let mut r = std::mem::zeroed::<Rect>();
            GetClientRect(hwnd, &mut r);
            FillRect(wp as isize, &r, LOOK.get().map_or(0, |l| l.page));
            1
        }
        WM_PAINT => {
            paint(hwnd);
            0
        }
        WM_DRAWITEM => {
            draw_button(&*(lp as *const DrawItem));
            1
        }
        WM_CTLCOLORSTATIC => {
            SetBkMode(wp as isize, 1);
            if CONTROLS.get().is_some_and(|c| c.detail == lp) {
                // the box with the detail of an error stays white
                return GetSysColorBrush(COLOR_WINDOW as i32);
            }
            // the texts sit on the page, not on the grey of the buttons
            SetTextColor(wp as isize, INK);
            LOOK.get().map_or(0, |l| l.page)
        }
        WM_APP_SCREEN => {
            let screen = Box::from_raw(lp as *mut Screen);
            draw(&screen);
            0
        }
        WM_APP_HIDE => {
            ShowWindow(hwnd, SW_HIDE);
            0
        }
        WM_APP_SHOW => {
            ShowWindow(hwnd, SW_SHOW);
            SetForegroundWindow(hwnd);
            0
        }
        WM_APP_QUIT => {
            DestroyWindow(hwnd);
            0
        }
        WM_CLOSE => {
            DestroyWindow(hwnd);
            0
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}

/// The thread of the window: creates it showing `first`, reports its handle and runs the message loop until it closes.
fn window_thread(first: Screen, ready: mpsc::Sender<()>) {
    unsafe {
        let inst = GetModuleHandleW(std::ptr::null());
        let class = wide(CLASS);
        let wc = WndClassW {
            style: 0,
            proc_: wndproc,
            cls_extra: 0,
            wnd_extra: 0,
            instance: inst,
            icon: LoadIconW(inst, 1), // the icon of the program (resource 1, see build.rs)
            cursor: LoadCursorW(0, IDC_ARROW),
            background: COLOR_WINDOW + 1,
            menu: std::ptr::null(),
            class_name: class.as_ptr(),
        };
        RegisterClassW(&wc);
        let style = WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX;
        let mut r = Rect { left: 0, top: 0, right: scale(CLIENT_W), bottom: scale(CLIENT_H) };
        AdjustWindowRect(&mut r, style, 0);
        let (w, h) = (r.right - r.left, r.bottom - r.top);
        let (x, y) = ((GetSystemMetrics(0) - w) / 2, (GetSystemMetrics(1) - h) / 3);
        let title = wide(launcher::text(gui().lang, Key::WindowTitle));
        let hwnd = CreateWindowExW(0, class.as_ptr(), title.as_ptr(), style, x, y, w, h, 0, 0, inst, std::ptr::null_mut());
        if hwnd == 0 {
            let _ = ready.send(());
            return;
        }
        gui().hwnd.store(hwnd, Ordering::SeqCst);
        draw(&first);
        ShowWindow(hwnd, SW_SHOW);
        SetForegroundWindow(hwnd);
        let _ = ready.send(());
        let mut msg = std::mem::zeroed::<Msg>();
        while GetMessageW(&mut msg, 0, 0, 0) > 0 {
            if IsDialogMessageW(hwnd, &mut msg) == 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
    }
    // the window is gone: so is the program, with the code the last run left
    std::process::exit(EXIT.load(Ordering::SeqCst));
}

fn open_window(first: Screen) {
    let g = gui();
    let mut slot = g.thread.lock().unwrap();
    if slot.is_some() {
        return;
    }
    let (tx, rx) = mpsc::channel();
    *slot = Some(std::thread::spawn(move || window_thread(first, tx)));
    let _ = rx.recv();
}

// ---- the `Ui` of the window ----

/// Tells the build's messages to the window. The window opens when something first has to be said, so a ROM that is
/// already built starts without one.
pub struct WindowUi {
    launched: AtomicBool,
    errors: Mutex<Vec<String>>,
}

impl WindowUi {
    fn new() -> Self {
        WindowUi { launched: AtomicBool::new(false), errors: Mutex::new(Vec::new()) }
    }

    fn busy(&self, text: String) {
        open_window(Screen::Busy(text.clone()));
        post_screen(Screen::Busy(text));
    }

    fn ask(&self, text: String) -> bool {
        open_window(Screen::Busy(String::new()));
        let (tx, rx) = mpsc::channel();
        *gui().answer.lock().unwrap() = Some(tx);
        post_screen(Screen::Ask(text));
        rx.recv().unwrap_or(false)
    }
}

impl Ui for WindowUi {
    fn status(&self, text: &str) {
        let g = gui();
        // the line that announces the first build is the one worth saying (in the language of the window)
        let shown = if text.starts_with("Building ") { launcher::text(g.lang, Key::FirstBuild).to_string() } else { text.to_string() };
        self.busy(shown);
    }

    fn error(&self, text: &str) {
        self.errors.lock().unwrap().push(text.to_string());
    }

    fn confirm(&self, question: &str) -> bool {
        self.ask(question.to_string())
    }

    fn offer_download(&self, notice: &DownloadNotice) -> bool {
        let text = match gui().lang {
            Lang::En => format!(
                "No C compiler was found on this machine. To build the game, {} ({} KiB) can be downloaded from {} and kept in {}.\n\nNothing about your ROM is sent; it is the only download.",
                notice.name,
                notice.kib,
                notice.url,
                launcher::shorten_path(&notice.dir.display().to_string(), 48)
            ),
            Lang::Es => format!(
                "No se ha encontrado ningún compilador de C en este equipo. Para construir el juego se puede descargar {} ({} KiB) de {} y guardarlo en {}.\n\nNo se envía nada de tu ROM; es la única descarga.",
                notice.name,
                notice.kib,
                notice.url,
                launcher::shorten_path(&notice.dir.display().to_string(), 48)
            ),
        };
        self.ask(text)
    }

    fn ask_rom(&self, _start_dir: &Path) -> Option<PathBuf> {
        None
    }

    fn launching(&self) {
        self.launched.store(true, Ordering::SeqCst);
        post(WM_APP_HIDE);
    }
}

// ---- entry points ----

fn init(cache: &Path, picks_rom: bool) {
    let _ = GUI.set(Gui {
        lang: ui_language(),
        cache: cache.to_path_buf(),
        picks_rom,
        hwnd: AtomicIsize::new(0),
        answer: Mutex::new(None),
        screen: Mutex::new(Screen::Pick),
        thread: Mutex::new(None),
    });
}

/// The launcher: the window opens at once and everything follows from its buttons. Returns when the window closes.
pub fn run_launcher(cache: &Path) -> i32 {
    init(cache, true);
    open_window(launcher::first_screen(cache));
    let handle = gui().thread.lock().unwrap().take();
    if let Some(h) = handle {
        let _ = h.join();
    }
    EXIT.load(Ordering::SeqCst)
}

/// A ROM given to the program with no terminal to talk to (dropped on the icon): builds and starts it, and shows a
/// window only when there is something to say (a build, a question, an error).
pub fn run_rom(rom: PathBuf, cache: &Path, launch: bool, fast: bool, download_yes: bool, game_args: Vec<String>, profiles: Vec<PathBuf>) -> i32 {
    init(cache, false);
    let ui = WindowUi::new();
    let code = port::run_port(rom, cache.to_path_buf(), launch, fast, download_yes, game_args, profiles, &ui);
    let had_window = gui().thread.lock().unwrap().is_some();
    if code == 0 {
        if had_window {
            EXIT.store(0, Ordering::SeqCst);
            post(WM_APP_QUIT);
        }
        return 0;
    }
    // a failure always gets its window, and the program waits for it to be read
    let screen = if ui.launched.load(Ordering::SeqCst) { launcher::game_failed(gui().lang, code) } else { launcher::failed(gui().lang, code, &ui.errors.lock().unwrap().join("\n")) };
    EXIT.store(code, Ordering::SeqCst);
    open_window(screen.clone());
    post_screen(screen);
    post(WM_APP_SHOW);
    let handle = gui().thread.lock().unwrap().take();
    if let Some(h) = handle {
        let _ = h.join();
    }
    code
}
