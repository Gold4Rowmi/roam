//! Windows side: host captures input with low-level hooks + raw input, client injects with SendInput.
use crate::gui::{arrived, heel, status, tray, visible, TRAY_AWAY, TRAY_HERE, TRAY_OFF};
use crate::net::{bind_lan, Link, Msg, CLIENT, HOST, MAX_CLIP, PORT};
use crate::trust;
use std::mem::{size_of, zeroed};
use std::net::{SocketAddr, UdpSocket};
use std::ptr::{copy_nonoverlapping, null, null_mut};
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicUsize, Ordering::Relaxed};
use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};
use windows_sys::Win32::Foundation::{LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows_sys::Win32::System::DataExchange::*;
use windows_sys::Win32::System::Memory::*;
use windows_sys::Win32::System::Threading::{GetCurrentThread, SetThreadPriority, THREAD_PRIORITY_HIGHEST};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
use windows_sys::Win32::UI::Input::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

const CF_UNICODETEXT: u32 = 13;
const VERSION_MISMATCH: &str = "The other PC runs a different Roam version. Update both PCs, then try again.";
pub const BROADCAST: SocketAddr = SocketAddr::new(std::net::IpAddr::V4(std::net::Ipv4Addr::BROADCAST), PORT);

type Job = Box<dyn FnOnce() + Send>;
static SIDE: OnceLock<Sender<Job>> = OnceLock::new();

/// Clipboard and sound run here, in order, never on an input path: moves would queue behind them,
/// and Windows removes a low-level hook that stalls too often.
fn later(job: impl FnOnce() + Send + 'static) {
    if let Some(side) = SIDE.get() { let _ = side.send(Box::new(job)); }
}

/// These threads mostly sleep, so this costs nothing until input arrives, then a busy PC can't hold it up.
fn urgent() { unsafe { SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_HIGHEST); } }

/// This PC's name as the other PC will show it.
pub fn name() -> String { std::env::var("COMPUTERNAME").unwrap_or_else(|_| "this PC".into()) }

/// A connection worked, so each side trusts the other from now on.
fn trust_peer(code: &str, peer: &str) {
    if !peer.is_empty() && peer != name() { trust::remember(code, peer) }
}

static REMOTE: AtomicBool = AtomicBool::new(false);
/// Which edge of the host screen leads to the client: 0 right, 1 left, 2 top, 3 bottom.
static EDGE: AtomicU8 = AtomicU8::new(0);
/// The monitor the other PC sits beside: index into the GUI's list, and its rect (x, y, w, h).
static TARGET: Mutex<(usize, (i32, i32, i32, i32))> = Mutex::new((0, (0, 0, 0, 0)));

fn rect_tuple(r: &RECT) -> (i32, i32, i32, i32) { (r.left, r.top, r.right - r.left, r.bottom - r.top) }

pub fn target() -> (usize, u8) { (TARGET.lock().unwrap().0, EDGE.load(Relaxed)) }

pub fn set_target(i: usize, e: u8, mons: &[RECT]) {
    EDGE.store(e, Relaxed);
    *TARGET.lock().unwrap() = (i, rect_tuple(&mons[i]));
}

pub fn save_target() -> String {
    let (l, t, w, h) = TARGET.lock().unwrap().1;
    format!("{l} {t} {w} {h} {}", EDGE.load(Relaxed))
}

/// Restores a saved "l t w h edge"; falls back to the primary monitor if that monitor is gone.
pub fn load_target(line: &str, mons: &[RECT]) {
    let v: Vec<i32> = line.split_whitespace().filter_map(|x| x.parse().ok()).collect();
    let (rect, e) = if v.len() == 5 { ((v[0], v[1], v[2], v[3]), v[4].clamp(0, 3) as u8) } else { (rect_tuple(&mons[0]), 0) };
    let i = mons.iter().position(|m| rect_tuple(m) == rect).unwrap_or(0);
    set_target(i, e, mons);
}

/// If `pt` is on `edge`, its position along that edge as 0..1.
fn at_edge(edge: u8, pt: POINT) -> Option<f32> {
    let (l, t, w, h) = *SCREEN.lock().unwrap();
    let (along_y, along_x) = ((pt.y - t) as f32 / h as f32, (pt.x - l) as f32 / w as f32);
    // only along this monitor's own span, so a neighbouring monitor's edge doesn't count
    let (in_y, in_x) = (pt.y >= t && pt.y < t + h, pt.x >= l && pt.x < l + w);
    match edge {
        0 if pt.x >= l + w - 1 && in_y => Some(along_y),
        1 if pt.x <= l && in_y => Some(along_y),
        2 if pt.y <= t && in_x => Some(along_x),
        3 if pt.y >= t + h - 1 && in_x => Some(along_x),
        _ => None,
    }
}

