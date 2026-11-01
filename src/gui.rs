//! The ROWMI Roam window: boot screen, Mint title band with Rommel, pairing code, your real
//! monitor layout with a draggable "Other PC" box, two buttons, a status line, and a tray icon.
//! Plain Win32 drawn by hand to the brand: Paper ground, 1 px frame + 2 px drop, two typefaces.
use crate::brand::{self, Face, BLACK, BLUE, DANGER, GREY, LIGHT, MINT, MUTED, PAPER, SUNKEN, TAN};
use crate::{net, trust, update, win};
use chacha20poly1305::aead::{rand_core::RngCore, OsRng};
use std::mem::{size_of, zeroed};
use std::path::{Path, PathBuf};
use std::ptr::{null, null_mut};
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicUsize, Ordering::Relaxed};
use std::sync::Mutex;
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::UI::HiDpi::*;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{EnableWindow, GetFocus, ReleaseCapture, SetCapture, SetFocus};
use windows_sys::Win32::UI::Shell::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

const APP: &str = "Roam";
const TITLE: &str = "ROWMI Roam";

const ID_CODE: isize = 1;
const ID_HOST: isize = 2;
const ID_CLIENT: isize = 3;
const ID_STATUS: isize = 4;
const ID_STOP: isize = 5;
const ID_UPDATE: isize = 7;
const ID_PEER: isize = 8; // 8, 9 and 10: one button per trusted PC found
const MAX_PEERS: usize = 3; // shortcut: the 3 first alphabetically show, raise it when the window can take more rows
const STYLE: u32 = WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX;
const HEIGHT: i32 = 530;
const ID_BACK: isize = 20; // the About screen's button
const SC_ABOUT: usize = 0x10; // our item in the window menu; system commands use the top bits
const TAGLINE: &str = "Playful tools with a lot of bark.";
const DEDICATION: &str = "Made in memory of Rommel.";
const ROW: i32 = 52; // the window grows by this for each extra button row: 44 tall plus an 8 gap
const SETUP_HELP: &str = "Drag the mint box next to the monitor edge the other PC sits beside, then pick what this PC does.";
const EM_SETREADONLY: u32 = 0xCF;
const BS_OWNERDRAW: u32 = 0x0B;
const WM_DRAWITEM: u32 = 0x2B;
const ODS_SELECTED: u32 = 0x01;
const ODS_DISABLED: u32 = 0x04;
const ODS_FOCUS: u32 = 0x10;
const SPI_GETCLIENTAREAANIMATION: u32 = 0x1042;
const ALPHABET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789"; // no 0/O/1/I lookalikes

#[repr(C)]
struct DrawItem { ctl_type: u32, ctl_id: u32, item_id: u32, action: u32, state: u32, hwnd: HWND, hdc: HDC, rc: RECT, data: usize }

const WM_TRAY: u32 = WM_APP + 1;
const WM_REFRESH: u32 = WM_APP + 2; // worker threads post this; the UI thread applies PENDING
const WM_CLOSE_NOW: u32 = WM_APP + 3; // the updater started the new exe; this one goes
const WM_ARRIVED: u32 = WM_APP + 4;   // the cursor landed here at (wparam, lparam)
const WM_PEERS: u32 = WM_APP + 5;     // the finder thread changed PEERS
const WM_HEEL: u32 = WM_APP + 6;      // the other PC said Bye during a connection
// The brand's pop sparkle: eight rust squares burst out of the middle in the eight directions,
// 4 steps of 8 or 16 px each over .42 s (rowmi-ui-guidelines.html, .spark and sparks()).
const SPARK_PX: i32 = 8;              // each spark is an 8 px square
const SPARK_REACH: i32 = 52;          // 3 steps of 16 px at the farthest, plus half a spark
const SPARK_FRAMES: u8 = 4;
const SPARK_TICK: u32 = 105;          // .42 s in 4 steps
const SPARK_DIRS: [(i32, i32); 8] = [(0, -1), (1, -1), (1, 0), (1, 1), (0, 1), (-1, 1), (-1, 0), (-1, -1)];
const TIMER_SPARK: usize = 5;
// The brand's bonk: 300 ms in 5 steps, left 8, right 8, left 5, right 3, rest (.bonk keyframes).
const BONK_TICK: u32 = 60;
const TIMER_BONK: usize = 6;
const BONK: [i32; 5] = [-8, 8, -5, 3, 0]; // sideways px at 96 dpi
const KEY_COLOUR: u32 = 0xFF00FF;     // the spark window's transparent colour
const CODE_HELP: &str = "A pairing code is 10 letters and numbers, like ABCDE-FGHJK.";
const EN_CHANGE: usize = 0x0300;
const EN_SETFOCUS: usize = 0x0100;
const EN_KILLFOCUS: usize = 0x0200;
const TIMER_BOOT: usize = 1;
const TIMER_FACE: usize = 2;
const TIMER_BAR: usize = 3;
const TIMER_EGG: usize = 4;
const BOOT_TICK: u32 = 55; // ms; the brand timeline is in multiples of 110
const MENU_SHOW: usize = 10;
const MENU_STOP: usize = 11;
const MENU_QUIT: usize = 12;
const MENU_ABOUT: usize = 13;
/// Tray states: 0 not connected, 1 mouse is on this PC, 2 mouse is on the other PC
pub const TRAY_OFF: u8 = 0;
pub const TRAY_HERE: u8 = 1;
pub const TRAY_AWAY: u8 = 2;

static MAIN: AtomicUsize = AtomicUsize::new(0);
static DPI: AtomicUsize = AtomicUsize::new(96);
static PAPER_BRUSH: AtomicUsize = AtomicUsize::new(0);
static PIXEL16: AtomicUsize = AtomicUsize::new(0); // Silkscreen 16, or null -> built-in face
static PIXEL24: AtomicUsize = AtomicUsize::new(0);
static PIXEL32: AtomicUsize = AtomicUsize::new(0);
static PLEX: [AtomicUsize; 4] = [AtomicUsize::new(0), AtomicUsize::new(0), AtomicUsize::new(0), AtomicUsize::new(0)]; // body, caption, code, button
static ABOUT: AtomicUsize = AtomicUsize::new(0); // the About screen while it is open, else 0
static SPARK_WIN: AtomicUsize = AtomicUsize::new(0);
static SPARK_AT: AtomicU8 = AtomicU8::new(0); // spark frame showing, 0..4
static SPARK_FAR: AtomicU8 = AtomicU8::new(0); // bit i: spark i takes 16 px steps, else 8, drawn fresh each burst
static ERR: AtomicBool = AtomicBool::new(false); // the code box is in its error state: red border and help text
static BONK_AT: AtomicU8 = AtomicU8::new(0);  // bonk frame showing, 1..=5, 0 = at rest
static EDIT_PROC: AtomicUsize = AtomicUsize::new(0); // the code box's original window procedure
static STARTED: AtomicBool = AtomicBool::new(false);
static HEELED: AtomicBool = AtomicBool::new(false); // the setup screen says so until the next connection
static BOOT: AtomicU8 = AtomicU8::new(0); // boot tick 1..=20 while booting, 0 when done
static FACE: AtomicU8 = AtomicU8::new(0);
static TRAY_STATE: AtomicU8 = AtomicU8::new(TRAY_OFF);
static OFFLINE: AtomicBool = AtomicBool::new(false); // true only after a connection was lost
static ICONS: [AtomicUsize; 3] = [AtomicUsize::new(0), AtomicUsize::new(0), AtomicUsize::new(0)];
static PENDING: Mutex<(Option<String>, Option<u8>, Option<bool>, bool)> = Mutex::new((None, None, None, false));
static MONITORS: Mutex<Vec<RECT>> = Mutex::new(Vec::new()); // physical pixels, primary first
static DRAG: Mutex<Option<((i32, i32), (i32, i32))>> = Mutex::new(None); // box top-left + grab offset
static PEERS: Mutex<Vec<net::Found>> = Mutex::new(Vec::new());  // trusted PCs heard right now, from the finder thread
static SHOWN: Mutex<Vec<net::Found>> = Mutex::new(Vec::new());  // what the buttons currently show, to spot new arrivals
static FINDER: AtomicUsize = AtomicUsize::new(0);                // generation: bumping it stops the running finder
static BAR: AtomicUsize = AtomicUsize::new(0);  // launch-bar step while searching
static TAPS: AtomicU8 = AtomicU8::new(0);        // taps on Rommel, for the easter egg
static EGG: AtomicU8 = AtomicU8::new(0);         // bubble frame while the egg plays

