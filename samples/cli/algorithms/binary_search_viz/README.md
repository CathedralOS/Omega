# Binary search visualization

The ordered grid contains 0 through 30; the target is 20. The search probes
15, 23, 19, 21 and 20. `expected_stdout.txt` records all five 31-cell rows and
the final prompt, including trailing spaces. `input.txt` supplies Enter.

Run `omega run --both --input input.txt --outputs build/acceptance main.omg`
from this folder. Both engines must exit zero and produce the exact output.
The sample gate also checks this input and output pair.

This customer belongs to `CANONICALIZE-SCALAR-RANGE-CONTRACTS`. Its original
`Main.x` interval, 0 through 31, is a standing data requirement. The terminal
counter value 31 remains valid; a store of 32 must reject. Midpoint and narrowing
clamps were present in the original program and remain part of its algorithm.
Calls that may reach the final blocking read acknowledge that effect explicitly.

The build binds all four hosted entries. Runtime qualification applies only to
the recorded host; this sample does not close the parent migration.
