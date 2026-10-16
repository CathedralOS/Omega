# Profile slot index

Dose-response profile endpoints live in an 8-element table, two sides for
each of four parameters. The slot is `position * 2 + side` with `side` exactly
0 or 1. Exact `i32` accepts that product when the disjunction
`side == 0 || side == 1` contributes the convex hull `[0, 1]`. A one-sided
disjunction such as `side == 0 || flag == 1` still leaves `side` unbounded.

The program stores 2.0 at slot (1, 0) through a local and 7.0 at slot (3, 1)
through the call result, then repeats slot (1, 0) with the mirrored spelling
`0 == side || 1 == side`. It prints `slots=2,7` and exits 0.

The invalid control multiplies `side * 2` under `side == 0 || flag == 1` and
must still be rejected: the flag alternative never bounds `side`.
