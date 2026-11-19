# Bouncing console

## Scalar contract migration

This `CANONICALIZE-SCALAR-RANGE-CONTRACTS` customer retains its original
`7a9a7b8287e6` field restrictions on the containing data declarations:

`Main.p`: `0 <= self.p && self.p <= 40`; `Main.frame`: `0 <= self.frame && self.frame <= 50`; `Main.x`: `0 <= self.x && self.x <= 40`.

The original endpoints, guards and operations remain; 26 terminal calls
acknowledge the existing blocking-input path. `input.txt` supplies one newline
and then EOF. `expected_stdout.txt` pins the complete independently derived
2213-byte transcript; native and Terminal execution must both produce it and
exit 0. Other hosted targets retain their entry bindings and need
matching-host execution.
