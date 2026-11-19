# Recursive sum

Two value-returning recursive machines sum `[5, 10, 15, 20]` and count its
elements. The caller verifies sum 50 and count 4 before printing success.
Preserve the shrinking slice, accumulator reads, and two sequential calls.

After reviewing this checkout's package closure, run from the repository root:

```text
omega run --both samples/cli/arithmetic/recursive_sum/main.omg
```

EOF and Enter must each complete with native and Terminal exit 70. The complete
native stdout is 144 bytes, with LF after each line including the final line:

```text
recursive_sum: recursive slice sum + count via value-returning machines
PASS: sum=50 and count=4 both verified (exit 70)
[press Enter to close]
```

The program emits no stderr; the CLI separately reports the two exits.
The combined CLI route compares exits, so verify native output bytes as well.

Windows x86-64 is verified at `b80e3d12bdd` with an optimized dev compiler
after local package review, using EOF and finite Enter input. Remaining
matching-host coverage is owned by [SAMPLE-CORPUS](../../../../TASKS.md).
