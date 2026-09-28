# Tasks

Unfinished product and language work for the Rust reference compiler. The
[completion plan](wiki/drafts/reference/rust_compiler_completion.md) defines acceptance;
this board identifies remaining work, not a passing baseline.

| Start here | Purpose |
| --- | --- |
| [Pipeline route](#pipeline-route) | One Psi pass over every target's bodies, Build once, providers behind Terminal Psi, one driver. |
| [Immediate product closure](#immediate-product-closure) | Application, sample and canary outcomes and their native-execution blockers. |
| [Reflection](#semantic-reflection) / [embedding](#embedding-and-interpreted-components) / [tests](#requirement-based-tests) | Accepted language and library capabilities awaiting connected implementation. |
| [P1–P5](#p1---authority-roots-and-entry) | Entry/storage, materialization, portable evidence, ABI and Cathedral customers. |
| [Parallel language work](#parallel-language-and-compiler-lanes) | Other accepted language capabilities and their concrete dependencies. |
| [Rust release closure](#rust-compiler-release-closure) | Full-gate evidence and matching-host runs, separate from feature implementation. |
| [Optimizer board](TASKS_OPTIMIZER.md) / [bootstrap board](TASKS_BOOTSTRAP.md) | Separate execution owners; link rather than duplicate their work. |

Keep missing behavior, owning code, necessary dependencies and acceptance.
Current evidence replaces stale diagnoses; a rerun confirming the same result
does not earn another paragraph. Delete completed work and duplicate aliases;
Git preserves history. Keep a revision-bound failure only when it changes
where to resume, and prefer stable symbols/tests over line numbers.

Compiler behavior is exercised through `tests/omega` fixtures and
`tools/corpus_gate.py`. The old `canary_suite`, `samples_compile` and most
per-feature compiler targets were deleted; the current Cargo manifest still
registers `callback_terminal_custody`. Check the manifest and test body before
using a Rust test name as acceptance. When replacing a deleted target, retain
its source customer and required corpus/native outcome; a similarly named
fixture or a checked-only result does not establish native execution.

Run `python3 tools/board_paths.py` to identify stale source citations;
`--hints` suggests matching basenames and `--apply-moves` handles recognized
moves. The checker understands relative and brace-alternative citations and
exits nonzero while dead paths remain. A missing anchor needs its surrounding
claim audited against current code, not merely a path substitution.

[AGENTS.md](AGENTS.md) governs task provenance, scoped checks, claims,
strategy pauses and publication. Live assignments belong in the
[claims registry](tools/claims.md), not the board. A paused strategy is not a
blocked language feature. Genuine owner decisions belong in
[OWNER_QUESTIONS.md](OWNER_QUESTIONS.md); implementation difficulty alone does not.

## Ownership firewall

Follow the [connected pipeline](omega-rust/pipeline.md): Psi owns source semantics
through Terminal Psi; Omega owns target realization; backends own physical
details; Cathedral owns OS policy and structures. Complete ordinary operations
and transfers on the common graph, not another source-shape recognizer.

Compiler verification does not require an accepted package lock, and unfinished
native realization does not block source-package installation. Package acceptance,
build authority, and artifact verification remain separate obligations.

## Pipeline route

The compiler is one driver feeding each stage into the next
([pipeline.md](omega-rust/pipeline.md#one-driver-one-pass-per-target-realization),
[multi-target compilation](wiki/spec/build/configuration.md#multi-target-compilation)).
Owner decision 2026-09-24: targets are data; Psi checks every target's bodies
once per compilation; the Build evaluates once; `--target` narrows realization
only. Owner direction 2026-09-25: reach the right architecture first; a
structural move may regress fixtures under the AGENTS.md `unimplemented:` rule
(corpus goldens re-recorded in the same commit, never an unsound fail-fixture
acceptance, crash or changed native exit). The items below land in the listed
order, one commit or a few per item, each through `tools/landing.py`; each
deletes the side doors it replaces. Validate with `tools/corpus_gate.py
--filter` over the affected groups plus `--native --filter` for anything
backend-visible; full corpus runs only at the end of an item.

- **MEMBER-POLICY-REMOVES-COMPOSITE-ROWS.** Complete whole-array and indexed-row
  assignment for arithmetic-policy elements and structural elements. Customers:
  `samples/cli/collections/matrix_multiply` assigns nested Wrapping arrays;
  `samples/cli/rendering/dungeon_render` assigns an array of payload-free
  `Cell` cases. Preserve plain-record arrays too. The reduced fixture,
  `tests/omega/pending/collections/element_policy_array_literal_store`, contains
  only the flat Wrapping case; its other policy, record and row controls are
  comments, not executable coverage.

  In stage 04 `execution/terminal_unit/structural_scalar_store/primitive.rs`,
  `array_literal_stores` uses `closed_array_store_type` and requires a field
  destination, excluding indexed rows separately. The shared
  `checking/src/value_custody/literals/constant_arrays.rs` predicate accepts
  bare primitive leaves; `closed_array_store_type` peels whole-array policies
  only. Keep that shared contract. The statement sequence already has constructed
  structural-value replacement through `borrowed_windows.rs`, and the shape
  collector admits policy-bearing array storage. Trace the failing assignment
  through those owners; neither globally widening the primitive predicate nor
  adding another aggregate representation is the repair.
  `types/arithmetic_policy_array_tests.rs` checks shape, not assignment execution.

  Acceptance: executable controls for bare/whole-array-policy/element-policy
  arrays, Wrapping and Saturating, nested arrays, records and cases, whole and
  indexed-row stores, alongside working element stores. Retain exact source
  types, bounds, qualifications, custody and RHS evaluation before replacement.
  Promote the reduced fixture into the ordinary corpus with its Build entry and
  exit-0 oracle. Both unchanged samples must execute: matrix exit 189; dungeon
  renders `######`, `#.+..#`, `#.~..#`, `######` and exits 0 after input.
  `tools/sample_gate.py --filter matrix_multiply,dungeon_render` checks only;
  native execution and the rendered rows are the acceptance.

- **REFERENCE-LOCAL-BINDING-ROUTES.** (new-scope) Establish reference-valued
  locals from every admitted producer, then preserve their loan identity,
  backing and access through reads, calls and explicit state transfers.
  Governed by [ownership](wiki/spec/language/ownership.md#borrows-and-aliases)
  and [loan formation/replay](wiki/spec/terminal-psi/loans.md). Owners:
  `execution/terminal_unit/control/statement_sequence/local_data.rs`,
  `terminal_unit/receiver_aliases/`, and stage 05 argument evaluation and
  `expression_preparation/bindings`.

  The existing direct-place exclusive-loan route supplies a control:
  `borrow/runtime_exclusive_loan_local_write_exit` writes through `&mut self.raw`,
  reads the original storage afterwards and has recorded native exit 70.
  Remaining producer cases are shared borrows, runtime-indexed places, reference
  recasts, plain-local borrows, and reference-returning calls. The pending
  fixtures `borrows/reference_local_binding_routes` and
  `borrows/reference_result_call_binding` exercise shared binding and a returned
  reference; their comments record the wider reduction matrix, not executable
  coverage of every row. Preserve ordinary scalar-result and reference-argument
  calls as controls. `dependent/runtime_sibling_len_index_exit` exercises the
  library returned-view form `let view: &[u8] = self.buf.as_slice()`.

  `samples/cli/systems/descriptor_walk` combines three independent triggers in
  `let d: &Desc = &self.buf[off] as &Desc`: shared access, runtime selection and
  recast. Test each separately and the unchanged sample together; coordinate
  **RECAST-SOURCE-POSITIONS** rather than declaring success after one trigger.
  Omissions vary with surrounding control flow; phase labels do not define
  separate capabilities.

  `receiver_aliases` admits Mutable/WriteOnly access and checks exclusive parent
  lineage; shared loans cannot gain mutation authority by widening that match.
  Establish their exact local carrier and projection in stage 05's
  `structural_locals` relation. `shared_structural_place` currently requires an
  empty stored path, so projected backing also needs path composition at use.
  Retain the local symbol as the loan subject: rewriting it to the root place
  loses borrow/write-frame/release identity and disagrees with independent
  local-field replay. A known-place loan need not become a fresh affine
  `EstablishReference` result. Preserve the supported exclusive marker/store
  route while extending observations and successor forwarding.

  Acceptance: every producer form, reads through scalar/record loan names,
  borrowed calls and state forwarding reach verified Terminal and matching-host
  execution with exact backing and no implicit copies. Both pending fixtures
  and the full sample must advance. Reject conflicting loans, source/path
  substitution and invalid escapes. Verify constrained referents such as
  `u8 [0..=1]` borrowed as `&mut u8` against the actual loan/store contract;
  normalized type equality must not erase the original storage's obligations.
  **STATE-LOCAL-VALUE-FRONTIER** owns integration with the composed caller.

- **DEFERRED-LOCAL-INITIALIZATION.** Carry storage declared without an
  initializer through later establishment and observation. Complete non-`mut`
  record member-fill and mutable scalar assignment; retain mutable-record and
  initialized controls, and the diagnostic for non-`mut` whole-scalar assignment.
  `checking/src/value_custody/places.rs` deliberately distinguishes whole-local
  rebinding from member/index fill; removing the mutability diagnostic is not
  the missing capability. The pending fixture
  `tests/omega/pending/control_flow/deferred_local_initialization` contains only
  `let mut total: i32; total = 1;` (exit 1); materialize the other controls.

  Stage 04 `control/statement_sequence/local_data.rs` and
  `values/scalar/computations.rs` admit initializer-free storage only through
  mutable zero-constructible declarations. Store destination discovery carries
  the same restriction. The primitive-local branch additionally demands an
  actual `StorageInitializer`, and `structural_scalar_store/primitive.rs`
  requires a valid initializer to rediscover the local. Repair establishment,
  its exact declaration identity and subsequent reads/writes together; preserve
  existing source-custody and borrow-event rejection checks.

  The real customer `pass/wire/runtime_wire_utf8_edge_verdicts_exit` fills
  `LabelSample { count, label: &[u8] in Utf8 }` before borrowing it for decode.
  Its reference field cannot be fabricated by zeroing: support partial storage
  and exact field establishment, including stored-loan custody. Follow
  [default-domain gating](wiki/spec/language/dependent_values.md#default-domains-and-zero-initialization)
  and [ownership establishment](wiki/spec/language/ownership.md#establishment-and-erasure);
  reject premature observation and do not mint linear obligations from zero.
  Acceptance uses the actual Build entries and ordinary corpus execution:
  promote the pending fixture with exit 1, retain `uninit_record_member_store`
  exit 77, and execute the wire customer with interpreter/native exit 70.
  Euro must decode successfully; overlong, surrogate, out-of-range and truncated
  UTF-8 must fail. Clearing an omission phase alone does not close the task.

- **TRANSITION-TAIL-CALL-TO-MACHINE.** Complete terminal machine calls in
  attached Unit transition arms, including `transition { _ -> self.finish() }`.
  [State contracts](wiki/spec/language/state_contracts.md#terminal-calls-in-transition-arms)
  define an ordinary invocation whose normal result completes the caller;
  an internal state target remains a jump. Preserve selected-arm-only execution,
  once-only operands, result/return contracts, cleanup, crash and suspension
  behavior, live backing and loans, and recursive calls without accumulated frames.

  Stage 04 `execution/terminal_unit/state_graph/mod.rs` excludes transition
  calls from the ordinary operation window; its successor builder resolves
  internal states. `state_graph/terminator.rs` already handles non-Unit value
  arms, and `tests/flow/terminal_unit/outer_calls.rs` checks nested structural
  calls in those arms. Extend the missing call-and-completion route with exact
  occurrence accounting; do not bypass call-count or write-frame checks.
  `structural_scalar_store/frame.rs` now replays the complete state frame, so
  the old alternating omission labels are reproduction clues, not the cause.

  Acceptance: promote `pending/calls/transition_tail_call_to_machine` with its
  Build entry and exit-0 oracle. Materialize its comment-only controls: with
  and without a preceding store, no arguments and ordinary arguments, an
  internal-state jump, and an ordinary call followed by a jump. Run the three
  actual customers through checked, interpreted and native corpus legs:
  `collections/runtime_frame_indexed_param_read_exit` exits 1;
  `slices/domained_runtime_end_subslice_exit` exits 70 with its Path-qualified
  runtime-end slice; `termination/runtime_shrinking_slice_recursion_exit` exits
  70 from 10+20+15+25. The recursive case must snapshot the subslice and head
  read before overwriting argument slots. Preserve the fixed-array argument;
  a no-array control separates this gap from boundary ABI transport.
  `wire/runtime_wire_decode_byte_slice_exit` stays with
  **RECORD-LITERAL-ARRAY-FIELD** and **REFERENCE-LOCAL-BINDING-ROUTES**;
  `wire/runtime_wire_utf8_edge_verdicts_exit` stays with
  **DEFERRED-LOCAL-INITIALIZATION**.
  `filesystem/dir_walk_wrappers_exit` and `systems/note_vault` retain their
  `Filesystem::create_dir_all` dependency. Shared phase labels do not merge them.

- **CASE-FIELD-TYPE-STORE-SHAPES.** Complete three independent construction
  cases: predicate-qualified members (case payloads and plain records, directly
  or through locals), erased payloads, and common fields beside case payloads.
  Follow [data construction](wiki/spec/language/data_and_literals.md#construction-and-case-identity)
  and [erased bindings](wiki/spec/proofs/contracts.md#explicit-erased-bindings):
  evaluate runtime fields once in authored order; retain erased semantic evidence
  without runtime slots; establish domains before observation. Arithmetic-policy
  payloads and direct scalar-domain stores remain controls, not this omission.

  `checking/src/value_custody/scalar_case_constructor.rs` requires numeric-owned
  contents, rejects common fields, requires the full authored payload roster,
  and rejects erased operands in its structural constructor. Predicate domains
  are outside that contents classifier. Stage 04
  `values/scalar/computations/structural_values.rs` shares the classifier and
  also gates record construction. Stage 05
  `attached_unit/structural_values/structural_case.rs` requires a sum shape and
  rejects erased initializers. Repair retained establishment through lowering;
  merely removing an admission guard cannot supply the missing evidence.

  The pending `structs/case_field_type_store_shapes` actually contains only the
  common-field case; materialize its comment-only domain/erased controls and
  retain payload-free, scalar/multiple/record payloads and local-mediated stores.
  Acceptance: promote it with its Build entry and exit-0 oracle. Execute the
  `pass/` customers to exit 70: `ranges/sum_payload_range_arith_narrowed_exit`,
  `relevance/erased_case_payload_field_exit`, `data/runtime_mixed_shape_exit`,
  `calls/runtime_enum_self_method_exit`, `generics/runtime_mixed_generic_erased_sum_exit`
  and `traits/equatable_mixed_shape_equality_exit`. Preserve domain rejection
  and wrong-case negatives, erased ABI positions, common-field reads/writes,
  valid zero omission on case replacement, all-tag dispatch and unequal-common-field
  equality. Each cause needs its own checked and interpreter/native outcome.

- **RECORD-LITERAL-ARRAY-FIELD.** Compose supplied fixed-array and borrowed-slice
  fields into record literals at local, nested, return and field-store sites.
  Stage 04 `values/scalar/computations/structural_values.rs` sends non-scalar
  record members through structural values, but `fixed_array_value` declines
  closed primitive arrays; their ordinary scalar-array producer is separate.
  Omitted zero-valid array fields already compose (`tests/flow/terminal_unit/returns.rs`).
  Stage 05 `attached_unit/structural_values/emission.rs` still refuses general
  `FixedArray` establishment. Reuse existing array/record producers and carry
  exact element types, evaluation order and ownership through the full route.

  Borrowed views are another field producer, not owned array storage:
  `borrowed_slice_view_place` currently requires the expression to be the
  entire immutable local initializer. Compose constructor operands with exact
  stored-loan origins and lifetimes; **REFERENCE-LOCAL-BINDING-ROUTES** owns
  reference locals. Existing record emission already accepts borrowed-view fields.

  The pending `structs/record_literal_array_field` contains only a flat
  `[i32; 4]` literal. Materialize its controls: inline/variable arrays,
  scalar/nested-record fields, bare array locals, element-filled fields,
  lengths/types, negative operands, nested and generic/const-generic records,
  and later by-reference use. Promote it with its Build entry and exit-0 oracle.
  Acceptance includes unchanged `samples/cli/systems/wire_protocol` and
  `pass/wire/runtime_wire_roundtrip_nested_and_repeated_exit`: exit 70 with
  13-byte encode/decode, room 300, samples [150, -2], depth -64. The borrowed
  customer `pass/wire/runtime_wire_decode_byte_slice_exit` must exit 70 after
  re-encoding the decoded view to five bytes with payload [72, 105]. Run checked
  and interpreter/native legs; leaving an omission cluster is not execution.

- **PASS-TIER-GUARDS-OWED.** Recheck and promote
  `tests/omega/pending/calls/copy_projection_from_borrowed_receiver` into
  `pass/calls/` with scoped checked and native golden updates. Preserve its
  nested `[copy]` field read through `&mut self`, by-value argument, named state,
  terminal effect and authored ProgramEntry bindings. Make the exit observe a
  nonzero copied value at the callee and the unchanged source; add
  `expected_native_exit.txt` so the corpus gate executes this fixture.
  Copy establishment exists in `values/scalar/computations/call_arguments.rs`;
  `calls/structural_arguments.rs` selects its fresh result instead of moving
  borrowed backing. Repair any remaining omission before promotion. Recheck
  product roots under **OMEGA-PRODUCT-COMPILER-SOURCE** separately; the fixture
  does not establish their later operations.

- **FLOAT-RANGE-ATTACHED-FIELD.** (new-scope) Carry float invariants and
  constrained const applications through attached/nested data execution.
  First migrate the customers under **CANONICALIZE-SCALAR-RANGE-CONTRACTS**:
  [domains](wiki/spec/language/domains.md) require ordinary field `where`
  facts and contracts, not scalar range suffixes or replacement nominal domains.
  `terminal_unit/types/scalar_fields.rs` retains bounded integers but refuses
  float bounds; `data_where_field_intervals` in
  `proof_contracts/default_domains/where_fact_intervals.rs` reads integer
  literals only. Preserve IEEE storage and float facts without treating them
  as integer bounds. Separately, `terminal_unit/types/mod.rs` requires an exact
  attached generic application; the old shared phase label does not establish
  that float predicates and const constraints need the same repair.
  Migrate `tests/omega/pending/float/float_range_attached_field` and
  `tests/omega/pending/generics/ranged_const_parameter_attached_shape`, then
  exercise their currently unused fields. Keep bare integer/float, integer
  `where`, `Finite`, unconstrained const, and plain/`Wrapping` element controls.
  Required customers under `tests/omega/pass/`:

  - `float/finite_core_domain_range_discharge`: restore finite/range facts
    before the transition, pass the value to a `Finite` parameter, exit **77**.
    Its transient NaN and out-of-range assignments exercise the
    [invariant window](wiki/spec/language/numeric_values.md), not permission
    to expose invalid values at a boundary.
  - `recast/{runtime_float_range_representation_recast_exit,
    runtime_shared_record_float_range_weakening_exit,
    runtime_mutable_equivalent_record_recast_exit}`: exit **70**, preserving
    shared weakening and bit views, equivalent mutable representation sets,
    and original-storage reads after the exclusive loan's last use. Follow
    [recast compatibility](wiki/spec/layouts/recasts.md); equal sizes alone
    cannot admit strengthening or mutable widening.
  - `generics/runtime_const_container_methods_exit`: distinct capacities
    **2/4**, generic method witnesses of those lengths, stored values **30/40**,
    exit **70**. Retain exact specialization identity/layout and reject const
    tuples violating their declared constraints.

  Acceptance: canonical source checks, published Terminal reloads/verifies,
  and all five customers execute with those oracles. A migrated unused field
  or disappearance of the old phase label alone does not close this task.

- **VALUE-CALL-SUBEXPRESSION.** (new-scope) Finish composing scalar machine
  results under surrounding expressions at each execution site. The builder
  in `values/scalar/computations.rs` and `computations/integers.rs` already
  handles calls, arithmetic, casts and comparisons; stage 05's
  `scalar_return_calls_source/integer_computations.rs` lowers a call under
  addition, comparison and short-circuiting. Do not rebuild this as a new
  left-operand special case. The recorded rejections for
  `tests/omega/pass/calls/{value_call_embedded_in_binary_exit,
  transition_arg_local_from_embedded_call_exit}` report a pure initializer;
  `control/statement_sequence.rs::scalar_computation_local_at` takes that
  fallback when the exact computation root is absent. Reproduce each remaining
  site and retain its state, statement, binding and call occurrence through
  construction/emission rather than substituting a pure expression.
  `tests/omega/pending/calls/value_call_subexpression` currently discards
  `twelve() + 0`; make its value observable. Exercise arithmetic on either
  side, casts and both comparison operands in locals, field stores and guards,
  with bare/parenthesized-call and call-free controls, ordinary/field receivers,
  and Exact/`Wrapping` policies. Preserve result-owned carrier/policy and the
  [evaluation schedule](wiki/spec/language/expressions.md): ordered, single
  invocation, selective short-circuiting, no reevaluation at state arrival.
  Acceptance: both corpus rows verify **46** and exit **70** after Terminal
  reload and native execution; the second must carry the local across a state
  edge. `samples/cli/probes/value_call_in_expr` must preserve **46, 40, 64, 70**
  through its nested calls and transitions and exit **70**. Its historical
  slot-collision comment is not a diagnosis of the current compiler.

- **COMPOSITE-TRANSITION-ARGUMENT.** (new-scope) Finish legal record literals,
  field projections and composite call results supplied directly to internal
  states. `successor_bindings` in `terminal_unit/state_graph/mod.rs` already
  forwards whole parameters, retained local results, admitted subslices and
  selected case payloads; ordinary projected roots and new argument values
  need their own construction/transfer. Preserve exact type, access, copy/move
  semantics and residual cleanup under [state arrivals](wiki/spec/language/state_contracts.md).
  `tests/omega/pending/control_flow/composite_state_parameter` discards its
  `Pair { a: 1 }`; observe the payload. Compare distinct literal, field, call
  and local sources on unconditional and guarded edges. Retain scalar-policy
  controls and exercise legal reference and fixed-array arguments separately.
  Existing `tests/omega/pass/structs/{mutable_record_state_transfer,
  mutable_array_record_state_transfer,affine_record_state_clone}` record native
  **70** for carried borrows/local backing; they do not cover direct composite
  arguments. Acceptance: selected operands execute once, retain their values
  through Terminal reload and native execution, and unselected operands have
  no effects. With **VALUE-CALL-TRANSITION-ARGUMENT**, preserve
  `samples/cli/systems/vending_machine`'s `Change` delivery and exit **70**.

- **VALUE-CALL-TRANSITION-ARGUMENT.** (new-scope) Complete calls in internal
  state arguments using the existing scalar computation route. Psi stage 04
  already records `TransitionArgument` roots; stage 05's
  `composed_control/state_graph/scalars.rs` rejoins them and
  `state_graph/emission/successor_edge.rs` emits calls after edge selection.
  Reproduce the remaining admission/emission gap, not a blanket absence of
  call vocabulary. `tests/omega/pending/calls/value_call_transition_argument`
  currently ignores the delivered `amount`; make it observable and compare
  direct calls against local-binding and arithmetic controls.
  The recorded rejection for
  `tests/omega/pass/calls/runtime_guarded_effectful_transition_argument_exit`
  is at conditional successor scalar arguments. Acceptance: its skipped arm
  leaves **0** hits; selected/local calls produce **7/20** with **1** hit;
  paired calls produce **10/11** with **2** hits; nested calls produce **21**
  with **2** hits; native and reloaded Terminal exit **70**. Add an effect
  sensitive to call order: the existing identical increments only count calls.
  Preserve [arrival snapshots](wiki/spec/language/state_contracts.md) when later
  arguments mutate storage. Share expression construction with
  **VALUE-CALL-SUBEXPRESSION**; composite delivery remains the preceding task.
  `samples/cli/systems/vending_machine` must retain **3** sales, the delivered
  extra remainder **20**, and checksum/exit **70**.

- **BOUNDARY-PRIMITIVE-ARRAY-PARAMETER.** (new-scope) Carry authored by-value
  fixed primitive arrays through boundary arguments and results. The formal,
  source-parameter and result refusals in `calls/trait_boundary_call.rs` use
  `is_closed_primitive_array_type` (nested literal extents ending in Boolean,
  fixed-width integer or IEEE elements). Removing the argument guard was
  recorded to advance only to `completion`. Rejoin by-value presentation and
  completion together: `return_unit_affine_discards` in
  `terminal_unit/types/mod.rs` classifies owned projections as moves and refuses
  borrowed roots. Preserve legitimate copies, moves and remaining caller storage.
  Stage 05's `scalar_array_source/call_results.rs` still deliberately rejects
  boundary array results, even empty or discarded ones; ordinary array-call
  support does not establish boundary payload transport.
  `tests/omega/pending/providers/boundary_primitive_array_argument` selects a
  macOS provider that ignores the array and returns constant **7**. Make the
  provider inspect transferred elements and the caller observe the result;
  retain its valid Build identity, selected provider and demanded entry. Cover
  field/literal/local sources, scalar/reference controls, nested arrays and
  array results across trait and direct boundary calls. Validate exact payload,
  extent, scalar/structural interleave and selected requirement/provider custody
  under [boundary calls](wiki/spec/terminal-psi/boundary_calls.md), through
  Terminal reload/verification and native execution.
  Customers: `samples/gui/{window_app,window_demo,image_viewer}` pass class
  arrays of **7** bytes and titles of **10/11/13** bytes. Acceptance retains
  these by-value signatures and realizes their calls; domain/reference rewrites
  evade the task. Track later provider or GUI failures under **SAMPLE-CORPUS**
  rather than treating a changed omission label as success.

- **OPERATOR-BOUNDARY-CALLS.** (split-of:PROVIDER-SELECTION-AFTER-TERMINAL)
  Finish requirement-level calls and Omega stage 00 installation for legacy
  named operators and compiler-known float realizations under
  [boundary applications](wiki/spec/terminal-psi/boundary_calls.md#operator-applications-and-physical-children).
  Token-bearing machines already enter the ordinary call route.
  `05_checked-trees-to-lowered-psi/tests/provider_candidates.rs` checks one
  artifact across two providers and two const tuples, drops checked custody,
  then installs and executes both applications in the Terminal interpreter.
  That is not native execution or coverage of the legacy operator path.

  Remaining work:

  - Replace the early named-operator return in
    `checking/src/machine_calls/calls/expression_scanning.rs` with ordinary
    requirement-call facts and boundary plans. Legacy `boundary operator`
    declarations still miss that route; the selected-dispatch
    `uninstalled.rs` machinery reports the omission. After replacement,
    remove `UninstalledOperator` and its retired operator-adapter /
    float-intrinsic resolution machinery rather than extend those rewrites.
  - Finish native specialization custody: retain the exact closed requirement,
    provider instance, static tuple and artifact-bound application occurrence
    through selected-plan settlement and physical-child replay. Reuse stage
    00's `boundary_application_settlement.rs`; `ClosedReachApplication`,
    a template alias or equal runtime signatures cannot authenticate the
    specialization. Re-drive
    `providers/specialized_mixed_structural_result_operator_hosted_native`:
    its recorded stop is now native physical replay's exact selected-plan
    rejoin, not the old zero-row authority review. The native replay owner is
    `native-artifact/src/physical/derivation/evidence.rs`. The new
    `boundary_internal_call_evidence` test covers a nongeneric requirement's
    internal physical child, not this whole generic matrix.
  - Install named float execution as Omega builtins, preserving each exact
    selected meaning: negate, min/max, square root, directed arithmetic,
    conversions, multiply-then-add and distinct FMA. Native
    `provider_admission/compiler_builtins.rs` currently handles hosted
    exit/read/write only. Re-drive `pending/arithmetic/float_computation_no_route`
    both with and without the ordinary float-operations import across
    arithmetic/comparison, field/local/guard positions and f32/f64.
    Preserve declared operator selection rather than use a spelling-based
    fallback. **FLOAT-PROVIDERS** owns the semantic contracts and
    **X86-FMA-PROVIDER-TRANSPORT** the admitted FMA transport.
  - Complete a receiver-bearing requirement's member-call witness through
    borrowed-field argument planning, provider signature refinement and native
    execution. The verifier compares receiver flags as part of the structural
    signature; preserve exact receiver/argument correspondence, not a blanket
    `is_self` exemption. Reproduce the old parameter-source/refinement stops
    before assigning their repairs.
  - Keep ordinary call borrow rules when spelling operators as calls.
    [Exclusive loans](wiki/spec/language/ownership.md#borrows-and-aliases) forbid a
    second alias reading the old contents. Preserve
    `recast/runtime_mutable_equivalent_record_recast_exit`'s read-through-view
    then last-use release. With **BORROW-PROOF-CONVERGENCE**, recheck plain-read,
    call-argument and float-comparison forms of the same live-loan conflict;
    retain their rejection parity without weakening the call check.

  Reuse the repaired const path: pre-resolution `has_no_authored_spelling`
  recognizes token-bearing machines, and `selected_provider_bodies` in
  `compiler/src/checked/checking/const_evaluation.rs` handles non-float
  checked bodies. `comptime/provider_boundary_range_endpoint_exit` now uses
  `boundary machine %`, distinguishes provider result 7 from builtin result 1,
  and has a recorded native exit 70. Retain that control; the alternate-provider
  const-generic fixture still records a selection failure, so finish its
  distinct-result and unselected-refusal acceptance with
  **TARGET-SEMANTIC-APPLICATIONS**. Do not claim every const application works
  from the endpoint case alone.

  Acceptance: preserve the native exit-70 controls
  `providers/checked_fixed_operator_dispatch_exit` and
  `providers/token_machine_family_override_exit`; close legacy named and
  generic physical-custody cases, plus structural-input/scalar-result,
  mixed-input/scalar-result and mixed-input/structural-result hosted cases,
  retaining the owned affine operand/result and its disposition.
  Recover deleted customers from `089bd3290c` where necessary:
  `specialized_{boundary,fixed}_operator_physical_custody` and
  `specialized_{structural_fixed,mixed_structural_fixed,mixed_structural_result}_operator_hosted_native`.
  Retain `checked_{boundary,fixed}_operator_physical_custody` build/replay
  checks and each runtime customer's original oracle; an unused helper beside
  an empty entry is not execution coverage. Restore the named-provider
  `pass/float` native matrix and remove `applies boundary operator` omissions
  by completing the route. Run unchanged `sensor_min_max`, `vector_distance`
  and `fibonacci_golden` under **SAMPLE-CORPUS**; the last exercises conversion,
  independently of binary arithmetic. Float field/index store admission is
  already present in `structural_scalar_store/value.rs`; reattribute later
  failures instead of repeating that repair.

- **SOURCE-SET-UNION.** (split-of:PSI-TARGET-FAMILIES) The assembled source set
  must contain physical sources, every target's entry contracts, package
  imports and dependency-generated sources once per checked context.
  `ImmutableSourceParseCheckpoint::assemble` already takes no target and seeds
  every entry contract; `compiler.rs` shares assembly through `SharedBuilds`
  for equal package inputs. The CLI's `compile_project_for_targets` and package
  operations still launch separate compilations with target-specific bundles.
  Feed their union into one request with **BUILD-EVALUATES-ONCE**; generated
  target variation belongs in tagged declarations.

  Preserve [dependency handoff](wiki/spec/build/execution.md#dependency-handoff)
  custody and separate Build/Product checked instances, even for identical
  bytes. A consumer incorporates producer bundles without rerunning the
  producer; its own generated-source extension cannot rebind earlier source.
  Acceptance: a two-target CLI compile assembles the union once, including
  nested and dual-purpose generated dependencies. Count actual invocations;
  shared timing records can be copied into several target reports.

- **PSI-TARGET-FAMILIES.** (new-scope) Target-scoped machine declarations
  survive every Psi stage as one family with bodies keyed by target, including
  unrealized and unreachable bodies, per
  [multi-target compilation](wiki/spec/build/configuration.md#multi-target-compilation).
  `TypedTrees::target_machine_families` already records stable member handles;
  `symbols/target_siblings.rs` binds same-target calls. The gap is
  `select_product_target` swapping bodies before checking and
  `prune_target_siblings` dropping siblings before Terminal. Carry every body's
  checked semantics, supply mode and conformances through lowering, codec and
  independent verification; `omega inspect-terminal` must expose each target
  tag. Complete missing lowering under its existing owners; entry-reached
  bodies alone do not satisfy retention. **PROVIDER-SELECTION-AFTER-TERMINAL**
  owns Omega's selection from that same artifact; independent ownership moves
  can land before the complete route.

  Acceptance: retain `targets/sibling_body_type_error_rejected` and the native
  exit-70 `targets/target_machine_gating_exit` control. Its target bodies return
  equal values, so add distinct-result bodies realized from one Terminal,
  including same-target sibling calls. Check invalid unrealized bodies and
  duplicate family members; a missing called body fails only the affected
  realization. The existing `family_roster_keys_every_body_and_the_swap_consumes_it`
  unit test proves typed roster/swap behavior, not portable retention or execution.

- **BUILD-EVALUATES-ONCE.** (new-scope) `build.omg` evaluates once per
  compilation, observes no realized target, and produces checked rows for every
  target: providers, entry, subsystem, application intent, opaque representation,
  x86 scalar FMA provider, grants, behavior exclusions and wire demands.
  `BuildConfig` remains largely global; target-qualified root bindings and
  `SharedBuilds` reuse for equal package inputs do not complete this contract.
  Finish authored rows, retain/check unrequested rows, and join CLI/package
  execution through **SOURCE-SET-UNION**.

  Eliminate the duplicate preliminary whole-program check by moving its
  obligations into the owning stages. Preserve
  [admission before execution](wiki/spec/build/execution.md#admission-before-execution):
  freeze authored selections and admit the exact Build entry before effects;
  then incorporate generated source and finish product checks. The current
  gate is `declaration_admission::validate_authored_declaration_selections_before_build`.
  Reconcile snapshot inputs, required outputs and authority before sharing;
  `SharedBuilds` currently keys package/generated inputs without the per-target
  `build_snapshot`. Preserve separate package-purpose execution contexts.

  Acceptance: count one Build activation and one final product check for a
  two-target request; std's six rows come from one evaluation. Reject malformed
  unrequested rows and preserve per-target missing-row outcomes. Retain nested
  generated-dependency and dual-purpose custody (`package_commands/build_purposes.rs`),
  pre-effect rejection, and snapshot/required-output settlement
  (`package_commands/snapshot_outputs.rs`). Timing rows alone are insufficient.

- **PROVIDER-SELECTION-AFTER-TERMINAL.** (new-scope) Finish one target-neutral
  Psi product, with per-target provider installation consuming Terminal Psi
  plus that target's Build rows in Omega stage 00. Follow
  [multi-target compilation](wiki/spec/build/configuration.md#multi-target-compilation)
  and the [pipeline](omega-rust/pipeline.md#one-driver-one-pass-per-target-realization).

  Boundary-application and integer/IEEE-comparison settlement now live in
  stage 00's `boundary_application_settlement.rs` and
  `comparison_settlement.rs`. They still borrow checked trees, source
  identities and frontend-derived selected evidence; moving files did not
  complete the portable boundary. Finish their inputs and replay through the
  existing artifact-bound occurrence scope, without another parallel registry.

  Remaining frontend decisions are in compiler checking's
  `execution_settlement.rs` and `phase_transitions.rs`: target-family body
  substitution, calling plans, program-entry selection, fused-provider
  settlement, selected legacy generic-operator and boundary-family requests,
  selected-dispatch execution, callback materialization, task activation,
  component progress, x86 FMA associations and fused entry establishments.
  Move their per-target realization to Omega while preserving source-semantic
  contracts and all authored candidates in Psi. Token-provider candidates
  already derive from authored conformance edges without Build selection;
  extend that independence to the remaining paths. Checking only the realized
  targets' union is insufficient: every family's bodies and every Build row
  must be checked and retained even when `--target` excludes realization.
  Remove provider-planning's `typed-trees` / `validation` dependencies and
  enforce the layering boundary.

  Preserve the spec's const exception: provider-dependent constants belong to
  the one checked program. Every realized target must select the same provider
  for that application; evaluate it once through that checked body in Psi,
  rejecting disagreement with the diverging targets named. Do not move a
  layout-determining fold into per-target native realization. Carry exact
  selection/replay evidence through the existing const evaluator.

  Retain each requirement's complete portable contract. Scalar `requires`,
  crash routes and content guarantees exist; general scalar `ensures` still
  have no declaration field in `BoundaryMachineDeclaration`. Complete their
  transport and independent provider/call validation without narrowing the
  public contract to the selected body's behavior.

  Acceptance, together with **BUILD-EVALUATES-ONCE** and
  **OPERATOR-BOUNDARY-CALLS**: one two-target request runs Psi 00–07 once and
  produces one Terminal artifact identity; each target's native output equals
  its single-target output byte-for-byte across the native corpus and passes
  the same execution oracles. Preserve the target-comparison customers
  `arithmetic/const_fold_unsigned_landed_ops_exit`,
  `expressions/match_float_patterns` and
  `providers/checked_boundary_requirement_dispatch_exit`, but do not treat
  their equal Terminal bytes as proof that checking ran once. Cover differing
  runtime providers, agreeing/disagreeing const selections, errors in an
  unrealized body, and a missing required realization row without suppressing
  the other target's outcome. Independent moves need not wait for full closure.

- **ONE-DRIVER-PER-STAGE.** (new-scope) Close the remainder after the items
  above: `compiler.rs` runs Psi 00–07 once, then Omega 00–09 and emission per
  realization, following the [pipeline](omega-rust/pipeline.md). Keep each
  stage's independent representation and transformation crate. Package review
  calls the same 00–03 prefix directly; remove alternate compile modes.
  Source preparation supplies stage 00; build-time evaluation, dynamic binding,
  const evaluation and semantic pruning belong inside stages 02–04. Carry
  discovery, grants, sponsors and snapshots in their owning stage inputs/outputs.

  `CompileRequest` and `ExplicitTargetSet` already exist: connect CLI/package
  callers to them and normalize CLI targets once. Acceptance: the compiler owns
  only composition, reports and publication; a two-target native compile proves
  one Psi stage chain and one Terminal identity, then one Omega chain per target,
  preserving native results and isolated realization failures as specified in
  **PROVIDER-SELECTION-AFTER-TERMINAL**. Prefix consumers stop at their actual
  checkpoint; stage counters must distinguish execution from reused reports.

  Retire the per-target source clones and reuse structures that merely mask
  repeated compilation (`NativeInputReuse`, `CandidateSourcePreparation`)
  as their work moves into the shared stages. Preserve single-target command
  behavior, Build-effect custody and per-target admission/publication checks.
  **PCC-PRODUCT-PUBLICATION** owns the separate Terminal-only command product.

## Immediate product closure

Prioritize unchanged customer programs reaching native execution over additional
evidence carriers without an exercising program. The
[Rust compiler completion plan](wiki/drafts/reference/rust_compiler_completion.md) defines
the complete product bar; focused successes below do not establish that baseline.

- **STATE-LOCAL-VALUE-FRONTIER.** Complete ordinary expression, call and
  storage composition through checked plans, source-free Terminal replay and
  native execution. Preserve the
  [evaluation schedule](wiki/spec/language/expressions.md#evaluation-schedule)
  and [argument/result custody](wiki/spec/terminal-psi/calls_and_outcomes.md#argument-and-result-ordering):
  exact producer, authored parameter position, result owner, loan activation
  and once-only effects. **CML4** owns residual cleanup;
  **GENERAL-CYCLIC-EXECUTION** owns cyclic control. Owners are
  `04_typed-trees-to-checked-trees/src/execution/terminal_unit/`, its checked
  computation producers, and `05_checked-trees-to-lowered-psi` argument
  evaluation, source-custody replay and attached Unit emission; Omega stage 02
  owns the remaining target admission. Extend common operations and remove
  superseded whole-machine recognizers as composition replaces them.

  Prioritize the operations blocking maintained programs, then finish their
  custody/ABI matrix. **SAMPLE-CORPUS** and **CANARY-CORPUS** retain the full
  outer program oracles; a moved omission or verified helper is not closure.

  - **Runtime-selected record fields and typed observations.** Complete
    `fixed_array[index].field` for `entity_list`, `bouncing_particles` and
    runtime case selection in `dutch_flag`. The pending reproductions are
    `collections/runtime_indexed_record_field_store` and
    `control_flow/runtime_indexed_case_dispatch` under `tests/omega/pending`.
    Stage 04's `scalar_lowering::view_element_field_read` still excludes fixed
    arrays via `through_view`; stage 05 must carry the element path after the
    selector. The Terminal obstacle remains in
    `terminal-psi/semantics/src/primitive_place.rs`: `primitive_projection_type`
    follows fields only into structural children, so it cannot end on a Scalar
    field. `IntegerStructuralField` instead takes a canonical, static path;
    `ElementViewRead` lacks the fixed-array carrier path. Complete one portable
    projection/observation route under the [product contract](wiki/spec/terminal-psi/product.md),
    with exact runtime-index bounds and independent type reconstruction.
    Removing the stage 04 guard alone cannot deliver this operation.

    Float field observation needs the same typed-read design, not another
    integer/Boolean-shaped copy. Store admission already accepts structural
    float reads, but `expression_preparation/prepare_expression.rs` still
    rejects non-integer `StructuralParameterField` observations. The current
    `src/tests/structural_float_member_read.rs` pins an integer-member clone
    that verifies, a float-member clone that declines, and nested calls in
    a record tail that fail call ordering. Carry the float source, computation,
    policy and observation through replay and execution; coordinate
    **OPERATOR-BOUNDARY-CALLS** / **FLOAT-PROVIDERS** for executable arithmetic.

  - **Computed calls and state transfers.** `vending_machine` passes
    `report(self.make_change(100, 55), 17)`: preserve the inline call and exact
    normalized argument occurrence. **VALUE-CALL-TRANSITION-ARGUMENT** and
    **COMPOSITE-TRANSITION-ARGUMENT** own its scalar-call and composite-source
    gaps; **VALUE-CALL-SUBEXPRESSION** covers the related operand position.
    The structural result transfer vocabulary already exists. Account the
    nested call as consumed by the normalized successor argument, include it
    in the state's ordered operations, then emit the terminator-site operation.
    `state_graph/mod.rs::operation_end` currently excludes Transition calls;
    `control/call_occurrences.rs` rejects an unconsumed nested transition call.
    Bypassing those guards without producer/consumer correspondence is unsound.

    Finish mixed scalar/structural operands, effectful arguments/returns,
    nested mutable-receiver calls, dependent/public-trait results and boundary
    consumers on that common route. Selected case edges retain the invocation
    receiver and local ownership. Preserve source evaluation order even when
    the physical signature drops an unread reference `self`:
    `Random::next_u32(&mut self, state: &mut RandomState)` needs the exact
    authored-to-dense parameter mapping in planning, operation frames and stores.
    `machine_lowering/machine_dispatch.rs` still rejects competing scalar/Unit
    dynamic plans; `unit/structural_unit_control.rs` still refuses checked
    expressions in its specialized scalar-successor route. Remove those shape
    limits through ordinary argument evaluation, not another whole-body plan.

  - **Loan locals and projected storage.** **REFERENCE-LOCAL-BINDING-ROUTES**
    owns shared/exclusive local establishment, reads, returned references and
    state forwarding; **RECAST-SOURCE-POSITIONS** owns typed recasts. Preserve
    the local's loan identity and exact backing through every consumer,
    including local-rooted stores and borrowed-record calls followed by reads.
    `structural_values::is_record_value` still gates on StructLiteral/Match
    roots; generalize the operation instead of rewriting a local observation
    into a different source subject. Primitive borrowed-local mutation already
    has serialized Terminal execution and forged-binding controls in
    `tests/borrowed_scalar_call_source.rs`; retain them as controls for the
    record extension. Fresh linear-record custody and non-vacated replacement
    must preserve bounds, layout, loans and claim lifecycle; construction does
    not itself issue authority.

  - **Aggregate construction, replacement and subsequent reads.** Keep
    whole-place and match-produced values, fixed-index element replacements,
    whole nominal receiver replacement, qualified byte-slice literals, case literals,
    and array literal elements that read storage. Policy-bearing nested arrays
    in `matrix_multiply` and record arrays in `dungeon_render` are owned by
    **MEMBER-POLICY-REMOVES-COMPOSITE-ROWS**; do not widen the shared bare-array
    classifier merely to route one store. The old request to construct relevant
    `UInt`/`Int` fields was invalid: these unbounded proof integers have no
    runtime representation, and checking now diagnoses them. Preserve erased
    proof fields and use represented carriers for runtime storage.

    Ordinary record-field replacement already uses `OpenWindows::replace`
    with move-out/store and exact displaced-value cleanup. Omega's
    `lowering/control_flow/borrowed_windows.rs` now resolves Record and Mixed
    parents, but still requires an owned activation home and rejects
    reference-bearing, linear, claimed or qualified replacement values.
    Complete those valid custody cases, including values arriving as owned
    parameters/block arguments, without double disposal or a fabricated move
    window. Verify empty-record locals both when forwarded and when discarded;
    a successful forwarding path does not prove edge cleanup. Non-scalar-leaf
    zero-length arrays still reject in native structural layout; preserve
    empty-index and literal-arity rejection while admitting empty construction.
    `filesystem/windows_canonicalize_exit` needs its stored UnitResult;
    `data/runtime_case_reassignment_exit` and
    `structs/deep_nested_write_paths_exit` need observations through case paths,
    which `bindings/structural_fields` still rejects. Coordinate
    **FILESYSTEM-RELEASE-CONTRACT**, **WRITE-ONLY-BORROW** and
    **NOMINAL-FIELD-FLOW**.

  - **Claim-bearing results and native receiving.** Carry claim transfers on
    calls before a composed structural return. The checked state planner still
    refuses nonempty call claim transfers although attached-call admission
    supports them. Extend returned custody to extracted projections, freshly
    established claims and claims from distinct owned inputs; retain temporary
    shared/mutable/write-only loans, self consumers and content guarantees
    independently of claim identity. Whole-array forwarding in
    `effects/structural_callback_reach/projected.omg` is only a control.

    Complete both native customers in Omega stage 00's integration tests:
    `scalar_affine_cleanup/structural_return.rs` preserves a qualified linear
    result and entry claims through Terminal-to-abstract lowering, but its
    successful target-lowering leg explicitly clears those claims and changes
    multiplicity to Unrestricted. Execute the original custody case; stage 02
    still rejects entry claims/observed owned-linear parameters. In
    `partial_affine_call_results/continuations.rs`, direct-register results
    reach target lowering, while wider/nested structural results still assert
    rejection. Carry both distinct result owners and ordered residual cleanup
    through native execution, including boundary-produced results and indirect/
    stack ABI slots such as Windows x64's 12-byte Msg, and scalar returns from
    borrowed service calls.

  - **Saved observations and arithmetic dependencies.** Stable unwritten
    mutable carriers already contribute saved facts in
    `checking/src/proof_contracts/contract_entailment/ranking_range/saved_arguments.rs`.
    Finish snapshots across actual writes/exclusive exposure, retaining
    case/index/generic/reference/float identity and totality. Share capture with
    **CRASH-CONTRACT**; body facts cannot impersonate entry observations and
    loop invariants must cover backedges. Complete bounded-slice endpoint,
    window, call and state transport through Terminal replay and native
    execution; the deleted `compiler --test bounded_slice_selectors` is no
    longer evidence for those distinct obligations.

    Carry exact anonymous arithmetic through generic/evidence-adapted,
    boundary, aggregate, parameter/constant, float and proof uses, retaining
    rational intermediates, result carrier/policy custody and warning origins/
    suppression. **OPERATOR-MACHINE-SUPPLY** owns executable selected helpers;
    nonconstant proof-Int arithmetic still needs independent evidence beyond
    source entailment in `contract_entailment/arithmetic_judgment.rs`.

  Preserve implemented controls while completing the remaining matrix:
  `borrow/runtime_exclusive_loan_local_write_exit` and
  `calls/runtime_value_call_nested_entry_call_exit` have recorded macOS native
  exit 70; the latter still needs matching-host coverage beyond an old Windows
  rejection. Record/case destructuring must execute bind-all, `as _` waivers,
  literal `__` field names, renamed fields and rest opt-out, including non-place
  subjects evaluated once.
  Keep `data/record_pattern_let_exit` and
  `control_flow/{record_pattern_arm_rename_guard_exit,arm_pattern_rest_optout_exit,case_pattern_rename_waive_exit,runtime_nonplace_record_pattern_single_evaluation_exit}`.
  Case-payload transfer emission now exists in `emission/successor_edge.rs`;
  `state_graph/tests/case_payload_edges.rs` checks forged source/target replay,
  so retain that coverage and execute a consumer of the selected payload.

  Acceptance: a composed caller tolerates renamed/reordered states and inserted
  computations; operands execute left-to-right once, skipped calls never run,
  and saved values/normal guarantees agree after serialization, replay and
  matching-host native execution. Explicit state transfers preserve custody
  without mandatory copies; implicit cross-state use rejects. Forged producers,
  bindings/origins, stale snapshots, conflicting loans, missing provider
  authority and incorrect guarantees reject. Preserve guard-operand write
  invalidation and claim/return-substitution controls; a failed custody rejoin
  never falls back to a weaker recognizer or synthesized arm states.
  Keep `7 / 2 * 2 == 7` and `4097 / 4096 * 4096 == 4097` with their warnings,
  reject fractional integer landing, and retain typed truncation (the `4097u32`
  case gives 4096), signed quotient/remainder laws and zero-divisor rejection.

- **LOWERING-ROUTE-CONSOLIDATION.** Replace competing whole-machine
  recognizers with composable checked operations and one shared lowering
  catalog, preserving the [pipeline](omega-rust/pipeline.md) boundaries.
  Owners: stage 04 execution planning, checked-tree plan rosters, and stage 05
  `machine_lowering/machine_dispatch.rs`. Dispatch wrappers already share
  publication, but `CheckedReturnPlan::for_machine` precedence, scalar/Unit
  assembly forks and `CompetingCandidates` still choose among overlapping
  bodies. Prioritize operation gaps blocking **STATE-LOCAL-VALUE-FRONTIER**
  and maintained programs; each replacement removes its superseded recognizer.

  - **Ordinary/state-graph parity.** Both use `operation_frame` and shared call
    admission. Finish claim transfers through state-graph calls and linear
    boundary-result custody: `ClaimBindings::Fixed` cannot mint a result claim,
    and `Evaluation::establish_structural_result` accepts linear locals only
    from ordinary structural-call producers. Preserve exact result ownership,
    qualifications and cleanup. Boundary scalar calls and scalar-call byte
    literals already have composed emission; keep
    `calls/runtime_value_call_literal_len_arm_guard_exit`'s two distinct results
    and recorded native exit 70 instead of reopening those repairs.
  - **One scalar closure assembler.** Fold `scalar_call_closure` into the shared
    Unit catalog; its pure assembler still refuses content effects across
    member calls. Preserve source mappings, structural types, services and
    proofs. Transfer multi-state scalar bodies from scalar-owner precedence
    only with continuation-arm custody, later-state mutable scalar stores and
    crash predicates, float prefix initializers and debug maps accounted for.
    `CheckedStructuralControlSuccessorPlan` still lacks continuation identity.
    Keep result guarantees, including distinct exits returning 20 and 30 under
    `ensures result >= 20`; the current `composed_scalar_results` guarantee
    control covers common exit facts. Preserve constant-stack mutual tail
    fusion and `calls/mutual_cycle_tail_admitted_exit`'s native exit 70.
  - **Returns and structural control.** Replace structural-scalar,
    boundary-scalar, structural, claim-free affine, trait-operator and guarded
    payloadless return rosters with ordinary operations plus result completion.
    Preserve outcome-selected evidence and proof-only tail arguments: the
    `payloadless_case_return_source/selected_witness_tail_uses.rs` customer
    independently verifies and interprets its guarded result within four fuel
    units. Coordinate operator returns with **OPERATOR-BOUNDARY-CALLS**;
    `providers/specialized_structural_fixed_operator_terminal_custody` contains
    an unused `exercise` beside an empty entry, so its native build does not
    prove operator execution. Fold `unit/structural_unit_control.rs` into the
    general graph while retaining ranked loops, transfer/cleanup frontiers,
    diamond joins and resumable execution from `tests::structural_control_cases`.
    That suite mixes source programs and injected legacy plans; port the latter's
    corruption checks to the replacement representation.
  - **Shared custody and namespaces.** Embed common dynamic-call custody in
    scalar and Unit plans instead of maintaining `DynamicCallCustody`, builder
    macros and separate lowering/join views. Preserve exhaustive control;
    relaxing the dynamic join's `_` check alone does not admit a two-case sum
    dispatch. Build nominal cleanup once rather than in both Unit candidates
    and `finalize_execution`. Replace per-roster `ShapeCollector` namespaces
    and the manual `UnitPlans::with_staged` union with shared type ownership
    inside the existing checked/lowered representations.

  Acceptance: each removed route's programs lower through the common path,
  retain codec/debug/source-custody evidence, independently verify and preserve
  interpreter/native outcomes. Keep distinct return, continuation, cleanup and
  tampering cases when replacing tests of retired plan types. The affected
  corpus and execution oracles must pass; fewer dispatch branches or an empty
  entry compiling do not establish closure.

- **STATE-GRAPH-SCALAR-RESULT-CUSTOMERS.** Finish native execution of
  result-bearing state graphs and their callers under **LOWERING-ROUTE-CONSOLIDATION**
  and **STATE-LOCAL-VALUE-FRONTIER**. Scalar result completion is implemented;
  retain `proofs/runtime_decreases_u64_measure_exit`'s five effects/result 5
  (recorded native exit 70) and `calls/value_call_param_effect_arm`'s one
  mutation (exit 1). The latter replaced the obsolete fail fixture; restoring
  its rejection would regress mutable-record transfer.

  - **Ranks through complete callers.** `CollectionLengthDistance` already
    carries a collection-bound cursor through stage 04, stage 05 replay and
    rank emission. `unit_state_graph/ranking.rs` verifies and interprets normal,
    empty and already-crossed bounds, and rejects substituted collections and
    nonadvancing cursors. Re-run `calls/runtime_value_call_statement_recursive_walk_exit`:
    its recorded omission is still natural ranks, but creating another measure
    is no longer the assignment. Preserve both separator-count effects and
    result 2, native exit 70. Connect `termination/rank_range_state_call`'s
    count/step cycle through its subordinate state with **GENERAL-CYCLIC-EXECUTION**
    and **TRANSITION-TAIL-CALL-TO-MACHINE**. It has no Build entry; its native
    golden says `checked`, which does not prove execution. Bind an executable
    customer preserving the weak/strict cycle and result 0.
  - **Results and control.** `calls/runtime_dispatch_float_terminal_exit`
    currently records a guard-expression omission before the remaining
    Boolean/integer-only result gate in native `scalar_graph_input/byte_views.rs`.
    Close both stages for its f64 result. Finish case-payload field observation
    in `expression_preparation/bindings/structural_fields/` for
    `calls/runtime_dispatch_result_enum_case_exit` (score 15, exit 70), and
    receiver transfer through `control_flow/runtime_tuple_transition_exit`
    (exit 22). Keep short-circuit evaluation and later-binding order from
    `unit_state_graph/scalars.rs`; the old prefix-initializer refusal has been
    replaced by ordinary conditional sequencing.
  - **Borrowed arguments.** Reconcile source admission and independent loan
    replay for `calls/runtime_param_receiver_second_instance_exit` and
    `calls/runtime_dispatch_{slice_element_terminal,machine_array_slice_arg}_exit`.
    They currently record overlapping-exclusive-argument refusals. Preserve
    second-instance value 9 versus 7 and slice element 7, with exit 70;
    **BORROW-PROOF-CONVERGENCE** owns compatibility evidence. Genuine conflicting
    parent/child loans must still reject; widening the verifier's pair check
    does not supply missing authority.

  Account for deleted customers through equivalent current coverage or recover
  their distinct behavior: `runtime_trailing_state_mut_param_phase_exit` at
  `ba6e88a4101^` observes two mutations before its outer guard and three before
  its trailing guard (exit 70). The old `rooted_residual_scalar_entry_cohort`
  at `c60792d7bdd^` exercised `guarded_transition_dispatch` and
  `record_array_field_access`, each returning 0 through a scalar callee;
  preserve their case dispatch, record-array/string operations and output.

  Acceptance: every named customer has its actual entry and interpreter/native
  oracle accounted for. Use focused corpus filters and matching-host native
  execution; the default corpus can expose lowering failures, but a `checked`
  or `built` record without execution does not satisfy a runtime oracle.

- **SAMPLE-CORPUS.** Close every maintained `samples/cli|gui|uefi` project
  through checked semantics, native publication for its authored targets, and
  documented exit/output on matching hosts under the
  [release matrix](wiki/drafts/reference/rust_compiler_completion.md#release-matrix).
  **SQUALR-HEADLESS** owns the separate application checkout;
  **CANARY-CORPUS** owns language fixtures. This item retains sample integration
  until each full program works, even when a capability repair has another owner.

  Use `python3 tools/sample_gate.py` for the complete checked sweep. It discovers
  projects with `build.omg` and `main.omg`, excludes `samples/apps/squalr`, builds
  the current CLI unless `--cli` is supplied, and checks in scratch directories.
  `--filter` narrows investigation; `--write-results` retains the actual results.
  The deleted `compiler --test samples_compile` cohort is no longer acceptance.
  `mbx run -p omega -- refresh-samples` follows ordinary package review and native
  publication on the host, but does not execute documented runtime oracles.
  Connect those oracles to maintained coverage; a successful refresh is only
  the build half. Preserve exact output, stderr, input consumption and timing
  requirements where the sample documents them. Review each checkout/target's
  package inputs rather than borrowing another checkout's accepted lock.

  Prioritize shared operations that unblock full programs. Reproduce with the
  current CLI and the sample's package/entry inputs: a stale binary, an earlier
  dependency-review refusal, or an effect-free reduction does not exercise the
  same route. Follow the reported state/statement and unavailable callee into
  `04_typed-trees-to-checked-trees/src/execution/terminal_unit/`; a trace phase
  records the last route entered and can misidentify the declining operation.
  Keep the algorithm, storage and observable behavior when migrating syntax.

  | Customer group | Capability owner and remaining acceptance |
  | --- | --- |
  | `arithmetic/dual_float`, `basics/unit_converter`, `simulation/particle_sim`; `arithmetic/{sensor_min_max,vector_distance,fibonacci_golden}`; `basics/temperature_convert`, `rendering/{bouncing_ball_2d,mandelbrot,mandelbrot_zoom}` | **OPERATOR-BOUNDARY-CALLS**: executable float comparisons/arithmetic, nested value calls, and selected `F64::{minimum,maximum,from_i32}` respectively. Conversion is a separate operation from binary arithmetic. Retain `pending/arithmetic/float_computation_no_route`; adding `core::float_operations` only changes where the missing route is exposed. |
  | `collections/entity_list`, `rendering/bouncing_particles`, `algorithms/dutch_flag` | **STATE-LOCAL-VALUE-FRONTIER** and **MATCH-SELECTIVE-LOWERING**: runtime-indexed record-field reads and case selection, pinned by `pending/collections/runtime_indexed_record_field_store` and `pending/control_flow/runtime_indexed_case_dispatch`. `dutch_flag` must construct, read and swap the actual Color array, interact with Console and exit 70; integer substitutes are not acceptance. |
  | `systems/descriptor_walk`, `systems/elapsed_timer` | **REFERENCE-LOCAL-BINDING-ROUTES** / **RECAST-SOURCE-POSITIONS** for the borrowed local recast; **PASS-TIER-GUARDS-OWED** for the copy projection from a borrowed receiver, also a product-compiler customer. Rerun complete samples after the focused operation is repaired. |
  | `collections/matrix_multiply`, `rendering/dungeon_render`, `systems/wire_protocol`, `probes/value_call_in_expr`, `systems/vending_machine` | **MEMBER-POLICY-REMOVES-COMPOSITE-ROWS**, **RECORD-LITERAL-ARRAY-FIELD**, **VALUE-CALL-SUBEXPRESSION**, and both **COMPOSITE-TRANSITION-ARGUMENT** / **VALUE-CALL-TRANSITION-ARGUMENT** own the respective operations. Retain `dungeon_render`'s field/glyph evidence under **NOMINAL-FIELD-FLOW** while verifying its complete program; transition transport is already implemented. |
  | `gui/{image_viewer,window_app,window_demo}` | **BOUNDARY-PRIMITIVE-ARRAY-PARAMETER** owns the authored by-value primitive-array boundary arguments/results. On macOS ARM64 at `7c17208d59`, `omega --check samples/gui/window_demo/main.omg` gets past the missing direct float import and anonymous bitmap-height arithmetic, then rejects `Gui::window_create(self.cls, self.title, ...)` at state 0, statement 22 (`call operation: boundary arguments`). Resume argument custody in `04_typed-trees-to-checked-trees/src/execution/terminal_unit/`; acceptance remains the complete native GUI run. Changing parameters to domain carriers or references evades that acceptance. Exact fixture/sample provider selection remains required. |
  | `systems/file_journal`, `systems/note_vault`, `interpreters/calculator_rpn` | **STATE-LOCAL-VALUE-FRONTIER**: result custody, library callee composition, and indexed nested-field initialization. Follow `Filesystem::create_dir_all` in `note_vault`; for `calculator_rpn`, retain the Console-bearing receiver and `let a: i32 in Saturating = self.stack.slots[0]`. Its std-free reduction does not reproduce the full program. |
  | Integer type/policy migration verification | **IMPLICIT-STORE-CONVERSION**: the rejection rule and many explicit casts are implemented. Recheck the previously affected cohort without changing arithmetic policy or algorithms: `longest_run`, `maze_flood`, `collatz_sequence`, `prime_sieve`, `multiplication_table`, `heat_grid`, `dice_histogram`, `dungeon_crawler_cli`, `rpn_calculator`, `stack_calculator`, `self_mutation_between_calls`, `histogram`, `game_of_life`, `rule_30`, `luhn_checksum`, `substring_search`, and `windowed_calculator`. Re-measure the current set instead of treating an old refusal count as a baseline. |
  | Clock-bearing samples | Complete declared-host provider wiring for `binary_search_viz`, `sort_visualizer`, `bouncing_console`, `ripple_field`, `tick_marquee`, `cellular_automaton`, `game_of_life_glider`, `langtons_ant`, and `random_walk`. Existing macOS wiring is in `samples/gui/window_app` and `source/library/std/macos_gui.omg`: exact Clock selection and sleep through `darwin_poll`. Missing host selection is distinct from a lowering failure. |
  | `basics/print_number`, `rendering/dungeon_render` and other text/index/match programs | **NOMINAL-FIELD-FLOW**, **WRITE-ONLY-BORROW**, **STATE-LOCAL-VALUE-FRONTIER**, **MATCH-SELECTIVE-LOWERING**, **CML4** and **OPERATOR-MACHINE-SUPPLY** own their missing operations/facts. Retain `Utf8` and ordinary domain evidence; recognized function names or weakened invariants cannot replace it. |

  Sample paths in the table are relative to `samples/cli` unless prefixed `gui/`;
  pending reproductions are under `tests/omega`. Repaired fixture-level cases
  such as the brightness-control dispatch chain still need their permanent
  pass guard and full sample outcome under **PASS-TIER-GUARDS-OWED**.

  Preserve these distinct product oracles alongside the shared capability work:

  | Customer | Required product behavior |
  | --- | --- |
  | `cli_mvp`, `euclid_gcd`, `generic_counters`, `number_guess` | Ordinary reviewed native execution across Windows x86-64 and both Linux hosts, retaining macOS ARM64 coverage; documented exact output and exits 0/12/16/70 respectively. For interactive input, keep EOF/Enter behavior, both lines and prompt timing, with empty stderr. |
  | `print_squares` | Unchanged byte-storage/cyclic program prints nine computed rows ending in `081`, exit 0. **NOMINAL-FIELD-FLOW**, **CRASH-CONTRACT** and **GENERAL-CYCLIC-EXECUTION** own facts, envelopes and plans. Resume this customer with a path from its full source closure to native execution; wrapping multiplication already has legalization/selection. |
  | `recursive_sum`, `framed_payload` | Complete the same hosted matrix through ordinary review; exact 144/165-byte output and exits 70/60 respectively. Preserve saved reads, untouched siblings and shared payload loans. |
  | `calendar` | **ARITHMETIC-POLICY-REALIZATION**: unchanged trapping conversions and native rendering of all five grid rows plus the live 20-byte header in capacity-21 buffers. Matching a banner containing `30` is insufficient. |
  | `longest_run`, `parse_int`, `roman_numeral`, `string_hash`, `substring_search` | Mixed `i32` cursor / `u64` length comparisons under **ARITHMETIC-POLICY-REALIZATION**, then documented runtime outputs. Do not change cursor types or cast away the settled comparison semantics; indexing still needs a nonnegative cursor. |
  | `cli/proofs/math_proofs` | Ordinary multiset data/slice extraction, selected laws and checked proof terms, with the false twin rejecting. Equal lengths do not prove equal contents; `core/seq.omg` is not a Bag implementation. |
  | Bounded Console input | Under [bounded input](wiki/spec/resources/bounded_input.md), preserve exact prefix/destination, once-only effects, cleanup, blocking/crash contracts, zero-capacity non-consumption, LF/EOF/Full, untouched tails, alias rejection, continuation, failure and invalid-result controls. Keep prefix guards until count-to-extent evidence exists; service receipt migration belongs to **ENTRY-CONTENT-ROOTS**. |

  Reproduce the reported long checks for `gui/windowed_calculator` and
  `cli/simulation/game_of_life_glider` on a current binary and locate the cost;
  the historical stall did not establish a provider-selection or proof-search
  cause. Reuse unchanged compilation inputs under **CHECK-CLOSURE-ONCE**, and
  measure the complete sweep. Non-tail self recursion in `arithmetic/stats_compute`
  remains a separate execution customer. GUI product acceptance remains
  **MACOS-APPLICATION-PUBLICATION**.

  Acceptance: every maintained sample checks, every applicable native product
  builds, and documented runtime oracles pass on the required matching hosts.
  Record unavailable hosts explicitly; filtered successes and diagnostic
  attribution do not establish complete coverage.

- **CANARY-CORPUS.** Close `tests/omega/{pass,fail,run}` at each fixture's
  promised stage under the
  [release matrix](wiki/drafts/reference/rust_compiler_completion.md#release-matrix).
  Use `python3 tools/corpus_gate.py`, `--filter fail/` for diagnostic attribution,
  and `--native` on each required matching host. `--both` adds the checked
  interpreter where supported. The maintained harness is
  `omega-rust/omega/compiler/tests/corpus_runner.rs`; `canary_suite` was deleted.
  **SAMPLE-CORPUS** owns full sample integration; non-corpus/package gates remain
  in [release closure](#rust-compiler-release-closure).

  The check leg already realizes a pass/run fixture that declares an entry
  root, using its bound host target or first declared target. Reopening that
  harness repair would duplicate existing work. A rootless fixture still only
  checks declarations; `checked` is never proof of execution. Native execution
  requires a matching bound host and a run-tier fixture, `_exit` suffix or
  `expected_native_exit.txt`. Explicit exit/trap oracles are enforced before
  recording; `input.txt` and `expected_stdout.txt` supply input/output evidence.
  See [test workflow](tools/testing.md#attributing-a-move) for attribution.

  Finish coverage and roster accounting here; the old
  `CANARY-CORPUS-ROSTER-ACCOUNTING` name has no separate assignment:

  - Reconcile added/removed fixtures with the check and per-host native goldens
    after attributing their actual outcomes. The removed
    `fail/calls/value_call_param_effect_arm_rejected` still has a check-golden
    row; its accepted replacement and the mutable-record transfer fixtures
    have explicit exits 1/70/70 and recorded macOS native results. Preserve
    `fail/borrows/mutable_record_state_alias` as the rejection control and
    finish other-host coverage; do not reopen the old three-gate repair.
  - Preserve one portable check golden without hiding semantic differences.
    Target-name/fingerprint normalization already exists, but host-selected
    providers can still change the actual refusal. Attribute those changes
    rather than repinning one host over another. Keep missing/removed rows,
    wrong-reason negatives, crashes and timeouts visible.
  - Restore runtime promises lost with deleted per-feature tests as maintained
    fixture/harness oracles. Many named customers below have no input or stdout
    file. `host/runtime_console_bounded_line_exit` binds only `Main::main` and
    currently records an empty-input exit; that does not exercise `Main::repeat`
    or the LF/Full/window cases. `filesystem/native_wrapper_write_all_result`
    has neither a run suffix nor an explicit exit file, so the native leg only
    builds it. Supply execution and output assertions before claiming closure.
  - Distinguish honest proof/declaration fixtures from executable customers.
    `terminal_psi/content_custody_exit` now invokes `Root::enter`, but still has
    no `build.omg` or executable oracle. Finish its promised custody execution.
    `core/content_{conservation_contract,retained_custody_round_trip}` need an
    invoking customer for runtime conservation; routed establishment belongs
    to **BOUNDARY-ISSUANCE**. Keep `proofs/quotient_*_managed_compile` proof-only
    and add the separate witness owed by **QUOTIENT-RUNTIME-REALIZATION**.

  Remaining diagnostic acceptance:

  - **IMPLICIT-STORE-CONVERSION** owns duplicate conversion/domain-weakening
    reports. Preserve the primary subject in
    `calls/terminal_return_type_mismatch_rejected`,
    `expressions/bool_numeric_operand_mixing_rejected`, and
    `arithmetic/{suffix_type_disagrees,undeclared_bare_name}_rejected`.
    `range/indexed_field_range_runtime_write_rejected` also repeats its
    where-fact failure. Emit stable source subjects instead of arena IDs in
    `expressions/token_bound_machine_duplicate_shape_rejected` and quotient
    rejections; attribute `operators/selected_crash_invocation` to the authored
    operator invocation rather than only statement/call coordinates.
  - `build/uefi_program_entry_wrong_calling_policy` must reach the physical
    MicrosoftX64 policy mismatch. Admission currently selects boundary schemas
    by bare name in `compiler/build-evaluation/src/admission/selection.rs`, so
    the local `UefiApplication` collides with the bundled schema first. Resolve
    exact schema identity under **UEFI-PHYSICAL-SEMANTIC-ENTRY** and retain the
    intended negative; repinning it onto the duplicate-schema error would
    leave the policy check untested.
  - `providers/checked_boundary_operator_const_application_unselected` must
    reject for missing exact authored selection, alongside the selected
    positive. Its recorded diagnostic and `expected.txt` disagree; inspect
    the actual selection failure under **OPERATOR-BOUNDARY-CALLS**, rather than
    treating any rejection as the intended evidence.
  - Keep independent narrowing coverage in
    `arithmetic/narrowing_anonymous_expression_unproven`: an anonymous array
    element reaches the range obligation, while `narrowing_signedness_rejected`
    tests carrier mismatch. The duplicate-data guard-fact cascade is already
    repaired at both transition and statement sites and covered by
    `modules/duplicate_data_keeps_guard_facts`; retain that control.

  Close shared capability failures through their existing owners, rerunning the
  unchanged outer fixture after each repair:

  - **OPERATOR-BOUNDARY-CALLS** owns nested named/token arithmetic, selected
    const applications, float match body supply and policy-preserving float
    providers. Keep `float/{x86_fma_plan_association,
    build_runtime_semantics_twins_x86_baseline,
    build_runtime_semantics_twins_windows_x64,named_provider_directed_divide_exit,
    named_provider_fused_multiply_add_exit,named_provider_multiply_then_add_exit}`;
    `expressions/match_float_{interpretation,patterns,subjects}`; and
    `providers/checked_boundary_operator_const_application_alternate_exit`.
    `float/{float_saturating_arithmetic_exit,float_trapping_overflow_traps,
    float_trapping_propagated_nan_traps}` require their authored policies;
    `as f32` erases the subject. Exact-cast guards in
    `float/float_to_int_exact_proofs_exit` need one authoritative comparison
    recognizer for both Binary expressions and token-bearing calls, shared by
    float fact readers. Keep operator-contract negatives for invalidated
    operands and unproven indexing/subslicing rejecting for their own reasons.
  - Float field/index store admission already includes structural reads in
    `structural_scalar_store/value.rs::admits`. Verify the full
    `sensor_min_max` route, including independent storage-read replay and native
    float comparison emission; another checked guard fact alone is insufficient.
    **SAMPLE-CORPUS** retains this customer and the `F64::minimum` dependency.
  - **ENTRY-CONTENT-ROOTS** owns `control_flow/guarded_leaf_branch_expansion`
    and `capabilities/uses_caller_folder`: owned-array/capability custody at a
    boundary is distinct from shared-reference copies and from deliberate
    move-and-restore under **BORROWED-STORAGE-RESTORATION**. Preserve nested
    service nomination and exact authored Gui/Input/Console providers; a
    bundled provider for a same-named requirement is insufficient.
  - `capabilities/windows_provides_import_exit` must preserve the producing
    statement of `self.m = self.beeper.beep(self.v)` and execute on Windows.
    Follow `statement_continuations` in the attached Unit state-graph lowering,
    retaining the ordinary-call control and actual Binding/ForeignBinding
    package inputs. A macOS PeByName refusal cannot measure that execution.
    `providers/external_leaf_dllimport_compile` needs its checked per-call
    authority carried into native `NormalizedForeign`, not a fixture permission
    or hardcoded provider cohort. Preserve
    `filesystem/runtime_local_host_result_dispatch_exit` as the Linux
    source-effect ordering customer; missing host providers are separate work
    from lowering the selected calls.
  - `proofs/cauchy_predicates_compile` needs both capabilities documented in
    `source/library/core/cauchy.omg`: positivity from the static-call projection
    `(Middle(middle_index)).den`, and an entailment tier for machine-parameterized
    `converges_together_at` guarantees. **PROOF-CONTRACT-MIGRATION** owns that
    proof work; inlining a local does not supply either capability. Keep exact
    intermediate arithmetic bounds in `proof_inductive_climbing_sum` and its
    unbounded-accumulator negative even when a theorem uses `embed`.

  Preserve these distinct native customers and oracles (paths relative to
  `tests/omega/pass`):

  | Customer | Required behavior and owner |
  | --- | --- |
  | `text/runtime_local_struct_string_field_concat_exit`, `text/runtime_string_stored_suffix_exit` | Exits 188/193 with unchanged shared-view Room records. **NOMINAL-FIELD-FLOW** and **STATE-LOCAL-VALUE-FRONTIER** must carry UTF-8 field provenance across state/call boundaries and lower borrowed-view stores. Copy permission cannot establish qualification or extend a loan. |
  | `text/runtime_stdin_command_branch_exit` | Preserve the reader and owned Command result-to-field storage, then execute its input/branch oracle. **STATE-LOCAL-VALUE-FRONTIER** owns composition; reattribute the current omission instead of inheriting a historical phase label. |
  | `host/runtime_console_bounded_line_exit` | Matching-host result and mutable-window execution: LF, EOF, Full, repeated middle-window reads, zero-capacity non-consumption, exact count/reason exits and output, next unread byte and untouched tails. Restore coverage for the alternate `repeat` entry; discard-result execution is insufficient. |
  | `text/runtime_stdin_line_buffering_exit` | Both blocking `echo_line` calls, caller CRLF normalization and exact completed prefixes. **ENTRY-CONTENT-ROOTS** owns borrowed intrinsic-service receipt transport through `write_prefix`, calls and state edges; preserve the two-read/helper flow. |
  | `filesystem/native_wrapper_write_all_result` | On macOS ARM64, store and dispatch the write result: nonexistent parent gives Error, `/tmp` gives Ok, stdout contains PASS and no FAIL. Exit zero alone is insufficient. This is distinct from **FILESYSTEM-RELEASE-CONTRACT** proof closure. |
  | `host/runtime_gui_foreground_window_exit` | Windows, exact authored Gui provider: create, query/store foreground result, destroy, exit 70. An invisible window need not become foreground. |
  | `control_flow/runtime_{integer,string}_literal_dispatch_exit` | Exits 22 and 11 for `"look"`, with authored selection order and ordinary result/field storage. Integer dispatch has recorded native results; retain it as the control while completing string dispatch and the host matrix. **MATCH-SELECTIVE-LOWERING** / **STATE-LOCAL-VALUE-FRONTIER**. |
  | `wire/runtime_wire_exact_array_without_count_exit` | Generated `[u32; 4]` encoding without a synthetic count sibling: seven bytes `00 00 04 01 02 03 04`, exit 70, and checked-interpreter agreement. **WIRE-RUNTIME-AND-INSTALLATION** owns the generated operation; native execution remains part of this integration gate. |

  Route additional omissions to **STATE-LOCAL-VALUE-FRONTIER**,
  **MATCH-SELECTIVE-LOWERING**, **GENERAL-CYCLIC-EXECUTION**,
  **NOMINAL-FIELD-FLOW**, **BORROW-PROOF-CONVERGENCE**,
  **OPERATOR-MACHINE-SUPPLY**, **MODULE-NAMESPACE-RESOLUTION** or
  **ARITHMETIC-POLICY-REALIZATION** by the missing operation and evidence.
  Acceptance: complete roster reaches promised stages, negatives reject for
  their intended reasons, and exact runtime oracles pass on matching hosts.
  Preserve coverage guards and valid accepted-language programs; attribution,
  repinned refusals and checked-only substitutions do not close execution work.

- **SQUALR-HEADLESS.** Run the independently versioned
  [Squalr application](samples/apps/README.md) through its ordinary nested
  packages to native geometry, supplied-byte scanning and repeat filtering.
  Its board owns the essentially 1:1 Rust port; this item owns compiler
  integration and unchanged application acceptance.

  Restore the acceptance route in `squalr-tests/main.omg` and
  `squalr-tests/src/command_driver.omg`. The pinned entry skips expansion,
  ordering and string parsing, leaves the six scalar `FixtureKit` scan/repeat
  checks disconnected, and prints PASS after seeded snapshot/tombstone checks.
  Connect those checks and production new-scan, element-scan, repeat, delete
  and list execution before success. Direct executor helpers do not cover the
  privileged-command dispatcher. Keep exact address/value and RLE-range oracles,
  including no/all matches, tails, genuine empty input and overlapping candidates.
  The current `verify_empty` supplies nonmatching bytes; `verify_overlap` scans
  one-byte values at stride two. Neither establishes its named edge case.

  True multi-byte overlap also needs production raw-byte storage:
  `SnapshotRegion::read_current_elements` divides
  `(base_delta + position * stride)` by element width, repeating decoded values
  when stride is smaller than width. Preserve byte coverage separately from
  candidate stride and growable output. The command fixture's eight-byte-spaced
  oracle needs explicit `Alignment8`; retain production's `Alignment1` default.

  Restore the real foreign-package `Optional<T>` APIs for `ScanFunctionScalar`
  and `SnapshotRegionFilter`. Stage 04's `case_payload_transfer` in
  `execution/terminal_unit/state_graph/mod.rs` still only forwards an existing
  shared-reference payload to a shared successor; add the checked loan of a
  selected owned affine payload subtree. Boolean-plus-payload or package-local
  sum replacements do not close that customer. Ordinary marker-data namespaces
  need no language decision. Reproduce the restored application on the current
  compiler before assigning any further source-signature or structural-argument
  failure: old diagnostics and expired claims are not current blockers.

  Preserve Windows and Linux ARM64 geometry, `set_alignment`, parsing/refusal
  and error-text parity, native clone/serialization of case-bearing alignment,
  and token/named ordering agreement: order by base, equal-base order equivalence,
  size-sensitive `equals`. Apply settled assertion/build contracts to Rust's
  debug assertions. Windows byte output still needs an admitted native route:
  `HostedWriteByteI32Realization` selects `HostedIntrinsicBundle`, whose current
  profiles exclude Windows despite std's declared `write_byte` provider.

  Run `python3 samples/apps/squalr/tools/verify.py native --target linux_x86_64
  --timeout 600 --omega <binary>` with Python 3.11+ and ordinary package review
  on Linux x86-64; the pinned entry and filesystem/time providers target Linux.
  The harness checks the host and calls `omega run --keep` without a compiler
  `--target` flag, since that flag only emits. Acceptance requires actual exit 0
  with `Squalr geometry: PASS` after the restored checks, plus exact scan/repeat
  addresses and ranges matching pinned Rust and independent expected cases.
  Preserve the 17-package/37-edge layout and verification harness; record compiler,
  std, application and upstream pins and host. Layout checks, source checking,
  Terminal publication and cross-emission do not establish execution.

  Connect **VEC-NATIVE-GROWTH** to partitioned scan output and repeat filtering;
  **BUMP-ALLOCATOR-CANARY** and **PLAN-LAID-VIEWS** supply backing and placed access.
  Follow the application's next missing operation without waiting for every
  related task to close. Keep growable storage, shared snapshots and the full
  port contract; no Rust FFI scanner, fixed-capacity substitute, pull-only
  workaround or application-specific compiler behavior. Artifact production keeps
  [conditional loading premises](wiki/spec/build/component_publication.md#products-and-authority),
  without a fabricated runtime installation grant or mandatory Rust supervisor.
  **CHECK-CLOSURE-ONCE** owns repeated checking: reuse only exact inputs and
  authority receipts, keep live runs to a verdict, and record timeout separately
  from source rejection.

- **VEC-NATIVE-GROWTH.** (split-of:BUMP-ALLOCATOR-CANARY) Implement ordinary
  library `Vec<T>` for Squalr's growable scan results under the
  [allocation contract](wiki/spec/resources/allocation.md). Owners are
  `source/library/core/vec.omg`, `source/library/alloc/vec.omg`, and the ordinary
  compiler operations exercised by that source.

  Core's generic storage/construction surface remains empty. The alloc package
  now has a nongeneric `Vec` containing inline `u32` arrays of length 4, 8 and 16:
  growth copies between those fixed fields, rejection returns zero without
  requesting storage, and close clears tag/length without consuming custody.
  This is a bounded copying witness, not allocator-backed growth or indirection.
  `pass/collections/runtime_vec_u32_growth_exit` records macOS native exit 70,
  but manually copies backing fields and accepts repeated close. Its weighted
  failure exit also needs a verdict that cannot lose high bits at process exit.
  The duplicate-cleanup and invalidated-loan fail fixtures use separate local
  types; preserve their controls and exercise the actual library implementation.

  Acceptance: native `Vec<u32>` takes explicit backing, appends through two
  growths through allocation, verifies the preserved prefix and new elements, cleans up
  exactly once and returns backing under the allocator contract. Cover empty
  cleanup, real allocation failure preserving contents/custody, invalidated
  loans and duplicate cleanup. Transfer exact element custody and explicitly
  return or retain old backing; variable retained storage requires indirection,
  not an infinitely recursive inline record or compiler-owned vector/Arena.

  **BUMP-ALLOCATOR-CANARY** owns allocation/returned extents;
  **PLAN-LAID-VIEWS** owns establishment/access/retirement;
  **CONSERVATION-CONTRACT / TERMINAL-CONTENT-CLAIMS** owns content-preserving
  transfer; **BORROWED-STORAGE-RESTORATION** owns move-out/replace when needed.
  Keep the source program through checking, interpretation and native execution,
  then use that same implementation in Squalr.

- **MACOS-APPLICATION-PUBLICATION.** Publish and run `window_app`,
  `window_demo` and `windowed_calculator` under the
  [macOS application contract](wiki/spec/build/macos_application.md).
  Reuse `compiler/src/report/{package,compile_report}.rs`: assembly, retained
  signing identity and checked package/inner-executable accessors already exist.

  The samples now declare nominal GUI/clock/input providers, macOS
  `provider_defaults` selections and identifiers. Preserve those implementations,
  signatures, rendering and effects; missing explicit selections in `build.omg`
  are not evidence that these providers are absent. Complete any remaining
  general lowering through **ENTRY-CONTENT-ROOTS**, **TR3-TR8** and
  **STATE-LOCAL-VALUE-FRONTIER**, keeping one occurrence-owned provider receiver
  across calls and nested services. Fused authority is not receiver storage;
  do not substitute global state or weaken occurrence matching.

  Stage 05's `attached_unit/call_catalog.rs` still fences write-only stores
  and atomic events in opaque callable providers. Admit them only with a
  verifier-checked judgment preserving non-observation and exact access to
  places exposed by the requirement's mutable parameters. Existing scalar
  adapter/result-overload native fixtures exercise stateless providers, not
  these GUI receivers; reproduce each application's current first failure.

  Use [window_app's ordinary command/review flow](samples/gui/window_app/README.md),
  correcting its stale missing-provider diagnosis. `python3 tools/sample_gate.py
  --filter gui/window_app,gui/window_demo,gui/windowed_calculator --cli <binary>`
  supplies a focused check, not native acceptance. All three authored apps must
  publish validated `.app` output and execute on macOS ARM64 with observed
  windows/rendering, input/close behavior and completion (`window_demo`: 60
  frames, exit 0). Exercise Finder launch separately. A two-second process
  survival check or test-owned package acceptance does not prove these outcomes.

  Preserve identifier requiredness and signature/plist/retained-identity agreement,
  deterministic bytes, publication in a separate invocation without frontend
  state, checked root/inner-path correspondence, tamper/extra/partial-output
  rejection and unchanged flat outputs. The old `build_target_activation` and
  `native_filesystem_canaries` targets were deleted; restore their relevant
  customers through current gates, not commands for absent targets.
  Requested native `.proof` placement already exists; its bounded
  `Incomplete(UnsupportedEvidence)` ceiling belongs to **PCC-PRODUCT-PUBLICATION**
  and does not postpone bundle-placement checks. Record unavailable macOS runtime
  coverage. Resources and `image_viewer` bundle-relative lookup remain outside v1;
  do not silently change working directories.

- **CHECK-CLOSURE-ONCE.** (new-scope) Make a std-importing `omega --check`
  invocation check the library once across package discovery, binding and
  production. Owners: package-manager's
  [candidate compilation](omega-rust/omega/package-manager/src/review/candidate/compilation.rs)
  and compiler's `checked/checking/`.

  Exact-input check memoization, typed-frontier reuse and carrying unchanged
  package compilations already exist. `ImmutableBoundLookup` also shares one
  index per frozen program; constructing another lookup no longer rebuilds it.
  The remaining preliminary/settled split belongs to BUILD-EVALUATES-ONCE.
  Changed provider bindings currently change checked products: do not erase
  them from reuse keys to manufacture a cache hit.

  `compilation/tests.rs::bound_pass_compiles_only_the_occurrences_its_bindings_changed`
  carries one unchanged dependency and reuses typed frontiers for two changed
  occurrences, but still requests preliminary and settled checks for each.
  It does not prove the complete CLI/std invocation checks once. Keep source
  drift, exact request identity, per-pass custody and restricted-grant checks.
  Reuse must retain preparation outputs and component discovery and reconcile
  sponsored evaluation, including discarded attempts; carried evaluations are
  charged only to the session that ran them, with zero usage for an empty one.

  Acceptance: `omega --check --timings samples/cli/basics/print_squares/main.omg`
  plus an owner regression proves one actual whole-program library check per
  invocation. Use existing per-package/pass compilation, carry, frontier-reuse
  and check timing counters; distinguish memo requests from executed checks.
  Changed sources or authority must still revalidate. Profile residual costs
  against that current route rather than treating historical timings as a gate.

- **TERMINATION-RANKING-CHECKS.** Carry exact rank/endpoint evidence through
  ordinary calls, named states and native execution under the
  [termination contract](wiki/spec/language/termination.md). Owners: stage 04's
  `checks/termination/ranking/`, validation's
  `proof_contracts/contract_entailment/ranking_range/` and
  `machine_calls/call_cycles/runtime_ranking/`.

  Quotient/remainder, left/right shift and bitwise-AND actuals already use
  `install_nonpolynomial_terms`; `rank_ranges/{call_components,field_endpoint_arithmetic}.rs`
  checks exact substitution, nested operations, shift widths, signed floor-shift
  and two's-complement mask bounds. Ordinary callee requirements accept stable
  saved locals; recursive-component checks still reject local names in
  `meanings::builtin`. Extend that route to
  saved values with exact binding-time identity and write/exposure invalidation.

  Replace residual unique-carrier discovery limits with explicit arrival
  correspondence. Reuse existing nested carriers, moved scalar/slice/record
  copies and produced rank facts; a same-typed or merely decreasing copy is not
  the ranked value. Mixed ranged/unranged components need actual correspondence
  and endpoint conservation. Recheck formation at every arrival: cancellation,
  equal intervals or unoriented disequality do not prove divisor validity,
  overflow freedom or identity. The independent interval-only fallback still
  requires one state; it cannot bypass arrival proofs.

  Recover executable customers from `rank_remainder_endpoints` and
  `rank_endpoint_borrows` (deleted in `c60792d7bdd`) as ordinary corpus entries:
  a named-state countdown with reordered, preserved `cap % divisor` endpoints,
  and a countdown whose bound is reached through a stored exclusive reference.
  Require checking, interpretation and native result 0, including loops whose
  header differs from entry and projected/borrowed ranks beside other work.
  Checked fixtures and the deleted direct-recursive native test do not establish
  those outcomes. **STATE-LOCAL-VALUE-FRONTIER** owns general realization gaps.

  Preserve selected-signature write frames for boundary/requirement/admitted/
  external calls, and disjoint-frame checks for direct, nested and composed
  initializers; `prefix_let_call_bindings_keep_the_call_frame_bar` already checks
  requirement reads versus exclusive writes. Execute those cases through the
  same provider contracts. Changed endpoints, stale copies, protected-carrier
  writes, overflowing intermediates and nondecreasing cycles must reject.
  Retain the private-witness/public-guarantee split, exact subject/view/range
  identity, subordinate-call obligations and descent on every complete mutual
  call cycle. Use `python3 tools/corpus_gate.py --filter termination/` and its interpreter
  and matching-host native legs; checked-only records do not close this task.

- **CANONICALIZE-SCALAR-RANGE-CONTRACTS.** Complete the settled
  [proposition-based scalar range contract](wiki/spec/language/domains.md#declaration-and-membership)
  and [generic inference rule](wiki/spec/language/generics.md).
  Common fields use data `where`, payloads use case `where`, machine/state
  parameters use `requires`, results use `ensures`, and locals use facts
  established by initializers, guards and calls. Remove integer and float
  `T [lo..hi]` / `T [lo..=hi]` suffixes after their customers migrate.
  Ordinary ranges, slices, arrays and bracketed multiplicity remain distinct.

  Prioritize bundled-library, sample and Squalr customers that unblock complete
  programs. Finish each customer's proposition transport and execution before
  moving to the next group; parser deletion is the final migration step.
  A domain retained by a migration must have an independently meaningful
  nominal identity, semantic contribution or establishment route, explained in
  the change. Endpoint-named or interval-only replacement domains are invalid
  even when they compile and execute. Preserve independently used domain/proof
  improvements; equal predicates do not merge domain identities.

  Complete a history-derived declaration inventory using the
  [migration recipe](wiki/drafts/designs/range_suffix_migration.md), including
  follow-ups and unlisted edits made under the old blanket premise. Starting
  revisions: `8f93c866a3`, `2c3c2aec00`, `b9b68383c9`, `cc36490282`,
  `3531aaa098`, `e0a884138f`, `2ef59d385a`, `c4d2776b1f`, `c5621e66ac`,
  `fc0fc2dcab`, `c5688363ec`, `f1d6e31013`, `9e3355b562`, `db4f9f9140`,
  `2858366bdb`, plus sample migrations `24d91ed906` / `7917a536d4` and
  library migration `c1303f89d4`. Record original/current spelling, owning
  position (field, payload, parameter, result, local, generic shell or static
  use), and restoration, contract correction or independent reason for
  retention. Include bare fields whose bounds were removed because inferred
  loop bounds happened to make native execution pass. A suffix grep or passing
  replacement alone cannot establish preservation of the original contract.

  Reuse current canonical controls: `collections/runtime_argmax_index_exit`,
  `collections/runtime_nested_const_row_indexed_read_exit`,
  `arithmetic/runtime_guard_narrowed_transition_arg_exit` and
  `arithmetic/runtime_exact_guarded_shift_count_exit` use `where` /
  `requires` and have recorded macOS native outcomes. Named-state
  `rank_range_declared_step_copies` / `rank_range_call_declared_step_copy`
  also use `requires`; their old missing-arrival diagnosis is not a current
  assignment. `proven_predicates_grant_domain` in
  `checks/contracts/calls.rs` already admits supported predicate-only
  memberships from live statement-call/guard premises, with stale-write and
  routed-domain controls. Do not rebuild that route or weaken its refusals.

  Remaining migration and verification work, with capability repairs under
  their existing owners:

  - Carry field/case propositions through construction, nonliteral and indexed
    writes, reads/index proofs, calls, state joins, mutable recasts and returns.
    Preserve exact subjects, invalidation and invariant-window closure.
    **NOMINAL-FIELD-FLOW**, **RECAST-SOURCE-POSITIONS** and
    **STATE-LOCAL-VALUE-FRONTIER** own the respective transport/execution.
    Re-drive the canonical forms of `core/zii_default_composite_exit`,
    `collections/runtime_dutch_flag_partition_exit`,
    `calls/runtime_mutable_call_before_transition_args_exit` and the
    `recast/runtime_*_mutable_write_exit` family, plus the historical sample
    field migrations. Recorded failures now include later lowering stops;
    reproduce before attributing them to proposition checking.

  - Carry guarded immutable-local, guarded-field, nested-call-result and
    destructured-payload facts into ordinary `requires`; carry entry premises
    into arithmetic formation of `ensures`, and state results into the
    enclosing machine's postconditions. Customers include
    `calls/runtime_dispatch_binary_call_argument_exit`,
    `calls/nested_value_call_arg_compile`,
    `arithmetic/anonymous_rational_arguments`,
    `proofs/accumulator_guarantee_exit` and
    `termination/ranked_callee_projected_receiver_compile`.
    Some already use canonical clauses but still need end-to-end execution;
    the projected-receiver fixture still has result/parameter suffixes.
    **STATE-LOCAL-VALUE-FRONTIER** owns composed calls;
    **TERMINATION-RANKING-CHECKS** owns ranked-cycle evidence.
    A mutable parameter's `requires` constrains its arrival, whereas its old
    suffix was store-enforced: re-prove every dependent read after a write,
    using `termination/rank_range_mutable_bounded_endpoint` and negative
    mutation controls. Update `include_str!` fixtures and string-rewrite
    probes together in `tests/termination/rank_ranges/`; preserve their
    intended proof obligation rather than leave a replacement silently inert.

  - Finish the library leg: `source/library/core/numeric_conversion.omg`
    still publishes suffixed widening results; `std/console.omg` retains
    the `ByteRead::Byte` payload and state-parameter suffixes;
    `std/time.omg` retains subsecond fields/locals and constant constructions.
    Transport cast/postcondition, active-payload and constant-construction
    facts through the canonical clauses. Preserve the
    `host/runtime_console_bounded_line_exit` input/output/exit behavior.
    Migrate `core/layout.omg` fuel/index contracts, `std/calling.omg`
    indexed-field obligations and target-entry `field_range` parameters;
    prove computed `fuel - 1` / `index + 1` arguments and numerically
    equivalent guards such as `< 255` versus `<= 254`.
    Re-probe these migrations against current checking, not old failure totals.

  - Retain build-time admission and genuine negative behavior. The
    `layouts/runtime_plan_laid_erased_field_exit` customer still needs its
    cross-state index proof. Migrate `fail/wire/wire_policy_plan_disagrees`
    while preserving its actual plan/schema disagreement rejection; its
    recorded diagnostic already reaches that mismatch.
    `comptime/runtime_const_measured_recursion_exit` needs an inductive
    relational accumulator bound: `acc <= 5050` alone does not justify
    `acc + n` at the recursive call. Moving that bound into `requires`
    cannot repair the contract.

  Owners: Rust `type_syntax/parse_type.rs`, syntax/resolved/typed constraint
  representations, generic matching, checked contract/default-domain facts
  and their Terminal consumers. The suffix parser and
  `TypeConstraintNode::Range` remain implemented. Coordinate explicit
  binders/equations with **STRUCTURAL-GENERIC-MATCHING** /
  **RUNTIME-VALUE-GENERICS**; flow narrowing must never select specialization
  or layout identity. Runtime endpoints stay runtime and grant no const value,
  fixed extent or implicit maximum-sized allocation.

  Acceptance: migrate spec/guide/compiler documentation, bundled libraries,
  samples, product/compiler source (including `source/psi`), corpus and the
  Epsilon-written Omega parser `bootstrap/5_omega/parser.epsilon` with its
  `tests/bootstrap/omega-parser` fixtures. Preserve integer/float construction,
  call, write, join, return and invalidation behavior through checked source,
  retained Terminal and source-free replay, plus each customer's native oracle.
  Use `tools/corpus_gate.py --filter` and its `--native` leg; the ordinary
  gate may realize rooted fixtures but does not execute them. An explicit
  matching-host run is required for a runtime claim. Reject invalid bounds,
  unproved arguments, stale facts, incompatible carriers and runtime bounds
  used as static layout values. After the inventory is accounted for, delete
  suffix parsing, Range type representations, range-shell inference and
  suffix-recommending diagnostics; require suffix rejection in every type
  position, preserve the unrelated range constructs, and delete the recipe.

- **NOMINAL-FIELD-FLOW.** Unify receiver and named mutable-argument field
  obligations under [ordinary mutable-receiver contracts](wiki/spec/language/dependent_values.md#mutable-receivers-and-declared-field-domains).
  An established `&mut self` owes the same declared field/default domains as
  `&mut T` at call admission, entry, return and fact handback, including domains
  that reject zero. This is an implementation gap, not an unsettled design.

  Owner: `omega-rust/psi/pipeline/04_typed-trees-to-checked-trees`.
  `src/semantic/facts/field_domains.rs` still seeds receiver rows through
  `append_data_field_domain_facts`' zero-validity filter, while
  `append_state_parameter_domain_facts` skips `self` and seeds named parameters
  without that filter. `src/checks/contracts/nominal_inputs.rs`,
  `src/checks/contracts/exits/result_domains.rs` and `src/flow/call_phases/referents.rs`
  consume the split rows. Replace the receiver exception with shared declared
  obligations; retain zero-validity checks for actual ZII storage establishment.
  Nominal annotations cannot resurrect facts retired by mutation.

  Transition argument transport already exists in `src/flow/state_values.rs` and
  `src/flow/state_values/fields.rs`: live literal, byte-predicate and integer-bound
  facts rebase onto named parameter storage and join across incoming edges.
  Incoming glyph bounds also reach `dungeon_render`'s `put(ch: u8)` path.
  Verify the complete sample using these routes. A byte store preserves `Utf8`
  only with the necessary carrier/byte evidence: `ValidUtf8` alone does not
  imply `AsciiOnly`.

  Acceptance: paired method/free-machine cases over the same established
  `&mut T` agree for nested non-ZII domains, valid mutation and return through
  the shared path. Invalid arrivals, unclosed invariant windows and stale aliases
  reject at calls, transitions and returns. Extra input-only `requires` are not
  implicit postconditions; preserve explicit `ensures`. Verify equally initialized
  ASCII carriers and guarded stores through receiver and named transition
  arguments, retaining the poisoned-edge/missing-fact controls in
  `tests/element_field_flow_probe.rs` and the join/mutation controls in
  `src/tests/contracts/state_field_values.rs`. Preserve indexing, view, copy and
  whole-extent controls in
  `src/tests/contracts/{element_fields,nominal_parameter_fields}.rs`;
  require exact finite candidate-origin coverage, never universal coverage from
  unresolved selectors.

  Re-drive `omega --check --target linux_x86_64 samples/cli/games/dungeon_crawler_cli/main.omg`
  without bypassing `RoomLookup`, `MazeBuilder` or game-state field validity.
  Coordinate canonical range migration with **CANONICALIZE-SCALAR-RANGE-CONTRACTS**,
  explicit policy casts with **IMPLICIT-STORE-CONVERSION**, and any `&mut Room`
  output migration with **WRITE-ONLY-BORROW**. Constrained nested `&write`
  forwarding already has checked-source coverage in
  `src/tests/borrow/write_only/write_only_subloans.rs`'s
  `qualified_field_write_only_subloans_carry_exact_atoms`; preserve it and verify
  the customer's actual stores. Preserve valid ZII locals without initializers;
  arbitrary incoming storage gains no ZII facts. Migrate revoked test syntax
  without weakening negative controls. **SAMPLE-CORPUS** retains complete sample
  publication and runtime oracles after checking closes.

- **GENERAL-CYCLIC-EXECUTION.** Complete general
  [cyclic control](wiki/spec/terminal-psi/control_flow.md) and
  [safety/progress checking](wiki/spec/language/termination.md) through native
  execution. Owners: Terminal verification's `validation/control_flow/`, stage
  04 `execution/terminal_unit/` and stage 05 `unit/attached_unit/` and expression
  preparation. The same-name optimizer-board task owns native receiving/replay.

  `tests/omega/pass/text/runtime_number_to_decimal_exit` and
  `runtime_bounded_carrier_write_read_exit` already execute with exit 70 on
  macOS ARM64. Projected byte-field reads, cyclic writes and the decimal
  divisor invariant are implemented; preserve these controls on the hosted
  matrix. The separate `domains/utf8_equals_literal_exit` still rejects at
  `Main::classify`'s conditional guard, without a cycle.

  - Replace remaining `unranked_cycles.rs` fences with checked arrival/custody
    relations: broader qualified/partial ownership and structural results,
    moving claims, content reshuffles/partitions, projected call arguments and
    boundary completion receipts. Plain case/record results, pinned entry
    claims and several effectful calls already pass eligibility. Eligibility
    supplies no proof; verify every arrival before removing a fence.
  - Verify unchanged `print_squares` through ordinary package review and native
    execution: `--check` passes; an isolated macOS `omega run` stops at missing
    package acceptance. Establish complete composition of projected helpers,
    Console operands, indexed/aggregate writes and computed results before
    assigning another lowering repair. **STATE-LOCAL-VALUE-FRONTIER** owns transport;
    **NOMINAL-FIELD-FLOW** and **CRASH-CONTRACT** own facts and call contracts.
    **SAMPLE-CORPUS** retains nine computed rows ending in `081`, exit 0.
  - Generalize independently proved invariant proposals beyond
    `proofs/scalar_block_invariants/lockstep.rs`'s divisor/counter recognizer;
    avoid one recognizer per customer or source-state duplication.

  Acceptance: unchanged programs run across the hosted matrix with exact
  caller/callee resources and ranking where declared. Preserve dominance,
  successors/frontiers, current-iteration guards and fuel suspension/resumption;
  corrupt arrivals, custody, effects and proof groups reject. Stage 05's
  `tests/owned_scalar_cycles/` covers rebound fields and custody mutations.
  Safety remains independent of termination or a test's fuel allowance.

- **CRASH-CONTRACT.** Complete invocation-specific crash obligations through
  selected operators, nested values, calls, cycles and package review. Owners:
  stage 04 `facts/{operator_crashes,crash_entry_values}.rs`,
  `CrashPlan::checked_operators`, `flow/expression.rs`, stage 05
  `retention/operation_crash_contracts.rs` and Terminal/native consumers.
  Preserve selected requirement, saved actuals, Match arm and surviving route;
  traps or opaque provider bodies cannot narrow authored contracts.

  - Generalize selected-use/operation joins beyond IEEE/integer comparisons.
    Named uses still reject before lowering; structural telescopes and call
    carriers need exact operand mapping, provider commitment and independent
    Psi/Omega replay through one occurrence relation. Scalar IEEE equality/
    inequality guards exist; float ordering, generic and structural-formal
    routes still need portable predicates, preserving the original guard.
  - Finish invocation-outcome trace/refinement under
    [reconstructed observations](wiki/spec/terminal-psi/observations.md#reconstructed-rows):
    actual outcome, substituted guards, abandoned claims, staged writeback and
    no-result/no-cleanup behavior. Stage 05 `tests/scalar_boundary_arguments.rs`
    already verifies and interprets scalar boundary outcomes; its structural
    guard control rejects. Native call continuations exist, but retained crash
    rows alone do not establish executed outcomes.
  - Extend ordered completion beyond supported scalar contracts, closed integer
    ranges and mutable Boolean snapshots to remaining qualified results,
    state/control contracts and field/arithmetic guarantees. Reuse
    `tests/unit_scalar_result_source/boundary_wrappers/`; preserve transitive
    contracts and exact entry versions across joins.
  - Complete package crash-expression projection in
    `review-evidence/src/capture/behavior/crash/expressions.rs`: indexed scalar
    reads, byte lengths and integer casts currently return `None`. Verify
    remaining generic-field and case-payload combinations through source and
    canonical recovery. `tests/callable_policy/case_membership.rs` already
    covers authored/indexed membership and projected result tags.
  - Extend entry provenance for opaque/content leaves, dynamic indices and
    whole-collection reads after writes with proven origins. Fixed-index/range
    disjointness exists; divergent arrivals and unknown cycles stay conservative.
    Share snapshots with **STATE-LOCAL-VALUE-FRONTIER** and
    **MATCH-SELECTIVE-LOWERING**.

  Acceptance: extend checked-only `tests/omega/pass/operators/crash_routes`
  and float controls through independent Terminal replay, execution and package
  review. Safe uses discharge routes; changed guards, captures, substitutions,
  sites and stale writes reject. Proof-only terms cannot authorize executable
  guards; legacy propositions remain **PROOF-CONTRACT-MIGRATION** scope.

- **ARITHMETIC-POLICY-REALIZATION.** Close remaining
  [numeric-policy](wiki/spec/language/numeric_values.md) realization and full
  program acceptance. Owners: stage 04 scalar checking, stage 05 expression
  preparation, Terminal `validation/trapping_integer.rs` and its observation,
  interpreter and native consumers.

  Mixed fixed-integer comparisons already preserve both carriers through
  checking, Terminal and native execution. The corpus's
  `expressions/mixed_integer_comparison_{widths,places}` and
  `mixed_integer_guard_comparison` execute on macOS ARM64, as does
  `core/numeric_cross_signed_conversion_surface`. Wrapping/Saturating casts and
  operation-level Trapping arithmetic exist; do not repeat those milestones.

  - Support guarded Trap ceilings for Trapping primitives. A source
    `u16 -> u8` conversion with `crashes Trap input > 255u16` rejects with
    `TrappingIntegerCrashUncovered`; `validate_trapping_crash_sites` requires
    `[CrashRouteGuard::Truth]`. Reconstruct the exact primitive's trap condition
    under incoming path facts and prove coverage by the authored same-cause
    guard; the false ceiling `input > 256u16` must reject. Preserve its operation
    site, operands, carrier, cause and normal-only
    result equation through observation, interpretation and native realization.
    Coordinate **CRASH-CONTRACT**; never replace the policy or invent a Crash edge.
  - Finish the five unchanged cursor/length customers under **SAMPLE-CORPUS**:
    `longest_run`, `parse_int`, `roman_numeral`, `string_hash`, `substring_search`.
    All five pass `tools/sample_gate.py`'s source check; establish reviewed
    native execution and diagnose any later failure while keeping their
    algorithms, declared cursor types and documented outputs.
  - Complete hosted acceptance for the public
    `source/library/core/numeric_conversion.omg` surface, including same-sign
    narrowing, sign changes, widening and all four policies. Only two
    `core/numeric_*` fixtures survive from the former six-canary surface.
    Surviving stage 05
    `tests/{integer_policy_realization,signed_wrapping_conversion_values}.rs`
    cover selected compositions and interpreted values, not every sign/width
    pair natively. Preserve current success and trap fixtures, including dead
    results and overflowing shifts; `tests/trapping_operation_sites.rs` owns
    exact-site and altered-cause/carrier/ceiling controls.
  - Keep distinct failures with their actual owners. Float policy/conversion
    customers remain **OPERATOR-BOUNDARY-CALLS** / **FLOAT-PROVIDERS** work:
    `float/float_trapping_*` and `expressions/runtime_numeric_cast_exit` currently
    stop at field-store scalar-expression construction. The float Trapping
    fixtures no longer reproduce their recorded policy-weakening diagnostics;
    preserve overflow/propagated-NaN traps without erasing Trapping.
    `types/runtime_addr_field_exit` asks for `addr -> i32 in Trapping`, outside
    the current fixed-integer denotation. Resolve that conversion under
    [address semantics](wiki/spec/language/counts_and_addresses.md), preserving
    address-field behavior rather than assuming `addr` is `u64`.

  Acceptance: comparisons and conversions agree across source evaluation,
  verified artifacts and matching-host execution. Preserve all six relations
  and operand orientations, constants and dynamic operands, differing widths,
  negative values versus unsigned zero/positive values, equality and extrema
  above `i64::MAX`. Retain negated guards, selected meanings, literal landing,
  once-only ordered evaluation and independent overflow obligations.
  `fail/slices/signed_slice_index_upper_only` must still reject: an upper bound
  supplies no nonnegativity proof. Coordinate library range-suffix migration
  with **CANONICALIZE-SCALAR-RANGE-CONTRACTS** without weakening these obligations.

## Automatic service reach

Finish portable coverage of
[static callback reach dependencies](wiki/spec/language/effects.md#static-callback-reach-dependencies).
The source-side finite-union inference, closed application retention, selected
contract bounds, and two-sided emitted-call replay already exist. Remaining:

- Retain the original dependency and full binder telescope when callees are
  inlined or absent from the emitted machine set. The current
  `prune_incomplete_closed_reach_applications` drops incomplete annotations
  and their dependents; preserving ordinary executable semantics this way is
  not completion of dependency coverage or proof of source correspondence.
- Replace frontend-local template identities with independently checkable
  original-contract projection evidence. Producer-side replay against the
  retained source graph is not source-free verification, and commitment hashes
  identify a projection rather than authenticate its derivation. Reuse exact
  specialization/contract identities and the PCC ledger/bridge work; do not
  invent a parallel selection identity or a second proof system.

Owners: `05_checked-trees-to-lowered-psi/src/retention/closed_reach_applications.rs`,
`omega-rust/psi/checking/src/machine_calls/static_machine_call_contracts.rs`, and
`terminal-verifier/src/validation/reach_applications.rs`. Keep ordinary semantic
replay distinct from optional producer-correspondence certification; neither
test coverage nor matching hashes justify a full generic-PCC claim.

Acceptance: publish, discard source/producer state, reload, and independently
check the same named traversal with pure and Console callbacks, nested/private
helpers, recursive components, selected generic schemas, and unused selections.
Inlined/missing callees must not silently remove requested evidence coverage.
Reordered calls and equivalent helper extraction preserve interface identity;
adding helper reach changes it and invalidates stale evidence. Reject changed
projections, redirected same-shaped calls, incomplete telescopes, missing direct
boundary declarations, and selections exceeding requirement bounds. Opaque
calls retain conservative contracts; suspension/blocking remain separate.

Keep the existing `service_reach_contracts` lowerer and codec regressions as
controls, with the
[nominal callback](tests/omega/pass/effects/nominal_callback_dependency/README.md),
[generic schema](tests/omega/pass/effects/generic_callback_schema_reach/README.md),
and [linear structural callback](tests/omega/pass/effects/structural_callback_reach/README.md)
customers. The pure callback's ordinary callers and hermetic evaluation must
see its specialized empty row without consuming the original generic definition.

## Semantic reflection

Connect [semantic reflection](wiki/spec/language/reflection.md) to authored Omega
and ordinary checked calls through Terminal replay. The schema, scoped-selection,
and visitation-plan helpers in
`build-time-evaluation/src/machine_execution/reflection/` exist, but their
construction/composition callers are Rust tests rather than a source-driven
reflection route. Reuse them where their contracts fit; helper-only tests do not
complete this task.

First deliver an inspector and serializer through the same source-level query,
policy evaluation, and per-member call mechanism. Psi owns elaboration and
checking; ordinary library code owns encoding/inspection policy. Derive query
authority from the actual lexical/package context, not a caller-supplied authority
flag. The selection helper already compares demanded type/lifetime applications.
Complete its source-driven callable-contract checking, including the attached
requirement route in `selection/requirement_resolution.rs`; preserve exact
qualifications, generic arguments, lifetimes, and explicit selection.
Generate real field subloans, fresh context reborrows, active-case dispatch, and
ordinary call/resource evidence; a list of resolved operation names is not that
execution plan. Freeze only owned evaluation results and retain their dependencies
for independent replay.

Then complete recursive derivation, explicit runtime metadata/adapters, and
authorized Placed access under the same contract. Use exact pending derivation
identities without assuming their obligations; runtime recursion needs provisioned
work storage. Fixed named callbacks can proceed now. Selection-dependent evaluation
uses Automatic service reach; anonymous machines and format-specific compiler
operations are not dependencies.

Staged reflection generates ordinary field calls that can run at runtime. Explicit
metadata is owned descriptive data; retaining it does not implicitly retain
getters, setters, factories or every known type. Exercise borrowed adapter custody
without requiring static lifetimes. Nonaddressable packed/fragmented fields reject
borrowed visitation unless an explicitly selected copied-read operation supplies
the contract. Schema enumeration performs no device access; Placed visitation
must use exact authorized operations.

Acceptance: compile and run the inspector/serializer from Omega source, publish
and reload their Terminal product, and reject corrupted retained evidence.
Use a 40-field/five-type record with reusable type rules and one member override;
zero/multiple/wrong-type/stale-key/insufficient-contract selections reject.
Exercise owner-delegated visitation across three packages and reject unauthorized
enumeration. Include qualified and erased fields, borrowed owned claims, zero-sized
fields, empty records, distinguishable nullary cases, common fields once, and
active payload only. Test loan escape rejection, dynamic-index relationships,
recursive reference types, snapshot independence, and explicit exhaustion rather
than truncation. Descriptions never create grants or access; adapters, decoding,
and editing still require ordinary authorized access/construction contracts.

## Scoped build execution

Implement [scoped build execution](wiki/spec/build/scoped_execution.md) for
ordinary code generators and the topology composition customer. These are
accepted contracts, not claims of implementation. Existing
`BUILD-ADMISSION-CHECKPOINT` owns retained frontend/generated-source replay;
extend that route, not a second build language or plugin executor. Any semantic
or trust amendment found here or later goes through [owner questions](OWNER_QUESTIONS.md).

- **BUILD-PRODUCT-REFERENCES.** Finish executable follow-through for
  [non-executing product selection](wiki/spec/build/scoped_execution.md#selecting-product-declarations-without-executing-them).
  `checked-interpreter`'s `build_machine/product_entries.rs` already enforces
  lexical product visibility and slot compatibility; `build-evaluation`'s
  `admission/selection/root_bindings.rs` retains the selected symbol and slot.
  Preserve [separate checked contexts](wiki/spec/build/scoped_execution.md#two-checked-contexts)
  and **BUILD-ADMISSION-CHECKPOINT** source custody, never durable authority from
  names or evaluator indices.

  Register the missing scalar/multi-state customer in `tests/omega`: the cited
  `build_target_activation` Rust suite was removed. Select each module's
  `Runner::run` as ProgramEntry and observe its provider's computed result
  (**11** versus **37**) through ordinary calls and the state branch. Selected
  identity must survive product descriptions, helper returns, Terminal production
  and native execution with explicit exit oracles. **STATE-LOCAL-VALUE-FRONTIER**
  owns scalar/provider and state-call closure; **ENTRY-CONTENT-ROOTS** owns receiver
  attachment.

  Retain source controls for qualified paths, computed receivers, named lifetimes,
  parent loans and conflicting later operands. Reject unauthorized private/sibling
  enumeration, stale/forged descriptions, wrong scope/target/slot, swapped or
  ambiguous declarations, description-to-callable conversion and same-build
  generated/layout cycles. Explicitly delegated private entry descriptions remain
  usable without enumeration authority.

- **BUILD-SNAPSHOT-OUTPUTS.** Finish coherent capture and host acceptance under the
  [captured-input and committed-output contract](wiki/spec/build/scoped_execution.md#inputs-and-default-filesystem)
  across ordinary compilation. `package-compilation/src/source_snapshot.rs`'s
  `require_stable_recapture` and `source-acquisition/src/tree/capture/` recheck
  observed trees; matching observations do not make a mutable tree atomic. Establish
  the exact capture/staging/cleanup isolation premise or refuse. Cover directory
  membership/link races and edits restoring observations, preserving logical path
  distinctions, inert links, retained parents and failure cleanup. Planted-alias
  controls do not cover every mutation interval. `FilesystemSponsor::create_private`
  relies on inherited directory isolation on Windows; Unix-only identity controls
  leave that host's behavior unverified.

  Reuse `build-evaluation::execution::output_custody` settlement, `build-output`
  custody and compiler publication, with **BUILD-ADMISSION-CHECKPOINT** and
  `PackageCheckedContext` source/occurrence identity. The acquired-generator test
  in `omega/tests/completed_build_outputs.rs` already exercises narrowed template
  reads, exact artifact/companion output, native invocation, retries and distinct
  package/target output sets. Re-run that customer on macOS and Windows; preserve
  `omega/tests/{build_input_inventory,package_commands/snapshot_outputs}.rs` and
  package-manager's `suite` filter `build_named_inputs::`. The cited compiler
  snapshot/named-input test targets were removed.

  Acceptance also covers negative lookups/metadata, substitution/link escapes,
  sealed mutation, cross-occurrence receipts, omitted outputs, interruption and
  final-check failure without publishing a partial set. Measure retained state
  and compare the spec's separate-tool route. No new executor, live-host grant
  extension or persistent writable cache.

## Process-exit contract

Finish the [canonical process-exit contract](wiki/spec/language/process_exit.md)
through source-produced Terminal Psi and independent replay. Core/std declarations,
checked-interpreter canonical recognition and selected native exit custody exist.
Terminal's [terminator](omega-rust/psi/representations/terminal-psi/src/terminal_module/control_flow/termination.rs)
has no external-completion variant; the
[trace codec](omega-rust/psi/representations/terminal-psi/codec/src/sections/terminal_trace_v1_profile.rs) writes zero external rows and
rejects a nonempty roster. A native nonreturning operation does not fill this gap.

- Close native customer gaps using existing `SelectedProcessExit` custody.
  [Normalized foreign-call custody](omega-rust/omega/backend/images/image-emission/src/object_artifact/normalized_foreign_calls.rs) still accepts
  scalar homes only from call-result producers; finish general computed-status
  argument transport and independently replay its selected source. Windows
  `ProcessExit` already selects kernel32 `ExitProcess` through `DllImport`.
  Complete the Console byte leaves through `WriteFile`/`ReadFile`; their current
  `CompilerIntrinsic` declarations have no Windows hosted bundle. Replace the
  fixed 256 KiB foreign-call reserve with per-callee stack evidence in Omega
  stage 02's [source-evaluated imports](omega-rust/omega/pipeline/02_abstract-operations-to-target-operations/src/provider_admission/source_evaluated_imports.rs). These native
  repairs need not wait for portable external-completion representation.
- Retain exact canonical requirement, domain-bound authority, status and a closed
  terminal invocation without a normal result/successor through checking,
  lowering, Terminal, codec, verifier, observations and interpreter. Neither Console reach nor provider
  selection grants exit authority. Preserve exact semantic `i32` status and
  preceding output independently of the host's physical status mapping.
- Conditional helpers retain results, cleanup and postconditions on returning
  paths. Exit paths abandon in-domain obligations without discharge receipts or
  a post-exit frontier. Automatic cleanup must return; normal root completion
  must settle/transfer outstanding task custody. Survivor crossing contracts
  must account for domain death without a whole-process resource census.

Acceptance: serialize and independently replay unconditional and conditional/helper
exits. Tampered identity, arguments, completion or provider evidence reject, as do
missing authority/reach/progress, lookalikes, returning/aborting substitutes,
cleanup exits and unsupported survivor contracts. Reuse the
`tests/omega/pass/host/process_exit_i32_status_{mapped,ordered}` customers; execute
on matching hosts and retain ordered output/status oracles. Interpreter exit ends
only its simulated domain. Cross-target emission is not runtime evidence.
**TWO-AXIS-TERMINAL-AUTHORITY-REVIEW** owns further receiver admission; no general
completion syntax or other terminal service is a prerequisite.

## P1 - Authority, roots, and entry

- **ENTRY-CONTENT-ROOTS.** Complete provisioned receiver activation/completion
  under [entry roots](wiki/spec/build/entry_roots.md) and
  [binding validity](wiki/spec/build/component_publication.md#bindings-and-era-entry).
  Reuse opaque affine `Binding<R>` in `source/library/core/binding.omg`;
  `ForeignBinding` is a separate locator. No authored `Bound` qualification,
  source-created binding or provider lookup repairs missing establishment.

  - Complete borrowed `Binding<R>` transport. Psi checking's
    [carrier placement gate](omega-rust/psi/checking/src/value_custody/type_references.rs)
    still rejects references to the carrier.
    Make `capabilities/uses_caller_folder` lend the caller's field to
    `Librarian::archive(folder: &Binding<Folder>)`; promote
    `fail/providers/service_borrowed_parameter_rejected` when checking,
    Unit/Terminal calls, selected dispatch, interpretation and native service
    effects work. Moving an affine binding from borrowed storage without
    replacement must still reject. Provider multiplicity, conformance and Fused
    erasure do not change declaration-owned affine semantics.
  - Complete activation/result/out-parameter joins while preserving exact closed
    requirement, occurrence and selected-plan validity through erased fields,
    Terminal and native replay. Reuse `program-entry-plan`, Psi's
    `terminal-production` entry receipts, Omega stage 00's `entry_settlement`
    and `native-artifact::publish_fragment_object`. Intrinsic service validity
    is separate from ordinary authorized domains; **DOMAIN-ISSUER-ROUTES** owns
    their result/out-parameter evidence. Migrate obsolete bare-carrier and
    qualification fixtures without permitting fabrication; unused declarations
    alone demand no provider.
  - Connect ordinary nominal cleanup and callback/signal occupancy to actual
    receiver completion. ZII/aggregate eligibility, hosted bridges, a retained
    cleanup marker and `ProgramLocalRootInstallationLedger` already exist.
    Marker propagation and Rust-ledger retirement tests do not execute an
    authored destructor. Preserve image-emission's
    [hosted receiver tests](omega-rust/omega/backend/images/image-emission/tests/artifacts/hosted_receiver.rs)
    replay controls; installation owns live occurrence custody. Compose
    application, bridge, provider and callback stack demand, not loader stack-size
    metadata or another storage profile.

  At `535d6ad19e9`, `python3 tools/corpus_gate.py --native --filter
  pass/capabilities/acquires_through_helper_return` builds/publishes the fixture's
  actual `Main::main` on macOS ARM64. That harness does not execute this fixture.
  Run the original nested binding/helper-return chain and verify propagated
  acquires/provenance.
  Published `number_guess`, `cli_mvp` and `generic_counters` must retain their
  documented behavior without test-supplied receivers/qualifications;
  **SAMPLE-CORPUS** owns full sample and host coverage.

  Reject missing/incompatible supply, forged zero/literal construction,
  lookalikes, redirected continuations, non-ZII state, invalid backing/alignment,
  overlapping partitions and stale occurrence/epoch. Forwarding keeps distinct
  applications and affine custody; erasure removes neither initialization nor
  cleanup. Targetless checks select no physical entry; deployment cannot replace
  the physical adapter with a semantic continuation. Descriptor-call customers
  retain their dependency; Fused migration does not wait for Independent installation.

- **CONSERVATION-CONTRACT / TERMINAL-CONTENT-CLAIMS.** Execute nonempty content
  claims through source, Terminal, selected providers and native execution
  under [content conservation](wiki/spec/resources/content_custody.md): partition
  an established input under an authored theorem, pass the exact residual to a
  boundary, retain/return the rest and consume it in the caller. Cover direct
  calls and checked wrappers with canonical `source/library/core/content.omg`
  identities.

  `tests/omega/pass/core/content_conservation_contract` already invokes
  `partition_and_retain`, but its build selects no `RegionSplit`/`RegionSink`
  providers; the checked-in macOS native outcome rejects those missing `Fused`
  selections. Supply executable providers and an observable native oracle.
  The fixture currently has neither a run-tier/`_exit` name nor
  `expected_native_exit.txt`, so merely reaching `built` would not execute it.
  `core/content_retained_custody_round_trip` still has an empty entry: its
  `Owned`/`Retained` issuer cycle needs an initial claim from
  **BOUNDARY-ISSUANCE**. Existing-input conservation can proceed independently.
  The two `pass/terminal_psi/{content_custody_exit,structural_content_passthrough}`
  fixtures use local `Content`/`CountedQuantity` lookalikes and lack build
  entries; they do not establish canonical content execution.

  In Psi stage 04, `checks/content.rs::compose_partition_wrappers` already
  derives transitive and staged-result compositions. Stage 05
  `src/proofs/content_conservation.rs::lower_content_partition_compositions`
  rejects nonzero `source_derivation_depth` and nonempty `result_rewrites`.
  Carry exact claim mappings through those forms into Terminal replay and
  custody checking. Reuse the existing normalized equations, identity
  reshuffles and direct authored-theorem replay. Stage-04
  `content_reshuffles_and_partitions.rs` tests checked facts; stage-05
  `tests/content_conservation.rs` and Terminal verification's
  `tests/straight_line/guards_and_partitions.rs` use constructed evidence.
  None closes the source-to-native route.

  Omega stage 00's `src/provider_installation/replay.rs` currently requires
  whole-root content arguments and accepts structural results only when
  affine, without claims or projected qualifications. Complete the exact
  provider/caller content join needed by partition results and projected
  residuals. Its `tests/provider_installation/projected_claims.rs` exercises
  ordinary projected ownership claims, without content projections. Core
  supplies proof vocabulary, not an executable splitter: implement the
  partition primitive with its theorem; admit provider custody acceptance,
  never residual arithmetic. Fresh equal-sized literals do not establish
  conserved ownership or discharge the original input.

  Acceptance: source/native execution preserves exact subject/revision,
  projection/algebra, geometry, lineage, route and installed occurrence through
  independent replay. A partition theorem is usable only after its exact
  successful invocation; provider custody acceptance is not residual arithmetic.
  Reconstruct each introduction/admitted issuance and account for every
  exit/residual. Overlap, gaps, wrong projection/unit/lineage, stale invocation,
  substituted theorem/arguments and authority inferred from scalar totals or
  fingerprints reject. Keep unrepresented runtime-indexed owned extraction
  rejected. Verify the invoked canonical route with
  `python3 tools/corpus_gate.py --native --filter core/content_conservation_contract,core/content_retained_custody_round_trip`
  and require the authored runtime result, alongside independent replay.

- **BOUNDARY-ISSUANCE.** Join source issuance evidence to provider planning,
  native settlement and the exact installed occurrence under
  [external roots and issuance](wiki/spec/resources/authority.md#external-roots-and-issuance).
  First customer: give `tests/omega/pass/core/content_retained_custody_round_trip`
  an authorized initial claim, invoke its submit/complete cycle and require an
  observable native result. Its current entry is empty and the two issuer
  routes form a cycle; an annotated `Buffer` cannot establish the first claim.

  Reuse Psi stage 04's `checks/content/{retained_custody,call_results}.rs` and
  qualification evidence: authorized bare-result and selected-case grants
  already check, and consumed input cannot count again as fresh supply.
  Complete the content join beyond those facts and Terminal route validation.
  In `compiler/provider-planning`, `service_schema/mod.rs` derives result
  claims containing only domain/carry, while `provider_planning/receipt_binding.rs`
  binds granted requirement identities to report receipts. Neither carries
  per-invocation geometry, backing or live supply. Carry the exact evidence
  through provider admission and native settlement, preserving every
  authorized source spelling and result path.

  Reuse finite installed-parameter schemas from Psi stage 05's
  `unit/attached_unit/catalog.rs` and the occurrence/capacity/epoch ledger in
  `external-roots/src/program_local/program_local_roots/`. Its
  `tests/program_local_epochs.rs` constructs runtime subjects in Rust;
  `tests/interrupt_entries.rs` checks acknowledgement receipt ingress.
  Those controls do not execute source-issued content. Wire real entry/provider
  observations into the exact installed account. **DOMAIN-ISSUER-ROUTES** owns
  route selection; **AP-BRINGUP** owns hardware arrival execution. Existing-account
  conservation remains independent.

  Derive geometry from exact arguments, callable-entry places and result paths.
  Admit backing/freshness premises, never interval arithmetic. Retain common
  custody roots for shared backing and explicit alias classes. Multiple new
  results require one separated supply relation; transferred input is not new
  capacity. Preserve the finite installed-parameter alternative to provider
  issuance; neither needs a source-visible receipt binder.

  Acceptance: source-issued content retains geometry, backing, issuer, lineage,
  route and installed occurrence through independent replay. Forged source
  construction, foreign/replayed receipts, substituted geometry and duplicate
  supply reject; legitimate issuance and identity-preserving transfers succeed
  without re-minting capacity.

## P2 - Materialization and placed access

- **PLAN-LAID-VIEWS.** Complete executable placed-view establishment, access
  and retirement under [placed access](wiki/spec/resources/placed_access.md#establishment-and-retirement).
  Start from `tests/omega/pass/layouts/placed_view_source_access_compile`:
  its `inspect` writes `transmit` and reads `status`, but `Main::main` is empty
  and `build.omg` declares only a package. Bind an admitted backing, invoke
  the accessors and require an observable runtime result. Psi stage 04's
  `types/mod.rs::add_placed_accessor_attachment` already admits the generated
  opaque attachment through its exact placement-plan row.

  Direct-entry admission already rejoins roster, referent type/path,
  qualifications and aliasing; interpreter completion releases those loans.
  Reuse stage 00's `artifact_admission/establishment.rs`,
  `terminal-interpreter/src/structural_inputs/placed_views.rs`, `ArtifactSections`
  and codec/optimization custody. Existing interpreter tests construct a
  return-unit module; they establish loan binding/retirement, not source access.

  Native publication still rejects nonempty bound establishments in
  `native-artifact/src/fragment_object_publication.rs`; `entry_receiver.rs`
  also rejects them when no receiver bridge survives. Extend the entry bridge
  to lend each exact referent through invocation and retirement. Stage 02's
  `lowering/placed_view_inputs.rs` supplies only a direct-entry pointer ABI for
  a parameterless, one-block Unit function, without access events. Join the
  selected placement and derived accessor operations into the normal native
  pipeline and preserve independent replay. Both native input admission and
  interpretation still reject non-entry roster rows: add call-bound custody
  from the caller. A pointer or roster alone must remain insufficient.

  Acceptance: one source program establishes, accesses and retires a view
  through interpretation and published native execution. Codec/optimization
  preserve semantics; stale/substituted plan, artifact, backing, range, rights,
  occurrence or lifetime reject before access. Failed establishment returns
  custody and retirement preserves declared resident/vacant state. Preserve
  multi-row order independence, exact type/path/domain rejoin, exclusive overlap
  rejection and shared/disjoint admission. Run matching supported hosts or
  explicitly retain unavailable runtime legs.

- **DEVICE-EXTENT-ACCESS.** Make `samples/cli/device/device_extent_access`
  execute provider-backed lending, completion and restored CPU access under
  [device custody and ordering](wiki/spec/resources/device_access.md). Its
  entrypoint is empty and it has no `build.omg`; `tools/sample_gate.py` therefore
  excludes it. `source/library/core/device.omg` declares the loan boundary but
  omits ordering roles. Select a concrete provider, invoke both loan directions,
  and check the resulting CPU access and data.

  Reuse `extents::{external_loans,ordering_events,mapping}`: exact confinement,
  release/fact checks, coverage rejection and retry custody already exist in
  Rust. `admit_acquisition_after_completion` only borrows and compares two event
  records; it consumes no completion or live loan. Coverage checks compare roles
  and authority but do not bind coordinates to a required provider schema.
  Connect admitted, opaque runtime occurrences to the loan and exact role-specific
  ranges/mappings, device, request and scope. Publication must track writes:
  an intersecting write invalidates it, notification consumes valid publication,
  and acquisition consumes completion only after borrower release and all
  coherence obligations. Preserve scoped events through Terminal and target
  realization, including instruction-free coherent implementations.

  Exercise missing/extra/duplicate roles, substituted coordinates, stale
  publication, wrong request/device/scope/mapping and premature restoration.
  Failed start returns the borrow; failed completion/acquisition retains the
  pending token and candidate for retry. Pin permitted suspension and CPU
  exclusion for each direction; device success alone cannot establish release.

  For shared memory, exercise an atomic/coherence lease with trusted peers or
  copy-and-validate/revoke-and-validate with hostile writable peers. Preserve
  `MappedExtent::stable_loan`'s exact revocation/invalidation requirement and
  `external-roots`' `ProgramLocalExtentRegistry::remap` checks: retained foreign
  pins, changed geometry and widened rights reject; replacement requires a fresh
  era. Their Rust receipt tests do not execute the source/provider protocol.
  Protocols, queues, isolation and translation policy stay provider/OS-owned;
  a CPU barrier is not device completion.

- **PLACED-ACCESS-NATIVE-OPS.** Close source-to-native indexed access under
  [structural access](wiki/spec/terminal-psi/structural_access.md), preserving
  original caller storage. Start with `collections/runtime_double_indexed_rmw_exit`
  and `runtime_indexed_struct_field_rmw_exit`, still rejected in the committed
  corpus snapshots; re-run them to locate the current failure. The macOS native
  snapshot already records `storage/runtime_machine_owned_indexed_integer_write_exit`
  at exit 79 and `collections/runtime_hoisted_index_write_exit` at exit 7;
  Windows records rejection. These are recorded observations, not fresh runs.

  Reuse checked `structural_scalar_store/selectors.rs`, c2l
  `emission/runtime_elements.rs`, and Omega's `structural_layout::runtime_projection`
  and `control_flow/scalar_sources.rs`. Computed integer selectors, stored-value
  leaf-copy selectors and field-loop invariant inference already exist.
  `dynamic_indexed_shared_receiver_emits_a_runtime_index_argument` checks
  `cells[i].get()` through Terminal production; the static-only
  `lower_structural_path` helper is not its missing lowering route.
  Keep one `AssignmentIndex { depth }` per selector, evaluation before the
  stored value, and independently reconstructed bounds on every `RuntimeIndex`.
  Interpreter tests in `runtime_index_arguments.rs` execute constructed Terminal
  artifacts; selection's `primitive_stores/runtime_elements.rs` checks four
  targets' strides, footprints and replay, without executing native code.

  Acceptance: source-driven native `&write` and `&mut` calls modify the selected
  element in the caller, leaving neighbors untouched. Cover interleaved fields
  and nested indices, computed selectors, field-held loop counters, leaf copies
  and shared receiver reads; reject invalid indices and substituted bounds,
  operands or access authority. Use contract/domain facts with
  **CANONICALIZE-SCALAR-RANGE-CONTRACTS**, then run the repaired cases through
  `python3 tools/corpus_gate.py --native --filter <fixture>` on matching hosts.
  **WRITE-ONLY-BORROW** and **STRUCTURAL-BORROW-IDENTITY** own general source
  loan admission; **TERMINAL-SLICE-VIEW-VOCABULARY** owns broader view bounds;
  **PLAN-LAID-VIEWS** owns provider-backed establishment and access plans.

- **RECAST-SOURCE-POSITIONS.** Carry representation-compatible shared/mutable
  re-views through checked trees and native execution, following the
  [recast contract](wiki/spec/layouts/recasts.md). `checking::value_custody::recasts`
  already judges statement-reachable guards, call arguments and nested operands.
  `judge_scalar_recast` emits diagnostics but no retained re-view judgment;
  checked trees have no corresponding representation. Stage 05's
  `expression_preparation/bindings/structural_fields::resolve` rejects observations
  whose scalar type differs from the declared field. Retain and replay the
  compatibility relationship instead of relaxing that equality check. Preserve
  checked layout/validity, backing identity, lifetime, access and whole-place
  reads through ordinary expression sequencing and temporary loans.

  The committed check/native records reject the `tests/omega/pass/recast`
  executable customers at local-binding, attached-shape or store planning;
  `corpus_runner` now realizes entry-bearing fixtures even on its check leg.
  `nested_operand_view_compile` checks without an executable entry. Reference
  locals initialized from place loans remain shared work with
  **STATE-LOCAL-VALUE-FRONTIER** / **REFERENCE-LOCAL-BINDING-ROUTES**; range syntax
  migration belongs to **CANONICALIZE-SCALAR-RANGE-CONTRACTS**. Re-measure each
  customer's first refusal after those repairs, rather than preserving old stops.

  Acceptance: let-bound and inline equivalents of supported scalar, record,
  array and slice re-views execute, with mutable writes visible through the
  original backing. Run `python3 tools/corpus_gate.py --native --filter recast/`
  on matching hosts and require the authored exit results. Incompatible
  geometry/validity, access escalation and conflicting backing use reject at
  their source locations. Keep proof-only recasts fenced and preserve precise
  symbolic/boundary-witness footprint refusals and placed-view restrictions.

- **EXTERNAL-DATA-SCHEMA-CONVERSION.** Finish and verify the authored
  preserving-codec customer under the [codec contract](wiki/spec/layouts/codecs.md),
  using ordinary library machines and call/storage/lifetime owners. Admission
  recognizes `source/library/std/wire.omg`'s `PreservingDecode` requirement.
  `tests/omega/pass/wire/wire_preserving_decode_relay_exit` validates one known
  byte and inspects the borrowed remainder's codec identity and ordered tail;
  it does not yet re-encode a relay. Committed check/native records reject its
  `DecodeResult<Relayed<LocalMessage>>` binding at `local data: structural result
  shape`; the fixture's execution comment is not runtime evidence.

  Follow `terminal_unit/control/call_results.rs::checked_structural_result_type`
  into `checking/src/value_custody/storage_contents.rs`.
  `has_owned_or_shared_view_fields` accepts generic applications only with empty
  type arguments and handles direct record fields only: removing that first
  restriction alone does not cover the result sum or its nested borrowed
  remainder. Finish closed substitution, case/nested-view shape and returned-loan
  custody through the existing owners; do not add a codec-specific classifier.
  Preserve exact conformance identity and rejection of unresolved generic
  returned-carrier lifetime frontiers.

  Acceptance: execute the source customer on a matching host with an explicit
  exit-70 oracle, then verify faithful relay through the same selected codec.
  Independently handle the validated known value while preserving exact unknown
  bytes/order in the output and retaining the input loan while its remainder
  lives. Codec substitution, invalid known input and conflicting input-buffer
  use must reject. Preserve unique historical-migration selection and existing
  generated-codec verification ownership: `checked-interpreter::verify_wire_schema_codec`
  supplies finite regression probes, which cannot earn `Derived` trust.

- **SYMBOLIC-MATERIALIZATION.** Restore repeatable native acceptance for the
  existing recursive [derived consumer](wiki/spec/layouts/plans.md#derived-consumers),
  including the outstanding Linux AArch64 execution leg. The cited compiler
  `layout_plans` test target and `writer_lowering.rs` were removed; that command
  cannot supply the missing evidence.

  Reuse `backend/layout/src/sum_materialization/tests/recursive.rs` for checked
  layout projection and closed generic substitution, and
  `psi/foundation/layout-plans/src/symbolic_materialization.rs` for nested
  field/index derivation. `executable-installation`'s
  `post_handoff_writer/tests/writer_execution.rs` compares portable Rust writes;
  `program-entry-plan/src/post_handoff_writer/mod.rs` lowers both Linux ISAs,
  checks normalized identity and replays bytes. These controls do not execute
  the emitted fragment.

  Provide a registered native harness connecting checked source geometry to
  the generated writer and its exact installed invocation, following
  [writer custody](wiki/spec/build/hardware_materialization.md#generated-writers-and-publication).
  On Linux AArch64 require complete destination bytes and surrounding guards
  to match the reference execution. Preserve the same normalized fragment and
  invocation across ISA-specific bytes, empty arrays, sibling writes and
  nested/generic cases. Retain other matching-host controls and explicitly
  identify unavailable runtime legs. Focused portable checks are
  `mbx nextest run -p layout --lib -E 'test(~sum_materialization::tests::recursive)'`
  and `mbx nextest run -p program-entry-plan --lib -E 'test(~post_handoff_writer)'`;
  neither substitutes for native execution.

  Keep open templates, non-closed member applications and runtime counts in
  static layout rejected. Reuse the recursive consumer and closed literal
  substitution; no depth-specific representation is needed.

- **ADDRESS-EXPOSURE.** (new-scope) Implement the settled
  [`address_of` queries](wiki/spec/language/counts_and_addresses.md#address-exposure)
  through Psi admission, Terminal encoding/replay, interpretation and native
  realization. These queries have no current `AuthoredDeclarationSelectionIntrinsic`
  or admission in checked `authored_selections/intrinsic_calls.rs`.
  Retain the selected machine application or original referent; a machine query
  adds ordinary executable demand even without a direct call. Preserve that
  demand through specialization/inlining and retain the interpreted body.
  Native entry/storage addresses use normal realization and relocations;
  preserve private callback ABI selection and custody.

  Reuse the interpreter's `StructuralRuntimePlace` and
  `LocalStructuralIdentities`, which currently identify storage within one
  execution, and its machine identities. Exposure still needs disjoint
  code/storage encodings covering loaded instances, runtime occurrences and
  subplaces. Compatible views must agree; full identities must fit without
  truncation, collision, or zero for live entries. No parallel registry,
  virtual byte memory or native trampoline is required.

  Acceptance: source programs execute `address_of<Selected<Arguments>>()` and
  `address_of(reference)` for generic machines, selected machine binders, locals
  and borrowed fields in both modes. Repeated exposure agrees and distinct live
  locations remain distinct. Reject unresolved selections, proof-only entities
  without executable meaning, invalid loans, nonaddressable projections and
  exhausted encodings. Addresses grant no borrow/call authority; machine/reference
  `as addr` is not an alias. Keep evaluator IDs out of portable constants and
  unresolved native relocation bits out of semantic evaluation. This remains separate from
  [instruction interpretation](wiki/spec/language/assembly.md#interpreted-execution)
  and adds no indirect-call syntax, arbitrary patching or native-address
  interchange for interpreter IDs.

- **DATA-LAYOUT-QUERIES.** (new-scope) Implement core `size_of<T>()` and
  `align_of<T>()` from the selected target's validated layout, following
  [layout queries](wiki/spec/layouts/plans.md#size-and-alignment-queries).
  Add source admission and semantic evaluation using existing
  `build-time-evaluation::compute_native_layout_plan`,
  `LayoutPlanReport::{size, align}` and `layout::layout_type_reference`, then
  ordinary constant materialization. Preserve exact schema, target and policy
  custody; no second size calculator or keyword. The native layout builder's
  unresolved-array fallback is `TypeLayout::default()` (size 0, alignment 1);
  a query must distinguish that absence from a validated zero-sized layout.
  Acceptance: default/custom layouts report size including padding and alignment
  in authored constants and runtime uses; erased fields, zero-sized layouts,
  reference-versus-referent storage, and target variation agree with actual
  layout/ABI. Reject unavailable fixed size, missing runtime layout, circular
  layout queries and machine declarations. A known alignment may remain available
  for a dynamic-size plan; no interpreter-host allocation size escapes.

- **NATIVE-CODE-LAYOUT-QUERIES.** (new-scope) Deliver
  [`code_layout<Machine>()`](wiki/spec/language/counts_and_addresses.md#native-code-layout-queries)
  for a selected application: entry address plus exact emitted instruction
  regions with `addr` bases and `u64` lengths. The low-level editor customer
  needs actual regions, not a guessed 2 KiB window or next-symbol subtraction.
  Reuse **ADDRESS-EXPOSURE**'s demand and `machine-emission::text_placement`'s
  retained/replayed `PlacedInstructionSpan` geometry. The existing callable-entry
  record exposes one section offset/byte count, not a complete region query.
  Join final instruction spans and relocations to the selected application;
  Psi records demand without running a second compiler or evaluating the
  unfinished native product. Acceptance: an otherwise uncalled generic body
  is retained and its runtime metadata matches the final
  artifact, including split/shared regions without claiming exclusive ownership
  or including padding, pools and callees. Reject missing native realization
  and use in the same product's type/constant/layout or optimization computation.
  Interpreted entry IDs do not receive a fabricated code size or byte region.

- **ASSUMED-BINDING-CONTRACTS.** (new-scope) Implement the root-only
  [`Build::assume_binding_contract<Required::operation, Supplied::operation>()`](wiki/spec/build/assumed_binding_contracts.md)
  relationship. `language-semantics::BuildOperation` has no admission for it;
  `package-manager/review-evidence/src/record/rows.rs` has no corresponding
  review kind. Extend existing Build/provider installation, Terminal conditional
  evidence/replay, interpreter/native call custody and package review/lock
  acceptance. Retain exact entry/exit assumptions, authorized issuer/subject
  mapping and selected implementation closure through erasure and inlining.
  Both bodies remain checked against their own contracts.

  Acceptance: a source caller executes through the root-selected link with
  caller-facing qualification, including input-side authority; direct supplier
  calls gain none and remain independently eligible. Reject carrier/order,
  passing-mode, ABI, lifetime/custody and operational incompatibility, incomplete,
  competing or cyclic supply, unapproved roots and stale project acceptance.
  Install/update/audit disclose **Extremely high risk** transitively; compact lock
  rows commit to exact contracts, mapping and implementation closure, requiring
  renewed acceptance when these change. Build may register the relationship;
  helpers, generators, const/layout evaluation and automatic tests reject its
  execution or assumptions, including erased reliance.

  Preserve the settled [refutable-connection exception](wiki/spec/build/assumed_binding_contracts.md#known-contradictions-at-the-trust-crossing):
  an explicitly accepted supplier guaranteeing `result == 0` may supply a
  requirement guaranteeing `result == 1`. Retain both contracts, the contradiction
  and supporting facts; distinguish unknown from refuted in audit and cover an
  input-side contradiction. Verify source-independent conditional evidence,
  without promising a runtime value or controlled trap under the false premise.
  Assumptions cannot discharge either body's checking or non-assumable compatibility.
  Ordinary refutable grants, invalid `satisfies`, stale approval and build-time
  reliance still reject. No unchecked body, override flag or parallel approval.

## Build-level behavior exclusions

Deliver the [exclusion contract](wiki/spec/build/behavior_exclusions.md) after the
existing compiler customers above. Checking/no-op assertions and optional logging
remain ordinary calls with unchanged public ceilings; exclusions add product
admission requirements, never permission to violate callable contracts.
Compiler paths below are relative to `omega-rust/omega/compiler/`. Reuse the
shared checker in `omega-rust/omega/backend/plans/provider-evidence/src/provider_evidence/executable_scopes/behavior_exclusions.rs`.

- **BUILD-SEMANTIC-EXCLUSIONS.** Finish source-to-publication acceptance for the
  full admitted entry/call closure. Executed selections in
  `build-evaluation/src/admission/behavior_exclusions.rs`, unoptimized entry and
  callback gates in `src/terminal/`, and retained-proposal replay in
  `src/report/terminal_product.rs` already exist. The shared checker/tests cover
  selected adapters, bounded dynamic dispatch, forwarded/rebound descriptor
  parameters and nominal cleanup. Complete source-produced coverage of generated
  entries and remaining dispatch forms. Conservative retained contracts may bound
  opaque targets; otherwise missing coverage rejects.
  Only independently justified guard facts can exclude paths; optional optimization
  and broad public ceilings cannot prove absence.

  Acceptance: unchanged checking/no-op implementations under public Trap ceilings
  yield the correct exclusion verdict with optimizations on/off through native
  publication. Reject eager argument traps and unrelated crashes. A silent ordinary
  logger can pass a service exclusion; a silent provider cannot erase an actual
  boundary invocation. Cover conditional/helper selections and independent replay;
  reject changed entries/providers/targets/scopes/policies and omitted coverage.
  Distinguish prohibited possible behavior from insufficient evidence.

  [The package verdict harness](omega-rust/omega/package-manager/tests/behavior_exclusion_verdicts.rs)
  asserts Linux realization of `no-op-app` and attributed service/machine refusal
  for `sink-app`. Add verdict coverage for `checking-app`, `quiet-logger-app` and
  `direct-unit.omg` under `tests/omega/packages/behavior-exclusions/`; the two
  kits are their dependencies. Exercise native publication/execution and both
  optimization settings; declaration-only checks do not establish them.
  Physical custody belongs to **BUILD-EXCLUSION-REALIZATION**.

- **BUILD-EXCLUSION-REALIZATION.** Carry exclusion policies through native
  artifact custody, installation and replacement. Canonical unions, mechanism
  classification and direct/retained realization checks already exist:
  `src/native/native_realization.rs` checks exercised classes before emission,
  independently of receiving permissions. `src/native/retained_native_product.rs`
  carries the proposal's union into that check, but
  `omega-rust/omega/backend/artifacts/native-artifact/src/native_artifact.rs`
  retains the mechanism review without the exclusion union.
  Join the policy to exact semantic/target/provider evidence through legalization,
  image emission, **COMPONENT-SUBSTRATE** replacement and
  **WIRE-RUNTIME-AND-INSTALLATION**. **TWO-AXIS-TERMINAL-AUTHORITY-REVIEW** owns
  receiving permission; reuse its mechanism classification without another
  classifier or synthesized approval.

  Acceptance: silent and output-producing providers receive distinct physical
  verdicts with/without receiving policy and optional optimization. Unknown
  classifications cannot prove absence; substituted providers/evidence reject,
  stricter receiving exclusions require a fresh join, and failed final checks
  publish no successful product. Retain foreign-boundary controls and supply
  exclusion-specific retained-artifact/source-free replay, publication and loud-sink
  controls through current harnesses. Exercise actual Windows/macOS runtime,
  installation and replacement; sentinel or cross-target realization is insufficient.
  Reuse `physical_exclusions_use_exercised_authority_not_receiver_permission`
  as a receipt-level control; it does not establish those product legs.

## Requirement-based tests

Implement the [testing contract](wiki/spec/build/testing.md) and
[authored source shape](wiki/language_guide/chapter_23_testing.md) after the existing
compiler customers above. Reuse ordinary satisfaction/entry checking, verified
Terminal Psi execution and embedding custody; no test attribute, special machine
name, calling permission or parallel compiler. Compiler paths below are under
`omega-rust/omega/compiler/`.

- **BUILD-TEST-GROUPS.** (new-scope) Build registration is missing:
  `build-evaluation/src/sources/source_assembly/build_prelude.rs`'s `Build` has
  no `tests` facet, and `build-evaluation/src/admission/configuration.rs`'s
  `BuildConfig` retains no groups. Build evaluation owns
  `builder.tests.group<ExactRequirement>()` registration and normalized enablement;
  compiler orchestration owns package-local concrete satisfaction discovery,
  separate runner roots, outcomes and publication.
  Preserve exact product-reference identity, private scope and source/target
  provenance; never discover by strings or infer generic applications.
  Acceptance: an ordinary build discovers two tests without per-test registration;
  a mandatory failing check prevents publication, disabling its group reports
  not-run, and a second requirement in the same trait remains distinct. Cover
  duplicate/inherited identities, non-runnable signatures, unresolved generics,
  dependency non-discovery and exhaustion distinct from failure. Run without std;
  provide an optional ordinary std testing requirement without compiler recognition
  or implicit dependency. Application Psi/native output excludes test-only roots;
  an ordinary native harness may explicitly call visible test machines.
  Receiver/service execution remains rejected until **BUILD-TEST-AUTHORITY**.
  Interpreter helper tests alone do not establish this project-build path.

- **BUILD-TEST-AUTHORITY.** (split-of:BUILD-TEST-GROUPS) Extend **BUILD-TEST-GROUPS**
  with group-scoped providers, provisioned receivers and fresh mock state.
  Reuse Build/provider admission, entry establishment and interpreter custody.
  Existing `build-evaluation/src/admitted_build_program.rs` restricted requests
  and `omega-rust/omega/package-manager/src/review/restricted_build_grants.rs`
  join normalized build-host requests to lock acceptance; extend them to retain
  test/group, fixture and provider attribution before execution. Existing
  [restricted-request tests](omega-rust/omega/package-manager/src/review/candidate/compilation/tests/restricted_build_grants.rs)
  cover missing, widened and wrong-purpose consent; group execution remains unwired.
  Acceptance: two tests receive fresh virtual filesystems and established
  `Binding` fields; missing bindings reject. Child filesystem/network/process
  requests cannot widen root grants or escape virtual backing. Install/update
  surfaces new test requests; locked builds cannot approve them, and acceptance
  without an executor grant still rejects. Cover provider/target mismatches,
  disabled-to-enabled rechecking, unsupported execution without native fallback,
  and crash containment without invented unwind/rollback. Automatic tests must
  reject assumed-binding execution or reliance, including through fixtures,
  helpers and providers.
  Mock configuration leaves production providers and application roots unchanged.
  Keep policy out of constant-evaluator admission and use the existing review flow.

## Embedding and interpreted components

Implement the [embedding contract](wiki/spec/build/embedding.md) as an ordinary
library, starting with one source-to-host customer. These additional execution
customers follow the existing compiler customers above; generic embedding need
not wait for Cathedral's instruction coverage. Interpreter paths below are under
`omega-rust/psi/interpreter/terminal-interpreter/`.

- **EMBEDDING-SOURCE-TO-HOST.** (new-scope) Deliver compile -> load -> bind ->
  invoke -> host result -> guest result -> close from an authored interface
  package, host implementation and script. Connect product compilation/public entry
  descriptions, the interpreter and checked host adapters using `Binding<R>`.
  `src/lib.rs` exposes artifact execution; `tests/unit/embedding_lifetimes.rs`
  exercises source-free scalar code retention, fuel resumption and result rejection.
  Public reusable programs, reclaimable instances, import-slot bindings and close
  remain. Share immutable admitted programs across independent instances; two
  same-trait slots address different host objects. Missing/wrong/unauthorized
  bindings and invalid host input/preconditions reject before effects; reject native
  result schema mismatch.
  Public descriptions and checked dynamic calls must use the same adapters and
  retained entries, without new generic instantiations or private access; reuse
  [Semantic reflection](#semantic-reflection) for requested type metadata.
  Inspect/update live objects only through exposed operations at valid access
  points; edited detached documents require validated construction/application
  and grant no live-storage authority. Loading existing Psi requires no source
  compiler, filesystem, global runtime or thread; keep compilation authority separate.

- **EMBEDDING-RESOURCE-LIFETIMES.** (new-scope) Extend **EMBEDDING-SOURCE-TO-HOST**
  through borrowed views, owned host resources, asynchronous completion and
  contracted callbacks. Owners: runtime/adapters and Terminal execution/loan/custody.
  `src/results.rs` has no waiting/completion protocol; the
  `raw_effect_sinks_do_not_pin_host_provider_identity` lifetime test permits handler
  substitution on resume. Pin admitted bindings and retain custody across pauses.
  Reuse the [source-driven registration control](omega-rust/omega/backend/runtime/component-publication/src/tests.rs):
  `interpreted_register_and_unregister_drive_the_registration_ledger` already
  exercises component ledger entry/teardown; runtime lifecycle integration remains.
  Pause within an invariant window: reject conflicting inspection/mutation,
  including edits preserving the type invariant but invalidating retained stronger
  facts; resume without repeated effects. Retained results/callbacks prevent close;
  dropping an invocation does not release debt. Reject stale/wrong-runtime handles
  and duplicate/late completions; match each completion exactly once to its pending
  request. Cover failure after host effects and unsupported abandonment without
  retry or fabricated cleanup; reject uncontainable resource
  modes before admission. Close one instance while another remains usable.
  A whole-runtime busy wrapper is only an initial floor.

- **PSI-COMPONENT-REPLACEMENT.** (new-scope) Provide interpreter-backed component
  installation/publication so a running guest replaces an independent subcomponent
  without returning from main. Depends on **COMPONENT-SUBSTRATE** and
  **EMBEDDING-RESOURCE-LIFETIMES**. Current
  `omega-rust/omega/compiler/src/terminal/terminal_artifact/composition_modes.rs`
  rejects independent products; checked descriptions alone do not unblock this.
  Reuse the closed import/export and era/custody contracts in
  `omega-rust/omega/backend/runtime/component-publication/`; its
  `RunnableComponentEraLedger` retains native installed artifacts. Connect Psi
  instances/entries to that publication contract, without a second module manager.
  Root/nested build composition must produce replacement Psi; the guest stages
  and publishes it under delegated authority. New entries use v2, paused old
  calls/sessions remain on v1, and retirement waits for every pin/disposition.
  Reject missing update authority, incompatible schema, widened authority,
  premature retirement and handler substitution on resume. Exercise shared
  external state and explicit state disposition while unaffected components remain
  usable. Loading never publishes. The application owns acquisition, migration
  and update policy; the outer host cannot implement its update coordinator.

- **INTERPRETED-ASSEMBLY.** (split-of:ASM-INSTRUCTION-CATALOG-EXPANSION)
  Deliver [checked instruction interpretation](wiki/spec/language/assembly.md#interpreted-execution)
  through source -> verified Terminal Psi -> ordinary interpreter and supplied
  target environment. Owners: catalog semantics, Terminal operations,
  interpreter custody and environment providers. The source catalog and
  `src/calls.rs`'s `PortWrite` effect dispatch cover slices, not the demanded
  instruction/environment closure, including admitted providers.
  Reuse value/place/transition operations and **PRIVILEGED-PORT-EFFECT-SETTLEMENTS**;
  no separate asm pipeline, whole-host emulator or required per-block source wrapper.
  Separate blocks may share explicit values/authorized memory, never hidden
  scratch. Run device and interrupt-state/idle/external-entry sequences across
  helpers and fuel pauses without repeating completed I/O. Check ordering, delayed
  effects, legal event points, control exits and custody against target semantics,
  not step counts. Missing semantics, missing/wrong environments, widened host
  authority and numeric-ID-as-physical-pointer attempts reject before effects,
  including component installation. Preserve hermetic/build-test admission and
  runtime-assumption bans. Validate modeled behavior separately from native
  encoding/hardware correspondence; providers supply mechanisms, Cathedral owns
  drivers and scheduling policy.

- **INTERPRETED-CATHEDRAL.** (new-scope) Run actual Cathedral startup, memory
  establishment and visible device work, then event delivery while live, then
  guest-owned component replacement. Depends on the embedding/component tasks,
  existing boot/entry/resource capabilities and **INTERPRETED-ASSEMBLY**'s demanded
  instruction/environment coverage; no new owner ruling is needed.
  Preserve the OS sequence and shared native/interpreted algorithms/state machines:
  select providers/build composition, without mode conditionals or assembly removal.
  Host mechanisms cannot replace Cathedral's boot, idle, device, scheduler or update
  policy. Report partial slices and remaining dependencies; a banner is insufficient,
  and simulation establishes modeled behavior, not native timing/hardware correctness.

  Cathedral's `tools/x86-legacy-bootstrap-composition-canary/main.omg` is explicitly
  compile-only with an empty entry. `tools/extent-root-canary/run.sh` and
  `tools/timer-root-work-canary/run.sh` still require retired
  `04_typed_trees.json` / `05_machine_contracts.json` dumps. Migrate those and
  equivalent canary oracles to intended behavior or supported artifact contracts
  before citing acceptance. Source/build migration or compile-report output alone
  cannot repair these assertions; preserve Omega's no-default-debug-dumps contract.

## P3 - Terminal Psi, PCC, and observation

- **PROOF-KERNEL-CORE.** Finish independently checked mathematical proofs
  under the [foundation](wiki/spec/proofs/foundation.md) and
  [W-based profile](wiki/spec/proofs/inductive_profile.md). Owner:
  `proof-admission/src/mathematical_core/`, coordinated with
  **PROOF-CONTRACT-MIGRATION**'s source integration.

  - Replace remaining `bounded_denotation.rs::rule_instance` assumptions with
    derivations or justified checked rules for actual source obligations.
    `an_undecided_crossing_keeps_the_explicit_rule_instance` pins a live case:
    open `Equal(!a, b)` expanded into a disjunction still assumes its conversion.
    Nested identity reversal and closed Truth/Falsehood carrier crossings
    already derive; reuse `equality_transport/tests.rs`, the Boolean,
    correlated-arithmetic and cast derivations, including `casts/tests.rs`'s
    zero-rule-axiom checks. Preserve numeric policy and exact operands.
    Fixed arithmetic laws and construction-budget fallbacks remain explicit
    assumptions; a judgment assuming its conclusion is not an independent proof.
  - Check source-produced indexed applications against exact parameters,
    indices, payloads, case constraints and recursive uses. The five parametric
    `indexed_scheme` declarations pass signature checking and tests of dependent
    induction and computation with neutral children. These Rust-built tests
    and codec round trips do not establish source correspondence. Cover the
    profile's vector, derivation, mutual and nested families; distinguish
    negative recursion, unsupported valid encoding and malformed producer
    evidence. Add only kernel support required by source integration, preserving
    the selected calculus without a parallel indexed/strict checker or untyped eta.

  Acceptance: the [migration examples](wiki/spec/proofs/foundation.md#migration-acceptance)
  reach source-free judgments from Omega: universe-polymorphic arbitrary
  predicates, dependent pairs, identity transport and induction; strict
  same-statement conversion preserves distinct relevant witnesses. Malformed
  universes, capture-changing substitution and illegal elimination reject.
  Preserve exact assumption closure through declaration statements/types,
  imports, erasure and wire, independently of unfolding. Exercise source-produced
  equality transport with false-source, missing-equation, changed-operand and
  changed-J-endpoint controls; the surviving `proofs/kernel_theorem_equality_*`
  corpus pair and codec theorem tests do not cover that entire chain.

  Measure source-produced term size, retained storage and checking cost in the
  application checker. Refresh the [metatheory evidence ledger](wiki/spec/proofs/kernel_metatheory.md#evidence-ledger)
  against surviving controls; its combined-profile argument remains paper-level,
  not mechanized. **PCC-CANONICAL-SEMANTIC-LEDGER** owns trusted-row soundness;
  **PROOF-CERTIFICATION-BRIDGE** owns loop correspondence. Calculus changes
  require `OWNER_QUESTIONS.md`.

- **C2L-PROOF-SEARCH-BLOWUP-CONTAINMENT.** (new-scope) Finish aggregate
  proof-search accounting and [outcome publication](wiki/spec/proofs/publication.md#outcomes).
  Reuse c2l's `proofs/operation_proofs.rs` machine preparation cache and
  `scalar_block_invariants.rs` verdict cache, shared roster-round/candidate
  budgets and cyclic-guarantee budget. Their exhaustion reaches
  `ProofSearchCapacityExhausted` and the compiler's `incomplete:` diagnostic;
  source-derived tests already distinguish a declined inference budget from
  invalid source. The computed-selector guard is gone, and the committed macOS
  snapshot records `runtime_hoisted_index_write_exit` running at exit 7.

  Remaining work: `nonzero_divisor_certificate/integer_selection/direct_add/conjunction`
  discards its usage and turns exhaustion into `None`. Carry such declines
  through fallback search and finalization when no other route proves the
  obligation. Preserve resource identity, limit, requested work and obligation
  coordinate into the [compiler-request outcome](wiki/spec/build/compiler_request.md);
  `compiler/request/outcomes.rs` currently maps every diagnostic failure to
  `BatchChildOutcome::Rejected`, including an `incomplete:` diagnostic.
  Complete production-path exhaustion controls beyond the existing helper-budget
  tests, retaining independent certificate verification.

  Rebaseline **RC-PCC-REPLAY**'s terminated case and the computed-selector
  customer with `OMEGA_PROOF_TIMINGS=1`. Measure aggregate reconstruction/search
  work and certificate size; remove remaining demonstrated duplication/growth
  instead of using caps to hide it. Acceptance requires completed source checks,
  accounted bounded work, and resource exhaustion reported as `Incomplete`
  through publication. Invalid evidence still rejects; missing evidence cannot
  authorize acceptance. [Kernel budgets](wiki/spec/proofs/kernel_metatheory.md)
  remain implementation policy, not narrower language semantics.

- **PROOF-CONTRACT-MIGRATION.** Deliver general mathematics through
  [machine contracts and witness/law bundles](wiki/spec/proofs/contracts.md#machines-and-bundles)
  and [mathematical bindings](wiki/spec/proofs/mathematical_bindings.md).
  Owners: Psi resolution/typing and contract checking, Terminal evidence/codec,
  and `source/library/core/`; use PROOF-KERNEL-CORE's terms.

  Checked mathematical signatures already publish in Terminal artifacts and
  recheck after serialization. Terminal codec's `verify_terminal_artifact_proof`
  checks that signature separately from the module proof; connect mathematical
  applications to exact program obligations and receiver assumption policy.
  Existing prefix applications, explicit generic arguments and bounded universe
  inference in stage 04's `proof/mathematical_signature/` are the starting point.

  Remaining work:

  - Complete body denotation and applied carriers, including mathematical
    member-call resolution and non-integer ordering. Preserve the distinction
    between interpreted operations and opaque carrier assumptions. Resolve
    non-type `core::*` occurrences by canonical symbol identity, then remove
    `proof/fixed_core_vocabulary.rs`'s unresolved-spelling fallback.
  - Replace `proposition`/hidden-witness routes, including `proof_output_calls.rs`
    and core/test consumers, with ordinary contracts/bundles. Preserve exact
    substitutions, witness identity, path/result availability, validity and
    assumption closure; remove retired parser/carrier/codec paths. Include
    `omega-rust/omega/package-manager/review-evidence/EVIDENCE_SCHEMA.md`'s
    replacement proof rows and QUOTIENT-THEOREM-LIFT's congruence-only lift payload.
  - Complete arbitrary mathematical application and theorem-only logical
    hypotheses under the binding spec's delivery controls. `TraitRequirement`
    still needs predicate/function terms beyond lifetime/type arguments. Keep
    executable callback selection, ordinary local bindings and complete machine
    calls intact; introduce no quantifier keywords or declaration-catalog
    substitute. Exact call preconditions and recursive descent remain required;
    OPERATOR-MACHINE-SUPPLY owns the shared call repair and Nat migration.
  - Connect source-established sequence/range predicates to element proof use.
    `ForAllInRangeFact` / `QuantifiedRangeFact` have no source producer or
    entailment consumer. Preserve collection, predicate, range and subject
    identity through mutation and wire; reject wrong, out-of-range or invalidated
    facts. Use the same mathematical calculus.
  - Enforce [executable demand](wiki/spec/proofs/mathematical_bindings.md#assumptions-and-executable-demand)
    at source use, evaluation and lowering: axioms supply no executable provider;
    constant branch arms do not justify choice-dependent control. Erasure must
    retain assumption closure. Migrate core relations, quotients and examples.

  Acceptance: source → Terminal serialization → independent checking under
  the [publication contract](wiki/spec/proofs/publication.md), with false twins:

  1. Two composed witness/law bundles preserve substitutions and distinct relevant
     witnesses; higher-order theorems accept arbitrary predicates/functions and
     derived Π-term evidence for machine-shaped logical hypotheses, satisfying
     all [binding delivery controls](wiki/spec/proofs/mathematical_bindings.md#delivery-controls).
  2. Nonconstructive existence permits erased references without unjustified
     executable extraction/branching. Squash-to-strict reasoning needs no choice;
     constrained records prove coupling, and relevant dependent pairs remain
     distinct from strict predicate gating.
  3. Accepting and denying policies distinguish the same theorem by its exact
     transitive closure through statements/types, import, erasure and wire.
  4. Cauchy/quotient reasoning uses the
     [set-quotient interface](wiki/spec/proofs/quotients.md#set-quotient-foundation)
     without hidden extensionality or new reduction. A quotient-refusing policy
     accepts representative operations and congruence with quotient-free closure,
     but rejects the assumption-bearing quotient theorem. Rat's comments do
     not prove that separation.

- **PROOF-CERTIFICATION-BRIDGE.** Retain checked source invariant derivations
  for lowering and independent Terminal verification under the
  [publication contract](wiki/spec/proofs/publication.md). Stage 04's
  `checks/contracts/exits/cyclic_headers.rs` caches only admitted exits;
  stage 05's `proofs/scalar_block_invariants/cyclic_guarantees.rs` searches again.

  Reuse the native `runtime_ranked_accumulator_guarantee_exit` control and
  source checking for a named cyclic state with carried integer parameters.
  Complete multi-state induction beyond that single-header restriction and
  storage-changing `self` transitions. Preserve establishment, every arrival's
  simultaneous substitution, and exact source/Terminal scope.

  `proof_inductive_gauss_sum` and `proof_inductive_climbing_sum` have no build
  entry or runtime oracle. Add native coverage of their authored receivers and
  results; coordinate demonstrated execution gaps with STATE-LOCAL-VALUE-FRONTIER.

  Reuse `ClosedIntegerRelation` normalization in
  `proof/admission/src/integer_rules/open_terms.rs`; PROOF-KERNEL-CORE and
  PCC-CANONICAL-SEMANTIC-LEDGER own its licensed derivation. Preserve exact
  overflow obligations and Wrapping's equality-only reasoning, without per-loop
  axioms or inferred residue order/disequality.

  Acceptance: source guarantees and arrival evidence survive serialization and
  native execution. A wrong update must fail preservation while the unchanged
  cycle certificate still answers the same termination question; missing arrival
  evidence and unestablished guarantees reject. Retain the native/wrong-step
  corpus pair, `ranked_value_guarantees` and
  `terminal-verifier/tests/ranked_scc/scalar_block_invariants.rs` controls.

- **PCC-CANONICAL-SEMANTIC-LEDGER.** Replace trusted fusion of artifact
  traversal and proof search with a total canonical-ledger generator and an
  untrusted certificate producer under the
  [verification contract](wiki/spec/terminal-psi/verification.md#canonical-semantic-ledger).
  Owner: `terminal-verifier`, with `terminal-codec` and `proof-admission`.

  The existing `trusted_surface.rs` inventory mechanically covers dispatch,
  reconstructed facts, dependencies, implementation sources and soundness
  statuses. Reuse it; coverage and a `Proved` label are not themselves proofs.
  Revalidate changed entries' justifications before refreshing source digests;
  a matching digest is not a soundness argument. Remaining work:

  - Preserve supplied `ProofBundle::crash_obligations` and untrusted search in
    `proof-admission::certificate_search`; verification reconstructs obligations
    and replays certificates without searching.
  - Discharge `ExplicitlyTrusted` reconstruction, normalization, scope,
    invalidation and call/cycle-composition rows with checked evidence and
    exact dependencies. Preserve `PROVED_ENTRIES`, dispatch/fact coverage,
    source-closure and unfinished-dependency checks; prove prerequisites
    rather than hiding trusted composition behind a proved leaf.
  - Audit the borrowed-`self` cleanup-hook entry/content claims in
    `validation/affine_cleanup.rs`: receiver and requires checks are visible,
    but the cleanup edge's discharge of the target's entry claims needs an
    explicit argument and substitution control. Coordinate with **CML4**;
    this is a targeted unresolved audit, not an established exploit.
  - Define the generator over canonical Terminal bytes, not a producer-decoded
    AST accepted by assertion. The common kernel and selected inductive
    profile's unfinished soundness/encoding obligations remain dependencies,
    not permission to assume generator success.

  Acceptance: a theorem-dependent program verifies after source and producer
  state are removed, under accepting and rejecting assumption policies.
  Wrong goals, profiles, scopes, premises, missing obligations and omitted
  transitive assumptions reject; normalization and erasure cannot hide
  dependencies. Use `terminal-verifier/tests/trusted_surface.rs` and codec
  trust-graph controls alongside the connected certificate case. This is not
  native refinement or bootstrap proof discharge; GAMMA-DERIVATION-CHECKER
  remains independently owned by `TASKS_BOOTSTRAP.md`.

- **PROOF-RELEVANCE-MIGRATION.** Complete carrier-independent erased arguments
  under [explicit erased bindings](wiki/spec/proofs/contracts.md#explicit-erased-bindings).
  Owners: stage 04 call plans, stage 05 contract/proof-term lowering, Terminal
  codec and independent call composition. Static scalar contract arguments and
  checked record/sum constructions with `ProofTerm::Scalar` leaves already have
  transport. `erased_record_parameter_exit` exercises signature stripping, not
  a contract consuming its witness. Array-field carriers still refuse; dynamic
  dispatch rejects erased formals in `checks/contracts/dynamic_erased_lane.rs`
  and its plans admit self-only calls without proof actuals.

  Extend the existing routes to retain exact erased subjects/actuals, witness
  identity, scope, multiplicity and assumptions through static/dynamic calls and
  serialized contracts, without runtime storage, ABI arguments or execution.
  The exploratory dynamic-lane draft adds no language prerequisite; erased-field
  custody/cleanup belongs to CLEANUP-HOOK-SELECTION-AND-ERASED-OWNERSHIP.

  Acceptance: erased record/enum and array-bearing formals, plus a dynamic trait
  call, use their erased evidence in checked contracts and execute with stripped
  runtime signatures. Replace `fail/relevance/erased_nonscalar_parameter` and
  `dynamic_erased_formal_lane`'s implementation-limit expectations with positives.
  Missing, substituted, out-of-scope or violating actuals must reject after wire
  round-trip. Preserve `erased_parameter_proof_only`,
  `erased_parameter_named_transition_forward`
  and `composed_unit_internal_calls`' exact erased ordinals, plus runtime-read,
  receiver, layout-dependent-erasure and invalid-qualifier refusals.

- **NORMALIZED-ABI-LOWERING.** Finish aggregate and descriptor foreign
  argument/result transport under the
  [calling-plan contract](wiki/spec/build/calling_plans.md). Owners:
  `abstract-operations-to-target-operations`,
  `target-operations-to-selected-instructions` and backend call-frame emission.

  Target lowering and replay already retain owned whole-place aggregates,
  borrowed descriptors and scalar arguments in formal order. Instruction
  legalization and selection still restrict structural arguments to borrowed
  field projections passed as one pointer:
  `legalization/scalar_graph_input/normalized_foreign.rs::structural_argument_at`
  and `selection/scalar_call_abi/normalized_foreign.rs::validate`.
  Realize the selected plan's aggregate locations, descriptor words and result
  custody through those consumers and emission; do not substitute an owned
  aggregate's indirect placement for a semantic borrow.

  Both oracles this acceptance named are gone with the `compiler --test`
  targets: neither
  `source_evaluated_native_realization::record_native_arguments::mixed_scalar_and_record_arguments_stop_at_legalization_custody`
  nor `scalar_native_arguments` resolves anywhere. What survives is
  `03_target-operations-to-selected-instructions/src/tests/legalization/normalized_foreign.rs`,
  whose six pins all pass, including
  `mixed_arguments_preserve_authored_order_through_selection_and_replay` and
  `flat_record_lane_projects_source_rooted_borrow_and_replays`. Mixed scalar
  and record arguments therefore already legalize, select and replay -- through
  the borrowed field-projection lane those pins exercise, on constructed plans,
  not through a source-to-native execution oracle.

  The restriction itself is unchanged and is what to remove.
  `structural_argument_at` rejects unless the path is non-empty and every
  segment is a `Field`, the access is one of the three borrow kinds, and the
  destination is a single pointer-sized register or stack word. An owned
  whole-place aggregate has an empty path and `Owned` access, so it cannot
  reach the lane at all.

  Acceptance: an owned whole-place aggregate argument executes source-to-C on
  a matching host, with a new oracle since the named ones are gone. Test exact
  argument/result values, stack/register placement, formal order, and rejection
  of substituted plans, placements and provider bindings. Each stage must
  independently reconstruct the ABI; Terminal Psi retains no target placement.
  Dynamic descriptors belong to RESTORE-DYNAMIC-DESCRIPTOR-AND-TABLE-CUSTODY;
  private callbacks belong to CALLBACK-PRIVATE-MATERIALIZATION. Use ordinary
  call operands and explicit custody, not one call family per signature shape.

- **OPAQUE-BY-VALUE-BOUNDARY-ABI.** Connect selected opaque representations
  to executable by-value exchanges under
  [representation agreement](wiki/spec/build/opaque_representations.md).
  Selection, independent rederivation, package attribution and installation
  commitment checks exist; they do not move the carrier's bytes.

  `provider-planning/calling_policy_plans/boundary_signatures.rs::opaque_representation_movement`
  derives exact parameter/result and nested-field placements, but its consumers
  remain planning, review and tests. Connect those movements to Omega's provider
  call-frame transport and backend emission. Preserve one semantic occurrence
  for affine/linear values even when placement copies bytes; only checked
  semantic copying creates another occurrence.

  Acceptance: independently compiled producer/consumer programs execute opaque
  argument, result and nested-field exchanges. Reconstruct exact application
  agreement at each composition edge; reject provider, carrier, target,
  lifecycle, multiplicity and historical-selection drift. Preserve
  `calling_policy_plans/opaque_boundaries.rs`, `opaque_boundary_agreement`
  and installation-record substitution controls. Equal layout or a compact
  fingerprint is not agreement. Keep sealed `Ptr<T>` target semantics,
  `EfiSystemTable` and erased proof-only `Real` distinct: proof-only use does
  not acquire a runtime representation demand.
  COMPONENT-SUBSTRATE owns replacement compatibility and stable-handle eras;
  carry these same application commitments into that path. V1 admits only
  `Inert` carriers: direct or nested cleanup-owning carriers must continue to
  reject, not acquire an unspecified lifecycle through this transport repair.

- **WRITE-ONLY-BORROW.** Finish `&write T` under
  [write-only authority](wiki/spec/terminal-psi/structural_access.md#write-only-authority),
  preserving original referents and exact place/loan custody. Owners:
  `04_typed-trees-to-checked-trees/src/execution/`, `checked-trees-to-lowered-psi`
  and native reference preparation in instruction legalization.

  Remaining source-to-native work: general aggregate and `[copy]` sum
  replacement beyond scalar-field record literals; runtime domain-qualified
  byte-field replacement with encoding evidence; guarded/computed runtime-index
  production; and computed IEEE store transport through FLOAT-PROVIDERS.
  PLACED-ACCESS-NATIVE-OPS owns native realization of the retained indexed-store
  operation. Its source fixture still needs the bracketed-range
  migration owned by CANONICALIZE-SCALAR-RANGE-CONTRACTS. Route mutable dynamic
  dispatch through FINITE-GENERIC-DISPATCH, common reference identity through
  STRUCTURAL-BORROW-IDENTITY, and sequencing through STATE-LOCAL-VALUE-FRONTIER.
  Consult the [parked IEEE recovery record](wiki/drafts/designs/write_only_borrow_ieee_store_branch.md)
  before duplicating work; its unpublished tip is not available in this checkout.

  Acceptance: express repaired limitations as runnable `write_only` corpus
  cases (the `run` tier or `*_exit`) executing against caller storage. Cover
  exact width, untouched neighbors, runtime signed/Boolean/floating sources,
  restoration/return custody, non-observation and independent replay. Reject
  reads, bare write-only forwarding, readable widening and overlapping
  exclusive arguments. General replacement must preserve displaced custody and
  whole-value validity, not merely decompose more source patterns. Observe
  computed stores in the original caller, not a copied frame home. Run
  `python3 tools/corpus_gate.py --native --filter write_only` on matching hosts,
  including both Linux targets and Windows/macOS. Cross-publication is not
  execution.

- **STRUCTURAL-BORROW-IDENTITY.** Complete source-owner/projection routes under
  [structural access](wiki/spec/terminal-psi/structural_access.md), retaining
  original caller storage through call preparation, native placement and
  independent receiving replay. Owners: checked execution's `receiver_calls`
  and `calls/computation_arguments`, lowered structural calls, target
  `structural_call_arguments` and image `argument_custody`.

  Reuse static `Field`/`FixedIndex` and `RuntimeIndex { index, obligation }`
  path transport.
  Remaining `src/tests/borrow/receiver_access.rs` pins cover local-rooted
  indexed receivers and runtime-indexed parameter receivers/explicit shared
  arguments. An unverified candidate for explicit shared dynamic-indexed arguments
  is `ecfdedb34b (revision absent from the repository)` on `swarm/linw4-structural-borrow-identity`; rebase and run
  focused borrow checks before adopting it. Coordinate bracketed-range migration with
  CANONICALIZE-SCALAR-RANGE-CONTRACTS. Owned-root and construction-local admission
  remain separate obligations; reuse `terminal-semantics::static_path`.

  Acceptance: repair the omission pins and execute caller-visible
  projected/forwarded writes, owned-field mutable/write-only subloans, legal
  synchronized shared observations and register/stack reference passing.
  Reject copied borrowed homes and substituted access/type/projection/placement
  evidence; shape equality does not create authority or a standalone field
  type. Complete matching-host coverage, especially the unrecorded Linux
  AArch64 and Windows legs; another callee observing a staged copy is not
  caller writeback.

  Preserve declared borrowed-receiver custody when an attached scalar-returning
  callee does not read `self`: the scalar Graph signature still omits it, while
  call selection prefers Graph over Operations. The Operations receiver fix
  therefore does not close this route. Caller operands and selected callee
  signatures must agree without a dummy field read. Exercise unread and observed
  receiver bodies through Terminal production and native execution, retaining
  duplicate-overload rejection.

- **BORROW-PROOF-CONVERGENCE.** Carry ordinary borrow compatibility from
  checked certificates to independent portable replay under
  [loans](wiki/spec/terminal-psi/loans.md).
  Owners: `04_typed-trees-to-checked-trees/src/checks/borrows/`,
  `omega-rust/psi/representations/checked-trees/src/checked_trees/borrow.rs`, checked-to-lowered publication
  and Terminal verification. Current checking supports immutable normalized
  bounds, requires/domain predicates, incoming guards, immutable whole-result
  call guarantees and transparent propositions. Two premises can already
  compose through a shared middle. These certificates remain checked-stage
  records, not portable authority.

  Retain exact formation, captured places/resource identities, source
  establishment and consulted premise order through publication. Reconstruct
  availability, dominance and value/place versions independently. Broader
  callee/domain predicates, mutable or projected results, same-statement and
  theorem-call establishment need exact substitution/version evidence.
  Reuse existing contract/range readers, including the original assignment's
  exact call occurrence; do not collect a second set of guarantees. Include
  adversarial call-prerequisite removal/replacement and cross-call copy
  substitution in the replay controls.

  Read-footprint expansion is paused for lack of a demonstrated customer:
  first recheck the recorded `items[low + 0u64..high]` lost-bound candidate.
  An incomplete footprint conservatively retires facts after writes; admitting
  more expression kinds without preserving a needed source fact is not progress.
  The builtin bound-meaning gate is distinct from the read-footprint check.

  Acceptance: each new establishment has source pass coverage for disjoint
  loans, writes and exclusive call operands, with independently replayed
  certificates. Missing, stale, reordered/substituted or insufficient premises,
  overlapping mutation and containment-only second exclusive loans reject.
  Proof never creates, widens, duplicates or extends authority. Preserve
  `src/tests/borrow/checks/premised_disjoint_writes.rs`, the `certificates/`
  suite and stated, sum, guarded, domain, returned-window, proposition and
  chained-premise corpus controls. CANARY-CORPUS routes borrow-obligation
  failures here; these are implementation gaps, not unsettled language rules.

- **CALLBACK-PRIVATE-MATERIALIZATION.** Complete native realization under
  [private callbacks](wiki/spec/build/private_callbacks.md). Reproduce and repair
  `callback_terminal_custody::direct_callback_relocation_resolves_to_its_private_function`'s
  reported `fragment import lacks selected call custody` failure. Restore the
  evaluated `ProgramStorageEntry::enter` calling plan in
  `a_package_local_calling_copy_rejects_beside_the_standard_library_entry` so
  that negative reaches its intended check. Neither reported failure establishes
  native artifact, final-address, or installation acceptance.

  Remaining: multiple callbacks and layout-field destinations; authenticated
  complete plan applications and independent authored-use-to-Terminal-operation
  correspondence; ordinary callback bodies and inbound signatures through
  proposal validation, native lowering and image replay. Psi's
  `machine_lowering/bounded_callbacks.rs` already delegates ordinary machine
  lowering. The remaining exact single-`u64` identity/Unit recognizer is in
  `omega-rust/omega/compiler/src/terminal/native_proposal/mod.rs`;
  native thunk/image consumers also reject call-bearing bodies. Replace those
  shape restrictions with requirement, ABI and call-custody checking, not more
  admitted body families. Close hosted private-stack callback occupancy and
  ordinary-product admission restrictions only with their missing custody.

  Owners: native proposal construction, `native-realization`'s
  `retained_native_product` and `callback_thunks`, selected-call ABI transport,
  and image private-function/relocation replay. See
  [receiving custody limits](omega-rust/omega/compiler/README.md#callback-custody-boundaries).
  Acceptance: the direct witness and
  `source/library/std/tests/callback_materialization_closure.omg` two-slot
  registrar produce native images binding exact function, symbol, relocation,
  executable region and destination through object/final replay. Missing,
  duplicate, reordered, substituted, overlapping or incompatible materializations
  reject. Private slots remain inaccessible to source. Registration outcome
  and lifetime belong to REGISTERED-CALLBACK-LIFETIME.

- **REGISTERED-CALLBACK-LIFETIME.** Complete an authored registrar lifecycle
  under [registration and lifetime](wiki/spec/build/private_callbacks.md#registration-and-lifetime)
  and [opaque retention](wiki/spec/build/component_publication.md#opaque-retention-and-quarantine).
  Owners: checked execution/result planning, Terminal result qualification and
  claim replay, installed-provider interpretation, and
  `component-publication/src/callback_registration.rs`.
  Routed-domain authorization of registration sum payloads, interpreted
  register/unregister ledger integration and installed-provider minting of bare
  linear result claims exist; do not reopen the resolved payload-authorization
  question.

  Carry the success/rejection sum through executable planning, case-conditional
  qualifications/claims and an authored provider/customer. The installed-provider
  path still rejects projected result qualifications and admits affine results
  only without claims/qualifications
  (`terminal-interpreter/src/calls.rs`).

  Reuse `validate_projected_qualification_roster` in
  `terminal-verifier/src/validation/foundation/machine_foundations.rs` and the
  interpreter's `live_claims` custody. Removing the empty-qualification gate is
  insufficient: the authored customer must establish correct case-conditional
  evidence and reject a wrong roster.
  Success must join the exact live-registration capacity occurrence to the
  external root and code/component leases; rejection preserves that capacity
  without a root. Teardown requires quiescence before lease release.
  Capacity counts live registrations, not emitted thunks; successful teardown
  returns the same capacity occurrence.
  Use ordinary custody, not registration-specific checker rules.

  Acceptance: one authored program rejects, retries, replaces using returned
  capacity, and unregisters. Dropped, repeated, stale/cross-occurrence
  registrations and spent-capacity reuse reject. Preserve
  `05_checked-trees-to-lowered-psi/tests/registered_callback_lifetime.rs` and
  the interpreted ledger tests, but manual ledger sequencing or injected
  provider-conformance rows do not close this authored acceptance.
  CALLBACK-PRIVATE-MATERIALIZATION supplies the native entry; FFIVAL owns
  the Windows foreign-invocation customer. Other hosts must report that leg
  unavailable rather than claim execution.

- **FOREIGN-RETAINED-ARGUMENT-BACKING.** Connect authored retained-argument
  custody to executable native backing under
  [outbound custody](wiki/spec/build/foreign_storage.md#outbound-custody).
  Owners: checked content/call planning, checked-to-lowered retention,
  Terminal boundary validation, native call-site marshaling, and
  `omega-rust/omega/backend/runtime/external-roots/src/program_local/program_local_extents/retained_foreign_arguments.rs`.

  Shared retained-borrow custody attaches to the invoked Terminal boundary;
  verification checks its exact shared source and retained result loan.
  `retain_foreign_argument_under_custody` selects shared lifetime retention
  from that row, but still has only test callers. Complete the authored
  source-to-Terminal-to-native call-site connection, materializing pointers
  only from established stable root/range/access/lifetime/revision-or-lease
  custody. Reuse the admitted result-`ensures`, nominal-match and structural-result
  catalog path rather than recreating those gates.

  Add the permitted-snapshot source/checked contract and persistent demand per
  live occurrence; copying must be explicitly allowed, without invented identity
  preservation or write-back. Connect moved backing through CONSERVATION-CONTRACT
  and TERMINAL-CONTENT-CLAIMS. Moved/snapshot registry helpers are test-only;
  promote a route only when authored custody selects it.

  Acceptance: authored moved, lifetime-borrowed and permitted-snapshot boundaries
  run natively. Completion redeems moved backing; a live shared loan excludes
  conflicting writes. Call-scoped, mutable lifetime-borrowed or ambiguous retention and
  unknown/stale/out-of-range/excess-rights backing reject. Preserve
  `retained_content_custody` and `retention/retained_borrow_custody` controls.
  Registration-specific lease custody remains REGISTERED-CALLBACK-LIFETIME.

- **TERMINAL-SLICE-VIEW-VOCABULARY.** (split-of:SLICE-VIEW-LOCAL-ENTRY-ESTABLISHMENT)
  Complete consuming non-byte `&[T]` calls and state transfers through Terminal
  production and native execution under
  [view custody](wiki/spec/terminal-psi/byte_views.md#element-typed-slice-views).
  Preserve exact backing, element identity, extent and loans through length,
  indexing and subslicing. `slices/callee_non_byte_view_len_index_subslice`
  exercises all three in source but has no build entry: bind an executable
  customer and verify its result rather than treating its `checked` corpus row
  as realization. An empty forwarding callee is insufficient.

  Reuse stage 04's `execution/terminal_unit/calls/view_subslice.rs` and stage
  05's `unit/attached_unit/view_ranges.rs`, `emission/operation_emission/view_subslice.rs`
  and `tests/view_locals.rs`. They share `CheckedSubsliceSite` and established
  view roots. Element-read paths now reach Omega's target/legalized operations;
  `runtime_slice_index_read_dispatch_exit` and `runtime_slice_record_field_offset_exit`
  record macOS ARM64 exits 43/61. Preserve whole-element stride, leaf offset/width
  and independent replay. Signed indices already use `exact_u64_index`'s
  proof-checked conversion; `runtime_slice_indexed_read_exit` records exit 70.

  Remaining joins:
  - Carry same-view bounds through state arrivals and guarded successors.
    `runtime_slice_fixed_index_guard_exit` and
    `runtime_slice_element_runtime_index_read_exit` record `OperationProofUnavailable`;
    the latter must retain the predecessor's `i < s.len`, not substitute
    `i <= 3` for a view-length proof.
  - Complete whole-record copies under
    [ownership](wiki/spec/language/ownership.md). Stage 04's
    `values/scalar/computations/structural_values.rs::view_element_copy` admits
    plain scalar fields of `[copy]` records; stage 05's `view_locals` verifies
    their per-field reads and `EstablishRecord`. Bounded, erased and structural
    fields need their own evidence. Confirm rejection of affine moves from
    shared views, then give the positive `runtime_slice_index_copy{,_dispatch}_exit`,
    `runtime_subslice_range_pointer_exit`, `runtime_nested_subslice_fixed_index_exit`,
    `runtime_slice_index_transition_exit` and `runtime_slice_iteration_exit`
    fixtures the required `[copy]` declaration and execute them.
  - Establish byte views over fixed byte fields for `runtime_subslice_len_exit`
    and `wire/runtime_wire_encode_byte_slice_exit`; `field_collection` currently
    admits only non-byte element views. Preserve the wire fixture's five exact
    bytes and count, not merely its descriptor shape.
  - Preserve dynamic, bounded, end-only and nested subslices through ordinary
    control sequencing. `runtime_subslice_dynamic_index_exit` calls its named
    state as `choose(tail, 1);`: migrate that call to the explicit transition
    required by [state contracts](wiki/spec/language/state_contracts.md), then
    verify the view route. Its recorded dependency cycle does not justify
    callable internal states.
  - Verify consuming mutable-view forwarding on the Unit-graph route with exact
    reborrow places available at successors. Reuse `tests/unit_state_graph/bindings.rs`
    for byte-view controls.

  Acceptance: independently verify emitted artifacts, execute consuming views
  natively and reject bounds, element-type, alias and access violations.
  Terminal interpretation in `terminal-interpreter/src/element_views.rs` still
  stores scalar elements and rejects record paths; checked-interpreter agreement
  does not cover that route. Preserve `fletcher_checksum` exit 56 and
  `recursive_sum` sum 50/count 4, exit 70 on Linux x86-64; **SAMPLE-CORPUS** owns
  their complete sample obligations. **GENERAL-CYCLIC-EXECUTION** and
  **STATE-LOCAL-VALUE-FRONTIER** own general control/value sequencing.

- **PSIIR.** Complete the remaining source-free execution and
  [logical-work bounds](wiki/spec/resources/logical_work.md) over the same
  canonical Terminal product, independent reconstruction, interpretation,
  native realization and installation. Operation-specific gaps stay with their
  owners and **TRANSLATION-VALIDATION** on `TASKS_OPTIMIZER.md`; optional work
  bounds must not delay otherwise legal native programs or add runtime metering.

  Reuse `terminal-fixed-fuel`'s
  [outcome bounds](omega-rust/psi/representations/terminal-psi/fuel/certification/src/fuel_certification/outcome_bounds.rs):
  acyclic maximum paths, ranked components, conditional/relational contracts,
  linear integer math with multiple variables and carrier extremes, verified
  block invariants and reconstructed entering-edge guards already contribute.
  Premises retain their contract/invariant/guard origins during derivation;
  certificate replay recomputes subject, schedule, used propositions and ceiling.
  Remaining:

  - Extend inference beyond linear forms and constant shifts, conservative
    carrier collapse and nonnegative transfer costs. Computed arrivals,
    structural payloads and unbounded incoming paths currently retain the type
    maximum unless checked facts cover every entry. Preserve exact premise
    provenance, overflow refusal and recompute-and-compare certificates.
  - Retain finite wait/foreign-completion contracts and exact unavailable-bound
    causes. A boundary with no in-module candidate currently contributes no
    callee work; that is not a finite response guarantee. Preserve
    `InvocationBoundCallee` and component-specific `Unranked`, `UnboundedRank`
    and `OpenCalleeSet`. `omega/src/inspection/evidence.rs` still treats the
    flat open-callee case as an error; distinguish valid unavailable bounds
    from malformed identities or broken ranking evidence.

  Acceptance: discard source/producer state, decode and independently verify,
  then interpret/lower the same artifact with exact resources and installation
  custody. Exercise tightened bounds and mutated premises through this route;
  raw-module inference tests alone do not cover it. Keep whole-entry,
  segment and analysis-catalog evidence distinct. Native/ABI claims need final
  realization evidence, never API metadata or opaque supply. Maintain the
  [closed encoding](wiki/spec/terminal-psi/encoding.md) with operation changes;
  codec fixture drift belongs to repository closure. **PCC-PRODUCT-PUBLICATION**
  owns command publication and the remaining optional-proof envelope migration.

- **PCC-PRODUCT-PUBLICATION.** Deliver requested Psi and standalone native
  evidence under [optional proof products](wiki/spec/proofs/publication.md),
  after the immediate executable customers. Keep both requests off by default,
  root-owned and independent of receiver policy or ordinary compiler checking.

  - Expose retained Terminal publication through `omega/src/compilation/`:
    `ProjectProduct` has only Check/NativeArtifact, and `publish_compilation`
    rejects Terminal reports. Connect `CompileReport::publish_retained_terminal_artifact`
    to an authored project/CLI customer publishing `.psi` and optional
    `.psi.proof` without native emission. Preserve check-only PCC rejection
    and Terminal-stop rejection of native PCC. Migrate `terminal-codec`'s
    `canonical_artifact.rs`, which still requires an embedded proof section,
    to the specified optional sidecar distribution without weakening ordinary
    consumption or silently changing existing format markers.
  - Complete native behavioral evidence in
    [native-artifact's proof owner](omega-rust/omega/backend/artifacts/native-artifact/src/proof.rs).
    Placed image/data inventories, entry/load-map custody and import-thunk
    replay exist; even valid evidence returns `Incomplete(UnsupportedEvidence)`.
    Decode instructions against closed target semantics, reconstruct all
    control-flow/entry obligations, and establish premise availability and
    Terminal-to-native correspondence or direct native proofs. Publication's
    `validate_native_pair` accepts the bounded incomplete sidecar; a publication
    receipt must not be presented as completed behavioral verification.
  - Exercise actual artifact/companion request-on/off replacement and retry
    through `platform-custody/src/file_publication.rs`. Its tests already cover
    staging failure preserving prior destinations. Installation is sequential:
    later failure removes the installed prefix without restoring overwritten
    files. Preserve truthful failure, cleanup-error reporting and stale-sidecar
    removal; never report a partially installed pair as successful. Connect
    these controls to proof-product publication.

  Acceptance: verify from artifact/companion bytes and independently pinned
  receiver dependencies after deleting source and producer state. Native
  verification must also work without separate Psi. Arbitrary native bytes plus
  valid Psi/recomputed producer hashes, tampering, relabeled profiles and stale
  sidecars reject. Bind final bytes
  after byte-changing finalization and check permitted loading/relocation
  correspondence. Preserve exact dependency identities and receiver-owned
  guarantees, premises and admission. Unsupported legs remain `Incomplete`;
  broader guarantees depend on **PROOF-KERNEL-CORE** and
  **PROOF-CERTIFICATION-BRIDGE**, not a policy DSL or reporting-layer checker.
  **MACOS-APPLICATION-PUBLICATION** owns actual bundle execution.

## P5 - Cathedral over general Omega primitives

- **BUMP-ALLOCATOR-CANARY.** Deliver an executable ordinary package allocator
  over qualified `Extent` under [allocation](wiki/spec/resources/allocation.md),
  supplying **VEC-NATIVE-GROWTH** and Squalr. Reuse
  `source/library/alloc/bump.omg` and `pass/memory/bump_allocator_canary`.
  The fixture selects `BumpRootProvider`, but its entry is empty and it selects
  no executable partition/resident-storage provider. `BumpVec` reserves extents
  without elements, and `RetiredSlot` holds only one old buffer.

  - Complete the invoked `ExtentPartition`/conservation route through
    **CONSERVATION-CONTRACT / TERMINAL-CONTENT-CLAIMS**. Prove aligned request
    geometry and the counted-residual/tail relationship; returned length laws
    alone do not establish fit. Preserve exact alternatives, caller substitution
    and independent custody reconstruction.
  - Replace fixture-local `ResidentStorage` with **PLAN-LAID-VIEWS**' evaluated
    layout/access, establishment, element transfer and retirement. Core's bare
    `Vacant`/`Resident` declarations need authorized establishment routes under
    **DOMAIN-ISSUER-ROUTES**. Enforce those declarations and exact resident indices;
    the current cast-forgery diagnostic relies on a heuristic about the
    carrier's other domains. An empty tag or linear carrier does not establish resident custody,
    and live residents cannot merge as vacant.
  - Implement temporary exclusive strategy borrowing through
    **BORROWED-STORAGE-RESTORATION**, ending at allocation return so allocations
    coexist. The existing API moves the strategy through result records.
  - Implement general retired storage, traversal and full recomposition.
    Current `release`/`shrink` immediately restore the tail; specified monotonic
    release cleans values and retains returned extents until reset. Tail-adjacent
    reuse does not satisfy that contract, and bulk reset cannot waive cleanup.

  Acceptance: supply real backing and execute coexisting allocations, unchanged
  state on failure, exact cleanup/return, and reset/finish to original backing
  only after all allocations and residents return. Run in the interpreter and on
  a supported native host, recording unavailable hosts separately. Live reset,
  dropped custody, double placement, wrong resident indices and unrouted
  introduction reject; retain `fail/memory/bump_allocator_*` and live-resident
  controls against the completed routes. Empty entry or finite checked-only
  reservations do not close executable acceptance.

- **UEFI-PHYSICAL-SEMANTIC-ENTRY.** Execute the source-authored
  [two-surface bootstrap](wiki/spec/build/uefi_entry.md#authored-firmware-definitions-and-adapters).
  The target package owns firmware layouts, integrity, calling policies and
  bootstrap bodies; the compiler supplies the physical shell and generic
  ABI/entry primitives. Reuse `std/targets/uefi_x86_64/tables.omg` layout replay,
  `program-entry-plan` physical/semantic plans and
  [native-artifact's receiver wrapper](omega-rust/omega/backend/artifacts/native-artifact/src/fragment_object_publication.rs).
  Connect them to actual arrival and scoped providers through
  `external-roots/src/platform_bringup/uefi_bootstrap/`.
  Supply a source customer and firmware/controlled-provider execution harness.

  Acceptance: authored integrity/projections/calls lead from physical arrival
  to the exact selected continuation. Establish both image and initial-storage
  roots together, excluding live stack/receiver/bootstrap storage; preserve
  disjoint ownership and derived stack demand. Returning applications release
  adapter allocations and return the correct status; rejection retains inputs
  without entering the continuation. Malformed geometry/header integrity,
  foreign occurrence, wrong package/target and overlapping roots reject.
  Independently replay source, plan and realization with scoped firmware
  authority. A layout commitment, readiness ledger or emitted semantic wrapper
  alone does not establish bootstrap execution. **UEFI-OS-HANDOFF** also needs
  exact nonreturning entry/stack-transfer and firmware-return mechanics;
  repair general capabilities in their owners without firmware-protocol intrinsics.

- **UEFI-OS-HANDOFF.** Execute the authored
  [Boot Services-to-OS handoff](wiki/spec/build/uefi_entry.md#returning-application-versus-os-handoff)
  through `tests/omega/pass/build/uefi_os_handoff_invocation` and
  `std/targets/uefi_x86_64/handoff.omg`. The acquire/grow/adopt and bounded
  stale-key cycle exists. Native selection still lacks closed execution
  identities for `UefiOsHandoffTermination::{transfer,firmware_return}` in
  `compiler/selected-dispatch/src/compiler_intrinsic.rs`.
  **UEFI-PHYSICAL-SEMANTIC-ENTRY** owns these mechanics, layouts and scoped
  firmware leaves.

  Complete authored custody/validation: `acquired` currently sends any success
  straight to exit without validating descriptor geometry/version; `free`
  adopts the new buffer without checking release status. Preserve both
  allocations on rejected release and retain freshest map/key, allocation
  lineage, final-map obligations and a stack that survives Boot Services exit.
  Keep allocation/status/retry policy in ordinary target code; replace
  superseded Rust sequencing in `uefi_bootstrap/os_handoff_cycle.rs` once covered.

  Acceptance: native emission followed by firmware/controlled-provider execution
  covers growth, stale-key retry, bounded exhaustion with the target error,
  rejected allocation/release, malformed maps, stale/foreign evidence, lost
  custody and forbidden Boot Services use after exit. Success enters the exact
  selected OS entry without returning. Reuse `os_handoff_cycle/tests.rs`,
  `get_memory_map/` and `exit_boot_services/` controls, which exercise Rust
  drivers/fake firmware rather than this emitted source. A bodyless provider,
  multi-call intrinsic or fallible cleanup destructor cannot replace the cycle.

- **AP-BRINGUP.** Execute Cathedral-owned secondary-processor startup under
  [external roots](wiki/spec/build/external_roots.md#secondary-processor-startup).
  Cathedral owns discovery, dispatch, acknowledgement, retry and cancellation;
  selected hardware boundaries supply premises. The compiler checks installed
  entry, placement, resource custody and evidence.

  `tests/omega/pass/memory/secondary_processor_canary` has authored entry,
  acknowledgement and roster declarations, but an empty `Main::main`.
  Connect an invoked customer to the emitted trampoline in
  `backend/machine-emission/src/startup_trampoline.rs`, the
  `external-roots/src/platform_bringup/secondary_processor.rs` ledger and real
  provider receipt ingress through **BOUNDARY-ISSUANCE**. Ledger tests use
  replacement bytes and Rust-created receipts; retain them as custody controls.

  Acceptance: execute the emitted trampoline and exact installed entry on a
  matching platform/emulator with dedicated, nonoverlapping stack/state.
  Bind provider profile, arrival regime and mechanism-specific vector/low-memory
  geometry to the selected contract. Definite nondispatch permits withdrawal;
  unconfirmed dispatch holds resources and admits a later exact arrival.
  Cancellation needs exact outstanding-invocation evidence of no future arrival
  and no stack/state execution; started retirement needs exact quiescence.
  Exercise timeout/late arrival, cancellation before confirmation, races,
  foreign/stale/replayed evidence, resource conflicts and retirement.
  Timeout releases nothing; hold resources while any attempt can reach them.
  Report unavailable execution legs explicitly; no compiler-owned APIC driver.

- **ADDRESS-TRANSLATION-CANARY.** Execute Cathedral-owned mapping
  installation and teardown under
  [mapping and reclamation](wiki/spec/resources/extents.md#mapping-and-reclamation).
  The corpus stand-in at `tests/omega/pass/memory/address_translation_canary`
  declares `CathedralTranslationProvider` requirement coverage and threads
  the owned `Mapping` through table install/remove before activation/release.
  It remains checked-only: `Main::main` is empty and bodyless hardware/receipt
  crossings have no selected executable provider joining the authored
  obligations to `extents::mapping` receipts.

  Finish that provider/receipt connection for exact source/destination custody,
  mapping identity and era, including borrowed-source carrier transport.
  Carried-loan mutation ceilings exist; borrowed install/remove routes retaining
  return/field qualifications remain missing. Reproduce their current diagnostic
  rather than assuming the fixture header's broad carried-loan blocker.
  Extend the one-leaf routes to multi-page mappings with ordinary ranked loops.
  TERMINATION-RANKING-CHECKS owns contract-aware boundary-call preservation;
  nested checked-body calls in inert initializers are already supported.
  Demand-grown tables depend on BUMP-ALLOCATOR-CANARY, placed read-back on
  PLAN-LAID-VIEWS, and fixture range migration on CANONICALIZE-SCALAR-RANGE-CONTRACTS.

  Validate Cathedral's entry encoding and geometry with a semantic/execution
  oracle: `cathedral/tables.omg::FRAME_MASK` must preserve frame bits 12..51
  (`0x000FFFFFFFFFF000`) without admitting flag bits. The checked-only fixture
  does not distinguish a correct mask from a wrong one. Select authority,
  hardware and shootdown realizations on a freestanding image through
  UEFI-PHYSICAL-SEMANTIC-ENTRY and UEFI-OS-HANDOFF, with a QEMU harness.

  Acceptance: install/tear down multi-page mappings in QEMU with explicit
  `Extent` and TLB custody. Mapped access exists only after activation and
  before unmap; reuse waits for completion. Source use after map, premature
  unmap, lost shootdown, forged carriers, borrowed-source reclaim, replayed
  activation and wrong-mapping/stale-era receipts reject. Preserve the eight
  `fail/core/translation_*` controls. A checked fixture or Rust receipt test
  is not execution. Cathedral owns the real package, formats, walk policy and
  lifecycle; no page-table/TLB types belong in the compiler.

- **EXCEPTION-ROOTS-AND-TIMER.** Execute Cathedral's fatal entries on
  dedicated critical stacks and a timer handler that acknowledges, records
  and wakes ordinary work, under [interrupt obligations](wiki/spec/build/interrupt_obligations.md),
  [hardware materialization](wiki/spec/build/hardware_materialization.md)
  and [final machine-state evidence](wiki/spec/build/machine_state_evidence.md).
  Cathedral owns exception coverage, gate/IST policy, controller/device
  protocols, table validation and fatal behavior. Compiler mechanisms own
  sealed installed roots, checked writers/instructions, entry/exit realization
  and custody.

  Reuse `tests/omega/pass/memory/interrupt_table_canary` and
  `compiler/tests/layout_plans/interrupt_descriptor_tables.rs`: authored
  root selection, table layout, member admission and validation exist, as do
  stub emission/replay with GPR/XMM saves, stack fragments and indirect copies.
  The source entry is empty; tests supply installed bytes, authority and
  resource receipts. Remaining:

  - Join `machine-emission` stub bytes and member-call relocation to real image
    text, installation and external-root admission. Bind exact entry/body
    coordinates, installed bytes, stack overhead and final transitive state
    evidence; retain separate stack, fuel and state columns.
    `X86_64DeriverStubEntryEmission` currently has only test producers.
    Unsupported ABI/backing must reject, not fabricate borrowed caller storage.
  - Complete authored table publication and remove compiler-owned table-policy
    fields/checks from `InterruptTableGateDescriptor`,
    `InterruptTableProfile` and established/publication records. Reuse the
    package validator/member verdict and generic writer; retain exact content,
    installed-root, publication-authority and checked-`lidt` joins. Root custody
    and the instruction contract remain compiler-owned.
  - Replace acknowledgement-only fixture bodies with Cathedral's fatal halt,
    controller/timer setup, tick record and wake behavior. Join completion to
    exact acknowledgement lineage through BOUNDED-INSTALLATION-REACH-ROWS and
    TOP-LEVEL-BOUNDARY-REQUIREMENTS. Shared inherited requirement identities
    must keep resolving under their own selected provider plan.
  - Integrate UEFI-PHYSICAL-SEMANTIC-ENTRY / UEFI-OS-HANDOFF, TR3-TR8 stack
    provisioning, and a reproducible QEMU harness. Keep external interrupts
    disabled until the exception floor is installed; NMI, machine-check and
    physical-failure assumptions remain explicit.

  Acceptance: QEMU boots the authored customer, reports timer ticks through
  owned output, halts between ticks, and sends a deliberately raised fault to
  its fatal handler on the dedicated stack without resuming ordinary work.
  Missing members, wrong writer/realization or changed table bytes, replayed
  publication/acknowledgement, forgotten/double completion, and retirement while
  the table names the root reject. Ledger tests and uninstalled stub bytes are
  not the witness. Record emulator/firmware prerequisites and unavailable hosts.

- **BOUNDED-INSTALLATION-REACH-ROWS.** Finish
  [installation-bound reach](wiki/spec/build/external_roots.md#installation-bound-reach)
  for component contracts and opaque-carrier completion. Selection and nested
  closure substitution already exist in `provider-planning/installation_reach.rs`
  and `omega-rust/omega/backend/runtime/external-roots/src/root_entry/root_validation.rs`; preserve the selected
  nested-row and unresolved-row controls in `calling_policy_plans/opaque_boundaries.rs`.

  The shipped `InterruptAcknowledgement::complete` in `core/interrupt.omg`
  still declares fixed `reaches PortIo`, and its test pins that limitation.
  Migrate it to the ratified bound beneath `MachineControl + PortIo` together
  with a selected satisfier that discharges the linear receiver and retains
  exact provider execution, policy and token lineage. TOP-LEVEL-BOUNDARY-REQUIREMENTS
  owns source requirement selection/discharge; owned receiver dispatch already
  exists, so do not recreate it. A test-local copyable lookalike does not close
  the real linear-carrier route.
  COMPONENT-SUBSTRATE supplies the admitted component contract needed to reject
  unresolved exported rows. This item owns the bound/substitution integration,
  not another component implementation.

  Acceptance: the shipped core requirement resolves PIC completion to `PortIo`
  and LAPIC/x2APIC completion to `MachineControl` through source, Terminal Psi
  and `InstalledInterruptCompletionRoute`. Cross-provider settlement despite
  equal rows, replayed token/era, an Independent component with an unresolved
  export, and final admission with any unresolved row reject. Conservative bounds
  grant no authority; exact selected execution and lineage authorize invocation.

## Parallel language and compiler lanes

- **IMPLICIT-STORE-CONVERSION.** Finish diagnostic cleanup and migration
  verification under [numeric carrier rules](wiki/spec/language/numeric_values.md)
  and [explicit coercion/erasure](wiki/spec/language/domains.md#exact-coercion-and-erasure).
  `omega-rust/psi/checking/src/proof_contracts/arithmetic_domains/assignments.rs`'s
  `check_narrowing_assignment` already rejects typed carrier/policy changes and
  names `as`, even when the source range fits. Preserve integer widening,
  signedness, float-format and policy-add/drop rejection; anonymous literal
  landing remains distinct. Exact `as` requires representability and unchanged
  denotation; diagnostics must not recommend it as unchecked truncation.

  Coordinate `src/program_validation/statements/`, `src/machine_calls/calls/argument_bounds.rs`
  and `src/proof_contracts/domain_weakening.rs` within that checking crate to avoid
  duplicate conversion/weakening reports and cascades after a primary type,
  suffix or unresolved-name error. Current corpus records retain those extra
  reports in **CANARY-CORPUS**'s diagnostic cases. Preserve their actual primary
  subjects and independent errors; do not blanket-suppress domain obligations.

  Recheck the migration through current source/package inputs: the sample
  cohort in **SAMPLE-CORPUS**, corpus fixtures, bundled libraries and
  `source/omega/main.omg` with `source/psi/{lex,parse,source}`. Many casts,
  including dungeon return/transition arguments, are already present; old
  refusal counts and masked downstream failures are not a remaining-work list.
  Use focused `tools/corpus_gate.py` / `tools/sample_gate.py` runs and the
  product/parser-gate CLI coverage in `omega-rust/omega/omega/tests/command_line.rs`.
  Preserve the algorithms and the policy of each operation when inserting casts.
  **SAMPLE-CORPUS** and **RC-BUILD-AND-PACKAGES** retain complete program acceptance.

  Keep the float-policy customers with **OPERATOR-BOUNDARY-CALLS** /
  **FLOAT-PROVIDERS**: `tests/omega/pass/float/{float_saturating_arithmetic_exit,float_trapping_overflow_traps,float_trapping_propagated_nan_traps,named_provider_multiply_then_add_exit}`.
  Their recorded argument-conversion refusals overlap named-operator admission;
  its weakening checker retains arithmetic policy while its numeric argument
  checker compares the bare parameter policy. Reconcile ordinary selected
  contracts and realization without erasing Saturating/Trapping to silence the
  diagnostic. Preserve two-rounding multiply-then-add, saturation and trap oracles.

  Acceptance: fail fixtures `tests/omega/fail/arithmetic/implicit_store_{conversion,widening}_rejected`
  retain actionable checking errors; explicit exact casts, Wrapping whole-array
  literals and `tests/omega/pass/slices/runtime_saturating_array_element_guard_exit`
  retain their valid behavior. Migrated negative fixtures
  must still reach the intended obligation, as in
  `argument_widening_does_not_reinterpret_saved_wrapping_computations` under
  `omega-rust/psi/pipeline/04_typed-trees-to-checked-trees/src/checks/contracts/call_bounds/tests/widening.rs`.
  Every ordinary cross-carrier/policy store in maintained pass fixtures and samples
  must spell its required conversion. Complete diagnostic and migration evidence
  without treating checked admission or a later lowering refusal as native success.

- **FILTERED-CALLEE-CALL-SITES.** Finish command-level acceptance for calls
  whose target-scoped callee is unavailable to a realization. Current code has
  `reject_inert_sibling_callers` in compiler build-evaluation's
  `admission/target_machines.rs` indexes machine and entry-state symbols,
  walks statement and expression calls, and reports the caller, callee and
  realized target. A caller scoped to the callee's own unrealized target is
  exempt. Do not add a blanket invalid-symbol rejection or receiver-type lookup;
  legitimate trait/provider calls can remain unresolved at earlier passes.

  Remaining evidence: `fail/targets/single_target_internal_caller_rejected`
  covers a value-call initializer and records the expected refusal.
  `pass/targets/single_target_internal_machine_skipped` declares `demo_target`
  and never calls its helper; it does not exercise the exemption. The two
  previously cited target-machine unit tests are gone. Cover an ordinary
  statement call, the value call, and a same-target caller/callee pair using a
  real foreign profile and actual Build entries; repeat through a dependency
  package. Assert diagnostic names and the positive program's execution, not
  merely the presence of an unused declaration. The old UEFI handoff witness
  is now an unscoped boundary machine and cannot establish this behavior.

  Keep these controls with **PROVIDER-SELECTION-AFTER-TERMINAL** as selection
  moves into Omega: every body remains checked and retained, and a missing
  called body rejects only the affected realization. Retire this item when
  the executable acceptance is accounted for.

- **BORROWED-STORAGE-RESTORATION.** (split-of:OMEGA-PRODUCT-COMPILER-SOURCE)
  Complete consuming-transform/replacement execution under
  [borrowed-storage invariant windows](wiki/spec/language/ownership.md#borrowed-storage-invariant-windows)
  and [Terminal restoration](wiki/spec/terminal-psi/ownership.md#borrowed-storage-restoration).
  The guide move-out/restore body already produces checked Move/Store rows and
  verified Terminal operations, including reference-local aliases. Reuse
  `execution/terminal_unit/borrowed_windows.rs`, emission's `BorrowedWindowLedger`
  and Omega's `lowering/machine/operation/borrowed_windows.rs`.

  Replace the producer's leading move-local/restore-only recognizer with
  ordinary operation sequencing and explicit place/value custody. Connect
  consuming calls and replacement values, disjoint sibling work and multi-block
  joins. Independently reconstruct equal open-place frontiers and closure at
  required exits, retaining evaluation order, nested/live-arm agreement,
  contained loans and outcome obligations. Whole-root, indexed, referent,
  scalar-field and nominal-drop fences stay until their evidence exists.

  **CANARY-CORPUS** owns the shared-reference copy-composition repair, not this
  row. A record such as `data Room { label: &[u8] in Utf8; }` copied from
  `&mut self` storage needs an explicit `[copy]` declaration; the current field
  whitelist wrongly rejects that declaration. See the settled
  [copy/loan rules](wiki/spec/language/ownership.md).
  `text/runtime_local_struct_string_field_concat_exit`,
  `text/runtime_string_stored_suffix_exit` and
  `text/runtime_slice_indexed_string_guard_exit` also exercise preservation of
  default-domain facts. Restoration of a deliberately moved-out value is a
  separate mechanism; do not work around the copy-composition gap with an
  unnecessary move-and-restore.

  Complete target/native realization without replacing caller storage by a
  staged copy. Promote `ownership/move_keyword_field_assignment` from
  checked-only when its full-product route succeeds. Reobserve that command's
  current failure rather than retaining the retired entry-multiplicity blocker.
  The product parser and **BUMP-ALLOCATOR-CANARY** consume this mechanism.

  Acceptance: a consuming transform then replacement executes with
  caller-visible updated contents and exact-once custody in Terminal
  interpretation and supported native targets, including sibling work, repair
  on both branches and contained-loan transport. Missing repair, early return,
  stale reads, overlapping loans, wrong-place repair, repeated extraction and
  whole-owner cleanup reject; tampered artifacts fail independent replay.
  Cover recoverable failure, suspension/resume/cancellation and crash/process-exit
  abandonment without invented rollback or survivor guarantees. Preserve the
  checker/interpreter `borrowed_restoration` suites, ledger controls and
  `fail/ownership/borrowed_storage_boundary_call`.

- **MATCH-SELECTIVE-LOWERING.** Complete
  [value dispatch](wiki/spec/language/patterns.md) for owned/borrowed/linear
  results, structural/case/domain patterns and coverage. Owners: validation's
  `value_custody/expression_types/{match_dispatch,result_type}.rs`, checked
  value continuations, Terminal production and package-review's
  `capture/contracts/expressions/projection/`. Preserve once-only subject
  evaluation, authored first-match order, selected-arm-only execution, exact
  result/loan origins and death edges.

  - Carry existing affine custody through branch-local calls/receivers and
    effectful subjects. Complete projected fields inside record-construction
    arms, heterogeneous residual custody and claim-bearing body plans;
    ordinary projected-child selections already lower. Resume from stage 05's
    `tests/value_dispatch/owned_results/{call_product_arms,interleaved_and_projected_sources,linear_child_carriers}.rs`.
    `folded_index_projection::folded_index_faces_the_literal_lowering_boundary`
    now expects literal and folded indices to meet the same fixed-array
    establishment refusal. **RECORD-LITERAL-ARRAY-FIELD** owns that construction
    gap; do not reopen folded-index planning.
  - Complete borrowed joins through source production and native execution.
    Reuse `abstract_operations::control_flow::address_joins`: shared primitive/
    plain aggregate admission and mutable whole-record address carriers exist.
    Primitive/projected exclusive arms, write-only joins (**WRITE-ONLY-BORROW**),
    case-bearing or reference/descriptor-bearing referents, dynamic indexes,
    ranges and computed roots still need exact place/loan evidence. Complete
    join-place reads/projections, primitive-local origins and borrowed-subject
    successor lifecycle publication. Retain source-state loan closure and end
    loans at last use; Omega's `current_ownership/address_joins.rs` currently
    pins origins through the whole dominated region.
  - Carry combined Psi rewrite provenance through legalization's `branch_edges`
    in `scalar_graph_input/control.rs`, which currently requires one Psi edge
    identity per successor, including for scalar matches.
  - Instantiate composite/constrained result shells and exact static arguments
    for indexed predicates/theorems and package memberships, retaining
    qualifications, routed membership, callable signatures and erasure.
    Bare operator `-> T` binding exists; dependent shells cannot export
    declaration-local identities. Package review already projects exact call
    arguments but explicitly refuses `Match`; extend canonical projection.
    **OPERATOR-MACHINE-SUPPLY** owns execution, **CRASH-CONTRACT** crash-qualified
    equality, and **STATE-LOCAL-VALUE-FRONTIER** numeric landing/sequencing.

  Generalize place/loan establishment across matches, locals and calls; do not
  expand an arm-expression roster or flatten ownership into statement-wide moves.
  Acceptance: extend `checked-trees-to-lowered-psi --test suite value_dispatch`
  through independent artifact replay and native execution, retaining
  [float Match customers](wiki/drafts/reference/float_realization.md#operation-and-control-custody).
  Cover effects, skipped trapping arms, overlapping patterns and full coverage.
  Preserve `match_anonymous_result_landing`, `numeric_operand_destinations` and
  `collections/runtime_dutch_flag_partition_exit`'s exit-70 oracle with copyable
  swaps. Wrong qualifications, ownership, result types and incompatible arms
  reject before an outer bare-carrier cast. Checking, target lowering or
  interpretation alone does not establish native execution.

- **OPERATOR-MACHINE-SUPPLY.** Complete
  [declaration-owned executable supply](wiki/spec/language/expressions.md#executable-supply),
  [call preconditions](wiki/spec/language/machines.md#call-preconditions) and
  [licensed normalization](wiki/spec/proofs/contracts.md#licensed-normalization).
  Prioritize existing calls reaching verified Terminal and native execution;
  broader algebra coverage and retiring the separate `operator` introducer
  remain follow-on work.

  - Retain call-containing scalar requirements through stage 05's
    `scalar_graph/scalar_contracts.rs` and `scalar_graph_lowering/graph_preparation.rs`.
    `tests/call_premise_terminal_boundary.rs` checks `restricted(saved, value)`
    under `requires observe(left) == observe(right)` but expects Terminal
    rejection: `covered_requires` refuses the unrepresented clause. Replace
    that limitation with canonical reload, independent verification and
    interpreter/native agreement, comparing a clause-free control. Do not drop
    premises or substitute execution of the callee for admission.
  - Complete formation checks in `contract_entailment/specification_calls.rs`
    and stage 04's `checks/contracts/evidence.rs` across static callable/evidence
    arguments and generic dispatch, calls inside abstract signatures,
    domain/default predicates, receiver/projection terms and citation/induction
    guarantees. Abstract callee
    attribution and nominal named-evidence calls already exist; the latter's
    exact binding and corruption controls are in package-evidence's
    `tests/contract_expressions/evidence_calls.rs`. Preserve input position,
    callee proposition, source evidence and instantiated proposition. Boolean
    premises still require independent prior facts; a supplied witness does not
    discharge them. Form results and import guarantees only after admission,
    including erased/discarded uses; recursive descent remains separate. Retain
    exact subjects, complete constructor values versus tag-only facts and
    public-contract boundaries.
  - Extend `operators/token_bound_machine_calls.rs` beyond binary/indexed
    expressions to open-ended ranges and body-bearing match equality. The
    existing floating requirement comparison does not execute a selected source
    body. Verify Build execution before retaining its old refusal as a blocker.
    General patterns belong to **MATCH-SELECTIVE-LOWERING**. Preserve
    [Equatable's sealed type-owned route](wiki/spec/language/conformances.md#core-equality-acquisition):
    verify that `machine == Card::same(..)` cannot override it through visibility.
    Reuse ordinary calls, receiver loans and once-only operands; no operator
    interpreter, satisfier search or fallback arithmetic.
  - Extend `open_index_expressions.rs` / `OpenIndexOperationSelection` with
    distributivity and zero/one law evidence and consumers. Commutativity and
    associativity already license separate rewrites; neither grants identities
    or integer-addition meaning. Preserve `checking/tests/open_index_algebra_laws.rs`
    and typed-trees' `type_identity/tests.rs`; verify source-produced selections
    through `monomorphization::refresh_closed_domain_instance_identities` and
    publication. Retain missing/ambiguous selection, wrong-operation and
    unproved-law rejection; noncommutative operations remain usable.
  - Migrate library/corpus/embedded declarations, then remove legacy parser/IR
    consumers and undifferentiated `MachineSupplyMode::Boundary`. The
    [retirement inventory](wiki/drafts/audits/operator_introducer_retirement_inventory.md)
    is navigation, not a census. Tokenless boundary migration depends on
    **TOP-LEVEL-BOUNDARY-REQUIREMENTS**, including generic requirements. Preserve
    ordinary trait token heads (`checking/tests/trait_token_heads.rs`),
    domain-homed top-level machines, diagnostics, `SpelledOperator`, provider
    planning, build-time execution, package evidence and result-domain dispatch.
    Preserve semantic identities or explicitly reject stale schemas.

  Acceptance: use `tests/omega/pass/expressions/declared_operator_match_result/`:
  `select_true` and `by_name` produce `260u64` from wrapped `250 + 10`;
  `select_false` produces `1` without invoking the operation. Bind an executable
  entry and observe native results; the existing fixture has no Build root.
  Checked interpretation, reloaded Terminal and native execution must agree.
  Stage 04 and checked-interpreter `tests/token_bound_machine_calls.rs` cover
  narrower ordinary-call/interpretation routes. Check generic/stateful bodies,
  private helpers, qualifiers, ordered operands and receiver/result loans through
  source-free replay. Missing bodies, bodyless-plus-satisfier supply, duplicate
  shapes, foreign primitive-family injection and forged primitive identity
  reject; unrelated imports/conformances cannot alter selection or collide.
  Runtime, compile-time and proof calls owe the same premises: preserve Nat
  named/token order requirements, equal operands and predecessor-at-one
  acceptance, and independent missing-
  premise/descent rejection. Operation, conformance/law and call/loan identities
  survive substitution; trait supply, **FLOAT-PROVIDERS** and compiler primitives
  retain their separate routes.

- **MODULE-NAMESPACE-RESOLUTION.** Complete exact declaration selection through
  checking, constant evaluation and source-independent publication under the
  [module/name contract](wiki/spec/language/modules.md).

  - Implement [default builds and anonymous owners](wiki/spec/build/declarations.md#default-builds-and-anonymous-owner-identity)
    in `build-evaluation/src/sources/` and compiler handoff. Its
    `source/source_storage.rs` still leaves loose roots without package identity;
    preserve supplied package/core owners and derive the versioned anonymous
    content record before build execution. Deduplicate dependency nodes while
    retaining separate host/target build instances. Missing builds use ordinary
    default configuration, with no catalog, host-path identity, ambient authority
    or special `main` discovery. Carry owner/declaration provenance through
    specialization and artifact encode/reload; dependency acceptance and visible
    collisions remain separate checks. Acceptance: one std owner in a diamond;
    distinct root/core/library owners; stable identity after relocation, rebuild
    and cache deletion; changed anonymous semantic inputs change identity;
    independent named lineages remain distinct despite equal content. Reject
    missing/mismatched provenance and genuine visible-name conflicts. Recheck
    `control_flow/guarded_leaf_branch_expansion` and `text/runtime_alias_string_write`
    on the loose native-host route: corpus inputs supply package identities and
    no longer record the old `Utf8` collision, so they do not establish this
    acceptance. Local identity repair need not await the versioned
    [standalone-request extension](wiki/spec/build/compiler_request.md) shared
    with OMEGA-D/OMEGA-C; named-only OCREQ v1 cannot silently encode anonymous owners.
  - Finish typed/checked/Terminal use of generic type-scoped constant attachments,
    indexed constraints and operator homes. Extend existing normalization and
    exact declaration selection in
    `02_syntax-trees-to-symbol-resolved-trees/src/preparation/`, preserving
    specialization, competing declarations, visibility and nontransitive file-local
    imports. Reuse `CaseMembership` and symbol-based multiplicity matching.
    `tests/module_namespace_residuals.rs` checks normalization/resolution, not
    downstream execution.
  - Extend declaration evaluation in
    `build-time-evaluation/src/const_evaluation/const_initializers/` and
    resolver `generic_data/const_evaluation/{facts,values}.rs`: specialized
    provider/target applications, exact authored NaN bits, nested record/array/case
    and whole-aggregate subjects, open applications and abstract-carrier `self`
    constraints. Preserve alias-owned bounds/index tuples, including compiler-owned
    atoms, through
    `checking/src/proof_contracts/domains.rs`. Computed payloads and empty fact
    replay cannot establish qualifications; routed constraints still need
    establishment, including for unused initializers.
  - For constant invocations, share **CRASH-CONTRACT** /
    **OPERATOR-MACHINE-SUPPLY** premise transport through stage 04's
    `facts/crash_entry_values.rs`. Extend beyond lossless widening and proven
    indices only with exact live evidence; value-changing casts, call-produced
    projections and unknown origins retain conservative crash ceilings.
    Interpretation or provider-body inspection cannot discharge admission.

  Acceptance: retain `tests/omega/pass/modules/{qualified_declarations,qualified_constants,match_constant_indices,nominal_constant_bodies,module_array_constant_indices}`
  and the `modules/` corpus gate, then verify source-free publication/reload,
  independent checking and matching-host execution for the new supported forms.
  Use maintained initializer tests beside `const_initializers.rs`, stage 04's
  `tests/contracts/main.rs::predicate_domain_initializer_*`, and
  `terminal-psi-to-abstract-operations --test suite scalar_array_construction`;
  the latter verifies serialized array retention, not native execution.
  Preserve exact floating bits/signed zero, field/index projections, selected
  operator meanings/widths/policies and all-arm obligations without evaluating
  skipped subjects; retain lossless-widening/signed-nonzero forwarding and
  zero/wrapping-to-zero, stale
  operand, wrong-owner, false-premise, invalid-unused and unproved-index controls.
  Runtime qualified `Holder<T>` reads and general array execution remain
  **STATE-LOCAL-VALUE-FRONTIER** / **TR3-TR8** integration dependencies:
  successful constant projection does not establish runtime materialization.
  Runtime-index fail fixtures change only after their materialization obligations
  are met.

- **RUNTIME-VALUE-GENERICS.** Complete
  [runtime-capable value binders](wiki/spec/language/generics.md#value-binders-and-const-requirements)
  and [captured index identity](wiki/spec/language/generics.md#runtime-index-identity-and-storage)
  for data applications, result and stored qualifications. Machine value binders
  already pass each captured subject as an ordinary argument to a shared body.
  Begin with fixed-representation data; **FINITE-GENERIC-DISPATCH** separately
  owns finite families.

  - Complete runtime data-argument admission and construction binding in stage
    02's `preparation/generic_data/` and `lowering/data.rs`, then transport exact
    subjects through stage 04's `monomorphization/` and checking's
    `value_custody/type_references/`. Data value arguments still use
    `validate_const_data_argument`; lowering synthesizes descriptor fields
    unconditionally. Retain executable indices as ordinary data, arguments or
    descriptors; erase proof-only uses only after discharging their obligations.
  - Preserve captured value versions through parameters/results, fields and
    artifact publication. Normalized type identity already retains generic
    arguments; shared layout/code does not establish application compatibility.
    Verify that, for
    `data Index<Limit: u32> [copy] where value < Limit, { value: u32; }`,
    an `Index<9>` holding 8 cannot pass as an `Index<7>` argument or field.
    Preserve distinct literal indices and require checked relationships between
    runtime subjects. Reassignment cannot retag stored values; equality admits
    only the relationship it proves. Retain construction/default-domain and
    later write obligations.
  - Complete domain/index qualification forwarding with live/stale guard
    controls. Migrate legacy range-shell fixtures through
    **CANONICALIZE-SCALAR-RANGE-CONTRACTS**, without extending suffix syntax or
    treating those fixtures as general domain-index support. Runtime inline-array
    extents and const positions remain rejected. Carry module-owned forms through
    **MODULE-NAMESPACE-RESOLUTION**; runtime values are never static cache keys.
  - Execute actual value-indexed data/storage through verified Terminal,
    interpretation and native ordinary call/storage routes, integrating
    **STATE-LOCAL-VALUE-FRONTIER** and **ENTRY-CONTENT-ROOTS**.

  Acceptance: `<Count: u32>` accepts static/runtime arguments under proved
  obligations; `<const Count: u32>` remains static. One dynamic body handles
  distinct counts, and parameter/result/field qualifications survive source
  reassignment. Invalid construction, bounds, stale/mismatched indices,
  static-only uses and duplicated/lost linear custody reject in source checking
  and independent artifact replay. Preserve stage 04's
  `monomorphization/{runtime_value_tests,membership_tests}.rs`, checked-interpreter's
  `tests/runtime_value_generics.rs` and
  `tests/omega/pass/generics/runtime_value_generic_subject_exit/`: these cover
  narrower scalar calls, indexed access and generic checking, not full data-index
  execution. The corpus canary binds four hosted targets; verify each host's
  native outcome separately. No hidden boxing, maximum-range stack reservation,
  runtime per-value code generation or specialization inferred from integer
  finiteness.

- **STRUCTURAL-GENERIC-MATCHING.** Complete
  [static type equality](wiki/spec/language/generics.md#static-type-equality),
  [structural equations](wiki/spec/language/generics.md#structural-type-equations-and-inference)
  and [canonical domain indices](wiki/spec/language/generics.md#canonical-domain-index-matching)
  through specialization, layout and source-free execution.

  First make the spec's TinyBytes infer Capacity from `u64::AtMost<256>` before
  layout, then construct and access its nested backing through serialized
  Terminal and native execution. Extend
  `generics/omitted_data_binder_range_equation`: its `Bytes`
  declaration preserves the domain-index direction, but `constructed_value`
  only casts a scalar qualification. Neither it nor `generics/machine_type_equations`
  binds a native entry. Include invalid-initializer controls.
  **STATE-LOCAL-VALUE-FRONTIER** owns storage and borrowed-local mutation;
  const staging, initialization and stack supply remain separate obligations.
  **CANONICALIZE-SCALAR-RANGE-CONTRACTS** owns the remaining range-shell migration.

  Reuse `preparation/type_equations/` and `machine_equations.rs`. Array,
  anonymous reference/slice and integer/Boolean application matching exist;
  registered `omitted_data_binders`, `domain_application_tests` and resolution's
  `machine_type_equations` exercise synthesis and rejection, not native execution.
  Remaining work:

  - Combine equations with argument/result inference and preserve obligations
    through forwarding, retained extensions, late-selected receivers, operator/
    conformance supply and machine/evidence/value binders. Machine preparation
    still requires closed explicit calls; keep unsolved-equation rejection
    until each route discharges them.
  - Complete domain matching by exact package/declaration and generic-carrier identity.
    `DomainHead` joins `u64::AtMost<256>` with `u64 in AtMost<256>`, but its
    local head lookup still declines generic carriers. Existing carrier-head
    name normalization does not close that matching route. Add selected
    named-lifetime identity and remaining const-index kinds;
    **WRITE-ONLY-BORROW** owns write-only admission.
  - Check unspecialized machine/requirement bodies under every admitted
    type-equality alternative, including finite disjunctions and branch-local
    equalities that cannot escape joins. Closed case decisions are insufficient.
  - Extend the existing typed const-expression and invocation evaluators to
    the remaining equation/index combinations. Retain declared carrier,
    arithmetic policy, selected operation and canonical identity through
    matching, layout and artifact replay; equivalent closed computations must
    select one instance. **ARITHMETIC-POLICY-REALIZATION** owns trapping-call
    exclusion; **RUNTIME-VALUE-GENERICS** owns runtime subjects.

  Acceptance: data/machine array and application equations infer omitted
  arguments without losing obligations. Reject repeated/explicit conflicts,
  missing indices, occurs cycles, kind mismatches, false constructor predicates
  and unmet conformances. Match open indices as whole expressions; no arbitrary
  equation solving or predicate maximum search. Distinct indices retain their
  identities without implicit variance; flow bounds cannot choose capacity,
  specialization or layout. Reuse ordinary evaluation and storage plans.

- **FINITE-GENERIC-DISPATCH.** Complete runtime selection under the
  [finite specialization contract](wiki/spec/language/generics.md#finite-specialization-boundary)
  and [dynamic method families](wiki/spec/terminal-psi/dynamic_dispatch.md#finite-generic-method-families)
  for scanner widths and runtime-selected datatype providers.
  `TypedTrees::finite_signature_family` owns the roster;
  `monomorphization/dynamic_families.rs` specializes selected providers.
  Static tuple selection, correlated multi-binder rosters, boundary family
  coverage and Terminal tuple identities already exist.

  First execute a source customer selecting widths 16/32/64 from a runtime
  value through one named conformance, with observable distinct tuple results
  and the actual instance through rebinding and forwarding.
  `traits/dyn_finite_family_dispatch`
  has no native entry and discards results; `dynamic_composed_unit/finite_family.rs`
  checks serialized tuple joins, while its rebound provider ignores Width.
  Extend these into interpreter/native execution without handwritten suffix
  methods. Begin with one scalar binder, a common result and dispatch around
  a region-sized operation.

  - Admit proved runtime roster membership in checking's
    `validate_dynamic_family_call_tuple` and checked planning's
    `dynamic_family_tuple`; both currently require closed static tuples.
    Reuse **RUNTIME-VALUE-GENERICS**' scalar argument/guard machinery without
    waiting for general value-indexed data. Extend the current self-only,
    Unit/i32/bool callable and restricted-body subset to scanner inputs/results.
    Unproved membership rejects; no JIT or invented fallback.
  - Complete native dispatch through
    **RESTORE-DYNAMIC-DESCRIPTOR-AND-TABLE-CUSTODY**'s ordinary indirect-call
    mechanism. Rebound `CallDynamicScalar` reaches target operations but lacks
    common-graph legalization; parameter-descriptor calls legalize and replay,
    but selected construction still rejects `DynamicParameterCall`.
    Carry complete source-produced tuple tables through independent artifact
    and image replay; test-built rows alone are insufficient.

  Acceptance: source pass/fail/run cases preserve effects, index identity and
  once-only moves/cleanup through selection, forwarding and storage. Normalize
  alternative order/duplicates deterministically; retain complete correlated
  tuples without range enumeration or cross-products. Reject missing, wrong-width,
  mixed-provider or shape-substituted rows, unproved membership, target-ineligible
  bodies and const-only dynamic inputs without a checked bridge. Escaping
  variable-shaped results need an authored sum or eligible descriptor with
  explicit ownership. Record compile/code-size and scan-loop dispatch placement
  before claiming improvement over explicit branches. No implicit boxing,
  general generic virtual-method or reflection extension.

- **DOMAIN-ISSUER-ROUTES.** Execute issuance, wrapper forwarding and consumption
  with an observable result, then serialize Terminal and independently replay
  [requirement, exact-machine and boundary establishment](wiki/spec/resources/authority.md#requirement-and-exact-machine-routes)
  without source. Cover scalar and structural carriers, private issuers and
  authorized selected payloads. `tests/omega/pass/domains/bodyless_owner_establishment`
  currently checks an ordinary issuer/forwarder but binds no executable entry.

  In `05_checked-trees-to-lowered-psi/src/unit/attached_unit.rs`, ordinary units
  publish no conformance applications; its `catalog.rs` emits result establishment
  bindings only for boundary requirements with an exact `CallEnsures` admitted
  receipt. Retain ordinary callee/conformance identities and bind checked evidence
  to the exact invocation result. Complete source-produced boundary roundtrips too.
  Reuse existing structural/scalar route catalogs, codec and issuer validation.
  `terminal-verifier --test suite`'s `calls/qualification_establishments.rs` and
  `straight_line/scalar_qualifications.rs` use assembled Terminal fixtures; they
  do not demonstrate this source path. Structural verification skips empty
  establishment lists: prove evidence completeness as well as validating present rows.

  Apply the settled [qualification rule](wiki/spec/language/domains.md#declaration-and-membership)
  to copyable, affine and linear carriers. Remove `domain_target_is_custody_marked`
  from stage 04's `facts/index_compatibility.rs`; unrelated domains must not
  restrict a route-free tag. Coordinate with **BUMP-ALLOCATOR-CANARY** to give
  `source/library/core/extent.omg`'s bare `Vacant`/`Resident` the actual custody
  routes. Test protected declarations against both casts and unevidenced calls;
  migrate `fail/memory/bump_allocator_cast_minted_vacant` off the heuristic.
  Preserve the `Region::Left`/`Right` linear move and vacuous scalar tags when
  unrelated predicate/routed domains are added.

  Acceptance: independently prove carrier, predicate and custody obligations
  without assuming the introduced qualification. Reject missing, forged or rebound
  evidence, same-spelled/module/package issuer substitution, and direct calls
  borrowing admitted requirement authority. Preserve
  [private catalog](wiki/spec/resources/authority.md#private-issuer-routes) identity
  and dependencies without granting consumer call/conformance/private-type access
  or hiding admissions. Public ordinary requirements still admit valid downstream
  conformers; resource-capacity and classification-specific restrictions remain.
  Keep `package-evidence --test suite`'s public-domain and module-namespace controls;
  their catalog/review roundtrips do not establish Terminal or executable acceptance.

- **TWO-AXIS-TERMINAL-AUTHORITY-REVIEW.** Deliver ordinary reviewed native
  production for console-exit-app, Cathedral native smoke, `cli_mvp` and Squalr
  under the [production/admission split](wiki/spec/build/permissions.md#artifact-production-versus-receiver-admission).
  Preserve existing host successes and coordinate **SAMPLE-CORPUS** /
  **SQUALR-HEADLESS**. No receiving policy means no admission claim; package
  acceptance must not manufacture receiver approval. The registered CLI
  `package_commands/console_exit_permission.rs` still allows publication or
  `Selection(Legalization(SourceCustodyMismatch))`. Reproduce its current result,
  repair the failing owner, and require the authored console line and exit 70.

  - Connect the Linux x86-64 filesystem plan to native settlement and syscall
    execution. `compiler/build-evaluation/src/provider_settlement/canonical_filesystem_host.rs`
    already mints supported positional syscall rows; its integration test
    inspects plans/provenance only. Linux x86-64/AArch64 calling plans exist,
    but `DirectSyscallRealization` and an admitted `ToolchainSettled` execution
    variant do not. Drive an actual `set_len` call through settlement, physical
    selection, encoding and replay; the former compiler witness is gone.
    Settle demanded `BoundaryCall`s exactly once. Retain argument/clobber and
    provider-plan identities; reject unsupported targets or demanded leaves needing
    unimplemented path/slice or host-error adaptations rather than guessing realizations.
  - Preserve optional receiver admission and separate it from package-row
    replay. `package-manager/src/operations/compile_project/tests/receiving_admission.rs`
    covers retained native production with absent, denying, sufficient and
    surplus policies using a minimal local Console provider. Its sufficient
    policy comes from emitted leaves. The direct/retained paths still call
    `provider_admission/terminal_authority_permissions.rs` to require receiver
    rows equal package permissions. Reconcile that coupling with independent,
    demand-based receiver containment, including sufficient differing permissions
    and undemanded package rows; keep exact accepted-package replay separately.
  - Finish exact [filesystem classifications](wiki/spec/build/permissions.md#portable-filesystem-control-and-lifecycle-authority)
    through discovery, acceptance and admission. Cohort classifications already
    emit from `provider_admission/settlements/source_imports.rs`; candidate
    `semantic_bindings.rs` supplies permission proposals. Preserve their separate
    identities. Remove the broad filesystem arm in
    `package-manager/review-evidence/src/capture/authority.rs`, dependent
    `src/review/audit/triage/` rendering and transitional
    `standard_library_package_resolution.rs` assertions after exact replacement.
    **FILESYSTEM-RELEASE-CONTRACT** owns occurrence-bound close evidence;
    generic close and build observations cannot justify empty runtime authority.

  Acceptance: all four customers produce without receiver input. Denying policies
  reject; independently sufficient policies admit, retaining the supplied identity,
  including surplus rows. Every demanded leaf has one exact mechanism/contract classification
  fitting permissions. Unknown, duplicate/conflicting, cyclic or substituted
  classifications, stale evidence and violated physical exclusions reject.
  Preserve explicit empty classes, service reach, review identity and settled-plan
  replay; ordinary `UniqueCoveringCandidate` plans remain subject to authored
  replay. No mandatory PCC request or synthesized receiver-policy file.

- **FILESYSTEM-RELEASE-CONTRACT.** Execute
  `tests/omega/pass/filesystem/windows_canonicalize_exit` on Windows: canonicalize
  a file and directory, preserve missing-path `NotFound`, clean up and exit 70.
  Its build selects `Main::main`; the separate `FailedOpen` entry has no remaining
  call-count harness. Exercise failed acquisition and prove it performs neither
  path query nor close. Keep the authored `UnitResult` stores, case observations,
  nested error matches and buffer handling.

  Prioritize prerequisites through **STATE-LOCAL-VALUE-FRONTIER** /
  **NOMINAL-FIELD-FLOW**. The corpus records conditional state-parameter transfer
  refusal at state 8, statement 2; re-drive the whole program before assigning a
  current blocker. Structural replacement already handles constructed and
  call-produced cases through move/store pairs: stage 05's registered
  `tests/constructed_field_replacement_source.rs` serializes, verifies and
  interprets these stores. Extend that composition rather than rebuild it.

  Derive the [bounded release proof](wiki/spec/build/permissions.md#bounded-occurrence-specific-release-proof)
  from checked program flow: successful acquisition under the exact object/argument
  contract, handle/alias identity, preservation by intervening calls, one release
  on applicable returning paths and no subsequent use. Retain Terminal evidence,
  independently validate it and bind the exact native occurrence. Helpers in
  `02_abstract-operations-to-target-operations/src/provider_admission/terminal_authority_policy/filesystem.rs`
  already construct contract commitments and syscall/foreign-import classification
  keys, but have no callers beyond exports. A supplied digest, open flags,
  query-only authority or build execution does not prove the lifecycle.

  Acceptance: a proved close earns one occurrence-bound empty classification;
  failed acquisition, escape, reused aliases, double close, invalidating calls and
  stale/substituted proofs cannot narrow authority. Changed arguments, flow or
  contracts require new evidence; otherwise use a justified conservative class
  or reject. Preserve Windows read/write/delete sharing: external pending deletion
  may complete on ordinary close, while deletion attached to this handle excludes
  the empty class. `filesystem/native_close` closes arbitrary descriptor 5 and
  remains an unconstrained control. General owned handles are not a prerequisite.
  **TWO-AXIS-TERMINAL-AUTHORITY-REVIEW** owns permission/review rows and broad
  filesystem-summary replacement.

- **R5.** Carry complete caller-visible may-write summaries through source
  checking under [state contracts](wiki/spec/language/state_contracts.md) and
  [subject preservation](wiki/spec/language/termination.md#subject-preservation).
  A helper choosing either mutable input and writing through its result in a
  value expression must preserve a disjoint index-bound fact and invalidate
  the overlapping one, including writes made while evaluating operands.

  Owner: `omega-rust/psi/checking/src/machine_calls/calls/write_frames/`, consumed
  by stage 04's flow mutation and range/progress checks. `CallFrameResolver`
  already shares call/expression summaries and transition fixed points.
  Stage 04's `src/tests/termination/write_frame_aggregate_results.rs` reaches
  checked trees for nested helpers, named-state results and producer writes;
  `write_frame_boundary_arguments.rs` checks disjoint/overlapping fact use.
  Reuse those paths.

  Complete the source-admission and fact-use joins still represented by
  resolver-only probes in `write_frame_candidate_origins.rs`,
  `write_frame_contextual_cases.rs`, `write_frame_carrier_results.rs` and
  `omega-rust/psi/checking/tests/wire_codec_write_frames.rs`. Proven candidate
  unions must survive computed receivers, codec arguments, nested results,
  state arrivals and stored-reference rebinds. Author explicit output lifetimes
  for multi-input helpers: `borrow/view_link.rs` already retains matching-input
  unions, but lifetime correspondence alone supplies no storage-origin evidence.

  Keep unknown origins, private escapes, unproved rebinds, unsupported recursive
  results and loads behind another reference without independent evidence opaque.
  A supported static callee's exact signature may bound writes when its body
  summary is incomplete; that proves neither returned origins nor termination.
  Frames never grant borrow access. Extend the shared origin/fixpoint owners,
  rather than adding a recognizer per reference spelling.

  Acceptance: the helper-choice program passes full checking with the disjoint
  bound; changing either candidate to overlap rejects the stale bound. Exercise
  the admitted program's authored result through **MATCH-SELECTIVE-LOWERING**'s
  reference-result transport. Preserve the finite-cycle controls in
  `write_frame_cycles.rs` and convergence on
  `omega --check source/psi/gates/parser/main.omg`; parameter permutations must
  retain finite write sets. Frame-only success does not establish source
  admission or execution.

- **TPR6.** Complete exact progress-subject transport through calls, state
  arrivals, private recursive components, exported contracts and provider/artifact
  evidence under [subject preservation](wiki/spec/language/termination.md#subject-preservation).
  Private ranking witnesses stay outside public identity; lineage transports an
  existing live qualification and cannot establish one.

  Owner: `omega-rust/psi/pipeline/04_typed-trees-to-checked-trees`.
  `src/checks/termination/progress/{origins,lineage,machine_summaries}.rs` already
  recover owned loads, exact callee replacement inputs and some guarded-result
  alternatives. `ParameterLineage::Exact` retains multiple subjects and
  `lineage::resolve` expands their premises. Complete the remaining path through
  `src/flow/value_origins.rs`, whose backward trace still returns one origin:
  projected/stored helper results and extra reference boundaries need exact
  provenance with the ordinary capture, alias and write-invalidation checks.
  Extend generic/dispatched callee and unresolved projection handling without
  confusing generic field traversal with evidence about a callee's body.

  Closed-array selector enumeration already returns alternative subjects, but
  `machine_summaries::instantiate_call_premise` still requires one, as does
  `src/checks/termination/progress/lineage/transfers.rs`. Carry complete finite
  alternatives into call coverage and state transfers. Every possible subject
  must be covered; picking one is unsound. Preserve exact public-schema,
  local admitted-receipt and build-bound provider coverage for each demand.
  Provider receivers must not become caller
  parameters, and route catalogs are not receipts. Reuse the existing provider
  schema/manifest machinery; **COMPONENT-SUBSTRATE** owns installed composition.

  Verify through full source checking, beyond the synthetic flow rows in
  `src/checks/termination/progress/origins/tests.rs`: guarded results and dynamic
  selectors with different known origins, both covered and one uncovered;
  nested wrappers, finite field/parameter permutations and private recursive
  premises; unknown arms, stale aliases, replaced reference prefixes and growing
  projections. Preserve `src/tests/termination/{progress_lineage.rs,progress_mutation/}`
  controls, including live qualification checks after later operand writes. The
  `embedded_call_written_on_the_demanded_path_has_no_exact_origin` control must
  still refuse: may-write information and aggregate root correspondence cannot
  recover overwritten contents. Each reference boundary needs its own load evidence.

  **STATE-LOCAL-VALUE-FRONTIER** owns nested structural-result execution needed
  by these consumers. Retain the inline record/array cases in
  `src/tests/borrow/carrier_results.rs` and `select(forward_array(values)[0])`;
  the earlier-local `select(forward_outer(outer).inner)` already has serialized
  Terminal execution/replay coverage in
  `omega-rust/psi/pipeline/05_checked-trees-to-lowered-psi/tests/reference_result_source.rs`.
  Complete structural-array ingress and residual cleanup through shared evaluation,
  retiring superseded `omega-rust/psi/checking/src/machine_calls/calls/expression_scanning/result_realization.rs`
  shape gates only when real producers preserve custody.

  Acceptance: calls evaluate once; result homes, carrier locations, loan occurrences
  and parents, qualifications and linear claims survive consumption, source-free
  execution and independent replay. Transfer selected permissions rather than
  create a child loan beneath a dying parent; relocate descriptors without
  copying referents. Preserve conservative lifetime unions and nested-reference
  host-interface rejection. Missing, stale or substituted provenance grants no
  progress guarantee. Keep rejection controls
  until complete custody is implemented; Terminal execution does not establish
  native acceptance.

- **CML4.** Complete edge-local residual cleanup under the
  [ownership contract](wiki/spec/terminal-psi/ownership.md): outgoing values
  materialize before transfer and disposal, every occurrence has one
  disposition, and independent dying roots clean in reverse declaration order.
  Abandoned partial construction/staging cleans its established prefix in
  reverse establishment order, as the [language rule](wiki/spec/language/ownership.md#construction-and-disposal-order) requires.
  Crash/abort/process-exit abandonment has no cleanup successor.
  Owners: `04_typed-trees-to-checked-trees/src/execution/control_cleanup.rs`
  and `execution/terminal_unit/cleanup/`, `05_checked-trees-to-lowered-psi/src/unit/unit_cleanup/`,
  and Omega's `02_abstract-operations-to-target-operations/src/lowering/`.

  Bounded parameter/local/result partial moves already reach encoded Terminal
  execution. Multiple ordinary or boundary-produced projected temporaries can
  share a Unit consumer (`partial_affine_result_source::anonymous_projected_operands_share_one_dying_continuation`).
  Native target lowering admits no-code residual actions on Jump and
  scalar/Unit-return edges; a lowering test is not native execution coverage.

  Remaining work:

  - Carry residual evidence through conditional and structural-case
    successors and structural returns. `SuccessorEdge`,
    `StructuralCaseSuccessorEdge` and `ReturnStructural` retain only
    whole-root discard lists; `ReturnUnitPartialAffine` already exists.
    Update the encoding contract and representation, producer, codec,
    independent verifier/interpreter and native consumers together.
  - Extend independently reconstructed residual scheduling to construction-local
    roots, partial construction, a second partially moved parameter root, and
    mixed dying roots beyond the supported parameter/temporary pair. Reuse
    `execution/terminal_unit/cleanup/anonymous.rs::append_continuation` and
    the partial-affine carrier's exact complements; keep machine-scoped
    parameter custody distinct from continuation-edge cleanup. No runtime
    liveness flags, expansion of untouched arrays into leaves, or cleanup
    deferred until final return.
  - Generalize projected temporary cleanup beyond empty, effect-free Unit
    consumers: compose other argument effects, non-Unit consumers,
    borrowed/owned temporary mixtures and retained claims/qualifications
    through **STATE-LOCAL-VALUE-FRONTIER**'s ordinary evaluation path.
    Each temporary retains its producer and exact dying continuation.
  - Close source-to-native storage and replay for result homes, projected
    results/copies, computed values across calls and cyclic control.
    `lowering/unobserved_owned.rs::accepts` still recognizes whole functions
    to suppress arrival homes; it no longer governs all return cleanup.
    Retire that support dependency through per-place storage/action evidence,
    not another allowlist. Executable nominal calls belong to
    **CLEANUP-HOOK-SELECTION-AND-ERASED-OWNERSHIP** and must preserve this
    schedule.

  Acceptance: source-produced artifacts reload and independently reconstruct
  exact complements and complete transfer/cleanup partitions; native runs
  preserve values and observable hook order/timing across the composed edges.
  Frontend admission or diagnostic ordering is not that runtime witness. Reject missing,
  duplicate, reordered, overlapping, wrong-root/type and post-transfer cleanup.
  Fuel exhaustion commits no disposal and retry cannot repeat one.
  Preserve `terminal-verifier`'s `jump_edge_residual_discards_close_the_projected_argument_root_in_order`
  and `owned_successors_reject_same_arity_aliases_and_transfer_after_disposal`
  controls. No affine occurrence disappears or survives past its required edge.

- **CLEANUP-HOOK-SELECTION-AND-ERASED-OWNERSHIP.** Complete ordinary
  consuming `drop<T>` and exact owner-attached hook execution under
  [nominal cleanup](wiki/spec/terminal-psi/ownership.md#nominal-cleanup).
  The generic consumer already selects and lowers eligible hooks, including
  bodies that read/write `self` and call ordinary helpers. Do not restore a
  cleanup-body recognizer or expose reserved `T::drop` as an authored call.

  Remaining work:

  - Realize executable cleanup in the common native graph:
    `02_abstract-operations-to-target-operations/src/lowering/control_flow/terminator.rs::plain_home_cleanup`
    rejects `InvokeNominal`. Preserve the exact receiver, hook contract,
    action order, result homes and continuation across the real call;
    no-code disposal is not a substitute. **CML4** owns residual partitions
    and mixed dying-root schedules.

    Two facts measured 3781c9e05c, before anyone plans this as a small edit.
    Cleanup rides on the EDGE: `cleanup_actions` is a field of
    `TargetControlSuccessor`, beside `bindings` and `target`, and an edge
    executes no operations. A hook invocation therefore needs a synthesized
    landing block that performs the call and jumps to the real target, with
    the downstream consequences that carries through register allocation,
    encoding and verification -- `plain_home_cleanup`'s `Ok(false)` is the
    honest answer for the representation as it stands, not an oversight, and
    its callers turn it into `unsupported_control_flow`.

    The acceptance also has no customer yet. `drops/core_drop_owner_hook`,
    `core_drop_owner_hook_body` and `core_drop_explicit_consume` carry no
    `build.omg`, so native production refuses them with "requires one exact
    selected program entry" before any cleanup lowering runs: they are
    terminal-tier fixtures by construction. Adding entry bindings first would
    only move them from checked-only to natively red, so bind them in the same
    change that lands the landing block.
  - Reject reads through consumed owners using ordinary ownership/access
    checking, not a special case for `drop`: a read of `g.handle` after
    `drop(g)` must reject.
    `flow/ownership/moves/observations.rs` does not record a read of the base
    name, while `linear_validation/recorded_events.rs` checks dead custody on
    transfers/consumption. Witness the rejection with a `fail/drops` corpus
    case, and an ordinary consuming helper and a valid snapshot copied before
    consumption with `pass/drops` cases.
  - Compose contextual requirements and dying local owners through ordinary
    cleanup edges. `05_checked-trees-to-lowered-psi/src/unit/unit_cleanup.rs::patch_nominal_cleanup_member`
    still excludes nonempty caller/hook prerequisites, and
    `04_typed-trees-to-checked-trees/src/execution/terminal_unit/control/checked_machine.rs`
    rejects nominal-drop locals left owned at return. Retain independently
    checked exact-place prerequisites; never infer new caller demands.
  - Complete erased-bearing record construction without runtime evidence
    storage or cleanup. Both Unit and scalar-graph record emitters reject
    erased initializers, while
    `terminal-codec/src/sections/semantic_module/module_foundation_validation/value_foundations.rs::validate_establish_record`
    demands a relevant initializer for every declaration. Reconcile semantic
    fields and proof custody with the physical roster, preserving erased
    subjects, multiplicity and provenance. **PROOF-RELEVANCE-MIGRATION** owns
    erased formal/actual transport. This is distinct from an owned dynamic
    descriptor whose hidden runtime payload still requires cleanup.

  Acceptance: `drops/core_drop_owner_hook{,_body}` and
  `drops/core_drop_explicit_consume` reach encoded, independently checked
  Terminal and native execution. Observe a nonempty hook's effect exactly
  once at early disposal and normal scope exit, with valid contextual
  prerequisites and an erased-field control producing no runtime cleanup.
  Missing/forged hook identity or premises, duplicate consumption and reads
  after consumption reject. Preserve ordinary helper calls and reserved-hook
  selection rejection.

- **TR3-TR8.** Connect activation planning and lifecycle accounting to a
  selected runtime executing ordinary named machines under the
  [task-runtime contract](wiki/spec/build/task_runtime.md) and
  [suspension contract](wiki/spec/terminal-psi/calls_and_outcomes.md#suspension).
  Owners: `provider-planning/src/task_plans/`, `task-plans`, suspension
  production/checking, and the selected runtime implementation.

  Reuse the sealed call-graph plans, argument marshalling, nonmoving leases and
  provider-instance claim ledger. Call-side suspension markers already require
  site/plan pairs; `suspension_call_plan_rejects_coordinated_site_and_plan_deletion`
  pins that control. The [implementation note](omega-rust/omega/backend/runtime/task-plans/README.md)
  distinguishes those static/accounting mechanisms from executable activation.

  - Join final physical frame/spill demand, alignment, entry overhead and complete
    live call chains to the exact `StackPlan` and actual provider backing.
    Checked-local/park-frontier layout is not final physical WCSU. Preserve
    unresolved-call refusal and same-stack versus separately provisioned demand.
  - Execute one source-selected runtime route: install marshalled arguments,
    start a distinct activation on its fixed nonmoving stack, reach checked
    suspension points, park/resume the same invocation and return actual outcomes.
    Bind provider selection, invocation receipt, runtime instance, storage era and
    source `Task<T>` route. Ledger transitions alone do not run a machine.
  - Extend exact suspension-frontier production beyond bounded scalar joins to
    receiver, persistent and structural places and live claims. Retain their
    Terminal identities and independently checked storage lifetime, pinning,
    aliasing, address stability and cancellation-outcome evidence. Keep crossing
    completeness through serialization/lowering; unsupported frontiers reject.

  Acceptance: concurrent start/park/resume/finish with observable work and moved
  arguments; rejection conserves caller-owned arguments/leases; foreign-instance
  or stale-era settlement rejects and reuse requires fresh eras. Parking produces
  no result/cleanup, and resumption repeats no committed work. Cancellation needs
  a recorded request observed at a declared safe point, never arbitrary unwinding.
  Preserve CPU/thread requirements and reject reclaim while child custody remains.
  Runtime custody, physical backing ownership and linear settlement authority stay
  distinct; a compiler/provider-owned continuation is not source-addressable.

- **ATOMIC-MEMORY-MODEL.** Complete the formal atomic/fence model and checked
  source-to-target realization under
  [concurrency](wiki/spec/language/concurrency.md) and the
  [atomic vocabulary contract](omega-rust/psi/foundation/language-core/atomic_operations.md).
  Owners: Psi event/evidence production and independent Terminal checking;
  Omega event validation, optimization preservation and target refinement.

  A serial load, store, fetch (add/sub/and/or/xor), swap or decisive
  compare-exchange on a record's atomic field reaches Terminal Psi as one
  verified `AtomicAccess` event (codec tag 93) at an exact root place, static
  carrier path and scalar field, with its orderings, operands and observed
  prior; the interpreter executes it serially. Terminal-to-abstract lowering
  keeps it as one abstract event at the same location, with reads-from and
  modification-after edges from a reaching-writes pass that the optimization
  unit's bounded happens-before replay rechecks. An event with no single
  latest write refuses. The eight `runtime_atomic_*_exit` canaries now stop in
  abstract-to-target lowering (`UnsupportedControlFlow`). No target
  operation, selection or encoding realizes an atomic event yet. Unadmitted:
  atomic cells behind a shared receiver (`shared_receiver_atomic_store`;
  Terminal types do not mark atomic cells), bounded fields, fences,
  single-attempt compare-exchange, and scalar-result machines
  (`atomic_global_order_operations` stops at ordered scalar control). None of
  this establishes a concurrent memory model.

  - Complete independently checkable `sequenced_before`, `reads_from`,
    `modification_order`, `synchronizes_with`, `happens_before` and
    `global_sequential_order` obligations, qualifying fence synchronization and
    outcome-sensitive RMW behavior. Preserve settled source guarantees; raise
    genuinely unsettled formal choices through the governing specification owner.
    Model/checker work and relation witnesses do not require a running scheduler.
  - Produce normalized events through checked source, Terminal Psi and abstract
    operations, retaining exact operation, place, ordering, outcome and custody.
    Add encoding, independent verification and source-correspondence controls.
    A fence remains an event even when no instruction is emitted.
  - Add target operations, realization and checked refinement for admitted
    widths/alignment and orderings. Retain exact decode/encode and round-trip
    obligations. Swap/fetch return the instruction-observed prior, not a separate
    load. Preserve observing/non-observing and decisive/single-attempt behavior,
    failure ordering, `Uncommitted` custody and retry-work attribution.
  - Use **TR3-TR8** for real concurrent publication, qualifying fence pairs,
    global-order and compare-exchange contention controls. That dependency gates
    runtime witnesses, not all preceding model, producer or target work.

  Acceptance: source-free checking binds every retained relation, place, ordering,
  outcome and realization; missing/substituted evidence rejects. Retain serial
  coherence and source-ordering controls, then add matching-host instruction and
  concurrent execution coverage. Serial tests and bounded exploration are not a
  formal memory-model proof. Keep the strong `Receive` baseline unless an exact
  protocol proof admits weakening; device/DMA and interruption ordering retain
  their separate contracts.

- **BLOCKEXEC.** Implement the ordinary package-level blocking executor in
  `source/library/blocking-executor/` under
  [task-runtime library contracts](wiki/spec/build/task_runtime.md#library-and-foreign-providers)
  and [foreign execution](wiki/spec/build/foreign_bindings.md).
  It owns bounded queues, moved submissions and linear completion claims, not a
  new compiler task-runtime mode.

  The package has contract declarations, not working queue/worker bodies.
  `compiler/tests/canary_suite/task_runtime.rs` checks package consumption,
  provider selection, linear-slot take/restore and conditional claim returns.
  These are checked-tree fixtures, not execution. The slot example uses literal
  index zero; the admission example passes a pre-existing ticket into `submit`.
  They refute blanket claims that slot vacancy or multi-case linear returns are
  unavailable, but do not establish a bounded FIFO or real claim issuance.

  - Implement generic bounded queue and executor bodies, with indexed occupancy,
    head/length/capacity proofs and surviving ownership of every queued payload.
    Exercise the actual combined type/const `[T; N]` methods through ordinary
    **RUNTIME-VALUE-GENERICS**; do not keep the obsolete blanket attached-method
    blocker or manufacture an executor-only lowering path.
  - Implement accepted/refused admission, provider-instance ticket issuance and
    exactly-once settlement using ordinary conserved-claim/outcome evidence.
    Acceptance must mint a claim against the actual executor, not receive a
    fabricated/pre-existing ticket as the canary does. Rejection returns the
    complete moved submission; close requires empty queue and no live claims.
  - Connect selected worker and wait/wake providers to **TR3-TR8** execution.
    Preserve thread affinity and the actual loans through suspension, including
    settlement's executor borrow; passing only a wait word does not verify that
    borrow or its instance relationship. Cancellation requests retain the claim.
  - For bounded recovery from hung workers, implement actual process-isolated
    execution with the exact parent endpoint, child manifest, custody and
    termination evidence required by provider selection. The empty
    `IsolatedWorkerProvider` marker and inherited conformance test supply no
    isolation evidence; never terminate an in-process worker to reclaim custody.

  Acceptance: consume the package through its normal dependency/build path and
  execute queued work, full-capacity refusal, FIFO delivery, park/wake and
  settlement with linear payloads. Reject duplicate/foreign/stale tickets, lost
  custody and premature close/reclaim. Exercise isolated hung-worker recovery
  separately; checked declarations and provider-plan selection alone do not pass.

- **QUOTIENT-THEOREM-LIFT.** Complete checked operation correspondence and
  publication under [lifting operations](wiki/spec/proofs/quotients.md#lifting-operations).
  Managed direct `Quotient::define` and transport-backed `Quotient::lift`
  already have checked-only source canaries. Reuse their post-termination
  admission and proof-only request handling; mathematical admission is not
  executable realization.

  The local relation/result-flow checks are broader than their canonical
  publisher. In validation's `proof_contracts/quotients/relation_plan/`,
  argument adaptation and immutable result aliases have checked forms.
  Reuse transport-backed `lift` argument maps (selection, permutation,
  repetition, omission), immutable result aliases, and finite-state forwarding
  through the `quotient-result-flow` codec and independent correspondence.
  `terminal_bridge.rs` still admits monomorphic,
  single-state, direct position-preserving rows elsewhere. Complete missing
  correspondence judgments and extend publication and independent
  reconstruction for result computation, preconditioned `define`, and
  generic/private applications; reuse the local judgments already implemented.
  Retain complete application, relation, precondition and theorem-role evidence.
  Do not multiply source-shape recognizers to publish each arrangement.

  Representative/theorem eligibility still requires checked termination,
  purity, crash freedom and exact hermetic identities; theorem closures must
  be bodyful and free of admitted assumptions. Check helper types/statements
  transitively, not only visible bodies. General assumption-closure machinery
  and the congruence-only `lift<F, Congruence>` canonical payload belong to
  **PROOF-CONTRACT-MIGRATION**; coordinate that payload's codec, verifier and
  package-review consumers rather than inventing a second wire form.

  Acceptance: package-backed direct `define` and transport-backed `lift`
  admit through ordinary `omega --check`, not only an isolated lowering
  helper. Their [published correspondence](wiki/spec/proofs/quotients.md#published-quotient-correspondence)
  rederives on decode and supports the broader admitted applications above.
  Preserve rejection of implicit lifts, missing/surplus/reversed roles,
  invalid laws, unproved termination, non-hermetic identities, admitted or
  boundary theorem closures, and custody-bearing quotients.
  **QUOTIENT-RUNTIME-REALIZATION** owns execution separately.

- **QUOTIENT-RUNTIME-REALIZATION.** Give admitted quotient operations a
  checked executable value/call path under the
  [representative realization contract](wiki/spec/proofs/quotients.md#representative-layer-and-executable-realization).
  The bounded direct correspondence forms already provide a starting point;
  this work need not wait for every generalization in QUOTIENT-THEOREM-LIFT.

  Checked contracts, ranges and termination, and subsequent nested/mixed-call
  lowering still exclude `quotient_operation` calls. Terminal validation
  rejects nonempty retained correspondence tables with
  `NonExecutableQuotientCorrespondence`.

  Measured: both named customers,
  `pass/proofs/quotient_define_managed_compile` and
  `quotient_lift_managed_compile`, pass checking, produce a Terminal module,
  and stop there at exactly that refusal. Nothing in checked contracts,
  ranges or termination refuses them first. But neither demonstrates an
  executable route, and that is deliberate: each ends in `machine
  Main::main(&mut self) {}`, and `quotient_define_managed_compile` says in
  its own header that "the retained correspondence is never an executable
  callee". They are honest compile canaries for the admission shape, not
  executable customers to be repaired by giving them entries. What the corpus
  lacks is a separate invoking customer -- there are 17
  `data X = Carrier % relation` declarations in all, 14 of them in `fail`,
  and none calls a quotient operation from a program entry. Author that
  customer before deciding which exclusion to license, and leave these two
  canaries proof-only. Replace those exclusions only
  where a checked representation/operation judgment licenses the ordinary
  representative call. Retaining a mathematical correspondence row alone
  does not establish all executable custody.

  Preserve the representative ABI and permitted uncanonicalized constant
  materialization without exposing representative-sensitive structural
  observers. Axiomatic operations supply no algorithm; executable use needs
  checked computation or a realization with correspondence. Keep custody-
  bearing quotients fenced and fact-only admission free of runtime calls,
  dictionaries or fuel charges.

  Acceptance: direct `define` and transport-backed `lift` customers reach
  source-free Terminal checking and matching-host native execution, with
  result/argument/relationship substitution and missing correspondence
  rejected independently. Preserve refusal of illicit representative
  extraction and unsupported realization. The quotient-refusing policy
  control additionally depends on PROOF-CONTRACT-MIGRATION's transitive
  assumption closure through helper types/statements; do not confuse that
  integration dependency with the initial executable route.

- **PRIVILEGED-PORT-EFFECT-SETTLEMENTS.** Connect checked adapter port I/O
  to native production under [consumer-owned settlement](wiki/spec/terminal-psi/boundary_calls.md#consumer-owned-settlement).
  PE/ELF/Mach-O evaluated locator custody is a separate, implemented route;
  privileged instructions belong to checked `asm`, not a foreign-binding
  instruction language.

  `TargetUnitOperation::PortWrite`, x86 encodings and independent
  port-effect custody readers exist. Ordinary selection, register-home/
  post-allocation transport and machine-fragment emission still do not
  produce the port effects: image-emission's
  `function_fragments/production.rs` publishes an empty roster.
  Carry an installed selected adapter's exact `PortIo` requirement,
  service/port/value, operation ordinal and byte span through that route.
  Preserve the effect-before-settlement join through object rebasing, final
  image and installation evidence; reuse the existing readers in
  native-artifact's `physical/derivation/provider_custody.rs`.

  Use a positive fixture whose checked adapter owns the selected requirement.
  Direct-root privileged operations must continue to reject without provider
  custody; do not turn their negative fixtures into a bypass. The
  `asm_runtime_port_msr_final_validation` customer also needs its actual
  selected bindings; unrelated machine-control/root-binding gaps do not
  belong to this port transport repair. Port read (`in`) additionally needs
  its Terminal operation and producer route; `DirectPortReadU8` settlement
  metadata alone does not supply them.

  Acceptance: an authored selected adapter produces and independently
  validates its exact native effects. Missing, duplicate, substituted,
  role-swapped or unconsumed records, changed service/port/value/target,
  detached spans and altered bytes reject. Keep unsupported-target refusal.
  Separate emitted-byte verification from execution, which requires an
  appropriately admitted privileged environment, not raw I/O in a hosted test.

- **FLOAT-PROVIDERS.** Complete checked source and independently replayed
  `FloatSemantics` obligations used by the actual `Float::*` slot contracts,
  under [FloatMeaning](wiki/spec/terminal-psi/mathematical_values.md#floatmeaning).
  Closed Meaning-valued applications and authored equality ensures already
  reach produced artifacts and independent proof replay; reuse that route.
  Its checked binder, evaluator and Terminal application carrier specialize
  Meaning results and Format/Meaning operands. Complete Boolean,
  classification and integer-conversion contract discharge through the
  appropriate proof carriers, not by admitting those results as FloatMeaning.

  Owners: checked `proof/float_meaning.rs`, validation's
  `float_projection_bindings`, checked/lowered contract production and
  Terminal semantic/proof replay. Preserve exact declaration/signature,
  owner/use-site, result and argument correspondence; shared numerical kernels
  and well-formed metadata alone do not establish a source obligation.

  Acceptance: actual slot-contract customers retain and discharge their
  obligations through produced source-free artifacts, including non-Meaning
  results and imported call/operator results. False claims, lookalike
  declarations, altered operands/formats/results and missing evidence reject.
  Preserve IEEE Boolean comparison separately from structural meaning
  equality, including NaNs and signed zeros. Keep
  `produced_artifact_verifies_authored_float_meaning_ensures` and its forged/
  missing-evidence controls. Native FMA transport belongs to
  **X86-FMA-PROVIDER-TRANSPORT**; constants to **FLOAT-IDENTITY-LITERAL-CARRIER**.

- **RESTORE-DYNAMIC-DESCRIPTOR-AND-TABLE-CUSTODY.** Finish ordinary native
  borrowed-descriptor invocation and forwarding under
  [dynamic dispatch](wiki/spec/terminal-psi/dynamic_dispatch.md).
  Start with the non-entry helper that forwards a descriptor, dispatches
  through it, and uses the result across calculations and branches.
  `forwarded_descriptor_calculations_execute_through_verified_artifact`
  establishes source-to-artifact interpreter execution; Target/Legalized
  descriptor ABI replay also exists. Instruction selection still rejects
  `LegalizedScalarInstructionKind::DynamicParameterCall`.
  The next milestone must advance this native customer, not repeat
  target-only descriptor composition.

  Connect the ordinary indirect-call operand through selected construction/
  replay, register allocation, ISA encoding and emission, then exact immutable
  table, relocation and final-image custody. Preserve the requirement-owned
  erased caller shape, ABI/clobber/effect/reach obligations, original referent
  and selected table through joins; Unit calls have no invented result home.
  Tables, zero trap slots and nonoverlapping descriptor/result storage must
  remain independently reconstructible. Remove superseded forwarded-call
  recognizers/records as the ordinary route replaces them, rather than adding
  another whole-body pattern or unproved devirtualization.
  Complete source composition through ordinary ordered-operation/storage
  owners: `unit/dynamic_composed_unit/forwarded_helpers.rs` still rejects
  extra effects, mutable helper locals and different helper/requirement result
  carriers. Retain their own call/result identities and access/effect evidence.
  Preserve `cross_package_visibility`'s public dynamic return with opaque
  producer-selected private evidence, exact receiver-loan origins and rejection
  of consumers directly selecting a dependency's private conformance.

  Acceptance: a source-rooted closed-conformance fixture publishes and
  independently replays Linux x86-64/AArch64, executes on matching hosts,
  selects distinct implementations at runtime, and preserves surrounding
  calculations and writes to the original referent. Reject substituted
  instance/table/slot/ABI/access/relocation/code-span evidence. Retain the
  interpreter and legalization mutation controls; include an additional
  ordered effect, mutable local and differing helper/requirement scalar result.
  Receiver-entry provisioning is a separate dependency of that entry customer,
  not of a non-entry helper.
  Owned erased cleanup remains with
  **CLEANUP-HOOK-SELECTION-AND-ERASED-OWNERSHIP**.

- **TARGET-SEMANTIC-APPLICATIONS.** Complete the portable
  [target-semantic capsule](wiki/spec/language/evaluation.md#target-semantic-capsule)
  and artifact-qualified
  [application closure](wiki/spec/terminal-psi/boundary_calls.md#operator-applications-and-physical-children).
  Provider-dependent const applications already defer and fold under exact
  selected execution. Preserve named/token, field/let/return, alternate-provider
  and unselected-provider controls rather than rebuild that route.

  Retain sealed versioned target semantics, exact selected plans,
  implementations and arguments as replayable inputs shared with realization.
  Typed observations name their actual address-space/layout/format/profile
  subject and reject unresolved subjects; compiler-host state supplies none.
  Reuse ordinary typed arguments and compiler-materialized target data where
  they express the customer. The spec does not approve a general
  `TargetSemantics::*` source API; do not add one merely to implement the
  capsule. A genuinely missing observation capability needs a concrete
  customer and the normal owner-escalation audit.

  Extend package manager review's `symbolic_boundary_applications` beyond
  explicitly supplied direct-type-binder requests to the complete reachable
  specialization set, preserving original mappings and complete canonical
  arguments for replay. Recheck actual reach, transitive proof obligations,
  target facts, admission and selected realization after substitution.
  A reviewed closed demand alone grants no Terminal/native, coverage or
  installation authority; complete-set composition is not an install/update
  prerequisite. Preserve the distinction between an operator's canonical
  empty telescope and a boundary-trait call with no telescope. Physical-child
  binding stays with TRANSLATION-VALIDATION in `TASKS_OPTIMIZER.md`.

  Acceptance: source-free cross-artifact customers reproduce exact selected
  application/target identity and all reachable obligations; missing, extra,
  stale, substituted or unresolved mappings cannot acquire coverage.
  Retain provider-free evaluation, distinct selected const results and
  false-result/unselected controls. Reject a build dependency cycle rather
  than searching visible satisfiers or silently choosing a body.
  Checked-source folding or a manually supplied closure request alone does
  not close this acceptance.

- **TOP-LEVEL-BOUNDARY-REQUIREMENTS.** Complete selected execution and
  source-free identity for explicit `boundary requirement Owner::name(...);`
  under [machine supply](wiki/spec/language/machines.md#supply),
  [provider selection](wiki/spec/build/provider_selection.md) and
  [requirement identity](wiki/spec/terminal-psi/boundary_calls.md#call-and-requirement-identity).

  First close executable customers using the existing dispatch machinery.
  `selected-dispatch/src/boundary_dispatch/adapter_rows.rs` admits owned,
  shared and mutable receivers, erased lifetime telescopes, and exact closed
  generic specialization rows. Extend remaining generic/family and projected
  receiver combinations through both engines; write-only and qualified
  receivers still reject. Settlement tests alone do not establish execution.
  Preserve `providers/checked_boundary_requirement_dispatch_exit` and
  `providers/lifetime_boundary_requirement_dispatch_exit`: recorded exits are
  70 (checked) and 41 (lifetime) on macOS ARM64 and Windows x86-64.

  Close external satisfiers with structural arguments and interpreter
  provider execution. `providers/external_boundary_requirement_via_exit`
  currently records native rejection at foreign-mechanism policy
  classification; the checked interpreter rejects bodyless external calls
  without a provider. Restore the mixed borrowed-record customer with its
  initialized fields and reused returned value, then execute it on supported
  hosts. The scalar fixture cannot establish that broader route; retain
  missing-import rejection and distinguish ordinary foreign exit from
  canonical ProcessExit.

  Complete serialized replay of the package-qualified operation, full static
  telescope, signature, contract, visibility, selected provider/adapter and era,
  with a canonical kind distinct from trait requirements. Reuse package
  review's existing top-level requirement kind and telescope projection.
  Lowering already retains the requirement and catalogs checked adapters;
  the registered `boundary_internal_call_evidence` test checks the installed
  provider's physical call custody. Preserve those joins while completing
  the portable contract with **PROVIDER-SELECTION-AFTER-TERMINAL**.

  Acceptance: value, statement, receiver and external-provider customers
  execute in the interpreter and natively under one exact selected plan;
  source-free replay rejects unselected, ambiguous, substituted and stale-era
  authority, unauthorized private/foreign declarations, and cross-package
  substitution of same-spelled declarations. Coordinate `Task::finish`/cancellation
  and interrupt completion with **BOUNDED-INSTALLATION-REACH-ROWS** and
  **COMPONENT-SUBSTRATE** for selection and lineage, without compiler-owned
  lifecycle policy. **OPERATOR-MACHINE-SUPPLY** owns source supply migration
  and retirement of `MachineSupplyMode::Boundary`.

- **BUILD-ADMISSION-CHECKPOINT.** Finish command-level
  [restricted-build acceptance](wiki/spec/packages/acceptance.md#restricted-build-acceptance)
  and executor-grant integration across install/update/review/resume and
  consuming compilation. The pre-effect checkpoint is implemented in
  `checking/build_continuation.rs::AdmittedBuildCheckpoint::execute`;
  `check_project`, `check_locked_sources` and `compile_project` supply it.
  Reuse its refused/granted-effect sentinel controls rather than rebuild the
  gate or move consent back after execution.

  Supply a real restricted-request command customer, including a dependency
  used for both Build/Product purposes and its generated-source handoff.
  Manager review sessions currently use compiler-owned private staging;
  empty request sets do not demonstrate this acceptance. Bind requests and
  grants to exact package occurrence, dependency path, checked purpose,
  target/execution profile, operation, logical scope and bounds. Product
  acceptance cannot authorize the same package's build effects.

  Acceptance: initial installation and newly added, widened or transitive
  requests stop before their restricted effects and expose those coordinates.
  Acceptance plus a genuine executor grant resumes the exact candidate, then
  reviews generated source; missing/refused/cross-purpose grants perform no
  restricted action. Audit inspection acquires no host authority. Unchanged
  request meaning needs no recurring approval; source changes remain visible.
  Resolution-only updates preserve approval meaning. Exercise interrupted
  review/publication and stale candidates without storing secrets or machine-
  specific grant paths in the lock. Dependency locks confer no host authority;
  already performed host
  effects have no implicit rollback.

  Preserve coherent source/authority snapshot replay, purpose-drift refusal
  and authored-before-generated resolution. Captured immutable inputs and
  bounded compiler-private staging remain benign, while supplied host
  directories remain restricted. No recursive build API, own-build
  final-component query or hidden post-compilation callback.
  **BUILD-SNAPSHOT-OUTPUTS** owns committed output sets and required-output
  settlement at publication.

- **OPTIONAL-STDLIB-SEMANTIC-BINDINGS.** Finish ordinary explicit std/alloc
  dependency migration under the [toolchain/library contract](wiki/spec/packages/toolchain.md).
  Std may be replaced, split or absent; only core and the specified
  compiler-injected vocabulary retain toolchain authority. Standalone
  std/alloc still receive broad `Toolchain` classification in
  `omega-rust/omega/compiler/build-evaluation/src/sources/source/source_storage.rs`.
  Remove that fallback as remaining consumers acquire exact source-byte
  catalog roles or accepted semantic bindings, not by relabeling a directory.

  The migration is 131 directories. 127 carry no `build.omg` at all and
  compile standalone straight into the fallback -- 91 `fail`, 23 `pass`, one
  `run` and 12 samples -- so each needs a build declaration before it can
  carry an ordinary edge. The other four have a `build.omg` with no `depend`:
  `fail/build/build_effects_undeclared`,
  `fail/build/uefi_program_entry_wrong_calling_policy`,
  `fail/build/build_machine_wrong_arity`, and `source/library/std` itself.
  Migrate remaining corpus/member imports and package roots, including
  bundled proof, host/objc, fail and run consumers, through ordinary
  dependency edges and target-correct Console, Filesystem and physical-entry
  bindings. Keep freestanding roots dependency-free. The existing narrow
  roles, including macOS x64 entry, are documented
  [beside package compilation](omega-rust/omega/compiler/package-compilation/semantic_bindings.md).
  Route exposed composed-call, ownership, entry and proof failures to their
  capability owners, retaining each customer's intended checking/execution
  acceptance; unrelated blocked features do not stop independent migrations.
  Private core proof declarations remain private: obtain an authorized proof
  route with PROOF-CONTRACT-MIGRATION/PROOF-KERNEL-CORE, not a per-fixture
  visibility exception.

  Acceptance: removing a dependency rejects its imports/provider selections;
  aliases, paths, names and same-spelled substitutes confer no authority.
  Rejoin exact package, declaration, schema and any role-required selected
  plan; stale/substituted bindings reject independently of lock replay.
  `repository_build_declarations.rs`'s sample check now handles a workspace
  root and its members and skips registered submodules, so it passes again;
  its 131-directory backlog is invisible to it, because a directory with no
  `build.omg` is never collected as a root. `samples/apps/squalr` is why the
  skip exists: all seventeen member declarations spell hyphenated package
  identities that Omega's snake_case rule refuses, and this checkout must not
  edit that submodule. Its sibling
  `omega_cases_declare_a_standard_library_edge_exactly_when_they_use_it`
  still fails, on the opposite error --
  `tests/omega/pass/providers/boundary_scalar_field_store` declares a
  standard-library edge it never uses. Extend those recursive member/import
  checks further as fixtures migrate and preserve the missing-edge/alias controls in
  `standard_library_package_resolution.rs`. Early missing-import rejection
  is sufficient; do not weaken assembly to manufacture a second diagnostic
  for a provider selection whose import already failed.

- **COMPONENT-SUBSTRATE.** Deliver independently selected component products
  and their admission/replacement closure under the
  [component publication contract](wiki/spec/build/component_publication.md).
  Runtime packages or Cathedral own deployment, acquisition and update policy.

  Description publication and checked `Independent` settlement already have a
  production path through package-manager dependency compilation,
  `compiler::published_independent_component_description` and
  `component-description::verify_component`. The current product boundary
  remains `checked-compilation-to-terminal-artifact`'s
  `terminal_artifact/composition_modes.rs`: both Terminal and native products
  reject settled independent edges because symbolic imports/exports,
  entry/leave resources and installation/replacement custody are absent.
  Remove that fence only when the requested independent product exists; never
  emit a silently fused substitute.

  Carry the exact import/export, complete entry/outgoing-authority, service,
  mapping, lease, stack and retained-provider inventories into the product.
  `CustodyKind` and `ObligationKind` already have Mapping/Lease vocabulary;
  supply actual producer facts, independent reconstruction and per-occurrence
  discharge, not more enum-only scaffolding. Connect native
  `component-candidate::describe_component`'s derived stack and realization
  identity to publication. A Psi-only capsule correctly has no native facts;
  do not fabricate them to fill the fields.

  Independent admission/replacement must consume the same verified-description
  contract as **TOPOLOGY-PLAN-VERIFICATION**, retaining exact artifact, profile,
  authority and resource correspondence. `component-deployment` currently
  binds candidate/installed-byte custody directly, not a verified description;
  preserve those checks while joining the shared contract. Topology's simulated
  replacement-profile controls do not establish loaded execution or retirement.
  **BOUNDED-INSTALLATION-REACH-ROWS** needs this complete component contract;
  **WIRE-RUNTIME-AND-INSTALLATION** owns platform installation/execution and
  **PSI-COMPONENT-REPLACEMENT** owns the interpreted embedding customer.

  Acceptance: an ordinary two-package build selecting `Independent` emits
  the independent products and publishes/consumes the dependency description
  without test-side attachment. A source-free consumer checks exact subject,
  profile, schema, all entries/outgoing authority, custody, retained providers
  and inseparable assumptions, including startup, callbacks, timers and cleanup.
  Preserve corrupt/truncated, substituted/stale/unrelated, early-frontier,
  omitted/forged inventory, duplicate dependency/plan and unaccepted-assumption
  rejection. Reading a description grants no callable or installation authority;
  installation facts require fresh per-occurrence admission.

  Exercise actual authorized stable-slot replacement: compatible imports/exports
  and behavior exclusions, fresh candidate resource/profile admission, new calls
  entering the new era, old sessions retaining theirs, and retirement only after
  every activation, registration, returned object and claim has a valid disposition.
  Pin state migration/retention, coexistence capacity and failure behavior.
  Unauthorized, incompatible, weaker-policy, underprovisioned and premature
  operations reject without losing custody. Continuity-free replacement need not
  drain every old era before publication; stronger continuity needs its explicit
  contract. Keep model-level checks distinct from this execution acceptance.

- **FFIVAL.** Complete and run the Windows user32 boundary-coherence
  customer at `tests/omega/pending/host/user32_window_procedure_registration`,
  through its authored build root and the generic
  [private-callback protocol](wiki/spec/build/private_callbacks.md).
  Reproduce its pinned `cannot prove requires contract` diagnostic before
  assigning a repair; generic result-payload qualification transport exists,
  while this customer uses a by-reference outcome.

  REGISTERED-CALLBACK-LIFETIME owns outcome/capacity lifecycle;
  CALLBACK-PRIVATE-MATERIALIZATION owns callback/private-layout realization.
  Supply the package-owned user32 provider/adapter, exact native ABI/layout
  and explicit per-window context protocol. The fixture currently declares
  an abstract boundary and a contextual procedure, not completed foreign
  bindings to Win32's actual callback.

  Acceptance on Windows: actual registration, `WM_NCCREATE` and dispatched
  callback entry, deterministic rejection/retry, teardown and reuse of returned
  capacity, with quiescence before releasing code leases. No raw callable
  address, implicit closure or Win32-specific compiler escape. Promote the
  pending canary only when its corresponding acceptance works; checking and
  diagnostic watchers alone do not establish foreign invocation. Other hosts
  explicitly report this runtime leg unavailable.

- **WIRE-RUNTIME-AND-INSTALLATION.** Connect the admitted executable
  installation lifecycle to an authored/package provider route and a
  matching-host executor under [executable installation](wiki/spec/build/executable_installation.md).
  Reuse `executable-installation`'s linear transitions, required-fact gates,
  sealed entry references, replacement/quarantine joins and
  `image-emission/installed_artifact.rs` custody binding.
  `OwnedImageProvider` performs convention-controlled buffer operations and
  its driver composes their gates; its resident-byte borrow does not establish
  physical invocation or platform execute/cache operations.

  Complete the receiving-provider/executor join: exact admitted content,
  placement, audience and installed occurrence; target-appropriate
  write/execute and fetch-visibility completion; invocation through a
  requirement-compatible sealed entry; live-root/provider custody through
  return, retirement and replacement. Keep platform operations with their
  provider rather than adding OS policy to the generic lifecycle model.
  `ConventionOnly` is permitted when honestly reported; do not universally
  require hardware enforcement or accept `Unsupported`. Ordinary executable
  file emission is not receiving installation. The lifecycle states are not
  prescribed source type names and do not by themselves require new grammar.

  Acceptance: an authored program reuses an admitted artifact across distinct
  linear placements, materializes/freezes/validates exact final bytes and
  footprint, installs through the contracted provider and actually invokes on
  a matching host. Retirement requires quiescence, execute removal, restored
  write authority and demanded facts; incomplete drain quarantines.
  Replacement patches only declared sites with bound admitted fragments and
  establishes visibility/write re-suspension before draining old custody.
  Reject substituted bytes/authority/contracts, transplanted validation,
  double-spent placement, missing completion facts and unsupported W^X;
  failed transitions return their inputs. Keep arbitrary bytes-to-code,
  JIT and numeric-address-to-execution routes unsupported; inert address
  observation belongs to **ADDRESS-EXPOSURE**, not installation authority.
  **COMPONENT-SUBSTRATE** owns component closure; this row owns generic
  executable custody.

- **FLOAT-IDENTITY-LITERAL-CARRIER.** Complete named floating constant calls
  and payloadless-NaN evaluation under [constant materialization](wiki/spec/language/constants.md#materialization)
  and [invocation admission](wiki/spec/language/evaluation.md#invocation-admission).
  Reuse the existing arithmetic/comparison, public/imported identity and
  ordinary-machine result paths; preserve one-time format landing.

  Connect `pub const ROOT: f32 = F32::square_root(4.0f32);` and classification,
  conversion, directed-rounding and fused-operation customers to exact selected
  execution and replay. Current gates span `symbols/targets/calls.rs`'s
  Data-receiver lookup, which only tries attached machines;
  `constant/initializer_dependencies.rs` and
  `const_generic_expressions/call_custody.rs`, which require Machine/State
  targets; and `const_initializers/invocations.rs::selected`, which requires
  a closed ordinary entry. Reconcile initializer dependency discovery with
  the existing typed/checked call selection rather than inventing a second
  resolver. One family symbol is not an exact overload: keep
  `ambiguous_named_operator_call_does_not_manufacture_early_identity` and the
  authored occurrence/declaration join.

  Reuse `closure_needs_operator_selection` and the checked interpreter's
  `FloatSemantics` execution, including selected intrinsic rewrites. A catalog
  identity or matching spelling alone does not admit a boundary provider;
  preserve selected-body, argument, contract and package authority before
  execution. `const_lengths/tests.rs` already checks named provider-body
  evaluation/replay and rejects unselected fallback. Extend this machinery to
  initializer calls without another float engine or name-dispatch table.
  **FLOAT-PROVIDERS** owns bodyless contract discharge;
  **PROVIDER-SELECTION-AFTER-TERMINAL** owns agreement of const selections
  across realized targets.

  Carry undetermined NaN meaning through admitted proof/compile-time uses
  without demanding runtime bits. `const_generic_expressions/value.rs`,
  `const_initializers/invocations.rs` and `materialize.rs` currently reject it
  at their value boundaries. Materialization still requires canonicalization,
  explicit bits or an exact selected realization; never publish the evaluator's
  arbitrary payload or add literal syntax to evade that requirement.

  Acceptance: source-authored f32/f64 customers evaluate and independently
  replay, then publish determined bits matching runtime behavior. Preserve
  signed zero, format, imported-alias and aggregate/call-result custody;
  `noncall_float_replay.rs` and `tests/floating_aggregates.rs` reject altered
  results, encodings, formats and erased computation, including coordinated
  forged NaN receipts. Proof-only NaN use must succeed while undetermined
  runtime representation and forged selection still reject. Existing
  `float/build_runtime_semantics_twins_*` exercise array-length calls, not the
  direct named initializer; retain parity without treating them as its acceptance.

  Recheck the std-importing constant-call customer through current canonical
  target-family selection. Its historical whole-closure refusal at
  `ConsoleNativeProvider::read_line`/`read_byte` is a lead for
  **FILTERED-CALLEE-CALL-SITES**, not a permanent no-std restriction. Use a
  reduced fixture to isolate admission, then close the imported customer.

- **X86-FMA-PROVIDER-TRANSPORT.** Carry checked/legalized scalar FMA
  occurrences through ordinary selected-instruction transformation,
  register-home assignment, machine planning and image publication.
  Reuse the feature-custodied encoder in `machine-emission/src/x86_fma.rs`
  and independent artifact readers; their test-driven fragments are not a
  connected provider-execution route.

  Preserve exact operation/operand/result identities, selected provider and
  AVX+FMA3 admission, XMM homes, canonical MXCSR save/install/restore, and
  per-occurrence physical coverage. Remove the object, entry and optimization
  fences only as their required transport is independently checked, not by
  bypassing them. The production native realization still rejects retained
  FMA before instruction selection.

  Acceptance: the source-evaluated-import/FMA customer pinned by
  `retained_x86_fma_and_source_evaluated_import_stop_at_fma_transport`
  publishes and executes on a matching admitted host, including nested
  foreign-call control custody. Both formats distinguish fused from
  separately rounded cancellation; absent/wrong-profile feature admission,
  operand/provider substitution and corrupted emitted evidence reject.
  Report cross-target byte replay separately from hardware execution.

- **ASM-INSTRUCTION-CATALOG-EXPANSION.** Carry accepted checked assembly
  through the ordinary source-to-native pipeline under
  [assembly](wiki/spec/language/assembly.md) and the
  [catalog](omega-rust/psi/foundation/language-core/inline_assembly.md).
  Catalog/checking coverage is not executable support.

  Resume at `omega-rust/psi/pipeline/04_typed-trees-to-checked-trees/src/execution/terminal_unit/calls/call_operations.rs`:
  dedicated asm-call planning handles `AsmPortOut`, while
  `asm_value_intrinsic_result_types_reach_the_call_operation_frontier` pins
  missing operation plans for value intrinsics. Complete checked plans,
  lowering, Terminal encoding, independent verification, target realization
  and final-span/state evidence together. Reuse ordinary assignments/transitions
  where they express the operation: canonical unordered `ldr`/`str` already
  lower through typed places. Do not require a new opcode per source spelling
  or a separate asm-only body recognizer.

  First acceptance is the existing `canary_suite/inline_asm.rs` x86 fence,
  interrupt, flags, MSR and control-register byte-emission customers, followed
  by its checked-only pipeline-directive/cache and AArch64 system-register
  fixtures. Preserve exact target, operand widths, authority/reach, clobbers,
  ordering and modeled exits through emitted bytes. Wrong-target, missing
  authority, stale postcondition, invalid saved-place, omitted clobber and
  mismatched evidence controls must reject. Cross-compilation/byte assertions
  do not establish privileged execution on a host.

  Further memory, atomic, barrier-option, cache/TLB and mode-transition families
  need a customer and complete contracted operands/effects, not just more
  recognized mnemonics. Memory access must retain authorized extent/view,
  bounds, alignment, initialization and access permission; numeric addresses
  confer none. Unknown instructions and hidden/unmodeled exits keep rejecting.
  **PRIVILEGED-PORT-EFFECT-SETTLEMENTS** owns the port-read/write provider
  adapter integration; do not duplicate that assignment.

  Preserve the settled build authority route: hosted grants for `port_io` and
  `interrupt_table` are independent; machine-owner authority remains
  freestanding-only. Reach is not authority, and helper calls cannot hide
  obligations. Keep the closed terminal-authority inventory and commitment
  consistent when catalog mechanisms change; exercise an ordinary native
  compile so a production inventory assertion cannot hide behind source checks.

  **INTERPRETED-ASSEMBLY** owns the settled interpreted execution route; native
  support need not wait on it. The checked interpreter's unit-return arms in
  `interpreter/evaluator/statements_and_calls.rs` do not
  establish fidelity for unmodeled halt/register/cache effects. Unsupported
  effects must reject, while genuinely elidable hints retain their catalog
  meaning; do not silently turn all asm into successful no-ops.

- **RUNTIME-SIZED-ACTIVATION-CONTRACT.** Connect the ratified
  [bounded activation claim](wiki/spec/resources/activation_storage.md) to
  authored source and Terminal Psi. Use ordinary callable/core-declaration
  mechanisms; an absent `claim` keyword is not an owner-design blocker.
  `psi/foundation/extents/src/activation_claims/` supplies bookkeeping,
  not an executable compiler route.

  Expose compiler-provisioned activation backing with exact activation
  provenance/lifetime, then retain committed extent, bound, release order and
  suspension claim-site rows through checking, lowering, codec and independent
  verification. Keep backing nonmoving with stable materialized addresses for
  the claim lifetime. Owners: ordinary call admission, extent/claim evidence,
  Terminal representation and stack-demand composition. Allocation packages
  can manage already-held backing but cannot mint its grant from an address
  and length. Runtime generic extents and fixed-array establishment are not
  this supply mechanism.

  Acceptance: a source-authored bounded claim supports access, reverse-order
  release and suspension retention under its declared bound; over-bound
  establishment returns checked failure. Escaping claims, missing/duplicate
  sites, wrong provenance, stale loans and invalid release/suspension custody
  reject independently. **FRAME-LAYOUT** in `TASKS_OPTIMIZER.md` owns final
  frame, probe, unwind and native replay. Do not add implicit variable-sized
  locals, provider-backed issuance, a new syntax category or an OS allocator.

## Checked boundary topology

Finish this later library customer under the
[reference-package contract](wiki/spec/packages/topology.md).
**COMPONENT-SUBSTRATE** owns component completeness;
**WIRE-RUNTIME-AND-INSTALLATION** owns generic executable custody. Keep graph
policy and orchestration in ordinary Omega packages; no compiler graph stage,
topology IR or trusted graph axiom.

- **TOPOLOGY-PLAN-VERIFICATION.** Connect and validate the Omega build-only
  composer in `tests/omega/packages/build-scope-topology` and consumer in
  `source/library/topology/verify.omg`. Reuse `omega-rust/library/topology/`'s
  normalization, policy, certificate and codec logic and
  `VerifiedComponent::export_contracts()`; derived contracts, not surface-name
  hashes, join demands to supplies.

  `topology-plan`'s `composition_build` test
  `the_package_composes_and_a_source_free_consumer_verifies` targets actual
  composition, required-output publication and independent Rust verification.
  At `c6a81ec63ea` on macOS arm64 it fails in `copy_fixture`: committed
  `request.bin` differs from `build_scope_inputs()`, before Omega compilation.
  Reconcile derived inputs and rerun that test; do not claim publication passes.
  Its admissions come from Rust-built Terminal modules, and it does not exercise
  the Omega consumer. Join ordinary component-description production/admission;
  **BUILD-SNAPSHOT-OUTPUTS** owns confined inputs and output publication.

  Acceptance: admit three component descriptions, publish `payments.plan`, and
  verify source-free against independently supplied current `TopologyRequest`.
  Establish ordinary algorithm correctness evidence; resource exhaustion is
  unsuccessful verification. Preserve the spec's
  [controls](wiki/spec/packages/topology.md#diagnostics-and-implementation-acceptance):
  missing/substituted components, stale subjects, forged completeness,
  direct/indirect bypasses, cycles, distinct instances, duplicate bindings,
  disconnected selectors, missing policies and corrupt plans. Never load
  unselected policy code or substitute live reads/handwritten inventories for
  admitted facts. Plan verification grants no installation authority.

- **TOPOLOGY-PRIVATE-PIPE-INSTALLATION.** Run the
  [three-process payment customer](wiki/spec/packages/topology.md#first-executable-realization)
  with an Omega-authored installer and component programs on Windows and macOS,
  after **TOPOLOGY-PLAN-VERIFICATION** and the shared component/installation
  dependencies above.

  Reuse `omega-rust/library/topology/src/topology_installation.rs` and its
  mediation, frame, schema, pipe and supervisor modules. The crate's
  `tests/installation.rs` and `tests/process_confinement.rs` exercise Unix
  processes re-executing Rust test binaries; `LocalProcessSupervisor` is Unix-only.
  Complete explicit Windows handle transfer and macOS validated/signed member
  images using the existing descriptor-transfer path. Package code orchestrates;
  providers own mechanisms. Disable general inheritance and deliver exactly the
  admitted endpoints before application entry.

  Acceptance: admit exact images/mappings; allow one outstanding schema-checked
  request/response per binding. Ungranted endpoints and substituted mappings
  reject; malformed frames and peer failure close the binding. Failed setup and
  quiescence clean the entire roster, including killed members, or return retained
  custody. Preflight replacement, quiesce the old generation, then activate the
  new one without overlap. Record actual execution on both hosts.

## Rust compiler release closure

The [completion contract](wiki/drafts/reference/rust_compiler_completion.md) requires
eight passing gates at one clean commit and the four matching-host runs.
The tasks below own evidence closure; capability repairs stay with their
implementation owners. Historical records are attribution leads, not current
baseline results. An unavailable runner, unexplained skip, filtered/empty run,
timeout or exhausted capacity leaves the affected gate open.
Retain applicable checked/cross-target coverage when a runner is unavailable,
but report the missing runtime leg explicitly; it does not close that host row.

- **RC-RELEASE-RECORD-AND-CLOSURE.** Complete the existing release-evidence
  route and assemble all eight gates and four matching-host rows at one clean
  commit. Owners: `tools/release/release_record.py`,
  `tools/release_matrix.py`, their tests and the
  [completion contract](wiki/drafts/reference/rust_compiler_completion.md#release-matrix).
  Both runners exist; they emit different record formats and neither joins
  independent host records into complete same-commit closure. Reconcile their
  evidence rather than introduce another recorder. The committed macOS records
  remain open; their native observation names the deleted `canary_suite`
  target. Historical records are not current release evidence.

  Finish validation before trusting a passing label. `validate_record`
  currently accepts a gate marked pass with a nonzero command exit, a nonzero
  skip count without named skips, and an empty `closure.open_rows` despite
  open platform rows. Neither runner proves that an exit-zero test command
  completed a nonempty, unfiltered run; inherited environment filters also
  need accounting. Derive gate/closure status from complete command evidence,
  reconcile named expected skips with observed counts, and reject omitted,
  stale, filtered, empty, interrupted or contradictory results. Preserve
  bidirectional command-manifest coverage, the portable formatting route and
  explicit native-execution evidence. A valid open record may be retained;
  it must not count as release closure.

  Acceptance: produce and revalidate the same-commit collection with pinned
  toolchain, host OS/architecture, exact commands/arguments, outcomes, elapsed
  time and expected skips; include emulator/version where admitted. Close only
  when the full contract passes, with actual execution on each required host.
  The current RC-PORTABLE-PSI commands run library tests in one process; restore
  the required separate-process reload/tamper witness through a maintained
  harness. RC-DIAGNOSTICS covers the full negative corpus. Sample refresh
  builds artifacts; the native rows and **SAMPLE-CORPUS** still owe execution
  and their behavioral oracles. Preserve release records and maintained
  regression gates with their owners before retiring the temporary completion
  plan. Rust remains a comparator; bootstrap work stays on its own board.

- **RC-REPOSITORY-CLOSURE.** Establish one clean-commit pass of all four
  repository commands in the
  [completion contract](wiki/drafts/reference/rust_compiler_completion.md#release-matrix):
  portable formatting, whole-workspace all-target Clippy with warnings denied,
  all-target check, and workspace library nextest with no fail-fast.
  Owners: repository gates and each failing crate. Use the full scope without
  crate exclusions, attribute current failures to their capability owners, and
  retain the complete result through **RC-RELEASE-RECORD-AND-CLOSURE**.
  [Dated failure attribution](wiki/drafts/measurements/known_baseline_failures.md)
  is a starting lead, not a current baseline. Acceptance: all four commands
  pass at the release commit with exact failure/skip attribution and complete
  test-run evidence; do not combine results from different revisions or hosts.

- **RC-BUILD-AND-PACKAGES.** Close both build/package commands in the
  [completion contract](wiki/drafts/reference/rust_compiler_completion.md#release-matrix):
  nextest and doctests for `build-declarations`, `build-evaluation`,
  `package-compilation`, `package-source`, `resolver-execution`,
  `package-evidence` and `package-manager`. Owners: build evaluation,
  package resolution/review/manager and compiler handoff. The old six compiler
  integration targets are deleted and are not release commands. Preserve their
  distinct source/package customers in maintained harnesses; passing the two
  package commands alone does not establish compiler integration.

  Acceptance: both complete commands pass at the release commit and their
  evidence validates under **RC-RELEASE-RECORD-AND-CLOSURE**. Reproduce current
  failures before assigning repairs; do not suppress review requirements or
  stale-service diagnostics. The named-workspace fixture now selects children
  from each caller's module path and asserts one test passed; its former
  shared-module selection failure is repaired.

  Retain these integration requirements with the existing capability owners:

  - **Package and selection identity:** carry exact checked-call, review,
    package-use and accepted provider-schema identity through compiler handoff.
    Keep the free process-exit helper without a synthetic attachment, the
    ordinary accepted UEFI schema and distinct instantiated package-use
    authorities as source/package customers. Preliminary selection finalization
    still permits unresolved toolchain-origin selections in
    `04_typed-trees-to-checked-trees/src/authored_selections/finalization.rs`
    (with separate target-sibling handling). Recover the package-owned std,
    targetless UEFI-handoff witness: dependency late binding must behave
    consistently with bundled std while unresolved root calls still reject.
    Do not widen the allowance to every `CheckedCall` or infer ownership from
    a path. **MODULE-NAMESPACE-RESOLUTION** owns exact package/declaration
    identity; **PROVIDER-SELECTION-AFTER-TERMINAL** owns provider placement,
    and **X86-FMA-PROVIDER-TRANSPORT** owns FMA transport.

  - **Private progress premises:** under
    [termination](wiki/spec/language/termination.md), **TPR6** owns finite
    sets of exact externally dischargeable subjects, including eligible premises
    not declared by the private caller. Recover source witnesses
    for two independent WeakFair premises, a private wrapper around a cycle,
    and recursively growing subject projections that must not become a finite
    promise. Follow `checks/contracts/calls.rs` and
    `checks/termination/progress/components.rs`; the latter still maps an
    unavailable derivation to `NoGuarantee`, but the deleted test failures
    need fresh reproduction. Preserve exact entry-rooted lineage through
    nested wrappers and stable component convergence. Private visibility alone
    cannot waive a ProgressProfile requirement: retain reference-operand
    mutation, later loop-arrival and same-named-formal negative controls.
    Coverage discharges the retained premise at the specified public,
    receipt or build-bound provider boundary.

  - **Demanded imports and callback custody:** provider planning and
    `omega-rust/omega/compiler/src/native/retained_native_product.rs` must
    refuse an actually called import without supplied execution/stack custody,
    including retained native reentry that returns the exact dynamic
    interpreter on refusal. The guard exists, but
    `exact_demanded_import_plans` omits requirements with no matching import
    row, so recover a source witness that follows demand through planning and
    settlement. Preserve a supplied-custody positive that authenticates the
    exact imported image. **CALLBACK-PRIVATE-MATERIALIZATION** owns callback
    calling/fragment custody; the explicit `callback_terminal_custody` target
    survives and includes native-image replay checks. Image validation alone
    does not prove execution of the callback.

  - **Foreign-domain mutable recasts:** **MODULE-NAMESPACE-RESOLUTION** /
    **RECAST-SOURCE-POSITIONS** must preserve a qualification when declaration
    and mutable view name the same exact foreign package domain, and reject
    exchange between different packages' same-named domains. The maintained
    `tests/omega/pass/recast/runtime_mutable_equivalent_domain_recast_exit`
    is only a same-file control; recover the separate-package customer before
    treating the old routing diagnosis as current. Keep routed establishment,
    exact selected identity and mutable write-through; spelling equality
    cannot grant the domain.

  - **Borrowed suspension and terminal arms:** **TR3-TR8**, checked loan/carry
    analysis and Terminal emission/verification own
    [borrow carry](wiki/spec/resources/carry.md#borrows-across-suspension) and
    [terminal calls](wiki/spec/language/state_contracts.md#terminal-calls-in-transition-arms).
    Restore a maintained source witness after deletion of the service test.
    A machine-target arm uses ordinary call/completion with exact
    `suspend`/`block` markers; a local-state jump is marker-free. Retain
    caller-local backing and shared, mutable, write-only, aggregate-carried and
    service-receiver loans, with exact parents, through nested parked calls.
    Borrowed/service types neither categorically reject nor waive evidence.
    Statement, terminal-expression and terminal-arm calls must agree on
    values/Unit, mutation, postconditions, crash/no-return behavior, cleanup
    and suspension in checked execution, reloaded verified Terminal and native
    execution. Cover repeated park/resume without repeated effects or cleanup,
    cancellation outcomes, and completion after resumption. Reject early parent
    access/backing release, missing carry/affinity, forbidden live resources,
    bad return contracts, underreported frontiers and missing/redundant
    markers. Preserve tail-recursion/WCSU rules; helper-only plans do not
    establish a working suspension runtime.

  - **Host-provider execution:** **SAMPLE-CORPUS** / **CANARY-CORPUS** own
    application and fixture outcomes; provider settlement and native
    realization own exact selected providers. Current
    `build-evaluation/src/provider_settlement/canonical_filesystem_host.rs`
    and `canonical_time_host.rs` supply limited Linux x86-64 syscall rows;
    they do not establish Darwin coverage or every host boundary. Darwin
    imports/adapters must retain locator receipts, argument choreography
    (including clock identifiers/selectors), result transport and time
    constants. `ProviderBinding::Import` exists and its receipt must commit
    to the exact locator; a syscall table cannot invent import authority.
    Use the ordinary authorized binding route. Current Objective-C fixtures
    already declare public domains and their reached services; remove those
    repaired visibility/reach diagnoses. Prove their selected-provider native
    outcomes, including GUI and multi-library calls, rather than resurrect
    the deleted filesystem test census or treat a missing host as a pass.

  Plan-laid repeated runtime, dynamic subslice endpoints and ranked-cycle
  evidence remain with **PLAN-LAID-VIEWS**, **TERMINAL-SLICE-VIEW-VOCABULARY** and
  **TERMINATION-RANKING-CHECKS** respectively; optimizer integration stays on
  `TASKS_OPTIMIZER.md`. Their deleted test targets are leads to recover
  behavior, not extra package commands or passing evidence.

- **RC-PCC-REPLAY.** Close the release gate for artifact/`.proof` pairs:
  round-trip valid evidence, reject hostile/substituted evidence before
  PCC-required interpretation or lowering, and keep ordinary non-PCC output
  independently checked. Run nextest and doctests for
  `checked-trees-to-lowered-psi`, `terminal-codec`, `terminal-verifier`,
  `terminal-interpreter` and `terminal-psi-to-abstract-operations` as specified
  by the completion contract. The block is red and includes a terminated
  proof-search case; a harness returning after killing a test is not
  successful termination of that test.
  C2L-PROOF-SEARCH-BLOWUP-CONTAINMENT owns that resource defect; package tests
  and capability owners repair their remaining attributed failures.
  Acceptance: complete successful commands at the release commit, including
  source-free hostile controls. Native behavioral correspondence stays with
  PCC-PRODUCT-PUBLICATION and PCC-CANONICAL-SEMANTIC-LEDGER; placed-image digest
  checks alone do not close it.

For each native host task below, run all of RC-NATIVE-MATRIX
(`python tools/corpus_gate.py --native`),
RC-SOURCE-SEMANTICS (`mbx nextest run -p compiler --all-targets --no-fail-fast`)
and RC-REPRESENTATIVE-PROGRAMS
(`mbx nextest run -p compiler --test samples_compile --no-fail-fast`).
Unset corpus/canary/sample filters. Verify emitted programs' required
exit/output observations and publish the host record at the agreed release
commit.
Cross-target emission does not substitute for execution. These are independently
deliverable host runs, not four implementations of the gate.

RC-SOURCE-SEMANTICS includes compiler integration targets outside `canary_suite`.
Use bounded batches during repair when linking all targets exceeds host capacity;
release acceptance still requires complete coverage. Remaining reported failures:

- `no_selection_golden::native_artifacts::retained_native_bytes_and_metadata_match_every_target_golden`:
  identify the semantic/proof-section drift before updating goldens; verify
  retained bytes, metadata and repeatability on every hosted target.
- `module_machine_indices::indexed_domains::qualified_record_projection_rejects_unproven_constructor_membership`:
  distinguish disproved membership from failure to prove it.
- `module_machine_indices::indexed_domains::separate_packages_cannot_exchange_mutable_domain_qualifications`:
  restore a valid initializer so the negative reaches mutable-recast checking.
- `module_machine_indices::rational::anonymous_comparisons_cannot_hide_undefined_values_or_landed_operands`:
  reconcile finite float comparison acceptance with the numeric contract while
  preserving undefined-value and landed-operand rejection.

- **RC-NATIVE-MATRIX-LINUX-X86-64.** Execute the complete native, source and
  sample gates above on Linux x86-64 and resolve remaining failures through
  their owning capability tasks.

  The `DT_RELAENT` defect that blocked emitted execution is repaired:
  `ElfDynamicTag::GeneralRelocationEntrySize` is tag 9 and
  `GeneralRelocationEntryByteCount` is emitted beside the `DT_RELA`/`DT_RELASZ`
  pair in
  `omega-rust/omega/backend/images/image-elf/src/dynamic_executable/dynamic_table/dynamic_tags.rs`. The image
  has not been run since, because verification needs a Linux host.

  **SAMPLE-CORPUS** owns current shared sample failures;
  **TWO-AXIS-TERMINAL-AUTHORITY-REVIEW** owns the canonical filesystem plan's
  package-review replay gap. Attribute failures against current code rather
  than treating a historical host record as a fresh reproduction.
  Acceptance: actual emitted ELF execution and all required full commands
  pass, not merely a selected ABI cohort.

- **RC-NATIVE-MATRIX-LINUX-ARM64.** Supply the same complete gate evidence
  for emitted AArch64 ELF programs. A Linux AArch64 runner or named/versioned
  emulation may satisfy the contract, but must execute the target runtime
  legs: an x86-64-built harness that cfg-excludes them does not.
  Cross-target checks plus a single `cli_mvp` QEMU execution do not close
  the full row. Acceptance: required native/source/sample gates pass with
  actual AArch64 execution and explicit, justified skips.
  Include Linux host, time/filesystem and `IntegerAt` projection/write paths;
  checked-target fixtures and cross-emission do not establish runtime behavior.

- **RC-NATIVE-MATRIX-MACOS-ARM64.** Run the complete native/source/sample
  gates on macOS AArch64, executing emitted Mach-O products. Procedure and
  cross-target coverage do not establish a completed matching-host run. Use
  the recorder procedure in `tools/release/README.md`; it takes no `--all`.
  Acceptance: same-commit full-gate results with native observations and exact
  expected skips; Linux emission evidence cannot close this task.

  The corpus native leg NOW HAS a checked-in macOS ARM64 golden, recorded
  2026-09-27 at `a9478e720fc`: `tests/omega/corpus_native_macos_arm64.txt`,
  724 fixtures. The file existed before that but held only its header, so the
  leg had zero recorded rows and nothing could detect movement in it.
  **724 fixtures: 287 checked, 187 built, 250 rejected; 137 EXECUTED, of which
  105 exit 70**, 0 with stdout differing from expected. The 250 rejections
  are ATTRIBUTED in
  [macos_arm64_native_rejections](wiki/drafts/measurements/macos_arm64_native_rejections.md),
  and the headline reframes this item: **144 of them are unit-plan omissions
  that never reach code generation**, plus 19 host provider-absence and a
  dozen authoring rows. The genuinely native-side population is about **50**
  -- Terminal production 34, canonical abstract optimization 6, authority
  closure 6, artifact and target lowering 6. Size this matrix against 50, not
  250. The largest omission phase (27 at `statement sequence: local data:
  structural call binding`) is dominated by `recast/` and `slices/` rows, and
  the recast ones are confirmed REFERENCE-LOCAL-BINDING-ROUTES customers --
  `runtime_bool_representation_recast_exit` spells
  `let flag_bits: &u8 = &self.flag as &u8;`, two of that item's three
  triggers on one line. Seven phases carrying 39 rows have no filed owner and
  are ranked there as the places new diagnosis pays. Six take `signal:5`
  and all six are the `*trapping*`/`*_traps*` fixtures, where the trap IS the
  asserted behavior (`runtime_trapping_overflow_traps` says so outright) --
  read them as passes, not crashes. The remaining executed rows exit their own
  expected codes. That roster is the RC-NATIVE-MATRIX-MACOS-ARM64 measurement
  leg; what is still missing is the lane row, below.

  Host prerequisite, measured on this host and corrected in
  `tools/release/README.md`: no Xcode is required. The README told the reader
  to point `SDKROOT` at `$(xcode-select -p)/Platforms/.../MacOSX.sdk`, a path
  that does not exist on a CommandLineTools-only host. Only the `MacOSX27.0.sdk`
  stub needs avoiding; where the default resolves to `MacOSX15.5.sdk`, the full
  native leg emits and executes Mach-O with `SDKROOT` UNSET under the pinned
  `nightly-2026-09-04`.

  Two `macos_arm64` records DO now exist under `tools/release/records/`, written
  by another session on 2026-09-27 at `0293d4cad9` (7 of 8 gates fail in the
  later one; RC-DIAGNOSTICS passes). The row is still open for the reason the
  next paragraph gives: every `platform_runs` entry, `macos_arm64` included,
  carries `native_execution_exit: None`.

  NOT CLAIMED, deliberately. The lane reads `recorded` when a
  `--native-execution` command exits 0, and `python3 tools/corpus_gate.py
  --native` now exits 0 against the golden just recorded -- but that golden
  PINS the 250 rejections, because the corpus golden is a movement detector
  that "pins the red as well as the green". Passing it as this lane's
  native-execution observation would certify Mach-O correctness on evidence
  that asserts no such thing, which is exactly what this item warns against.
  Closing the lane needs an observation whose success means the programs ran
  correctly, not that they ran the same as last time.
  `-p compiler --all-targets` (RC-SOURCE-SEMANTICS) at `04d2099ae1`: 3281 run,
  2014 passed, 1267 failed, 0 skipped (22583.2 s) — the `samples_compile` leg inside it
  (RC-REPRESENTATIVE-PROGRAMS) read 33 run, 11 passed, 22 failed. The full
  1215-name failure list and dominant diagnostic classes are in
  [macos_arm64_rc_gates](wiki/drafts/measurements/macos_arm64_rc_gates.md):
  ProgramEntry/Terminal-attachment rejoins, `runtime policy realization`
  lowering gaps (ARITHMETIC-POLICY-REALIZATION), and domain-admission gaps
  (CANONICALIZE-SCALAR-RANGE-CONTRACTS). The recorder writes a lane only with
  a passing `--native-execution` observation, so the row stays open on this
  evidence even though two `macos_arm64` records now exist.

  Use [known baseline failures](wiki/drafts/measurements/known_baseline_failures.md)
  for prior failing names and expected skips, then reproduce at the release
  revision. Partial native/canary runs do not establish complete source/sample
  coverage or general Mach-O correctness. Route missing Unit plans to
  **STATE-LOCAL-VALUE-FRONTIER** while retaining independent host validation.
  Compare `const_fold_unsigned_shift_right_arg` across macOS and Linux when
  attributing the recorded cross-host exit difference.

- **RC-NATIVE-MATRIX-WINDOWS-X64.** Run the complete native/source/sample
  gates on Windows x86-64, executing emitted PE products, including the
  hosted receiver. Procedure evidence is not an executed release row. Use
  the recorder procedure in `tools/release/README.md`.
  Acceptance: same-commit full-gate results with native observations and exact
  expected skips; cross-emission or compiler-only success is insufficient.
  Include `windows_set_file_time_exit_canary_runs`: SetFileTime followed by
  `_stat64` must read back Unix time 1,500,000,000 and exit 70. Its corrected
  unsigned byte assembly is not evidence that Windows execution passed.

- **MACOS-X64-HOST-PROFILE.** Execute the compiler/runtime route on an Intel
  macOS host. Target/provider recognition, exact physical-entry reconstruction,
  accepted package binding, hosted receiver and x86-64 Mach-O import pairing
  have implementations; cross-target checks do not establish host execution.
  Exercise emitted Mach-O entry/exit, receiver and provider/import behavior,
  recording exact commands, commit, observations and justified skips. Attribute
  failures to their capability owners. This additional platform task is separate
  from the completion contract's four required release hosts and does not depend
  on an Alpha bootstrap seed.

  Reproduce the [host record](wiki/drafts/measurements/macos_x64_host_profile.md)'s
  two admission gaps: unselected `ConsoleNativeProvider::exit_process` and an
  unbound `macos_x86_64::ProgramEntry` root. Complete provider settlement and
  root binding before claiming emitted entry/exit, receiver, or import execution.

## Omega-written compiler (after Rust completion)

Finish the Rust [completion contract](wiki/drafts/reference/rust_compiler_completion.md)
before starting the product-language migration. Rust can remain a differential
implementation afterward, but neither Rust agreement nor Rust-specific machinery
is bootstrap authority. Bootstrap construction stays on `TASKS_BOOTSTRAP.md`.
- **OMEGA-PRODUCT-COMPILER-SOURCE.** After the Rust release-completion gate,
  complete the Omega-written production compiler as separate `source/psi/`
  and `source/omega/` packages, retaining `source/omega/{build.omg,main.omg}`
  as product roots. Psi owns target-neutral parsing, resolution, typing,
  checking, proof and Terminal Psi production; Omega consumes Terminal Psi for
  provider selection, realization, ABI and artifact emission. Package separation
  and the firewall are settled.

  The current root drives a lexer and partial whole-file parser, then exits;
  it produces no compiler artifact. Extend the connected product in ordinary,
  compositional vertical slices through the complete language and requested
  products. No bootstrap-private dialect, file/AST allowlist, source-shape-only
  lowering path or parallel source-to-native route. D in `bootstrap/5_omega/`
  is the separate Epsilon-written implementation; Rust is a nonauthoritative
  differential comparator.

  On macOS, both `omega --check source/omega/main.omg` and
  `omega --check source/psi/gates/parser/main.omg` stop at an unavailable
  `Parser::parse` callee: local construction reports
  `structural field store: pure source: scalar expression row`
  (state 3, statement 1). The reader is
  `04_typed-trees-to-checked-trees/src/execution/terminal_unit/structural_scalar_store/value.rs`;
  coordinate with its live claim before editing. Resolve that retained scalar
  expression through the existing store path, then rerun both product roots.
  One reported diagnostic does not establish that later stages are complete.

  The copied-argument regression remains in
  `tests/omega/pending/calls/copy_projection_from_borrowed_receiver` pending
  fixture promotion and its corpus-golden update; coordinate that shared file
  with its live owner. Preserve the authored ProgramEntry root and native exit
  check when moving it to `pass/calls/`. A targetless check alone does not
  exercise its original omission.

  Then freshly build/run `source/psi/test-parser.sh` with explicit
  `OMEGA_CLI` and `OMEGA_TARGET`.
  This is a parser golden-observation gate, not a two-compiler differential
  harness. Preserve acceptance/rejection, lexical handoff, structural identity,
  capacity and determinism controls; Python only decodes/compares observations.
  Cached artifacts and checked-source success do not establish native execution.
  Reproduce and attribute the previously reported package-name-shadowing,
  entry-custody and expensive-checking issues before assigning repairs; their
  old diagnostics and timings are not a current frontier. Route general
  value-transport/native gaps to **STATE-LOCAL-VALUE-FRONTIER** and existing
  borrowed-storage/crash gaps to **BORROWED-STORAGE-RESTORATION** and
  **CRASH-CONTRACT**. Do not infer invalid source from a lowering omission:
  the nested-receiver narrowing-cast fixture carries range evidence.

  Acceptance: the exact package-resolved source closure implements the complete
  language and production pipeline under the
  [compiler product contract](wiki/spec/build/compiler_request.md), passes the
  shared product suite and Rust/Omega differential behavior checks, and publishes
  a deterministic manifest of every transitive compiler/build input. Preserve
  requested-product/target and admission boundaries; partial parser acceptance
  is not compiler completion. C's ordinary Alpha obligation remains required;
  Rust Alpha emission is not a prerequisite. Bootstrap construction and
  `omega0 → omega` closure remain on `TASKS_BOOTSTRAP.md`.
