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
  Merge or delete umbrella/helper owners; put transformations in literal
  `X-to-Y` stages and optimizations in `X-to-X` stages.
  Acceptance: folders expose the connected program sequence, no competing
  entrances or orphan outputs remain, and coordinators only sequence typed
  stages. Renaming a helper or adding a wrapper is not completion.

- **REPRESENTATION-OWNERSHIP.** Finish
  `omega-rust/{omega,psi}/representations/`: one named program-root file beside
  `lib.rs`, with concept-owned subdirectories and current data independent of
  producer history. Move durable schemas out of transforms/backends where
  necessary, alongside their consuming stage changes.
  Acceptance: current programs outlive their producers; ordinary consumers
  read current data directly; historical inputs remain separate replay evidence.

## Product pruning and rollout

- **WORKSPACE-ROLLOUT.** Keep every rule explicit opt-in until the frozen-tree
  command `CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=1 mbx test --workspace
  --no-fail-fast` passes. Do not replace it with `--all-targets`, which omits
  doctests. Acceptance: the command passes from a clean checkout and every
  promoted exact rule has its required rollout evidence.

## Validation, translation, and publication

- **TRANSLATION-VALIDATION.** Complete independent source-to-target replay for
  admitted operations and transfers on the common graph. Scalar expression-family
  planners, Unit/structural whole-function templates, their catalogs and
  compatibility fixtures are retired: do not restore
  them to recover arithmetic, crash, cleanup or borrowed-call coverage. Extend
  ordinary graph operations and their receiving checks instead. Remaining native
  operations and publication routes fail closed until target applicability,
  result, effect, cleanup, proof and custody facts reconstruct.

  Close [physical-child replay](wiki/spec/terminal-psi/boundary_calls.md#closed-application-and-physical-occurrence)
  with one source-reachable selected-lowering operation carrying
  nonempty physical evidence through allocation, layout, native emission, and
  independent replay. Bind the immutable Terminal product, validated
  optimization projection, and each surviving boundary occurrence to exactly
  one physical child with its operator-application or boundary-settlement parent.
  Acceptance: post-Psi optimization
  and final publication reject missing, duplicate, stale, substituted, padded,
  or role-swapped children; only verified eliminated occurrences need no child.
  General calls depend on `FRAME-LAYOUT` and `GENERAL-CALL-CLOBBERS` below.
  These are native compiler guarantees, independent of package locks or
  `PackageInstance` construction. One source-reachable
  selected-lowering operation now carries nonempty physical evidence through
  the common physical stages to native emission and independent replay —
  `optimizer_opt_in.rs::
  selected_lowering_boundary_occurrence_replays_one_exact_physical_child`
  compiles a package-bound `boundary operator` program under
  `SelectedIncomingU12CompareImmediate`, replays the artifact, and binds
  exactly one `OperatorApplicationCoverage` physical child (nonempty
  machine/object spans, `ResolvedInternalCall` relocation) to the one
  surviving operator occurrence and the validated projection identity. The
  same test replays mutated custody: missing, duplicated, role-swapped
  (operator→boundary), padded-span, and substituted-projection children all
  reject. `selected_lowering_replays_one_physical_child_per_surviving_occurrence_role`
  retains both surviving roles in one program — the closed operator
  application and a hosted console exit — and binds each surviving
  occurrence to exactly one physical child: `OperatorApplicationCoverage`
  for the operator occurrence and `BoundaryTraitSettlement`
  (`HostedExitProcessI32`, `DirectInstructionBytes`) for the boundary
  occurrence; dropping either role, duplicating a child, or swapping a
  child's parent all reject on independent replay.
  `verified_eliminated_occurrence_needs_no_physical_child` closes the
  child-exemption half: two private machines each apply a covered boundary
  operator while a constant-false transition arm keeps the dead callee
  source-reachable through Terminal admission, so checked D29 coverage
  names both operations and the ordinary build binds each surviving
  occurrence to its own child. Replaying the published artifact sections
  through `optimize_verified_abstract_input` under `ControlFlowCleanup`
  folds the dead arm and proves the dead callee unreachable — pruned
  machine custody plus a `ProvenUnreachableAt` ledger row for the exact
  eliminated call node — and the validated optimized projection keeps
  only the surviving occurrence. The same coverage over the identity plan
  still projects both occurrences, so the exemption attaches to the
  verified elimination rather than the coverage row; a replayed physical
  child bound to the eliminated occurrence's canonical identity rejects,
  and dropping a still-required child rejects too.

- **CUSTODY-MUTATION-COVERAGE.** Complete authenticated one-field mutation
  tests for every remaining manifest, receipt, codec, and artifact-custody
  family. Acceptance: each representable field can be changed independently,
  the containing identity can be recomputed, and independent replay still
  rejects the substitution. Landed: installed function rows
  (`image-emission/tests/artifacts.rs`,
  `installation_function_row_rejects_every_one_field_substitution`) — machine,
  attachment, unit stack, and scalar stack are independently representable and
  rejected by replay; text offset, byte count, dropped rows, and the unit-body
  flag are rejected at encoding as non-canonical. Landed: installed internal
  unit call rows
  (`installation_internal_unit_call_row_rejects_every_one_field_substitution`)
  — the cleanup-owned call's byte count is independently representable and
  rejected by replay; every other field on operation- and cleanup-owned rows,
  dropped rows, and the semantic-result projection are rejected at encoding
  as non-canonical. Landed: installed boundary-settlement rows
  (`installation_boundary_settlement_rejects_every_one_field_substitution`)
  — `psi_operation`, `boundary`, the write row's `operation_ordinal`, the
  admitted-provider execution identity, byte-sequence `literal_operation` and
  structural-type identity, scalar-argument `source_value` and `immediate`
  are independently representable and rejected by replay; machine,
  text/code offsets, byte counts, realization/execution swaps, argument
  rosters, completion custody, native results, the exit row's ordinal, a
  duplicate `psi_operation`, and roster reorder are rejected at encoding as
  non-canonical; a dropped row is rejected by replay. Landed: installed
  compiler-private callback rows
  (`installation_private_function_row_rejects_every_one_field_substitution`)
  — callback-thunk continuation and placement index, source-Psi fingerprint,
  machine, and scalar-ABI parameter/result values are independently
  representable and rejected by replay; a non-thunk identity kind, text
  offset, zero/extended byte count, non-canonical ABI placements and value
  collisions, and a dropped row are rejected at encoding as non-canonical.
  Landed: installed dynamic conformance tables and dynamic call rows
  (`installation_dynamic_conformance_table_rejects_every_one_field_substitution`)
  — the application report fingerprint, an unresolved slot's target, and the
  call's operation, text offset, and byte count are independently
  representable and rejected by replay; the application commitment, a zero
  fingerprint, table data offset and byte count, slot row indices and data
  offsets, resolved-slot retarget or erasure, unknown slot targets, the
  caller machine, initial and rebound sources, the selected table byte
  offset, the realization, and swapped, dropped, or duplicated slots,
  tables, and calls are rejected at encoding as non-canonical. Landed:
  installed dynamic-parameter call rows
  (`installation_dynamic_parameter_call_rejects_every_one_field_substitution`)
  — `operation`, `source_value`, `requirement_slot`, in-function text offset,
  and in-bounds byte count are independently representable and rejected by
  replay; an unknown or other-function machine, out-of-function text offsets,
  zero or overflowing byte counts, and a duplicated row are rejected at
  encoding as non-canonical; a dropped row is rejected by replay. Landed:
  installed stored dynamic call rows
  (`installation_stored_dynamic_call_rejects_every_one_field_substitution`)
  — the table's application report fingerprint and an unresolved slot's
  target, the dispatch's operation and text offset and byte count, the
  establishment's operation and byte count, and the descriptor and
  selection ordinals and descriptor home offset are independently
  representable and rejected by replay; the application commitment, caller
  machine, unselected realization, unknown source place, selected table
  byte offset, empty or out-of-function byte counts, and out-of-range or
  overlapping establishment and dispatch offsets are rejected at encoding
  as non-canonical, as are dropped, duplicated, or swapped
  conformance-table slots and dropped or duplicated calls and tables.
  Landed: installed forwarded dynamic-parameter call rows
  (`installation_forwarded_dynamic_parameter_call_rejects_every_one_field_substitution`)
  — the call's operation, source value, scalar type, self-function callee,
  text offset, and byte count are independently representable and rejected
  by replay; an unknown or other-function machine or callee, parameter
  ordinals, a cleared or non-integer scalar type, out-of-function text
  offsets, an empty byte count, and a duplicated row are rejected at
  encoding as non-canonical; a dropped row is rejected by replay. Landed:
  installed forwarded dynamic-descriptor adapter, table, and call rows
  (`installation_forwarded_dynamic_descriptor_rejects_every_one_field_substitution`)
  — the table's application report fingerprint, and the call's operation,
  other-function callee, source place and access, text offset, and byte
  count are independently representable and rejected by replay; adapter and
  table application commitments, adapter row index, realization, text
  offset and byte count, table data offset and byte count, slot row index,
  realization and offset joins, the call's machine, callee, commitment and
  semantic result, and out-of-range offsets and byte counts are rejected at
  encoding as non-canonical, as are dropped or duplicated adapters, tables,
  and calls. Landed: installed semantic code attribution rows
  (`installation_semantic_code_attribution_rejects_every_one_field_substitution`)
  — operation- and edge-site identities and in-bounds byte counts are
  independently representable and rejected by replay; an unknown machine, a
  roster-reordering ordinal, drifted code and text offsets, a past-function
  byte count, and the boundary-joined return-edge row's site, ordinal, and
  byte count are rejected at encoding as non-canonical; a dropped unjoined
  row is rejected by replay while a dropped return-edge row, a duplicated
  site, and a reordered roster are rejected at encoding. Landed: installed
  privileged port-effect rows
  (`installation_port_effect_rejects_every_one_field_substitution`)
  — an unbound row's operation, service, port, value, and ordinal are
  independently representable and rejected by replay; its machine, code
  offset, byte count, and text offset are rejected at encoding as
  non-canonical; every field of the `MetadataOnlyPort`-consumed row is
  rejected at encoding through the canonical offset joins or the settlement
  realization; a dropped unbound row is rejected by replay while a dropped
  consumed row, a duplicated `(machine, operation)` pair, and a reordered
  roster are rejected at encoding. Landed: installation header and manifest
  axes (`installation_header_rejects_every_one_field_substitution`) — the
  program fingerprint, architecture, whole-target, and bound image
  fingerprint substitutions, the image-section layout addresses, the retained
  compiler text-validation receipt, and its report-only derivation
  fingerprint are independently representable and rejected by replay, as is
  every derivation-digest input once the receipt identity is honestly
  recomputed; the profile decision and the committed component-progress
  manifest and acceptance identities sit outside the image join and break
  the published record identity a deployment journal replays; a retained
  subsystem, unsupported target facts, non-canonical section projections,
  and receipt fields without a consistent derivation digest are rejected at
  encoding as non-canonical, while the COFF subsystem value is rejected by
  replay and its absence or a non-COFF target at encoding; the magic,
  format, and vocabulary markers, unknown enum tags, the reserved field,
  zero profile and progress identities, and presence-flag lies reject at
  the wire. Landed: selected provider-plan closure rows
  (`installation_selected_provider_plan_rejects_every_one_field_substitution`)
  — substituting, extending, or dropping an unexecuted selected plan is
  independently representable, produces the record an honest admission of
  the mutated closure builds, and breaks the published record identity a
  deployment journal replays while the image join cannot see it;
  substituting or dropping the executed plan, clearing the roster,
  reordering it, or duplicating an entry is rejected at encoding through
  the settlement-closure and canonical-order joins, a reported execution
  outside the selected closure or a reported closure diverging from the
  image's retained executions is rejected at admission, and a zero
  identity, a non-canonical or duplicated wire order, and an uncarried
  count reject at decode. Landed: installed structural-return rows
  (`installation_structural_return_rejects_every_one_field_substitution`)
  — the affine and linear rows' result places and the linear row's carried
  claim identity are independently representable and rejected by replay;
  machine, `psi_edge`, scalar and structural parameters, placements, source
  and result signature fields, affine-lane claims, trivial locals and
  discards, code offset, and byte count are rejected at encoding as
  non-canonical, as are a machine-descending swap and a duplicated row,
  while a dropped row is rejected by replay. Landed: installed attached-Unit
  scalar-call rows (`image-emission/tests/internal_unit_scalar_calls.rs`,
  `installation_internal_unit_scalar_call_row_rejects_every_one_field_substitution`,
  `951c09bb9c`) — the call's `operation_ordinal`, in-span `byte_count`, the
  nested argument and result `code_offset`/`byte_count` intervals, and a
  `Home` argument source naming an earlier producer are independently
  representable and rejected by replay, as are dropped rows; machine, text
  offset, owner, target, call-plan parameters/result/stack alignment, every
  result-home field, the result source, out-of-span or zero intervals,
  constant or out-of-order ordinals, argument parameter index, destination
  and source substitutions, extra or dropped arguments, and duplicated or
  swapped rows are rejected at encoding as non-canonical. Landed: sealed
  optimized-object artifact records, manifests, and custody receipts
  (`compiler/tests/object_artifact_custody.rs`,
  `optimized_object_artifact_custody_rejects_every_one_field_substitution`)
  — every artifact-record field (terminal-artifact, semantic, obligation,
  and proof digests, the debug section, selections, all four target axes,
  semantic entry, the six manifest identities, object and container
  identities, and all four statistics) and every manifest field (artifact,
  terminal-artifact, semantic, selections, all four target axes, semantic
  entry, container-manifest, object, container, and all four statistics)
  is independently representable under an honestly recomputed containing
  identity and rejected by independent replay against the retained terminal
  artifact and relocation-free object container; each of the six
  custody-receipt fields is rejected by replay; foreign or stale containing
  identities, the closed stage and unavailable markers, unknown vocabulary,
  architecture, object-format, and optional-section tags, a zero machine
  identity, and trailing or truncated envelopes are rejected at canonical
  decoding. Landed: deployment-journal canonical records, durable storage,
  and runtime recovery joins (`component-publication/src/tests.rs`,
  `component_deployment_journal_rejects_every_one_field_substitution`,
  f125c9e2d4) — the journal identity, phase, both contract identities,
  every prior, live-era, and candidate era-occurrence axis, each live-era
  row's state and active-entry count, the entry-plan and admission-receipt
  identities, the envelope identity and canonical bytes, and every
  admission class, subject, and identity are independently representable
  and rejected by replay: the Prepared-to-Activated transition binds the
  exact durable predecessor, durable storage replays the retained record
  against its bytes, restart reconciliation binds the journal and
  contract identities plus the offered recovery choices, and the runtime
  recovery join replays occurrence axes and canonical installation
  evidence against the live ledger; honestly recomputed installation
  evidence still encodes yet rejects at the same replays; zero axes,
  empty text and byte fields, unordered, duplicated, dropped, or
  extended rosters, and a drifted installation fingerprint or record are
  rejected at encoding or decoding as non-canonical, as are the magic,
  version, phase, state, and presence tags, count ceilings, non-UTF-8
  text, trailing bytes, truncation, and on-disk tampering at decode.
  Landed: relocation-free object-container plans, containers, manifests,
  and custody receipts (`compiler/tests/object_container_custody.rs`,
  `relocation_free_object_container_custody_rejects_every_one_field_substitution`)
  — every manifest field (the source text-section manifest and text-section
  identities, program fingerprint, fuel schedule, selections, selected
  plan, all four target axes, semantic entry and its symbol, the object
  and object-container identities, and all seven statistics) and every
  object-plan field (the source text-section identity, program
  fingerprint, fuel schedule, selections, selected plan, the whole target
  and all four target axes, text-section name, alignment, byte count, and
  bytes, each symbol-row field, dropped and duplicated rows, semantic
  entry and its symbol, and the relocation-record count) is independently
  representable under an honestly recomputed containing identity and
  rejected by independent replay across all four declared targets; each
  of the five custody-receipt fields and the container's object identity,
  container identity, and honestly re-identified truncated bytes are
  rejected by replay or decoding; foreign or stale containing identities,
  the closed stage, vocabulary, symbol-policy, requirement, and
  unavailable markers, unknown architecture and object-format tags, zero
  fuel-schedule, machine, and symbol identities, and trailing or
  truncated envelopes are rejected at canonical decoding.
  Landed: executable-installation decoded containers and canonical wire
  containers
  (`executable-installation/src/executable_installation/container/tests.rs`,
  `executable_container_rejects_every_one_field_substitution`, and
  `container_bytes/tests.rs`,
  `executable_container_wire_rejects_every_one_field_substitution`) —
  the claimed artifact identity, architecture, code bytes and extent,
  contracts, declared footprint, placement plan, phase, alignment,
  permitted range, machine regime, and installation scope, the entry set
  and each entry's identity and code offset, the relocation set and each
  relocation's kind, destination, target, and addend, entry and
  relocation rosters, and the strong authority commitments are
  independently representable under an honestly recomputed compatibility
  fingerprint and are rejected by independent replay; the exact proof
  payload stays outside the content identity yet is replay-bound
  evidence; envelope-only axes (declared length beyond the joined
  sections, section roster order, payload coordinates) canonicalize to
  the identical artifact; report-coordinate, directory-identity, roster,
  marker, and count substitutions that cannot keep the canonical joins
  reject at validation, while every raw header, directory, placement,
  relocation, entry, and authority byte-field substitution — including
  remarking v2 bytes as v1 — rejects at canonical decoding, with
  semantic-value payload substitutions landing at the
  content-fingerprint join. Landed: a genuinely emitted Windows x64
  foreign call's installed foreign-call-stack row
  (`installation_foreign_call_stack_row_rejects_every_one_field_substitution`)
  — a normalized `kernel32.dll` PE import call under an admitted provider
  execution with exact site bytes, evaluated call and boundary-entry
  plans, MXCSR save/restore custody, outbound shadow-space evidence, and
  an admitted same-stack contribution survives object construction, PE
  image emission, and installation, so every retained field — owner,
  text offset, caller-live bytes, provider-plan report identity,
  contribution report identity, contribution commitment, contribution
  bytes, and contribution alignment — is independently representable and
  rejected by replay; a provider-plan identity outside the selected
  closure is rejected at encoding as non-canonical, and a dropped row is
  rejected by replay. Landed: canonical component descriptions
  (`component-description/src/component_verification/tests.rs`,
  `component_description_rejects_every_one_field_substitution`,
  `component_description_declared_fields_stay_identity_bound`) — the
  schema, frontier, and embedded artifact substitution, every
  module-derived roster field (import requirement and contract identity,
  export identity, entry kind/identity/evidence, outgoing-authority class,
  identity, and evidence on both sealed and unsealed requirement rows,
  custody kind/identity/evidence, provider plan digest and requirement
  roster, a zeroed provider-closure digest, and obligation kind and
  identity), plus dropped or forged rows are independently representable
  and rejected by replay; declared evidence-only fields (import slot
  index, provider report identity, obligation detail, a nonzero
  closure-digest substitution, the optional realization identity,
  assumption-bound evidence weakening, declared-row kind/identity/drop,
  and over-declared import and obligation rows) stay identity-bound
  rather than replay-bound, while a rebound declared-row digest and a
  renamed roster digest reject by replay; duplicated rows, an
  over-bound identity, a truncated embedded artifact, and raw wire
  substitutions of the magic, frontier, entry-kind, and entry-evidence
  tags, an over-bound roster count, trailing bytes, and truncation are
  rejected at canonical decoding, and a pure roster reorder canonicalizes
  to the identical encoding. Landed: installed-realization lifecycle
  records and occurrence custody
  (`executable-installation/src/executable_installation/tests.rs`,
  `installed_realization_rejects_every_one_field_substitution`) — every
  field retained in the installed-occurrence evidence (artifact code
  bytes, architecture, identity, entry set, entry identity and code
  offset, relocation roster, admission receipt, retained container-proof
  presence, digest, and bytes, installed identity, placement identity,
  installation scope, every placement-constraint axis, installation
  audience, extent base, length, address space, rights, provenance,
  mapping era, and lineage, realized footprint, final validation
  identity, and W^X mode) is independently representable under an
  honestly recomputed occurrence digest and rejected by replay through
  the one-shot registry authority, the quarantined stale-entry fault,
  and the install, retirement, and quarantine gates binding the
  substituted evidence in either direction; resolver-dependent final
  bytes diverge under equal artifact identities, compact installed
  report-identity collisions cannot forge stale-entry or receipt
  evidence, and an unretained provider-issuance origin canonicalizes to
  the identical realization; install authority scoping, receipt binding,
  visibility completeness, and the unsupported execute transition, each
  retirement quiescence, execute-removal, write-restore, and
  required-fact claim, and each quarantine disable, unmapping,
  reservation, and attributed-cause claim substitute independently,
  while a claimed quarantine report identity and attributed cause are
  adopted verbatim and replay-bound; zero normalized identities are
  unrepresentable across the family. Landed: installed integer-constant
  rows (`image-emission/tests/internal_unit_scalar_calls.rs`,
  `installation_function_integer_constant_rows_reject_every_one_field_substitution`)
  — every field of a retained constant no call references (defining
  operation, source value, scalar type, value, operation ordinal), a
  dropped unreferenced row, and a distinct inserted row are independently
  representable and rejected by replay; every field of the constant the
  scalar-call argument sources name, a dropped referenced row, an
  address-carrier or out-of-width scalar type, an unadmitted or
  sign-mismatched value, non-increasing or successor-overtaking ordinals,
  defining-operation and source-value collisions, and swapped or
  duplicated rows are rejected at encoding as non-canonical. Landed:
  fixed-frame
  function-relative realization manifests, custody receipts, and
  retained exit contracts
  (`compiler/tests/realization_custody.rs`,
  `fixed_frame_realization_custody_rejects_every_one_field_substitution`)
  — every representable manifest field (the four phase-selection
  identities, selected-lowering completion, both manifest identities,
  selected plan, machine-effect and post-allocation machine identities,
  both encoding identities, both resolved-layout identities, the
  optional branch-relaxation and post-allocation-optimization custody
  slots, the exit-contract identity, all four target axes, layout
  policy, the fixed-frame disposition and both member identities, and
  all six statistics), each of the eight custody-receipt fields
  (allocation evidence, machine, callee-saved requirements and storage,
  frame layout and protocol, exit contract, and the manifest itself),
  and every representable retained exit-contract field (the five joined
  identities, layout custody, policy, frame disposition, entry
  assumption, stack pointer, alignment, red zone, result view,
  callee-saved roster, and each function row's machine, entry block,
  stack delta, modified units, and process-exit roster plus every
  return row's block, source edge, instruction, offset, bytes, value,
  trap, and mechanism) is independently representable under honestly
  recomputed containing identities and rejected by independent replay
  across all four declared targets; detached foreign allocation and
  exit-contract records and corrupted encoding and baseline-layout
  bytes reject through their component replays; stale or foreign
  manifest and contract identities, the single-variant stage, scope,
  and seven unavailable markers, unknown completion, relaxation,
  optimization-custody, optimization, architecture, object-format,
  layout-policy, and frame-disposition tags, conflicting physical
  transformations, trailing bytes, and truncation are rejected at
  canonical decoding.

## Psi optimization and loops

- **GENERAL-CYCLIC-EXECUTION.** Complete ordinary cyclic Terminal Psi execution
  with authenticated SCCs, dominance and
  frontiers, optional well-founded ranking, productive unranked components,
  and structured finite-work failures. The dedicated unsigned-countdown native
  carrier is removed; its old custody rejects. Extend the common graph and
  ordinary ranking evidence rather than restoring a second native route.

  First bounded call-composition milestone: implement the
  [ranked callee on a projected receiver](wiki/spec/language/termination.md#ranked-callees-on-projected-receivers),
  beyond the current whole-entry-only admission. Acceptance: an ordinary caller
  borrows a nested field, its ranked callee preserves that referent across
  backedges, and the caller observes writes after return. Conflicting parent
  access and missing/invalid callee ranking reject. Validate argument identity,
  call/return, cleanup, and composed resource bounds through native replay and
  execution on both Linux architectures. This needs no new receiver syntax.
  Broader changing-reference transfers still require loan/alias and ranking
  substitution work; increasing the parameter count alone does not close them.

- **TERMINAL-SCC-CONSUMERS.** Retarget LICM and other loop consumers from the
  exact countdown slice to validated Terminal SCCs. General invariant
  discovery, profitability, and motion remain open.

- **GENERAL-LICM.** Implement motion only through transformations that
  invalidate and reconstruct component, loop-carried custody, ranking,
  provenance, effect, and fuel evidence. The dedicated countdown zero/one
  relocation is not general LICM authority. The shared-source preheader
  boundary relocates scalar constant leaves, side-effect-free scalar
  computations — including exact, wrapping-divide/remainder, and
  saturating-divide/remainder variants whose verifier-discharged totality
  obligation moves byte-exact inside the relocated operation — place
  observations whose storage root rebinds to its invariant
  representative, `ByteSequenceRead` reads whose `index`
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
  identity, and scalar-signature `Call`
  nodes — the boundary's first call relocation — whose callee's
  transitive effect summary proves no observable effect, crash, or
  suspension (the `structural_state` axis stays exempt because a scalar
  call passes no places) while every member node stays unobservable, so
  hoisting the call's possible non-return reorders nothing anyone could
  see; a call carrying `crash_continuations` keeps its crash-route
  custody inside, and discharged `requirement_obligations` move
  byte-exact inside the moved operation. Invariant discovery resolves
  member scalar and structural
  parameters transitively across component-internal edges to the
  representative every reaching edge agrees on — and a member parameter
  every reaching edge binds to one run-covered member result substitutes
  to that preserved result — rebinding moved operands
  and observed roots to the preheader-visible anchor. The insertion
  preheader is the one block every authenticated entry edge departs —
  several entry edges of one dispatching terminator share it, while
  entries departing different blocks decline — and non-leaf motion
  additionally requires every terminator successor to enter the component
  plus a member block every traversal that leaves executed; the freeze
  fence re-derives both halves of that non-speculative gate from the
  authenticated topology. Remaining: other
  non-scalar families, profitability, and motion beyond the shared-source
  preheader. Operand-carrying structural establishments
  (`EstablishScalarArray`, `EstablishRecord`, `EstablishScalarCase`,
  `EstablishPrimitiveLocal`, `EstablishTrivialAffineLocal`) and
  structural-signature calls (`CallStructuralScalar`, `CallStructural`,
  `CallUnit`) currently have no admitted relocation evidence — and the
  establishments other than the byte literal still have no source route
  into a cyclic member block, since scalar-graph arrays only emit as
  call arguments and a structural `let` inside a Unit state removes the
  machine's ranked cycle evidence — so the next non-scalar family needs
  an admitted source shape before admission work can begin.

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

- **SELECTED-ABI-VALIDATION.** Validate ABI operands, calls, clobbers, effects,
  traps, provenance, cleanup, and logical fuel across every selected rule.
  The effect catalog now pins every encoded footprint to its owning
  semantic: `validate_declaration` binds the declared trap surface —
  hosted trap results name their owning hosted operation, and a
  never-faulting declaration cannot also declare a memory access — and
  `validate_encoded_effects` binds each row's shape, so control-flow
  rules pin their exact encoded control inside the barrier class,
  returns and calls can no longer borrow each other's activation-stack
  and return-address lifecycle rows, indexed-pointer and frame-storage
  rows reject foreign semantics, and the plain fallthrough row admits no
  hosted trap shape (crate `nextest`: 41 pass, including
  cross-borrowing, foreign-semantic, trap-understatement, and
  catch-all-evasion negatives; 234 isa-x86_64, isa-aarch64, and
  register-environment lib tests confirm every real catalog row still
  validates on Linux x86-64). Each encoded catalog row's operand custody
  now also restates its register-constraint row exactly — external writes
  equal the row's Def and UseDef operands and external reads equal its Use
  and UseDef operands — with named exceptions only for returns, whose
  result homes are placed for the caller, and the all-aliased x - x
  subtract realization, which depends on neither input home (crate
  `nextest`: 42 pass; the register-environment forgery sweeps moved the
  newly structural rejections from canonical-replay mismatch to
  `InvalidEncodedEffects`). Landed: the x86-64 `WrappingRemainderI64`
  row no longer declares the early-clobber fixed `rdx` output that
  liveness and fixed-precolored interval validation refuse — the divisor
  is pinned to `rcx` (a fixed view disjoint from `rax` and the scratch)
  so the realized form may zero `rdx` before reading it, and the `rdx`
  scratch is an ordinary late definition; selected construction unshares
  a dividend/divisor register through a witnessed copy that independent
  replay mirrors, a catalog-wide invariant now proves no operand may
  carry a fixed early-clobber view, and a liveness witness derives the
  real `X86_64_REMAINDER_I64` and `AARCH64_REMAINDER_I64` rows and admits
  them while the replaced shape still rejects (crate `nextest`: all
  isa-x86_64, register-environment, target-operations, and
  selected-instructions lib tests pass on Linux x86-64; the remainder
  contract forgery suite now covers all-fixed rows, and the host-native
  modulo canaries' upstream failures predate this row — verified
  byte-identical against base `86328e9b`). Remaining: provenance,
  cleanup, and logical-fuel dimensions; Windows and macOS runs were
  unavailable on this host.

## Register allocation and frames

- **SPILL-REALIZATION.** Extend executable spill recovery beyond dominating
  nonaddress instruction results in acyclic or cyclic functions and
  edge-initialized block parameters in acyclic or cyclic functions; admitted
  uses now include body and terminator operands, register-transport arguments
  on outgoing successor edges, and stored structural-transport snapshot
  arguments on edge-transfer continuations.
  The owning paths are `selected-instructions-to-register-homes/src/assignment/runtime_spill/`
  and `selected-instructions-to-selected-instructions/src/rewrites/runtime_spill/`.
  Complete broader CFG/type and fixed-use recovery, composition with selected
  recovery rules, physical slot reuse/coloring, and independent validation.
  Existing logical spill plans grant no frame, unwind, instruction, or
  publication authority.
  Extend final spill-inclusive frame demand into stack provisioning under the
  [compiler-owned stack contract](wiki/spec/resources/storage.md#compiler-owned-stack-accesses).
  Keep `WRITE-ONLY-BORROW`'s source-backed three-call regression as the hosted
  execution control, including its installed-image demand replay and stale-frame
  rejection. Reuse the existing final-local-frame demand path, not a parallel
  spill-byte estimate.
  Acceptance: slot reuse is not double-counted; changed allocation/frame
  realization invalidates stale demand; insufficient supply rejects before
  execution; and generated loads/stores independently replay their physical
  geometry and value lineage without adding source crash routes. A byte ceiling
  alone must not stand in for valid stack backing. Target-required probing and
  setup remain frame/provisioning work, not an owner-blocked language choice.

- **ALLOCATION-REFINEMENT.** Complete coalescing, live-range splitting,
  fixed/precolored intervals, and rematerialization cost decisions while
  preserving exact register-unit aliases, liveness, and target custody.
  Landed: runtime-spill recovery orders rematerialization before private
  storage — a pressured victim defined by one pure MaterializeI64
  regenerates a fresh materialization at each admitted flexible use, creating
  no reload interval or stack slot, independently replayed through the
  post-allocation manifest ledger (codec 9) on all four targets
  (`runtime_rematerialization_pressure::loop_carried_rematerialization_replays_through_callable_publication`).
  Landed: home-assignment coalescing ranks legal candidates by the guaranteed
  coalesces a still-unassigned constrained neighbor would lose, after
  assigned-partner edges and partner votes, mirrored in independent replay and
  the scan reference
  (`copy_affinity_avoids_stealing_a_constrained_neighbors_guaranteed_home`).
  Landed: runtime-spill recovery admits fixed-view body instruction uses —
  a pressured victim feeding an ABI-pinned call operand gets its own
  address/load pair immediately before the consumer while the operand
  keeps its fixed view, pinning the fresh reload to that physical unit
  for exactly the load-to-use window on all four targets
  (`fixed_view_instruction_uses_pin_their_reload_at_the_call_operand`).
  Landed: the default allocation route sequences the fixed/precolored
  interval stages for authenticated unresolved entry-fixed-view
  transitions — segment homes, a leaf-local fixed-view copy policy,
  selected reanalysis, and post-copy homes retain and publish with
  replay-bound evidence while the declared shared-entry route and
  runtime-spill recovery stay distinct
  (`default_path_routes_entry_transitions_into_the_leaf_local_fixed_view_sequence`).
  Remaining: coalescing across split points and live-range splitting.

- **FRAME-LAYOUT.** Extend exact nonzero-frame realization beyond the landed
  CFG families: red-zone policy, probing, unwind
  information, stable-address loans, and dynamic-allocation constraints.
  General calls need target-owned frame, callee-save, link-register, and
  call-site alignment plans. Witnessed: the ordinary three-block/two-return
  fixture and the cyclic loop-carried runtime-spill fixture (six emitted
  blocks, backward branch, nonzero local spill storage) both replay through
  frame application, relocation-free object construction, and validated
  ordinary callable publication on x86-64 and AArch64
  (`runtime_spill_pressure::loop_carried_spill_frame_replays_private_accesses_through_callable_publication`).
  A scalar caller passing one argument past each target's register capacity
  now replays its exact outgoing-ABI-area write and the callee's exact
  incoming-activation read against the validated frame geometry through the
  same publication boundary on all four targets
  (`register_arity::stack_argument_calls_replay_frame_accesses_through_callable_publication`).
  Probing is landed: a caller whose outgoing ABI area exceeds one
  stack-commit granule commits its frame through an exact per-granule
  touch roster recorded in the validated layout and replays through
  ordinary callable publication on all four targets
  (`stack_probe_commit::wide_outgoing_area_commits_through_exact_probe_roster_and_publication`).
  x86-64 emits move-and-touch chunks; AAPCS64 emits a
  shifted-then-unshifted `sub sp` pair per chunk followed by an
  `ldr xzr, [sp]` touch of each newly entered page, and `FrameAddress`
  forms every displacement a committable frame resolves through the same
  add pair (both Darwin's 16 KiB unprobed granule and Linux's 4 KiB
  probed granule publish). The red-zone policy is landed: a leaf whose
  fixed-register divide pressure recovers through runtime spill keeps its
  whole addressed extent resident below the unadjusted entry stack pointer
  on System V AMD64 — empty prologue and epilogue spans, signed
  below-RSP displacements replayed row by row against the validated
  geometry — while the same program stays an ordinary committed frame on
  every other admitted ABI, a suppressed or invented resident extent
  replays false, and the artifact still publishes as an ordinary callable
  on all five admitted targets
  (`red_zone_resident_frame::resident_leaf_spill_frame_publishes_through_ordinary_callable_entry`).
  Stable-address loans are landed: a caller that establishes a `Pair`
  record in activation-local storage and calls its borrowed `total`
  receiver materializes that slot's stable address across the call, and
  the validated layout records the exact loaned-local roster —
  canonically ascending, duplicate-free, closed over declared local
  slots, and excluding private spill reload windows — while replay
  independently recovers the same roster from the physical
  `FrameAddress` operations rather than the producer's claims. Every
  materialized activation-local address resolves only through a
  rostered slot's committed coordinates; an omitted, invented,
  reordered, or misattributed loan fails closed, and the artifact still
  reaches ordinary callable publication on all five admitted targets
  (`stable_address_loans::loaned_local_addresses_replay_through_ordinary_callable_entry`).
  Unwind information is landed: the validated layout records each
  frame's exact unwind roster — the ordered view restorations the
  epilogue performs, with a saved link register first and every
  preservation slot in reverse save order so the roster is strictly
  descending frame offset — plus the committed extent the unwind
  releases and the return-address custody the continuation is
  recovered through, so the record alone is a complete unwind
  description. Frame identity moves to v10 and binds the roster;
  replay recovers the same roster from the validated preservation
  storage and custody rather than the producer's claims, and the
  frame protocol's emitted epilogue performs exactly the recorded
  sequence byte for byte on all five admitted targets. An omitted,
  invented, reordered, or misattributed restore — or a wrong release
  or custody restatement — fails closed, epilogue bytes that do not
  encode the validated roster reject, and the artifact still reaches
  ordinary callable publication on all five admitted targets
  (`unwind_roster::unwind_restore_roster_replays_through_ordinary_callable_entry`).
  The call-site stack contract is landed: a calling frame records the
  selected preservation convention's declared stack alignment as both
  its pre-call boundary and its ABI restatement, and commits an extent
  carrying the (architecture, convention) pair's call-entry residue —
  eight bytes of call-pushed return address on x86-64, zero on AArch64
  — so the post-prologue stack pointer meets the declared boundary
  before any call site runs rather than a literal the frame allocator
  happened to know. An undeclared pair answers no contract instead of
  borrowing another row's constants. A caller keeping values live
  across three scalar calls replays the emitted prologue's exact
  commit and the epilogue's matching release against the recorded
  fields through ordinary callable publication on all five admitted
  targets; a row recording a borrowed alignment or a residue-breaking
  extent fails closed under independent replay
  (`call_site_alignment::call_site_stack_contract_replays_through_ordinary_callable_entry`).
  Acceptance: every admitted frame policy
  replays its exact physical accesses through callable publication;
  requirements artifacts remain non-authoritative until that replay
  succeeds. General-call legs are landed: the frame resolves its
  target-owned policy and call-site stack contract through declared
  (Architecture, ObjectFormat) pair matrices, the callee-save plan is
  recorded and independently replayed, saved-link-register restore
  order rides the unwind roster (frame identity v10), and call-site
  alignment rejects residue-breaking rows under replay. Remaining:
  dynamic-allocation constraints only, which are blocked upstream —
  no runtime-sized stack-allocation representation exists (every
  selected local/outgoing slot resolves to a static byte extent), so
  this leg waits on a language/Terminal-Psi alloca-style contract
  rather than on frame-layout work itself.

- **GENERAL-CALL-CLOBBERS.** Extend live-across-call allocation and clobber
  validation from the landed attached-Unit fork/join slice through general
  scalar and structural calls on each ABI. Witnessed: an ordinary
  scalar-returning caller keeping a parameter and earlier call results live
  across later calls reports and independently replays callee-saved
  requirements on `linux_x64` and `linux_arm64`
  (`tests/native-differential/.../fixtures/scalar_call_preserving.rs`,
  `.../register_allocation/callee_saved_requirements/`). The same callers now
  report, store, and replay convention-specific preservation on all five
  native targets: System-V, Microsoft x64 (Windows and UEFI), AAPCS64, and
  Darwin AAPCS64, including roster distinctions (Microsoft x64 adds rdi/rsi
  and xmm6-15; Darwin moves x18 from caller-saved to fixed), clobber-candidate
  exclusion at call points, exact callee-save slot geometry, and per-target
  frame-layout corruption replay
  (`.../register_allocation/callee_saved_requirements/`,
  `.../register_allocation/callee_save_storage/`,
  `.../register_allocation/runtime_scalar_call_chain/`). A caller passing a
  borrowed structural argument through three scalar-returning call sites —
  extent pointer live across every call, scalar parameter live across the
  first call, first result live across the second — now reaches the same
  chain: clobber-candidate exclusion and callee-saved homes on all five
  targets, legalized-plan and call-template corruption replay on all five
  targets, exact frame-layout and protocol-encoding replay, and fixed-frame
  object-artifact publication with custody rejection on both Linux ISAs
  (`tests/native-differential/.../fixtures/structural_call_preserving.rs`,
  `.../register_allocation/structural_call_chain/`). The bounded
  reload-homes lane now witnesses a reload interval that survives an
  intervening call: under an explicit physical-view allowlist a `CallUnit`'s
  clobber set reduces the spill victim's common candidates to the single
  callee-saved view (rbx on x86-64, x19 on AArch64) on all five native
  targets, and independent replay rejects a caller-saved home, cleared
  candidate or coexisting-home rosters, a shrunken interval, and root or
  usage mismatches
  (`tests/native-differential/.../fixtures/call_spanning_reload.rs`,
  `.../register_allocation/reload_value_homes.rs`). The executable
  runtime-spill rewrite realizes a block's flexible victim uses through one
  shared reload register only while every instruction inside the open span
  writes no unit — a clobber or an implicit definition closes the pair, so
  no produced interval ever demands a cross-call home — when admission
  proves some allocatable view of the victim's class is free of
  function-wide implicit uses, pinned views, and reservations; ABI-pinned
  uses keep a private pair so the pin covers only the load-to-use window,
  and independent replay reconstructs the same grouping
  (`selected-instructions-to-selected-instructions/src/rewrites/runtime_spill.rs`,
  `.../runtime_spill/rewrite.rs`, `.../runtime_spill/validation.rs`). The
  sequenced native fixture drives that boundary on all five native
  targets: the victim's two uses straddle an intervening `CallUnit` and
  each opens a private frame-address/load pair whose window holds no call,
  while the fixture's own filler still spans the first call onto the
  callee-saved survivor (rbx on x86-64, x19 on AArch64); independent
  replay rejects a dropped pair, a post-call use rebound to the pre-call
  reload or to the victim, a bogus extra load, a non-canonical reload
  home, and a foreign legality root
  (`tests/native-differential/.../register_allocation/runtime_spill_call_spanning.rs`).
  Remaining: none of the stated legs are outstanding — every
  `NativeTarget` constructor is witnessed; Windows AArch64 has no
  `NativeTarget` constructor, so that ABI combination stays unwitnessable
  until the target vocabulary grows one.

## Machine optimization

- **DECLARATIVE-PEEPHOLES.** Generalize the landed symbolic instruction-pair
  descriptors (`selected_lowering/literal_fold/pair_rule.rs`) to physical
  register units, effects, traps, memory, stack, and control flow without
  replacing the independent validator. Landed: `PairResultDisposition`
  declares whether the rewritten instruction delivers its result through a
  scalar `Def` operand or implicit physical-unit condition-state
  definitions, and the producer admits constraint-row and consumer operand
  shapes through it (crate `nextest`: 157 pass; a `CompareI64` consumer
  carrying a scalar-result shape now rejects as `ConsumerMismatch` instead
  of indexing past the row). `PairUnitEffects` covers implicit-unit uses,
  clobbers, and operand unit bindings; `PairMachineEffects` covers the
  memory, trap, stack, control-flow, barrier, call, and cleanup surface;
  `PairOperandShape` now carries `BinaryLeftLiteral` — the commuted
  exact-add grammar folding the literal at operand 0 through the same
  `ExactAddI64Immediate` row — so the exact-add selection declares one pair
  per operand position and the recorded action binds the surviving register
  rather than a fixed operand (226 crate lib tests pass, including firing
  on both targets plus decision-field corruption and wrong-position
  negatives; the replay re-derives the grammar from the consumer kind and
  recorded operand position alone).
  `PairMachineEffects::IndexedPointerReadFold { index_operand }` declares
  the first non-isolated relationship — an indexed pointer read folded to
  an immediate-offset read — and `LOAD8_INDEXED_U12` folds a
  `MaterializeI64` index at operand 1 of `Load8Indexed` into `Load8`'s
  byte offset under `LiteralFoldPolicy::LOAD8_INDEXED_V1`, bounded at 4095
  by the aarch64 displacement field (crate `nextest`: 235 pass, including
  firing on both Linux targets, the unencodable-4096 boundary,
  decision-field substitution, and wrong-policy negatives; the validator
  restates the relationship independently as `indexed_read_fold_admission`
  instead of reusing the pair descriptor).
  `PairOperandShape::BinaryRightLiteralConstantResult` declares a
  constant-result grammar — the fold drops the surviving dividend `Use`
  and every dead scratch `Def` past the result, each proven dead in the
  function — and `PairUnitEffects::BoundEarlyClobberConsumerOperands`
  admits the fixed pins and early-clobber marks an `idiv`-class remainder
  realization declares while still rejecting tied operands;
  `WRAPPING_REMAINDER_ONE_MATERIALIZE` folds `MaterializeI64(1)` feeding
  `WrappingRemainderI64` into a `MaterializeI64` of zero at the result
  register under `LiteralFoldPolicy::WRAPPING_REMAINDER_V1`, discharging
  the consumer's architectural fault surface (334 crate lib tests pass,
  including firing on both Linux targets, scratch-custody,
  decision-field substitution, and wrong-policy negatives; the replay
  re-derives the divisor literal, result register, dropped-`Def` custody,
  and rebuilt materialization row independently).
  `PairOperandShape::BinaryLeftLiteralConstantResult` declares the
  left-operand constant-result grammar — the operand-0 literal alone
  fixes the result, so unlike `BinaryLeftLiteral` no commutation is
  attested — and `BITWISE_AND_ZERO_FOLDS` declares one pair per `Use`
  position, folding `MaterializeI64(0)` feeding `BitwiseAndI64` at either
  operand into a `MaterializeI64` of zero at the result register under
  `LiteralFoldPolicy::BITWISE_AND_ZERO_V1`, with `x & 0` and `0 & x` both
  annihilating to zero and the dropped non-victim `Use` plus every dead
  scratch `Def` falling under the same occurrence-free custody (364
  crate lib tests pass, including firing on both Linux targets at either
  operand position, exact-zero versus nonzero literals, forbidden
  operand bindings, scratch-custody, decision-field substitution, and
  wrong-policy negatives; the replay restates both grammars through its
  own `AndZero`/`AndZeroLeft` source shapes and never consults the pair
  descriptor).
  `BITWISE_XOR_ZERO_COPIES` declares one pair per `Use` position,
  folding `MaterializeI64(0)` feeding `BitwiseXorI64` at either operand
  into a `CopyI64` of the surviving `Use` at the result register under
  `LiteralFoldPolicy::BITWISE_XOR_ZERO_V1` — `x ^ 0` and `0 ^ x` are
  both `x` — with the left pair declaring `BinaryLeftLiteral`, the
  commutation attestation the surviving-operand binding requires, and
  both grammars rewriting through the `CopyI64` row the divide-identity
  family already binds (377 crate tests pass, including firing on both
  Linux targets at either operand position, exact-zero versus nonzero
  literals, wrong-position claims, forbidden operand bindings,
  scratch-`Def` rejection under the exact copy grammar, decision-field
  substitution, and wrong-policy negatives; the replay restates both
  grammars through its own `XorZero`/`XorZeroLeft` source shapes and
  never consults the pair descriptor).
  `WRAPPING_ADD_ZERO_COPIES` declares one pair per `Use` position,
  folding `MaterializeI64(0)` feeding `WrappingAddI64` at either operand
  into a `CopyI64` of the surviving `Use` at the result register under
  `LiteralFoldPolicy::WRAPPING_ADD_ZERO_V1` — `x + 0` and `0 + x` are
  both `x` modulo `2^64` — with the left pair declaring
  `BinaryLeftLiteral`, the commutation attestation the
  surviving-operand binding requires, and both grammars rewriting
  through the `CopyI64` row under an isolated unit and machine-effect
  surface (396 crate tests pass, including firing on both Linux targets
  at either operand position, exact-zero versus nonzero literals,
  wrong-position claims, forbidden operand bindings, scratch-`Def`
  rejection under the exact copy grammar, decision-field substitution,
  and wrong-policy negatives; the replay restates both grammars through
  its own `WrappingAddZero`/`WrappingAddZeroLeft` source shapes and
  never consults the pair descriptor).
  `BITWISE_AND_ONES_COPIES` declares one pair per `Use` position,
  folding `MaterializeI64(u64::MAX)` feeding `BitwiseAndI64` at either
  operand into a `CopyI64` of the surviving `Use` at the result
  register under `LiteralFoldPolicy::BITWISE_AND_ONES_V1` — `x & MAX`
  and `MAX & x` are both `x` — with the left pair declaring
  `BinaryLeftLiteral` and both grammars rewriting through the
  `CopyI64` row under an isolated unit and machine-effect surface. The
  family shares its consumer kind and operand positions with the
  and-zero annihilator rules, so pair admission now keys on the
  consumer kind, the victim operand position, and the literal's value:
  `AdmittedPairs::for_consumer` requires exactly one enabled pair to
  admit the recorded immediate and refuses an overlapping catalog
  outright, and admission reports a literal outside every enabled
  bound as `UnsupportedImmediate`, distinct from an unadmitted kind or
  position (446 crate tests pass, including firing on both Linux
  targets at either operand position, exact-all-ones versus zero, one,
  and near-bound literals, both-families-enabled value dispatch in each
  direction, wrong-position claims, forbidden operand bindings,
  scratch-`Def` rejection under the exact copy grammar, decision-field
  substitution, and wrong-policy negatives; the replay restates both
  grammars through its own `AndOnes`/`AndOnesLeft` source shapes,
  selects the family on the literal's value alone, and never consults
  the pair descriptor).
  `PairOperandShape::BinaryLeftLiteralConstantResultAuxiliaryUses`
  declares the constant-result grammar extended past the scalar `Def`
  result: the operand-0 literal alone fixes the result, the operand-1
  `Use` drops with the form, and every `Use` past the result drops only
  under provenance custody requiring each register to be defined in the
  function solely by `MaterializeI64` instructions producing
  `Unsigned(0)` — the provably-zero high-half input an x86-64 `div`
  realization reads. `EXACT_DIVIDE_ZERO_DIVIDEND_MATERIALIZE` folds
  `MaterializeI64(0)` feeding `ExactDivideU64` at operand 0 into a
  `MaterializeI64` of zero at the result register under
  `LiteralFoldPolicy::EXACT_DIVIDE_ZERO_V1` — `0 / x` is `0` for every
  `x` the kind's carried nonzero-divisor obligation admits — with
  `FaultDischargedByObligation` now covering the `ExactDivideU64` kind's
  recorded obligation and `BoundConsumerOperands` admitting the pinned
  form's `fixed_view` decorations while rejecting `tied_to` and
  `early_clobber` (502 crate lib tests pass, including firing on both
  Linux targets, nonzero-dividend, missing-obligation, auxiliary-custody,
  forbidden-binding, decision-field substitution, and wrong-policy
  negatives; the replay restates the grammar through its own
  `DivideZeroDividend` source shape — re-deriving the literal, result
  register, dropped-`Use` custody, and obligation custody from the
  instruction record — and never consults the pair descriptor).
  `PairMachineEffects::DeadConsumerUnitDefs` declares the first
  dead-implicit-definition relationship — the producer remains
  effect-isolated, the consumer may define implicit physical units the
  rewrite retires only when every such unit is dead across the whole
  function including terminator records, consumer clobbers narrow
  destruction freely, consumer implicit uses and any non-isolated
  rewritten form reject — and `SATURATING_ADD_ZERO_COPIES` declares one
  pair per `Use` position, folding `MaterializeI64(0)` feeding
  `SaturatingAdd(SaturatingCarrier::U64)` at either operand into a
  `CopyI64` of the surviving `Use` at the result register under
  `LiteralFoldPolicy::SATURATING_ADD_ZERO_V1` — `x +| 0` and `0 +| x`
  are both `x` — retiring aarch64's implicit `nzcv` definition under the
  deadness proof while x86-64's `rflags` clobber and early-clobber mark
  drop unconditionally with the replaced operand list (549 crate tests
  pass, including firing on both Linux targets at either operand
  position, live-`nzcv` rejection under a conditional-branch terminator,
  wrong-position claims, forbidden operand bindings, decision-field
  substitution, and wrong-policy negatives; the replay restates both
  grammars through its own `SaturatingAddZero`/`SaturatingAddZeroLeft`
  source shapes, runs its own dead-unit scan, and never consults the
  pair descriptor).
  `PairOperandShape::BinaryRightLiteralScratchDefs` and
  `BinaryLeftLiteralScratchDefs` declare the surviving-`Use` grammars
  extended past the scalar `Def` result: every operand past the result
  is a scratch `Def` the fold drops under occurrence-free custody —
  each dropped register must occur nowhere else in the function — and
  `SATURATING_ADD_ZERO_COPIES` now declares one pair per `Use` position
  per carrier, folding `MaterializeI64(0)` feeding `SaturatingAdd` on
  any of the eight carriers into a `CopyI64` of the surviving `Use`
  under `LiteralFoldPolicy::SATURATING_ADD_ZERO_V1`. The u64 pairs keep
  the exact three-operand grammar; every other carrier binds the
  clamped row whose bound scratch `Def` — early-clobber on both
  targets — the realization computes its saturation bound through, and
  the same `DeadConsumerUnitDefs` gate retires aarch64's `nzcv`
  definition while x86-64's `rflags` clobber drops unconditionally
  (595 crate tests pass, including firing on both Linux targets at
  either operand position of every clamped carrier, live-`nzcv`
  rejection under a conditional-branch terminator, cross-carrier
  kind-versus-row rejection, scratch-custody negatives — a tail `Use`
  or a register read or defined elsewhere — forbidden operand bindings,
  decision-field substitution, and wrong-policy negatives; the replay
  restates the clamped grammars through its own
  `SaturatingAddZeroScratch`/`SaturatingAddZeroLeftScratch` source
  shapes, re-derives each dropped `Def`'s custody itself, and never
  consults the pair descriptor).
  Remaining: further unit roles beyond retired implicit definitions,
  stack- and control-flow-carrying relationships, and trap relationships
  beyond the existing `FaultDischargedByLiteral` and
  `FaultDischargedByObligation` descriptors. Those fault-discharge
  variants do not admit arbitrary trap preservation or hosted-trap
  effects.

- **EXACT-MACHINE-SIMPLIFICATIONS.** Add copy removal, redundant extension
  removal, address folding, compare/test selection, and scheduling only where
  each transformation is independently verifiable. Existing narrow same-view
  and compare-adjacent cases do not imply general authority. Landed:
  `rewrites/redundant_extension` rewrites an extension whose input's unique
  producer already guarantees the normalized bits to `CopyI64` — the
  producer table spans the fixed normalizations and the partial carriers
  `ZeroExtendU32`, `LoadPacked`, and `Float32ToBits`, whose contracts fix
  nothing above their meaningful width and so witness only `ZeroExtendU32`,
  the one extension whose own result leaves those bits unmeaningful, and
  only when the meaningful width already fits in 32 bits; wider packed
  loads genuinely narrow rather than witness, and admission resolves the
  defining operand so a packed load's undocumented scratch shares none of
  the assembled result's guarantee — and `rewrites/literal_compare`
  rewrites a `CompareI64` whose right operand's
  unique producer is a `MaterializeI64` inside the shared twelve-bit
  unsigned immediate bound to `CompareI64Immediate` — or `CompareI64Zero`
  for a literal of zero — keeping the compare's identity, position, and
  published flag surface while retaining the materialization for other
  readers — and `rewrites/copy_removal` removes a `CopyI64` whose
  destination's only mentions are plain `Use` operands later in the same
  block, rebinding each to the source and dropping the destination roster
  row, admitting a source redefinition on the last use's own instruction
  but none inside the open interval, each under replayed
  restore-by-content validation — and `rewrites/address_fold` rebinds the
  base operand of a `Load8`/`Load16`/`Load32`/`Load64`, referent `Store`,
  or `AddressOffset` whose last in-block definition of it is an
  `AddressOffset` to that producer's own base, carrying the combined
  displacement at the scaled unsigned bound every target encoder shares
  while nothing inside the open interval redefines the base and the
  semantic access roster stays untouched (crate `nextest`: 354 pass)
  — and `rewrites/literal_minuend` rewrites a `CompareI64` whose left
  operand's unique producer is a `MaterializeI64` inside the same
  twelve-bit unsigned immediate bound to `CompareI64Immediate` — or
  `CompareI64Zero` for a literal of zero — which fixes the literal as the
  subtrahend and so computes the swapped subtraction: the zero condition
  survives but ordering predicates invert, so admission walks every
  condition-state unit the compare defines through successor edges until
  a redefinition or clobber ends it and admits only when every reached
  reader is `MaterializeBooleanEqual` or `ConditionalBranchNonZero`,
  under the same replayed restore-by-content validation (crate `nextest`:
  431 pass) — and `rewrites/local_schedule` interchanges two named
  body instructions in one selected block across the bounded window
  they span — the distance-one case is the adjacent pair — when no
  register or condition-state hazard runs between either member and
  any crossed position in either direction, no call, hosted effect,
  barrier kind, or call-roster entry sits anywhere in the window, no
  boundary settlement falls inside the window's span, and the
  validated memory roster accounts for every access a memory-capable
  member can reach — at most one window member may carry rows, so
  row-less members observe nothing and no recorded access changes
  order while an interior accounted access keeps its position between
  two memory-inert members — under the same replayed
  restore-by-content validation (crate `nextest`: 487 pass)
  — and `rewrites/local_relocation` relocates one named body
  instruction to a named destination's position in its block — the
  window between them rotates one slot toward the member's vacated
  index rather than trading two endpoints, so it admits windows the
  interchange refuses: a row-less member cannot observe memory and
  crosses any run of roster-carrying positions while every recorded
  access keeps its relative order, and a roster-carrying member
  crosses only row-less positions — when no register or
  condition-state hazard runs between the member and any crossed
  position in either direction, no call, hosted effect, barrier
  kind, or call-roster entry sits anywhere in the window, and no
  boundary settlement falls inside the window's span, under the
  same replayed restore-by-content validation (crate `nextest`:
  528 pass) — and `rewrites/run_relocation` relocates the
  contiguous run two named members bound in one body block to a
  named destination instruction's position as a single body — the
  window the move crosses rotates so the run lands on the
  destination's edge while every crossed position keeps its
  relative order, letting internally coupled members move together
  through a window neither could cross alone — when no register
  or condition-state hazard runs between any member and any
  crossed position, no call, hosted effect, barrier kind, or
  call-roster entry sits anywhere in the window, no boundary
  settlement falls inside the window's span, and the validated
  memory roster accounts for every access a memory-capable member
  can reach — a roster-carrying run crosses only row-less
  positions while a row-less run crosses any accounted mix —
  under the same replayed restore-by-content validation (crate
  `nextest`: 571 pass) — and `rewrites/edge_relocation`
  relocates one named body instruction across its block's unique
  semantic `Jump` edge onto a named position in the edge's
  sole-predecessor target — the member leaves its block's body,
  every crossed position keeps its order, and it lands on the
  destination's index while both terminators, the edge's
  transports, and every roster stay untouched — when no register
  or condition-state hazard runs between the member and any
  crossed position in either direction, the `Jump` terminator's
  own operand and implicit surface included, when no register
  transport would hand a binding a stale or overwritten value,
  when no call, hosted effect, barrier kind, or call-roster
  entry sits anywhere in the window, when the validated memory
  roster accounts for every access a memory-capable member can
  reach — a roster-carrying member crosses only row-less
  positions, the terminator and the edge's own access rows
  counting as accounted positions — and when no boundary
  settlement in either block would observe the member inside a
  changed executed prefix, under the same replayed
  restore-by-content validation (crate `nextest`: 592 pass)
  — and `rewrites/diamond_relocation` relocates one named
  body instruction out of a block ending in a two-successor
  conditional branch, across each distinct arm and its
  unconditional `Jump`, onto a named position in the one join
  block the arms alone feed — each arm a plain source block
  reached by that branch's edges alone, every edge into the
  join leaving an arm, so each traversal executes exactly one
  arm and the member keeps its execution count of one — when
  no register or condition-state hazard runs between the
  member and any crossed position, the branch terminator and
  arm `Jump`s included, when no crossed edge's register
  transports would hand a binding a stale or overwritten
  value, when no call, hosted effect, barrier kind, or
  call-roster entry sits inside the window, when the validated
  memory roster accounts for every access a memory-capable
  member can reach — a roster-carrying member crosses only
  row-less positions, each terminator and the edge-origin rows
  counting as accounted boundary positions — and when no
  boundary settlement past the member's index in its own block
  or past the landing index in the join would observe a
  changed executed prefix, under the same replayed
  restore-by-content validation (crate `nextest`: 636 pass)
  — and `rewrites/constant_boolean` rewrites a
  `MaterializeBooleanEqual`, `MaterializeBooleanU64LessThan`,
  `MaterializeBooleanI64LessThan`,
  `MaterializeBooleanU64LessOrEqual`, or
  `MaterializeBooleanI64LessOrEqual` whose every implicit
  condition-state use resolves to one compare — the last flag
  event on every path reaching the materialization, found
  in-block or by the least-fixpoint entry-event walk over the
  backward-reachable predecessor cone, where edge transports
  carry registers, storage payloads, case fields, and fuel but
  no condition-state units so edges pass flag state through
  unchanged — when that compare's operands are compile-time
  constant, replacing the reader with the target's own
  `MaterializeI64` of the decided predicate while the compare
  keeps publishing flag state for other readers and branch
  terminators — a clobber, a different definition, unknown
  entry state, an eventless path, or paths that disagree all
  refuse — under the same replayed restore-by-content
  validation (crate `nextest`: 642 pass)
  — and `rewrites/constant_branch` rewrites a
  `ConditionalBranch`, `ConditionalBranchU64LessThan`, or
  `ConditionalBranchI64LessThan` terminator whose implicit
  uses partition under the flag universe the target's three
  compare rows publish — flag units must resolve to one
  compare through the same least-fixpoint entry-event walk
  read at the terminator position, non-flag units must lie in
  the jump row's implicit surface, and the jump row must
  republish the branch's implicit definitions and clobbers
  exactly — when that compare's operands are compile-time
  constant, replacing the terminator with the target's own
  `Jump` carrying the decided `SelectedSuccessor` record
  verbatim while the compare keeps publishing flag state for
  other readers — a clobber, a different definition, unknown
  entry state, an eventless path, disagreeing paths, or a
  non-flag observation the jump surface cannot carry all
  refuse — under the same replayed restore-by-content
  validation (crate `nextest`: 685 pass). The flag walk and
  constant-operand audit both folds share now live in
  `rewrites/condition_state`.
  Remaining: scheduling past the proven bounded window
  interchange, single-member relocation, multi-member run
  relocation, the single-edge move, and the branch-diamond
  move — relocation through other converging or branching
  control flow — and compare/test selection past the landed
  literal folds and the constant-flag boolean materialization
  and conditional-branch folds.

## Proof-, ownership-, and state-aware optimization

- **ALIAS-AWARE-MEMORY.** Add borrow-aware load forwarding, dead-store
  elimination, and mutation motion on the selected CFG, each decided from
  the validated `memory_accesses` roster under place exclusivity and
  replayed by restore-by-content validation. Landed:
  `rewrites/load_forwarding` rewrites a `Load64`/`Load32`/`Load16`/`Load8`
  whose same-block writer is a `Store` with the read's exact same-width
  `WritePlace` row at the identical byte offset — `CopyI64` of the stored
  register at full width, the matching `ZeroExtend` at sub-word width,
  since a same-width store-then-load round-trips the register's low bits
  in the target's own byte order — and `rewrites/dead_store` removes an
  exact-width `Store` whose dead range a later place store rewrites
  unobserved — the first access on the dead place decides, and the
  covering store's single `WritePlace` row need only contain the dead
  range entirely, so a wider `Store` or a `StorePacked` kills a
  narrower or shifted dead store while partial overlap, a row that
  disagrees with its instruction's encoded range, and a packed store
  on the wrong constraint still reject; boundary settlements inside
  the dead interval reject while later positions shift one ordinal.
  Dead-store search also walks forward
  across edges when every outgoing edge of a crossed block names one
  block — each path forward then reaches the covering store before any
  observer — checking each crossed terminator's roster rows and each
  crossed edge's transports: joins at crossed blocks are harmless
  because coverage looks forward, while forks to distinct blocks,
  returns and hosted exits, re-entered blocks, and edge transports
  writing or retiring the dead place's storage end the walk unproven.
  `rewrites/store_motion` sinks a place `Store` carrying exactly one
  `WritePlace` row to just before the first position that must stay
  ordered after it — an interfering roster row on the moved place, a
  call or hosted effect, a carried pointer or value redefinition, an
  unaccounted memory-capable instruction, or a boundary settlement —
  and keeps walking forward across a terminator when every outgoing
  edge names one block that sees the crossed block as its only
  predecessor, checking each crossed terminator's roster rows and each
  edge's transports: forks to distinct blocks, joins into the
  successor, successors that can reach back to the walked chain, and
  transports redefining the carried registers or writing the moved
  place's storage land the store at the last proven position rather
  than rejecting. The roster names the store by instruction identity,
  so it stays unchanged; boundary settlements at or after a
  crossed-block landing shift one ordinal. Load forwarding also walks
  back across edges when the load's block has exactly one predecessor
  block — every
  path to the load then carries that block's writer — checking each
  crossed terminator's roster rows and each crossed edge's transports:
  edge-defined register parameters colliding with the carried value or
  the load's result reject, as do structural destinations, case custody
  slots, and custody discards touching the forwarded place's storage,
  while joins, unreachable blocks, self-loops, and the function entry
  end the walk unproven (crate `nextest`: 357 pass). Dead-store
  admission also accepts `StorePacked` as the removed store: the
  instruction's encoded byte offset and packed width define the dead
  range, its single `WritePlace` row must name that exact place and
  range, and the instruction must carry the target's declared
  `store_packed` constraint with its `[use pointer, use packed value,
  def scratch]` operand row — the scratch `Def` early-clobbered in the
  constraint — while the scratch virtual register must occur exactly
  once in the whole function across body and terminator operands,
  successor structural bindings, case payloads, and edge transports,
  so the removal cannot strand a surviving mention; the kill side is
  unchanged, with any later covering store — plain or packed —
  rewriting the dead range unobserved in-block or across a
  single-successor edge (crate `nextest`: 414 pass). Dead-store
  covering also admits a write into the dead place's own local
  storage — its `StructuralParameter` or `StructuralBlockParameter`
  slot, which is the place's storage under the same byte
  coordinates: a `Store64` naming that slot, or a place
  `Store`/`StorePacked` through its materialized address, each
  carrying exactly one `WriteLocal` row whose encoded range
  contains the dead range; a disjoint `WriteLocal` on any
  place-backed slot now walks past by range intersection since it
  cannot touch the dead bytes either way, while an operation-owned
  `Structural` slot — which can stage bytes that merely name the
  place, like a call's staged view descriptor — still interferes
  on intersection and never covers (crate `nextest`: 450 pass).
  Load forwarding also crosses joins: a block whose body shows no
  interfering access defers to every predecessor block, and it
  resolves once each predecessor path resolves to the same stored
  register — a store dominating the join decides all its legs, as
  does each leg's own last writer of that register — while the
  entry block, a block no edge reaches, legs whose writers store
  different registers, and deferred regions that never resolve,
  self-loops and writerless cycles included, still leave the pair
  unproven (crate `nextest`: 493 pass). Load-forwarding writers
  also reach the forwarded place's own local storage — its
  `StructuralParameter` or `StructuralBlockParameter` slot, which
  is the place's storage under the same byte coordinates: a
  `Store64` into that slot directly, or a place `Store` through
  the slot's materialized address, each sourcing the forward when
  its single `WriteLocal` row names the read's exact byte range;
  the slot store is always eight bytes, so only `Load64` pairs
  with it. A `WriteLocal` row now decides interference by range
  intersection the way `WritePlace` does, so a disjoint local
  write walks past whether its slot is the place's storage or
  only stages bytes naming the place, while an operation-owned
  `Structural` slot still interferes on intersection and never
  sources, a row disagreeing with its instruction's slot or
  encoded range rejects, and a `Store64` under a sub-word read
  rejects (crate `nextest`: 535 pass).
  Remaining: operation-slot and dynamic-extent writes still cannot
  cover or source — an operation-owned `Structural` slot records
  no storage-versus-staging role and a dynamic extent proves no
  fixed containment — and legs whose paths disagree or reach
  writerless cycles stay unproven.
- **REPRESENTATION-SPECIALIZATION.** Add field/variant relevance and
  invariant-window specialization.
- **CLEANUP-PRUNING.** Add cleanup and transition reachability pruning without
  losing affine/linear custody.
- **STATE-SPECIALIZATION.** Add state-argument/result specialization with exact
  edge provenance.
- **INTERPROCEDURAL-SUMMARIES.** Add service/call summaries and proof-bound
  inlining.
- **PROOF-DIRECTED-LOOPS.** Add loop-bound reasoning, induction
  simplification, and vectorization with exact lane semantics.

## Verification and rollout

- **PER-RULE-COVERAGE.** Finish positive, negative, boundary, disabled, budget,
  determinism, fixed-point/idempotence, and corruption coverage for every exact
  rule. Do not call repeated reconstruction idempotence when the published
  artifact is not a legal second input. The eight SelectedLowering literal-fold
  rules are now fully covered in
  `selected-instructions-to-selected-instructions` (326 lib tests pass on Linux
  x86-64): every rule carries positive, negative, disabled-policy, budget,
  determinism, and replay-corruption evidence; the five 12-bit-immediate rules
  (exact add, exact subtract, compare, indexed byte load, byte-view address)
  exercise both sides of the 4095/4096 boundary; and each fixed-point leg feeds
  the published `ValidatedLiteralFold` back through the real liveness, range,
  legality, victim-selection, and classification analyses as the legal second
  input. `CheckedTreeProductPruning` is now fully covered in
  `typed-trees-to-checked-trees` and the `assembled-syntax-to-checked-compilation`
  phase entrance (20 focused tests pass on macOS arm64): enabled pruning,
  disabled/unselected identity, wrong-phase rejection, duplicate/unknown/absent
  roots, empty-root interface-surface retention, boundary retention, transitive
  and duplicate dependency retention, determinism, root-order canonicalization,
  exact root-set identity binding, roster and root corruption rejected by
  independent product validation, and a fixed-point leg that feeds the
  published pruned product back through the same plan as a legal second input
  and observes an empty pruned roster; the phase has no budget axis. The
  three named memory-rewrite rules in
  `selected-instructions-to-selected-instructions` — `dead_store`,
  `load_forwarding`, and `store_motion` — are now fully covered (375 lib
  tests pass on Linux x86-64): each carries same-block and cross-block
  positive, negative, and boundary legs; deterministic repeated runs;
  same-block and cross-block replay-corruption rejection; exact measured
  validation-step boundaries that admit at the measured count and reject one
  step below on both the proposal and independent-replay paths; and
  fixed-point legs that feed the published validated artifact itself back as
  the legal second input and observe the terminal rejection on both the
  transformed instruction and the covering store. The three rules have no
  disabled-policy axis: they are not `Optimization` selection-vocabulary
  members, so admission is an explicit per-instruction validated call.
  `address_fold` and the two allocation-recovery selection members —
  `SharedEntryFixedViewCopyAfterCompareBeforeBranchV1` and
  `ActiveResidentImmediateU64MultiUseRematerializationV1` — now carry the
  same matrix (392 lib tests pass on macOS arm64). The address fold gains
  the measured validation-step boundary at two block sizes on both the
  proposal and independent-replay paths, replay-corruption rejection for
  drift in blocks the fold never touched, and a producer-site terminal
  leg; it has no disabled-policy axis for the same reason. The
  allocation-recovery entrance pins the empty-phase decline shared by both
  rules. The shared-entry copy carries determinism at the build core, the
  exact two-boundary admission window — an empty boundary set admits no
  copy while one or three refuse — and transformed-function re-admission
  refusal on both the build and replay cores, whose second input is a
  legal raw `SelectedFunction`; its measured staged budget boundary and
  disabled legs live in the native-differential `fixed_view_copy_operational`
  suite. The multiple-use rematerialization carries determinism and
  terminal re-admission at the rule core — the transformed plan's
  rewritten uses cannot admit a second application — with staged budget,
  disabled, corruption, and rebuilt-analysis fixed-point legs in the
  native-differential suite. The phase's four remaining per-instruction
  exact rules — `copy_removal`, `literal_compare`,
  `redundant_extension`, and `runtime_rematerialization` — now carry the
  same matrix (400 lib tests pass on macOS arm64): each admits at its
  measured validation-step count and rejects one step below on both the
  proposal and independent-replay paths at two fixture sizes — an
  edge-transport successor for `copy_removal`, a widened instruction
  scan for `literal_compare` and `redundant_extension`, and a dominated
  successor block for `runtime_rematerialization` — and each rejects
  replay drift in blocks the rewrite never touched through the
  restore-by-content check. `copy_removal` also gains its determinism
  and fixed-point legs: the published artifact is a legal second input
  through the sealed analysis boundary, re-admission at the removed
  copy's site refuses, and a surviving chained copy whose destination
  nobody reads is dead code the rule declines. Like the memory rules,
  none of the four is an `Optimization` selection-vocabulary member, so
  the disabled-policy axis stays absent. The phase's last two
  per-instruction rules — `literal_minuend` and `local_schedule` — now
  carry the same matrix (480 lib tests pass on Linux x86-64).
  `literal_minuend` already held the positive, negative, boundary,
  corruption, determinism, and fixed-point legs — the published
  `ValidatedLiteralMinuend` feeds back as a legal second input and the
  folded compare's site refuses re-admission — and its measured
  validation-step boundary now admits at the exact count and rejects
  one step below on both the proposal and independent-replay paths at a
  second fixture size whose flag audit crosses an edge into the
  consumer's block. `local_schedule` gains the measured boundary at
  three fixture sizes — the single-block scan, a two-block scan with
  the pair in the later block, and a roster-carrying load member
  growing the member-surface and roster terms — admitting at the exact
  count and rejecting one step below on both paths; determinism across
  repeated runs; and replay-corruption rejection for drift in a block
  the interchange never touched. Its fixed-point leg is an involution:
  the published `ValidatedLocalSchedule` is a legal second input
  through the sealed analysis boundary, the pair's stale order refuses,
  and the flipped order interchanges back to restore the source
  bit-identically while a hazard-coupled pair still declines and a
  different independent pair still admits. Neither rule is an
  `Optimization` selection-vocabulary member, so the disabled-policy
  axis stays absent. All six
  `lowered-psi-to-lowered-psi` rules — `ControlFlowCleanup`,
  `SparseConditionalConstantPropagation`, `CopyPropagation`,
  `GlobalValueNumbering`, `DeadPureScalarElimination`, and
  `ProofCheckElision` — now carry the same matrix through the public
  `run_psi_optimization` entrance (117 tests pass on macOS arm64): each
  is a `PsiOptimization` selection member, so the disabled-policy axis
  is a sibling-selection identity leg on the rule's own workload; the
  validators carry no step budget, so the phase has no measured-budget
  axis; every fixed-point leg feeds the published `LoweredPsi` back
  through the entrance as a legal second input and observes the
  recorded identity; and replay corruption is the independent
  `terminal_verifier::validate_*` check rejecting forged `after`
  modules — non-copy or non-total removals, survivors with drifted
  contents, parameter and edge-argument drift, missing dominating
  equivalents, structural change — plus malformed carriers refused at
  the stage's module-validation gate. Both proof-freeze triggers are
  covered: a contract `ensures` clause and an operation-site obligation
  each freeze the whole closure with an identity record.
  `resolved-layout-to-resolved-layout`'s sole exact rule,
  `X86RelaxConditionalBranchesToRel8V1` (`x86_branch_relaxation`), now
  carries the same matrix (17 lib tests and 13 native-differential stage
  tests pass on macOS arm64): it is the sole `FunctionRelativeLayout`
  `Optimization` selection member, so the disabled leg is the empty phase
  projection retaining the baseline layout by shared `Arc`; positive legs
  cover all three conditional predicates with their `jb`/`jl` opcode
  choices and jump re-encoding across a shrink; boundary legs cover the
  +127/-128 reachable and +128/-129 refused displacements on both the
  production and replay inspectors; structural legs cover absent taken
  blocks, non-adjacent fallthroughs, drifted recorded effects, and
  malformed short opcodes; the measured five-axis budget admits at the
  exact usage and refuses one below on each axis, and the
  independent-replay admission path both honors the measured budget and
  rejects a forged recorded budget; determinism holds across independent
  stagings and repeated phase executions; and corruption legs reject
  reauthenticated action bytes, attempt-roster outcomes, retained-layout
  drift outside the rewrite, foreign evidence substitution, and every
  receipt, manifest, and exit-contract field. Its fixed-point leg is the
  recorded terminal no-change sweep that re-declines every conditional
  branch on the final layout — the relaxed published layout is
  intentionally not a legal second input because baseline admission plans
  the six-byte branch rows the rewrite replaced — while a change-free
  output admits as a fresh baseline and re-stages to no actions.
  All six `abstract-operations-to-abstract-operations` selection members
  — `SparseConditionalConstantPropagation`, `ControlFlowCleanup`,
  `CopyPropagation`, `GlobalValueNumbering`, `ProofCheckElision`, and
  `DeadPureScalarElimination` — now carry the same matrix through the
  public `run_psi_pipeline`, `publish_optimization_run`, and
  `optimize_abstract_operations` entrances (365 lib tests pass on macOS
  arm64): each positive leg runs a verified Terminal-Psi artifact
  admitted through the real `lower_artifact_for_optimization` boundary
  and commits through the selection's own rule; negative legs decline
  the empty workload; boundary legs decline near-miss workloads — a
  conditional join with distinct returns for control-flow cleanup, a
  merge parameter with a distinct false-arm source for copy
  propagation, an add over unknown parameters for sparse conditional
  constant propagation and global value numbering, a certified
  obligation no elision rule covers for proof-check elision, and
  all-live scalar work for dead scalar elimination; every member is a
  `PsiOptimization` selection, so disabled legs are sibling-selection
  identities on the member's own positive workload; the measured
  five-axis `OptimizationWorkBudget` admits at the exact recorded usage
  and refuses one step below on every axis; determinism legs compare
  independent runs and publications across commits, usage, decisions,
  manifests, ledgers, and identity bundles; fixed-point legs re-admit
  the published run's transformed unit through
  `VerifiedPsiOptimizationSession::from_transformed` — the legal second
  input — and re-run the selected registry to a terminal decline;
  corruption legs reject drifted phase projections, forged commit
  outputs, foreign sessions, ledgers, manifests, decision logs, and
  usage records through the independent publication replay, plus a
  foreign complete-selection projection refused at the phase entrance;
  and malformed carriers fail closed at the external-decision decode
  and artifact admission boundaries.
  The phase's `runtime_spill` exact rule — including this wave's shared
  call-spanning reloads — now carries the same matrix (484 lib tests pass
  on Linux x86-64): positive legs cover instruction-result and
  edge-initialized parameter victims across body, terminator, binding, and
  case-payload uses on all four targets, with a dedicated leg showing one
  shared reload interval spanning an intervening `CallUnit`; negative legs
  cover address and non-GPR-width values, undominated and bypassed uses,
  inconsistent transport declarations, and incomplete edge arrivals; the
  measured validation-step boundary admits at the exact count and rejects
  one step below on both the proposal and independent-replay paths at
  three fixture sizes exercising the plan-scan, use, definition, and
  block terms; the shared/private boundary keeps every flexible use on a
  private pair when no view of the victim's class survives the block's
  effects; determinism holds across repeated runs; replay corruption
  rejects drift in storage, stream, terminator, settlement, roster, and
  slot content, including a forged extra pair inside the shared shape;
  and the fixed-point leg feeds the published `ValidatedRuntimeSpill` back
  through the sealed analysis boundary — the real liveness and live-range
  analyses accept it, re-admission of the same victim refuses on its
  existing slot, the produced `SpillAddress` register stays outside
  admission, and the shared reload register admits a second independently
  validated spill whose receipt chains the first artifact's identity.
  Like the memory rules, `runtime_spill` is not an `Optimization`
  selection-vocabulary member — admission is an explicit per-victim
  validated call — so the disabled-policy axis stays absent.
  Every exact rule in the current phase set now carries the full matrix,
  re-verified on Linux x86-64: 117 `lowered-psi-to-lowered-psi` tests,
  502 `selected-instructions-to-selected-instructions` lib tests, 378
  `abstract-operations-to-abstract-operations` lib tests, 17
  `resolved-layout-to-resolved-layout` tests, and the 20 checked-tree
  product-pruning legs all pass. No uncovered exact-rule family remains:
  the target-operations and pre-allocation selections are empty identity
  boundaries, and the retired post-allocation machine spellings reject
  rather than execute. Remaining: the same matrix for exact rules as
  they land in other phases.

- **BENCHMARKS.** Publish versioned compile-time, peak-memory, code-size, and
  runtime benchmarks keyed by exact rule selection and target.
