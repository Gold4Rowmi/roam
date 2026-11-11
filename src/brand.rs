//! ROWMI brand, from the brand package v1.0: tokens, Rommel's seven poses and the wordmark as exact
//! pixel data (logo/ and rommel/), and the real WAVs (sounds/) embedded in the app.
//! Drawing them is per platform: `brand_gdi.rs` on Windows, `mac_gui.rs` on the Mac.
#![cfg_attr(not(windows), allow(dead_code))] // the Mac window doesn't use every pose, palette or sound yet

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering::Relaxed};
use std::sync::OnceLock;

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
pub static STARTUP: &[u8] = wav!("startup");
pub static CLICK: &[u8] = wav!("click");
pub static CONFIRM: &[u8] = wav!("confirm");
pub static BACK: &[u8] = wav!("back");
pub static ON: &[u8] = wav!("on");
pub static OFF: &[u8] = wav!("off");
pub static ERROR: &[u8] = wav!("error");
pub static CHIME: &[u8] = wav!("chime");
pub static POP: &[u8] = wav!("bubble-pop");
pub static YIP: &[u8] = wav!("yip");
pub static WHINE: &[u8] = wav!("whine");
pub static TAG: &[u8] = wav!("tag");
pub static WOOF: &[u8] = wav!("woof");

// ---------- sound settings: Sound (on), Volume (11 detents, 6 by default) and Dog sounds, as the brand's Settings ----------

static SOUND_ON: AtomicBool = AtomicBool::new(true);
static VOLUME: AtomicU8 = AtomicU8::new(6);
static DOGS: AtomicBool = AtomicBool::new(true);

fn sound_file() -> std::path::PathBuf { crate::trust::dir().join("sound.txt") }

/// (sound on, volume 0..=10, dog sounds on)
pub fn sound() -> (bool, u8, bool) { (SOUND_ON.load(Relaxed), VOLUME.load(Relaxed), DOGS.load(Relaxed)) }

/// "on volume dogs", as "1 6 1"; None if the file is damaged, so the defaults stay.
fn parse_sound(s: &str) -> Option<(bool, u8, bool)> {
    let v: Vec<u8> = s.split_whitespace().map(|x| x.parse().ok()).collect::<Option<_>>()?;
    match v[..] { [on, vol, dogs] if on < 2 && vol <= 10 && dogs < 2 => Some((on == 1, vol, dogs == 1)), _ => None }
}

pub fn load_sound() {
    let (on, vol, dogs) = std::fs::read_to_string(sound_file()).ok().and_then(|s| parse_sound(&s)).unwrap_or((true, 6, true));
    store_sound(on, vol, dogs);
}

/// Applies at once and is remembered; there is no Save button.
pub fn set_sound(on: bool, vol: u8, dogs: bool) {
    store_sound(on, vol, dogs);
    let _ = std::fs::create_dir_all(crate::trust::dir());
    let _ = std::fs::write(sound_file(), format!("{} {vol} {}", on as u8, dogs as u8)); // fine if it can't be saved
}

fn store_sound(on: bool, vol: u8, dogs: bool) {
    SOUND_ON.store(on, Relaxed);
    VOLUME.store(vol.min(10), Relaxed);
    DOGS.store(dogs, Relaxed);
    #[cfg(windows)]
    unsafe { // this app's own volume in the Windows mixer, which PlaySound follows
        let v = vol.min(10) as u32 * 0xFFFF / 10;
        windows_sys::Win32::Media::Audio::waveOutSetVolume(std::ptr::null_mut(), v | v << 16);
    }
}

/// Dog sounds are seasoning, so they have their own switch: woof, yip, whine and the tag jingle.
fn audible(w: &[u8]) -> bool {
    let (on, vol, dogs) = sound();
    on && vol > 0 && (dogs || ![WOOF, YIP, WHINE, TAG].iter().any(|d| std::ptr::eq(d.as_ptr(), w.as_ptr())))
}

