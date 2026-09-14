# Longest equal-byte run

The complete scanner walks `aaabbbbccdaa`, prints its result, consumes one input
line, and exits with **4**. Preserve the runtime byte loop and its bounded cursor;
unrolling the input or changing its types does not establish this sample.

From the repository root on macOS:

```sh
OMEGA_SAMPLE_RUNTIME_FILTER=longest_run RUST_MIN_STACK=67108864 \
  cargo nextest run -p compiler --test samples_compile --no-fail-fast \
  -E 'test(=samples_with_documented_exit_run_correctly)'
```

PowerShell:

```powershell
$env:OMEGA_SAMPLE_RUNTIME_FILTER = 'longest_run'
$env:RUST_MIN_STACK = '67108864'
cargo nextest run -p compiler --test samples_compile --no-fail-fast `
  -E 'test(=samples_with_documented_exit_run_correctly)'
```

Use `mbx` instead of `cargo` when available. The harness supplies the input line
and checks the exit code; running only source checking is not acceptance.

## Compiler dependencies

The outer command currently fails before execution with
`selected ProgramEntry establishment rejoins 0 Terminal attachment identities; expected one`:
the complete scanner body does not yet produce its Terminal entry. This is not
an observed wrong native exit.

Bounded scalar field writes carry independently checked pre-write range proofs.
The native regression `bounded_integer_field_stores_run_natively` checks signed
literal initialization followed by guarded `i32` addition and negative `i8`
subtraction in one body under a conjunctive guard, with updated values observed
by the caller after return. Disjoint writes preserve the other guard facts;
short-circuit control reaches the writes without merging Boolean outcomes first.
Native selection uses its existing checked edge bridges when conditional
fallthrough destinations conflict. The source
regression
`guarded_bounded_integer_field_increment_publishes_checked_terminal` checks the
combined updates through Terminal production and rejects redirected guard edges.
Neither closes this scanner.

Live bounded byte-field lengths reach verified Terminal interpretation through
ordinary scalar operands. The field-length regression observes a nested field's
length after a helper shrinks it and after empty replacement, without substituting
capacity or changing sibling contents.

Remaining dependencies include field-backed byte reads and native byte-field
length/replacement. After those operations compose, the complete state
loop must still pass its termination/proof and native execution checks. Keep this
command as the outer acceptance check rather than deriving completion from a
passing isolated store test.
