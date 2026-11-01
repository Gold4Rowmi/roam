//! Mac side: the host captures input with a Quartz event tap, the client injects with CGEventPost.
//! Same connection logic as win.rs, over the same protocol. Needs the Accessibility permission.
//! shortcut: this mirrors win.rs's session loops rather than sharing them, so Windows stayed untouched; merge them once the Mac is proven.
use crate::gui::{arrived, heel, status, tray, visible, TRAY_AWAY, TRAY_HERE, TRAY_OFF};
use crate::keymap;
use crate::net::{bind_lan, Link, Msg, CLIENT, HOST, MAX_CLIP, PORT};
use crate::trust;
use std::ffi::c_void;
use std::io::Write;
use std::net::{SocketAddr, UdpSocket};
use std::process::{Command, Stdio};
use std::ptr::{null, null_mut};
use std::sync::atomic::{AtomicBool, AtomicPtr, AtomicU64, AtomicU8, AtomicUsize, Ordering::Relaxed};
use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

const VERSION_MISMATCH: &str = "The other PC runs a different Roam version. Update both PCs, then try again.";
const NEEDS_ACCESS: &str = "Roam needs Accessibility. Turn it on for Roam in the settings that just opened, then press Heel and try again.";
pub const BROADCAST: SocketAddr = SocketAddr::new(std::net::IpAddr::V4(std::net::Ipv4Addr::BROADCAST), PORT);

type Event = *mut c_void;
type TapFn = unsafe extern "C" fn(*mut c_void, u32, Event, *mut c_void) -> Event;

#[repr(C)]
#[derive(Clone, Copy)]
struct Point { x: f64, y: f64 }
#[repr(C)]
#[derive(Clone, Copy)]
struct Size { w: f64, h: f64 }
#[repr(C)]
#[derive(Clone, Copy)]
struct Rect { origin: Point, size: Size }

#[link(name = "ApplicationServices", kind = "framework")]
#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    static kCFRunLoopCommonModes: *const c_void;
    static kCFRunLoopDefaultMode: *const c_void;
    fn CFRelease(o: *const c_void);
    fn CFRunLoopGetCurrent() -> *mut c_void;
    fn CFRunLoopAddSource(l: *mut c_void, s: *mut c_void, mode: *const c_void);
    fn CFRunLoopRemoveSource(l: *mut c_void, s: *mut c_void, mode: *const c_void);
    fn CFRunLoopRunInMode(mode: *const c_void, seconds: f64, return_after: u8) -> i32;
    fn CFMachPortCreateRunLoopSource(alloc: *const c_void, port: *mut c_void, order: isize) -> *mut c_void;
    fn CGEventTapCreate(tap: u32, place: u32, options: u32, mask: u64, callback: TapFn, info: *mut c_void) -> *mut c_void;
    fn CGEventTapEnable(tap: *mut c_void, on: bool);
    fn CGEventGetIntegerValueField(e: Event, field: u32) -> i64;
    fn CGEventSetIntegerValueField(e: Event, field: u32, value: i64);
    fn CGEventGetLocation(e: Event) -> Point;
    fn CGEventGetFlags(e: Event) -> u64;
    fn CGEventSetFlags(e: Event, flags: u64);
    fn CGEventCreate(source: *const c_void) -> Event;
    fn CGEventCreateMouseEvent(source: *const c_void, kind: u32, at: Point, button: u32) -> Event;
    fn CGEventCreateKeyboardEvent(source: *const c_void, key: u16, down: bool) -> Event;
    fn CGEventCreateScrollWheelEvent(source: *const c_void, units: u32, count: u32, wheel1: i32, ...) -> Event;
    fn CGEventPost(tap: u32, e: Event);
    fn CGWarpMouseCursorPosition(p: Point) -> i32;
    fn CGAssociateMouseAndMouseCursorPosition(connected: i32) -> i32;
    fn CGGetActiveDisplayList(max: u32, list: *mut u32, count: *mut u32) -> i32;
    fn CGDisplayBounds(d: u32) -> Rect;
    fn AXIsProcessTrusted() -> bool;
}

