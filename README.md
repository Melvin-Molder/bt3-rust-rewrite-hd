# BT3 Recomp HD

The enhanced edition of [BT3 Recomp](https://github.com/Melvin-Molder/bt3-rust-rewrite), an independent
reimplementation of **Dragon Ball Z: Budokai Tenkaichi 3** for the PlayStation 2 (USA release, SLUS-21678),
written in Rust with [Bevy](https://bevyengine.org/). It runs on a PC and reads everything it needs (models,
stages, textures, sounds, scripts) from your own disc image as it runs.

This edition adds modern options on top of the faithful port: **high internal resolution** and **HD texture
packs**, alongside the full-screen scaling and keyboard and mouse controls. Every one of them is optional. With
no options the game is drawn exactly as the console draws it, pixel for pixel the same as the original project.

This repository contains **no game data, no copy of the game, no texture pack and no decompiled code**. To run it
you need your own disc image of that release.

## What's new in this edition

| Feature | Option | What it does |
|---|---|---|
| High internal resolution | `--internal-scale 2..4` | Draws every pass at 2 to 4 times the PS2's 512 x 448 (up to 2048 x 1792). Polygons, effects, post effects and outlines are drawn at the higher resolution while the game's coordinates stay the console's, so nothing moves or changes size. |
| HD texture packs | `--textures DIR` | Loads replacement textures made for the game under PCSX2 (DDS or PNG), so community packs such as Sal9im's AI-upscaled pack work as they are. Each texture is named as PCSX2 names it and swapped in when it is uploaded. |
| Full screen and window scaling | `--fullscreen`, `--aspect`, `--filter`, `--scale`, F11 | The finished frame scaled to any window or screen at 4:3, square pixels or stretched, smooth or sharp. |
| Keyboard and mouse | (always on) | Dragon Ball: Sparking! ZERO's PC layout. A gamepad works as the PS2 pad. |

The first two are this edition's own; the others come from the original project. The faithful 512 x 448 output
stays the default.

### Recommended setup

Full screen at three times the resolution, with an HD pack:

```
cargo run --release -p bt3-game -- --fullscreen --internal-scale 3 --textures "textures/SLUS-21678"
```

| Screen | `--internal-scale` |
|---|---|
| 1080p | 3 (drawn at 1344 lines, scaled down: a little extra smoothing) |
| 1440p | 3 |
| 4K | 4 |

If it stutters, use one step lower.

### Texture packs

1. Download a PCSX2 pack for the USA release, for example
   [Sal9im's AI Upscaled Texture Pack (SLUS-21678)](https://gbatemp.net/threads/dragon-ball-z-budokai-tenkaichi-3-slus-21678-ai-upscaled-texture-pack.659662/).
2. Extract it anywhere, for example to `textures/` at the top of the repository (ignored by git).
3. Pass its `SLUS-21678` folder with `--textures`. Its `replacements` folder is read, at any depth.

A pack's optional extras (such as alternative button prompts or fonts) work as PCSX2 uses them: copy them into
`replacements`. With Sal9im's pack about nine in ten of the textures a battle uses are replaced; textures
smaller than one GS block (16 x 16 at 8 bits, 32 x 16 at 4 bits) are not, as PCSX2 names those another way.
Files are read the first time a texture is used, so loading a battle takes a moment longer.
`BT3_TEX_LOG=1` prints each texture looked up, found or missing, with the file name it would need.

## Status

Every mode reachable from the main menu is rebuilt, from the boot screens to the ending:

| Area | State |
|---|---|
| Boot, logos, opening movie, title, main menu, options, saves | Done |
| Dragon History (story): sagas, episodes, scripted battles, results, ending | Done |
| Duel: 1P vs COM, 2P split screen, team and DP battles | Done |
| Ultimate Battle: Mission 100, Survival, Sim Dragon, Disc Fusion, Ultimate Battle Z | Done |
| World Tournament, Training, Evolution Z (customising, shop), Data Center, Shenron | Done |
| Battle engine: fighters, techniques, transformations, clashes, computer opponent, effects, cameras, HUD, replays | Done |
| Sound: the game's sound driver emulated (sequenced effects, music, voices, announcer) | Done |

The port follows the original's code function by function, and each Rust function names the original one it
rebuilds. A few visuals that depend on exactly how the PS2's graphics chip draws (a shadow's destination-alpha
test, the objects' distance fade) are left out rather than guessed. The opening and ending movies are decoded by
`ffmpeg`. [PROGRESS.md](PROGRESS.md) has the full list, and the details are in
[IMPLEMENTATION.md](IMPLEMENTATION.md).

## Supported disc

| Title | Region | Product code | Executable |
|---|---|---|---|
| Dragon Ball Z: Budokai Tenkaichi 3 | USA (English / Japanese) | SLUS-21678 | `SLUS_216.78` |

Only this release is supported: the port reads the game's files by their places on this disc. Other releases
(PAL, Japanese) are untested and not expected to work. At start the port checks the disc's executable and warns
when it is not `SLUS_216.78`.

## Running

You need:

- your disc image of the supported release (above), in a `rom/` folder at the top of the repository. Any file
  name works; to run an image from elsewhere, pass its path as the first argument;
- a [Rust](https://rustup.rs/) toolchain (edition 2024);
- `ffmpeg` on your `PATH`, for the movies.

```
cargo run --release -p bt3-game
```

Saves go to the `memcard/` folder. The game is drawn at the PS2's own 512 x 448 and only the finished frame is
scaled to the window, at 4:3 as a television shows it.

| Option | Effect |
|---|---|
| `--fullscreen` | Start in full screen (F11 switches at any time) |
| `--aspect tv \| pixel \| stretch` | 4:3 (the default), square pixels, or fill the window |
| `--filter linear \| nearest` | Smooth (the default) or sharp scaling |
| `--scale N` | The window's starting size, 1 to 8 (default 2) |
| `--internal-scale N` | Optional enhancement, off by default: draw at N times 512 x 448 (2 to 4); 1 is the console's resolution |
| `--textures DIR` | Optional enhancement, off by default: replace textures from a PCSX2 texture pack (its `SLUS-21678` folder) |
| `--title`, `--menu` | Skip to the title screen or the main menu |
| `--battle` | Go straight to a battle (`--stage N`, `--mode N`, `--split` for 2P) |
| `--memcard DIR` | Use another save folder |
| `--disc FILE` | A second disc for Disc Fusion's disc swap (F1 opens the tray, F2 changes the disc) |

### Controls

A gamepad works as the PS2 pad. The keyboard and mouse follow Dragon Ball: Sparking! ZERO's PC defaults: each key
does the same action here (it presses the PS2 button that does that action in this game).

| Battle | Key | PS2 |
|---|---|---|
| Move | W A S D | D-pad |
| Swing the camera to a side (hold) / pick the member a switch brings in | Arrow keys | Right stick |
| Melee (rush) | Left mouse button | Square |
| Ki blast | Right mouse button | Triangle |
| Guard | E | Circle |
| Dash / step | F | Cross |
| Ascend / descend | Space / Left Ctrl | R1 / R2 |
| Ki charge | Left Shift | L2 |
| Lock-on | Q | L1 |
| Transform (hold: fuse) | Z | R3 |
| Switch member | X | L3 + R3 |
| Skill 1 / 2 | 1 / 2 | L2 + Circle / L2 + Up + Circle |
| Blast 2 (first / second) | 3 / 4 | L2 + Triangle / L2 + Up + Triangle |
| Ultimate Blast | 5 | L2 + Down + Triangle |
| Pause | Tab or Enter | Start |
| Camera distance (its "switch camera") | C | Select |

Sparking! ZERO's PC key list has no lock-on, so Q is this port's choice. In the menus, including the battle's pause and result menus: W A S D or the arrows move, Space confirms, Escape or Backspace goes
back, Tab or Enter is Start, Q / E are L1 / R1, 1 / 3 are L2 / R2, and C is Circle.

## Building and testing

```
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace
```

Many tests run against the real game: they read the disc image from `rom/` and are skipped when it is not
there. `cargo run -p bt3-data --bin bt3-inspect` prints what the reader finds on the disc.

## Layout

| Path | Contents |
|---|---|
| `crates/bt3_data` | Reading the disc: ISO9660, the AFS archives, compressed packs, and each resource format (models, motions, stages, textures, fonts, sound banks, Flash screens, ADX audio) |
| `crates/bt3_game` | The game: the battle engine, every menu and mode, rendering and sound, in Bevy |
| [PROGRESS.md](PROGRESS.md) | What is done, what is left, and the audits of the port against the original |
| [IMPLEMENTATION.md](IMPLEMENTATION.md) | How each part of the game is rebuilt, in detail |
| [bt3-format-research.md](bt3-format-research.md) | The research the file formats are based on |

## Related

- [z3xox/BT3-Decompiled](https://github.com/z3xox/BT3-Decompiled), a matching decompilation of the game (CC0
  1.0). The port was written against it as a reference: its names for the original functions are the ones the
  Rust code cites. No decompiled code was copied.
- [gang-beasts-rust](https://github.com/muffinmxn/gang-beasts-rust) (MIT) informed the split between the data
  and game crates, and [hacktools](https://github.com/Illidanz/hacktools) (MIT) and
  [PS2Tek](https://israpps.github.io/ps2tek/PS2/GS/Textures.html) the texture formats. No code was copied from
  either.
- [z3xox/Tenkaichi3Decomp](https://github.com/z3xox/Tenkaichi3Decomp) (MIT), a PC port of the game, worked out
  how PCSX2 names this game's replacement textures; `--textures` follows that naming. No code was copied.
- The rest of the format research is credited in [bt3-format-research.md](bt3-format-research.md).

This project was made with the help of AI.

## Licence

The code in this repository is released under the [MIT Licence](LICENSE). That covers this project's own work only:
the game, its data and its trademarks belong to their rights holders.

## Legal

This project is not affiliated with or endorsed by the game's developers, publishers or rights holders. It
exists for preservation, study and interoperability. All rights to the game belong to their owners. You need
your own legally obtained copy of the game to use anything here.