/// A point 2px inside `edge`, at position `frac` along it.
fn near_edge(edge: u8, frac: f32) -> (i32, i32) {
    let (l, t, w, h) = *SCREEN.lock().unwrap();
    let (x, y) = (l + (frac * w as f32) as i32, t + (frac * h as f32) as i32);
    match edge { 0 => (l + w - 3, y), 1 => (l + 2, y), 2 => (x, t + 2), _ => (x, t + h - 3) }
}

/// Is a delta pushing outward through `edge`?
fn pushing_out(edge: u8, dx: i16, dy: i16) -> bool {
    match edge { 0 => dx > 0, 1 => dx < 0, 2 => dy < 0, _ => dy > 0 }
}
static LINK: Mutex<Option<Arc<Link>>> = Mutex::new(None);
static SCREEN: Mutex<(i32, i32, i32, i32)> = Mutex::new((0, 0, 0, 0)); // virtual screen x, y, w, h
static LAST_PING: Mutex<Option<Instant>> = Mutex::new(None);
static SESSION: AtomicUsize = AtomicUsize::new(0); // bumping it ends the running connection's threads
static ONE: Mutex<()> = Mutex::new(());            // held for a whole connection: the next waits for the last to finish

/// Taken before each use, never held across a send: the hooks must not wait on anything.
fn link() -> Option<Arc<Link>> { LINK.lock().unwrap().clone() }
fn live(gen: usize) -> bool { SESSION.load(Relaxed) == gen }

/// Called on the UI thread when a role is picked, so a Heel straight after it still counts.
pub fn begin() -> usize { SESSION.fetch_add(1, Relaxed) + 1 }

fn init(secret: &str, sock: UdpSocket, role: u8, peer: Option<SocketAddr>) -> Arc<Link> {
    // host: the chosen monitor; client: its whole desktop
    SIDE.get_or_init(|| {
        let (tx, rx) = channel::<Job>();
        std::thread::spawn(move || for job in rx { job() });
        tx
    });
    let target = TARGET.lock().unwrap().1;
    *SCREEN.lock().unwrap() = if role == HOST && target.2 > 0 { target } else { unsafe {
        (GetSystemMetrics(SM_XVIRTUALSCREEN), GetSystemMetrics(SM_YVIRTUALSCREEN),
         GetSystemMetrics(SM_CXVIRTUALSCREEN), GetSystemMetrics(SM_CYVIRTUALSCREEN))
    }};
    let link = Arc::new(Link::new(secret, sock, role, peer));
    *LINK.lock().unwrap() = Some(link.clone());
    link
}

/// Back to no connection. Last thing a connection does, so nothing can leave input swallowed.
fn clean_up() {
    *LINK.lock().unwrap() = None;
    *LAST_PING.lock().unwrap() = None;
    REMOTE.store(false, Relaxed);
    MODS.store(0, Relaxed);
}

// ---------- host ----------