// CGEventType
const LEFT_DOWN: u32 = 1;
const LEFT_UP: u32 = 2;
const RIGHT_DOWN: u32 = 3;
const RIGHT_UP: u32 = 4;
const MOVED: u32 = 5;
const LEFT_DRAG: u32 = 6;
const RIGHT_DRAG: u32 = 7;
const KEY_DOWN: u32 = 10;
const KEY_UP: u32 = 11;
const FLAGS: u32 = 12;
const SCROLL: u32 = 22;
const OTHER_DOWN: u32 = 25;
const OTHER_UP: u32 = 26;
const OTHER_DRAG: u32 = 27;
const TAP_OFF_TIMEOUT: u32 = 0xFFFF_FFFE;
const TAP_OFF_USER: u32 = 0xFFFF_FFFF;
// CGEventField
const CLICK_COUNT: u32 = 1;
const BUTTON_NUMBER: u32 = 3;
const DELTA_X: u32 = 4;
const DELTA_Y: u32 = 5;
const KEYCODE: u32 = 9;
const SCROLL_Y: u32 = 11;
const SCROLL_X: u32 = 12;
// CGEventFlags
const SHIFT: u64 = 0x2_0000;
const CONTROL: u64 = 0x4_0000;
const OPTION: u64 = 0x8_0000;
const COMMAND: u64 = 0x10_0000;
const CAPS_KEY: u16 = 0x39;
const ESCAPE_KEY: u16 = 0x35;

fn mask(kinds: &[u32]) -> u64 { kinds.iter().fold(0, |m, k| m | 1 << k) }

type Job = Box<dyn FnOnce() + Send>;
static SIDE: OnceLock<Sender<Job>> = OnceLock::new();

/// Clipboard and sound run here, never on an input path: the tap is disabled if it stalls.
fn later(job: impl FnOnce() + Send + 'static) {
    if let Some(side) = SIDE.get() { let _ = side.send(Box::new(job)); }
}

/// This Mac's name as the other PC will show it, asked for once: pings carry it twice a second.
pub fn name() -> String {
    static NAME: OnceLock<String> = OnceLock::new();
    NAME.get_or_init(|| {
        let asked = Command::new("/usr/sbin/scutil").args(["--get", "ComputerName"]).output();
        asked.ok().map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).filter(|n| !n.is_empty()).unwrap_or_else(|| "this Mac".into())
    }).clone()
}

fn trust_peer(code: &str, peer: &str) {
    if !peer.is_empty() && peer != name() { trust::remember(code, peer) }
}

static REMOTE: AtomicBool = AtomicBool::new(false);
/// Which edge of the host screen leads to the client: 0 right, 1 left, 2 top, 3 bottom.
static EDGE: AtomicU8 = AtomicU8::new(0);
/// The display the other PC sits beside: (x, y, w, h) in points, 0 wide until chosen.
static TARGET: Mutex<(i32, i32, i32, i32)> = Mutex::new((0, 0, 0, 0));
static LINK: Mutex<Option<Arc<Link>>> = Mutex::new(None);
static SCREEN: Mutex<(i32, i32, i32, i32)> = Mutex::new((0, 0, 0, 0));
static LAST_PING: Mutex<Option<Instant>> = Mutex::new(None);
static SESSION: AtomicUsize = AtomicUsize::new(0); // bumping it ends the running connection's threads
static ONE: Mutex<()> = Mutex::new(());            // held for a whole connection: the next waits for the last to finish
static TAP: AtomicPtr<c_void> = AtomicPtr::new(null_mut());
static HELD: AtomicU64 = AtomicU64::new(0); // device modifier bits last seen by the tap

fn link() -> Option<Arc<Link>> { LINK.lock().unwrap().clone() }
fn live(gen: usize) -> bool { SESSION.load(Relaxed) == gen }
pub fn begin() -> usize { SESSION.fetch_add(1, Relaxed) + 1 }

/// The displays as (x, y, w, h) in points, the main display first.
pub fn displays() -> Vec<(i32, i32, i32, i32)> {
    let (mut ids, mut n) = ([0u32; 16], 0u32);
    unsafe { CGGetActiveDisplayList(16, ids.as_mut_ptr(), &mut n) };
    ids[..n as usize].iter().map(|d| {
        let r = unsafe { CGDisplayBounds(*d) };
        (r.origin.x as i32, r.origin.y as i32, r.size.w as i32, r.size.h as i32)
    }).collect()
}

fn desktop() -> (i32, i32, i32, i32) {
    let d = displays();
    let (l, t) = (d.iter().map(|r| r.0).min().unwrap_or(0), d.iter().map(|r| r.1).min().unwrap_or(0));
    let (r, b) = (d.iter().map(|r| r.0 + r.2).max().unwrap_or(1920), d.iter().map(|r| r.1 + r.3).max().unwrap_or(1080));
    (l, t, r - l, b - t)
}

