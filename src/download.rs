//! The only code of the program that uses the network: the download of a portable C compiler when the machine has
//! none, and only after the user agreed. What is downloaded must match a SHA-256 fixed in the program, or it is
//! thrown away and never run. The ROM and anything derived from it never go through here (nor anywhere else on the
//! network): the program sends nothing, it only fetches one file.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::cell::Cell;

/// A compiler that can be downloaded, with the hash its file must have.
pub struct Portable {
    pub name: &'static str,
    pub url: &'static str,
    pub sha256: &'static str,
    pub size: u64,
    /// Where the compiler is once the zip is extracted into `<cache>/toolchain`.
    pub exe: &'static str,
}

/// TinyCC 0.9.27 (LGPL): compiles the whole game in under a second and runs at about twice the speed needed (see
/// `openspec/changes/rom-to-port-builder/design.md`).
pub const TINYCC: Portable = Portable {
    name: "TinyCC 0.9.27",
    url: "https://download.savannah.gnu.org/releases/tinycc/tcc-0.9.27-win64-bin.zip",
    sha256: "34a721949a2583fdff725312da092fa0f5f1f284b702e6f811c6954714faabb2",
    size: 489_586,
    exe: "tcc/tcc.exe",
};

thread_local! {
    static REQUESTS: Cell<usize> = const { Cell::new(0) };
}

/// How many downloads this thread has started (the tests check that a local build starts none).
pub fn network_requests() -> usize {
    REQUESTS.with(Cell::get)
}

/// The folder of the downloaded compilers inside the cache.
pub fn toolchain_dir(cache: &Path) -> PathBuf {
    cache.join("toolchain")
}

/// The downloaded compiler, if it is already in the cache.
pub fn installed(cache: &Path, p: &Portable) -> Option<PathBuf> {
    let exe = toolchain_dir(cache).join(p.exe);
    exe.exists().then_some(exe)
}

/// How many times a download is tried before giving up.
const ATTEMPTS: usize = 4;

/// Downloads `url` into `dest`, keeping it only if its SHA-256 is `sha256` (hex). A file that does not match is
/// deleted and an error says so.
pub fn fetch_verified(url: &str, sha256: &str, dest: &Path) -> Result<(), String> {
    REQUESTS.with(|r| r.set(r.get() + 1));
    let part = dest.with_extension("part");
    let _ = std::fs::remove_file(&part);
    // the download site redirects each request to one of its mirrors, and some of them are down or slow: a failed attempt
    // is tried again (the next request may land on another mirror), waiting at most 10 s to connect
    let mut failure = String::new();
    let mut done = false;
    for _ in 0..ATTEMPTS {
        let out = Command::new("curl")
            .args(["--fail", "--silent", "--show-error", "--location", "--connect-timeout", "10", "--max-time", "300", "--output"])
            .arg(&part)
            .arg(url)
            .output()
            .map_err(|e| format!("cannot run curl to download {url}: {e}"))?;
        if out.status.success() {
            done = true;
            break;
        }
        failure = String::from_utf8_lossy(&out.stderr).trim().to_string();
    }
    if !done {
        let _ = std::fs::remove_file(&part);
        return Err(format!("the download of {url} failed: {failure}"));
    }
    let data = std::fs::read(&part).map_err(|e| format!("{}: {e}", part.display()))?;
    let got = hex(&sha256_of(&data));
    if !got.eq_ignore_ascii_case(sha256) {
        let _ = std::fs::remove_file(&part);
        return Err(format!("the downloaded file does not have the expected SHA-256 (expected {sha256}, got {got}): discarded"));
    }
    std::fs::rename(&part, dest).map_err(|e| format!("{}: {e}", dest.display()))
}

/// Downloads and unpacks a portable compiler into the cache; returns its executable.
pub fn install(cache: &Path, p: &Portable) -> Result<PathBuf, String> {
    let dir = toolchain_dir(cache);
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let zip = dir.join("compiler.zip");
    fetch_verified(p.url, p.sha256, &zip)?;
    // `tar` (bsdtar, part of Windows 10 and later) unpacks zip files
    let out = Command::new("tar").arg("-xf").arg(&zip).arg("-C").arg(&dir).output().map_err(|e| format!("cannot run tar to unpack {}: {e}", zip.display()))?;
    let _ = std::fs::remove_file(&zip);
    if !out.status.success() {
        return Err(format!("cannot unpack {}: {}", p.name, String::from_utf8_lossy(&out.stderr).trim()));
    }
    installed(cache, p).ok_or_else(|| format!("{} was unpacked but {} is not there", p.name, p.exe))
}

/// Asks `question` and reads the answer from `input`: only `y` or `yes` agree.
pub fn confirm(question: &str, input: &mut dyn std::io::BufRead) -> bool {
    eprint!("{question} [y/N] ");
    let mut line = String::new();
    input.read_line(&mut line).is_ok() && matches!(line.trim().to_ascii_lowercase().as_str(), "y" | "yes")
}

// ---- SHA-256 ----

fn primes(n: usize) -> Vec<u64> {
    let mut v = Vec::new();
    let mut c = 2u64;
    while v.len() < n {
        if v.iter().all(|p| c % p != 0) {
            v.push(c);
        }
        c += 1;
    }
    v
}

