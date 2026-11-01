#![cfg_attr(windows, windows_subsystem = "windows")]

mod net;
#[cfg(any(windows, target_os = "macos"))]
mod brand;
#[cfg(any(test, target_os = "macos"))]
mod keymap;
#[cfg(any(windows, target_os = "macos"))]
mod signing;
#[cfg(any(windows, target_os = "macos"))]
mod trust;
#[cfg(any(windows, target_os = "macos"))]
mod update;
#[cfg(windows)]
mod gui;
#[cfg(windows)]
mod win;
#[cfg(target_os = "macos")]
#[path = "mac_gui.rs"]
mod gui;
#[cfg(target_os = "macos")]
#[path = "mac.rs"]
mod win;

fn main() {
    #[cfg(any(windows, target_os = "macos"))]
    gui::run();
    #[cfg(not(any(windows, target_os = "macos")))]
    eprintln!("roam runs on Windows and macOS; this build only has the protocol tests (cargo test).");
}
