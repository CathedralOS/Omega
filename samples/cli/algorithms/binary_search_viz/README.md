# Binary search visualization

The ordered grid contains 0 through 30; the target is 20. The search probes
15, 23, 19, 21 and 20. `expected_stdout.txt` records all five 31-cell rows and
the final prompt, including trailing spaces. `input.txt` supplies Enter.

From this folder on an ARM64 Mac, run:

```sh
omega run --target macos_arm64 --build-input build.omg --build-input product.omg --build-input main.omg --input input.txt --outputs build/acceptance build.omg
```

Both engines must exit zero and produce the exact output.
The sample gate also checks this input and output pair.

This customer belongs to `CANONICALIZE-SCALAR-RANGE-CONTRACTS`. Its original
`Main.x` interval, 0 through 31, is a standing data requirement. The terminal
counter value 31 remains valid; a store of 32 must reject. Midpoint and narrowing
clamps were present in the original program and remain part of its algorithm.
Calls that may reach the final blocking read acknowledge that effect explicitly.

The first Build supplies the system package to `product.omg`. That stage grants
the application access to the clock, standard input, standard output and process
exit on ARM64 macOS, x86-64 Windows, and ARM64 and x86-64 Linux.
