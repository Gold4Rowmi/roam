# ROWMI Roam: handover

Read this first. It says what exists, what is verified, what is not, and what the owner has already decided.

## What it is
A Windows-only Rust GUI app that shares one mouse and keyboard between two PCs on the same LAN. Push the cursor off a chosen monitor edge on the controlling PC and it appears on the other PC; push back through the same edge to return. Clipboard text follows. It is a ROWMI brand app (8-bit, Rommel the pixel doberman, Mint shell). Names it has had: edgehop, Fetch, now Roam. Old settings files `edgehop.code` and `fetch.code` are obsolete.

- Controlling PC = "host" (button: **Roam from this PC**). Controlled PC = "client" (button: **Be controlled**). Stop = **Heel**.
- Same exe on both PCs. Pairing is a 10-character code shown on the client and typed on the host.

## State at handover
- **Version 0.2.0.** `cargo test` (5 tests, plus 1 ignored network test) and `cargo build --release` pass on Windows. `src/gui.rs` compiles, including the `OFFLINE` and `settled_face()` edit.
- **Seen working on one PC:** the real fonts, the Update button, settings migration, and a full self-update using a patched `get()` that read local files: real signature check, a failed download and a bad signature each left the exe untouched, then the running exe swapped, relaunched and `roam.old` was removed. WinHTTP was separately tested against real GitHub, including the asset redirect.
- **Installer tested on one PC:** `installer.iss` built with Inno Setup 6.7.3; silent install, launch and uninstall all worked, without admin rights. `roam-setup.exe` is unsigned, so SmartScreen will warn once.
- **Released:** v0.2.0 is on GitHub, built by the release workflow.
- **Crossing:** the owner has used it between two real PCs on 0.2.0.
- **Real update confirmed:** the installed 0.2.0 offered 0.2.1 from GitHub, updated itself on one press and relaunched as 0.2.1. That release is live.
- **Not yet run:** the tray balloon, the arrival Rommel during a real crossing (it was only triggered directly). Treat these as untested until you see them work.

## Build
Windows with Rust (MSVC toolchain): `cargo test` then `cargo build --release`, or double-click `build.cmd` (runs both, leaves `roam.exe` beside it). `build.rs` embeds `roam.ico` using the `winresource` crate (needs `rc.exe`). Dependencies: `chacha20poly1305`, `ed25519-dalek`, `sha2`, `windows-sys 0.59`; build-dep `winresource`.

## Releasing
Bump `version` in `Cargo.toml`, commit, then push a matching tag (`v0.2.1`). `.github/workflows/release.yml` tests, builds, signs and publishes `roam.exe`, `roam.exe.sig`, `roam-setup.exe` and `SHA256SUMS.txt`. A tag that doesn't match `Cargo.toml` fails the run. If a run fails, `build.log` is attached to the prerelease named `ci-failure`.

The signing key is the Actions secret `ROAM_SIGNING_KEY`; the public half is in `src/signing.rs`. Lose the key and installed copies can't update: a new key means a new public key, which only reaches users through a manual reinstall. `sign gen` (in `examples/sign.rs`) refuses to overwrite a key file.

## Files
| File | Job |
|---|---|
| `src/main.rs` | Entry. `windows_subsystem = "windows"`. Calls `gui::run()`. `net` compiles on any OS so its tests run anywhere. |
| `src/net.rs` | `Msg` enum and encoding, `Link` (encrypted UDP). Port 47321. |
| `src/win.rs` | Host and client logic: hooks, raw input, `SendInput`, clipboard, edge geometry, panic key. |
| `src/gui.rs` | The window, boot screen, layout panel, owner-drawn buttons, tray, easter egg. |
| `src/brand.rs` | Palette, Rommel (7 poses), wordmark, app icon, fonts, embedded WAVs. Pixel data copied from the brand package. |
| `src/update.rs` | Daily check, WinHTTP download, signature check, exe swap. Holds the repo name (`REPO`). |
| `src/signing.rs` | The public key and what a signature covers. |
| `examples/sign.rs` | Makes the key and signs releases. Not in the shipped exe. |
| `installer.iss` | Inno Setup script. |
| `assets/sounds/*.wav` | Brand package sounds, embedded with `include_bytes!`. |
| `assets/fonts/` | Silkscreen and IBM Plex Mono with their OFL licences, embedded. |
| `roam.ico`, `build.rs` | Exe icon (Rommel on the Mint shell, whole-pixel sizes). |
| `fonts/README.txt` | The folder is an optional override now. |
| `docs/brand/` | Small brand references: `rowmi.json`, `rowmi.css`, `SKILL.md`, package README. The full package lives with the owner. |

