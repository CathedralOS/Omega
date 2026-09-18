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
  Landed: runtime-spill recovery composes after active-resident
  rematerialization on residual `NoCompatibleHome` — the declared route
  proves a pressure prefix (choices, classifications, the rewrite, and
  rebuilt liveness/ranges/legality), assigns homes over the rebuilt facts,
  and hands the identical prefix to executable spill recovery when
  assignment still has no home, keeping `PressureRematerialization` first
  in the post-allocation manifest and binding the published allocation to
  `ActiveResidentImmediateU64MultiUseRematerializationV1` on x86-64 and
  AArch64
  (`runtime_spill_composition::residual_active_resident_pressure_composes_runtime_spill_after_rematerialization`).

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
  `rewrites/condition_state`. Also landed:
  `rewrites/fork_relocation` sinks one named body
  instruction out of its branching block through the one
  plain branch edge the pair selects onto a named position
  in that arm's body — the member becomes conditional on
  the edge, and a forward dead-path fixpoint proves every
  location it writes unread until rewritten on each path
  the move removes — and `rewrites/join_relocation`
  hoists one named body instruction out of a converging
  join back through the branch diamond that feeds it onto
  a named position in the one fork head the arms descend
  from — the reverse burden being total supply rather
  than partial death: every edge into the join leaves an
  arm the head alone feeds and every edge the head names
  reaches an arm, so each traversal into the join crossed
  the member's new position and each traversal of the
  head reaches the join — under the same replayed
  restore-by-content validation (crate `nextest`: 734
  pass) — and `rewrites/arm_relocation` hoists one
  named body instruction out of a conditional branch
  arm back into the one fork head whose edges alone
  reach it — the member becomes unconditional and so
  speculates onto every traversal leaving the head's
  other edges, which reverses the sink family's burden:
  only pure register and condition-state work may rise
  (no barrier kind, call roster, rostered or unaccounted
  memory access, or potentially-faulting kind may newly
  run where it never ran), and a forward dead-path
  fixpoint proves every location the member writes
  unread until rewritten on each path the execution is
  new on, with the member's own new position republishing
  foreign rather than familiar values on those paths —
  under the same replayed restore-by-content validation
  (crate `nextest`: 797 pass) — and
  `rewrites/bypass_relocation` sinks one named body
  instruction out of a block ending in a two-successor
  conditional branch, across the bypassed triangle it
  heads, onto a named position in the one join the branch
  itself names on at least one edge — every other distinct
  edge target a plain source arm the branch alone reaches
  that ends in a plain `Jump` back to the join and every
  edge into the join leaving the head or an arm, so each
  traversal of the head reaches the join exactly once
  whether it bypassed the arm or ran it and the member
  keeps its execution count of one — when no register or
  condition-state hazard runs between the member and any
  crossed position — the head tail, the branch terminator,
  both branch edges, the arm's body, `Jump`, and edge, and
  the join's prefix — when no crossed edge's register
  transports would hand a binding a stale or overwritten
  value, when no call, hosted effect, barrier kind, or
  call-roster entry sits inside the window, when the
  validated memory roster accounts for every access a
  memory-capable member can reach, and when no boundary
  settlement past the member's index in the head or the
  landing index in the join would observe a changed
  executed prefix, under the same replayed
  restore-by-content validation (crate `nextest`: 865
  pass). Also landed without a board record:
  `rewrites/predecessor_relocation` hoists one named
  body instruction into its block's sole predecessor,
  `rewrites/confluence_relocation` sinks one named body
  instruction through its block's lone `Jump` into a
  multi-inflow join under a forward dead-path proof on
  the shared continuations, and
  `rewrites/triangle_relocation` hoists one named body
  instruction out of a converging join back through the
  bypassed triangle onto the fork head — and
  `rewrites/run_interchange` interchanges two disjoint
  runs of body instructions in one selected block — each
  the contiguous span its named first and last members
  bound, of at least two members — while the interior
  between them keeps its relative order shifted by the
  length difference: every member meets the schedulable
  bar and trades order only with positions outside its
  own run inside the window, so a roster-carrying run
  crosses only row-less positions while roster-carrying
  members inside one run keep their recorded order, no
  call, hosted effect, barrier kind, or call-roster
  entry sits anywhere in the window, and no boundary
  settlement inside the window's span observes a changed
  executed prefix, under the same replayed
  restore-by-content validation (crate `nextest`: 999
  pass). Also landed earlier without a board record:
  `rewrites/member_run_interchange` interchanges one
  named member against the contiguous run two named
  members bound inside one block — and
  `rewrites/commuting_interchange` interchanges two
  named body instructions in one selected block — the
  pair interchange's own geometry — when every roster
  row that newly trades order commutes with every row
  of the position it crosses: two non-writing rows
  always commute, and a writer commutes only when the
  rows reach provably disjoint bytes — distinct places,
  distinct slots, a place against a slot that is not
  its storage or any outgoing slot, or disjoint fixed
  extents of shared storage — so the memory roster
  itself follows the new execution order, the window's
  rows rewritten in place, under the same replayed
  restore-by-content validation that re-derives the
  permutation from the source rather than the
  proposal's own grouping (crate `nextest`: 1105
  pass). Also landed: `rewrites/commuting_run_interchange`
  interchanges two disjoint runs of body instructions
  in one selected block — the run interchange's own
  geometry, each run the contiguous span its named
  first and last members bound, of at least two
  members — when every roster row that newly trades
  order commutes with every row of the position it
  crosses, so the memory roster itself follows the new
  execution order, the window's rows rewritten in
  place. The commutation audit the commuting families
  share now lives in `rewrites/commuting_accesses`.
  A window whose trading pairs carry no rowed-vs-rowed
  pair stays with the plain run interchange, and a
  one-member run stays with the pair and
  member-against-run families, under the same replayed
  restore-by-content validation (crate `nextest`: 1117
  pass). Also landed:
  `rewrites/commuting_member_run_interchange`
  interchanges one named body instruction against the
  contiguous run two named members bound in one block —
  the member-against-run interchange's own geometry,
  the member strictly on one side of the run's span of
  at least two members — when every roster row that
  newly trades order commutes with every row of the
  position it crosses, so the memory roster itself
  follows the new execution order, the window's rows
  rewritten in place. A window whose trading pairs
  carry no rowed-vs-rowed pair stays with the plain
  member-against-run interchange, and a one-member run
  stays with the commuting pair, under the same
  replayed restore-by-content validation (crate
  `nextest`: 1132 pass). Also landed:
  `rewrites/commuting_run_relocation` relocates the
  contiguous run two named members bound in one block
  onto a named destination instruction's position —
  the run relocation's own geometry, the window
  rotating one run-width toward the vacated span with
  every crossed position keeping its relative order —
  when every roster row that newly trades order
  commutes with every row of the position it crosses,
  so the memory roster itself follows the new
  execution order, the window's rows rewritten in
  place. A window whose trading pairs carry no
  rowed-vs-rowed pair stays with the plain run
  relocation, and a one-member run stays with the
  commuting member relocation, under the same replayed
  restore-by-content validation (crate `nextest`: 1152
  pass). Also landed: `rewrites/boundary_branch`
  rewrites a `ConditionalBranchU64LessThan` or
  `ConditionalBranchI64LessThan` terminator whose
  implicit uses partition under the flag universe the
  target's three compare rows publish — flag units
  must resolve to one compare through the same
  least-fixpoint entry-event walk read at the
  terminator position and must be among that
  compare's published definitions, and every other
  observed unit must lie in the jump row's implicit
  surface when the fold lands on `Jump` — when that
  compare carries one operand at its carrier
  domain's pole while the other side stays
  unresolved: a far pole (`x < 0` unsigned, `x <
  i64::MIN` signed, or a strict less-than issued
  from the domain maximum) or two known operands
  decides the predicate outright and the terminator
  becomes the target's own `Jump` carrying the
  decided `SelectedSuccessor` record verbatim, while
  a near pole (`0 < x` unsigned, `x < u64::MAX`,
  `i64::MIN < x`, `x < i64::MAX` signed) collapses
  the ordering to the nonzero condition on the
  identical published flag state — the terminator
  becomes `ConditionalBranch` carrying
  `ConditionalBranchNonZero` on the one constraint
  row the conditional-branch kinds share, the
  branch's instruction identity, implicit surface,
  and provenance retained and `when_less`
  republished as `when_nonzero` — admitted only
  when every flag-universe unit, not only the ones
  the source branch declared, resolves to that
  compare, since the collapsed reader can observe
  any unit its kind's encoding implies. A
  `ConditionalBranch`/`ConditionalBranchNonZero`
  pair reads the equality condition no single pole
  decides, and the identity
  `register - register` compare stays with the
  constant family; a clobber, a different
  definition, unknown entry state, an eventless
  path, disagreeing paths, or a non-flag
  observation the jump surface cannot carry all
  refuse — under the same replayed
  restore-by-content validation (crate `nextest`:
  1171 pass).
  Remaining: scheduling past the proven bounded window,
  run, member-against-run, commuting-pair,
  commuting-run, and commuting-member-against-run
  interchanges and the commuting member and run
  relocations — relocation through further converging
  or branching control flow, and compare/test
  selection past the landed literal folds, the
  constant-flag boolean materialization and
  conditional-branch folds, and the boundary-pole
  branch folds.

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
  re-verified on Linux x86-64: 142 `lowered-psi-to-lowered-psi` tests —
  the six selection rules plus the shared `retained_identities` family
  (proposition-carried proof values, machine crash routes, operation
  crash continuations, crash-site guards, recorded source-call joins,
  and ranked-scc coverage), which carries the same matrix through
  helper, consumer, and public-entrance legs — 1392
  `selected-instructions-to-selected-instructions` lib tests, 413
  `abstract-operations-to-abstract-operations` lib tests, 17
  `resolved-layout-to-resolved-layout` tests, and the 20 checked-tree
  product-pruning legs all pass. No uncovered exact-rule family remains:
  the target-operations and pre-allocation selections are empty identity
  boundaries, and the retired post-allocation machine spellings reject
  rather than execute. Remaining: the same matrix for exact rules as
  they land in other phases.

- **BENCHMARKS.** Publish versioned compile-time, peak-memory, code-size, and
  runtime benchmarks keyed by exact rule selection and target.
