//! The ROWMI Roam window on the Mac: boot screen, Mint header with Rommel, pairing code, your displays
//! with a draggable "Other" box, buttons, a status line and a menu bar item. One view draws everything,
//! as gui.rs does on Windows, at the same sizes; only the code box is a native control.
//! Worker threads write to PENDING; a 55 ms timer on the main thread applies it and steps the animations.
//! shortcut: no typing filter in the code box (a bad code bonks on press), Tab reaches only the code
//! box, and no bubbles egg. Add them if the Mac version earns its keep.
use crate::brand::{self, Face, BLACK, BLUE, DANGER, GREY, GRID, LIGHT, MINT, MUTED, PAPER, SUNKEN, TAN, WORD};
use crate::{net, trust, update, win};
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObjectProtocol, ProtocolObject};
use objc2::{define_class, msg_send, sel, AnyThread, DefinedClass, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::*;
use objc2_foundation::*;
use std::cell::OnceCell;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};
use std::sync::Mutex;
use std::time::{Duration, Instant};

pub const TRAY_OFF: u8 = 0;
pub const TRAY_HERE: u8 = 1;
pub const TRAY_AWAY: u8 = 2;

const WIDTH: f64 = 360.0;
const HEIGHT: f64 = 530.0;
const ROW: f64 = 52.0; // 44 tall plus an 8 gap
const MAX_PEERS: usize = 3;
const PANEL: Rect = (16.0, 168.0, 328.0, 180.0);
const SETUP_HELP: &str = "Drag the mint box next to the display edge the other PC sits beside, then pick what this Mac does.";
const TAGLINE: &str = "Playful tools with a lot of bark.";
const DEDICATION: &str = "Made in memory of Rommel.";
const ALPHABET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
const BONK: [f64; 5] = [-8.0, 8.0, -5.0, 3.0, 0.0];
const SPARK_DIRS: [(f64, f64); 8] = [(0.0, -1.0), (1.0, -1.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0), (-1.0, 1.0), (-1.0, 0.0), (-1.0, -1.0)];
const SPARK_SIZE: f64 = 104.0;

type Rect = (f64, f64, f64, f64);
type Mon = (i32, i32, i32, i32);

#[derive(Clone, Copy, PartialEq)]
enum Id { Host, Client, Stop, Update, Peer(usize), Back }

#[derive(Default)]
struct Pending { text: Option<String>, tray: Option<u8>, visible: Option<bool>, offer: bool, close: bool, heel: bool, arrived: Option<(i32, i32)> }

struct Ui {
    boot: i32, // ticks since launch, 0 when done
    started: bool,
    face: Face,
    face_until: Option<Instant>,
    tray: u8,
    offline: bool,
    heeled: bool,
    err: bool,
    bonk: usize, // frame showing, 1..=5, 0 at rest
    text: String,
    update_on: bool,
    update_busy: bool,
    peers: Vec<net::Found>,
    about: bool,
    drag: Option<((f64, f64), (f64, f64))>, // box top-left + grab offset
    pressed: Option<Id>,
    bar: usize,
    spark: Option<(i32, u8)>, // frame, and which sparks take the long steps
    mons: Vec<Mon>,
}

static PENDING: Mutex<Option<Pending>> = Mutex::new(None);
static PEERS: Mutex<Option<Vec<net::Found>>> = Mutex::new(None); // set by the finder thread, taken on the tick
static FINDER: AtomicUsize = AtomicUsize::new(0);
static UI: Mutex<Option<Ui>> = Mutex::new(None);
static SPARK_NOW: Mutex<(i32, u8)> = Mutex::new((0, 0));

fn pending(f: impl FnOnce(&mut Pending)) { f(PENDING.lock().unwrap().get_or_insert_with(Pending::default)) }

pub fn status(s: &str) { pending(|p| p.text = Some(s.to_string())) }
pub fn tray(state: u8) { pending(|p| p.tray = Some(state)) }
pub fn visible(on: bool) { pending(|p| p.visible = Some(on)) }
pub fn offer() { pending(|p| p.offer = true) }
pub fn close() { pending(|p| p.close = true) }
pub fn heel() { pending(|p| p.heel = true) }
pub fn arrived(x: i32, y: i32) { pending(|p| p.arrived = Some((x, y))) }

// ---------- drawing ----------

fn rect((x, y, w, h): Rect) -> NSRect { NSRect::new(NSPoint::new(x, y), NSSize::new(w, h)) }

/// The brand colours are COLORREFs, 0x00BBGGRR.
fn colour(c: u32) -> Retained<NSColor> {
    let part = |shift: u32| ((c >> shift) & 0xFF) as f64 / 255.0;
    NSColor::colorWithSRGBRed_green_blue_alpha(part(0), part(8), part(16), 1.0)
}

fn fill(r: Rect, c: u32) {
    colour(c).set();
    NSRectFill(rect(r));
}

fn rounded(r: Rect, face: u32, border: u32, radius: f64) {
    let path = NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(rect((r.0 + 0.5, r.1 + 0.5, r.2 - 1.0, r.3 - 1.0)), radius, radius);
    colour(face).set();
    path.fill();
    colour(border).set();
    path.stroke();
}

/// The brand frame: 1 px border, 2 px hard drop, radius 6. The only depth there is.
fn frame(r: Rect, face: u32, border: u32, drop: u32) {
    rounded((r.0 + 2.0, r.1 + 2.0, r.2, r.3), drop, drop, 6.0);
    rounded(r, face, border, 6.0);
}

