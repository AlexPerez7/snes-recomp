//! Update notice and installation for the desktop packages (`desktop-updates`): the launcher asks GitHub for the latest
//! release of the game's repository, says when it is newer than the installed one and, if the person agrees, downloads
//! the package of this platform, checks it against the `SHA256SUMS` of the same release and copies its files over the
//! installation (the saves and anything else that is not in the package stay). Off by default: nothing goes to the
//! network until the person turns it on. The installation says what it is and where to look in `update.info`
//! (`repo=owner/name`, `version=0.1`) next to the executable; without that file there is nothing to check.
//! Only `fetch_verified` and `http_get` touch the network, and they only fetch: nothing of the ROM or the machine is sent.

use crate::download;
use std::ffi::{c_char, CStr, CString};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Where the installation says what it is.
pub struct Info {
    pub repo: String,
    pub version: String,
}

pub fn read_info(exe_dir: &Path) -> Option<Info> {
    let text = std::fs::read_to_string(exe_dir.join("update.info")).ok()?;
    let get = |k: &str| text.lines().find_map(|l| l.strip_prefix(k)?.strip_prefix('=')).map(|v| v.trim().to_string()).filter(|v| !v.is_empty());
    let (repo, version) = (get("repo")?, get("version")?);
    (repo.split('/').count() == 2 && !repo.contains(' ') && looks_like_version(&version)).then_some(Info { repo, version })
}

fn parts(v: &str) -> Option<Vec<u64>> {
    let v = v.strip_prefix('v').unwrap_or(v);
    v.split('.').map(|p| p.parse::<u64>().ok()).collect()
}

pub fn looks_like_version(v: &str) -> bool {
    parts(v).is_some_and(|p| !p.is_empty())
}

/// Whether `tag` (`v0.10`) is a newer version than `current` (`0.9`); numeric, so 0.10 > 0.9. A tag that is not a version never is.
pub fn is_newer(tag: &str, current: &str) -> bool {
    let (Some(mut a), Some(mut b)) = (parts(tag), parts(current)) else { return false };
    let n = a.len().max(b.len());
    a.resize(n, 0);
    b.resize(n, 0);
    a > b
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Release {
    pub tag: String,
    pub page: String,
    pub assets: Vec<(String, String)>,
}

pub fn parse_release(json: &str) -> Result<Release, String> {
    let v: serde_json::Value = serde_json::from_str(json).map_err(|e| format!("not a release: {e}"))?;
    let tag = v["tag_name"].as_str().ok_or("no tag_name")?.to_string();
    let page = v["html_url"].as_str().ok_or("no html_url")?.to_string();
    if !safe_url(&page) || !looks_like_version(&tag) {
        return Err(format!("unusable release {tag}"));
    }
    let assets = v["assets"]
        .as_array()
        .map(|a| a.iter().filter_map(|x| Some((x["name"].as_str()?.to_string(), x["browser_download_url"].as_str()?.to_string()))).filter(|(_, u)| safe_url(u)).collect())
        .unwrap_or_default();
    Ok(Release { tag, page, assets })
}

/// Only HTTPS links are followed (plus this machine, for the tests of the update with a local server).
fn safe_url(u: &str) -> bool {
    u.starts_with("https://") || u.starts_with("http://127.0.0.1") || u.starts_with("http://localhost")
}

/// The suffix of the package of this platform in the release.
pub fn platform_suffix() -> &'static str {
    if cfg!(windows) {
        "-win64.zip"
    } else {
        "-linux-x86_64.tar.gz"
    }
}

pub fn package_of(r: &Release) -> Option<&(String, String)> {
    r.assets.iter().find(|(n, _)| n.ends_with(platform_suffix()))
}

/// The hash of `name` in a `SHA256SUMS` text (`<hex>  <name>` per line).
pub fn hash_in_sums(sums: &str, name: &str) -> Option<String> {
    sums.lines().find_map(|l| {
        let (h, n) = l.split_once(char::is_whitespace)?;
        (n.trim().trim_start_matches('*') == name && h.len() == 64 && h.bytes().all(|b| b.is_ascii_hexdigit())).then(|| h.to_ascii_lowercase())
    })
}

