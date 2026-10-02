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

The named entry forwards its context through the states. Both engines exit 0
on macOS ARM64. Each displayed row has two trailing spaces in the actual output,
making 16 bytes before LF; the complete native stdout, including banner, PASS
line and closing prompt, is 235 bytes. All five rows and the full output were
compared exactly.

This example composes guarded array reads/writes, loops, integer arithmetic,
and byte rendering. Proof search must retain proved consequences and share case
selection across those operations; retrying permutations of the same facts can
make compilation time out without changing which obligations are provable.

For noninteractive native acceptance with package-review preparation, use
Python 3 (`python3` on macOS; `python` on Windows):

```text
python3 tools/sample_native_gate.py --cli target/release/omega --filter algorithms/maze_flood --jobs 1 --timeout 45
```

On Windows, use `target/release/omega.exe` for `--cli`. The gate checks the
authored exit/output fragments and accepts package review for measurement;
it is not a package audit.
