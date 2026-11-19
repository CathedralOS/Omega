# Ripple field

## Scalar contract migration

This `CANONICALIZE-SCALAR-RANGE-CONTRACTS` customer retains its original
`7a9a7b8287e6` field restrictions on the containing data declarations:

`Main.frame`: `0 <= self.frame && self.frame <= 60`; `Main.y`: `0 <= self.y && self.y <= 16`; `Main.x`: `0 <= self.x && self.x <= 30`; `Main.dx`: `-15 <= self.dx && self.dx <= 15`; `Main.dy`: `-8 <= self.dy && self.dy <= 8`; `Main.dist`: `-23 <= self.dist && self.dist <= 23`.

The original endpoints, guards and operations remain; 30 terminal calls
acknowledge the existing blocking-input path. `input.txt` supplies one newline
and then EOF. `expected_stdout.txt` pins the complete independently derived
30074-byte transcript; native and Terminal execution must both produce it and
exit 0. Other hosted targets retain their entry bindings and need
matching-host execution.