/// GET with curl (Windows 10 and later have it) or wget; at most 6 s to connect and 20 s in all.
pub fn http_get(url: &str) -> Result<Vec<u8>, String> {
    let out = Command::new("curl")
        .args(["--fail", "--silent", "--location", "--connect-timeout", "6", "--max-time", "20", "-H", "User-Agent: snes-port-updater", "-H", "Accept: application/vnd.github+json"])
        .arg(url)
        .output();
    match out {
        Ok(o) if o.status.success() => return Ok(o.stdout),
        Ok(o) => return Err(format!("{url}: {}", String::from_utf8_lossy(&o.stderr).trim())),
        Err(_) => {}
    }
    let o = Command::new("wget").args(["-q", "-O", "-", "--timeout=10", "--user-agent=snes-port-updater"]).arg(url).output().map_err(|_| "neither curl nor wget is available".to_string())?;
    if o.status.success() {
        Ok(o.stdout)
    } else {
        Err(format!("{url}: download failed"))
    }
}

pub fn latest(repo: &str) -> Result<Release, String> {
    // SNES_PORT_UPDATE_API replaces the GitHub address (to try the update against a local server)
    let base = std::env::var("SNES_PORT_UPDATE_API").ok().filter(|b| safe_url(b)).unwrap_or_else(|| "https://api.github.com".to_string());
    let body = http_get(&format!("{base}/repos/{repo}/releases/latest"))?;
    parse_release(&String::from_utf8_lossy(&body))
}

// ---- the person's choices, in a small file ----

#[derive(Default, Debug, PartialEq, Eq)]
pub struct Settings {
    pub check: bool,
    /// Unix seconds of the last check that worked.
    pub last: u64,
    pub tag: String,
    pub page: String,
    pub skipped: String,
}

fn settings_path(dir: &Path) -> PathBuf {
    dir.join("update.txt")
}

pub fn load_settings(dir: &Path) -> Settings {
    let text = std::fs::read_to_string(settings_path(dir)).unwrap_or_default();
    let get = |k: &str| text.lines().find_map(|l| l.strip_prefix(k)?.strip_prefix('=')).unwrap_or("").trim().to_string();
    Settings { check: get("check") == "1", last: get("last").parse().unwrap_or(0), tag: get("tag"), page: get("page"), skipped: get("skipped") }
}

pub fn save_settings(dir: &Path, s: &Settings) {
    let _ = std::fs::create_dir_all(dir);
    let _ = std::fs::write(settings_path(dir), format!("check={}\nlast={}\ntag={}\npage={}\nskipped={}\n", s.check as u8, s.last, s.tag, s.page, s.skipped));
}

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

const DAY: u64 = 24 * 3600;

#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Nothing newer (or the check is off).
    UpToDate,
    /// A newer version that the person has not skipped.
    Available { tag: String, page: String },
    /// A newer version that the person chose to skip.
    Skipped,
    Failed(String),
}

/// The check of the launcher: with `force` it always asks; otherwise only when the check is on and the last answer is
/// more than a day old (a fresh answer is reused without touching the network).
pub fn check(dir: &Path, info: &Info, force: bool) -> Outcome {
    let mut s = load_settings(dir);
    if !force && !s.check {
        return Outcome::UpToDate;
    }
    if force || now().saturating_sub(s.last) >= DAY || s.tag.is_empty() {
        match latest(&info.repo) {
            Ok(r) => {
                s.last = now();
                s.tag = r.tag;
                s.page = r.page;
                save_settings(dir, &s);
            }
            Err(e) => return Outcome::Failed(e),
        }
    }
    if !is_newer(&s.tag, &info.version) {
        Outcome::UpToDate
    } else if s.skipped == s.tag {
        Outcome::Skipped
    } else {
        Outcome::Available { tag: s.tag, page: s.page }
    }
}

pub fn skip(dir: &Path, tag: &str) {
    let mut s = load_settings(dir);
    s.skipped = tag.to_string();
    save_settings(dir, &s);
}

pub fn set_check(dir: &Path, on: bool) {
    let mut s = load_settings(dir);
    s.check = on;
    save_settings(dir, &s);
}

// ---- installing ----

