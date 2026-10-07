Run with `omega run main.omg` (or `omega run --both main.omg`). Success is exit 0.

The binary64 singleton containing 0.1 rounds to the binary32 singleton containing
0.1 through the named conversion; Exact `as` cannot request lossy rounding.
Both helper return guarantees and the receiving local keep those rounded bounds.
The second helper exercises a fractional value inside a strict IEEE range, whose
upper endpoint must not undergo integer predecessor arithmetic.
