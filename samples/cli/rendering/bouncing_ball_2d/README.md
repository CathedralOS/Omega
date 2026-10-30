# Bouncing ball in 2D

Animate a ball under gravity for 70 frames in a 24-column, 12-row terminal
area. The entry receives an explicit `main: &mut Main` context and forwards
it to each nested machine. From the repository root:

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

Both native execution and Terminal interpretation exit 0 on macOS ARM64.
The CLI supplies the host clock environment, so Terminal waits use the host
sleep implementation. Earlier checked-tree observations used a virtual-time
sleep adapter; the current native/Terminal qualification is recorded below.

## Scalar contract migration

This `CANONICALIZE-SCALAR-RANGE-CONTRACTS` customer retains its original
`7a9a7b8287e6` field restrictions on the containing data declarations:

`Main.frame`: `0 <= self.frame && self.frame <= 70`; `Main.y`: `0 <= self.y && self.y <= 12`; `Main.x`: `0 <= self.x && self.x <= 24`.

The original endpoints, guards and operations remain; 35 terminal calls
acknowledge the existing blocking-input path. `input.txt` supplies one newline
and then EOF. `expected_stdout.txt` pins the complete independently derived
21372-byte transcript; native and Terminal execution must both produce it and
exit 0. Other hosted targets retain their entry bindings and need
matching-host execution.

The binary64 trajectory keeps its original operation order and truncating
conversion. Zero-based frame 44 renders x=5: the stored position is just below
six. The oracle includes every frame and preserves that boundary.
