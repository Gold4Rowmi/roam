---
name: rowmi-design
description: Design and build anything in the ROWMI brand (edgehop/Roam, SaveForge/dig, minibot, VITACART, and the brand itself) - app UI, websites, splash and boot screens, icons, posts, sounds. Carries the exact pixel grids for Rommel and the wordmark. Use whenever ROWMI, Rommel, or one of those apps comes up.
---

# ROWMI design

ROWMI is the brand for a family of small, dependable apps: **edgehop** (mouse and keyboard sharing; also called ROWMI Roam), **SaveForge** (save editor; also called dig), **minibot** (minigame autoplayer), **VITACART** (PS Vita launcher). It is 8-bit, DS-Lite-hub nostalgic, with modern cues from teenage engineering hardware. The logo is **Rommel**, a pixel doberman, the family dog of ten years. The brand is dedicated to him.

Tagline: **Playful tools with a lot of bark.** Dedication line, used quietly once per app (About screen, footer): **Made in memory of Rommel.**

When a ROWMI package is available (`tokens/`, `logo/`, `rommel/`, `icons/`, `patterns/`, `sounds/`, `docs/`), use its files rather than redrawing anything. The brand book (`docs/rowmi-brand-book.html`) and UI guidelines (`docs/rowmi-ui-guidelines.html`) are the full reference; this skill is the working summary.

## The ten non-negotiables

1. **One pixel unit per lockup.** Dog, word and pixel type share it. No mixels (mixed pixel sizes in one composition).
2. **Whole steps only.** Scale pixel art by 2, 3, 4, never 1.5. Pixel art moves in whole pixels on frame ticks; it never scales, eases or fades.
3. **Two typefaces.** Silkscreen for short words, IBM Plex Mono for sentences.
4. **One shell per app.** Paper and Black build the screen; the app's shell colour gives it a voice. Tan belongs to Rommel.
5. **Blue means selected.** Cursor Blue is never decoration, never a second brand colour.
6. **One Rommel per screen.** Idle is the logo and the default. Every other face is a reaction under a second long, or a state. Never decoration. He reacts, never talks, never guilts.
7. **Word on the chin line.** In horizontal lockups the wordmark sits level with his chin (x=12, y=4 in logo units).
8. **The frame is the only depth.** 1 px border + 2 px hard drop. No glows, no gradients, no soft shadows.
9. **Every input answers.** A movement and a sound, short and quiet.
10. **Honest by default.** Offline, no tricks, buttons do what they say. No dark patterns, ever.

## Colour

Core palette (use the token names from `tokens/rowmi.css`):

| Token | Hex | Job |
|---|---|---|
| Paper | #ECEEF1 | the ground of everything |
| Rommel Black | #16181D | text, the dog, filled buttons, dark screens |
| Rommel Tan | #D9893B | his markings, ROWMI's own colour, small doses |
| Cartridge Grey | #C9CED6 | frames, borders, the 2 px drop |
| Deep Tan | #9C4F12 | tan for text on Paper |
| Cursor Blue | #2F6FE0 (#7AA7F2 on dark) | selection and focus only |
| Tongue Pink | #E07A6B | Rommel only |

App shells, one per app. Text on a shell is always black; the deep tone is for shell-coloured words on Paper.

| App | Shell | Hex | Deep |
|---|---|---|---|
| edgehop | Mint | #2FC7A1 | #0E6E57 |
| SaveForge | Berry | #F0548C | #A8174F |
| minibot | Lime | #9BCB3B | #4A6B12 |
| VITACART | Gold | #F4C430 | #7A5A00 |

Shells go on the app icon, title bar, splash, progress, the A glyph and that app's posts. Never on body text, selection, or Rommel. Meaning colours (success, warning, danger) beat the shell: delete is red even in SaveForge. A new app gets a shell far in hue from these four, from tan and from blue, and black text on it must pass 4.5 : 1.

Dark mode is a separate design, not an inversion: near-black tinted surfaces, lighter as they rise, borders instead of shadows, rust desaturated to #E8A96B. Rommel stays black, tan and pink on dark grounds with a 1 px Paper outline. Never a white Rommel.

