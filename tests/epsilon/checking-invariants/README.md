# Checking invariant controls

Run `sh tests/epsilon/checking-invariants/run.sh` from the repository root.
The selected Delta compiler compiles the complete manifested Epsilon evaluator
and this ordinary Delta test closure. The selected Gamma evaluator executes
the resulting receipt. Host code only composes, identifies, invokes, and
compares bytes; it does not implement checking or candidate selection.

Start at [main.delta](main.delta). [states.delta](states.delta) calls the
production `epsilon_validate_state_formed_types` entry with synthetic syntax.
The empty source backing and overlapping coordinates deliberately bypass
parsing and admission. These are checker-representation controls, not valid
Epsilon programs or source-language diagnostic expectations.

The two controls distinguish exact nested candidate selection:

- `min(TypeMismatch@10, min(InvalidArrayLength@50, TypeMismatch@50))`
  yields `Conflict@50`. Flattening the first state's parameter/body pair
  would incorrectly discard the later conflict and yield `TypeMismatch@10`.
- `min(TypeMismatch@50, min(InvalidArrayLength@50,
  min(TypeMismatch@70, InvalidArrayLength@70)))` yields `Conflict@70`.
  The completed suffix conflict absorbs the earlier ordinary candidates.
  Pre-collapsing both states into separate conflicts would instead yield
  `Conflict@50`.

[observations.delta](observations.delta) owns a private candidate codec:
`00` for absence, `01 + reason byte + u32 offset` for a candidate, and
`02 + u32 offset` for a conflict. Offsets are nonnegative little-endian values.
This codec is neither a final evaluator envelope nor source admission.
[expected.hex](expected.hex) independently fixes the ten observation bytes.

[checking_invariants.delta.sources](checking_invariants.delta.sources) binds
the three authored members. [receipt.tsv](receipt.tsv) binds the measured
701,234-byte Gamma receipt; the gate verifies that exact reconstruction before
execution.
Both subprocesses require status zero and empty stderr under separate
300-second watchdogs. Generated sources and receipts remain temporary.