pub fn target() -> ((i32, i32, i32, i32), u8) { (*TARGET.lock().unwrap(), EDGE.load(Relaxed)) }

pub fn set_target(rect: (i32, i32, i32, i32), e: u8) {
    EDGE.store(e, Relaxed);
    *TARGET.lock().unwrap() = rect;
}

pub fn save_target() -> String {
    let (l, t, w, h) = *TARGET.lock().unwrap();
    format!("{l} {t} {w} {h} {}", EDGE.load(Relaxed))
}

/// Restores a saved "l t w h edge"; falls back to the main display if that one is gone.
pub fn load_target(line: &str, mons: &[(i32, i32, i32, i32)]) {
    let v: Vec<i32> = line.split_whitespace().filter_map(|x| x.parse().ok()).collect();
    let saved = (v.len() == 5).then(|| (v[0], v[1], v[2], v[3])).filter(|r| mons.contains(r));
    set_target(saved.unwrap_or(mons[0]), if v.len() == 5 { v[4].clamp(0, 3) as u8 } else { 0 });
}

/// If `p` is on `edge`, its position along that edge as 0..1.
fn at_edge(edge: u8, p: Point) -> Option<f32> {
    let (l, t, w, h) = *SCREEN.lock().unwrap();
    let (x, y) = (p.x as i32, p.y as i32);
    let (along_y, along_x) = ((y - t) as f32 / h as f32, (x - l) as f32 / w as f32);
    let (in_y, in_x) = (y >= t && y < t + h, x >= l && x < l + w);
    match edge {
        0 if x >= l + w - 1 && in_y => Some(along_y),
        1 if x <= l && in_y => Some(along_y),
        2 if y <= t && in_x => Some(along_x),
        3 if y >= t + h - 1 && in_x => Some(along_x),
        _ => None,
    }
}

fn near_edge(edge: u8, frac: f32) -> Point {
    let (l, t, w, h) = *SCREEN.lock().unwrap();
    let (x, y) = (l + (frac * w as f32) as i32, t + (frac * h as f32) as i32);
    let (x, y) = match edge { 0 => (l + w - 3, y), 1 => (l + 2, y), 2 => (x, t + 2), _ => (x, t + h - 3) };
    Point { x: x as f64, y: y as f64 }
}

fn pushing_out(edge: u8, dx: i16, dy: i16) -> bool {
    match edge { 0 => dx > 0, 1 => dx < 0, 2 => dy < 0, _ => dy > 0 }
}

fn init(secret: &str, sock: UdpSocket, role: u8, peer: Option<SocketAddr>) -> Arc<Link> {
    SIDE.get_or_init(|| {
        let (tx, rx) = channel::<Job>();
        std::thread::spawn(move || for job in rx { job() });
        tx
    });
    let target = *TARGET.lock().unwrap();
    *SCREEN.lock().unwrap() = if role == HOST && target.2 > 0 { target } else { desktop() };
    let link = Arc::new(Link::new(secret, sock, role, peer));
    *LINK.lock().unwrap() = Some(link.clone());
    link
}

fn clean_up() {
    *LINK.lock().unwrap() = None;
    *LAST_PING.lock().unwrap() = None;
    REMOTE.store(false, Relaxed);
    HELD.store(0, Relaxed);
    unsafe { CGAssociateMouseAndMouseCursorPosition(1) };
}

/// False after pointing the user at the setting: without it the tap fails and injected input is dropped.
fn allowed() -> bool {
    let ok = unsafe { AXIsProcessTrusted() };
    if !ok {
        let _ = Command::new("/usr/bin/open").arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility").spawn();
        crate::brand::play(crate::brand::ERROR);
        status(NEEDS_ACCESS);
    }
    ok
}

// ---------- host ----------