## Protocol and security (`net.rs`)
- Datagram = 12-byte nonce (`[role, PROTOCOL, 0,0, u64 LE sequence]`) + ChaCha20-Poly1305 ciphertext of one `Msg`. Key = SHA-256 of the pairing code. The code (about 50 bits, no slow KDF) is the only secret.
- Sequence starts at wall-clock nanoseconds; the receiver drops any sequence <= the last accepted one (replay and reordering). Role byte stops a side accepting its own packets. Source IP must be private, link-local or loopback.
- Messages: `Enter(edge, frac)`, `Leave(frac)`, `Move(dx,dy)`, `Button(raw flags, data)`, `Key{vk,scan,up,ext}`, `Clip(text)`, `Ping`, `Bye`. Edge: 0 right, 1 left, 2 top, 3 bottom.
- A `Bye` received during a live connection makes the receiver heel too (`gui::heel()` posts `WM_HEEL`, which runs the same `stop()` as the Heel button). A timeout without a `Bye` does not: that PC keeps waiting. Quit and the updater also send `Bye`, so the other PC returns to setup then as well.
- Discovery: the host broadcasts Pings to `255.255.255.255:47321` until the client answers, then talks unicast. Both sides ping every 500 ms. 2 s of silence = connection lost. `Bye` on stop makes the other side react at once.
- `PROTOCOL` is authenticated because it is in the nonce. A packet with the right code and another version is dropped and the status line says the other PC runs a different Roam version. Builds before 0.2 sent 0, so they no longer connect to 0.2. Bump it only when the wire format changes; updating one PC at a time then keeps working otherwise.

## How crossing works (`win.rs`)
- Host: low-level mouse and keyboard hooks swallow input while `REMOTE` is true; a hidden window gets `WM_INPUT` for true mouse deltas. The host's screen rectangle is the chosen monitor (fixed at Start); the client uses the whole virtual desktop.
- Crossing a chosen edge sends `Enter(edge, frac)`. The client enters at the opposite edge (`edge ^ 1`) and sends `Leave` when pushed back out. Clipboard text goes both ways on each crossing.
- **Ctrl+Alt+Esc** on the host is the panic key. The client releases held keys and buttons on `Leave`, `Bye` or timeout.
- Sounds: `on` when the cursor leaves, `off` when it comes home, `tag` when it arrives on the client.

## GUI notes (`gui.rs`)
- Hand-drawn Win32: STATIC and EDIT controls plus owner-drawn buttons. Worker threads never touch the UI; they write to `PENDING` and post `WM_REFRESH`.
- Boot runs once per launch (55 ms ticks, about 1.1 s), skippable after 300 ms, honours the Windows reduced-motion setting, ends on Idle Rommel.
- Window hides to the tray when the PCs connect and returns when the link drops. Heel (button or tray menu) restarts the process.
- Tray icon is Rommel on a colour: Mint = mouse here, Paper = mouse on the other PC, Cartridge Grey with sleepy Rommel = not connected.
- Layout panel draws the real monitors (primary first) and a draggable "Other" box that snaps to the nearest monitor edge.
- `layout()` and `load_type()` hold all sizes and fonts, and rerun on `WM_DPICHANGED`. The EDIT box must stay taller than its font's line or it paints over its frame.
- When the mouse lands on a PC, and when it comes back to the controlling PC, the brand's pop sparkle plays there: eight rust (#D9893B) 8 px squares burst out in the eight directions, 4 steps of 8 or 16 px chosen per spark, 105 ms a step. It follows `rowmi-ui-guidelines.html` (`.spark`, `sparks()`). It is a click-through colour-keyed window that never takes focus. The cursor lands 2 px inside a screen edge, so the burst's middle is pulled inward to keep the whole thing on screen. Only the PC the mouse arrived on shows it.
- Wrong input bonks, as in the book's `.bonk` and `.field.err`: the label, frame and code box shake -8, +8, -5, +3, 0 px over 5 steps of 60 ms with the error sound, then stay in an error state (red border, red help text) until the box is edited. Triggers: a key a code can't hold or an 11th character (the box is subclassed in `code_proc`), and pressing a button with a code that isn't 10 characters. Pasting bypasses the key filter; `start()` still catches a bad code. The full brand package (book, UI guidelines, icon sources) is in `rowmi-brand-package-v1` beside this folder; check it before drawing any motion.
- Title band: the wordmark, then "Roam" as a Silkscreen 16 label on its baseline, as the brand's title-bar spec says. A 32 px app name next to the wordmark read as two competing typefaces.
- Tap Rommel five times: grin and bubbles. One tap: alert and woof.