pub fn host(secret: &str, gen: usize) {
    let _one = ONE.lock().unwrap_or_else(|e| e.into_inner());
    if !live(gen) { return }
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
    urgent(); // this thread runs the hooks every mouse move on this PC passes through
    unsafe {
        let hooks = [SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_hook), null_mut(), 0), SetWindowsHookExW(WH_KEYBOARD_LL, Some(key_hook), null_mut(), 0)];
        // hidden window that receives WM_INPUT; hooks only swallow, raw input gives true deltas
        let cls: Vec<u16> = "STATIC\0".encode_utf16().collect();
        let hwnd = CreateWindowExW(0, cls.as_ptr(), null(), 0, 0, 0, 0, 0, null_mut(), null_mut(), null_mut(), null());
        let rid = RAWINPUTDEVICE { usUsagePage: 1, usUsage: 2, dwFlags: RIDEV_INPUTSINK, hwndTarget: hwnd };
        RegisterRawInputDevices(&rid, 1, size_of::<RAWINPUTDEVICE>() as u32);
        status("Looking for the other PC...");
        let (mut abs_x, mut abs_y) = (None, None);
        let mut msg: MSG = zeroed();
        let tick = SetTimer(null_mut(), 0, 200, None); // an idle GetMessage would never notice a Heel
        while live(gen) && GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {
            if msg.message == WM_INPUT && REMOTE.load(Relaxed) {
                let mut raw: RAWINPUT = zeroed();
                let mut sz = size_of::<RAWINPUT>() as u32;
                GetRawInputData(msg.lParam as _, RID_INPUT, &mut raw as *mut _ as _, &mut sz, size_of::<RAWINPUTHEADER>() as u32);
                if raw.header.dwType == RIM_TYPEMOUSE {
                    let m = raw.data.mouse;
                    let (mut dx, mut dy) = (m.lLastX, m.lLastY);
                    if m.usFlags & 1 != 0 { // MOUSE_MOVE_ABSOLUTE (some touchpads/RDP): 0..65535 coords
                        let (_, _, w, h) = *SCREEN.lock().unwrap();
                        dx = abs_x.map_or(0, |p: i32| (m.lLastX - p) * w / 65535);
                        dy = abs_y.map_or(0, |p: i32| (m.lLastY - p) * h / 65535);
                        (abs_x, abs_y) = (Some(m.lLastX), Some(m.lLastY));
                    }
                    if dx != 0 || dy != 0 {
                        link.send(&Msg::Move(dx.clamp(-32767, 32767) as i16, dy.clamp(-32767, 32767) as i16));
                    }
                    let bf = m.Anonymous.Anonymous.usButtonFlags;
                    if bf != 0 {
                        link.send(&Msg::Button(bf, m.Anonymous.Anonymous.usButtonData as i16));
                    }
                }
            }
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        KillTimer(null_mut(), tick);
        for h in hooks { UnhookWindowsHookEx(h); }
        let gone = RAWINPUTDEVICE { usUsagePage: 1, usUsage: 2, dwFlags: RIDEV_REMOVE, hwndTarget: null_mut() };
        RegisterRawInputDevices(&gone, 1, size_of::<RAWINPUTDEVICE>() as u32);
        DestroyWindow(hwnd);
    }
    let _ = receiver.join();
    let _ = pinger.join();
    clean_up();
}

fn client_up() -> bool {
    LAST_PING.lock().unwrap().is_some_and(|t| t.elapsed() < Duration::from_secs(2))
}

