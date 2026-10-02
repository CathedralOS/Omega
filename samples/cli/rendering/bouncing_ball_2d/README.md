# Bouncing ball in 2D

Animate a ball under gravity for 70 frames in a 24-column, 12-row terminal
area. The entry receives an explicit `main: &mut Main` context and forwards
it to each named state. From the repository root:

```text
mbx run -p omega -- run --both samples/cli/rendering/bouncing_ball_2d/main.omg
```

Use Cargo if `mbx` is unavailable. Complete the ordinary
[package review](../../../../omega-rust/omega/application/cli/README.md#command-surface)
when requested. Leave `--target` off `run` to execute on the matching host.
Enter or EOF closes the native program.

The native stdout oracle begins with this line:

```text
bouncing_ball_2d: a ball under gravity animated in a 24x12 box for 70 frames
```

Each frame starts with the three cursor-home bytes `ESC [ H`, followed by
12 rows of exactly 24 ASCII cells and a newline. Each frame contains one
`O`; all other cells are spaces. Coordinates start at zero. The first ball
position is `(3, 1)` and the last is `(14, 11)`.

The initial position is `(3.0, 1.0)` and velocity is `(0.85, 0.0)`. After
rendering, add `0.45` to vertical velocity, then add velocity to position.
Clamp the floor at `y = 11`, reversing vertical velocity and multiplying
it by `0.72`; clamp the ceiling at `y = 0` and reverse vertical velocity.
Clamp the side walls at `x = 0` and `x = 23` and reverse horizontal velocity.
Coordinates for the next frame truncate these binary64 positions to integers.
The sample sleeps for 40 milliseconds after each update.

After the final frame, stdout ends with these two lines:

```text
done: 70 frames of gravity-bounce animation complete (exit 0)
[press Enter to close]
```

The complete stdout is 21,372 bytes, including all trailing spaces, cursor
escapes, and LF newlines. Its SHA-256 is
`5bc7bcff6f0250c1f0514e950aac320e93745128136f199346b351597ab42e53`.
This complete output was compared with an independent simulation on macOS
ARM64; native execution exits 0. Other declared hosted targets still require
matching-host execution.

The checked interpreter currently declines this program because it has no
provider for the selected `ClockNativeProvider::poll` realization. With
`--both`, the CLI reports `native exit 0` and this interpreter limitation on
stderr; its process exit of 0 does not establish interpreter execution.
