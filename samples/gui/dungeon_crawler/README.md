# Hollow

Hollow is a dungeon game written in Omega for macOS ARM64 and Windows x86-64.
Omega draws the 256×192 framebuffer and handles gameplay and collision. Cocoa
and Win32 present the pixels at 4× scale and deliver input.

## Play

Run the native application from the repository root with the command below.

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

`engine/configuration/configuration.omg` defines room and tile geometry, capacities, combat
timings, movement limits, frame dimensions, event budget and window scale.
Room layout decorations, spawn positions, health values and sprite artwork
remain authored campaign content.

## Build and run

From the repository root, build Omega and review the game's dependencies for
the host target. After accepting the decisions in the generated proposal,
resume the update and run the game.

```sh
mbx build -p omega
target/debug/omega packages update --project samples/gui/dungeon_crawler
target/debug/omega packages update --resume --project samples/gui/dungeon_crawler
target/debug/omega run --engine native samples/gui/dungeon_crawler/build.omg
```

## Acceptance

The campaign and component checks are sibling applications that import the
same engine as the windowed game. Review dependencies in each application's
directory before its first run.

The campaign covers collision, doorway travel and backtracking, directional
combat, persistent defeat, the key and locked passage, victory, and selected
framebuffer pixels. Both engines print
`PASS: dungeon campaign, collision, combat, pixels` and exit 0 on Windows
x86-64. The Windows desktop check also opened the 1024×768 window,
sampled its pixels, and closed it. [Windows verification](https://github.com/CathedralOS/Omega/pull/6499)

```sh
target/debug/omega run --engine native samples/gui/dungeon_crawler_smoke/build.omg
target/debug/omega run --engine psi samples/gui/dungeon_crawler_smoke/build.omg
```

The component model uses four rooms, two actors and three controller bindings.
It checks runtime health, exact movement, knockback and input fan-out. The
third binding receives input only after the model increases the live count.
The expected line is `PASS: bounded components and terminating controller fan-out`.

```sh
target/debug/omega run --engine native samples/gui/dungeon_crawler_models/build.omg
target/debug/omega run --engine psi samples/gui/dungeon_crawler_models/build.omg
```

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