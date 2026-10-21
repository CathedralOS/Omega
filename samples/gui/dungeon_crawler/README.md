# Hollow

A native macOS dungeon game written in Omega, with an original 256x192 CPU
framebuffer displayed at 4x scale. Cocoa supplies the window, event pump and
pixel presentation; gameplay, collision, sprites and rasterization are Omega.

## Play

Open `build/native/dungeon_crawler.app` after building.

- Arrow keys or WASD: move and face.
- Space: swing the sword. Holding it repeats attacks.
- R: restart the dungeon.
- Escape or the window close button: quit.

Walk through the openings in room walls to travel. Bats pursue on entering
their room; sword attacks hit only in front of the player. Defeated enemies
stay defeated when revisiting. Clear the archive and approach its chest for
the key. The key opens the sanctum, whose guarded relic completes the game.
The cistern chest restores health once. The six-room map records exploration.
Input and simulation pause when the window loses focus. There is no save file.

## Build

From the repository root, using the optimized development compiler:

```sh
cargo build -p omega
target/debug/omega update --project samples/gui/dungeon_crawler
# Review the generated decisions, then resume the accepted proposal:
target/debug/omega update --resume --project samples/gui/dungeon_crawler
target/debug/omega --build-dir samples/gui/dungeon_crawler/build/native samples/gui/dungeon_crawler/main.omg
open samples/gui/dungeon_crawler/build/native/dungeon_crawler.app
```

After editing source, refresh the package review before rebuilding. The local
build and accepted lock are ignored. Only macOS ARM64 has been implemented and
executed; this sample does not claim Windows or Linux support.

## Acceptance

The `smoke` application imports the same `engine` package as the windowed game.
It checks room geometry, wall collision, doorway travel and backtracking,
directional combat, persistent defeat, key acquisition, locked passage,
victory, and representative framebuffer pixels. Its authored result is exit 0
and `PASS: dungeon campaign, collision, combat, pixels`.

```sh
target/debug/omega update --project samples/gui/dungeon_crawler/smoke
# Review and resume as above.
target/debug/omega run --both samples/gui/dungeon_crawler/smoke/main.omg
```

The native desktop regression requires an unlocked macOS desktop, Accessibility
and Screen Recording access. Close any existing copy, then run:

```sh
python3 samples/gui/dungeon_crawler/verify_window.py --cycles 2
```

It launches the original `.app`, requires its window to be on screen, checks
that actual keyboard input changes rendered pixels, clicks the red close
button, and verifies that the process exits before the next launch. Captures
are retained under `build/window-*.png`. Both launch/input/close cycles were
observed passing under the unchanged `com.omega.dungeon-crawler` identifier.
Those interactive observations used producer `ba7501de328` before integration.
A locked desktop prevented repeating input/close observations on the integrated
producer; the test refuses that environment before launching the game. The
integrated producer at `1636562ddf3` still completed the campaign in both engines
and rendered all 60 frames of the existing window demo with exit 0.

Native and Terminal Psi executions both observed exit 0 on macOS ARM64 with
the compiler built from `ba7501de328` and the accompanying local standard-library
changes. A native window was captured successfully. The existing `window_app`
sample also checks against the retained fixed-size blit API, and `window_demo`
renders all 60 frames and exits 0 against the repaired shared Cocoa host.

The standard-library changes add a dimension-checked borrowed framebuffer
presenter and map Space, A, D and W in the existing macOS keyboard adapter.
The shared window creator also completes Cocoa launch and services a scheduled
stop through `NSApplication.run` before returning to the application-owned
frame loop. Merely creating a window and polling events could leave the
original application identity with an invisible, unresponsive window. The
same behavior was reproduced with a native Objective-C control; the bounded
application-loop initialization repaired it without changing identifiers or
adding another windowing library.
No Rust compiler changes were necessary for this game.