fn wide(s: &str) -> Vec<u16> { s.encode_utf16().chain([0]).collect() }
fn px(v: i32) -> i32 { v * DPI.load(Relaxed) as i32 / 96 }
fn prect(x: i32, y: i32, w: i32, h: i32) -> RECT { brand::rect(px(x), px(y), px(w), px(h)) }

const FACES: [Face; 7] = [Face::Idle, Face::Alert, Face::Happy, Face::Sad, Face::Sleepy, Face::Wink, Face::Grin];
fn face() -> Face { FACES[FACE.load(Relaxed) as usize] }
fn set_face(f: Face) { FACE.store(FACES.iter().position(|x| *x == f).unwrap() as u8, Relaxed) }
/// Idle is the logo and the default. Sleepy is only for a real offline state: a link that was up and dropped.
fn settled_face() -> Face { if OFFLINE.load(Relaxed) { Face::Sleepy } else { Face::Idle } }

/// Rommel reacts for `ms`, then the timer settles him on the face the tray state implies.
unsafe fn react(hwnd: HWND, now: Face, ms: u32) {
    set_face(now);
    SetTimer(hwnd, TIMER_FACE, ms, None);
    InvalidateRect(hwnd, &prect(0, 0, 360, 64), 0);
}

/// Pixel type: Silkscreen at 16, 24 or 32 px, else the built-in 5x5 face at 2, 3 or 4 units.
unsafe fn pixel_text(hdc: HDC, r: &RECT, size: i32, s: &str, c: u32, center: bool) {
    let f = match size { 32 => &PIXEL32, 24 => &PIXEL24, _ => &PIXEL16 }.load(Relaxed) as HFONT;
    if f.is_null() {
        let u = px(size / 8);
        let (w, h) = (brand::text_w(s) * u, 5 * u);
        let (x, y) = if center { ((r.left + r.right - w) / 2, (r.top + r.bottom - h) / 2) } else { (r.left, r.top) };
        brand::text(hdc, x, y, u, s, c);
        return;
    }
    let old = SelectObject(hdc, f as _);
    SetBkMode(hdc, 1);
    SetTextColor(hdc, c);
    let mut rr = *r;
    if !center { // cap tops on r.top, like the built-in face: Silkscreen capitals are 5 of the 8 units in its em
        let mut m: TEXTMETRICW = zeroed();
        GetTextMetricsW(hdc, &mut m);
        rr.top += 5 * px(size / 8) - m.tmAscent;
    }
    DrawTextW(hdc, wide(s).as_ptr(), -1, &mut rr, DT_SINGLELINE | DT_NOPREFIX | DT_NOCLIP | if center { DT_CENTER | DT_VCENTER } else { DT_LEFT | DT_TOP });
    SelectObject(hdc, old);
}

// ---------- thread-safe reporting from the worker threads ----------

pub fn status(s: &str) {
    PENDING.lock().unwrap().0 = Some(s.to_string());
    unsafe { PostMessageW(MAIN.load(Relaxed) as HWND, WM_REFRESH, 0, 0) };
}

pub fn tray(state: u8) {
    PENDING.lock().unwrap().1 = Some(state);
    unsafe { PostMessageW(MAIN.load(Relaxed) as HWND, WM_REFRESH, 0, 0) };
}

/// Hide the window into the tray (false) or bring it back (true).
pub fn visible(on: bool) {
    PENDING.lock().unwrap().2 = Some(on);
    unsafe { PostMessageW(MAIN.load(Relaxed) as HWND, WM_REFRESH, 0, 0) };
}

/// Show the Update button, or give it back after a failed attempt.
pub fn offer() {
    PENDING.lock().unwrap().3 = true;
    unsafe { PostMessageW(MAIN.load(Relaxed) as HWND, WM_REFRESH, 0, 0) };
}

pub fn close() {
    unsafe { PostMessageW(MAIN.load(Relaxed) as HWND, WM_CLOSE_NOW, 0, 0) };
}

/// Heel this PC because the other one did. Safe from any thread; a dropped link is not this, only a Bye.
pub fn heel() {
    unsafe { PostMessageW(MAIN.load(Relaxed) as HWND, WM_HEEL, 0, 0) };
}

/// The mouse just landed on this PC at (x, y). Safe from any thread.
pub fn arrived(x: i32, y: i32) {
    unsafe { PostMessageW(MAIN.load(Relaxed) as HWND, WM_ARRIVED, x as isize as usize, y as isize) };
}

/// True even while the main window sits in the tray, unlike IsWindowVisible.
unsafe fn flagged_visible(hwnd: HWND, id: isize) -> bool {
    GetWindowLongW(GetDlgItem(hwnd, id as i32), GWL_STYLE) as u32 & WS_VISIBLE != 0
}

/// Sizes the window and every control for the current DPI. Runs at launch, when the window
/// crosses to a monitor with another scale, and when a found-PC or Update button appears or goes.
unsafe fn layout(hwnd: HWND) {
    let peers = (0..MAX_PEERS as isize).filter(|i| flagged_visible(hwnd, ID_PEER + i)).count() as i32;
    let grew = (peers + flagged_visible(hwnd, ID_UPDATE) as i32) * ROW;
    let mut r = RECT { left: 0, top: 0, right: px(360), bottom: px(HEIGHT + grew) };
    AdjustWindowRectExForDpi(&mut r, STYLE, 0, 0, DPI.load(Relaxed) as u32);
    SetWindowPos(hwnd, null_mut(), 0, 0, r.right - r.left, r.bottom - r.top, SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE);
    let mut place = |id: isize, y: i32, h: i32| { MoveWindow(GetDlgItem(hwnd, id as i32), px(16), px(y), px(328), px(h), 1); };
    for i in 0..MAX_PEERS as i32 { place(ID_PEER + i as isize, 468 + i * ROW, 44) }
    place(ID_HOST, 364, 44);
    place(ID_CLIENT, 416, 44);
    place(ID_STOP, 364, 44);
    place(ID_UPDATE, 468 + peers * ROW, 44);
    place(ID_STATUS, 466 + grew, 56);
    // an EDIT paints ~2 px past its own edge: keep it well inside the frame
    place_code(hwnd);
}

/// Type for the current DPI: Silkscreen 16/24/32 and IBM Plex Mono 400/600, with fallbacks.
unsafe fn load_type(hwnd: HWND) {
    for (slot, size) in [(&PIXEL16, 16), (&PIXEL24, 24), (&PIXEL32, 32)] {
        DeleteObject(slot.swap(brand::font("Silkscreen", px(size), 400, true) as usize, Relaxed) as _);
    }
    let plex = |size: i32, weight: i32| {
        // Windows sees the 600 weight as its own family
        let f = brand::font(if weight == 600 { "IBM Plex Mono SemiBold" } else { "IBM Plex Mono" }, px(size), weight, false);
        if f.is_null() { brand::font("Consolas", px(size), weight, false) } else { f }
    };
    let old = [(plex(14, 400), &PLEX[0]), (plex(12, 400), &PLEX[1]), (plex(20, 600), &PLEX[2]), (plex(15, 400), &PLEX[3])].map(|(f, slot)| slot.swap(f as usize, Relaxed));
    for (font, ids) in [(0, &[ID_HOST, ID_CLIENT, ID_STOP, ID_STATUS][..]), (2, &[ID_CODE])] {
        for id in ids { SendMessageW(GetDlgItem(hwnd, *id as i32), WM_SETFONT, PLEX[font].load(Relaxed), 1); }
    }
    for f in old { DeleteObject(f as _); } // only now, when no control still holds them
}

fn spark_size() -> i32 { px(2 * SPARK_REACH) }

