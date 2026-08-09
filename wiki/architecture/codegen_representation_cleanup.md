# Codegen representation cleanup

This page records the remaining backend-structure rules. Git history owns the
completed migration diary.

## Invariants

- Represent a semantic concept once and share it across stages that do not
  change it.
- A stage that only annotates a representation adds a side table or wrapper;
  it does not clone the whole representation.
- Addressing is a `Place` algebra consumed by a small operation vocabulary.
  Do not restore operation variants for every source/destination shape.
- Resolve scalar type, width, signedness, domain, and place once, then carry the
  result. A backend must not rediscover those facts from syntax.
- Unsupported lowering fails explicitly. Missing selection must never silently
  discard a write or substitute another slot.

## Current shape

Target and assigned operations share their operation and operand vocabulary.
The assigned layer adds register/scratch homes rather than redeclaring target
operations. Copy, write/RMW, text, comparison, and operand lowering use
composable places instead of a Cartesian family of addressing-specific opcodes.

Table-backed place/value resolution is canonical. Most former owned-tree
fallbacks either delegate through an inserted tree or were deleted after
differential tests proved them dead. Regression canaries retain the slot,
width, indexed-place, call-result, and case-payload failures that motivated the
cleanup; the root-cause prose does not need a parallel archive here.

## Remaining work

- Remove the residual alias-dependent non-table mutation operand path after
  callers substitute aliases or the table path gains equivalent substitution.
- Stop copying target arenas into the assigned plan when an immutable target
  plan plus a homes side table can satisfy all consumers.
- Move any cross-stage vocabulary still owned by an incidental representation
  crate into the lowest neutral owner. Do this only for genuinely identical
  concepts; keep real lowering boundaries distinct.
- Replace hand-maintained instruction byte-width mirrors with symbolic emission
  followed by one layout/relocation pass. Encoder output must be the authority
  for final size.
- Revisit frame realization when self-looping calls or separate compilation
  require a real call stack or another proved-disjoint scheme. Do not attribute
  unrelated historical selection bugs to frame overlap.

## Acceptance

- Each operation, operand, place, and scalar fact has one authoritative
  representation at a given semantic level.
- Annotation-only stages retain their input and add only their annotations.
- Selection has one resolver per concept rather than a table/owned-tree and
  addressing-shape matrix.
- Every executable byte and relocation is laid out from the same sequence that
  is emitted.
- Differential interpreter/native canaries and both target encoders remain
  green throughout each independent migration.
