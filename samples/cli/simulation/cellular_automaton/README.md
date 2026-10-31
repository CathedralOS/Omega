# Cellular automaton

Rule 90 starts with the centre cell alive and renders 32 generations of a
64-cell grid. `expected_stdout.txt` records the complete output, computed from
the independent rule `next[i] = left XOR right` with dead cells outside the grid.
`input.txt` supplies Enter at the final prompt. Run with `omega run --both
main.omg`; both engines must exit zero and produce that output.

This sample exercises `CANONICALIZE-SCALAR-RANGE-CONTRACTS`: the two grids retain
their original per-element bounds, and `gen`, `rp`, `window`, `cell`, `nv`, `x`
and `cv` retain their original scalar bounds as ordinary data predicates.
The recursive `cells_valid` predicate establishes the grid invariant;
`cell_is_bit` carries it to selected reads. Guarded increments and indexed writes
restore the same obligations before nested calls and returns. No value clamp
substitutes for the original bounds.

The render path also carries current byte facts through nested calls: reading
the ASCII palette establishes the glyph fact, and a byte store preserves the
current ASCII line's UTF-8 domain. Replacing the line or using a multibyte palette
must not preserve that proof. The executable fixture
`tests/omega/pass/borrows/runtime_nested_byte_field_capture_exit` checks both
palette choices and neighboring bytes; its reject controls are
`nested_byte_field_capture_breaks_utf8` and
`nested_byte_field_capture_stale_carrier` under `tests/omega/fail/borrows`.

This customer demonstrates these original bounds through the complete checking,
Terminal Psi and native route. It does not close the parent migration or claim
execution on hosts other than the measured host.