pub fn host(secret: &str, gen: usize) {
    let _one = ONE.lock().unwrap_or_else(|e| e.into_inner());
    if !live(gen) || !allowed() { return }
    let sock = UdpSocket::bind("0.0.0.0:0").unwrap();
    sock.set_broadcast(true).unwrap(); // pings go to the whole LAN until the client answers
    let link = init(secret, sock, HOST, Some(BROADCAST));
    let (code, l) = (secret.to_string(), link.clone());
    let receiver = std::thread::spawn(move || host_recv(&l, &code, gen));
    let l = link.clone();
    let pinger = std::thread::spawn(move || while live(gen) {
        l.send(&Msg::Ping(name()));
        std::thread::sleep(Duration::from_millis(500));
    });
    drop(link);
    let kinds = mask(&[LEFT_DOWN, LEFT_UP, RIGHT_DOWN, RIGHT_UP, MOVED, LEFT_DRAG, RIGHT_DRAG, KEY_DOWN, KEY_UP, FLAGS, SCROLL, OTHER_DOWN, OTHER_UP, OTHER_DRAG]);
    unsafe {
        let tap = CGEventTapCreate(1 /*session*/, 0 /*head*/, 0 /*can swallow*/, kinds, on_event, null_mut());
        if tap.is_null() {
            allowed();
        } else {
            TAP.store(tap, Relaxed);
            let source = CFMachPortCreateRunLoopSource(null(), tap, 0);
            let run_loop = CFRunLoopGetCurrent();
            CFRunLoopAddSource(run_loop, source, kCFRunLoopCommonModes);
            CGEventTapEnable(tap, true);
            status("Looking for the other PC...");
            while live(gen) { CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.2, 0); }
            CGEventTapEnable(tap, false);
            CFRunLoopRemoveSource(run_loop, source, kCFRunLoopCommonModes);
            CFRelease(source);
            TAP.store(null_mut(), Relaxed);
            CFRelease(tap);
        }
    }
    while live(gen) { std::thread::sleep(Duration::from_millis(200)); } // no tap: the setup message stays until Heel
    let _ = receiver.join();
    let _ = pinger.join();
    clean_up();
}

fn client_up() -> bool {
    LAST_PING.lock().unwrap().is_some_and(|t| t.elapsed() < Duration::from_secs(2))
}

fn host_recv(link: &Link, code: &str, gen: usize) {
    let (start, mut linked, mut warned, mut peer) = (Instant::now(), false, false, String::new());
    while live(gen) {
        let mut bye = false;
        let msg = link.recv();
        if !live(gen) { break }
        match msg {
            Some(Msg::Leave(f)) => take_back(f),
            Some(Msg::Clip(s)) => later(move || clip_set(&s)),
            Some(Msg::Ping(n)) => { *LAST_PING.lock().unwrap() = Some(Instant::now()); peer = n }
            Some(Msg::Bye) => { *LAST_PING.lock().unwrap() = None; bye = true }
            _ => {}
        }
        if link.mismatch() { status(VERSION_MISMATCH) }
        let was_linked = linked;
        if linked != client_up() {
            linked = !linked;
            if linked {
                trust_peer(code, &peer);
                status(&format!("Connected. Move the mouse off the {} edge of the screen.", ["right", "left", "top", "bottom"][EDGE.load(Relaxed) as usize]));
                tray(TRAY_HERE);
                visible(false);
            } else {
                status("The other PC disconnected. Looking for it again...");
                tray(TRAY_OFF);
                visible(true);
                take_back(0.5);
                link.set_peer(BROADCAST);
            }
        }
        if bye && was_linked { heel() }
        if !linked && !warned && start.elapsed() > Duration::from_secs(3) {
            warned = true;
            status("No answer yet. On the other PC: same code, press Be controlled, and allow Roam on the network.");
        }
    }
}

/// Heel or quit: give the client its input back, say goodbye, and end this connection's threads.
pub fn shutdown() {
    SESSION.fetch_add(1, Relaxed);
    if let Some(l) = LINK.lock().unwrap().take() {
        if REMOTE.swap(false, Relaxed) { l.send(&Msg::Leave(0.5)) }
        l.send(&Msg::Bye);
    }
    unsafe { CGAssociateMouseAndMouseCursorPosition(1) };
}

fn hand_over(frac: f32) {
    if !client_up() { return }
    let Some(link) = link() else { return };
    link.send(&Msg::Enter(EDGE.load(Relaxed), frac));
    REMOTE.store(true, Relaxed);
    unsafe { CGAssociateMouseAndMouseCursorPosition(0) }; // the cursor stays put while the deltas keep coming
    later(move || {
        if let Some(c) = clip_get() { link.send(&Msg::Clip(c)) }
        crate::brand::play(crate::brand::ON);
        status("Roaming on the other PC. Move back through the same edge, or press Control+Option+Esc (Heel), to return.");
        tray(TRAY_AWAY);
    });
}