fn rommel(x: f64, y: f64, u: f64, f: Face) {
    for (rx, ry, w, h, c) in brand::rommel_rects(f, &LIGHT) {
        fill((x + rx as f64 * u, y + ry as f64 * u, w as f64 * u, h as f64 * u), c);
    }
}

fn wordmark(x: f64, y: f64, u: f64, c: u32) {
    for (rx, ry, w, h) in WORD { fill((x + rx as f64 * u, y + ry as f64 * u, w as f64 * u, h as f64 * u), c) }
}

fn font(names: &[&str], size: f64) -> Retained<NSFont> {
    names.iter().find_map(|n| NSFont::fontWithName_size(&NSString::from_str(n), size))
        .unwrap_or_else(|| NSFont::monospacedSystemFontOfSize_weight(size, unsafe { NSFontWeightRegular }))
}
fn pixel(size: f64) -> Retained<NSFont> { font(&["Silkscreen-Regular", "Silkscreen"], size) }
fn plex(size: f64) -> Retained<NSFont> { font(&["IBMPlexMono", "IBM Plex Mono"], size) }
fn plex_bold(size: f64) -> Retained<NSFont> { font(&["IBMPlexMono-SemiBold", "IBM Plex Mono SemiBold"], size) }

fn attrs(f: &NSFont, c: u32) -> Retained<NSDictionary<NSAttributedStringKey, AnyObject>> {
    let ink = colour(c);
    unsafe {
        let keys: [&NSAttributedStringKey; 2] = [NSFontAttributeName, NSForegroundColorAttributeName];
        NSDictionary::from_slices(&keys, &[f as &AnyObject, &*ink as &AnyObject])
    }
}

/// Text from (x, y), or centred in `r` when `center`, wrapping inside r.
fn text(r: Rect, f: &NSFont, s: &str, c: u32, center: bool) {
    let (s, a) = (NSString::from_str(s), attrs(f, c));
    unsafe {
        if center {
            let size = s.sizeWithAttributes(Some(&a));
            s.drawAtPoint_withAttributes(NSPoint::new(r.0 + (r.2 - size.width) / 2.0, r.1 + (r.3 - size.height) / 2.0), Some(&a));
        } else {
            s.drawInRect_withAttributes(rect(r), Some(&a));
        }
    }
}

/// Silkscreen stays square only with smoothing off.
fn pixel_text(r: Rect, size: f64, s: &str, c: u32, center: bool) {
    let context = NSGraphicsContext::currentContext();
    if let Some(g) = &context { g.setShouldAntialias(false) }
    text(r, &pixel(size), s, c, center);
    if let Some(g) = &context { g.setShouldAntialias(true) }
}

fn draw_boot(tick: i32, h: f64) {
    let (u, x, y) = (8.0, 24.0, 200.0);
    fill((0.0, 0.0, WIDTH, h), PAPER);
    let rows = match tick { ..=9 => 0, 10 => 13, 11 => 26, _ => 40 };
    if rows > 0 { fill((0.0, h - h * rows as f64 / 40.0, WIDTH, h), MINT) }
    let f = if (10..15).contains(&tick) { Face::Alert } else { Face::Idle };
    rommel(x, y, u, f);
    if f == Face::Alert {
        fill((x + 4.0 * u, y - 6.0 * u, 2.0 * u, 3.0 * u), TAN);
        fill((x + 4.0 * u, y - 2.0 * u, 2.0 * u, u), TAN);
    }
    let drop = match tick { 1 | 2 => -18.0, 3 | 4 => -12.0, 5 | 6 => -6.0, 7 => 1.0, _ => 0.0 };
    wordmark(x + 12.0 * u, y + (4.0 + drop) * u, u, BLACK);
    if tick >= 17 { pixel_text((x + 12.0 * u, y + 12.0 * u, 216.0, 32.0), 24.0, "Roam", BLACK, false) }
}

fn band() {
    fill((0.0, 0.0, WIDTH, 64.0), MINT);
    fill((0.0, 64.0, WIDTH, 1.0), GREY);
}

fn draw_button(ui: &Ui, id: Id, r: Rect, label: &str) {
    let primary = id == Id::Host;
    let pressed = ui.pressed == Some(id);
    let disabled = id == Id::Update && ui.update_busy;
    let (face, ink, border) = match (primary, disabled) {
        (_, true) => (SUNKEN, MUTED, GREY),
        (true, _) => (BLACK, SUNKEN, BLACK),
        (false, _) => (PAPER, BLACK, GREY),
    };
    let body = (r.0, r.1, r.2 - 2.0, r.3 - 2.0);
    if pressed || disabled {
        let body = if pressed { (body.0 + 2.0, body.1 + 2.0, body.2, body.3) } else { body };
        rounded(body, face, border, 6.0);
    } else {
        frame(body, face, border, if primary { MUTED } else { GREY });
    }
    let at = if pressed { (body.0 + 2.0, body.1 + 2.0, body.2, body.3) } else { body };
    text(at, &plex(15.0), label, ink, true);
    if primary {
        let a = (at.0 + 12.0, at.1 + at.3 / 2.0 - 10.0, 20.0, 20.0);
        rounded(a, MINT, MINT, 10.0);
        pixel_text(a, 16.0, "A", BLACK, true);
    }
}

fn buttons(ui: &Ui) -> Vec<(Id, Rect, String)> {
    let at = |y: f64| (16.0, y, 328.0, 44.0);
    if ui.about { return vec![(Id::Back, at(height(ui) - 60.0), "Back".into())] }
    let mut v = if ui.started { vec![(Id::Stop, at(364.0), "Heel".to_string())] }
        else { vec![(Id::Host, at(364.0), "Roam from this PC".to_string()), (Id::Client, at(416.0), "Be controlled".to_string())] };
    let mut y = 468.0;
    for (i, f) in ui.peers.iter().take(MAX_PEERS).enumerate() {
        v.push((Id::Peer(i), at(y), if f.wants { format!("Allow {}", f.name) } else { format!("Roam onto {}", f.name) }));
        y += ROW;
    }
    if ui.update_on { v.push((Id::Update, at(y), "Update".to_string())) }
    v
}

