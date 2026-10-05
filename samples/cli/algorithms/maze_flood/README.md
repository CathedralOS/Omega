# Maze flood

From the repository root, run with a built Omega CLI:

```text
omega run --both samples/cli/algorithms/maze_flood/main.omg
```

Press Enter at the closing prompt (EOF also works). Native execution and the
checked interpreter must both exit **0**. The maze's shortest distances are:

```text
 0  1  2  3  4
## ## ## ##  5
10  9  8  7  6
11 ## ## ## ##
12 13 14 15 16
```

The entry context holds only the Console binding. The entry creates a local
`MazeSearch` and passes it explicitly to static machines and states. Cell,
direction, and rendering temporaries are local bindings or state parameters;
each output line and the closing input buffer have their own local storage.

Each displayed row has two trailing spaces, making 16 bytes before LF. The
complete expected stdout, including banner, PASS line and closing prompt, is
235 bytes.

This example composes guarded array reads/writes, loops, integer arithmetic,
and byte rendering. `render_row` returns a Boolean while mutating a borrowed
ASCII-qualified byte owner. It must update the caller's actual line without
changing its live length or losing its ASCII invariant.

For noninteractive native acceptance with package-review preparation, use
Python 3 (`python3` on macOS; `python` on Windows):

```text
python3 tools/sample_native_gate.py --cli target/release/omega --filter algorithms/maze_flood --jobs 1 --timeout 45
```

On Windows, use `target/release/omega.exe` for `--cli` and add
`--target windows_x86_64`. The gate checks the
authored exit/output fragments and accepts package review for measurement;
it is not a package audit.