fn take_back(frac: f32) {
    if !REMOTE.swap(false, Relaxed) { return }
    let p = near_edge(EDGE.load(Relaxed), frac);
    unsafe {
        CGWarpMouseCursorPosition(p);
        CGAssociateMouseAndMouseCursorPosition(1);
    }
    arrived(p.x as i32, p.y as i32);
    later(|| crate::brand::play(crate::brand::OFF));
    status("Back on this PC.");
    tray(TRAY_HERE);
}

/// The raw-input button flags Windows uses on the wire, (down, up), for a CGEvent button number.
fn wire_button(n: i64) -> (u16, u16) {
    match n { 0 => (0x001, 0x002), 1 => (0x004, 0x008), 2 => (0x010, 0x020), 3 => (0x040, 0x080), _ => (0x100, 0x200) }
}

fn send_key(link: &Link, mac: u16, down: bool) {
    if let Some(vk) = keymap::to_vk(mac) {
        link.send(&Msg::Key { vk, scan: 0, up: !down, ext: keymap::extended(vk) });
    }
}

/// The device-side modifier bits, which tell left from right (NX_DEVICE*KEYMASK).
fn modifier_bit(key: u16) -> u64 {
    match key { 0x3B => 0x1, 0x38 => 0x2, 0x3C => 0x4, 0x37 => 0x8, 0x36 => 0x10, 0x3A => 0x20, 0x3D => 0x40, 0x3E => 0x2000, _ => 0 }
}

unsafe extern "C" fn on_event(_: *mut c_void, kind: u32, e: Event, _: *mut c_void) -> Event {
    if kind == TAP_OFF_TIMEOUT || kind == TAP_OFF_USER { // macOS turns off a tap it thinks stalled
        CGEventTapEnable(TAP.load(Relaxed), true);
        return e;
    }
    if !REMOTE.load(Relaxed) {
        if matches!(kind, MOVED | LEFT_DRAG | RIGHT_DRAG | OTHER_DRAG) {
            if let Some(frac) = at_edge(EDGE.load(Relaxed), CGEventGetLocation(e)) { hand_over(frac) }
        }
        return e;
    }
    let Some(link) = link() else { return e };
    let field = |f: u32| CGEventGetIntegerValueField(e, f);
    match kind {
        MOVED | LEFT_DRAG | RIGHT_DRAG | OTHER_DRAG => {
            let (dx, dy) = (field(DELTA_X).clamp(-32767, 32767) as i16, field(DELTA_Y).clamp(-32767, 32767) as i16);
            if dx != 0 || dy != 0 { link.send(&Msg::Move(dx, dy)) }
        }
        LEFT_DOWN | RIGHT_DOWN | OTHER_DOWN | LEFT_UP | RIGHT_UP | OTHER_UP => {
            let (down, up) = wire_button(field(BUTTON_NUMBER));
            link.send(&Msg::Button(if matches!(kind, LEFT_DOWN | RIGHT_DOWN | OTHER_DOWN) { down } else { up }, 0));
        }
        SCROLL => {
            let (y, x) = (field(SCROLL_Y), field(SCROLL_X));
            if y != 0 { link.send(&Msg::Button(0x400, (y * 120).clamp(-32767, 32767) as i16)) }
            if x != 0 { link.send(&Msg::Button(0x800, (-x * 120).clamp(-32767, 32767) as i16)) }
        }
        KEY_DOWN | KEY_UP => {
            let key = field(KEYCODE) as u16;
            let flags = CGEventGetFlags(e);
            if key == ESCAPE_KEY && kind == KEY_DOWN && flags & (CONTROL | OPTION) == CONTROL | OPTION { // the panic button
                link.send(&Msg::Leave(0.5));
                take_back(0.5);
                return null_mut();
            }
            send_key(&link, key, kind == KEY_DOWN);
        }
        FLAGS => {
            let (key, flags) = (field(KEYCODE) as u16, CGEventGetFlags(e));
            if key == CAPS_KEY { // a toggle: the Mac reports one change per press
                send_key(&link, key, true);
                send_key(&link, key, false);
            } else {
                let bit = modifier_bit(key);
                let down = flags & bit != 0;
                if down != (HELD.load(Relaxed) & bit != 0) {
                    HELD.fetch_xor(bit, Relaxed);
                    send_key(&link, key, down);
                }
            }
        }
        _ => {}
    }
    null_mut() // swallowed: the input belongs to the other PC
}

