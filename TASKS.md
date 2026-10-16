# Tasks

This board is reference, not the work queue. Compiler work follows
[the sample loop](AGENTS.md#the-sample-loop): take a failing sample from
`python3 tools/sample_queue.py`, make it run, and when it already runs, write a
harder one. The [completion contract](wiki/drafts/reference/rust_compiler_completion.md)
defines release acceptance. Squalr is a customer, not the destination or a
reason to special-case source; all accepted language scope stays required.

## Working the board

The sections below describe capability gaps, release requirements and design
context under stable IDs that other documents link to. Read the section a
sample's failure leads into; do not take an item as work on its own. A sample
that runs because of a repair is the record that the repair is done: subtract
what now works from any item that names that sample or its root cause, and
delete the item when nothing remains.

An item stands on its own only when no sample can carry it:

- an owner decision (linked to [OWNER_QUESTIONS.md](OWNER_QUESTIONS.md)) or an
  unavailable environment;
- release closure ([Rust compiler release closure](#rust-compiler-release-closure))
  and its full gates;
- a regression that could not be repaired where it was found.

Optimizer and bootstrap work keep their own boards
([TASKS_OPTIMIZER.md](TASKS_OPTIMIZER.md), [TASKS_BOOTSTRAP.md](TASKS_BOOTSTRAP.md))
and acceptance; Cathedral structures stay Cathedral's.

A claim note carries the sample, its oracle, the current failure, the next
necessary action and prerequisites. After compaction or a checkpoint, reread
the note before proceeding. A roadblock you discover belongs to the work that
found it: fix it. A checkpoint, diagnosis or child task does not make a sample
run. File conflicts are reconciled at rebase, not avoided by inventing another
item. Never retain competing implementations as modes.

### Development-health outcomes

After landing a sample, an agent may take either duty below.
These standing entries are parked when no concrete incident is assigned. An
active claim must name an experienced obstruction, reproducible workflow,
baseline and adequate acceptance target. Close that incident and return the
duty to parked; keep measurements in its commit, not a growing incident log.
Use an existing specific repair item when one already owns the obstruction.

- **ITERATION-PERFORMANCE.** (new-scope) Restore a usable edit/build/run workflow
  that is measurably obstructing compiler development. The session fixes its cause
  and demonstrates the agreed time/resource target on comparable inputs while
  preserving behavior. Stop at usable iteration, not exhaustive optimization.
- **VALIDATION-ENVIRONMENT.** (new-scope) Restore meaningful, bounded feedback
  for a named development workflow. The session repairs test selection, harness or
  environment friction and demonstrates that affected regressions are detected
  without an unnecessary full-suite run. Preserve release coverage and runtime
  oracles; recorded unrelated failures need not become this incident's repairs.

Language behavior is tested by corpus fixtures under
`tests/omega/{pass,fail,run}/` through `tools/corpus_gate.py` (`--native`
builds and executes eligible fixtures); `tools/sample_native_gate.py` checks
documented sample exit/output on the host. Most compiler crates have no Rust
tests: an acceptance names a corpus fixture, or asks for a new one. Proof-carrying
code was removed ([PCC removal](#pcc-removal)); no item here requires shipped
proofs, receiver re-verification or independent replay of the compiler's own
output. Run
`python tools/board_paths.py --board TASKS.md` after moving cited paths.
Workflow is in [AGENTS.md](AGENTS.md), live assignments in the
[claims registry](tools/claims.md), semantic decisions in
[OWNER_QUESTIONS.md](OWNER_QUESTIONS.md).

## Connected scientific library

- **W100-CONNECTED.** (new-scope) Complete the connected rank-two scientific
  library in `source/library/numerics/` and its W100 API consumers in
  `samples/cli/scientific/w100/`. Reuse preserved candidate sources and frozen
  mathematical oracles; the integration owner implements all required
  interfaces without active-worker prerequisites. Current frontier: connect
  descriptor-backed moments to the shared layout, canonical float slice access
  and numeric policy; qualify both complete consumers and separate invalid
  controls, then continue the retained rank-two kernels. Preserve unrelated work
  and land coherent qualified chunks through the normal queue. Candidate preservation in
  IMPORTS.json is not capability acceptance.

## Updating the boards

Update by subtraction, not accumulation. This applies to all three task boards.

- Read the entire owning item before editing. Remove completed work rather than
  replacing it with a success report; delete the item when no work remains.
- Retain the remaining failure, owning mechanism, necessary context, dependencies
  and acceptance. Keep passing regression commands only where they constrain the
  remaining repair. Run histories and success totals belong in commit messages
  and test records, not the board. Keep a revision only when needed to reproduce
  or qualify an unresolved observation.
- Search the boards for the repaired capability and affected program; reconcile
  stale claims elsewhere. A passing narrower example does not close a wider task.
- Do not create a task for each diagnostic or next implementation step. Splits
  retain a parent integration owner; unrelated findings enter the board only
  with a distinct required outcome and acceptance. No mining to fill slots.
- Reconcile concurrent edits by the remaining requirements, not a union of old
  and new paragraphs. Remove superseded directions and duplicate blockers.
- Review the diff: what unfinished requirement did this change remove? If the
  item grew, what newly established unfinished work or necessary context explains
  the growth? There is no line quota and no requirement to invent a deletion.

## Ownership firewall

Follow the [connected pipeline](omega-rust/pipeline.md): Psi owns source
semantics through Terminal Psi, Omega owns target realization, backends own
physical detail, Cathedral owns OS policy and structures. Complete ordinary
operations on the common graph instead of adding a source-shape recognizer.

Compilation does not require an accepted package lock, and unfinished native
realization does not block source-package installation; package acceptance,
build authority and native output are separate obligations.

## High-priority composition repairs

The items in [Immediate product closure](#immediate-product-closure) are
ready implementation work. Reproduce the named program's current failure
first, with `omega run --both <main.omg>` on a matching host plus its
registered corpus fixtures, and record separately where it stops: source
checking, Terminal Psi production, interpreted execution, native exit/output.
A moved refusal is still unfinished work.

Repairs follow the
[clean-as-you-go rule](omega-rust/pipeline.md#compositional-lowering): delete
the replaced special path in the same change. A new helper around the same
exception matrix, a source rewrite that avoids the operation, or an isolated
helper that passes does not close an item.

## Immediate product closure

- **CHECKED-PACKAGE-REUSE.** (new-scope) A dependent's check re-judges its
  dependencies' bodies: on Windows (opt-level 1, `90dcfa2bcbe`) a 10-line std
  program checks in 15.6 s, std's package step 5.8 s plus 7.1 s of Stage 05
  in the program's own check. A std-free fixture checks in 1.26 s. The corpus
  is 767 CPU-minutes (median fixture 5.6 s; 1,385 of 3,998 fixtures depend on
  std). std is already checked once at package review. Dependents should
  consume that judgment through compiler-private checked package reuse, with
  exact dependency and compiler identities. Reuse must retain the same facts
  and body-dependent queries at the same precision, so it is
  implementation, not a language change. Reachability pruning was measured
  and rejected: the existing product closure keeps 904 of 1,174 library
  machines for the 10-line program, because every provider of a reached
  requirement stays live for every target. Acceptance: fixture median under
  2 s and a std probe check under 3 s, with `corpus_gate --compare` over the
  touched groups losing nothing. Frontier: prepared `vector_distance --check`
  on macOS ARM64 at `e01a67855f4` takes 7.50 s with an opt-level-1 compiler.
  Six coarse summaries and bodyless-supply handling are insufficient:
  `CallFrameResolver` also derives returned-reference origins and binding
  stability, denotational admission checks observation freedom, and
  `return_ranges::infer_return_interval` traverses captured arrivals using
  the caller's actual intervals. Keep frozen dependency bodies available to
  these queries; do not invent a second summary language or weaken imports
  to boundary ceilings. Reuse the existing checked graph and fact owners,
  with one declaration/state/expression correspondence, while new local bodies
  and generic substitutions retain their own obligations. Package policy
  acceptance is not a checked judgment. Same-invocation reuse should borrow
  retained graphs; persisted compiler-private reuse needs exact source,
  dependency and compiler binding, not process-local handles. Native consumers
  still require executable checked bodies or reused Terminal products.
  Next: preserve this graph boundary and remove measured repeated checking;
  demonstrate the timing bound through public checks and invalid controls.

- **FLOAT-PAYLOAD-COMPILE-TIME.** (new-scope) Finish restoring the float
  case-payload cohort's full corpus-check path to its historical 4–5s bound.
  `pass/control_flow/runtime_{float_payload_snapshot_after_case_replacement,projected_float_case_payload,root_float_case_payload}_exit`
  now meets that bound through public `--check` on macOS ARM64; the corpus
  check leg, which also realizes the bound entry, still records 6.4–7.3s at
  one worker. Profile that remaining work before choosing another repair.
  Linux's last 20–26s measurement predates shared family analysis, exact
  declaration lookups and immutable case-reference reuse; remeasure there
  rather than extrapolating from this Mac. Use an optimized dev compiler
  (`CARGO_PROFILE_DEV_OPT_LEVEL=1`), comparable host/load and the same path
  when comparing with `a800cc9ab970`; retain native/Terminal exit 70.
  Any reuse must preserve raw versus observed values, nested observation
  types, scalar/indexed authority and literal landings: structural semantic
  equality deliberately erases authority and landing metadata and cannot
  key returned payloads.

- **CHECKED-ASM-LABEL-CONTRACTS.** (split-of:STATIC-MACHINE-MIGRATION)
  Implement the ratified checked-assembly control-flow model in
  [assembly](wiki/spec/language/assembly.md#blocks-and-assertions): explicit
  labels are basic blocks, each label may carry ordinary `requires`/`ensures`,
  and every forward edge and backedge is proved against exact modeled register,
  flag, memory, place, authority and ownership state. Finite acyclic local
  label paths retain one lexical storage frame and prove place/value contracts
  through shared flow and Terminal/native execution. Register/flag versions,
  conditional and cyclic edges with ordinary loop obligations, and checked
  hardware-effect retirement of the exact linear obligation remain unfinished.
  Extend that control model rather than adding an opaque asm parser or verifier.

  Own the source grammar and catalogs in
  `01_tokens-to-syntax-trees/src/bodies/statements/inline_assembly.rs` and
  `language-core/src/inline_assembly/`, checked CFG/state transfer and contracts
  in stage 04, durable target-neutral operations and stage 05 lowering, and the
  shared Terminal/native execution paths. Separate `asm` statements retain no
  hidden scratch state; labels inside one body carry explicit register/flag
  versions. A label memory proposition is a proof-only observation and grants
  no runtime access. Ambiguous width, address space or modeled backing rejects.
  Loops use ordinary invariant and ranked-decrease reasoning when termination
  is required.

  A checked machine satisfying a consuming boundary-trait member may retire a
  linear argument only when instruction semantics and label contracts prove the
  required effect on every normal exit. `nop`, a wrong port/place/value,
  missing ordering, an unmatched installation/token lineage or a bypass path
  leaves the argument live and rejects. `asm where` and label `ensures` are
  proof goals, never admissions. Do not add a hardware `ForeignBinding`, a
  compiler intrinsic for Cathedral policy, or a trust annotation around asm.

  Integration customers are `pass/memory/interrupt_table_canary` and
  `pass/memory/secondary_processor_canary`. Under
  **TOP-LEVEL-BOUNDARY-REQUIREMENTS**, migrate completion to one-member boundary
  traits and checked implementations whose asm proves LAPIC/PIC completion and
  the startup handshake. Preserve `Pending` domain preconditions, exact
  installation/token identity, bounded reach and linear consumption.

  Acceptance: labeled straight-line, diamond and loop bodies execute identically
  through Terminal and native paths; wrong-edge invariants, unmodeled memory,
  wrong controller coordinates, missing effects/order and live linear exits
  reject. The interrupt and secondary-processor canaries check with no fake
  `CompilerIntrinsic` provider, while forged, unrelated, replayed and
  double-completion controls reject.

- **NAMED-IMPLEMENTATION-CONFORMANCE-MIGRATION.** (split-of:STATIC-MACHINE-MIGRATION)
  Replace data-subject satisfaction with named implementations under
  [conformances](wiki/spec/language/conformances.md#data-and-code-are-independent).
  `NormalCounter satisfies CounterOperations<CounterData>` names code, not
  data; `CounterData` is an ordinary trait argument. There is no `Self`, hidden
  first type binder, implicit implementing type or special subjectless variant.
  - Current frontier: data-subject satisfaction is deleted from the
    reference compiler and the product lexer. Every conformance is a named
    implementation `Name<binders> satisfies Trait<Args> { rows }`; an
    evidence binder is `Order satisfies Trait<Args>`. `Name: Data satisfies`,
    bodyless maps and `where T satisfies` reject, `Self` is an unknown type
    name, and a trait requirement declares its bare name. Syntax, resolved,
    typed and Terminal closed-conformance applications and package review
    records carry no subject. Boundary roots (the interrupt and
    secondary-processor canaries) are named implementations whose rows name
    existing entry machines; provider planning installs each through the
    implementation's own plan and resolves installation reach from its rows.
    Remaining: product-producer parity for positional trait implementation
    binders and their ordered mixed telescopes; const-expression closure over
    open binders and package projection for generic implementation applications;
    specialization of generic calling-policy realization machines and nested
    data arguments before plan evaluation. Reflection selection rejects a
    named implementation as a member's operation until callable-contract checking lands
    (**Semantic reflection**).
  - Own parsing, resolution, typing, checking and conformance publication in
    `omega-rust/psi/compilation/pipeline/01_tokens-to-syntax-trees` through
    `07_lowered-psi-to-terminal-psi`, their representations and
    `omega-rust/psi/compilation/checking`; apply the same source contract to
    the product producer in `source/psi/`. Carry implementation identity,
    complete ordinary trait arguments, rows and laws through Terminal codecs,
    interpretation and package review metadata. Retain source checking and
    authored mathematics proof checking.
    Remove subject/carrier fields and receiver correspondence used solely for
    trait satisfaction; do not rename them into a hidden distinguished binder.
  - Generic implementation binders and exact `satisfies Trait<...>::member`
    edges substitute declared arguments only. Defaults call the selected map's
    rows. Keep explicit selection, visibility, laws and complete row identity;
    fix the named-witness declaration arity path instead of preserving an
    implicit argument through qualification. No data-owned trait discovery.
  - **Owner-authorized breaking migration:** break old source APIs, fixtures,
    snapshots and internal schemas as needed. Intermediate commits may have
    failures attributable to this removed model; name the affected cases and
    remaining owning migration task. Delete legacy-only tests and code; rewrite
    tests of real behavior under the new model. No compatibility modes, aliases,
    fallback inference, dual implementations or requirement to keep legacy
    tests green. This does not permit weakening proofs, accepting invalid new
    programs, or declaring unfinished runtime behavior complete. The same
    authorization governs the source and dynamic migration tasks below.
  Acceptance: the named counter program runs and the migrated witness program
  checks; wrong trait arguments, incomplete maps, data-as-implementation and
  retired `Self`/subject syntax reject. Run
  `python tools/corpus_gate.py --filter traits/,proofs/named_witness_static_trait_plural_compile`
  on the rewritten coverage and focused package checks for changed owners.
  Product-producer parity is required; changes to the accepted bootstrap chain
  still follow its proofs, not a golden-file reset. Coordinate source batches
  with **TRAIT-IMPLEMENTATION-SOURCE-MIGRATION**, not a second syntax dialect.

- **TRAIT-IMPLEMENTATION-SOURCE-MIGRATION.** (split-of:STATIC-MACHINE-MIGRATION)
  Aggressively migrate authored requirements, implementations and consumers to
  the [named-implementation model](wiki/spec/language/conformances.md).
  Depends on **NAMED-IMPLEMENTATION-CONFORMANCE-MIGRATION** for compiler
  support; source conversion can be prepared concurrently in disjoint paths.
  - Own the affected declarations in `source/library/`, `samples/`, and
    `tests/omega/`, with item/path claims for independent batches. Update exact
    satisfaction edges, generic bounds, inherited trait arguments, defaults,
    operator wrappers, placed-access families and test-group
    registrations together with their callers. No blanket `Self` replacement:
    declare each actual data type parameter where the signature needs it.
  - Customers include `source/library/core/layout.omg`,
    `samples/gui/{window_app,window_demo,windowed_calculator,image_viewer}`,
    clock-bearing CLI samples, and
    `pass/progress/provider_receiver_progress_installation`. Providers remain
    stateless (**STATELESS-PROVIDERS**). Persistent state is explicit returned
    data passed back by callers. Vtable addresses belong to the foreign binding
    plan, not an invented provider object. Capability preconditions name real
    declared values and their exact service relationship; do not remove a
    fairness requirement merely to eliminate an old receiver.
  - Audit current source rather than trusting retired receiver-declaration counts.
    Preserve real behavior oracles; rename or replace fixtures whose names or
    shapes pin data-subject satisfaction. Delete tests whose only purpose is
    legacy acceptance. Apply the breaking-migration authorization above.
  Acceptance: the migrated counter/witness programs, placed access corpus and
  progress customer work under their existing behavioral obligations; clock
  samples and the four GUI customers compile and run on their supported hosts
  with their authored results. Use `python tools/corpus_gate.py --filter traits/,layouts/,progress/`
  plus `python tools/sample_gate.py --filter gui/window_app,gui/window_demo,gui/windowed_calculator,gui/image_viewer`
  and focused clock sample filters for source checks, then
  `mbx run -p omega -- run <sample/main.omg>` on supported hosts against each
  sample's runtime oracle. Existing unrelated backend blockers
  remain named dependencies, not permission to restore a subject. No active
  Omega source retains `Self`, data-subject satisfaction, or implicit receiver supply.

- **DYNAMIC-IMPLEMENTATION-SELECTION-MIGRATION.** (split-of:RESTORE-DYNAMIC-DESCRIPTOR-AND-TABLE-CUSTODY)
  Replace data-attached trait objects with runtime selection of named
  implementations under [dynamic dispatch](wiki/spec/terminal-psi/dynamic_dispatch.md).
  The implementation selector and every data argument have distinct identities;
  no argument is an erased subject and no caller requires an attachment.
  - Current frontier: Psi produces runtime selection end to end. Checking
    instantiates each reached requirement under its selector application into
    a slot (`monomorphization/dynamic_slots.rs`; `TypedTrees::dynamic_slots`)
    and retargets each selector call to it, selector first, so every
    operand/borrow/frame analysis sees a concrete bodyless callee. Stage 04
    plans Unit-route (`CheckedUnitCallRoute::Dynamic`) and computation-route
    (`CheckedScalarCallee::Dynamic`) calls plus a `CheckedDynamicSlotPlan`
    per slot; stage 05 emits `SelectImplementation`/`CallDynamic`, roots each
    selected implementation's rows in the closure and assembles the module's
    interface/table catalog from the slot plans. Mixed scalar/structural slots
    retain dense positions in each ordinary ABI lane. The `pass/traits/runtime_dyn_*`
    customers use the shared native and Terminal execution routes. Keep their
    result/output agreement while extending the remaining slot contracts;
    no selector-specific execution adapter is needed.
  - Remaining requirement-contract work: implication beyond exact fact
    correspondence, transparent proposition aliases and supported integer
    arithmetic. Preserve the requirement's public contract at
    selector calls independently of stronger implementation guarantees.
  - Next: suspending requirements and finite-family selector
    calls (which keep their requirement targets, unslotted) decline at planning.
    Byte-backed returned-region containment remains unsupported by Terminal
    interpretation. Preserve owned result shapes,
    qualifications and exact active claims through the common call/return path
    (`pass/traits/runtime_dyn_{structural,nested,array,guarded,linear}_result_exit`).
    A requirement with published crash routes keeps no
    crash-refinement row because its operands lead with the selector.
    Termination treats a selector call as an edge to every implementation
    selected for its slot, so a recursion cycle through runtime selection
    rejects (`fail/traits/dyn_selector_reentry_unranked`,
    `fail/traits/dyn_selector_self_reentry_unranked`) until a ranking crosses a
    dispatch edge. Package review does not yet consume the new records, and
    `source/psi/` has no producer parity.
  - Own Psi dynamic checking/facts and stages 04/05/07, Terminal selection/call
    representation and codec, the interpreter, and corresponding `source/psi/`
    producers. Preserve exact rows, contracts, finite families, proof actuals
    and private selection visibility. Package review consumes the same new
    records.
  - **RESTORE-DYNAMIC-DESCRIPTOR-AND-TABLE-CUSTODY** owns native consumption,
    including `source/omega/` parity. Hiding input/output types is separate
    work, not a prerequisite and not permission to restore `Self`.
  Acceptance: `python tools/corpus_gate.py --filter traits/` covers the migrated
  programs with interpreter/native result agreement on matching supported hosts.
  A selector changes implementation at runtime while operating on the same
  explicit data; `runtime_dyn_data_argument_counts_exit` uses zero and two data
  arguments, `runtime_dyn_joined_selector_exit` joins two selections and
  `runtime_dyn_selector_record_field_exit` stores one. Copying or rebinding a
  selector neither captures data nor changes its borrow lifetime.

- **STATIC-MACHINE-MIGRATION.** (new-scope) Eliminate machine/state `self`
  receivers and implicit receiver calls under the
  [static-machine contract](wiki/spec/language/machines.md#static-declarations-and-explicit-state).
  This is an owner decision, not an optional compatibility mode. The
  [named-implementation decision](wiki/spec/language/conformances.md#data-and-code-are-independent)
  also removes data-subject conformances and `Self`; the three implementation
  migration tasks below own that work. Their authorization to break legacy tests
  and remove compatibility paths applies to this migration too. The parser
  rejects `self` parameters and implicit `self` expressions
  (`fail/machines/retired_self_receiver`); do not restore them to make an
  unmigrated program pass.
  - Source remainder: the two memory canaries
    (**TOP-LEVEL-BOUNDARY-REQUIREMENTS** and **CHECKED-ASM-LABEL-CONTRACTS**).
    Build receiver calls remain for facets such as test groups, and source still
    carries compiler-installed services through `Binding<R>` fields/parameters.
    Move their producers and consumers to
    namespace-qualified calls with explicit arguments, following `Build::bind`,
    `Build::select_provider`, `Build::exclude_service`, and
    `Build::select_representation`, `Build::accept_boundary`,
    `Build::require_wire_compatibility`, and `Build::assume_binding_contract`.
    A boundary requirement call spells `Requirement::operation(arguments)` and
    demands one exact static provider slot; checking accepts that form, and a
    leading `Binding<R>` operand is the only remaining receiver-lane entry.
    Migrate source and fixtures: 40 test and 7 sample files still name
    `Binding<`. The test remainder pins Binding-specific refusals and
    receiver lanes; the sample remainder is four probes plus `file_journal`,
    `note_vault` and `private_issuer_provider_result`, which move with std's
    `Filesystem` carrier under **FILESYSTEM-PLATFORM-SERVICES**. Deleting the
    carrier waits for that record: std must keep checking. An inherited
    requirement is called through its declaring trait (`Parent::reset`), not
    the refining child's name, and an applied one as `Parent<u64>::read()`.
    Then delete `source/library/core/binding.omg`, Binding type recognition, entry-field
    establishment, receiver/leading-argument paths and source-visible routing
    custody. Add fail controls for `Binding<R>` in fields, locals and parameters
    and for ambiguous provider supply; provider admission already rejects a
    requirement the entry reaches with no selected provider. Multiple static roles use exact
    generic applications or wrapper traits; runtime multiplicity uses protocol
    handles/descriptors/`dyn`, never a generic routing carrier.
  - Compiler remainder: typed parameters now have only explicit subject
    identities; checking, interpretation, provider arity and review capture
    consume those identities without an implicit receiver parameter. The
    checked structural signatures, Terminal parameters, provider signatures,
    retained-borrow roots and Omega artifacts no longer carry receiver flags.
    Retire remaining attachment helpers; proof-machine classification must
    continue to depend on explicit values, never namespace qualification.
    Layout and task stack sizing no longer manufacture namespace storage;
    activation storage comes from explicit parameters, locals and call arguments.
    Carry no longer derives storage or reachability from namespace fields;
    activation planning combines direct envelopes over its checked call graph.
    Suspension production still needs checked per-value loan carry evidence.
    Preserve independent rejection of malformed evidence while removing the
    obsolete representation; do not restore hidden argument insertion. Keep
    unrelated receiver-side artifact admission and mathematical subject
    binding.
  - Entry context: provision the exact ordinary mutable context parameter of
    the [entry shape](wiki/spec/build/entry_roots.md#entry-shape-and-arrival-bridge),
    preserving ZII eligibility, selected bindings, root lineage, storage
    partitioning and cleanup. Entry provisioning now joins explicit source
    positions; remove remaining attachment-driven discovery without changing
    provider or storage authority.
  - `samples/cli/basics/generic_counters` (exit 16) needs matching-host runs
    on both Linux targets, preserving Windows x86-64 and macOS ARM64 behavior.
  Acceptance: static customers execute with their authored exit/output; the
  migrated corpus and sample gates pass; no accepted receiver syntax,
  implicit receiver argument, source-visible `Binding<R>`, or
  receiver-specific signature authority remains. Exact static role applications
  select independently; missing/ambiguous supply rejects; protocol handles and
  `dyn` values preserve actual runtime multiplicity. Keep the local-owner loan fixtures under `pass/calls/local_*`,
  `pass/calls/static_*`, `pass/borrows/static_*`, `pass/text/local_bounded_text_*`
  and `pass/storage/forwarded_context_byte_field_read_store_exit` at their
  recorded exits.

Prioritize unchanged customer programs reaching native execution over
additional evidence without an exercising program. The
[Rust compiler completion plan](wiki/drafts/reference/rust_compiler_completion.md)
defines the complete product bar.

- **STATE-LOCAL-VALUE-FRONTIER.** Complete ordinary expression, call and
  storage composition through checked plans and native execution, preserving
  the [evaluation schedule](wiki/spec/language/expressions.md#evaluation-schedule)
  and [argument/result custody](wiki/spec/terminal-psi/calls_and_outcomes.md#argument-and-result-ordering):
  exact producer, authored parameter position, result owner, loan activation
  and once-only effects. **CML4** owns residual cleanup;
  **GENERAL-CYCLIC-EXECUTION** owns cyclic control. Owners:
  `omega-rust/psi/compilation/pipeline/04_typed-trees-to-checked-trees/src/execution/terminal_unit/`,
  its checked computation producers, stage 05 argument evaluation and
  attached Unit emission; Omega stage 02 owns target admission. Extend common
  operations and remove superseded whole-machine recognizers as you go.
  **SAMPLE-CORPUS** and **CANARY-CORPUS** keep the full program oracles.

  - **Computed calls and state transfers.** Finish selected mixed scalar/structural
    operands, effectful arguments/returns, nested mutable-argument calls,
    dependent/public-trait results and boundary consumers on the common
    `EstablishStructuralValue` route through ordinary argument evaluation,
    not another whole-body plan. Preserve once-only
    field/operand order, selective short-circuit effects, computed-result float
    format and selected affine cleanup:
    `pass/ownership/runtime_projected_constructor_operands_exit` preserves
    projected record/case/array operands and their unconsumed siblings;
    `pass/float/computed_comparison_literal_landing_exit`,
    `pass/calls/runtime_constructed_payload_arguments_exit`,
    `pass/calls/runtime_constructed_copy_payload_arguments_exit`,
    `pass/borrows/runtime_computed_case_copy_order_exit` and
    `pass/operators/runtime_constructed_operand_result_exit` (exit 0 in both
    engines). Computed copies retain selectors and the selected value before
    later operands mutate their source. Use ordinary expression continuations
    for selected payload returns, not raw jump arguments.
    Value dispatch still needs its full construction schedule. Preserve
    occurrence-bound routed memberships through completed
    children, copies and nested projections:
    `pass/calls/runtime_constructor_child_declared_membership_exit` and
    `pass/domains/runtime_nested_record_membership_transport_exit` stay at 0
    in both engines; `fail/domains/record_child_membership_requires_issuance`
    must reject literal forgery, stale source epochs and sibling substitution.
    Keep `pass/domains/runtime_case_constructor_child_memberships_exit` at 0
    in both engines: bound and inline case arguments retain completed common
    and payload issuer memberships across copies and later sibling writes;
    omitted zero-valid siblings and inactive payloads preserve common facts.
    Reject `fail/domains/case_child_membership_requires_issuance` for forged,
    stale and wrong-issuer children, and
    `fail/dependent/mixed_common_nested_zero_requires_establishment` for an
    omitted gated common child. Never grant membership from the receiver's type.
    Keep `pass/calls/runtime_constructor_child_pure_calls_exit` at 0 in both
    engines without adding an ensures clause or a Terminal certificate.
    Keep stored snapshot controls
    `pass/calls/runtime_stored_{constructor_snapshots,equal_observations}_exit`
    at 0 in both engines and reject
    `fail/calls/constructor_child_local_epochs_not_equal`: remaining composed
    calls must preserve completed child observations, not replay initializers.
    The checked interpreter still declines explicit borrowing of an owned call
    result in `checked-interpreter/.../names_recasts_and_places.rs::resolve_place`
    (`pass/calls/runtime_temporary_lending_order_exit`).
    Keep `runtime_constructed_borrowed_payload_arguments_exit`,
    `runtime_selected_trivial_local_cleanup_exit` and
    `runtime_selected_local_owner_exit` (exit 0): completed owners must outlive
    their shared calls, copyable temporaries owe no affine disposal, and each
    selected completion owns its transfer/disposal partition. A producer's
    return-discard flag is not liveness authority. Keep
    `pass/calls/runtime_case_payload_scalar_array_read_exit` as the exact
    selected collection and sibling-payload observation control.
    Keep `pass/data/runtime_mixed_case_nested_common_fields_exit` at 0 in both
    engines for recursive omitted common zeros, erased-member rosters, ordered
    constructor effects and indexed common reads; reject invalid zero domains
    and duplicate linear common/payload operands.
    **CML4** owns remaining residual cleanup. Reference debt must
    follow ordinary disposal, not a trivial affine discard.
    Keep: `pass/expressions/runtime_temporary_record_field_call_exit` (a
    call-produced plain-owned record supports immediate field observation,
    its call effect performed once),
    `pass/expressions/runtime_nested_record_field_call_exit` (nested call results,
    named-owned locals and ordered mixed scalar/structural operands, exit 0),
    `pass/calls/inline_record_scalar_call_arguments_exit`,
    `pass/boundary/static_foreign_scalar_result_locals_exit`,
    `pass/boundary/later_state_requirement_scalar_result_exit` (exit 70).
  - **Loan locals and projected storage.** **REFERENCE-LOCAL-BINDING-ROUTES**
    owns local loan establishment, reads, returned references and state
    forwarding; [representation-view checking](omega-rust/psi/compilation/checking/recasts.md)
    owns typed recasts. Preserve loan identity and exact backing through
    every consumer, including local-rooted stores and borrowed-record calls
    followed by reads. `is_record_value` in
    `.../04_typed-trees-to-checked-trees/src/values/scalar/computations/structural_values.rs`
    still gates on StructLiteral/Match roots; generalize the operation rather
    than rewriting a local observation into a different subject. Fresh
    linear-record custody and non-vacated replacement must preserve bounds,
    layout, loans and claim lifecycle. Keep:
    `pass/calls/mixed_projection_sources_in_one_call` (0),
    `pass/calls/scalar_local_record_view_call_exit` (70),
    `pass/termination/runtime_stored_exclusive_endpoint_exit` (0),
    `pass/borrows/local_reference_record_transfer_exit` (70),
    `pass/borrows/projected_reference_record_{argument,residual}_exit` (29).
  - **Aggregate construction, replacement and reads.** Finish runtime/case
    projections of stored references and opaque incoming referents that need
    retained inner-loan evidence; a reference-shaped type alone must not
    invent it. Omega's
    `omega-rust/omega/compilation/pipeline/02_abstract-operations-to-target-operations/src/lowering/control_flow/borrowed_windows.rs`
    needs exact activation/incoming storage and retained descriptor origins;
    complete missing provenance cases, including values arriving as owned
    parameters/block arguments, without double disposal or a fabricated move
    window. Unbounded `UInt`/`Int` fields have no runtime
    representation and stay rejected. `pass/filesystem/windows_canonicalize_exit`
    is recorded rejected ("performed write has unknown destination origins")
    and needs its stored UnitResult; coordinate **FILESYSTEM-RELEASE-CONTRACT**,
    **WRITE-ONLY-BORROW** and **NOMINAL-FIELD-FLOW**. Keep:
    `pass/collections/case_array_literal_field_store_exit`,
    `pass/expressions/borrow_carrying_data_field_exit`,
    `pass/borrows/stored_reference_projection_exit`,
    `pass/borrows/nested_reference_crossings_exit`,
    `pass/collections/runtime_{projected_element_descriptor,owned_descriptor_parameter}_read_exit`
    (0; projected views keep their live extent and backing through borrowed
    and owned record arrivals), with
    `fail/slices/projected_descriptor_index_requires_live_bound` and
    `fail/borrows/projected_descriptor_retains_backing_loan`,
    `pass/borrows/runtime_reference_array_pressure_exit` (70; call-free
    frames must preserve allocated link-register writes as well as call effects),
    `pass/collections/runtime_whole_array_value_copy_exit` (70),
    `pass/collections/{empty_record_array_borrow_exit,nested_empty_record_array_borrow_exit}` (0),
    and rejections `fail/borrows/{stored_reference_live_source_write,nested_reference_live_source_write}`.
  - **Claim-bearing results and native receiving.** A structural call result
    binds the claim the callee's returning edge holds at each result path; an
    inline claim-bearing construction is an anonymous owner whose claims
    transfer whole to its call; a projected owned argument carries the claims
    beneath its path; a state's entry claim continues the claim its arriving
    edge carries. `pass/ownership/{linear_call_result_claims_exit,linear_ambiguous_state_result_mapping,linear_transparent_record_frontier}`,
    `pass/ownership/linear_{inline_constructed_argument_claims,result_member_projection_arguments,result_owners_residual_cleanup,call_result_place_assignment}_exit`
    and `pass/ownership/linear_{state_graph_call_arguments,state_graph_claim_transport,computed_call_claim_transfers}_exit`
    run natively. Keep the shared receiving-value join across record, case and
    fixed-array construction: `linear_inline_constructed_argument_claims_exit`
    pins nested claims, multiple claims per element and authored effect order;
    `linear_computed_call_claim_transfers_exit` pins computed operands.
    Reject second use, missing child consumption and wrong array type/extent
    in `fail/ownership/linear_{constructor_call_member_second_use,array_constructor_duplicate_operand,array_member_scope_loss,array_call_argument_wrong_element,array_call_argument_wrong_extent,computed_boundary_second_use,computed_requirement_argument_second_use}`.
    Keep `pass/ownership/linear_computed_requirement_argument_exit` at 0 in
    both engines: linear arguments to direct requirement calls in helpers,
    statements and local initializers, and mixed borrowed/owned helper
    arguments, retain custody.
    Preserve projected replacement and sibling custody in
    `pass/ownership/linear_call_result_place_assignment_exit` and reject
    `fail/ownership/linear_replaced_field_second_use`. Preserve ordinary
    qualified result, state-return and disposal custody in
    `pass/ownership/linear_call_result_claims_exit`; missing result membership
    and reuse of the transferred original must reject.
    `pass/effects/structural_callback_reach/projected.omg` is only a control.
  - **Saved observations and arithmetic.** Stable unwritten mutable carriers
    contribute saved facts in
    `omega-rust/psi/compilation/checking/src/proof_contracts/contract_entailment/ranking_range/saved_arguments.rs`;
    finish snapshots across writes and exclusive exposure, retaining
    case/index/generic/reference/float identity. Share capture with
    **CRASH-CONTRACT**; loop invariants must cover backedges. Exclusive computed
    ranges use ordinary view formation and call/state-edge evaluation:
    keep `python3 tools/corpus_gate.py --both --filter
    runtime_call_endpoint,runtime_inclusive_call_endpoint` (70),
    `fail/slices/inclusive_*` for bounds/overlap rejection, and
    `layouts/runtime_reborrow_preserves_store_origin_exit` (70) for address
    origins across shared-value and write-prefix queries. Carry exact
    anonymous arithmetic through generic, boundary, aggregate, parameter,
    float and proof uses with result carrier/policy and warning origins.
    **OPERATOR-MACHINE-SUPPLY** owns executable selected helpers.
  - **Patterns and dispatch.** Record/case destructuring executes bind-all,
    `as _` waivers, `__` field names, renamed fields and rest opt-out, with
    non-place subjects evaluated once
    (`pass/control_flow/runtime_nonplace_record_pattern_single_evaluation_exit`
    exits 70 natively and in Terminal). Keep
    `pass/data/record_pattern_let_exit`,
    `pass/control_flow/{ordered_unconditional_arm_exit,record_pattern_arm_rename_guard_exit,arm_pattern_rest_optout_exit,case_pattern_rename_waive_exit}`,
    `pass/borrow/runtime_exclusive_loan_local_write_exit` and
    `pass/calls/runtime_value_call_nested_entry_call_exit` (exit 70).

  Acceptance: a composed caller tolerates renamed/reordered states and
  inserted computations; operands execute left-to-right once, skipped calls
  never run, and saved values agree between the checked interpreter and
  native execution. Explicit state transfers preserve custody without
  mandatory copies; implicit cross-state use, conflicting loans, missing
  provider authority and stale snapshots reject. Keep guard-operand write
  invalidation; a failed custody join never falls back to a weaker
  recognizer. Keep `7 / 2 * 2 == 7` and `4097 / 4096 * 4096 == 4097` with
  their warnings, reject fractional integer landing, and keep typed
  truncation (`4097u32` gives 4096), signed quotient/remainder laws and
  zero-divisor rejection.

- **BORROWED-STORAGE-RESTORATION.** (split-of:OMEGA-PRODUCT-COMPILER-SOURCE)
  Complete consuming-transform and replacement execution under
  [ownership windows](wiki/spec/language/ownership.md#borrowed-storage-invariant-windows)
  and [borrowed-storage restoration](wiki/spec/terminal-psi/ownership.md#borrowed-storage-restoration).
  - Qualify borrowed byte-storage state joins on Windows x86-64 and
    matching-host Linux x86-64. Keep
    `pass/borrows/runtime_record_byte_region_loan_exit` (50),
    `pass/collections/borrowed_byte_array_state_{reads,writes}_exit` (70),
    `pass/text/borrowed_bounded_byte_indexed_write_exit` (70),
    `pass/text/qualified_byte_owner_state_loan_cycle_exit` (70), and
    `pass/control_flow/runtime_local_record_array_byte_render_exit` (0)
    for loan transport across direct and conditional successors.
    Reproduce with `python tools/corpus_gate.py --both --filter <fixture>`
    on each bound host; byte-owner loans retain their own qualifications,
    rather than becoming pointer/length views.
  - Where: `OpenWindows::replace` in
    `omega-rust/psi/compilation/pipeline/04_typed-trees-to-checked-trees/src/execution/terminal_unit/borrowed_windows.rs`
    separates exact-place restoration debt from detached value custody;
    direct and named call results already use ordinary owned result
    bindings.
  - Missing: qualify remaining referent windows and keep evaluation order,
    disjoint siblings and exact open-place agreement across recoverable failure,
    suspension/resume/cancellation and process-exit abandonment;
    do not invent rollback. Qualify those remaining paths on supported hosts.
    Keep `pass/collections/runtime_bounded_byte_reference_{read,forward}_exit`
    (70) for shared observation and mutable forwarding over caller-backed
    fixed byte storage in native and Terminal execution; qualify the remaining
    supported hosts without treating bounded loans as pointer/length views.
    Keep `pass/recast/runtime_inline_{shared_slice_record_read,slice_view_mutable_write,slice_recast_call}_exit`
    (70) for captured regions through state arrivals and scalar-returning
    call operands, including affine descriptor retirement before a new loan;
    qualify their remaining bound hosts.
    The ordinary cases below still need Windows x86-64 and matching-host
    Linux x86-64 execution; the bounded
    [Linux qualification](wiki/drafts/reference/rust_compiler_completion.md#borrowed-storage-linux-qualification)
    qualifies native ARM64 normal paths, including captured inline slice
    regions through states and scalar calls, stored descriptor observations,
    repeated carried-loan repair, direct and nested consuming replacements,
    ordinary generic success/failure carriers and operand ordering, but not
    physical suspension/resume/cancellation.
    Keep `pass/ownership/runtime_{captured_replacement_selector,captured_construction_selector,captured_selector_call,borrowed_captured_selector}_exit`
    (exit 0) for pre-RHS destination capture and exact-once target computation
    through old-value cleanup and store despite later out-of-range selectors.
    Keep `pass/drops/runtime_borrowed_selected_cleanup_postcondition_exit`
    and `fail/drops/borrowed_selected_cleanup_missing_child_requirement` for
    parent-first guarantees and child prerequisites at that captured receiver.
    Keep `pass/ownership/runtime_{borrowed,whole_borrowed}_window_crash_exit`
    (Trap) for projected and retained-alias whole-referent abandonment without
    repair or normal cleanup. The whole-referent interpreter traps, but this
    macOS host times out reaping the native trap on the earlier compiler too;
    retain its oracle and qualify on hosts that can reap trap exits (the same
    host limitation recorded under **TRANSITION-TAIL-CALL-TO-MACHINE**).
    Keep `fail/ownership/borrowed_window_crash_does_not_repair_return` for a
    returning sibling that still owes restoration despite a later crash.
    Keep `pass/ownership/runtime_borrowed_projected_linear_return_exit` (0)
    and `runtime_nested_projected_claim_return_exit` (0) for selected-claim
    relocation, retained siblings and by-path call binding. Keep
    `runtime_borrowed_case_result_restore_exit` (0) for ordinary success/failure
    sums restoring a detached linear token and its original reference backing
    on both arms through selected cleanup, and `runtime_linear_case_payload_forward_exit` (0)
    for unexamined sums and projected aggregate arguments. Keep
    `runtime_generic_borrowed_case_restore_exit` (0) for generic affine
    outcome carriers retaining the original backing across both repairs;
    qualify its remaining bound hosts. Keep
    `pass/drops/runtime_owner_scoped_generic_cleanup_exit` (0) for universal
    owner hooks across distinct static argument tuples and observable writes
    through retained borrowed fields; qualify its remaining bound hosts.
    Keep `runtime_generic_linear_case_restore_exit` and
    `runtime_linear_constructor_instances_exit` (0) for fresh and forwarded
    conserved claims, applied-owner cleanup and scalar-leaf observation;
    `runtime_borrowed_value_inference_exit` (0) constrains direct borrowed
    argument inference, constructed temporary homes and lifetime-tagged
    reborrows through generic calls.
    Keep `fail/ownership/linear_case_result_second_transfer` and
    `generic_linear_constructor_{second_transfer,keeps_backing_loan}` for
    exact-once returned custody and retained backing loans, including
    `generic_borrowed_value_retains_backing_loan` through a type-only binder.
    The result programs still need Windows x86-64 and
    matching-host Linux x86-64 execution; inactive cases owe no fabricated
    claims. Suspension/resume/cancellation qualification depends on the
    selected runtime's **TR3-TR8** execution and retained-frontier joins.
    Keep `runtime_known_case_scalar_call_exit` and
    `runtime_borrowed_sum_identity_restore_exit` (0) for typed-path call
    admission and stored-tag extraction; qualify these on remaining hosts.
    Keep `runtime_borrowed_sum_retag_exit` and
    `runtime_borrowed_sum_empty_restore_exit` (0) for returned caller-local
    claim rosters, empty/payload case changes, computed mutating calls and a
    previously moved disjoint sibling; qualify remaining hosts.
    Keep `runtime_view_parameter_element_window_exit` (0) for copy and affine
    whole-element and element-field windows on a `&mut [T]` parameter's own
    loan, bounded by the view's live length, with
    `fail/ownership/view_element_window_{unproven_index,live_borrow,missing_repair}`;
    qualify remaining hosts. Keep `runtime_local_subslice_element_window_exit`
    (0) for local and nested subslice offsets, field writes, computed copy
    replacement and runtime-indexed affine extraction/repair through computed
    immutable copies and constant shifts with untouched siblings; keep
    `fail/ownership/local_subslice_window_wrong_repair` for an index that
    does not restore the hole. Keep `borrowed_window_changed_selector` and
    `runtime_window_fixed_observation` for changed mutable selectors and
    observing a fixed element that a runtime-selected hole may overlap.
    Keep `runtime_computed_selector_snapshot_window_exit` (0) and
    `computed_window_selector_keeps_snapshot` for captured computed values
    after their mutable initializer changes; they are not aliases of that
    live storage. Keep `computed_window_selector_requires_builtin_meaning`
    for a selected authored operator whose result is not the builtin sum.
    Keep `runtime_mutable_selector_window_exit` (0) for unchanged mutable
    selector captures, disjoint intervening writes and detached copies after
    an alias write; keep `borrowed_window_selector_alias_write` and
    `borrowed_window_mutable_selector_observes_hole` for changed versions and
    observing an absent selected element. Keep
    `runtime_referent_replacement_fact_transport_exit` (0) for nested computed
    replacement facts through a retained alias after the source changes, and
    `fail/contracts/referent_replacement_false_guarantee` for retiring old and
    overwritten referent facts; qualify the remaining native hosts. Keep
    `runtime_replacement_call_child_snapshot_exit` (0) and
    `fail/contracts/replacement_call_child_false_snapshot` for completed helper
    results through nested replacement and array return, without stale facts
    or false exact values from intervals; qualify remaining native hosts. Keep
    `pass/drops/runtime_narrowed_cleanup_receiver_exit` (0) for authored
    prerequisites at a captured local subslice receiver, exact replacement
    result guarantees and untouched siblings; keep
    `fail/drops/narrowed_cleanup_requires_actual_element` and
    `fail/contracts/{aggregate_result_false_guarantee,projected_result_guarantee_after_overwrite}`
    for wrong-coordinate, false-producer and stale-result rejection. Qualify
    these cleanup prerequisites on the remaining native hosts.
    Qualify runtime-selected cleanup on the remaining native hosts. Keep
    `pass/drops/runtime_case_selected_cleanup_continuations_exit` (0) for
    captured scalar returns, computed temporaries, parent retention and
    projected return/state-transfer custody across selected cleanup blocks.
    Keep direct consuming replacements in `runtime_borrowed_sum_retag_exit`
    and `runtime_borrowed_record_transform_exit`, plus
    `runtime_borrowed_operand_order_exit` (0) for scalar evaluation before
    extraction and `fail/ownership/borrowed_operand_later_read` for reversed
    order rejection; qualify remaining hosts.
    Keep `runtime_nested_borrowed_consumption_exit` (0) for nested consuming
    calls restoring both normal branches and
    `fail/ownership/nested_borrowed_operand_later_read` for an observation
    after the inner call vacates borrowed storage; qualify remaining hosts.
    Keep `runtime_borrowed_sum_empty_restore_exit` as the direct empty/full
    assignment control; do not explicitly detach and drop its old value.
  - Consumers: the product parser and **BUMP-ALLOCATOR-CANARY**.
  Acceptance: consuming transforms, replacement values and repair on both
  branches execute with caller-visible updates and exact-once custody in the
  common Terminal core and supported native targets, without staging caller
  storage. Keep `pass/ownership/runtime_borrowed_record_transform_exit`
  (exit 42), `pass/ownership/runtime_borrowed_projected_claim_transform_exit`
  (exit 42) and `pass/ownership/move_keyword_field_assignment` (exit 41).
  Keep `pass/borrows/runtime_record_carried_loan_state_return_exit` (42) as
  the ordinary-state return-origin control and
  `fail/ownership/borrowed_storage_repair_keeps_carried_loan` as the backing
  mutation rejection after carrier restoration.
  Keep `pass/borrows/runtime_returned_reference_array_bounded_read_exit` (70)
  and its nested, forwarded and mutable-reference controls for stored-carrier
  custody and caller-backing transport; qualify their remaining native hosts.
  Keep `pass/ownership/runtime_carried_loan_field_replacement_exit` (42) for
  replacement's new backing, retained sibling and nonzero storage offset, and
  `fail/borrows/replaced_carrier_keeps_new_backing_loan` for the installed loan.
  Keep `pass/ownership/runtime_borrowed_carried_loan_roundtrip_exit` (42) for
  observing primitive backing through the restored incoming descriptor.
  Keep `runtime_borrowed_{carried_loan_transform,carrier_repeated_repair,shared_alias_carrier_transform}_exit`
  (42) for consuming call results, repeated repair and complete multi-source
  descriptor groups through the borrowed referent, and
  `fail/borrows/record_result_retains_declared_lifetime_origins` for retaining
  candidates a body does not select.
  Keep `pass/ownership/runtime_whole_borrowed_transform_exit` (42) for
  whole-parameter extraction and both fresh and identity result repair, and
  `pass/ownership/runtime_retained_alias_referent_transform_exit` (42) for
  retained and forwarded aliases, projected nonzero-offset caller storage
  and nested linear cleanup, and
  `fail/ownership/whole_borrowed_{missing_repair,stale_observation,shared_extraction,live_descendant,forwarded_live_descendant}`
  for exact restoration and exclusive-route admission.
  Missing repair, early return, stale reads, overlapping loans, wrong-place
  repair, repeated extraction and whole-owner cleanup reject, including
  `fail/ownership/borrowed_storage_boundary_call` and
  `fail/ownership/linear_temporary_scalar_observation`.

- **CML4.** Complete edge-local residual cleanup under the
  [ownership contract](wiki/spec/terminal-psi/ownership.md). Materialize
  outgoing values before transfer/disposal; give every occurrence one
  disposition. Clean dying roots in reverse declaration order and abandoned
  construction prefixes in
  [reverse establishment order](wiki/spec/language/ownership.md#construction-and-disposal-order).
  Crash/abort/process exit has no cleanup successor. Remaining work:
  - Child reconstruction below a conditionally transferred owner is not a
    capability gap: the checker rejects every store below a transferred
    region ("assign a whole value to `pair` to re-establish it"), as
    ownership.md licenses no value with a hole, so the stage-04 refusal
    behind it is unreachable. A zero-valid declared local is established
    with its zero value and owes its hooks (`fail/drops/{empty_array_owner,
    zero_case_container_keeps}_cleanup_prerequisite`); only a zero-gated
    type defers establishment to its stores.
    Preserve each occurrence's residual custody until its actual normal
    death; neither a selection join nor a later reread of the condition
    decides hook timing. Keep
    `pass/drops/runtime_selected_transfer_fieldwise_construction_exit` as
    the deferred-binding transfer control (a field consumed out of a
    field-wise established local, hook order byte-exact) and
    `pass/drops/runtime_selected_transfer_incomplete_construction_exit` as
    the selected-transfer-during-construction control: a retained aggregate
    that completes folds and dies in declared order, one that never
    completes dies in reverse establishment order. Keep
    `pass/drops/runtime_selected_partial_owner_cleanup_exit` for generic
    record/fixed-array regions, interleaved residual hook order, field/whole
    replacement, optional mixed payloads and exact old/new linear claims. Keep
    `fail/drops/selected_partial_{cleanup_prerequisite,replacement_old_cleanup_prerequisite,requires_disposition}`,
    `fail/drops/selected_replacement_old_cleanup_prerequisite` and
    `fail/ownership/selected_partial_owner_reuse` for skipped and pre-store
    obligations and unavailable regions. Keep
    `pass/drops/runtime_selected_nominal_owner_cleanup_exit` for whole-owner
    Boolean selection, nested/branch-only births, overwritten conditions and
    once-only argument effects followed by linear construction/forwarding.
    Keep `fail/drops/selected_nominal_owner_keeps_cleanup_prerequisite` and
    `fail/ownership/selective_constructed_owner_reuse` for skipped-path hook
    prerequisites and conservative source availability. Preserve
    `pass/ownership/runtime_selective_constructed_owner_custody_exit` for
    no-code custody, generic records and field/indexed replacement before
    selected whole transfer.
  - Qualify the ordered normal-edge cleanup plans in
    `omega-rust/psi/compilation/pipeline/04_typed-trees-to-checked-trees/src/execution/terminal_unit/cleanup/normal_edges.rs`
    on Windows and Linux, including abandoned construction prefixes and
    field replacement. Preserve establishment order in the ordinary action
    stream; keep machine parameter custody distinct from edge cleanup. No
    runtime liveness flags, untouched-array leaf expansion or deferral to final
    return. Keep
    `pass/drops/runtime_projected_nominal_cleanup_exit` (`XDCBEA`) and
    `pass/drops/runtime_indexed_nominal_cleanup_exit` (`XGFDCBEA`) as
    value-transfer and recursive cleanup-order controls.
    Keep `pass/drops/runtime_cleanup_helper_normal_guarantee_exit` as the
    pre-cleanup scalar-capture and surviving call-contract control when
    extending these edges.
    Keep `runtime_empty_array_owner_cleanup_exit` and
    `runtime_nested_empty_array_owner_cleanup_exit` as zero-cardinality
    initialization and containing-owner cleanup controls. Empty element storage
    does not owe a value initialization or element cleanup prerequisite.
    Keep `runtime_record_construction_cleanup_order_exit` as the
    authored-output control and `runtime_replaced_record_custody_transfer_exit`
    as the owned/borrowed replacement followed by whole-record transfer control.
    Keep
    `runtime_nested_cleanup_postcondition_exit` and
    `runtime_generic_reserved_construction_exit` as parent-first guarantee and
    construction controls. Keep `runtime_abandoned_record_construction_prefix_exit`
    as the reserved-home versus established-value control; unwritten fields owe
    no cleanup. Keep `runtime_borrowed_nominal_replacement_exit` with
    `runtime_nominal_array_replacement_exit` for materialization before old
    element hooks and stores, and
    `fail/drops/borrowed_replacement_old_hook_prerequisite` as the old-value
    prerequisite control. Preserve exact source-window call occurrences until
    cleanup effects finish; an incomplete effect table cannot decide whether
    a call may cross an open window. While one is open, preserve the exact
    vacant-place repair authority from the ownership contract.
    Keep `runtime_owned_replacement_reestablishment_exit` and
    `runtime_owned_{record_replacement,nominal_replacement,replacement_composition}_exit`
    and `fail/drops/owned_replacement_{old_hook_prerequisite,transferred_source_read}`
    as value-home, occurrence and invalid-custody controls.
  - Reference-bearing nominal owners remain excluded by
    `checking/src/machine_calls/reference_result_custody.rs`; admitting their
    intact receiver also reaches `InvalidNominalAffineCleanup` in the retiring
    replay. Preserve receiver custody through the ordinary callable path as
    that replay is removed, rather than expanding its bounded receiver model.
  - Complete scalar materialization of moved linear places and claim-bearing
    call results on **STATE-LOCAL-VALUE-FRONTIER**'s computed-call path. Preserve the
    checked ordered temporary action roster and its scalar-call continuation in
    `omega-rust/psi/compilation/pipeline/05_checked-trees-to-lowered-psi/src/scalar_lowering/computations/calls.rs`
    with exact live-claim dispositions; claim-free discard/hook actions do not
    consume retained claims. Preserve the selected transfer and ordered
    residual complement under the original owner, never a second whole
    temporary for the selected field.
    Keep `pass/drops/runtime_claim_bearing_temporary_cleanup_exit` and
    `pass/drops/runtime_nested_claim_bearing_temporary_cleanup_exit` as native
    and Terminal controls for conditional establishment, descendant identity
    transport, disjoint affine complements, exact linear hook disposition and
    constructor snapshots across implicit borrowing.
    Keep `runtime_parent_hook_preserves_linear_descendants_exit` as the intact
    nominal receiver and retained child-claim control, with child cleanup
    prerequisites established by the parent hook's conjunction. Keep
    `fail/drops/parent_cleanup_{missing_child_guarantee,wrong_child_guarantee,cross_field_guarantee_retired,disjunction_not_conjunction,requires_before_guarantees}`
    as missing/wrong facts, relation invalidation, disjunction and hook-entry
    prerequisite controls.
    An anonymous returned owner without a unique conserved claim map must
    still reject rather than mint replacement identities.
    Preserve existing identities through moved fields and exact call-result
    maps; new per-field identities cannot replace forwarded claims. Keep
    `pass/ownership/runtime_partial_linear_record_case_cleanup_exit` as the
    selected residual and home-retirement control after linear field consumption.
    Keep `fail/drops/{claim_bearing_temporary_requires_disposition,borrowed_temporary_projection_retains_owner_disposition,claim_bearing_temporary_hook_prerequisite}`
    and `pass/drops/statically_skipped_claim_bearing_temporary` as whole-owner
    disposition and evaluated-occurrence controls. Keep
    `pass/drops/runtime_nested_nominal_producer_loans_exit` as the operand-loan
    lifetime control: cleanup must not prolong a completed borrow-free
    producer's input loans.
  Acceptance: add corpus fixtures where native runs and the Terminal
  interpreter preserve values and hook order across composed edges, including
  a projected-argument jump edge closing residuals in order. Missing,
  duplicate, reordered, overlapping, wrong-root and post-transfer cleanup,
  and transfer after disposal, reject. Fuel exhaustion commits no disposal;
  retry cannot repeat one. No affine occurrence outlives its required edge.
  The output-bearing projected, indexed and composed controls run with exit 0
  and authored hook-order stdout in both native and Terminal execution on
  macOS ARM64. Keep `pass/drops/runtime_projected_nominal_core_exit` and
  `pass/drops/runtime_composed_nominal_return_hook_exit` as
  host-independent native/Terminal value-preservation controls; they do not
  establish observable hook order.
  Preserve those observable controls with `python tools/corpus_gate.py --both --filter
  runtime_projected_nominal_cleanup_exit,runtime_indexed_nominal_cleanup_exit,runtime_composed_nominal_output_hook_exit`.
  Tree comparison is being retired and does not establish Terminal acceptance.

- **ATOMIC-MEMORY-MODEL.** Complete atomic/fence semantics and native execution
  under [concurrency](wiki/spec/language/concurrency.md) and the
  [atomic vocabulary](omega-rust/psi/foundation/language-core/atomic_operations.md).
  Serial load/store/fetch/swap/decisive exchange already execute as single
  Terminal events (`omega-rust/psi/compilation/representations/terminal-psi/src/terminal_module/control_flow/atomic.rs`);
  Omega stage 00 retains location, orderings and coherence edges
  (`omega-rust/omega/compilation/pipeline/00_terminal-psi-to-abstract-operations/src/lowering/machine/operation/atomic_events.rs`),
  but its unique-reaching-write restriction is not a concurrent memory model.
  Current state: `pass/atomics/runtime_atomic_{fetch_add,compare_exchange}_exit`
  exit 70 natively on macOS ARM64 and Linux x86-64;
  `pass/atomics/shared_receiver_atomic_store` exits 70 on macOS ARM64 but is
  rejected on Linux x86-64 and Windows x86-64; `samples/cli/systems/atomics_cross`
  runs with exit 70; `pass/atomics/atomic_global_order_operations` only checks
  (no executable entry).

  - Target operations, selection/encoding and checked refinement for every
    admitted width, alignment and ordering. Swap/fetch return the
    instruction-observed prior, never a separate load. Preserve
    observing/non-observing and decisive/single-attempt behavior, failure
    ordering, `Uncommitted` custody and target retry-work attribution.
  - Source/Terminal event production for bounded atomic fields and
    single-attempt exchange: the abstract single-attempt carrier has no
    Terminal producer. Preserve the location-free fence event and its ordering
    through target refinement, including any future instruction-free realization.
  - Checked `sequenced_before`, `reads_from`, `modification_order`,
    `synchronizes_with`, `happens_before` and `global_sequential_order`,
    fence synchronization and outcome-sensitive RMW obligations, keeping
    existing serial coherence. Genuinely unsettled axioms go to the spec owner.
    Model and target work can proceed now; **TR3-TR8** gates the concurrent
    publication, fence-pair, global-order and compare-exchange contention
    witnesses.

  Acceptance: `shared_receiver_atomic_store` and `atomic_global_order_operations`
  (given an entry) execute on every matching host; missing or substituted
  relations, locations, orderings or outcomes reject in checking; concurrent
  witnesses run once **TR3-TR8** lands. Serial tests and bounded exploration do
  not prove a memory model. Keep strong `Receive` unless an exact protocol proof
  permits weakening; device/DMA and interruption ordering keep separate contracts.

- **SAMPLE-CORPUS.** Close every maintained `samples/cli|gui|uefi` project
  through checked semantics, native publication for its authored targets, and
  documented exit/output on matching hosts under the
  [release matrix](wiki/drafts/reference/rust_compiler_completion.md#release-matrix).
  **SQUALR-HEADLESS** owns `samples/apps/squalr`; **CANARY-CORPUS** owns language
  fixtures; GUI product acceptance is **MACOS-APPLICATION-PUBLICATION**. This
  item keeps sample integration until each full program works, even when the
  capability repair has another owner.

  Gates: `python tools/sample_gate.py` (checked sweep; `--filter`, `--cli`,
  `--write-results`) and `python tools/sample_native_gate.py --cli <binary>
  --target <host> --jobs 1` (local review, build, documented exit/output).
  Pending review decisions accepted by the native gate are measurement, not a
  dependency audit. Samples without an oracle stay unscored; freestanding
  samples (`samples/uefi/uefi_hello`) need their own runner. Reproduce with the
  current CLI and the sample's own package/entry inputs; a stale binary, an earlier review
  refusal or an effect-free reduction does not exercise the same route. A trace
  phase records the last route entered and can misidentify the declining
  operation.

  Host coverage still owed:
  - std `Clock` on Linux (`rendering/{bouncing_ball_2d,mandelbrot_zoom,bouncing_particles}`):
    `tests/omega/pass/host/std_clock_sleep_exit` exits 70 on macOS ARM64 and
    builds for both Linux targets; it needs actual 200 ms waiting and exit 70 on
    each Linux host, as does `tests/omega/pass/providers/kernel_import_result_chain_exit`.
  - Clock-bearing samples pass on macOS ARM64; verify the other declared hosts:
    `algorithms/{binary_search_viz,sort_visualizer}`,
    `rendering/{bouncing_console,ripple_field,tick_marquee}`,
    `simulation/{cellular_automaton,game_of_life_glider,langtons_ant,random_walk}`.
  - `arithmetic/stats_compute`: both Linux hosts still need the
    [runtime oracle](samples/cli/arithmetic/stats_compute/README.md#runtime-acceptance),
    preserving the Windows x86-64 and macOS ARM64 output and exit checks.
    Non-tail self recursion is not a reproduced blocker.
  - Text/index/match programs: missing operations belong to
    **NOMINAL-FIELD-FLOW**, **WRITE-ONLY-BORROW**, **STATE-LOCAL-VALUE-FRONTIER**,
    **MATCH-SELECTIVE-LOWERING**, **CML4** and **OPERATOR-MACHINE-SUPPLY**. Keep
    `Utf8` and ordinary domain evidence; recognized function names or weakened
    invariants cannot replace it.

  Distinct product oracles (paths under `samples/cli`); retain Windows x86-64
  and macOS ARM64 checks and close the stated remaining host obligations:
  - `cli_mvp`, `euclid_gcd`, `generic_counters`, `number_guess`: exact output,
    exits 0/12/16/70; interactive EOF/Enter behavior, both lines, prompt timing,
    empty stderr. Both Linux hosts remain owed.
  - `basics/print_squares`: rows `001,004,...,081` in order, exit 0, EOF/Enter
    completion; the banner's `081` alone is not an oracle. Both Linux hosts
    remain owed.
  - `recursive_sum`: the [exact 144-byte output](samples/cli/arithmetic/recursive_sum/README.md),
    exit 70; both Linux hosts remain owed.
  - `systems/framed_payload`: the [exact-output oracle](samples/cli/systems/framed_payload/README.md),
    exit 60; both Linux hosts remain owed.
  - `simulation/calendar`: the 277-byte/exit-0 oracle in
    [its README](samples/cli/simulation/calendar/README.md); a banner containing
    `30` is insufficient. Both Linux hosts remain owed.
  - `longest_run`, `parse_int`, `roman_numeral`, `string_hash`, `substring_search`:
    exits 4/150/70/40/4 per the [scanner](samples/cli/algorithms/longest_run/README.md)
    and [text](samples/cli/text/README.md) READMEs; Windows x86-64 and both
    Linux hosts remain owed.
  - `proofs/math_proofs`: ordinary multiset extraction and checked proof terms;
    the false twin rejects. `source/library/core/seq.omg` is not a Bag.
  - Bounded Console input under [bounded input](wiki/spec/resources/bounded_input.md):
    exact prefix/destination, once-only effects, LF/EOF/Full, zero-capacity
    non-consumption, untouched tails, alias rejection.

  The `gui/window_app` and `gui/windowed_calculator` sweep timeouts are
  compile cost, not interactive waiting. On macOS ARM64 (debug CLI,
  2026-10-07 main), a scratch copy of `samples/gui/windowed_calculator`
  reviews in 503 s, and `omega --timings --target macos_arm64` then
  publishes `windowed_calculator.app` in 463 s: 9 s checking std, 142 s
  checking the sample package, 136 s in settle (CheckedTrees ->
  SelectedExecution) and about 175 s after the native-realization proposal,
  which `--timings` does not break down. Its runtime oracle still needs a
  desktop session.

  Acceptance: every maintained sample checks, every applicable native product
  builds, and documented runtime oracles pass on every required matching host.
  Record unavailable hosts explicitly; filtered successes do not establish
  coverage.

- **CANARY-CORPUS.** Close `tests/omega/{pass,fail,run}` at each fixture's
  promised stage under the
  [release matrix](wiki/drafts/reference/rust_compiler_completion.md#release-matrix),
  with `python tools/corpus_gate.py` (`--filter fail/` for diagnostics,
  `--native` / `--both` on each matching host). Harness:
  `omega-rust/omega/compilation/driver/tests/corpus_runner.rs`; attribution per
  [testing](tools/testing.md#attributing-a-move). **SAMPLE-CORPUS** owns
  samples; non-corpus gates belong to [release closure](#rust-compiler-release-closure).
  The check leg already realizes rooted fixtures; native execution needs a
  matching host and a run-tier fixture, `_exit` suffix or
  `expected_native_exit.txt`. `checked` is never execution evidence.

  Roster and oracle accounting:
  - The runner invokes production application/compiler/package/interpreter
    facades; it never parses `build.omg`, synthesizes package identities,
    discovers target bindings, infers dangerous services or mints accepted
    semantic bindings. Delete `driver/tests/support/fixture_package_inputs.rs`,
    `console_acceptance.rs`, `dangerous_service_acceptance.rs` and
    `process_exit_acceptance.rs`. Package fixtures run the real isolated
    update/review/lock workflow through package-manager APIs before ordinary
    compilation. Language-only fixtures use the compiler facade with an
    explicit constructed default Build. A green corpus therefore exercises a
    workflow users can run, not repository-only trust policy.
  - The harness does not independently project realization targets from
    `build.omg`; product requests and reports expose the selected targets. Keep
    fixture discovery, sharding, timeouts and oracle comparison as test-owned
    concerns, but remove every duplicated source/build semantic query.
  - Reconcile added/removed fixtures with the check and per-host native
    goldens after attributing their outcomes, without treating a missing row
    as a run.
  - Keep one portable check golden without hiding semantic differences:
    host-selected providers can change a refusal; attribute it rather than
    repin one host over another. Keep missing rows, wrong-reason negatives,
    crashes and timeouts visible.
  - Fixture package locks and admissions stay local; a committed foreign
    `omega.lock` blocks the ordinary command
    (`pass/calls/local_records_mixed_record_result_arguments_exit` README).
  - Separate proof/declaration fixtures from executable customers:
    `pass/terminal_psi/content_custody_exit` invokes `Root::enter` but has no
    `build.omg` or oracle; `pass/core/content_{conservation_contract,retained_custody_round_trip}`
    need an invoking customer (routed establishment: **BOUNDARY-ISSUANCE**);
    `pass/proofs/quotient_{define,lift}_managed_compile` stay proof-only; the
    separate executable customer is `samples/cli/proofs/quotient_runtime_carrier`.

  Diagnostic acceptance:
  - **IMPLICIT-STORE-CONVERSION** owns duplicate conversion/weakening reports in
    `fail/calls/terminal_return_type_mismatch_rejected`,
    `fail/expressions/bool_numeric_operand_mixing_rejected` and
    `fail/arithmetic/{suffix_type_disagrees,undeclared_bare_name}_rejected`.
  - Keep `fail/arithmetic/narrowing_anonymous_expression_unproven` (range
    obligation) distinct from `narrowing_signedness_rejected` (carrier), and
    the `fail/modules/duplicate_data_keeps_guard_facts` control.

  Shared failures, by owner (rerun the unchanged fixture after each repair):
  - Terminal interpreter declines on programs that pass natively
    (`omega run --both` status 201):
    - `run/contained_health_loop_command_branch` has no build.omg or
      ProgramEntry binding and declares its own `Console` boundary trait, so
      it needs a host adapter (**TERMINAL-RUNTIME-DRIVERS**).
    Find a site with an env-gated `std::backtrace::Backtrace::force_capture()`
    in `errors::malformed()`; the last trace before the failure is the refusing
    site. Known limits that still fail closed: aggregate re-views refuse case
    path segments, `ViewByte` storage is read-only, and a literal byte place
    cannot be lent mutably.
  - **OPERATOR-BOUNDARY-CALLS**: `pass/float/{x86_fma_plan_association,build_runtime_semantics_twins_x86_baseline,build_runtime_semantics_twins_windows_x64,named_provider_directed_divide_exit,named_provider_fused_multiply_add_exit,named_provider_multiply_then_add_exit}`,
    `pass/expressions/match_float_{interpretation,subjects}`;
    `pass/float/{float_saturating_arithmetic_exit,float_trapping_overflow_traps,float_trapping_propagated_nan_traps}`
    need their authored policies (`as f32` erases the subject).
  - **ENTRY-CONTENT-ROOTS**: `pass/control_flow/guarded_leaf_branch_expansion`,
    `pass/capabilities/uses_caller_folder` (exact authored Gui/Input/Console
    providers; a same-named bundled provider is insufficient).
  - `pass/capabilities/windows_provides_import_exit` must keep the producing
    statement of `self.m = self.beeper.beep(self.v)` and execute on Windows
    (follow `statement_continuations` in stage 05's attached Unit state graph).
    `pass/providers/external_leaf_dllimport_compile` needs checked per-call
    authority carried into native `NormalizedForeign`, not a fixture permission.
    Keep `pass/filesystem/runtime_local_host_result_dispatch_exit` as the Linux
    effect-ordering customer.
  - **PROOF-CONTRACT-MIGRATION** owns `pass/proofs/cauchy_predicates_compile`
    (recorded rejected on main), generalized fact-only citation erasure and
    the missing native citation witness. Its task below records the remaining
    checks and diagnostic repair; do not treat unpublished checkpoints as a
    corpus result. Keep exact bounds in
    `pass/proofs/proof_inductive_climbing_sum` and its unbounded negative.

  Native customers (under `tests/omega/pass`):
  - `host/runtime_console_bounded_line_exit` and its
    `_{lf,full,partial_eof,middle_window}_exit` siblings: exits 20/13/34/22/70
    with exact stdout on macOS ARM64; remaining Windows and Linux.
    **WINDOWS-CONSOLE-INPUT** owns remaining Windows input/output execution
    qualification and the exact selected foreign-leaf adapters.
  - `host/runtime_gui_foreground_window_exit`: Windows, exact Gui provider,
    create/query/store/destroy, exit 70.
  - `control_flow/runtime_{integer,string}_literal_dispatch_exit`: exits 22/11
    on macOS ARM64 and Linux x86-64; remaining Windows.
  - `wire/runtime_wire_exact_array_without_count_exit`: seven bytes
    `00 00 04 01 02 03 04`, exit 70 on macOS/Linux; remaining Windows
    (**WIRE-RUNTIME-AND-INSTALLATION** owns the operation).
  - `wire/runtime_wire_policy_authored_plan_exit`: the independent eight-byte
    oracle and decoded values exit 70 natively and interpreted on macOS ARM64.
    Linux x86-64 previously ran the round trip; rerun the strengthened byte
    oracle there. Windows execution remains unverified.

  Route other omissions to **STATE-LOCAL-VALUE-FRONTIER**,
  **MATCH-SELECTIVE-LOWERING**, **GENERAL-CYCLIC-EXECUTION**,
  **NOMINAL-FIELD-FLOW**, **BORROW-PROOF-CONVERGENCE**,
  **OPERATOR-MACHINE-SUPPLY**, **MODULE-NAMESPACE-RESOLUTION** or
  **ARITHMETIC-POLICY-REALIZATION** by the missing operation.
  Acceptance: the complete roster reaches promised stages, negatives reject for
  their intended reasons, and exact runtime oracles pass on matching hosts.
  Repinned refusals and checked-only substitutions do not close execution work.

- **WINDOWS-CONSOLE-INPUT.** (split-of:CANARY-CORPUS) Run unchanged
  `tests/omega/pass/calls/runtime_mutable_call_before_transition_args_exit/main.omg`
  on Windows x86-64 with its authored input, native exit 70 and interpreter
  exit 70. The program reads four bytes, completes a mutable call, then
  observes its writes in transition arguments.

  Qualify native input/output and CLI comparison on Windows; macOS can build
  these images but cannot execute them. Owners: std's selected Windows Console
  providers and **TERMINAL-RUNTIME-DRIVERS**. Keep the exact selected imports
  and checked wrappers; provider spelling or matching signature grants no host
  behavior.

  Frontier (w1 witness at fetched main 44e66b7): the MSVCRT binary-mode repair
  (63ba05a4, `_setmode` per read/write) is landed and the corpus gate
  acceptance is freshly witnessed on Windows — Loader exit 70 native and
  interpreted, bounded-line exits 20/13/34/22/70 with byte-exact stdout.
  Edge exercise on built natives: authored four bytes exit 70; empty/short
  input takes the authored failure exit 72 (EOF distinct from failure);
  NUL and CR bytes move raw with byte-exact stdout (no text conversion);
  a failed host read (write-only stdin) traps via `crashes Trap`
  (STATUS_ILLEGAL_INSTRUCTION), never surfaces `LineReadResult::Invalid`.
  Those edge behaviors are now pinned by landed pinned-input fixtures
  `host/runtime_console_bounded_line_nul_crlf_exit` (NUL+CRLF raw),
  `host/runtime_console_bounded_line_cr_midline_exit` (CR mid-line is data),
  `host/runtime_console_bounded_line_eof_repeat_exit` (repeat read past
  EndOfInput stays EndOfInput), `calls/runtime_mutable_call_short_input_exit`
  (2-byte read → exit 72) and `calls/runtime_mutable_call_empty_input_exit`
  (empty stdin → exit 72), witnessed exits 14/21/72 native and interpreted.
  Remaining: the Loader's ordinary `omega run --both` route is blocked behind
  std package admission — the ordinary review raises 285 pending decisions
  (141 callable, 141 external_supply, dangerous_capability grants `filesystem`
  via FilesystemHost and `process` via Console, 1 terminal_permission), all
  audit-recommended; completing that review is an owner/coordinator admission
  decision, not an execution result.

  Acceptance: `python tools/corpus_gate.py --both --filter
  calls/runtime_mutable_call_before_transition_args_exit,host/runtime_console_bounded_line`
  preserves the Loader's exit 70 and the five interpreted exits
  20/13/34/22/70 with authored stdout. Attribute their existing native output
  refusals separately. Exercise empty input, raw bytes and invalid input
  handles, and establish the Loader's ordinary `omega run --both` route;
  unresolved package admission is a named dependency, not runtime success.

- **SQUALR-HEADLESS.** Run the independently versioned
  [Squalr application](samples/apps/README.md) (`samples/apps/squalr`
  submodule) through its ordinary nested packages to native geometry,
  supplied-byte scanning and repeat filtering. The application's board owns the
  1:1 Rust port; this item owns compiler integration and unchanged application
  acceptance.

  Application-side gaps (in the submodule):
  - `squalr-tests/main.omg` and `squalr-tests/src/command_driver.omg` skip
    expansion, ordering and string parsing, leave the six scalar `FixtureKit`
    scan/repeat checks disconnected, and print PASS after seeded checks. Connect
    those and production new-scan, element-scan, repeat, delete and list before
    success; direct executor helpers do not cover the privileged-command
    dispatcher. `verify_empty` supplies nonmatching bytes and `verify_overlap`
    scans one-byte values at stride two; neither tests its named case.
  - True multi-byte overlap needs raw-byte storage:
    `SnapshotRegion::read_current_elements` divides `(base_delta + position *
    stride)` by element width. The command fixture's eight-byte-spaced oracle
    needs explicit `Alignment8`; production keeps `Alignment1`.
  - Restore the real foreign-package `Optional<T>` APIs for
    `ScanFunctionScalar` and `SnapshotRegionFilter`; the compiler dependencies
    run (`tests/omega/pass/borrows/case_payload_shared_transition_exit`,
    `pass/generics/foreign_case_{membership_dispatch,payload_reads}_exit`,
    `pass/borrows/runtime_carrier_reference_store_exit`). Boolean-plus-payload
    or package-local sum replacements do not close it.
  - Live reads alias captured-current values and mark failure-default reads
    present; keep the seven-result delete/list oracle and add distinct
    recent-read presence/value.
  - The candidate application commit `248a79e66025` carries unpublished
    source-lifetime repairs and its remote denies writes: do not advance the
    gitlink.

  Integration frontier:
  - The published gitlink `43d56a7` and application remote main `964ea1c`
    contain the same source tree, including retired Build receiver calls.
    With compiler `fd8672848bf`, ordinary fresh offline
    `omega update --project samples/apps/squalr/squalr-tests --target
    linux_x86_64` rejects dependency requests before application checking:
    they must use a static Build call with the root machine's first parameter
    as the first operand. Preserve the historical lock before fresh review;
    its text Git content/tree mismatch is not repaired by editing the lock.
    The unpublished checkpoint's 105 Build-call migrations in 28 files still
    need publication, followed by machine/state declarations and calls under
    **STATIC-MACHINE-MIGRATION**. Keep the 17-package/37-edge layout, never
    enable implicit receivers or transplant approvals. The available account
    has read but no push access to Squalr; application publication requires
    repository write access before updating the gitlink. Source checking is
    not native acceptance.
  - Unverified: the earlier result-store stop in stage 04
    `omega-rust/psi/compilation/pipeline/04_typed-trees-to-checked-trees/src/execution/terminal_unit/control/call_results.rs`
    (`EnginePrivilegedState` task slots holding `Binding<WaitWake>`); retire
    those fields into exact `WaitWake<Role>` static slot demands. Empty roles
    create no demand and transport keeps ordinary authority evidence.
    Reproduce before assigning.
  - Windows byte output: `HostedWriteByteI32Realization`
    (`omega-rust/omega/compilation/backend/artifacts/native-artifact/src/physical/derivation/hosted_builtins.rs`)
    selects `HostedIntrinsicBundle`, whose profiles exclude Windows.
  - **VEC-NATIVE-GROWTH** supplies partitioned scan output and repeat
    filtering; **BUMP-ALLOCATOR-CANARY** and **PLAN-LAID-VIEWS** supply backing
    and placed access. Follow the application's next missing operation without
    waiting for every related task.

  Constraints: no Rust FFI scanner, fixed-capacity substitute, pull-only
  workaround or application-specific compiler behavior; no fabricated runtime
  installation grant (see [products and authority](wiki/spec/build/component_publication.md#products-and-authority)).
  Keep Windows and Linux ARM64 geometry, `set_alignment`, parsing/refusal and
  error-text parity, case-bearing alignment clone/serialization, and ordering
  agreement (order by base, equal-base equivalence, size-sensitive `equals`).

  Acceptance: on Linux x86-64 with Python 3.11+, `python3
  samples/apps/squalr/tools/verify.py native --target linux_x86_64 --timeout 600
  --omega <binary>` after ordinary package review exits 0 with `Squalr
  geometry: PASS` after the restored checks, and scan/repeat addresses and
  ranges match pinned Rust and the independent expected cases (no/all matches,
  tails, empty input, overlapping candidates). Keep the 17-package/37-edge
  layout; record compiler, std, application and upstream pins. Cross-target
  checking or emission does not establish execution.

- **VEC-NATIVE-GROWTH.** (split-of:BUMP-ALLOCATOR-CANARY) Complete ordinary
  library `Vec<T>` for Squalr's scan results under the
  [allocation contract](wiki/spec/resources/allocation.md). Owners:
  `source/library/core/vec.omg`, `source/library/alloc/vec.omg` and their
  ordinary compiler operations. Core's generic construction/storage is empty.

  Alloc has a linear nongeneric handle, caller-owned `Backing4/8/16`, fallible
  Pool requests and fixed growth/drain machines, but: `Request::Granted(rung)`
  is publicly constructible, Pool supply/releases are numbers, and growth
  accepts caller-selected old/fresh stores without joining their identity to
  the handle or request; `held` flags and rung numbers do not establish an
  allocation claim; growth copies whole fixed rungs, not the exact live extent.
  `tests/omega/pass/collections/runtime_vec_u32_growth_exit` (exit 70 on macOS
  ARM64 and Linux x86-64) exercises requests, two growths, denial, drain and
  counters, but never calls the consuming `Vec::close`.
  Squalr's 1024-element scan window needs a runtime-length owned array with
  contiguous `&[u64]` projection: every `[T; N]` is compile-time sized, data
  cannot hold borrows, and `alloc::vec`'s backing is non-contiguous.

  Dependencies: **BUMP-ALLOCATOR-CANARY** (allocation/returned extents),
  **PLAN-LAID-VIEWS** (access/retirement), **CONSERVATION-CONTRACT /
  TERMINAL-CONTENT-CLAIMS** (content transfer), **BORROWED-STORAGE-RESTORATION**
  (move-out/replace). No compiler-owned vector/Arena or recursive inline storage.

  Runtime qualification still needs package preparation and
  `omega run --both tests/omega/pass/collections/runtime_vec_u32_growth_exit/main.omg`.
  Normal `omega update --project
  tests/omega/pass/collections/runtime_vec_u32_growth_exit --target linux_x86_64`
  exceeded a measured 600-second bound at checkable checkpoint
  `4a420793ebe249021066b696624eb0a6a773281d` before producing a review document.
  The unoptimized compiler accumulated 543 CPU seconds with an active
  `omega-compile` worker and peaked below 608 MiB resident memory.
  Attribute the remaining checking cost through the ordinary review route.
  Complete that review and rerun the authored exit-70 oracle on both engines;
  no current native or Terminal runtime result is established.

  Acceptance: native `Vec<u32>` appends through two allocations, preserves the
  exact live prefix and new elements, and cleans up once with exact backing
  return or explicit retention; empty cleanup and real request failure (with
  unchanged contents/custody) work. Keep `fail/collections/{vec_duplicate_cleanup,vec_invalidated_loan_after_growth}`
  and `fail/borrow/vec_view_invalidated_by_push`; add fail fixtures for wrong
  backing, forged/replayed grant, wrong rung, duplicate return and partially
  occupied growth. Then complete generic element custody through the same
  route and use it in Squalr, with interpreter and matching-host native oracles.

- **STATELESS-PROVIDERS.** (new-scope) Owner ruling 2026-10-04: a provider
  is its satisfier machines and holds no state
  ([machines](wiki/spec/language/machines.md#static-declarations-and-explicit-state):
  a satisfier declares no parameter its requirement lacks). Checking rejects
  such a parameter on every conformance path
  (`fail/providers/satisfier_extra_parameter_rejected`). Finish removing the
  implicit provider state that installation still models.
  - Progress premises still name an installed provider receiver:
    `ServiceProgressSubject::ProviderReceiver` and `BuildBoundProgressDemand`
    (checked-trees `facts/termination.rs`, stage 04
    `checks/termination/progress/`), their consumer
    `provider-evidence/.../component_eras/component_progress_manifest.rs`, the
    provider-occurrence receipts in
    `external-roots/src/root_entry/progress_profile_installation.rs`, and the
    review/package record codecs that still decode tag 0 `provider_receiver`.
    No producer creates a receiver subject or a build-bound demand, so the
    route is unreachable. Replace it with premises over the requirement's
    declared parameters or delete it, together with the
    `pass/progress/provider_receiver_progress_installation` customer that
    **TRAIT-IMPLEMENTATION-SOURCE-MIGRATION** migrates.
  - A foreign-bound provider whose fields hold vtable addresses
    (`ForeignBinding::VtableField`) takes those from its foreign binding plan,
    not from provider data. **TRAIT-IMPLEMENTATION-SOURCE-MIGRATION** owns
    the requirement conversion under the settled named-implementation contract.

  Acceptance: no installation or interpretation route supplies a provider
  value or receiver its requirement did not author; the four GUI samples check;
  `tests/omega/pass/providers/runtime_boundary_primitive_array_argument_exit`
  still exits 70 in both engines.

- **MACOS-APPLICATION-PUBLICATION.** Publish and run `samples/gui/window_app`,
  `window_demo` and `windowed_calculator` under the
  [macOS application contract](wiki/spec/build/macos_application.md). Reuse
  `omega-rust/omega/compilation/backend/artifacts/native-artifact/src/package_publication.rs`
  for package assembly and signing, and
  `omega-rust/omega/compilation/products/src/compilation_products.rs` for
  package/inner-executable accessors. Keep provider selection as named
  implementations and window state as explicit owned data.

  Finish visual rendering and input/close verification for all three apps
  on a desktop session that permits observation and input. Their public CLI
  paths now publish `.app` bundles. `window_demo`, using the shared owned
  `Window` API, prints `PASS: 60 frames blitted (exit 0)` and exits 0.
  `window_app` and `windowed_calculator` create application-window records
  through both direct execution and separate LaunchServices launches, but
  rendered pixels and keyboard/close interaction remain unverified. Reproduce
  in a session with Screen Recording, event-posting and Accessibility access,
  using [window_demo](samples/gui/window_demo/README.md) and
  [window_app](samples/gui/window_app/README.md); the calculator follows the
  same update/run flow under `samples/gui/windowed_calculator`.

  Acceptance: all three publish a validated `.app` and execute on macOS ARM64
  with observed windows/rendering, input/close behavior and completion
  (`window_demo`: 60 frames, exit 0), plus a separate Finder launch. A
  two-second survival check or test-owned package acceptance is not evidence.
  Keep identifier requiredness, signature/plist agreement, deterministic bytes,
  publication in a separate invocation, root/inner-path correspondence,
  tamper/extra/partial-output rejection and unchanged flat outputs. Resources
  and `image_viewer` bundle-relative lookup are outside v1; do not change the
  working directory.

- **TERMINATION-RANKING-CHECKS.** Carry exact rank/endpoint evidence through
  ordinary calls, named states and native execution under the
  [termination contract](wiki/spec/language/termination.md). Owners:
  `omega-rust/psi/compilation/pipeline/04_typed-trees-to-checked-trees/src/checks/termination/ranking/`,
  `omega-rust/psi/compilation/checking/src/proof_contracts/contract_entailment/ranking_range/`
  and `omega-rust/psi/compilation/checking/src/machine_calls/call_cycles/runtime_ranking/`.

  Quotient/remainder, shifts and bitwise-AND actuals already use
  `install_nonpolynomial_terms`. Executing controls:
  `pass/control_flow/termination_index_distance_compile` (exit 0 on macOS and
  Linux; Windows rejected), `pass/termination/runtime_named_state_remainder_endpoint_exit`
  and `pass/termination/rank_endpoint_borrows_recovery` (exit 0).

  Retain executable controls `pass/termination/runtime_saved_rank_cycle_exit`
  for saved values, requirements and disjoint writes,
  `pass/termination/joint_rank_acyclic_finish_exit` for a joint preserving/strict
  cycle with a rankless finish,
  `pass/termination/runtime_rankless_boundary_suffix_exit` for a finishing
  boundary call with no synchronous-invocation permission, and
  `pass/termination/rank_range_record_snapshot_compile` for an ordinary
  snapshot return (saved value five, exit 0). Empty `invokes` does not imply termination.
  Rankless exits returning to recurrence,
  nondecreasing cycles, stale premises, overwritten requirement inputs and
  unproved arrivals reject; entry requirements do not become internal-arrival
  facts. Free and attached mutual owners retain authored claim identities;
  keep `pass/ownership/linear_call_result_members_fused_owner_exit` and
  `pass/calls/runtime_static_tail_{local_custody,reordered_local_custody}_exit`
  for fresh/call-produced claims and reordered parameters.

  Rank observations use the shared scalar-computation arena over exact captured
  entry and completed arrival values. Keep
  `pass/termination/runtime_record_measure_constructed_arrivals_exit` and
  `pass/termination/runtime_computed_projected_measure_arrivals_exit` for
  reconstructed records, renamed formals and narrow arithmetic formation,
  `pass/termination/runtime_ranked_sequence_arrival_coordinates_exit` for byte
  and element descriptors after an unrelated scalar, and
  `pass/termination/runtime_distance_crossed_header_exit` for total distance
  ranks when the cursor has already crossed its bound. Keep
  `pass/termination/runtime_projected_element_rank_arrivals_exit` for a shrinking
  selected descriptor reconstructed at each arrival, and
  `pass/collections/runtime_nested_selected_element_extents_exit` /
  `runtime_record_exclusive_element_extent_exit` for distinct selected counts
  and exclusive descriptor storage. These controls execute natively and in
  Terminal on macOS ARM64; independent overflow, stale subject, overlapping
  loan and shared-to-exclusive receiving controls remain obligatory.

  Keep `pass/termination/runtime_selected_requirement_rank_prefix_exit` and
  `runtime_selected_requirement_rank_composed_prefix_exit`, with their write
  negatives, for readonly requirement results in prefix `let` bindings.
  Keep `pass/collections/runtime_projected_range_call_nonempty_requirement_exit`
  and `runtime_range_call_captured_endpoint_exit` for completed descriptor
  observations before later actual writes, and
  `pass/calls/completed_effectful_result_requirement_exit` for a completed
  scalar result's published bound. Invalid selected extents, shared-to-exclusive
  access, late endpoint rereads and out-of-bound result guarantees reject.

  Keep `pass/termination/runtime_nested_borrowed_rank_carrier_exit` and
  `pass/control_flow/runtime_completed_readonly_home_replacement_exit` for
  moved carrier homes, live borrowed backing and completed-home replacement.
  `pass/termination/runtime_selected_boundary_requirement_identity_exit`
  retains exact public identity guarantees through direct, nested and composed
  checked boundary calls. Decreasing siblings and unrelated formals cannot
  replace a transported rank; unknown operand completion cannot establish its
  absence. Guarded fresh reception may retain the full declared selector under
  a new containing owner.

  Keep `pass/termination/runtime_public_result_field_rank_arrivals_exit`
  for exact public result-field equations through constructed recurrent
  actuals, and `rank_arrival_unobserved_array_extent_compile` for demand-based
  discovery beside large unrelated arrays. Wrong-subject and missing-public-
  equation controls reject; private helper bodies supply no caller evidence.

  Keep `runtime_quotient_result_rank_arrivals_exit` and
  `runtime_shift_result_rank_arrivals_exit` for captured integer equations
  through direct, nested, saved and projected arrivals, variable divisors and
  independently typed/policy-bearing counts, including record fields. Signed
  truncation and signed right-shift floor semantics remain distinct. Division
  by one, zero-count shifts, preserving masks, changed endpoint counts, absent
  signed-field floors, missing premises, alternative results, wrong operands,
  invalid counts and overflowing intermediates must reject. Keep
  `runtime_computed_result_rank_arrivals_exit` and its policy controls, plus
  `runtime_selected_result_ranked_arrival_exit` and
  `external_result_ranked_arrival_compile` with missing-guarantee, unguarded
  and stale-prefix controls.

  Keep `pass/termination/runtime_float_requirement_integer_rank_exit` for
  mixed IEEE requirements and independent bounded integer ranks through calls,
  named states, saved guards and public results; stale captures, NaN and false
  integer conjuncts reject. IEEE predicates supply no integer bounds.

  External result controls still refuse corpus production with
  `provider candidate requires beyond its boundary's declared obligations`.
  Execution also needs **ASSUMED-BINDING-CONTRACTS**' Build/root
  trust-to-installation join; source acceptance and provider selection supply
  no admission receipt. Re-run the external-result and std-loop execution
  controls through the completed provider route before closing this item.

  Acceptance: `python tools/corpus_gate.py --filter termination/` with `--both`
  and `--native` on matching hosts; changed endpoints, stale copies,
  protected-carrier writes, overflowing intermediates and nondecreasing cycles
  reject. Keep the private-witness/public-guarantee split, exact
  subject/view/range identity, subordinate-call obligations and descent on
  every complete mutual call cycle. Checked-only records do not close this.

- **CANONICALIZE-SCALAR-RANGE-CONTRACTS.** Complete the settled
  [proposition-based scalar range contract](wiki/spec/language/domains.md#declaration-and-membership)
  and [generic inference rule](wiki/spec/language/generics.md): common fields use
  data `where`, payloads case `where`, machine/state parameters `requires`,
  results `ensures`, locals the facts from initializers, guards and calls.
  Remove integer and float `T [lo..hi]` / `T [lo..=hi]` suffixes after their
  customers migrate; ordinary ranges, slices, arrays and bracketed multiplicity
  stay. Finish each customer group's transport and execution before the next;
  parser deletion is last.

  Design constraints: a domain kept by a migration needs an independently
  meaningful nominal identity, semantic contribution or establishment route,
  explained in the change; endpoint-named or interval-only replacement domains
  are invalid even when they run. Equal predicates do not merge domain
  identities. Flow narrowing never selects specialization or layout identity;
  runtime endpoints grant no const value, fixed extent or implicit maximum-sized
  allocation. Coordinate binders/equations with **STRUCTURAL-GENERIC-MATCHING** /
  **RUNTIME-VALUE-GENERICS**. Do not rebuild `proven_predicates_grant_domain`
  (`omega-rust/psi/compilation/pipeline/04_typed-trees-to-checked-trees/src/checks/contracts/calls.rs`)
  or weaken its refusals.

  - Inventory: build the history-derived declaration inventory with the
    [migration recipe](wiki/drafts/designs/range_suffix_migration.md) from
    revisions `8f93c866a3`, `2c3c2aec00`, `b9b68383c9`, `cc36490282`,
    `3531aaa098`, `e0a884138f`, `2ef59d385a`, `c4d2776b1f`, `c5621e66ac`,
    `fc0fc2dcab`, `c5688363ec`, `f1d6e31013`, `9e3355b562`, `db4f9f9140`,
    `2858366bdb`, samples `24d91ed906` / `7917a536d4`, library `c1303f89d4`,
    plus follow-ups. Reconcile the audited library declaration groups beside
    their runtime witnesses; inventory the remaining original/current spelling,
    owning position and restoration or reason for retention, including bare
    fields whose bounds were removed because inferred loop bounds made execution pass.
    A suffix grep or passing replacement does not establish preservation.
  - Field/case propositions through construction, nonliteral and indexed
    writes, reads, calls, state joins, mutable recasts (the
    [representation judgment](omega-rust/psi/compilation/checking/recasts.md))
    and returns (**NOMINAL-FIELD-FLOW**, **STATE-LOCAL-VALUE-FRONTIER**). Re-drive
    `pass/core/zii_default_composite_exit`,
    `pass/collections/runtime_dutch_flag_partition_exit`, the
    `pass/recast/runtime_*_mutable_write_exit` family and the sample field
    migrations; reproduce before attributing failures to proposition checking.
    Keep the [ten-position recast inventory](wiki/drafts/designs/range_suffix_migration.md#audited-recast-selectors-and-aggregate-leaves)
    for captured byte offsets, nested indexed stores and shared/mutable leaf
    facts. Its array-view controls must reject predicate strengthening and
    out-of-range indexed writes; remaining recast customers and hosts stay open.
    Run `pass/float/named_integer_conversion_source_proofs_exit` (data `where`,
    exit 78) on Linux/Windows, keeping its invalid-write control.
    Constructor bounds now use the shared occurrence environment in
    `checking/src/value_custody/struct_literals/construction_bounds.rs`, not
    the retired immutable-operand filter or obsolete local initializers.
    `python3 tools/corpus_gate.py --native --filter data/runtime_constructor_observation_exit`
    executes with exit 70 on macOS ARM64; stale-local and earlier-operand
    mutation controls reject. Ordinary CLI package preparation still requires
    this new fixture's local package review; no acceptance policy is committed.
    Keep the [19-position reference-carrier inventory](wiki/drafts/designs/range_suffix_migration.md#audited-bounded-reference-carriers)
    and its six authored exit-70 witnesses as regression constraints: nested
    reference arrays/records, constructor transfers, shared cohorts and case
    retirement retain field predicates and bare-local facts. Invalid field
    and index stores and a same-named unbounded field must still reject.
    Keep the [eight-position scalar-loan inventory](wiki/drafts/designs/range_suffix_migration.md#audited-scalar-field-loans-and-selector-ordering),
    its exit-0 field-loan and exit-70 matrix-selector witnesses, and the
    reset-to-2 handback refusal. Borrowed field predicates must survive local
    aliasing and checked normal return; disjoint write-only sibling projections
    must not erase predicates that constrain the selected storage.
    Keep the [six-position call-selector inventory](wiki/drafts/designs/range_suffix_migration.md#audited-call-selectors-and-mutable-arrival-observations):
    complete/empty/short input must retain exits 70/72/72 after mutable helper
    return; nested scalar calls and selector-order witnesses retain exit 70.
    One-past nonliteral row and column stores must still reject.
    Continue with the nonliteral/indexed writes and consumer transports above;
    this does not close the complete field/case migration.
  - Guarded immutable-local, guarded-field, nested-call-result and
    destructured-payload facts into `requires`; entry premises into `ensures`
    formation; state results into enclosing postconditions. Customers:
    `pass/calls/nested_value_call_arg_compile`,
    `pass/arithmetic/anonymous_rational_arguments`,
    `pass/proofs/accumulator_guarantee_exit`.
    A mutable parameter's `requires` constrains arrival where the old
    suffix was store-enforced: re-prove dependent reads after a write
    (`pass/termination/rank_range_mutable_bounded_endpoint` plus negative
    mutation controls). Keep `pass/dependent/runtime_state_dependent_index_forward_exit`
    and `fail/dependent/{dependent_arg_unrelated_rejected,dependent_forward_after_write_rejected}`
    as controls for captured state arguments and invalidated bound observations.
  Canonical controls to keep: `pass/collections/runtime_argmax_index_exit`,
  `pass/calls/runtime_dispatch_binary_call_argument_exit` and
  `fail/calls/state_arrival_call_result_above_bound` (guarded re-entry satisfies
  the ordinary state arrival requirement; a guaranteed call result above the
  bound does not),
  `pass/termination/ranked_callee_projected_receiver_compile`,
  `pass/termination/rank_endpoint_borrows_recovery` and
  `fail/termination/{nested_field_without_predicate_cannot_bound_result,rank_range_mutable_endpoint_overflow_after_write,rank_range_subject_intervening_write}`
  (declaration-owned nested field bounds and ranked loan transport survive;
  neither matching field names nor stale mutable arrival facts grant bounds),
  `pass/termination/runtime_rank_state_constraint_disjoint_write_exit`,
  `pass/termination/runtime_rank_arrival_field_premise_disjoint_write_exit` and
  `fail/termination/rank_state_dependent_bound_prefix_write` (the shared rank
  premise-coordinate roster preserves disjoint writes but does not reuse a
  dependent arrival proposition after writing its observed ceiling),
  `pass/termination/rank_range_record_snapshot_compile`,
  `pass/termination/rank_arrival_{captured_record_fields,same_carrier_fields}_exit`
  and `fail/termination/rank_arrival_unrelated_field_step` (record fields keep
  their predicates through arrivals and captures; a saved scalar survives
  countdown without becoming its measure, and decreasing a sibling is not
  evidence about the selected field),
  `pass/host/runtime_console_bounded_line_{lf,full,partial_eof,middle_window}_exit`
  and `pass/host/runtime_console_byte_payload_bounds_exit`
  and `fail/host/console_byte_payload_{negative,above_byte}` (active i32 payload
  bounds survive case extraction and the reader's requires-based state arrival;
  result counts remain tied to each supplied window, not a new nominal cap),
  `pass/arithmetic/runtime_widened_result_predicates_exit` and
  `fail/arithmetic/{widened_result_does_not_grant_stricter_bound,widened_result_predicate_invalid_write}`
  (all 12 historical widening-result bounds remain published postconditions;
  their result facts establish ordinary record predicates and survive calls
  and state transport without granting stronger bounds),
  `pass/time/runtime_duration_core_exit`,
  `pass/time/runtime_duration_units_and_scaling_exit`,
  `pass/time/runtime_checked_time_arith_exit` and
  `fail/time/{instant_constant_unnormalized,system_time_constant_unnormalized,instant_mutation_leaves_invalid_return}`
  (initializer facts survive bare local carriers; normalized timestamps retain
  construction and mutation obligations through data predicates),
  `pass/constants/runtime_predicate_constant_materialization_exit` and the
  `fail/constants/unused_*_predicate_constant`, `unused_*_default_domain_constant`
  and `predicate_constant_*` controls (ordinary materialization never grants
  const-index eligibility),
  `pass/layouts/placed_view_source_access_compile`,
  `pass/layouts/schema_field_at_declared_size`,
  `pass/layouts/schema_identity_payload_within_predicate` and
  `fail/layouts/{schema_field_exceeds_declared_size,schema_identity_payload_predicate,resource_profile_exceeds_region_capacity}`
  (compiler-supplied arrivals owe ordinary construction predicates; preserve
  declared schema rosters, active payloads and valid inactive-tail alignment),
  `fail/layouts/placement_policy_unproved_callee_premise` and
  `pass/filesystem/native_crud` (policy admission keeps each concrete callee
  premise without checking unrelated unfinished runtime layouts),
  `pass/calling/calling_count_boundaries_exit` and
  `fail/calling/{native_field_path_empty,call_plan_count_invalid_handback}`
  (canonical count predicates keep active-case lower bounds, nested placement
  counts and mutation handback; whole-value edge snapshots use private frame
  storage rather than keeping every aggregate word resident),
  `fail/wire/wire_policy_plan_disagrees`,
  `pass/termination/runtime_ranked_assignment_call_prefix_exit` and
  `fail/termination/ranked_{assignment,index}_call_prefix_write` (complete
  operand frames preserve only disjoint ranked premises),
  `pass/collections/runtime_nested_const_row_indexed_read_exit`,
  `pass/arithmetic/runtime_guard_narrowed_transition_arg_exit`,
  `pass/arithmetic/runtime_exact_guarded_shift_count_exit`,
  `pass/termination/rank_range_call_component`,
  `pass/termination/runtime_joint_rank_computed_copy_exit` and
  `fail/termination/{rank_range_call_component_prefix_write,rank_range_declared_step_unbounded}`
  (complete mutual cycles preserve reordered endpoints and computed copies;
  restoring scalar bounds after a subject reset supplies no descent, and
  an unconstrained zero stride still refuses strict decrease),
  `pass/termination/{runtime_public_result_field_rank_arrivals_exit,rank_arrival_fresh_nested_owner}`
  and the paired missing-result-equation, wrong-subject and opaque-completion
  `fail/termination/rank_arrival_*` controls (data predicates keep field bounds
  through result equations and fresh guarded reception; bounds alone neither
  identify the previous rank nor grant descent),
  `pass/termination/{measure_field_rank_endpoint,rank_arrival_unobserved_array_extent_compile}`,
  `pass/borrows/runtime_projected_fixed_byte_array_local_exit` and
  `fail/termination/measure_field_{arrival_contract,limit_changed,computed_limit_overflow}`
  (field bounds retain their default predicates without conserving replaced
  endpoints or hiding call requirements; unrelated fixed arrays and captured
  projected byte carriers remain ordinary data, not additional rank subjects),
  `pass/dependent/zero_case_predicate_entry_exit` and
  `fail/dependent/case_where_zero_{payload_requires_establishment,wide_arithmetic_not_saturated}`
  (first-case zero establishment proves its actual predicates, never a later
  case or a saturated mathematical expression; preserve command facts through
  locals, parameters, results and matched writes),
  `pass/float/runtime_{exact_format_narrowing,exact_integer_coercion,format_conversion_result_bounds,result_predicate_custody}_exit`
  and `fail/float/{exact_format_conversion_requires_denotation,exact_integer_coercion_requires_denotation,format_conversion_exclusive_upper,parameter_guarantee_does_not_bound_result,result_predicate_stale_or_wrong_value,unbounded_result_does_not_prove_guarantee}`
  (selected IEEE result predicates survive local, field, state and saved-copy
  transport; neither receiving bounds, stale storage nor parameter-only
  guarantees authorize Exact conversion or stricter rounded results),
  `pass/termination/rank_range_{declared_step_copies,call_declared_step_copy}`.

  The 16 recursive filesystem parameter suffixes remain unmigrated. Their
  `pass/filesystem/dir_walk_wrappers_exit` customer's portable binding
  dependency is **FILESYSTEM-PLATFORM-SERVICES**. Continue the other customer groups
  without manufacturing an implicit re-export or provider-name fallback.

  Collection frontier: qualify remaining view and capture compositions through
  shared place coordinates, contract flow and termination, without assuming
  arbitrary predicates are hereditary. Keep the five-position
  [whole-slice inventory](samples/cli/probes/slice_view_whole/README.md#historical-scalar-positions),
  its i32 carriers and original calculation 31 as regression constraints.
  Reproduce with normal `omega update --project
  samples/cli/probes/slice_view_whole --target macos_arm64` review followed by
  `omega run --both samples/cli/probes/slice_view_whole/main.omg` (exit 0).
  Keep symbolic descriptor/read identity, functional store versions and checked
  well-founded predicate implication; do not unfold every possible length. Keep
  `pass/dependent/runtime_symbolic_slice_cell_theorem_exit` (forward/reverse
  traversal, dynamic selector, checked arithmetic and result bounds),
  `fail/dependent/cited_cell_result_exceeds_guarantee` (too-small result bound
  and a selector changed after citation) and
  `fail/dependent/symbolic_collection_consequence_requires_inspection`
  (uninspected cell, wrong collection, stronger-than-predicate bound).
  Keep `pass/dependent/runtime_symbolic_slice_predicate_store_exit` (saved
  operands, repeated store, forward/reverse predicate handback and complete
  checked requirements at both state arrivals) and
  `fail/dependent/symbolic_slice_predicate_store_requires_establishment`
  (invalid replacement, absent premise, wrong subject, partial restoration and
  a valid byte that breaks a relation between coordinates, and mutation that
  invalidates a subsequent state requirement).
  Re-drive std-backed
  examples (`Filesystem::mkall_walk` previously refused its disjoint prefix).
  Preserve exact call occurrences, public result guarantees and complete
  mutation frames; do not reinstate a separate self-call recognizer or admit
  opaque prefix effects.
  Keep `pass/termination/runtime_self_call_forwarding_state_exit` (direct,
  captured parenthesized, preceding-arrival, signed and computed-bound descent),
  `runtime_ranked_recursive_argmax_forwarding_exit`, the captured-result
  `runtime_{computed,quotient,shift}_result_rank_arrivals_exit` family and
  `runtime_selected_boundary_requirement_identity_exit`, plus their nondecreasing,
  moving-bound, missing-witness, genuine state-cycle and non-denotational-loop
  controls. `checking/src/proof_contracts/collection_predicates` owns symbolic
  predicate judgments. Keep `pass/dependent/runtime_{slice_coordinate_citation,
  whole_view_collection_fact,computed_slice_index_relation}_exit` and their
  wrong-subject, partial-view and changed-value controls as regression constraints
  while closing the predicate proofs.

  Acceptance: spec/guide/compiler docs, bundled libraries, samples,
  `source/psi`, the corpus, `bootstrap/5_omega/parser.epsilon` and
  `tests/bootstrap/omega-parser` migrated; each customer's native oracle runs on
  a matching host; invalid bounds, unproved arguments, stale facts,
  incompatible carriers and runtime bounds used as layout values reject. Then
  delete suffix parsing
  (`omega-rust/psi/compilation/pipeline/01_tokens-to-syntax-trees/src/type_syntax/parse_type.rs`),
  `TypeConstraintNode::Range`, range-shell inference and suffix-recommending
  diagnostics; reject suffixes in every type position; delete the recipe.

- **NOMINAL-FIELD-FLOW.** Close the Dungeon Crawler's field-domain execution
  under [mutable arguments and declared field domains](wiki/spec/language/dependent_values.md#mutable-arguments-and-declared-field-domains)
  and [static declarations](wiki/spec/language/machines.md#static-declarations-and-explicit-state).
  Qualified machine names contribute no storage or arguments; established
  mutable inputs owe their declared field/default domains at call admission,
  entry, return and fact handback, including zero-rejecting domains.

  Owner: `omega-rust/psi/compilation/pipeline/04_typed-trees-to-checked-trees`.
  `src/semantic/facts/field_domains.rs` supplies shared parameter obligations;
  transition argument transport is in `src/flow/state_values.rs` and
  `src/flow/state_values/fields.rs`. `pass/domains/qualified_mutable_field_exit`
  and `pass/domains/threaded_subject_keeps_established_domains` exit 0 natively
  on macOS ARM64. A byte store preserves `Utf8` only with carrier/byte
  evidence: `ValidUtf8` does not imply `AsciiOnly`.

  Restore [representation-erased domain qualification](wiki/spec/language/domains.md):
  `[u8; N] in D` is the same exact-length fixed array as `[u8; N]`, with
  `.len == N`. Delete the `has_domain => BoundedOwned` source-shape classifier
  in `checking/src/value_custody/reference_shells.rs` and every downstream
  assumption that domain presence supplies live-length storage. Preserve
  `BoundedOwned` only for explicit variable-fill carriers. Migrate text and
  Dungeon Crawler values that need inline capacity plus live length to
  `FixedVec<u8, N> in Utf8`; use `Vec<u8> in Utf8` where growth is intended,
  and obtain qualified live-prefix views through ordinary checked accessors.
  Keep genuinely exact-width text as `[u8; N] in Utf8`.

  Acceptance: `samples/cli/probes/{write_only_domain_store,ascii_indexed_write}`
  realize their authored fixed-array copies/indexed stores without carrier
  mismatch; predicate erasure changes no storage or length. Fixed-array `.len`
  remains `N`; `FixedVec`/`Vec` `.len` observes the explicit owner field. Migrate
  the 71 text fixtures and Dungeon Crawler while preserving their authored
  content/output oracles. Wrong UTF-8/ASCII writes still reject, and variable
  growth without proved capacity or allocation remains refused.

  The unchanged Dungeon Crawler runs natively on macOS ARM64 and Windows x86_64
  through the public EOF and `help`/`exit` routes, printing the authored opening
  room and goodbye before exit 0. Qualified byte snapshots retain live contents and
  lengths through state arrivals and field replacement. Reassigned view locals
  carry the current descriptor through calls, states and returns in native and
  Terminal execution; an earlier child retains its captured selector and backing
  when the parent view or selector local changes. Invalid replacement bounds,
  local-backing escapes and overlapping child loans reject. Owned copies of
  borrowed case leaves capture computed selectors in path order before later
  operands can mutate them or the source; local and slice copies retain the same
  bounds obligations. Retain these controls, close the remaining domain/proof
  compositions below, and qualify the other declared hosts. The whole application
  remains the acceptance driver; its README carries the public commands.

  Acceptance:
  - Paired qualified/free-machine corpus cases over the same established
    `&mut T` agree for nested non-ZII domains, valid mutation and return.
    Invalid arrivals, unclosed invariant windows and stale aliases reject at
    calls, transitions and returns. Input-only `requires` are not implicit
    postconditions. Add corpus fixtures for the poisoned-edge/missing-fact,
    join/mutation, indexing/view/copy/whole-extent and finite candidate-origin
    controls formerly held by deleted Rust tests, and for constrained nested
    `&write` subloans carrying exact atoms.
  - Keep `pass/control_flow/element_view_enum_equality_exit`,
    `pass/borrows/{indexed_record_field_replacement_exit,qualified_byte_owner_exclusive_call_exit,branch_selected_record_composed_receiver_exit,returned_record_case_field_store_exit,interleaved_parameter_field_replacement_exit}`,
    `pass/structs/qualified_record_element_assignment_exit` and
    `pass/text/{nested_mutable_receiver_byte_result_exit,qualified_byte_state_snapshot_exit,qualified_byte_value_forwarding_exit}`.
    Executable predicate-only structural root returns owe their declared domains;
    `fail/text/byte_field_snapshot_domain_widening` must reject. Guarded and
    closed-case returns keep physical view cleanup and semantic custody.
  Coordinate with **CANONICALIZE-SCALAR-RANGE-CONTRACTS**,
  **IMPLICIT-STORE-CONVERSION** (policy casts) and **WRITE-ONLY-BORROW**
  (`&mut Room` output). Valid ZII locals need no initializer; arbitrary incoming
  storage gains no ZII facts.

- **GENERAL-CYCLIC-EXECUTION.** Complete general
  [cyclic control](wiki/spec/terminal-psi/control_flow.md) and
  [safety/progress checking](wiki/spec/language/termination.md) through native
  execution. Owners: stage 04
  `omega-rust/psi/compilation/pipeline/04_typed-trees-to-checked-trees/src/execution/terminal_unit/`,
  stage 05 `omega-rust/psi/compilation/pipeline/05_checked-trees-to-lowered-psi/src/unit/attached_unit/`
  and expression preparation; runtime arrivals are in
  `omega-rust/psi/interpreter/terminal-interpreter/src/terminators/`.
  The optimizer board's same-name task owns native receiving.

  Executing controls: `pass/text/runtime_number_to_decimal_exit` and
  `pass/text/runtime_bounded_carrier_write_read_exit` (exit 70);
  `pass/collections/runtime_shared_fixed_array_cycle_exit` (42),
  `runtime_shared_array_dot_product_exit` (32),
  `runtime_shared_array_polynomial_exit` (70) with exact stdout, mirroring
  `samples/cli/{collections/array_max,arithmetic/dot_product,algorithms/horner_eval}`;
  acyclic control `pass/collections/runtime_borrowed_fixed_array_state_reads_exit`.

  Reproduce the current frontier for remaining cycle shapes after deletion of
  the old Terminal-verifier fences: broader qualified/partial
  ownership, content reshuffles/partitions, broader structural-result custody,
  boundary completion receipts, and claim transfers through Unit/scalar calls.
  Each needs stage 04 to check the arrival/custody relation and stage
  05 to lower it; deleting the fence alone is not the repair.
  Keep `pass/control_flow/runtime_cyclic_reference_and_claim_rotation_exit`
  (changing projected references and call-returned root/nested linear claims,
  affine residuals and borrowed backing across ranked backedges, including
  zero, odd and even counts, component/owner checks at scalar-returning exits,
  and selected affine-local call consumption versus cleanup),
  `fail/borrows/cyclic_arrival_duplicates_exclusive_reference`,
  `fail/ownership/cyclic_arrival_restores_consumed_field` and
  `fail/ownership/conditional_scalar_return_preserves_live_claim`. The hosted native
  matrix and unchanged public samples remain qualification requirements;
  portable Terminal installations do not establish native execution.
  Current control: `pass/slices/runtime_guarded_view_copy_cycle_exit` checks
  and runs with exit 70 on macOS ARM64. The loop transports the shared input,
  mutable fixed-buffer owner, index and limit through a four-conjunct arrival
  contract; the oracle covers limited, zero-limit and input-extent termination.
  Full application acceptance remains with **SQUALR-HEADLESS**, whose published
  source currently stops before this graph at retired Build call syntax.

  Acceptance: the controls above and the unchanged samples run across the
  hosted matrix with exact caller/callee resources and ranking where declared;
  dominance, successors/frontiers, current-iteration guards and fuel
  suspension/resumption are preserved; corrupt arrivals, custody and effects
  reject in checking (add fail fixtures for rebound fields and custody
  mutations). Safety stays independent of termination and of a test's fuel.

- **CRASH-CONTRACT.** Complete invocation-specific crash obligations through
  selected operators, nested values, calls, cycles and package review. Owners:
  stage 04 `omega-rust/psi/compilation/pipeline/04_typed-trees-to-checked-trees/src/facts/{operator_crashes,crash_entry_values}.rs`,
  `CrashPlan::checked_operators`, `src/flow/expression.rs`, stage 05 lowering
  of crash routes and the native consumers. Preserve selected requirement,
  saved actuals, Match arm and surviving route; traps or opaque provider bodies
  cannot narrow authored contracts.

  Current: `pass/operators/crash_routes` runs with exit 0 natively and
  interpreted on macOS ARM64 and Linux x86-64 ([oracle](tests/omega/pass/operators/crash_routes/README.md));
  `fail/operators/selected_crash_invocation` rejects. Named uses join the
  emitted `BoundaryCall` through one source-occurrence relation.
  `pass/contracts/ensures_{fact_list,wrapping_field_arithmetic,borrowed_parameter_field,named_parameter_arithmetic,saturating_field_arithmetic}_exit`
  and `pass/collections/runtime_fixed_vec_round_trip_exit` execute.
  `pass/contracts/runtime_float_{field,nested,indexed,case,fused}_crash_guard_exit`
  executes entry guards across structural inputs and mutual calls; preserve
  signed-zero equality, NaN unordered outcomes and stale-write refusals.

  - Selected-use joins for remaining intrinsic spellings, generic telescopes
    and provider-backed structural calls need exact operand mapping and provider
    commitment. Extend these remaining generic/structural-formal routes through
    portable predicates and execution, keeping the original guard.
    Preserve authored public ceilings independently of executable applications;
    `pass/providers/finite_requirement_boolean_const_exit` and
    `pass/providers/calling_application_region_scopes_exit` constrain package
    review across constant arguments and separate supplying/method regions.
  - Executed outcomes: actual outcome, substituted guards, abandoned claims,
    staged writeback and no-result/no-cleanup behavior must execute correctly
    in both engines, not only be retained as crash rows.
  - Ordered completion beyond the current scalar/integer-range/Boolean/field
    cases: remaining qualified results, state/control contracts, compound
    arithmetic, and nonliteral wrapping operands; keep transitive contracts and
    exact entry versions across joins.
  - Entry-operand normalization: `EntryOperands::scalar`
    (`src/values/scalar/contract_entry.rs`) declines indexed reads,
    nonempty-path lengths, policy casts and representation views; keep exact
    entry coordinates and recursive operand meaning, no source-arrangement
    exceptions, and private/local roots never become public parameters.
    Fixed-array field paths already work for crash-entry float operands.
  - Entry provenance for opaque/content leaves, dynamic indices and
    whole-collection reads after writes with proven origins; divergent
    arrivals and unknown cycles stay conservative. Share snapshots with
    **STATE-LOCAL-VALUE-FRONTIER** and **MATCH-SELECTIVE-LOWERING**.

  Acceptance: add executing corpus fixtures for each extended route and the
  float controls, through package review; changed guards, captures,
  substitutions, sites and stale writes reject. Proof-only terms cannot
  authorize executable guards; legacy propositions are
  **PROOF-CONTRACT-MIGRATION** scope.

- **ARITHMETIC-POLICY-REALIZATION.** Close remaining
  [numeric-policy](wiki/spec/language/numeric_values.md) realization and full
  program acceptance. Owners: stage 04 scalar checking, stage 05 expression
  preparation, and the Terminal trapping-integer representation
  (`omega-rust/psi/compilation/representations/terminal-psi/src/terminal_module/control_flow/trapping_integer.rs`)
  with its interpreter and native consumers.

  Mixed fixed-integer comparisons and operation-level Trapping arithmetic
  exist (`pass/expressions/mixed_integer_comparison_{widths,places}`,
  `mixed_integer_guard_comparison`, `pass/core/numeric_cross_signed_conversion_surface`
  execute on all three hosts; `pass/core/numeric_trapping_conversion_overflow`
  traps). Guarded Trap ceilings run:
  `pass/core/runtime_guarded_trapping_conversion_exit` exits 70, and the
  checker refuses an integer or float-to-int Trapping site its `crashes Trap`
  routes miss (`fail/core/guarded_trap_ceiling_*_rejected`). Preserve operation
  site, operands, carrier, cause and normal-only result equation; coordinate
  **CRASH-CONTRACT** and never invent a Crash edge.

  Remaining: `pass/core/numeric_*` reach all 170 public machines of
  `source/library/core/numeric_conversion.omg` but run natively and
  interpreted on macOS ARM64 only; run them on Linux x86-64 and Windows x86-64
  and record those goldens. Preserve exact coercion under every policy:
  `as` requires unchanged denotation and representability; wrapping and
  clamping belong to the named machines of `core/numeric_conversion.omg`.
  Float policy/conversion customers belong to **OPERATOR-BOUNDARY-CALLS** /
  **FLOAT-PROVIDERS**.

  Acceptance: comparisons and conversions agree between the interpreter and
  matching-host native execution across all six relations and operand
  orientations, constants and dynamic operands, differing widths, negatives
  versus unsigned values, equality and extrema above `i64::MAX`; negated
  guards, literal landing, once-only ordered evaluation and overflow
  obligations hold. `fail/slices/signed_slice_index_upper_only` still rejects.
  Coordinate library suffix migration with
  **CANONICALIZE-SCALAR-RANGE-CONTRACTS** without weakening these obligations.

## Universal interpreter cutover

Move executable Omega computation to one Psi-owned core consuming Terminal
Psi. Ordinary runs, semantic evaluation, Build execution and embedding differ
in their inputs, effect bindings and result handling; they do not own separate
operation dispatchers, call stacks or value/store implementations. The existing
`omega-rust/psi/interpreter/terminal-interpreter/` is the destination. Do not
introduce a third IR, a third engine, a tree-input execution mode or a shared
wrapper around both old engines. Governing contracts are the
[pipeline](omega-rust/pipeline.md),
[semantic evaluation](wiki/spec/language/evaluation.md) and
[embedding](wiki/spec/build/embedding.md).

This is an owner-authorized breaking migration. Establish the small build,
check and run path needed to develop the compiler, then delete tree execution
before restoring all feature coverage. Do not retain the old engine merely to
keep every canary functional, extend its unsupported features, or wait for full
parity before cutting over. A lost capability becomes explicitly unimplemented
with an owning restoration item; its authored source and result oracle stay.
Unimplemented is unfinished work, never a successful execution or release pass.
Existing items that ask for a checked-interpreter repair must instead restore
that behavior through the common core once its deletion lands.

The minimum development path is a Rust build, a source check accepting a
known-valid program, a source check rejecting a known-invalid program, and an
ordinary package-aware `omega run` producing its authored result. Freeze those
specific witnesses and their transitive requirements in **TERMINAL-BUILD-SEED**;
do not invent a larger compatibility exception. Retain legacy execution only
until that seed can use the common core. Afterward there is no tree fallback.

Missing migration coverage may be a warning in an explicitly optional
development comparison, with the comparison reported as unavailable. A
requested execution still reports unimplemented; an actual wrong result is a
failure. Type, arithmetic definedness, ownership, termination, effect/grant and
foreign-package admission failures remain errors. Remove redundant production
replay under
[verification placement](omega-rust/pipeline.md#verification-placement)
rather than downgrading it into a new warning pass. Do not turn arbitrary
errors into unimplemented or warn and run code whose required semantic checks
failed.

Claim individual assignments and paths. Each restoration lands one operation
or caller family, removes its superseded mechanism, and names an unchanged
program plus an invalid control. Reuse existing capability owners rather than
duplicate their representation or lowering work. Native/corpus success does
not establish ordinary CLI package admission; report both routes accurately.
Delete these items as their acceptance closes. The order is:
gap reporting -> core entry -> early jobs -> development seed -> tree deletion,
then independent claimed restoration slices -> final closure.

- **INTERPRETER-CORE-ENTRY.** (new-scope) Make the existing Terminal engine's
  program/instance lifecycle the common execution entry, without checked-tree
  dependencies, compiler-owned source lookup or a separate trusted execution
  engine. Reuse one core for direct interpretation and embedding; drivers own
  exact entries, supplied values, effects, resource provisions and observations.
  `TerminalProgram::load` now decodes directly without proof bytes or receiver
  verification. Keep genuine
  foreign-package/build confinement at its existing trust boundary.
  Owners: `omega-rust/psi/interpreter/terminal-interpreter/src/program.rs`,
  `src/instance.rs`, `src/execution.rs`, `src/adapters.rs` within that crate.
  Expose one interpreter facade that accepts the common borrowed Terminal
  artifact view, exact provisions and host resources. Its source convenience
  route asks the Psi driver for an artifact; its binary route admits owned
  bytes once. The application and tests depend on this facade, never directly
  on interpreter internals or compiler runtime helpers.
  Acceptance: a produced Terminal program executes through the common lifecycle;
  missing/wrong-instance imports fail before effects, and pause/resume does not
  repeat an effect. Source constructors and byte loaders reach that same core.
  A current `--both` result alone cannot close this item while it uses trees.
  Current frontier: direct, embedded and corpus artifact entries share the
  admitted program/instance lifecycle. The corpus uses the native report's
  exact Terminal artifact/profile for the same selected target; refusal retains
  its original error without replaying source or Build. Actual Unit completion
  and typed Trap observations execute through the core. Exact reachable import
  rosters reject missing, wrong-signature and wrong-instance bindings before
  effects; fuel pauses preserve four ordered effects without replay. Alternative
  reference-result backing is an explicit pre-effect capability gap, with the
  unchanged canary designated in `tests/omega/interpreter_migration.json`.
  Exact entry/host adapters remain **TERMINAL-RUNTIME-DRIVERS** work. The missing
  source-program constructor and production execution callers remain; do not
  add another wrapper around the existing engine.

- **EARLY-TERMINAL-JOBS.** (new-scope) Produce a checked, closed Terminal job
  for a demanded compile-time machine and its complete call dependencies before
  the whole application's types/array lengths are ready. Reuse declaration
  selection, specialization and Psi stages 04-07; do not add a second lowering
  implementation or complete compiler driver. Keep unrelated product
  declarations as non-executable metadata, order prerequisite evaluations, and
  reject real cycles with their dependency chain rather than inserting zero
  lengths or weakening admission. Psi compile-time cannot depend upward on the
  Omega product driver; extract only the reusable target-neutral production
  operation into its Psi owner. Depends on **INTERPRETER-CORE-ENTRY**.
  Owners: `omega-rust/psi/compilation/compile-time/src/machine_execution/`,
  `omega-rust/psi/compilation/compile-time/src/const_evaluation/const_lengths.rs`,
  `omega-rust/omega/compilation/driver/terminal-production/src/` and the owning
  Psi stages. One immutable declaration/type arena supplies every job. Job-local
  overlays retain selected closures, evaluated values and materialized results;
  compile-time evaluation must not clone `SyntaxTrees`/`TypedTrees`, inject a
  probe into a cloned whole program or call `lower_typed_trees` as a private
  second driver. Remove those routes from array lengths, range arguments,
  initializers and named-requirement evaluation rather than hiding them behind
  the early-Terminal adapter. Acceptance includes source guards for those
  forbidden clone/recheck shapes and a measured peak-memory reduction on a
  package with many shared constant dependencies.
  `pass/comptime/runtime_const_array_length_transitive_exit`
  computes 16 through Terminal and its unchanged application exits 70;
  `pass/build/const_index_dependency_order_exit` respects leaf-before-outer
  ordering. `fail/comptime/{negative_const_array_length,effectful_const_array_length}`
  still reject. Record a precise dependency if the demanded closure itself
  cannot yet check; do not establish a permanent typed-tree execution exception.
  Current frontier: the shared stage 05-07 production operation is now owned by
  `omega-rust/psi/compilation/driver/`; the Omega product adapter
  delegates to it and retains product timing/report vocabulary. Fixed-array
  jobs now demand within the original executable roster, settle shared type
  prerequisites in one private arena and reject actual cycles with their chain.
  Each job retains its checked facts and exact prepared entry; selected-provider
  meaning survives ordinary call normalization. Common Terminal job production
  and execution still depend on **INTERPRETER-CORE-ENTRY**. The current evaluator
  remains the existing tree engine; this does not close the Terminal acceptance.

- **TERMINAL-BUILD-SEED.** (new-scope) Establish the minimum package-aware
  Build activation that lets agents keep building/checking/running the compiler
  after tree execution is removed. Depends on **EARLY-TERMINAL-JOBS**. Inventory
  the actual transitive Build/core/stdlib requirements of the seed before
  selecting its scope; a minimal source file does not imply a trivial Build.
  Retire static syntax projection as a second Build language. Put project-role
  and dependency request operations in ordinary `omega::core` boundary
  traits/machines with compiler/package-manager host implementations. Compile
  the one free Build entry against core, package-local Build code and pre-known
  provider-slot requirements, then run it once in the Terminal interpreter with one
  mutable `Build`. `depend` and `build_depend` operations record exact requests
  and may start package work without suspending the activation. When Build code
  needs a helper, `suspend Build::await_provider<R>(builder, alias)` parks that exact
  continuation until package management resolves/reviews the request, recursively
  activates dependency Builds and Psi admits the selected implementation. The
  interpreter installs the exact Build-scoped provider slot and resumes without replay. Helpers and
  control flow are ordinary checked Omega, not direct-call syntax recognized by
  Rust. Static paths from a newly requested dependency remain unavailable to the
  root's initial checking; subsequent helper calls use
  `R::operation(arguments)`.
  `bind`, `select_provider`, `assume_binding_contract` and test registration are
  admitted Build operations whose product operands resolve in the product
  context ([scoped execution](wiki/spec/build/scoped_execution.md#selecting-product-declarations-without-executing-them));
  they remain inert during Build and resolve at the final join rather than
  becoming dependency discovery or executing product bodies.
  A missing `build.omg` is compiler-constructed ordinary source for the same
  one-entry request/binding/join contract, not a separate
  `BuildConfig::default()` semantic route. Keep execution deterministic, bounded
  and authority-explicit. Product selections may retain inert references while
  dependency work proceeds, but no product body executes during Build. On Build
  return, join every outstanding request, reject failed/cyclic/unadmitted edges,
  resolve product references and freeze the output before product compilation.
  Product description queries do not execute product bodies. Preserve
  **BUILD-EVALUATES-ONCE**, pre-effect admission and all-target authored rows;
  do not retain a feature/fixture-specific Build evaluator.
  Preserve the seed and invalid acceptance below. Add acceptance for a request
  emitted from a local helper/conditional, an unused build dependency completing
  without suspension, typed acquisition parking and resuming once, recursive
  dependency Build activation, and cycle/failure rejection at binding install
  and final join. An unresolved static dependency import must reject.
  Owners: `omega-rust/omega/compilation/build-evaluation/src/`, Psi compile-time
  admission, exact Build catalog contracts and Terminal lowering/adapters.
  Acceptance on Windows and macOS: `mbx check -p omega`; ordinary source check
  of the valid seed; rejection of
  `tests/omega/fail/arithmetic/array_constructor_divisor_requires_nonzero/main.omg`;
  package-aware `omega run tests/omega/pass/float/named_provider_directed_divide_exit/main.omg`
  after normal local package preparation produces 70. Freeze its actual closure,
  not the whole passing corpus. Unrelated unsupported features may become gaps.
  Current frontier: `Build::application`, `Build::depend(Source::Path)` and std's
  `Build::package` are statically projected by
  `omega-rust/omega/packages/build-declarations`; that parser is migration debt,
  not the retained architecture. Delete its role/dependency call-name grammar,
  package-manager projection consumers and fixture copies after the interpreted
  Build activation carries the same exact identities. Implement application
  coordination for asynchronous dependency jobs and final join, package-manager
  resolution/review, Psi requirement admission, and Terminal continuation plus
  provider-slot installation as their existing owners rather than another Build
  driver. The seed's
  runtime work is its four authored `Build::bind` rows. Unpublished source
  preparation carries generalized `RootBinding` plans, exact declaration
  coordinates and authenticated original Build inputs through the common core;
  its four ordered observations, pause/resume without replay, actual settings
  readback and foreign/malformed-input refusals are measured. This is not the
  default package Build route. Forwarded identifier mutation still refuses
  `InvalidStructuralByteViewFieldStore(OperationId(8))`: the mutable boundary
  receiver has no portable stored-descriptor effect/lifetime contract.
  `RetainedBorrow` covers shared inputs returned as structural results, not
  Unit-return receiver writeback. Carry an authenticated general mutable-slot
  effect/backing relation through checked admission, boundary declarations,
  production, provider installation and normal-exit replay. Do not infer slot
  preservation from the current observation-only adapter, a call name or source
  custody; later unknown mutable writes remain unknown. Separate current-store
  origins from output safety while retaining rejection of local backing escaping
  through borrowed output on a Unit return. Every package root and dependency
  Build is sponsored. Connect the independent
  `evaluation-accounting` invocation accounts to actual Terminal semantic fuel,
  owned live storage and result custody before allocation or publication;
  preserve aggregate allowances across the package closure and retries. Retained
  usage and cache reuse must keep the engine, fuel and storage measurements
  distinct; checked-tree counters cannot represent Terminal execution. Native
  execution through the existing tree route does not close this acceptance.

- **CHECKED-INTERPRETER-DELETED.** (new-scope) Delete the tree execution
  engine immediately after **TERMINAL-BUILD-SEED** closes; explicit migration-gap
  reporting is implemented and full feature parity is not a dependency.
  Remove `omega-rust/psi/interpreter/checked-interpreter/` and production
  dependency edges, expression walkers, call/frame/store machinery and worker
  entrypoints. Remaining consumers are compile-time and Build evaluation,
  CLI/corpus comparison, provider selection and trust records. Move their
  still-required data interfaces and genuine host helpers to their
  semantic/tooling owners; retire receipt
  families already revoked by the owner. Do not leave the old crate as a
  facade or move its evaluator into a host adapter. Every unimplemented caller
  returns a specific capability gap before unsafe or partial execution, with
  an owning restoration item. No fake values, generic catch-and-continue,
  warning-based semantic admission or tree fallback.
  Acceptance: Rust `omega` still builds; the seed check/run controls work;
  missing features appear in gap results with preserved authored canaries;
  source/dependency audit finds no checked-interpreter or executable tree
  visitor. Optional comparison can report unavailable, never agreement.

- **TERMINAL-CONST-SCALARS.** (split-of:EARLY-TERMINAL-JOBS) After
  **CHECKED-INTERPRETER-DELETED**, restore pure scalar evaluation callers
  through early Terminal jobs, one caller family per landing: const lengths,
  range endpoints, generic values and initializers. Calls, branching, mutation
  and measured recursion use the ordinary core, not a constant-fold evaluator.
  Owners: `omega-rust/psi/compilation/compile-time/src/const_evaluation/` and
  Terminal scalar/call operations. Acceptance: restore
  `pass/comptime/runtime_const_measured_recursion_exit` (70),
  `pass/comptime/range_endpoint_requires_discharge_exit` and
  `pass/generics/runtime_const_data_machine_call_exit` to their authored
  oracles; `fail/comptime/recursive_accumulator_requires_coupling` still
  rejects and fuel exhaustion is not a semantic value. Preserve integer
  width/domain behavior; explicitly account for changed charging/cache identity.

- **TERMINAL-CONST-PROVIDERS.** (split-of:EARLY-TERMINAL-JOBS) Restore
  provider-dependent early computation through exact Terminal operation and
  selected-provider identities. Depends on **CHECKED-INTERPRETER-DELETED**;
  coordinate **PROVIDER-SELECTION-AFTER-TERMINAL** and
  **OPERATOR-BOUNDARY-CALLS** rather than introduce another selection registry.
  Preserve the once-per-Psi, cross-target constant provider agreement contract;
  no per-target layout-determining folds or spelling-selected primitives.
  Owners: compile-time selected operators/admission, Terminal selection
  vocabulary and existing provider installation owners. Acceptance:
  `pass/comptime/selected_float_array_length_exit` exits 70;
  `pass/comptime/provider_boundary_range_endpoint_exit` and
  `pass/providers/checked_boundary_operator_const_application_alternate_exit`
  meet their oracles; same-spelled user operators gain no primitive privilege
  and divergent requested-target selections reject. Mark missing installations
  unimplemented; do not weaken physical feature/ABI checks.

- **TERMINAL-CONST-STRUCTURES.** (split-of:EARLY-TERMINAL-JOBS) Restore
  owned record/case results, arrays/bytes and local references through one
  snapshot bridge and the core's storage/loan implementation. Depends on
  **CHECKED-INTERPRETER-DELETED**. Land each semantic value family separately.
  Inputs are owned values; only typed recursive snapshots return to the
  compiler. Materialization remains a separate judgment; no runtime cell,
  interpreter alias or host pointer escapes. Handle-backed identity remains
  authoritative across aggregate paths and calls.
  Owners: Psi compile-time materialization and shared snapshot vocabulary;
  Terminal values/storage/references. Acceptance: restore the structured
  consumers in `pass/layouts/runtime_plan_laid_value_field_exit` and
  `pass/layouts/runtime_plan_laid_nested_fixed_array_mutable_write_exit`;
  mismatched nominal case/width and escaping references reject, while local
  alias writes remain visible without copy-out. Scope this item to value
  transport; existing layout/loan owners still own their semantic repairs.

- **TERMINAL-LAYOUT-WIRE-EVALUATION.** (split-of:EARLY-TERMINAL-JOBS)
  Restore compile-time layout, placed-view, wire and executable proof-value
  consumers through the common early-job producer/snapshot bridge. Depends
  on the required **TERMINAL-CONST-STRUCTURES** families; coordinate existing
  layout, reflection and **WIRE-CODEC-LIBRARY** owners. Land one consumer
  family, retire its direct tree call, and promote its canary only after the
  authored oracle executes. Compiler schema queries return explicit immutable
  descriptions; host adapters do not interpret guest code. Missing executable
  semantic value/operation coverage remains a named gap. Symbolic mathematical
  kernel checking is a separate judgment, not a justification for a second
  executable interpreter.
  Owner: `omega-rust/psi/compilation/compile-time/src/layouts/` and owning
  Terminal operation/value representations. Acceptance: restore
  `pass/wire/runtime_wire_policy_authored_plan_exit` and
  `pass/layouts/runtime_plan_laid_value_field_exit`; malformed policy results
  and inadmissible host observations still reject. Retain value-sensitive
  constant materialization rather than accepting a result merely by type.

- **TERMINAL-RUNTIME-DRIVERS.** (new-scope) Restore CLI comparison/corpus
  execution over the produced Terminal program, exact selected entry and host
  bindings after **CHECKED-INTERPRETER-DELETED**. Share program/instance/core
  execution with embedding. Runtime drivers provision stdin/output, context,
  virtual clocks and exact imports; they do not recompile the authored file,
  rerun Build or evaluate source expressions. Restore complete operation/import
  families, not a simple-program eligibility matrix. Unsupported interpretation
  stays explicitly unavailable; actual disagreement is an error.
  Owner: the interpreter facade under `omega-rust/psi/interpreter/` owns
  Terminal loading, entry establishment, supplied-slot installation and
  execution. Target-aware host adapters belong outside Psi in Omega-owned host
  integration, using independent backend ABI/locator/mechanism libraries, not
  compiler stages. Split `omega-rust/omega/compilation/driver/src/runtime.rs`
  along that boundary; retain exact foreign identity and ABI checks before
  effects. CLI execution, embedding and corpus consume these owning APIs.
  Stdin/stdout/stderr, virtual clocks, provisions and observations are not
  compiler responsibilities. Remove `compiler::runtime` and direct
  `terminal-interpreter` dependencies from the application. Add a production
  command/API that executes an owned Terminal artifact without compiling
  source, beside the source route that asks only the Psi driver.
  Replace source-checking-dependent `program-entry-plan` inputs with source-free
  runtime provisions; the producer owns the exact provision/schema join.
  Follow [target environments and engines](wiki/spec/build/embedding.md#target-environments-and-execution-engines):
  Psi gains no Omega dependency, ABI catalog copy or native fallback. Hosts
  explicitly supply bindings; ordinary provider bodies can execute in Psi.
  Acceptance: restore **WINDOWS-CONSOLE-INPUT**'s Loader and
  five bounded-line controls through the common core. Missing/wrong-instance
  imports reject before effects; stdout/stderr and exit oracles are independent.
  A native compilation failure or comparison decline cannot count as agreement.
  Current frontier: the corpus adapter consumes the produced Terminal artifact
  and uses the common core; it no longer calls the checked-tree interpreter.
  The shared `driver/src/runtime.rs` consumes retained native entry/provider
  provisions, establishes fresh private hosted backing and erased root fields,
  binds typed buffered line/byte output. Preserve
  `float/{named_provider_directed_divide_exit,runtime_float_integer_conversion_boundaries_exit}`,
  `providers/foreign_out_parameter_write_observed_exit` and
  `host/process_exit_explicit_standalone_domain_exit`: standalone sponsors must
  explicitly delegate exit to their private instance; embeddings default to no
  authority, independently of entry fields, reach and provider selection.
  Exact instance/slot and crossing admission remain mandatory. Complete external
  demands, including possible dynamic
  rows, are admitted before effects; normal imports cannot accept external
  signatures. Remaining arrivals include scalar entry arguments, placed host
  loans, case replacement involving opaque or inline-byte children without a
  semantic codec, fragmented/authored case-member storage, accesses into actual
  padding or reference bytes,
  non-hosted roots and required
  receiver qualifications. Foreign write-only loans depend on
  **WRITE-ONLY-BORROW**'s admitted implementation-bound non-observation evidence;
  widening runtime permission cannot bypass the source rejection. Preserve
  `host/runtime_descriptor_binding_{state_forwarding_input,record_result}_exit`
  and `fail/borrows/binding_state_mutable_from_shared`: scalar and structural
  completion must retain the original Binding occurrence without elevating
  shared access to mutable access.
  Hosted byte input uses
  an explicit finite replay stream shared by selected input slots; CLI and
  corpus give each engine the same authored bytes with independent positions.
  Preserve `calls/runtime_call_carrier_partition_exit`,
  `borrows/runtime_projected_reference_call_return_exit` and
  `calls/runtime_heterogeneous_case_argument_exit` together with
  `structs/runtime_mixed_common_field_store_exit` while completing host
  arrivals: physical backing prefixes are not logical partial ownership,
  case transport validates selected contents, not inactive variants,
  and common mixed fields use the member namespace, never a case payload.
  Preserve `host/runtime_descriptor_case_replacement_exit`,
  `host/runtime_descriptor_nested_case_array_exit` and
  `host/runtime_descriptor_case_atomic_input_exit`: exclusive host input may
  retag original storage only with complete valid active contents; unread
  bytes, byte-array backing, common fields, atomic cells and outside siblings
  retain their actual custody. A borrowed state arrival keeps its address or
  descriptor through foreign-call lowering, not an invented owned home.
  Preserve `calls/runtime_atomic_case_shared_argument_exit` and
  `atomics/runtime_mixed_common_atomic_cell_exit`: completed scalar initializers
  establish owned case cells, and shared helper events mutate their original
  member backing at the exact post-tag offset.
  Complete clock imports beyond admitted Darwin reads/signal-free waits and
  Linux clock_gettime, finite descriptorless poll and signal-free nanosleep: descriptor readiness
  needs actual custody, CPU clocks need accounting, Darwin invalid nanosleep
  requests need an errno/cancellation model, and remaining target calls
  need exact adapters. Preserve `time/runtime_kernel_clock_wait_loans_exit`:
  read-only timespec requests retain original backing and suffixes, invalid
  requests return raw kernel status without advancing time, and successful waits
  do not invent access to the unobserved remainder carrier. Linux native execution
  still needs matching-host qualification; cross-target interpretation is not it.
  Preserve `time/runtime_clock_shared_import_slots_exit` across those repairs:
  all selected read/wait slots share one invocation's explicitly supplied time
  model, with unchanged checked calibration; simulation is not native latency
  evidence. Preserve `time/runtime_clock_out_parameter_loans_exit`: both clock
  result formats share that model; timespec writes use retained native byte
  placement and authenticated original loans, not record names or field order.
  Helpers, captures and fixed arrays keep backing; unwritten suffixes and sibling
  storage remain unchanged. Unsupported selectors need their actual time model,
  never invented CPU accounting.
  Keep `time/runtime_clock_padded_out_parameter_prefix_exit`: the fixed timespec
  footprint cannot demand or synthesize unaccessed tail padding in a larger loan.
  Preserve `host/runtime_descriptor_output_streams_exit`,
  `host/runtime_descriptor_direct_array_exit` and
  `host/runtime_descriptor_nested_array_window_exit` while extending host buffers:
  nonzero and empty windows retain the original backing and exact interval,
  including nested fields forwarded through an ordinary helper. Their
  views transport pointer/length, primitive loans retain their exact extent,
  and stdout/stderr oracles remain independent. Preserve the public-command
  byte handoff in `host/runtime_descriptor_binary_output_exit`: neither stream
  is UTF-8 text, and a failed forwarding write cannot report success. Preserve
  `borrows/runtime_direct_immutable_scalar_loan_exit` too: computed loans,
  captures, assignments and helper writes must share original scalar backing.
  Preserve `host/runtime_descriptor_scalar_input_exit`: canonical boundary-trait
  and Linux kernel slots share one finite input cursor; short/zero/EOF reads
  retain untouched backing, and discarded counts still transfer bytes to the
  independently observed streams. Remaining descriptors need actual custody;
  oversized kernel transfers need selected page-size limits rather than
  fabricated complete writes. Linux native input/output remains unqualified.
  Preserve `host/runtime_descriptor_{float_loan_input,boolean_loan_output}_exit`
  and `fail/borrows/boundary_primitive_loan_mutable_from_shared`: boundary and
  checked-body calls must share primitive-home loan admission and argument
  validation. Floating-point output retains exact IEEE bits; Boolean output is
  its canonical byte, not text. Thin foreign pointers may unwrap an internal
  scalar descriptor only when the existing storage-view relation proves its
  exact byte footprint; a mutable loan exports its logical width, while shared
  and write-only ambiguous-width scalar exports remain unavailable. That
  mutable rule relies on Psi rejecting narrowed (`IntegerAt`) storage before a
  bodiless callee; Psi still accepts a narrower `&mut i8` or a refined field
  passed to a wider `&mut` formal whose callee may write out of range, and
  lowering refuses both today, so whoever completes that lowering must add the
  invocation fit admission.
  Preserve `host/runtime_descriptor_immutable_primitive_loans_exit`: taking
  an address must keep one original owned home for direct imports, captured
  references, helper/state forwarding and later scalar/index reads. Copied
  bindings remain snapshots; binding replacement mutability is not borrow authority.
  Preserve the mutable-view and record-array input fixtures too: original
  array/field windows retain exact live extents and unread/outside bytes;
  field presentation cannot substitute the record owner's type or grant resizing.
  Preserve `host/{runtime_descriptor_record_loan_input_exit,runtime_descriptor_nested_record_loan_input_exit}`:
  whole-record loans retain original scalar fields, nested arrays and restrictions;
  native placement supplies byte coordinates without inventing padding or pointers.
  Preserve `host/runtime_descriptor_padded_prefix_loans_exit`: read/write demand
  covers only the accessed byte prefix, retains unread bytes inside a touched
  scalar and leaves neighboring fields unchanged. Padding outside that interval
  supplies no bytes and cannot invalidate an otherwise backed prefix.
  Preserve `host/runtime_descriptor_reference_record_prefix_exit`: borrowing a
  record is not an owned copy; untouched descriptors retain their original
  referents while the demanded scalar prefix receives writeback. Non-scalar
  members remain uncovered byte intervals, never interpreter pointer values.
  Preserve `host/runtime_descriptor_exclusive_atomic_record_exit`: exclusive
  aggregate host loans retain original atomic backing through partial reads and
  instruction-observed fetch results; shared output may access a disjoint
  ordinary prefix but cannot snapshot the atomic tail. Opaque calls expire old
  residency facts; their sequencing boundary is not an internal-write witness.
  Preserve `host/runtime_descriptor_local_array_loan_input_exit`: shared and mutable
  whole-array loans keep their owner's activation across calls and state edges;
  only owned occurrences copy payload, and short reads retain unread elements.
  Selected-reference backing retains its separate capability attribution.
  CLI comparison consumes the exact native report before publication; missing
  imports decline without reinterpreting trees or rerunning source/Build.
  Preserve `errors/runtime_trap_process_presentation`: CLI comparison matches
  typed Trap to the host's exact architectural termination presentation and
  rejects unrelated signals or numeric exit fallbacks; unmapped Abort still
  declines. Presentation agreement is not crash-site or full-trace equality.
  A same-target native failure returns
  no retained intermediate
  artifact; observing that Terminal program requires intermediate-artifact error
  custody rather than a second source compilation. Qualify Windows native
  input/output and comparison on Windows after binding the exact foreign leaves;
  macOS cross-realization alone supplies no Windows execution evidence.

- **TERMINAL-BUILD-QUERIES.** (split-of:TERMINAL-BUILD-SEED) Restore Build
  query/selection families beyond the development seed after
  **CHECKED-INTERPRETER-DELETED**. Guest computation always runs in the core;
  exact compiler-service contracts provide product descriptions, schema,
  providers, target rows, exclusions and dependency declarations. Describing an
  uncompiled declaration does not make its body executable. No string-based
  privilege, fabricated runtime markers or relocated Build evaluator. Bind
  catalog operations through closed identity and explicit activation authority.
  Owners: Build evaluation, Psi compile-time admission, existing compiler
  catalog/query owners and Terminal operation lowering. Acceptance: restore
  `pass/build/{described_runner_entry_right_exit,runtime_depend_mapping_exit}`;
  `fail/build/{product_entry_query_incompatible_signature,program_entry_binding_outside_build}`
  and fabricated-activation controls still reject. Land one query family at a
  time and count one Build activation under **BUILD-EVALUATES-ONCE**.

- **TERMINAL-BUILD-EFFECTS.** (split-of:TERMINAL-BUILD-SEED) Restore granted
  filesystem, logging, generated source/dependency and output staging through
  Build-specific host bindings after **CHECKED-INTERPRETER-DELETED**. Pure jobs
  receive no such bindings; fresh instances do not roll back real host effects.
  Generated inputs return to ordinary loading/resolution. Restore filesystem/
  grants, source/output staging and nested dependencies as separate landings;
  keep **BUILD-EVALUATES-ONCE**, **BUILD-SNAPSHOT-OUTPUTS** and existing
  output/receipt retirement semantics. Resource provisions and effect order
  stay explicit; do not equate tree steps with Terminal fuel or preserve dead
  receipt machinery merely for cache compatibility.
  Owners: Build driver, package compilation-inputs/build-output/source owners,
  exact filesystem bindings and common core resource lifetimes. Acceptance:
  an authored Build emits one declared source/output and the application meets
  its oracle, including a nested generated dependency; denied traversal,
  missing required output, wrong grant and exhaustion retain correct failure
  and partial-effect reporting. Use ordinary CLI package preparation. The
  existing `pass/build/artifact_only_required_output_completion` and
  `fail/build/artifact_only_{rollback_overlay,unsealed_output}_refused` controls
  already use sealed corpus source staging. Restore them through the core;
  **COMPILE-REPORT-PRODUCTS** owns the missing no-partial-publication assertion.

- **UNIVERSAL-INTERPRETER-CLOSURE.** (new-scope) Finish after the above
  restoration items close: one execution core, no executable trees, no legacy
  fallbacks/facades, and no migration gaps in the accepted release scope.
  Audit every retired crate consumer and update architecture/entry documentation
  where the old split remains. Inspect `source/psi/` and `source/omega/` against
  the same job/value/effect contract and name any missing product implementation
  on its owning task; Rust-only consolidation is not completed product compiler
  delivery. Keep the accepted bootstrap chain and its proofs unchanged.
  Acceptance: common-core public run and negative controls on Windows/macOS;
  justified final native/interpreter corpus coverage meets unchanged authored
  results; source/dependency audits find one dispatcher/call/store/resource
  implementation and no tree interpreter. Every migration-designated canary is
  either restored and promoted or still an explicit open release dependency,
  never silently skipped or marked passing. Remove obsolete comparison audits
  and migration-only exceptions; optional debugging cannot become another engine.

## Pipeline route

Follow one connected stage chain under the
[pipeline](omega-rust/pipeline.md#one-driver-one-pass-per-target-realization)
and [multi-target contract](wiki/spec/build/configuration.md#multi-target-compilation):
Psi checks every target's bodies once, Build evaluates once, and `--target`
narrows realization only. Complete ordinary operation repairs against their
named customers while the shared-source/provider work converges. Remove each
replaced route as its common operation takes over; preserve negative checking,
crash and native exit behavior.

- **PIPELINE-ARCHITECTURE-ENFORCEMENT.** (new-scope) Own convergence to, and
  permanent enforcement of, the connected route in
  [the pipeline contract](omega-rust/pipeline.md): Psi 00-07 and Build execute
  once over every authored target body; one canonical Terminal Psi product is
  the only portable handoff; Omega 00-09 realizes each requested target from
  that product and target-keyed Build rows. This item covers integration and
  acceptance across **PSI-TARGET-FAMILIES**, **BUILD-EVALUATES-ONCE**,
  **PROVIDER-SELECTION-AFTER-TERMINAL**, **ONE-DRIVER-PER-STAGE**,
  **ONE-TRANSITION-LOWERING**, **ONE-CALL-LOWERING** and the route-retirement
  items. Contributors may own those slices; a slice landing or an architecture
  lint alone does not close this outcome.
  - Remove the current violations rather than grandfathering them:
    `compiler.rs` checks Product once and shares checked arenas and Terminal
    storage across distinct entry roots and callback bodies. Package-manager
    batches supply dependency-ordered input plans but still retain separate
    checked activations per target. `execution_settlement.rs` plans
    providers after checking but before Terminal production. Target-layout source
    folding is removed,
    with portable layout-query execution tracked by **TARGET-SEMANTIC-APPLICATIONS**;
    `realization/provider-planning` still depends on typed/checked trees,
    stage 04, validation and the checked interpreter; Omega stage 00 consumes
    checked entry and source-shaped callback ABI/resource placements beside the
    canonical artifact; stage 05 selects shape-specific routes. Callback body
    identity lives in Terminal machines and demands; native callback identities
    use the caller's Terminal identity and placement ordinal, not source handles.
    Native re-entry retains produced callback bodies and ABI/argument rows,
    not checked-source placement records. Stage 00 consumes completed calling
    applications rather than provider-planning's evaluator records; its direct
    dependency on that source-aware planner is gone. Callback placements now
    cross with portable contract/ABI records; source selection stays at the
    producer, and backend-plan no longer depends on checked trees. Initial
    ABI/resource planning and entry signatures still need source-free inputs.
    Native stage 00 and product records consume shared Terminal installation
    contracts, not interpreter types; runtime dispatch stays in the interpreter.
    Native sequencing now lives in `native-production`, with no transitive
    source-IR/checker/interpreter dependency; `check_native_boundary.py` enforces
    that cut. Extend enforcement to the remaining source-aware producer and
    consumer boundaries below, rather than restoring access through evidence.
  - Make topology enforce ownership. Remove forbidden production dependencies
    so Rust cannot name a representation or operation outside the consuming
    stage's declared inputs. Add one workspace architecture gate derived from
    `cargo metadata` that rejects a Psi dependency on Omega, an Omega
    pipeline/realization dependency on source/typed/checked Psi or Psi transform,
    source-checking or interpreter crates, and any stage edge outside the declared
    producer/consumer graph. Allow explicit test-only fixture construction
    without adding the dependency to the production target. The gate names the
    forbidden edge and the owning stage; it does not maintain a second pipeline
    roster unrelated to `omega-rust/pipeline.md`.
  - Enforce product facades. Only the respective compiler drivers assemble
    the Psi 00–07 and Omega 00–09 production sequences. Transforms still depend
    directly on their declared input/output representations and independently
    owned semantic, foundation and backend libraries. Package manager,
    application, interpreter, inspection and test runners depend on those
    drivers/facades plus shared request/artifact contracts. Add `cargo metadata`
    guards for these application/package/interpreter boundaries. Remove the
    application's remaining target and package-compilation implementation
    dependencies. Public Terminal artifact/view types remain shared contracts,
    not implementation access to hide behind a driver re-export. Inspection
    must continue through the compiler's explicit production operation.
  - Enforce complete producer records. Stage 05 may consume checked plans, not
    `checked.typed`, source statement/expression tables, validation helpers or
    source-node kinds. Omega may consume canonical Terminal Psi, Build rows and
    target/backend contracts, not lowered-Psi occurrences or checked receipts.
    When a consumer lacks a fact, extend the producing stage's durable output
    and migrate every consumer in the same coherent change. A downstream
    reconstruction, source-shape recognizer, fallback route or revalidation of
    the producer's invariant is a failed architecture check, not a temporary
    feature repair.
  - Remove the remaining source joins beside Terminal: publication now transfers
    its semantic module, and provider installation/runtime borrow it directly,
    and boundary demands/candidates plus callback registrar applications are
    canonical producer output. Callback body/placement settlement and entry
    admission still consume checked/lowered receipts; provider planning still
    reads frontend declarations. `LoweredSourceCustody` is producer-private,
    used only by explicit correspondence audits, not native settlement.
    Carry their required meaning in the canonical producer output and delete
    those source-dependent inputs. External package/artifact ingress decodes
    once; it is not an internal handoff API.
  - Carry representation classes instead of recognizing source spellings.
    Psi assigns the builtin text/string identity and descriptor class; Terminal
    retains it; Omega layout consumes it. Delete backend checks of
    `name.as_str() == "string"` in `layout::builder::type_reference_layouts`
    and `layout::boundary_values`, and add a production source guard against
    source/display-name comparisons deciding semantic type or layout identity.
  - Keep orchestration visible in `compiler.rs`: one source preparation, one
    Psi/Build chain, then an explicit per-target Omega loop. Prefix consumers
    call the same stage entrances and stop at a typed checkpoint; they do not
    create alternate compiler modes or replay an earlier stage.
  - Make the route trivial to drive. Expose one Psi compilation operation over
    prepared source/package custody and explicit Build authority. It returns a
    canonical Terminal product or one typed pending-Build-effects continuation;
    executing admitted effects and resuming that continuation is the only
    interruption of the source-to-Terminal chain. Generated sources re-enter
    through that same continuation, with no callback into a private frontend,
    mode flag or second coordinator. Package review, accepted compilation,
    compile-time jobs and tests supply policy, authority and their stop point;
    they do not assemble stages 00-07, provider settlement or sidecars. Review
    and accepted compilation may run in separate process invocations and need
    not share an in-memory artifact, but both drive exactly this operation.
  - Replace oversized Rust call stacks with explicit arena-handle worklists and
    invocation-owned frame storage suitable for the Omega port. Remove the
    256 MiB command, compile, checked-interpreter and proof-verification thread
    stacks; package/review, syntax, checking and proof traversals run on the
    platform default stack with measured bounded depth. Acceptance includes a
    source guard against production `stack_size(256 * 1024 * 1024)` and peak
    stack measurements for deep syntax, package graphs and proof terms. Heap or
    arena budgets remain separately accounted and no recursion-limit increase
    counts as repair.

  Acceptance: a two-target integration fixture proves with invocation counters
  that source/Psi 00-07, Build and Terminal publication each execute once, then
  Omega 00-09 and emission execute once per requested target; both products
  equal their single-target bytes and one target's realization failure does not
  suppress the other. An invalid body in an unrequested target still rejects
  during Psi, while a missing target-specific realization row fails only that
  target. Retained-artifact realization succeeds from canonical Terminal bytes
  plus the selected target's Build rows after the checked frontend and lowered
  sidecars are dropped. The dependency/source-access gate fails on seeded
  forbidden edges and passes the workspace; target-family positive/negative
  corpus controls and matching-host native results remain unchanged. A minimal
  package caller checks a valid source, rejects an invalid source and resumes an
  admitted generated-source Build through the public Psi operation without
  importing a numbered stage crate, checked-driver carrier, provider planner or
  compiler-internal sidecar; denied effects occur zero times and pause/resume
  does not repeat an effect. A source/dependency guard permits direct numbered
  stage entrance calls only inside the Psi coordinator and explicit stage tests.
  Delete the transitional dependency exceptions and pipeline debt text when
  this passes.

- **REFERENCE-LOCAL-BINDING-ROUTES.** (new-scope) Reference-valued locals from
  every admitted producer keep their loan identity, backing and access through
  reads, calls and explicit state transfers, with exact backing and no implicit
  backing copies.
  - Spec: [ownership: borrows and aliases](wiki/spec/language/ownership.md#borrows-and-aliases),
    [loans](wiki/spec/terminal-psi/loans.md), record re-views under
    [the representation judgment](omega-rust/psi/compilation/checking/recasts.md).
  - Owners: `omega-rust/psi/compilation/pipeline/04_typed-trees-to-checked-trees/src/execution/terminal_unit/control/statement_sequence/local_data.rs`,
    `.../terminal_unit/receiver_aliases/`, stage 05
    `expression_preparation/bindings/` and
    `scalar_lowering/computations/fields.rs`. Reuse the shared source-place
    and view-narrowing operations; do not add local-specific call or range routes.
  - Qualified on Linux x86_64: indexed shared primitive loans through named
    locals, copies, scalar-result and Unit calls, including mixed state formals
    and mutable selectors captured at formation. Native and captured Terminal
    agree for the 16 affected runtime controls; bounds, shared-write, backing
    lifetime and constrained exclusive-store refusals remain enforced.
  - Remaining: qualify the other acceptance routes on Ubuntu and Windows.
    Runtime-indexed shared constrained-field reads also need cross-host
    execution under **PLACED-ACCESS-NATIVE-OPS**; retain the
    [unchanged authored witness](tests/omega/pass/layouts/runtime_indexed_restricted_field_read/README.md).
    The public `samples/cli/systems/descriptor_walk` 50/50 result remains
    unqualified pending authority review: its capability rows were mechanically
    accepted. Preserve the selected-helper, replacement, projected-return and
    shared-cohort controls below while completing these host and authority gates.
  - Constraints: shared loans cannot gain mutation authority by widening the
    exclusive `receiver_aliases` match. Keep the local symbol as the loan
    subject and compose its captured projection at use; substituting the root
    loses borrow/write-frame/release identity. A known-place loan need not
    become a fresh affine `EstablishReference` result. Descriptor cleanup stays
    separate from backing permission and keeps each registered result's complete
    leaf group; whole-owner root releases remain distinct. Returned shared views
    cannot grant mutation or escape activation-owned storage. Named-state forwarding and Unit
    completion must not regain source-shape gates.
  - **STATE-LOCAL-VALUE-FRONTIER** owns integration with the composed caller.

  Acceptance: every producer form, reads through scalar/record loan names,
  borrowed calls and state forwarding execute natively and agree with the
  interpreter. Keep `pass/ownership/runtime_selected_reference_local_exit` (0)
  for distinct incoming and activation-owned backings, descriptor retirement
  and restored mutation, with
  `fail/borrows/selected_primitive_reference_live_backing_write` rejected.
  Keep these controls at their oracles:
  `pass/borrows/runtime_{projected_reference_call_return,replaced_reference_record_return,selected_reference_record_helper,selected_shared_reference_cohort,projected_call_loan_custody}_exit` (0),
  `pass/borrows/runtime_record_byte_region_loan_exit` (50),
  `run/borrows/{reference_local_binding_routes,reference_result_call_binding,shared_record_loan_member_read}` (0),
  `pass/borrows/shared_recast_loan_member_read` (0),
  `borrow/runtime_exclusive_loan_local_write_exit`,
  `recast/runtime_guarded_offset_recast_exit`,
  `recast/runtime_field_precondition_capture_exit`,
  `storage/runtime_{local,parameter}_slice_forward_exit`,
  `slices/runtime_shared_{whole_view,subslice}_return_exit`,
  `calls/caller_result_binding_mutable_loan_exit`,
  `calls/runtime_alias_write_through_guarded_transition_exit` (all 70),
  `slices/runtime_literal_view_return_exit` (105),
  `control_flow/conditional_unit_completion_exit`, and
  `dependent/runtime_sibling_len_index_exit`. The four
  `fail/recast/{field_precondition,incoming_offset}*_invalidated_before_capture`
  fixtures reject, as do `fail/borrows/{forwarded_call_view,projected_identity_result}_retains_operand_extent`
  and `fail/borrows/returned_operand_parent_cannot_bypass_child`.
  Reject conflicting loans, source/path substitution and invalid escapes.
  A `u8` field constrained to zero or one by an ordinary
  data/domain predicate keeps its storage obligation through `&mut u8`;
  do not restore retired range-type syntax. The original sample
  `samples/cli/systems/descriptor_walk/main.omg` runs through
  `omega run --both` and returns 50 in both engines after local package review.

- **TRANSITION-TAIL-CALL-TO-MACHINE.** Terminal machine calls in attached Unit
  transition arms complete the caller for every result and operand form, per
  [state contracts](wiki/spec/language/state_contracts.md#terminal-calls-in-transition-arms):
  a terminal call is an ordinary invocation whose normal result completes the
  caller; an internal state target remains a jump.
  - Where: `calls/runtime_guarded_reference_return_crash` has matching native
    and captured Terminal trap outcomes on Linux x86-64. Matching-host native
    qualification for the strengthened fixture remains on Linux ARM64,
    macOS ARM64 and Windows x86-64.
  - State: Unit, scalar, record, qualified-scalar, case-payload, boundary
    requirement, shared and mutable-reference returns through finite internal
    state cycles, and effectful-guard/operand-order forms execute in both engines.
    Ordinary wrappers, mixed call/value arms, and writes through terminal and generic reference
    results retain the caller's backing storage. Returned call carriers preserve
    alternative record fields and nested argument projections, including ordinary
    reference calls from projected state parameters. Guarded reference, scalar
    and Unit completions before a crash fallback also execute in both engines.
    Whole Linear owners retain their exact claim identities through local moves,
    reordered state arrivals and unconditional, conditional, and guarded terminal
    calls, including scalar and Unit completions. Ordinary consuming
    core `drop` uses the exact owner-authorized normal scalar-record cleanup plan.
  - Remaining:
    - validate matching-host native trap and normal-return oracles for
      `calls/runtime_guarded_reference_return_crash` and its before-crash
      sibling on Linux ARM64, macOS ARM64 and Windows x86-64. The preserved
      macOS obstruction leaves both a native trap and system-clang
      `int main(void) { __builtin_trap(); }` in uninterruptible exit; its native
      qualification needs a host that can reap trap exits. Each declared
      target now has a portable Terminal trap/normal-return oracle, which does
      not replace matching-host native execution. The false-guard caller ends
      normally if the call returns; only the callee's authored crash may
      satisfy its trap oracle.
  - Not this item: `filesystem/dir_walk_wrappers_exit` and
    `samples/cli/systems/note_vault` depend on `Filesystem::create_dir_all`.

  Acceptance: establish the strengthened crash/normal-return pair's native
  oracles on the remaining matching hosts that can reap trap exits. Keep `pass/calls/runtime_linear_guarded_owner_disposition_exit`
  and `pass/calls/runtime_linear_guarded_unit_owner_disposition_exit` at 0 in both
  engines, exercising both selected alternatives. Keep
  `pass/calls/runtime_linear_terminal_owner_disposition_exit`
  and `pass/calls/runtime_linear_state_owner_disposition_exit` at 0 in both
  engines, including sequential local moves and swapped same-typed state owners. Keep
  `pass/calls/runtime_guarded_terminal_mutable_reference_result_exit` and
  `pass/calls/runtime_guarded_terminal_reference_write_exit` at 0 in both engines,
  including writes through a generic reference helper. Keep
  `pass/calls/runtime_selected_reference_field_result_exit` at 0 in both engines,
  including writes through either field and nested nonzero backing offsets. Keep
  `pass/calls/runtime_state_reference_field_result_exit` at 0 in both engines,
  including reordered state parameters, shared and mutable selected fields,
  ordinary reference calls from projected state parameters, and nested backing.
  Keep `pass/calls/runtime_cyclic_state_reference_result_exit` at 0 in both
  engines, including self-loops, multi-state cycles, and projected same-typed
  reference parameters swapped across a backedge.
  Keep `pass/calls/runtime_guarded_reference_return_before_crash_exit` at 0 in both
  engines, including scalar and Unit completion variations. Keep
  `pass/calls/runtime_guarded_terminal_reference_result_exit` at 0 in both engines,
  including its ordinary wrapper and mixed call/value arms.
  Keep `fail/calls/terminal_call_result_type_mismatch_rejected`
  rejecting and these controls at their oracles: `pass/calls/transition_tail_call_to_machine`,
  `calls/{conditional_arm_tail_call_to_machine,guarded_arm_tail_call_to_state_machine,value_tail_call_to_machine,guarded_arm_value_tail_call,runtime_guarded_boundary_tail_call}_exit` (70),
  `calls/{guarded_terminal_call_chain,runtime_guarded_terminal_operand_order,runtime_guarded_terminal_guard_order,runtime_guarded_terminal_array_operands,runtime_guarded_terminal_structural_result,runtime_case_terminal_payload_result,runtime_local_case_terminal_payload_result,runtime_computed_case_terminal_payload_result,runtime_computed_case_subject_affine_argument,runtime_nested_case_subject_operand_order,runtime_qualified_terminal_result,runtime_terminal_requirement_projected_context}_exit` (0),
  `structural/runtime_structural_result_crash_branch_exit`,
  `slices/{domained_runtime_end_subslice,qualified_subslice_composition}_exit` (70),
  `termination/runtime_{shrinking_slice,receiver_byte_slice}_recursion_exit` (70).

- **OPERATOR-BOUNDARY-CALLS.** (split-of:PROVIDER-SELECTION-AFTER-TERMINAL)
  Legacy named operators and compiler-known float realizations finish the
  ordinary requirement-call route and Omega stage 00 installation, per
  [boundary applications](wiki/spec/terminal-psi/boundary_calls.md#operator-applications-and-physical-children).
  Token-bearing machines and nongeneric named operators already enter that
  route; nested boundary applications compose as checked `BoundaryCall`
  computations; float policy results (`FloatResultPolicy`) and whole-carrier
  float conversion casts (`validation::bind_float_conversion_casts`) execute.
  Remaining:
  - **Float/receiver signatures.** Complete call composition for the
    remaining signatures. Retain
    `float/runtime_qualified_carrier_conversion_exit` (0) and
    `float/qualified_conversion_requires_operand_proof` for selected exact
    conversions beneath declared tags, aliases and inline arguments; a rounding
    conversion cannot stand in for an exact cast.
    Run `float/named_integer_conversion_source_proofs_exit` (78) on
    Linux/Windows; those matching-host results remain unverified.
    Retain `float/build_runtime_semantics_twins_x86_baseline` at native and
    Terminal exit 70. `float/build_runtime_semantics_twins_windows_x64` still
    needs matching-host native and Terminal exit 70; Linux execution does not
    establish Windows execution. **TWINS-GOLDEN-RECORDS-A-LOAD-ARTIFACT** owns
    stale macOS timeout records. Late certificate search is gone; do not add
    a float-specific replay.
  - **Windows GUI providers.** Verify explicit Windows provider selection and
    native execution for `samples/gui/{window_app,windowed_calculator}`.
    Their current named maps select macOS operations; historical stage-05
    store/borrow diagnostics are not the current frontier. macOS application
    acceptance is owned by **MACOS-APPLICATION-PUBLICATION**.
  - **Specialized providers.** Retain the exact closed requirement, provider
    instance, static tuple and application occurrence through selected-plan
    settlement into native realization
    (`omega-rust/omega/compilation/pipeline/00_terminal-psi-to-abstract-operations/src/boundary_application_settlement.rs`,
    `omega-rust/omega/compilation/backend/artifacts/native-artifact/src/physical/derivation/evidence.rs`).
    `ClosedReachApplication`, a template alias or equal runtime signatures
    cannot identify the specialization. The four pruned specialized
    fixtures are recovered on current syntax, and
    `specialized_structural_fixed_operator_terminal_custody` now executes
    its operator through native realization: an owned structural parameter
    whose target shape has zero bytes no longer claims an activation-local
    frame home (stage 03 `retain_owned_home` matches the block-parameter
    and edge-transport treatment), which had failed physical staging as
    `FrameLayout(NonCanonicalLayout)`. Specialization retention through
    settlement is pinned by the six checked outcomes; matching-host
    execution remains the native-leg customer.
  - **Directed float arithmetic.** Directed binary arithmetic and directed
    square root (ordinals 67..=69) execute through the local control envelope.
    Directed fused multiply-add remains. Install any further named directed
    arithmetic as Omega builtins through stage 00
    `provider_admission/compiler_builtins.rs` (one
    `NearestIeeeFloatArithmetic` operation per execution), stage 02
    `lowering/control_flow/ieee_float_arithmetic.rs`, and each ISA's
    `selected_form_encoding/ieee_float_arithmetic.rs`. Selected-form tag 230
    carries the operation/format ordinal; append identities, never renumber.
    Nearest FMA (ordinal 44) stays AArch64-only until
    **X86-FMA-PROVIDER-TRANSPORT**. With
    `omega::language::core::float_operations` imported, f32/f64 arithmetic
    and comparison in local, field and guard positions run natively and in
    Terminal on macOS ARM64. Comparison builtins now admit: the six IEEE
    relations ride `IeeeFloatArithmeticOperation::Compare` ordinals 61-66
    with x86-64 and AArch64 selected forms and the host evaluator, so the
    pending canary clears its missing `Float::equal` boundary target and
    stops at the pre-existing program-entry acceptance wall that also
    blocks every entry-bound fixture on this host. Without the import, the
    unchanged `tests/omega/pending/arithmetic/float_computation_no_route`
    stops in checking at "statement sequence: local data: mutable primitive
    local: storage initializer". Preserve declared operator selection; no
    spelling-based fallback.
  - **Receiver-bearing requirements.** Complete a member-call witness through
    borrowed-field argument planning, provider signature refinement and native
    execution, keeping exact receiver/argument correspondence (no blanket
    `is_self` exemption). Reproduce the parameter-source/refinement stops
    before assigning repairs.
  - Checked interpretation (`omega-rust/psi/interpreter/checked-interpreter/src/interpreter/evaluator/execution/operator_applications.rs`)
    declines a requirement with several checked satisfiers because checked Psi
    does not retain the Build's provider choice; this closes with
    **PROVIDER-SELECTION-AFTER-TERMINAL**.

  Acceptance: the `pending/arithmetic/float_computation_no_route` matrix
  executes natively.
  Recover and execute the deleted customers from `089bd3290c`:
  `specialized_{boundary,fixed}_operator_physical_custody` and
  `specialized_{structural_fixed,mixed_structural_fixed}_operator_hosted_native`,
  covering structural-input/scalar-result, mixed-input/scalar-result and
  mixed-input/structural-result hosted cases with the owned affine
  operand/result disposition; an unused helper beside an empty entry is not
  coverage. Keep at their oracles: `providers/{checked_fixed_operator_dispatch,token_machine_family_override,checked_boundary_operator_const_application_alternate}_exit`,
  `providers/{checked_boundary,checked_fixed}_operator_physical_custody`,
  `providers/specialized_mixed_structural_result_operator_hosted_native`,
  `providers/generic_provider_structural_result_composition_exit` (ordinary
  helper calls and constructed results across two closed provider instances),
  `generics/closed_generic_local_field_observation_exit` (nested substituted
  fields retain their scalar type and arithmetic policy),
  `comptime/provider_boundary_range_endpoint_exit`,
  `providers/selected_const_provider_call_identity_exit` (named, token and
  helper calls retain the authored requirement under one selected provider),
  `recast/runtime_mutable_equivalent_record_recast_exit`, and the `pass/float`
  named-provider, policy, conversion and FMA fixtures. Run `sensor_min_max`,
  `vector_distance`, `temperature_convert` and `fibonacci_golden` unchanged
  under **SAMPLE-CORPUS**.

- **PSI-TARGET-FAMILIES.** (new-scope) Target-scoped machine declarations
  survive every Psi stage as one family with bodies keyed by target, including
  unrealized and unreachable bodies, per
  [multi-target compilation](wiki/spec/build/configuration.md#multi-target-compilation).
  - `TypedTrees::target_machine_families` records member handles;
    `omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/symbols/target_siblings.rs`
    binds same-target calls.
  - Exercise filesystem `read_dir_count`, `read_dir_nth`, `remove_dir_all`,
    `open_with` and `lock` from a consumer package once its portable binding
    is supplied by **FILESYSTEM-PLATFORM-SERVICES** through the selected
    target provider's retained raw-family closure.
  - The common lowering route retains runtime bodies and opaque contracts,
    including unreachable members and uncatalogued tags, in Terminal family
    rows; inspection exposes tags and exact callable ids. Semantic conformance
    queries expose every member's obligations, including inherited progress;
    selected provider derivation owns its realization scope. Product pruning
    preserves those roots. Typed provider-default programs have an explicit
    declaration purpose; final Build admission retains every target's effects.
    Entry and private-callback realization share one executable scope for
    bodies, boundary contracts, selected candidates and call occurrences;
    canonical Terminal storage stays complete. Its concrete provider catalog
    includes foreign checked members and their uncalled satisfied requirements;
    execution matches candidate IDs present in the selected deployment scope.
  - Direct portable calls, closed generic attachments, same-target sibling
    calls and family entries now dispatch in native and Terminal execution
    through the complete catalog. The independent head carries all members'
    premises and conservative effects/crashes, not their private result facts.
    Exact member selection shares ordinary internal-call and current receipt
    custody, including claims reseated into owned fields; native custody joins
    the complete selected call graph rather than its pointer-only mirror.
    It supplies no provider grant or boundary issuance.
    Nested machine/runtime operands must not enter an
    applied view or executable cache before their complete admission. Preserve
    the explicit refusal of unjoined evidence member references in signature
    payloads until their namespace and member-local telescope are represented;
    generic specification-call denotation is a separate prerequisite.
    Preserve `generics/contextual_static_machine_captured_operand_return_exit`
    and `generics/contextual_static_machine_returned_view_local_exit` through
    both engines, with returned-item loan and local-escape rejection controls.
    The elided consumer variant must still reject its two candidate inputs.
    Preserve the explicit/universal lifetime, captured
    operand, two-source result and elided-result callback fixtures through both
    engines. Fresh/captured identity (including unused lifetime operands,
    phantom and reference-descendant regions), complete tuples and exact result
    sources remain independent checking obligations; executable carrier sharing
    grants none. Preserve
    `traits/conformance_unused_lifetime_scope_exit` through both engines;
    undeclared call/header regions and foreign-scope unused operands must reject.
    Preserve `generics/contextual_static_machine_conformance_application_exit` and
    `generics/contextual_static_machine_evidence_scopes_exit` through both
    engines; forwarded evidence must guarantee the exact demanded application,
    independent selected maps must not share a symbol-only cache entry, and
    incomplete or excessive tuples and stronger effects must reject. Preserve
    `generics/contextual_static_machine_open_const_exit` and
    `generics/contextual_static_machine_const_scopes_exit` through both engines;
    open const payloads must not become closed cache operands, lose caller scope,
    discard unused binders or evade declared properties. Preserve
    `traits/applied_nominal_callback_local_types_exit` and
    `generics/contextual_static_machine_applications_exit`, plus
    `traits/applied_nominal_callback_closed_local_types_exit` and
    `generics/static_callable_local_type_argument_exit` through both engines;
    uncalled consumers with wrong values, wrong callback carriers or missing
    unused local type arguments must reject. Preserve
    `targets/runtime_family_applied_nominal_visitor_exit` and
    `traits/applied_nominal_callback_scopes_exit` through both engines. Keep
    `targets/runtime_family_callback_requirement_conjunction_exit` and
    `targets/runtime_family_generic_callback_forwarding_exit` running with
    their non-first-member precondition and effect-ceiling rejection controls.
    Preserve `targets/runtime_family_callback_guarantee_conjunction_exit`
    through both engines and its first-member-only guarantee rejection control;
    each member owns its exact applied demand, independently of the selected
    executable's private telescope.
    Keep `targets/runtime_family_generic_nominal_callback_conjunction_exit`,
    `targets/runtime_family_nested_nominal_callback_conjunction_exit` and
    `targets/runtime_family_deep_nominal_callback_variance_exit` running;
    wrong applied authorities and alternating nested input variance must reject
    through ordinary callable admission, not carrier comparison.
    Every ordinary and attached entry/result producer uses the complete
    family catalog. Keep
    `targets/runtime_family_nested_projected_results_exit`,
    `targets/runtime_family_composed_owned_results_exit`
    and `targets/runtime_family_current_completion_paths_exit` running through
    both engines (`tools/corpus_gate.py --both
    --filter <fixture>`).
    Preserve the whole-interface target defaults, named maps sharing bodies and
    explicit Build override fixtures
    `targets/runtime_target_{default_provider_rows,named_provider_defaults,provider_build_override}_exit`:
    receiving installation joins complete selected plans to exact canonical
    candidates, including uncalled requirements, without source or another
    Build evaluation. Unrelated providers introduce no demand. Provider-default
    declarations remain distinct from runtime invocation dependencies.
    Externalize these in-process proposals and every target's entry backing in
    the source-free Build companion. Preserve exact choosing-declaration custody
    through native installation and physical call origins; colliding native
    selections must refuse, not merge owners. Preserve
    `targets/portable_family_entry_installations_exit` (four selected outcomes
    from one unchanged Terminal artifact) and
    `targets/selected_family_exclusion_closure_exit` through both engines;
    selected traps and actual service invocations must still reject exclusions.
    Distinct tagged native realizations with their actual ABI/entry backing
    remain to be demonstrated from that same artifact and companion. Preserve
    `targets/runtime_target_inherited_named_provider_defaults_exit` through
    both engines and its unchanged-artifact macOS/Windows outcomes, with the
    competing authored-map, phantom-argument and erased-region rejection controls.
    Preserve `targets/runtime_target_inherited_provider_roster_exit` through
    both engines and its unchanged-artifact macOS/Windows outcomes, including
    uncalled finite coordinates. Complete tuple identity and the original
    choosing declaration must survive receiving installation; forbidden integer
    and Boolean tuples, crossed tuples and unconstrained forwarding must reject.
    Preserve `providers/inherited_applied_binding_{dispatch,forwarded_value,finite_family,borrowed_result}_exit`
    through both engines, including unused requirements, borrowed forwarding,
    outer type arguments and local const tuples. Wrong receiver applications
    and unused property/const obligations must reject. Invocation-lifetime
    results retain their exact input loan and access; shared-to-mutable calls,
    unrelated returned sources and live-source writes must reject.
    Keep `providers/inherited_boundary_field_postcondition_exit` and
    `recast/boundary_postcondition_{backing_scalar,disjoint_aliased_write}_exit`
    through both engines when extending region-bearing calls: public guarantees
    follow original storage, never private guarantees or view field spelling.
    Aliased and nested-call writes must retire overlapping bounds; signed
    reinterpretation cannot transfer an unsigned bound. Preserve
    `recast/reference_view_{helper_backing_write,shared_call_backing,shared_primitive_neighbor}_exit`
    as same-backing and mixed-argument controls.
    Preserve `build/explicit_lifetime_entry_context_exit` through both engines:
    ordinary entry contexts use their retained closed data applications, without
    bypassing mutable access or ZII eligibility. This storage correspondence is
    not a portable region identity. Preserve
    `providers/applied_entry_nested_binding_context_exit` through both engines
    and its wrong-receiver rejection control: applied data storage must retain
    closed trait-contract operands, and direct/nested field discovery must carry
    the enclosing data applications into selection and establishment.
    Preserve `providers/applied_binding_field_telescopes_exit` through both
    engines: two applications of the same field declaration beneath one entry,
    including nested wrappers, must retain distinct complete operands. Formal
    type/const telescopes are checked without admitting runtime value operands;
    a differently applied selected provider must still reject. Executable
    requirement preparation checks closure after complete scope substitution.
    Preserve `providers/occurrence_region_binding_{exit,forwarding_exit,projected_type_exit}`
    through both engines: original field applications, borrowed locals and helper
    telescopes retain complete receiving region tuples. Conflicting direct/saved
    inputs and fabricated borrowed carriers must reject. Build selects the
    provider declaration, not a runtime caller region. Preserve
    `traits/default_row_captured_invocation_regions_exit` through both engines:
    owned rows capture their map's regions without replacing their fresh member
    telescope, including unused binders; external selected rows capture no map
    scope. Preserve `traits/default_row_shadowed_captured_operand_exit` through
    both engines: same-spelled captured phantom and fresh member binders remain
    separate through default-body locals and generic data substitution.
  - **PROVIDER-SELECTION-AFTER-TERMINAL** owns Omega's selection from the same
    artifact; independent ownership moves can land first.

  Acceptance: keep `fail/targets/sibling_body_type_error_rejected` and
  `pass/targets/target_machine_gating_exit` (70). Preserve distinct results and
  same-target sibling calls across installations of one Terminal artifact.
  Invalid unrealized bodies and duplicate family
  members reject; a missing called body fails only the affected realization.

- **BUILD-EVALUATES-ONCE.** (new-scope) `build.omg` evaluates once per
  compilation, observes no realized target, and produces checked rows for every
  target: providers, entry, subsystem, application intent, opaque
  representation, x86 scalar FMA provider, grants, behavior exclusions and wire
  demands.
  - `BuildConfig` is still largely global. Finish authored rows and retain/check
    unrequested rows. Direct `CompileRequest` Product checking and source-custody
    sealing run once; package batches still need the same shared checked product.
    Separate target-invariant opaque-receipt derivation and pending evaluation
    from genuinely target-keyed inputs. Reuse requires exact semantic inputs
    and preserves each occurrence's authority and custody; equal result values
    alone do not authorize reuse. Provider-folded constants obey the existing
    same-provider rule in multi-target configuration. This is implementation
    work, not an owner decision on whether sharing is allowed.
  - The duplicate preliminary whole-program check is gone for equal inputs:
    sibling occurrence rows of one invocation now consult a shared
    `sibling_build_checks` pool before `prepare_checked_build`, and an
    exact-input match takes the earlier retained judgment through
    `reuse_retained_check` — each activation still re-admits its own
    declaration authority, provider settlement and grants, so
    [admission before execution](wiki/spec/build/execution.md#admission-before-execution)
    is preserved (freeze authored selections and admit the exact Build entry
    before effects; generated source and product checks finish after).
    Rows whose canonical `typed` legitimately differs — projected after an
    intervening mutation — still check their own program.
  - A `CompileRequest` now executes Build and completes authored/generated
    typing once before target settlement. Package batch compilation borrows
    one invocation across targets — `compile_accepted_project` adopts a
    caller-owned `PackageBuildSession` instead of opening a fresh activation
    per target, so sponsors, staging and the pure-Build store span the batch.
    Keep separate Build/Product purposes, current-pass logical charges and
    invocation-owned staging; captured outputs publish without repeating effects.

  Acceptance: a two-target request counts one Build activation and one final
  product check (timing rows alone are insufficient); std's rows for every
  target come from one evaluation — satisfied by the shared pure-Build store:
  executions and settled checks dedupe across targets, and across review
  rounds through the session-owned store. Malformed unrequested rows reject;
  per-target missing-row outcomes stay per target. Nested generated-dependency
  and dual-purpose custody, pre-effect rejection and snapshot/required-output
  settlement keep their behavior, covered by
  tests/omega/pass/build/nested_generated_dependency_dual_purpose and
  corpus fixtures scoped_build_stops_before_unaccepted_effect,
  artifact_only_unsealed_output_refused and
  captured_source_generated_constant.

- **PROVIDER-SELECTION-AFTER-TERMINAL.** (new-scope) One target-neutral Psi
  product; per-target provider installation consumes Terminal Psi plus that
  target's Build rows in Omega stage 00. Follow
  [multi-target compilation](wiki/spec/build/configuration.md#multi-target-compilation)
  and the [pipeline](omega-rust/pipeline.md#one-driver-one-pass-per-target-realization).
  - Boundary applications and comparisons now settle from canonical demands,
    checked body candidates and selected Build rows; no checked-application scope
    crosses publication. Build exclusions retain selected service identities,
    and native proposal construction no longer takes checked trees. Entry
    receipts and signatures use portable declaration identities; source entry
    selection and Extent capture remain producer-side. Exclusion admission now
    borrows unoptimized Terminal semantics before optimization; exact service
    owners travel in declarations, without a retained lowering clone or source
    lookup. Callback facts carry source entry resources, not target ABI hashes.
    Product source now checks once before provider settlement; selected
    providers cannot rewrite its contracts or fold target constants into it.
    Remaining frontier: converting execution_settlement's Omega-00
    installation joins to Terminal Psi plus that target's Build rows.
  - `omega-rust/omega/compilation/driver/src/execution_settlement.rs` owns the
    Omega-00 joins outside the Psi facade: calling plans, program-entry
    selection, provider settlement, selected legacy generic-operator and
    boundary-family requests, selected-dispatch execution, callback
    materialization, task activation, component progress, x86 FMA associations
    and fused entry establishments. Logical Binding shapes now remain independent
    of provider mode; entry establishment still joins source schema and selected
    provenance there. Settlement now splits at provider selection: the
    canonical terminal artifact is produced from the selection custody before
    any target's selected-execution install runs. The install-phase joins
    (selected dispatch, task activation, component progress, intrinsic review,
    fused entry establishments) still read the typed program and checked
    records: convert their inputs to Terminal Psi plus that target's Build
    rows, keeping source-semantic contracts and all authored candidates in
    Psi. Every family body and Build row is checked even when `--target`
    excludes it.
  - Product constants requiring selected-provider evaluation now close under
    each target: Psi rosters each deferred position as a retained constant
    recipe, checks the shared program against the symbolic node and records
    index, window and membership comparisons as obligations; after settlement
    Omega resumes the continuation on the authored source under the selected
    rows, discharges the obligations and hands a closed program copy to
    Terminal production and installation (`f3dde6cf4b4`). Remaining: a
    recipe-extent array whose element owes cleanup or qualification rows, a
    const-generic argument other than a direct call, and the twins' ~250 s
    closure re-check of the recipe closure.
  - Completion-role review now fixes a provisional exact source declaration
    before checking and rejoins every proposal to its selected external supply
    afterward. Ordinary compilation still requires the accepted schema/plan
    binding; provider selection never rewrites checked source. Discovery uses
    the complete authored external-supply roster, independent of target choice.
    Replace this nomination with explicit early consumer designations for mixed
    returning/external candidates: an unused external candidate currently makes
    a selected returning implementation fail closed. Keep no-role ordinary
    returning services and stale/foreign/unmatched binding controls.
  - `BoundaryMachineDeclaration` scalar contract agreement is landed both
    directions: codec module admission checks every declared row over the
    declaration telescope, and each provider candidate covers the
    substituted declared obligations conjunct-wise — the authored
    `source_checked_requires` roster survives into `MachineContract` for
    the requires direction, and authored `ensures result in Domain`
    clauses publish the result qualifications that declaration and call
    sites replay (`capabilities/runtime_qualified_record_return_exit`
    runs clean). Conversion work must not weaken candidate contract
    admission.

  Acceptance (with **BUILD-EVALUATES-ONCE** and **OPERATOR-BOUNDARY-CALLS**):
  a two-target request runs Psi 00–07 once and produces one Terminal artifact;
  each target's native output equals its single-target output byte-for-byte
  across the native corpus. Keep `arithmetic/const_fold_unsigned_landed_ops_exit`,
  `expressions/match_float_patterns` and
  `providers/checked_boundary_requirement_dispatch_exit` (equal Terminal bytes
  there do not prove checking ran once). Cover differing runtime providers,
  agreeing/disagreeing const selections, errors in an unrealized body, and a
  missing realization row without suppressing the other target. Keep
  `{fail,pass}/targets/single_target_internal_*`, root-local and through
  `tests/omega/packages/paradigm-internals`: a missing called body rejects only
  the affected realization, naming caller, callee and target, while a caller
  scoped to its callee's own unrealized target executes.

- **ONE-DRIVER-PER-STAGE.** (new-scope) `compiler.rs` runs Psi 00–07 once,
  then Omega 00–09 and emission per realization, per the
  [pipeline](omega-rust/pipeline.md). Closes after the items above.
  - `compile` completes source, Build and Product checking once; realizations
    borrow the same checked storage; matching Psi-optimization requests share
    Terminal production and storage across distinct Build entry roots, with
    separate target admissions and exact per-application execution entries.
    Callback bodies share that Terminal program and native provider context;
    ABI adapters retain their selected machine and parameter-origin mappings.
    Psi owns resolver/typing and generated-source continuation; the product
    driver, Build evaluator and product carriers no longer import stages
    02/03/03b/04. Psi owns preparation, retained-check matching, checking and
    Product pruning; shared projections/operator queries have semantic owners.
    Source assembly, Build role projection and dependency editing now use Psi's
    parsing facade; no package consumer imports the lexer/parser stages.
    Psi establishes source completion roles before retained-check matching;
    provider cycle checks consume its retained invocation facts. Psi owns the
    static/executed provider-selection decoder and returns Product defaults
    before pruning, shared across targets outside Build admission. Join the
    remaining source/package assembly contracts to this facade.
    Package batches now share checked/pruned and Terminal storage through
    the compiler invocation; package-owned frontend/Pure Build/proof caches
    and `compile_checked_reusing_builds` are removed. Accepted target plans now
    enter together; matching post-Build root continuations complete source once,
    with separate target admission and receipts. Dependency-first rounds submit
    demanded independent products together for source completion. Still converge
    source preparation across the complete closure and standalone/session routes;
    rounds are not a single source/Build prefix. Preserve fresh occurrence
    admission and charges without copying mutable typing state per realization.
  - Package review uses the compiler session's checked stop, with no
    `compile_to_checked` wrapper. Unify this session and `CompileRequest`
    into the same source-to-product request. Source preparation
    supplies stage 00; build-time evaluation, dynamic binding, const evaluation
    and semantic pruning belong inside stages 02–04. Carry discovery, grants,
    sponsors and snapshots in the owning stage inputs/outputs.
  - Check, native, run and inspection now request finished products; package
    operations no longer return `CheckedCompilation` or resume the pipeline.
    Root Product permission consent is input, compared with fresh declarations
    inside compilation before native production. The compiler owns generated
    joins, Build continuations and independent-component publication.
    Standalone/session products now share selection → Terminal → installation;
    the installed-checkpoint completion route is removed. Review descriptions
    borrow the same production facts without a second producer mode.
    Remaining: converge session and standalone source preparation, and move
    source-aware provider selection itself behind Terminal as required above.
  - Keep each stage's independent representation and transformation crate,
    single-target command behavior, Build-effect custody and per-target
    publication.

  Acceptance: the compiler owns only composition, reports and publication; a
  two-target native compile shows one Psi chain and one Terminal identity, then
  one Omega chain per target, with native results and isolated realization
  failures as in **PROVIDER-SELECTION-AFTER-TERMINAL**. Prefix consumers stop at
  their checkpoint; stage counters distinguish execution from reused reports.

- **MACOS-NATIVE-LOST-SUCCESSES.** (new-scope)
  Restore the macOS ARM64 corpus rows that met their authored check, build or
  runtime oracle and no longer do. Every such check row carries a
  `regressed-from:` token, so `python3 tools/corpus_gate.py
  --list-pinned-regressions` is the roster: 11 check rows at `261d656cb29`,
  no cluster larger than the three omissions one lane holds. Measure with
  `python3 tools/corpus_gate.py` and `--native`, then route by cause, not by
  the revision that recorded a row.

  - Owned elsewhere: `interrupt_table_canary` and `secondary_processor_canary`
    select providers but lack an executable intrinsic, which
    **CHECKED-ASM-LABEL-CONTRACTS** owns;
    `fail/calls/struct_constant_requires_argument_rejected` is
    **STRUCT-CONSTANT-REQUIRES-REFUTATION-DRIFT**; the three stage 04/05
    omissions `pass/concurrency/runtime_retained_reference_boundary_exit`,
    `pass/text/runtime_mutable_string_parameter_concat_write_line` and
    `pass/ownership/runtime_owned_carried_loan_extract_transform_exit` have a
    live lane.
  - Unowned singletons: `pass/arithmetic/closed_call_transition_return_proves_exact_sum_exit`
    (implicit store conversion),
    `pass/blockexec/blocking_executor_custody_claims_compile` (attached Unit
    closure missing a checked plan), `pass/capabilities/windows_provides_import_exit`
    (external evaluation not build-time admissible),
    `pass/generics/open_computed_quantity_result` (const parameter
    materialization), `pass/modules/same_name_requirement_calls_exit`
    (Unit services with empty or duplicate canonical identities).
  - Cross-host control: Linux records still reject
    `pass/text/local_bounded_text_case_result_loan_exit` and mark
    `local_bounded_text_scalar_loan_exit` incomplete, while macOS recorded
    exit 70. A Linux host must remeasure them; never copy another host's
    success. **NATIVE-GOLDEN-LINUX-BOUND-UNMEASURED** owns that recording.

  Acceptance: every remaining loss reaches its authored check/build/runtime
  oracle and the corresponding host record is updated from that observation.
  Preserve negative checks and source oracles. Already recovered rows are
  regression controls, not new repair assignments.
- **NATIVE-GOLDEN-LINUX-BOUND-UNMEASURED.**
  (split-of:NATIVE-GOLDEN-CANNOT-RECORD-A-REGRESSION)
  Record `tests/omega/corpus_native_linux_x86_64.txt` on a Linux host at
  one identified revision using each fixture's selected realization target.
  Run `python3 tools/corpus_scoreboard.py --cross-golden` and investigate
  differences against same-revision, same-target observations. Different
  revisions, timeouts and stale rows are not compiler portability failures.

  Acceptance: Linux outcomes are observed, not copied from macOS; remaining
  differences have exact reproductions and owning capability tasks. Repair
  actual regressions under those owners rather than changing source or
  expected results to make the report empty.


## Lowering simplification

Owner ruling 2026-10-04: stage 05 is cut down to a mechanical lowering of
stage 04's plans. Three things go:
- **Re-checking:** Stage 05 re-reads the typed program and re-validates the
  plan it is handed. That is the in-process self-validation the
  [verification placement](omega-rust/pipeline.md#verification-placement) removes.
- **Parallel routes:** Bodies are lowered by 13 leaf routes, chosen in
  `omega-rust/psi/compilation/pipeline/05_checked-trees-to-lowered-psi/src/machine_lowering/machine_dispatch.rs`,
  and each route re-implements calls, stores and transitions.
- **Shape-specific plans and rungs** where a general operation would work.

Delete redundant checks, establish shared operations and retire superseded
routes in dependency order. New capability work
(**STATE-LOCAL-VALUE-FRONTIER** and others) uses shared operations,
never a new per-route case. Land coherent cuts that delete their superseded
code and preserve check/native behavior for the named customer slices.

- **OUTCOME-SPECIFIC-CALL-EVIDENCE.** (split-of:RETURN-ROSTERS-RETIRED)
  Selection runs end to end from an input-free callee (`1c794804e9f`,
  `run/calls/outcome_specific_selected_guarantee_exit` and `_variants_exit`)
  and from a callee that takes scalar inputs and returns from several
  payloadless case exits (`run/calls/outcome_specific_selected_guarantee_inputs_exit`,
  exit 0 natively and interpreted on macOS arm64; the control
  `fail/calls/outcome_specific_witness_on_nonmatching_exit` pins the checker's
  refusal of a witness assigned on a nonmatching exit). A selected
  proposition that names the callee's `result` carries the row's result
  substitution and the call result as its validity dependency
  (`run/calls/outcome_specific_selected_guarantee_result_exit`, exit 0 in
  both engines), and one that names a scalar input carries an argument
  substitution to the call's argument
  (`run/calls/outcome_specific_selected_guarantee_input_exit`, exit 0 in both
  engines). Frontier: a callee with structural inputs, entry claims, other
  contract rows or crash routes, and a proposition argument or dependency
  rooted at anything but the callee's `result` or a bare scalar parameter
  (a projection, a computed value) are refused by name ("... with structural
  inputs or other contracts is unimplemented", "... names neither the
  callee's result nor a scalar input is unimplemented", "... beyond the call
  result are unimplemented"). Coordinate operator returns with
  **OPERATOR-BOUNDARY-CALLS**.

## Checker simplification

Owner ruling 2026-10-04: remove the checker's re-checking, duplicate analyses,
shape gates and review/certificate support. Owners are stage 04
(`omega-rust/psi/compilation/pipeline/04_typed-trees-to-checked-trees/src`)
and the `validation` crate (`omega-rust/psi/compilation/checking`). The rulings
in [PCC removal](#pcc-removal) and
[Lowering simplification](#lowering-simplification) apply here. Related items
already filed elsewhere: **TYPED-FROZEN-BEFORE-CHECKING**.

Each coherent repair deletes what it replaces and keeps every
check-corpus verdict and diagnostic family unchanged unless the item says
otherwise. Paths below are relative to the stage 04 `src/` (written `omega-rust/psi/compilation/pipeline/04_typed-trees-to-checked-trees/src/`) or
the validation `src/` (written `omega-rust/psi/compilation/checking/src/`) unless spelled in full.

- **ONE-INTEGER-BOUNDS-ENGINE.** (new-scope) At least nine separate engines
  answer "what range can this integer take":
  - stage 04: `omega-rust/psi/compilation/pipeline/04_typed-trees-to-checked-trees/src/checks/ranges/facts` (`RangeFacts`),
    `omega-rust/psi/compilation/pipeline/04_typed-trees-to-checked-trees/src/checks/contracts/intervals.rs`, `omega-rust/psi/compilation/pipeline/04_typed-trees-to-checked-trees/src/checks/contracts/evaluator`,
    `omega-rust/psi/compilation/pipeline/04_typed-trees-to-checked-trees/src/checks/termination/ranking`
    ranges (`Bounds`), `omega-rust/psi/compilation/pipeline/04_typed-trees-to-checked-trees/src/checks/borrows/overlap/indexes.rs`
    (`NormalizedBound`), `omega-rust/psi/compilation/pipeline/04_typed-trees-to-checked-trees/src/checks/ranges` `MergedBound` and the loop
    invariant `Bounds`;
  - validation: `omega-rust/psi/compilation/checking/src/proof_contracts/arithmetic_domains/interval.rs`,
    `omega-rust/psi/compilation/checking/src/proof_contracts/contract_entailment/arithmetic_judgment.rs`,
    `omega-rust/psi/compilation/checking/src/proof_contracts/arithmetic_domains/invariant_bounds.rs`,
    `omega-rust/psi/compilation/checking/src/proof_contracts/arithmetic_domains/integer_ranges.rs`,
    `omega-rust/psi/compilation/checking/src/value_custody/struct_literals/construction_bounds.rs`;
  - `omega-rust/psi/compilation/proof/src/checker/integer_ranges.rs`.
  Selected scalar plans (`CheckedScalarExpression`) are already evaluated by
  validation's `arithmetic_domains/selected_scalar_intervals.rs` over a
  client-owned `SelectedScalarIntervalSource`; stage 04's
  `values/place_intervals.rs` is that client and computes nothing.

  Keep one interval domain and one premise-chaining oracle in `validation`
  (`arithmetic_domains`), and make every listed site a client. Validation
  publishes each walked integer expression's interval
  (`validation::ExpressionInterval`). The stage 04 index checker consumes it
  for fixed-length reads, and the call checks consume it for
  parameter-versus-literal requirements and call-argument domain membership.
  `checks/contracts/intervals.rs` keeps only the guard-label reader the
  label routes use. Name-keyed proofs and unknown-length slice bounds remain
  private. Float intervals
  (`omega-rust/psi/compilation/checking/src/proof_contracts/arithmetic_domains/float_arithmetic.rs`) stay
  separate. This is large: land it per client, each commit deleting that
  client's private engine. Acceptance: no private integer interval type
  remains outside `arithmetic_domains`; corpus `--filter ranges,arithmetic/,
  collections/,slices/,termination/,contracts` unchanged. Preserve signed-minimum
  divisor magnitudes, both remainder extrema and the dividend hull including
  zero at every migrated client, even without a finite divisor endpoint;
  `arithmetic/{remainder_signed_minimum_underbound_rejected,runtime_signed_minimum_remainder_exit}`
  constrain this interval meaning independently of divisor definedness.
  `arithmetic/{runtime_signed_minimum_remainder_postcondition_exit,remainder_postcondition_signed_minimum_underbound_rejected}`
  constrain the same bounds at immutable postcondition consumers.
  `arithmetic/{runtime_bounded_dividend_remainder_exit,remainder_dividend_range_excludes_zero_rejected,zero_dividend_remainder_requires_nonzero}`
  constrain dividend precision without assuming a positive remainder floor.
  Preserve full-width unsigned quotient endpoints from their source carrier,
  retaining exact unsigned snapshots; `arithmetic/runtime_full_width_unsigned_quotient_exit`
  and `arithmetic/{unsigned_quotient_underbound_rejected,unsigned_quotient_operand_overflow_rejected,unsigned_quotient_operand_landing_rejected,zero_dividend_quotient_requires_nonzero}`
  constrain result bounds independently of operand formation.
  Keep immutable unsigned AND/right-shift queries on the shared carrier-aware
  bitwise provider and selected exact right shifts on its integer value kernel,
  retaining the count's own carrier; `arithmetic/runtime_immutable_unsigned_bitwise_bounds_exit`
  and `arithmetic/immutable_unsigned_{shift_underbound,mask_false_floor,shift_invalid_count,shift_operand_overflow}_rejected`
  constrain full-width shift bounds independently of formation and false bounds.
  Keep both selected Exact shift directions on the shared admitted-endpoint
  range kernel, preserving left-shift value representability separately from
  count validity; `arithmetic/runtime_exact_left_shift_bounds_exit` and
  `arithmetic/exact_left_shift_{false_ceiling,false_floor,invalid_count,value_overflow}_rejected`
  constrain wide unsigned and signed cross-zero bounds without clipping overflow.
  Preserve zero-product bounds for unbounded integer operands without erasing
  operand formation obligations; `arithmetic/runtime_unbounded_zero_product_exit`
  and `arithmetic/zero_product_operand_{overflow,division}_rejected` constrain
  that invariant at every migrated client.

## Data-flow defects

Each item removes one verified copy, rescan or repeated validation in the
compiler's flow from source bytes to image bytes. Re-read the cited symbol
before starting; paths drift. Each coherent repair lands with the cited corpus
slice unchanged (check plus `--native` where the code is backend-visible).
Re-checking stays allowed at trust boundaries; these items remove repetition
of a check the same flow already performed, not the check itself.

- **TYPED-FROZEN-BEFORE-CHECKING.** (new-scope) Complete qualification of the
  frozen typed-to-checked boundary. Typed elaboration now belongs to
  `omega-rust/psi/compilation/pipeline/03b_typed-trees-to-typed-trees/src/preparation.rs`;
  stage 04 consumes `FrozenTypedTrees` and publishes authored selections and
  specialization commitments into `CheckFacts`. Source queries remain with
  independent validation/proof owners; no checking-time mutation or typed
  commitment fallback is permitted.

  Remaining: qualify exact erased applications, clone correspondence and
  lexical owners through folding and substitution against the full check
  corpus and focused native cohort. Attribute remaining affected failures
  against the recovered series' clean base; historical golden disagreements
  are not introduced regressions. Preserve the authored exact-selection
  refusal in `checked_boundary_operator_const_application_unselected`.
  Its portable recipe dependency remains with **PROVIDER-SELECTION-AFTER-TERMINAL**;
  target-selected folds must not become checking inputs.
  Keep portable callback refinement with
  **PSI-TARGET-FAMILIES**. Full qualification must retain the portable callback's
  first-member-only guarantee refusal and the inverted-distance
  refusal, fixed bounded-distance upper coordinates across state and call
  arrivals, crossed entry cursors and renamed/permuted call-component binders.
  Entry-bearing native refusals and
  sponsored Build controls must retain their intended diagnostics without
  replaying the accepted source compilation.
  Revalidate historical reds before full qualification;
  fixed-producer evidence is not latest-main acceptance. Retain borrowed
  temporary projection execution and its missing sibling-disposition refusal
  in full qualification; each loan keeps the complete producer and selected
  path through cleanup. Interpreter qualification of
  `runtime_foreign_encoded_reference_exit` needs the exact memory-import adapters
  under **TERMINAL-RUNTIME-DRIVERS**; native execution alone is not agreement.
  Keep closed Console catalog identity and dependency entry-binding
  approval with **PIPELINE-ARCHITECTURE-ENFORCEMENT**; the public
  `record_array_field_access_exit` run still needs that approval. Include owned
  projected locals, constructor captures and fixed-array slot replacements
  through calls; preserve sibling loans and reject callee-local backing escapes.
  Continuation transport must retain both storage root and field path.
  Keep
  computed indices, renamed/permuted binders and independent result regions in
  qualification; private implementation summaries cannot supply missing abstract
  termination promises. Bounded operator, algebra-law and computed-index
  execution does not replace full-corpus acceptance.
  Do not repin these refusals or hide accepted controls with a later missing-entry
  error. Include
  `runtime_computed_backing_reference_copy_exit`, `runtime_boundary_range_view_exit`
  and their escape/conflicting-write controls; unchosen backing homes must not
  acquire custody and nested requirements must not select a provider in Psi.
  Stored references retain finite closed
  identity edges through Terminal and native catalogs. Identity-only references
  no longer grow the pre-resolution declaration catalog. Closed case operands
  consume prepared member types without
  checking-side substitution;
  borrowed array payload observations retain their indexed scalar endpoints.
  Qualify case-qualified descriptor storage without granting permissions to
  inactive payloads; retain the growing `Chain`, nested constructors and mixed-case
  controls in
  `runtime_case_qualified_reference_storage_exit`, including independent custody
  auditing, named element references from local byte slices and empty/offset
  stored byte views projected in helpers or forwarded through call results and
  local bindings. Descriptor
  inventories distinguish known slots from opaque referents; unavailable
  subtrees must not become storage origins or complete return/effect summaries.
  Include `runtime_linear_constructor_instances_exit` and effectful generic
  cleanup in independent native auditing; retain their exit-zero oracles.
  Carry these compositions through execution, without restoring native
  permission replay. Include qualified descriptor replacement and detached-copy
  content observations; local observations cannot acquire entry provenance.
  Include `runtime_qualified_byte_array_storage_exit` and its element-domain,
  replacement and mutable-domain-loss controls in full qualification. Array
  elements retain their own memberships through construction, copies, returned
  storage and loans; record/case fields cannot substitute for that evidence.
  Migrate remaining qualified-byte
  storage fixtures' variable-fill `[u8; N] in Utf8` assumptions under
  [qualified fixed-array storage](wiki/spec/language/domains.md): qualification
  cannot select a different carrier or live length. Keep exact-array and explicit
  variable-fill storage distinct while sharing their ordinary loan transport.
  Runtime-selected collection prefixes
  still need exact cleanup custody; missing death evidence must not imply
  vacancy. Preserve source copy/move classification and the containing linear
  claim while disposing the residual partition. Element restrictions and extent
  remain fixed before checking, without a literal-specific store path.
  Include prepared lifetime-bearing field
  views, independent nested regions, repeated generic heads, authored callable
  templates, symbolic view-free arrays, inherited const arguments and complete
  returned-loan frontiers. Preserve current storage occurrences through
  repeated replacement and by-value forwarding, inactive cases, anonymous
  operands and the complete-versus-unavailable distinction in that qualification.
  Keep
  `omega run tests/omega/pass/generics/runtime_inherited_composite_requirement_exit/main.omg`
  and conflicting-mutation/distinct-region controls in that qualification;
  the bounded operator, algebra-law and returned-loan cohort does not replace it.
  Attribute remaining moves before claiming qualification; do not repin
  refusals. Include the std-dependent setter and const-container gates under
  **PIPELINE-ARCHITECTURE-ENFORCEMENT** in that qualification.
  Keep the const-container gate's independently named receiver and whole-window
  variation in lexical-owner qualification. Include
  `runtime_applied_default_domain_setter_exit` and its unrelated-bound,
  stale-mutable-requirement and wrapping-arithmetic controls in predicate-scope
  qualification. Schematic bodies owe their source predicates before runtime
  closure; current value facts must independently justify restoration.
  Tuple bounds use the actual binding declaration, not an authored trait
  requirement. Only live private-graph roots own admission work. Universal
  template commitments use the retained authored graph, never a concrete
  instance's narrower effects. Runtime selected-provider choices are not a
  preparation input; source checks every candidate before Omega selection.
  Checking indexes, memo and recursion state stay invocation-owned and explicit;
  declaration relationships stay typed-owned. Do not port runtime borrow
  checking into Omega.

  Acceptance: the immutable-boundary source guard, full check corpus and focused
  native cohort are unchanged, with no writes to `CheckedTrees.typed` after
  construction and no preparation fallback in stage 04. Checking/stage-04
  production paths contain no `thread_local!` or `RefCell` working state
  (source-scanning guard). Nested checking and partitioned workers use explicit
  disjoint contexts over shared immutable arenas.

- **IMAGE-SELF-REPLAY-DELETED.** (new-scope) Direct and dynamic ELF image
  emission now each build their final image once. Only retained artifacts
  read back through `DynamicElfNativeArtifact::from_replayed_parts` and bytes
  admitted by another writer run (`emit_admitted_dynamic_elf_image`) rebuild
  and compare it. The `linux_x86_64` build leg is unchanged. Acceptance still
  owed: the `linux_x86_64` native corpus leg on a Linux host, with execution,
  is unchanged.

## PCC removal

Owner ruling: proof-carrying code is a failed experiment and is removed. PCC
means independently re-verifying a Terminal artifact and a shipped proof
bundle at a receiver that does not trust the compiler. Until the items
below land, do no PCC work inside any other item: skip its proof-bundle,
certificate-transport, independent-replay or receiver-verification clauses.

These stay:
- the mathematics world: authored theorems, proof terms, the inductive
  profile, quotients, and the `proof-admission` kernel that checks user
  mathematics (`omega-rust/psi/compilation/proof/admission/src/` `mathematical_core/`,
  `proof/`, `integer_rules/`, `predicate_denotation/`);
- contract and refinement reasoning in Psi checking
  (`omega-rust/psi/compilation/checking/src/proof_contracts/`,
  `omega-rust/psi/compilation/pipeline/04_typed-trees-to-checked-trees/src/proof/`,
  the `proof` crate at `omega-rust/psi/compilation/proof/`);
- `bootstrap/proofs/`;
- the Omega optimizer's use of proven facts, re-sourced as below.

Coordinate dependent producer/consumer removals so no intermediate product
needs deleted evidence. Preserve each named corpus slice except for intended
deletions, which the commit lists.

- **CHECKER-CARRIES-OPERATION-DECISIONS.** (new-scope) Complete checking-time
  discharge of primitive operation conditions and consume those decisions in
  stage 05 and stage 00 source-checked operation facts. Owners: stage 04
  arithmetic/range checking and `CheckFacts`, stage 05 operation emission and
  stage 00 optimization-unit construction.

  Reuse occurrence-bound `ArithmeticDecisions`, `proven_scalar_indexes`,
  `proven_byte_writes` and `decided_call_requires`, carried through
  `formation_decided`/`SourceChecked`. Bounds alone do not establish a byte
  domain, and inferred additions to an arrival withdraw its source-checked mark.
  Call conditions and result facts belong to the selected callable identity,
  not to a builtin-like spelling.
  Remaining:
  - Carry decisions through generated selectors and remaining
    semantic-to-storage casts. Done for byte-region view expansions: every
    generated byte read records the footprint proof as `bound_decided` under
    a declared extent (`expression_preparation/representation_view.rs`).
    Still open: inner element-path runtime selectors
    (`prepare_expression.rs` hardcodes `bound_decided: false` because
    `CheckedRuntimeIndex` carries no authored occurrence handle to join),
    view-range runtime segments (`view_ranges.rs`), record-loan element reads
    (`scalar_lowering/computations/fields.rs`), synthesized widenings and
    generated scalar-graph casts — each still needs exact source-decision
    transport. Join the exact checked occurrence rather than performing
    another syntax walk.
  - Cover representation geometry, capacities
    and remaining bounded stores outside bounded-integer field replacements
    and interval/finite-refined element-view writes, including generated indexed
    stores. Finish the formation audit outside statement-level concrete scalar
    constructor first stores and captured unrestricted record/array child memberships.
    Finish affine aggregate entries, contract-bearing scalar continuations and
    proposition positions; unify
    abstract declaration-contract carrier resolution with the ordinary formation walk.
  - Carry source decisions for invocation-entry, scalar-graph,
    local-refinement and inferred arrival obligations, and conditional cleanup
    prerequisites. Keep call/cleanup contracts and mathematical proofs distinct
    from primitive operation conditions.
    The synthetic initial edge may use the machine's already-established entry
    premises under exact argument substitution; retain that correspondence
    rather than enumerating callers. Reentry and strengthened invariants still
    need their own arrival proofs. Missing occurrence transport is engineering,
    not authority to re-prove the same checked condition on the compile path;
    follow [verification placement](omega-rust/pipeline.md#verification-placement).
  - Preserve non-Exact division's nonzero obligation, domain and custody
    checks, and intermediate-write requirements; a final value contract cannot
    authorize opaque intermediate operations. Public layout literals remain
    unsupported independently of this work.
  Regression controls: `arithmetic/runtime_short_circuit_cast_decisions_exit`
  (70), `arithmetic/runtime_qualified_cast_decision_exit` (0),
  `calls/runtime_constructor_child_declared_membership_exit` and
  `domains/runtime_nested_record_membership_transport_exit` (0 in both
  engines; literal, stale-epoch and sibling membership forgeries reject),
  `collections/runtime_array_child_membership_transport_exit` and
  `domains/runtime_constructor_ensured_membership_exit` (0 in both engines;
  returned children keep memberships and `collections/array_child_membership_requires_issuance`
  rejects literal, stale, sibling and lookalike-issuer substitutions),
  `arithmetic/runtime_returned_constructor_operation_exit` and
  `structural/runtime_nested_constructor_payload_exit` (0 in both engines),
  `samples/cli/collections/heat_grid` (45), nested projected stores and
  `collections/runtime_fixed_array_{captured_selector,selector_call}_store_exit`
  (70) and `collections/runtime_{indexed_assignment,element_view}_selector_snapshot_exit`
  (0); retain `collections/runtime_refined_element_view_call_write_exit`,
  `collections/runtime_nested_array_view_call_write_exit` and
  `collections/runtime_nested_record_element_window_store_exit` (0) for
  receiving-field domains, captured selectors and original window backing.
  `collections/runtime_multi_field_element_view_domains_exit` (0) and
  `range/element_view_field_domain_occurrence` constrain full one-sided carrier
  bounds and prevent transferring a write judgment between fields.
  `collections/runtime_composed_owned_element_reads_exit` (0) constrains
  ordered member/selector projections, computed selectors and declared scalar
  fields; `collections/nested_read_index_uses_selected_extent` must reject an
  inner read even when the same operand fits the outer extent.
  `collections/runtime_composed_{element_view,subview}_reads_exit` (0) constrain
  live view bounds, ordered descendant selectors and nonzero window offsets;
  `collections/element_view_descendant_uses_selected_extent` and
  `collections/element_view_read_requires_live_extent` must reject independently.
  `slices/runtime_boolean_alias_guard_transport_exit` (0) constrains selected
  comparison bounds across Boolean aliases, inline descriptor arguments and
  state transfers; `collections/authored_index_guard_bound` must not acquire
  bounds from a user-defined comparison.
  `slices/runtime_inline_mutable_{view,byte_window}_call_write_exit` (0)
  constrain evaluated exclusive descriptors, Unit/scalar call completion and
  original backing; `slices/inline_mutable_call_windows_overlap` and
  `slices/inline_shared_window_mutable_call` must reject independently.
  `arithmetic/runtime_boolean_{alias,field,state}_shift_count_exit` (0) constrain
  conditional arithmetic observations across Boolean stores, copies, polarity
  and state transfers; `arithmetic/boolean_shift_guard_{subject_written,overwritten}`
  must not reuse withdrawn count facts. Exact left shifts still independently
  owe result representability; Saturating never admits an invalid count.
  `arithmetic/runtime_{boolean_guarded,transitive}_{bit_value,product}_exit`
  (0) constrain arithmetic result bounds, including the high unsigned bit,
  signed endpoints and scoped-out intermediaries. Different-subject and
  missing-arrival guards must not gain bounds; non-strict count chains and
  composed `!=` observations must not prove a strict bound or nonzero divisor.
  `calls/runtime_authored_numeric_{names,guard_result}_exit` (0),
  `calls/authored_numeric_name_requires`, `arithmetic/authored_max_result_bound`
  and `modules/ambiguous_numeric_callable_not_builtin` constrain selected-call
  behavior, declared contracts, result bounds and fail-closed ambiguity.
  `calls/runtime_numeric_operand_{single_evaluation,order}_exit` (0) constrain
  occurrence identity, operand snapshots, ordered effects and evaluation lifetime;
  `calls/numeric_operand_{call_requires,exclusive_arguments_overlap}` and
  `arithmetic/numeric_operand_abs_unrepresentable` must reject independently.
  `float/runtime_float_call_same_carrier_cast_exit` (0) constrains ordinary
  finite-result guarantees and payload-preserving casts in callers and returns;
  `float/same_carrier_cast_cannot_mint_finite` and
  `float/finite_result_requires_proof` must still reject independently.
  `float/runtime_format_conversion_result_bounds_exit` (0 in both engines)
  constrains rounded named-conversion result guarantees and strict IEEE ranges.
  `float/format_conversion_{requires_result_proof,exclusive_upper,lookalike}`
  must reject overflow, unknown NaN, rounded excluded ends and spelling lookalikes.
  `float/runtime_exact_format_narrowing_exit` (native/Terminal 0) constrains
  declaration-backed unchanged-denotation proofs and retained IEEE endpoints;
  `float/exact_format_conversion_requires_denotation` must reject lossy literals,
  unknown or finite-only operands, non-singleton windows and receiving-type premises.
  `float/runtime_exact_integer_coercion_exit` (0 in both engines) constrains
  unchanged-denotation coercions, live IEEE equality facts, ordinary result/entry
  predicates and integer bounds from selected named-conversion guarantees;
  `float/exact_integer_coercion_requires_denotation` and
  `float/integer_conversion_lookalike` must reject fractional values, NaN,
  overflow, integer-ended fractional intervals, stale writes and spelling lookalikes.
  `float/runtime_refined_conversion_target_exit` (native/Terminal 0) constrains
  Exact carrier conversion with ordinary result postconditions;
  `float/refined_result_wrong_or_excluded_bounds` rejects false output demands,
  while `float/refined_conversion_target_requires_source_proof` independently
  rejects fractional and lossy inputs. An output demand grants no input proof.
  `float/runtime_coercion_future_policy_exit` (0) constrains value-preserving
  float coercions and subsequent Wrapping/Saturating/Trapping operations;
  `float/policy_cast_requires_unchanged_denotation` and
  `arithmetic/float_policy_cast_fractional` must reject policy-qualified
  fractional, lossy and out-of-range coercions; policy qualification cannot
  authorize a payload-changing conversion.
  `float/runtime_named_policy_conversion_composition_exit` and
  `expressions/runtime_numeric_cast_exit` (70) constrain explicitly named
  truncation/clamping across calls and state transfers.
  `float/runtime_exact_integer_to_float_coercion_exit` (0) constrains selected
  sealed conversions, live guarded integer representability, full-width unsigned
  snapshots, field reads and future arithmetic policies;
  `float/integer_to_float_coercion_requires_denotation` must reject rounded,
  unknown, non-singleton and stale-write operands independently.
  `float/runtime_qualified_carrier_conversion_exit` (0) constrains preserved
  scalar qualification through exact conversion, copies, returns, parameters
  and inline alias arguments; `float/qualified_conversion_requires_operand_proof`
  must reject unproved predicates, unauthorized provenance and lossy operands.
  `domains/runtime_computed_qualification_exit` (0) constrains checked operand
  facts and completed-result membership across selected conversions, local
  updates, guarded arrivals, inline arguments and returns;
  `domains/computed_qualification_requires_operand_facts` must reject missing
  or withdrawn facts, later guards, false predicates, routed provenance and
  callable lookalikes.
  `calls/runtime_anonymous_result_local_namespace_exit` (70) and
  `float/float_to_int_trapping_nan_traps` (trap) constrain authored local
  identity after anonymous operation results in ordinary bodies.
  `collections/runtime_finite_element_view_call_write_exit` (0) constrains
  finite-result stores, captured indices and ordinary float carriers;
  `float/finite_constructor_requires_operand_proof` must reject an unqualified
  operand independently of its receiving field. Retain
  `float/runtime_finite_record_invariant_window_exit` (0 in both engines) for
  temporary poison repaired before borrowing.
  Existing wildcard debt and
  out-of-domain results must still reject. Wrong-path, escaped, stale-index,
  mutated-subject facts and byte-domain corruption must still reject.

  Acceptance: every supported primitive operation reaches stage 05 with its
  exact checking-time decision carried through `SourceChecked`; corpus failures
  rejected for a missing source decision reject from `omega --check` with
  checking diagnostics instead, without weakening other check outcomes.

## Automatic service reach

Finish [static callback reach dependencies](wiki/spec/language/effects.md#static-callback-reach-dependencies)
for every call shape. Finite-union inference, closed application retention and
selected contract bounds exist; retention still drops coverage.

- Owners:
  `omega-rust/psi/compilation/pipeline/05_checked-trees-to-lowered-psi/src/retention/closed_reach_applications.rs`
  and `omega-rust/psi/compilation/checking/src/machine_calls/static_machine_call_contracts.rs`.
- `retain_closed_reach_applications` skips proposition and
  requirement-identity binders, and
  `prune_incomplete_closed_reach_applications` removes incomplete annotations
  and their dependents. Retain the original dependency and telescope instead,
  including for inlined or absent emitted callees.
- Reuse exact specialization/contract identities; do not add a parallel
  selection identity.

Acceptance: corpus fixtures cover a named traversal with pure and Console
callbacks, nested/private helpers, recursive components, selected generic
schemas and unused selections. Inlining or a missing callee never removes
reach coverage. Reordered calls and equivalent helper extraction keep the
interface identity; adding helper reach changes it. Redirected same-shaped
calls, incomplete telescopes, missing direct boundary declarations and
selections exceeding requirement bounds reject. Opaque calls keep conservative
contracts; suspension/blocking stay separate. The pure callback's ordinary
callers see its specialized empty row without consuming the generic
definition. Keep `tests/omega/pass/effects/{nominal_callback_dependency,generic_callback_schema_reach,structural_callback_reach}`
passing.

## Semantic reflection

Connect [semantic reflection](wiki/spec/language/reflection.md) to authored
Omega and ordinary checked calls. The schema graph, scoped selection and
visitation plan in `omega-rust/psi/compilation/compile-time/src/reflection/`
are exported from `compile-time` but nothing outside the crate calls them;
`RuntimeVisitationPlan` records operation identities without field loans or
active-case execution.

- First deliver an inspector and a serializer through one source-level query,
  policy evaluation and per-member call mechanism. Psi owns elaboration and
  checking; ordinary library code owns encoding/inspection policy.
- Derive query authority from the lexical/package context, not a
  caller-supplied flag. Finish source-driven callable-contract checking in
  `selection/requirement_resolution.rs`, keeping exact qualifications, generic
  arguments, lifetimes and explicit selection.
- Generate real field subloans, fresh context reborrows, active-case dispatch
  and ordinary call/resource checking; a list of resolved operation names is
  not an execution plan. Freeze only owned evaluation results.
- Then recursive derivation, explicit runtime metadata/adapters and authorized
  Placed access under the same contract. Runtime recursion needs provisioned
  work storage. Fixed named callbacks can proceed now; selection-dependent
  evaluation uses [Automatic service reach](#automatic-service-reach).
- Explicit metadata is owned descriptive data; retaining it does not retain
  getters, setters, factories or every known type. Nonaddressable
  packed/fragmented fields reject borrowed visitation unless a selected
  copied-read operation supplies the contract. Schema enumeration performs no
  device access.

Acceptance: the inspector/serializer compile from Omega source and run.
Corpus fixtures cover a 40-field/five-type record with reusable type rules and
one member override; zero/multiple/wrong-type/stale-key/insufficient-contract
selections reject. Owner-delegated visitation across three packages works and
unauthorized enumeration rejects. Cover qualified and erased fields, borrowed
owned claims, zero-sized fields, empty records, distinguishable nullary cases,
common fields once, active payload only, loan escape rejection, dynamic-index
relationships, recursive reference types, snapshot independence and explicit
exhaustion rather than truncation. Descriptions never create grants or access.

- **WIRE-CODEC-LIBRARY.** (new-scope) Depends on the reflection work
  above. Wire `Schema::encode`/`Schema::decode` are realized by
  `omega-rust/omega/packages/compilation-inputs/src/wire_codec_sources.rs`,
  which renders Omega codec machines as source text from syntax trees and
  injects them as `.omega/generated/wire-codec.omg`. Both engines run those
  machines. This contradicts [reflection](wiki/spec/language/reflection.md#explicit-operation-selection):
  encoding is an ordinary library machine selected per member. The generator
  only covers call sites whose argument types render from a named local or a
  data-parameter field, and only "directly expressible" member shapes.
  Implement the compact_binary codec in Omega in `source/library/std/wire.omg`
  over the reflection serializer, satisfying its `Encode`/`StrictDecode`
  requirements. Then delete the generator, its injection in
  `omega-rust/omega/packages/compilation-inputs/src/source_assembly.rs`, and
  the intrinsic special cases for codec calls
  (`omega-rust/psi/compilation/checking/src/machine_calls/calls/write_frames/wire_codecs.rs`,
  `omega-rust/psi/compilation/pipeline/04_typed-trees-to-checked-trees/src/authored_selections/intrinsic_calls.rs`).
  Acceptance: the `tests/omega/pass/wire/` and `fail/wire/` fixtures and
  `samples/cli/systems/wire_protocol` keep their outcomes in both engines
  with no generated source.

## Scoped build execution

Implement [scoped build execution](wiki/spec/build/scoped_execution.md) for
ordinary code generators and the topology customer. **BUILD-ADMISSION-CHECKPOINT**
owns frontend/generated-source custody; extend it through checked Build
execution and retained outputs.

- **BUILD-PRODUCT-REFERENCES.** Finish
  [non-executing product selection](wiki/spec/build/scoped_execution.md#selecting-product-declarations-without-executing-them).
  Lexical visibility and slot compatibility are enforced in
  `omega-rust/psi/interpreter/checked-interpreter/src/interpreter/evaluator/build_machine/product_entries.rs`;
  `omega-rust/omega/compilation/realization/provider-planning/src/program_entry/root_bindings.rs`
  retains the selected symbol and slot. The scalar/multi-state customers
  `pass/build/{described,same_leaf}_runner_entry_{left,right}_exit` run (exit
  11 and 37). `fail/build/product_entry_*` and
  `fail/build/root_binding_foreign_helper_private_entry` pin the existing
  rejections. Keep [separate checked contexts](wiki/spec/build/scoped_execution.md#two-checked-contexts);
  never derive authority from names or evaluator indices.

  Acceptance: add fail fixtures for stale descriptions, description-to-callable
  conversion and same-build generated/layout cycles, and a pass fixture where an
  explicitly delegated private entry description stays usable without
  enumeration authority.

- **BUILD-SNAPSHOT-OUTPUTS.** Finish coherent capture and host acceptance under
  the [captured-input and committed-output contract](wiki/spec/build/scoped_execution.md#inputs-and-default-filesystem).
  `require_stable_recapture` in
  `omega-rust/omega/packages/compilation-inputs/src/source_snapshot.rs` and
  `omega-rust/omega/packages/source-acquisition/src/tree/capture/` recheck
  observed trees; matching observations do not make a mutable tree atomic.
  Establish the exact capture/staging/cleanup isolation premise or refuse,
  covering directory membership/link races and edits that restore observations,
  with logical path distinctions, inert links, retained parents and failure
  cleanup. Private directory creation relies on inherited directory isolation on
  Windows, which is unverified. Reuse the output custody in
  `omega-rust/omega/packages/build-output/` and compiler publication with
  **BUILD-ADMISSION-CHECKPOINT** and `PackageCheckedContext` source/occurrence
  identity. No new executor, live-host grant extension or persistent writable
  cache.

  Acceptance: an acquired generator package performs narrowed template reads,
  exact artifact/companion output, native invocation, retries and distinct
  package/target output sets on macOS and Windows. Negative lookups/metadata,
  substitution/link escapes, sealed mutation, cross-occurrence receipts, omitted
  outputs, interruption and final-check failure publish no partial set. The
  former CLI and package-manager tests for this were deleted; cover it with
  corpus fixtures whose `build.omg` depends on a generator package by
  `Source::Path`. There is no separate package harness.

## Process-exit contract

Finish the [canonical process-exit contract](wiki/spec/language/process_exit.md)
through source-produced Terminal Psi. Remaining source authority, provider
conformance and survivor-crossing checks must use the closed external edge,
not an ordinary Unit call. Checking/lowering, Terminal codec and observations,
the common core and native custody retain that edge and its exact i32 operand.
The hosted reference driver explicitly establishes an isolated containing domain;
ordinary import bindings cannot confer exit authority. Extend the same contract
to the remaining arrivals and product-source implementation.

- Native gaps (need not wait for the Terminal representation):
  - [normalized foreign-call custody](omega-rust/omega/compilation/backend/images/image-emission/src/object_artifact/normalized_foreign_calls.rs)
    accepts scalar homes only from call-result producers; transport a general
    computed status argument;
  - **WINDOWS-CONSOLE-INPUT** owns native Windows qualification. A native
    cross-build is not Windows execution acceptance;
  - replace the fixed 256 KiB foreign-call reserve (`SAME_STACK_BYTES`) in
    [source-evaluated bindings](omega-rust/omega/compilation/pipeline/00_terminal-psi-to-abstract-operations/src/provider_admission/source_evaluated_bindings.rs)
    with per-callee stack demand.
- Finish source-level containing-domain authority admission and migrate the
  transitional Console convenience reach. Console reach, imports and provider
  selection must not grant exit authority; reject missing authority/reach/progress
  and lookalike or returning/aborting substitutes.
- Exercise conditional helpers with both returning and exiting paths, preserving
  results, cleanup and postconditions only on returns. Keep
  `pass/host/process_exit_preserves_scalar_contract` as the unreachable-suffix
  control: exit cannot erase a declared scalar result or demand dead literals.
  Enforce transitive normal return for automatic cleanup and task settlement on
  normal root completion. Survivor crossing contracts must account for domain
  death without a whole-process resource census or invented discharge receipts.

Acceptance: unconditional and conditional/helper exits run in the interpreter
and natively. Missing authority/reach/progress, lookalikes, returning/aborting
substitutes, cleanup exits and unsupported survivor contracts reject.
`tests/omega/pass/host/process_exit_i32_status_ordered` exits 70 after one
stdout line and `process_exit_i32_status_mapped` presents status -44 as 212
(interpreter keeps -44), matching their `expected_native_exit.<target>.txt`
on macOS ARM64 and on Linux x86-64/AArch64 hosts; Windows native `ordered`
and `mapped` need matching-host measurements, including the full-i32 status.
**TWO-AXIS-TERMINAL-AUTHORITY-REVIEW** owns further receiver admission.

## P1 - Authority, roots, and entry

- **ENTRY-CONTENT-ROOTS.** Provisioned receivers are not yet activated and
  completed end to end under [entry roots](wiki/spec/build/entry_roots.md) and
  [provider-slot realization](wiki/spec/build/component_publication.md#provider-slots-and-era-entry).
  Delete `source/library/core/binding.omg`; `ForeignBinding` remains a separate
  compile-time locator. Checked boundary calls demand exact static slots and no
  authored qualification, source-created routing value or provider lookup may
  stand in for missing/ambiguous supply.
  - Former borrowed-service customers: migrate
    `samples/cli/games/dungeon_crawler_cli`'s Console fields/parameters
    (**NOMINAL-FIELD-FLOW**) to static `Console::operation` calls. Make
    `tests/omega/pass/capabilities/uses_caller_folder` call the exact `Folder`
    slot and carry only folder protocol data. Migrate the nested
    binding/helper-return chain of
    `tests/omega/pass/capabilities/acquires_through_helper_return` to named role
    applications or protocol handles, then verify equivalent reach/provenance.
    Keep
    `tests/omega/pass/text/runtime_stdin_line_buffering_exit` (exit 0, exact
    `first\nsecond\n`) as the control.
  - Activation/result/out-parameter joins must preserve exact closed slot
    applications, call occurrences and selected plans to native execution.
    Reuse `omega-rust/omega/compilation/backend/plans/program-entry-plan/`,
    `omega-rust/omega/compilation/driver/terminal-production/` entry receipts,
    `omega-rust/omega/compilation/pipeline/00_terminal-psi-to-abstract-operations/src/entry_settlement/`
    and `native-artifact`'s `publish_fragment_object`. Intrinsic service
    validity is separate from authorized domains; **DOMAIN-ISSUER-ROUTES** owns
    their result/out-parameter evidence. Migrate obsolete bare-carrier and
    qualification fixtures without permitting fabrication; unused declarations
    demand no provider.
  - Connect nominal cleanup and callback/signal occupancy to actual receiver
    completion. ZII/aggregate eligibility, hosted bridges
    (`omega-rust/omega/compilation/backend/images/image-emission/src/hosted_receiver.rs`),
    and a retained cleanup marker exist, but no authored destructor executes.
    The selected runtime/provider owns live receiver and callback occupancy;
    compose application, bridge, provider and callback stack demand, not loader
    stack-size metadata or another compiler ledger.

  Acceptance: published `samples/cli/basics/{number_guess,cli_mvp,generic_counters}`
  keep their documented behavior without service fields or test-supplied
  receivers (**SAMPLE-CORPUS** owns full sample coverage); the customers above
  execute natively. Reject missing/incompatible/ambiguous supply, lookalikes,
  redirected continuations, non-ZII ordinary state, invalid backing/alignment,
  overlapping partitions and stale slot eras. Distinct role applications stay
  distinct; fused erasure removes neither provider identity nor review checks.
  Targetless checks select no physical entry; deployment cannot replace the
  physical adapter with a semantic continuation.

- **CONSERVATION-CONTRACT / TERMINAL-CONTENT-CLAIMS.** Nonempty content claims
  do not execute through source, Terminal, selected providers and native code
  under [content conservation](wiki/spec/resources/content_custody.md):
  partition an established input under an authored theorem, pass the exact
  residual to a boundary, retain/return the rest and consume it in the caller,
  for direct calls and checked wrappers using canonical
  `source/library/core/content.omg` identities.
  - `tests/omega/pass/core/content_conservation_contract_exit` executes its
    checked `RegionSplit`/`RegionSink` providers natively and in the
    interpreter (exit 70): the provider body proves its own `separate` theorem
    for counted quantities, and installation joins a content argument minted
    inside the caller (call, construction or destructuring result) by the
    qualifications bound at the argument place. Still owed there: an
    interval-set provider body has no theorem judgment (the content tier
    stands down), a content argument below a root (`accept(split.left)`
    without destructuring) still refuses at installation, and a
    wrapper-composed exit (`forward(partition, whole)`) still records its
    composition against the wrapper.
  - `tests/omega/pass/core/content_retained_custody_round_trip` has an empty
    entry; its first claim depends on **BOUNDARY-ISSUANCE**. Existing-input
    conservation proceeds independently.
  - `tests/omega/pass/terminal_psi/{content_custody_exit,structural_content_passthrough}`
    use local lookalike types and no build entries; they do not establish
    canonical content execution.
  - Stage 04's `omega-rust/psi/compilation/pipeline/04_typed-trees-to-checked-trees/src/checks/content.rs::compose_partition_wrappers`
    derives transitive and staged-result compositions, but
    `omega-rust/psi/compilation/pipeline/05_checked-trees-to-lowered-psi/src/proofs/content_conservation.rs::lower_content_partition_compositions`
    rejects nonzero `source_derivation_depth` and nonempty `result_rewrites`.
    Carry the exact claim mappings through those forms, reusing the existing
    normalized equations, identity reshuffles and authored-theorem lowering.
  - `omega-rust/omega/compilation/pipeline/00_terminal-psi-to-abstract-operations/src/provider_installation/replay.rs`
    still requires whole-root content arguments. Complete the provider/caller
    content join for projected residuals (a content claim passed at a nonempty
    argument path).

  Acceptance: `python3 tools/corpus_gate.py --native --filter core/content_conservation_contract,core/content_retained_custody_round_trip`
  executes both with their authored results, preserving subject/revision,
  projection, geometry, lineage and installed occurrence. A partition theorem
  is usable only after its exact invocation; provider custody acceptance is not
  residual arithmetic, and fresh equal-sized literals do not discharge the
  input. Add fail fixtures where overlap, gaps, wrong projection/unit/lineage,
  stale invocation, substituted theorem/arguments and authority inferred from
  scalar totals reject. Runtime-indexed owned extraction stays rejected.

- **BOUNDARY-ISSUANCE.** Source issuance evidence is not joined to provider
  planning, native settlement and the installed occurrence under
  [external roots and issuance](wiki/spec/resources/authority.md#external-roots-and-issuance).
  First customer: give `tests/omega/pass/core/content_retained_custody_round_trip`
  an authorized initial claim and run its submit/complete cycle; today its
  entry is empty and the two issuer routes form a cycle, and an annotated
  `Buffer` cannot establish the first claim.
  - Stage 04's `checks/content/{retained_custody,call_results}.rs` already
    check authorized bare-result and selected-case grants and refuse to count
    consumed input as fresh supply.
  - In `omega-rust/omega/compilation/realization/provider-planning/src/`,
    `service_schema/mod.rs` derives result claims with only domain/carry, and
    `provider_planning/receipt_binding.rs` binds granted requirement identities
    to report receipts. Neither carries per-invocation geometry, backing or live
    supply; carry them through provider admission and native settlement.
  - Reuse stage 05's `unit/attached_unit/catalog.rs` finite installed-parameter
    schemas and carry real entry/provider observations through the general
    program-entry, selected-provider and boundary-settlement contracts. The
    authored runtime owns occurrence, capacity and epoch accounting.
  - **DOMAIN-ISSUER-ROUTES** owns route selection; **AP-BRINGUP** owns hardware
    arrival execution.

  Derive geometry from exact arguments, callable-entry places and result paths;
  admit backing/freshness premises, never interval arithmetic. Shared backing
  keeps a common custody root; several new results need one separated supply
  relation; transferred input is not new capacity. The finite
  installed-parameter alternative to provider issuance remains; neither needs a
  source-visible receipt binder.

  Acceptance: the customer executes natively with its authored result,
  retaining geometry, backing, issuer, lineage and installed occurrence. Forged
  source construction, foreign/replayed receipts, substituted geometry and
  duplicate supply reject (fail fixtures); identity-preserving transfers
  succeed without re-minting capacity.

## P2 - Materialization and placed access

- **PLAN-LAID-VIEWS.** Placed views cannot be established, accessed and
  retired by an executing source program under
  [placed access](wiki/spec/resources/placed_access.md#establishment-and-retirement).
  Start from `tests/omega/pass/layouts/placed_view_source_access_compile`
  (checks only): its `inspect` writes `transmit` and reads `status`, but
  `build.omg` declares only a package. `Main::main` lends its placed input to
  `inspect`; supply executable entry and call-bound custody for that input.
  Bind an admitted backing, invoke the accessors and observe a runtime result.
  - Checking admits the generated accessor attachment through its placement
    plan row (`add_placed_accessor_attachment` in
    `omega-rust/psi/compilation/pipeline/04_typed-trees-to-checked-trees/src/execution/terminal_unit/types/shape_collector.rs`).
  - Reuse `omega-rust/omega/compilation/pipeline/00_terminal-psi-to-abstract-operations/src/artifact_admission/establishment.rs`
    and `omega-rust/psi/interpreter/terminal-interpreter/src/structural_inputs/placed_views.rs`;
    interpreter completion already releases direct-entry loans.
  - Native publication rejects nonempty bound establishments in
    `omega-rust/omega/compilation/backend/artifacts/native-artifact/src/fragment_object_publication.rs`,
    and `entry_receiver.rs` rejects them without a receiver bridge. Extend the
    entry bridge to lend each referent through invocation and retirement.
  - `omega-rust/omega/compilation/pipeline/02_abstract-operations-to-target-operations/src/lowering/placed_view_inputs.rs`
    has no executable placed-view transfer lowering; join retained placement
    and accessor operations into normal native input and lowering.
  - Native admission still rejects subordinate placed input rows. Terminal
    ordinary calls now reborrow the caller's exact established occurrence,
    including its original backing and qualifications; native input/lowering
    must carry the same custody. A pointer or roster alone stays insufficient.

  Acceptance: the fixture (renamed `_exit` with `expected_native_exit.txt`)
  establishes, accesses and retires a view in the interpreter and natively
  (`python3 tools/corpus_gate.py --both --filter placed_view_source_access`).
  Stale/substituted plan, backing, range, rights, occurrence or lifetime reject
  before access; failed establishment returns custody; retirement preserves
  declared resident/vacant state; multi-row order independence, exclusive
  overlap rejection and shared/disjoint admission hold.

- **DEVICE-EXTENT-ACCESS.** `samples/cli/device/device_extent_access` checks
  but cannot execute provider-backed lending, completion and restored CPU
  access under [device custody and ordering](wiki/spec/resources/device_access.md).
  The sample's provider (`LoopbackDeviceLoanProvider`, ten
  `ForeignBinding::CompilerIntrinsic` crossings on macos_arm64) owns the range
  pool and the whole reserve -> lend -> publish -> notify -> settle ->
  maintain_cache -> settle -> complete -> acquire -> release cycle;
  `source/library/core/device.omg` declares the event roles and retry carriers.
  Two gaps keep `Main::main` inert:
  - Terminal unit plans admit no `BoundaryOpaque` types in reachable state or
    callee signatures (`shape_collector.rs` plans only `CheckedShape` data), so
    no entry can carry a live `DeviceLoan` or `DeviceOrderingEvent` across a
    transition.
  - The ten crossings have no runtime bodies.

  Reuse `omega-rust/psi/foundation/extents/src/{external_loans,ordering_events,mapping}/`:
  confinement, release/fact checks, coverage rejection and retry custody exist.
  `admit_acquisition_after_completion` only compares two event records and
  consumes no completion or live loan; coverage checks do not bind coordinates
  to a provider schema. Connect opaque runtime occurrences to the loan and
  role-specific ranges/mappings, device, request and scope. An intersecting
  write invalidates publication; notification consumes valid publication;
  acquisition consumes completion only after borrower release and all
  coherence obligations. Preserve scoped events through target realization,
  including instruction-free coherent implementations. Preserve
  `MappedExtent::stable_loan`'s revocation requirement and
  `ProgramLocalExtentRegistry::remap`'s checks (retained foreign pins, changed
  geometry and widened rights reject; replacement needs a fresh era).
  Protocols, queues, isolation and translation policy stay provider/OS-owned;
  a CPU barrier is not device completion.

  Acceptance: `omega run` executes the sample's full lifecycle in both
  directions. Add fail fixtures for missing/extra/duplicate roles, substituted
  coordinates, stale publication, wrong request/device/scope/mapping and
  premature restoration. Failed start returns the borrow; failed
  completion/acquisition retains the pending token for retry; permitted
  suspension and CPU exclusion are pinned per direction. For shared memory,
  exercise an atomic/coherence lease with trusted peers or
  copy-and-validate/revoke-and-validate with hostile writable peers.

- **PLACED-ACCESS-NATIVE-OPS.** Source-to-native indexed access through
  borrowed caller storage is incomplete across hosts under
  [structural access](wiki/spec/terminal-psi/structural_access.md).
  Current state:
  - macOS ARM64 native: `collections/runtime_indexed_struct_field_rmw_exit`
    exit 1, `collections/runtime_parameter_nested_literal_index_read_exit` exit
    70, `collections/runtime_double_indexed_rmw_exit` exit 1,
    `storage/runtime_machine_owned_indexed_integer_write_exit` exit 79,
    `collections/runtime_hoisted_index_write_exit` exit 7 (all authored).
  - Windows x86-64 native rejects `runtime_double_indexed_rmw_exit` and has no
    exit recorded for `runtime_indexed_struct_field_rmw_exit` or
    `runtime_hoisted_index_write_exit`; Linux x86-64 has no record for the
    nested-literal fixture.

  Reuse stage 04's `execution/terminal_unit/structural_scalar_store/selectors.rs`,
  stage 05's `emission/runtime_elements.rs`, and stage 02's
  `lowering/structural_layout.rs` (`runtime_projection`) and
  `lowering/control_flow/scalar_sources.rs`. Computed integer selectors,
  stored-value leaf-copy selectors and field-loop invariant inference exist.
  Keep one `AssignmentIndex { depth }` per selector, evaluate it before the
  stored value, and bound every `RuntimeIndex`.

  Acceptance: on Windows, macOS and both Linux targets,
  `python3 tools/corpus_gate.py --native --filter runtime_indexed_struct_field_rmw_exit,runtime_parameter_nested_literal_index_read_exit,runtime_double_indexed_rmw_exit,runtime_machine_owned_indexed_integer_write_exit,runtime_hoisted_index_write_exit`
  executes the fixtures above with their authored exits: `&write` and `&mut`
  calls modify the selected caller element and leave neighbors untouched. Add
  fixtures for field-held loop counters and leaf copies. Complete the same
  cross-host execution coverage for nested field/index call borrows in
  [the selector regression family](tests/omega/pass/collections/runtime_computed_nested_borrow_call_exit/README.md):
  shared reads, computed and signed indices, exact-once authored argument order,
  later mutation, private joins and dynamic Unit calls retain the selected
  caller cell and leave neighbors untouched. Keep the selected-extent and
  shared-authority refusal controls. Include the constrained shared-field
  [authored witness](tests/omega/pass/layouts/runtime_indexed_restricted_field_read/README.md)
  and padded two-selector variation in that cross-host execution coverage.
  Use
  **CANONICALIZE-SCALAR-RANGE-CONTRACTS** for bounds. **WRITE-ONLY-BORROW** and
  **STRUCTURAL-BORROW-IDENTITY** own general loan admission;
  **TERMINAL-SLICE-VIEW-VOCABULARY** owns view bounds; **PLAN-LAID-VIEWS** owns
  provider-backed views.

- **SYMBOLIC-MATERIALIZATION.** The recursive
  [derived consumer](wiki/spec/layouts/plans.md#derived-consumers) has no
  repeatable native execution check, including the Linux AArch64 leg. Its
  source customers' compiler tests were removed; add a corpus or sample
  customer that exercises it.
  - Reuse `omega-rust/omega/compilation/backend/layout/src/sum_materialization/tests/recursive.rs`
    (checked layout projection, closed generic substitution) and
    `omega-rust/psi/foundation/layout-plans/src/symbolic_materialization.rs`
    (nested field/index derivation).
  - `omega-rust/omega/compilation/backend/runtime/executable-installation/src/post_handoff_writer/tests/writer_execution.rs`
    compares portable Rust writes, and
    `omega-rust/omega/compilation/backend/plans/program-entry-plan/src/post_handoff_writer/mod.rs`
    lowers both Linux ISAs; neither executes the emitted fragment.

  Connect checked source geometry to the generated writer and its installed
  invocation under [writer custody](wiki/spec/build/hardware_materialization.md#generated-writers-and-publication).
  Open templates, non-closed member applications and runtime counts in static
  layout stay rejected; no depth-specific representation is needed.

  Acceptance: a registered native check on Linux AArch64 (and the other
  matching hosts) executes the generated writer and matches the reference
  destination bytes and surrounding guards, covering empty arrays, sibling
  writes and nested/generic cases with the same normalized fragment across
  ISAs.

- **ADDRESS-EXPOSURE.** (new-scope) The settled
  [`address_of` queries](wiki/spec/language/counts_and_addresses.md#address-exposure)
  have no `AuthoredDeclarationSelectionIntrinsic` or admission in
  `omega-rust/psi/compilation/pipeline/04_typed-trees-to-checked-trees/src/authored_selections/intrinsic_calls.rs`,
  nor any Terminal, interpreter or native support.
  - Retain the selected machine application or original referent; a machine
    query adds ordinary executable demand even without a direct call, which
    survives specialization/inlining and keeps the interpreted body.
  - Native entry/storage addresses use normal realization and relocations;
    preserve private callback ABI selection and custody.
  - The interpreter's `StructuralRuntimePlace` and `LocalStructuralIdentities`
    identify storage within one execution. Exposure needs disjoint code/storage
    encodings covering loaded instances, runtime occurrences and subplaces,
    fitting without truncation, collision or zero for live entries. No parallel
    registry, virtual byte memory or native trampoline.

  Acceptance: corpus programs execute `address_of<Selected<Arguments>>()` and
  `address_of(reference)` for generic machines, selected machine binders,
  locals and borrowed fields in both engines; repeated exposure agrees and
  distinct live locations differ. Fail fixtures reject unresolved selections,
  proof-only entities, invalid loans, nonaddressable projections and exhausted
  encodings. Addresses grant no borrow/call authority; `as addr` is not an
  alias. Evaluator IDs stay out of portable constants and unresolved relocation
  bits out of semantic evaluation. This adds no indirect-call syntax, patching
  or interpreter-ID interchange and stays separate from
  [instruction interpretation](wiki/spec/language/assembly.md#interpreted-execution).

- **NATIVE-CODE-LAYOUT-QUERIES.** (new-scope) Source cannot query
  [`code_layout<Machine>()`](wiki/spec/language/counts_and_addresses.md#native-code-layout-queries)
  for a selected application: entry address plus exact emitted instruction
  regions (`addr` bases, `u64` lengths). The low-level editor customer needs
  actual regions, not a guessed window or next-symbol subtraction.
  `omega-rust/omega/compilation/backend/machine-emission/src/code_layout.rs`
  (`describe_code_layouts`) already projects `PlacedInstructionSpan` geometry
  per placed application but has no caller. Missing: Psi admission and demand
  (reusing **ADDRESS-EXPOSURE**'s), and joining the descriptions plus final
  relocations into the selected application's runtime metadata. Psi records
  demand without running a second compiler or evaluating the unfinished native
  product.

  Acceptance: a native corpus program reads `code_layout` of an otherwise
  uncalled generic body and the metadata matches the final artifact, including
  split/shared regions, excluding padding, pools and callees. Fail fixtures
  reject a missing native realization and use inside the same product's
  type/constant/layout or optimization computation. Interpreted entries get no
  fabricated code size or region.

- **ASSUMED-BINDING-CONTRACTS.** (new-scope) The root-only
  [`Build::assume_binding_contract<Required::operation, Supplied::operation>(builder)`](wiki/spec/build/assumed_binding_contracts.md)
  relationship is not delivered. Psi classifies the call
  (`BuildOperation::AssumedBindingContract` in
  `omega-rust/psi/foundation/language-semantics/src/declaration_selection/mod.rs`,
  admitted in stage 04's `authored_selections/intrinsic_calls.rs`), the
  terminal interpreter accepts an `AssumedBindingSelection` policy input
  (`omega-rust/psi/interpreter/terminal-interpreter/src/effects.rs`), and
  review evidence has an `AssumedBindingContract` row kind
  (`omega-rust/omega/packages/review-evidence/src/record/rows.rs`). Build
  evaluation (`omega-rust/omega/compilation/build-evaluation/`) does not
  harvest the call, provider installation and native call custody do not
  honor it, and no corpus fixture uses it.

  Retain exact entry/exit assumptions, authorized issuer/subject mapping and
  the selected implementation closure through erasure and inlining. Both
  bodies remain checked against their own contracts.

  Acceptance: a corpus caller executes through the root-selected link with
  caller-facing qualification, including input-side authority; direct supplier
  calls gain none. Fail fixtures reject carrier/order, passing-mode, ABI,
  lifetime/custody and operational incompatibility; incomplete, competing or
  cyclic supply; unapproved roots; stale project acceptance; and use from
  helpers, generators, const/layout evaluation and automatic tests, including
  erased reliance. Install/update/audit disclose **Extremely high risk**
  transitively, and lock rows commit to the contracts, mapping and closure,
  requiring renewed acceptance when they change. Preserve the
  [refutable-connection exception](wiki/spec/build/assumed_binding_contracts.md#known-contradictions-at-the-trust-crossing):
  an accepted supplier guaranteeing `result == 0` may supply a requirement
  guaranteeing `result == 1`; audit distinguishes unknown from refuted and
  covers an input-side contradiction; no runtime value or controlled trap is
  promised under the false premise. Ordinary refutable grants, invalid
  `satisfies` and stale approval still reject. No unchecked body, override flag
  or parallel approval.

## P3 - Terminal Psi, proofs, and observation

- **WRITE-ONLY-BORROW.** Caller-visible `&write T` mutation is incomplete under
  [write-only authority](wiki/spec/terminal-psi/structural_access.md#write-only-authority).
  Remaining:
  - opaque foreign declarations and boundary-trait requirements need admitted
    non-observation evidence bound to the selected implementation and receipt
    before original-backing foreign output transport. Source checking rejects
    them today; preserve `fail/borrow/opaque_write_only_boundary`. ABI shape and
    a host output offer do not prove native non-observation;
  - aggregate replacement carrying conserved displaced custody, preserving
    whole-value validity without observing old contents through write-only
    access. Preserve the mutable controls
    `borrows/runtime_reference_aggregate_{call,alias,projected,array_alias,call_result}_replacement_exit`
    (0), including exact old/new loan lifetimes and projected reference storage;
  - computed IEEE store/readback (through **FLOAT-PROVIDERS**; the mutable
    controls stop in caller guard planning).

  Preserve `borrow/runtime_write_only_{indexed_record,captured_record,sum}_copy_exit`
  (0): whole-value installation, untouched neighbors, copied sources and
  captured selectors, with child-before-parent reference retirement.
  Keep `borrow/runtime_write_only_array_{indexed_calls,selector_snapshot}_exit`
  and `borrow/runtime_write_only_record_array_argument_exit` (0): indexed calls
  and captured write-only descriptors retain original backing and selector
  values, with one descriptor lifetime and unchanged neighboring fields.
  Keep `borrow/runtime_write_only_aggregate_selector_snapshot_exit` and
  `operators/runtime_generic_reference_selector_snapshot_exit` (0): capture
  scalar indices before later writes, including observations of returned
  references. Declared index operators retain their reference operands;
  bounded referents retain their store restrictions.
  Keep `borrow/runtime_write_only_local_affine_aggregate_exit` and
  `borrow/runtime_write_only_affine_sum_exit` (0): statically discardable
  affine replacements transfer once through write-only access, including local
  array backing. Nominal cleanup, linear displacement, and incoming reuse
  remain rejected.
  Keep `borrow/runtime_write_only_utf8_field_exit` (0): runtime concatenation
  and empty byte-field replacement restore declared encoding membership across
  a write-only call and subsequent state forwarding. Unqualified incoming bytes
  still fail the destination's domain obligation.

  Keep `borrow/runtime_write_only_reference_aggregate_{call,alias,projected,array_alias,call_result,copy,detached,capture}_exit`
  (0): discard exact old shared descriptor cohorts before installing incoming
  storage, preserving detached copies, copied incoming values and stored-pointer
  projections. Conditional replacement retains the old loan; new and detached
  loans still prevent conflicting writes. Write-only stored pointers remain
  unreadable, shared pointers cannot widen to exclusive access, and reference
  constraints survive projection and capture.

  Keep `borrow/runtime_write_only_exclusive_reference_*_exit` (0), covering
  direct, projected, forwarded, detached-child, call-result, call-operand,
  returned-view and owned-wrapper replacement. Replacement transfers exclusive
  descriptors once, writes through installed pointers at nonzero field offsets,
  and preserves old child loans until their final use. Incoming reuse, live
  incoming children and conflicting writes to either borrowed backing remain
  rejected.

  Coordinate sequencing with **STATE-LOCAL-VALUE-FRONTIER**, reference identity
  with **STRUCTURAL-BORROW-IDENTITY**, mutable dynamic dispatch with
  **FINITE-GENERIC-DISPATCH**, indexed realization with
  **PLACED-ACCESS-NATIVE-OPS**, and bounds with
  **CANONICALIZE-SCALAR-RANGE-CONTRACTS**.

  Acceptance: `python3 tools/corpus_gate.py --both --filter write_only` passes
  on Windows, macOS and both Linux targets, with fixtures covering exact widths,
  untouched neighbors, runtime signed/Boolean/floating sources, restoration,
  return custody and guarded/computed indices observed in original caller
  storage. Fail fixtures reject reads, bare forwarding, readable widening and
  overlapping exclusive arguments.

- **STRUCTURAL-BORROW-IDENTITY.** Original caller storage is not yet preserved
  through every projection, call preparation and native placement under
  [structural access](wiki/spec/terminal-psi/structural_access.md). Owners:
  stage 04 `execution/terminal_scalar/`
  and `execution/terminal_unit/calls/computation_arguments/`; stage 02
  `validation/structural_call_arguments.rs`; image-emission `argument_custody`.

  Remaining: current matching-host qualification of projected receiver custody
  on Linux x86-64 and Windows x86-64. The bounded Linux AArch64 qualification
  is in the [platform record](wiki/drafts/reference/rust_compiler_completion.md#linux_arm64--2026-10-08--structural-references-only-row-open).
  Retain the original owner's claims, live
  descriptor and ordered pointer crossings, never a guessed owner or referent copy.
  Reuse `Field`, `FixedIndex` and `RuntimeIndex { index, obligation }`; coordinate
  range spelling with **CANONICALIZE-SCALAR-RANGE-CONTRACTS**. Declared receiver
  custody stays independent of field observations and state count.

  Keep runtime-selected stored-reference calls on the same bounded descriptor
  roster as direct observations. `borrows/runtime_indexed_stored_reference_call`,
  `borrows/runtime_indexed_scalar_descriptor_call` and
  `borrows/runtime_indexed_reference_call_regions` retain selected and neighboring
  caller values, scalar codecs and selectors on both sides of pointer crossings.
  Missing bounds and writable access through a shared carrier remain refused.

  Controls (keep passing): `calls/unread_multistate_receiver_exit` (native exit
  70 on macOS, Linux x86-64 and Windows without a dummy read),
  `borrows/returned_record_field_read_exit` (70) and
  `borrows/static_record_reference_return_exit` (0) in both engines, plus
  `borrow/runtime_indexed_write_only_local_receiver_exit` (0) for caller-visible
  local-array writes through forwarded and direct indexed loans.
  `borrows/returned_owned_byte_field_snapshot_exit` and
  `borrows/local_record_byte_field_snapshot_exit` preserve independent snapshots
  after field replacement. `borrows/local_record_nested_sequence_extent_exit`
  retains nested local projections and distinguishes live extent from capacity;
  `borrows/owner_byte_read_during_shared_record_loan_exit` keeps shared owner
  observations available without permitting exclusive overlap.
  `borrows/local_shared_record_sequence_observations_exit` and
  `borrows/nested_reference_byte_observations_exit` observe original byte backing
  across ordered pointer crossings; `borrows/local_reference_scalar_completion_exit`
  keeps scalar results while releasing their local reference before its owner.
  `fail/borrows/shared_record_byte_write_before_last_observation` rejects mutation
  before the reference's final observation.
  `borrows/linear_call_result_shared_observation_exit` and
  `borrows/projected_claim_owner_shared_observation_exit` retain the original
  owner's claim through whole and projected helper loans and subsequent explicit
  disposal; shared observation must not discharge an unconsumed linear owner.
  `borrows/linear_owner_materialized_shared_reference_exit` and
  `borrows/linear_owner_materialized_mutable_projection_exit` preserve named
  descriptor custody through whole/projected loans and owning-helper transfer;
  fail controls retain live-loan exclusion and the owner's disposal obligation.

  Acceptance: corpus fixtures execute caller-visible projected/forwarded
  writes, owned-field mutable/write-only subloans, synchronized shared
  observations and register/stack reference passing on matching hosts
  including Linux AArch64 and Windows; a callee observing staged bytes does not
  prove caller writeback. Fail fixtures reject copied borrowed homes, missing
  dereference, changed access and duplicate overloads.

- **NORMALIZED-ABI-LOWERING.** Complete borrowed descriptor arguments and native
  indirect results across foreign calls under the
  [calling-plan contract](wiki/spec/build/calling_plans.md). Owners: Omega
  stages 02 and 03, backend call-frame emission. Owned whole-place arguments
  and register-returned aggregate results execute:
  `providers/foreign_owned_record_exit`, `providers/foreign_record_result_exit`
  and `providers/foreign_multiregister_result_exit` exit 70 on macOS ARM64.
  Foreign imports share one mechanism template per target ABI, derived from
  the evaluated plans.
  - Qualify remaining descriptor carriers and register/stack placement on each
    matching host. Preserve `host/runtime_descriptor_direct_array_exit` and
    `host/runtime_descriptor_output_streams_exit`: readonly byte views transport
    their evaluated pointer/length on macOS ARM64, never the descriptor's address.
  - Qualify indirect (memory-returned) aggregate results on the remaining
    matching hosts. Linux x86-64 uses caller-owned result storage through
    foreign calls, ordinary helpers and owned state arrivals; retain
    `providers/foreign_indirect_record_{result,relay}_exit` at exit 70.
    Preserve `providers/foreign_indirect_record_distinct_exit` at exit 70
    during remaining host qualification: earlier snapshots survive allocation,
    later results and release.

  Acceptance: corpus fixtures pass a borrowed descriptor and receive an
  indirect aggregate result from C with exact values, register/stack placement
  and formal order on each matching host. Terminal Psi stays target-neutral.
  **RESTORE-DYNAMIC-DESCRIPTOR-AND-TABLE-CUSTODY** owns implementation selector
  semantics; private callbacks retain their authored entry ABI and fragment custody.

- **OPAQUE-BY-VALUE-BOUNDARY-ABI.** Selected opaque arguments, results and
  nested fields are not exchanged at runtime under
  [representation agreement](wiki/spec/build/opaque_representations.md),
  building on **NORMALIZED-ABI-LOWERING**.
  `omega-rust/omega/compilation/realization/provider-planning/src/calling_policy_plans/boundary_signatures.rs::opaque_representation_movement`
  derives occurrence paths and placements; connect them to provider call-frame
  transport and backend emission. Physical copies preserve one semantic
  occurrence for affine and linear values; only checked semantic copying
  creates another.

  Acceptance: independently compiled producer and consumer programs exchange
  exact values at runtime; agreement is checked at each composition edge, and
  provider, carrier, target, lifecycle, multiplicity and historical-selection
  drift reject. Equal layout or compact fingerprints are insufficient. Keep
  sealed `Ptr<T>`, `EfiSystemTable` and erased proof-only `Real` distinct;
  proof-only use creates no runtime demand. V1 requires `Inert` carriers and
  rejects direct or nested cleanup ownership. **COMPONENT-SUBSTRATE** owns
  replacement compatibility and stable-handle eras.

- **PROOF-KERNEL-CORE.** The mathematical kernel does not yet check all
  source-produced proofs under the [foundation](wiki/spec/proofs/foundation.md)
  and [W-based profile](wiki/spec/proofs/inductive_profile.md). Owner:
  `omega-rust/psi/compilation/proof/admission/src/mathematical_core/`;
  **PROOF-CONTRACT-MIGRATION** owns source integration.
  - Replace remaining `bounded_denotation.rs::rule_instance` assumptions with
    derivations or justified checked rules for actual source obligations. Live
    case: open `Equal(!a, b)` expanded into a disjunction still assumes its
    conversion. Reuse the Boolean, correlated-arithmetic, cast and equality
    transport derivations under `bounded_denotation/`. Fixed arithmetic laws
    and construction-budget fallbacks stay explicit assumptions; a judgment
    assuming its conclusion is not a proof.
  - Check source-produced indexed applications against exact parameters,
    indices, payloads, case constraints and recursive uses. The `indexed_scheme`
    declarations in `mathematical_core/indexed.rs` are exercised only by
    Rust-built terms. Cover the profile's vector, derivation, mutual and nested
    families; distinguish negative recursion, unsupported valid encoding and
    malformed source. Add only kernel support source integration requires,
    without a parallel indexed checker or untyped eta.

  Acceptance: the [migration examples](wiki/spec/proofs/foundation.md#migration-acceptance)
  check from Omega source: universe-polymorphic arbitrary predicates,
  dependent pairs, identity transport and induction; strict same-statement
  conversion keeps distinct relevant witnesses. Malformed universes,
  capture-changing substitution and illegal elimination reject. Assumption
  closure is exact through statements/types, imports and erasure. Extend
  `tests/omega/{pass,fail}/proofs/kernel_theorem_equality_*` with false-source,
  missing-equation, changed-operand and changed-J-endpoint twins. Measure
  source-produced term size, storage and checking cost, and refresh the
  [metatheory evidence ledger](wiki/spec/proofs/kernel_metatheory.md#evidence-ledger)
  (its combined-profile argument is paper-level). Calculus changes go to
  `OWNER_QUESTIONS.md`.

- **PROOF-CONTRACT-MIGRATION.** General mathematics is not yet delivered through
  [machine contracts and witness/law bundles](wiki/spec/proofs/contracts.md#machines-and-bundles)
  and [mathematical bindings](wiki/spec/proofs/mathematical_bindings.md).
  Owners: Psi resolution/typing and contract checking, and
  `source/library/core/`; use **PROOF-KERNEL-CORE**'s terms. Start from stage
  04's `proof/mathematical_signature/` (prefix applications, explicit generic
  arguments, bounded universe inference).
  - Complete body denotation and applied carriers, including mathematical
    member-call resolution and non-integer ordering, keeping interpreted
    operations distinct from opaque carrier assumptions. Resolve non-type
    `core::*` occurrences by canonical symbol identity and remove
    `proof/fixed_core_vocabulary.rs`'s unresolved-spelling fallback.
  - Replace `proposition`/hidden-witness routes (including
    `proof_output_calls.rs` and core/test consumers) with ordinary
    contracts/bundles, preserving substitutions, witness identity,
    path/result availability and assumption closure; remove the retired
    parser/carrier paths and the replacement proof rows in
    `omega-rust/omega/packages/review-evidence/EVIDENCE_SCHEMA.md`, plus
    **QUOTIENT-THEOREM-LIFT**'s congruence-only lift.
  - Complete arbitrary mathematical application and theorem-only logical
    hypotheses: `TraitRequirement` needs predicate/function terms beyond
    lifetime/type arguments. Keep executable callback selection, local
    bindings and complete machine calls intact; no quantifier keywords or
    declaration-catalog substitute. **OPERATOR-MACHINE-SUPPLY** owns the shared
    call repair and Nat migration.
  - Connect source-established sequence/range predicates to element proofs:
    `ForAllInRangeFact` / `QuantifiedRangeFact` have no source producer or
    entailment consumer. Preserve collection, predicate, range and subject
    identity through mutation; reject wrong, out-of-range or invalidated facts.
  - Enforce [executable demand](wiki/spec/proofs/mathematical_bindings.md#assumptions-and-executable-demand)
    at source use, evaluation and lowering: axioms supply no executable
    provider; constant branch arms do not justify choice-dependent control;
    erasure retains assumption closure. Migrate core relations, quotients and
    examples.

  Acceptance (corpus pass fixtures each with a false twin):
  1. Two composed witness/law bundles keep substitutions and distinct relevant
     witnesses; higher-order theorems accept arbitrary predicates/functions and
     derived Π-term evidence for machine-shaped hypotheses, meeting the
     [binding delivery controls](wiki/spec/proofs/mathematical_bindings.md#delivery-controls).
  2. Nonconstructive existence permits erased references without executable
     extraction/branching; squash-to-strict reasoning needs no choice;
     constrained records prove coupling; relevant dependent pairs stay distinct
     from strict predicate gating.
  3. Accepting and denying assumption policies distinguish the same theorem by
     its exact transitive closure through statements/types, imports and erasure.
  4. Cauchy/quotient reasoning uses the
     [set-quotient interface](wiki/spec/proofs/quotients.md#set-quotient-foundation)
     without hidden extensionality or new reduction; a quotient-refusing policy
     accepts representative operations and congruence but rejects the
     assumption-bearing quotient theorem.

  Concrete remaining controls:
  - Reproduce `omega --check tests/omega/pass/proofs/cauchy_predicates_compile/main.omg`.
    It rejects an unmet instantiated `rat_close_triangle_split` premise and an
    unsupported proof-only `Nat` guarantee. Establish both judgments without
    weakening the theorem.
    `checking/src/proof_contracts/contract_entailment.rs` tells users to wait
    for undocumented "math roster N3"; replace that with an actionable missing
    capability or a documented reference, not another untracked tier.
  - `checking/src/machine_calls/fact_only_citations.rs` recognizes only empty
    nongeneric theorem bodies. Replace that source-shape rule with checked
    termination, effect and custody eligibility, settled before execution
    planning; integrate with stage 04's
    `execution/terminal_unit/control/statement_sequence.rs`. Keep exact callable
    substitution, nested field identity and isolated-parameter preservation.
  - Add the planned `pass/proofs/runtime_static_result_projection_citation_exit`
    fixture (absent from main): a theorem citation must establish the required
    result-projection fact without becoming a runtime structural call. Ordinary
    package review and native execution must reach authored exit 70. Preserve
    effectful/nonterminating and linear-receipt-reuse rejection controls.

- **PROOF-RELEVANCE-MIGRATION.** Erased arguments are not carrier-independent
  under [explicit erased bindings](wiki/spec/proofs/contracts.md#explicit-erased-bindings).
  Owners: stage 04 call plans and stage 05 contract/proof-term lowering. Static
  scalar contract arguments and record/sum constructions with
  `ProofTerm::Scalar` leaves work; `pass/relevance/erased_record_parameter_exit`
  covers signature stripping, not a contract consuming its witness. Array-field
  carriers refuse, and dynamic calls do not lower yet
  (**DYNAMIC-IMPLEMENTATION-SELECTION-MIGRATION**). Extend the existing routes to keep
  exact erased subjects/actuals, witness identity, scope, multiplicity and
  assumptions through static and dynamic calls, without runtime storage or ABI
  arguments. Preserve erased-field custody and the ordinary
  [normal cleanup](wiki/spec/terminal-psi/calls_and_outcomes.md#normal-cleanup)
  contract.

  Acceptance: erased record/enum and array-bearing formals and a dynamic trait
  call use their erased evidence in checked contracts and execute with stripped
  signatures; `fail/relevance/erased_nonscalar_parameter` and
  `fail/relevance/dynamic_erased_formal_lane` become passing positives. Missing,
  substituted, out-of-scope or violating actuals reject. Keep
  `pass/relevance/erased_parameter_proof_only` and
  `pass/relevance/erased_parameter_named_transition_forward`, plus runtime-read,
  receiver, layout-dependent-erasure and invalid-qualifier refusals.

- **BORROW-PROOF-CONVERGENCE.** Borrow checking cannot yet use every premise
  kind to establish loan disjointness under [loans](wiki/spec/terminal-psi/loans.md).
  Owner: stage 04 `checks/borrows/`. Requires/domain/guard/proposition and
  chained premises, pinned mutable results, fixed projections and guarantees
  from statement calls and nested call results work. Complete remaining
  callee/domain predicates, runtime projections and same-statement/theorem
  establishment using exact call substitution and the existing contract/range
  readers, not a second guarantee collector. Expand read footprints only for a
  demonstrated fact loss; distinguish builtin bound-meaning admission from fact
  invalidation.
  Unverified: the `low + 0u64` subslice lead (matched probes reject `view[0]`
  for unknown slice length even before an unrelated write).

  Acceptance: corpus pass fixtures accept disjoint loans, writes and exclusive
  call operands established by each remaining premise kind; fail fixtures reject
  missing, stale or insufficient premises, removed call prerequisites,
  cross-call copy substitution, overlapping mutation and containment-only second
  exclusive loans. Proof cannot create, widen, duplicate or extend authority.
  Keep `pass/slices/disjoint_mut_subslice_windows_compile` and
  `fail/slices/overlapping_mut_subslice_windows_rejected` as controls.
  **CANARY-CORPUS** borrow-obligation failures route here.

- **FOREIGN-RETAINED-ARGUMENT-BACKING.** Authored retention is not connected to
  native argument backing under
  [outbound custody](wiki/spec/build/foreign_storage.md#outbound-custody).
  Owners: checked content/call planning, stage 05
  `retention/retained_borrow_custody.rs`, native marshaling and the selected
  provider's ordinary retention contract. Complete the source-to-native
  connection including installed-provider custody for retained-borrow
  guarantees. Materialize pointers only from exact stable-root,
  range, access, lifetime and revision/lease evidence; reuse result-`ensures`,
  nominal matching and structural-result catalogs. Add explicit snapshot
  permission, private backing and persistent demand with capacity per live
  occurrence; copying implies neither identity preservation nor write-back.
  Moved backing goes through **CONSERVATION-CONTRACT / TERMINAL-CONTENT-CLAIMS**.
  Authored custody selects each disposition.

  Acceptance: moved, lifetime-borrowed and permitted-snapshot boundaries run
  natively; completion redeems moved backing and live shared loans exclude
  conflicting writes. Call-scoped, mutable lifetime-borrowed or ambiguous
  retention and unknown/stale/out-of-range/excess-rights backing reject.
  Registration leases follow the [authored lifecycle](wiki/spec/build/private_callbacks.md#registration-and-lifetime).

- **TERMINAL-SLICE-VIEW-VOCABULARY.** (split-of:SLICE-VIEW-LOCAL-ENTRY-ESTABLISHMENT)
  Consuming non-byte `&[T]` calls and state transfers are incomplete under
  [view custody](wiki/spec/terminal-psi/byte_views.md#element-typed-slice-views).
  Length, indexing and subslicing through calls and guarded state transfers
  execute natively with interpreter agreement on macOS ARM64 (controls:
  `slices/callee_non_byte_view_len_index_subslice` exit 9,
  `slices/runtime_subslice_and_element_read_call_operands_exit`,
  `calls/runtime_dispatch_slice_element_terminal_exit`,
  `calls/runtime_dispatch_machine_array_slice_arg_exit`,
  `slices/runtime_slice_iteration_exit`,
  `slices/runtime_byte_view_guarded_state_contract_exit`, `samples/cli/systems/wire_protocol`;
  all `fail/slices` fixtures reject). Reuse stage 04's
  `execution/terminal_unit/calls/view_subslice.rs` and stage 05's
  `unit/attached_unit/view_ranges.rs` and
  `emission/operation_emission/view_subslice.rs` (shared `CheckedSubsliceSite`).
  Remaining:
  - nonempty restricted scalar array and primitive-local construction still
    lack an initializer-obligation roster when zero does not satisfy the
    declared predicate. Terminal refuses these constructors; empty arrays
    instantiate no element. Refined scalar parameter/state-arrival contracts
    still decline non-interval memberships in
    `checking/src/machine_calls/structural_call_custody.rs`; source checking of
    narrowed record-element slice arguments still lacks their default-domain
    window restoration. Preserve the complete predicate copy/replacement control
    `slices/runtime_predicate_qualified_scalar_copy_exit` and the bounded
    copy and replacement controls with
    `omega run --both tests/omega/pass/slices/runtime_range_qualified_scalar_copy_exit/main.omg`,
    its local and fixed-array snapshot variants, and
    `slices/runtime_range_qualified_scalar_dynamic_store_exit` (Linux x86-64,
    authored exit 0). These constrain retained scalar declarations, exact
    receiving assignment receipts, independent snapshots, and live per-element
    domains. Missing range/bounds, invalidated or partially initialized domains,
    and shared-view writes must continue to reject.
  - cyclic view forwarding compositions on the Unit-graph route remain
    unverified. Preserve call-result descriptor retirement and exclusive
    reborrowing when extending this route. Acyclic named-state subslice forwarding
    retains caller backing for scalar and record writes (controls:
    `slices/runtime_mutable_subslice_state_forwarding_exit`,
    `slices/runtime_dynamic_record_subslice_state_forwarding_exit`), including
    record field writes through the Terminal interpreter's structural backing.

  Acceptance: consuming views execute natively and in the Terminal interpreter
  for scalar and record elements; bounds, element-type, alias and access
  violations reject. `samples/cli/text/fletcher_checksum` exits 56 and
  `samples/cli/arithmetic/recursive_sum` exits 70 on Linux x86-64
  (**SAMPLE-CORPUS** owns the full sample obligations). **GENERAL-CYCLIC-EXECUTION**
  and **STATE-LOCAL-VALUE-FRONTIER** own general control/value sequencing.

## Build-level behavior exclusions

Deliver the [exclusion contract](wiki/spec/build/behavior_exclusions.md).
Checking/no-op assertions and optional logging stay ordinary calls with
unchanged public ceilings; exclusions add product admission requirements,
never permission to violate callable contracts. Reuse the shared checker in
`omega-rust/omega/compilation/backend/plans/provider-evidence/src/provider_evidence/executable_scopes/behavior_exclusions.rs`.
The customers are the corpus fixtures `build/crash_exclusion_*` and
`build/*service_exclusion*` under `tests/omega/{pass,fail}/`, which depend on
the `assert-kit` and `logger-kit` packages in `tests/omega/packages/` by
path; both items are accepted through `tools/corpus_gate.py`.

- **BUILD-SEMANTIC-EXCLUSIONS.** Cover the full admitted entry/call closure.
  Executed selections
  (`omega-rust/omega/compilation/realization/trust-model/src/behavior_exclusions/selections.rs`),
  entry/callback gates in `omega-rust/omega/compilation/driver/src/terminal/`
  and stage 00's
  `omega-rust/omega/compilation/pipeline/00_terminal-psi-to-abstract-operations/src/retained_terminal.rs`
  exist; the checker covers selected adapters, bounded dynamic dispatch,
  forwarded/rebound descriptor parameters and nominal cleanup. Remaining:
  source-produced coverage of generated entries and the remaining dispatch
  forms. Conservative retained contracts may bound opaque targets; otherwise
  missing coverage rejects. Only guard facts established by checking can
  exclude paths; optimization and broad public ceilings cannot prove absence.
  Namespace dispatch must never silently execute its empty prelude body.

  Acceptance: with optimization off and on, `no-op-app` and `quiet-logger-app`
  build and run natively; checking/direct-Unit assertions, eager predicate
  traps and actual Sink invocations refuse with attribution. A silent logger
  can pass a service exclusion; a silent provider cannot erase a boundary
  invocation. Cover conditional/helper selections, and distinguish prohibited
  possible behavior from insufficient coverage.

- **BUILD-EXCLUSION-REALIZATION.** Carry exclusion policies through native
  artifact custody, installation and replacement.
  `omega-rust/omega/compilation/driver/src/native/native_realization.rs` checks
  exercised mechanism classes before emission and
  `omega-rust/omega/compilation/driver/src/native/retained_native_product.rs`
  passes the proposal's union into that check, but
  `omega-rust/omega/compilation/backend/artifacts/native-artifact/src/native_artifact.rs`
  keeps the mechanism review without the exclusion union. Join the policy to
  the exact semantic/target/provider selection through legalization, image
  emission, **COMPONENT-SUBSTRATE** replacement and
  **WIRE-RUNTIME-AND-INSTALLATION**. **TWO-AXIS-TERMINAL-AUTHORITY-REVIEW**
  owns receiving permission; reuse its mechanism classification.

  Acceptance: silent and output-producing providers get distinct physical
  verdicts with/without receiving policy and optimization. Unknown
  classifications cannot prove absence; substituted providers reject, stricter
  receiving exclusions require a fresh join, and a failed final check
  publishes nothing. Exercise actual Windows and macOS runtime, installation
  and replacement; cross-target realization is insufficient.

## Requirement-based tests

Implement the [testing contract](wiki/spec/build/testing.md) and
[authored source shape](wiki/language_guide/chapter_23_testing.md). Reuse
ordinary satisfaction/entry checking, Terminal Psi and native execution, and
embedding custody; no test attribute, special machine name, calling permission or
parallel compiler.

- **BUILD-TEST-GROUPS.** (new-scope) Build has no test registration:
  `Build` in `omega-rust/omega/packages/build-declarations/src/prelude.rs` has
  no `tests` facet, and `EvaluatedBuildSettings` in
  `omega-rust/omega/packages/compilation-inputs/src/build_execution/build_value/configuration.rs`
  retains no groups. Build evaluation must carry
  `builder.tests.group<ExactRequirement>()` registration and enablement;
  compiler orchestration (`omega-rust/omega/compilation/driver/`) owns
  package-local satisfaction discovery, separate runner roots, outcomes and
  publication. Retain group target/engine overrides and requirements; default
  to all applicable permitted engines on the host. Expose the execution plan
  before effects, including unavailable combinations and reasons. Produce native
  test executables separately from application images through the normal native
  driver; reuse compatible checked inputs, not a second test compiler.
  Keep exact product-reference identity, private scope and
  source/target provenance; never discover by strings or infer generic
  applications.

  Acceptance: an ordinary build discovers two tests without per-test
  registration; a failing mandatory check prevents publication; disabling its
  group reports not-run; a second requirement in the same trait stays
  distinct. Cover duplicate/inherited identities, non-runnable signatures,
  unresolved generics, dependency non-discovery and exhaustion distinct from
  failure. Runs without std; an optional std testing requirement needs no
  compiler recognition. Application Psi/native output excludes test-only roots;
  a native harness may explicitly call visible test machines. Run a pure group
  on both engines with distinct results and equal deterministic observations;
  cover explicit overrides, required/optional unavailable engines and no runnable
  engine. Scheduled failure/exhaustion cannot become an optional skip.
  Receiver/service execution rejects until **BUILD-TEST-AUTHORITY**.

- **BUILD-TEST-AUTHORITY.** (split-of:BUILD-TEST-GROUPS) Add group-scoped
  providers, provisioned receivers and fresh mock state. Extend the restricted
  requests in `omega-rust/omega/compilation/build-evaluation/src/admitted_build_program.rs`
  and `omega-rust/omega/packages/package-manager/src/review/restricted_build_grants.rs`
  (which join build-host requests to lock acceptance) to carry test/group,
  fixture and provider attribution before execution. Keep policy out of
  constant-evaluator admission and use the existing review flow.

  Acceptance: two tests receive fresh virtual filesystems through distinct
  group-scoped provider-slot applications and ordinary mock state; missing or
  ambiguous supply rejects. Child filesystem/network/process
  requests cannot widen root grants or escape virtual backing. Install/update
  surfaces new test requests; locked builds cannot approve them; acceptance
  without an executor grant rejects. Cover provider/target mismatches,
  disabled-to-enabled rechecking, required/optional engine availability, and
  crash containment without invented unwind. Virtual-filesystem tests run on
  both engines without a native escape to real backing; a missing interpreter
  adapter does not silently replace the selected service. Automatic tests
  reject assumed-binding execution, including through fixtures, helpers and
  providers. Mock configuration leaves production providers and roots
  unchanged.

## Embedding and interpreted components

Implement the [embedding contract](wiki/spec/build/embedding.md) as an ordinary
library, starting with one source-to-host customer. Generic embedding does not
wait for Cathedral's instruction coverage. The interpreter is
`omega-rust/psi/interpreter/terminal-interpreter/`.

- **EMBEDDING-SOURCE-TO-HOST.** (new-scope) Deliver compile -> load -> supply ->
  invoke -> host result -> guest result -> close from an authored interface
  package, host implementation and script, connecting public entry
  descriptions, the interpreter and checked host adapters through exact static
  import/provider slots.
  Reuse `TerminalProgram::load`/`instantiate` in `src/program.rs` and
  `TerminalInstance::bind_import`/`invoke`/`close` in `src/instance.rs`, with
  typed adapters in `src/adapters.rs`. These lifecycle APIs already exist;
  the remaining acceptance is the source-authored interface/host/script join,
  not another program or instance abstraction. The corpus execution helper
  loads `TerminalProgram`, but that alone does not exercise host import binding.
  - Share immutable admitted programs across instances; two static roles use
    distinct exact trait applications, while runtime host objects are ordinary
    protocol instance data.
  - Missing/wrong/unauthorized bindings and invalid host input reject before
    effects; native result schema mismatch rejects.
  - Public descriptions and dynamic calls use the same adapters and retained
    entries, without new generic instantiations or private access; reuse
    [Semantic reflection](#semantic-reflection) for type metadata.
  - Inspect/update live objects only through exposed operations at valid access
    points. Loading Psi needs no source compiler, filesystem, global runtime or
    thread.

  Acceptance: an end-to-end embedding fixture runs the full sequence and the
  listed rejections.

- **EMBEDDING-RESOURCE-LIFETIMES.** (new-scope) Extend
  **EMBEDDING-SOURCE-TO-HOST** with borrowed views, owned host resources,
  asynchronous completion and contracted callbacks. `src/results.rs` has no
  waiting/completion protocol. Extend the instance-owned adapter roster, which
  already remains bound across fuel pauses; do not build the public lifecycle
  on raw effect sinks that permit substituting a handler on resume.
  The host/runtime owns registration, teardown and component eras; compiler
  descriptions and installation evidence grant no live lifecycle authority.

  Acceptance: a pause inside an invariant window rejects conflicting
  inspection/mutation (including edits that keep the type invariant but break
  retained stronger facts) and resumes without repeated effects. Retained
  results/callbacks prevent close; dropping an invocation does not release
  debt. Stale/wrong-runtime handles and duplicate/late completions reject; each
  completion matches its pending request once. Failure after host effects and
  unsupported abandonment neither retry nor fabricate cleanup. One instance
  closes while another stays usable.

- **PSI-COMPONENT-REPLACEMENT.** (new-scope) A running guest replaces an
  independent interpreted subcomponent without returning from main. Depends on
  **COMPONENT-SUBSTRATE** and **EMBEDDING-RESOURCE-LIFETIMES**.
  `omega-rust/omega/compilation/realization/provider-planning/src/provider_settlement/composition_modes.rs`
  rejects independent products. Connect Psi instances/entries to the
  component contract's import/export and era requirements through the
  embedding runtime's existing instance owner; no second module manager or
  compiler-owned era ledger. Root/nested build composition produces the
  replacement Psi, and the guest stages and publishes it under delegated
  authority. The application owns acquisition, migration and update policy.

  Acceptance: new entries use v2, paused old calls stay on v1, and retirement
  waits for every pin. Missing update authority, incompatible schema, widened
  authority, premature retirement and handler substitution on resume reject.
  Shared external state and explicit state disposition work while unaffected
  components stay usable. Loading never publishes.

- **INTERPRETED-ASSEMBLY.** (split-of:ASM-INSTRUCTION-CATALOG-EXPANSION)
  Deliver [checked instruction interpretation](wiki/spec/language/assembly.md#interpreted-execution)
  through source -> Terminal Psi -> the ordinary interpreter with a supplied
  target environment. The source catalog and `src/calls.rs`'s `PortWrite`
  effect cover slices, not the demanded instruction/environment closure. Reuse
  value/place/transition operations and **PRIVILEGED-PORT-EFFECT-SETTLEMENTS**;
  no separate asm pipeline, whole-host emulator or per-block source wrapper.
  Blocks share explicit values/authorized memory, never hidden scratch.
  Providers supply mechanisms; Cathedral owns drivers and scheduling.

  Acceptance: device, interrupt-state, idle and external-entry sequences run
  across helpers and fuel pauses without repeating completed I/O; ordering,
  delayed effects, event points, control exits and custody follow target
  semantics. Missing semantics, missing/wrong environments, widened host
  authority and numeric-ID-as-pointer attempts reject before effects. Modeled
  behavior is validated separately from native encoding.

- **INTERPRETED-CATHEDRAL.** (new-scope) Run Cathedral startup, memory
  establishment and visible device work, then live event delivery, then
  guest-owned component replacement, in the interpreter. Depends on the
  embedding/component items and **INTERPRETED-ASSEMBLY**. Keep the OS sequence
  and shared native/interpreted algorithms; select providers and build
  composition, without mode conditionals or assembly removal. Host mechanisms
  cannot replace Cathedral's boot, idle, device, scheduler or update policy.

  In the Cathedral repository, `tools/x86-legacy-bootstrap-composition-canary/main.omg`
  is compile-only with an empty entry, and
  `tools/{extent-root-canary,timer-root-work-canary}/run.sh` still read retired
  `04_typed_trees.json`/`05_machine_contracts.json` dumps; migrate those
  oracles to observed behavior (Omega emits no debug dumps by default).

  Acceptance: the sequence runs with reported partial slices; a banner is
  insufficient, and simulation establishes modeled behavior only.

## P5 - Cathedral over general Omega primitives

- **BUMP-ALLOCATOR-CANARY.** Deliver an executable package allocator over
  qualified `Extent` under [allocation](wiki/spec/resources/allocation.md),
  needed by **VEC-NATIVE-GROWTH** and Squalr. `source/library/alloc/bump.omg`
  and `tests/omega/pass/memory/bump_allocator_canary` exist, but the fixture's
  entry is empty and selects no executable partition/resident-storage
  provider; `BumpVec` reserves extents without elements; `RetiredSlot` holds one
  old buffer.
  - Complete the `ExtentPartition`/conservation route through
    **CONSERVATION-CONTRACT / TERMINAL-CONTENT-CLAIMS**: prove aligned request
    geometry and the counted-residual/tail relationship.
  - Replace fixture-local `ResidentStorage` with **PLAN-LAID-VIEWS** layout,
    establishment, element transfer and retirement. Core's `Vacant`/`Resident`
    need authorized establishment routes under **DOMAIN-ISSUER-ROUTES**; the
    current cast-forgery diagnostic is a heuristic over the carrier's other
    domains. Live residents cannot merge as vacant.
  - Borrow the strategy exclusively through **BORROWED-STORAGE-RESTORATION**,
    ending at allocation return so allocations coexist (today the strategy
    moves through result records).
  - Monotonic release cleans values and retains returned extents until reset;
    current `release`/`shrink` restore the tail immediately.

  Acceptance: with real backing, coexisting allocations, unchanged state on
  failure, exact cleanup/return, and reset to original backing only after all
  allocations return, in the interpreter and on a native host. Live reset,
  dropped custody, double placement, wrong resident indices and unrouted
  introduction reject; keep `fail/memory/bump_allocator_*` passing.

- **UEFI-PHYSICAL-SEMANTIC-ENTRY.** Execute the
  [two-surface bootstrap](wiki/spec/build/uefi_entry.md#authored-firmware-definitions-and-adapters).
  The target package owns firmware layouts, integrity, calling policies and
  bootstrap bodies; the compiler supplies the physical shell and generic
  ABI/entry primitives. Connect `source/library/std/targets/uefi_x86_64/tables.omg`,
  the program-entry physical/semantic plans and
  [native-artifact's receiver wrapper](omega-rust/omega/compilation/backend/artifacts/native-artifact/src/fragment_object_publication.rs)
  to actual arrival and scoped providers through ordinary program-entry,
  calling, installed-root and provider contracts.
  Native publication retains the unfulfilled physical-entry obligation for
  both free and context-bearing semantic continuations. Supply and consume
  that bridge before publishing an image; the staged semantic wrapper alone
  is not firmware entry. The two variants are `pass/build/uefi_entry_schema_same_leaf`
  and `pass/build/uefi_provisioned_context_roots`.
  Cathedral's `source/kernel/boot/uefi` is the source customer;
  `tools/boot-harness/run.py` there builds a fresh image and checks the greeting
  followed by the owned-memory report under QEMU/OVMF. Its ordinary boot
  machine is not yet connected to this physical-to-semantic adapter. Repair
  general capabilities in their owners; no firmware-protocol intrinsics.

  Acceptance: authored integrity/projections/calls lead from physical arrival
  to the selected continuation, establishing image and initial-storage roots
  together (excluding live stack/receiver/bootstrap storage) with disjoint
  ownership and derived stack demand. Returning applications release adapter
  allocations and return the right status; rejection keeps inputs without
  entering the continuation. Malformed geometry/header integrity, foreign
  occurrence, wrong package/target and overlapping roots reject.

- **UEFI-OS-HANDOFF.** Execute the
  [Boot Services-to-OS handoff](wiki/spec/build/uefi_entry.md#returning-application-versus-os-handoff)
  in `tests/omega/pass/build/uefi_os_handoff_invocation` (rejected on every
  host today) and `source/library/std/targets/uefi_x86_64/handoff.omg`.
  Native selection lacks execution identities for
  `UefiOsHandoffTermination::{transfer,firmware_return}` in
  `omega-rust/omega/compilation/realization/selected-dispatch/src/compiler_intrinsic.rs`;
  **UEFI-PHYSICAL-SEMANTIC-ENTRY** owns those mechanics.
  - `acquired` sends any success straight to exit without validating
    descriptor geometry/version; `free` adopts the new buffer without checking
    release status. Keep both allocations on rejected release and keep the
    freshest map/key, allocation lineage and a stack that survives Boot
    Services exit.
  - Keep allocation/status/retry and firmware-call sequencing in target source;
    do not recreate the deleted Rust firmware runtime.

  Acceptance: emitted code under firmware or a controlled provider covers
  growth, stale-key retry, bounded exhaustion with the target error, rejected
  allocation/release, malformed maps, stale/foreign evidence, lost custody and
  forbidden Boot Services use after exit; success enters the selected OS entry
  without returning. A bodyless provider, multi-call intrinsic or fallible
  cleanup destructor cannot replace the cycle. A source/dependency guard
  permits UEFI-specific compiler code only in target descriptions, ABI/calling
  and entry/image encoding.

- **AP-BRINGUP.** Execute Cathedral-owned secondary-processor startup under
  [external roots](wiki/spec/build/external_roots.md#secondary-processor-startup).
  Cathedral owns discovery, dispatch, acknowledgement, retry and cancellation;
  the compiler checks installed entry, placement and resource custody.
  `tests/omega/pass/memory/secondary_processor_canary` declares entry,
  acknowledgement and roster but has an empty `Main::main`. Connect it to the
  trampoline in
  `omega-rust/omega/compilation/backend/machine-emission/src/startup_trampoline.rs`,
  generic installed-entry/resource custody and provider receipt ingress through
  **BOUNDARY-ISSUANCE**. Do not recreate the deleted compiler-side startup
  protocol.

  Acceptance: on a matching platform or emulator, the emitted trampoline enters
  the installed entry with dedicated, nonoverlapping stack/state, bound to the
  provider profile, arrival regime and vector/low-memory geometry. Definite
  nondispatch permits withdrawal; unconfirmed dispatch holds resources and
  admits a later exact arrival; cancellation needs evidence of no future
  arrival; retirement needs quiescence; timeout releases nothing. Exercise
  timeout/late arrival, cancellation before confirmation, races,
  foreign/stale/replayed receipts and resource conflicts. No compiler-owned
  APIC driver. A source guard allows target trampoline bytes, placement and
  entry ABI but no compiler-owned processor roster or lifecycle state machine.

- **ADDRESS-TRANSLATION-CANARY.** Execute Cathedral-owned mapping installation
  and teardown under [mapping and reclamation](wiki/spec/resources/extents.md#mapping-and-reclamation).
  `tests/omega/pass/memory/address_translation_canary` checks single-leaf
  install/remove routes, but its entry is empty and its hardware/receipt
  crossings are bodyless. Join selected authority, hardware and shootdown
  providers to `extents::mapping`'s source/destination, identity, era and
  activation/release receipts.
  - Calls forwarding `BorrowedMapping in Pending`/`Installed` to checked
    identity helpers cannot prove `held_source in Extent::Granted`; borrowed
    installers also need returned-loan provenance and correct table-result
    ownership.
  - Multi-page loops need **TERMINATION-RANKING-CHECKS**; demand-grown tables
    need **BUMP-ALLOCATOR-CANARY**, placed read-back **PLAN-LAID-VIEWS**, range
    migration **CANONICALIZE-SCALAR-RANGE-CONTRACTS**; boot integration needs
    **UEFI-PHYSICAL-SEMANTIC-ENTRY** / **UEFI-OS-HANDOFF**.
  - `FRAME_MASK` is `0x000FFFFFFFFFF000` (bits 12..51); verify geometry and
    level-specific leaf/link encoding against Cathedral's table/walk oracles.

  Acceptance: QEMU installs and tears down multi-page mappings with Extent/TLB
  custody; access exists only between activation and unmap; reuse waits for
  completion. Source use after map, premature unmap, lost shootdown, forged
  carriers, borrowed-source reclaim, replayed activation and wrong-mapping or
  stale-era receipts reject; keep `fail/core/translation_unmap_before_activation`.
  No compiler-owned page-table/TLB types.

- **EXCEPTION-ROOTS-AND-TIMER.** Execute Cathedral fatal entries on dedicated
  critical stacks and a timer that acknowledges, records and wakes work under
  [interrupt obligations](wiki/spec/build/interrupt_obligations.md),
  [hardware materialization](wiki/spec/build/hardware_materialization.md) and
  [machine-state evidence](wiki/spec/build/machine_state_evidence.md).
  `tests/omega/pass/memory/interrupt_table_canary` has authored roots, layouts
  and validators, but an empty entry, and both handlers only acknowledge.

  The fixture now selects a `CompilerIntrinsic` provider for
  `InterruptAcknowledgement::complete`, but that operation has no native
  realization. **TOP-LEVEL-BOUNDARY-REQUIREMENTS** owns migration to a
  one-member boundary trait and **CHECKED-ASM-LABEL-CONTRACTS** owns its checked
  target implementation; adding another selection row does not solve it.
  Retain the argument's checked domain evidence and exact installation
  relationships; provider selection remains a separate implementation duty.
  Resolve that compiler/source dependency before the backend joins below.
  - Join the stub and member-call relocation from
    `omega-rust/omega/compilation/backend/machine-emission/src/entry_exit_stub.rs`
    to image placement, installation and external-root admission;
    `X86_64DeriverStubEntryEmission` has no production producer. Bind entry/body
    coordinates, bytes, stack overhead and final state, keeping stack, fuel and
    state separate.
  - Preserve only general installed-entry, publication-authority and checked-
    `lidt` joins; do not recreate the deleted compiler-side table membership,
    dispatch or invocation-stage model.
  - Implement fatal halt, timer/controller setup, tick recording and wake.
    **BOUNDED-INSTALLATION-REACH-ROWS** / **TOP-LEVEL-BOUNDARY-REQUIREMENTS** own
    acknowledgement lineage; integrate **UEFI-PHYSICAL-SEMANTIC-ENTRY** /
    **UEFI-OS-HANDOFF** and **TR3-TR8** provisioning with Cathedral's QEMU
    harness. Keep interrupts disabled until the exception floor exists.

  Acceptance: QEMU reports ticks through owned output, halts between ticks and
  routes a deliberate fault to its dedicated-stack fatal handler without
  resuming ordinary work. Missing members, wrong writer/realization, changed
  bytes, replayed publication/acknowledgement, forgotten/double completion and
  retirement while the table names a root reject. A source guard allows exact
  target-owned `lidt` encoding and relocations but no compiler-owned member
  roster, vector policy, dispatch decision or enter/body/exit state.

- **BOUNDED-INSTALLATION-REACH-ROWS.** Complete
  [installation-bound reach](wiki/spec/build/external_roots.md#installation-bound-reach)
  for the shipped acknowledgement in `source/library/core/interrupt.omg`:
  `InterruptAcknowledgement::complete` still declares `reaches PortIo`; migrate
  it to `reaches <= MachineControl + PortIo` with a selected satisfier that
  consumes the linear receiver. **TOP-LEVEL-BOUNDARY-REQUIREMENTS** owns the
  boundary-trait migration and selected dispatch;
  **CHECKED-ASM-LABEL-CONTRACTS** owns checked target discharge. Reuse
  `omega-rust/omega/compilation/realization/provider-planning/src/provider_planning/installation_reach.rs`
  and carry the exact completion route through Terminal, selected-provider and
  installation evidence. Component deployment stays with **COMPONENT-SUBSTRATE**.

  Acceptance: PIC completion selects `PortIo` and LAPIC/x2APIC
  `MachineControl` through the shipped requirement, Terminal Psi and
  `InstalledInterruptCompletionRoute`, with complete nested substitution
  including service unions across resolution passes. Cross-provider settlement
  despite equal rows, replayed token/era, unresolved Independent exports and
  unresolved final-admission rows reject. Conservative bounds grant no
  authority.

## Parallel language and compiler lanes

- **IMPLICIT-STORE-CONVERSION.** Finish the implicit-conversion rejection: one
  diagnostic per authored conversion and a migrated corpus, under
  [numeric carrier rules](wiki/spec/language/numeric_values.md) and
  [explicit coercion/erasure](wiki/spec/language/domains.md#exact-coercion-and-erasure).
  - `check_narrowing_assignment` in
    `omega-rust/psi/compilation/checking/src/proof_contracts/arithmetic_domains/assignments.rs`
    already rejects typed carrier/policy changes and names `as`, even when the
    value fits. Widening, signedness, float-format and policy add/drop all stay
    rejected; anonymous literal landing stays distinct. Exact `as` requires
    representability; diagnostics must not present it as unchecked truncation.
  - Resolved fixed-integer value deliveries leave arithmetic-policy refusal to
    the shared carrier check, including local/field/array stores, arguments and returns;
    `fail/arithmetic/implicit_integer_policy_erasure_rejected` pins one
    conversion report per occurrence. Semantic/provenance atoms, reference
    qualifications and float policy still retain their own obligations.
    Remaining: diagnostic cascades after a primary type, suffix or
    unresolved-name error and duplicate non-integer or unresolved deliveries.
    Coordinate
    `omega-rust/psi/compilation/checking/src/program_validation/statements/`,
    `omega-rust/psi/compilation/checking/src/machine_calls/calls/argument_bounds.rs`
    and `omega-rust/psi/compilation/checking/src/proof_contracts/domain_weakening.rs`.
    Reproduction: `fail/arithmetic/implicit_store_call_result_conversion_rejected`
    reports an arity error, then an overflow, then the conversion. Keep real
    primary subjects and independent errors; do not blanket-suppress domain
    obligations.
  - Float-policy customers: revalidate the reported Windows named-operator
    admission failure; the saturation, overflow-trap, propagated-NaN-trap and
    two-rounding multiply-then-add oracles now execute correctly on macOS ARM64.
    Primitive Float adapters retain evaluated operand policies, while ordinary
    named calls obey their declared parameter and result policies. Preserve
    that distinction when deduplicating diagnostics with
    **OPERATOR-BOUNDARY-CALLS** / **FLOAT-PROVIDERS**; an `F32`/`F64` namespace
    never supplies an adapter. Run `python tools/corpus_gate.py --native --filter
    float_saturating_arithmetic_exit,float_trapping_overflow_traps,float_trapping_propagated_nan_traps,named_provider_multiply_then_add_exit,runtime_named_policy_result_exit`.
  - Migration: every cross-carrier/policy store in maintained pass fixtures,
    samples (**SAMPLE-CORPUS**), bundled libraries and `source/omega/main.omg`
    with `source/psi/{lex,parse,source}` spells its conversion, preserving each
    operation's algorithm and policy. Checked admission or a later lowering
    refusal is not native success.

  Acceptance: `tests/omega/fail/arithmetic/implicit_store_{conversion,widening,call_result_conversion}_rejected`
  and `implicit_call_operand_conversion_rejected` each report one actionable
  conversion error. Keep `implicit_call_result_policy_change_rejected` refusing
  implicit policy additions independently of result bounds and `ensures`;
  `runtime_explicit_call_result_policy_exit` preserves explicit policy changes,
  anonymous literal landing and declared qualified results (exit 0 in both engines).
  The shared carrier check in
  `omega-rust/psi/compilation/checking/src/proof_contracts/arithmetic_domains/operand_reports.rs`
  keeps treating declared types as types, not range facts; heterogeneous declared
  operators, mixed-integer comparisons and independently typed shift counts stay
  valid. `python tools/corpus_gate.py --both --filter runtime_explicit_call_operand_policy_exit,runtime_shift_count_domain_exit,runtime_comparison_signedness_exit,runtime_wrapping_operand_truncation_exit`
  exits 70 native and interpreted; `tests/omega/pass/slices/runtime_saturating_array_element_guard_exit`,
  explicit exact casts and Wrapping whole-array literals keep working; migrated
  negative fixtures still reach their intended obligation (for example, argument
  widening does not reinterpret a saved Wrapping computation).

- **MATCH-SELECTIVE-LOWERING.** Lower [value dispatch](wiki/spec/language/patterns.md)
  for owned, borrowed and linear results, structural/case/domain patterns and
  coverage through to native execution. Preserve once-only subject evaluation,
  authored first-match order, selected-arm-only execution, exact result/loan
  origins and death edges.

  Owners: `omega-rust/psi/compilation/checking/src/value_custody/expression_types/{match_dispatch,result_type}.rs`,
  checked value continuations, Terminal production, and package review's
  `omega-rust/omega/packages/review-evidence/src/capture/contracts/expressions/projection/`.

  - Owned results: carry affine custody through branch-local calls/receivers and
    effectful subjects; projected fields inside record-construction arms,
    heterogeneous residual custody and claim-bearing body plans. Ordinary
    projected-child selections and literal/folded index moves already lower; do
    not reopen folded-index planning.
  - Borrowed joins: reuse
    `omega-rust/omega/compilation/representations/abstract-operations/src/abstract_operations/control_flow/address_joins.rs`
    (shared primitive/plain aggregate and mutable whole-record carriers exist).
    Still missing: primitive/projected exclusive arms, write-only joins
    (**WRITE-ONLY-BORROW**), case/reference/descriptor-bearing referents,
    runtime-sized element-view selections, ranges, computed roots, join-place reads/projections,
    primitive-local origins and borrowed-result successor lifecycle. End loans
    at last use: `omega-rust/omega/compilation/representations/abstract-operations/optimization-unit/semantics/src/current_ownership/address_joins.rs`
    currently pins origins through the whole dominated region.
  - `branch_edges` in
    `omega-rust/omega/compilation/pipeline/03_target-operations-to-selected-instructions/src/legalization/scalar_graph_input/control.rs`
    requires one Psi edge identity per successor, including for scalar matches;
    carry combined Psi rewrite provenance instead.
  - Instantiate composite/constrained result shells and exact static arguments
    for indexed predicates/theorems and package memberships, keeping
    qualifications, routed membership, callable signatures and erasure. Bare
    operator `-> T` binding exists; dependent shells cannot yet export
    declaration-local identities.

  **OPERATOR-MACHINE-SUPPLY** owns operator execution, **CRASH-CONTRACT**
  crash-qualified equality, **STATE-LOCAL-VALUE-FRONTIER** numeric landing.
  Generalize place/loan establishment across matches, locals and calls; do not
  grow an arm-expression roster or flatten ownership into statement-wide moves.

  Acceptance: add corpus run fixtures for owned call-product arms, interleaved
  and projected sources, linear child carriers and borrowed joins, each running
  natively with interpreter agreement; cover effects, skipped trapping arms,
  overlapping patterns and full coverage, and the
  [float Match customers](wiki/drafts/reference/float_realization.md#operation-and-control-custody).
  Keep `tests/omega/pass/expressions/{match_anonymous_result_landing,numeric_operand_destinations}`
  and `tests/omega/pass/collections/runtime_dutch_flag_partition_exit` (exit 70,
  copyable swaps). Wrong qualifications, ownership, result types and
  incompatible arms reject before an outer bare-carrier cast.

- **OPERATOR-MACHINE-SUPPLY.** Run calls to declared operations and their
  premises through to native execution under
  [declaration-owned executable supply](wiki/spec/language/expressions.md#executable-supply),
  [call preconditions](wiki/spec/language/machines.md#call-preconditions) and
  [licensed normalization](wiki/spec/proofs/contracts.md#licensed-normalization).
  Existing calls reaching native execution come first; broader algebra and
  retiring the `operator` introducer follow.

  - Call-containing scalar requirements: the scalar graph route that refused
    a clause such as `requires observe(left) == observe(right)` on a call
    `restricted(saved, value)` (`covered_requires`, `graph_preparation.rs`)
    is deleted; re-probe where the Unit route now refuses it. Lower it with interpreter/native agreement
    against a clause-free control. Do not drop premises or execute the callee in
    place of admission.
  - Formation checks in
    `omega-rust/psi/compilation/checking/src/proof_contracts/contract_entailment/specification_calls.rs`
    and `omega-rust/psi/compilation/pipeline/04_typed-trees-to-checked-trees/src/checks/contracts/evidence.rs`:
    extend to static callable/evidence arguments and generic dispatch, calls in
    abstract signatures, domain/default predicates, receiver/projection terms
    and citation/induction guarantees. Abstract callee attribution and nominal
    named-evidence calls exist. A supplied witness does not discharge Boolean
    premises; results and guarantees are formed only after admission, including
    erased/discarded uses. Keep constructor values distinct from tag-only facts.
  - Extend `omega-rust/psi/compilation/pipeline/03b_typed-trees-to-typed-trees/src/operators/call_binding.rs`
    beyond binary/indexed expressions to open-ended ranges and body-bearing
    match equality (the floating requirement comparison does not execute a
    selected body). Keep [Equatable's sealed type-owned route](wiki/spec/language/conformances.md#core-equality-acquisition):
    `machine == Card::same(..)` must not override it through visibility. Reuse
    ordinary calls, receiver loans and once-only operands; no operator
    interpreter, satisfier search or fallback arithmetic.
  - Extend `OpenIndexOperationSelection` in
    `omega-rust/psi/compilation/checking/src/value_custody/type_references/open_index_expressions.rs`
    with distributivity and zero/one law evidence. Commutativity and
    associativity already license separate rewrites and grant no identities.
    Source-produced selections must survive
    `monomorphization::finalize_domain_instance_identities`. Missing,
    ambiguous, wrong-operation and unproved-law selections reject;
    noncommutative operations stay usable.
  - Migrate library/corpus/embedded declarations off the `operator` introducer,
    then remove legacy parser/IR consumers and undifferentiated
    `MachineSupplyMode::Boundary`. The
    [retirement inventory](wiki/drafts/audits/operator_introducer_retirement_inventory.md)
    is navigation, not a census. Tokenless boundary migration depends on
    **TOP-LEVEL-BOUNDARY-REQUIREMENTS**. Keep ordinary trait token heads,
    domain-homed top-level machines, `SpelledOperator`, provider planning,
    build-time execution and result-domain dispatch.

  Acceptance: `tests/omega/pass/expressions/declared_operator_match_result/`
  (`select_true` and `by_name` produce `260u64` from wrapped `250 + 10`;
  `select_false` produces `1` without invoking the operation; `Main` exits 70)
  runs natively with interpreter agreement on every host. Add fixtures for
  generic/stateful bodies, private helpers, qualifiers, ordered operands and
  receiver/result loans. Missing bodies, bodyless-plus-satisfier supply,
  duplicate shapes, foreign primitive-family injection and forged primitive
  identity reject; unrelated imports cannot alter selection. Runtime,
  compile-time and proof calls owe the same premises (Nat order requirements,
  predecessor-at-one acceptance, missing-premise rejection). Trait supply,
  **FLOAT-PROVIDERS** and compiler primitives keep their separate routes.

- **MODULE-NAMESPACE-RESOLUTION.** Complete exact declaration selection through
  checking, constant evaluation and publication under the
  [module/name contract](wiki/spec/language/modules.md).

  - Default builds and anonymous owners
    ([spec](wiki/spec/build/declarations.md#default-builds-and-anonymous-owner-identity)):
    `omega-rust/omega/packages/compilation-inputs/src/source_imports/frontier.rs`
    leaves loose roots without package identity. Derive the versioned anonymous
    content record after the one Build activation's dependency requests join and
    before product source compilation. Use an invocation-private activation
    identity only for Build state/continuations; never publish it as a declaration
    owner. Keep supplied package/core owners;
    deduplicate dependency nodes while keeping separate host/target build
    instances. A missing build uses default configuration, with no catalog,
    host-path identity, ambient authority or special `main` discovery. Carry
    owner/declaration provenance through specialization and artifact
    encode/reload. Named-only OCREQ v1 cannot encode anonymous owners; local
    repair need not wait for the
    [standalone-request extension](wiki/spec/build/compiler_request.md).
    Corpus inputs already supply package identities, so
    `pass/control_flow/guarded_leaf_branch_expansion`,
    `pass/text/runtime_alias_string_write` and
    `fail/build/package_local_calling_copy` do not exercise the loose route.
    Open: what the negative should pin once loose roots carry owners (for
    example, a policy built from copied types must not select as `std::calling`).
  - Direct-import runtime acceptance: the supplemental no-Console
    `pass/modules/direct_file_import_process_exit` constructs the directly
    imported `A`, observes its value, and exits 7 in native and Terminal
    execution. The original Console witness remains pending:
    `tests/omega/pending/modules/direct_file_import_exit` retains native and
    interpreter exit 7. Reviewed fixture compilation stops at missing portable
    target-family contracts for Console byte operations, also reproduced on
    the base compiler. On the Linux cloud sandbox, `omega run --both` stops
    earlier at private cache ancestry custody (`/` is owned by `nobody`).
    Resume the witness after those existing dependencies are resolved.
  - Indexed constraints and operator homes through typed/checked/Terminal:
    extend normalization and exact declaration selection in
    `omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/preparation/`,
    keeping specialization, competing declarations, visibility and
    nontransitive file-local imports. Reuse `CaseMembership` and symbol-based
    multiplicity matching.
  - Declaration evaluation in
    `omega-rust/psi/compilation/compile-time/src/const_evaluation/const_initializers/`
    and `omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/preparation/generic_data/const_evaluation/{facts,values}.rs`:
    specialized provider/target applications, exact authored NaN bits,
    nested array/case and whole-aggregate subjects, open applications and
    abstract-carrier constraints over explicit named subjects. Keep alias-owned
    bounds/index tuples
    through `omega-rust/psi/compilation/checking/src/proof_contracts/domains.rs`.
    Computed payloads cannot establish qualifications. Pending leaf collection
    still requires completed declared geometry: `const SIZE:u64=1+0; const
    SOURCE:[u64;SIZE]=[3+0]; const ALIAS:[u64;1]=SOURCE; machine main(){}` stops
    at `array extent constant is not a completed integer literal` on Linux
    x86_64. Complete the exact extent dependency before collecting computed
    leaves in `constant/initializer_leaves.rs`; never observe a provisional
    layout as an evaluated dependency. Module-owned public arrays with computed
    extents hit the same earlier fence in `preparation/module_normalization.rs`.
  - Constant invocations share **CRASH-CONTRACT** / **OPERATOR-MACHINE-SUPPLY**
    premise transport through
    `omega-rust/psi/compilation/pipeline/04_typed-trees-to-checked-trees/src/facts/crash_entry_values.rs`.
    Beyond lossless widening and proven indices, extend only with exact live
    evidence; value-changing casts, call-produced projections and unknown
    origins keep conservative crash ceilings.

  Acceptance: `tests/omega/pass/modules/{qualified_declarations,qualified_constants,match_constant_indices,nominal_constant_bodies,module_array_constant_indices,runtime_nominal_constant_lexical_ownership_exit,runtime_foreign_generic_scoped_constant_exit,runtime_qualified_static_mutation_exit}`
  and `python tools/corpus_gate.py --filter modules/` stay green. Anonymous
  owners: one std owner in a diamond; distinct root/core/library owners; stable
  identity after relocation, rebuild and cache deletion; changed semantic inputs
  change identity; equal-content named lineages stay distinct; missing or
  mismatched provenance and real visible-name conflicts reject. Add fixtures for
  the new constant forms preserving exact float bits/signed zero, array/case
  projections, non-Exact record fields and operator widths/policies, all-arm
  obligations without evaluating skipped subjects, and the stale-operand, wrong-owner,
  false-premise, invalid-unused and unproved-index rejections. Runtime qualified
  `Holder<T>` reads and general array execution stay with
  **STATE-LOCAL-VALUE-FRONTIER** / **TR3-TR8**.

- **RUNTIME-VALUE-GENERICS.** Support runtime-capable value binders on data
  ([value binders](wiki/spec/language/generics.md#value-binders-and-const-requirements),
  [runtime index identity](wiki/spec/language/generics.md#runtime-index-identity-and-storage))
  for applications, results and stored qualifications. Machine value binders
  already pass each subject as an ordinary argument to one shared body. Start
  with fixed-representation data; **FINITE-GENERIC-DISPATCH** owns finite
  families.

  - Admission and construction binding: data value arguments still go through
    `validate_const_data_argument`
    (`omega-rust/psi/compilation/checking/src/value_custody/type_references/generic_arguments.rs`),
    and `omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/lowering/data.rs`
    synthesizes descriptor fields unconditionally. Work through
    `omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/preparation/generic_data/`
    and stage 04 `monomorphization/`. Executable indices are ordinary data,
    arguments or descriptors; erase proof-only uses only after discharge.
  - Identity: shared layout/code does not make applications compatible. For
    `data Index<Limit: u32> [copy] where value < Limit, { value: u32; }`, an
    `Index<9>` holding 8 cannot pass as an `Index<7>` argument or field.
    Reassignment cannot retag; equality admits only what it proves.
  - Forwarding: a forwarded generic result type still exposes an unsubstituted
    callee type in the caller's inferred hoist. Domain/index qualification
    forwarding needs live/stale guard controls. Legacy range-shell fixtures
    migrate through **CANONICALIZE-SCALAR-RANGE-CONTRACTS**; module-owned forms
    through **MODULE-NAMESPACE-RESOLUTION**. Runtime inline-array extents and
    const positions stay rejected; runtime values are never static cache keys.
  - Execution of value-indexed data/storage integrates
    **STATE-LOCAL-VALUE-FRONTIER** and **ENTRY-CONTENT-ROOTS**.

  Acceptance: `<Count: u32>` accepts static and runtime arguments under proved
  obligations; `<const Count: u32>` stays static; one body handles distinct
  counts; parameter/result/field qualifications survive reassignment. Add
  fixtures that run value-indexed data natively with interpreter agreement and
  that reject invalid construction, bounds, stale/mismatched indices,
  static-only uses and duplicated/lost linear custody. Keep
  `tests/omega/pass/generics/{runtime_value_generic_subject_exit,runtime_forwarded_value_contract_exit}`
  running on every bound host. No hidden boxing, maximum-range stack
  reservation, per-value code generation or specialization inferred from
  integer finiteness.

- **STRUCTURAL-GENERIC-MATCHING.** Complete
  [static type equality](wiki/spec/language/generics.md#static-type-equality)
  beyond closed nominal name equations,
  [structural equations](wiki/spec/language/generics.md#structural-type-equations-and-inference)
  and [canonical domain indices](wiki/spec/language/generics.md#canonical-domain-index-matching)
  through specialization, layout and execution. The Std-backed customer is
  `tests/omega/pass/generics/omitted_data_binder_range_equation` (infers
  Capacity = 256 from `u64::AtMost<256>`, authored native/interpreted exit 92);
  its std checking shares the Console loop-ranking dependency under
  **PSI-TARGET-FAMILIES**. Reuse
  `omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/preparation/type_equations/`
  and `.../preparation/machine_equations.rs`; array, anonymous reference/slice
  and integer/Boolean application matching exist, including source-selected
  constructor constants with declared-carrier custody and closed named actual
  array extents with source selection retained through instance identity.
  Closed instances substitute selected extent fields and composite generic
  field arguments recursively while preserving lexical constructor-head
  dependencies. Ordinary type binders cannot supply generic constructors;
  const and runtime value binders cannot supply type positions.

  - Combine equations with argument/result inference and keep obligations
    through forwarding beyond closed scalar constructor indices, retained
    extensions, late-selected receivers, operator/conformance supply and
    machine/evidence/value binders. Machine preparation still requires closed
    explicit calls; keep unsolved-equation
    rejection until each route discharges them. Scalar return-domain inference
    from a typed parameter is separate.
  - Add named-lifetime identity and remaining const-index kinds;
    **WRITE-ONLY-BORROW** owns write-only admission.
  - Check unspecialized machine/requirement bodies under every admitted
    type-equality alternative, including finite disjunctions and branch-local
    equalities that cannot escape joins.
  - Extend the typed const-expression and invocation evaluators to selected
    computed calls and structured const value/equation combinations,
    keeping carrier, policy, selected operation and
    canonical identity; equivalent closed computations select one instance.
    **ARITHMETIC-POLICY-REALIZATION** owns trapping-call exclusion,
    **RUNTIME-VALUE-GENERICS** runtime subjects, **STATE-LOCAL-VALUE-FRONTIER**
    storage, **CANONICALIZE-SCALAR-RANGE-CONTRACTS** range-shell migration.

  Acceptance: fixtures where data/machine array and application equations infer
  omitted arguments and run. Repeated/explicit conflicts, missing indices,
  occurs cycles, kind mismatches, false constructor predicates and unmet
  conformances reject. Open indices match as whole expressions (no general
  equation solving or predicate maximum search); distinct indices keep identity
  without implicit variance; flow bounds never choose capacity, specialization
  or layout.

  The Std-backed `tests/omega/pass/generics/runtime_generic_carrier_equation_exit`
  remains open: its unchanged native/interpreted exit 7 needs an accepted
  `linux_x86_64::ProgramEntry` binding. Normal
  `omega update --project tests/omega/pass/generics/runtime_generic_carrier_equation_exit --target linux_x86_64`
  exceeded the measured 600-second bound before emitting a review document.
  Preserve entry-review authority. The Std-backed
  `omitted_data_binder_range_equation` also rejects during source checking on
  Linux: it shares the Console loop-ranking dependency under
  **PSI-TARGET-FAMILIES**. Preserve ordinary std package admission.
  The bounded package-free
  carrier repair does not satisfy this original Std-backed acceptance.

- **FINITE-GENERIC-DISPATCH.** Select a finite generic instance at runtime under
  the [finite specialization contract](wiki/spec/language/generics.md#finite-specialization-boundary)
  and [dynamic method families](wiki/spec/terminal-psi/dynamic_dispatch.md#finite-generic-method-families),
  for scanner widths and runtime-selected datatype providers.
  `finite_signature_family` (typed trees) owns the roster;
  `omega-rust/psi/compilation/pipeline/04_typed-trees-to-checked-trees/src/monomorphization/dynamic_families.rs`
  specializes selected providers. Static tuple selection, correlated
  multi-binder rosters, boundary family coverage and Terminal tuple identities
  exist. `tests/omega/pass/traits/dyn_finite_family_dispatch` has no native
  entry and discards its results.

  First customer: select width 16/32/64 from a runtime value through one named
  conformance, with distinct observable results through rebinding and
  forwarding; one scalar binder, a common result, dispatch around a region-sized
  operation; no handwritten suffix methods. Demonstrated by
  `tests/omega/run/dyn_finite_family_runtime_dispatch` (native and
  interpreter exit 70; roster written 64,16,16,32 to pin deterministic
  normalization of alternative order and duplicates),
  `tests/omega/run/dyn_finite_family_owned_carrier` (a linear Packet moves
  once into a unit-result family call through a `&mut` subject, native and
  interpreter exit 70) and `tests/omega/run/dyn_finite_family_correlated`
  (two-binder correlated tuples (16,4)/(32,8), native and interpreter exit
  70), plus fail controls
  `tests/omega/fail/traits/dyn_finite_family_unproved_membership`,
  `.../dyn_finite_family_wrong_tuple` and
  `.../dyn_finite_family_narrowed_provider`.

  - Runtime roster membership: `validate_family_tuple`
    (`omega-rust/psi/compilation/checking/src/machine_calls/calls/dynamic_requirement.rs`)
    admits a complete tuple of closed constants or ordinary runtime subjects
    (RUNTIME-VALUE-GENERICS' `Value`-binder shape); immutable integer machine
    and exact caller-state parameters supply Requires atoms
    (`proof_contracts/contract_entailment/static_applications.rs`) so the
    signature's where-disjunction proves membership at the call's discharge.
    A closed non-member tuple defers to the roster diagnostic ahead of the
    generic where judgment (`static_arguments/requirement_predicates.rs`);
    unproved membership rejects; no JIT or fallback.
  - Finite-family dispatch rides the ordinary implementation-selector
    indirect-call mechanism: `TerminalDynamicSlot` commits one canonical
    tuple per slot (`dynamic_dispatch.slots`), `CallDynamic` carries the
    evaluated `family_actuals`, 05 expands the call into authored
    conditional/test/leaf blocks ending on per-tuple indirect calls, and
    interpreter/native execution resolves the row from those slots. Tuple
    tables are source-produced from the named conformance's roster.

  Frontier: escaping results whose type mentions a family binder, e.g.
  `-> [u8; Width]`, need an authored sum or descriptor with explicit ownership.
  A common concrete result, including an ordinary sum, uses structural block
  parameters and per-leaf publication; regression fixtures are
  `tests/omega/pass/traits/runtime_finite_family_*_result_exit`, with qualified
  linear, affine-cleanup, correlated-case and named-state forwarding coverage.
  Borrowed descriptor results still need a public slot-relative returned-loan
  origin contract; do not infer one from the selected implementation.
  Structural field reads rooted in entry content claims retain their separate
  target-stage restriction. Qualify target-ineligible bodies and const-only
  inputs without a checked bridge. Compile/code-size and dispatch-placement
  measurements against explicit branches remain before any improvement claim.

  Acceptance: pass/fail/run fixtures preserve effects, index identity and
  once-only moves/cleanup through selection, forwarding and storage;
  alternative order/duplicates normalize deterministically; correlated tuples
  stay complete without range enumeration. Missing, wrong-width, mixed-provider
  or shape-substituted rows, unproved membership, target-ineligible bodies and
  const-only inputs without a checked bridge reject. Escaping variable-shaped
  results need an authored sum or descriptor with explicit ownership. Measure
  compile/code size and dispatch placement against explicit branches before
  claiming improvement. No implicit boxing or general generic virtual methods.

- **DOMAIN-ISSUER-ROUTES.** Execute domain issuance, wrapper forwarding and
  consumption with an observable result under
  [requirement, exact-machine and boundary establishment](wiki/spec/resources/authority.md#requirement-and-exact-machine-routes),
  for scalar and structural carriers, private issuers and authorized selected
  payloads. `tests/omega/pass/domains/bodyless_owner_establishment` checks an
  issuer/forwarder but binds no executable entry.

  - Lowering: in
    `omega-rust/psi/compilation/pipeline/05_checked-trees-to-lowered-psi/src/unit/attached_unit.rs`
    ordinary units publish no conformance applications, and
    `.../src/unit/attached_unit/catalog.rs` binds result establishments only for boundary requirements with
    an exact `CallEnsures` receipt. Keep ordinary callee/conformance identities
    and bind the checked qualification to the exact invocation result, for
    ordinary and boundary calls. Reuse the structural/scalar route catalogs.
  - Apply the [qualification rule](wiki/spec/language/domains.md#declaration-and-membership)
    to copyable, affine and linear carriers. Remove the
    `domain_target_is_custody_marked` heuristic from
    `omega-rust/psi/compilation/pipeline/04_typed-trees-to-checked-trees/src/facts/index_compatibility.rs`;
    unrelated domains must not restrict a route-free tag. With
    **BUMP-ALLOCATOR-CANARY**, give `source/library/core/extent.omg`'s bare
    `Vacant`/`Resident` real custody routes and migrate
    `tests/omega/fail/memory/bump_allocator_cast_minted_vacant` off the
    heuristic. Keep the `Region::Left`/`Right` linear move and vacuous scalar
    tags.

  Acceptance: `bodyless_owner_establishment` (or a sibling run fixture) binds an
  entry and runs issuer -> forwarder -> consumer natively with interpreter
  agreement. Checking proves carrier, predicate and custody obligations without
  assuming the introduced qualification. Missing, forged or rebound evidence,
  same-spelled/module/package issuer substitution, casts to protected
  declarations, unevidenced calls and direct calls borrowing admitted
  requirement authority reject. [Private catalog](wiki/spec/resources/authority.md#private-issuer-routes)
  identity grants no consumer access to calls, conformances or private types;
  public requirements still admit valid downstream conformers.

- **TWO-AXIS-TERMINAL-AUTHORITY-REVIEW.** Produce reviewed native artifacts for
  `tests/omega/pass/host/console_exit_process_exit`, Cathedral native smoke,
  `samples/cli/basics/cli_mvp` and Squalr without receiver input, under the
  [production/admission split](wiki/spec/build/permissions.md#artifact-production-versus-receiver-admission),
  and keep receiver admission of authority as a separate optional check.
  Coordinate **SAMPLE-CORPUS** / **SQUALR-HEADLESS**. Package acceptance must not
  manufacture receiver approval; no receiving policy means no admission claim.
  The corpus native leg records `console_exit_process_exit`'s result; it must
  print its console line and exit 70.

  - Implement [critical service audit policy](wiki/spec/packages/review.md#critical-service-audit-policy):
    core's exact critical service catalog plus root `Build::audit_service<R>`
    selections, not blanket danger for provider-interface names. Carry policy
    inputs and exact critical findings through install/update review and compact
    lock acceptance; unregistered reach stays in the full report. Reuse existing
    comparison and physical-authority classification. Test a checked pure float
    provider (no new service effect), direct/wrapped filesystem write (always
    surfaced), custom MotorControl before/after root registration, and same-spelled
    foreign declarations. A dependency cannot suppress the core set, self-approve
    findings or gain authority; changed root policy cannot reuse stale acceptance.

  - Linux x86-64 filesystem: `omega-rust/omega/compilation/realization/provider-planning/src/provider_settlement/canonical_filesystem_host.rs`
    mints positional syscall rows, and Linux calling plans exist, but no direct
    syscall realization or admitted `ToolchainSettled` execution variant
    exists. Drive a real `set_len` call through settlement, physical
    selection and encoding, settling each demanded `BoundaryCall` once with its
    argument/clobber and provider-plan identities. Unsupported targets or
    leaves needing unimplemented path/slice or host-error adaptation reject.
  - Receiver containment: the direct and retained paths call
    `omega-rust/omega/compilation/pipeline/00_terminal-psi-to-abstract-operations/src/provider_admission/terminal_authority_permissions.rs`
    to require receiver rows equal package permissions. Replace with
    demand-based containment: a sufficient policy with differing permissions or
    surplus rows admits; undemanded package rows do not matter.
  - Exact [filesystem classifications](wiki/spec/build/permissions.md#portable-filesystem-control-and-lifecycle-authority):
    cohort classifications emit from
    `omega-rust/omega/compilation/pipeline/00_terminal-psi-to-abstract-operations/src/provider_admission/settlements/source_imports.rs`;
    `omega-rust/omega/packages/package-manager/src/review/candidate/semantic_bindings.rs`
    supplies permission proposals; keep their identities separate. After exact
    replacement, remove the broad filesystem arm in
    `omega-rust/omega/packages/review-evidence/src/capture/authority.rs` and its
    rendering in `omega-rust/omega/packages/package-manager/src/review/audit/triage/`.
    **FILESYSTEM-RELEASE-CONTRACT** owns occurrence-bound close evidence.

  Acceptance: all four customers produce without receiver input and run.
  Denying policies reject; sufficient policies admit and keep the supplied
  identity, including surplus rows. Every demanded leaf has exactly one
  mechanism/contract classification within its permissions; unknown,
  duplicate, conflicting, cyclic or substituted classifications and violated
  physical exclusions reject. Explicit empty classes, service reach and review
  identity stay. Core-critical and root-registered service findings survive
  wrappers and install/update lock round trips without granting authority;
  unregistered reach remains inspectable. No synthesized receiver-policy file.

- **FILESYSTEM-PLATFORM-SERVICES.** Implement the remaining portable
  [platform services](wiki/spec/build/platform_services.md) providers on
  Windows x86-64 and Linux ARM64. Portable `FilesystemService` has one routed
  slot; POSIX algorithms are shared, while each target owns its layouts,
  flags, errno mapping and raw provider closure. Do not introduce target-varying
  portable data, raw-family routing fields or hidden source values.

  Remaining acceptance:
  - Supply complete target implementations, preserving zero handles as absent,
    zero OpenOptions as defaults and closed failure results. Windows stores
    HANDLE; POSIX stores descriptor + 1, preserving descriptor 0.
    Windows ownership, duplication, file permissions, data synchronization
    and unsupported operations need explicit portable behavior, not POSIX
    conventions or sentinel implementations.
  - Run `file_journal` (exit 7), `note_vault` (exit 14) and applicable
    filesystem fixture oracles on the remaining hosts. Linux x86-64 retains
    its portable provider; the moved shared algorithms and Linux-only
    `write_all_full_device_exit` need Linux execution.
  - Execute the Windows raw breadth and set-file-time fixtures on Windows
    x86-64 (exit 70). Foreign artifact production does not establish those
    outcomes. Preserve oversized-read, asynchronous-handle, invalid UTF-8 and
    failed-time-query controls.
  - Remove unnecessary 4096-byte staging in raw read-link, directory,
    metadata and timestamp adapters while preserving native layout, alignment,
    minimum storage and separate NUL-terminated path construction.
  - `native_metadata_ino` still reaches the proof-search budget
    (`proof_search_result_guarantee_work`);
    **CHECKER-CARRIES-OPERATION-DECISIONS** owns that prerequisite.
  - Retain every nested raw requirement/provider identity in artifact/cache
    identity, cycle detection, review, authority evidence, replacement admission
    and live-era custody. Missing, ambiguous, cyclic, wrong-target, stale or
    unadmitted closure members reject. Replacing a nested raw provider rechecks
    the complete portable closure.
  - The checked interpreter's generated host-operation roster and bare-name
    fallback retire through the universal cutover, not another legacy
    dispatcher. Terminal execution still needs trusted implementations for
    the exact authored foreign bindings.

- **FILESYSTEM-RELEASE-CONTRACT.** Run
  `tests/omega/pass/filesystem/windows_canonicalize_exit` on Windows:
  canonicalize a file and a directory, keep missing-path `NotFound`, clean up,
  exit 70; and derive the occurrence-bound close proof that lets a proved
  close carry empty authority.

  - Current state: Windows rejects with "selected ProgramEntry Binding field
    `Main::fs::host` requires a selected Fused provider for boundary
    `FilesystemHost`". Re-drive the whole program before naming the blocker;
    prerequisites route through **STATE-LOCAL-VALUE-FRONTIER** /
    **NOMINAL-FIELD-FLOW**. Structural field replacement of constructed and
    call-produced values already works through move/store pairs; extend it.
    Keep the authored `UnitResult` stores, case observations, nested error
    matches and buffer handling. The build selects `Main::main`; the
    `FailedOpen` entry has no harness: exercise failed acquisition and show it
    performs neither path query nor close.
  - [Bounded release proof](wiki/spec/build/permissions.md#bounded-occurrence-specific-release-proof)
    from checked program flow: successful acquisition under the exact
    object/argument contract, handle/alias identity, preservation by
    intervening calls, one release on each applicable returning path, no later
    use; bind it to the native occurrence. Helpers in
    `omega-rust/omega/compilation/pipeline/00_terminal-psi-to-abstract-operations/src/provider_admission/terminal_authority_policy/filesystem.rs`
    build contract commitments and classification keys but have no callers. A
    supplied digest, open flags, query-only authority or build execution does
    not prove the lifecycle.

  Acceptance: the fixture exits 70 on Windows. A proved close earns one
  occurrence-bound empty classification; failed acquisition, escape, reused
  aliases, double close, invalidating calls and stale proofs do not narrow
  authority; changed arguments, flow or contracts need new evidence or a
  conservative class. Windows sharing: external pending deletion may complete on
  ordinary close; deletion attached to this handle excludes the empty class.
  `tests/omega/pass/filesystem/native_close` (closes arbitrary descriptor 5)
  stays an unconstrained control. **TWO-AXIS-TERMINAL-AUTHORITY-REVIEW** owns
  permission rows and the broad filesystem-summary replacement.

- **R5.** Carry complete caller-visible may-write summaries through source
  checking under [state contracts](wiki/spec/language/state_contracts.md) and
  [subject preservation](wiki/spec/language/termination.md#subject-preservation):
  a helper that picks either mutable input and writes through its result in a
  value expression must keep a disjoint index-bound fact and invalidate the
  overlapping one, including writes made while evaluating operands.

  Owner: `omega-rust/psi/compilation/checking/src/machine_calls/calls/write_frames/`,
  consumed by stage 04 flow mutation and range/progress checks.
  `CallFrameResolver` already shares call/expression summaries and transition
  fixed points. Proven candidate unions must survive computed receivers, codec
  arguments, nested results, state arrivals and stored-reference rebinds.
  Multi-input helpers need explicit output lifetimes:
  `omega-rust/psi/compilation/pipeline/04_typed-trees-to-checked-trees/src/borrow/view_link.rs`
  keeps matching-input unions, but lifetime correspondence alone gives no
  storage-origin evidence. Unknown origins, private escapes, unproved rebinds,
  unsupported recursive results and loads behind another reference stay opaque.
  A static callee's signature may bound writes when its body summary is
  incomplete; that proves neither returned origins nor termination. Frames never
  grant borrow access. Extend the shared origin/fixpoint owners, not a
  recognizer per reference spelling.

  Acceptance: add a pass fixture for the helper-choice program that checks with
  the disjoint bound and runs its authored result (reference-result transport
  via **MATCH-SELECTIVE-LOWERING**), and a fail fixture where an overlapping
  candidate rejects the stale bound. Add fixtures for nested helpers,
  named-state results, codec arguments and finite write cycles; parameter
  permutations keep finite write sets; `omega --check tests/psi/parser/main.omg`
  still converges.

- **TPR6.** Carry exact progress subjects through calls, state arrivals, private
  recursive components, exported contracts and provider evidence under
  [subject preservation](wiki/spec/language/termination.md#subject-preservation).
  Private ranking witnesses stay out of public identity; lineage transports an
  existing live qualification and cannot establish one.

  Owner: `omega-rust/psi/compilation/pipeline/04_typed-trees-to-checked-trees`.
  `src/checks/termination/progress/{origins,lineage,machine_summaries}.rs`
  recover owned loads, exact callee replacement inputs and some guarded-result
  alternatives; `ParameterLineage::Exact` keeps multiple subjects.
  - `src/flow/value_origins.rs` still traces back to one origin: projected/stored
    helper results and extra reference boundaries need exact provenance with the
    ordinary capture, alias and write-invalidation checks. Extend
    generic/dispatched callee and unresolved projection handling without
    treating generic field traversal as evidence about a callee's body.
  - Closed-array selector enumeration returns alternatives, but
    `machine_summaries::instantiate_call_premise` and
    `src/checks/termination/progress/lineage/transfers.rs` require one subject.
    Carry all finite alternatives into call coverage and state transfers; every
    possible subject must be covered.
  - Keep exact public-schema, local admitted-receipt and build-bound provider
    coverage per demand. Route catalogs are not receipts. **COMPONENT-SUBSTRATE** owns installed
    composition; **STATE-LOCAL-VALUE-FRONTIER** owns nested structural-result
    execution. Retire shape gates in
    `omega-rust/psi/compilation/checking/src/machine_calls/calls/expression_scanning/result_realization.rs`
    only when real producers preserve custody.

  Acceptance: corpus fixtures through full checking for guarded results and
  dynamic selectors with different origins (all covered passes, one uncovered
  rejects); nested wrappers, finite field/parameter permutations and private
  recursive premises; unknown arms, stale aliases, replaced reference prefixes
  and growing projections reject. A call written on the demanded path still has
  no exact origin: may-write information cannot recover overwritten contents.
  Inline record/array carrier results and `select(forward_array(values)[0])`
  run. Calls evaluate once; loans transfer to the selected result rather than
  hang beneath a dying parent; descriptors relocate without copying referents.
  Missing, stale or substituted provenance grants no progress guarantee.

- **TR3-TR8.** Execute ordinary named machines through a selected task runtime
  under [task activation](wiki/spec/build/task_runtime.md) and
  [suspension](wiki/spec/terminal-psi/calls_and_outcomes.md#suspension). Reuse
  `omega-rust/omega/compilation/realization/provider-planning/src/task_plans/` and
  [task-plans](omega-rust/omega/compilation/backend/runtime/task-plans/README.md):
  sealed call graphs, argument layouts, suspension frontiers, stack-demand
  composition and selected-provider bindings exist; execution consumes none
  of the activation sidecar yet, and invocation receipts are not produced.
  The deleted compiler-side lifecycle ledger never executed a task; implement start,
  park/resume, cancellation and settlement in the selected runtime/provider.
  `tests/omega/pass/tasks/task_runtime_machine_selection_compile`
  checks a probe beside an empty, unbound `Main::main`.
  `pass/core/task_lifecycle_operations` produces Terminal; `omega run` refuses
  missing `Task::finish` selection. Its manually constructed Task fields are
  not activation custody; native execution needs a real selected runtime.

  - Stacks: join final physical frames/spills, alignment, entry overhead and
    live chains to the exact `StackPlan` and a real nonmoving backing.
    `task_frame_summary` (`task_plans/stack_graphs.rs`) measures checked locals
    and parked arguments; the activation plan describes shape, not backing.
    Keep unresolved-call refusal and same-stack/separately provisioned demand.
  - Complete incoming-referent and remaining incomplete actual loan-leaf/lifetime
    joins in stage 04 `checks/carry.rs` and stage 05
    `retention/suspension_call_plan.rs`;
    descriptor slots cannot stand in for caller-supplied referents. Complete
    the selected runtime's physical backing and cancellation preservation.
    Keep `pass/concurrency/runtime_retained_reference_boundary_exit` (0) for
    enclosing-invocation roots, a multi-leaf generic carrier with a retained
    parent reborrow, and a write-only reference used after the call. It also
    exercises fresh and forwarded reference actuals and stored/constructed
    two-leaf generic actuals whose backing has no continuation use. Keep
    `runtime_array_loan_suspension_exit` (0) for a lent descriptor array and
    both backing roots, followed by an ordinary runtime-selected referent read;
    this inline-completion control still needs physical park/resume preservation.
    Keep `runtime_returned_carrier_suspension_exit` (0) for declared lifetime
    frontiers through generic returned carriers, nested construction and result
    projection; it also remains an inline-completion control. Keep
    `fail/borrows/generic_result_{keeps_lifetime_source,retains_lifetime_candidates}`
    for unrelated-source rejection and the complete declared candidate union.
    Keep `fail/concurrency/suspend_reference_array_backing_write` for the
    array's retained sibling loan after one leaf is observed. Keep
    `fail/concurrency/suspend_retained_loan_conflicting_backing_write` for
    access exclusion. Checked crossings retain current occurrences and
    parent/supplier closure; continuation and captured-actual backing demands
    close to a fixed point without reviving replaced descriptors. Keep
    `fail/concurrency/suspend_with_indirect_resource_backing`
    for resource floors and indirect backing. The canonical Terminal frontier
    retains formation captures, backing coordinates, accesses and loan links;
    it is a demand, not physical-preservation or completed-restoration evidence.
    Preserve distinct demand occurrences at equal storage coordinates, pinned
    by `pass/concurrency/runtime_suspension_repeated_actual_exit` (0).
    Activation-wide analysis separately treats borrow formation as strict.
    Descriptor `Carry::AcrossSuspend` permission cannot substitute for a
    missing lifetime join, including incoming fields, slices and unknown shapes;
    keep `fail/concurrency/suspend_reference_permission_without_backing`.
    Join the remaining obligations without a reference-type or inline-provider
    exception. Keep
    `pass/concurrency/boundary_suspension_inline_completion_exit` (0) as the
    inline boundary-completion control, not a park/resume witness.
    Keep storage lifetime, pinning, aliasing,
    address stability and cancellation outcome; unsupported frontiers reject.
  - Connect one source-selected provider to argument installation, distinct
    activation, park/resume and returned outcomes, bound to its selection,
    invocation, runtime instance, storage era and task route. CPU/thread
    preservation stays an explicit obligation.

  Acceptance: a run fixture with concurrent observable work, moved arguments,
  start/park/resume/finish and fresh eras on reuse. Rejection returns caller
  arguments/leases; foreign/stale settlement and reclaim with child custody
  reject. Parking establishes no result or cleanup; resumption repeats no
  committed work. Cancellation is a recorded request observed at a declared safe
  point, never unwinding. Runtime custody, backing ownership and linear
  settlement authority stay distinct; continuations stay source-inaccessible.

- **BLOCKEXEC.** Implement `source/library/blocking-executor/` as an ordinary
  package under [task-runtime libraries](wiki/spec/build/task_runtime.md#library-and-foreign-providers)
  and [foreign execution](wiki/spec/build/foreign_bindings.md#operational-coherence).
  `queue.omg` and `executor.omg` hold contracts, not working bodies.
  `tests/omega/pass/blockexec/blocking_executor_custody_claims_compile`
  duplicates the declarations, supplies a pre-existing ticket to `submit`,
  settles against a wait word instead of the executor borrow, and never invokes
  the probe or consumes the package. `tests/omega/fail/blockexec/blocking_executor_ticket_scope_loss`
  checks only linear scope loss.

  - Generic `[T; N]` queue bodies with indexed occupancy, head/length/capacity
    proofs and conserved payloads (`N` is const; runtime value generics are not
    a prerequisite). Combined type/const attached methods go through ordinary
    lowering; use **BORROWED-STORAGE-RESTORATION** for linear slot moves out of
    a borrowed container.
  - Accepted/refused admission and exactly-once settlement with conserved
    claims: acceptance issues a claim bound to the executor instance; refusal
    returns the whole moved submission; close requires an empty queue and no
    live claims.
  - Worker and wait/wake providers through **TR3-TR8**, keeping thread affinity
    and loans (including settlement's executor borrow) across suspension.
    Cancellation requests keep the claim.
  - Bounded hung-worker recovery with real process isolation, exact parent
    endpoint/child manifest, custody and termination evidence. The empty
    `IsolatedWorkerProvider` trait in `executor.omg` proves none of this; never
    kill an in-process worker to reclaim custody.

  Acceptance: a fixture consumes the real package through dependency/build
  selection and runs linear-payload queueing, full-capacity refusal, FIFO
  delivery, park/wake and settlement. Duplicate/foreign/stale tickets, lost
  custody and early close/reclaim reject. Isolated hung-worker recovery has its
  own witness.

- **QUOTIENT-THEOREM-LIFT.** Extend quotient correspondence beyond public
  parameter positions under [lifting operations](wiki/spec/proofs/quotients.md#lifting-operations)
  and the [correspondence judgment](wiki/spec/proofs/quotients.md#quotient-correspondence-judgment).
  `omega-rust/psi/compilation/checking/src/proof_contracts/quotients/relation_plan/terminal_bridge.rs`
  already handles parameter selection/permutation/repetition/omission,
  immutable result aliases and finite state forwarding; reuse it. It is now
  only the admission judgment (`admit_checked_quotient_requests` admits a
  batch the bridge can express) and still builds the old Terminal row shape
  and its `correspondence_certificate.rs` certificate to decide; replace that
  row-shaped output with the normalized judgments as the forms broaden.
  - Its `canonical_lift_runtime_positions` covers only public-parameter
    positions; the bridge requires empty static applications and single
    representative/theorem states and refuses `define` preconditions. Support
    closed literal arguments, preconditioned `define`, result computation and
    generic applications, replacing those source-shape gates with the
    normalized judgments and keeping ordered theorem roles.
  - Package review has no quotient capture (the unused non-executable one
    was deleted); project the broader applications and private
    dependencies under visibility rules when review needs them.
  - Selected theorems stay bodyful, terminating, pure and assumption-free.
    **PROOF-CONTRACT-MIGRATION** owns transitive assumption closure and the
    congruence-only `lift<F, Congruence>` payload.
  Acceptance: package-backed `define` and `lift` with the broader forms pass
  `omega --check`; the existing `fail/proofs/quotient_*` rejections (missing,
  surplus or reversed roles, missing respect/symmetry, admitted/boundary
  theorem closures) still reject, plus new fail fixtures for implicit lifts and
  unproved termination. Preserve executable copy-carrier coverage in
  `samples/cli/proofs/quotient_runtime_carrier` as these forms broaden.

- **PRIVILEGED-PORT-EFFECT-SETTLEMENTS.** Produce checked adapter port I/O
  through ordinary native lowering and
  [consumer-owned settlement](wiki/spec/terminal-psi/boundary_calls.md#consumer-owned-settlement),
  using checked `asm`.
  - Start with an installed adapter owning the exact selected `PortIo`
    requirement and a closed `out`. Checked planning keeps only exact
    `u16`/`u8` constant operands; runtime operands need their own checked
    carrier.
  - `TargetUnitOperation::PortWrite` and x86 encodings exist, but
    `omega-rust/omega/compilation/pipeline/02_abstract-operations-to-target-operations/src/lowering/control_flow/operations.rs`
    has no arm and stage 03 scalar-graph admission refuses the family.
    `omega-rust/omega/compilation/backend/images/image-emission/src/function_fragments/production.rs`
    emits an empty port roster and `function_fragments/validation.rs` rejects
    nonempty ones. Carry service/port/value and operation identity through
    selection, register homes and emission.
  - Reuse `omega-rust/omega/compilation/backend/artifacts/native-artifact/src/physical/derivation/provider_custody.rs`
    for the effect settlement join.
  - `pass/inline_asm/asm_port_out_final_validation` is a direct root and
    correctly rejects for missing provider requirement custody; keep that.
    `asm_runtime_port_msr_final_validation` needs runtime operands (see
    **ASM-INSTRUCTION-CATALOG-EXPANSION**). Port read (`in`) needs its own
    checked/Terminal operation; `DirectPortReadU8` metadata is not enough.
  Acceptance: a new pass fixture with an authored selected adapter emits the
  port write bytes; missing, duplicate, substituted or unconsumed settlements
  and changed service/port/value/target reject; unsupported targets refuse.
  Verify bytes separately from execution; hosted runs never execute raw port
  I/O.

- **RESTORE-DYNAMIC-DESCRIPTOR-AND-TABLE-CUSTODY.** Consume the selector/data
  separation from **DYNAMIC-IMPLEMENTATION-SELECTION-MIGRATION** in Omega
  lowering and native execution under [dynamic dispatch](wiki/spec/terminal-psi/dynamic_dispatch.md).
  Start with a helper that forwards a selector, accepts explicit data arguments,
  and uses the result across calculations and branches. The breaking-migration
  authorization in **NAMED-IMPLEMENTATION-CONFORMANCE-MIGRATION** applies.
  - Remaining: matching-host Windows execution and corresponding
    `source/omega/` implementation. Rust native lowering carries selector,
    public requirement ABI and separate ordinary data operands through the
    shared call, register and image owners; keep that mechanism rather than
    reintroducing a subject or representative implementation signature.
  - Preserve shared Terminal/native result and output agreement with
    `python3 tools/corpus_gate.py --both --filter pass/traits/runtime_dyn_`.
    Extend retained entry/provider/import provisions through the common runtime,
    not a selector-specific interpretation path.
  - Preserve complete 8-byte tables, zero trap slots, exact private callable
    identities and patched image targets. Extend public slot metadata with
    returned-borrow origins and parameter qualification preconditions before
    accepting those contracts. Preserve the shared result shape and typed
    ingress/return claim paths; never infer public facts from a private row.
  Acceptance: `runtime_dyn_rebound_selector_forwarded_exit`,
  `runtime_dyn_two_impl_dispatch_exit` and the other `pass/traits/runtime_dyn_*`
  fixtures run on matching supported hosts, including
  Windows and macOS. Their results and writes to explicitly supplied data agree
  with interpretation. Preserve producer-private selection on returned
  selectors. Run
  `python tools/corpus_gate.py --filter traits/` and affected backend checks.
  A `dyn` selector owns no implicit data; explicit owners retain the ordinary
  [normal cleanup obligations](wiki/spec/terminal-psi/calls_and_outcomes.md#normal-cleanup).

- **TARGET-SEMANTIC-APPLICATIONS.** Make compile-time evaluation consume the
  [target-semantic capsule](wiki/spec/language/evaluation.md#target-semantic-capsule)
  of the selected target, and close package review over
  [operator applications](wiki/spec/terminal-psi/boundary_calls.md#operator-applications-and-physical-children).
  - Package review has no supplied symbolic boundary-application closure
    (the unused one was deleted); a reviewed customer needs one that covers
    the complete reachable specialization closure including const
    arguments. Lifetime and
    non-type producer telescopes still reject. Recheck reach, obligations,
    target facts and admission after substitution; a review demand grants no
    install authority. Keep empty operator applications distinct from absent
    boundary-trait telescopes.
  Acceptance: a fixture folding a target query under a non-host `--target`
  yields the selected target's value; package review over a generic provider
  package reports the full reachable application set, and missing, extra or
  unresolved mappings reject. Keep provider-free evaluation, alternate-provider,
  false-result and unselected-provider controls; build dependency cycles
  reject without guessing a satisfier.

- **TOP-LEVEL-BOUNDARY-REQUIREMENTS.** Retire the standalone `boundary
  requirement Owner::name(...);` declaration kind under the settled
  [machine-supply rule](wiki/spec/language/machines.md#supply). Data is not a
  code owner. Every such operation becomes a requirement in a boundary trait;
  use a one-member trait when it needs an independently selected provider.
  Carrier, authority and token types remain ordinary explicit parameters.

  Migrate `source/library/`, samples and corpus fixtures first, preserving
  exact contracts, generic/lifetime telescopes, reach, domains, suspension,
  blocking, provider selection, external `via` bindings and runtime results.
  Raw platform families become boundary traits selected beneath portable
  implementations. Interrupt acknowledgement, startup confirmation, task,
  executor, device, extent and collection lifecycle operations use ordinary
  named implementations; no hidden receiver or data-subject satisfaction
  returns. Keep one-member traits where grouping would force unrelated rows to
  share selection.

  Then delete the special role through parsing, syntax/resolved/typed/checked
  representations, checking, stages 04-07, Terminal codec, provider planning,
  selected dispatch, component/native artifacts, package review, interpreter
  and `source/psi`/`source/omega`. Converge identity and policy on the existing
  trait requirement/application rows rather than translating old records or
  retaining a second provider-slot kind. The old spelling rejects with a
  migration diagnostic; no compatibility alias, carrier-owned code namespace
  or package-name inference remains. Coordinate ordinary external imports with
  **PROVIDER-SELECTION-AFTER-TERMINAL**.

  Preserve the current valid/invalid distinctions while migrating coverage:
  value, statement, generic/lifetime, borrowed/write-back, external-provider,
  overload, package-review and same-spelled cross-package cases. The
  bare-metal completion customers additionally depend on
  **CHECKED-ASM-LABEL-CONTRACTS** for checked target bodies; task lifecycle
  coordinates with **BOUNDED-INSTALLATION-REACH-ROWS** and
  **COMPONENT-SUBSTRATE**. **OPERATOR-MACHINE-SUPPLY** continues to own the
  separate retirement of `MachineSupplyMode::Boundary`.

  Frontier: the library's 39 caller-free collection, blocking-executor and
  `Vec::close` rows are boundary-trait requirements. Migrate the remaining
  84 std raw platform, 2 interrupt and 126 float rows under
  [explicit boundary reach](wiki/spec/language/effects.md#implementation-boundaries-and-capability-identities).
  Remove automatic containing-trait contributions from checking, call summaries,
  installation substitution, Terminal evidence, exclusions and both compilers.
  Requirements without a reach clause have an empty row; no omitted-versus-empty
  compatibility mode. Supply core's critical capability declarations and put
  explicit rows on critical operations and portable contracts, including
  filesystem facets and ProcessExit; preserve interrupt MachineControl/PortIo
  bounds and pure numeric rows. Migrate existing boundary declarations and
  callers that relied on automatic self reach, not just the retired forms.
  Checked providers cannot hide actual callees; physical authority and foreign
  trust findings remain with **TWO-AXIS-TERMINAL-AUTHORITY-REVIEW**.
  `core/task.omg`'s generic `request_cancel` and `finish` wait on stage
  04's borrow-elision fence (`checks/borrows/elision/templates.rs`), which
  rejects a direct call to a generic trait requirement with a
  template-dependent result carrier unless static machine arguments close
  it. 78 files under `tests/` still author 99 retired rows.

  Preserve [source-selected trait tokens](wiki/spec/language/expressions.md#operator-declarations)
  through the migration. One explicit proof-static implementation binder may
  select a token-bearing requirement in a generic body; specialization retains
  and substitutes its exact row before lowering. No binder and multiple
  applicable binders reject. `Build::select_provider` may realize an already
  selected boundary row but never supplies token evidence, activates a trait
  token, overrides a primitive/direct family or changes resolution by target.
  Keep `samples/cli/probes/operator_requires_effects`: its direct boundary
  declaration fixes `==` before Build selects `CheckedEquality` as provider.
  Add migration twins where a boundary-trait provider without source evidence
  cannot make `==` resolve, two explicit evidence binders are ambiguous, and a
  named requirement call remains available for a noncanonical meaning.

  Acceptance: no accepted source, product source, durable IR, package record or
  artifact schema contains the standalone declaration kind. Migrated customers
  run through Terminal and native paths under boundary-trait selection;
  unselected, ambiguous, substituted, unauthorized and cross-package lookalike
  implementations reject. A parser fail fixture pins the retired spelling, and
  repository search finds it only in that rejection fixture and historical
  records. Windows/POSIX requirements propagate the same declared filesystem
  capability without implicit family reach; pure float selection adds none;
  explicit custom reach propagates through helpers and exclusions. A checked
  provider reaching outside an empty/narrow requirement rejects.

- **BUILD-ADMISSION-CHECKPOINT.** Complete command-level
  [restricted-build acceptance](wiki/spec/packages/acceptance.md#restricted-build-acceptance)
  across install, update, review, resume and consuming compilation.
  `omega-rust/omega/compilation/driver/src/checked/checking/build_continuation.rs`
  (`evaluate_build_and_continue`) joins requests before running the admitted
  build, and `omega-rust/omega/packages/package-manager/src/review/restricted_build_grants.rs`
  joins retained lock decisions; no customer observes a refused effect.
  - Supply a restricted-request command customer using one dependency in both
    Build and Product purposes with a generated-source handoff (manager review
    uses private staging with empty request sets, so it does not exercise
    consent). Bind requests and executor grants to package occurrence,
    dependency path, purpose, target profile, operation, scope and bounds.
    Product acceptance cannot grant Build effects. **BUILD-TEST-AUTHORITY**
    owns test/fixture attribution through this gate.
    Current review APIs accept captured inventories, not non-private executor
    scopes; named dependency inputs and accepted rows cannot supply those grants.
    Establish an independently motivated supported restricted execution protocol
    before adding a command consent phase; the scoped-execution spec defers
    extensible live-host grants. Do not replace private staging with a live mode
    or manufacture pending findings merely to exercise recovery.
  Acceptance: initial, added, widened and transitive requests stop before any
  effect, show their coordinates and leave review incomplete; resuming the
  exact candidate with accepted requests runs and reviews the generated
  source; a filesystem sentinel shows missing, refused or cross-purpose grants
  perform nothing. Unchanged requests need no reapproval; interrupted or stale
  candidates keep the prior lock; locks hold decisions, never secrets or host
  paths. No recursive build API or hidden post-compilation callback.
  **BUILD-SNAPSHOT-OUTPUTS** owns committed outputs.

- **COMPONENT-SUBSTRATE.** Execute an ordinary two-package call with
  root-selected `Independent` composition under the
  [component publication contract](wiki/spec/build/component_publication.md),
  then stable-slot replacement. Runtime packages or Cathedral own deployment
  and update policy; the deleted compiler-side component deployment/publication
  simulators are not a substrate to restore.
  - Package review publishes the dependency description
    (`omega-rust/omega/packages/package-manager/src/build_session/component_publication.rs`),
    and provider planning checks it, but
    `omega-rust/omega/compilation/realization/provider-planning/src/provider_settlement/composition_modes.rs`
    rejects every `Independent` edge before production. Replace that fence
    only with real independent products carrying symbolic imports/exports,
    entry/leave resources and installation obligations; never silently fuse.
  - Complete the entry, outgoing-authority, service, mapping, lease, stack and
    retained-provider inventories; include native stack and realization
    identity in the published description where a native compilation supplies
    them. Join an authored runtime/Cathedral deployment consumer to the
    description contract used by **TOPOLOGY-PLAN-VERIFICATION**; descriptions
    and installation evidence grant no live deployment authority.
  - The runtime owns replacement eras: compatible imports/exports, new calls
    enter the new era, existing sessions keep theirs, candidate resources fit
    the frozen envelope, and retirement waits until every activation,
    registration and returned object is disposed. Rejected operations preserve
    custody. Stronger continuity needs its explicit contract.
  Acceptance: a two-package corpus build with `Independent` composition
  emits separate products and executes the crossing call; descriptions grant
  no install authority; omitted, duplicate or unaccepted-assumption cases
  reject; a replacement customer shows new-era routing and old-era drain.
  **BOUNDED-INSTALLATION-REACH-ROWS** consumes this; **WIRE-RUNTIME-AND-INSTALLATION**
  owns platform invocation; **PSI-COMPONENT-REPLACEMENT** the interpreted
  customer.

- **WIRE-RUNTIME-AND-INSTALLATION.** Execute an authored program through a
  receiving-provider installation and matching-host invocation under
  [executable installation](wiki/spec/build/executable_installation.md).
  - `omega-rust/omega/compilation/backend/runtime/executable-installation`'s
    `OwnedImageProvider` installs, patches, seals and retires resident buffers;
    `call_code` hands back borrowed bytes but nothing performs a physical call.
    `omega-rust/omega/compilation/backend/images/image-emission/src/installed_artifact.rs`
    binds the exact bytes.
  - Supply the authored/package provider and executor join: content,
    placement, installed occurrence, requirement-compatible entry and
    live-root custody through return and retirement. Write/execute and fetch
    visibility belong to the provider. Keep `ConventionOnly`; reject
    `Unsupported`. File emission is not installation.
  Acceptance: one artifact installed at distinct placements, invoked on a
  matching host, returns its authored result. Retirement needs quiescence,
  execute removal and restored write authority; incomplete drain quarantines.
  Replacement uses admitted fragments at declared sites (the buffer patcher
  rejects relocated fragments; wider support needs the target resolver).
  Substituted bytes, double-spent placement and unsupported W^X reject and
  return all inputs. Arbitrary bytes-to-code, JIT and numeric-address
  execution stay unsupported. **ADDRESS-EXPOSURE** owns address observation;
  **COMPONENT-SUBSTRATE** owns component replacement.

- **X86-FMA-PROVIDER-TRANSPORT.** Execute admitted x86 FMA through std
  consumers and Linux x86-64 hardware.
  - `NearestIeeeFloatArithmeticRealization` carries the admitted AVX+FMA3
    provider from stage 00 catalog admission through stage 02/03 lowering,
    legalization and selection; x86 encodes `VFMADD213SS`/`SD` with the addend
    in `xmm3`. Std-free `pass/float/fused_multiply_add_single_rounding_exit`
    exits 0 on Windows x86-64, both formats trap when the helpers use
    multiply-then-add, and the `linux_x86_64` cross-build carries one
    VFMADD213 per format. Without admission the plan join still rejects.
  - Remaining: run that fixture on Linux x86-64 hardware; run
    `x86_fma_plan_association`, `named_provider_fused_multiply_add_exit` and
    `admitted_numeric_imports_preserve_semantics` (all admit AvxFma3) on
    matching hosts now that std Console source checking is restored.
  - Delete the superseded constant-operand
    `TargetUnitOperation::NearestIeeeFloatFusedMultiplyAdd` route, machine-code
    FMA fragments/occurrences and their digest plumbing,
    `object_artifact/replay/x86_fma.rs`, and the unused ObjectArtifact /
    ExecutableImage `x86_scalar_fma_provider` fields. Preserve the admitted
    provider in the realization plan, ordinary runtime-operand lowering and
    floating-control behavior. Do not extend replay to runtime operands or
    replace it with another FMA-specific image ledger; see
    [numeric realization](wiki/spec/language/numeric_values.md#target-realization-and-control-state).
  Acceptance: the three std fixtures run on Windows and Linux x86-64 at their
  oracles and the std-free fixture runs on Linux x86-64 hardware; the dormant
  image custody route and its callers are absent, with missing-feature and
  fused-versus-unfused controls retained.

- **ASM-INSTRUCTION-CATALOG-EXPANSION.** Carry checked assembly through
  source-to-native production under [assembly](wiki/spec/language/assembly.md)
  and the [catalog](omega-rust/psi/foundation/language-core/inline_assembly.md).
  - `omega-rust/psi/compilation/pipeline/04_typed-trees-to-checked-trees/src/execution/terminal_unit/calls/call_operations.rs`
    plans only constant `AsmPortOut`. `pass/inline_asm/asm_runtime_port_msr_final_validation`
    and `asm_aarch64_system_registers_compile` reject with "attached Unit
    closure is missing a checked transitive machine plan".
    `asm_port_out_final_validation` reaches native authority review
    (**PRIVILEGED-PORT-EFFECT-SETTLEMENTS** owns the adapter).
  - Complete checked plans, lowering, Terminal encoding, target realization
    and final-span evidence. Reuse ordinary assignments/transitions where
    exact: `jmp` and typed-place `ldr`/`str` already build. Keep catalog target
    restrictions through that desugaring.
  - Restore source-to-byte coverage, in order: x86 fences, interrupt control,
    flags, MSR/control registers, then pipeline/cache directives and AArch64
    system registers. Fixtures without entry bindings only check; bytes do not
    prove privileged execution. Extend memory, atomic, barrier, cache/TLB and
    mode-transition families only with customers and complete operand/effect
    contracts; numeric addresses grant no access. No opcode per spelling or
    separate asm-body recognizer.
  - Hosted `port_io` and `interrupt_table` grants stay independent;
    machine-owner authority stays freestanding-only. **INTERPRETED-ASSEMBLY**
    owns interpretation.
  Acceptance: the two rejecting `inline_asm` fixtures reach native bytes with
  exact widths, clobbers and ordering; wrong-target, missing-authority,
  omitted-clobber and invalid saved-place controls reject (existing
  `fail/inline_asm/*` stay rejected); unknown instructions and hidden exits
  reject.

- **RUNTIME-SIZED-ACTIVATION-CONTRACT.** Execute a source-authored
  [bounded activation claim](wiki/spec/resources/activation_storage.md):
  establish runtime-sized backing within a declared bound, access it, retain
  it across suspension and release in reverse order. Ordinary calls/core
  declarations expose it; no new keyword or owner decision is needed.
  - `ActivationClaimLedger` and `compose_claim_bounds` in
    `omega-rust/psi/foundation/extents/src/activation_claims/mod.rs` have no
    compiler consumers and hold numeric rows, not backing or activation
    identity. Connect call admission, extent evidence and Terminal production
    to compiler-provisioned storage, keeping each site's bound, committed
    extent, provenance, release order and suspension role. Co-live bounds sum
    (aligned); exclusive branches take the maximum; open demand rejects before
    establishment. Backing does not move.
  Acceptance: a run fixture where `committed > bound` returns checked failure
  (no trap or clamp) and a within-bound claim executes; escaping claims, wrong
  activation, stale loans and invalid release order reject. **FRAME-LAYOUT** in
  `TASKS_OPTIMIZER.md` owns final frame, probe and unwind. Allocation packages
  cannot mint this grant from an address/length; runtime generic extents,
  implicit variable-sized locals and OS allocators do not supply it.

## Checked boundary topology

A later library customer under the
[reference-package contract](wiki/spec/packages/topology.md).
**COMPONENT-SUBSTRATE** owns component completeness;
**WIRE-RUNTIME-AND-INSTALLATION** owns executable custody. Graph policy and
orchestration stay in ordinary Omega packages; no compiler graph stage,
topology IR or trusted graph axiom.

- **TOPOLOGY-PLAN-VERIFICATION.** Connect the Omega build-only composer in
  `tests/omega/pending/build/build_scope_topology_plan` (it does not check
  against the current Build facets; its header has the diagnostics) and the
  consumer `source/library/topology/verify.omg`. Complete the ordinary Omega
  package's normalization, policy, certificate and codec logic against verified
  component descriptions; derived contracts, not surface-name hashes, join
  demands to supplies. `verify.omg` has never been driven; component admission
  must come from ordinary component-description production, not a Rust oracle
  or Rust-built Terminal module.
  **BUILD-SNAPSHOT-OUTPUTS** owns confined inputs and output publication.
  Refresh the stale test names in `wiki/spec/packages/topology.md`'s acceptance
  status when this lands.

  Acceptance: the composer fixture admits three component descriptions, publishes
  `payments.plan`, and `verify.omg` verifies it without source against a
  separately supplied `TopologyRequest`; exhaustion is unsuccessful
  verification. Keep the spec's
  [controls](wiki/spec/packages/topology.md#diagnostics-and-implementation-acceptance):
  missing/substituted components, stale subjects, forged completeness,
  direct/indirect bypasses, cycles, distinct instances, duplicate bindings,
  disconnected selectors, missing policies and corrupt plans. Unselected policy
  code is never loaded; plan verification grants no installation authority.

- **TOPOLOGY-PRIVATE-PIPE-INSTALLATION.** Run the
  [three-process payment customer](wiki/spec/packages/topology.md#first-executable-realization)
  with an Omega-authored installer and component programs on Windows and macOS,
  after **TOPOLOGY-PLAN-VERIFICATION**. Implement mediation, frame/schema
  validation, private-pipe adaptation and supervision in ordinary Omega/runtime
  packages over general process and pipe boundaries. Add explicit Windows
  handle transfer and macOS validated/signed member images. Disable general
  inheritance and deliver exactly the admitted endpoints before application
  entry.

  Acceptance: on both hosts, exact images/mappings are admitted with one
  outstanding schema-checked request/response per binding. Ungranted endpoints
  and substituted mappings reject; malformed frames and peer failure close the
  binding. Failed setup and quiescence clean the whole roster, including killed
  members. Replacement preflights, quiesces the old generation, then activates
  the new one without overlap.

## Rust compiler release closure

The [completion contract](wiki/drafts/reference/rust_compiler_completion.md)
requires its six gates (`RC-REPOSITORY`, `RC-SOURCE-SEMANTICS`,
`RC-BUILD-AND-PACKAGES`, `RC-NATIVE-MATRIX`, `RC-DIAGNOSTICS`,
`RC-REPRESENTATIVE-PROGRAMS`) to pass at one clean commit, plus direct
execution on the four required hosts. These items own evidence closure;
capability repairs stay with their owners. An unavailable runner, unexplained
skip, filtered or empty run, timeout or exhausted capacity leaves a gate open;
report a missing runtime leg explicitly.

- **RC-RELEASE-RECORD-AND-CLOSURE.** Produce the same-commit release record.
  `tools/release/release_record.py` is the only recorder: `run` records one
  lane on its own host, `check` re-derives gate status, and `close` joins one
  record per lane, rejecting missing, duplicated, mixed-commit, stale,
  mixed-toolchain and wrong-host records. `RC-NATIVE-MATRIX` and
  `RC-REPRESENTATIVE-PROGRAMS` must pass on every lane; the others must pass
  wherever they ran and run somewhere; each lane needs a passing
  `--native-execution` observation. No record exists yet.
  `RC-REPRESENTATIVE-PROGRAMS` uses `tools/sample_native_gate.py` to perform
  each applicable sample's real review, build, execution and documented oracle;
  **SAMPLE-CORPUS** owns failures and non-host runners.

  Acceptance: four same-commit records (pinned toolchain, host
  OS/architecture, exact commands, outcomes, elapsed time, expected skips,
  emulator/version where used) and a `close` result that reads closed. Rust
  stays a comparator; bootstrap work stays on its own board.

- **RC-REPOSITORY-CLOSURE.** Pass the four `RC-REPOSITORY` commands at one
  clean commit with no crate exclusions: `python tools/fmt.py --check`,
  workspace all-target Clippy with warnings denied, workspace all-target check,
  and `mbx nextest run --workspace --lib --no-fail-fast`. Attribute each
  failure to its capability owner. Acceptance: all four pass at the release
  commit, recorded through **RC-RELEASE-RECORD-AND-CLOSURE**.

- **RC-BUILD-AND-PACKAGES.** Pass the gate's two commands (nextest and
  doctests for `build-declarations`, `package-source` and
  `resolver-execution`) at the release commit; the `package-source` host
  failures were repaired on both hosts (`ab6ba3198a5`, `289501b5c2f`; 141 of
  141 on macOS and 93 of 93 on Windows). These commands alone do not
  establish compiler integration. The following integration requirements lost
  their Rust tests; each needs a corpus or package fixture under its capability
  owner:

  - **Package and selection identity:** carry exact checked-call, review,
    package-use and provider-schema identity through compiler handoff.
    `allow_unresolved_toolchain` in
    `omega-rust/psi/compilation/pipeline/04_typed-trees-to-checked-trees/src/authored_selections/finalization.rs`
    still lets toolchain-origin selections stay unresolved. A package-owned std
    with a targetless UEFI-handoff call must late-bind like bundled std while
    unresolved root calls reject; do not widen the allowance to every
    `CheckedCall` or infer ownership from a path. **MODULE-NAMESPACE-RESOLUTION**,
    **PROVIDER-SELECTION-AFTER-TERMINAL** and **X86-FMA-PROVIDER-TRANSPORT** own
    the respective parts.
  - **Private progress premises:** under
    [termination](wiki/spec/language/termination.md), **TPR6** owns finite sets
    of externally dischargeable subjects. Fixtures needed: two independent
    WeakFair premises, a private wrapper around a cycle, and recursively growing
    subject projections that must not become a finite promise; negative controls
    for reference-operand mutation, later loop arrival and same-named formals.
    Private visibility cannot waive a ProgressProfile requirement. Code:
    `checks/contracts/calls.rs` and `checks/termination/progress/components.rs`
    under stage 04. Unverified: whether an unavailable derivation still maps to
    `NoGuarantee`.
  - **Demanded imports and callback custody:** provider planning and
    `omega-rust/omega/compilation/driver/src/native/retained_native_product.rs`
    must refuse an actually called import without supplied execution/stack
    custody. `exact_demanded_import_plans` in stage 00's
    `source_evaluated_bindings.rs` omits requirements with no matching import
    row; a fixture must follow demand through planning and settlement, plus a
    supplied-custody positive. Preserve exact callback calling/fragment
    custody at native admission.
  - **Foreign-domain mutable recasts:** **MODULE-NAMESPACE-RESOLUTION** keeps a
    qualification when declaration and mutable view name the same foreign
    package domain, and rejects exchange between different packages'
    same-named domains. `pass/recast/runtime_mutable_equivalent_domain_recast_exit`
    is a same-file control; a separate-package fixture is needed.
  - **Borrowed suspension and terminal arms:** **TR3-TR8** and loan/carry
    analysis own [borrow carry](wiki/spec/resources/carry.md#borrows-across-suspension)
    and [terminal calls](wiki/spec/language/state_contracts.md#terminal-calls-in-transition-arms).
    A machine-target arm uses ordinary call/completion with exact
    `suspend`/`block` markers; a local-state jump is marker-free. Statement,
    terminal-expression and terminal-arm calls agree on values, mutation,
    postconditions, crash/no-return, cleanup and suspension in the interpreter
    and natively. Cover caller-local backing and shared, mutable, write-only,
    aggregate-carried and service-receiver loans through nested parked calls;
    repeated park/resume without repeated effects; cancellation; early parent
    access, missing carry, forbidden live resources and missing/redundant
    markers reject.
  - **Host-provider execution:** use the existing TimeHost providers at
    `source/library/std/targets/<target>/time_host_impl.omg`; do not add a
    second provider path.
    `pass/time/runtime_time_elapsed_since_exit` has
    recorded native exit 70 on Linux x86-64 and macOS ARM64, and a portable
    `checked` row. Windows x86-64 still records rejection; Linux ARM64 has no
    native record. Obtain matching-host exit 70 on those two hosts, then
    refresh all required lanes at the release commit. Cross-target builds
    and historical successes do not close release acceptance.
    Keep `pass/time/runtime_time_host_native_darwin{,_x86_64}_exit` controls
    and `pass/providers/foreign_out_parameter_write_observed_exit`: the latter
    must observe the foreign write, not its pre-call initializer (exit 72 is
    failure, 70 success). Remaining beside that: filesystem adapters with real
    syscall marshalling and result transport beyond the Linux x86-64 rows in
    `omega-rust/omega/compilation/realization/provider-planning/src/provider_settlement/canonical_filesystem_host.rs`.
    Follow [toolchain-selected implementations](wiki/spec/build/provider_selection.md#toolchain-selected-checked-implementations);
    `ProviderBinding::Import` must commit to the exact locator. Prove native
    outcomes of the `tests/omega/pass/objc/` fixtures, including GUI and
    multi-library calls.

  Plan-laid repeated runtime, dynamic subslice endpoints and ranked cycles stay
  with **PLAN-LAID-VIEWS**, **TERMINAL-SLICE-VIEW-VOCABULARY** and
  **TERMINATION-RANKING-CHECKS**.

- **RC-NATIVE-MATRIX-LINUX-X86-64.** Pass the shared gates with direct ELF
  x86-64 execution, including dynamic imports. **SAMPLE-CORPUS** owns sample
  failures and **TWO-AXIS-TERMINAL-AUTHORITY-REVIEW** owns provider authority.
  Reproduce current failures from `tests/omega/corpus_native_linux_x86_64.txt`;
  a historical record cannot close the row.

- **RC-NATIVE-MATRIX-LINUX-ARM64.** Pass the shared gates with AArch64 ELF
  execution on Linux or on a named, versioned emulator that runs the target
  runtime legs. Cover host/time/filesystem and `IntegerAt` projection/write
  behavior. Build-owned [loader selection](wiki/spec/build/configuration.md#elf-executable-loader-selection)
  is wired end to end; `pass/providers/external_boundary_requirement_via_exit`
  exits 21 on Linux x86-64 and must also exit 21 here, binding libc
  `abs`/`exit` at GLIBC_2.17. No `corpus_native_linux_arm64.txt` exists yet.

- **RC-NATIVE-MATRIX-MACOS-ARM64.** Pass the shared gates with Mach-O execution
  on macOS AArch64. `tests/omega/corpus_native_macos_arm64.txt` still records
  unfinished programs. Use
  [native rejection attribution](wiki/drafts/measurements/macos_arm64_native_rejections.md)
  and [release-gate history](wiki/drafts/measurements/macos_arm64_rc_gates.md)
  as reproduction leads; route omitted Unit plans to
  **STATE-LOCAL-VALUE-FRONTIER** and other failures to their owners.

- **RC-NATIVE-MATRIX-WINDOWS-X64.** Pass the shared gates with PE execution on
  Windows x86-64, including the hosted receiver.
  `tests/omega/corpus_native_windows_x86_64.txt` predates 00e62d483f, whose
  portable Console cutoff rejected every std consumer on every host. Its
  current shared dependency is recorded under **PSI-TARGET-FAMILIES**;
  re-record after it lifts. At bff9578e24 Windows built every
  fixture a macOS cross-build built except
  `pass/float/{admitted_numeric_imports_preserve_semantics,named_provider_fused_multiply_add_exit}`
  (x86 FMA, now admitted with AvxFma3; **X86-FMA-PROVIDER-TRANSPORT** runs
  them once std checks), and every
  executed fixture met its oracle. Remaining:
  - 49 fixtures bind only macOS or POSIX hosts through Darwin/POSIX foreign
    providers (`pass/host/runtime_descriptor_*`, `pass/providers/foreign_*`,
    `pass/filesystem/native_*`, `pass/time/runtime_clock_*`) yet keep portable
    `expected_native_exit.txt`/`expected_stdout.txt` oracles, so the Windows
    leg reports each as an unmet promise. Author Windows provider rows where
    the capability applies (msvcrt `_read`/`_write` return `int`, so an
    adapter body, as in `pass/host/runtime_foreign_scalar_loan_writeback_exit`)
    or scope the oracle to its bound hosts; `expected_stdout.txt` has no
    per-target form.
  - `tests/omega/pass/filesystem/windows_set_file_time_exit` calls the mixed
    `FilesystemHost` directly and stops at provider supply; migrate it to the
    applicable static raw-family requirement and add `expected_native_exit` so
    it must call SetFileTime then `_stat64`, read back Unix time 1,500,000,000
    and exit 70.

- **MACOS-X64-HOST-PROFILE.** Cover Intel macOS: Mach-O entry/exit, hosted
  receiver and provider/import pairing. `macos_x86_64` emits and runs under
  Rosetta on Apple silicon (`cli_mvp` exits 0 with its two lines;
  `external_boundary_requirement_via_exit` exits 21); see the
  [host observations](wiki/drafts/measurements/macos_x64_host_profile.md).
  Rosetta is not an Intel CPU. This platform is outside the four-host
  completion matrix.

  Acceptance: on an Intel Mac, record `tests/omega/corpus_native_macos_x86_64.txt`
  with `python3 tools/corpus_gate.py --native --record`, compare it with the
  Rosetta leg (`--native-target macos_x86_64`), and record
  `omega inspect-terminal --target macos_x86_64`. Extend fixture and sample
  builds to `macos_x86_64` only with an Intel or Rosetta observation.

## Omega-written compiler (after Rust completion)

Starts after the Rust [completion contract](wiki/drafts/reference/rust_compiler_completion.md)
closes. Rust can remain a differential implementation, but neither Rust
agreement nor Rust-specific machinery is bootstrap authority. Bootstrap
construction stays on `TASKS_BOOTSTRAP.md`.

- **OMEGA-PRODUCT-COMPILER-SOURCE.** Finish the Omega-written compiler in
  `source/psi/` and `source/omega/`, rooted at `source/omega/build.omg` and
  `source/omega/main.omg`. Psi owns source semantics through proof and
  Terminal Psi; Omega consumes Terminal Psi for provider selection,
  realization, ABI and emission. Extend the one package-resolved pipeline
  through the full language, with one OCREQ V1 request producing one OCOUT V1
  outcome. No private dialect, file/AST allowlist, source-shape-only lowering
  or parallel source-to-native route. Implemented judgments and representation
  boundaries belong in [the Psi source guide](source/psi/README.md), not this
  execution item.

  Current frontier: complete the request-to-source connection and the
  unsupported language and library closure below, then carry checked source
  through proof, Terminal Psi and Omega realization. Revalidate the integrated
  source with the parser, resolver, typing and checking gates, including the
  whole Psi package; retain the actual remaining diagnostics rather than
  carrying forward earlier case counts or package-size measurements.

  - Request semantics (`source/omega/request/`): well-formed requests stop at
    `coverage_request_semantics`. For phase 3, replace the two-tag lineage
    text with the canonical package-origin structure and shared key computation
    under [shared package identity](wiki/spec/build/compiler_request.md#shared-package-identity).
    Specify encoding and migrate producers, D, C and shared vectors together;
    change the unreleased format in place, without legacy readers. Verify each
    supported origin agrees with package management; alias changes preserve
    identity, lineage changes change it, and resolution changes only the instance.
    Implement package identity/order, dependency graph, snapshot materialization,
    admissions and subject commitment (phases 3, 4, 5, 7 and 8), preserving
    the contract's diagnostic selection order, then hand the root source to
    Psi. Keep D's request vectors in `tests/product/request/test-request.sh`.
  - Parsing and resolution (`source/psi/parse/`, `source/psi/resolve/`): admit
    the remaining valid forms, including callback telescopes (`native callback
    procedure from`), applied type-position domains (`in Quantity<Units::METER>`),
    installation reach (`reaches <= Bound`), juxtaposed requires facts and
    indexed membership domains (`in AtMost<8>`). Bind mathematical definitions and transparent trait
    refinements, nominal `where machine ... satisfies` contracts, generic
    structural contracts and their remaining clauses, `where proposition`
    contracts and proposition/evidence binders. Complete operator binding
    beyond ordinary owned arithmetic/order machines, including equality,
    index and range operators and trait requirements. Preserve the named
    refusals until each form's ordinary semantics is implemented.
  - Typing (`source/psi/typing/`): complete trait requirement calls, generic
    conformance members, conformance static arguments, requirements with
    their own generic parameters, target-scoped family calls and case domain
    qualifiers. Implement declared equality through `Equatable` named
    conformances under [core equality acquisition](wiki/spec/language/conformances.md#core-equality-acquisition).
    Implement [data identity in facts](wiki/spec/proofs/contracts.md#data-identity-in-facts):
    `==` between values of one identical non-primitive data type in a fact or
    proposition elaborates to proof-relevant `Id`, without `Equatable` lookup
    or runtime comparison. Keep executable `==`, explicit named equality calls,
    primitive numeric/address relations and static type equality on their
    existing routes. Preserve the authored `Nat`, `Seq`, `IntPair`, `Real` and
    `content` theorem spellings. Select by checked context and operand types,
    never the Rust implementation's application/literal source-shape split.
    Acceptance: a generic `Seq<T>` identity theorem checks without
    `Equatable<T>`; the same executable comparison rejects without selected
    equality; a named equality-machine call in a fact retains its requirements;
    cross-width integer facts retain their existing relation semantics.
  - Checking and proof (`source/psi/checking/`): implement proof-only `embed`
    under [contracts](wiki/spec/proofs/contracts.md), mathematical/proposition
    facts beyond the current whole-fact bindings, and contract refinement
    beyond crash causes, including satisfier and static-machine-argument
    premises and guarantees. Prove indexes against symbolic `const` extents,
    case memberships, memberships inside larger facts, byte predicates and
    stores affecting case-specific `where` facts. Represent storage origins
    for returned references, reference fields and selected reference values;
    nonempty writes through an unknown origin must remain refused rather
    than acquire independent storage facts. Track payload paths on held-loan
    rows so projected carriers retain only their own backing loans; the
    current root-based transfer conservatively retains sibling loans.
    Checking and ownership follow
    [destination-first stores](wiki/spec/language/expressions.md#evaluation-schedule);
    executing `pass/ownership/runtime_captured_replacement_selector_exit`,
    `runtime_captured_construction_selector_exit` and
    `runtime_selective_constructed_owner_custody_exit` with the product
    waits on Terminal Psi production below.
  - Bundled-library and entry closure: remeasure the first actual refusal in
    each source set rather than retaining the old parse/resolve totals.
    **TOP-LEVEL-BOUNDARY-REQUIREMENTS** owns the remaining retired boundary
    requirement/operator migration, and **CANONICALIZE-SCALAR-RANGE-CONTRACTS**
    owns remaining range suffixes; do not accept retired syntax to bypass
    them. Preserve the outstanding closure cases in `std/calling.omg`
    (`Policy satisfies CallingPolicy`), the device/arena/extent imports
    (`extent`'s mathematical `no_wrap` declaration), `core/slice.omg`'s index
    operator, and `std/process_exit.omg`'s provider type used without a direct
    import. `core/option.omg`'s `zero_value<Optional<T>>()` observation has no
    specified source meaning; resolve that contract instead of importing the
    Rust parser's contextual recognition. `std/build.omg` needs its build
    context's `Build`. The product entry calls `Console::read_byte()` directly,
    the settled static provider call form that creates the exact `Console`
    slot demand
    ([expressions](wiki/spec/language/expressions.md#calls-places-and-temporaries),
    [boundary calls](wiki/spec/terminal-psi/boundary_calls.md)). Reconcile std target
    providers' undocumented `Owner::provider_defaults` / provider-data
    convention with named implementation selection in
    [provider selection](wiki/spec/build/provider_selection.md).
  - Capacity and validation: publish parser capacity refusals through the
    [compiler request contract's scalar-resource provisions](wiki/spec/build/compiler_request.md).
    Its published limits and counting scope must cover the actual shared
    source-set tables and compiler closure; the implementation's raised
    capacities alone do not settle whether the request counts per unit or
    needs larger provisions. Preserve acceptance/rejection twins, lexical
    handoff, structural identities, capacity edges, deterministic observations
    and range-proven narrowing casts. Run `tests/psi/{parser,resolver,typing}/`
    gates on Linux and Windows as well as macOS, and retain checking and
    request coverage in the shared product suite. Use an explicit fresh
    `OMEGA_CLI` and `OMEGA_TARGET`; keep the gate decoders semantic-free.

  Acceptance: the package-resolved closure implements the
  [compiler product contract](wiki/spec/build/compiler_request.md), passes the
  shared product suite and Rust/Omega differential checks, and publishes a
  deterministic manifest of all transitive compiler/build inputs. Rust
  agreement is not bootstrap authority. Epsilon-written D and
  `omega0 -> omega` construction stay on `TASKS_BOOTSTRAP.md`.
