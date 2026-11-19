# Bubble-sort visualizer

This customer of `CANONICALIZE-SCALAR-RANGE-CONTRACTS` preserves five field intervals from `7a9a7b8287e6`: `i` and `j` stay in `0..=7`; `jp`, `ry` and `cx` stay in `0..=8`. The common data predicate retains both endpoints, including terminal render counters of eight. Loop guards and the sorting/rendering operations remain unchanged. The 34 terminal calls acknowledge the existing blocking-input path.

`input.txt` supplies one newline and then EOF. `expected_stdout.txt` pins all eight frames, their home escape sequences and padded rows, and the final prompt: 754 bytes. The last frame contains bars of lengths one through eight. Native and Terminal execution must both exit zero and produce those exact bytes.

The expected frames were independently derived from the initial array and seven bubble-sort passes before execution. The historical field contracts are formation restrictions as well as use-site bounds: out-of-range stores must reject, even if a later index guard would avoid reading them.
