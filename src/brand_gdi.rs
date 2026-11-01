//! GDI drawing for the brand: the frame, Rommel, the wordmark, the app icon and type.
//! Type: Silkscreen and IBM Plex Mono are embedded, so the single exe and its updates carry them.
//! A fonts/ folder next to the exe is an optional override.
use super::*;
use std::mem::zeroed;
use std::ptr::null;
use windows_sys::Win32::Foundation::RECT;
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::UI::WindowsAndMessaging::{CreateIconIndirect, HICON, ICONINFO};

pub unsafe fn fill(hdc: HDC, r: &RECT, c: u32) {
    let b = CreateSolidBrush(c);
    FillRect(hdc, r, b);
    DeleteObject(b as _);
}

pub fn rect(x: i32, y: i32, w: i32, h: i32) -> RECT { RECT { left: x, top: y, right: x + w, bottom: y + h } }

/// The brand frame: 1 px border, 2 px hard drop, radius 6 outer (tokens.radius). The only depth there is.
pub unsafe fn frame(hdc: HDC, r: &RECT, face: u32, border: u32) {
    rounded(hdc, &rect(r.left + 2, r.top + 2, r.right - r.left, r.bottom - r.top), GREY, GREY, 6);
    rounded(hdc, r, face, border, 6);
}

pub unsafe fn rounded(hdc: HDC, r: &RECT, face: u32, border: u32, radius: i32) {
    let pen = CreatePen(PS_SOLID, 1, border);
    let brush = CreateSolidBrush(face);
    let (op, ob) = (SelectObject(hdc, pen as _), SelectObject(hdc, brush as _));
    RoundRect(hdc, r.left, r.top, r.right, r.bottom, radius, radius);
    SelectObject(hdc, op);
    SelectObject(hdc, ob);
    DeleteObject(pen as _);
    DeleteObject(brush as _);
}

/// Grid paper: the default ground, lines every `cell` px (the content's pixel unit).
pub unsafe fn grid_paper(hdc: HDC, r: &RECT, cell: i32) {
    fill(hdc, r, PAPER);
    let mut x = r.left + cell;
    while x < r.right { fill(hdc, &rect(x, r.top, 1, r.bottom - r.top), GRID); x += cell }
    let mut y = r.top + cell;
    while y < r.bottom { fill(hdc, &rect(r.left, y, r.right - r.left, 1), GRID); y += cell }
}

/// Draws Rommel with his top-left at (x, y), `u` screen pixels per unit.
pub unsafe fn draw_rommel(hdc: HDC, x: i32, y: i32, u: i32, f: Face, p: &Pal) {
    let rs = rommel_rects(f, p);
    if let Some(oc) = p.outline {
        // one-pixel Paper outline, like a game sprite: every empty cell touching a filled one
        let mut cells = [[false; 12]; 12];
        for (rx, ry, w, h, _) in &rs { for cx in *rx..rx + w { for cy in *ry..ry + h { cells[(cy + 1) as usize][(cx + 1) as usize] = true } } }
        for cy in 0..12i32 { for cx in 0..12i32 {
            if cells[cy as usize][cx as usize] { continue }
            let near = [(0, 1), (0, -1), (1, 0), (-1, 0)].iter().any(|(dx, dy)| {
                let (nx, ny) = (cx + dx, cy + dy);
                (0..12).contains(&nx) && (0..12).contains(&ny) && cells[ny as usize][nx as usize]
            });
            if near { fill(hdc, &rect(x + (cx - 1) * u, y + (cy - 1) * u, u, u), oc) }
        } }
    }
    for (rx, ry, w, h, c) in rs { fill(hdc, &rect(x + rx * u, y + ry * u, w * u, h * u), c) }
}

pub unsafe fn wordmark(hdc: HDC, x: i32, y: i32, u: i32, c: u32) {
    for (rx, ry, w, h) in WORD { fill(hdc, &rect(x + rx * u, y + ry * u, w * u, h * u), c) }
}

/// Horizontal lockup, 39x10 units: Rommel, then the word on his chin line (x=12, y=4).
pub unsafe fn lockup(hdc: HDC, x: i32, y: i32, u: i32, f: Face, p: &Pal, word: u32) {
    draw_rommel(hdc, x, y, u, f, p);
    wordmark(hdc, x + 12 * u, y + 4 * u, u, word);
}