fn host_recv(link: &Link, code: &str, gen: usize) {
    urgent();
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
                let (i, e) = target();
                status(&format!("Connected. Move the mouse off the {} edge of monitor {}.", ["right", "left", "top", "bottom"][e as usize], i + 1));
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
        if bye && was_linked { heel() } // the other PC pressed Heel (or quit): follow it back to the setup screen
        if !linked && !warned && start.elapsed() > Duration::from_secs(3) {
            warned = true;
            status("No answer yet. On the other PC: same code, press Be controlled, and allow it in Windows Firewall.");
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
}

fn hand_over(frac: f32) {
    if !client_up() { return } // cursor just stops at the edge; the status line already says why
    let Some(link) = link() else { return };
    link.send(&Msg::Enter(EDGE.load(Relaxed), frac));
    REMOTE.store(true, Relaxed);
    MODS.store(0, Relaxed);
    later(move || {
        if let Some(c) = clip_get() { link.send(&Msg::Clip(c)) }
        crate::brand::play(crate::brand::ON);   // the cursor crossed over
        status("Roaming on the other PC. Move back through the same edge, or press Ctrl+Alt+Esc (Heel), to return.");
        tray(TRAY_AWAY);
    });
}

fn take_back(frac: f32) {
    if !REMOTE.swap(false, Relaxed) { return }
    let (x, y) = near_edge(EDGE.load(Relaxed), frac);
    unsafe { SetCursorPos(x, y) };
    arrived(x, y);
    later(|| crate::brand::play(crate::brand::OFF));  // and came home
    status("Back on this PC.");
    tray(TRAY_HERE);
}

unsafe extern "system" fn mouse_hook(code: i32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    if code >= 0 {
        if REMOTE.load(Relaxed) { return 1 } // swallow: input goes to the client
        let m = &*(lp as *const MSLLHOOKSTRUCT);
        if wp as u32 == WM_MOUSEMOVE {
            if let Some(frac) = at_edge(EDGE.load(Relaxed), m.pt) {
                hand_over(frac);
                return 1;
            }
        }
    }
    CallNextHookEx(null_mut(), code, wp, lp)
}

static MODS: AtomicU8 = AtomicU8::new(0); // bit0 ctrl, bit1 alt, tracked here because swallowed keys never reach GetAsyncKeyState

unsafe extern "system" fn key_hook(code: i32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    if let (true, true, Some(link)) = (code >= 0, REMOTE.load(Relaxed), link()) {
        let k = &*(lp as *const KBDLLHOOKSTRUCT);
        let up = k.flags & LLKHF_UP != 0;
        let bit = match k.vkCode as u16 { VK_CONTROL | VK_LCONTROL | VK_RCONTROL => 1, VK_MENU | VK_LMENU | VK_RMENU => 2, _ => 0 };
        if bit != 0 { if up { MODS.fetch_and(!bit, Relaxed); } else { MODS.fetch_or(bit, Relaxed); } }
        if k.vkCode as u16 == VK_ESCAPE && !up && MODS.load(Relaxed) == 3 { // Ctrl+Alt+Esc: panic button
            MODS.store(0, Relaxed);
            link.send(&Msg::Leave(0.5)); // tells the client to release its held keys
            take_back(0.5);
            return 1;
        }
        link.send(&Msg::Key {
            vk: k.vkCode as u8, scan: k.scanCode as u8,
            up: k.flags & LLKHF_UP != 0, ext: k.flags & LLKHF_EXTENDED != 0,
        });
        return 1;
    }
    CallNextHookEx(null_mut(), code, wp, lp)
}

// ---------- client ----------

pub fn client(secret: &str, gen: usize) {
    let _one = ONE.lock().unwrap_or_else(|e| e.into_inner());
    if !live(gen) { return }
    // an updated or restarted copy, or the finder, can still hold the port for a moment
    let sock = bind_lan().expect("port 47321 is in use. Is another copy of Roam running?");
    let link = init(secret, sock, CLIENT, None);
    urgent();
    let l = link.clone();
    let pinger = std::thread::spawn(move || while live(gen) {
        std::thread::sleep(Duration::from_millis(500));
        l.send(&Msg::Ping(name()));
    });
    let mut edge = 0u8; // host's edge; we enter and leave through the opposite one
    status("Waiting for the other PC. Type this code there and press Roam from this PC.");
    // what the host currently holds down here, so it can be released if control leaves or the host vanishes
    let mut held_keys: Vec<(u8, u8, bool)> = Vec::new();
    let mut held_buttons = 0u32; // MOUSEEVENTF_*UP flags to send
    // nothing heard yet: start already timed out, so "Connected" waits for a real ping
    let (mut active, mut last_seen, mut linked, mut peer) = (false, Instant::now() - Duration::from_secs(3), false, String::new());
    while live(gen) {
        let msg = link.recv();
        if !live(gen) { break }
        if link.mismatch() { status(VERSION_MISMATCH) }
        let (bye, was_linked) = (matches!(msg, Some(Msg::Bye)), linked);
        match &msg {
            Some(Msg::Bye) => last_seen = Instant::now() - Duration::from_secs(3), // counts as already timed out
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
                release(&mut held_keys, &mut held_buttons);
                active = false;
            }
        }
        if bye && was_linked { heel() } // the other PC pressed Heel (or quit): follow it back to the setup screen
        match msg {
            Some(Msg::Enter(e, f)) => unsafe {
                (active, edge) = (true, e ^ 1);
                let (x, y) = near_edge(edge, f);
                SetCursorPos(x, y);
                arrived(x, y);
                later(|| crate::brand::play(crate::brand::TAG));  // tag jingle: it arrived here
                tray(TRAY_HERE);
            },
            Some(Msg::Leave(_)) => { active = false; release(&mut held_keys, &mut held_buttons); tray(TRAY_AWAY) }
            Some(Msg::Clip(s)) => later(move || clip_set(&s)),
            Some(Msg::Move(dx, dy)) => unsafe {
                mouse(dx as i32, dy as i32, MOUSEEVENTF_MOVE, 0);
                let mut p: POINT = zeroed();
                GetCursorPos(&mut p);
                if active && pushing_out(edge, dx, dy) {
                    if let Some(frac) = at_edge(edge, p) {
                        active = false;
                        release(&mut held_keys, &mut held_buttons);
                        let l = link.clone();
                        later(move || if let Some(c) = clip_get() { l.send(&Msg::Clip(c)) });
                        link.send(&Msg::Leave(frac));
                        tray(TRAY_AWAY);
                    }
                }
            },
            Some(Msg::Button(bf, d)) => {
                let (flags, data) = button_flags(bf, d);
                // down flag -> its matching up flag is one bit higher
                held_buttons |= (flags & (MOUSEEVENTF_LEFTDOWN | MOUSEEVENTF_RIGHTDOWN | MOUSEEVENTF_MIDDLEDOWN)) << 1;
                held_buttons &= !(flags & (MOUSEEVENTF_LEFTUP | MOUSEEVENTF_RIGHTUP | MOUSEEVENTF_MIDDLEUP));
                unsafe { mouse(0, 0, flags, data) };
            }
            Some(Msg::Key { vk, scan, up, ext }) => {
                held_keys.retain(|k| k.0 != vk);
                if !up { held_keys.push((vk, scan, ext)) }
                unsafe { key(vk, scan, up, ext) };
            }
            _ => {}
        }
    }
    release(&mut held_keys, &mut held_buttons);
    let _ = pinger.join();
    clean_up();
}

