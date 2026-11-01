# ROWMI Roam

One mouse and keyboard for two Windows PCs on the same network. Push the cursor off a monitor edge and it roams onto the other PC; push back through the same edge, or press Ctrl+Alt+Esc (Heel), and it comes home. Clipboard text follows you.

Part of ROWMI. Made in memory of Rommel.

## Install
Download `roam-setup.exe` from the latest GitHub release and run it on both PCs. It installs for your user only and needs no admin rights. It isn't code-signed, so Windows may show SmartScreen once: choose **More info**, then **Run anyway**.

## Build
1. Install Rust: https://rustup.rs (defaults are fine).
2. Double-click `build.cmd`. It runs the tests and leaves `roam.exe` in this folder. The fonts are built in.

## Updates
Roam asks GitHub for the latest release when it opens and once a day. That request is the only thing it sends beyond your LAN, and it carries no data about you or your PCs. When a newer version exists Rommel perks up, the tag jingle plays and an Update button appears. Nothing is downloaded until you press it. The new exe must carry a valid ed25519 signature; if it doesn't, it is thrown away. Roam then restarts at the setup screen. Update both PCs.

## Use
1. On the PC you want to control, open Roam and press **Be controlled**. It shows a pairing code. Allow it on **private** networks when Windows Firewall asks.
2. On the PC whose mouse and keyboard you use, open Roam, type that code, drag the mint **Other** box next to the monitor edge the other PC sits beside (your real monitor layout is shown; 1 is the primary), press **Roam from this PC**.

Both sides remember the code and layout, and each PC remembers the other as **trusted** once they have connected. A PC you paired earlier is found through its saved code. When both have Roam open at the setup screen (neither needs to be waiting), each shows a button for the other: **Roam onto NAME** on the PC whose mouse you want to use. On the other PC that button turns into **Allow NAME**, and nothing connects until it is pressed. The normal setup (type a code, pick a button) is still there for a new PC, and nothing happens by itself. Right-click a found PC to **Forget** it.

Finding trusted PCs sends a small encrypted beacon to your local network every 2 seconds, only while Roam sits at the setup screen and only once you have a saved code or a trusted PC. It never leaves your network, and anything without the shared code can't read it or answer it. To open Roam when you sign in to Windows, tick "Open Roam when I sign in to Windows" in the installer (off by default), or put a shortcut to `roam.exe` in the folder you get from pressing Win+R and running `shell:startup`. The window tucks into the tray once the two PCs connect: Rommel on mint means the mouse is here, Rommel on paper means it is on the other PC, sleepy Rommel means not connected. Right-click him to Heel (stop) or quit. No IP addresses: the host finds the client by LAN broadcast.

## Brand
Rommel, the wordmark, the app icon, the boot timeline and every sound come from the ROWMI brand package v1.0 (`assets/sounds` holds the WAVs; the pixel data is in `src/brand.rs`). Tap Rommel five times.

## How it stays low-lag and local-only
- Raw UDP (port 47321), one small encrypted datagram per input event; no buffering, no retries.
- ChaCha20-Poly1305 keyed from the pairing code; packets with the wrong key, a replayed sequence number, or a non-LAN source IP are dropped.
- If either side stops answering for 2 s, the host takes control back and the client releases any held keys, so nothing gets stuck.