/// Copies the files of `from` over `to`. A file that cannot be replaced because it is running (the executable on
/// Windows) is moved aside as `<name>.old` first; `clean_old` removes those at the next start.
pub fn copy_over(from: &Path, to: &Path) -> Result<(), String> {
    std::fs::create_dir_all(to).map_err(|e| format!("{}: {e}", to.display()))?;
    for entry in std::fs::read_dir(from).map_err(|e| format!("{}: {e}", from.display()))? {
        let entry = entry.map_err(|e| e.to_string())?;
        let (src, dst) = (entry.path(), to.join(entry.file_name()));
        if entry.file_type().map_err(|e| e.to_string())?.is_dir() {
            copy_over(&src, &dst)?;
            continue;
        }
        let tmp = dst.with_file_name(format!("{}.new", entry.file_name().to_string_lossy()));
        std::fs::copy(&src, &tmp).map_err(|e| format!("{}: {e}", tmp.display()))?;
        if std::fs::rename(&tmp, &dst).is_err() {
            let old = dst.with_file_name(format!("{}.old", entry.file_name().to_string_lossy()));
            let _ = std::fs::remove_file(&old);
            std::fs::rename(&dst, &old).map_err(|e| format!("{}: {e}", dst.display()))?;
            std::fs::rename(&tmp, &dst).map_err(|e| format!("{}: {e}", dst.display()))?;
        }
    }
    Ok(())
}

/// Removes the `.old` files a previous update left (at start; whatever cannot be removed stays for the next time).
pub fn clean_old(dir: &Path) {
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                clean_old(&p);
            } else if p.extension().is_some_and(|x| x == "old") {
                let _ = std::fs::remove_file(&p);
            }
        }
    }
}

/// Downloads the package of this platform of the latest release, checks it against its `SHA256SUMS` and installs it
/// over `install_dir`. Returns the version installed.
pub fn install(info: &Info, install_dir: &Path, work: &Path) -> Result<String, String> {
    let r = latest(&info.repo)?;
    if !is_newer(&r.tag, &info.version) {
        return Err("there is no newer version".into());
    }
    let (name, url) = package_of(&r).ok_or_else(|| format!("the release {} has no package for this system", r.tag))?;
    let (_, sums_url) = r.assets.iter().find(|(n, _)| n == "SHA256SUMS").ok_or("the release has no SHA256SUMS: not installing")?;
    let sums = String::from_utf8_lossy(&http_get(sums_url)?).to_string();
    let hash = hash_in_sums(&sums, name).ok_or_else(|| format!("SHA256SUMS does not list {name}: not installing"))?;
    let _ = std::fs::remove_dir_all(work);
    std::fs::create_dir_all(work).map_err(|e| format!("{}: {e}", work.display()))?;
    let pkg = work.join(name);
    download::fetch_verified(url, &hash, &pkg)?;
    let out = work.join("x");
    std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
    let t = Command::new("tar").arg("-xf").arg(&pkg).arg("-C").arg(&out).output().map_err(|e| format!("cannot run tar: {e}"))?;
    if !t.status.success() {
        return Err(format!("cannot unpack {name}: {}", String::from_utf8_lossy(&t.stderr).trim()));
    }
    // the package is one folder; its executable must be there or this is not a package of the game
    let root = std::fs::read_dir(&out).map_err(|e| e.to_string())?.flatten().map(|e| e.path()).find(|p| p.is_dir()).ok_or("the package is empty")?;
    if !root.join("update.info").is_file() {
        return Err("the package has no update.info: not installing".into());
    }
    copy_over(&root, install_dir)?;
    let _ = std::fs::remove_dir_all(work);
    Ok(r.tag.trim_start_matches('v').to_string())
}

// ---- C interface for the Linux launcher ----

/// C interface (`update` of `linux/launcher.c`): all strings are UTF-8, results are freed with `snes_upd_free`.
pub mod ffi {
    use super::*;

    pub struct SnesUpd {
        /// 0 up to date, 1 available, 2 skipped, 3 failed
        kind: i32,
        a: CString,
        b: CString,
    }

    unsafe fn s(p: *const c_char) -> String {
        if p.is_null() {
            String::new()
        } else {
            CStr::from_ptr(p).to_string_lossy().into_owned()
        }
    }

    fn cs(x: &str) -> CString {
        CString::new(x.replace('\0', "")).unwrap()
    }

    fn info(exe_dir: &str) -> Option<Info> {
        read_info(Path::new(exe_dir))
    }