unsafe extern "system" fn spark_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    if msg != WM_PAINT { return DefWindowProcW(hwnd, msg, wp, lp) }
    let mut ps: PAINTSTRUCT = zeroed();
    let hdc = BeginPaint(hwnd, &mut ps);
    let (frame, far, size) = (SPARK_AT.load(Relaxed) as i32, SPARK_FAR.load(Relaxed), spark_size());
    brand::fill(hdc, &brand::rect(0, 0, size, size), KEY_COLOUR);
    for (i, (dx, dy)) in SPARK_DIRS.iter().enumerate() {
        let off = frame * if far >> i & 1 == 1 { 16 } else { 8 }; // steps of 8 or 16 px, as in the book
        let half = px(SPARK_PX) / 2;
        brand::fill(hdc, &brand::rect(size / 2 + px(dx * off) - half, size / 2 + px(dy * off) - half, px(SPARK_PX), px(SPARK_PX)), TAN);
    }
    EndPaint(hwnd, &ps);
    0
}

/// Click-through and never takes focus, so it can't get in the way of what the mouse does next.
unsafe fn make_spark() {
    let cls = wide("ROWMI.roam.spark");
    let wc = WNDCLASSW {
        style: 0, lpfnWndProc: Some(spark_proc), cbClsExtra: 0, cbWndExtra: 0, hInstance: null_mut(), hIcon: null_mut(),
        hCursor: null_mut(), hbrBackground: null_mut(), lpszMenuName: null(), lpszClassName: cls.as_ptr(),
    };
    RegisterClassW(&wc);
    let t = CreateWindowExW(WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_TRANSPARENT | WS_EX_LAYERED,
        cls.as_ptr(), null(), WS_POPUP, 0, 0, 0, 0, null_mut(), null_mut(), null_mut(), null());
    SetLayeredWindowAttributes(t, KEY_COLOUR, 0, LWA_COLORKEY);
    SPARK_WIN.store(t as usize, Relaxed);
}

/// The sparkle bursts at the cursor, on the PC the mouse just arrived on (and came back to).
/// The cursor lands 2 px inside a screen edge, so the middle is pulled in far enough for the whole burst to show.
unsafe fn show_spark(hwnd: HWND, x: i32, y: i32) {
    let (t, size) = (SPARK_WIN.load(Relaxed) as HWND, spark_size());
    let mut m: MONITORINFO = zeroed();
    m.cbSize = size_of::<MONITORINFO>() as u32;
    GetMonitorInfoW(MonitorFromPoint(POINT { x, y }, MONITOR_DEFAULTTONEAREST), &mut m);
    let r = m.rcMonitor;
    let (cx, cy) = (x.clamp(r.left + size / 2, r.right - size / 2), y.clamp(r.top + size / 2, r.bottom - size / 2));
    let mut far = [0u8; 1];
    OsRng.fill_bytes(&mut far);
    SPARK_FAR.store(far[0], Relaxed);
    SPARK_AT.store(0, Relaxed);
    SetWindowPos(t, HWND_TOPMOST, cx - size / 2, cy - size / 2, size, size, SWP_NOACTIVATE | SWP_SHOWWINDOW);
    InvalidateRect(t, null(), 0);
    SetTimer(hwnd, TIMER_SPARK, SPARK_TICK, None);
}

/// Bonk, for a wrong input: the field shakes in 5 steps, with the error sound, and stays in its error
/// state (red border, red help text) until it is edited, as in the book.
unsafe fn bonk(hwnd: HWND) {
    brand::play(brand::ERROR);
    ERR.store(true, Relaxed);
    InvalidateRect(GetDlgItem(hwnd, ID_STATUS as i32), null(), 1);
    BONK_AT.store(1, Relaxed);
    place_code(hwnd);
    SetTimer(hwnd, TIMER_BONK, BONK_TICK, None);
}

fn code_dx() -> i32 { BONK_AT.load(Relaxed).checked_sub(1).map_or(0, |i| BONK[i as usize]) }

/// The whole field (label, frame, box) shifted by the bonk step (0 at rest).
unsafe fn place_code(hwnd: HWND) {
    MoveWindow(GetDlgItem(hwnd, ID_CODE as i32), px(20 + code_dx()), px(104), px(320), px(28), 1);
    InvalidateRect(hwnd, &prect(0, 76, 360, 68), 0);
}

/// The code box only takes what a code can hold; anything else bonks instead of being typed.
unsafe extern "system" fn code_proc(h: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    if msg == WM_CHAR && wp >= 32 && wp != 127 {
        let c = char::from_u32(wp as u32).unwrap_or('?');
        let mut buf = [0u16; 64];
        let n = GetWindowTextW(h, buf.as_mut_ptr(), 64) as usize;
        let have = String::from_utf16_lossy(&buf[..n]).chars().filter(char::is_ascii_alphanumeric).count();
        let sel = SendMessageW(h, 0xB0 /*EM_GETSEL*/, 0, 0) as u32;
        let full = c.is_ascii_alphanumeric() && have >= 10 && (sel & 0xFFFF) == (sel >> 16);
        if !(c.is_ascii_alphanumeric() || c == '-') || full {
            bonk(MAIN.load(Relaxed) as HWND);
            status(CODE_HELP);
            return 0;
        }
    }
    CallWindowProcW(std::mem::transmute::<usize, WNDPROC>(EDIT_PROC.load(Relaxed)), h, msg, wp, lp)
}

/// Plex Mono text; `font` indexes PLEX.
unsafe fn plex_text(hdc: HDC, r: &RECT, font: usize, s: &str, c: u32, center: bool) {
    let old = SelectObject(hdc, PLEX[font].load(Relaxed) as _);
    SetBkMode(hdc, 1);
    SetTextColor(hdc, c);
    let mut rr = *r;
    DrawTextW(hdc, wide(s).as_ptr(), -1, &mut rr, DT_SINGLELINE | DT_NOPREFIX | if center { DT_CENTER | DT_VCENTER } else { DT_LEFT | DT_TOP });
    SelectObject(hdc, old);
}

/// The only place the version, tagline and dedication appear, per the brand. Version at the top and
/// the other two at the foot, because the dedication never sits next to a version number.
unsafe extern "system" fn about_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_PAINT => {
            let mut ps: PAINTSTRUCT = zeroed();
            let hdc = BeginPaint(hwnd, &mut ps);
            let mut c: RECT = zeroed();
            GetClientRect(hwnd, &mut c);
            brand::fill(hdc, &c, PAPER);
            title_band(hdc, Face::Idle);
            plex_text(hdc, &prect(16, 84, 328, 20), 0, &format!("Version {}", update::VERSION), BLACK, false);
            plex_text(hdc, &brand::rect(px(16), c.bottom - px(112), px(328), px(20)), 0, TAGLINE, BLACK, false);
            plex_text(hdc, &brand::rect(px(16), c.bottom - px(88), px(328), px(16)), 1, DEDICATION, MUTED, false);
            EndPaint(hwnd, &ps);
            0
        }
        WM_DRAWITEM => { draw_button(&*(lp as *const DrawItem)); 1 }
        WM_COMMAND | WM_CLOSE => { close_about(); 0 } // Back, Esc or Alt+F4
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}

/// Covers the main window, which stays disabled meanwhile, so it can't be dragged to another scale.
unsafe fn open_about(hwnd: HWND) {
    if ABOUT.load(Relaxed) != 0 { return }
    brand::play(brand::CLICK);
    let mut c: RECT = zeroed();
    GetClientRect(hwnd, &mut c);
    let mut p = POINT { x: 0, y: 0 };
    ClientToScreen(hwnd, &mut p);
    let a = CreateWindowExW(0, wide("ROWMI.roam.about").as_ptr(), wide(TITLE).as_ptr(), WS_POPUP | WS_VISIBLE,
        p.x, p.y, c.right, c.bottom, hwnd, null_mut(), null_mut(), null());
    let back = CreateWindowExW(0, wide("BUTTON").as_ptr(), wide("Back").as_ptr(), WS_CHILD | WS_VISIBLE | WS_TABSTOP | BS_OWNERDRAW,
        px(16), c.bottom - px(60), px(328), px(44), a, ID_BACK as HMENU, null_mut(), null());
    ABOUT.store(a as usize, Relaxed);
    EnableWindow(hwnd, 0);
    SetFocus(back);
}

unsafe fn close_about() {
    let a = ABOUT.swap(0, Relaxed) as HWND;
    if a.is_null() { return }
    brand::play(brand::BACK);
    let hwnd = MAIN.load(Relaxed) as HWND;
    EnableWindow(hwnd, 1); // before the destroy, or Windows activates some other app
    DestroyWindow(a);
    SetForegroundWindow(hwnd);
}

