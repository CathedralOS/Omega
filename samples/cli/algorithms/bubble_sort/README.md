# Bubble sort

From the repository root, run with a built Omega CLI:

```text
omega run --both samples/cli/algorithms/bubble_sort/main.omg
```

Press Enter at the closing prompt (EOF also works). Native execution and the
Terminal interpreter must both exit **10**, the minimum of the sorted input.

This unchanged example composes nested loops, guard-derived adjacent indices,
field store/read chains, and runtime-indexed swaps. Bounds must survive those
ordinary operations and selected control-flow edges; recognizing the sort's
source shape is not an acceptable substitute for checking their evidence.

For noninteractive native acceptance with package-review preparation, use
Python 3 (`python3` on macOS; `python` on Windows):

```text
python3 tools/sample_native_gate.py --cli target/release/omega --filter algorithms/bubble_sort --jobs 1 --timeout 45
```

On Windows, use `target/release/omega.exe` for `--cli`. The gate accepts package
review for measurement; it is not a package audit.

## Scalar contract migration

This `CANONICALIZE-SCALAR-RANGE-CONTRACTS` customer retains its original
`7a9a7b8287e6` field restrictions on the containing data declarations:

`Main.i`: `self.i <= 5`; `Main.j`: `self.j <= 4`.

The original endpoints, guards and operations remain; 15 terminal calls
acknowledge the existing blocking-input path. `input.txt` supplies one newline
and then EOF. `expected_stdout.txt` pins the complete independently derived
183-byte transcript; native and Terminal execution must both produce it and
exit 10. Other hosted targets retain their entry bindings and need
matching-host execution.