    /// The version installed, or NULL when this installation does not update itself.
    #[no_mangle]
    pub unsafe extern "C" fn snes_upd_version(exe_dir: *const c_char) -> *mut SnesUpd {
        match info(&s(exe_dir)) {
            Some(i) => Box::into_raw(Box::new(SnesUpd { kind: 0, a: cs(&i.version), b: cs(&i.repo) })),
            None => std::ptr::null_mut(),
        }
    }

    #[no_mangle]
    pub unsafe extern "C" fn snes_upd_enabled(dir: *const c_char) -> i32 {
        load_settings(Path::new(&s(dir))).check as i32
    }

    #[no_mangle]
    pub unsafe extern "C" fn snes_upd_set_enabled(dir: *const c_char, on: i32) {
        set_check(Path::new(&s(dir)), on != 0)
    }

    #[no_mangle]
    pub unsafe extern "C" fn snes_upd_skip(dir: *const c_char, tag: *const c_char) {
        skip(Path::new(&s(dir)), &s(tag))
    }

    #[no_mangle]
    pub unsafe extern "C" fn snes_upd_clean(exe_dir: *const c_char) {
        clean_old(Path::new(&s(exe_dir)))
    }

    /// Blocks on the network (call it from a thread). `force` ignores the setting and the one-day memory.
    #[no_mangle]
    pub unsafe extern "C" fn snes_upd_check(dir: *const c_char, exe_dir: *const c_char, force: i32) -> *mut SnesUpd {
        let (d, e) = (s(dir), s(exe_dir));
        let r = match info(&e) {
            None => SnesUpd { kind: 0, a: cs(""), b: cs("") },
            Some(i) => match check(Path::new(&d), &i, force != 0) {
                Outcome::UpToDate => SnesUpd { kind: 0, a: cs(""), b: cs("") },
                Outcome::Available { tag, page } => SnesUpd { kind: 1, a: cs(&tag), b: cs(&page) },
                Outcome::Skipped => SnesUpd { kind: 2, a: cs(""), b: cs("") },
                Outcome::Failed(m) => SnesUpd { kind: 3, a: cs(&m), b: cs("") },
            },
        };
        Box::into_raw(Box::new(r))
    }

    /// Blocks (download): installs over `exe_dir`. The result is kind 0 with the version in `a`, or kind 3 with the error.
    #[no_mangle]
    pub unsafe extern "C" fn snes_upd_install(dir: *const c_char, exe_dir: *const c_char) -> *mut SnesUpd {
        let (d, e) = (s(dir), s(exe_dir));
        let r = match info(&e) {
            None => SnesUpd { kind: 3, a: cs("this installation does not update itself"), b: cs("") },
            Some(i) => match install(&i, Path::new(&e), &Path::new(&d).join("update-work")) {
                Ok(v) => SnesUpd { kind: 0, a: cs(&v), b: cs("") },
                Err(m) => SnesUpd { kind: 3, a: cs(&m), b: cs("") },
            },
        };
        Box::into_raw(Box::new(r))
    }

