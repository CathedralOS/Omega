# Home-layout optimization

Ratified 2026-11-07. The compiler may compact internal storage, including removing
a sum's separate tag, while preserving language semantics and representation
commitments. This replaces the blanket niche prohibition without adding source
syntax, mandatory authored encodings or stricter invariant-window rules.

The normative contract is [home-layout realization](../../spec/layouts/plans.md#home-layout-realization).
It permits any sound realization rather than prescribing an eligibility algorithm.
The two-case zero/nonzero encoding is an example, not a required byte layout.
ABI contracts, layout queries, same-storage recasts and other admitted observations
constrain optimization. Jointly changing both sides of a recast is permitted when
all observations remain compatible. An optimization must preserve admitted uses
or retain a compatible layout.

## Implementation work

Track realization under `LIBRARY-REPRESENTATION-FAMILIES` in [TASKS](../../../TASKS.md).

- Carry the chosen case encoding and geometry in the validated plan and its
  identity through construction, projection, matching, storage, cleanup, layout
  queries, recasts and native realization. Psi interpretation must agree on case
  semantics and target-layout observations; its host objects need not share the
  native byte layout.
- Establish genuine encoding disjointness. Unknown zero validity and gated
  construction alone cannot license a niche. Keep evaluation/search mechanisms
  in the implementation's recorded policy; no new mandatory declaration-level
  three-way judgment is part of the source language.
- Preserve legal predicate repair, payload borrowing, case inspection and
  cleanup. Account for permitted survivor/recovery paths without inventing
  crash unwinding or treating all surviving storage as abandoned. Retain case
  information or a compatible representation wherever the compact encoding
  cannot carry the required observations alone.
- Keep committed layouts and ABI relationships intact. Coordinate transformations
  across same-storage views; never replace a recast with copying conversion or
  silently strengthen source obligations to make a layout fit.

## Acceptance

Demonstrate a useful compact optional representation and its measured geometry,
with interpreter/native agreement through the existing value-transport places.
Cover zero construction, case projection and equality, legal temporary predicate
violation followed by restoration, owned cleanup, and permitted survivor paths.
Controls must retain correct behavior for a zero-valid payload and for unproved
niche eligibility. Check compatible jointly transformed recasts, incompatible
encodings with equal sizes, mutable aliases, explicit ABI layouts, and consistent
`size_of`/`align_of` observations. Existing legal repair or cast programs must not
become source errors merely because the optimization is selected.