fn height(ui: &Ui) -> f64 {
    HEIGHT + (ui.peers.len().min(MAX_PEERS) + ui.update_on as usize) as f64 * ROW
}

/// Maps display points into the panel: all displays plus room for the other PC on any side.
struct Map { s: f64, ox: f64, oy: f64 }

fn map(mons: &[Mon]) -> Map {
    let p = mons[0];
    let (l, t) = (mons.iter().map(|m| m.0).min().unwrap() - p.2, mons.iter().map(|m| m.1).min().unwrap() - p.3);
    let (r, b) = (mons.iter().map(|m| m.0 + m.2).max().unwrap() + p.2, mons.iter().map(|m| m.1 + m.3).max().unwrap() + p.3);
    let (w, h) = ((r - l) as f64, (b - t) as f64);
    let s = ((PANEL.2 - 16.0) / w).min((PANEL.3 - 16.0) / h);
    Map { s, ox: PANEL.0 + (PANEL.2 - w * s) / 2.0 - l as f64 * s, oy: PANEL.1 + (PANEL.3 - h * s) / 2.0 - t as f64 * s }
}

impl Map {
    fn to_panel(&self, m: Mon) -> Rect {
        ((m.0 as f64 * self.s + self.ox).floor() + 2.0, (m.1 as f64 * self.s + self.oy).floor() + 2.0,
         (m.2 as f64 * self.s).floor() - 4.0, (m.3 as f64 * self.s).floor() - 4.0)
    }
}

/// Where the other PC would sit, docked on display `m`'s edge `e`, at the size of the main display.
fn docked(p: Mon, m: Mon, e: u8) -> Mon {
    let (x, y) = match e { 0 => (m.0 + m.2, m.1), 1 => (m.0 - p.2, m.1), 2 => (m.0, m.1 - p.3), _ => (m.0, m.1 + m.3) };
    (x, y, p.2, p.3)
}

fn other_box(ui: &Ui, m: &Map) -> Rect {
    if let Some((pos, _)) = ui.drag {
        let d = m.to_panel(docked(ui.mons[0], ui.mons[0], 0));
        return (pos.0, pos.1, d.2, d.3);
    }
    let (at, e) = win::target();
    m.to_panel(docked(ui.mons[0], if at.2 > 0 { at } else { ui.mons[0] }, e))
}

fn labelled(r: Rect, face: u32, label: &str, dropped: bool) {
    if dropped { frame(r, face, GREY, GREY) } else { rounded(r, face, GREY, 6.0) }
    pixel_text(r, 16.0, label, BLACK, true);
}

fn draw_panel(ui: &Ui) {
    frame(PANEL, PAPER, GREY, GREY);
    let (x, y, w, h) = (PANEL.0 + 2.0, PANEL.1 + 2.0, PANEL.2 - 4.0, PANEL.3 - 4.0);
    let mut at = 8.0;
    while at < w { fill((x + at, y, 1.0, h), GRID); at += 8.0 }
    at = 8.0;
    while at < h { fill((x, y + at, w, 1.0), GRID); at += 8.0 }
    let m = map(&ui.mons);
    for (i, r) in ui.mons.iter().enumerate() { labelled(m.to_panel(*r), PAPER, &(i + 1).to_string(), true) }
    labelled(other_box(ui, &m), MINT, "Other", ui.drag.is_none());
}

fn draw_window(ui: &Ui) {
    let h = height(ui);
    if ui.boot > 0 { return draw_boot(ui.boot, h) }
    fill((0.0, 0.0, WIDTH, h), PAPER);
    band();
    if ui.about { // the wordmark and the three About lines appear only here
        let (x, y, u) = (16.0, 17.0, 3.0);
        rommel(x, y, u, Face::Idle);
        wordmark(x + 12.0 * u, y + 4.0 * u, u, BLACK);
        pixel_text((x + 39.0 * u + 12.0, y + 6.0 * u - 2.0, 160.0, 20.0), 16.0, "Roam", BLACK, false);
        text((16.0, 84.0, 328.0, 20.0), &plex(14.0), &format!("Version {}", update::VERSION), BLACK, false);
        text((16.0, h - 112.0, 328.0, 20.0), &plex(14.0), TAGLINE, BLACK, false);
        text((16.0, h - 88.0, 328.0, 16.0), &plex(12.0), DEDICATION, MUTED, false);
    } else {
        rommel(16.0, 12.0, 4.0, ui.face);
        pixel_text((64.0, 18.0, 280.0, 40.0), 32.0, "Roam", BLACK, false);
        if ui.started && ui.tray == TRAY_OFF { // launch bar while looking for the other PC
            fill((0.0, 60.0, WIDTH, 4.0), BLACK);
            fill(((ui.bar % 40) as f64 * 8.0, 60.0, 40.0, 4.0), MINT);
        }
        let dx = bonk_dx(ui);
        pixel_text((16.0 + dx, 76.0, 328.0, 20.0), 16.0, "Pairing code", BLACK, false);
        pixel_text((16.0, 148.0, 328.0, 20.0), 16.0, "Where is the other PC", BLACK, false);
        // the code box is the only thing here that takes focus, and it is read-only once a role runs
        frame((16.0 + dx, 96.0, 328.0, 44.0), PAPER, if ui.err { DANGER } else if ui.started { GREY } else { BLUE }, GREY);
        draw_panel(ui);
        let y = 466.0 + (height(ui) - HEIGHT);
        text((16.0, y, 328.0, 56.0), &plex(14.0), &ui.text, if ui.err { DANGER } else { BLACK }, false);
    }
    for (id, r, label) in buttons(ui) { draw_button(ui, id, r, &label) }
}