fn release(keys: &mut Vec<(u8, u8, bool)>, buttons: &mut u32) {
    for (vk, scan, ext) in keys.drain(..) { unsafe { key(vk, scan, true, ext) } }
    if *buttons != 0 { unsafe { mouse(0, 0, *buttons, 0) } }
    *buttons = 0;
}

unsafe fn key(vk: u8, scan: u8, up: bool, ext: bool) {
    let mut i: INPUT = zeroed();
    i.r#type = INPUT_KEYBOARD;
    i.Anonymous.ki = KEYBDINPUT {
        wVk: vk as u16, wScan: scan as u16,
        dwFlags: (if up { KEYEVENTF_KEYUP } else { 0 }) | (if ext { KEYEVENTF_EXTENDEDKEY } else { 0 }),
        time: 0, dwExtraInfo: 0,
    };
    SendInput(1, &i, size_of::<INPUT>() as i32);
}

unsafe fn mouse(dx: i32, dy: i32, flags: u32, data: i32) {
    let mut i: INPUT = zeroed();
    i.r#type = INPUT_MOUSE;
    i.Anonymous.mi = MOUSEINPUT { dx, dy, mouseData: data as _, dwFlags: flags, time: 0, dwExtraInfo: 0 };
    SendInput(1, &i, size_of::<INPUT>() as i32);
}

/// Raw-input RI_MOUSE_* flags -> SendInput MOUSEEVENTF_* flags + mouseData.
fn button_flags(bf: u16, wheel: i16) -> (u32, i32) {
    let (mut f, mut data) = (0, 0);
    for (raw, ev, d) in [
        (0x001, MOUSEEVENTF_LEFTDOWN, 0), (0x002, MOUSEEVENTF_LEFTUP, 0),
        (0x004, MOUSEEVENTF_RIGHTDOWN, 0), (0x008, MOUSEEVENTF_RIGHTUP, 0),
        (0x010, MOUSEEVENTF_MIDDLEDOWN, 0), (0x020, MOUSEEVENTF_MIDDLEUP, 0),
        (0x040, MOUSEEVENTF_XDOWN, 1), (0x080, MOUSEEVENTF_XUP, 1),
        (0x100, MOUSEEVENTF_XDOWN, 2), (0x200, MOUSEEVENTF_XUP, 2),
        (0x400, MOUSEEVENTF_WHEEL, wheel as i32), (0x800, MOUSEEVENTF_HWHEEL, wheel as i32),
    ] {
        if bf & raw != 0 { f |= ev; data = d }
    }
    (f, data)
}

// ---------- clipboard (text only) ----------

fn clip_get() -> Option<String> {
    unsafe {
        if OpenClipboard(null_mut()) == 0 { return None }
        let h = GetClipboardData(CF_UNICODETEXT);
        let mut out = None;
        if !h.is_null() {
            let p = GlobalLock(h) as *const u16;
            if !p.is_null() {
                let mut n = 0;
                while *p.add(n) != 0 { n += 1 }
                let mut s = String::from_utf16_lossy(std::slice::from_raw_parts(p, n));
                let mut cut = MAX_CLIP.min(s.len());
                while !s.is_char_boundary(cut) { cut -= 1 }
                s.truncate(cut);
                out = Some(s);
                GlobalUnlock(h);
            }
        }
        CloseClipboard();
        out
    }
}

fn clip_set(s: &str) {
    unsafe {
        if OpenClipboard(null_mut()) == 0 { return }
        EmptyClipboard();
        let w: Vec<u16> = s.encode_utf16().chain([0]).collect();
        let h = GlobalAlloc(GMEM_MOVEABLE, w.len() * 2);
        if !h.is_null() {
            copy_nonoverlapping(w.as_ptr(), GlobalLock(h) as *mut u16, w.len());
            GlobalUnlock(h);
            SetClipboardData(CF_UNICODETEXT, h);
        }
        CloseClipboard();
    }
}
