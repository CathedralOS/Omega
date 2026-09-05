# Interpreted Omega bootstrap experiment

This gate tests where Alpha tape production should resume after the selected
Gamma evaluator begins interpreting higher compiler sources.

The selected experiment keeps Epsilon execution interpreted and makes
`alpha_bootstrap` an ordinary Omega product target:

```text
Delta-written Epsilon evaluator + exact Epsilon-written Omega D
  -> interpreted Omega compiler D
  -> Omega C for alpha_bootstrap
  -> omega compiler Alpha tape
```

The gate requires the Omega product build to bind exactly one
`alpha_bootstrap::ProgramEntry`, requires Omega D to retain its Alpha tape
construction, and rejects any `EpsilonAlpha`/`epsilon_alpha_` backend residue in
the Delta-written Epsilon implementation. The evaluator is currently 9,566
lines / 477,364 bytes.

The executable slice runs complete checking, locates the fixed `Main::main`,
and executes an empty entry, scalar `let`, zero-initialized scalar receiver
fields and fixed arrays of `i32` or `u8`, scalar and indexed assignment/read,
grouped/local scalar reads, `assert`, all scalar operators, unary negation,
equality/order comparisons, and direct
`Console.write_byte` and `Console.exit_process` statements. It preserves output
before exit, overflow, `ByteRange`, and `Assertion` traps; non-Boolean assertions
trap separately. Indexed access evaluates the receiver field before its index,
uses zero-initialized per-index homes, and traps as `Bounds` before a read or
right-side evaluation. Byte reads zero-extend to `i32`. Byte stores check
`0..255` after evaluating the right side and before committing any update;
out-of-range values trap as `ByteRange`, without truncation or wrapping.
Every other entry statement remains explicitly
`Unsupported`; that staging outcome is not an Epsilon observation and cannot
survive in the final evaluator. The exact evaluator plus eight-line driver
compiles to a 563,256-byte Gamma receipt. Thirty-one retained controls cover
success, local, receiver-field, and fixed-array values, repeated mutation,
output, comparisons, bitwise/shift/division behavior, short-circuiting, bounds
ordering, byte-storage range boundaries, and trap prefixes. The explicit
`fixtures.tsv` inventory pins each fixture's bytes, digest, and expected
observation; the gate rejects missing or unlisted fixtures.

This is executable boundary evidence, not a completed interpreter edge.
Acceptance still requires execution of all Epsilon statements, expressions,
state transfers, traps, and Console observations, followed by exact composition
with D.
