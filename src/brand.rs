//! ROWMI brand, from the brand package v1.0: tokens, Rommel's seven poses and the wordmark as exact
//! pixel data (logo/ and rommel/), and the real WAVs (sounds/) embedded in the app.
//! Drawing them is per platform: `brand_gdi.rs` on Windows, `mac_gui.rs` on the Mac.
#![cfg_attr(not(windows), allow(dead_code))] // the Mac window doesn't use every pose, palette or sound yet

#[cfg(windows)]
#[path = "brand_gdi.rs"]
mod gdi;
#[cfg(windows)]
pub use gdi::*;

// tokens/rowmi.json colours, as COLORREF (0x00BBGGRR)
pub const PAPER: u32 = 0xF1EEEC;      // #ECEEF1
pub const BLACK: u32 = 0x1D1816;      // #16181D Rommel Black
pub const TAN: u32 = 0x3B89D9;        // #D9893B Rommel Tan
pub const GREY: u32 = 0xD6CEC9;       // #C9CED6 Cartridge Grey
pub const GRID: u32 = 0xE6E1DD;       // #DDE1E6 grid-paper lines (brand book --grid)
pub const SUNKEN: u32 = 0xF7F5F4;     // #F4F5F7 sunken surface / Rommel's eyes
pub const BLUE: u32 = 0xE06F2F;       // #2F6FE0 Cursor Blue: selection and focus only
pub const PINK: u32 = 0x6B7AE0;       // #E07A6B Tongue Pink
pub const MINT: u32 = 0xA1C72F;       // #2FC7A1 the Roam shell (edgehop's in the package)
pub const MUTED: u32 = 0x70625B;      // #5B6270 muted text (brand book --muted), captions
pub const DANGER: u32 = 0x2F35C0;     // #C0352F

// ---------- Rommel, 10x10 units, exactly as rommel/*.svg ----------

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Face { Idle, Alert, Happy, Sad, Sleepy, Wink, Grin }

pub struct Pal { pub ink: u32, pub tan: u32, pub eye: u32, pub tongue: u32, pub outline: Option<u32> }
pub const LIGHT: Pal = Pal { ink: BLACK, tan: TAN, eye: SUNKEN, tongue: PINK, outline: None };
pub const DARK: Pal = Pal { ink: BLACK, tan: TAN, eye: SUNKEN, tongue: PINK, outline: Some(PAPER) };

/// Rommel's cells as (x, y, w, h, colour). Mirrors rommel() in the brand book.
pub fn rommel_rects(f: Face, p: &Pal) -> Vec<(i32, i32, i32, i32, u32)> {
    use Face::*;
    let up = matches!(f, Alert | Happy | Grin);
    let down = matches!(f, Sad | Sleepy);
    let cl = matches!(f, Happy | Sleepy);
    let cr = cl || f == Wink;
    let tg = matches!(f, Happy | Wink | Grin);
    let mut v = vec![(2, 0, 2, 1, p.ink), (6, 0, 2, 1, p.ink), (0, 1, 10, 3, p.ink), (2, 4, 6, 3, p.ink)];
    if up { v.extend([(0, 0, 1, 1, p.ink), (9, 0, 1, 1, p.ink)]) } else { v.extend([(0, 4, 1, 1, p.ink), (9, 4, 1, 1, p.ink)]) }
    if down { v.extend([(0, 5, 1, 1, p.ink), (9, 5, 1, 1, p.ink)]) }
    v.extend([(3, 2, 1, 1, p.tan), (6, 2, 1, 1, p.tan)]); // the eyebrows
    v.extend([(3, 4, 1, 1, if cl { p.ink } else { p.eye }), (6, 4, 1, 1, if cr { p.ink } else { p.eye })]);
    v.extend([(2, 5, 1, 1, p.tan), (7, 5, 1, 1, p.tan), (3, 6, 4, 1, p.tan), (3, 7, 1, 1, p.tan), (6, 7, 1, 1, p.tan)]);
    v.push((4, 7, 2, 2, p.ink)); // nose
    if tg { v.push((4, 9, 2, 1, p.tongue)) }
    v
}

/// The wordmark, 27x5 units, exactly as logo/wordmark/*.svg.
pub const WORD: [(i32, i32, i32, i32); 22] = [
    (0, 0, 4, 1), (0, 1, 1, 4), (4, 1, 1, 2), (1, 3, 3, 1), (4, 4, 1, 1),            // R
    (7, 0, 3, 1), (6, 1, 1, 3), (10, 1, 1, 3), (7, 4, 3, 1),                         // O
    (12, 0, 1, 4), (16, 0, 1, 4), (14, 2, 1, 2), (13, 4, 1, 1), (15, 4, 1, 1),       // W
    (18, 0, 1, 5), (22, 0, 1, 5), (19, 1, 1, 1), (21, 1, 1, 1), (20, 2, 1, 1),       // M
    (24, 0, 3, 1), (25, 1, 1, 3), (24, 4, 3, 1),                                      // I
];

// ---------- sounds: the package WAVs, embedded ----------

macro_rules! wav { ($n:literal) => { include_bytes!(concat!("../assets/sounds/", $n, ".wav")) }; }
pub const STARTUP: &[u8] = wav!("startup");
pub const CLICK: &[u8] = wav!("click");
pub const CONFIRM: &[u8] = wav!("confirm");
pub const BACK: &[u8] = wav!("back");
pub const ON: &[u8] = wav!("on");
pub const OFF: &[u8] = wav!("off");
pub const ERROR: &[u8] = wav!("error");
pub const CHIME: &[u8] = wav!("chime");
pub const POP: &[u8] = wav!("bubble-pop");
pub const YIP: &[u8] = wav!("yip");
pub const WHINE: &[u8] = wav!("whine");
pub const TAG: &[u8] = wav!("tag");
pub const WOOF: &[u8] = wav!("woof");

/// Plays one embedded WAV without blocking. A new sound replaces one still playing, which suits
/// chips this short.
#[cfg(windows)]
pub fn play(w: &'static [u8]) {
    use windows_sys::Win32::Media::Audio::{PlaySoundW, SND_ASYNC, SND_MEMORY, SND_NODEFAULT};
    unsafe { PlaySoundW(w.as_ptr() as *const u16, std::ptr::null_mut(), SND_MEMORY | SND_ASYNC | SND_NODEFAULT) };
}

/// shortcut: afplay adds a few tens of ms and sounds can overlap, move to AVAudioPlayer if that shows.
#[cfg(target_os = "macos")]
pub fn play(w: &'static [u8]) {
    let file = std::env::temp_dir().join(format!("roam-{}-{}.wav", w.as_ptr() as usize, w.len()));
    if file.exists() || std::fs::write(&file, w).is_ok() {
        let _ = std::process::Command::new("/usr/bin/afplay").arg(file).spawn();
    }
}