## Type

- **Silkscreen** for titles, labels and short words, only at 8, 16, 24, 32 or 64 px so its pixels stay square. Font smoothing off. Weight 400 by default; 700 only for display headings.
- **IBM Plex Mono** for everything people read or type, weights 400 and 600 only. Body 15 px (14 in dense apps), lead 20 px.
- Both are free (SIL Open Font License) and must be bundled with native apps: Silkscreen https://fonts.google.com/specimen/Silkscreen, IBM Plex Mono https://fonts.google.com/specimen/IBM+Plex+Mono. If a font can't be loaded, fall back to the system monospace; never substitute another pixel font.
- The wordmark is never set in Silkscreen or any font. It is the 27 x 5 pixel grid below.
- Hierarchy: Display Silkscreen 64 · Title 32 · Heading 24 · Label 16 · Lead Plex 20 · Body 15 · Strong 600 · Caption 12 grey.
- Sentence case. No all-caps labels, no tracked eyebrows above headings, no gradient or italic accent words.

## Rommel's faces and sounds

Poses: **Idle** (the logo, default), Alert (something new: ears up, `!`), Happy (success: eyes shut, tongue), Sad (an error), Sleepy (offline), Wink (a tip), Grin (bubbles: eyes open, tongue). Reactions: woof (launch), yip (big success), sniff (search starts), tag jingle (something arrived), whine (task failed), grr (about to delete), awoo (long job done).

## Logo and Rommel: the exact pixels

Never draw Rommel or the wordmark from memory or from a font. Use these grids exactly; they are the brand. Legend: `#` Rommel Black #16181D, `t` Rommel Tan #D9893B (tan-soft #F8E2CC when he sits on Tan), `e` eye #F4F5F7, `p` Tongue Pink #E07A6B, `.` empty. Each character is one pixel (one unit, u). Render at whole multiples only.

Rommel, Idle (the logo), 10 x 10:
```
..##..##..
##########
###t##t###
##########
#.#e##e#.#
..t####t..
..#tttt#..
...t##t...
....##....
..........
```
Rows 1 to 4 are his head and crown, the `t` at row 3 are his eyebrows, row 5 has his eyes with the ears hanging at the sides, rows 6 to 8 his tan muzzle and cheeks, row 9 his nose.

Other poses are small edits of Idle:
- **Alert** (ears up): row 1 becomes `#.##..##.#`, row 5 becomes `..#e##e#..`.
- **Happy** (ears up, eyes shut, tongue): row 1 `#.##..##.#`, row 5 `..######..`, row 10 `....pp....`.
- **Grin** (ears up, eyes open, tongue): row 1 `#.##..##.#`, row 5 `..#e##e#..`, row 10 `....pp....`.
- **Sad** (ears down): row 6 becomes `#.t####t.#`.
- **Sleepy** (ears down, eyes shut): row 5 `#.######.#`, row 6 `#.t####t.#`.
- **Wink**: row 5 `#.#e####.#`, row 10 `....pp....`.

ROWMI wordmark, 27 x 5, `#` only:
```
####...###..#...#.#...#.###
#...#.#...#.#...#.##.##..#.
#...#.#...#.#.#.#.#.#.#..#.
####..#...#.#.#.#.#...#..#.
#...#..###...#.#..#...#.###
```

Lockup: a 39 x 10 grid. Rommel at x 0 to 9; the wordmark starts at x = 12 with its top row on y = 4 (his chin line), so its bottom row is y = 8. Keep 2 u of clear space around the whole thing. Stacked lockup: wordmark centred under him with a 2 u gap. Minimum sizes: mark 16 px (u = 1.6 is not allowed, so 20 px), lockup u = 3 (117 x 30 px).

On dark grounds: same colours, plus a 1 px Paper #ECEEF1 outline on every pixel edge that touches the background (4-neighbour, like a sprite), and the wordmark in Paper #F4F5F7. Never a white Rommel. One-colour stamp: `#` black, `t` `e` `p` all white.