/// First offer of this run: make room for the button, then Rommel, the jingle and a tray balloon.
unsafe fn announce(hwnd: HWND) {
    ShowWindow(GetDlgItem(hwnd, ID_UPDATE as i32), SW_SHOW);
    layout(hwnd);
    InvalidateRect(hwnd, null(), 0);
    brand::play(brand::TAG);
    react(hwnd, Face::Alert, 800);
    if STARTED.load(Relaxed) {
        let mut d = nid(NIF_INFO);
        d.dwInfoFlags = NIIF_NOSOUND; // the tag jingle has already played
        let (title, info) = (wide(TITLE), wide("A new Roam is ready. Click Rommel in the tray, then press Update."));
        d.szInfoTitle[..title.len()].copy_from_slice(&title);
        d.szInfo[..info.len()].copy_from_slice(&info);
        Shell_NotifyIconW(NIM_MODIFY, &d);
    }
}

/// Finds the PCs this one has connected to before while it sits on the setup screen, and shows a
/// button for each. A later call replaces the running finder (after Forget); starting a role stops it.
fn start_finder() {
    let gen = FINDER.fetch_add(1, Relaxed) + 1;
    let mut codes: Vec<String> = trust::load().into_iter().map(|(code, _)| code).collect();
    // the code saved by the last pairing counts too, so PCs paired before trust was recorded are found
    if let Some(saved) = saved_code() { codes.push(saved) }
    if codes.is_empty() { return } // nothing is sent until there is someone to look for
    codes.sort();
    codes.dedup();
    std::thread::spawn(move || {
        let Some(sock) = net::bind_lan() else { return };
        let _ = sock.set_broadcast(true);
        net::find(sock, win::BROADCAST, &win::name(), &codes, || FINDER.load(Relaxed) != gen, |list| {
            if FINDER.load(Relaxed) != gen { return }
            *PEERS.lock().unwrap() = list.to_vec();
            unsafe { PostMessageW(MAIN.load(Relaxed) as HWND, WM_PEERS, 0, 0) };
        });
    });
}

/// One button per found PC. "Allow" when its host is already looking for us, else "Roam onto".
unsafe fn show_peers(hwnd: HWND) {
    let list = PEERS.lock().unwrap().clone();
    let prev = std::mem::replace(&mut *SHOWN.lock().unwrap(), list.clone());
    for i in 0..MAX_PEERS {
        let b = GetDlgItem(hwnd, (ID_PEER + i as isize) as i32);
        match list.get(i) {
            Some(f) => {
                let label = if f.wants { format!("Allow {}", f.name) } else { format!("Roam onto {}", f.name) };
                SetWindowTextW(b, wide(&label).as_ptr());
                ShowWindow(b, SW_SHOW);
                InvalidateRect(b, null(), 0);
            }
            None => { ShowWindow(b, SW_HIDE); }
        }
    }
    layout(hwnd);
    InvalidateRect(hwnd, null(), 0);
    if list.iter().any(|f| !prev.contains(f)) { // someone new, or someone now asking: something arrived
        brand::play(brand::TAG);
        react(hwnd, Face::Alert, 800);
    }
    let text = match list.iter().find(|f| f.wants) {
        Some(f) => format!("{} wants to use this PC's mouse and keyboard. Press Allow, or ignore it.", f.name),
        None if list.is_empty() => SETUP_HELP.to_string(),
        None => "Found a trusted PC. Press its button, or right-click it to forget it.".to_string(),
    };
    status(&if HEELED.load(Relaxed) { format!("Heeled. {text}") } else { text });
}

/// Right-click, or the menu key, on a found PC: Forget it, so it is no longer found or trusted.
unsafe fn forget_menu(hwnd: HWND, i: usize, from_keyboard: bool) {
    let Some(f) = PEERS.lock().unwrap().get(i).cloned() else { return };
    let m = CreatePopupMenu();
    AppendMenuW(m, 0, 1, wide(&format!("Forget {}", f.name)).as_ptr());
    let mut p: POINT = zeroed();
    if from_keyboard { // no mouse: open it under the button
        let mut r: RECT = zeroed();
        GetWindowRect(GetDlgItem(hwnd, (ID_PEER + i as isize) as i32), &mut r);
        (p.x, p.y) = (r.left + px(16), r.bottom);
    } else {
        GetCursorPos(&mut p);
    }
    SetForegroundWindow(hwnd); // required or the menu won't close on click-away
    let pick = TrackPopupMenu(m, TPM_RETURNCMD | TPM_RIGHTBUTTON, p.x, p.y, 0, hwnd, null());
    DestroyMenu(m);
    if pick == 1 {
        trust::forget(&f.name);
        forget_saved_code(&f.code);
        if saved_code().is_none() { SetWindowTextW(GetDlgItem(hwnd, ID_CODE as i32), wide("").as_ptr()); }
        PEERS.lock().unwrap().retain(|x| x.name != f.name);
        start_finder();
        show_peers(hwnd);
        brand::play(brand::OFF);
    }
}

unsafe fn refresh(hwnd: HWND) {
    let (text, state, vis, offered) = std::mem::take(&mut *PENDING.lock().unwrap());
    if offered {
        let b = GetDlgItem(hwnd, ID_UPDATE as i32);
        EnableWindow(b, 1);
        if !flagged_visible(hwnd, ID_UPDATE) { announce(hwnd) }
        InvalidateRect(b, null(), 0);
    }
    match vis {
        Some(true) => show(hwnd),
        Some(false) => { ShowWindow(hwnd, SW_HIDE); }
        None => {}
    }
    if let Some(t) = text {
        SetDlgItemTextW(hwnd, ID_STATUS as i32, wide(&t).as_ptr());
        if STARTED.load(Relaxed) {
            let mut d = nid(NIF_TIP);
            let tip = wide(&format!("{TITLE}: {t}"));
            let n = tip.len().min(127);
            d.szTip[..n].copy_from_slice(&tip[..n]);
            Shell_NotifyIconW(NIM_MODIFY, &d);
        }
    }
    if let Some(s) = state {
        let was = TRAY_STATE.swap(s, Relaxed);
        if s != TRAY_OFF { OFFLINE.store(false, Relaxed) } else if was != TRAY_OFF { OFFLINE.store(true, Relaxed) }
        if STARTED.load(Relaxed) {
            let mut d = nid(NIF_ICON);
            d.hIcon = ICONS[s as usize].load(Relaxed) as HICON;
            Shell_NotifyIconW(NIM_MODIFY, &d);
        }
        if was == TRAY_OFF && s != TRAY_OFF { brand::play(brand::YIP); react(hwnd, Face::Happy, 800) }
        if was != TRAY_OFF && s == TRAY_OFF { brand::play(brand::WHINE); react(hwnd, Face::Sad, 800) }
        InvalidateRect(hwnd, &prect(0, 0, 360, 64), 0);
    }
}

// ---------- tray ----------

unsafe fn nid(flags: u32) -> NOTIFYICONDATAW {
    let mut d: NOTIFYICONDATAW = zeroed();
    d.cbSize = size_of::<NOTIFYICONDATAW>() as u32;
    d.hWnd = MAIN.load(Relaxed) as HWND;
    d.uID = 1;
    d.uFlags = flags;
    d.uCallbackMessage = WM_TRAY;
    d
}

// ---------- the layout panel ----------

const PANEL: (i32, i32, i32, i32) = (16, 168, 328, 180); // x, y, w, h in 96-dpi units
fn panel() -> RECT { prect(PANEL.0, PANEL.1, PANEL.2, PANEL.3) }

unsafe extern "system" fn mon_cb(_: HMONITOR, _: HDC, r: *mut RECT, _: LPARAM) -> BOOL {
    MONITORS.lock().unwrap().push(*r);
    1
}

fn load_monitors() {
    unsafe { EnumDisplayMonitors(null_mut(), null(), Some(mon_cb), 0) };
    let mut m = MONITORS.lock().unwrap();
    m.sort_by_key(|r| (r.left != 0 || r.top != 0, r.left, r.top)); // primary (at 0,0) first
    if m.is_empty() { m.push(RECT { left: 0, top: 0, right: 1920, bottom: 1080 }) }
}

