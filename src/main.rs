#![cfg_attr(windows, windows_subsystem = "windows")]

mod net;
#[cfg(windows)]
mod brand;
#[cfg(windows)]
mod gui;
#[cfg(windows)]
mod signing;
#[cfg(windows)]
mod trust;
#[cfg(windows)]
mod update;
#[cfg(windows)]
mod win;

fn main() {
    #[cfg(windows)]
    gui::run();
    #[cfg(not(windows))]
    eprintln!("roam runs on Windows; this build only has the protocol tests (cargo test).");
}
