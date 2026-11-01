//! Announced updates: check GitHub at launch and daily, tell the user, and only on their press
//! download the exe and signature, verify, swap the files and relaunch.
use crate::signing::{message, PUBLIC_KEY};
use crate::{brand, gui};
use ed25519_dalek::{Signature, VerifyingKey};
use std::path::{Path, PathBuf};
use std::ptr::{null, null_mut};
use std::sync::Mutex;
use std::time::Duration;
use windows_sys::Win32::Networking::WinHttp::*;

const REPO: &str = "Gold4Rowmi/roam";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
const MAX_DOWNLOAD: usize = 64 << 20; // far above the real exe; stops a bad server filling memory

static AVAILABLE: Mutex<Option<String>> = Mutex::new(None);

type Version = (u32, u32, u32);

fn parse(v: &str) -> Option<Version> {
    let mut p = v.strip_prefix('v').unwrap_or(v).split('.').map(|x| x.parse().ok());
    let out = (p.next()??, p.next()??, p.next()??);
    p.next().is_none().then_some(out)
}

/// The tag from a releases/latest reply, if it is a plain version newer than `current`.
fn newer(json: &str, current: &str) -> Option<Version> {
    let (_, rest) = json.split_once("\"tag_name\"")?;
    let tag = parse(rest.split('"').nth(1)?)?;
    (tag > parse(current)?).then_some(tag)
}

fn verify(key: &[u8; 32], version: &str, exe: &[u8], sig: &[u8]) -> bool {
    let (Ok(key), Ok(sig)) = (VerifyingKey::from_bytes(key), Signature::from_slice(sig)) else { return false };
    key.verify_strict(&message(version, exe), &sig).is_ok()
}

/// Windows lets a running exe be renamed but not overwritten. Puts the old one back if the move fails.
fn swap(exe: &Path, new: &[u8]) -> std::io::Result<()> {
    let (tmp, old) = (exe.with_file_name("roam.new"), exe.with_file_name("roam.old"));
    std::fs::write(&tmp, new)?;
    let _ = std::fs::remove_file(&old);
    let moved = std::fs::rename(exe, &old).and_then(|_| {
        std::fs::rename(&tmp, exe).inspect_err(|_| { let _ = std::fs::rename(&old, exe); })
    });
    if moved.is_err() { let _ = std::fs::remove_file(&tmp); }
    moved
}

/// HTTPS GET through WinHTTP. Follows redirects; None on any failure or a non-200 reply.
fn get(url: &str) -> Option<Vec<u8>> {
    let wide = |s: &str| -> Vec<u16> { s.encode_utf16().chain([0]).collect() };
    let (host, path) = url.strip_prefix("https://")?.split_once('/')?;
    let (host, path) = (wide(host), wide(&format!("/{path}")));
    unsafe {
        let session = WinHttpOpen(wide("ROWMI-Roam").as_ptr(), WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY, null(), null(), 0);
        let conn = WinHttpConnect(session, host.as_ptr(), 443, 0);
        let req = WinHttpOpenRequest(conn, wide("GET").as_ptr(), path.as_ptr(), null(), null(), null(), WINHTTP_FLAG_SECURE);
        let body = read_reply(req);
        for h in [req, conn, session] { WinHttpCloseHandle(h); }
        body
    }
}

unsafe fn read_reply(req: *mut core::ffi::c_void) -> Option<Vec<u8>> {
    if WinHttpSendRequest(req, null(), 0, null(), 0, 0, 0) == 0 || WinHttpReceiveResponse(req, null_mut()) == 0 { return None }
    let (mut code, mut size) = (0u32, 4u32);
    WinHttpQueryHeaders(req, WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER, null(), &mut code as *mut _ as _, &mut size, null_mut());
    if code != 200 { return None }
    let (mut body, mut chunk) = (Vec::new(), vec![0u8; 64 * 1024]);
    loop {
        let mut n = 0u32;
        if WinHttpReadData(req, chunk.as_mut_ptr() as _, chunk.len() as u32, &mut n) == 0 { return None }
        if n == 0 { return Some(body) }
        body.extend_from_slice(&chunk[..n as usize]);
        if body.len() > MAX_DOWNLOAD { return None }
    }
}

fn check() -> Option<Version> {
    let json = get(&format!("https://api.github.com/repos/{REPO}/releases/latest"))?;
    newer(&String::from_utf8_lossy(&json), VERSION)
}

/// Starts the daily check. Failures stay quiet: being offline is not news.
pub fn start() {
    std::thread::spawn(|| {
        std::thread::sleep(Duration::from_secs(3)); // past the boot screen, and the previous exe has exited
        if let Ok(exe) = std::env::current_exe() { let _ = std::fs::remove_file(exe.with_file_name("roam.old")); }
        let mut told = None;
        loop {
            let found = check();
            if let (Some((a, b, c)), true) = (found, found != told) {
                told = found;
                let v = format!("{a}.{b}.{c}");
                *AVAILABLE.lock().unwrap() = Some(v.clone());
                gui::offer(&v);
                gui::status(&format!("Roam {v} is ready. Press Update to install it."));
            }
            std::thread::sleep(Duration::from_secs(24 * 60 * 60));
        }
    });
}