/// Where the "Other PC" would sit, in physical pixels, docked on monitor `i`'s edge `e`.
fn docked(mons: &[RECT], i: usize, e: u8) -> RECT {
    let (m, p) = (mons[i], mons[0]);
    let (w, h) = (p.right - p.left, p.bottom - p.top);
    let (x, y) = match e {
        0 => (m.right, m.top),
        1 => (m.left - w, m.top),
        2 => (m.left, m.top - h),
        _ => (m.left, m.bottom),
    };
    RECT { left: x, top: y, right: x + w, bottom: y + h }
}

/// Maps physical pixels into the panel: all monitors plus room for the other PC on any side.
struct Map { s: f32, ox: f32, oy: f32 }

fn map(mons: &[RECT]) -> Map {
    let p = mons[0];
    let (mw, mh) = (p.right - p.left, p.bottom - p.top);
    let (l, t) = (mons.iter().map(|r| r.left).min().unwrap() - mw, mons.iter().map(|r| r.top).min().unwrap() - mh);
    let (r, b) = (mons.iter().map(|r| r.right).max().unwrap() + mw, mons.iter().map(|r| r.bottom).max().unwrap() + mh);
    let pn = panel();
    let inner = px(8);
    let s = ((pn.right - pn.left - 2 * inner) as f32 / (r - l) as f32).min((pn.bottom - pn.top - 2 * inner) as f32 / (b - t) as f32);
    let ox = pn.left as f32 + ((pn.right - pn.left) as f32 - (r - l) as f32 * s) / 2.0 - l as f32 * s;
    let oy = pn.top as f32 + ((pn.bottom - pn.top) as f32 - (b - t) as f32 * s) / 2.0 - t as f32 * s;
    Map { s, ox, oy }
}

impl Map {
    fn to_panel(&self, r: &RECT) -> RECT {
        let g = px(2); // gap between boxes so the edges read as separate
        RECT {
            left: (r.left as f32 * self.s + self.ox) as i32 + g, top: (r.top as f32 * self.s + self.oy) as i32 + g,
            right: (r.right as f32 * self.s + self.ox) as i32 - g, bottom: (r.bottom as f32 * self.s + self.oy) as i32 - g,
        }
    }
}

fn other_box(mons: &[RECT], m: &Map) -> RECT {
    if let Some((pos, _)) = *DRAG.lock().unwrap() {
        let d = m.to_panel(&docked(mons, 0, 0));
        return RECT { left: pos.0, top: pos.1, right: pos.0 + d.right - d.left, bottom: pos.1 + d.bottom - d.top };
    }
    let (i, e) = win::target();
    m.to_panel(&docked(mons, i.min(mons.len() - 1), e))
}

unsafe fn labelled_box(hdc: HDC, r: &RECT, face: u32, label: &str, dropped: bool) {
    if dropped { brand::frame(hdc, r, face, GREY) } else { brand::rounded(hdc, r, face, GREY, 6) }
    pixel_text(hdc, r, 16, label, BLACK, true);
}

unsafe fn paint_panel(hdc: HDC) {
    let pn = panel();
    brand::frame(hdc, &pn, PAPER, GREY);
    let inner = brand::rect(pn.left + 2, pn.top + 2, pn.right - pn.left - 4, pn.bottom - pn.top - 4);
    brand::grid_paper(hdc, &inner, px(8));
    let mons = MONITORS.lock().unwrap().clone();
    let m = map(&mons);
    for (i, r) in mons.iter().enumerate() {
        labelled_box(hdc, &m.to_panel(r), PAPER, &(i + 1).to_string(), true);
    }
    let dragging = DRAG.lock().unwrap().is_some();
    labelled_box(hdc, &other_box(&mons, &m), MINT, "Other", !dragging); // lifted while dragging: no drop
}

fn lparam_xy(lp: LPARAM) -> (i32, i32) { ((lp & 0xFFFF) as i16 as i32, ((lp >> 16) & 0xFFFF) as i16 as i32) }

unsafe fn drag(hwnd: HWND, msg: u32, lp: LPARAM) {
    let (x, y) = lparam_xy(lp);
    let mons = MONITORS.lock().unwrap().clone();
    let m = map(&mons);
    let pn = panel();
    let b = other_box(&mons, &m); // computed before taking the DRAG lock
    let mut d = DRAG.lock().unwrap();
    match msg {
        WM_LBUTTONDOWN if !STARTED.load(Relaxed) && x >= b.left && x < b.right && y >= b.top && y < b.bottom => {
            *d = Some(((b.left, b.top), (x - b.left, y - b.top)));
            SetCapture(hwnd);
            brand::play(brand::ON);
        }
        WM_MOUSEMOVE => {
            if let Some((pos, off)) = d.as_mut() {
                *pos = ((x - off.0).clamp(pn.left, pn.right - (b.right - b.left)), (y - off.1).clamp(pn.top, pn.bottom - (b.bottom - b.top)));
            }
        }
        WM_LBUTTONUP => {
            if let Some((pos, _)) = d.take() {
                ReleaseCapture();
                // snap to whichever monitor edge the dropped box is closest to
                let (cx, cy) = (pos.0 + (b.right - b.left) / 2, pos.1 + (b.bottom - b.top) / 2);
                let mut best = (i64::MAX, 0usize, 0u8);
                for i in 0..mons.len() {
                    for e in 0..4u8 {
                        let r = m.to_panel(&docked(&mons, i, e));
                        let (dx, dy) = (((r.left + r.right) / 2 - cx) as i64, ((r.top + r.bottom) / 2 - cy) as i64);
                        let dist = dx * dx + dy * dy;
                        if dist < best.0 { best = (dist, i, e) }
                    }
                }
                win::set_target(best.1, best.2, &mons);
                brand::play(brand::CONFIRM);
            }
        }
        _ => {}
    }
    let redraw = d.is_some() || msg != WM_MOUSEMOVE;
    drop(d);
    if redraw { InvalidateRect(hwnd, &pn, 0); }
}

// ---------- easter egg: tap Rommel five times ----------

fn in_rommel(lp: LPARAM) -> bool {
    let (x, y) = lparam_xy(lp);
    let r = prect(16, 17, 30, 30);
    x >= r.left && x < r.right && y >= r.top && y < r.bottom
}

unsafe fn tap_rommel(hwnd: HWND) {
    if TAPS.fetch_add(1, Relaxed) + 1 >= 5 {
        TAPS.store(0, Relaxed);
        brand::play(brand::POP);
        EGG.store(1, Relaxed);
        set_face(Face::Grin);
        SetTimer(hwnd, TIMER_EGG, 110, None);
    } else {
        brand::play(brand::WOOF);
        react(hwnd, Face::Alert, 500);
    }
}

/// Bubbles rising past Rommel's ears, one unit per tick; he grins while they pass.
unsafe fn paint_bubbles(hdc: HDC, u: i32) {
    let f = EGG.load(Relaxed) as i32;
    for (i, (dx, size)) in [(11, 1), (14, 2), (9, 2), (12, 1)].into_iter().enumerate() {
        let rise = (f - i as i32 * 5).rem_euclid(20) + if f > i as i32 * 5 { 0 } else { 99 };
        if rise > 14 { continue }
        let y = px(17) + (10 - rise) * u;
        if y < 0 { continue }
        let r = brand::rect(px(16) + dx * u, y, size * u, size * u);
        brand::fill(hdc, &r, BLACK);
        brand::fill(hdc, &brand::rect(r.left, r.top, u, u), PAPER);
    }
}

// ---------- painting ----------

/// Lockup at u = 3 on the left, then the app name as a small Silkscreen 16 label on the wordmark's baseline.
unsafe fn title_band(hdc: HDC, f: Face) {
    brand::fill(hdc, &prect(0, 0, 360, 64), MINT);
    let (x, y, u) = (px(16), px(17), px(3));
    brand::lockup(hdc, x, y, u, f, &LIGHT, BLACK);
    pixel_text(hdc, &brand::rect(x + 39 * u + px(12), y + 9 * u - 5 * px(2), px(160), px(16)), 16, APP, BLACK, false);
    brand::fill(hdc, &prect(0, 64, 360, 1), GREY);
}