fn bonk_dx(ui: &Ui) -> f64 { if ui.bonk > 0 { BONK[ui.bonk - 1] } else { 0.0 } }

/// The menu bar icon: Rommel on a colour. Mint = mouse here, Paper = on the other PC, grey = not connected.
fn tray_image(state: u8) -> Retained<NSImage> {
    let (shell, f) = match state { TRAY_HERE => (MINT, Face::Idle), TRAY_AWAY => (PAPER, Face::Idle), _ => (GREY, Face::Sleepy) };
    let image = NSImage::initWithSize(NSImage::alloc(), NSSize::new(24.0, 24.0));
    #[allow(deprecated)]
    image.lockFocus();
    fill((0.0, 0.0, 24.0, 24.0), shell);
    for (rx, ry, w, h, c) in brand::rommel_rects(f, &LIGHT) { // an image's origin is bottom-left
        fill(((rx * 2 + 2) as f64, (24 - (ry + h) * 2 - 2) as f64, (w * 2) as f64, (h * 2) as f64), c);
    }
    #[allow(deprecated)]
    image.unlockFocus();
    image
}

// ---------- the view ----------

struct Parts {
    field: OnceCell<Retained<NSTextField>>,
    window: OnceCell<Retained<NSWindow>>,
    item: OnceCell<Retained<NSStatusItem>>,
    spark: OnceCell<Retained<NSWindow>>,
}

define_class!(
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[ivars = Parts]
    struct Canvas;

    impl Canvas {
        #[unsafe(method(isFlipped))]
        fn flipped(&self) -> bool { true }

        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            if let Some(ui) = UI.lock().unwrap().as_ref() { draw_window(ui) }
        }

        #[unsafe(method(mouseDown:))]
        fn mouse_down(&self, e: &NSEvent) { self.press(self.at(e)) }

        #[unsafe(method(mouseDragged:))]
        fn mouse_dragged(&self, e: &NSEvent) { self.dragged(self.at(e)) }

        #[unsafe(method(mouseUp:))]
        fn mouse_up(&self, e: &NSEvent) { self.released(self.at(e)) }

        #[unsafe(method(rightMouseDown:))]
        fn right_down(&self, e: &NSEvent) { self.forget_menu(self.at(e), e) }

        #[unsafe(method(tick:))]
        fn tick(&self, _t: &NSTimer) { self.step() }

        #[unsafe(method(showRoam:))]
        fn show_roam(&self, _s: Option<&AnyObject>) { brand::play(brand::CLICK); self.bring_up() }

        #[unsafe(method(heelRoam:))]
        fn heel_roam(&self, _s: Option<&AnyObject>) { if with(|ui| ui.started) { brand::play(brand::BACK); self.stop() } }

        #[unsafe(method(aboutRoam:))]
        fn about_roam(&self, _s: Option<&AnyObject>) {
            brand::play(brand::CLICK);
            self.bring_up();
            with(|ui| ui.about = true);
            self.field().setHidden(true);
            self.fit();
        }

        #[unsafe(method(quitRoam:))]
        fn quit_roam(&self, _s: Option<&AnyObject>) { self.quit() }

        #[unsafe(method(forgetPeer:))]
        fn forget_peer(&self, sender: &NSMenuItem) {
            let i = sender.tag() as usize;
            let Some(f) = with(|ui| ui.peers.get(i).cloned()) else { return };
            trust::forget(&f.name);
            forget_saved_code(&f.code);
            if saved_code().is_none() { self.field().setStringValue(&NSString::from_str("")) }
            with(|ui| ui.peers.retain(|x| x.name != f.name));
            start_finder();
            self.peers_changed();
            brand::play(brand::OFF);
        }
    }

    unsafe impl NSObjectProtocol for Canvas {}

    unsafe impl NSWindowDelegate for Canvas {
        #[unsafe(method(windowShouldClose:))]
        fn should_close(&self, _w: &NSWindow) -> bool {
            if with(|ui| ui.started) { self.window().orderOut(None) } else { self.quit() }
            false // running: closing only hides it, Heel and Quit live in the menu bar
        }
    }
);

fn with<T>(f: impl FnOnce(&mut Ui) -> T) -> T { f(UI.lock().unwrap().as_mut().unwrap()) }

define_class!(
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    struct SparkView;

    impl SparkView {
        #[unsafe(method(isFlipped))]
        fn flipped(&self) -> bool { true }

        // the brand's pop sparkle: eight tan squares burst from the middle, 4 steps of 8 or 16 px
        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            let (frame_at, far) = *SPARK_NOW.lock().unwrap();
            NSColor::clearColor().set();
            NSRectFill(rect((0.0, 0.0, SPARK_SIZE, SPARK_SIZE)));
            for (i, (dx, dy)) in SPARK_DIRS.iter().enumerate() {
                let off = frame_at as f64 * if far >> i & 1 == 1 { 16.0 } else { 8.0 };
                fill((SPARK_SIZE / 2.0 + dx * off - 4.0, SPARK_SIZE / 2.0 + dy * off - 4.0, 8.0, 8.0), TAN);
            }
        }
    }
);

