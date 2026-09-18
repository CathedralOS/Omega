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
  canonical decoding. Landed: the Terminal Trace V1 observation
  profile
  (`terminal-codec/tests/artifact/trace_profile_custody.rs`,
  `terminal_trace_v1_profile_rejects_every_one_field_substitution`) —
  the module commitment's program fingerprint, the root row's entry
  machine and its scalar, structural, and result schema fields, and
  each crash-site, boundary crash-site, and ordinary-event row's
  machine, block, and edge or operation coordinate, crash cause and
  route bucket, boundary identity, event kind, scalar and structural
  argument schemas, result schema, and every structural type,
  multiplicity, access, direct and projected qualification, path
  segment, and comparison field are independently representable and
  rejected by module-bound replay, as is a foreign module on the
  replay side of the join; the domain, schema, and vocabulary
  markers, zero module and row identities, unknown row, event, and
  enum tags, invalid UTF-8 and boolean encodings, roster over- and
  under-counts, row and route-alternative orderings, qualification
  and path-segment orderings, and trailing or truncated bytes are
  rejected at canonical encoding or decoding. Landed: installed
  external-root admission custody
  (`external-roots/src/tests/root_admission_custody.rs`,
  `installed_root_admission_rejects_every_one_field_substitution`) —
  every `RootAdmission` field is mutated independently and rejected by
  the installed-root ledger's install replay: the copied root evidence,
  root report identity, execution identity and report fingerprint,
  installed-code identity, receipt context, artifact, slot, owner, and
  the trust-receipt roster bind against the retained arguments; the
  retained provider-execution evidence is replayed against the exact
  validated root through `matches_root`, its compact report identity is
  honestly recomputed, and its opaque exit assurance revalidates, so
  evidence-internal plan, identity, fingerprint, and foreign-root
  substitutions reject; the record's reportable provider-plan,
  exit-assurance, and exit-assurance-fingerprint copies must equal the
  retained evidence's values; coupled honest recomputations, wholesale
  foreign executions, duplicate root identities, and occupied slots
  reject; the admission's own minted identity is adopted verbatim and
  still moves the ledger's containing fingerprint honestly; admission
  construction is the family's encoding leg and rejects an execution
  minted for another root.

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

- **FRAME-LAYOUT.** Extend exact nonzero-frame realization beyond the landed
  CFG families: red-zone policy, probing, unwind
  information, stable-address loans, and dynamic-allocation constraints.
  General calls need target-owned frame, callee-save, link-register, and
  call-site alignment plans. Witnessed: the ordinary three-block/two-return
  fixture replays through frame application, relocation-free object
  construction, and validated ordinary callable publication on x86-64 and
  AArch64, and the cyclic loop-carried runtime-spill fixture (six emitted
  blocks, backward branch, nonzero local spill storage) replays the same
  boundary on all five admitted targets
  (`runtime_spill_pressure::loop_carried_spill_frame_replays_private_accesses_through_callable_publication`).
  A scalar caller passing one argument past each target's register capacity
  now replays its exact outgoing-ABI-area write and the callee's exact
  incoming-activation read against the validated frame geometry through the
  same publication boundary on all four targets
  (`register_arity::stack_argument_calls_replay_frame_accesses_through_callable_publication`).
  Probing is landed: a caller whose outgoing ABI area exceeds one
  stack-commit granule commits its frame through an exact per-granule
  touch roster recorded in the validated layout and replays through
  ordinary callable publication on all five admitted targets
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
  rather than on frame-layout work itself. The coverage remainder is
  landed: the register_arity and general_cfg_fixed_frame rosters
  replay on all five admitted targets, and the realization stage's
  remaining frame-policy rosters — ordinary-callable ABI replay,
  fixed-frame custody/substitution and foreign-machine rejection, and
  the exit-contract mutation replay — now run on every admitted
  target as well
  (`callable_entry`, `fixed_frame_callable_entry`, `exit_replay`).

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
  edge provenance.
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