unsafe fn paint(hwnd: HWND) {
    let mut ps: PAINTSTRUCT = zeroed();
    let hdc = BeginPaint(hwnd, &mut ps);
    let tick = BOOT.load(Relaxed);
    if tick > 0 {
        paint_boot(hdc, tick as i32);
    } else {
        brand::fill(hdc, &prect(0, 0, 360, HEIGHT + (MAX_PEERS as i32 + 1) * ROW), PAPER);
        title_band(hdc, face());
        if EGG.load(Relaxed) > 0 { paint_bubbles(hdc, px(3)) }
        // launch bar: a mint block stepping along the band's foot while we look for the other PC
        if STARTED.load(Relaxed) && TRAY_STATE.load(Relaxed) == TRAY_OFF {
            brand::fill(hdc, &prect(0, 60, 360, 4), BLACK);
            let step = (BAR.load(Relaxed) % 40) as i32;
            brand::fill(hdc, &prect(step * 8, 60, 40, 4), MINT);
        }
        pixel_text(hdc, &prect(16 + code_dx(), 80, 328, 16), 16, "Pairing code", BLACK, false);
        pixel_text(hdc, &prect(16, 152, 328, 16), 16, "Where is the other PC", BLACK, false);
        let code_frame = if ERR.load(Relaxed) { DANGER } else if GetFocus() == GetDlgItem(hwnd, ID_CODE as i32) { BLUE } else { GREY };
        brand::frame(hdc, &prect(16 + code_dx(), 96, 328, 44), PAPER, code_frame); // behind the code box
        paint_panel(hdc);
    }
    EndPaint(hwnd, &ps);
}

/// Boot, 1.1 s on 55 ms ticks, following tokens/rowmi.json: the word steps down six rows per claw tap
/// (0/110/220/330 ms) and lands with a one-row bump; the ping at 420 ms is in startup.wav; on the
/// ring (510) Rommel goes alert with the ! emote and the shell rises; 800 idle; 900 app name; 1100 done.
unsafe fn paint_boot(hdc: HDC, tick: i32) {
    let full = prect(0, 0, 360, HEIGHT);
    let (u, x, y) = (px(8), px(24), px(200)); // Display 64: lockup 39x10 units at 8 px, centred
    brand::fill(hdc, &full, PAPER);
    let rows = match tick { ..=9 => 0, 10 => 13, 11 => 26, _ => 40 }; // shell rises 4 dot-rows per frame, 10 frames
    if rows > 0 { brand::fill(hdc, &brand::rect(0, full.bottom - full.bottom * rows / 40, full.right, full.bottom), MINT) }
    let f = if (10..15).contains(&tick) { Face::Alert } else { Face::Idle };
    brand::draw_rommel(hdc, x, y, u, f, &LIGHT);
    if f == Face::Alert { brand::emote_alert(hdc, x + u, y - 9 * u, u) }
    let drop = match tick { 1 | 2 => -18, 3 | 4 => -12, 5 | 6 => -6, 7 => 1, _ => 0 };
    brand::wordmark(hdc, x + 12 * u, y + (4 + drop) * u, u, BLACK);
    if tick >= 17 { pixel_text(hdc, &brand::rect(x + 12 * u, y + 12 * u, 27 * u, px(24)), 24, APP, BLACK, false) }
}

unsafe fn end_boot(hwnd: HWND) {
    if BOOT.swap(0, Relaxed) == 0 { return }
    KillTimer(hwnd, TIMER_BOOT);
    for id in [ID_CODE, ID_HOST, ID_CLIENT, ID_STATUS] { ShowWindow(GetDlgItem(hwnd, id as i32), SW_SHOW); }
    SetFocus(GetDlgItem(hwnd, ID_CODE as i32));
    InvalidateRect(hwnd, null(), 1);
    show_peers(hwnd); // anything found while the boot screen was up
}

/// Owner-drawn button: the frame, Plex label, A glyph on the filled one, blue only for focus.
unsafe fn draw_button(d: &DrawItem) {
    let primary = d.ctl_id as isize == ID_HOST;
    let pressed = d.state & ODS_SELECTED != 0;
    let disabled = d.state & ODS_DISABLED != 0;
    let mut r = d.rc;
    r.right -= 2; r.bottom -= 2; // room for the drop
    brand::fill(d.hdc, &d.rc, PAPER);
    if pressed { r.left += 2; r.top += 2; r.right += 2; r.bottom += 2; } // squish: sits down into its drop
    let (face, ink, frame) = match (primary, disabled) {
        (_, true) => (SUNKEN, MUTED, GREY),
        (true, _) => (BLACK, SUNKEN, BLACK),
        (false, _) => (PAPER, BLACK, GREY),
    };
    if !pressed && !disabled { // the drop: grey, or the darker border on the filled button
        let drop = if primary { MUTED } else { GREY };
        brand::rounded(d.hdc, &brand::rect(r.left + 2, r.top + 2, r.right - r.left, r.bottom - r.top), drop, drop, 6);
    }
    brand::rounded(d.hdc, &r, face, if d.state & ODS_FOCUS != 0 { BLUE } else { frame }, 6);
    if d.state & ODS_FOCUS != 0 { // a 2 px blue ring
        brand::rounded(d.hdc, &brand::rect(r.left + 1, r.top + 1, r.right - r.left - 2, r.bottom - r.top - 2), face, BLUE, 5);
    }
    let mut buf = [0u16; 64];
    let n = GetWindowTextW(d.hwnd, buf.as_mut_ptr(), 64) as usize;
    plex_text(d.hdc, &r, 3, &String::from_utf16_lossy(&buf[..n]), ink, true);
    if primary && !disabled { // the A glyph in the shell colour
        let a = brand::rect(r.left + px(12), (r.top + r.bottom) / 2 - px(10), px(20), px(20));
        brand::rounded(d.hdc, &a, MINT, MINT, px(20));
        pixel_text(d.hdc, &a, 16, "A", BLACK, true);
    }
}

// ---------- pairing code ----------

fn code_file() -> PathBuf {
    PathBuf::from(std::env::var_os("APPDATA").unwrap_or_default()).join(r"ROWMI\Roam\roam.code")
}

/// The pairing code from the last time a button was pressed, if there is one.
fn saved_code() -> Option<String> {
    let saved = std::fs::read_to_string(code_file()).ok()?;
    let code = saved.lines().next()?.trim();
    (code.len() == 10).then(|| code.to_string())
}

/// Forget on a PC that was only found through the saved code: empty that code too, or it is found again.
fn forget_saved_code(code: &str) {
    if saved_code().as_deref() != Some(code) { return }
    let rest: Vec<String> = std::fs::read_to_string(code_file()).unwrap_or_default().lines().skip(1).map(String::from).collect();
    let _ = std::fs::write(code_file(), format!("\n{}", rest.join("\n")));
}

/// Settings used to sit beside the exe. Copied before the delete, so a failure loses nothing.
fn migrate_code(old: &Path, new: &Path) {
    if let Some(dir) = new.parent() { let _ = std::fs::create_dir_all(dir); }
    if !new.exists() && std::fs::copy(old, new).is_ok() { let _ = std::fs::remove_file(old); }
}

fn gen_code() -> String {
    let mut b = [0u8; 10];
    OsRng.fill_bytes(&mut b);
    b.iter().map(|x| ALPHABET[(*x % 32) as usize] as char).collect()
}

fn pretty(code: &str) -> String {
    if code.len() == 10 { format!("{}-{}", &code[..5], &code[5..]) } else { code.to_string() }
}

// ---------- window ----------

