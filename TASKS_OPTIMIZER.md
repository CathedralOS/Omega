# Optimizer tasks

This is the optimizer execution board, not its history. Architecture and
rationale live in
[optimizer implementation](omega-rust/optimization.md),
and landed milestones live in Git. Remove work from this file when its
acceptance condition passes. Tag an added item's provenance on its first line:
`(new-scope)` for newly discovered work, `(split-of:<parent-item>)` when it
decomposes an existing item. Items added before 2026-09-14 are untagged.

Selections are exact names. Do not add `O1`/`O2`/`O3`, `debug`/`release`, or
another broad alias. Every rule must name the exact semantic, proof, ownership,
effect, target, and provenance facts it consumes, and retain the identities
needed for independent replay through publication.

## Pipeline cleanup follow-ups

These are separately schedulable follow-ups, not an ongoing cleanup run.
The [ownership contract](omega-rust/pipeline.md)
governs all three: replace obstructive implementations, preserve semantics and
independent validation, and keep empty/nonempty optimization selections on one
physical route. Unsupported cases reject rather than restoring a fallback.

- **PIPELINE-OWNER-CONSOLIDATION.** Finish ownership in
  `omega-rust/{omega,psi}/pipeline/` and their compiler/backend coordinators.
  Every pipeline crate is already a literal `X-to-Y` or `X-to-X` stage and the
  folders match the [connected route](omega-rust/pipeline.md#connected-program-route);
  `tests/architecture/representation_ownership.rs` pins the `X-to-X` phase
  directories and keeps native coordination and target setup out of the stage
  list. That does not establish the acceptance below: validated owners with
  public entrances still sit beside the route without being called from it.

  Remaining work:

  - `selected-instructions-to-selected-instructions/src/rewrites/` holds about
    40 rewrite modules (`copy_removal`, `dead_store`, `load_forwarding`,
    `store_motion`, `address_fold`, `literal_compare`, `redundant_extension`,
    the relocation and interchange families) whose entrances, such as
    `remove_selected_copy` and `eliminate_selected_dead_store`, are called only
    from their own tests. `optimize_selected_instructions` runs
    `run_selected_lowering_optimizations` and nothing else, and none of these
    rewrites has an exact name in
    [rules.md](omega-rust/omega/representations/optimization-core/rules.md).
    Give each retained rewrite a catalog entry executed by the stage entrance,
    or delete it. Their behavior stays with `EXACT-MACHINE-SIMPLIFICATIONS`,
    `DECLARATIVE-PEEPHOLES` and `ALIAS-AWARE-MEMORY`.
  - `selected-instructions-to-register-homes/src/unsequenced_spill_stages/`
    holds 18 spill families, each with public plan and receipt records, that
    `stage_register_allocation` never calls. `SPILL-REALIZATION` owns
    sequencing the ones it needs; delete or merge the ones the executable
    `assignment/runtime_spill` route has superseded.
  - `native-realization/src/optimized_semantic_wrapper_{encoding,object}/`
    keeps a staged record, codec and validator inside a coordinator crate.
    `select_optimized_program_storage_semantic_wrapper_encoding` and
    `stage_validated_optimized_program_storage_semantic_wrapper_object` have no
    caller outside their own tests. Move the live part to its backend or
    representation owner, or delete it.
  - Audit the remaining stage and coordinator crates the same way: a public
    stage entrance that no coordinator or successor stage calls is an orphan
    output.

  Acceptance: folders expose the connected program sequence, no competing
  entrances or orphan outputs remain, and coordinators only sequence typed
  stages. Renaming a helper or adding a wrapper is not completion.

  Flag: three owners carry validated, replayed and mutation-tested machinery
  that no executable route reaches (about 40 selected rewrites, 18 spill
  families, the ProgramStorage wrapper object). Each new slice adds tests and
  board text for code the compiler never runs. The general mechanism is the
  one [optimization.md](omega-rust/optimization.md#catalogs-and-independent-replay)
  already names: one ordered catalog per owning stage, executed by the stage
  entrance.

- **REPRESENTATION-OWNERSHIP.** Finish
  `omega-rust/{omega,psi}/representations/` under
  [native representation ownership](omega-rust/omega/representations/README.md):
  one named program-root file beside `lib.rs`, concept-owned subdirectories,
  and current data independent of producer history.
  `tests/architecture/representation_ownership.rs` already enforces the named
  root for all ten Psi representations and for eight Omega programs (abstract,
  target, legalized and selected operations, register homes, physical
  instructions, machine code, representation selections) and rejects
  `StagedOptimized` ancestry inside them. It does not cover the other ten
  Omega representation crates, and consumers in the selected and allocation
  stages still reach current data through producer history.

  Remaining work:

  - Replace stage-ancestry walks with direct reads of the current program.
    `selected-instructions-to-selected-instructions/src/selected_optimization.rs`
    obtains its selections through
    `ranges.liveness_stage().selected_stage().optimized_target().optimized()`;
    the same accessor chains occur on about 100 non-test lines of
    `selected-instructions-to-register-homes` and about 60 of
    `selected-instructions-to-selected-instructions`. Keep the retained inputs
    as replay evidence only.
  - Settle `representations/optimization-unit`. It holds an executable entrance
    (`construction/`, `reconstruct_psi_optimization_unit_seed`) that projects an
    `AbstractOperationPlan` into a second program, `PsiOptimizationUnit`, plus
    three root files beside `lib.rs`. Either the unit is private working state
    of `abstract-operations-to-abstract-operations` and moves there, or it is a
    named representation with one root and its projection moves to the stage.
  - Move durable codecs out of transforms and coordinators with their
    consuming stage changes: `post_allocation_manifest/codec` and
    `rewrites/allocation_recovery/fixed_view_copy/codec` in the two selected
    stages, and `optimized_semantic_wrapper_object/codec` in
    `native-realization` (see `PIPELINE-OWNER-CONSOLIDATION` for whether that
    owner survives).
  - For `optimization-core`, `register-model`, `task-plans` and `effects`,
    which keep several root files, decide whether each is shared vocabulary
    (exempt under [pipeline.md](omega-rust/pipeline.md#placement-and-semantic-ownership))
    or a program needing one root, and extend the guard's table to match.

  Acceptance: current programs outlive their producers; ordinary consumers
  read current data directly; historical inputs remain separate replay
  evidence; the architecture guard names every program representation.

## Product pruning and rollout

- **WORKSPACE-ROLLOUT.** Keep every rule explicit opt-in and `Experimental` in
  the [exact-rule inventory](omega-rust/omega/representations/optimization-core/rules.md)
  until the frozen-tree command `CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=1 mbx test
  --workspace --no-fail-fast` passes. Do not replace it with `--all-targets`,
  which omits doctests. The command cannot pass today: `AGENTS.md` records a
  full `canary_suite` run as red, and
  [known baseline failures](wiki/drafts/known_baseline_failures.md) lists open
  library and native-differential failures. No
  [promotion record](omega-rust/omega/representations/optimization-core/promotions/README.md)
  exists yet; `omega-architecture-test`'s
  `exact_rule_rollout_is_complete_and_promotion_gated` keeps the inventory and
  any record in step.
  Acceptance: the command passes from a clean checkout and every promoted
  exact rule has the evidence the
  [promotion contract](wiki/spec/build/optimizations.md#release-rollback-and-promotion)
  requires.

## Validation, translation, and publication

- **TRANSLATION-VALIDATION.** Complete independent source-to-target replay for
  admitted operations and transfers on the common graph. Every surviving
  executable boundary occurrence binds exactly one physical child under
  [closed application and physical occurrence](wiki/spec/terminal-psi/boundary_calls.md#closed-application-and-physical-occurrence);
  `native-artifact/src/physical/derivation/` derives those children, and
  selected-lowering operations, both occurrence roles, and the exemption for
  verified-eliminated occurrences already replay through it with missing,
  duplicate, stale, substituted, padded and role-swapped children rejecting.
  Coverage is still all-or-nothing per artifact: `derive_physical_evidence`
  returns `Ok(None)` at eight points, so one occurrence whose realization has
  no span arm drops the physical evidence for the whole artifact, and a
  consumer calling `require_native_physical_evidence` sees only
  `NativePhysicalEvidenceUnavailable` without learning which occurrence failed.

  Remaining work:

  - Operator applications. `physical/operator_applications.rs` spans only
    `NongenericCheckedBody` and `SpecializedCheckedBody` — a direct call span,
    or a fragment call under fragment publication — and
    `ExactCompilerIntrinsic`, as an FMA span plus an IEEE float-compare
    fragment. `derive_checked_call_span` yields no span for an operation kind
    outside `Call`, `CallUnit`, `CallStructuralScalar` and
    `CallStructuralWithScalarArguments`.
  - Boundary settlements. `derivation/evidence.rs` matches three hosted
    builtins by exact execution and realization pair (`HostedExitProcessI32`,
    `HostedWriteByteI32`, `HostedReadByte`), the admitted-provider settlement,
    and the normalized foreign call. Any other builtin, and any occurrence
    carrying neither an installed settlement nor a foreign call, yields no
    evidence.
  - Privileged port effects. Every retained effect must be consumed by an
    exact `MetadataOnlyPort` settlement join; one unowned effect drops the
    artifact's evidence.
  - General calls wait on `FRAME-LAYOUT`, itself blocked on a contract for
    runtime-sized activation storage.

  Acceptance: a program whose occurrences include a realization outside those
  arms publishes complete physical evidence binding each surviving occurrence
  to one child with nonempty machine, object and final-image spans, and replay
  rejects missing, duplicate, stale, substituted, padded and role-swapped
  children. An artifact that cannot span an occurrence names that occurrence
  instead of publishing with no evidence at all. These are native compiler
  guarantees, independent of package locks or `PackageInstance` construction.

  Scalar expression-family planners, Unit and structural whole-function
  templates, and their catalogs and compatibility fixtures are retired. Do not
  restore them to recover arithmetic, crash, cleanup or borrowed-call coverage;
  extend ordinary graph operations and their receiving checks instead.

  Flag: the settlement side grows one derivation per hosted builtin.
  `derivation/children.rs` holds `derive_exit_group_child` (153 lines),
  `derive_write_byte_child` (123) and `derive_read_byte_child` (164), each
  selected by matching one `(CompilerBuiltinExecution, BoundaryRealization)`
  pair and each re-spelling its own supported-target list and empty-roster
  checks; `BoundaryTraitSettlementRole` carries `CompilerBuiltin`,
  `CompilerBuiltinRuntimeScalar` and `CompilerBuiltinStructural`, which differ
  only in argument and result shape. The general mechanism is one
  settlement-span derivation driven by the emitted settlement record's
  declared argument and result shapes and a target-applicability fact on the
  builtin, so that a fourth hosted builtin is a catalog row rather than a match
  arm, a role variant and a function.

- **CUSTODY-MUTATION-COVERAGE.** Complete authenticated one-field mutation
  tests for every remaining manifest, receipt, codec, and artifact-custody
  family, as
  [validation when extending a stage](omega-rust/optimization.md#validation-when-extending-a-stage)
  requires. 85 `*_rejects_every_one_field_substitution` tests already cover the
  image-emission installation records, executable installation and its wire
  container, component publication and description, external-root admission,
  the compiler's object-artifact, object-container, realization, text-section,
  callable-entry and fragment-emission custody, build and package records, and
  the Terminal codec sections. Each mutates one field, recomputes the
  containing identity honestly, and requires independent replay to reject.
  None of them covers this board's own physical-pipeline stages.

  Remaining work:

  - `omega-rust/omega/pipeline/` has no one-field substitution test at all,
    while 16 `Staged*CustodyReceipt` families are produced there: allocation
    legality, liveness, live ranges, register homes, post-copy,
    post-literal-fold and post-selected-lowering homes, fixed view copy, fixed
    precolored segment homes, active-resident rematerialization and its
    pressure receipt, selection, selected reanalysis, literal fold,
    selected-lowering optimization, and the post-allocation machine. Today
    they reach replay only through the joined receipt that
    `compiler/tests/realization_custody.rs` mutates.
  - `ProductionCompilationManifest`
    (`compilation-report/src/production_manifest.rs`) has no custody test. That
    crate's `compile_report/custody_tests.rs` covers only the executable and
    native-package publication receipts.
  - `OfflinePolicyRegressionManifest`
    (`tooling/optimization-policy-offline/src/cost_threshold_policy/regression_manifest/`)
    has a codec, an identity and a validator, but only the whole-record
    `corpus_model_and_report_substitution_fail_closed` refusal.
  - Name the independent checker for a family that never encodes. A staged
    receipt consumed in memory has a validator, not a decoder; say which one
    rejects the substitution, and record the fields that are unrepresentable
    instead of leaving them unlisted.

  Acceptance: each representable field of a family changes independently, its
  containing identity recomputes honestly, and independent replay still rejects
  the substitution; a field that cannot be represented is rejected at canonical
  encoding and named as such. A new record family lands with its matrix rather
  than acquiring one later.

  Flag: the matrices are hand-written, one per record shape.
  `image-emission/tests/artifacts/installation_function_nested_custody.rs` alone
  holds 18 of them in 6,942 lines, and six such files run 15,157 lines
  together; only the wrapper-object matrix under
  `native-realization/src/optimized_semantic_wrapper_object/tests/` is factored
  into a reusable shape. Nothing fails when a family has no matrix, which is
  how 16 pipeline receipts reached zero coverage. The general mechanism is one
  substitution harness driven by each record's canonical field inventory plus a
  per-family honest-recomputation hook, so that a new family declares its
  fields instead of adding another several-hundred-line test, and a family with
  no entry fails a repository gate.

## Psi optimization and loops

- **GENERAL-CYCLIC-EXECUTION.** Carry ordinary cyclic Terminal Psi through the
  post-Terminal stages to native publication. Natural-ranked and unranked
  modules already take the ordinary verification and abstract-lowering route
  ([ranked native admission](omega-rust/omega/pipeline/terminal-psi-to-abstract-operations/README.md#ranked-native-admission)),
  the optimizer authenticates their components and freezes the complete cyclic
  function under
  [catalogs and independent replay](omega-rust/optimization.md#catalogs-and-independent-replay),
  and admission rejects the retired unsigned-countdown custody outright instead
  of falling back. Artifact admission alone establishes no downstream
  optimization, target lowering, or publication support for those cycles. The
  item of the same name in `TASKS.md` owns the Psi half — Terminal production
  of cyclic machines, the verifier's cyclic shape allowlist, and the
  `print_squares` and Console-writer customers. This item starts at admitted
  Terminal input and owns the receiving graph, native selection, and replay.

  Remaining: implement
  [ranked callees on projected receivers](wiki/spec/language/termination.md#ranked-callees-on-projected-receivers),
  beyond the current whole-entry-only admission. Composed argument references,
  call and return, cleanup, callee measure checking, and composed resource
  evidence are all missing; a graph representation establishes none of them,
  and unsupported transfers reject until their ordinary operation and proof
  joins exist. Extend the common graph and ordinary ranking evidence rather
  than restoring a second native route.

  Acceptance: an ordinary caller borrows a nested field, its ranked callee
  preserves that referent across backedges, and the caller observes writes
  after return. Conflicting parent access and missing or invalid callee ranking
  reject. Argument identity, call and return, cleanup, and composed resource
  bounds validate through native replay and execution on both Linux
  architectures. This needs no new receiver syntax; broader changing-reference
  transfers still require loan/alias and ranking substitution work, and
  widening the parameter count alone closes none of them.

- **GENERAL-LICM.** Implement motion only through transformations that
  invalidate and reconstruct component, loop-carried custody, ranking,
  provenance, effect, and fuel evidence. The dedicated countdown zero/one
  relocation is not general LICM authority. The shared-source preheader
  boundary relocates scalar constant leaves, side-effect-free scalar
  computations — including exact, wrapping-divide/remainder, and
  saturating-divide/remainder variants whose verifier-discharged totality
  obligation moves byte-exact inside the relocated operation — place
  observations whose storage root is visible at the preheader, is
  produced by a node the same run relocates ahead of the read — a
  uniquely member-produced root the moved observation keeps spelling
  byte-exact — or rebinds to the invariant representative its member
  structural parameter resolves to, `ByteSequenceRead` reads whose `index`
  operand substitutes through the same scalar rule, whose bounds
  obligation stays byte-exact, and whose `length` operand relocates with
  its `ByteSequenceLength` producer measuring the same rebound root in the
  same run, `ByteSequenceSubslice` views whose scalar operands rebind
  the same way while the structural result and bounds obligation stay
  byte-exact inside the moved operation,
  `EstablishByteSequenceLiteral` establishments — the boundary's first
  non-observation structural relocation — whose declared place,
  structural type, and payload move byte-exact inside the moved
  operation while every consumer keeps spelling the same place
  identity, scalar-signature `Call`
  nodes — the boundary's first call relocation — whose callee's
  transitive effect summary proves no observable effect, crash, or
  suspension (the `structural_state` axis stays exempt because a scalar
  call passes no places) while every member node stays unobservable, so
  hoisting the call's possible non-return reorders nothing anyone could
  see; a call carrying `crash_continuations` keeps its crash-route
  custody inside, and discharged `requirement_obligations` move
  byte-exact inside the moved operation, `EstablishPrimitiveLocal`
  establishments whose declared place, structural type, and claim-free
  result custody move byte-exact while the scalar initializer
  substitutes under the member-parameter rule, and structural calls
  (`CallUnit`, `CallStructuralScalar`) behind the same purity and
  unobservable-roster evidence whose every structural argument is a
  non-owned borrow naming a preheader-visible, member-parameter-resolved,
  or run-covered member-produced root — a mutating borrow's root must be
  uniquely member-produced and covered by the same run, and a producing
  establishment relocates only when every member mutable borrower of its
  root relocates too, and unrestricted `EstablishRecord`
  establishments — the boundary's first operand-carrying structural
  establishment — whose declared place, structural type, claim-free and
  qualification-free result custody, declaration-ordered field
  initializers, and range obligations move byte-exact inside the moved
  operation, while every scalar field initializer substitutes under the
  member-parameter rule and every structural field initializer copies a
  whole unrestricted root the run resolves to a preheader-visible,
  member-parameter-resolved, or uniquely member-produced run-covered
  place the moved record keeps spelling, under the same whole-component
  custody bound that no member stores to the declared place. Invariant
  discovery resolves
  member scalar and structural
  parameters transitively across component-internal edges to the
  representative every reaching edge agrees on — a member parameter
  every reaching edge binds to one run-covered member result substitutes
  to that preserved result, and a member structural parameter bound the
  same way to one uniquely produced member root rebinds to that covered
  root — rebinding moved operands
  and observed roots to the preheader-visible anchor. The insertion
  preheader is the one block every authenticated entry edge departs —
  several entry edges of one dispatching terminator share it, while
  entries departing different blocks decline — and non-leaf motion
  additionally requires every terminator successor to enter the component
  plus a member block every traversal that leaves executed; the freeze
  fence re-derives both halves of that non-speculative gate from the
  authenticated topology. Operand-carrying structural establishments now
  relocate in bounded slices: unrestricted `EstablishScalarArray`
  establishments move under the same whole-component custody bound, and
  `EstablishScalarCase` establishments move unrestricted under that bound
  or affine under a containment admission that strips the result from
  member-internal `StructuralCase` dispatch-edge discard rosters and
  disposes the persistent preheader result on component exit edges and
  member returns — ownership-frontier membership for the relocated place
  is invalidated and re-derived from the transformed graph rather than
  trusted, and freeze replay independently re-derives both the admission
  and the custody rewrite. Remaining: other
  non-scalar families, profitability, and motion beyond the shared-source
  preheader. `EstablishTrivialAffineLocal` and place-result structural
  calls (`CallStructural`) still have no admitted relocation evidence; the
  remaining establishments still need an admitted cyclic source shape —
  scalar-graph arrays only emit as call arguments — before admission
  work can begin. Cycle-admission evidence (linw2): the verifier's
  cyclic-operation whitelist `unranked_cycles::eligible` now admits
  unrestricted `EstablishRecord` establishments — `record::fields`
  validation proves the fresh result place, declaration-paired field
  initializers, and claim-free result, and the unrestricted result never
  carries a per-iteration disposal obligation — and admits unrestricted
  `EstablishScalarArray` establishments through `scalar_array::shape`
  (fresh result place, declared leaf shape, claim-free result;
  re-entry replaces the stored payload without moving custody), while
  `EstablishTrivialAffineLocal` falls to the scalar-result fallthrough —
  and admits `EstablishScalarCase` and place-result `CallStructural`
  only bound to the same block's `StructuralCase`/`ReturnStructural`
  terminator, whose edges trivially discard the result place — the
  affine scalar-case slice relocates past that shape by rewriting the
  dispatch custody, while a hoisted `CallStructural` would still leave
  later traversals dispatching a disposed place. Synthetic fixtures cannot
  bypass the fence: optimizer admission replays
  `verify_module_for_optimization`. Profitability likewise has no
  bounded leg — every operation and terminator costs one fuel unit, so
  no zero-cost speculation exists beyond the constant-leaf exemption —
  and motion past the shared preheader needs new blocks or run
  duplication the frozen block roster and unique-occurrence freeze
  reject. The remaining work therefore starts upstream in psi.

  Region custody constrains every remaining motion family: the counted-loop
  `LoopRegion` is projected from validated Terminal-SCC custody, never from a
  private loop forest or a second edge/reachability walk inside the countdown
  leaf. Reducibility under the certified header rests on the custody's unique
  entry edge landing on that header plus the verifier's all-blocks-reachable
  control graph — a component holding the machine entry block has no entry edge
  at all, because any non-member reaching into it would join its cycle. The
  independent reconstruction lives in
  `validation/context/ranked_cycles/ordinary.rs`, which rebuilds components from
  the current optimizer body and requires them to equal the verifier's Terminal
  surface exactly. Keep both halves; do not reintroduce a loop-forest producer
  to recover a region.

## Lowering and instruction selection

- **EXACT-SELECTION-FAMILIES.** Add address-mode folding, compare/branch
  selection, extension elimination, and constant materialization one exact
  named family at a time. Each family needs a disjoint source grammar,
  independent validator, target applicability, corruption controls, and
  publication replay. Resume evidence (verified at `8c0d4dbb45`, Linux
  x86-64): the only existing family seam is the selected-lowering literal
  fold in `selected-instructions-to-selected-instructions`
  (`rewrites/selected_lowering/literal_fold`). It is not a general peephole:
  it fires only from a pressure-recovery
  `ImmediateU64RematerializationCandidate` with `Incoming` role, folds one
  `MaterializeI64` into the `[left, result]` consumer at operand 1 with a
  `Def` result and u12 immediate, and both `SelectedInstructionPairRule` and
  the inline validator (`validate/replay.rs`) assume that shape. Compare with
  a zero literal is already selected directly as `CompareI64Zero`
  (`target-operations-to-selected-instructions` `zero_compare.rs`), so no
  compare/zero fold has a customer. The next compare/branch family
  (`CompareI64` + u12 literal → `CompareI64Immediate`) needs a new
  `SelectedInstructionKind` across `selected-instructions`, both ISA encoders,
  `register-environment`, selection identity, and the optimizer vocabulary,
  plus a fold action for flag-defining consumers without a `Def` result.
  Landed: the `CompareI64Immediate` kind and both encoders (x86-64 `cmp
  r64,imm32`, AArch64 `subs xzr,xN,#imm12`; immediate domain is u12, semantic
  tag 53), with canonical zero decode on AArch64 reusing `CompareI64Zero`,
  and the compare fold family itself: `Optimization::
  SelectedIncomingU12CompareImmediate` (tag 21) enables the
  `COMPARE_IMMEDIATE_U12` pair rule, the flag-defining/no-`Def` fold action
  (`LiteralFoldAction.result: Option`, codec v4), flag-shape immediate-row
  validation in compute and independent replay, and firing/corruption/replay
  coverage on both Linux targets. Selected-lowering selections now
  execute through the common physical stages — the staging gate resolves the
  exact catalog instead of rejecting the phase
  (`native-realization/.../physical_pipeline/phase_selections.rs`), and fixed-frame
  realization binds the completion identity and literal-fold transformation
  ledger into the expected manifest
  (`machine-emission/.../assembly/fixed_frame.rs`). Add, subtract, and compare
  return-only builds replay to `ValidatedOptimizedProjection`, and a
  boundary-operator program replays one exact physical child.
  Landed: the extension-elimination family —
  `Optimization::SelectedIncomingLiteralExtensionElimination` (tag 23)
  enables `EXTENSION_V1` and the six `EXTENSION_LITERAL_FOLDS` unary pair
  rules (`MaterializeI64` + `ZeroExtendU8`/`U16`/`U32` or
  `SignExtendI8`/`I16`/`I32` → `MaterializeI64` of the folded output bits).
  The catalog payload now names a slice of pair rules per selection, and
  `PairOperandShape` (binary right-literal vs unary sole-operand) is a third
  declared rule dimension beside result disposition and the unit-effect
  surface. The independent replay re-derives the grammar, folded bits, and
  materialized `IntegerValue` from the consumer kind and the surviving
  result register's scalar type; a `windows_x86_64` return-only build
  replays to `ValidatedOptimizedProjection`
  (`compiler/tests/optimizer_extension_elimination.rs`). Landed: the
  constant-materialization family —
  `Optimization::SelectedIncomingLiteralCopyMaterialization` (tag 25)
  enables `COPY_V1` and the `COPY_LITERAL_FOLD` unary pair rule
  (`MaterializeI64` + `CopyI64` → `MaterializeI64` of the literal itself at
  the copy's destination). The fold admits the full u64 literal domain — no
  narrowing — so the copy's result register must admit the literal. The
  independent replay binds the shared `MaterializeI64` constraint row under
  a second policy-gated slot so a copy fold cannot replay under the
  extension selection alone; a `windows_x86_64` return-only build replays to
  `ValidatedOptimizedProjection`
  (`compiler/tests/optimizer_copy_materialization.rs`). The deferred
  `phase_selections/tests.rs` enable-list addition landed with it
  (indexed-load and copy rows both listed now). Landed: the address-mode
  folding family —
  `Optimization::SelectedIncomingU12ByteViewAddressOffset` (tag 26)
  enables `BYTE_VIEW_ADDRESS_V1` and the `BYTE_VIEW_ADDRESS_OFFSET_U12`
  pair rule (`MaterializeI64` + `ByteViewAddress` → `AddressOffset` of the
  folded offset): the projection's operand-1 `Use` is the folded literal,
  its operand-0 `Use` base survives, and its operand-2 `Def` is the result.
  Both forms are effect-isolated on x86-64 and AArch64, so the rule rides
  the ordinary binary-right-literal grammar and isolated effect surface;
  the declared u12 bound is the aarch64 `add`-immediate encoding limit.
  The independent replay binds the `AddressOffset` row under its own
  policy-gated slot and rebuilds the constant-offset form from the
  consumer kind alone; a `windows_x86_64` return-only build replays to
  `ValidatedOptimizedProjection`
  (`compiler/tests/optimizer_byte_view_address_offset.rs`).
  Remaining: none of the originally suggested families are outstanding;
  any further family follows the same one-exact-named-family-at-a-time
  contract.

## Register allocation and frames

- **SPILL-REALIZATION.** Finish executable pressure recovery and join its
  frames to stack provisioning.
  [Register allocation](omega-rust/omega/pipeline/selected-instructions-to-register-homes/README.md)
  chooses victims in `src/assignment/runtime_spill/`; the rewrite owner,
  `selected-instructions-to-selected-instructions/src/rewrites/runtime_spill/`,
  inserts and independently replays the private stores and reload pairs. That
  route already spills general-purpose-class instruction results, block
  parameters and entry-bound registers in acyclic and cyclic functions; serves
  body, terminator, edge-transport and stored-snapshot uses; keeps an ABI pin
  on its own reload pair; carries a reload across a call onto a surviving
  view; and composes after fixed-view copies and active-resident
  rematerialization. It does not establish the cases below.

  Remaining work:

  - Victim and use admission (`rewrites/runtime_spill/admission.rs`). Still
    rejected: a victim whose register class differs from the `FrameAddress`,
    `Load64` and `Store64` rows (vector-class values); a definition by
    `FrameAddress`, `AddressOffset` or `ByteViewAddress`; tied, early-clobber
    and redefining references; an entry-bound victim when an edge targets the
    entry block; and a multi-chunk stored snapshot whose chunk loads are
    pinned or separated by a unit-writing instruction. Recovery then tries
    the next roster candidate and fails when the roster is exhausted.
  - Slot assignment. Every victim declares a private eight-byte
    `LocalStorageSlotId::Spill` slot. `runtime_spill/slot.rs` shares an
    existing slot only for the zero-offset `Store64` or
    `FrameAddress`-plus-`Load64` idiom when a last-writer replay proves the
    windows cannot interleave. Interval-based coloring exists only as
    `unsequenced_spill_stages/stack_slot_coloring`, which
    `stage_register_allocation` never calls.
  - Composition (`src/register_allocation.rs`). A completed selected-lowering
    run takes `assignment::transformed` homes and surfaces pressure as
    `TransformedHomes`; it never enters `assignment::runtime_spill`. A
    selected-lowering selection beside an allocation-recovery selection, and
    two allocation-recovery selections, reject as `UnsupportedComposition`.
  - Stack provisioning under the
    [compiler-owned stack contract](wiki/spec/resources/storage.md#compiler-owned-stack-accesses).
    Reuse the final-frame demand path, not a parallel spill-byte estimate.
    `image-emission` derives each function's peak from its validated final
    frame (`src/function_fragments/production.rs`, replayed by
    `validation/stack.rs`) and composes it in
    `object_artifact/stack_demand.rs` and
    `installation_record/record_construction.rs::derive_installation_stack_demand`.
    External-root admission (`external-roots/src/root_entry/root_validation.rs`)
    and hosted-receiver partitioning (`image-emission/src/hosted_receiver.rs`)
    compare that demand with supply. No test carries a runtime-spilled
    function through this path to an admission or a rejection. Target-required
    probing and setup are frame/provisioning work, not an owner-blocked
    language choice.

  Acceptance: slot reuse is not double-counted; changed allocation or frame
  realization invalidates stale demand; insufficient supply rejects before
  execution; and generated loads and stores independently replay their
  physical geometry and value lineage without adding source crash routes. A
  byte ceiling alone does not stand in for valid stack backing. Keep
  `WRITE-ONLY-BORROW`'s source-backed three-call regression
  (`tests/native-differential/tests/terminal_psi_indexed_receivers/primitive_stores.rs`)
  as the hosted execution control, with its installed-image demand replay and
  stale-frame rejection (`terminal_psi_indexed_receivers/stack_pointers/`).

  Flag: `unsequenced_spill_stages/` holds 18 spill families in about 25,700
  non-test lines (logical spill operations, slot coloring, abstract
  insertion, reload-value homes, recursive and generalized worklists,
  pseudo-instruction lowering, memory effects, access constraints).
  `stage_register_allocation` calls none of them; native-differential tests,
  architecture ladders and machine emission's non-authoritative
  `frame_layout/spill_requirements/` are their only consumers. The executable
  route is about 1,100 non-test lines of allocation code plus 2,950 of
  rewrite, and slot reuse now has two owners. Sequence a staged family behind
  the executable route or delete it; do not extend both.

- **ALLOCATION-REFINEMENT.** Add general live-range splitting to
  [register allocation](omega-rust/omega/pipeline/selected-instructions-to-register-homes/README.md)
  while preserving exact register-unit aliases, liveness, and target custody.
  Copy-affinity coalescing in home assignment and in fixed/precolored segment
  homes, rematerialization ahead of private storage, and the fixed/precolored
  interval stages on the default route already exist. Splitting exists only
  as fixed-use recovery in `selected-instructions-to-selected-instructions`:
  `src/rewrites/allocation_recovery/fixed_view_copy/` inserts a copy only for
  a `u64` `EntryParameter` whose entry fixed view differs from the fixed view
  of a `Return` operand in a non-entry block (`compute.rs`,
  `compute/preflight.rs::find_leaf_block`). Every other pressure case goes to
  rematerialization or runtime spill.

  Remaining work:

  - Split a live range at allocation-chosen points and home each segment
    independently, for any admitted origin, scalar type and use, not only an
    entry parameter returned from a leaf. Insert the connecting copies through
    the selected rewrite owner, then accept homes only over fresh liveness,
    ranges and legality.
  - Lift the limits in `src/analyses/fixed_precolored_split_requirements/`.
    Tied registers, early-clobber domains, and ranges whose fragments join,
    cycle or lack a source connector reject as `UnsupportedTiedRegister`,
    `UnsupportedEarlyClobberDomain` and `UnsupportedCrossBlockRange`
    (`compute/partition.rs`, `compute/topology.rs`).
  - Place splitting in the recovery order.
    `assignment/runtime_spill/recovery.rs` tries rematerialization, then a
    call-crossing spill, then a bounded spill; a split that frees a home
    without storage has no position there.

  Acceptance: a value other than a leaf-returned entry parameter, with uses on
  incompatible views or across a pressure region, allocates through recorded
  split points and per-segment homes, and replays through callable
  publication on every admitted target. Independent replay reconstructs the
  split points, copies and segment homes from current facts. A moved or
  omitted split point, a copy across an incompatible fixed-use boundary, a
  segment home that aliases a live register unit, and stale post-split
  analyses reject. `SPILL-REALIZATION` owns private storage; this item adds
  no slot.

  Flag: one of the two allocation-recovery selections,
  `SharedEntryFixedViewCopyAfterCompareBeforeBranchV1`, is a whole-function
  template. `fixed_view_copy/compute/shared_entry.rs` requires exactly two
  boundaries of one `u64` entry parameter, an entry block holding a single
  `CompareI64Zero`, and a `ConditionalBranch` to two distinct return leaves.
  General splitting with a copy-placement decision (one copy at a dominating
  point when that is cheaper than one per use) should replace this selection,
  not gain a sibling per CFG arrangement.

- **FRAME-LAYOUT.** Complete exact nonzero-frame realization. Red-zone policy,
  stack probing, unwind information, stable-address loans, the general-call
  frame/callee-save/link-register/call-site-alignment plans, and the
  realization stage's frame-policy rosters all replay through ordinary
  callable publication on the admitted targets.

  Remaining: dynamic-allocation constraints, blocked upstream. Every selected
  local and outgoing slot resolves to a static byte extent, so there is no
  runtime-sized stack allocation to constrain. This leg waits on a
  language and Terminal Psi contract for runtime-sized activation storage,
  not on frame-layout work; open that contract before resuming here.

  Acceptance: a program with a runtime-sized activation allocation publishes a
  frame whose committed extent, probe roster and unwind information replay
  against the validated layout on every admitted target, and a suppressed or
  invented extent replays false.

## Machine optimization

- **DECLARATIVE-PEEPHOLES.** Generalize the symbolic instruction-pair
  descriptors in `selected-instructions-to-selected-instructions`
  (`src/rewrites/selected_lowering/literal_fold/pair_rule.rs`) to physical
  register units, effects, traps, memory, stack, and control flow without
  replacing the independent validator: `literal_fold/validate/replay.rs`
  restates every grammar from instruction records and never reads a
  descriptor. `SelectedInstructionPairRule` declares producer, consumer and
  rewritten kinds, operand shape, immediate bound, result disposition, unit
  effects and machine effects, and the
  [exact-rule inventory](omega-rust/omega/representations/optimization-core/rules.md)
  lists the 23 landed `SelectedIncoming*` selections. Every landed pair
  eliminates an effect-isolated `MaterializeI64` that immediately precedes its
  single consumer in the same block. The candidate is one pressure-recovery
  `ImmediateU64RematerializationCandidate` per function and fixed-point
  iteration (`compute/actions.rs::derive_action`), under the caller-saved-only
  availability policy
  (`src/analyses/legality/policies.rs::frameless_leaf_caller_saved_views`).
  Every rewritten form except the indexed-read fold carries no memory, trap,
  stack or control effect.

  Remaining work:

  - Unit roles. `PairUnitEffects` requires a rewritten row with no implicit
    uses or clobbers and rejects every `tied_to` operand, and every
    `PairMachineEffects` variant rejects a consumer with implicit unit uses.
    Landed relationships cover implicit definitions as the result channel,
    retired dead definitions (`DeadConsumerUnitDefs`) and operand-swapped
    definitions kept for equality readers (`OperandSwappedUnitDefs`).
    Flag-consuming forms, clobbering rewritten rows and tied operands have no
    declaration.
  - Traps. `FaultDischargedByLiteral`, `FaultDischargedByObligation` and
    their two `...DeadUnitDefs` forms retire a consumer fault that the folded
    literal or a carried obligation makes unreachable. None admits trap
    preservation, where the rewritten form keeps the consumer's
    `MayArchitecturalFaultV1` surface, or hosted-trap effects.
  - Memory. `IndexedPointerReadFold` is the only memory relationship: one
    indexed pointer read folded to an offset read of the same bytes. A pair
    whose consumer or rewritten form writes memory has no declaration.
  - Stack and control flow. No relationship exists. Every variant requires
    alternatives that leave the stack unchanged and fall through, and neither
    instruction of a pair may be a terminator or sit in another block.

  Acceptance: each dimension above has a declared relationship with a firing
  rule on x86-64 and AArch64, a validator restatement that reconstructs the
  relationship from instruction records alone, the corruption, wrong-policy,
  boundary and budget negatives required by
  [stage-extension validation](omega-rust/optimization.md#validation-when-extending-a-stage),
  and one compiler-generated publication replay. A relationship that cannot
  be replayed independently stays rejected. Another literal identity over
  already-declared dimensions does not advance this item.

  Flag: the descriptor grows by enumeration, not composition.
  `PairOperandShape` has 12 variants that are hand-written products of literal
  position, result kind and tail-operand custody (for example
  `BinaryLeftLiteralConstantResultAuxiliaryUsesOrScratchDefs`).
  `PairMachineEffects` spells fault discharge times dead definitions as
  `FaultDischargedByLiteralDeadUnitDefs` and
  `FaultDischargedByObligationDeadUnitDefs`. The validator's `SourceShape` has
  30 variants, one per identity and operand position. Each of the 23
  selections also takes an `Optimization` tag and a `LiteralFoldPolicy` bit,
  and the per-identity test files run 566 to 1,292 lines. More than 20
  commits each added one identity or operand position while stack, control
  flow and trap preservation stayed at zero relationships. The general
  mechanism is independent descriptor axes
  (literal position, result kind, tail custody, fault discharge,
  implicit-definition disposition) that compose, so that a new identity is a
  catalog row and not a new variant, policy bit, validator shape and
  vocabulary tag.

- **EXACT-MACHINE-SIMPLIFICATIONS.** Execute copy removal, redundant-extension
  removal, address folding, compare/test selection, and scheduling on
  compiler-produced selected programs, each as an
  [atomic candidate with independent validation](wiki/spec/build/optimizations.md#atomic-candidates-and-independent-validation).
  Owner: `omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/`.
  `src/rewrites/` already holds 37 modules for these transformations:
  `copy_removal`, `redundant_extension`, `address_fold`, `literal_arithmetic`,
  seven compare and flag rewrites (`literal_*`, `constant_*`, `boundary_*`,
  `dead_compare`), and 26 interchange and relocation families. Each entrance
  takes caller-named instruction identities and replays by
  restore-by-content. None is an `Optimization` member, has a catalog row or
  candidate discovery, or has a caller outside its own tests, which
  hand-build `SelectedInstructionPlan` fixtures. `src/selected_optimization.rs`
  runs only the identity route or the selected-lowering literal folds, so
  none of them has changed a compiled program.

  Remaining work:

  - Give the stage an execution route under
    [catalogs and independent replay](omega-rust/optimization.md#catalogs-and-independent-replay):
    exact selection names, one ordered catalog whose descriptors retain rule,
    validator, policy, analyses, invalidations, budgets and applicability,
    candidate discovery that binds source and selection identities, and the
    call from `optimize_analyzed_selected_instructions`. Empty and nonempty
    selections stay on one physical route. Decision rows and receipts must
    survive replay through allocation and emission.
  - Separate validation from proposal. Every module's `validation.rs` calls
    the same `admission::admit` as its `rewrite.rs`, then checks that undoing
    the edit restores the source. That detects a wrong edit, not a wrong
    legality decision. The validator must reconstruct the preconditions
    without the producer's admission routine.
  - Scheduling refuses any window containing a call, hosted effect, barrier
    kind or call-roster entry, any cross-block move through a block that is
    not a plain `Source` block, and any control-flow shape without its own
    family. Replace the per-shape families with one relocation admission
    before covering more shapes (see Flag).
  - Compare/test selection: `SelectedInstructionKind` has three compare kinds
    and no bit-test kind, and `literal_minuend` admits only equality readers
    because no reversed ordering predicate exists. `copy_removal` substitutes
    within one block only. `address_fold` needs the `AddressOffset` producer
    in the consumer's block and applies the AArch64 scaled 12-bit
    displacement bound on every target, although x86-64 encodes disp32; the
    wider form needs target applicability in the descriptor.

  Acceptance: source-produced programs select each rule by exact name through
  `optimize_selected_instructions`, execute natively on a supported host, and
  replay independently after publication. The empty selection and each
  disabled rule reproduce identity output. Include one valid window that no
  current shape enumerates, and rejections for a register or condition-state
  hazard, a crossed call or hosted effect, a non-commuting memory access, a
  boundary settlement inside the window, a stale candidate, an exhausted
  budget, and a legality error the producer accepts but the validator must
  refuse. A hand-built plan passing its own module's test is not the customer.

  Flag: scheduling has grown one family per window shape: five in-block
  moves, each repeated as a commuting variant, ten cross-block shapes
  (`edge`, `predecessor`, `diamond`, `join`, `fork`, `arm`, `bypass`,
  `triangle`, `confluence`, `inflow`), and six of those repeated for runs.
  That is about 64,000 lines, 49,000 of them tests, and the product of shape,
  member or run, and plain or commuting is still open. The families share
  `window_hazards.rs`, `block_edges.rs`, `dead_path.rs` and
  `commuting_accesses.rs` and differ only in how they locate the window. The
  general mechanism is one relocation rule over a member run and a
  destination point that derives the crossed positions and edges on every
  path between them and the traversals that gain or lose the run, then
  applies the hazard, dead-path and commutation audits once. Separately,
  `literal_compare` and `literal_arithmetic` re-implement folds the cataloged
  pair rules already produce; widen candidate nomination for those
  descriptors under DECLARATIVE-PEEPHOLES instead of keeping a second
  producer.

  EXACT-SELECTION-FAMILIES and DECLARATIVE-PEEPHOLES own the cataloged
  pair-rule folds. ALIAS-AWARE-MEMORY owns the load, store and mutation
  rewrites in the same directory. PER-RULE-COVERAGE owns the disabled and
  downstream-replay legs once these rules are selectable.

## Proof-, ownership-, and state-aware optimization

- **ALIAS-AWARE-MEMORY.** Execute borrow-aware load forwarding, dead-store
  elimination, and mutation motion on compiler-produced selected programs,
  with non-aliasing justified by retained ownership evidence under
  [evidence and control flow](wiki/spec/build/optimizations.md#evidence-and-control-flow).
  Owner: `load_forwarding`, `dead_store` and `store_motion` in
  `omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/`.
  Each entrance takes one caller-named load or store, decides interference
  from the validated `memory_accesses` roster, walks across edges, and replays
  by restore-by-content. They handle exact-width `Store` and `StorePacked`,
  the place's own local slot, byte-sequence stores, indexed byte loads, and
  constant-count `CopyBytes` covers. None is an `Optimization` member, has a
  catalog row or candidate discovery, or has a caller outside its own tests,
  which hand-build `SelectedInstructionPlan` fixtures.

  Remaining work:

  - Bind the non-aliasing premise to evidence. `interferes` in each
    `admission.rs` treats a row naming another `PlaceId` as unable to observe
    or disturb the subject, on the documented premise that exclusivity was
    enforced before selection. `SelectedMemoryAccess` and the three receipts
    carry no loan, compatibility-certificate or accepted-fact identity for
    that premise, and [loans](wiki/spec/terminal-psi/loans.md) states that a
    live exclusive loan does not prove projections disjoint. Either retain
    the consumed fact identities in candidate and receipt, or establish at
    the roster's producer, under independent validation, that distinct
    `PlaceId`s in one function never overlap. `AnalysisKind::PlaceAliases`
    and `MemoryVersions` are declared in `optimization-core` and have no
    producer.
  - Add the execution route and separate validation from proposal as
    EXACT-MACHINE-SIMPLIFICATIONS describes. All three `validation.rs` files
    call their own rewrite's `admission::admit`.
  - Remaining refusals: forwarding into a join whose legs stored different
    registers, which needs a merged value on the edges; store motion through
    a fork or into a join, where the store lands at the last proven position;
    a dynamic dead extent without the byte-exact sequence write; a span whose
    count is not a materialized literal; a constant-index sequence row that
    lands off the dead byte still interferes; stores into `Structural`
    staging slots are not candidates.

  Acceptance: source-produced programs select each rule by exact name through
  `optimize_selected_instructions`, execute natively on a supported host, and
  replay independently after publication. The empty selection and each
  disabled rule reproduce identity output. Negative controls: a read through
  a shared reborrow or field projection of the stored place between two
  stores (the first store survives), an intervening call or hosted effect, a
  volatile or placed access, a partial overlap, a stale candidate, an
  exhausted budget, and one-field corruption of the receipt's fact
  identities. A hand-built plan is not the customer.

  Flag: byte-extent reasoning has one copy per rewrite. `intersects`,
  `reached_by` and `interferes` are defined in all three `admission.rs`
  files (the `dead_store` and `store_motion` copies of `interferes` differ
  only in the subject's name), the place, packed and local store-shape
  recognizers twice, and `commuting_accesses.rs` holds a fourth disjointness
  decision. Each access kind (packed, local slot, byte sequence, byte span)
  was then added to each rewrite as its own role-by-route case. The general
  mechanism is one owner that maps a roster row to place, storage route and
  extent (exact range, lower-bounded dynamic reach, or base plus index value)
  with may-overlap, must-cover and must-equal relations, consumed by the
  three rewrites and the commutation audit. `place_storage.rs` is the start
  of that owner.
- **REPRESENTATION-SPECIALIZATION.** Add field/variant relevance and
  invariant-window specialization.
- **CLEANUP-PRUNING.** Add cleanup and transition reachability pruning without
  losing affine/linear custody.
- **STATE-SPECIALIZATION.** Add state-argument/result specialization with exact
  edge provenance. One bounded family exists in
  `omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/state_specialization/`:
  an unconditional `Jump` that binds the parameter of a single-`Conditional`
  dispatch block to a proven Boolean constant is fused with the resolved arm
  and carries both edges' provenance and fuel settlements, with separate
  proposal, validation and application. `lib.rs` exports it, but it has no
  `PSI_PASS_CATALOG` entry, selection name, or caller outside its tests. It
  declines every machine holding a cyclic component and does not cover
  non-Boolean state arguments, conditional incoming edges, or result
  specialization.
  Acceptance: a source-produced state machine selects the rule by exact name
  through `optimize_abstract_operations`, publishes, and replays
  independently. Forged or stale edge provenance, a dispatch whose every
  incoming edge is constant, and a disabled selection behave as
  [validation when extending a stage](omega-rust/optimization.md#validation-when-extending-a-stage)
  requires.
- **INTERPROCEDURAL-SUMMARIES.** Add proof-bound inlining and the service/call
  summaries it needs. Transitive per-function effect summaries (observable,
  structural-state, crash, suspension, services, boundaries) and the direct
  call graph already exist in
  `omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/analyses/`
  and feed control-flow cleanup and loop-invariant scalar motion. They
  classify effects only: no summary names the places a callee reads or
  writes, and no rule inlines or otherwise rewrites across a call.
- **PROOF-DIRECTED-LOOPS.** Add loop-bound reasoning, induction
  simplification, and vectorization with exact lane semantics.

## Verification and rollout

- **PER-RULE-COVERAGE.** Keep positive, negative, boundary, disabled, budget,
  determinism, fixed-point/idempotence, and corruption coverage complete for
  every exact rule, as
  [validation when extending a stage](omega-rust/optimization.md#validation-when-extending-a-stage)
  requires. Do not call repeated reconstruction idempotence when the published
  artifact is not a legal second input. Every row of the checked
  [rule inventory](omega-rust/omega/representations/optimization-core/rules.md)
  has those legs beside its rule, as do the mandatory `runtime_spill` and
  `runtime_rematerialization` recovery rewrites, which have no disabled axis
  by design. The 40 uncalled rewrite modules under
  `selected-instructions-to-selected-instructions/src/rewrites/` (the
  EXACT-MACHINE-SIMPLIFICATIONS and ALIAS-AWARE-MEMORY families) have
  positive, negative, boundary, budget, determinism, fixed-point and
  corruption legs on hand-built plans only. They have no disabled,
  exact-selection, empty-identity, unsupported-composition or
  downstream-replay leg, because none has a selection name or a stage caller.
  An isolated applied-rule test does not establish compiler-generated
  application or publication support.

  Remaining work:

  - As those two items give a module a catalog row, add the missing legs
    through `optimize_selected_instructions` and the native-differential
    suites: exact selection, disabled and empty selection producing identity
    output, rollback by `--disable-optimization`, unsupported composition,
    and replay after allocation and emission.
  - New exact rules in any phase land with the full matrix; this item does
    not list them. `TargetOperations` and `PreAllocation` are empty identity
    boundaries today.

  Acceptance: every inventory row and every rewrite reachable from a stage
  exercises each axis through its stage's public entrance, with the published
  artifact as the second input for the fixed-point leg. No rule counts as
  covered while an axis is recorded as absent.

  Flag: coverage is maintained by hand. Under `rewrites/`, 45 test modules
  each define their own `budget()`, 44 their own `instruction()` fixture, and
  37 rewrite modules their own measured-boundary test, about 119,000 test
  lines in total; this entry tracked the result as prose. No check fails when
  a rule lacks an axis. `tests/architecture/optimizer_rollout` already
  derives the rule set from `Optimization::ALL` and the stage catalogs and
  reconciles names, phase, applicability and rollback. The general mechanism
  is a checked per-rule axis table in that gate, or one shared matrix harness
  parameterised by a rule fixture, so that a missing axis fails the
  repository gate.

- **BENCHMARKS.** Publish versioned compile-time, peak-memory, code-size, and
  runtime benchmarks keyed by exact rule selection and target.
