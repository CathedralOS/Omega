# Tasks

Unfinished product and language work for the Rust reference compiler. The
[completion plan](wiki/drafts/reference/rust_compiler_completion.md) defines acceptance;
this board identifies remaining work, not a passing baseline.

Implementation priority is a complete program's authored exit/output. Pick the
nearest failing program, repair its shared operation, and rerun the unchanged
customer through publication and execution. All accepted language scope remains
required; the order below guides implementation, not release exemptions.

| Priority | Work and acceptance |
| --- | --- |
| 1. Run existing customers and remove their structural blockers | [High-priority composition repairs](#high-priority-composition-repairs) and [Immediate product closure](#immediate-product-closure): use SAMPLE-CORPUS, CANARY-CORPUS and SQUALR-HEADLESS to choose a concrete failure. Repair the general operation, not another combination-specific route. |
| 2. Remove source and pipeline blockers | [Pipeline route](#pipeline-route): ordinary storage/call/transition repairs, then shared source/Build/Terminal convergence where needed. Independent customer repairs need not wait for the whole structural campaign. |
| 3. Connect the remaining accepted capabilities | [Entry and authority](#p1---authority-roots-and-entry), [materialization](#p2---materialization-and-placed-access), [service reach](#automatic-service-reach), [reflection](#semantic-reflection), [embedding](#embedding-and-interpreted-components), [tests](#requirement-based-tests), and [parallel language work](#parallel-language-and-compiler-lanes). Advance an item earlier when the chosen program needs it. |
| 4. Close the full product matrix | [Rust release closure](#rust-compiler-release-closure): all accepted features and required matching-host runs. Focused successes do not satisfy full gates. |
| Separate owners | [Optimizer](TASKS_OPTIMIZER.md), [bootstrap](TASKS_BOOTSTRAP.md), and [Cathedral](#p5---cathedral-over-general-omega-primitives) retain their full scope; native correctness dependencies move with the exercising program. |

Use the registered harness and its actual coverage: `tools/corpus_gate.py`
checks source, its `--native` leg realizes and executes eligible fixtures, and
`tools/sample_native_gate.py` checks documented sample exit/output on the host.
Check manifests before naming Rust test targets; preserve deleted tests' source
customers and their required outcomes. Run `tools/board_paths.py --board TASKS.md`
for citation consistency and audit the claim behind each moved path.
[AGENTS.md](AGENTS.md) owns workflow; live assignments belong in the
[claims registry](tools/claims.md), and semantic decisions in
[OWNER_QUESTIONS.md](OWNER_QUESTIONS.md).

## Ownership firewall

Follow the [connected pipeline](omega-rust/pipeline.md): Psi owns source semantics
through Terminal Psi; Omega owns target realization; backends own physical
details; Cathedral owns OS policy and structures. Complete ordinary operations
and transfers on the common graph, not another source-shape recognizer.

Compiler verification does not require an accepted package lock, and unfinished
native realization does not block source-package installation. Package acceptance,
build authority, and artifact verification remain separate obligations.

## High-priority composition repairs

These are ready-to-pick implementation tasks, not design proposals or work
reserved to the static-migration agent. Check live claims for actual assignments;
this board grants no directory fences. Evidence below is source inspection at
`89160904b7a6`, not a new execution result. First reproduce the named customer's
current boundary. These repairs follow the mandatory
[clean-as-you-go rule](omega-rust/pipeline.md#compositional-lowering): delete the
replaced special paths with the repair. A new helper around the same exception
matrix, a source rewrite avoiding the operation, or a passing isolated helper
does not close an item. Keep independent receiving verification.

For each item run its customer with `omega run --both <main.omg>` on a supported
matching host and the affected registered corpus checks. Record separately:
source checking, Terminal production/independent verification, interpreted
execution and native exit/output. A moved refusal is still unfinished work.

## Immediate product closure

- **STATIC-DERIVED-ACCESSOR-SUBJECT.** (split-of:STATIC-MACHINE-MIGRATION)
  The three `PlacedField` accessors in `source/library/core/layout.omg` cannot
  migrate by renaming, because the machine that satisfies the requirement is
  DERIVED, not the one written. 3 declarations, plus the two fixtures that call
  them.
  Measured 2026-10-02. `boundary machine PlacedField::read<T>(&self) -> T
  satisfies Readable<T>::read;` respelled as
  `(placed_field: &PlacedField<T>)` reports two things at once:
  "qualified call `write` has no resolved callable declaration" and
  "machine `PlacedField<RegPlacement,Registers,status>::read` state `read` does
  not satisfy trait `Readable` machine `read` parameter 0: expected `&self`, got
  `placed_field: &PlacedField<u32>`".
  So the conformance is checked against a derived specialization
  (`PlacedField<RegPlacement,Registers,status>`) whose attached data is not the
  authored template `PlacedField<T>`, which is why
  `parameter_declares_attached_data` refuses it even after 972d5894a30. And a
  call site cannot name the template either: `PlacedField::read(&view.status)`
  does not resolve, because the callable is the derived accessor rather than the
  written declaration. layout.omg says as much -- "The compiler derives
  conforming accessor types from a normalized placement; source cannot construct
  the opaque template or use it as placement evidence."
  Scope: the derivation has to produce the subject parameter on the accessor it
  derives, and the call spelling for a derived accessor has to exist, before the
  authored declaration can name its subject. Customers:
  `pass/layouts/placed_view_source_access_compile` and
  `fail/build/placed_view_program_entry_input`, which call these through
  `view.<field>.read()`.
  Acceptance: both fixtures keep their current outcome with the three accessors
  naming their subject.

- **STATIC-CALL-ARGUMENT-LENT-ROOT.** (split-of:STATIC-MACHINE-MIGRATION)
  The receiver population is 1637 declarations in 259 files, down from 3154.
  137 of those are trait requirements and their satisfiers, blocked on
  OWNER_QUESTIONS Q4 `trait-requirement-subject-binding` rather than on any
  compiler gap; 1448 are ordinary declarations.
  `source/library`'s remaining 271 are attributed: 221 belong to the
  `Filesystem` wrapper and its five target implementations, which no gate on
  this host can verify because every one of the 21 `pass/filesystem` fixtures
  is refused on main for one reason -- "selected ProgramEntry Binding field
  `Main::fs` requires a selected Fused provider for boundary `FilesystemHost`"
  -- and supplying a complete Darwin provider needs the
  `filesystem-host-platform-coverage` decision already recorded under
  SAMPLE-WORKFLOWS. Migrating that surface before then would land an
  unverifiable change to the most-used library in the repository. The other 50
  are 21 UEFI-target, 19 macOS GUI and 10 trait requirements.
  `tests/omega/packages` is at zero, `tests/omega/fail` has 85 left in 39 files
  (7 of them pinning the receiver rejection itself, which retires with the
  syntax), and what remains is 737 in corpus `pass/run`, 529 in samples and 271
  in `source/library`.
  Those are not tool-limited any more. The migration tool screens 102 of 149
  corpus candidates clean and the compiler refuses 87 of them, so the work left
  is compiler work with a measured shape. The dominant refusal is one call
  shape: a call that lends the data parameter and mentions it again. A receiver
  hid this, because the receiver was not an ordinary argument; written
  statically, `Main::verify(main, main.k)` evaluates `main` into a mutable loan
  and then reads `main.k` through it, which
  [expressions](wiki/spec/language/expressions.md#evaluation-schedule) orders
  left to right and [ownership](wiki/spec/language/ownership.md) refuses:
  "Exclusivity prevents reading old contents through a second alias." The
  refusal is correct; the authored program changes.
  Of 22 such call sites measured across the candidates, 9 pass `&mut
  main.field` beside `main`, 5 a bare field projection, 4 an indexed
  projection, 2 a shared projection and 2 a computed expression. The bare and
  indexed ones the tool now reads into a local before the call, which recovered
  one fixture. The `&mut` ones are a second exclusive loan and need the call
  restructured so only the projection is passed, or the callee to reach the
  field through the data it already holds.
  Dropping the projected argument instead, so the callee projects it from the
  data it already receives, was tried and measured WORSE: screen-clean
  candidates rose 96 to 102 and fixtures that previously checked stopped
  checking, 0 of 102 surviving against 2 before. Do not retry that without a
  per-callee reason; the parameter is positional and other arms supply it.
  A second measured finding: of 14 candidates that pass `--check`, 12 fail
  their native oracle. Checking is not the oracle for an `_exit` fixture, so a
  batch gated only on the checked leg will land programs that do not run.
  Acceptance: those fixtures build and meet their native oracles in the static
  spelling with no exclusivity exception added to checking; a genuine second
  exclusive loan still rejects, with a fixture naming that.

- **STATIC-DUNGEON-NATIVE-BUILD-COST.** (split-of:STATIC-MACHINE-MIGRATION)
  `samples/cli/games/dungeon_crawler_cli` holds 253 of the 458 receiver
  declarations left in the tree -- 55% of the migration -- and its rewrite is
  finished and sound. It is blocked on BUILD COST.
  The rewrite checks. `omega --check` on the migrated 18-file app completes in
  about 62 seconds with no diagnostics, which it could not do before
  fbbbc0253983 (projected arguments compose per argument). Three earlier walls
  cleared on the way: the default-domain obligation on a threaded subject
  (743851d348c), a transform that dropped the subject of a bodyless `satisfies`
  declaration, and a transform that rewrote `self.add_item(..)` -- a sibling
  MACHINE call -- into an unqualified call resolving to the same-named STATE.
  The diff is audited clean: no by-value receiver became a borrow, no
  boundary-trait requirement was touched, and the three signatures that lost
  their subject (`MazeBuilder::{blank_exit,empty_enemy,empty_reward}`) have
  bodies that never read the data and call sites that pass none.
  The cost is measured, not unknown. `tools/sample_native_gate.py --filter
  games/dungeon_crawler_cli --timeout 3000` TIMED OUT: the migrated app's
  native build exceeds 3000 seconds. Checking takes 62 of those seconds, so
  the remaining time is in lowering, optimization or codegen, not in Psi.
  That is why the migration is not landed. The app's baseline is a FAST
  failure (`lowering unsupported: borrowed argument changed its checked access
  or target custody`), and trading a prompt diagnostic for a fifty-minute
  build that may never terminate is not an improvement the sample rule -- a
  migration may not make a sample fail earlier -- can bless.
  Next step, and it is a PERFORMANCE question rather than a migration one:
  profile where the native build spends its time on this app. An 18-file
  application whose checking costs 62 seconds and whose lowering costs more
  than 3000 is a compiler cost defect that the migration exposes rather than
  causes; the receiver spelling never reached lowering here, so it never paid
  it. The rewrite itself is ready to land the moment that build terminates.

- **STATIC-MACHINE-MIGRATION.** (new-scope) Eliminate machine/state `self`
  receivers and implicit receiver calls under the
  [static-machine contract](wiki/spec/language/machines.md#static-declarations-and-explicit-state).
  This is an owner decision, not a feature proposal or optional compatibility mode.
  - Rewrite the remaining spec/guide examples, bundled libraries, product sources,
    canaries and samples to qualified static calls with explicit arguments. Keep
    ownership, mutation, termination and output oracles. Retired source syntax
    now rejects instead of keeping compatibility alive while customers migrate;
    a fixture stopped by that diagnostic is unfinished migration, not evidence
    that its underlying ownership or execution behavior was checked.
    Build declarations obey the same cut: use `Build::package(builder, name)`
    and `Build::depend(builder, source)`, not receiver-style calls. Migrate the
    remaining build files and package-manager fixtures; do not restore implicit
    builder insertion in their projector or generated prelude.
    `STATIC-DERIVED-ACCESSOR-SUBJECT` owns generated placed-accessor parameter
    substitution and the remaining accessor call sites.
  - Replace special receiver signatures, lookup, argument insertion, state carry
    and evidence flags with ordinary parameter identities through both producers,
    interpreters, Terminal verification and native lowering. Remove the retired
    internal receiver paths and evidence flags; parser receiver construction and
    implicit-self transition/tail-call routes are removed. Do not restore them
    to make an unmigrated sample or fixture pass.
    Do not delete unrelated receiver-side artifact admission or mathematical
    subject binding. Implement the explicit domain subject binder from
    [domains](wiki/spec/language/domains.md#declaration-and-membership):
    `domain Player::Alive(player) requires player.health > 0;`. Infer its carrier
    type, substitute the membership candidate, and keep it separate from generic
    indices and runtime parameters. Migrate domain predicates and reject implicit
    domain `self`; test generic families, lexical scope and binder-renaming identity.
  - Provision the exact ordinary mutable context parameter named by the
    [entry shape](wiki/spec/build/entry_roots.md#entry-shape-and-arrival-bridge),
    preserving ZII eligibility, selected bindings, root lineage, stack/storage
    partitioning and cleanup. Remove attachment/`is_self` as authority inputs.
    The checked interpreter selects host boundary operations by the call's
    resolved requirement alone, in statement and value position; its remaining
    receiver inputs are the `frame.self_cell` routes for the half-migrated
    `&mut self` customers (receiver-instance and sibling-state resolution,
    implicit self fields), which retire with those customers.
  - Move scratch fields to ordinary locals/local records and pass only genuinely
    live values across states. Repair local construction, loans and cleanup rather
    than moving scratch back into context. Temporary-scoping language design is
    deferred; it is not a prerequisite or permission to invent new scope rules.
  Acceptance: static source customers execute with their authored exit/output;
  migrated corpus/sample gates pass; no accepted machine/state receiver syntax,
  implicit receiver argument or receiver-specific signature authority remains.
  `heat_grid` now uses a static entry, local `Grid`, and explicit state arguments.
  Its Windows `omega --check samples/cli/collections/heat_grid/main.omg` passes
  after predicate-scoped call-proof admission replaces the all-arguments gate.
  `omega run --both` still stops at missing local package acceptance, including
  bundled `FilesystemHost` authority; approve that review before establishing
  native/interpreted exit 45. Checked success alone does not close this migration.
  `slice_maximum` and `value_call_in_expr` already use static entries and local
  buffers, but their Windows native runs still need the Console provider closure
  tracked under process exit. Their checked success is not full migration acceptance.
  The three `pass/borrows/borrow_{domain,guarded,returned}_window_write_and_call`
  fixtures use local `WindowBuffer` storage and static calls; all three execute
  with native and checked-interpreter exit 0 on Windows at `bcc4f637ab1c`.
  `pass/borrows/static_record_reference_return_exit`,
  `static_projected_local_reference_exit` and `transition_shared_borrow_result`
  exercise local owners with returned shared/mutable references, projected
  arguments and transition-tail returns. Windows native and checked interpretation
  produce their authored exits (0, 0 and 181); source-free Terminal execution
  retains the exact backing. Loan carriers release before their local owners
  are discarded. No receiver flag supplies the return origin.
  `pass/borrows/runtime_index_mut_loan_write_exit` now keeps its byte buffer
  local and passes it explicitly across calls and state edges; Windows native
  and checked interpretation both exit 0. Primitive byte-view access resolves
  the backing independently of descriptor transport and retains bounds evidence.
  On macOS ARM64 a static entry's local arrays and zeroed records now lend their
  homes natively: a whole local `[u8; N]`/`[T; N]` presents through `&[u8]`,
  `&mut [u8]` and `&[T]` views for Unit-, scalar- and structural-result calls,
  `.as_slice()` establishes a view over a local home, and a zeroed local record
  lends `&mut` whole (`pass/calls/local_fixed_byte_array_loan_exit`,
  `pass/slices/local_array_element_view_exit`); a local record lends `&mut` or
  `&` whole and `&mut` by fixed field path to Unit-, scalar- and
  plain-record-result calls, numeric field domains included
  (`pass/calls/local_{record_mutable,record_shared,projected_field,projected_field_scalar_result}_argument_exit`,
  `pass/calls/local_{record,projected_field}_record_result_argument_exit`,
  `pass/calls/local_records_mixed_record_result_arguments_exit`).
  `dutch_flag` (70), `modular_exponentiation` (87), `value_call_in_expr` (70)
  and `slice_maximum` (9) run with native/interpreter agreement. Direct scalar
  element reads from local arrays retain the original home after a borrowed
  writer (`pass/calls/local_array_direct_read_exit`, native exit 70 on macOS
  ARM64). A shared projection of a local (`&dash.engine`) lends into Unit and
  scalar-result callees (`pass/calls/local_projected_field_shared_argument_exit`).
  A static entry forwards its lent `context: &mut Main` bare into one helper
  after another, across projected stores and state edges
  (`pass/calls/static_context_forwarded_twice_exit`). A state's forwarded
  context lends its fixed-array fields as byte and element windows to Unit-,
  scalar- and structural-result calls, the Console reader included
  (`pass/calls/static_state_context_field_window_exit`). The fully static
  `vending_machine`, `stack_vm` and `account_ledger` exit **70** natively and
  interpreted on macOS ARM64. The five migrated counters — `sensor_min_max`,
  `print_number`, `array_sum`, `run_length_encode` and `caesar_cipher` — meet
  their documented oracles on macOS ARM64 under `tools/sample_native_gate.py`
  with their loop bounds inferred rather than authored, reading and storing
  domain-qualified byte fields through the forwarded context; the corpus pins
  that capability at
  [pass/storage/forwarded_context_byte_field_read_store_exit](tests/omega/pass/storage/forwarded_context_byte_field_read_store_exit/main.omg). The static `game_of_life`, with its canonical
  collection invariant, meets its native oracle;
  `pass/time/runtime_duration_core_exit` and
  `pass/calls/runtime_nested_value_call_guard_exit` exit **70** natively and
  interpreted while carrying local Duration records and a call's operand copy
  across state edges.
  A value-returning callee forwards a lent `&mut` record (a static context,
  or a local whole or by field) across its own states to scalar and record
  results (`pass/calls/state_graph_callee_forwards_mut_parameter_exit`,
  `pass/calls/local_record_forwarded_across_callee_states_exit`).
  A static
  `generic_counters` uses explicit generic calls and two invocation-local
  counters, preserving repeated writes and separate integer/Boolean values.
  On macOS ARM64 at `bbd0a9fdb8` plus namespace-call interpretation,
  `omega run --both samples/cli/basics/generic_counters/main.omg` exits **16**
  in both engines. Matching-host execution on the other declared targets
  remains open.
  A domain-qualified byte local built from a shorter literal (`let text: [u8;
  8] in Utf8 = "150";`) is a bounded owner with its own backing and live
  length ([encoding domains](wiki/spec/language/domains.md#byte-containers-and-encoding-domains)):
  it lends whole to static machines, whose `&[u8; N] in Utf8` loans forward
  across states and read the live length at any index, so `parse_int` exits
  150 natively on macOS ARM64 (`pass/text/local_bounded_text_{scalar,case_result}_loan_exit`).
  A case dispatch over a computed call subject evaluates the call once at its
  first arm, so `roman_numeral` exits 70 natively as authored
  (`pass/control_flow/runtime_computed_case_subject_single_evaluation_exit`).
  The state graph still declines such a dispatch when the subject's operands
  call or the subject is a boundary call.

Prioritize unchanged customer programs reaching native execution over additional
evidence carriers without an exercising program. The
[Rust compiler completion plan](wiki/drafts/reference/rust_compiler_completion.md) defines
the complete product bar; focused successes below do not establish that baseline.

- **REFERENCE-BEARING-RECORD-LOCAL-FILL.** (split-of:STATE-LOCAL-VALUE-FRONTIER)
  Root a field store at a declared record local whose fields include a
  borrowed view. `pass/text/runtime_string_stored_suffix_exit`,
  `pass/text/runtime_local_struct_string_field_concat_exit` and
  `pass/wire/runtime_wire_utf8_edge_verdicts_exit` remain native customers.
  The text reduction is three lines:
  `data Room [copy] { label: &[u8] in Utf8; }`, then `let room: Room;` and
  `room.label = source.label;`.

  On macOS ARM64 at `767b982de12` plus the evaluated-predicate repair,
  `omega run --both
  tests/omega/pass/wire/runtime_wire_utf8_edge_verdicts_exit/main.omg`
  gets through predicate lowering and rejects `LabelMessage::decode` at
  `structural field store: borrowed view field: declared domains`
  (state 32, statement 0). Evaluated predicates retain the exact descriptor or
  byte-field identity through state contracts and independent Terminal replay;
  overlapping writes and mutable calls invalidate current-content equations.
  `pass/proofs/runtime_byte_predicate_state_contract_exit` exits 70 in both
  engines for the euro sign and four malformed UTF-8 cases. This control does
  not supply the decoder's stored-loan lifetime relation.

  The text customers, and the unqualified wire byte decoder, also stop at
  `structural field store: borrowed view field: declared domains`:
  `structural_scalar_store/view_field_store.rs` admits literals only, not
  another field's view or an input-buffer view. General replacement needs the
  source's stored-loan provenance and lifetime through source-custody and
  Terminal reference verification; matching byte predicates alone is not enough.
  Preserve the literal-only boundary in
  [structural access](wiki/spec/terminal-psi/structural_access.md) until that
  relation exists. **DEFERRED-LOCAL-INITIALIZATION** owns partial storage;
  neither widening its zero-construction eligibility nor copying descriptor
  bytes supplies the missing reference establishment.
  Acceptance: the three fixtures reach their authored native exits, with the
  three-line reduction pinned as its own fixture.

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

  - **Computed calls and state transfers.** A structural call supplied directly
    as a state argument, such as `vending_machine`'s
    `report(context, Main::make_change(100, 55), 17)`, is an ordinary
    `EstablishStructuralValue` performed on the selected edge; the static
    sample exits **70** natively.

    Finish mixed scalar/structural operands, effectful arguments/returns,
    nested mutable-receiver calls, dependent/public-trait results and boundary
    consumers on that common route. Preserve
    `calls/inline_record_scalar_call_arguments_exit`: record construction and
    scalar operands execute once in authored order before scalar-result calls,
    including calls in machines with later states. A normalized foreign call's
    scalar result is an ordinary SSA home without an attachment, and a later
    state may call a top-level requirement that publishes a service ceiling; keep
    `pass/boundary/static_foreign_scalar_result_locals_exit` and
    `pass/boundary/later_state_requirement_scalar_result_exit` at native exit
    70. Selected case edges retain the invocation
    receiver and local ownership. Preserve source evaluation order even when
    the physical signature drops an unread reference `self`:
    `Random::next_u32(&mut self, state: &mut RandomState)` needs the exact
    authored-to-dense parameter mapping in planning, operation frames and stores.
    `machine_lowering/machine_dispatch.rs` still rejects competing scalar/Unit
    dynamic plans; `unit/structural_unit_control.rs` still refuses checked
    expressions in its specialized scalar-successor route. Remove those shape
    limits through ordinary argument evaluation, not another whole-body plan.

    Calls nested inside a record literal's tail fields (`Handle { name:
    self.get_name(), .. }`) still decline at `outer calls: ordered statement
    call` (`05_checked-trees-to-lowered-psi/src/tests/structural_float_member_read.rs`).

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
    and array literal elements that read storage. Case and record array
    literals replace one element window at a time
    (`pass/collections/case_array_literal_field_store_exit`); do not widen the
    shared bare-array classifier merely to route one store. The old request to
    construct relevant `UInt`/`Int` fields was invalid: these unbounded proof
    integers have no runtime representation, and checking now diagnoses them.
    Preserve erased proof fields and use represented carriers for runtime
    storage.

    Keep `collections/runtime_whole_array_value_copy_exit` as the qualified
    replacement control: `omega run --both` must preserve independent array
    contents and exit 70 in both engines. Its value identity retains arithmetic
    policy while storage uses the registered carrier; remaining qualification
    and custody work must not conflate those identities.

    Ordinary record-field replacement already uses `OpenWindows::replace`
    with move-out/store and exact displaced-value cleanup. Omega's
    `lowering/control_flow/borrowed_windows.rs` now resolves Record and Mixed
    parents, but still requires an owned activation home and rejects
    reference-bearing, linear, claimed or qualified replacement values.
    Complete those valid custody cases, including values arriving as owned
    parameters/block arguments, without double disposal or a fabricated move
    window. Verify empty-record locals both when forwarded and when discarded;
    a successful forwarding path does not prove edge cleanup. Empty record-array
    fields now use the ordinary checked catalog and zero-byte native layout;
    `collections/{empty_record_array_borrow_exit,nested_empty_record_array_borrow_exit}`
    preserve mutable caller writes with native/interpreter exit 0 on Linux x86-64.
    Unknown/cyclic element graphs, stored reference custody, byte-view substitution,
    empty indexing and literal arity remain independent rejection checks.
    `filesystem/windows_canonicalize_exit` needs its stored UnitResult. Coordinate
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
  On `ed218c8b1f5` plus ordered-tail reachability on macOS ARM64,
  `omega run tests/omega/pass/control_flow/ordered_unconditional_arm_exit/main.omg`
  exits **70**, preserving guards before the first unconditional arm and
  never evaluating later target operands. The unchanged
  `runtime_nonplace_record_pattern_single_evaluation_exit` now reaches
  `conditional successors: receiver transfer` (state 2, statement 0):
  `check_values(x, y)` carries no context but jumps to states requiring
  `&mut self`. Resolve its explicit context frontier with the
  `terminal_unit/state_graph/mod.rs` owner, then execute the original
  single-evaluation/value oracle; do not synthesize ambient context.
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
  body precedence and `CompetingCandidates` still choose among overlapping
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
  - **Multi-state body ownership.** Transfer multi-state bodies from scalar-owner
    precedence only with continuation-arm custody, later-state mutable scalar
    stores and crash predicates, float prefix initializers and debug maps
    accounted for.
    `CheckedStructuralControlSuccessorPlan` still lacks continuation identity.
    Keep result guarantees, including distinct exits returning 20 and 30 under
    `ensures result >= 20`; `source_selection` covers those distinct results and
    `composed_scalar_results` covers common exit facts. Preserve constant-stack
    mutual tail fusion and `calls/mutual_cycle_tail_admitted_exit`'s native exit 70.
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

  - **Results and control.** `calls/runtime_dispatch_float_terminal_exit` and
    `calls/runtime_dispatch_result_enum_case_exit` (score 15) exit 70 on macOS
    ARM64. `control_flow/runtime_tuple_transition_exit` now exits 22 natively
    and interpreted with explicit context parameters on every state edge;
    its old receiver-less intermediate state relied on retired implicit carry.
    Keep short-circuit evaluation and later-binding order from
    `unit_state_graph/scalars.rs`; the old prefix-initializer refusal has been
    replaced by ordinary conditional sequencing.
  - **Borrowed arguments.** `calls/runtime_param_receiver_second_instance_exit`
    (value 9 versus 7) and
    `calls/runtime_dispatch_{slice_element_terminal,machine_array_slice_arg}_exit`
    (slice element 7) exit 70 on macOS ARM64. **BORROW-PROOF-CONVERGENCE** owns
    compatibility evidence. Genuine conflicting parent/child loans must still
    reject; widening the verifier's pair check does not supply missing
    authority.

  The deleted customers are recovered with explicit context parameters:
  `calls/runtime_trailing_state_mut_param_phase_exit` (two mutations before the
  outer guard, three before the trailing one) exits 70 natively, and
  `control_flow/guarded_transition_dispatch_exit` exits 0 with `look` on stdout,
  both agreeing with the checked interpreter.
  `collections/record_array_field_access_exit` still needs exit 0 and its
  three authored stdout lines. On macOS ARM64, the opaque-region inventory
  repair based on `1d666f28d800` makes `omega run --both
  tests/omega/pass/collections/record_array_field_access_exit/main.omg`
  advance past write-frame agreement to `statement sequence: assignment:
  structural operand calls` in `result_code`. An unrelated opaque
  `Binding<Console>` sibling had poisoned the whole inventory; checking now
  retains its unknown region while preserving disjoint declared writes.
  The next producer dependency is stage 04
  `execution/terminal_unit/structural_scalar_store/mod.rs`:
  `record_literal_stores` still excludes indexed destinations and non-scalar
  case members of `Exit`. This file is assigned to
  **DEFERRED-LOCAL-INITIALIZATION**; coordinate the shared operation rather
  than adding an indexed-record special route. Recheck downstream borrowed
  windows and Omega projected arguments after that producer can emit the
  unchanged customer; older failure descriptions predate their shared
  projection work. Acceptance remains the public command's actual output,
  not the passing write-frame tests.

  Acceptance: every named customer has its actual entry and interpreter/native
  oracle accounted for. Use focused corpus filters and matching-host native
  execution; the default corpus can expose lowering failures, but a `checked`
  or `built` record without execution does not satisfy a runtime oracle.

- **BORROWED-STORAGE-RESTORATION.** (split-of:OMEGA-PRODUCT-COMPILER-SOURCE)
  Complete consuming-transform/replacement execution under
  [ownership windows](wiki/spec/language/ownership.md#borrowed-storage-invariant-windows)
  and [independent restoration replay](wiki/spec/terminal-psi/ownership.md#borrowed-storage-restoration).
  The executable floor is
  `omega run --both tests/omega/pass/ownership/runtime_borrowed_record_transform_exit/main.omg`:
  consuming `taken.bump()` and restoring its result exits 42 in native macOS
  ARM64 and the checked interpreter. Its increment explicitly wraps; unbounded
  exact addition still requires an overflow proof. The original-value control
  `ownership/move_keyword_field_assignment` executes with exit 41.

  `OpenWindows::replace` separates exact-place restoration debt from detached
  value custody: vacant fields need only a store; occupied fields displace and
  dispose their old content. Direct and named call results use ordinary owned
  result bindings, including consuming receivers, through independently checked
  Terminal and native lowering. Extend multi-block transport:
  composed-control emission currently requires closure after each operation
  sequence. Preserve evaluation order, disjoint siblings, exact open-place
  agreement and contained loans. Indexed paths already have support; retain
  whole-root, referent, scalar-field and nominal-cleanup restrictions until
  their evidence exists.

  Acceptance: consuming transforms, replacement values and repair on both
  branches execute with caller-visible updates and exact-once custody in
  Terminal interpretation and supported native targets, without staging caller
  storage. Preserve the two executable controls above. Missing repair, early
  return, stale reads, overlapping loans,
  wrong-place repair, repeated extraction, whole-owner cleanup and tampered
  artifacts reject. Cover recoverable failure, suspension/resume/cancellation
  and crash/process-exit abandonment without invented rollback or survivor
  guarantees. Preserve the restoration suites, ledger controls and
  `fail/ownership/borrowed_storage_boundary_call`.

  The product parser and **BUMP-ALLOCATOR-CANARY** consume this mechanism.
  Shared-reference copy admission is implemented. **CANARY-CORPUS** retains
  text/domain-provenance work; copies need no restoration window.

- **CLEANUP-HOOK-SELECTION-AND-ERASED-OWNERSHIP.** Complete ordinary
  consuming `drop<T>` under [nominal cleanup](wiki/spec/terminal-psi/ownership.md#nominal-cleanup).
  Generic selection already admits hooks reading/writing `self` and calling
  helpers. Preserve that route and reserved `T::drop` authored-call rejection.

  - Carry nominal cleanup through Omega ownership replay and native calls.
    With macOS ARM64 `ProgramEntry(Main::main)` bindings (e62553b906a),
    `drops/core_drop_owner_hook_body` stops at `CurrentCleanupMismatch` in
    canonical abstract optimization; `core_drop_owner_hook` reaches unsupported
    target control flow. `abstract-operations/optimization-unit/semantics/src/current_ownership/cleanup.rs`
    excludes runtime receivers/premises, and stage 02's
    `lowering/control_flow/terminator.rs::plain_home_cleanup` rejects
    `InvokeNominal`. Preserve receiver, contract, ordered actions, outgoing
    result homes and continuation through executable calls on return and
    successor edges. **CML4** owns residual partitions and mixed-root order.
  - Compose contextual hooks and dying local owners beyond the isolated
    scalar/Unit cases already encoded and independently verified by
    `nominal_affine_source::{scalar_returns,unit_cleanup}`.
    Stage 05's `unit/unit_cleanup.rs::patch_nominal_cleanup_member` excludes
    caller/hook prerequisites; stage 04's
    `execution/terminal_unit/control/checked_machine.rs` declines nominal
    locals at return. Preserve exact-place premises without inventing demands.
  - Finish erased-bearing construction: `core_drop_explicit_consume` with
    the same entry stops at "erased record member has no runtime initializer".
    Stage 05's Unit/scalar record emitters and codec
    `value_foundations.rs::validate_establish_record` require runtime fields
    for semantic declarations. Retain erased subjects, multiplicity, proof
    custody and provenance without storage or disposal.
    **PROOF-RELEVANCE-MIGRATION** owns erased formal/actual transport;
    owned dynamic descriptors still clean their hidden runtime payloads.

  Acceptance: bind entries for those three checked-only fixtures and execute
  encoded, independently verified Terminal and native artifacts. Observe
  nonempty hooks exactly once at early disposal and normal scope exit with
  contextual prerequisites; erased fields cause no runtime cleanup. Reject
  missing/forged hook identity or premises and duplicate consumption; retain
  helper-call and reserved-hook selection controls.

- **CML4.** Complete edge-local residual cleanup under the
  [ownership contract](wiki/spec/terminal-psi/ownership.md). Materialize outgoing
  values before transfer/disposal; give every occurrence one disposition.
  Clean dying roots in reverse declaration order and abandoned construction
  prefixes in [reverse establishment order](wiki/spec/language/ownership.md#construction-and-disposal-order).
  Crash/abort/process exit has no cleanup successor.

  Bounded parameter/local/result moves already execute through encoded
  Terminal. `partial_affine_result_source::anonymous_projected_operands_share_one_dying_continuation`
  covers multiple ordinary/boundary-produced temporaries with an empty Unit
  consumer; it establishes custody, not native payload or hook execution.

  - Transport the existing stage 04
    `execution/terminal_cleanup.rs::build_projected_edge_plan` through
    `execution/terminal_unit/state_graph/` and stage 05's attached Unit graph.
    `projected_transition_cleanup_retains_construction_local_residuals`
    already reconstructs local-before-parameter residuals, but composed
    `Pair.left` transition programs still lack admitted plans (parameter
    transfer; with a consumed local projection, projected operand support).
    Extend to partial construction, a second partial parameter and mixed
    dying roots. Reuse exact complements and anonymous continuations; keep
    machine parameter custody distinct from edge cleanup. No runtime liveness
    flags, untouched-array leaf expansion or deferral to final return.
  - Add residual evidence to conditional/structural-case successors and
    structural returns: `SuccessorEdge`, `StructuralCaseSuccessorEdge` and
    `ReturnStructural` only carry whole-root discards. `ReturnUnitPartialAffine`
    exists. Change producer, representation/encoding, independent verification,
    interpreter and native consumers together.
  - Generalize temporary cleanup beyond empty effect-free Unit consumers:
    other argument effects, non-Unit results, borrowed/owned mixtures and
    claims/qualifications use **STATE-LOCAL-VALUE-FRONTIER**'s ordinary
    evaluation path, retaining exact producers and dying continuations.
  - Complete native result homes, projected results/copies and computed values
    across calls/cycles. Stage 02's `lowering/unobserved_owned.rs::accepts`
    suppresses arrival homes using a whole-function gate; replace that
    dependency with per-place storage/action evidence. Existing no-code
    Jump/scalar/Unit-return cleanup admission is not a native execution witness.
    **CLEANUP-HOOK-SELECTION-AND-ERASED-OWNERSHIP** owns executable hooks.

  Acceptance: source-produced artifacts reload and independently reconstruct
  exact complements and complete transfer/cleanup partitions; native runs
  preserve values and hook order/timing across composed edges. Reject missing,
  duplicate, reordered, overlapping, wrong-root/type and post-transfer cleanup.
  Fuel exhaustion commits no disposal; retry cannot repeat one. Preserve
  `jump_edge_residual_discards_close_the_projected_argument_root_in_order` and
  `owned_successors_reject_same_arity_aliases_and_transfer_after_disposal` in
  `terminal-verifier`. No affine occurrence outlives its required edge.

- **ATOMIC-MEMORY-MODEL.** Complete atomic/fence semantics and native execution
  under [concurrency](wiki/spec/language/concurrency.md) and the
  [atomic vocabulary](omega-rust/psi/foundation/language-core/atomic_operations.md).
  `atomics/runtime_atomic_{fetch_add,compare_exchange}_exit` exit 70 natively on
  macOS ARM64. `samples/cli/systems/atomics_cross` also runs with a named entry
  and forwarded state storage, native/interpreter exit 70: events retain their
  exact local roots, and independent coherence follows whole-storage bindings
  while rejecting wrong backing, stale residency and intervening writes. Serial
  load/store/fetch/swap/decisive exchange already encode, independently verify
  and interpret as single Terminal events (`atomic_access_events.rs`).
  Omega stage 00 retains location/orderings and coherence edges
  (`atomic_events.rs`); its unique-reaching-write restriction is not a concurrent
  memory model.

  - Add target operations, selection/encoding and checked refinement for admitted
    widths, alignment and orderings. Swap/fetch return instruction-observed
    priors, never separate loads. Preserve decode/encode and round-trip laws,
    observing/non-observing and decisive/single-attempt behavior, failure
    ordering, `Uncommitted` custody and target retry-work attribution.
  - Complete source/Terminal event production and correspondence for shared
    atomic cells, bounded fields, scalar-result composition, fences and
    single-attempt exchange. `shared_receiver_atomic_store` fails checked
    planning because Terminal lacks atomic-cell identity;
    `atomic_global_order_operations` only checks without an executable entry.
    Abstract fence/single-attempt carriers exist but lack Terminal producers.
    A fence remains an event even when realization emits no instruction.
  - Complete independently checked `sequenced_before`, `reads_from`,
    `modification_order`, `synchronizes_with`, `happens_before` and
    `global_sequential_order`, qualifying fence synchronization and
    outcome-sensitive RMW obligations. Preserve existing serial coherence and
    optimization replay; resolve genuinely unsettled axioms through the spec
    owner. Model, relation-witness and target work can proceed before a scheduler.
    **TR3-TR8** gates actual concurrent publication, fence-pair, global-order
    and compare-exchange contention witnesses.

  Acceptance: source-free verification binds each relation, location, operation,
  ordering, outcome, custody and realization; missing/substituted evidence rejects.
  Preserve source-ordering controls and add matching-host instruction/execution
  and concurrent coverage. Serial tests and bounded exploration prove no complete
  memory model. Keep strong `Receive` unless an exact protocol proof permits
  weakening; device/DMA and interruption ordering retain separate contracts.

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
  `tools/sample_native_gate.py --cli <current-binary> --target <host> --jobs 1`
  completes local review, builds and checks documented exit/output; it accepts
  pending review decisions for measurement, not as evidence of a dependency audit.
  Samples without an oracle remain unscored, and freestanding samples need their
  own runner. Add the remaining exact stderr, input-consumption and timing oracles
  where documented. `omega refresh-samples` publishes but does not run them.
  Retain each checkout/target's own package review and complete host coverage.

  Prioritize shared operations that unblock full programs. Reproduce with the
  current CLI and the sample's package/entry inputs: a stale binary, an earlier
  dependency-review refusal, or an effect-free reduction does not exercise the
  same route. Follow the reported state/statement and unavailable callee into
  `04_typed-trees-to-checked-trees/src/execution/terminal_unit/`; a trace phase
  records the last route entered and can misidentify the declining operation.
  Keep the algorithm, storage and observable behavior when migrating syntax.

  At `bea3827921` on macOS ARM64, `sample_native_gate.py --jobs 2 --timeout
  1500` records 135 of 142 scored samples meeting their documented oracles
  (149 samples; 6 document no oracle and `uefi_hello` is not host-runnable).
  None built with a wrong exit or output, and no package review timed out.
  Since `b7b8d2a448` (131), `collections/matrix_multiply` (exit 189),
  `algorithms/horner_eval`, `arithmetic/dot_product` and
  `collections/array_max` joined them. The 7 that still build no native
  program are `games/dungeon_crawler_cli`, which now checks and stops in
  Terminal verification at `ControlCycleIneligible` (block 66), and the
  `systems/{file_journal,note_vault}` and
  `gui/{image_viewer,window_app,window_demo,windowed_calculator}` rows below.

  | Customer group | Capability owner and remaining acceptance |
  | --- | --- |
  | std `Clock` on Linux hosts (`rendering/{bouncing_ball_2d,mandelbrot_zoom,bouncing_particles}`) | At `59d57ace39f0` plus the shared local-storage loan repair, the unchanged `host/std_clock_sleep_exit` builds through final artifact admission for both Linux targets. ARM64 now constructs a checked 16-byte seconds/nanoseconds request and lends it to syscall 101; x64 retains its full-u32 poll adapter. Ordinary and foreign calls reuse completed storage custody, dominating homes, exact projection and ABI pointer transport; no Clock-specific compiler path or integer-address cast is involved. On macOS ARM64, `python3 tools/corpus_gate.py --native --filter providers/local_record_foreign_borrow_exit,providers/runtime_foreign_leaf_borrowed_byte_argument_exit,calls/local_record_leaf_loan_scalar_calls_exit,host/std_clock_sleep_exit` runs all four programs with exit 70. The new composition includes scalar computation, whole/projected local loans, repeated foreign calls and post-call reads. Scoped source/trust and native replay tests total 76; six local-loan negative fixtures retain their rejections. No Linux runtime result is claimed: Docker has no running daemon and no local Linux emulator is available. Acceptance remains actual 200 ms waiting and exit 70 on each Linux host, then complete rendering samples; `providers/kernel_import_result_chain_exit` must also execute with exit 70. |
  | `gui/{image_viewer,window_app,window_demo}` | At `0d3a0f15b72` plus the borrowed-reference boundary plans on macOS ARM64, `sample_native_gate.py --filter gui/window_demo,gui/window_app,gui/windowed_calculator --jobs 3 --timeout 1800` builds none of the three. Borrowed `&u8`/`&i32` requirements (`ObjectiveC::get_class`, `register_selector`, `send_string`, `send_byte_string`, `CoreGraphics::bitmap_context`) now have boundary plans, and a provider body's forwarded `&u8` loan and fixed element `&words.bytes[2]` reach libSystem `_strlen` natively (`pass/providers/runtime_foreign_leaf_borrowed_byte_argument_exit`, exit **70**). `window_demo` and `windowed_calculator` stop in stage 05 `structural_calls/result_uses.rs` ("Unit structural result argument has invalid claim-free custody") at `objc_get_class(&cbuf1[0])`, a shared loan of a local array's fixed element. Ordinary scalar callees now read through `&u8` formals and affine local records lend single leaves (`pass/calls/{scalar_callee_borrowed_primitive_reads_exit,local_record_leaf_loan_scalar_calls_exit}`, exit **70**), but trusted Terminal verification still refuses every element loan rooted at a local array, ordinary or boundary: `validation/operations.rs` admits an owned scalar-array payload source only with an empty path (`ScalarArrayResultMismatch`) and `argument_checks.rs` resolves no projected path on an `EstablishScalarArray` result (`UnknownStructuralArgument`); then `attached_unit/parameters.rs` ("invalid checked custody"); the by-value parameter element `&title[0]` next stops at "primitive array parameter requires exact whole owned transport", and Omega's normalized foreign call still admits only caller-parameter-rooted loans, not a local array's structural home. `window_app` composes `let padded: [u8; 11]; padded[0] = title[0];` (`pass/locals/runtime_local_array_member_stores_without_mut_exit`, exit **70**) and stops at stage 05's `call_catalog.rs` fence: "write-only stores and atomic events in opaque provider candidates require a pinned non-observation judgment". The providers' fields remain owner question `provider-private-state-home`: the 30 stateful satisfiers in `window_app`, `window_demo` and `windowed_calculator` need persistent installation storage before named-subject migration; per-call empty storage cannot preserve their field state. **BOUNDARY-PRIMITIVE-ARRAY-PARAMETER** retains the argument/result matrix. Acceptance remains the complete native GUI run with authored class/title signatures and exact provider selection. |
  | `systems/file_journal`, `systems/note_vault` | At `96b196739593` on macOS ARM64, `omega run samples/cli/systems/file_journal/main.omg` exits 200 during fresh package review: `Main::fs` and `Main::files::host` have no selected Fused `FilesystemHost` provider. The canonical table in `provider-planning/src/provider_settlement/canonical_filesystem_host.rs` covers only limited Linux x86-64 leaves. The raw boundary's mixed-platform full-schema contract needs the `filesystem-host-platform-coverage` decision in **OWNER_QUESTIONS.md** before a complete Darwin provider can be supplied honestly. Next acceptance is the real seven-step workflow, exit 7; no filesystem effects occurred in this run. Subsequent result custody, library callee composition and indexed nested-field initialization remain with **STATE-LOCAL-VALUE-FRONTIER**; `note_vault`'s `Main::main` now plans past `readback`, a closed-sum dispatch over `IoResult` whose unread `Error` case holds an `ErrorKind` payload (`pass/control_flow/case_dispatch_unread_structural_payload_exit`, exit **70**). It stops inside `Filesystem::create_dir_all` → `Filesystem::mkall_walk` state 0 at `state graph: state signature: parameter custody shape: qualified non-linear parameter`: the walker re-enters its entry state carrying `path: &[u8] in Path`, and stage 04 `forwards_qualified_loan` (`state_graph/mod.rs`) refuses every qualified loan into state 0. After that it needs the same `FilesystemHost` provider. |
  | Integer type/policy migration verification | **IMPLICIT-STORE-CONVERSION**: the remaining native customers are `windowed_calculator` on the GUI row's stage 05 local-array element loan (`objc_get_class(&cbuf1[0])`) and `dungeon_crawler_cli` on borrowed `Binding<Console>` transport (**ENTRY-CONTENT-ROOTS**; frontier in **NOMINAL-FIELD-FLOW**). |
  | Clock-bearing samples | Verify pacing and authored execution on the other declared hosts for `algorithms/{binary_search_viz,sort_visualizer}`, `rendering/{bouncing_console,ripple_field,tick_marquee}` and `simulation/{cellular_automaton,game_of_life_glider,langtons_ant,random_walk}`. Their empty named-subject providers need no persistent installation storage. The macOS ARM64 native output/exit and checked-interpreter exit oracles now pass for all nine, including the marquee's elapsed-time assertion. |
  | Text/index/match programs | **NOMINAL-FIELD-FLOW**, **WRITE-ONLY-BORROW**, **STATE-LOCAL-VALUE-FRONTIER**, **MATCH-SELECTIVE-LOWERING**, **CML4** and **OPERATOR-MACHINE-SUPPLY** own their missing operations/facts. Retain `Utf8` and ordinary domain evidence; recognized function names or weakened invariants cannot replace it. |

  Sample paths in the table are relative to `samples/cli` unless prefixed `gui/`;
  pending reproductions are under `tests/omega`. Repaired fixture-level cases
  such as the brightness-control dispatch chain still need their permanent
  pass guard and full sample outcome under **PASS-TIER-GUARDS-OWED**.

  Preserve these distinct product oracles alongside the shared capability work:

  | Customer | Required product behavior |
  | --- | --- |
  | `cli_mvp`, `euclid_gcd`, `generic_counters`, `number_guess` | Ordinary reviewed native execution across Windows x86-64 and both Linux hosts, retaining macOS ARM64 coverage; documented exact output and exits 0/12/16/70 respectively. For interactive input, keep EOF/Enter behavior, both lines and prompt timing, with empty stderr. |
  | `print_squares` | Preserve the macOS ARM64 nine-row/exit-0 result while completing Windows x86-64 and both Linux hosts. Run `omega run samples/cli/basics/print_squares/main.omg` through ordinary package review; require the computed rows `001,004,009,016,025,036,049,064,081` in order and EOF/Enter completion. The banner's `081` alone is not an oracle; an explicit `run --target` compiles without execution. |
  | `recursive_sum` | Complete the same hosted matrix through ordinary review; exact 144-byte output and exit 70. Preserve saved reads, untouched siblings and shared payload loans. |
  | `framed_payload` | Complete Windows x86-64 and both Linux matching-host runs, retaining the macOS ARM64 [exact-output oracle](samples/cli/systems/framed_payload/README.md). At `b0c5a2d9c78`, the unchanged shrinking-window `sum` runs after the shared machine-re-entry custody repair: the filtered sample native gate scores 1/1; `omega run --both --keep samples/cli/systems/framed_payload/main.omg` with EOF returns 60, prints exactly 165 stdout bytes, and reports interpreter exit 60. Direct execution of that retained image with EOF and Enter independently returns 60 with the same exact output and empty stderr. The CLI comparison took 81.85 seconds; execution alone took under 3 ms per input. Preserve the shared payload loan and state edge. Test-package acceptance is disposable measurement, not a dependency audit; other hosts remain unverified. |
  | `calendar` | Complete Windows x86-64 and both Linux matching-host runs, retaining the macOS ARM64 exact 277-byte/exit-0 oracle in [Calendar](samples/cli/simulation/calendar/README.md). Preserve trapping conversions, all five grid rows, the live 21-byte header, and EOF/Enter completion. Matching a banner containing `30` is insufficient. |
  | `longest_run`, `parse_int`, `roman_numeral`, `string_hash`, `substring_search` | Complete Windows x86-64 and both Linux matching-host runs, retaining macOS ARM64 exact outputs and computed exits 4/150/70/40/4. Use the [scanner](samples/cli/algorithms/longest_run/README.md) and [text algorithm](samples/cli/text/README.md) native commands/oracles, including EOF/Enter completion. Preserve authored cursor types, mixed-width length comparisons and bounded indexing. |
  | `cli/proofs/math_proofs` | Ordinary multiset data/slice extraction, selected laws and checked proof terms, with the false twin rejecting. Equal lengths do not prove equal contents; `core/seq.omg` is not a Bag implementation. |
  | Bounded Console input | Under [bounded input](wiki/spec/resources/bounded_input.md), preserve exact prefix/destination, once-only effects, cleanup, blocking/crash contracts, zero-capacity non-consumption, LF/EOF/Full, untouched tails, alias rejection, continuation, failure and invalid-result controls. Keep prefix guards until count-to-extent evidence exists; service receipt migration belongs to **ENTRY-CONTENT-ROOTS**. |

  Reproduce the reported long check for `gui/windowed_calculator` on a current
  binary and locate the cost (its `--check` takes 20 to 55 s under load); the
  historical stall did not establish a provider-selection or proof-search
  cause. Reuse unchanged compilation inputs through the check-once memo and
  persisted review cache, and measure the complete sweep. Non-tail self recursion in `arithmetic/stats_compute`
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
  `omega-rust/omega/compilation/driver/tests/corpus_runner.rs`; `canary_suite` was deleted.
  **SAMPLE-CORPUS** owns full sample integration; non-corpus/package gates remain
  in [release closure](#rust-compiler-release-closure).

  The check leg already realizes a pass/run fixture that declares an entry
  root, using its bound host target or first declared target. Reopening that
  harness repair would duplicate existing work. A rootless fixture still only
  checks declarations; `checked` is never proof of execution. Native execution
  requires a matching bound host and a run-tier fixture, `_exit` suffix or
  `expected_native_exit.txt`. Explicit exit/trap and authored stdout oracles
  are enforced before comparison or recording; missing execution cannot satisfy
  an output promise, and duplicate execution facts reject as malformed evidence.
  `input.txt` and `expected_stdout.txt` supply input/output evidence.
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
    `calls/local_records_mixed_record_result_arguments_exit` executes unchanged
    with native/interpreter exit 70 on Linux x86-64 at `9782e38b64a` after fresh
    local package review. Its committed foreign-checkout `omega.lock` prevented
    the ordinary product command before compilation; test locks and admissions
    must stay local. The fixture README records preparation and the outer run.
  - Preserve one portable check golden without hiding semantic differences.
    Target-name/fingerprint normalization already exists, but host-selected
    providers can still change the actual refusal. Attribute those changes
    rather than repinning one host over another. Keep missing/removed rows,
    wrong-reason negatives, crashes and timeouts visible.
  - Restore runtime promises lost with deleted per-feature tests as maintained
    fixture/harness oracles. Many named customers below have no input or stdout
    file. `filesystem/native_wrapper_write_all_result`
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
    by bare name in `provider-planning/src/program_entry.rs`, so
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
  | `text/runtime_local_struct_string_field_concat_exit`, `text/runtime_string_stored_suffix_exit` | Exits 188/193 with unchanged shared-view Room records. `StructuralByteViewFieldStore` ([store vocabulary](wiki/spec/terminal-psi/structural_access.md#store-vocabulary)) replaces a shared view field's descriptor with an established literal whose octets imply the field predicates; native literals are image-backed (`ByteLiteralImageAddress` into the image's read-only literal pool), so the store lowers in every activation. `render(self.source)` copies the `[copy]` Room across the state edge as one `StructuralLeafCopy` whose view keeps its source loan; owned block arrivals transport the shared view field as its two-word descriptor (`text/copy_record_view_field_state_transfer_exit` and `text/owned_arrival_view_descriptor_forwarded_exit` exit 70 natively and interpreted). Both customers now stop at stage 04 `structural field store: destination parameter` (state 1, statement 2): `room.label = source.label` stores a non-literal view into a state-local record ([Q3](OWNER_QUESTIONS.md) `stored-view-lifetime-authority`), and the concatenation then reads a view field operand. Psi still declines other view-record copies: a state-local copy (`let copied: Badge = main.badge`) stops at stage 04 `scalar field store sequence: write frame agreement`, a nested record holding a view record stops at `state graph: terminator: jump successor: parameter transfer`, an owned call operand at `call operation: projected operand support`, and a case with a view payload at `structural field store: case field type`. `traits/equatable_string_field_equality_exit` stores both record literals (a quoted view member is a `ByteLiteralView` that decomposes into its `StructuralByteViewFieldStore`; `domains/record_literal_view_member_store_exit` exits 70) and stops at `statement sequence: local data: scalar local: pure initializer` (state 0, statement 2): synthesized `self.left == self.right` needs content equality of two `&[u8] in Utf8` view fields, which `boolean_lowering.rs` unrolls only for bounded owners or a literal operand, and Terminal has no executable byte-sequence equality operation (`ByteSequenceEqual` is proposition tag 12 only). It needs a trusted Terminal executable view-equality operation, or a looped library comparison the synthesized equality calls. Copy permission cannot establish qualification or extend a loan. **NOMINAL-FIELD-FLOW** / **STATE-LOCAL-VALUE-FRONTIER**. |
  | `host/runtime_console_bounded_line_exit` | On macOS ARM64 the empty-input fixture exits 20 and its `host/runtime_console_bounded_line_{lf,full,partial_eof,middle_window}_exit` siblings exit 13, 34, 22 and 70 with exact stdout on both engines (LF stored and counted, Full without reading on, partial EOF, zero-capacity non-consumption, untouched `#` tails, next unread byte, and the `Main::repeat` middle-window entry). Remaining: the same oracles on the Windows and Linux matching hosts. |
  | `filesystem/native_wrapper_write_all_result` | On macOS ARM64, store and dispatch the write result: nonexistent parent gives Error, `/tmp` gives Ok, stdout contains PASS and no FAIL. Exit zero alone is insufficient. This is distinct from **FILESYSTEM-RELEASE-CONTRACT** proof closure. |
  | `host/runtime_gui_foreground_window_exit` | Windows, exact authored Gui provider: create, query/store foreground result, destroy, exit 70. An invisible window need not become foreground. |
  | `control_flow/runtime_{integer,string}_literal_dispatch_exit` | Exits 22 and 11 for `"look"`, with authored selection order and ordinary result/field storage. Both exit natively on macOS ARM64 with checked-interpreter agreement; retain integer dispatch as the control while completing the host matrix. **MATCH-SELECTIVE-LOWERING** / **STATE-LOCAL-VALUE-FRONTIER**. |
  | `wire/runtime_wire_exact_array_without_count_exit` | Generated `[u32; 4]` encoding without a synthetic count sibling: seven bytes `00 00 04 01 02 03 04`, exit 70, and checked-interpreter agreement. **WIRE-RUNTIME-AND-INSTALLATION** owns the generated operation; native execution remains part of this integration gate. |
  | `wire/runtime_wire_policy_authored_plan_exit` | On Linux x86-64 at `a885574d3d6` plus the shared policy-context repair, the unchanged authored plan and eight-byte round trip execute with native/interpreter exit 70 under `omega run --both tests/omega/pass/wire/runtime_wire_policy_authored_plan_exit/main.omg` after local package review. Build now supplies fresh, established context storage at the policy's exact declared subject position; layout, access and wire scheduling share that operation. The fixture README records preparation, and `expected_native_exit.txt` pins 70. Preserve policy/codec agreement, effect admission, zero-domain establishment and reference-field rejection. |

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

  Current candidate application `248a79e66025` has six unpublished source-lifetime
  repairs; its remote denies writes, so do not advance the application gitlink.
  On macOS ARM64, `omega update --project samples/apps/squalr/squalr-tests
  --target linux_x86_64` with compiler `b9498b04547` returned after 599.74 seconds:
  `ProgramStorageEntry::enter` had no evaluated calling plan. The dependency's
  calling vocabulary differed from the bundled source only by a binder rename;
  checking's `declarations/standard_declarations.rs` required byte equality.
  Recognition now uses the resolved nominal calling interface, with one route
  for toolchain and dependency sources. Policy execution, closed-result decoding
  and independent plan validation remain required. Nine interface tests and
  29 calling-plan tests pass, including invalid-result rejection without a
  retained plan. Four checked build fixtures and two native build fixtures
  retain their outcomes, including one authored native exit.

  Full review with `702aed0e8c1` was deliberately interrupted after 835.04 seconds
  without a verdict. Two profiles found repeated declaration-roster scans in
  crash-contract predicate traversal. `fb24797a5eb` reuses the existing indexed
  first-match owner query without changing predicate enumeration; six predicate
  tests and seven index tests pass. The identical application review completed
  on macOS ARM64 in 777.83 seconds with exit 3: ordinary pending capability
  decisions, not a compilation failure. Maximum resident memory was 9,549,742,080
  bytes. The missing calling-plan frontier is cleared; the interrupted baseline
  does not establish an end-to-end speedup. Review output is
  `squalr-tests/build/package-manager/review-linux_x86_64.txt` in the candidate
  application checkout. Accepted project files remain unchanged. Review the
  pending source-replacement and capability decisions before `omega update
  --project samples/apps/squalr/squalr-tests --target linux_x86_64 --resume`;
  do not manufacture approval. Native emission and execution remain unverified.

  Reconcile the unpublished carrier-custody and pointer-load repairs against
  current main before publishing them. Preserve the actual carrier home and
  explicit `Field(destination), Referent` geometry through source replay,
  custody checking and native address loading. Keep newer main's exact storage
  receipts, mutable-view backing and state-arrival joins. Do not remove the
  store from the customer or substitute inferred origin for its address.
  Escaped candidate windows remain live after their temporary formals disappear;
  a later arena write must still reject. The same package-review command and
  documented native application oracle are the acceptance route.

  The public macOS native command stops at missing exact-target package review;
  the existing Linux lock names a different checkout's local source identities.
  Preserve pins and request ordinary fresh package review, not a transplanted
  accepted lock or invented approval. Cross-target checking is not native
  execution. Keep the application command and its exact result oracles.

  The earlier result-store frontier, not reached by this current review,
  isolates stage 04's
  `control/call_results.rs` result-shape classification: `EnginePrivilegedState`
  nests task slots whose held payload carries `Binding<WaitWake>`. Empty slots
  need no fabricated binding, but transporting the aggregate must preserve
  routed authority and must not grant plain no-code disposal. The restored
  native reproduction still needs the routed-owned classifier and
  ordinary result consumers revalidated against the
  current Binding representation; their old `InvalidErasedStructuralField`
  diagnostic is not evidence of the current next failure. Check current claims
  on shared `checking/src/value_custody/storage_contents.rs` before editing it.

  Resolve the current borrow refusals, rerun the complete application, and
  revalidate owned-result transport if it becomes the first failure again. Complete
  the documented native acceptance on Linux x86-64 after emission succeeds;
  cross-target compilation on macOS does not establish execution.
  Live reads still alias captured-current
  values and mark failure-default reads present while attached; preserve the
  seven-result delete/list oracle and add distinct recent-read presence/value.

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
  Repeated checking reuses only exact inputs and authority receipts, keeps
  live runs to a verdict, and records timeout separately from source
  rejection.

- **VEC-NATIVE-GROWTH.** (split-of:BUMP-ALLOCATOR-CANARY) Complete ordinary
  library `Vec<T>` for Squalr's scan results under the
  [allocation contract](wiki/spec/resources/allocation.md). Owners:
  `source/library/core/vec.omg`, `source/library/alloc/vec.omg`, and their
  ordinary compiler operations. Core's generic construction/storage remains empty.

  Alloc now has a linear nongeneric handle, caller-owned `Backing4/8/16`,
  fallible Pool requests and fixed growth/drain machines. Preserve this control
  while binding the vector to actual owned backing: `Request::Granted(rung)` is
  publicly constructible, Pool supply/releases are numbers, and growth accepts
  caller-selected old/fresh stores without joining their identity to the handle
  or request. `held` flags and rung numbers do not establish an allocation claim.
  Growth copies each entire fixed rung, not an exact generic live-element extent.

  `runtime_vec_u32_growth_exit` now exercises requests, two growths, denial,
  drain and counter reconciliation with a bounded first-failure exit code.
  Its receiver-held handle never calls the consuming `Vec::close` requirement;
  drain/counter checks do not prove executable cleanup or exact extent return.
  The duplicate-cleanup and invalidated-loan fail fixtures now use the shipped
  library. Retain them and add wrong backing, forged/replayed grant, wrong rung,
  duplicate return and partially occupied growth controls.

  Squalr-side measured dependency (zergling census 2026-09-29): the
  1024-element scan-result window is inexpressible at the current pin — every
  `[T;N]` is compile-time sized (`Snapshot.regions` is `[SnapshotRegion;1]`),
  data structures cannot hold borrows, `alloc::vec`'s non-contiguous backing
  cannot yield `&[u64]`, and `core::Vec` waits on the `Allocation<T>`/lifetime
  model. A runtime-length owned array with contiguous slice projection is the
  required surface for the element-window port.

  Acceptance: native `Vec<u32>` appends through two allocations, preserves the
  exact live prefix and new elements, and cleans up once with exact backing
  return or explicit retention. Cover empty cleanup, real request failure with
  unchanged contents/custody, invalidated loans and duplicate cleanup. Complete
  generic element custody and storage indirection through the same source route,
  then use it in Squalr; no compiler-owned vector/Arena or recursive inline storage.
  **BUMP-ALLOCATOR-CANARY** owns allocation/returned extents; **PLAN-LAID-VIEWS**
  owns access/retirement; **CONSERVATION-CONTRACT / TERMINAL-CONTENT-CLAIMS** owns
  content transfer; **BORROWED-STORAGE-RESTORATION** owns needed move-out/replace.
  Preserve checking, source-free interpretation and matching-host native oracles.

- **MACOS-APPLICATION-PUBLICATION.** Publish and run `window_app`,
  `window_demo` and `windowed_calculator` under the
  [macOS application contract](wiki/spec/build/macos_application.md).
  Reuse `native-artifact/src/package_publication.rs` for package assembly and
  signing custody, and `products/src/compilation_products.rs` for checked
  package/inner-executable accessors.

  The samples now declare nominal GUI/clock/input providers, macOS
  `provider_defaults` selections and identifiers. Preserve those implementations,
  signatures, rendering and effects; missing explicit selections in `build.omg`
  are not evidence that these providers are absent. Complete any remaining
  general lowering through **ENTRY-CONTENT-ROOTS**, **TR3-TR8** and
  **STATE-LOCAL-VALUE-FRONTIER**, keeping one occurrence-owned provider receiver
  across calls and nested services. Fused authority is not receiver storage;
  do not substitute global state or weaken occurrence matching. Design-blocked:
  where a provider that owns resources across calls keeps its state in a fused
  build is owner question `provider-private-state-home` in OWNER_QUESTIONS.md;
  stage 05's installation adapter refuses every provider with fields until it
  is answered.

  Stage 05's `attached_unit/call_catalog.rs` still fences write-only stores
  and atomic events in opaque callable providers. Admit them only with a
  verifier-checked judgment preserving non-observation and exact access to
  places exposed by the requirement's mutable parameters. Existing scalar
  adapter/result-overload native fixtures exercise stateless providers, not
  these GUI receivers; reproduce each application's current first failure.

  All three apps pass `omega --check` after the `core::float_operations`
  import repair (d5eb3e41ae; sample gate 3/3 on macOS ARM64). Native
  production then stops inside `*GuiProvider::window_create`: `window_demo`
  and `windowed_calculator` at stage 05's shared loan of a local array element
  (`objc_get_class(&cbuf1[0])`, **SAMPLE-CORPUS** GUI row), `window_app` at
  the `call_catalog.rs` fence above; `provider-private-state-home` sits behind
  those. `python3 tools/sample_gate.py
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

- **TERMINATION-RANKING-CHECKS.** Carry exact rank/endpoint evidence through
  ordinary calls, named states and native execution under the
  [termination contract](wiki/spec/language/termination.md). Owners: stage 04's
  `checks/termination/ranking/`, validation's
  `proof_contracts/contract_entailment/ranking_range/` and
  `machine_calls/call_cycles/runtime_ranking/`.

  Quotient/remainder, left/right shift and bitwise-AND actuals already use
  `install_nonpolynomial_terms`; `rank_ranges/{call_components,field_endpoint_arithmetic}.rs`
  checks exact substitution, nested operations, shift widths, signed floor-shift
  and two's-complement mask bounds. The unchanged
  `control_flow/termination_index_distance_compile` customer executes natively
  with exit 0 on macOS ARM64: an unranked caller retains the ranked callee's
  ordinary call boundary. Source-free Terminal execution checks returned values
  4 and 7, including an already-crossed bound after a local computation; missing
  cycle evidence and changed result/source coordinates reject. Keep this control
  when extending endpoint transport. Ordinary callee requirements accept stable
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

  `float/float_to_int_exact_proofs_exit` now retains its former common-field
  `bounded: f64 [0.0..=100.0]` contract as `bounded: f64` with data
  `where bounded >= 0.0 && bounded <= 100.0`, not a replacement domain.
  The original stores, guarded conversion and state transfers execute with
  authored exit 78 natively and interpreted on macOS ARM64. Public `omega run`
  still stops at pending standard-library package review (exit 200); fixture
  package admission does not satisfy that external prerequisite.
  Its invalid-write control requires the field
  invariant to be restored even when the subsequent integer conversion fits.

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

  - Retain build-time admission and genuine negative behavior.
    Migrate `fail/wire/wire_policy_plan_disagrees`
    while preserving its actual plan/schema disagreement rejection; its
    recorded diagnostic already reaches that mismatch.

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

- **NOMINAL-FIELD-FLOW.** Close the Dungeon Crawler's field-domain execution
  acceptance under [ordinary mutable-argument contracts](wiki/spec/language/dependent_values.md#mutable-receivers-and-declared-field-domains)
  and [static declarations](wiki/spec/language/machines.md#static-declarations-and-explicit-state).
  Qualified machine names contribute no storage or arguments. Established
  mutable inputs owe their declared field/default domains at call admission,
  entry, return and fact handback, including domains that reject zero.

  Owner: `omega-rust/psi/compilation/pipeline/04_typed-trees-to-checked-trees`.
  `src/semantic/facts/field_domains.rs` now supplies shared parameter obligations;
  the separate machine-field origin and threaded-subject admission exemption
  are removed. Zero-validity remains an actual storage-establishment check.
  `domains/qualified_mutable_field_exit` and
  `domains/threaded_subject_keeps_established_domains` execute with native exit
  0 on macOS ARM64 on the `011a5aaf4e`-based candidate. The first observes nested
  NonEmpty fields before and after qualified and free-machine mutations; the
  second retains an established record across explicit state transitions.
  Nominal annotations cannot resurrect facts retired by mutation; unknown writes
  remain conservative, and an exclusive whole-record loan still conflicts with
  a live read-only projection even when its callee writes nothing.

  Transition argument transport already exists in `src/flow/state_values.rs` and
  `src/flow/state_values/fields.rs`: live literal, byte-predicate and integer-bound
  facts rebase onto named parameter storage and join across incoming edges.
  Incoming glyph bounds also reach `dungeon_render`'s `put(ch: u8)` path, which
  runs natively. A byte store preserves `Utf8`
  only with the necessary carrier/byte evidence: `ValidUtf8` alone does not
  imply `AsciiOnly`.

  Acceptance: paired qualified/free-machine cases over the same established
  `&mut T` agree for nested non-ZII domains, valid mutation and return through
  the shared path. Invalid arrivals, unclosed invariant windows and stale aliases
  reject at calls, transitions and returns. Extra input-only `requires` are not
  implicit postconditions; preserve explicit `ensures`. Verify equally initialized
  ASCII carriers and guarded stores through ordinary parameters and transition
  arguments, retaining the poisoned-edge/missing-fact controls in
  `tests/element_field_flow_probe.rs` and the join/mutation controls in
  `src/tests/contracts/state_field_values.rs`. Preserve indexing, view, copy and
  whole-extent controls in
  `src/tests/contracts/{element_fields,nominal_parameter_fields}.rs`;
  require exact finite candidate-origin coverage, never universal coverage from
  unresolved selectors.

  Remaining application dependency: the checked-in Dungeon Crawler still uses
  retired `self` syntax. **STATIC-DUNGEON-NATIVE-BUILD-COST** and
  **STATIC-MACHINE-MIGRATION** own its explicit-parameter migration. Earlier
  EOF/help native runs used the old language surface and do not establish
  acceptance after its retirement. On the migrated application,
  run closed stdin and `help`/`exit` through the commands and output oracle in
  [its README](samples/cli/games/dungeon_crawler_cli/README.md#runtime-acceptance).
  require fresh native exit/output evidence for both routes before closing this
  application row. Preserve the view-membership and
  returned-reference controls, including
  `control_flow/element_view_enum_equality_exit`,
  `borrows/indexed_record_field_replacement_exit`,
  `borrows/qualified_byte_owner_exclusive_call_exit`,
  `borrows/branch_selected_record_composed_receiver_exit`,
  `returned_record_case_field_store_exit` and
  `interleaved_parameter_field_replacement_exit`.

  Preserve exact initializer identity, nested consuming-call coordinates,
  finite reference-origin coverage, selected result addresses and loan release;
  statement coincidence or a same-typed sibling cannot substitute provenance.
  Keep the returned-record receiver/projected-call/byte-store controls,
  `structs/qualified_record_element_assignment_exit`,
  `text/nested_mutable_receiver_byte_result_exit`, copied nested-record
  predicates, and the local-loan/mutable-and-shared-argument negative controls.
  Guarded and closed-case returns must preserve physical view cleanup as well
  as semantic custody. The application acceptance remains native exit 0 with
  its authored room, prompt, command and goodbye output; independently checked
  helper plans are not native application execution evidence.
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
  matrix.

  The samples `collections/array_max`, `arithmetic/dot_product` and
  `algorithms/horner_eval` loop over a shared `&[T; N]` machine parameter.
  Stage 05 now reads it through its declared extent
  (`collections/runtime_borrowed_fixed_array_state_reads_exit` exits 70
  without a loop). In a loop, `unranked_cycles.rs` reports
  `ControlCycleIneligible { reason: Parameter { index: 0 } }`: its parameter
  roster admits shared plain records, borrowed byte views, element views and
  scalars, not a shared fixed array, whose frozen extent and contents serve
  every arrival the same way.

  - Replace remaining `unranked_cycles.rs` fences with checked arrival/custody
    relations: broader qualified/partial ownership, reference-bearing or
    claim-carrying structural call results, moving claims, content
    reshuffles/partitions, broader structural-result custody and boundary
    completion receipts. Plain case/record results, pinned entry claims,
    several effectful calls, unrestricted plain leaf copies, field-replacement
    window pairs, claim-free reference-free affine structural call results
    with call-scoped loans (outside the cycle, or discarded, moved into an
    owned successor parameter or reseated inside it), case dispatch over a
    block's own owned parameter already pass eligibility. Unit/scalar calls use
    ordinary argument validation and exact frontier replay independent of
    argument producers; their claim transfers remain fenced. Eligibility
    supplies no proof; verify
    every arrival before removing a fence. No cycle fence remains in
    squalr-tests' linux_x86_64 module. Its production stops at obligation
    9223372036854775820, the authored `requires ... position < name.len ...`
    arrival into `ProcessInfo::set_name`'s `store` state (machine 36, header
    465, edge 571). The guard's `name.len` read has no equation to the
    `ViewExtent` of the view the edge transfers, so no retained fact proves
    `position < name.len`. A standalone copy of that loop stops at
    `OperationProofUnavailable`. The same-view length join belongs to
    **TERMINAL-SLICE-VIEW-VOCABULARY**. The report names the
    `proof_search_result_guarantee_work` capacity only because that pass's
    module-wide 4096 budget always declines on this module.
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
    Named uses now join the exact emitted `BoundaryCall` through one
    source-occurrence relation: the named call's crash contract stays on its
    boundary declaration, whose published routes retention redecides against
    the checked site, and a missing, duplicated or non-boundary join fails
    closed. Borrow, flow, service reach, Terminal emission and source custody
    share the canonical operator identity. Structural telescopes and call
    carriers still need exact operand mapping, provider commitment and
    independent Psi/Omega replay; crash-qualified operator invocations reject
    before Terminal production at `validate_selected_operator_terminal_custody`,
    so no run fixture exists yet. Scalar IEEE equality/inequality guards
    exist; float ordering, generic and structural-formal routes still need
    portable predicates, preserving the original guard.
  - Finish invocation-outcome trace/refinement under
    [reconstructed observations](wiki/spec/terminal-psi/observations.md#reconstructed-rows):
    actual outcome, substituted guards, abandoned claims, staged writeback and
    no-result/no-cleanup behavior. Stage 05 `tests/scalar_boundary_arguments.rs`
    already verifies and interprets scalar boundary outcomes; its structural
    guard control rejects. Native call continuations exist, but retained crash
    rows alone do not establish executed outcomes.
  - Extend ordered completion beyond scalar contracts, closed integer ranges,
    mutable Boolean snapshots and Unit post-state field guarantees to remaining
    qualified results, state/control contracts and compound arithmetic
    guarantees. Ordinary and composed Unit bodies share guarantee publication
    in stage 05's `unit/attached_unit/normal_guarantees.rs`; every clause still
    needs an independently reconstructed exit proof. Fact lists, Exact/Wrapping
    field arithmetic and fields below `&`/`&mut` parameters now publish and
    carry package exit evidence, so package preparation completes and
    `collections/runtime_fixed_vec_round_trip_exit` and
    `contracts/ensures_{fact_list,wrapping_field_arithmetic,borrowed_parameter_field}_exit`
    exit 70 natively and interpreted on macOS ARM64. Named `&mut` parameters
    now use the same live-field arithmetic bounds as `self` in stage 04:
    `contracts/ensures_named_parameter_arithmetic_exit` stores `start + 1`,
    passes both guarantees to `tick`, and exits 0 natively and interpreted on
    Linux x86-64 (package preparation and run commands are in its README).
    Stale writes and wrapping counterexamples still reject. Saturating field
    addition and subtraction now carry inclusive bounds through the shared
    affine certificate's clamped endpoints. On Linux x86-64,
    `omega run --both tests/omega/pass/contracts/ensures_saturating_field_arithmetic_exit/main.omg`
    exits 70 natively and interpreted after local package review (commands in
    its README); `arm(4)` stores 5 and `tick` consumes the returned guarantees
    and returns 4. Stage 05's `integer_selection/policy_translation.rs` assembles
    those bounds and proof admission's `integer_affine` independently replays
    them; strict plateau bounds and inverse saturation witnesses reject.
    The earlier constant Wrapping-store probe (`remaining = 5`) also already
    exits 70 on base `9782e38b64a`; it is not a remaining exit-proof gap.
    Remaining: nonliteral wrapping operands. Reuse
    `tests/unit_scalar_result_source/boundary_wrappers/`; preserve transitive
    contracts and exact entry versions across joins.
  - Generalize entry-operand normalization in stage 04
    `values/scalar/contract_entry.rs`: `EntryOperands::scalar` still declines
    indexed reads, nonempty-path lengths, policy casts and representation views
    in ordinary expression requirements. Preserve exact entry coordinates and
    recursive operand meaning instead of adding source-arrangement exceptions;
    private/local roots must not become public parameters. Package crash
    evidence now preserves this vocabulary recursively through capture and
    bounded canonical recovery, including selector scope checks. Its source
    regression covers whole-view length, Boolean composition and a nonzero
    entry root; it does not close the earlier normalization gap. Verify remaining
    generic-field and case-payload composition through source and recovery.
    `tests/callable_policy/case_membership.rs` already covers authored/indexed
    membership and projected result tags.
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

  - Guarded Trap ceilings cover their Trapping primitive:
    `core/runtime_guarded_trapping_conversion_exit` (`u16 -> u8` under
    `crashes Trap input > 255u16`) exits 70 natively on macOS ARM64, and
    `fail/core/guarded_trap_ceiling_wider_than_primitive_rejected` (`input >
    256u16`) rejects with `UndischargedCrashObligations` at Terminal
    production. Preserve the operation site, operands, carrier, cause and
    normal-only result equation through observation, interpretation and
    native realization. Coordinate **CRASH-CONTRACT**; never replace the
    policy or invent a Crash edge.
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
    `python3 tools/corpus_gate.py --native --filter float/,expressions/runtime_numeric_cast_exit`
    on macOS ARM64 executes 31 programs with their authored results, including
    `float/exact_fractional_integer_boundary_exit` (70) and the Trapping controls.
    Exact conversions use the ordinary destination-owned call; shared operand
    capture and state joins retain IEEE bounds without a cast-only proof path.
    The outer `omega run tests/omega/pass/float/exact_fractional_integer_boundary_exit/main.omg`
    still requires fresh project/package review acceptance. The original
    `float/float_to_int_exact_proofs_exit` executes with exit 78 after preserving
    its field bound as a canonical data `where` clause; the retired range suffix
    was the structural-signature blocker, not an Exact-conversion proof gap.

  Acceptance: comparisons and conversions agree across source evaluation,
  verified artifacts and matching-host execution. Preserve all six relations
  and operand orientations, constants and dynamic operands, differing widths,
  negative values versus unsigned zero/positive values, equality and extrema
  above `i64::MAX`. Retain negated guards, selected meanings, literal landing,
  once-only ordered evaluation and independent overflow obligations.
  `fail/slices/signed_slice_index_upper_only` must still reject: an upper bound
  supplies no nonnegativity proof. Coordinate library range-suffix migration
  with **CANONICALIZE-SCALAR-RANGE-CONTRACTS** without weakening these obligations.

## Pipeline route


Follow one connected stage chain under the
[pipeline](omega-rust/pipeline.md#one-driver-one-pass-per-target-realization)
and [multi-target contract](wiki/spec/build/configuration.md#multi-target-compilation):
Psi checks every target's bodies once, Build evaluates once, and `--target`
narrows realization only. Complete ordinary operation repairs against their
named customers while the shared-source/provider work converges. Remove each
replaced route as its common operation takes over; preserve negative checking,
crash and native exit behavior. Use affected check/native corpus slices under
[the validation policy](tools/testing.md), with full gates at release closure.

- **REFERENCE-LOCAL-BINDING-ROUTES.** (new-scope) Establish reference-valued
  locals from every admitted producer, then preserve their loan identity,
  backing and access through reads, calls and explicit state transfers.
  Reuse the shared
  source-place and view-narrowing operations rather than adding local-specific
  call or range routes here.
  Governed by [ownership](wiki/spec/language/ownership.md#borrows-and-aliases)
  and [loan formation/replay](wiki/spec/terminal-psi/loans.md). Owners:
  `execution/terminal_unit/control/statement_sequence/local_data.rs`,
  `terminal_unit/receiver_aliases/`, and stage 05 argument evaluation,
  `expression_preparation/bindings`, and `scalar_graph/scalar_computations/fields.rs`.

  With this change on base `5b5821b2f0`, on Linux x86_64,
  `python3 tools/corpus_gate.py --both --golden tests/omega/corpus_native_linux_x86_64.txt
  --filter pass/borrows/runtime_record_byte_region_loan_exit` builds and executes
  the byte-identical `samples/cli/systems/descriptor_walk/main.omg` body through
  the repository's reviewed fixture policy: native and interpreter exit **50**,
  in agreement. The fixture has an explicit exit oracle and equivalent Build
  roots. Its shared runtime-indexed record recast now observes each scalar's
  declaration-derived byte region in the original backing, with independently
  replayed index obligations and no copied record carrier. The authored sample's
  direct `omega run --both` command still stops at ordinary package-review
  acceptance; no production acceptance was added by this implementation.

  The former pending fixtures now live under `run/borrows/`:
  `reference_local_binding_routes` and `reference_result_call_binding` both
  execute with native/interpreter exit **0** and matching stdout. Their comments
  describe a wider reduction matrix, not executable coverage of every row.
  The direct-place exclusive-loan control
  `borrow/runtime_exclusive_loan_local_write_exit` retains native/interpreter
  exit **70**, including caller-visible writes. `recast/runtime_guarded_offset_recast_exit`
  likewise agrees at **70**, and `run/borrows/shared_record_loan_member_read`
  agrees at **0**. Preserve ordinary scalar-result and reference-argument calls;
  `dependent/runtime_sibling_len_index_exit` exercises the library returned-view
  form `let view: &[u8] = self.buf.as_slice()`.

  The next record-view dependency is formation-time capture for mutable
  field-held selectors. Runtime byte overlays currently retain immutable scalar
  parameter selectors. Constant locals fold to static selector paths; the current producer
  does not mint a dynamic-local coordinate. A live field selector explicitly
  declines instead of reloading a changed selector at observation.
  Preserve `pass/borrows/shared_recast_loan_member_read`'s native/interpreted
  exit 0 and canonical place ordering beside provider attachments. Extend
  captured loans through remaining producer/read/call compositions. Whole mutable
  view locals now retain their backing and exclusive permission through named
  state transfers: `storage/runtime_local_slice_forward_exit` and
  `storage/runtime_parameter_slice_forward_exit` both execute with native and
  interpreter exit **70** on macOS ARM64. The latter checks caller-visible
  writes across two states and untouched sibling fields. Shared view results
  use one descriptor-value return path, preserving original backing independently
  of the carrier home. Retain `slices/runtime_shared_whole_view_return_exit`
  and `slices/runtime_shared_subslice_return_exit` as native/interpreter exit-70
  controls, including nonzero offset and exact extent through nested returns.
  On base `e3da317976b`, macOS ARM64,
  `python3 tools/corpus_gate.py --both --golden tests/omega/corpus_native_macos_arm64.txt
  --filter slices/runtime_literal_view_return_exit` agrees at **105**:
  image-backed bytes retain their length and contents through a literal getter,
  whole-descriptor forwarding, and a returned subslice. Caller arguments rejoin
  exact checked read access instead of recognizing literal initializers.
  The unprepared public `omega run --both`
  route still requires ordinary package review; fixture policy is not owner
  acceptance. Continue with the remaining producer/read/state compositions.
  Descriptor cleanup remains separate from backing permission; returned shared
  views cannot grant mutation authority or escape activation-owned storage.
  Coordinate record re-views with **RECAST-SOURCE-POSITIONS**.

  Retain `calls/caller_result_binding_mutable_loan_exit` and
  `calls/runtime_alias_write_through_guarded_transition_exit` as native exit-70
  controls: callee writes through captured whole-local loans remain visible
  after direct and conditional completion. `control_flow/conditional_unit_completion_exit`
  additionally exercises both conditional arms with local disposal versus
  transfer. Their macOS ARM64 execution is established; neither named-state
  forwarding nor ordinary Unit completion may regain source-shape gates.

  Shared loans cannot gain mutation authority by widening the exclusive
  `receiver_aliases` match. Retain the local symbol as the loan subject and
  compose its exact captured projection at use; substituting the root loses
  borrow/write-frame/release identity. A known-place loan need not become a
  fresh affine `EstablishReference` result. Preserve the supported exclusive
  marker/store route while extending observations and successor forwarding.

  Acceptance: every producer form, reads through scalar/record loan names,
  borrowed calls and state forwarding reach verified Terminal and matching-host
  execution with exact backing and no implicit copies. Complete the remaining
  fixtures and the original sample's product route. Reject conflicting loans,
  source/path substitution and invalid escapes. Verify constrained referents
  such as `u8 [0..=1]` borrowed as `&mut u8` against the actual loan/store
  contract; normalized type equality must not erase the original storage's
  obligations. **STATE-LOCAL-VALUE-FRONTIER** owns integration with the composed
  caller.

- **DEFERRED-LOCAL-INITIALIZATION.** Complete partial local storage whose
  fields cannot be established by whole-shape zero construction. Retain the
  declaration's exact identity through field establishment, subsequent reads,
  borrowing and cleanup. Whole-local rebinding permission is independent of
  member writes: preserve the diagnostic for non-`mut` whole-scalar assignment,
  not a blanket mutable-local requirement on record storage.

  Stage 04 `control/statement_sequence/local_data.rs`,
  `values/scalar/computations.rs` and the structural store owners must carry
  partial establishment into stage 05's source-custody and stored-loan replay.
  Reuse declaration-backed zero construction only for types that qualify;
  removing its eligibility check cannot supply a missing reference or domain.

  The real customer `pass/wire/runtime_wire_utf8_edge_verdicts_exit` fills
  `LabelSample { count, label: &[u8] in Utf8 }` before borrowing it for decode.
  Its reference field cannot be fabricated by zeroing: support partial storage
  and exact field establishment, including stored-loan custody. Follow
  [default-domain gating](wiki/spec/language/dependent_values.md#default-domains-and-zero-initialization)
  and [ownership establishment](wiki/spec/language/ownership.md#establishment-and-erasure);
  reject premature observation and do not mint linear obligations from zero.
  Acceptance uses the actual Build entries and ordinary corpus execution:
  retain `structs/runtime_declared_record_member_fill_exit` exit 70,
  `structs/uninit_record_member_store` exit 77 and the `let mut total: i32;
  total = 1;` control `locals/runtime_deferred_scalar_initialization_exit`
  exit 1, each with native/interpreter agreement on macOS ARM64. Those three
  controls execute with their expected exits. At `767b982de12` plus the
  evaluated-predicate repair, `omega run --both
  tests/omega/pass/wire/runtime_wire_utf8_edge_verdicts_exit/main.omg` passes
  predicate lowering but still stops in `LabelMessage::decode` at
  `structural field store: borrowed view field: declared domains`
  (state 32, statement 0). Next: reconstruct exact stored-view origins and
  normal-return field writeback through source loans and independent Terminal
  verification before admitting nonliteral descriptor replacement in
  `structural_scalar_store/view_field_store.rs`. The wire customer's required
  interpreter/native exit remains 70.
  Euro must decode successfully; overlong, surrogate, out-of-range and truncated
  UTF-8 must fail. Clearing an omission phase alone does not close the task.

- **TRANSITION-TAIL-CALL-TO-MACHINE.** Finish the customers of terminal machine
  calls in attached Unit transition arms.
  [State contracts](wiki/spec/language/state_contracts.md#terminal-calls-in-transition-arms)
  define an ordinary invocation whose normal result completes the caller;
  an internal state target remains a jump.

  Unit and primitive scalar completion run from any state: an unconditional
  arm is the state's last sequenced call (a scalar result is the state's
  returned binding), and a guarded arm's call is its own
  `CheckedConditionalReturnArm::Call` in the arm's completion block, which
  returns the result the call binds (`state_graph/terminal_calls.rs`).
  Checking rejects a terminal call whose result differs from its caller's
  (`fail/calls/terminal_call_result_type_mismatch_rejected`).
  `pass/calls/transition_tail_call_to_machine`,
  `calls/conditional_arm_tail_call_to_machine_exit`,
  `calls/guarded_arm_tail_call_to_state_machine_exit`,
  `calls/value_tail_call_to_machine_exit` and
  `calls/guarded_arm_value_tail_call_exit` exit 70 natively and on the
  interpreter on macOS ARM64; `collections/runtime_frame_indexed_param_read_exit`
  still exits 1. `calls/guarded_terminal_call_chain_exit` now exits **0**
  natively and in the checked interpreter on macOS ARM64: three ordered arms
  forward 20, 30, and 40 while mutating the borrowed counter exactly once per
  invocation. Source-free Terminal replay also executes mixed call/state/value
  completion and rejects substituted call placement, target, coordinate, access,
  and service evidence. Structural-result tail calls, closed-sum case arms, range-constrained
  result compatibility, arm calls whose guard or operands make calls, and
  claim-transferring arm calls remain declined there.

  Qualified runtime-end byte slices now execute: after package preparation,
  `omega run --both tests/omega/pass/slices/domained_runtime_end_subslice_exit/main.omg`
  exits **70** on macOS ARM64 with `prefix: &[u8] in Path` retained.
  `slices/qualified_subslice_composition_exit` also exits **70** in both
  engines, observing preserved ASCII contents and an empty subslice across
  consecutive calls. Terminal and Omega independently reconstruct primitive
  predicates from exact sources; physical descriptors need no qualification
  annotations. Nonempty and UTF-8 domains do not survive arbitrary byte cuts.
  Keep the fixed-array range and guarded tail-call controls at **3** and **70**.

  Remaining range customers include field/element ranges in ordinary bodies
  (`ordinary_machine.rs`) and statement-call range operands in multi-state
  bodies (`state_graph/mod.rs`, operation custody). Preserve exact endpoint,
  source, access and evaluation-order replay as these routes converge.
  `termination/runtime_shrinking_slice_recursion_exit` now executes unchanged
  with native exit **70** on macOS ARM64, summing 10+20+15+25 through staged
  subslices and head reads. Whole element views use their existing element
  reader even when selecting record fields; entry and arrival ranks observe
  the declared carrier's length. Independent rank replay preserves descriptor
  identity and byte/element units; a prior descriptor or missing decrease
  evidence rejects. Its byte-view control
  `termination/runtime_receiver_byte_slice_recursion_exit` still exits **70**.
  `wire/runtime_wire_decode_byte_slice_exit` stays with
  **RECORD-LITERAL-ARRAY-FIELD** and **REFERENCE-LOCAL-BINDING-ROUTES**;
  `wire/runtime_wire_utf8_edge_verdicts_exit` stays with
  **DEFERRED-LOCAL-INITIALIZATION**.
  `filesystem/dir_walk_wrappers_exit` and `systems/note_vault` retain their
  `Filesystem::create_dir_all` dependency. Shared phase labels do not merge them.

- **CASE-FIELD-TYPE-STORE-SHAPES.** Complete case payload and record members
  under non-interval predicate domains. Follow [data construction](wiki/spec/language/data_and_literals.md#construction-and-case-identity)
  and [erased bindings](wiki/spec/proofs/contracts.md#explicit-erased-bindings):
  evaluate runtime fields once in authored order; establish domains before
  observation. Arithmetic-policy payloads and direct scalar-domain stores
  remain controls, not this omission.

  Common fields beside case payloads, erased payload members and
  interval-domained payload members construct, store, dispatch and read:
  `pass/structs/case_field_type_store_shapes` exits 70 natively and on the
  checked interpreter on macOS ARM64 (one check machine per family, failure
  codes 1-23), as do `ranges/sum_payload_range_arith_narrowed_exit`,
  `relevance/erased_case_payload_field_exit`, `data/runtime_mixed_shape_exit`,
  `calls/runtime_enum_self_method_exit`,
  `generics/runtime_mixed_generic_erased_sum_exit` and
  `traits/equatable_mixed_shape_equality_exit`; the terminal interpreter
  establishes mixed cases with payload cells in their own namespace.
  Record and case literals now share the structural-construction contents
  rule. `pass/domains/record_interval_domain_member_exit` constructs and stores
  an interval-qualified member directly and through a local, returning 70
  natively and on the checked interpreter on macOS ARM64; its out-of-range
  record control rejects before establishment.
  `pass/domains/case_payload_predicate_domain_member_exit`
  (`i32 in NonZero`) stops at `structural field store: case field type`:
  Terminal Psi's structural field vocabulary carries integer bounds but no
  general predicate membership, and `EstablishStructuralCase`/`EstablishRecord`
  roster entries carry only range obligations, so retaining that evidence
  through lowering needs a trusted-surface extension (field type, roster
  evidence, verification, codec), not an admission change.
  Acceptance: the predicate-domain fixture exits 70 natively and on the checked
  interpreter; preserve the passing interval-member program.
  `fail/ranges/case_payload_domain_literal_out_of_range` and
  `fail/ranges/sum_payload_non_case_guard_unproven` keep rejecting.

- **RECORD-LITERAL-ARRAY-FIELD.** Compose supplied borrowed-slice fields and
  the remaining owned-array producers into record literals.
  Empty record-array fields and their nested array extents now lend through the
  ordinary mutable-record call path: `omega run --both
  tests/omega/pass/collections/empty_record_array_borrow_exit/main.omg` exits 0
  natively and interpreted on Linux x86-64, after the helper writes the caller's
  scalar neighbor. `nested_empty_record_array_borrow_exit` checks both sides of
  a nested empty-array field and the parent record's neighbors; index 0 into the
  same empty element type still rejects. This closes empty-shell admission and
  native geometry, not the remaining owned element producers below.
  Literal closed primitive array members already compose at local, nested,
  return and field-store sites: stage 04's shared `structural_value` sends them to
  `CheckedStructuralValueKind::ScalarArray`, whose row-major leaves are keyed
  `StructuralArrayElement { expression, element_ordinal }`, and stage 05
  replays each leaf and emits `EstablishScalarArray` as the record's field.
  `pass/structs/record_literal_array_field` runs natively with exit 0; it
  observes through `self` fields because same-state reads of such a record
  local (`local_scalar_record_field`) and moving the local into a field
  (`continuation cleanup owner`) are still refused. A state's owned record
  arrival reads its array members in its block home:
  `pass/structs/runtime_state_record_array_element_read_exit` exits 70
  natively and on the checked interpreter.
  Named primitive-array operands now use ordinary `CopiedStructuralPlace`:
  `pass/structs/runtime_named_array_record_member_exit` exits 70 natively and
  on the checked interpreter. Copy permission is separate from root discovery,
  where whole-array call arguments already have a payload producer.
  Case payloads now use that same producer and authored member replay.
  `pass/structs/runtime_case_array_payload_exit` exits 70 with `omega run --both`
  on macOS ARM64: construction stores [30, 40], and selected payload reads
  observe both elements. Case-qualified reads compose an independently checked
  payload-carrier copy with an ordinary primitive read; copied aggregate homes
  retain their producer/type custody in native selection. Nested-array reads
  verify, while leaf substitution and copies above their selecting case reject.
  Structural-element array literals whose members are themselves literals
  (records, closed primitive arrays, omitted zeros, nested such arrays) over a
  zero-constructible shape now establish as one `EstablishZeroedStructural`
  plus one authored leaf store each (stage 05
  `structural_values/fixed_array.rs`; bounded fields carry their range
  obligation). `pass/structs/runtime_struct_array_literal_exit` exits 70
  natively. Remaining owned producers: elements that transfer an existing owner
  (places, call products), shapes without a universal zero (bounded fields
  excluding 0, views, byte sequences) and element-policy arrays still meet the
  "fixed array literal has no Terminal establishment" refusal. The Terminal
  interpreter lags the verifier here: it refuses affine
  `EstablishZeroedStructural`, scalar stores into zeroed homes, and projected
  moves out of records with fixed-array members, so stage 05's
  `structural_array_literals` tests verify rather than execute. Both local
  record-array customers run natively with exit 171:
  `pass/calls/caller_structural_result_indexed_borrow_exit` lends
  `&mut rooms[i]` of the local home as its bare element, and
  `pass/calls/runtime_mutable_local_parameter_write_exit` reads
  `rooms[0].exit_count` back through the selector and member. Primitive-array
  elements now use the same exact-subtree loan rule: on macOS ARM64,
  `pass/calls/runtime_local_scalar_array_element_loan_exit` exits 70 after a
  helper mutates `values[position]` and the caller observes both the changed
  value and the untouched neighbor. Terminal verification and canonical
  encoding retain parameter/producer backing and the original projection;
  access, availability, bounds and overlap remain independently checked.

  `pass/structs/runtime_array_member_snapshot_exit` exits 70 natively and on
  the checked interpreter on macOS ARM64 at `03e30df59d33` plus this repair:
  `Packet { samples: context.samples, stamp: stamp(context) }`
  retains [30, 40] while the later operand writes the source's first element
  to 90. Scalar bindings and effects share the projected-assignment inventory;
  an incomplete scalar graph cannot preempt the complete ordinary effect body.
  Checked interpretation captures owned operands before later effects through
  one operation shared by calls and record, case, and array construction;
  reference destinations and referents retain their aliases. The call-order
  control `structural/runtime_structural_operand_snapshot_exit` now also
  agrees between engines (exit 42). A mutable local-array version still needs the
  scalar-array local establishment
  in stage 04 `control/statement_sequence/local_data.rs`, which currently
  refuses mutable bindings, and whole-array computation-argument loans.

  Borrowed views are another field producer, not owned array storage.
  `computations/slice_view.rs` constructs field-site views, and a generated
  codec machine's `&Sample` parameter keeps the entry's place through its
  states (`state_graph/entry_aliases.rs`). On macOS ARM64 at `d5edf66da5`
  plus the byte-loan access repair, `python3 tools/corpus_gate.py --both
  --filter wire/runtime_wire_encode_byte_slice_exit` executes the unchanged
  program with native/interpreter exit 70: exactly `[0, 0, 2, 72, 105]` and
  written count 5. Terminal's `frontier/descriptor_loans/accesses.rs`
  distinguishes unrestricted by-value copies from exclusive backing access and
  retains fixed byte-range geometry under the original field. Aggregate capture
  still extends the backing loan; overlapping writes and owner consumption
  reject. The public `omega run --both` command reaches pending ordinary package
  review (exit 200), not native execution; see the fixture README.
  `wire/runtime_wire_encode_borrowed_scalar_slice_exit`
  still executes incorrectly: native 71 versus interpreter 70, reproduced
  with unchanged `d5edf66da5` compiler sources. Preserve the authored range and
  the single packed-byte-length prefix; its retained repair also needs record
  field descriptor-demand discovery in native selection.
  `wire/runtime_wire_decode_byte_slice_exit` stops at stage 04
  `structural field store: borrowed view field: declared domains`.
  **REFERENCE-LOCAL-BINDING-ROUTES** owns reference locals. Those two wire
  programs remain required native runs, not successes merely because their
  rejection matches an old golden.

  The wire customers construct their records and call the checked codec
  machines `omega/packages/compilation-inputs/src/wire_codec_sources.rs`
  generates per called schema. The generated decoder's final
  `verdict = WireVerdict::Sound` replaces the whole referent of its
  `&mut WireVerdict` state loan through a root window
  (`MoveStructuralField`/`StoreStructuralField` with no field and an empty
  path; `pass/borrows/runtime_whole_referent_*_exit` exit 70 natively). On
  macOS ARM64 with the view-arrival repair on `5fb2e19e1f1`, unchanged
  `pass/wire/runtime_wire_decode_rejects_wrong_era_exit` and
  `pass/wire/runtime_wire_roundtrip_nested_and_repeated_exit` compile and execute
  with exit 70 under `python3 tools/corpus_gate.py --native --filter
  wire/runtime_wire_decode_rejects_wrong_era_exit,wire/runtime_wire_roundtrip_nested_and_repeated_exit`.
  Terminal reconstruction now joins each borrowed view's observed length to
  its own `ViewExtent` and substitutes exact structural arguments in state
  arrival requirements. The full sample and noncanonical-varint customers
  still need rerunning; their prior view-length blocker is not a current failure
  measurement. The authored-plan customer now executes with native/interpreter
  exit 70 on Linux x86-64 after the shared Build policy-context repair recorded
  under CANARY-CORPUS. The round trip compiles in 163884 ms; sampling
  found register-home validation under spill recovery, specifically
  `05_selected-instructions-to-register-homes/src/assignment/home_assignment/validate/`.
  Reduce that measured work without bypassing replay. Acceptance stays unchanged `samples/cli/systems/wire_protocol`
  and `pass/wire/runtime_wire_roundtrip_nested_and_repeated_exit`: exit 70 with
  13-byte encode/decode, room 300, samples [150, -2], depth -64. The borrowed
  customer `pass/wire/runtime_wire_decode_byte_slice_exit` must exit 70 after
  re-encoding the decoded view to five bytes with payload [72, 105]. Run checked
  and interpreter/native legs; leaving an omission cluster is not execution.

- **BOUNDARY-PRIMITIVE-ARRAY-PARAMETER.** (new-scope) Finish authored by-value
  fixed primitive arrays through boundary arguments and results under
  [boundary calls](wiki/spec/terminal-psi/boundary_calls.md). At `020c1dd8933`
  on macOS ARM64, `omega run --both
  tests/omega/pass/providers/runtime_boundary_array_result_exit/main.omg`
  delivers the selected checked provider's `[30, 40]` into the caller and exits
  **70** natively and in the interpreter. Ordinary and installed calls share
  concrete result transport; exact result multiplicity, conformance, source
  custody and native home identities are independently replayed. Nested and
  empty array results survive artifact reload and fuel suspension. Missing or
  substituted installation and result custody reject; opaque host effects
  cannot fabricate array payloads. Existing boundary argument programs retain
  scalar/structural interleave and snapshots around neighboring mutations.

  Remaining: complete direct-boundary argument routes and scalar/reference
  controls. Direct calls retain array gates in `calls/structural_arguments.rs`.
  Trait and direct array result declarations now lower, including empty and
  discarded results; installed trait-provider execution is covered above.
  The promoted `pass/providers/boundary_primitive_array_argument_exit` runs
  on Linux x86-64 with native/interpreter exit **70**. Its checked provider
  reads all four elements between scalar arguments; the final operand writes
  the caller's source array after its owned snapshot was captured. Scalar
  initializer admission follows the resolved result and destination rather
  than the boundary's receiver/supply spelling, and interpreter argument
  capture is shared by ordinary calls and selected boundary adapters.
  `fail/calls/boundary_nested_argument_conflicting_loan` still rejects a later
  exclusive parent loan while the earlier shared argument remains live.
  Remaining acceptance: direct-boundary argument routes and scalar/reference
  controls, with exact payload/interleave through Terminal reload/verification
  and native execution; ordinary-array support alone is insufficient.

  Customers `samples/gui/{window_app,window_demo,image_viewer}` retain their
  by-value class/title signatures. The complete `window_demo` stops at stage
  05's local-array element loan into `ObjectiveC::get_class`
  (**SAMPLE-CORPUS** GUI row); **SAMPLE-CORPUS** retains full GUI execution.
  Domain/reference rewrites or a changed omission label do not satisfy this
  task.

- **OPERATOR-BOUNDARY-CALLS.** (split-of:PROVIDER-SELECTION-AFTER-TERMINAL)
  Finish requirement-level calls and Omega stage 00 installation for legacy
  named operators and compiler-known float realizations under
  [boundary applications](wiki/spec/terminal-psi/boundary_calls.md#operator-applications-and-physical-children).
  Token-bearing machines and nongeneric legacy named operators enter the
  ordinary requirement-call route. At `3dd73f4494f7` plus codec fixture closure on macOS
  ARM64, `python3 tools/corpus_gate.py --native --filter
  providers/checked_boundary_operator_physical_custody,providers/checked_fixed_operator_dispatch_exit,providers/token_machine_family_override_exit,float/named_provider_negate_is_nan_exit`
  executes the named customer and both controls with exit **70**. The named
  customer's source is unchanged. The float customer also executes with exit
  **70** through the typed negate and NaN-classification operations.
  `05_checked-trees-to-lowered-psi/tests/provider_candidates.rs` also executes
  a consumed named-call result as **71** after dropping checked custody.
  Declaration kind distinguishes provider-field roots from independent
  operator calls; stage 07 rejoins the named occurrence and canonical empty
  application. Missing, substituted and duplicate source custody rejects.

  Remaining work:

  - Complete ordinary call composition for the remaining float/receiver
    signatures. Nested boundary applications compose: a nested `a * b`,
    `F64::from_i32(x)` or guard operand is a checked `BoundaryCall`
    computation whose one Terminal `BoundaryCall` runs between its operands
    and its consumer. Field stores and immutable locals whose value is such a
    tree pass the checker, including named operators with literal operands
    (`min(a * b, 11.5)`). On macOS ARM64, `pass/float/nested_float_operator_applications_exit`
    exits **70**, `arithmetic/stats_compute` **70**, `basics/unit_converter`
    **70** and `rendering/mandelbrot` **0** natively.
    `validation::bind_float_conversion_casts` binds whole-carrier-exact float
    widenings, and float-to-integer `in Saturating`/`in Trapping`
    casts, to their exact destination-owned requirement. Same-carrier policy
    erasure adds no operation. On macOS ARM64, `omega run
    tests/omega/pass/expressions/runtime_numeric_cast_exit/main.omg` exits
    **70** unchanged, as does `float/runtime_float_to_integer_policy_casts_exit`.
    Value-dependent float representability and target-predicate casts still
    need their proof-preserving lowering; a rounding named conversion cannot
    substitute for an exact cast or erase its predicate.
    `abs`/`clamp` applications keep their authored call's acknowledgement and
    one Call selection each. Exact float-to-integer casts still have no route:
    the unqualified conversion's `requires` entailment reads neither declared
    float ranges nor dominating guards (**FLOAT-PROVIDERS**), so
    `float/float_to_int_exact_proofs_exit` stays unexecuted and binding Exact
    casts would reject valid programs.
    With these on base `b5cfc489741`, `basics/temperature_convert` met its
    exit **68** oracle and `arithmetic/vector_distance` its exit **5**. On
    `f34b0c7f56` both stop, like `arithmetic/sensor_min_max`, in stage 05
    storage-read custody (`storage_reads/mod.rs`: `scalar read differs from
    its authored binding or mutable place`). Two float-only programs reproduce
    it with no cast or `min`/`max`: a state doing
    `self.console.write_line("start"); self.celsius = 20.0; self.fahrenheit =
    self.celsius * 1.8 + 32.0;` whose successor guards on `self.fahrenheit`
    (red at `447fa43066` without these commits; the same state lowered at
    `b5cfc489741`), and `self.nums[0] = 20.5; self.lo = self.nums[0];` in
    one state (already red at `b5cfc489741`). `rendering/bouncing_ball_2d` and
    `rendering/mandelbrot_zoom` meet their exit 0 and output oracles natively
    with std `Clock`; `gui/{window_demo,windowed_calculator}` stop after
    their float statements at a borrowed byte-array element argument and
    `gui/window_app` at a structural field store destination parameter.
    Scalar graphs and ordinary bodies share the closure's exact boundary
    declarations. On macOS ARM64,
    `omega run tests/omega/pass/expressions/match_float_subjects/main.omg`
    exits **0** unchanged; `expressions/scalar_match_boundary_composition_exit`
    exits **70**, checking nested applications, ordinary helper operands,
    intervening calls and match fallthrough in both float formats. Direct
    scalar entries use the same catalog; missing invocation custody rejects.
    Whole-value boundary results inside scalar helpers use the ordinary
    operation sequence: a bodyless declaration cannot acquire a competing
    direct-body call graph. On macOS ARM64,
    `omega run tests/omega/pass/float/runtime_std_is_finite_exit/main.omg`
    exits **70** unchanged; `float/scalar_helper_boundary_sequence_exit`
    also exits **70** with intervening ordinary calls and nested operands.
    Both build for linux_x86_64. Checked interpretation runs the same
    selection (`checked-interpreter/.../execution/operator_applications.rs`):
    one checked satisfier body, compiler-intrinsic destination-owned
    conversions keyed by source carrier, destination carrier and result
    domain, and the sealed `Float` slots bound from spelled operations. With
    `OMEGA_CORPUS_NATIVE=1 OMEGA_CORPUS_INTERPRET=1` these fixtures and the
    rest of the float/provider cohort report `interpreter:agrees`. A
    requirement with several checked satisfiers still declines there,
    because checked Psi does not retain the Build's provider choice.
    Bound requirements retain arithmetic policy through the same checked
    expression-selection projection as legacy named and spelled operators;
    the interpreter rejects missing, substituted or duplicate selections.
    Native policy-result adaptation executes on macOS ARM64 at `6361c00b32`
    based on `4aed40d441`:
    `python3 tools/corpus_gate.py --native --filter
    float/float_saturating_arithmetic_exit,float/float_trapping_overflow_traps,float/float_trapping_propagated_nan_traps`
    observes **77**, **trap**, **trap**, with program sources unchanged.
    One checked `FloatResultPolicy` follows the original boundary call and
    composes through ordinary value dataflow. Source replay rejects missing or
    substituted adapters and raw-result bypass; native replay binds format,
    ordered operands, exact scratch effects and the operation-owned trap.
    Eight native policy/cast/nested-call controls meet their authored results;
    the three policy programs also build for Linux x86-64, Windows x86-64 and
    Linux ARM64, without foreign-host execution evidence. Both ISA encodings,
    physical recovery and exit checking pass their focused controls.
    All eleven trust-ledger controls pass after registering the two new
    implementation sites and reviewing the twelve changed digests. Twenty-one
    focused verifier, codec, interpreter
    and backend controls pass; seven negative programs retain their expected
    rejections. The current source-replay suite rerun still cannot compile:
    `DUNGEON-STAGE05-TEST-COMPILE` owns the missing `FusedServiceBinding` arm in
    `tests/crash_member_source/fenced_and_float_equality.rs`, unchanged from
    base `4aed40d441`. After that repair, rerun
    `mbx nextest run -p checked-trees-to-lowered-psi --test suite -E
    'test(nested_boundary_applications_source) | test(trapping_operation_sites)'
    --no-fail-fast`; do not treat the native evidence as completion of that suite.
    A `let` after a call-result field store keeps its own value and source:
    source roles key it by its authored local ordinal
    (`TypedTrees::immutable_scalar_locals_before`), while its plan keeps the
    dense binding ordinal that also counts a store result the planner binds.
    `pass/float/local_after_float_call_store_exit`,
    `pass/calls/local_after_call_result_store_exit` and
    `float/named_provider_min_max_sqrt_exit` exit **70** on macOS ARM64.
    Mutable scalar storage retains boundary applications through the same
    computation roots as ordinary calls, including nested operands. On macOS
    ARM64, `omega run tests/omega/pass/float/named_provider_fused_multiply_add_exit/main.omg`
    exits **70** without changing its authored mutable local. The unchanged
    `calls/runtime_std_math_sin_cos_exit` and
    `calls/runtime_value_call_terminal_exit` also execute with native exit **70**
    on macOS ARM64. Composed bodies establish mutable locals through ordinary
    operation sequencing; transition-return boundary calls retain their exit
    computations. Neither path weakens assignment or invocation custody checks.
    The native FMA dependency is executable on macOS ARM64: `omega run
    tests/omega/pass/float/runtime_named_fma_sequence_exit/main.omg` exits **70**,
    distinguishing fused/unfused cancellation in both formats and reusing a
    runtime result through an ordinary helper. x86 FMA still requires the
    feature-custodied transport below.
  - Finish native specialization custody: retain the exact closed requirement,
    provider instance, static tuple and artifact-bound application occurrence
    through selected-plan settlement and physical-child replay. Reuse stage
    00's `boundary_application_settlement.rs`; `ClosedReachApplication`,
    a template alias or equal runtime signatures cannot authenticate the
    specialization. The existing
    `providers/specialized_mixed_structural_result_operator_hosted_native`
    control now executes with exit **0** in the 40-fixture provider/float
    cohort at the revision above; its old selected-plan rejoin stop is closed.
    The remaining matrix must retain substitution rejection as well as execution.
    The native replay owner is
    `native-artifact/src/physical/derivation/evidence.rs`. The new
    `boundary_internal_call_evidence` test covers a nongeneric requirement's
    internal physical child, not this whole generic matrix.
  - Install the remaining named directed arithmetic executions
    as Omega builtins, preserving each exact selected meaning. Reuse stage 00
    `compiler_builtins.rs`, which admits each ordinary execution as one
    `NearestIeeeFloatArithmetic` operation against the local target catalog
    and the exact boundary signature of that operation, stage 02
    `control_flow/ieee_float_arithmetic.rs` realizes the call inline, and each
    ISA encodes one form through scratch FP registers with its own decoder.
    Nearest FMA is a distinct three-operand operation (ordinal 44) on AArch64;
    it is excluded from x86's baseline admission, selection, machine catalog
    and encoder until **X86-FMA-PROVIDER-TRANSPORT** retains its feature evidence.
    Selected-form tag 230 carries the stable operation/format ordinal payload;
    append new identities without renumbering existing ones. Preserve the
    `runtime_named_float_to_integer_conversion_exit` and conversion-boundary/trap
    fixtures as controls when extending the shared operation catalog.
    An unqualified conversion of a runtime field still fails its `requires`
    proof even under a dominating non-NaN/range guard (**FLOAT-PROVIDERS**).
    Re-drive `pending/arithmetic/float_computation_no_route`
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
  recognizes token-bearing machines; `SelectedConstantEvaluation` in
  `realization/selected-dispatch/src/constant_evaluation.rs` prepares selected
  checked provider bodies for Psi evaluation and replay. `comptime/provider_boundary_range_endpoint_exit` now uses
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
  `BuildConfig` remains largely global. Finish authored rows, retain/check
  unrequested rows, and complete shared preliminary and final product checking.

  The bounded package route now consumes a completed output-free Pure Build
  under one evaluator-private compilation owner. Fresh checked-program/fact,
  package-purpose, capture, authority and current-pass resource admission still
  precede consumption; filesystem, restricted, named-input and required-output
  activations do not qualify. The cold Linux witness
  `two_target_native_products_share_nested_source_union` emits the root's actual
  BuildLog line once, keeps five source assemblies, publishes both target
  artifacts, and runs the Linux artifact to exit 7. Final product checking is
  still per target; this does not complete the acceptance below.

  Eliminate the duplicate preliminary whole-program check by moving its
  obligations into the owning stages. Preserve
  [admission before execution](wiki/spec/build/execution.md#admission-before-execution):
  freeze authored selections and admit the exact Build entry before effects;
  then incorporate generated source and finish product checks. The current
  gate is `build-evaluation/src/admission/checking.rs::prepare_checked_build`.
  Ordinary `PreparedBuilds` reuse already includes the exact `build_snapshot`;
  its full-input key and whole-review keys remain unchanged. The narrower Pure
  carrier does not authorize sharing effects or output obligations. Preserve
  separate Build/Product purposes and current-pass logical charges while
  completing the remaining preliminary/final checking and authored-row work.

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

  Compiler checking's `execution_settlement.rs` still joins frontend-dependent
  `provider-planning` and `selected-dispatch` decisions: target-family body
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
  the other target's outcome. Keep the inert-sibling controls
  `{fail,pass}/targets/single_target_internal_*`, root-local and through
  `tests/omega/packages/paradigm-internals`: a missing called body rejects only
  the affected realization, naming caller, callee and target, while a caller
  scoped to its callee's own unrealized target stays exempt and executes.
  Independent moves need not wait for full closure.

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

- **DRIVER-EARNS-ITS-LINES.** (new-scope) The product driver should read as
  "`build.omg` in, program out": prepare sources, run the stages, publish.
  Measured 2026-10-02 on `eb241170ef`: `omega/compilation/driver` holds about
  1.8k production lines and `driver/terminal-production` about 700 (plus ~590
  test lines), while the stage chain itself is roughly 15 lines in
  `terminal_production.rs` (select entry, Psi 05 lower, 06 optimize, 07
  publish) and 40 in `compiler.rs`. The rest is plumbing:

  - Two timing systems: `terminal-production/src/stage_timings.rs` keeps its
    own ladder that `terminal/terminal_artifact.rs` merges back into
    `CompileTimings`. Keep one.
  - Custody wrappers: `terminal-production/src/custody.rs` threads a generic
    caller payload through production only to hand it back on failure, with
    wrapper error types for it. Use a plain result.
  - Self re-validation: stage 07's entry receipt is prepared and then rejoined
    against the artifact production just published. The producing stage
    should return the receipt it validated once.
  - Checked-tree sidecars carried past Terminal (source-call occurrences, IEEE
    and integer comparison occurrences, the boundary-operator scope rebuilt
    from checked trees) exist because Omega still reads checked trees; they
    leave with **PROVIDER-SELECTION-AFTER-TERMINAL**.
  - Reuse caches in `driver/src/checked` (`PreparedBuilds`, typed-frontier
    retention, the pure-build owner, `compile_checked_reusing_builds`,
    counters) deduplicate per-target and per-package repetition; they leave
    with **ONE-DRIVER-PER-STAGE**. The typed-to-checked wedge
    (`checked/checking/execution_settlement.rs`) leaves with
    **PROVIDER-SELECTION-AFTER-TERMINAL**; the driver's product re-admissions
    belong to **COMPILE-REPORT-PRODUCTS**.

  The first three bullets are this item's own work and can land now; the
  rest is the closing gate for those items. Acceptance: driver plus
  terminal-production production code is about 200 lines or fewer, and every
  remaining line is a stage call, report construction or publication; a caller
  compiles a package root without assembling per-target configurations itself.

- **COMPILE-REPORT-PRODUCTS.** (new-scope) Make product admission part of
  report construction in
  [compile_report.rs](omega-rust/omega/compilation/products/src/compilation_products.rs),
  then use one report-settlement call in
  [compiler.rs](omega-rust/omega/compilation/driver/src/compiler.rs).
  Variant-owned artifact/rollback/application payloads already exist.
  Remaining eligibility checks live in `admit_requested_product` and package
  admission, while `with_pcc_context` can attach incompatible requests directly.
  `CheckedCompilation`'s remaining product operations (`production_subject`,
  `admit_terminal_production`, `terminal_native_proposal_request`,
  `terminal_report`, `native_report`, `check_report`, `prepare_product_report`)
  are this item's report-construction surface.

  Centralize the [proof-product](wiki/spec/proofs/publication.md#product-selection)
  and [rollback](wiki/spec/build/optimizations.md#release-rollback-and-promotion)
  refusals at product-owned construction boundaries: artifact-only builds cannot
  satisfy rollback or proof requests, check stops cannot satisfy proof requests,
  and Terminal stops cannot satisfy native proof requests. Preserve early refusal
  before unsupported production or publication; a late constructor check alone
  is insufficient. Route package-manager's `admission/terminal_authority.rs`
  and `operations/check_project.rs` through the same admission. Preserve payload
  custody, application metadata, build outputs, trust settlement and timings;
  proof requests confer no receiver authority.

  Acceptance: remove the route's duplicate admission helper and scattered
  decoration. Direct construction/context attachment and compiler/package routes
  must reject every incompatible request without publishing partial outputs.
  Exercise check, Terminal, retained native and
  [artifact-only](wiki/spec/build/scoped_execution.md) products, retained-to-published
  conversion, and the existing ObjectContainer compatibility contract if retained.
  Run compiler check/nextest, package-manager's `suite` filters
  `semantic_binding_review` and `build_named_inputs`, and check/native corpus
  slices. The corpus runner has no Terminal-request mode: cover Terminal and
  artifact-only admission directly, including valid optional-proof combinations,
  instead of treating a check/native corpus pass as their acceptance.

## Data-flow defects

Each item removes one verified copy, rescan or repeated validation in the
compiler's flow from source bytes to image bytes. Sites were read at
`017136d36f` (2026-10-02); re-read the cited symbol before starting, since
paths drift. Each item lands alone with the cited corpus slice unchanged
(check plus `--native` where the code is backend-visible). Re-checking stays
allowed at trust boundaries; these items remove repetition of a check the
same flow already performed, not the check itself.

- **STD-CHECKED-ONCE-PER-INVOCATION.** (new-scope) `omega --check --timings`
  of `tests/omega/pass/borrows/runtime_record_byte_region_loan_exit` takes
  14.8 s on the Windows release build, of which its own stages take about
  120 ms. With `OMEGA_REVIEW_TIMINGS=1`, `omega_language_std` compiles twice:
  11.0 s in the `discovery` review pass and 1.6 s in the `bound` pass
  (`package-manager` `review/candidate/compilation/package_pass.rs`
  `compile_dependency_closure`, run once per pass). Reuse the discovery pass's
  checked dependency compilation in the bound pass when that dependency's
  compilation inputs are unchanged. Acceptance: the same command prints one
  `stage=package omega_language_std` row, and its outcome and diagnostics are
  unchanged.

- **CHECK-MEMO-PROGRAM-CLONES.** (new-scope) Stage 04's
  `checking/exact_input_memo.rs` deep-copies whole programs. A miss clones
  the input `TypedTrees` (`let retained = program.clone()`) and the result
  (`result: result.clone()`). A hit clones the stored result.
  `programs_equal_for_check` clones two programs to clear one field before
  comparing them. Retain shared `Arc` values and compare without clearing
  copies. Acceptance: `exact_input_memo.rs` contains no `TypedTrees` or
  checked-result `.clone()`, and memo hit/miss counts on
  `python tools/corpus_gate.py --filter providers/` are unchanged.

- **LOWERING-NOMINAL-RETURN-CLONE.** (new-scope) Stage 05's
  `returns/structural_scalar_return/nominal.rs` (`let mut staged =
  checked.clone()`) copies the entire `CheckedTrees` for each nominal
  structural return it stages. Stage the return against a local record of the
  changed facts instead. Acceptance: no `checked.clone()` in stage 05
  non-test code; corpus `--filter returns,structs/` check and native
  unchanged.

- **CHECKED-LEDGER-VERIFY-ONCE.** (new-scope) Stage 05's
  `machine_lowering/checked_ledger.rs::verify` runs on every
  `lower_machine` call (`machine_lowering.rs`), so once per entry and
  callback artifact. Each permission event does a linear
  `checked.machines().iter().find(..)` plus a state scan, and duplicate
  detection uses `events.contains` on a `Vec` (quadratic). Index machines and
  states once, detect duplicates with a set, and verify once per checked
  program. Acceptance: one `verify` call per `CheckedTrees`, linear in
  events; the ledger rejection fixtures still reject.

- **RESOLVE-SYNTAX-FOREST-CLONE.** (new-scope) Stage 02's
  `preparation/mod.rs` starts with `let mut syntax = syntax.clone()`, copying
  the whole syntax forest it was handed by reference. Take `SyntaxTrees` by
  value at the stage entry and move it through preparation. Acceptance: the
  stage 02 entry consumes its input and preparation performs no whole-forest
  clone; full check corpus unchanged.

- **REFERENCE-CUSTODY-STATE-SCANS.** (new-scope)
  `checking/src/machine_calls/reference_result_custody.rs` finds a call's
  target state by scanning every state of every machine
  (`machines().iter().flat_map(|machine| program.machine_states(machine))`,
  five sites), once per call expression. Resolve the state through its
  symbol's parent machine or the existing declaration index
  (`TypedTrees::machine_by_symbol`). Acceptance: no whole-program state scan
  remains in that file; corpus `--filter borrows,references/` unchanged.

- **TYPE-IDENTITY-RENDER-ONCE.** (new-scope)
  `TypedTrees::normalized_type_identity` renders a fresh
  `NormalizedTypeIdentity(String)` on every call, with no cache, and stages
  04, 05 and checking call it at about 500 sites. Memoize the rendered
  identity per `TypeReferenceHandle` inside `TypedTrees` and return it by
  reference or shared handle. Acceptance: a second call for the same handle
  does no rendering, and identities and the full check corpus are unchanged.

- **PSI-OPTIMIZATION-VALIDATE-ONCE.** (new-scope) Stage 06's
  `psi_optimization.rs` runs the full Terminal verifier through
  `validate_carrier` on its input, which stage 05 already verified at the end
  of lowering (`validate_module`), and again on its output. Validate the
  output only. Acceptance: one `validate_carrier` call per optimization run,
  and the rejection controls in stage 06's tests still reject.

- **PSI-CLEANUP-MODULE-CLONE.** (new-scope) Stage 06's
  `control_flow_cleanup/machines.rs` clones the whole module on each prune
  iteration (`let mut proposed = module.clone()`) to compute
  `block_local_evidence` over the kept machines. Compute the evidence over the
  kept set without copying the module. Acceptance: no module clone in that
  loop; the `ControlFlowCleanup` optimizer tests and corpus `--filter
  optimizer/` unchanged.

- **TERMINAL-DECODE-ONCE.** (new-scope) Omega non-test code calls
  `terminal_codec::decode_module` at 22 sites, eight in stage 00 alone
  (artifact admission, callback settlement, entry settlement, service
  establishment, retained-terminal admission), and
  `native-artifact/src/fragment_object_publication.rs` round-trips the
  artifact through `from_bytes(&terminal.to_bytes())`. Decode once at stage
  00 admission and pass the decoded module with its memoized validation to
  the other consumers; remove the round trip. Acceptance: one decode per
  realization (assert with a counter in a test), and native corpus `--filter
  entry/,providers/` unchanged.

- **EMISSION-SOURCE-VALIDATE-ONCE.** (new-scope) machine-emission's
  `fragment_emission/mod.rs` calls `validate_source(&source)` and then
  `validate_optimized_function_fragment_emission`, which calls
  `validate_source` on the same source again. `frame_application` and
  `text_placement` repeat the pattern. Validate each step's source once.
  Acceptance: one `validate_source` per emission step, and native corpus
  unchanged.

- **SELECTION-CUSTODY-REVALIDATION.** (new-scope) Stage 03's
  `optimized/validation.rs::validate_optimized_selection_custody`
  re-legalizes and re-selects (`validate_legalized_operations`, then
  selection validation) work that `legalize`/`select` already validated. Stage
  04's liveness custody (`analyses/liveness/staging/validation.rs`) calls it
  again, and the live-range and legality custody validators each re-run the
  validator before them. Have each validator accept the sealed receipt of its
  predecessor instead of re-running it. Acceptance: selection custody
  validation runs once per selected plan, with existing substitution and
  mismatch controls still rejecting.

- **BRANCH-RELAXATION-BATCH.** (new-scope) Stage 09's
  `x86_branch_relaxation/compute/production.rs` loop relaxes one branch per
  iteration. Each iteration scans every function for the first candidate,
  re-encodes it, reflows every function and hashes the whole program twice
  (`revision_identity`). Relax every out-of-range branch found in a sweep,
  reflow once and hash once per sweep. Acceptance: iteration count is bounded
  by sweeps rather than relaxed branches, and the relaxed bytes on native
  corpus `--filter control_flow/` are unchanged.

- **PRE-ALLOCATION-BATCH-COMMIT.** (new-scope) Stage 04's
  `rewrites/pre_allocation/execution.rs` commits the first admissible
  candidate and restarts the whole sweep ("The first admissible candidate
  commits and the whole sweep restarts"). Commit every non-conflicting
  candidate in a sweep and restart only when a commit enables another family.
  Acceptance: the same fixed point is reached in at most as many sweeps as
  family interactions require, with the pre-allocation optimizer tests and
  native corpus `--filter optimizer/` unchanged.

- **IMAGE-BUILT-ONCE.** (new-scope) image-emission's `image_output.rs`
  builds the final image (`image::build_final_image`) and
  `validate_direct_executable_image` builds it again from the same inputs to
  compare them. Keep the image's structural validation and drop the rebuild.
  Acceptance: one `build_final_image` call per emitted image, and native
  corpus image bytes unchanged.

- **OBJECT-CONTAINER-NO-ROUNDTRIP.** (new-scope) object-file's
  `fragment_container/validation.rs` decodes
  (`decode_relocation_free_object`) the container its production just
  encoded. Validate the staged plan before encoding and keep the decoder for
  independent readers. Acceptance: the production path encodes the container
  once and does not decode it; native corpus unchanged.

- **ENCODED-ROW-ALLOCATIONS.** (new-scope) machine-code's
  `encoding/rows.rs` stores every encoded instruction as its own
  `bytes: Vec<u8>` plus a `Box<SelectedFormDecodedFootprint>`, so each
  instruction costs two heap allocations. Store a function's bytes in one
  buffer with per-row offset spans, and keep footprints inline or in a
  per-function table. Acceptance: no per-row heap allocation in encoded rows,
  and native corpus bytes unchanged.

- **CHECKING-FROZEN-PROGRAM-CONTEXT.** (new-scope)
  `checking/src/frozen_program.rs` keeps its memos in a `thread_local!`
  `RefCell` slot keyed by the program's address, which AGENTS.md rules out
  as an ownership escape hatch. It has 11 uses in 9 files. Replace it with an
  explicit context value passed to those consumers. The global
  `ENVIRONMENTS` mutex cache with a linear find in Omega stage 03's
  `selection/mod.rs` goes the same way: build the environment once per
  realization and pass it. Acceptance: neither the thread-local nor the
  static mutex remains, and the check corpus is unchanged.

## Automatic service reach

Finish portable coverage of
[static callback reach dependencies](wiki/spec/language/effects.md#static-callback-reach-dependencies).
The source-side finite-union inference, closed application retention, selected
contract bounds, and two-sided emitted-call replay already exist. Remaining:

- Complete original dependency and telescope retention, including inlined or
  absent emitted callees. `retain_closed_reach_applications` skips proposition
  and requirement-identity binders; `prune_incomplete_closed_reach_applications`
  removes incomplete annotations and their dependents. Its fixed-point test
  injects a missing projection: it checks pruning, not complete source coverage.
- Replace frontend-local template identities with independently checkable
  original-contract projection evidence. Producer-side replay against the
  retained source graph is not source-free verification, and commitment hashes
  identify a projection rather than authenticate its derivation. Reuse exact
  specialization/contract identities and the PCC ledger/bridge work; do not
  invent a parallel selection identity or a second proof system.

Owners: `05_checked-trees-to-lowered-psi/src/retention/closed_reach_applications.rs`,
`omega-rust/psi/compilation/checking/src/machine_calls/static_machine_call_contracts.rs`, and
`terminal-psi/verification/src/validation/reach_applications.rs`. Keep ordinary semantic
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
`build-time-evaluation/src/reflection/` exist, but their
construction/composition callers are Rust tests. Their replay takes typed trees;
`RuntimeVisitationPlan` records operation identities without field loans or
active-case execution. Connect these helpers to source elaboration and portable
evidence rather than treating their checks as a working reflection route.

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
extend that route through ordinary checked Build execution and retained outputs.

- **BUILD-PRODUCT-REFERENCES.** Finish executable follow-through for
  [non-executing product selection](wiki/spec/build/scoped_execution.md#selecting-product-declarations-without-executing-them).
  `checked-interpreter`'s `build_machine/product_entries.rs` already enforces
  lexical product visibility and slot compatibility; `provider-planning`'s
  `program_entry/root_bindings.rs` retains the selected symbol and slot.
  Preserve [separate checked contexts](wiki/spec/build/scoped_execution.md#two-checked-contexts)
  and **BUILD-ADMISSION-CHECKPOINT** source custody, never durable authority from
  names or evaluator indices.

  The scalar/multi-state customer runs. `pass/build/described_runner_entry_{left,right}_exit`
  (sibling leaves) and the literal same-leaf twins
  `same_leaf_runner_entry_{left,right}_exit` (both modules declare `Reader`,
  `Provider`, `Runner`) exit **11** and **37** natively and interpreted.
  **STATE-LOCAL-VALUE-FRONTIER** owns scalar/provider and state-call closure;
  **ENTRY-CONTENT-ROOTS** owns receiver attachment.

  Retain source controls for qualified paths, computed receivers, named lifetimes,
  parent loans and conflicting later operands. `fail/build/product_entry_*` and
  `root_binding_foreign_helper_private_entry` pin private/sibling enumeration,
  build-scope aliases, undeclared slots, unknown targets, incompatible
  signatures, slot mismatch, ambiguous leaves and forged descriptions. Still
  unpinned: stale descriptions, description-to-callable conversion, same-build
  generated/layout cycles, and the positive control that an explicitly
  delegated private entry description stays usable without enumeration
  authority.

- **SAME-NAME-REQUIREMENT-CALL-TARGETS.** (new-scope) Remaining: replace
  name-matched attachment lookups where a same-name-module customer reaches
  them: stage 04 `execution/terminal_unit/calls/signatures.rs`
  (`structural_signature_with_partial_affine`), `operators/receiver.rs`,
  the contained-target filter in `facts/carry.rs`,
  `checks/contracts/evaluator/collections.rs`,
  `monomorphization/body_cloning.rs`; stage 05
  `returns/boundary_scalar_return/source_custody.rs`. Use the retained
  attachment identity, including closed applications, rather than a global
  record-name match. Extend a runtime customer through the affected storage,
  borrow, or return path before replacing its lookup.

  `pass/providers/same_name_requirement{,_right}_dispatch_exit` and
  `pass/modules/same_name_requirement_field_calls_exit` exit 70 natively and
  interpreted on macOS ARM64. The modules fixture's `self.sink.emit(..)`
  statements reached `borrow/roots.rs`, `lookup/calls.rs`, the owned-receiver
  multiplicity in `calls/signatures.rs` and the topology fields in
  `facts/carry.rs`, which now select by attachment symbol; instrumenting the
  sites above showed no current customer reaching them. Preserve rejection of
  forged targets from a different requirement.

- **BUILD-SNAPSHOT-OUTPUTS.** Finish coherent capture and host acceptance under the
  [captured-input and committed-output contract](wiki/spec/build/scoped_execution.md#inputs-and-default-filesystem)
  across ordinary compilation. `compilation-inputs/src/source_snapshot.rs`'s
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
  in `omega-rust/omega/application/cli/tests/completed_build_outputs.rs` already exercises narrowed template
  reads, exact artifact/companion output, native invocation, retries and distinct
  package/target output sets. Re-run that customer on macOS and Windows; preserve
  `omega-rust/omega/application/cli/tests/{build_input_inventory,package_commands/snapshot_outputs}.rs` and
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
Terminal's [terminator](omega-rust/psi/compilation/representations/terminal-psi/src/terminal_module/control_flow/termination.rs)
has no external-completion variant; the
[trace codec](omega-rust/psi/compilation/representations/terminal-psi/codec/src/sections/terminal_trace_v1_profile.rs) writes zero external rows and
rejects a nonempty roster. A native nonreturning operation does not fill this gap.

- Close native customer gaps using existing `SelectedProcessExit` custody.
  [Normalized foreign-call custody](omega-rust/omega/compilation/backend/images/image-emission/src/object_artifact/normalized_foreign_calls.rs) still accepts
  scalar homes only from call-result producers; finish general computed-status
  argument transport and independently replay its selected source. Windows
  `ProcessExit` already selects kernel32 `ExitProcess` through `DllImport`.
  Complete the Console byte leaves through `WriteFile`/`ReadFile`; their current
  `CompilerIntrinsic` declarations have no Windows hosted bundle. Replace the
  fixed 256 KiB foreign-call reserve with per-callee stack evidence in Omega
  stage 00's [source-evaluated bindings](omega-rust/omega/compilation/pipeline/00_terminal-psi-to-abstract-operations/src/provider_admission/source_evaluated_bindings.rs). These native
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
`tests/omega/pass/host/process_exit_i32_status_{mapped,ordered}` customers and
their `expected_native_exit.<target>.txt` oracles on the three Unix targets:
`ordered` exits 70 after its one stdout line, and `mapped` presents status -44
as 212 while the interpreter keeps -44 (both measured on macOS ARM64, where the
runner's `--both` agreement applies the host presentation rule). Windows gets
its promises once `ordered` builds there (its golden records a rejection) and
a Windows host measures `mapped`'s full-i32 presentation. Interpreter exit ends
only its simulated domain.
Cross-target emission is not runtime evidence.
**TWO-AXIS-TERMINAL-AUTHORITY-REVIEW** owns further receiver admission; no general
completion syntax or other terminal service is a prerequisite.

## P1 - Authority, roots, and entry

- **ENTRY-CONTENT-ROOTS.** Complete provisioned receiver activation/completion
  under [entry roots](wiki/spec/build/entry_roots.md) and
  [binding validity](wiki/spec/build/component_publication.md#bindings-and-era-entry).
  Reuse opaque affine `Binding<R>` in `source/library/core/binding.omg`;
  `ForeignBinding` is a separate locator. No authored `Bound` qualification,
  source-created binding or provider lookup repairs missing establishment.

  - Execute the remaining borrowed-service customers:
    `samples/cli/games/dungeon_crawler_cli` now uses `&mut Binding<Console>`
    formals; **NOMINAL-FIELD-FLOW** records its current command-flow frontier.
    Carry that existing flow to native execution. Make
    `capabilities/uses_caller_folder` lend the caller's field to
    `Librarian::archive(folder: &Binding<Folder>)` and execute its service effects.
    Shared/mutable machine and state formals now retain exact binding identity,
    loan access and Fused dispatch receipts; bare boundary-trait value formals
    reject. The two-read control `omega run --both
    tests/omega/pass/text/runtime_stdin_line_buffering_exit/main.omg` with its
    `input.txt` executes both engines at exit 0 on macOS ARM64, with exact
    `first\nsecond\n` native output; keep that control while expanding customers.
    Moving an affine binding from borrowed storage without replacement must
    still reject. Provider multiplicity, conformance and Fused erasure do not
    change declaration-owned affine semantics.
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
    [hosted receiver tests](omega-rust/omega/compilation/backend/images/image-emission/tests/artifacts/hosted_receiver.rs)
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
  [device custody and ordering](wiki/spec/resources/device_access.md). The
  checked contract now checks clean: `build.omg` accepts the device boundary
  slot, `LoopbackDeviceLoanProvider` supplies ten
  `via ForeignBinding::CompilerIntrinsic` crossings on macos_arm64 through
  `provider_defaults`, and `device.omg` declares `DeviceOrderingEvent` with
  Publication/CacheMaintenance/Notification/Completion roles plus retry
  carriers (`DeviceCompletionAttempt`, `DeviceAcquisitionAttempt`), with
  `Extent::Granted` issued by `DeviceLoanProvider::reserve` and
  `DeviceLoanProvider::acquire`'s `Acquired` payload. A hosted entry supplies
  no arrival extents, so the provider owns the range pool: `reserve` mints
  `Granted` ranges and `release` returns them, keeping the whole lifecycle —
  reserve -> lend -> publish -> notify -> settle -> maintain_cache -> settle
  -> complete -> acquire -> release — inside one authority. The drivers stage
  both directions with Pending-retry states, and `sample_gate` reports 1/1.
  The frontier is two gaps. First, Terminal unit plans admit no
  `BoundaryOpaque` types in reachable state or callee signatures
  (`shape_collector` only plans `CheckedShape` data), so no bound entry can
  carry a live `DeviceLoan` or `DeviceOrderingEvent` across a transition —
  the drivers are deliberately unreachable and `Main::main` is inert. Second,
  the ten crossings have no runtime bodies, so `omega run` cannot execute the
  protocol.

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
  original caller storage. `collections/runtime_indexed_struct_field_rmw_exit`
  exits 1 as authored on macOS ARM64, and
  `collections/runtime_parameter_nested_literal_index_read_exit` exits **70**:
  helper machines borrow a caller's `[[i32; 4]; 3]` and `[Row; 3]` locals,
  store through literal and runtime selectors, and read `grid[1][2]`,
  `rows[2].data[1]` and `rows[2].tag` back. Preserve
  `runtime_double_indexed_rmw_exit` at native exit **1** on macOS ARM64:
  plain array loans now follow the same exclusive state-name and address
  transfer as record loans, including backing created in a non-entry state.
  Source checking does not establish native store correctness. The macOS native
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
  judges statement-reachable guards, call arguments and nested operands.
  Checked trees retain a whole-place scalar re-view as
  `CheckedScalarExpression::RepresentationView` over the source read at its
  declared carrier; stage 05's `expression_preparation/representation_view.rs`
  replays equal-width integer views as the modular image, and the field-read
  equality check in `bindings/structural_fields::resolve` stays exact. Preserve
  checked layout/validity, backing identity, lifetime, access and whole-place
  reads through ordinary expression sequencing and temporary loans.

  On base `f64782b891147a98314a566696a5a5e044378c43`, Linux x86-64
  native production rejects `runtime_shared_domain_weakening_recast_exit` and
  the equivalent mutable domain/range siblings at stage 05 `Unit graph dropped
  or added a body effect`; their checked interpretation returns 70. Replaying
  storage with `validation::declared_carriers_match` in
  `unit/attached_unit/statement_custody.rs` instead of full qualification
  identity makes all three unchanged programs return 70 natively and in the
  interpreter (`python tools/corpus_gate.py --both --filter
  runtime_shared_domain_weakening_recast_exit,runtime_mutable_equivalent_domain_recast_exit,runtime_mutable_equivalent_range_recast_exit`;
  [reproducer](tests/omega/pass/recast/runtime_shared_domain_weakening_recast_exit/README.md)).
  The checked recast judgment still owns qualification legality; exact loan
  geometry, access and activation still own statement erasure.

  The unchanged total-order customer (`omega run --both
  tests/omega/pass/float/runtime_total_order_satisfiers_exit/main.omg`;
  [reproducer](tests/omega/pass/float/runtime_total_order_satisfiers_exit/README.md))
  still rejects at stage 05 `computed primitive borrow lost its established
  local`; interpreter exit 70. The scalar-parameter shared-view control returns
  70 in both engines. Next acceptance is native total-order exit 70 after
  retaining the scalar parameter's backing home and passing its shared
  representation-view loan through borrowed helper calls in
  `expression_preparation/bindings::ScalarBindings::primitive_borrow`.
  Reference-view replay requires the live static-tail-loan assignment's
  Terminal/Abstract references validation and the Squalr reference-store
  assignment's Terminal result contracts; coordinate those dependencies before
  editing. Ordinary package-review acceptance can independently stop the public
  command at exit 200; the corpus command uses reviewed fixture policy.

  Keep `runtime_bool_representation_recast_exit` and
  `runtime_float_range_representation_recast_exit` as native/interpreter
  exit-70 controls. Scalar and record-leaf cross-carrier observations use
  Terminal `ScalarRepresentationView`; byte-to-bool guards retain and prove
  the canonical 0/1 landing through ordinary scalar computations. Preserve
  write-only rejection and the mutable bool-as-unconstrained-byte rejection.

  Fixed-index nested-array reads and stores through byte-backed record loans
  now retain normalized geometry and live backing identity through stage 05's
  `Evaluation` and `ScalarBindings`; shared read-only loans use the same route.
  On base `e94b9f79576` with this repair, macOS ARM64
  `python3 tools/corpus_gate.py --native
  --filter runtime_plan_laid_nested_fixed_array_mutable_write_exit` observes
  native exit 70 on the unchanged source; the `--both` leg also observes
  interpreter exit 70. The public
  `omega run --both tests/omega/pass/layouts/runtime_plan_laid_nested_fixed_array_mutable_write_exit/main.omg`
  stops at missing package-review acceptance (exit 200); review approval remains
  separate from compiler acceptance. Runtime array selectors still need retained
  per-dimension bounds. Full value replay needs local-root projection semantics
  in `source_custody/value_correspondence/mod.rs` (currently requires a parameter
  root for nested element suffixes); coordinate with its live static-text owner.
  Carry the captured-loan context through remaining structural/literal operand
  namespaces when extending those compositions, not through per-use recognizers.

  With this change on base `e416dc26b7e`, `python3 tools/corpus_gate.py --native
  --filter recast/` on macOS ARM64 builds and exits 70 for
  `runtime_shared_domain_weakening_recast_exit`,
  `runtime_mutable_equivalent_domain_recast_exit` and
  `runtime_guarded_offset_recast_exit`; a runtime-offset byte-region read
  replays byte by byte from its first `u8` element, and
  `runtime_integer_bit_view_recast_exit` pins equal-width integer and signed
  byte-region views with the interpreter in agreement. The remaining executable
  customers stop at these missing capabilities:

  - Stores through a `&mut` recast loan
    (`runtime_mutable_equivalent_range_recast_exit`, `structural field store:
    target place`): `structural_scalar_store/destination.rs::rejoin_loan_local`
    does not peel the recast, and the stored view-carrier value needs a retained
    store re-view into the source carrier that stage 05 replays.
  - Runtime-offset byte-region stores
    (`runtime_offset_byte_recast_mutable_write_exit` at `structural field store:
    target place`) share the `&mut` recast store gap above; the store must
    write each footprint byte with its own index obligation.
  - Record, record-array and slice views stop at `attached data shape`
    (`runtime_mutable_equivalent_record_recast_exit`,
    `runtime_shared_record_float_range_weakening_exit`), `structural field
    store: target place` (`runtime_record_array_view_mutable_write_exit`),
    `parameter transfer` (`runtime_aggregate_slice_representation_recast_exit`;
    `runtime_interior_slice_view_mutable_write_exit` and
    `runtime_interior_slice_congruent_offset_exit` at `conditional successors:
    parameter transfer`, state 0 statement 2). Their `&mut place as &mut [T]`
    locals are exclusive place-loan carriers that establish no view, so there is
    no binding to forward. Terminal has no operation for either view: a mutable
    byte window at a runtime offset over a fixed `[u8; N]` field
    (`ByteSequenceSubslice` and `ElementViewSubslice` produce immutable windows
    only), or a byte-to-element re-view (`EstablishElementView` and
    `EstablishMutableElementView` require the collection's own element type).
    That vocabulary and its verifier rules are a trusted-surface dependency.
    Record views share referee-record declaration minting.

  `nested_operand_view_compile` checks without an executable entry. Immutable
  `&mut` place-loan locals read their loaned place in scalar operands; other
  reference locals initialized from place loans remain shared work with
  **STATE-LOCAL-VALUE-FRONTIER** / **REFERENCE-LOCAL-BINDING-ROUTES**, and range
  syntax migration belongs to **CANONICALIZE-SCALAR-RANGE-CONTRACTS**.

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
  layout-plan and writer-lowering compiler tests were removed; restore the
  exercising source customers through the current harness.

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
  `omega-rust/omega/packages/review-evidence/src/record/` has no corresponding
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
Compiler paths below are relative to `omega-rust/omega/compilation/driver/`. Reuse the
shared checker in `omega-rust/omega/compilation/backend/plans/provider-evidence/src/provider_evidence/executable_scopes/behavior_exclusions.rs`.

- **BUILD-SEMANTIC-EXCLUSIONS.** Finish source-to-publication acceptance for the
  full admitted entry/call closure. Executed selections in
  `realization/trust-model/src/behavior_exclusions/selections.rs`, unoptimized
  entry/callback gates in compiler `src/terminal/`, and stage 00's
  `retained_terminal.rs` proposal replay already exist. The shared checker/tests cover
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

  [The package verdict harness](omega-rust/omega/packages/package-manager/tests/behavior_exclusion_verdicts.rs)
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
  `omega-rust/omega/compilation/backend/artifacts/native-artifact/src/native_artifact.rs`
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
`omega-rust/omega/compilation/driver/`.

- **BUILD-TEST-GROUPS.** (new-scope) Build registration is missing:
  `build-declarations/src/prelude.rs`'s `Build` has no `tests` facet, and
  `compilation-inputs/src/build_execution/build_value/configuration.rs`'s
  `EvaluatedBuildSettings` retains no groups. Build evaluation and its
  package-owned value protocol must carry
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
  and `omega-rust/omega/packages/package-manager/src/review/restricted_build_grants.rs`
  join normalized build-host requests to lock acceptance; extend them to retain
  test/group, fixture and provider attribution before execution. Existing
  [restricted-request tests](omega-rust/omega/packages/package-manager/src/review/candidate/compilation/tests/restricted_build_grants.rs)
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
  Reuse the [source-driven registration control](omega-rust/omega/compilation/backend/runtime/component-publication/src/tests.rs):
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
  `omega-rust/omega/compilation/realization/provider-planning/src/provider_settlement/composition_modes.rs`
  rejects independent products; checked descriptions alone do not unblock this.
  Reuse the closed import/export and era/custody contracts in
  `omega-rust/omega/compilation/backend/runtime/component-publication/`; its
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

- **WRITE-ONLY-BORROW.** Complete caller-visible `&write T` mutation under
  [write-only authority](wiki/spec/terminal-psi/structural_access.md#write-only-authority).
  Owners: checked execution, checked-to-lowered publication and native reference
  preparation.

  First carry write-only calls through composed control in
  `execution/terminal_unit/state_graph/`:
  `operation_custody.rs` excludes write-only arguments, and `mod.rs` excludes
  write-only state parameters. A caller invoking
  `forward(&write self.value, 42)` loses its plan at `unit call arguments`;
  an otherwise identical mutable-borrow program executes with correct caller
  writeback. The same comparison holds for `values[position + 1] = value`.
  Preserve loan/restoration evidence through composition, not just admission.

  Remaining: general aggregate and `[copy]` sum replacement beyond scalar-field
  record literals; runtime domain-qualified byte-field replacement with encoding
  evidence. Verify computed IEEE store/readback through FLOAT-PROVIDERS; the
  mutable controls stop in caller guard planning. Checking already admits
  closed copy sums; computed integer selectors and
  scalar-computation store sources already have plans. Reuse them. The
  `write_only` corpus's two passing fixtures only check source.

  Coordinate sequencing with STATE-LOCAL-VALUE-FRONTIER, reference identity with
  STRUCTURAL-BORROW-IDENTITY, mutable dynamic dispatch with FINITE-GENERIC-DISPATCH,
  and broader indexed realization with PLACED-ACCESS-NATIVE-OPS. Use
  CANONICALIZE-SCALAR-RANGE-CONTRACTS' contract/domain bounds.

  Acceptance: runnable `write_only` corpus cases preserve exact widths,
  untouched neighbors, runtime signed/Boolean/floating sources, restoration,
  return custody and independently replayed non-observation. Reject reads,
  bare forwarding, readable widening and overlapping exclusive arguments.
  General replacement preserves displaced custody and whole-value validity.
  Observe original caller storage, including guarded/computed indices, rather
  than copied homes. Run `python3 tools/corpus_gate.py --native --filter write_only`
  on Windows, macOS and both Linux targets; cross-publication is not execution.

- **STRUCTURAL-BORROW-IDENTITY.** Preserve original caller storage through
  projections, call preparation, native placement and independent receiving
  replay under [structural access](wiki/spec/terminal-psi/structural_access.md).
  Reuse shared checked/source argument normalization for the wider placement
  and receiving-replay acceptance below.
  Owners: checked `terminal_scalar` signatures, `receiver_calls` and
  `calls/computation_arguments`; lowered scalar-callee selection; target
  `structural_call_arguments` and image `argument_custody`.

  Complete local-rooted indexed receivers, owned-root/construction-local
  admission and runtime-indexed write-only receivers. Reuse `Field`,
  `FixedIndex`, `RuntimeIndex { index, obligation }` and
  `terminal-semantics::static_path`; coordinate range spelling with
  CANONICALIZE-SCALAR-RANGE-CONTRACTS.

  Keep declared receiver custody independent of field observations and state
  count. After package preparation,
  `omega run --both tests/omega/pass/calls/unread_multistate_receiver_exit/main.omg`
  must return 70 in both paths without a dummy read. Retain shared/mutable,
  indexed and explicit-reference calls, duplicate-overload rejection and
  changed-access rejection as controls.

  Acceptance: execute caller-visible projected/forwarded writes, owned-field
  mutable/write-only subloans, legal synchronized shared observations and
  register/stack reference passing on matching hosts, including Linux AArch64
  and Windows. A callee observing staged bytes does not prove caller writeback.
  Reject copied borrowed homes and substituted access/type/projection/placement
  evidence; shape equality grants neither authority nor a standalone field type.

- **NORMALIZED-ABI-LOWERING.** Execute owned aggregate and descriptor foreign
  arguments/results under the [calling-plan contract](wiki/spec/build/calling_plans.md).
  Owners: Omega stages 02 and 03, backend call-frame emission.

  Stage 02 already retains whole owned arguments from caller parameters or
  affine call-result homes, borrowed descriptors, and scalar formal order.
  Stage 03's `legalization/scalar_graph_input/normalized_foreign.rs::structural_argument_at`
  and `selection/scalar_call_abi/normalized_foreign.rs::validate` only admit
  nonempty field-path borrows placed as one pointer word. Extend these consumers
  through emission using the selected aggregate locations and descriptor words;
  owned indirect placement must not become a semantic borrow.

  Aggregate results stop earlier: stage 02's
  `lower_normalized_foreign_scalar_result` accepts only Unit/scalar results.
  Carry structural result custody through target lowering, legalization,
  selection and emission. Reuse backend `calling-conventions::BoundaryValueGraph`
  classification and ordinary call operands, without signature-specific families.

  Acceptance: an owned whole-place argument and aggregate result execute
  source-to-C on a matching host with exact values, register/stack placement
  and formal order. Each stage independently reconstructs the ABI and rejects
  substituted plans, placements and provider bindings; Terminal Psi remains
  target-neutral. Preserve stage 02's owned/descriptor and affine-home replay
  controls and stage 03's `tests/legalization/normalized_foreign.rs` mixed-argument
  controls; these constructed plans do not establish native execution.
  RESTORE-DYNAMIC-DESCRIPTOR-AND-TABLE-CUSTODY owns dynamic descriptor semantics;
  CALLBACK-PRIVATE-MATERIALIZATION owns private callbacks.

- **OPAQUE-BY-VALUE-BOUNDARY-ABI.** Execute selected opaque argument, result
  and nested-field exchanges under
  [representation agreement](wiki/spec/build/opaque_representations.md),
  building on **NORMALIZED-ABI-LOWERING**'s transport.

  `provider-planning/calling_policy_plans/boundary_signatures.rs::opaque_representation_movement`
  derives exact occurrence paths and placements; its consumers replay plans
  and capture review evidence. Connect these records to provider call-frame
  transport and backend emission. Physical copies preserve one semantic
  occurrence for affine and linear values; only checked semantic copying
  creates another occurrence.

  Acceptance: independently compiled producer/consumer programs exchange exact
  values at runtime. Reconstruct application agreement at every composition
  edge; reject provider, carrier, target, lifecycle, multiplicity and
  historical-selection drift. Equal layout or compact fingerprints are
  insufficient. `package-manager/tests/opaque_boundary_agreement.rs` currently
  checks review compilation, not execution; preserve it alongside
  `review-evidence/tests/calling_policy_opaque.rs`, provider-planning's movement
  replay and installation-record substitution controls.

  Keep sealed `Ptr<T>` TargetSemantics, `EfiSystemTable` and erased proof-only
  `Real` distinct; proof-only use creates no runtime demand. V1 requires
  `Inert` carriers, rejecting direct or nested cleanup ownership.
  COMPONENT-SUBSTRATE owns replacement compatibility and stable-handle eras;
  carry the same application commitments there.

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
  Every named proof-search provision, including the divisor certificate
  search's, declines into `ProofSearchCapacityExhausted`, whose
  `ExceededProvision` publishes the compiler request's `Incomplete` outcome
  (batch row tag 2, `omega` exit 2, `omega run` status 202) under its
  [compiler-request](wiki/spec/build/compiler_request.md) scalar-resource row,
  codes 27 to 34. Source-derived tests distinguish a declined inference
  budget from invalid source. `OMEGA_PROOF_TIMINGS=1` prints each verdict and
  operation search's time, each roster arrival's verdict, and the discovery
  pool left after each producer.

  Remaining work: refused roster verdicts still grow with scope size. On
  macOS ARM64 (debug), `cellular_automaton`'s roster pass takes 239 s of its
  373 s compile and run over 26 proof rounds; its 321 refusals average 3.4 s
  at 300 axioms, 7.1 s at 400 and 13.3 s at 600, and every round re-proves
  each arrival under that round's axioms. Remove that growth rather than
  raising provisions. `tests/bootstrap/ocreq-tables.sh` reports codes 27 to
  34 unprojected in the `bootstrap/5_omega` outcome tables. Complete
  production-path exhaustion controls beyond the existing helper-budget tests,
  retaining independent certificate verification.

  Rebaseline **RC-PCC-REPLAY**'s terminated case and the computed-selector
  customer with `OMEGA_PROOF_TIMINGS=1`, and measure certificate size.
  Acceptance requires completed source checks, accounted bounded work, and
  resource exhaustion reported as `Incomplete` through publication. Invalid
  evidence still rejects; missing evidence cannot authorize acceptance.
  [Kernel budgets](wiki/spec/proofs/kernel_metatheory.md) remain
  implementation policy, not narrower language semantics.

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
    `omega-rust/omega/packages/review-evidence/EVIDENCE_SCHEMA.md`'s
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

  Retain the native `runtime_ranked_accumulator_guarantee_exit`,
  `runtime_two_state_accumulator_exit`, `runtime_sequential_ranked_components_exit`,
  `runtime_sequential_relational_accumulator_exit`,
  `runtime_ranked_intermediary_accumulator_exit`,
  `runtime_mutable_ranked_accumulator_exit`,
  `runtime_inductive_climbing_sum_exit`, and `runtime_inductive_gauss_sum_exit`
  controls. Their guarantees survive
  package review, serialization, independent verification,
  and execution. Preserve Wrapping's equality-only polynomial reasoning and
  each additive ranking step's independently proved no-wrap condition. Wrong
  accumulator arrivals must reject while the same cycle certificate remains
  valid; weakening the decrement guard must reject that cycle certificate.
  Mutable arrivals retain current storage values and exact invocation origins,
  with the same write invalidation and simultaneous substitution as other edges.
  PROOF-KERNEL-CORE and PCC-CANONICAL-SEMANTIC-LEDGER own the licensed
  arithmetic derivation, without per-loop axioms or residue order assumptions.

  Acceptance: source guarantees and arrival evidence survive serialization and
  native execution. A wrong update must fail preservation while the unchanged
  cycle certificate still answers the same termination question; missing arrival
  evidence and unestablished guarantees reject. Retain the native/wrong-step
  corpus pair, `ranked_value_guarantees` and
  `terminal-psi/verification/tests/ranked_scc/scalar_block_invariants.rs` controls.

- **PCC-CANONICAL-SEMANTIC-LEDGER.** Define and justify the total generator
  from canonical Terminal bytes to the semantic ledger under the
  [verification contract](wiki/spec/terminal-psi/verification.md#canonical-semantic-ledger).
  Owner: `terminal-verifier`, `terminal-codec`, `proof-admission`.
  Codec currently reconstructs from a validated `TerminalModule`; its
  `framework:canonical-semantic-ledger-v1` remains explicitly trusted.

  Crash certificates already come from untrusted `certificate_search` and
  supplied `ProofBundle::crash_obligations`; preserve independent obligation
  reconstruction and certificate replay without search. Remaining work:

  - Discharge `ExplicitlyTrusted` reconstruction, normalization, scope,
    invalidation and call/cycle composition with checked evidence and exact
    dependencies. Preserve `trusted_surface.rs`
    dispatch/fact/source-closure checks, `PROVED_ENTRIES` and rejection of
    unfinished dependencies. Labels, checker-token presence and source digests
    do not prove soundness; revalidate justifications before updating digests.
  - Close the cleanup entry-claim gap in `validation/affine_cleanup.rs` and
    scalar cleanup in `validation/frontier.rs`, coordinated with **CML4**.
    A borrowed-`self` Unit hook with structural or content entry claims validates
    and verifies without caller transfer; the equivalent ordinary call rejects
    with `UnitCallClaimPresenceMismatch` or `UnitCallContentClaimMismatch`.
    Reconstruct the edge's claim discharge and exact receiver substitution;
    retain honest cleanup and wrong-claim controls. This raw Terminal probe
    establishes missing validation, not a source-level exploit.
  - Establish the canonical-byte generator's definition and soundness, rather
    than assuming a producer-decoded AST. The common kernel and selected
    inductive profile's soundness/encoding remain dependencies.

  Acceptance: a theorem-dependent program verifies without source or producer
  state under accepting/rejecting assumption policies. Wrong goals, profiles,
  scopes, premises, missing obligations and omitted transitive assumptions
  reject, including after normalization and erasure. Combine the connected
  certificate case with `terminal-psi/verification/tests/trusted_surface.rs` and codec
  trust-graph controls. Native refinement is separate; bootstrap proof discharge
  remains with GAMMA-DERIVATION-CHECKER in `TASKS_BOOTSTRAP.md`.

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

- **BORROW-PROOF-CONVERGENCE.** Publish ordinary borrow-compatibility
  certificates through Terminal with independent replay under
  [loans](wiki/spec/terminal-psi/loans.md). Owners: checked `checks/borrows/`
  and `checked_trees::borrow`, checked-to-lowered publication, Terminal codec
  and verification.

  Preserve implemented requires/domain/guard/proposition and chained-premise
  checking, pinned mutable results, fixed projections, and guarantees from
  statement calls and nested call results. `borrow_certificate_replay` invokes
  checked replay with typed source and facts; it neither lowers nor reloads
  a Terminal artifact.

  Retain exact formation, captured places/resources, establishment and consulted
  premise order. Independently reconstruct availability, dominance and value/place
  versions. Complete remaining callee/domain predicates, runtime projections and
  same-statement/theorem establishment using exact call substitution and existing
  contract/range readers, rather than a second guarantee collector.

  Expand read footprints only for a demonstrated fact loss. The `low + 0u64`
  subslice lead remains unverified: matched plain/computed audit probes reject
  `view[0]` for unknown slice length even before an unrelated write. Distinguish
  builtin bound-meaning admission from fact invalidation.

  Acceptance: source passes for disjoint loans, writes and exclusive call
  operands must retain independently replayed certificates through artifact
  reload. Missing, stale, reordered/substituted or insufficient premises,
  removed/replaced call prerequisites, cross-call copy substitution, overlapping
  mutation and containment-only second exclusive loans reject. Proof cannot
  create, widen, duplicate or extend authority. Preserve `certificates/`,
  `premised_disjoint_writes.rs` and stated/sum/guard/domain/returned-window/
  proposition/chained controls. Route CANARY-CORPUS borrow-obligation failures here.

- **CALLBACK-PRIVATE-MATERIALIZATION.** Complete native realization under
  [private callbacks](wiki/spec/build/private_callbacks.md). Ordinary Psi
  production and Omega's `callback_settlement` already validate requirement
  selection and inbound interfaces. Extend native callback bodies, call/import
  closures and signatures beyond `machine-emission/private_callback`'s single
  emitted scalar-ABI function and image private-function replay's custody limits;
  use ordinary lowering with ABI and call evidence, not body recognizers.

  Remaining: authenticate complete plan applications and independently replay
  authored-use-to-Terminal-operation correspondence; preserve callback custody
  through optimization; verify ordinary product publication and hosted
  private-stack integration. Owners: Omega's callback settlement, native
  `callback_thunks`, machine emission, selected-call ABI transport and image replay.

  Preserve `callback_terminal_custody`'s direct relocation and two-slot registrar
  witnesses: both now construct Windows x64 images, check final addresses and
  replay installation records. The two-slot harness supplies import/stack
  evidence; neither executes callbacks. Extend acceptance to general bodies and
  inbound signatures, binding exact function, symbol, relocation, executable
  region and direct/private-field destination through object/final/installation
  replay. Missing, duplicate, reordered, substituted, overlapping or incompatible
  materializations reject. Private slots remain inaccessible to source; preserve
  the package-local calling-vocabulary duplicate rejection. Registration outcome
  and lifetime belong to REGISTERED-CALLBACK-LIFETIME.

- **REGISTERED-CALLBACK-LIFETIME.** Execute an authored registrar lifecycle
  under [registration and lifetime](wiki/spec/build/private_callbacks.md#registration-and-lifetime)
  and [opaque retention](wiki/spec/build/component_publication.md#opaque-retention-and-quarantine).
  Carry the success/rejection sum through checked result planning, Terminal
  case-conditional qualifications/claims and installed-provider interpretation.
  `terminal-interpreter/src/calls.rs` still rejects projected result qualifications
  and affine results carrying claims/qualifications. Reuse the verifier's
  `structural/qualification_rosters.rs` and interpreter `live_claims`; a valid
  roster must come from authored evidence, not merely removal of the gate.

  Join success to the exact live-registration capacity, external root and
  code/component leases through `component-publication/src/callback_registration.rs`.
  Rejection creates no root and preserves capacity. Unregister and quiescence
  precede lease release and return that same capacity occurrence; capacity counts
  live registrations, not thunks. Use ordinary custody rules.

  Acceptance: an authored provider/customer rejects, retries, replaces with
  returned capacity and unregisters; dropped, repeated, stale/cross-occurrence
  registrations, spent-capacity reuse and wrong qualification rosters reject.
  Preserve `05_checked-trees-to-lowered-psi/tests/registered_callback_lifetime.rs`:
  routed-payload checks and interpreted bare-registration round trips exist,
  but the installed-provider witness injects conformance records and does not
  close this lifecycle. CALLBACK-PRIVATE-MATERIALIZATION supplies the native
  entry; FFIVAL owns Windows foreign invocation, unavailable on other hosts.

- **FOREIGN-RETAINED-ARGUMENT-BACKING.** Connect authored retention to native
  argument backing under [outbound custody](wiki/spec/build/foreign_storage.md#outbound-custody).
  Owners: checked content/call planning, checked-to-lowered retention, Terminal
  boundary verification, native marshaling and `external-roots`'s
  `program_local/program_local_extents/retained_foreign_arguments.rs`.

  Shared-borrow declaration transport and invoked-call verification exist;
  `retain_foreign_argument_under_custody` still has only test callers. Complete
  the authored source-to-Terminal-to-native connection, including installed-provider
  custody (`foundation/provider_candidates.rs` currently refuses retained-borrow
  guarantees). Materialize pointers only from exact stable-root, range, access,
  lifetime and revision/lease evidence. Reuse result-`ensures`, nominal matching
  and structural-result catalogs.

  Add explicit snapshot permission, private backing and persistent demand with
  capacity per live occurrence; copying implies neither identity preservation
  nor write-back. The snapshot registry helper is test-only. Implement moved
  backing through CONSERVATION-CONTRACT / TERMINAL-CONTENT-CLAIMS; no moved
  registry implementation remains. Authored custody must select each disposition.

  Acceptance: moved, lifetime-borrowed and permitted-snapshot boundaries run
  natively; completion redeems moved backing and live shared loans exclude
  conflicting writes. Call-scoped, mutable lifetime-borrowed or ambiguous
  retention and unknown/stale/out-of-range/excess-rights backing reject. Preserve
  `retained_content_custody` and `retention/retained_borrow_custody` controls;
  their declaration and constructed-Terminal witnesses do not prove native
  calls. Registration leases remain REGISTERED-CALLBACK-LIFETIME.

- **TERMINAL-SLICE-VIEW-VOCABULARY.** (split-of:SLICE-VIEW-LOCAL-ENTRY-ESTABLISHMENT)
  Complete consuming non-byte `&[T]` calls and state transfers through Terminal
  production and native execution under
  [view custody](wiki/spec/terminal-psi/byte_views.md#element-typed-slice-views).
  Preserve exact backing, element identity, extent and loans through length,
  indexing and subslicing. `slices/callee_non_byte_view_len_index_subslice`
  exercises all three through its `Main::run` build entry and exits 9 natively
  on macOS ARM64 with checked-interpreter agreement;
  `slices/runtime_subslice_and_element_read_call_operands_exit` and its Unit
  state twin `runtime_unit_state_subslice_and_read_call_operands_exit` (exit
  42 each) pass a subslice and an element read of one view to one call in both
  authored orders.

  Reuse stage 04's `execution/terminal_unit/calls/view_subslice.rs` and stage
  05's `unit/attached_unit/view_ranges.rs`, `emission/operation_emission/view_subslice.rs`
  and `tests/view_locals.rs`. They share `CheckedSubsliceSite` and established
  view roots. Element-read paths now reach Omega's target/legalized operations;
  `runtime_slice_index_read_dispatch_exit` and `runtime_slice_record_field_offset_exit`
  record macOS ARM64 exits 43/61. Preserve whole-element stride, leaf offset/width
  and independent replay. Signed indices already use `exact_u64_index`'s
  proof-checked conversion; `runtime_slice_indexed_read_exit` records exit 70.

  Keep `calls/runtime_dispatch_slice_element_terminal_exit` and
  `calls/runtime_dispatch_machine_array_slice_arg_exit` as native exit-70
  controls for caller-supplied view bounds through guarded state transfers.
  The shared all-arrival descriptor check retains both the dominating length
  observation and any independently established backing extent.
  `slices/runtime_slice_iteration_exit` remains the exit-91 loop control.
  At `a822939054ad` on macOS ARM64, both unchanged dispatch programs execute
  with exit 70 through `omega run <fixture>/main.omg`; all five slice controls
  retain their authored exits and all 11 `fail/slices` fixtures reject.
  Native acceptance is recorded. Recheck these controls with
  `python3 tools/corpus_gate.py --native --filter runtime_dispatch_slice_element_terminal_exit,runtime_dispatch_machine_array_slice_arg_exit,runtime_slice_iteration_exit`
  on macOS ARM64 while completing the remaining joins below.

  With the view-arrival repair on `5fb2e19e1f1` on macOS ARM64, the join also
  runs `slices/runtime_byte_view_guarded_state_contract_exit` and the unchanged
  wrong-era and nested/repeated-field wire fixtures with exit 70. Both dispatch
  controls retain exit 70, the iteration control retains exit 91, and all 11
  `fail/slices` fixtures reject. Byte and element readers share the observation
  rule; structural state arguments substitute by exact root/projection, not type
  equality. Existing backing/arrival equations retain precedence over the added
  descriptor alias in first-definition inference. Other wire customers and
  interpreter agreement remain under **RECORD-LITERAL-ARRAY-FIELD**.

  At `4202376a2d`, including literal-custody repair `eb36ac8f6a`, macOS ARM64
  `python3 tools/corpus_gate.py --both --golden tests/omega/corpus_native_macos_arm64.txt
  --filter domained_runtime_end_subslice_exit,runtime_bounded_fixed_array_subslice_arg_exit,dependent/runtime_sibling_len_index_exit`
  observes native/interpreter exits 70, 3 and 7. The domain-qualified runtime
  window preserves `no_nul`; literal descriptors retain shared-borrow custody,
  and call validation independently reconstructs their content predicates.
  The latter two fixtures already execute on `11e21a0895e`.
  The public sibling-index command
  remains pending ordinary package review (exit 200), separate from these
  reviewed-corpus execution results.

  Remaining joins:
  - Complete range-qualified scalar and erased-field coverage for `[copy]`
    view elements under [ownership](wiki/spec/language/ownership.md).
    Do not infer an implementation gap from `view_element_copy`'s scalar-only
    reconstruction: its sibling `copied_place` already retains whole structural
    contents through `StructuralLeafCopy`. At `dd19299af7d` on macOS ARM64,
    `python3 tools/corpus_gate.py --both --golden tests/omega/corpus_native_macos_arm64.txt
    --filter slices/runtime_qualified_record_copy_local_exit` executes with
    native/interpreter exit **70**. A slice element containing bounded owned
    text and an enum copies through a local into independent destination storage;
    replacing the source afterward preserves the copied bytes and untouched
    siblings. This coverage needs no compiler change. The corresponding
    `omega run --both tests/omega/pass/slices/runtime_qualified_record_copy_local_exit/main.omg`
    compiles for fresh package review but remains pending ordinary acceptance
    (exit 200); the fixture policy does not supply that acceptance.
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
  [outcome bounds](omega-rust/psi/compilation/representations/terminal-psi/fuel/certification/src/fuel_certification/outcome_bounds.rs):
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
    and `OpenCalleeSet`. `omega-rust/omega/application/cli/src/inspection/evidence.rs` still treats the
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
    [native-artifact's proof owner](omega-rust/omega/compilation/backend/artifacts/native-artifact/src/proof.rs).
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
  [native-artifact's receiver wrapper](omega-rust/omega/compilation/backend/artifacts/native-artifact/src/fragment_object_publication.rs).
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
  `realization/selected-dispatch/src/compiler_intrinsic.rs`.
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

- **ADDRESS-TRANSLATION-CANARY.** Execute Cathedral-owned mapping installation
  and teardown under [mapping and reclamation](wiki/spec/resources/extents.md#mapping-and-reclamation).
  `tests/omega/pass/memory/address_translation_canary` checks owned single-leaf
  install/remove routes, but its empty entry and bodyless hardware/receipt
  crossings do not execute translations. Join selected authority, hardware and
  shootdown providers to `extents::mapping`'s exact source/destination, identity,
  era and activation/release receipts.

  Complete borrowed-carrier transport: calls forwarding `BorrowedMapping in Pending`
  or `Installed` to checked identity helpers cannot prove the required
  `held_source in Extent::Granted`. Full borrowed installers also
  need unambiguous returned-loan provenance and correct table-result ownership.
  Extend installation to multi-page ranked loops through
  TERMINATION-RANKING-CHECKS; demand-grown tables need BUMP-ALLOCATOR-CANARY,
  placed read-back PLAN-LAID-VIEWS, and range migration
  CANONICALIZE-SCALAR-RANGE-CONTRACTS.

  Reuse Cathedral's numeric table/walk oracles and verify this canary's geometry
  and level-specific leaf/link encoding. The corrected `FRAME_MASK` is
  `0x000FFFFFFFFFF000` (bits 12..51);
  source checking alone does not test its behavior. Integrate
  UEFI-PHYSICAL-SEMANTIC-ENTRY / UEFI-OS-HANDOFF with Cathedral's boot harness.

  Acceptance: QEMU installs and tears down multi-page mappings with explicit
  Extent/TLB custody. Access exists only after activation and before unmap;
  reuse waits for completion. Source use after map, premature unmap, lost
  shootdown, forged carriers, borrowed-source reclaim, replayed activation and
  wrong-mapping/stale-era receipts reject. Retain the surviving
  `fail/core/translation_unmap_before_activation` control and cover the remaining
  cases. Rust receipt tests do not establish execution. Cathedral owns formats,
  walks and lifecycle; no compiler-owned page-table/TLB types.

- **EXCEPTION-ROOTS-AND-TIMER.** Execute Cathedral fatal entries on dedicated
  critical stacks and a timer that acknowledges, records and wakes work under
  [interrupt obligations](wiki/spec/build/interrupt_obligations.md),
  [hardware materialization](wiki/spec/build/hardware_materialization.md) and
  [final machine-state evidence](wiki/spec/build/machine_state_evidence.md).
  `tests/omega/pass/memory/interrupt_table_canary` retains authored roots,
  layouts and validators, but its entry is empty and both handlers only
  acknowledge. The former compiler layout-test witness is gone; current stub
  and ledger tests use supplied coordinates, bytes and receipts.

  Join `machine-emission/src/entry_exit_stub.rs`'s emitted stub and member-call
  relocation to image placement, installation and external-root admission.
  `X86_64DeriverStubEntryEmission` has only test producers. Bind exact entry/body
  coordinates, bytes, stack overhead and final transitive state; keep stack,
  fuel and state columns separate. Unsupported ABI/backing rejects.

  Finish authored table publication through the package validator/member verdict
  and generic writer. Remove remaining compiler-owned gate/IST policy from
  `external-roots`'s `InterruptTableGateDescriptor` and `InterruptTableProfile`,
  preserving exact content, installed-root, publication-authority and checked-`lidt`
  joins. Cathedral owns coverage, controller protocols and fatal behavior;
  compiler mechanisms retain root custody and instruction contracts.

  Implement fatal halt, timer/controller setup, tick recording and wake behavior.
  BOUNDED-INSTALLATION-REACH-ROWS / TOP-LEVEL-BOUNDARY-REQUIREMENTS own exact
  acknowledgement lineage; inherited requirements retain their selected plans.
  Integrate UEFI-PHYSICAL-SEMANTIC-ENTRY / UEFI-OS-HANDOFF and TR3-TR8 provisioning
  with Cathedral's QEMU harness. Keep interrupts disabled until the exception
  floor exists; record firmware/emulator prerequisites, unavailable hosts and
  NMI/machine-check/physical-failure assumptions.

  Acceptance: QEMU reports ticks through owned output, halts between ticks and
  routes a deliberate fault to its dedicated-stack fatal handler without
  resuming ordinary work. Missing members, wrong writer/realization, changed
  bytes, replayed publication/acknowledgement, forgotten/double completion and
  retirement while the table names a root reject. Uninstalled stubs and ledger
  tests do not close this execution witness.

- **BOUNDED-INSTALLATION-REACH-ROWS.** Complete
  [installation-bound reach](wiki/spec/build/external_roots.md#installation-bound-reach)
  for the shipped `source/library/core/interrupt.omg` acknowledgement.
  `InterruptAcknowledgement::complete` still fixes `reaches PortIo`; migrate it
  to `reaches <= MachineControl + PortIo` with a selected satisfier that
  consumes the real linear receiver. **TOP-LEVEL-BOUNDARY-REQUIREMENTS** owns
  selected dispatch/discharge; reuse its owned-receiver route.

  Reuse `omega-rust/omega/compilation/realization/provider-planning/src/provider_planning/installation_reach.rs`
  and external-roots' `root_entry/root_validation.rs`. Provider-planning's
  `tests/{provider_derivation,independent_components}.rs` already cover selected
  reach and unresolved provider/component rejection. Full component deployment
  remains with **COMPONENT-SUBSTRATE**.
  External-roots' `tests/interrupt_entries.rs` checks installed completion
  coordinates using constructed provider facts; it does not connect the shipped
  opaque carrier through source and Terminal Psi.

  Acceptance: PIC completion selects `PortIo`, LAPIC/x2APIC `MachineControl`,
  through the shipped requirement, Terminal Psi and
  `InstalledInterruptCompletionRoute`. Preserve complete nested substitution,
  including service unions across multiple resolution passes. Cross-provider
  settlement despite equal rows, replayed token/era, unresolved Independent
  exports and any unresolved final-admission row reject. Conservative bounds
  grant no authority; exact provider execution, policy and token lineage do.

## Parallel language and compiler lanes

- **IMPLICIT-STORE-CONVERSION.** Finish diagnostic cleanup and migration
  verification under [numeric carrier rules](wiki/spec/language/numeric_values.md)
  and [explicit coercion/erasure](wiki/spec/language/domains.md#exact-coercion-and-erasure).
  `omega-rust/psi/compilation/checking/src/proof_contracts/arithmetic_domains/assignments.rs`'s
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
  product/parser-gate CLI coverage in `omega-rust/omega/application/cli/tests/command_line.rs`.
  Preserve the algorithms and the policy of each operation when inserting casts.
  **SAMPLE-CORPUS** and **RC-BUILD-AND-PACKAGES** retain complete program acceptance.

  Keep the float-policy customers with **OPERATOR-BOUNDARY-CALLS** /
  **FLOAT-PROVIDERS**: `tests/omega/pass/float/{float_saturating_arithmetic_exit,float_trapping_overflow_traps,float_trapping_propagated_nan_traps,named_provider_multiply_then_add_exit}`.
  Their recorded argument-conversion refusals overlap named-operator admission;
  its weakening checker retains arithmetic policy while its numeric argument
  checker compares the bare parameter policy. Reconcile ordinary selected
  contracts and realization without erasing Saturating/Trapping to silence the
  diagnostic. Preserve two-rounding multiply-then-add, saturation and trap oracles.

  Stores now take an unranged call result's carrier from its result type
  (`proof_contracts/arithmetic_domains/assignments.rs` `stored_value_carrier`),
  so `enemy.max_health = 6 + self.rng.range(random, 3)` into an
  `i32 in Wrapping` field rejects. Binary operands still do not: operand
  analysis keeps the first typed operand's carrier and treats a plain call
  result's policy as neutral, so `i32 in Wrapping` plus a `u32` call result, and
  `u32 in Wrapping` plus an Exact `u32` call result, both check;
  `samples/cli/games/dungeon_crawler_cli` now spells both. Reject that mixing
  at operators using the same result-type carrier, without giving range
  analysis full-range call primitives.

  Acceptance: fail fixtures `tests/omega/fail/arithmetic/implicit_store_{conversion,widening,call_result_conversion}_rejected`
  retain actionable checking errors; explicit exact casts, Wrapping whole-array
  literals and `tests/omega/pass/slices/runtime_saturating_array_element_guard_exit`
  retain their valid behavior. Migrated negative fixtures
  must still reach the intended obligation, as in
  `argument_widening_does_not_reinterpret_saved_wrapping_computations` under
  `omega-rust/psi/compilation/pipeline/04_typed-trees-to-checked-trees/src/checks/contracts/call_bounds/tests/widening.rs`.
  Every ordinary cross-carrier/policy store in maintained pass fixtures and samples
  must spell its required conversion. Complete diagnostic and migration evidence
  without treating checked admission or a later lowering refusal as native success.

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
    `folded_index_projection::folded_index_selects_the_literal_element`
    pins literal and folded indices to the same verified element move; do not
    reopen folded-index planning.
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
  - Extend `operators/call_binding.rs` beyond binary/indexed
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
  `select_false` produces `1` without invoking the operation; its Console-bound
  `Main` exits 70 natively on macOS ARM64 with checked-interpreter agreement.
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
    in package source preparation and compiler handoff.
    `compilation-inputs/src/source_imports/frontier.rs` still leaves loose roots
    without package identity;
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
    acceptance. For the same reason `fail/build/package_local_calling_copy`
    now checks on the corpus route: its package-local `calling` module no
    longer collides with std's, while `callback_terminal_custody`'s
    loose-root Rust test still rejects the copy as `duplicate data`. Decide
    what that negative should pin once the loose route carries owners (for
    example, a policy built from the copied types must not select as
    `std::calling`). Local identity repair need not await the versioned
    [standalone-request extension](wiki/spec/build/compiler_request.md) shared
    with OMEGA-D/OMEGA-C; named-only OCREQ v1 cannot silently encode anonymous owners.
  - Finish typed/checked/Terminal use of indexed constraints and operator homes.
    Extend existing normalization and
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
  and `runtime_foreign_generic_scoped_constant_exit` (foreign generic scoped
  constant, native/interpreted exit 0), plus `runtime_qualified_static_mutation_exit`
  (ordinary module-qualified mutation
  and readback, native/interpreted exit 70), plus the `modules/` corpus gate,
  then verify source-free publication/reload,
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
  execution. `runtime_forwarded_value_contract_exit` now forwards a runtime
  binder through a nested required call and checks the returned 3; native and
  checked interpretation both exit 0 on macOS ARM64 with `omega run --both
  tests/omega/pass/generics/runtime_forwarded_value_contract_exit/main.omg`.
  Checking distinguishes type selections from forwarded value/const binders by
  declared kind. Mixed type/value scalar calls retain their separate bindings;
  forwarding a generic result type still exposes an unsubstituted callee type
  in the caller's inferred hoist, so result transport remains unfinished.
  The corpus canaries bind four hosted targets; verify each host's
  native outcome separately. No hidden boxing, maximum-range stack reservation,
  runtime per-value code generation or specialization inferred from integer
  finiteness.

- **STRUCTURAL-GENERIC-MATCHING.** Complete
  [static type equality](wiki/spec/language/generics.md#static-type-equality),
  [structural equations](wiki/spec/language/generics.md#structural-type-equations-and-inference)
  and [canonical domain indices](wiki/spec/language/generics.md#canonical-domain-index-matching)
  through specialization, layout and source-free execution.

  `generics/omitted_data_binder_range_equation` now infers Capacity = 256
  from `u64::AtMost<256>` before layout, constructs a nested `DomainBytes`
  backing, and writes/reads element 255. Its bound native entry and checked
  interpreter both exit 92 with `omega run --both
  tests/omega/pass/generics/omitted_data_binder_range_equation/main.omg`.
  Serialized Terminal reload verifies without typed source; changing the
  retained fixed index to 256 is rejected. The invalid-initializer control
  rejects a length of 257. Closed integer domain indices retain their exact
  instance identities through scalar interval proof and qualification replay.
  Next extend this customer to the remaining matching routes below;
  scalar return-domain inference from a typed parameter remains separate
  from the now-running literal mint and nested backing route.
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
    execution. `provider-planning/src/provider_settlement/canonical_filesystem_host.rs`
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
    `omega-rust/omega/packages/review-evidence/src/capture/authority.rs`, dependent
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
  `00_terminal-psi-to-abstract-operations/src/provider_admission/terminal_authority_policy/filesystem.rs`
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

  Owner: `omega-rust/psi/compilation/checking/src/machine_calls/calls/write_frames/`, consumed
  by stage 04's flow mutation and range/progress checks. `CallFrameResolver`
  already shares call/expression summaries and transition fixed points.
  Stage 04's `src/tests/termination/write_frame_aggregate_results.rs` reaches
  checked trees for nested helpers, named-state results and producer writes;
  `write_frame_boundary_arguments.rs` checks disjoint/overlapping fact use.
  Reuse those paths.

  Complete the source-admission and fact-use joins still represented by
  resolver-only probes in `write_frame_candidate_origins.rs`,
  `write_frame_contextual_cases.rs`, `write_frame_carrier_results.rs` and
  `omega-rust/psi/compilation/checking/tests/wire_codec_write_frames.rs`. Proven candidate
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

  Owner: `omega-rust/psi/compilation/pipeline/04_typed-trees-to-checked-trees`.
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
  `omega-rust/psi/compilation/pipeline/05_checked-trees-to-lowered-psi/tests/reference_result_source.rs`.
  Complete structural-array ingress and residual cleanup through shared evaluation,
  retiring superseded `omega-rust/psi/compilation/checking/src/machine_calls/calls/expression_scanning/result_realization.rs`
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

- **TR3-TR8.** Execute ordinary named machines through a selected runtime
  under [task activation](wiki/spec/build/task_runtime.md) and
  [suspension](wiki/spec/terminal-psi/calls_and_outcomes.md#suspension).
  Reuse `provider-planning/src/task_plans/` and
  [task-plans](omega-rust/omega/compilation/backend/runtime/task-plans/README.md): sealed
  call graphs, argument images, lease accounting and routed `Task<T>` claims
  exist. Ledger park/resume tests change accounting state; they execute no
  worker or native stack switch. `tasks/task_runtime_machine_selection_compile`
  checks a probe beside an empty, unbound `Main::main`.

  - Join final physical frames/spills, alignment, entry overhead and complete
    live chains to the exact `StackPlan` and actual nonmoving backing.
    `stack_graphs.rs::task_frame_summary` measures checked locals and parked
    arguments; `TaskRuntimeAdmission` stores slot shapes, not allocated stacks.
    Preserve unresolved-call refusal and same-stack/separately provisioned demand.
  - Complete suspension production in stage 05's
    `retention/suspension_call_plan.rs`: receiver, persistent/structural places,
    live claims and Unit-call scalar environments/arguments need exact joins.
    Boundary targets also fail canonical identity because checked-trees'
    `facts/carry.rs::symbol_identity` only resolves machine/state symbols;
    preserve `boundary_call_suspension_frontier_fails_closed_on_target_identity`
    until that join exists. Retain storage lifetime, pinning, aliasing, address
    stability and cancellation-outcome evidence through encoding and independent
    checking; unsupported frontiers reject. Preserve the verifier's
    `suspension_call_plan_rejects_coordinated_site_and_plan_deletion` control.
  - Connect one source-selected provider to argument installation, distinct
    activation, checked park/resume and actual returned outcomes. Bind its exact
    selection, invocation receipt, runtime instance, storage era and source task
    route. CPU/thread preservation remains an explicit obligation.

  Acceptance: concurrent observable work with moved arguments, start/park/resume/
  finish and fresh eras on reuse. Rejection returns caller arguments/leases;
  foreign/stale settlement and reclaim with child custody reject. Parking
  establishes no result/cleanup; resumption repeats no committed work.
  Cancellation requires a recorded request observed at a declared safe point,
  never arbitrary unwinding. Runtime custody, backing ownership and linear
  settlement authority stay distinct; continuations remain source-inaccessible.

- **BLOCKEXEC.** Implement `source/library/blocking-executor/` as an ordinary
  package under [task-runtime libraries](wiki/spec/build/task_runtime.md#library-and-foreign-providers)
  and [foreign execution](wiki/spec/build/foreign_bindings.md#operational-coherence).
  Its queue/executor files contain contracts, not working bodies.
  `blockexec/blocking_executor_custody_claims_compile` duplicates concrete
  declarations, supplies a pre-existing ticket to `submit`, and settles against
  a wait word instead of the executor borrow. Its bound empty entry builds
  without invoking the probe or consuming the package; that establishes neither
  FIFO execution nor claim issuance. `blocking_executor_ticket_scope_loss`
  checks ordinary linear scope loss.

  - Implement generic `[T; N]` queue bodies with indexed occupancy,
    head/length/capacity proofs and conserved queued payloads. `N` is const:
    runtime value-argument support is not a prerequisite. Exercise combined
    type/const attached methods through ordinary lowering, not an executor-only
    path. Use **BORROWED-STORAGE-RESTORATION** where linear slot moves require
    restoring the borrowed container; no blanket attached-method blocker.
  - Implement accepted/refused admission and exactly-once settlement using
    ordinary conserved-claim/outcome evidence. Acceptance issues a claim bound
    to the actual executor instance; refusal returns the
    complete moved submission. Close requires an empty queue and no live claims.
  - Connect worker and wait/wake providers to **TR3-TR8**. Preserve thread
    affinity and actual loans across suspension, including settlement's executor
    borrow and instance relationship. Cancellation requests retain the claim.
  - Implement bounded hung-worker recovery with real process isolation, exact parent
    endpoint/child manifest, custody and termination evidence. The empty
    `IsolatedWorkerProvider` marker proves none of this; never kill an in-process
    worker to reclaim custody.

  Acceptance: consume the real package through dependency/build selection and
  execute linear-payload queueing, full-capacity refusal, FIFO delivery, park/wake
  and settlement. Reject duplicate/foreign/stale tickets, lost custody and early
  close/reclaim. Witness isolated hung-worker recovery separately.

- **QUOTIENT-RUNTIME-REALIZATION.** Execute admitted quotient value/call
  operations under the [representative realization contract](wiki/spec/proofs/quotients.md#representative-layer-and-executable-realization).
  Start with the existing bounded correspondence forms; broader
  **QUOTIENT-THEOREM-LIFT** generalization is not a prerequisite. Follow
  [ordinary checked-body elaboration](wiki/spec/proofs/quotients.md#execution-through-ordinary-checked-bodies):
  preserve public operation identity and checked correspondence while lowering
  representative calls and authored adaptations through the normal pipeline.
  Any generated wrapper uses a stable derived identity and ordinary machine
  machinery; correctness cannot depend on inlining. No backend-specific quotient
  engine or executable transport-theorem witness.

  - Admit representable, custody-free carriers under an exact realization
    judgment. Validation's `proof_contracts/quotients/formation_collection.rs`
    currently requires a proof-only carrier: replacing the managed canaries'
    recursive `Representative` with `data Representative [copy] { value: i32; }`
    rejects with "has a runtime layout; quotient carriers must be proof-only".
    Preserve the separate `carrier_fence.rs` rejection of affine/linear content
    and routed custody; relaxing that would identify distinct authorities.
  - Carry the licensed representative ABI through construction, arguments,
    results, ordinary calls, Terminal operations and native realization.
    The generic `quotient_operation` exclusions concern sealed proof requests;
    do not turn those requests into ordinary runtime calls by deleting guards.
    Stage 05 retains proof-only correspondence, and the verifier's
    `validation/quotient_correspondence.rs` rejects every nonempty table with
    `NonExecutableQuotientCorrespondence`. Separate validated proof-only metadata
    from runtime-relevant demand: mere presence must not reject an otherwise
    executable module. Demanded operations still require complete checked
    representation/operation evidence; do not merely disable validation.
  - Add separate invoking customers with observable results. The managed
    `proofs/quotient_{define,lift}_managed_compile` fixtures use recursive
    proof-only data and empty entries; their declaration-only path reaches the
    Terminal fence. Calling them from runtime correctly rejects proof-only
    values. Keep those admission fixtures and fact-only use free of calls,
    dictionaries and fuel charges. Axiomatic operations supply no algorithm.

  Acceptance: an executable root coexists with valid undemanded proof-only
  quotient metadata; demanding an unrealizable operation rejects. Direct
  `define` and transport-backed `lift` execute from verified
  source-free artifacts on matching native hosts. Preserve representative ABI
  and allowed uncanonicalized constant materialization without structural
  observers or illicit representative extraction. Independently reject substituted
  arguments/results/relations, missing correspondence and unsupported realization.
  Pair a quotient operation with its representative theorem under a
  quotient-refusing policy; **PROOF-CONTRACT-MIGRATION**'s transitive helper
  type/statement assumption closure gates that control, not initial execution.

- **QUOTIENT-THEOREM-LIFT.** Complete quotient correspondence publication
  under [lifting operations](wiki/spec/proofs/quotients.md#lifting-operations)
  and [published correspondence](wiki/spec/proofs/quotients.md#published-quotient-correspondence).
  Validation's `proof_contracts/quotients/relation_plan/terminal_bridge.rs`
  already retains transport-lift parameter selection/permutation/repetition/
  omission, immutable result aliases and finite state forwarding. The codec and
  independent replay support those rows; package review also projects forwarded
  results. Reuse these mechanisms rather than reassigning them as missing.

  - Extend canonical evidence for closed literal arguments, preconditioned
    `define`, result computation and generic applications. Local correspondence
    already judges bounded literals, but `canonical_lift_runtime_positions`
    has only public-parameter positions. The bridge requires empty static
    applications and representative/theorem single states, and refuses `define`
    preconditions. Preserve complete applications, relations, contract implications,
    normal-result coverage and ordered theorem roles through decode/reconstruction.
    Replace incidental source-shape gates with the normalized judgments.
  - Complete package projection for the broader applications and private
    dependencies under visibility rules. Private source operations already check;
    `review-evidence/src/capture/quotients.rs` rejects private operation rows.
    Extractor/codec tests with supplied termination facts are not ordinary
    package-check acceptance.
  - Preserve checked termination, purity, crash freedom and hermetic identities;
    selected theorems remain bodyful and assumption-free through helper
    types/statements. **PROOF-CONTRACT-MIGRATION** owns general transitive
    assumption closure and the congruence-only `lift<F, Congruence>` payload;
    coordinate codec, verifier and package review without a second wire form.

  Acceptance: package-backed `define` and transport-backed `lift` pass ordinary
  `omega --check` and publish the broader independently reconstructible rows.
  Reject implicit lifts, missing/surplus/reversed roles, invalid laws, unproved
  termination, non-hermetic identities, admitted/boundary theorem closures and
  custody-bearing quotients. **QUOTIENT-RUNTIME-REALIZATION** owns execution;
  the existing managed canaries still stop at Terminal's execution fence.

- **PRIVILEGED-PORT-EFFECT-SETTLEMENTS.** Produce checked adapter port I/O
  through ordinary native lowering and
  [consumer-owned settlement](wiki/spec/terminal-psi/boundary_calls.md#consumer-owned-settlement).
  Use checked `asm`; evaluated PE/ELF/Mach-O locator custody is separate.

  Start with an installed adapter owning the exact selected `PortIo` requirement
  and closed `out`. Checked planning retains only exact `u16`/`u8` operands;
  runtime operands need their own checked carrier. `TargetUnitOperation::PortWrite`
  and x86 encodings exist, but stage 02's `lowering/control_flow/operations.rs`
  has no production arm; stage 03 scalar-graph admission also refuses the
  family. Carry service/port/value and operation identity through selection,
  register homes and post-allocation emission. Image-emission's
  `function_fragments/production.rs` emits an empty port roster and its
  `validation.rs` rejects nonempty rosters; implement both production and replay.

  Reuse native-artifact's `physical/derivation/provider_custody.rs` readers
  for the preceding effect/zero-byte settlement join, ordinal and byte
  span across machine, object, final image and installation evidence.
  `asm_port_out_final_validation` is a direct root rejected for missing
  provider custody, not a positive selected-adapter witness. Preserve that
  refusal. `asm_runtime_port_msr_final_validation` also needs checked runtime
  operands and actual selected bindings; its machine-control gaps remain
  separate. Port read (`in`) still needs its checked/Terminal operation and
  producer route; `DirectPortReadU8` metadata is insufficient.

  Acceptance: an authored selected adapter emits independently validated native
  effects. Reject missing, duplicate, substituted, role-swapped or unconsumed
  records, changed service/port/value/target, detached spans and altered bytes;
  preserve unsupported-target refusal. Verify emitted bytes separately from
  execution in an admitted privileged environment. Hosted tests must not execute
  raw port I/O.

- **FLOAT-PROVIDERS.** Complete the `FloatSemantics` obligations of the actual
  `Float`, `F32`/`F64` and conversion slots under
  [FloatMeaning](wiki/spec/terminal-psi/mathematical_values.md#floatmeaning).
  Closed Meaning arithmetic already reaches artifacts: `add(BINARY32,
  meaning32(1), meaning32(2)) == meaning32(3)` succeeds; replacing `3` with `4`
  rejects. Reuse that route.

  Checked `proof/float_meaning.rs` and validation's
  `float_projection_bindings/semantic_values.rs` accept only Meaning results
  with Format/Meaning operands. Complete Boolean/classification/integer results
  and integer operands such as `from_integer` through their appropriate proof
  carriers and independent Terminal replay. Even closed true `is_finite`,
  `to_i32` and `from_integer` ensures currently stop at unsupported proof
  entailment. Non-Meaning results must not occupy the FloatMeaning carrier.

  Acceptance: actual slot contracts and imported call/operator results retain
  and discharge obligations in produced source-free artifacts, beyond existing
  imported-result binding and roster-custody tests. Preserve exact declaration,
  signature, owner/use-site and argument/result correspondence. Reject false
  claims, lookalike declarations, changed
  operands/formats/results and missing or forged proof evidence. Preserve IEEE
  Boolean comparison separately from structural meaning equality, including
  NaNs and signed zeros. **OPERATOR-BOUNDARY-CALLS** owns executable named
  provider installation and **X86-FMA-PROVIDER-TRANSPORT** native FMA.

- **RESTORE-DYNAMIC-DESCRIPTOR-AND-TABLE-CUSTODY.** Finish native borrowed
  [dynamic dispatch](wiki/spec/terminal-psi/dynamic_dispatch.md), starting with
  a non-entry helper that forwards a descriptor and uses its call result across
  calculations and branches. Stage 05's
  `tests/dynamic_composed_unit/direct_dynamic_units.rs`
  publishes scalar-forwarding artifacts and interprets Unit helper locals/two-hop
  forwarding. Stage 03's `tests/legalization/dynamic_parameter_call.rs` checks
  Linux x86-64 scalar/Unit ABI replay and substitutions; selection still rejects
  `LegalizedScalarInstructionKind::DynamicParameterCall`.

  Carry the ordinary indirect call through selected construction/replay,
  register allocation and ISA emission, then immutable tables, relocations and
  final-image/installation custody. Image-emission's `function_fragments` emits
  empty dynamic rosters and rejects nonempty ones. Preserve the requirement-owned
  erased caller shape, exact instance/table/slot, ABI/clobber/effect/reach and
  receiver-loan identities. Unit calls have no result home; descriptor/result
  storage cannot overlap. Retain complete tables, including zero trap slots.
  Retire superseded forwarded-call recognizers/records without unproved
  devirtualization.

  Complete ordinary checked storage composition: hosted
  `traits/runtime_local_named_dyn_pass_through_exit` stops at checked
  descriptor reassignment (`structural field store: target place`). In stage 05,
  `unit/dynamic_composed_unit/forwarded_helpers.rs` still requires one call,
  immutable scalar locals, matching helper/requirement result carriers, and
  empty contracts/service summaries. Carry additional ordered effects, mutable
  storage and differing results through
  ordinary operation owners with their own call/result and access evidence.

  Acceptance: source-rooted closed conformances publish and independently replay
  on Linux x86-64/AArch64, execute on matching hosts, select distinct implementations
  at runtime, and preserve calculations and writes to the original referent.
  Reject instance/table/slot/ABI/access/relocation/code-span substitutions.
  Preserve interpreter and legalization controls, all three helper extensions,
  and public dynamic returns with opaque producer-private selection; consumers
  cannot directly select private conformances. Entry-receiver provisioning is a
  separate customer dependency; owned erased cleanup remains with
  **CLEANUP-HOOK-SELECTION-AND-ERASED-OWNERSHIP**.

- **TARGET-SEMANTIC-APPLICATIONS.** Complete source-free replay of the
  [target-semantic capsule](wiki/spec/language/evaluation.md#target-semantic-capsule)
  and [application closure](wiki/spec/terminal-psi/boundary_calls.md#operator-applications-and-physical-children).
  Retain sealed, versioned target semantics, exact selected plans,
  implementations and canonical arguments as inputs shared with realization.
  Typed observations must close their address-space/layout/format/profile
  subject without compiler-host facts. Reuse ordinary typed arguments and
  compiler-materialized target data; a new projection needs a concrete customer
  and owner audit, not a general `TargetSemantics::*` source API.

  Reuse selected-provider const folding and
  `provider-planning/src/data_layout_queries.rs`: `size_of`/`align_of`
  already fold retained requests through native layout authority into `u64`
  literals. These checked-program paths do not establish portable capsule
  replay; targetless settlement currently chooses the host.

  Extend package review's `symbolic_boundary_applications` from supplied
  direct-type-binder requests to complete reachable specialization closure,
  including canonical const arguments. Its exact plan rejoin, mapping retention
  and deduplication already support bounded type binders; lifetime/non-type
  producer telescopes still reject. Recheck reach, transitive proof obligations,
  target facts, admission and realization after substitution. A closed review
  demand grants no coverage, Terminal/native or installation authority;
  complete-set composition is not an install/update prerequisite.
  Preserve canonical empty operator applications versus absent boundary-trait
  telescopes. Physical-child binding belongs to TRANSLATION-VALIDATION in
  `TASKS_OPTIMIZER.md`.

  Acceptance: source-free cross-artifact customers reproduce exact application,
  target and complete reachable obligations; missing, extra, stale, substituted
  or unresolved mappings cannot acquire coverage. Preserve provider-free
  evaluation and named/token, field/let/return, alternate-provider,
  distinct-result, false-result and unselected-provider controls. Reject build
  dependency cycles without satisfier guessing. Checked folding or manually
  supplied requests alone do not close acceptance.

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
  builds and exits 21 natively on macOS ARM64; the checked interpreter rejects
  bodyless external calls without a provider. The named-subject customer
  `providers/runtime_foreign_leaf_borrowed_byte_argument_exit` likewise returns
  70 natively but still declines the selected libSystem `_strlen` leaf in the
  checked interpreter; preserve its forwarded and projected borrowed arguments
  when providing admitted interpreter execution. Restore the mixed borrowed-record
  customer with its initialized fields and reused returned value, then execute it
  on supported hosts. The scalar fixture cannot establish that broader route; retain
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

- **BUILD-ADMISSION-CHECKPOINT.** Complete command-level
  [restricted-build acceptance](wiki/spec/packages/acceptance.md#restricted-build-acceptance)
  across install/update/review/resume and consuming compilation.
  `checking/build_continuation.rs::evaluate_frontier_and_continue` already
  joins requests before executing the admitted build; `check_project`,
  `check_locked_sources` and `compile_project` wire retained consent.
  [Existing pass tests](omega-rust/omega/packages/package-manager/src/review/candidate/compilation/tests/restricted_build_grants.rs)
  cover matching, missing, widened and wrong-purpose consent. They do not
  assert a refused-effect filesystem sentinel.

  Supply a restricted-request command customer with the same dependency in
  Build/Product purposes and a generated-source handoff. Manager review uses
  private staging; its empty request sets do not exercise restricted consent.
  Bind requests and real executor grants to package occurrence, dependency
  path, purpose, target/execution profile, operation, logical scope and bounds.
  Product acceptance cannot grant Build effects. **BUILD-TEST-AUTHORITY** owns
  test/group, fixture and selected-provider attribution through this gate.

  Acceptance: initial, added, widened and transitive requests stop before
  effects, expose those coordinates and leave review incomplete. Resume only
  the exact candidate with accepted request meaning and actual executor grants,
  then review generated source. Observe filesystem effects across refusal and
  resume: missing/refused/cross-purpose grants perform no restricted action;
  inspection alone grants nothing. Unchanged requests need no reapproval;
  source changes stay visible and resolution-only updates preserve meaning.
  Interrupted review/publication and stale candidates preserve the prior lock.
  Locks retain decisions, never secrets, host paths or execution authority;
  performed host effects have no implicit rollback.

  Preserve coherent source/authority snapshot replay, purpose-drift rejection and
  authored-before-generated resolution. Captured immutable inputs and bounded
  private staging remain benign; supplied host directories remain restricted.
  No recursive build API, own-build final-component query or hidden
  post-compilation callback. **BUILD-SNAPSHOT-OUTPUTS** owns committed outputs
  and required-output settlement at publication.

- **OPTIONAL-STDLIB-SEMANTIC-BINDINGS.** Remove broad standalone std/alloc
  `Toolchain` provenance from
  `compilation-inputs/src/source_imports/frontier.rs` under the
  [toolchain/library contract](wiki/spec/packages/toolchain.md).
  Package-aware compilation already retains ordinary std ownership and limits
  the magic mount to core. Finish migrating fallback consumers to ordinary
  dependency edges and exact source-byte catalog roles or accepted semantic
  bindings; directory relabeling grants no authority. Reuse the
  [existing roles](omega-rust/omega/packages/compilation-inputs/semantic_bindings.md)
  for Console, host services and target-correct physical entries.

  Prioritize executable host/objc customers, then proof/fail
  consumers and nested member imports such as the interrupt/secondary-processor
  canaries' std calling contracts. Preserve intended execution and diagnostics,
  including malformed build fixtures. Build and product imports require their
  respective edges. Route exposed call, ownership, entry and proof failures to
  their capability owners without blocking independent migrations.
  **UEFI-PHYSICAL-SEMANTIC-ENTRY** owns the wrong-policy canary's earlier
  duplicate-schema rejection. Private core proofs stay private;
  **PROOF-CONTRACT-MIGRATION** and **PROOF-KERNEL-CORE** own the authorized
  proof route.

  Replace the build-file census with an inventory of actual library use.
  `repository_build_declarations.rs` collects only roots with `build.omg`;
  its pass/run std-edge check scans member text for one alias, omits build
  sources and never visits fail-tier imports. Extend coverage to those
  consumers and imports below member directories; respect registered submodule
  ownership. Preserve valid dependency-free/default-build roots: a missing
  build file alone is not migration debt, and std's own ordinary package
  does not need a dependency on itself.

  Acceptance: remaining consumers retain their intended check, rejection or
  execution outcome after broad fallback removal. Removing an edge rejects its
  imports/provider selections; aliases, paths, names and same-spelled substitutes
  confer no authority. Rejoin exact package, declaration, schema and role-required
  selected plan; stale/substituted bindings reject independently of lock replay.
  Preserve `standard_library_package_resolution.rs`'s missing-edge and alias
  controls. Early import rejection suffices; do not weaken assembly to obtain
  a second provider diagnostic. Std remains replaceable, splittable or absent.

- **COMPONENT-SUBSTRATE.** First execute an ordinary two-package call with
  root-selected `Independent` composition under the
  [component publication contract](wiki/spec/build/component_publication.md).
  Then close stable-slot replacement. Runtime packages or Cathedral own
  deployment, acquisition and update policy.

  Package-manager dependency compilation already calls
  `compiler::published_independent_component_description`; the consuming
  compilation runs `component-description::verify_component`. Preserve
  `review_publishes_the_named_component_description_for_an_independent_selection`:
  its consumer entry is empty, so it proves review, not a crossing call.
  `provider-planning/src/provider_settlement/composition_modes.rs` rejects
  Independent before Terminal/native production. Replace that fence only with
  actual independent products carrying symbolic imports/exports, entry/leave
  resources and installation/replacement obligations; never silently fuse.

  Complete entry, outgoing-authority, service, mapping, lease, stack and
  retained-provider inventories, with producer facts, independent reconstruction
  and per-occurrence discharge. Mapping/Lease enum variants already exist.
  Connect `component-candidate::describe_component`'s native stack and
  realization identity to publication; Psi-only descriptions carry no native
  facts. Join deployment/admission/replacement to the verified-description
  contract used by **TOPOLOGY-PLAN-VERIFICATION**, preserving exact artifact,
  profile, authority and resource correspondence. `component-deployment`
  currently joins candidate/installed bytes directly and rejects registries
  retaining live external roots; finish their owned teardown custody.

  Acceptance: the two-package build publishes and consumes its dependency
  description without test-side attachment, emits independent products and
  executes the crossing. Source-free replay covers exact subject/schema/profile,
  all entries and outgoing authority (startup, callbacks, timers, cleanup),
  custody, retained providers and inseparable assumptions. Corrupt/truncated,
  stale/substituted/unrelated, early-frontier, omitted/forged, duplicate
  dependency/plan and unaccepted-assumption cases reject. Descriptions grant
  no callable/install authority; each installation needs fresh admission.

  Reuse `component-publication`'s era-entry and sealed-entry custody.
  Replacement preserves compatible imports/exports and behavior exclusions;
  new calls enter the new era, existing sessions retain theirs. Admit candidate
  resources/profile within the frozen envelope and peak coexistence; specify
  state migration/retention and failure behavior. Retire only after every
  activation, registration,
  returned object and claim has a valid disposition. Unauthorized, incompatible,
  weaker-policy, underprovisioned or premature operations preserve custody on
  rejection. Continuity-free publication may precede old-era drain; stronger
  continuity needs its explicit contract. **BOUNDED-INSTALLATION-REACH-ROWS**
  consumes this closure; **WIRE-RUNTIME-AND-INSTALLATION** owns platform
  invocation and **PSI-COMPONENT-REPLACEMENT** the interpreted customer.

- **FFIVAL.** Run the Windows user32 customer at
  `tests/omega/pending/host/user32_window_procedure_registration` through its
  authored build and the [private-callback protocol](wiki/spec/build/private_callbacks.md).
  `omega --check <fixture>/main.omg` currently rejects the by-reference
  outcome: both `Registered` paths lack `Registration::Live`, and both
  success/rejection payloads remain unestablished linear values. Carry the
  registrar's actual outcome and capacity through ordinary qualification and
  custody rules; existing call-result transport does not establish this out-parameter.

  **REGISTERED-CALLBACK-LIFETIME** owns that lifecycle;
  **CALLBACK-PRIVATE-MATERIALIZATION** owns native callback realization.
  Supply the package-owned user32 adapter, exact ABI/layout and explicit
  per-window context protocol. The fixture declares an abstract boundary;
  its retry branch neither forces rejection nor re-registers returned capacity.
  Complete those observable customer steps.

  Acceptance on Windows: actual registration, synchronous `WM_NCCREATE`,
  dispatched callback entry, deterministic rejection/retry, teardown and
  registration with the returned capacity; quiescence precedes code-lease
  release. No raw callable address, implicit closure or Win32 compiler escape.
  The corpus excludes `pending`; replace the removed watcher with acceptance
  at the appropriate tier. Checking or byte emission cannot close foreign
  invocation; other hosts report that runtime leg unavailable.

- **WIRE-RUNTIME-AND-INSTALLATION.** Execute an authored program through
  an admitted artifact's receiving-provider installation and matching-host
  invocation under [executable installation](wiki/spec/build/executable_installation.md).
  Reuse `executable-installation`'s linear transitions, required-fact gates,
  sealed entries, replacement/quarantine joins and
  `image-emission/src/installed_artifact.rs`'s exact byte/custody binding.

  The `OwnedImageProvider` drivers install, patch, seal and retire resident
  buffers. `call_code` hands back borrowed resident bytes; its entry/replacement
  tests assert those bytes and custody flags, without a physical call.
  Supply the authored/package provider and executor join: exact admitted
  content, placement, audience, installed occurrence, requirement-compatible
  entry and live-root/provider custody through return and retirement.
  Target write/execute and fetch-visibility operations belong to that provider.
  Keep honest `ConventionOnly` support; reject `Unsupported`. File emission
  does not establish receiving installation, and lifecycle states prescribe
  neither source type names nor new grammar.

  Acceptance: reuse one admitted artifact across distinct linear placements;
  materialize, freeze and validate exact final bytes/footprint, install, then
  invoke and observe the authored result on a matching host. Retirement needs
  quiescence, execute removal, restored write authority and required completion
  facts; incomplete drain quarantines. Replacement uses bound admitted
  fragments at declared sites, establishes visibility and suspends writes
  before draining old custody. The current buffer patcher rejects relocated
  fragments; wider support needs the exact target resolver and replay.

  Reject substituted bytes/authority/contracts, transplanted validation,
  double-spent placement, missing facts and unsupported W^X; failed transitions
  return all inputs. Arbitrary bytes-to-code, JIT and numeric-address execution
  remain unsupported. **ADDRESS-EXPOSURE** owns inert address observation;
  **COMPONENT-SUBSTRATE** owns component closure and stable-slot replacement.

- **X86-FMA-PROVIDER-TRANSPORT.** Connect selected x86 FMA plans to exact
  Terminal occurrences, then ordinary target lowering, instruction selection,
  register homes, machine planning and image publication. Checked plan
  associations and the feature-custodied `machine-emission/src/x86_fma.rs`
  encoder exist; native `comparison_settlement.rs` rejects every retained FMA
  without selected occurrence custody. Stage 02 also rejects FMA; stage 03
  admission and function-fragment emission remain missing.

  Preserve exact operation/operand/result and selected-provider identity,
  AVX+FMA3 admission, XMM homes, canonical MXCSR save/install/restore and complete
  physical coverage. Extend `object_artifact/replay/x86_fma.rs`
  beyond its constant-load operand records to ordinary runtime value transport.
  Keep fences until their required evidence replays independently.

  Acceptance: `float/x86_fma_plan_association` publishes and runs on an admitted
  matching host; both formats distinguish fused cancellation from separately
  rounded multiply/add, including repeated FMA using a prior result. Its nested
  call and named-provider installation gaps belong to **OPERATOR-BOUNDARY-CALLS**;
  direct `F64::fused_multiply_add` also lacks a requirement-level Terminal route.
  Include source-evaluated imports with nested foreign-call control custody.
  Reject absent/wrong-profile feature admission, operand/provider substitution
  and corrupted emitted evidence. Report cross-target byte replay separately
  from hardware execution.

- **ASM-INSTRUCTION-CATALOG-EXPANSION.** Carry checked assembly through
  ordinary source-to-native production under [assembly](wiki/spec/language/assembly.md)
  and the [catalog](omega-rust/psi/foundation/language-core/inline_assembly.md).
  Start with `tests/omega/pass/inline_asm`: runtime port/MSR and AArch64
  system-register fixtures lack checked operation plans; constant port output
  reaches native authority review but lacks selected-provider requirement
  custody. **PRIVILEGED-PORT-EFFECT-SETTLEMENTS** owns that adapter integration.

  `04_typed-trees-to-checked-trees/src/execution/terminal_unit/calls/call_operations.rs`
  only specially plans constant `AsmPortOut`. Complete checked plans,
  lowering, Terminal encoding/replay, target realization and final-span/state
  evidence. Reuse ordinary assignments/transitions where exact: `jmp` and
  typed-place `ldr`/`str` already build. Preserve catalog target restrictions
  through that desugaring.

  Restore the deleted inline-assembly and value-intrinsic coverage through
  source-to-byte acceptance for x86 fences, interrupt control, flags,
  MSR/control registers, then pipeline directives/cache and AArch64 system
  registers. Fixtures without entry bindings only check; emitted bytes do not
  prove privileged execution. Retain exact widths, reach, authority, clobbers,
  ordering and modeled exits. Wrong-target, missing-authority, stale-postcondition,
  invalid saved-place, omitted-clobber and substituted-evidence controls reject.

  Extend memory, atomic, barrier-option, cache/TLB and mode-transition families
  only with customers and complete operand/effect contracts. Preserve authorized
  extent/view, bounds, alignment, initialization and permission; numeric addresses
  grant none. Unknown instructions and hidden exits reject. No opcode per spelling
  or separate asm-body recognizer.

  Hosted `port_io` and `interrupt_table` grants stay independent;
  machine-owner authority stays freestanding-only. Helpers cannot hide obligations.
  Keep the terminal-authority inventory/commitment consistent and exercise native
  compilation. **INTERPRETED-ASSEMBLY** owns interpretation independently:
  the checked interpreter's Unit stubs and fabricated flags are not modeled
  execution. Reject unsupported effects; retain only catalog-permitted no-ops.

- **RUNTIME-SIZED-ACTIVATION-CONTRACT.** Execute a source-authored
  [bounded activation claim](wiki/spec/resources/activation_storage.md):
  establish runtime-sized backing within a declared bound, access it, retain
  it across suspension and release in reverse order. Ordinary calls/core
  declarations can expose this mechanism; no new keyword or owner decision
  is required.

  `extents::ActivationClaimLedger` and `compose_claim_bounds` have model
  tests but no compiler consumers. They hold numeric rows, not backing or
  an exact activation identity. Connect ordinary call admission, extent
  evidence and Terminal production/codec/replay to compiler-provisioned
  storage, retaining each site's bound, committed-extent computation,
  provenance, release order and suspension storage role. Charge aligned
  co-live bounds by sum and exclusive branches by maximum; reject open demand
  before establishment. Keep backing nonmoving and materialized addresses stable.

  Acceptance: `committed > bound` returns checked failure, never a trap or
  clamp. Split/merge conserves coverage and lineage. Escaping claims, wrong
  activation/provenance, stale loans, missing/duplicate sites and invalid
  release/suspension custody reject independently. **FRAME-LAYOUT** in
  `TASKS_OPTIMIZER.md` owns final frame, probe, unwind and native replay;
  ordinary fixed-size calls need not wait. Allocation packages may manage held
  backing but cannot mint this grant from an address/length. Runtime generic
  extents, implicit variable-sized locals, provider grants and OS allocators
  do not supply it.

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
  Committed `request.bin`/`components.bin` are regenerated from the fixture's
  own `build_scope_inputs()` boundary-kind encoding and the test is green.
  Its admissions still come from Rust-built Terminal modules and it does not
  exercise the Omega consumer: `source/library/topology/verify.omg` has no
  driver test. Join ordinary component-description production/admission;
  **BUILD-SNAPSHOT-OUTPUTS** owns confined inputs and output publication.
  `wiki/spec/packages/topology.md`'s Acceptance status cites deleted test
  names; refresh it when the consumer leg lands.

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
  commit. Owners: `tools/release/release_record.py`, its tests and the
  [completion contract](wiki/drafts/reference/rust_compiler_completion.md#release-matrix).
  No release record is committed.

  `release_record.py` is the only recorder (schema 3). `run` records one lane
  on its own host. A gate passes only with exit 0, every skip named and
  declared for that gate, nothing filtered, a nonzero executed count, a clean
  checkout and no inherited filter variables; a timeout, unavailable tool or
  `--limit` leaves it open. `close` joins one record per lane and rejects
  missing, duplicated, mixed-commit, stale, mixed-toolchain and wrong-host
  records. It reads closed only when both per-host gates pass on all four
  lanes, every other gate passes wherever it ran and ran somewhere, and each
  lane carries its direct-execution observation. The frontier is four real
  same-commit records and a closed join; none exists.

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
    `omega-rust/omega/compilation/driver/src/native/retained_native_product.rs` must
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
    `provider-planning/src/provider_settlement/canonical_filesystem_host.rs`
    and `canonical_time_host.rs` supply limited Linux x86-64 syscall rows;
    they do not establish Darwin coverage or every host boundary. Darwin
    imports/adapters must retain locator receipts, argument choreography
    (including clock identifiers/selectors), result transport and time
    constants. macOS TimeHost now uses the ordinary reviewed provider in
    `source/library/std/targets/macos_arm64/time_host_impl.omg`: checked
    selectors, calibration and retrying blocking sleep over private libSystem
    imports, with no constant provider kind or name-folding exemption.
    Implementation `dfbd9bc4ec99` is verified with `omega run` on
    `tests/omega/pass/time/runtime_time_elapsed_since_exit/main.omg` and
    `tests/omega/pass/time/runtime_time_host_native_darwin_exit/main.omg`
    exits 70 on macOS AArch64; the latter checks all six operations and at
    least 30 ms of sleep. Remaining host work is Windows/Linux TimeHost
    coverage and filesystem adapters, including real syscall marshalling and
    result transport. Follow
    [toolchain-selected implementations](wiki/spec/build/provider_selection.md#toolchain-selected-checked-implementations),
    retaining exact package/schema/target/body provenance and rejecting
    missing, mismatched and duplicate selections. `ProviderBinding::Import`
    exists and its receipt must commit
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

- **RC-PCC-REPLAY.** Close the [release gate](wiki/drafts/reference/rust_compiler_completion.md#release-matrix):
  valid artifact/`.proof` pairs round-trip without source or producer state;
  hostile/substituted evidence rejects before PCC-required interpretation or
  lowering; ordinary non-PCC output remains independently checked. Run complete
  nextest and doctest selections for `checked-trees-to-lowered-psi`,
  `terminal-codec`, `terminal-verifier`, `terminal-interpreter` and
  `terminal-psi-to-abstract-operations` at the release commit.

  Rebaseline the byte-buffer argument paths, bounded-byte reads/stores/equality
  and interpreter length admission failures observed at `1ed469454e5`.
  Reconcile retained carriers and test expectations with
  [byte views](wiki/spec/terminal-psi/byte_views.md) and
  [structural access](wiki/spec/terminal-psi/structural_access.md):
  `byte_sequence_length::parameter_length_requires_whole_readable_borrowed_view_and_real_contents`
  still rejects bounded-owner length metadata that the contract permits.
  Preserve payload-access restrictions, separate indexed-write bounds from UTF-8
  preservation, and retain sidecar substitution, sealed-subject and pre-execution
  admission controls.

  The full package run terminates; its failures replace the old killed-test
  diagnosis. **C2L-PROOF-SEARCH-BLOWUP-CONTAINMENT** still owns bounded search;
  killing or filtering a test cannot satisfy this gate. Native behavioral
  correspondence remains with **PCC-PRODUCT-PUBLICATION** and
  **PCC-CANONICAL-SEMANTIC-LEDGER**; placed-image digests do not establish it.

The four required native-host rows share the
[completion contract](wiki/drafts/reference/rust_compiler_completion.md#release-matrix)
and [recording procedure](tools/release/README.md#recording-a-runner-row).
At one clean release commit, run unfiltered, unsharded:

- RC-SOURCE-SEMANTICS: `python3 tools/corpus_gate.py`.
- RC-NATIVE-MATRIX: `python3 tools/corpus_gate.py --native`.
- RC-REPRESENTATIVE-PROGRAMS: `mbx run -p omega -- refresh-samples`,
  plus every documented deterministic sample exit/output oracle.

Retain live compiler integration coverage with
`mbx nextest run -p compiler --all-targets --no-fail-fast`; bounded linking
batches may cover it during repair, but release coverage must be complete.

Inspect the outcomes, not just command exits. Corpus goldens may retain rejected
positive programs; `built` can mean cross-emission without execution and
unbound fixtures only check. `expected_native_exit.txt` enforces an authored
exit/trap independently of the golden; verify required stdout too. Sample refresh
only builds. `tools/sample_native_gate.py` exercises documented oracles on the
matching host but leaves no-oracle samples unscored; **SAMPLE-CORPUS** owns its
coverage. Missing runtime coverage stays open.

Preserve the deleted integration tests' obligations in maintained harnesses:
no-selection bytes/metadata and repeatability across targets; constructor-domain
membership with disproved versus unproved outcomes; cross-package mutable
qualification rejection using valid initializers; finite rational/float comparison
with undefined/landed operands rejected; and matching unsigned-right-shift
results across macOS/Linux. Deleted test names are not runnable release commands.

Record exact commands, selected coverage, toolchain, host, observations and
justified skips; **RC-RELEASE-RECORD-AND-CLOSURE** joins the same-commit evidence.
These are independent host runs of shared gates; capability repairs retain
their existing owners.

- **ELF-LOADER-CONFIGURATION.** Build-owned [loader selection](wiki/spec/build/configuration.md#elf-executable-loader-selection)
  is wired end to end: `builder.elf_interpreter.<target>` rows override the
  Linux target environments' declared glibc defaults, the effective loader
  rides `NativeApplicationMetadata` through the retained proposal and package
  path, `settle_elf_loader` turns only import-bearing ELF objects into
  `DynamicElf`, and publication installs the flat dynamic ELF; relative paths
  reject (`fail/build/elf_interpreter_relative_path`) and static output has no
  `PT_INTERP`. Remaining acceptance: a Linux x86-64 host run of
  `providers/external_boundary_requirement_via_exit` (libc.so.6 `abs`/`exit` at
  GLIBC_2.2.5) records `built … exit:21` in `corpus_native_linux_x86_64.txt`.
  A Linux AArch64 run of the same fixture binds `abs`/`exit` at GLIBC_2.17,
  the first version aarch64 glibc defines. The
  dynamic writer resolves image-local relocations (internal calls, callback
  addresses, `.text` references into `.data`/`.bss`, data pointers) at its load
  layout and names `.shstrtab` in `e_shstrndx`;
  `pass/capabilities/callback_direct_install_compile` now checks and builds on
  the macOS ARM64 check and native legs.

- **RC-NATIVE-MATRIX-LINUX-X86-64.** Close the shared gates with actual ELF
  x86-64 execution, including dynamic imports (**ELF-LOADER-CONFIGURATION**).
  Retain `image-elf`'s corrected
  `GeneralRelocationEntrySize = 9` and entry-size row beside
  `DT_RELA`/`DT_RELASZ`; encoder/replay coverage does not establish execution.
  **SAMPLE-CORPUS** owns sample failures and
  **TWO-AXIS-TERMINAL-AUTHORITY-REVIEW** owns package/provider authority replay.
  Reproduce current failures; a historical host record or selected ABI cohort
  cannot close the full row.

- **RC-NATIVE-MATRIX-LINUX-ARM64.** Close the shared gates with actual
  AArch64 ELF execution on Linux or contract-permitted, named/versioned
  emulation. The emulator must run the target runtime legs; an x86-64 harness
  that cfg-excludes them does not suffice. Cover host/time/filesystem and
  `IntegerAt` projection/write behavior. A single `cli_mvp` execution,
  cross-target checks or emitted bytes cannot replace full coverage and
  justified skips.

- **RC-NATIVE-MATRIX-MACOS-ARM64.** Close the shared gates with emitted
  Mach-O execution on macOS AArch64. The native corpus has a populated golden;
  stable results still include unfinished programs. Both committed macOS
  release records remain open, with native observations failing on the deleted
  `canary_suite` target. Produce current full-gate evidence and actual
  exit/output observations.

  Use [native rejection attribution](wiki/drafts/measurements/macos_arm64_native_rejections.md),
  [release-gate history](wiki/drafts/measurements/macos_arm64_rc_gates.md) and
  [baseline failures](wiki/drafts/measurements/known_baseline_failures.md) as
  reproduction leads, not current totals. Route omitted Unit plans to
  **STATE-LOCAL-VALUE-FRONTIER** and other failures to their capability owners.
  Follow the recording guide's host prerequisites; partial runs and Linux
  emission do not establish this host's acceptance.

- **RC-NATIVE-MATRIX-WINDOWS-X64.** Close the shared gates with emitted
  PE execution on Windows x86-64, including the hosted receiver.
  `tests/omega/pass/filesystem/windows_set_file_time_exit` must perform
  SetFileTime followed by `_stat64`, read back Unix time 1,500,000,000 and
  exit 70. Enforce that authored oracle: the fixture currently has only a
  comment, while the corpus records its rejection. Correct unsigned byte
  assembly and cross-emission cannot substitute for the Windows runtime leg.

- **MACOS-X64-HOST-PROFILE.** Run an ordinary terminating program through
  the compiler and runtime on Intel macOS, then cover Mach-O entry/exit,
  hosted receiver and provider/import pairing. Target recognition, exact
  physical-entry reconstruction and receiver machinery exist; reuse them.

  macos_x86_64 now emits and runs under Rosetta on Apple silicon: free Unit
  entry, hosted receiver (`cli_mvp` exits 0 printing its two lines), Darwin
  syscall Console and ProcessExit leaves, and dyld `libSystem` import pairing
  (`external_boundary_requirement_via_exit` exits 21). With every
  macos_arm64-bound fixture temporarily bound to macos_x86_64, 345 of 556
  non-trapping pass/run fixtures executed with the same exit and stdout facts
  as macos_arm64 and none diverged; see the
  [host observations](wiki/drafts/measurements/macos_x64_host_profile.md).
  Rosetta is translation, not an Intel CPU: on an Intel Mac, record
  `tests/omega/corpus_native_macos_x86_64.txt` with
  `python3 tools/corpus_gate.py --native --record`, compare it with the
  Rosetta leg (`--native-target macos_x86_64` on Apple silicon), and record
  `omega inspect-terminal --target macos_x86_64` legs. 579 fixture and 146
  sample builds still bind macos_arm64 without macos_x86_64; extend them only
  with an Intel or Rosetta observation. This additional platform remains
  separate from the four-host Rust completion matrix and has no Alpha
  bootstrap dependency.

## Omega-written compiler (after Rust completion)

Finish the Rust [completion contract](wiki/drafts/reference/rust_compiler_completion.md)
before starting the product-language migration. Rust can remain a differential
implementation afterward, but neither Rust agreement nor Rust-specific machinery
is bootstrap authority. Bootstrap construction stays on `TASKS_BOOTSTRAP.md`.
- **OMEGA-PRODUCT-COMPILER-SOURCE.** After Rust release completion, finish the
  Omega-written compiler in separate `source/psi/` and `source/omega/` packages,
  with `source/omega/{build.omg,main.omg}` as product roots. Psi owns parsing,
  resolution, typing, checking, proof and Terminal Psi; Omega consumes Terminal
  Psi for provider selection, realization, ABI and emission.

  The current product reads source, lexes, parses supported `use`/`data` roots
  and exits without a compiler artifact. Extend that connected pipeline through
  the full language and requested products in ordinary compositional slices.
  Preserve the package/firewall split; no private dialect, file/AST allowlist,
  source-shape-only lowering or parallel source-to-native route.

  Recheck `omega --check source/omega/main.omg` and
  `omega --check source/psi/gates/parser/main.omg`, then build/run
  `source/psi/test-parser.sh` with explicit freshly built `OMEGA_CLI` and
  `OMEGA_TARGET`. Both source checks currently reach `Parser::parse` and
  omit its unit plan at state 7, `conditional successors: guard expression`.
  Minimize the nested `Token::Keyword { keyword: KeywordKind::Data }`
  guard in `source/psi/parse/parser.omg` before assigning its lowering repair.
  Local mixed-aggregate construction, scalar guards beside array fields and
  projected borrowed-root state transfers are covered by
  `calls/local_nested_record_call_then_state_exit` (native and interpreted
  exit 70); passing that fixture does not establish either compiler root.
  General realization belongs to **STATE-LOCAL-VALUE-FRONTIER**;
  **BORROWED-STORAGE-RESTORATION** and **CRASH-CONTRACT** retain their gaps.
  The copied-argument fixture is now
  `tests/omega/pass/calls/copy_projection_from_borrowed_receiver`: retain its
  authored entry, nonzero callee observation, unchanged caller value and exit-70
  oracle. A source check cannot substitute for that native result.

  Preserve the parser gate's acceptance/rejection, lexical handoff, structural
  identity, capacity and determinism controls. Python only decodes/compares
  observations; this is not Rust/Omega differential coverage. Cached artifacts,
  partial parser acceptance and lowering omissions establish neither native
  execution nor invalid source. Preserve range-proven narrowing casts.

  Acceptance: the exact package-resolved closure implements the complete
  [compiler product contract](wiki/spec/build/compiler_request.md), passes the
  shared product suite and Rust/Omega differential checks, and publishes a
  deterministic manifest of all transitive compiler/build inputs. Preserve
  requested products, targets and admission boundaries. Rust is a nonauthoritative
  comparator; C still owes ordinary Alpha output without requiring Rust Alpha
  emission. Epsilon-written D and `omega0 → omega` construction remain on
  `TASKS_BOOTSTRAP.md`.