impl Canvas {
    fn field(&self) -> &NSTextField { self.ivars().field.get().unwrap() }
    fn window(&self) -> &NSWindow { self.ivars().window.get().unwrap() }

    fn at(&self, e: &NSEvent) -> (f64, f64) {
        let p = self.convertPoint_fromView(e.locationInWindow(), None);
        (p.x, p.y)
    }

    fn redraw(&self) { self.setNeedsDisplay(true) }

    fn bring_up(&self) {
        self.window().makeKeyAndOrderFront(None);
        #[allow(deprecated)]
        NSApplication::sharedApplication(self.mtm()).activateIgnoringOtherApps(true);
    }

    /// Sizes the window to its rows, keeping its top edge still (a window's origin is bottom-left).
    fn fit(&self) {
        let h = with(|ui| height(ui));
        let w = self.window();
        let old = w.contentRectForFrameRect(w.frame());
        let top = old.origin.y + old.size.height;
        let content = NSRect::new(NSPoint::new(old.origin.x, top - h), NSSize::new(WIDTH, h));
        w.setFrame_display(w.frameRectForContentRect(content), true);
        self.setFrame(rect((0.0, 0.0, WIDTH, h)));
        self.redraw();
    }

    fn place_code(&self) {
        let dx = with(|ui| bonk_dx(ui));
        self.field().setFrame(rect((20.0 + dx, 104.0, 320.0, 28.0)));
    }

    fn react(&self, f: Face, ms: u64) {
        with(|ui| {
            ui.face = f;
            ui.face_until = Some(Instant::now() + Duration::from_millis(ms));
        });
        self.redraw();
    }

    fn bonk(&self) {
        brand::play(brand::ERROR);
        with(|ui| { ui.err = true; ui.bonk = 1 });
        self.place_code();
        self.redraw();
    }

    fn set_tray(&self, state: u8) {
        if let Some(b) = self.ivars().item.get().and_then(|i| i.button(self.mtm())) { b.setImage(Some(&tray_image(state))) }
    }

    fn peers_changed(&self) {
        let fresh = PEERS.lock().unwrap().take();
        let arrived = with(|ui| {
            let Some(list) = fresh else { return false };
            let new = list.iter().any(|f| !ui.peers.contains(f));
            ui.peers = list;
            new
        });
        with(|ui| {
            let help = match ui.peers.iter().find(|f| f.wants) {
                Some(f) => format!("{} wants to use this PC's mouse and keyboard. Press Allow, or ignore it.", f.name),
                None if ui.peers.is_empty() => SETUP_HELP.to_string(),
                None => "Found a trusted PC. Press its button, or right-click it to forget it.".to_string(),
            };
            ui.text = if ui.heeled { format!("Heeled. {help}") } else { help };
        });
        self.fit();
        if arrived {
            brand::play(brand::TAG);
            self.react(Face::Alert, 800);
        }
    }

    /// A press sits the button into its drop, or starts dragging the Other box, or taps Rommel.
    fn press(&self, (x, y): (f64, f64)) {
        let inside = |r: Rect| x >= r.0 && x < r.0 + r.2 && y >= r.1 && y < r.1 + r.3;
        let boot = with(|ui| ui.boot);
        if boot >= 6 { self.end_boot() } // any input after 300 ms skips
        if boot > 0 { return }
        let tapped = with(|ui| {
            if let Some((id, _, _)) = buttons(ui).into_iter().find(|b| inside(b.1)) {
                if !(id == Id::Update && ui.update_busy) { ui.pressed = Some(id) }
                return false;
            }
            if ui.about || ui.started { return false }
            let b = other_box(ui, &map(&ui.mons));
            if inside(b) {
                ui.drag = Some(((b.0, b.1), (x - b.0, y - b.1)));
                brand::play(brand::ON);
                return false;
            }
            inside((16.0, 12.0, 40.0, 40.0))
        });
        if tapped {
            brand::play(brand::WOOF);
            self.react(Face::Alert, 500);
        }
        self.redraw();
    }

    fn dragged(&self, (x, y): (f64, f64)) {
        with(|ui| {
            let (b, Some((_, off))) = (other_box(ui, &map(&ui.mons)), ui.drag) else { return };
            ui.drag = Some(((
                (x - off.0).clamp(PANEL.0, PANEL.0 + PANEL.2 - b.2),
                (y - off.1).clamp(PANEL.1, PANEL.1 + PANEL.3 - b.3),
            ), off));
        });
        self.redraw();
    }

    fn released(&self, (x, y): (f64, f64)) {
        let dropped = with(|ui| {
            let m = map(&ui.mons);
            let b = other_box(ui, &m);
            let Some(((px, py), _)) = ui.drag.take() else { return false };
            let (cx, cy) = (px + b.2 / 2.0, py + b.3 / 2.0);
            let mut best = (f64::MAX, ui.mons[0], 0u8);
            for mon in &ui.mons {
                for e in 0..4u8 {
                    let r = m.to_panel(docked(ui.mons[0], *mon, e));
                    let (dx, dy) = (r.0 + r.2 / 2.0 - cx, r.1 + r.3 / 2.0 - cy);
                    if dx * dx + dy * dy < best.0 { best = (dx * dx + dy * dy, *mon, e) }
                }
            }
            win::set_target(best.1, best.2);
            true
        });
        if dropped { brand::play(brand::CONFIRM) }
        let hit = with(|ui| {
            let p = ui.pressed.take()?;
            buttons(ui).into_iter().find(|b| b.0 == p && x >= b.1.0 && x < b.1.0 + b.1.2 && y >= b.1.1 && y < b.1.1 + b.1.3).map(|_| p)
        });
        self.redraw();
        match hit {
            Some(Id::Host) | Some(Id::Client) => self.start(hit == Some(Id::Host)),
            Some(Id::Stop) => { brand::play(brand::BACK); self.stop() }
            Some(Id::Back) => {
                brand::play(brand::BACK);
                with(|ui| ui.about = false);
                self.field().setHidden(false);
                self.fit();
            }
            Some(Id::Update) => {
                brand::play(brand::CONFIRM);
                with(|ui| ui.update_busy = true);
                update::install();
            }
            Some(Id::Peer(i)) => {
                if let Some(f) = with(|ui| ui.peers.get(i).cloned()) {
                    self.field().setStringValue(&NSString::from_str(&pretty(&f.code)));
                    self.start(!f.wants);
                }
            }
            None => {}
        }
    }

