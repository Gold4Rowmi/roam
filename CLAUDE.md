# ROWMI Roam

Read `HANDOVER.md` first: it has the state, the architecture, what is unverified and the agreed plan.

Working rules for this repo:
- Compile before you report. `cargo test` and `cargo build --release` on Windows. Say what you did not run.
- Smallest change that fully solves the task. No new dependency for a few lines. Comment only the why; mark known shortcuts as `shortcut: <limit>, <when to upgrade>`.
- This is a ROWMI app: follow the brand rules in `HANDOVER.md` and `docs/brand/SKILL.md` for anything the user sees or hears. Use the pixel data in `src/brand.rs`; never redraw Rommel or the wordmark.
- Worker threads never touch the UI; use `gui::status`, `gui::tray` and `gui::visible`.
- Changing the protocol in `src/net.rs` breaks older builds. Say so, and bump the version once that exists.
- Never add dark patterns, silent updates, or telemetry. The owner decided updates are announced, not silent.