pub fn run() {
    unsafe {
        SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        load_monitors();
        brand::load_fonts();
        PAPER_BRUSH.store(CreateSolidBrush(PAPER) as usize, Relaxed);
        let cls = wide("ROWMI.roam");
        let wc = WNDCLASSW {
            style: 0, lpfnWndProc: Some(wndproc), cbClsExtra: 0, cbWndExtra: 0, hInstance: null_mut(),
            hIcon: brand::app_icon(48, MINT, Face::Idle), hCursor: LoadCursorW(null_mut(), IDC_ARROW),
            hbrBackground: PAPER_BRUSH.load(Relaxed) as HBRUSH, lpszMenuName: null(), lpszClassName: cls.as_ptr(),
        };
        RegisterClassW(&wc);
        let hwnd = CreateWindowExW(0, cls.as_ptr(), wide(TITLE).as_ptr(), STYLE, CW_USEDEFAULT, CW_USEDEFAULT, 0, 0, null_mut(), null_mut(), null_mut(), null());
        MAIN.store(hwnd as usize, Relaxed);
        SendMessageW(hwnd, WM_SETICON, 0 /*small*/, brand::app_icon(GetSystemMetrics(SM_CXSMICON), MINT, Face::Idle) as isize);
        // tray, at the size the tray really shows: the app icon = mouse is here, Rommel on Paper = it's
        // on the other PC, sleepy on Cartridge Grey = not connected
        let sm = GetSystemMetrics(SM_CXSMICON);
        ICONS[TRAY_OFF as usize].store(brand::app_icon(sm, GREY, Face::Sleepy) as usize, Relaxed);
        ICONS[TRAY_HERE as usize].store(brand::app_icon(sm, MINT, Face::Idle) as usize, Relaxed);
        ICONS[TRAY_AWAY as usize].store(brand::app_icon(sm, PAPER, Face::Idle) as usize, Relaxed);

        DPI.store(GetDpiForWindow(hwnd) as usize, Relaxed);

        // controls start hidden behind the boot screen; layout() places them
        let ctl = |class: &str, text: &str, extra: u32, id: isize| {
            CreateWindowExW(0, wide(class).as_ptr(), wide(text).as_ptr(), WS_CHILD | extra,
                0, 0, 0, 0, hwnd, id as HMENU, null_mut(), null())
        };
        let code = ctl("EDIT", "", WS_TABSTOP | 1 /*center*/ | 8 /*uppercase*/ | 0x80 /*autohscroll*/, ID_CODE);
        ctl("BUTTON", "Roam from this PC", WS_TABSTOP | BS_OWNERDRAW, ID_HOST);
        ctl("BUTTON", "Be controlled", WS_TABSTOP | BS_OWNERDRAW, ID_CLIENT);
        ctl("BUTTON", "Heel", WS_TABSTOP | BS_OWNERDRAW, ID_STOP);
        ctl("BUTTON", "Update", WS_TABSTOP | BS_OWNERDRAW, ID_UPDATE);
        for i in 0..MAX_PEERS as isize { ctl("BUTTON", "", WS_TABSTOP | BS_OWNERDRAW, ID_PEER + i); }
        ctl("STATIC", SETUP_HELP, 0, ID_STATUS);
        load_type(hwnd);
        layout(hwnd);
        make_spark();
        let about_cls = wide("ROWMI.roam.about");
        RegisterClassW(&WNDCLASSW { lpfnWndProc: Some(about_proc), hCursor: wc.hCursor, lpszClassName: about_cls.as_ptr(), ..zeroed() });
        let sys = GetSystemMenu(hwnd, 0);
        AppendMenuW(sys, MF_SEPARATOR, 0, null());
        AppendMenuW(sys, 0, SC_ABOUT, wide("About Roam").as_ptr());
        EDIT_PROC.store(SetWindowLongPtrW(code, GWLP_WNDPROC, code_proc as usize as isize) as usize, Relaxed);
        migrate_code(&std::env::current_exe().unwrap().with_file_name("roam.code"), &code_file());
        if let Ok(saved) = std::fs::read_to_string(code_file()) {
            let mut lines = saved.lines();
            SetWindowTextW(code, wide(&pretty(lines.next().unwrap_or("").trim())).as_ptr());
            win::load_target(lines.next().unwrap_or(""), &MONITORS.lock().unwrap());
        }
        set_face(Face::Idle);
        // boot, once per launch; reduced motion = the finished screen for 800 ms, sound kept
        let mut anim: BOOL = 1;
        SystemParametersInfoW(SPI_GETCLIENTAREAANIMATION, 0, &mut anim as *mut _ as _, 0);
        BOOT.store(if anim != 0 { 1 } else { 17 }, Relaxed);
        brand::play(brand::STARTUP);
        SetTimer(hwnd, TIMER_BOOT, if anim != 0 { BOOT_TICK } else { 800 }, None);
        ShowWindow(hwnd, SW_SHOW);
        update::start();
        start_finder();

        let mut msg: MSG = zeroed();
        while GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {
            let dialog = match ABOUT.load(Relaxed) { 0 => hwnd, a => a as HWND }; // Tab and Esc go to what's on top
            if IsDialogMessageW(dialog, &msg) == 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
    }
}

unsafe fn start(hwnd: HWND, id: isize) {
    let mut buf = [0u16; 64];
    let n = GetDlgItemTextW(hwnd, ID_CODE as i32, buf.as_mut_ptr(), 64) as usize;
    let mut code: String = String::from_utf16_lossy(&buf[..n]).to_uppercase().chars().filter(|c| c.is_ascii_alphanumeric()).collect();
    if id == ID_CLIENT && code.is_empty() {
        code = gen_code();
    }
    if code.len() != 10 {
        bonk(hwnd);
        react(hwnd, Face::Sad, 800);
        status("Type the 10-character code shown on the other PC.");
        return;
    }
    brand::play(brand::CONFIRM);
    ERR.store(false, Relaxed);
    SetDlgItemTextW(hwnd, ID_CODE as i32, wide(&pretty(&code)).as_ptr());
    let _ = std::fs::write(code_file(), format!("{code}\n{}", win::save_target())); // fine if it can't be saved
    ShowWindow(GetDlgItem(hwnd, ID_HOST as i32), SW_HIDE);
    ShowWindow(GetDlgItem(hwnd, ID_CLIENT as i32), SW_HIDE);
    ShowWindow(GetDlgItem(hwnd, ID_STOP as i32), SW_SHOW);
    SendMessageW(GetDlgItem(hwnd, ID_CODE as i32), EM_SETREADONLY, 1, 0);
    let mut d = nid(NIF_MESSAGE | NIF_ICON | NIF_TIP);
    d.hIcon = ICONS[TRAY_OFF as usize].load(Relaxed) as HICON;
    let tip = wide(TITLE);
    d.szTip[..tip.len()].copy_from_slice(&tip);
    Shell_NotifyIconW(NIM_ADD, &d);
    STARTED.store(true, Relaxed);
    FINDER.fetch_add(1, Relaxed); // a role owns the port now
    PEERS.lock().unwrap().clear();
    show_peers(hwnd); // hides the buttons; the status below replaces its text
    SetTimer(hwnd, TIMER_BAR, 120, None);
    HEELED.store(false, Relaxed);
    let gen = win::begin();
    status(if id == ID_HOST { "Looking for the other PC. This window tucks into the tray once they connect." }
           else { "Waiting. Type this code on the other PC and press Roam from this PC." });
    std::thread::spawn(move || {
        let r = std::panic::catch_unwind(|| if id == ID_HOST { win::host(&code, gen) } else { win::client(&code, gen) });
        if let Err(e) = r {
            let why = e.downcast_ref::<String>().cloned().or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string())).unwrap_or_default();
            brand::play(brand::ERROR);
            status(&format!("Stopped: something went wrong inside ({why}). Press Heel and try again."));
            tray(TRAY_OFF);
        }
    });
}

/// Heel: end the connection and put this same window back on the setup screen, saying so.
unsafe fn stop(hwnd: HWND) {
    win::shutdown();
    KillTimer(hwnd, TIMER_BAR);
    Shell_NotifyIconW(NIM_DELETE, &nid(0));
    STARTED.store(false, Relaxed);
    TRAY_STATE.store(TRAY_OFF, Relaxed);
    OFFLINE.store(false, Relaxed);
    HEELED.store(true, Relaxed);
    ShowWindow(GetDlgItem(hwnd, ID_STOP as i32), SW_HIDE);
    ShowWindow(GetDlgItem(hwnd, ID_HOST as i32), SW_SHOW);
    ShowWindow(GetDlgItem(hwnd, ID_CLIENT as i32), SW_SHOW);
    SendMessageW(GetDlgItem(hwnd, ID_CODE as i32), EM_SETREADONLY, 0, 0);
    show(hwnd);
    SetFocus(GetDlgItem(hwnd, ID_CODE as i32));
    start_finder();
    show_peers(hwnd);
    react(hwnd, Face::Happy, 800);
}