    fn forget_menu(&self, (x, y): (f64, f64), e: &NSEvent) {
        let hit = with(|ui| buttons(ui).into_iter().find(|b| x >= b.1.0 && x < b.1.0 + b.1.2 && y >= b.1.1 && y < b.1.1 + b.1.3).map(|b| b.0));
        let Some(Id::Peer(i)) = hit else { return };
        let Some(f) = with(|ui| ui.peers.get(i).cloned()) else { return };
        let mtm = self.mtm();
        let menu = NSMenu::new(mtm);
        let item = menu_item(mtm, &format!("Forget {}", f.name), Some(sel!(forgetPeer:)), "");
        unsafe { item.setTarget(Some(self)) };
        item.setTag(i as isize);
        menu.addItem(&item);
        NSMenu::popUpContextMenu_withEvent_forView(&menu, e, self);
    }

    fn start(&self, host: bool) {
        let mut code: String = self.field().stringValue().to_string().to_uppercase().chars().filter(char::is_ascii_alphanumeric).collect();
        if !host && code.is_empty() { code = gen_code() }
        if code.len() != 10 {
            self.bonk();
            self.react(Face::Sad, 800);
            status("Type the 10-character code shown on the other PC.");
            return;
        }
        brand::play(brand::CONFIRM);
        self.field().setStringValue(&NSString::from_str(&pretty(&code)));
        self.field().setEditable(false);
        let _ = std::fs::create_dir_all(trust::dir());
        let _ = std::fs::write(code_file(), format!("{code}\n{}", win::save_target())); // fine if it can't be saved
        FINDER.fetch_add(1, Relaxed); // a role owns the port now
        *PEERS.lock().unwrap() = None;
        with(|ui| {
            ui.err = false;
            ui.heeled = false;
            ui.started = true;
            ui.peers.clear();
            ui.text = if host { "Looking for the other PC. This window hides once they connect.".into() }
                      else { "Waiting. Type this code on the other PC and press Roam from this PC.".into() };
        });
        self.set_tray(TRAY_OFF);
        self.fit();
        let gen = win::begin();
        std::thread::spawn(move || {
            let r = std::panic::catch_unwind(|| if host { win::host(&code, gen) } else { win::client(&code, gen) });
            if let Err(e) = r {
                let why = e.downcast_ref::<String>().cloned().or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string())).unwrap_or_default();
                brand::play(brand::ERROR);
                status(&format!("Stopped: something went wrong inside ({why}). Press Heel and try again."));
                tray(TRAY_OFF);
            }
        });
    }

    /// Heel: end the connection and put this same window back on the setup screen, saying so.
    fn stop(&self) {
        win::shutdown();
        with(|ui| {
            ui.started = false;
            ui.tray = TRAY_OFF;
            ui.offline = false;
            ui.heeled = true;
            ui.peers.clear();
        });
        self.set_tray(TRAY_OFF);
        self.field().setEditable(true);
        self.bring_up();
        self.window().makeFirstResponder(Some(self.field()));
        start_finder();
        *PEERS.lock().unwrap() = Some(Vec::new());
        self.peers_changed();
        self.react(Face::Happy, 800);
    }

    fn quit(&self) {
        win::shutdown();
        brand::play(brand::OFF);
        NSApplication::sharedApplication(self.mtm()).terminate(None);
    }

    fn end_boot(&self) {
        with(|ui| ui.boot = 0);
        self.field().setHidden(false);
        self.window().makeFirstResponder(Some(self.field()));
        self.redraw();
    }

    fn apply_pending(&self) {
        let Some(p) = PENDING.lock().unwrap().take() else { return };
        if p.offer {
            let first = with(|ui| {
                let first = !ui.update_on;
                ui.update_on = true;
                ui.update_busy = false;
                first
            });
            self.fit();
            if first {
                brand::play(brand::TAG);
                self.react(Face::Alert, 800);
            }
        }
        match p.visible {
            Some(true) => self.bring_up(),
            Some(false) => self.window().orderOut(None),
            None => {}
        }
        if let Some(t) = p.text { with(|ui| ui.text = t) }
        if let Some(s) = p.tray {
            let (was, started) = with(|ui| (std::mem::replace(&mut ui.tray, s), ui.started));
            with(|ui| if s != TRAY_OFF { ui.offline = false } else if was != TRAY_OFF { ui.offline = true });
            if started { self.set_tray(s) }
            if was == TRAY_OFF && s != TRAY_OFF { brand::play(brand::YIP); self.react(Face::Happy, 800) }
            if was != TRAY_OFF && s == TRAY_OFF { brand::play(brand::WHINE); self.react(Face::Sad, 800) }
        }
        if let Some((x, y)) = p.arrived { self.show_spark(x, y) }
        if p.heel && with(|ui| ui.started) { brand::play(brand::BACK); self.stop() }
        if p.close { self.quit() }
        self.redraw();
    }

    /// Bursts at the cursor, pulled in far enough for the whole burst to show.
    fn show_spark(&self, x: i32, y: i32) {
        let Some(t) = self.ivars().spark.get() else { return };
        let mons = win::displays();
        let m = mons.iter().find(|m| x >= m.0 && x < m.0 + m.2 && y >= m.1 && y < m.1 + m.3).copied().unwrap_or(mons[0]);
        let half = SPARK_SIZE / 2.0;
        let cx = (x as f64).clamp(m.0 as f64 + half, (m.0 + m.2) as f64 - half);
        let cy = (y as f64).clamp(m.1 as f64 + half, (m.1 + m.3) as f64 - half);
        let flipped_y = mons[0].3 as f64 - cy; // Quartz counts down from the main display's top, Cocoa up from its bottom
        t.setFrame_display(rect((cx - half, flipped_y - half, SPARK_SIZE, SPARK_SIZE)), false);
        let far = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.subsec_nanos() as u8);
        with(|ui| ui.spark = Some((0, far)));
        *SPARK_NOW.lock().unwrap() = (0, far);
        t.orderFrontRegardless();
        if let Some(v) = t.contentView() { v.setNeedsDisplay(true) }
    }

    fn step(&self) {
        let boot = with(|ui| {
            if ui.boot > 0 { ui.boot += 1 }
            ui.boot
        });
        if boot > 0 {
            if boot > 20 { self.end_boot() } else { self.redraw() }
            return;
        }
        self.apply_pending();
        if PEERS.lock().unwrap().is_some() && !with(|ui| ui.started) { self.peers_changed() }
        let (bonked, settled, searching, spark) = with(|ui| {
            let bonked = ui.bonk > 0;
            if bonked { ui.bonk = if ui.bonk >= BONK.len() { 0 } else { ui.bonk + 1 } }
            let settled = ui.face_until.is_some_and(|t| Instant::now() >= t);
            if settled {
                ui.face_until = None;
                ui.face = if ui.offline { Face::Sleepy } else { Face::Idle };
            }
            let searching = ui.started && ui.tray == TRAY_OFF;
            if searching { ui.bar += 1 }
            let spark = ui.spark.map(|(at, far)| (at + 1, far));
            ui.spark = spark.filter(|s| s.0 < 8); // 4 frames of 2 ticks each, about 105 ms a frame
            (bonked, settled, searching, spark)
        });
        if let (Some((at, far)), Some(t)) = (spark, self.ivars().spark.get()) {
            if at >= 8 { t.orderOut(None) } else {
                *SPARK_NOW.lock().unwrap() = (at / 2, far);
                if let Some(v) = t.contentView() { v.setNeedsDisplay(true) }
            }
        }
        if bonked { self.place_code() }
        if bonked || settled || searching { self.redraw() }
    }
}

