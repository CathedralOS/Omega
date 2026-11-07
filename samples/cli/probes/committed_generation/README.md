# Committed generation

A 4-cell line steps once, copies the proved next cells back onto the live
board, and steps again.

Seed `board = [0, 1, 1, 0]`. Cell 0 is born when the sum of `board[1]` and
`board[2]` is 2, and the other next cells are stored 0, so the first
next-board sum is 1. Commit copies each next cell into `board`. The copied
value is that cell after `cell_is_bit`, a proved 0..=1 fact rather than a
0/1 literal, and `cells_valid` holds again after the stores. The second step
sees `board = [1, 0, 0, 0]`, cell 0 dies, and the next-board sum is 0.

On that pair of sums the program exits 50.

An intended-invalid control in `invalid/` stores an unbounded `i32` into a
`cells_valid` cell and is rejected with
"data `Grid`'s default domain is still FALSE at state exit".

```sh
python3 tools/sample_native_gate.py --cli build/dev-target/debug/omega --target linux_x86_64 --filter simulation/committed_generation --jobs 1 < /dev/null
```