App icon: the Idle mark on the app's shell colour with 1 u of padding (a 12 x 12 tile), corners radius 1/6 of the tile. Files for all of this are in `logo/` and `rommel/` when the package is present.

The `!` emote over his head is 2 pixels wide in his own unit: a 2 x 3 bar, a one-pixel gap, a 2 x 1 dot, in columns 4 and 5, with one empty row above his ears.

## Motion and sound

Two kinds of motion. **UI chrome** (cursor, buttons, switches, sheets) may spring, scale and ease: cursor glide 240 ms spring with squash on landing; squish 80 ms down, 160 ms back; snap 180 ms overshoot; page slide 260 ms. **Pixel art** (Rommel, icons, wordmark, emotes, sparks) moves in whole pixels of its own grid on 60 Hz frame ticks, with `steps()`, never scaling, easing or fading. A 40 px dog on a 10-grid moves in 4 px steps.

Every press makes a short chip sound (`sounds/ui/`), every Rommel reaction has its own (`sounds/rommel/`). Sounds are quiet and under half a second except awoo.

## Boot screen (every app, once per launch)

A 64×40 dot matrix, one dot per logo pixel. Four claw taps; the ROWMI word steps down six rows on each (0, 110, 220, 330 ms) and lands on Rommel's chin line with a one-dot bump. The ping (G5 then C6) at 420 ms. At 510 ms, on the ring, all at once: Rommel alert with `!` and the shell colour rising four rows per frame for ten frames. 800 ms Rommel idle (the real logo). 900 ms app name in Silkscreen 24 under the lockup. 1100 ms done. Any input after 300 ms skips with a hard cut. Never a loader: draw the app behind it; if not ready, hold the shell with the launch bar. Reduced motion: finished screen for 800 ms, keep the sound. Sound: `sounds/startup.wav` (Trot & ping, rendered within Game Boy sound-chip limits).

## Layout and components

- 4 px spacing scale; pixel art on 8. Radii 6 outer, 4 inner. One separation device per group.
- One filled (ink) button per section: Rommel Black fill, Paper text, a round A glyph in the app's shell colour on the left; secondary buttons are Paper with a Cartridge Grey frame and the 2 px drop; destructive is red and specific. Buttons are 44 to 48 px tall, radius 6, label in Plex Mono 15, sentence case.
- App window chrome: a title bar in the app's shell with the lockup at u = 3 on the left (Rommel 30 px, word 81 x 15 px) and the app name in Silkscreen 16; nothing else in the bar. Body on Paper with the grid-paper ground at the content's pixel size.
- Console menu feel: arrows move, A opens, B goes back, L/R switch tabs, everywhere. Every gesture has a visible alternative. Touch targets 44 px.
- Grid paper is the default ground, scaled to the content's pixel unit. Patterns in `patterns/`: the pack, confetti, cartridge ribs, grid paper, bubbles. Patterns are fixed tiles on the 8 px grid; a safe zone around Rommel is left empty, never masked.
- Posts: text top left, Rommel bottom right, lockup bottom left, margins 10 % of the short side. One headline, one sentence.
- Empty, loading, error and success states always designed; Rommel carries them (Sleepy, launch bar, Sad, Happy).
- Easter egg: typing `bubbles` or tapping Rommel five times. Never on a timer, never blocking.

## Voice

Short, warm, plain. Say what happens, not what it's called. No "supercharge", no exclamation marks in UI copy, no guilt. Errors say what to try next. Rommel never speaks.

## Before shipping, check

Rommel and the wordmark drawn from the grids in this skill, never from memory or a font · Whole pixel units, no mixels · Silkscreen only at 8/16/24/32/64 · Plex Mono 400/600 only · one shell, text on it black · blue only for selection · Rommel Idle unless reacting; black on dark with the outline · frame depth only · pixel art stepped, never scaled or faded · boot once per launch, skippable, ends on the logo · every press has a sound · contrast 4.5 : 1 · 44 px targets · all four states designed · no dark patterns.