// ---------- pairing code ----------

fn code_file() -> PathBuf { trust::dir().join("roam.code") }

fn saved_code() -> Option<String> {
    let saved = std::fs::read_to_string(code_file()).ok()?;
    let code = saved.lines().next()?.trim();
    (code.len() == 10).then(|| code.to_string())
}

fn forget_saved_code(code: &str) {
    if saved_code().as_deref() != Some(code) { return }
    let rest: Vec<String> = std::fs::read_to_string(code_file()).unwrap_or_default().lines().skip(1).map(String::from).collect();
    let _ = std::fs::write(code_file(), format!("\n{}", rest.join("\n")));
}

/// Time-seeded would be guessable: the OS's random source, as on Windows.
fn gen_code() -> String {
    use chacha20poly1305::aead::{rand_core::RngCore, OsRng};
    let mut b = [0u8; 10];
    OsRng.fill_bytes(&mut b);
    b.iter().map(|x| ALPHABET[(*x % 32) as usize] as char).collect()
}

fn pretty(code: &str) -> String {
    if code.len() == 10 { format!("{}-{}", &code[..5], &code[5..]) } else { code.to_string() }
}

/// Finds the PCs this one has connected to, while on the setup screen. Nothing is sent without any.
fn start_finder() {
    let gen = FINDER.fetch_add(1, Relaxed) + 1;
    let mut codes: Vec<String> = trust::load().into_iter().map(|(code, _)| code).collect();
    if let Some(saved) = saved_code() { codes.push(saved) }
    if codes.is_empty() { return }
    codes.sort();
    codes.dedup();
    std::thread::spawn(move || {
        let Some(sock) = net::bind_lan() else { return };
        let _ = sock.set_broadcast(true);
        net::find(sock, win::BROADCAST, &win::name(), &codes, || FINDER.load(Relaxed) != gen, |list| {
            if FINDER.load(Relaxed) == gen { *PEERS.lock().unwrap() = Some(list.to_vec()) }
        });
    });
}

// ---------- window ----------

fn menu_item(mtm: MainThreadMarker, title: &str, action: Option<objc2::runtime::Sel>, key: &str) -> Retained<NSMenuItem> {
    unsafe { NSMenuItem::initWithTitle_action_keyEquivalent(NSMenuItem::alloc(mtm), &NSString::from_str(title), action, &NSString::from_str(key)) }
}