fn download_and_swap(v: &str) -> Result<PathBuf, &'static str> {
    let exe = std::env::current_exe().map_err(|_| "Could not find the Roam app file. Nothing was changed.")?;
    let url = format!("https://github.com/{REPO}/releases/download/v{v}/roam.exe");
    let (new, sig) = get(&url).zip(get(&format!("{url}.sig")))
        .ok_or("The download did not finish. Check the connection and press Update again.")?;
    if !verify(&PUBLIC_KEY, v, &new, &sig) {
        return Err("The update failed its signature check, so it was thrown away. Nothing was changed.");
    }
    swap(&exe, &new).map_err(|_| "Could not replace the app file. Nothing was changed. Close other copies of Roam and try again.")?;
    Ok(exe)
}

/// Runs after the Update press. On success the new exe starts and this one closes.
pub fn install() {
    let Some(v) = AVAILABLE.lock().unwrap().clone() else { return };
    std::thread::spawn(move || {
        gui::status(&format!("Downloading Roam {v}..."));
        match download_and_swap(&v).map(|exe| std::process::Command::new(exe).spawn()) {
            Ok(Ok(_)) => gui::close(),
            Ok(Err(_)) => gui::status("Roam is updated. Quit and open it again to finish."),
            Err(why) => {
                brand::play(brand::ERROR);
                gui::status(why);
                gui::offer(&v);
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};

    #[test]
    fn only_a_newer_plain_version_counts() {
        let reply = |tag: &str| format!("{{\"url\":\"x\", \"tag_name\": \"{tag}\", \"body\":\"\"}}");
        assert_eq!(newer(&reply("v0.3.0"), "0.2.0"), Some((0, 3, 0)));
        assert_eq!(newer(&reply("v0.10.0"), "0.9.0"), Some((0, 10, 0))); // numbers, not strings
        assert_eq!(newer(&reply("v0.2.0"), "0.2.0"), None);
        assert_eq!(newer(&reply("v0.1.9"), "0.2.0"), None);
        for bad in ["v1.0", "v1.0.0-rc1", "v1.0.0.0", "v1.0.0/../../evil", ""] {
            assert_eq!(newer(&reply(bad), "0.2.0"), None, "{bad}");
        }
        assert_eq!(newer("{\"message\":\"Not Found\"}", "0.2.0"), None);
    }

    #[test]
    fn signature_binds_key_exe_and_version() {
        let key = SigningKey::from_bytes(&[7; 32]);
        let public = key.verifying_key().to_bytes();
        let sig = key.sign(&message("0.3.0", b"exe")).to_bytes();
        assert!(verify(&public, "0.3.0", b"exe", &sig));
        assert!(!verify(&public, "0.3.0", b"evil", &sig));
        assert!(!verify(&public, "0.4.0", b"exe", &sig)); // old signed exe offered as a newer release
        assert!(!verify(&[9; 32], "0.3.0", b"exe", &sig));
        assert!(!verify(&public, "0.3.0", b"exe", &sig[..63]));
    }

    #[test]
    fn swap_keeps_the_old_file_and_changes_nothing_on_failure() {
        let dir = std::env::temp_dir().join(format!("roam-swap-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let exe = dir.join("roam.exe");
        std::fs::write(&exe, "old").unwrap();
        swap(&exe, b"new").unwrap();
        assert_eq!(std::fs::read(&exe).unwrap(), b"new");
        assert_eq!(std::fs::read(dir.join("roam.old")).unwrap(), b"old");
        assert!(!dir.join("roam.new").exists());

        std::fs::create_dir(dir.join("roam.new")).unwrap(); // the temp file can't be written
        assert!(swap(&exe, b"newer").is_err());
        assert_eq!(std::fs::read(&exe).unwrap(), b"new");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// Real GitHub, so ignored by default: cargo test -- --ignored
    #[test]
    #[ignore]
    fn winhttp_reaches_github_and_follows_the_asset_redirect() {
        let json = get("https://api.github.com/repos/rust-lang/rust-analyzer/releases/latest").unwrap();
        let json = String::from_utf8_lossy(&json);
        assert!(newer(&json, "0.0.0").is_none()); // its tags are dates, not versions, and must be refused
        let tag = json.split_once("\"tag_name\"").unwrap().1.split('"').nth(1).unwrap();
        let url = format!("https://github.com/rust-lang/rust-analyzer/releases/download/{tag}/rust-analyzer-no-server.vsix");
        assert!(get(&url).unwrap().len() > 100_000);
        assert!(get("https://github.com/rust-lang/rust-analyzer/releases/download/nope/nope").is_none());
    }
}