/// SHA-256 of `data`. The constants are computed (fractional parts of the square and cube roots of the first
/// primes), as the standard defines them, instead of typed in.
pub fn sha256_of(data: &[u8]) -> [u8; 32] {
    let frac = |x: f64| ((x - x.floor()) * 4294967296.0) as u32;
    let p = primes(64);
    let k: Vec<u32> = p.iter().map(|&q| frac((q as f64).cbrt())).collect();
    let mut h: [u32; 8] = std::array::from_fn(|i| frac((p[i] as f64).sqrt()));
    let mut msg = data.to_vec();
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&((data.len() as u64) * 8).to_be_bytes());
    for chunk in msg.chunks(64) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([chunk[4 * i], chunk[4 * i + 1], chunk[4 * i + 2], chunk[4 * i + 3]]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16].wrapping_add(s0).wrapping_add(w[i - 7]).wrapping_add(s1);
        }
        let mut v = h;
        for i in 0..64 {
            let s1 = v[4].rotate_right(6) ^ v[4].rotate_right(11) ^ v[4].rotate_right(25);
            let ch = (v[4] & v[5]) ^ (!v[4] & v[6]);
            let t1 = v[7].wrapping_add(s1).wrapping_add(ch).wrapping_add(k[i]).wrapping_add(w[i]);
            let s0 = v[0].rotate_right(2) ^ v[0].rotate_right(13) ^ v[0].rotate_right(22);
            let maj = (v[0] & v[1]) ^ (v[0] & v[2]) ^ (v[1] & v[2]);
            let t2 = s0.wrapping_add(maj);
            v = [t1.wrapping_add(t2), v[0], v[1], v[2], v[3].wrapping_add(t1), v[4], v[5], v[6]];
        }
        for i in 0..8 {
            h[i] = h[i].wrapping_add(v[i]);
        }
    }
    let mut out = [0u8; 32];
    for i in 0..8 {
        out[4 * i..4 * i + 4].copy_from_slice(&h[i].to_be_bytes());
    }
    out
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    #[test]
    fn sha256_matches_the_standard_vectors() {
        assert_eq!(hex(&sha256_of(b"")), "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
        assert_eq!(hex(&sha256_of(b"abc")), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
        assert_eq!(
            hex(&sha256_of(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq")),
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
        );
        let million_a = vec![b'a'; 1_000_000];
        assert_eq!(hex(&sha256_of(&million_a)), "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0");
    }

    /// Serves `body` once on a local port; returns the URL.
    fn serve(body: Vec<u8>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        std::thread::spawn(move || {
            if let Ok((mut s, _)) = listener.accept() {
                let mut req = [0u8; 1024];
                let _ = s.read(&mut req);
                let head = format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
                let _ = s.write_all(head.as_bytes());
                let _ = s.write_all(&body);
            }
        });
        format!("http://127.0.0.1:{port}/compiler.zip")
    }

    fn temp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("snes-dl-test-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn a_download_with_the_wrong_hash_is_discarded_and_one_with_the_right_hash_is_kept() {
        if Command::new("curl").arg("--version").output().is_err() {
            eprintln!("skipped: no curl");
            return;
        }
        let dir = temp("hash");
        let good = b"pretend this is a compiler".to_vec();
        let want = hex(&sha256_of(&good));
        // altered on the way: rejected, and nothing is left behind
        let mut altered = good.clone();
        altered[3] ^= 1;
        let dest = dir.join("compiler.zip");
        let err = fetch_verified(&serve(altered), &want, &dest).unwrap_err();
        assert!(err.contains("SHA-256") && err.contains("discarded"), "{err}");
        assert!(!dest.exists() && !dest.with_extension("part").exists(), "the altered file is not kept");
        // the right file: kept
        fetch_verified(&serve(good.clone()), &want, &dest).unwrap();
        assert_eq!(std::fs::read(&dest).unwrap(), good);
        // a server that does not answer well: an error, nothing kept
        let dest2 = dir.join("other.zip");
        assert!(fetch_verified("http://127.0.0.1:9/none.zip", &want, &dest2).is_err() && !dest2.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn only_a_yes_agrees_to_the_download() {
        for (answer, yes) in [("y\n", true), ("YES\n", true), ("n\n", false), ("\n", false), ("", false), ("sure\n", false)] {
            assert_eq!(confirm("Download?", &mut answer.as_bytes()), yes, "{answer:?}");
        }
    }

    #[test]
    fn the_compiler_to_download_has_a_fixed_https_source_and_hash() {
        assert!(TINYCC.url.starts_with("https://"));
        assert_eq!(TINYCC.sha256.len(), 64);
        assert!(TINYCC.sha256.bytes().all(|b| b.is_ascii_hexdigit()));
    }

    /// Needs the network: `cargo test -- --ignored the_real_tinycc`.
    #[test]
    #[ignore]
    fn the_real_tinycc_download_matches_its_hash_and_unpacks() {
        let cache = temp("real");
        let exe = install(&cache, &TINYCC).unwrap();
        assert!(exe.exists() && installed(&cache, &TINYCC).is_some());
        assert!(!toolchain_dir(&cache).join("compiler.zip").exists());
        let _ = std::fs::remove_dir_all(&cache);
    }
}