/// The main menu. The Edit menu is what makes paste work in the code box.
fn main_menu(mtm: MainThreadMarker, view: &Canvas) -> Retained<NSMenu> {
    let bar = NSMenu::new(mtm);
    let target = |item: Retained<NSMenuItem>| { unsafe { item.setTarget(Some(view)) }; item };
    let app_menu = NSMenu::new(mtm);
    app_menu.addItem(&target(menu_item(mtm, "About Roam", Some(sel!(aboutRoam:)), "")));
    app_menu.addItem(&target(menu_item(mtm, "Quit Roam", Some(sel!(quitRoam:)), "q")));
    let edit = NSMenu::initWithTitle(NSMenu::alloc(mtm), &NSString::from_str("Edit"));
    for (title, action, key) in [("Cut", sel!(cut:), "x"), ("Copy", sel!(copy:), "c"), ("Paste", sel!(paste:), "v"), ("Select All", sel!(selectAll:), "a")] {
        edit.addItem(&menu_item(mtm, title, Some(action), key));
    }
    for sub in [app_menu, edit] {
        let holder = menu_item(mtm, "", None, "");
        holder.setSubmenu(Some(&sub));
        bar.addItem(&holder);
    }
    bar
}

fn status_menu(mtm: MainThreadMarker, view: &Canvas) -> Retained<NSMenu> {
    let menu = NSMenu::new(mtm);
    for (title, action) in [("Show window", sel!(showRoam:)), ("Heel (stop)", sel!(heelRoam:)), ("About Roam", sel!(aboutRoam:)), ("Quit", sel!(quitRoam:))] {
        let item = menu_item(mtm, title, Some(action), "");
        unsafe { item.setTarget(Some(view)) };
        menu.addItem(&item);
    }
    menu
}

fn spark_window(mtm: MainThreadMarker) -> Retained<NSWindow> {
    let r = rect((0.0, 0.0, SPARK_SIZE, SPARK_SIZE));
    let w = unsafe { NSWindow::initWithContentRect_styleMask_backing_defer(NSWindow::alloc(mtm), r, NSWindowStyleMask::Borderless, NSBackingStoreType::Buffered, false) };
    w.setOpaque(false);
    w.setBackgroundColor(Some(&NSColor::clearColor()));
    w.setIgnoresMouseEvents(true);
    w.setLevel(25); // the status bar level: above ordinary windows
    w.setCollectionBehavior(NSWindowCollectionBehavior::CanJoinAllSpaces);
    let view: Retained<SparkView> = unsafe { msg_send![SparkView::alloc(mtm), initWithFrame: r] };
    w.setContentView(Some(&view));
    unsafe { w.setReleasedWhenClosed(false) };
    w
}

pub fn run() {
    let mtm = MainThreadMarker::new().expect("Roam starts on the main thread");
    let app = NSApplication::sharedApplication(mtm);
    app.setActivationPolicy(NSApplicationActivationPolicy::Regular);
    let mons = win::displays();
    *UI.lock().unwrap() = Some(Ui {
        boot: 1, started: false, face: Face::Idle, face_until: None, tray: TRAY_OFF, offline: false, heeled: false, err: false, bonk: 0,
        text: SETUP_HELP.to_string(), update_on: false, update_busy: false, peers: Vec::new(), about: false, drag: None, pressed: None,
        bar: 0, spark: None, mons: mons.clone(),
    });

    let content = rect((0.0, 0.0, WIDTH, HEIGHT));
    let style = NSWindowStyleMask::Titled | NSWindowStyleMask::Closable | NSWindowStyleMask::Miniaturizable;
    let window = unsafe { NSWindow::initWithContentRect_styleMask_backing_defer(NSWindow::alloc(mtm), content, style, NSBackingStoreType::Buffered, false) };
    unsafe { window.setReleasedWhenClosed(false) };
    window.setTitle(&NSString::from_str("ROWMI Roam"));
    window.center();

    let parts = Parts { field: OnceCell::new(), window: OnceCell::new(), item: OnceCell::new(), spark: OnceCell::new() };
    let view: Retained<Canvas> = unsafe { msg_send![super(Canvas::alloc(mtm).set_ivars(parts)), initWithFrame: content] };

    let field = NSTextField::initWithFrame(NSTextField::alloc(mtm), rect((20.0, 104.0, 320.0, 28.0)));
    field.setBordered(false);
    field.setDrawsBackground(false);
    field.setFocusRingType(NSFocusRingType::None);
    field.setAlignment(NSTextAlignment::Center);
    field.setFont(Some(&plex_bold(20.0)));
    field.setTextColor(Some(&colour(BLACK)));
    field.setHidden(true);
    view.addSubview(&field);

    let item = NSStatusBar::systemStatusBar().statusItemWithLength(NSVariableStatusItemLength);
    if let Some(b) = item.button(mtm) { b.setImage(Some(&tray_image(TRAY_OFF))) }
    item.setMenu(Some(&status_menu(mtm, &view)));

    let spark = spark_window(mtm);
    window.setContentView(Some(&view));
    window.setDelegate(Some(ProtocolObject::from_ref(&*view)));
    app.setMainMenu(Some(&main_menu(mtm, &view)));

    if let Ok(saved) = std::fs::read_to_string(code_file()) {
        let mut lines = saved.lines();
        field.setStringValue(&NSString::from_str(&pretty(lines.next().unwrap_or("").trim())));
        win::load_target(lines.next().unwrap_or(""), &mons);
    } else {
        win::load_target("", &mons);
    }
    let _ = view.ivars().field.set(field);
    let _ = view.ivars().window.set(window);
    let _ = view.ivars().item.set(item);
    let _ = view.ivars().spark.set(spark);

    view.bring_up();
    brand::play(brand::STARTUP);
    unsafe { NSTimer::scheduledTimerWithTimeInterval_target_selector_userInfo_repeats(0.055, &view, sel!(tick:), None, true) };
    update::start();
    start_finder();
    app.run();
}