/// The `!` over Rommel at (x, y): a 2 x 3 bar, a gap, a 2 x 1 dot, in columns 4 and 5, one empty row above his ears.
pub unsafe fn emote_alert(hdc: HDC, x: i32, y: i32, u: i32) {
    fill(hdc, &rect(x + 4 * u, y - 6 * u, 2 * u, 3 * u), TAN);
    fill(hdc, &rect(x + 4 * u, y - 2 * u, 2 * u, u), TAN);
}

/// App icon as logo/app-icons/*.svg: a 12x12 shell square with Rommel at (1,1). `size` is the pixel
/// size; the unit is the largest whole one that fits, so nothing is ever resampled.
pub unsafe fn app_icon(size: i32, shell: u32, f: Face) -> HICON {
    let mut bmi: BITMAPINFO = zeroed();
    bmi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
    (bmi.bmiHeader.biWidth, bmi.bmiHeader.biHeight, bmi.bmiHeader.biPlanes, bmi.bmiHeader.biBitCount) = (size, -size, 1, 32);
    let hdc = CreateCompatibleDC(std::ptr::null_mut());
    let mut bits: *mut u32 = std::ptr::null_mut();
    let color = CreateDIBSection(hdc, &bmi, DIB_RGB_COLORS, &mut bits as *mut _ as _, std::ptr::null_mut(), 0);
    let old = SelectObject(hdc, color as _);
    let u = (size / 12).max(1);
    let pad = (size - 12 * u) / 2;
    fill(hdc, &rect(0, 0, size, size), 0); // transparent corners (alpha set below)
    fill(hdc, &rect(pad, pad, 12 * u, 12 * u), shell);
    draw_rommel(hdc, pad + u, pad + u, u, f, &LIGHT);
    SelectObject(hdc, old);
    DeleteDC(hdc);
    for y in 0..size { for x in 0..size {
        let inside = x >= pad && x < pad + 12 * u && y >= pad && y < pad + 12 * u;
        let p = bits.add((y * size + x) as usize);
        *p = if inside { *p | 0xFF00_0000 } else { 0 };
    } }
    let zeros = vec![0u8; (size * size / 8 + 4) as usize];
    let mask = CreateBitmap(size, size, 1, 1, zeros.as_ptr() as _);
    let ii = ICONINFO { fIcon: 1, xHotspot: 0, yHotspot: 0, hbmMask: mask, hbmColor: color };
    CreateIconIndirect(&ii)
}

// ---------- fonts ----------

/// The embedded fonts, then any fonts/*.ttf next to the exe, for this process only.
pub unsafe fn load_fonts() {
    for ttf in [
        &include_bytes!("../assets/fonts/Silkscreen-Regular.ttf")[..],
        &include_bytes!("../assets/fonts/IBMPlexMono-Regular.ttf")[..],
        &include_bytes!("../assets/fonts/IBMPlexMono-SemiBold.ttf")[..],
    ] {
        let mut added = 0;
        AddFontMemResourceEx(ttf.as_ptr() as _, ttf.len() as u32, null(), &mut added);
    }
    let Ok(rd) = std::fs::read_dir(std::env::current_exe().unwrap().with_file_name("fonts")) else { return };
    for e in rd.flatten() {
        let p: Vec<u16> = e.path().to_string_lossy().encode_utf16().chain([0]).collect();
        AddFontResourceExW(p.as_ptr(), 0x10 /*FR_PRIVATE*/, null());
    }
}

/// A font by face name, or null if that face isn't installed. `px` is the em size; pixel faces
/// are drawn with smoothing off so their pixels stay square.
pub unsafe fn font(face: &str, px: i32, weight: i32, pixel: bool) -> HFONT {
    let w: Vec<u16> = face.encode_utf16().chain([0]).collect();
    let f = CreateFontW(-px, 0, 0, 0, weight, 0, 0, 0, 1, 0, 0, if pixel { 3 /*NONANTIALIASED*/ } else { 5 /*CLEARTYPE*/ }, 0, w.as_ptr());
    let hdc = CreateCompatibleDC(std::ptr::null_mut());
    let old = SelectObject(hdc, f as _);
    let mut got = [0u16; 64];
    let n = GetTextFaceW(hdc, 64, got.as_mut_ptr()) as usize;
    SelectObject(hdc, old);
    DeleteDC(hdc);
    if String::from_utf16_lossy(&got[..n.saturating_sub(1)]).eq_ignore_ascii_case(face) { f } else { DeleteObject(f as _); std::ptr::null_mut() }
}

