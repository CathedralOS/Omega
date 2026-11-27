# Parked Psi rewrite and whole-unit validators

These files held the independent validators for the parked
[stage 03 Psi rule engine](../../../pipeline/03_abstract-operations-to-abstract-operations/README.md):
`candidates.rs` replays and checks every Psi rewrite candidate, and
`current_value_ranges.rs` checks the value-range facts the rules consumed.
`lib.rs`, `unit_validation.rs` and `current_ownership.rs` are the full files as
they stood at `079e25a8735569a47488b13c39b20d26219efcf2`. They include whole-unit validation
(`validate_psi_optimization_unit_with_admitted_cycle_machines`), the
ownership-frontier reconstruction the rules read, the current-ownership replay
of claims, partial-affine moves and loans, and their tests. Only the Psi rules
and those validators' own tests called these entry points.

The active crate keeps what native stage 04 and image emission call:
`callable_values`, `descriptor_results`, `access_references`, the
structural storage projections, and the parts of `unit_validation.rs` and
`current_ownership.rs` those reach. When a Psi rule migrates to source stage
06, its validator follows it there and checks Terminal Psi; the ownership
meaning it needs comes from `psi/semantics`, not a copy of this replay.
