# Bubble sort

From the repository root, run with a built Omega CLI:

```text
omega run --both samples/cli/algorithms/bubble_sort/main.omg
```

Press Enter at the closing prompt (EOF also works). Native execution and the
checked interpreter must both exit **10**, the minimum of the sorted input.

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
