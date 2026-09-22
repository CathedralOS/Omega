# FILESYSTEM-RELEASE-CONTRACT — scope verification and paused-closure plan (z140)

Verified at `069276b986dc` on linux x86-64. Item row: `TASKS.md` FILESYSTEM-RELEASE-CONTRACT
board line ~4713. Delete this plan once the program-side occurrence derivation
lands and the structural field-store closure clears the customer's recorded
stop.

## What the item owns

The [bounded occurrence-specific release proof](wiki/spec/build/permissions.md#bounded-occurrence-specific-release-proof)
for a program's own open/query/close occurrence:

1. Program-side derivation as retained Terminal evidence through checked flow —
   exact object/argument contract, handle/alias identity preserved through
   intervening calls, exactly one applicable release, no later use.
2. Rejoin of the exact occurrence at native realization in
   `native-realization/src/native_realization/terminal_authority_policy/filesystem.rs`
   — an evidence-bound explicit-empty mechanism row, never a synthesized empty row.
3. Native customer: `tests/omega/pass/filesystem/windows_canonicalize_exit`,
   whose recorded stop is `structural field store: scalar field type`.

Explicit non-goals the row names: TWO-AXIS-TERMINAL-AUTHORITY-REVIEW owns the
receiver rows and the broad `Filesystem` summary; general owned-handle design
stays separate; build execution observations establish nothing.

## Machinery inventory (already landed)

The realization side is built and only lacks its producer:

- `FilesystemOrdinaryReleaseContract` + `filesystem_ordinary_release_contract(occurrence_commitment)`
  mint a domain-separated `CheckedSyscallArgumentContractIdentity`
  (`omega.checked-syscall-argument-contract.filesystem-ordinary-release.v1`).
- `filesystem_release_bound_mechanism` binds that contract into a
  `TerminalMechanismIdentity::Syscall` (replaces the conservative contract) or
  `NormalizedForeign` (carries it beside the admitted calling plan);
  `CompilerIntrinsic`/`CheckedPhysical` refuse.
- `filesystem_release_mechanism_row` emits the explicit-empty
  `TerminalAuthorityPolicyRow` only when the mechanism key actually carries the
  contract coordinate — a mechanism with the admitted plan alone, a different
  contract, or a coordinate-less role earns no row.
- `settled_filesystem_cohort` already classifies `close`/`find_close`/`close_handle`
  as `OrdinaryReleaseContract`, so the generic `filesystem_mechanism_row`
  refuses them today — the fence the customer needs lowered is selective, not
  absent.

Every production-visible caller is a test; nothing in checked flow produces
`occurrence_commitment` yet. So the missing leg is the program-side derivation
plus the call that feeds it into the bound mechanism key.

## Re-verified customer stops at `069276b986dc`

Both recorded stops have moved upstream — the structural-store frontier is real
but currently masked:

- `omega --check tests/omega/pass/filesystem/native_close/main.omg` (linux
  x86-64, ~11min): rejects `selected ProgramEntry Service field Main::fs
  requires a selected Fused provider for boundary FilesystemHost` — entry-side
  fused-provider selection, the exact gate the canary_suite roster note names
  for this family (canary_suite.rs:985). The customer never reaches checked
  flow, so the structural-store stop is not observable on this leg.
- `omega --check --target windows_x86_64
  tests/omega/pass/filesystem/windows_canonicalize_exit/main.omg` (linux host
  compiling the windows target, ~13min): rejects `selected ProgramEntry
  establishment rejoins 0 Terminal attachment identities; expected one; the
  machine's unit plan was omitted at local construction at state graph: state
  signature: parameter signature: attached data shape (state 0)` — the
  ENTRY-CONTENT-ROOTS family that dominates the samples_compile residuals.
  Upstream of the recorded `structural field store: scalar field type` stop.

Consequence: the item's paused closure remains the right plan, but landing the
release-contract proof also needs the entry-side fused-provider and
ProgramEntry-attachment legs closed first — both fenced by sibling claims
(TOP-LEVEL-BOUNDARY-REQUIREMENTS / ENTRY-CONTENT-ROOTS lanes).

## Paused prerequisite: the structural field-store closure

The customer's recorded stop is real but masked: `structural_scalar_store`
admits only scalar-typed field stores (`structural field store: scalar field
type` at `execution/terminal_unit/structural_scalar_store/mod.rs:1182` — destination
fields must resolve to `PrimitiveType::{Bool,F32,F64}` or
integer-literal-accepting types). `self.unit_result =
self.fs.write_all(..)` stores a structural `UnitResult` sum.

The closure, per the item's own pause note, covers nested structural sum
construction/extraction, borrowed case observation, and whole nominal receiver
replacement including match-produced assignments — through shared state/value
planning, preserving recursive layout and referent identity. Plan:

1. **Structural field-store plan kind.** Extend `CheckedUnitEffectOperationPlan`
   with a structural-value field store whose source is a
   `CheckedStructuralValueHandle` / `CheckedUnitStructuralResultBindingPlan`
   (the same currency `EstablishStructuralValue`/`StructuralCall` mint), not a
   scalar computation root. Destination stays a structural parameter position +
   `terminal_field_identity`. Admission site: a new arm beside
   `scalar field type` in `build_structural_field_store_at`.
2. **Stored-case observation.** `transition self.unit_result { UnitResult::Ok -> ... }`
   reads a stored structural field's case; the read path must observe through
   the stored referent, not the initial value — same referent-identity rule the
   effect plan already enforces on `EstablishPrimitiveLocal` (reads name the
   symbol, never the initializer). Case payload extraction needs borrowed
   observation so `Error { kind }` projections don't consume the field.
3. **Whole nominal receiver replacement.** `self.unit_result = <call result>`
   is a replacement of the field's entire nominal content — referent identity
   must survive so later transitions/reads see the stored object, and a
   match-produced assignment (`transition` arm that writes the field) reuses the
   same store kind at its join point.
4. **Recursive layout.** `UnitResult::Error { kind: ErrorKind }` is a nested
   nominal payload; layout plans must nest rather than flatten, and identity
   keys stay the stored field's — never a synthesized flat scalar id.
5. **Where it lands.** The store legs live in
   `execution/terminal_unit/structural_scalar_store/` (state/value planning input);
   observation lives in the transition/case machinery under
   `execution/terminal_unit/composed_control` + `state_graph`; lowering consumes the new
   plan kind in `checked-trees-to-lowered-psi` machine lowering. Per the board,
   the frontier is owned by STATE-LOCAL-VALUE-FRONTIER with CORPUS-RED-FAMILY
   coordination on `structural_scalar_store`/`primitive_store.rs`.

## Fence landscape at `069276b986dc` (04:1xZ)

- `terminal_authority_policy/filesystem.rs` — TWO-AXIS-TERMINAL-AUTHORITY-REVIEW
  (Zergling-68, ~04:54Z).
- `structural_scalar_store` + `primitive_store.rs` — CORPUS-RED-FAMILY-TRAPSTORE
  per PSI-NATIVE-FIELD-STORES's recorded fence set; STATE-LOCAL-VALUE-FRONTIER
  owns the closure.
- `execution/terminal_unit/{control,state_graph,composed_control}` — rotating
  GENERAL-CYCLIC-EXECUTION claims.
- The checked-flow occurrence-evidence producer (leg 1 of the item) has no
  landed substrate at all — it is new machinery in checked flow plus a new
  retained-evidence row kind.

## Disposition

This claim's slice: scope verification, re-measured customer stops, and this
plan. Implementation is fenced (rejoin surface) or paused pending the closure
plan (customer legs). Next worker: implement leg-1 derivation once the
structural store closure lands and `filesystem.rs` frees, or take the store
closure itself under STATE-LOCAL-VALUE-FRONTIER coordination.