unsafe fn quit(hwnd: HWND) {
    win::shutdown();
    DestroyWindow(hwnd);
}

unsafe fn show(hwnd: HWND) {
    ShowWindow(hwnd, SW_SHOW);
    SetForegroundWindow(hwnd);
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_COMMAND => {
            let id = (wp & 0xFFFF) as isize;
            match id {
                ID_HOST | ID_CLIENT => start(hwnd, id),
                ID_STOP => { brand::play(brand::BACK); stop(hwnd) }
                ID_UPDATE => {
                    brand::play(brand::CONFIRM);
                    SetFocus(GetDlgItem(hwnd, ID_CODE as i32)); // a disabled button can't keep it
                    EnableWindow(GetDlgItem(hwnd, ID_UPDATE as i32), 0);
                    update::install();
                }
                ID_CODE if wp >> 16 == EN_CHANGE => { // edited: the error state is over
                    if ERR.swap(false, Relaxed) {
                        InvalidateRect(hwnd, &prect(0, 76, 360, 68), 0);
                        InvalidateRect(GetDlgItem(hwnd, ID_STATUS as i32), null(), 1);
                    }
                }
                ID_CODE if wp >> 16 == EN_SETFOCUS || wp >> 16 == EN_KILLFOCUS => { InvalidateRect(hwnd, &prect(0, 76, 360, 68), 0); }
                _ if (ID_PEER..ID_PEER + MAX_PEERS as isize).contains(&id) => {
                    let found = SHOWN.lock().unwrap().get((id - ID_PEER) as usize).cloned();
                    if let Some(f) = found {
                        // the same path as typing the code and pressing the button, so the code is visible and saved
                        SetWindowTextW(GetDlgItem(hwnd, ID_CODE as i32), wide(&pretty(&f.code)).as_ptr());
                        start(hwnd, if f.wants { ID_CLIENT } else { ID_HOST });
                    }
                }
                _ => {}
            }
            0
        }
        WM_DRAWITEM => { draw_button(&*(lp as *const DrawItem)); 1 }
        WM_CTLCOLORSTATIC | WM_CTLCOLOREDIT => {
            SetBkColor(wp as HDC, PAPER);
            let id = GetDlgCtrlID(lp as HWND) as isize;
            SetTextColor(wp as HDC, if id == ID_STATUS && ERR.load(Relaxed) { DANGER } else { BLACK });
            PAPER_BRUSH.load(Relaxed) as LRESULT
        }
        WM_TIMER if wp == TIMER_BOOT => {
            let t = BOOT.load(Relaxed);
            if t == 0 { return 0 }
            if t >= 20 { end_boot(hwnd) } else { BOOT.store(t + 1, Relaxed); InvalidateRect(hwnd, null(), 0); }
            0
        }
        WM_TIMER if wp == TIMER_BAR => {
            BAR.fetch_add(1, Relaxed);
            if TRAY_STATE.load(Relaxed) == TRAY_OFF && IsWindowVisible(hwnd) != 0 { InvalidateRect(hwnd, &prect(0, 60, 360, 4), 0); }
            0
        }
        WM_TIMER if wp == TIMER_EGG => {
            let f = EGG.load(Relaxed);
            if f >= 60 { KillTimer(hwnd, TIMER_EGG); EGG.store(0, Relaxed); set_face(settled_face()); } else { EGG.store(f + 1, Relaxed); }
            InvalidateRect(hwnd, &prect(0, 0, 360, 64), 0);
            0
        }
        WM_TIMER if wp == TIMER_FACE => {
            KillTimer(hwnd, TIMER_FACE);
            if EGG.load(Relaxed) == 0 { set_face(settled_face()) }
            InvalidateRect(hwnd, &prect(0, 0, 360, 64), 0);
            0
        }
        WM_KEYDOWN | WM_LBUTTONDOWN if BOOT.load(Relaxed) >= 6 => { end_boot(hwnd); 0 } // any input after 300 ms skips
        WM_REFRESH => { refresh(hwnd); 0 }
        WM_CLOSE_NOW => { quit(hwnd); 0 }
        WM_SYSCOMMAND if wp & 0xFFF0 == SC_ABOUT => { open_about(hwnd); 0 }
        WM_HEEL => {
            if STARTED.load(Relaxed) { brand::play(brand::BACK); stop(hwnd) }
            0
        }
        WM_ARRIVED => { show_spark(hwnd, wp as isize as i32, lp as i32); 0 }
        WM_TIMER if wp == TIMER_SPARK => {
            let f = SPARK_AT.fetch_add(1, Relaxed) + 1;
            let t = SPARK_WIN.load(Relaxed) as HWND;
            if f >= SPARK_FRAMES { KillTimer(hwnd, TIMER_SPARK); ShowWindow(t, SW_HIDE); } else { InvalidateRect(t, null(), 0); }
            0
        }
        WM_TIMER if wp == TIMER_BONK => {
            let f = BONK_AT.load(Relaxed) + 1;
            if f as usize > BONK.len() { BONK_AT.store(0, Relaxed); KillTimer(hwnd, TIMER_BONK); } else { BONK_AT.store(f, Relaxed) }
            place_code(hwnd);
            0
        }
        WM_PEERS => { // while booting, end_boot shows whatever was found
            if BOOT.load(Relaxed) == 0 && !STARTED.load(Relaxed) { show_peers(hwnd) }
            0
        }
        WM_CONTEXTMENU => {
            let i = GetDlgCtrlID(wp as HWND) as isize - ID_PEER;
            if (0..MAX_PEERS as isize).contains(&i) { forget_menu(hwnd, i as usize, lp == -1); return 0 }
            DefWindowProcW(hwnd, msg, wp, lp)
        }
        WM_DPICHANGED => { // the window crossed to a monitor with another scale
            DPI.store((wp & 0xFFFF) as usize, Relaxed);
            let r = &*(lp as *const RECT);
            SetWindowPos(hwnd, null_mut(), r.left, r.top, 0, 0, SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE);
            load_type(hwnd);
            layout(hwnd);
            InvalidateRect(hwnd, null(), 1);
            0
        }
        WM_PAINT => { paint(hwnd); 0 }
        WM_ERASEBKGND => 1, // paint() covers everything; skipping the erase stops the boot frames flickering
        WM_LBUTTONDOWN if BOOT.load(Relaxed) == 0 && in_rommel(lp) => { tap_rommel(hwnd); 0 }
        WM_LBUTTONDOWN | WM_MOUSEMOVE | WM_LBUTTONUP if BOOT.load(Relaxed) == 0 => { drag(hwnd, msg, lp); 0 }
        WM_TRAY if lp as u32 == WM_LBUTTONUP => { brand::play(brand::CLICK); show(hwnd); 0 }
        WM_TRAY if lp as u32 == WM_RBUTTONUP => {
            let m = CreatePopupMenu();
            AppendMenuW(m, 0, MENU_SHOW, wide("Show window").as_ptr());
            AppendMenuW(m, 0, MENU_STOP, wide("Heel (stop)").as_ptr());
            AppendMenuW(m, 0, MENU_ABOUT, wide("About Roam").as_ptr());
            AppendMenuW(m, 0, MENU_QUIT, wide("Quit").as_ptr());
            let mut p: POINT = zeroed();
            GetCursorPos(&mut p);
            SetForegroundWindow(hwnd); // required or the menu won't close on click-away
            let pick = TrackPopupMenu(m, TPM_RETURNCMD | TPM_RIGHTBUTTON, p.x, p.y, 0, hwnd, null());
            DestroyMenu(m);
            match pick as usize {
                MENU_SHOW => { brand::play(brand::CLICK); show(hwnd) }
                MENU_STOP => { brand::play(brand::BACK); stop(hwnd) }
                MENU_ABOUT => { show(hwnd); open_about(hwnd) }
                MENU_QUIT => { brand::play(brand::OFF); quit(hwnd) }
                _ => {}
            }
            0
        }
        // once running, closing the window only hides it; Heel/Quit live in the window and tray menu
        WM_CLOSE if STARTED.load(Relaxed) => { ShowWindow(hwnd, SW_HIDE); 0 }
        WM_CLOSE => { quit(hwnd); 0 }
        WM_DESTROY => { Shell_NotifyIconW(NIM_DELETE, &nid(0)); PostQuitMessage(0); 0 }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}
