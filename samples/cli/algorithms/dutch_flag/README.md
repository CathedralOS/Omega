# Dutch flag

Dijkstra's three-pointer in-place partition over a real enum array: everything
below `w` is Red, everything from `b` up is Blue, and the White cursor walks
the unknown middle until it meets the fence. `[White, Blue, Red]` becomes
`[Red, White, Blue]`; the program exits **70**.

The sample uses the field-counter idiom: plain `Exact` counters plus
literal re-guard states dominate every runtime-indexed access, and the
`confused(9x)` arms stand as honest runtime checks for the loop invariants a
verified port would carry as `requires`/`ensures` facts.

## Run and check

From the repository root, after completing the checkout's ordinary package
review:

```sh
omega run --both samples/cli/algorithms/dutch_flag/main.omg
```

Press Enter at the final prompt, or supply EOF. On macOS's shell, append
`< /dev/null` for a noninteractive run. PowerShell can pipe an empty line:
`'' | omega run --both samples/cli/algorithms/dutch_flag/main.omg`.

The program partitions `[White, Blue, Red]` in place, checks all three resulting
elements, prints `PASS: the flag reads [Red, White, Blue] (exit 70)`, and exits
70. Both native execution and the checked interpreter must report 70.

## Host status

- macOS ARM64: the explicit-parameter migration is under validation; native
  execution and checked interpretation must both retain exit 70.
- Windows x86-64 and both Linux hosts need current matching-host execution;
  coverage tracking lives in [SAMPLE-CORPUS](../../../../TASKS.md).