    #[no_mangle]
    pub unsafe extern "C" fn snes_upd_kind(r: *const SnesUpd) -> i32 {
        (*r).kind
    }
    #[no_mangle]
    pub unsafe extern "C" fn snes_upd_a(r: *const SnesUpd) -> *const c_char {
        (*r).a.as_ptr()
    }
    #[no_mangle]
    pub unsafe extern "C" fn snes_upd_b(r: *const SnesUpd) -> *const c_char {
        (*r).b.as_ptr()
    }
    #[no_mangle]
    pub unsafe extern "C" fn snes_upd_free(r: *mut SnesUpd) {
        if !r.is_null() {
            drop(Box::from_raw(r));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("snes-upd-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn versions_compare_numerically() {
        assert!(is_newer("v0.10", "0.9"));
        assert!(is_newer("v0.2", "0.1"));
        assert!(is_newer("v1.0.1", "1.0"));
        assert!(!is_newer("v0.1", "0.1"));
        assert!(!is_newer("v0.0.9", "0.1"));
        assert!(!is_newer("nightly", "0.1"));
        assert!(!is_newer("v0.2", "x"));
    }

    #[test]
    fn a_release_is_read_and_its_package_found() {
        let j = r#"{"tag_name":"v0.2","html_url":"https://github.com/a/b/releases/tag/v0.2","assets":[
            {"name":"g-0.2-win64.zip","browser_download_url":"https://x/w.zip"},
            {"name":"g-0.2-linux-x86_64.tar.gz","browser_download_url":"https://x/l.tgz"},
            {"name":"SHA256SUMS","browser_download_url":"https://x/s"},
            {"name":"evil","browser_download_url":"http://x/e"}]}"#;
        let r = parse_release(j).unwrap();
        assert_eq!(r.tag, "v0.2");
        assert_eq!(r.assets.len(), 3, "an http link is dropped");
        assert!(package_of(&r).unwrap().0.ends_with(platform_suffix()));
        assert!(parse_release(r#"{"tag_name":"nightly","html_url":"https://x"}"#).is_err());
        assert!(parse_release(r#"{"tag_name":"v1","html_url":"http://x"}"#).is_err());
    }

    #[test]
    fn sums_are_looked_up_by_name() {
        let h = "a".repeat(64);
        let sums = format!("{h}  g-0.2-win64.zip\n{}  *other\nbad  g-0.2-linux-x86_64.tar.gz\n", "B".repeat(64));
        assert_eq!(hash_in_sums(&sums, "g-0.2-win64.zip").as_deref(), Some(h.as_str()));
        assert_eq!(hash_in_sums(&sums, "other").unwrap(), "b".repeat(64));
        assert!(hash_in_sums(&sums, "g-0.2-linux-x86_64.tar.gz").is_none(), "a malformed hash is not one");
        assert!(hash_in_sums(&sums, "missing").is_none());
    }

    #[test]
    fn info_needs_a_repo_and_a_version() {
        let d = tmp("info");
        assert!(read_info(&d).is_none());
        std::fs::write(d.join("update.info"), "repo=AlexPerez7/alttp-recomp\nversion=0.1\n").unwrap();
        let i = read_info(&d).unwrap();
        assert_eq!((i.repo.as_str(), i.version.as_str()), ("AlexPerez7/alttp-recomp", "0.1"));
        std::fs::write(d.join("update.info"), "repo=oops\nversion=0.1\n").unwrap();
        assert!(read_info(&d).is_none());
    }

    #[test]
    fn the_check_is_off_by_default_and_asks_nothing_then() {
        let d = tmp("off");
        let i = Info { repo: "a/b".into(), version: "0.1".into() };
        let before = download::network_requests();
        assert_eq!(check(&d, &i, false), Outcome::UpToDate);
        assert_eq!(load_settings(&d), Settings::default());
        assert_eq!(download::network_requests(), before);
    }

    #[test]
    fn a_fresh_answer_is_reused_and_a_skipped_version_stays_quiet() {
        let d = tmp("cache");
        let i = Info { repo: "a/b".into(), version: "0.1".into() };
        save_settings(&d, &Settings { check: true, last: now(), tag: "v0.2".into(), page: "https://p".into(), skipped: String::new() });
        assert_eq!(check(&d, &i, false), Outcome::Available { tag: "v0.2".into(), page: "https://p".into() });
        skip(&d, "v0.2");
        assert_eq!(check(&d, &i, false), Outcome::Skipped);
        save_settings(&d, &Settings { check: true, last: now(), tag: "v0.1".into(), page: "https://p".into(), skipped: String::new() });
        assert_eq!(check(&d, &i, false), Outcome::UpToDate);
    }

    #[test]
    fn copy_over_keeps_what_is_not_in_the_package_and_clean_old_removes_leftovers() {
        let (from, to) = (tmp("from"), tmp("to"));
        std::fs::create_dir_all(from.join("lib")).unwrap();
        std::fs::write(from.join("app"), "new").unwrap();
        std::fs::write(from.join("lib/x.so"), "newlib").unwrap();
        std::fs::create_dir_all(to.join("saves")).unwrap();
        std::fs::write(to.join("saves/a.srm"), "mine").unwrap();
        std::fs::write(to.join("app"), "old").unwrap();
        std::fs::write(to.join("app.old"), "leftover").unwrap();
        copy_over(&from, &to).unwrap();
        assert_eq!(std::fs::read_to_string(to.join("app")).unwrap(), "new");
        assert_eq!(std::fs::read_to_string(to.join("lib/x.so")).unwrap(), "newlib");
        assert_eq!(std::fs::read_to_string(to.join("saves/a.srm")).unwrap(), "mine");
        assert!(!to.join("app.new").exists());
        clean_old(&to);
        assert!(!to.join("app.old").exists());
    }
}
