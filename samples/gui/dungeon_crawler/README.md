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

## Engine

The engine uses exact arithmetic and default domains on its data records.
Positions, runtime vitality, countdowns, actor IDs and live storage counts carry
their bounds with their values; maximum health is actor data, not a global limit.

`actors` owns components, travel, combat and storage passes. `control` maps input
devices and pursuit controllers to actor commands, independently of actor kind.
One input device may control several actors. Ground versus air traversal is a
movement property, not a player/enemy branch. `world` owns room metadata and
campaign content, `simulation` orchestrates the tick, and `presentation` supplies
an immutable frame snapshot to the software renderer.

Actor, controller and dungeon storage are const-generic. The campaign currently
uses one adventurer and one active room enemy; input routing, movement, combat
and hostile scans traverse the actual stored work rather than assuming two
actors. Control and actor passes have explicit termination guarantees and
decreasing slice/work-count witnesses. The interactive application loop does
not promise termination.

`engine/configuration.omg` defines room and tile geometry, capacities, combat
timings, movement limits, frame dimensions, event budget and window scale.
Room layout decorations, spawn positions, health values and sprite artwork
remain authored campaign content.

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
are retained under `build/window-*.png`. It refuses a locked desktop before
launching or injecting input. The exact refactor's campaign and two original-ID
launch/input/close cycles are checked before landing; run identities and scoped
compiler regression comparisons are recorded with the change.

The separate `smoke/models` application checks a four-room/two-actor instance,
runtime health maximum 12, exact movement and terminating input fan-out. The
compiler corpus pins generic component/result/window composition and rejects
invalid generic bounds, unequal windows and nondecreasing mutable traversal.

The standard-library changes add a dimension-checked borrowed framebuffer
presenter and map Space, A, D and W in the existing macOS keyboard adapter.
The shared window creator also completes Cocoa launch and services a scheduled
stop through `NSApplication.run` before returning to the application-owned
frame loop. Merely creating a window and polling events could leave the
original application identity with an invisible, unresponsive window. The
same behavior was reproduced with a native Objective-C control; the bounded
application-loop initialization repaired it without changing identifiers or
adding another windowing library.

The exact modular engine also exercises compiler repairs for complete zero-domain
establishment, exact generic application and result identities, captured operand
predicates, generic field view types, private by-value writes and mutable-slice
ranking. These are compiler rules, not special treatment of this sample.