// ---------- built-in 5x5 pixel face (fallback when Silkscreen isn't in fonts/) ----------

fn glyph(c: char) -> ([u8; 5], i32) {
    let g: [u8; 5] = match c {
        'A' => [0x0E, 0x11, 0x1F, 0x11, 0x11], 'B' => [0x1E, 0x11, 0x1E, 0x11, 0x1E], 'C' => [0x0F, 0x10, 0x10, 0x10, 0x0F],
        'D' => [0x1E, 0x11, 0x11, 0x11, 0x1E], 'E' => [0x1F, 0x10, 0x1E, 0x10, 0x1F], 'F' => [0x1F, 0x10, 0x1E, 0x10, 0x10],
        'G' => [0x0F, 0x10, 0x13, 0x11, 0x0F], 'H' => [0x11, 0x11, 0x1F, 0x11, 0x11], 'I' => return ([7, 2, 2, 2, 7], 3),
        'J' => [0x07, 0x02, 0x02, 0x12, 0x0C], 'K' => [0x11, 0x12, 0x1C, 0x12, 0x11], 'L' => [0x10, 0x10, 0x10, 0x10, 0x1F],
        'M' => [0x11, 0x1B, 0x15, 0x11, 0x11], 'N' => [0x11, 0x19, 0x15, 0x13, 0x11], 'O' => [0x0E, 0x11, 0x11, 0x11, 0x0E],
        'P' => [0x1E, 0x11, 0x1E, 0x10, 0x10], 'Q' => [0x0E, 0x11, 0x15, 0x12, 0x0D], 'R' => [0x1E, 0x11, 0x1E, 0x12, 0x11],
        'S' => [0x0F, 0x10, 0x0E, 0x01, 0x1E], 'T' => [0x1F, 0x04, 0x04, 0x04, 0x04], 'U' => [0x11, 0x11, 0x11, 0x11, 0x0E],
        'V' => [0x11, 0x11, 0x11, 0x0A, 0x04], 'W' => [0x11, 0x11, 0x15, 0x1B, 0x11], 'X' => [0x11, 0x0A, 0x04, 0x0A, 0x11],
        'Y' => [0x11, 0x0A, 0x04, 0x04, 0x04], 'Z' => [0x1F, 0x02, 0x04, 0x08, 0x1F],
        '0' => [0x0E, 0x13, 0x15, 0x19, 0x0E], '1' => [0x04, 0x0C, 0x04, 0x04, 0x0E], '2' => [0x0E, 0x11, 0x06, 0x08, 0x1F],
        '3' => [0x1E, 0x01, 0x06, 0x01, 0x1E], '4' => [0x12, 0x12, 0x1F, 0x02, 0x02], '5' => [0x1F, 0x10, 0x1E, 0x01, 0x1E],
        '6' => [0x0E, 0x10, 0x1E, 0x11, 0x0E], '7' => [0x1F, 0x01, 0x02, 0x04, 0x08], '8' => [0x0E, 0x11, 0x0E, 0x11, 0x0E],
        '9' => [0x0E, 0x11, 0x0F, 0x01, 0x0E],
        '-' => return ([0, 0, 7, 0, 0], 3), '.' => return ([0, 0, 0, 0, 1], 1), '!' => return ([1, 1, 1, 0, 1], 1),
        _ => return ([0; 5], 3),
    };
    (g, 5)
}

pub fn text_w(s: &str) -> i32 {
    let n = s.chars().count() as i32;
    s.chars().map(|c| glyph(c.to_ascii_uppercase()).1).sum::<i32>() + (n - 1).max(0)
}

pub unsafe fn text(hdc: HDC, x: i32, y: i32, u: i32, s: &str, c: u32) {
    let mut cx = x;
    for ch in s.chars() {
        let (g, w) = glyph(ch.to_ascii_uppercase());
        for (row, bits) in g.iter().enumerate() {
            for col in 0..w {
                if bits >> (w - 1 - col) & 1 == 1 { fill(hdc, &rect(cx + col * u, y + row as i32 * u, u, u), c) }
            }
        }
        cx += (w + 1) * u;
    }
}

