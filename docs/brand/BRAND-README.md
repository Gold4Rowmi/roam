# ROWMI brand package, v1.0

Playful tools with a lot of bark. Made in memory of Rommel.

Everything here comes from one set of pixel data, so the mark, the dog, the icons and the patterns always agree.

## What's inside

| Folder | Contents |
|---|---|
| `docs/` | The interactive brand book, the UI/UX guidelines, the startup-sound picker and the logo editor. Open them in a browser; they are single files. |
| `tokens/` | `rowmi.css` (CSS custom properties: palette, shells, dark mode, type, space, motion) and `rowmi.json` (the same, plus the rules, for code). |
| `logo/` | Mark, wordmark and lockups as SVG and PNG at whole-pixel sizes; app icons on each shell. |
| `rommel/` | All seven poses in light, dark (with the Paper outline) and tan versions, SVG and PNG, plus a sprite sheet. |
| `icons/` | 109 blocky 8 × 8 icons as individual SVGs, PNGs at 32 and 64 px, an SVG sprite, `icons.json`, a contact sheet, and the source script. |
| `patterns/` | Repeating tiles: the pack (160 px), confetti (128 px), bubbles (192 px), as SVG and 2× PNG, plus CSS for cartridge ribs and grid paper. |
| `sounds/` | `startup.wav` (Trot & ping, the boot sound, rendered within Game Boy sound-chip limits), the UI set, Rommel's seven reactions, and the 14 other startup candidates for reference. 44.1 kHz 16-bit mono. |
| `skill/` | `rowmi-design/SKILL.md`: a skill that teaches an AI assistant to design in this brand. Add it to Claude's skills or keep it in a repo as `CLAUDE.md` material. |

## The rules in one breath

One pixel unit per lockup, whole steps only, two typefaces, one shell per app, blue means selected, one Rommel per screen (Idle is the logo), word on the chin line, the frame is the only depth, every input answers, honest by default.

## Fonts

Both faces are free on Google Fonts and may be bundled with apps:
- Silkscreen (400, 700): https://fonts.google.com/specimen/Silkscreen (used only at 8, 16, 24, 32, 64 px)
- IBM Plex Mono (400, 600): https://fonts.google.com/specimen/IBM+Plex+Mono

## Using the tokens

```css
@import "tokens/rowmi.css";
:root { --shell: var(--shell-minibot); --shell-text: var(--shell-minibot-deep); }  /* once per app */
```
Components read `--shell`, `--ink`, `--surface-*` and `--text-*`; never a hex.

## Boot screen

Every app opens the same way, once per launch, 1.1 s, skippable: four claw taps, the word steps down onto Rommel's chin line, the ping wakes him and the shell colour rises with it, and it ends on the logo. Timeline and rules are in `tokens/rowmi.json` under `boot`, and the brand book chapter 13 plays it.

## Sizes that stay sharp

Pixel art is drawn on grids of 8 (icons) and 10 (Rommel). Render at whole multiples: icons at 16, 24, 32, 64; Rommel at 20, 40, 80, 120, 160; lockups at a unit of 3, 4, 8, 12, 16 px. Never 1.5×.
