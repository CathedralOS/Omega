# Restricted fixed-fuel checking

The [logical-work specification](../../../../wiki/spec/resources/logical_work.md)
owns the theorem and evidence-role rules. Enter
[fuel_certification.rs](src/fuel_certification.rs) for certificate derivation,
whole-roster replay, and sealing. Public evidence records remain in
[lib.rs](src/lib.rs). Subordinate owners separate
[segment partitioning](src/fuel_certification/segment_partition.rs) and
[outcome composition](src/fuel_certification/outcome_bounds.rs).

Ordinary acyclic derivation computes outcome-sensitive maximum paths and the
complete reachable segment partition. Keep whole-entry, segment, and
analysis-only ranked catalogs distinct in constructors and installation binders.

Each ordinary segment/catalog invocation prepares machine, dynamic-call, and
selected-machine block lookups once. Exact module identity is computed at most
once, when first required; an empty unsealed roster still needs none. The
reachable partition and its rows share callee outcome summaries. Derive-and-seal
still reconstructs and compares the complete roster a second time, borrowing
only immutable preparation and starting with fresh outcome working state.
External replay prepares everything anew; no cache crosses invocations or
semantic subjects. Callee block lookups are still built on each fresh outcome
derivation, and segment paths are still charged separately for each row.

## Exact countdown implementation

The separate unsigned-countdown verifier carrier does not widen ordinary
acyclic admission. Its whole-entry formula is:

```text
preheader + (upper_bound - lower_bound) * (header + decrement)
          + header + exit
```

Costs come from the current schedule, with checked arithmetic and checked final
`u64` conversion. The exact `u32` source-countdown control currently yields
`5 + 6n`, with all-input ceiling `25_769_803_775`.
Its separate safe-point catalog has five block-local per-traversal rows:
preheader jump, true/false header outcomes, decrement backedge, and return.
Under that same schedule their ceilings are `1, 3, 3, 3, 1`.

Reconstruct the complete canonical roster and exact subject, schedule, machine,
block/edge, preconditions, and ceiling. Missing, duplicate, reordered, foreign,
or wider-rank rows reject. The installed non-clonable correspondence binds the
whole catalog to an exact code occurrence and entry; borrowed rows cannot enter
whole-root or ordinary acyclic-segment composition.

The interpreter, fixed-work checker, analysis catalog, and native ranked
lowerer have different verifier-issued carriers. A passing interpreter budget
run or rendered proof synopsis cannot construct another role's authority.
Keep subject replay through object/image publication, not only mutually
consistent projected coordinates. General tail-call, natural-rank, and
relevant-precondition bounds require their own checked derivation.

`omega inspect-terminal` exposes recomputed semantic bounds, not installed-root
stack backing or native execution permission. Failed ranked derivation must not
fall back to an ordinary execution carrier.