// ---------- client ----------

fn cursor() -> Point {
    unsafe {
        let e = CGEventCreate(null());
        let p = CGEventGetLocation(e);
        CFRelease(e);
        p
    }
}

/// What is held down here for the host, so it can be let go, and to fill in what a Mac expects on each event.
#[derive(Default)]
struct Held {
    keys: Vec<u8>,
    buttons: u32,  // bit n: mouse button n
    flags: u64,
    last_click: Option<(Instant, u32, i64)>, // when, which button, how many in a row
}

unsafe fn post(e: Event, flags: u64) {
    if e.is_null() { return }
    CGEventSetFlags(e, flags);
    CGEventPost(0 /*HID*/, e);
    CFRelease(e);
}

fn modifier_flag(key: u16) -> u64 {
    match key { 0x37 | 0x36 => COMMAND, 0x38 | 0x3C => SHIFT, 0x3A | 0x3D => OPTION, 0x3B | 0x3E => CONTROL, _ => 0 }
}

unsafe fn inject_key(h: &mut Held, vk: u8, up: bool) {
    let Some(key) = keymap::from_vk(vk) else { return };
    h.keys.retain(|k| *k != vk);
    if !up { h.keys.push(vk) }
    let flag = modifier_flag(key);
    if up { h.flags &= !flag } else { h.flags |= flag }
    post(CGEventCreateKeyboardEvent(null(), key, !up), h.flags);
}

unsafe fn inject_move(h: &Held, dx: i16, dy: i16) {
    let (l, t, w, hh) = *SCREEN.lock().unwrap();
    let p = cursor();
    let to = Point {
        x: (p.x + dx as f64).clamp(l as f64, (l + w - 1) as f64),
        y: (p.y + dy as f64).clamp(t as f64, (t + hh - 1) as f64),
    };
    // a move with a button down must be a drag, or the Mac sees the button let go
    let (kind, button) = match h.buttons {
        0 => (MOVED, 0),
        b if b & 1 != 0 => (LEFT_DRAG, 0),
        b if b & 2 != 0 => (RIGHT_DRAG, 1),
        b => (OTHER_DRAG, b.trailing_zeros()),
    };
    let e = CGEventCreateMouseEvent(null(), kind, to, button);
    if e.is_null() { return }
    CGEventSetIntegerValueField(e, DELTA_X, dx as i64);
    CGEventSetIntegerValueField(e, DELTA_Y, dy as i64);
    post(e, h.flags);
}

unsafe fn inject_button(h: &mut Held, button: u32, down: bool) {
    let kind = match (button, down) { (0, true) => LEFT_DOWN, (0, false) => LEFT_UP, (1, true) => RIGHT_DOWN, (1, false) => RIGHT_UP, (_, true) => OTHER_DOWN, _ => OTHER_UP };
    if down {
        // macOS doesn't work out double clicks from posted events
        let count = match h.last_click {
            Some((t, b, n)) if b == button && t.elapsed() < Duration::from_millis(500) => n + 1,
            _ => 1,
        };
        h.last_click = Some((Instant::now(), button, count));
        h.buttons |= 1 << button;
    } else {
        h.buttons &= !(1 << button);
    }
    let e = CGEventCreateMouseEvent(null(), kind, cursor(), button);
    if e.is_null() { return }
    CGEventSetIntegerValueField(e, CLICK_COUNT, h.last_click.map_or(1, |c| c.2));
    CGEventSetIntegerValueField(e, BUTTON_NUMBER, button as i64);
    post(e, h.flags);
}

unsafe fn inject_wire(h: &mut Held, bf: u16, data: i16) {
    for (i, (down, up)) in [(0x001, 0x002), (0x004, 0x008), (0x010, 0x020), (0x040, 0x080), (0x100, 0x200)].into_iter().enumerate() {
        if bf & down != 0 { inject_button(h, i as u32, true) }
        if bf & up != 0 { inject_button(h, i as u32, false) }
    }
    let lines = (data as i32 / 120).clamp(-20, 20);
    if bf & 0x400 != 0 { post(CGEventCreateScrollWheelEvent(null(), 1 /*lines*/, 1, lines), h.flags) }
    if bf & 0x800 != 0 { post(CGEventCreateScrollWheelEvent(null(), 1, 2, 0i32, -lines), h.flags) }
}