/// The volume's detents: 16 ms square blips rising 90 Hz a step from 1500 Hz, as the UI guidelines' engine.
pub fn detent(n: u8) -> &'static [u8] {
    static TONES: OnceLock<Vec<Vec<u8>>> = OnceLock::new();
    &TONES.get_or_init(|| (0..=10).map(|i| blip(1500.0 + i as f32 * 90.0)).collect())[n.min(10) as usize]
}

fn blip(freq: f32) -> Vec<u8> {
    const RATE: u32 = 44_100;
    let n = RATE as usize * 16 / 1000;
    let samples: Vec<i16> = (0..n).map(|i| {
        let t = i as f32 / RATE as f32;
        let env = (t / 0.004).min(1.0) * (-(t / 0.016) * 6.0).exp(); // 4 ms in, then a quick fall
        let wave = if (t * freq).fract() < 0.5 { 1.0 } else { -1.0 };
        (wave * env * 6000.0) as i16 // well under the package chips, which peak near 18000
    }).collect();
    let data = (n * 2) as u32;
    let mut w = Vec::with_capacity(44 + n * 2);
    w.extend(b"RIFF"); w.extend((36 + data).to_le_bytes()); w.extend(b"WAVEfmt ");
    w.extend(16u32.to_le_bytes()); w.extend(1u16.to_le_bytes()); w.extend(1u16.to_le_bytes()); // PCM, mono
    w.extend(RATE.to_le_bytes()); w.extend((RATE * 2).to_le_bytes()); w.extend(2u16.to_le_bytes()); w.extend(16u16.to_le_bytes());
    w.extend(b"data"); w.extend(data.to_le_bytes());
    for x in samples { w.extend(x.to_le_bytes()) }
    w
}

/// Plays one embedded WAV without blocking. A new sound replaces one still playing, which suits
/// chips this short.
#[cfg(windows)]
pub fn play(w: &'static [u8]) {
    if !audible(w) { return }
    use windows_sys::Win32::Media::Audio::{PlaySoundW, SND_ASYNC, SND_MEMORY, SND_NODEFAULT};
    unsafe { PlaySoundW(w.as_ptr() as *const u16, std::ptr::null_mut(), SND_MEMORY | SND_ASYNC | SND_NODEFAULT) };
}

/// shortcut: afplay adds a few tens of ms and sounds can overlap, move to AVAudioPlayer if that shows.
#[cfg(target_os = "macos")]
pub fn play(w: &'static [u8]) {
    if !audible(w) { return }
    let file = std::env::temp_dir().join(format!("roam-{}-{}.wav", w.as_ptr() as usize, w.len()));
    if file.exists() || std::fs::write(&file, w).is_ok() {
        let _ = std::process::Command::new("/usr/bin/afplay").arg("-v").arg((VOLUME.load(Relaxed) as f32 / 10.0).to_string()).arg(file).spawn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sound_settings_parse_and_refuse_damage() {
        assert_eq!(parse_sound("1 6 1"), Some((true, 6, true)));
        assert_eq!(parse_sound("0 0 0\n"), Some((false, 0, false)));
        for bad in ["", "1 6", "1 11 1", "2 6 1", "1 -1 1", "1 6 1 1", "on 6 1"] { assert_eq!(parse_sound(bad), None, "{bad}") }
    }

    #[test]
    fn detents_are_valid_wavs_rising_in_pitch() {
        let (a, b) = (detent(0), detent(10));
        assert_eq!(&a[..4], b"RIFF");
        assert_eq!(u32::from_le_bytes(a[4..8].try_into().unwrap()) as usize, a.len() - 8);
        assert_eq!(a.len(), 44 + 705 * 2);
        assert_ne!(a, b);
        assert!(std::ptr::eq(detent(3), detent(3))); // made once, so async playback never outlives its buffer
    }

    #[test]
    fn dog_sounds_have_their_own_switch() {
        store_sound(true, 6, false);
        assert!(!audible(WOOF) && !audible(TAG) && audible(CLICK));
        store_sound(true, 0, true);
        assert!(!audible(CLICK));
        store_sound(false, 6, true);
        assert!(!audible(CLICK));
        store_sound(true, 6, true);
        assert!(audible(WOOF));
    }
}
