# Array sum

## Scalar contract migration

This `CANONICALIZE-SCALAR-RANGE-CONTRACTS` customer retains its original
`7a9a7b8287e6` field restrictions on the containing data declarations:

`Main.i`: `0 <= self.i && self.i <= 2`.

The original endpoints, guards and operations remain; 6 terminal calls
acknowledge the existing blocking-input path. `input.txt` supplies one newline
and then EOF. `expected_stdout.txt` pins the complete independently derived
170-byte transcript; native and Terminal execution must both produce it and
exit 70. Other hosted targets retain their entry bindings and need
matching-host execution.

The input and expected-total constants use equivalent hexadecimal spellings
to expose the 32-bit sign boundary. Their values, widening and Wrapping
accumulator are unchanged.