unsafe fn release(h: &mut Held) {
    for vk in std::mem::take(&mut h.keys) { inject_key(h, vk, true) }
    for b in 0..5 { if h.buttons & 1 << b != 0 { inject_button(h, b, false) } }
}

pub fn client(secret: &str, gen: usize) {
    let _one = ONE.lock().unwrap_or_else(|e| e.into_inner());
    if !live(gen) || !allowed() { return }
    let sock = bind_lan().expect("port 47321 is in use. Is another copy of Roam running?");
    let link = init(secret, sock, CLIENT, None);
    let l = link.clone();
    let pinger = std::thread::spawn(move || while live(gen) {
        std::thread::sleep(Duration::from_millis(500));
        l.send(&Msg::Ping(name()));
    });
    let mut edge = 0u8; // host's edge; we enter and leave through the opposite one
    status("Waiting for the other PC. Type this code there and press Roam from this PC.");
    let mut held = Held::default();
    let (mut active, mut last_seen, mut linked, mut peer) = (false, Instant::now() - Duration::from_secs(3), false, String::new());
    while live(gen) {
        let msg = link.recv();
        if !live(gen) { break }
        if link.mismatch() { status(VERSION_MISMATCH) }
        let (bye, was_linked) = (matches!(msg, Some(Msg::Bye)), linked);
        match &msg {
            Some(Msg::Bye) => last_seen = Instant::now() - Duration::from_secs(3),
            Some(m) => {
                last_seen = Instant::now();
                if let Msg::Ping(n) = m { peer = n.clone() }
            }
            None => {}
        }
        if linked != (last_seen.elapsed() < Duration::from_secs(2)) {
            linked = !linked;
            if linked {
                trust_peer(secret, &peer);
                status("Connected. The other PC's mouse can now come onto this screen.");
                tray(TRAY_AWAY);
                visible(false);
            } else {
                status("The other PC disconnected. Waiting for it to come back...");
                tray(TRAY_OFF);
                visible(true);
                unsafe { release(&mut held) };
                active = false;
            }
        }
        if bye && was_linked { heel() }
        match msg {
            Some(Msg::Enter(e, f)) => unsafe {
                (active, edge) = (true, e ^ 1);
                let p = near_edge(edge, f);
                CGWarpMouseCursorPosition(p);
                arrived(p.x as i32, p.y as i32);
                later(|| crate::brand::play(crate::brand::TAG));
                tray(TRAY_HERE);
            },
            Some(Msg::Leave(_)) => { active = false; unsafe { release(&mut held) }; tray(TRAY_AWAY) }
            Some(Msg::Clip(s)) => later(move || clip_set(&s)),
            Some(Msg::Move(dx, dy)) => unsafe {
                inject_move(&held, dx, dy);
                if active && pushing_out(edge, dx, dy) {
                    if let Some(frac) = at_edge(edge, cursor()) {
                        active = false;
                        release(&mut held);
                        let l = link.clone();
                        later(move || if let Some(c) = clip_get() { l.send(&Msg::Clip(c)) });
                        link.send(&Msg::Leave(frac));
                        tray(TRAY_AWAY);
                    }
                }
            },
            Some(Msg::Button(bf, d)) => unsafe { inject_wire(&mut held, bf, d) },
            Some(Msg::Key { vk, up, .. }) => unsafe { inject_key(&mut held, vk, up) },
            _ => {}
        }
    }
    unsafe { release(&mut held) };
    let _ = pinger.join();
    clean_up();
}

// ---------- clipboard (text only) ----------

/// pbcopy and pbpaste are UTF-8 only when told so.
fn clip_get() -> Option<String> {
    let out = Command::new("/usr/bin/pbpaste").env("LC_CTYPE", "UTF-8").output().ok()?;
    let mut s = String::from_utf8(out.stdout).ok()?;
    if s.is_empty() { return None }
    let mut cut = MAX_CLIP.min(s.len());
    while !s.is_char_boundary(cut) { cut -= 1 }
    s.truncate(cut);
    Some(s)
}

fn clip_set(s: &str) {
    let Ok(mut child) = Command::new("/usr/bin/pbcopy").env("LC_CTYPE", "UTF-8").stdin(Stdio::piped()).spawn() else { return };
    if let Some(mut pipe) = child.stdin.take() { let _ = pipe.write_all(s.as_bytes()); }
    let _ = child.wait();
}
