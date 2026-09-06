# Runtime invariant controls

Run `sh tests/epsilon/runtime-invariants/run.sh` from the repository root.
The selected Delta compiler compiles the complete manifested Epsilon evaluator
plus this ordinary Delta test closure. The selected Gamma evaluator executes
the resulting receipt. Host code only packs, identifies, invokes, and compares
bytes; it does not parse Epsilon or implement its runtime behavior.

Start at [`main.delta`](main.delta). The concept-owned controls call production
helper entry points with synthetic runtime values and ASTs. Those inputs
deliberately bypass checking; they are not admitted Epsilon programs and do
not establish source-language conformance.

- [`projections.delta`](projections.delta): nine outcomes distinguish malformed
  aggregate view storage (`Internal` at type starts 17 and 29), an absent
  ordinary field fact (`Internal` at expression start 41), and valid array/view
  lengths, reads, and bounds traps.
- [`control.delta`](control.delta): thirteen outcomes cover all twelve reachable
  non-call core variants plus a valid resultless return. Nested grouping must
  retain core offset 67. Binary and projection children would trap if wrongly
  executed. Other offsets extend beyond 255 to expose truncation. The grouped
  match arm itself is unreachable after normalization and is not claimed as
  covered by constructing another group.
- [`scalars.delta`](scalars.delta): nine outcomes distinguish logical operators
  incorrectly sent to the eager scalar helper (`Internal` at 0) from ordinary
  arithmetic, arithmetic traps, and correctly short-circuited evaluation. A
  reached missing local reports offset 107; skipped copies cannot do so.

Every tested runtime call receives an existing stdout prefix `A`. The private
codec in [`observations.delta`](observations.delta) encodes helper variants:
`00 + u32 value + stdout` for scalar values, `01 + kind + stdout` for traps,
and `03 + u32 offset` for internal failure. The retired staging tag `04`
remains unassigned.
Resultless completion is `05 + stdout`; other variants have distinct tags.
All integers encoded here are nonnegative and little-endian. Internal failures
carry no output. These are helper-outcome observations,
not a test of the production final observation adapter or a RunEpsilon envelope.

[`expected.hex`](expected.hex) independently specifies 152 bytes for the 31
outcomes, in source order. The exact test source membership is recorded in
[`runtime_invariants.delta.sources`](runtime_invariants.delta.sources).
[`receipt.tsv`](receipt.tsv) binds the measured 716,976-byte Gamma receipt;
reconstruction must match its exact length and digest before execution.
Generated source and receipts remain outside the repository.