## Settings on disk
`%APPDATA%\ROWMI\Roam\roam.code`, migrated once from beside the exe. Line 1: pairing code. Line 2: `left top width height edge` of the chosen host monitor (physical pixels) and edge number. `trusted.txt` beside it: one `CODE NAME` per PC that has ever connected (`src/trust.rs`, newest first, 8 kept, one per name). The installer has an unticked task that adds a Startup shortcut.

## Finding trusted PCs (`net::find`, `gui::start_finder`)
- Only runs on the setup screen and only if `trusted.txt` has entries or `roam.code` holds a code (so PCs paired before trust was recorded are found too; Forget then empties that code as well). The PC being looked for only has to be at the setup screen, not waiting. Binds port 47321, sends `Msg::Here(name)` every 2 s to the broadcast address under each trusted code's key (random nonce), and listens. Starting a role bumps `FINDER`, which stops it; `client()` then takes the port (`bind_lan` retries for 3 s).
- A found PC is one whose beacon decrypts under a trusted code, or whose host-role `Ping` does (`wants`: its host is looking for us, so the button reads Allow). Entries expire after 5 s without a packet.
- Pressing a button puts that PC's code in the code box and calls `start()` as host (Roam onto) or client (Allow), so the code is visible and saved like a normal pairing. The controlled PC never connects without its own press.
- `Ping` now carries the sender's computer name (`COMPUTERNAME`); a connection that works calls `trust::remember`. Old builds send an empty name, which is never trusted. `Here` and the name in `Ping` are additive: 0.2.1 ignores them, and `PROTOCOL` stays 1.
- Right-click (or the menu key) on a found PC offers Forget. Two PCs with the same computer name can't be told apart.

## Brand rules that must not regress
One pixel unit per lockup; whole-step scaling only; two typefaces (Silkscreen at 8/16/24/32/64 only, IBM Plex Mono 400/600); one shell per app (Roam = Mint `#2FC7A1`, black text on it); Cursor Blue means selected or focused only; one Rommel per screen, Idle by default, Sleepy only for offline, he reacts and never talks; the frame (1 px border, 2 px hard drop, 6 px radius) is the only depth; every input answers with a movement and a sound; honest by default, no dark patterns. Full text in `docs/brand/SKILL.md` and `rowmi.json`.

## Agreed plan
Goal: Claude builds the exe or installer, and the installed app updates itself. All the code is done. What remains needs the owner's GitHub account:
1. **Repo.** Owner creates an empty **public** GitHub repo. Public so update checks need no token. `REPO` in `src/update.rs` must match it. Push this folder.
2. **First release.** Add the `ROAM_SIGNING_KEY` secret, push tag `v0.2.0`, check the run. Then bump to 0.2.1 and tag again to test a real update from the installed 0.2.0.

Security note for the updater: it ships code to both PCs of a tool that sees every keystroke. Signed releases stop tampered downloads; they do not stop a hijacked GitHub account, so the owner should use 2FA. Unsigned installers trigger SmartScreen once ("More info, Run anyway"); a paid code-signing certificate would remove that. Files the app downloads itself do not.

## Known limitations and risks
- Text-only clipboard, capped at 60,000 bytes (truncated on a character boundary).
- One client at a time. The monitor and edge are fixed after Start; quit and reopen to change. Monitor and DPI changes are not re-read after launch.
- Broadcast discovery can fail with a VPN or virtual adapter holding the default route. No manual-IP fallback yet.
- Pairing code stored in plain text.
- Controlling elevated windows (UAC prompts, apps run as admin) probably does not work without running as admin. Untested.
- After a self-update, Windows' Installed Apps list still shows the version the installer put there.
- The Alert `!` emote isn't drawn with the update announcement; the title band has no room above him.
- Not built: dark mode, button hover state, the exact dot-matrix boot (approximated with rectangles).

## History worth knowing
The authoring environment (a cloud container) could not download the Windows Rust standard library or crates, so the code was written without a compiler and corrected from the owner's build output. Fixes so far were imports (`SetCapture`, `ReleaseCapture`) and a pointer cast for `PlaySoundW`. If you can compile, you can verify what I could not.

## Suggested first message to the next session
"Read HANDOVER.md. Run `cargo test` and `cargo build --release` and tell me the results before changing anything. Then start on the agreed plan, in order."
