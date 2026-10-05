# Tasks

Unfinished product and language work for the Rust reference compiler. The
[completion contract](wiki/drafts/reference/rust_compiler_completion.md)
defines release acceptance; this board lists the remaining work, not a
passing baseline.

Pick the nearest program that fails to compile or run, repair the shared
operation it reaches, and rerun the unchanged program through native
execution. All accepted language scope stays required; the order below guides
implementation, it does not exempt anything from release.

| Priority | Work |
| --- | --- |
| 1. Run existing programs | [High-priority composition repairs](#high-priority-composition-repairs) and [Immediate product closure](#immediate-product-closure). Choose a concrete failure from SAMPLE-CORPUS, CANARY-CORPUS or SQUALR-HEADLESS and repair the general operation. |
| 2. Remove source and pipeline blockers | [Universal interpreter cutover](#universal-interpreter-cutover), [Pipeline route](#pipeline-route), [Data-flow defects](#data-flow-defects), [Trust ceremony](#trust-ceremony), [Red tests](#red-tests) and [PCC removal](#pcc-removal). |
| 3. Connect remaining accepted capabilities | [Entry and authority](#p1---authority-roots-and-entry), [materialization](#p2---materialization-and-placed-access), [service reach](#automatic-service-reach), [reflection](#semantic-reflection), [scoped build execution](#scoped-build-execution), [process exit](#process-exit-contract), [exclusions](#build-level-behavior-exclusions), [tests](#requirement-based-tests), [embedding](#embedding-and-interpreted-components) and [parallel lanes](#parallel-language-and-compiler-lanes). Move an item earlier when the chosen program needs it. |
| 4. Close the release matrix | [Rust compiler release closure](#rust-compiler-release-closure). Focused successes do not satisfy full gates. |
| Separate owners | [Optimizer](TASKS_OPTIMIZER.md), [bootstrap](TASKS_BOOTSTRAP.md) and [Cathedral](#p5---cathedral-over-general-omega-primitives). A native correctness dependency moves with the program that exercises it. |

Language behavior is tested by corpus fixtures under
`tests/omega/{pass,fail,run}/` through `tools/corpus_gate.py` (`--native`
builds and executes eligible fixtures); `tools/sample_native_gate.py` checks
documented sample exit/output on the host. Most compiler crates have no Rust
tests: an acceptance names a corpus fixture, or asks for a new one. Proof-carrying
code is being removed; no item here requires proof bundles, receiver
re-verification or independent replay of the compiler's own output. Run
`python tools/board_paths.py --board TASKS.md` after moving cited paths.
Workflow is in [AGENTS.md](AGENTS.md), live assignments in the
[claims registry](tools/claims.md), semantic decisions in
[OWNER_QUESTIONS.md](OWNER_QUESTIONS.md).

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

- **REFERENCE-CUSTODY-JOIN-SCALING.** (new-scope) Investigate and remove
  avoidable compilation cost in reference-custody joins without weakening their
  semantics. `pass/expressions/runtime_indexed_float_record_fields_exit` was
  recorded moving from roughly 3.3s to 9s across stored-reference changes;
  those historical measurements are a lead, not a current baseline or an
  accepted cost envelope. The former `custody-join-build-cost` escalation is
  engineering under [Psi ownership](omega-rust/pipeline.md#psi-implementation-and-deferred-human-audit).
  - Current [measurements](wiki/drafts/measurements/reference_custody_join_cost.json):
    at `a195e6cdf06` on macOS ARM64, three serial native runs take
    4080–4104 ms for indexed float fields and 1604–1623 ms for the
    stored-reference sibling, all exit 70. Float capacities 8/32 stay near
    3.9s. Eight reference carriers run; 32 exposed a call-free link-register
    preservation bug, now covered by
    `pass/borrows/runtime_reference_array_pressure_exit` (native/interpreter
    exit 70). These probes repeat two reference roots, not distinct owners.
    The historical Linux regression still needs a same-host revision
    comparison and scaling with distinct roots/observations; no superlinear
    custody join or compile-time speedup is established by this measurement.
  - Reproduce on one host/configuration and profile the exact reader before
    choosing a repair. Own the evidenced Psi custody/fact propagation path;
    compare against stored-reference sibling programs and increasing record/
    reference counts to distinguish fixed work from superlinear recomputation.
    Do not add timing thresholds to semantic tests or omit custody checks.
  Acceptance: the customer still produces its authored result; report repeated
  before/after timings and scaling on the same configuration, plus
  `python tools/corpus_gate.py --filter expressions/runtime_indexed_float_record_fields_exit,borrows/runtime_indexed_reference_array_bounded_read_exit`.
  If the regression no longer reproduces, remove this task with that evidence;
  do not invent a new language decision to justify keeping it open.

- **ENTRY-SLOT-EXACT-SCHEMA-SELECTION.** (split-of:UEFI-PHYSICAL-SEMANTIC-ENTRY)
  Finish alternate accepted-contract coverage after exact
  [target-slot](wiki/spec/build/entry_roots.md#slots) selection.
  - The slot now selects its bundled source owner or explicit accepted
    package/declaration binding in
    `omega-rust/omega/compilation/realization/provider-planning/src/program_entry.rs`.
    Physical requirements follow resolved schema composition; semantic arrival
    keeps core's exact requirement identity. Unrelated same-leaf declarations
    neither shadow the selection nor create another candidate.
  - On macOS ARM64, `omega run tests/omega/pass/build/entry_schema_same_leaf_exit/main.omg`
    now exits 0; base `199028b84b4` rejected the same program for two loaded
    `MacosApplication` declarations. Rebased onto `4939829df0e`, this repair on macOS ARM64:
    `python3 tools/corpus_gate.py --native --filter build/uefi_entry_schema_same_leaf,control_flow/nonreturning_callee_exit`
    produces the UEFI artifact with its authored image/storage roots and runs
    the hosted non-returning-callee witness with exit 0. Aggregate legalization
    retains admitted content claims; machine-emission accepts an empty exit
    roster without inventing a return or frame epilogue. UEFI execution was
    not exercised on this host. The remaining acceptance here is the alternate
    accepted-contract customer below, not another source-shape lowering path.
    `pass/build/uefi_os_handoff_invocation` retains its separate omitted
    `UefiOsHandoffCycle::acquire` lowering dependency.
  - `fail/build/uefi_entry_schema_wrong_owner` rejects an unaccepted supplier
    despite matching the contract's path/name. The old local-shadowing
    construction in `fail/build/uefi_program_entry_wrong_calling_policy` is
    replaced with a real invalid physical plan (AAPCS64 with x86 registers).
    The corpus currently accepts only the bundled entry supplier. Add coverage
    of a different explicitly accepted package/schema whose otherwise valid
    physical convention mismatches the slot; do not invent local shadowing.
  Acceptance: `python tools/corpus_gate.py --filter build/uefi_` plus native
  `build/entry_schema_same_leaf_exit`; UEFI reaches its intended product,
  unrelated declarations preserve it, and invalid selected plans reject for
  their actual contract rather than an accidental name collision.

- **STATIC-DUNGEON-NATIVE-BUILD-COST.** (split-of:STATIC-MACHINE-MIGRATION)
  The native build of `samples/cli/games/dungeon_crawler_cli` does not finish.
  - Current state: the app is migrated to static calls and `omega --check`
    passes on all of its sources in under a minute, but
    `python tools/sample_native_gate.py --filter games/dungeon_crawler_cli --timeout 5400`
    returns `{"state": "timeout"}`. A `detail` of `>Ns` is a lower bound on an
    unfinished run, not a duration. The cost is single-threaded CPU after
    checking, in lowering and the stages after it.
  - Profiled cost sites:
    (a) `omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/structural/byte_sequence_fields/freshness.rs`
    (`established_on_every_path`) is quadratic in block count. Deriving the
    predecessor index once was tried and gave no measurable gain; do not
    retry it as the fix.
    (b) whole-module validation repeated several times per publish.
    `encode_module` (which `semantic_fingerprint` and `terminal_psi_identity`
    call) no longer runs `validate_module_representation`. Stage 01's
    `omega-rust/omega/compilation/pipeline/01_abstract-operations-to-abstract-operations/src/validation/projection/initial_ledger.rs`
    and `.../validation/context/mod.rs` (`ranked_cycles/replay.rs`,
    `context_projection.rs`) each re-derive identity from the same module.
    Under the PCC-removal and trust-ceremony rulings this repetition is
    in-process self-validation and is removed, not preserved:
    **OMEGA-READS-TERMINAL-UNVERIFIED** removes the remaining verifier
    passes, and **TERMINAL-VERIFIER-DELETED** deletes (a). Rerun the sample
    gate first to measure what the encode change bought.
    (c) `omega-rust/psi/compilation/proof/admission/src/mathematical_core/certificate.rs`
    starts one 256 MiB-stack thread per certificate admission. Operation
    certificates go away with **PCC-CERTIFICATES-DELETED**; a persistent
    worker needs an ownership change and is not the next step.
  - Cost scales near-linearly per source line on smaller samples, so the
    multiplier is repetition, not per-line work.
  Acceptance: the sample native gate builds the app within its normal timeout
  and the app meets the oracle in
  [its README](samples/cli/games/dungeon_crawler_cli/README.md#runtime-acceptance).

- **STATIC-BOUNDARY-REQUIREMENT-CALL-FORM.** (split-of:STATIC-MACHINE-MIGRATION)
  Repair selection for explicit completion calls under
  [domain authority and separate implementation selection](wiki/spec/resources/authority.md#completion-authority-and-implementation-selection).
  The authority question is settled: checked domain establishment and retained
  provenance travel with the qualified argument under ordinary transfer rules.
  Public boundary requirements still need a selected, admitted implementation.
  - Customers: `pass/memory/address_translation_canary` calls
    `Shootdown::discharge(owed)`; `pass/memory/secondary_processor_canary`
    calls `StartupEnvelope::confirm(envelope)`; `pass/memory/interrupt_table_canary`
    calls `InterruptAcknowledgement::complete`. Recorded failures name the
    missing selected provider. Reproduce the current frontier for each.
  - Trace each requirement through source selection and
    `omega-rust/omega/compilation/realization/selected-dispatch/src/boundary_dispatch.rs`.
    Supply a missing implementation/selection in the owning package, or repair
    propagation where an existing selection is lost. Keep named implementations
    separate from data; do not infer a body from the argument's domain.
  - Preserve checked domain preconditions, exact installation/token relationships
    required by the contract, and linear consumption. A recipient may forward
    established evidence without becoming an issuer. No new authority mechanism,
    implicit receiver, or data-subject conformance is needed.
  Acceptance: `python tools/corpus_gate.py --filter memory/address_translation_canary,memory/secondary_processor_canary,memory/interrupt_table_canary`
  checks the intended completion flows and rejects forged, unrelated or replayed
  completion evidence. Ensure the provider repair does not bypass those checks.

- **STATIC-LOWERING-BORROWED-ARGUMENT-TRANSFER.** (split-of:STATIC-MACHINE-MIGRATION)
  Lower a call that consumes a linear input and returns an affine result
  as an ordinary call.
  - Customer: `tests/omega/pass/core/task_lifecycle_operations`. It is
    recorded rejected at "receiver-bearing suspension frontier lacks an exact
    Terminal receiver place join", raised in
    `omega-rust/psi/compilation/pipeline/05_checked-trees-to-lowered-psi/src/retention/suspension_call_plan.rs`
    when a suspension crossing still carries a `receiver`. That carry fact is
    a receiver remnant; give the crossing ordinary parameter identity.
  - Next known stop: `Main::settle` statement 1, `call operation: claim-free
    affine result`. Stage 04's
    `omega-rust/psi/compilation/pipeline/04_typed-trees-to-checked-trees/src/execution/terminal_unit/calls/structural_result_call.rs`
    and `omega-rust/psi/compilation/checking/src/machine_calls/structural_call_custody.rs`
    conflate input transfers with returned claims. The Terminal verifier's
    copy (`.../validation/structural/operations/unit_operation/structural_calls.rs`)
    is deleted by **OMEGA-READS-TERMINAL-UNVERIFIED**; do not extend it.
  Acceptance: the unchanged customer checks and produces a terminal artifact
  through `omega run`. Native execution additionally needs a selected
  `Task::finish` provider; source declarations alone do not supply one.

- **NAMED-IMPLEMENTATION-CONFORMANCE-MIGRATION.** (split-of:STATIC-MACHINE-MIGRATION)
  Replace data-subject satisfaction with named implementations under
  [conformances](wiki/spec/language/conformances.md#data-and-code-are-independent).
  `NormalCounter satisfies CounterOperations<CounterData>` names code, not
  data; `CounterData` is an ordinary trait argument. There is no `Self`, hidden
  first type binder, implicit implementing type or special subjectless variant.
  - Customers: migrate `pass/traits/static_subject_satisfies_receiver_requirement_exit`
    (authored exit 1) and `pass/proofs/named_witness_static_trait_plural_compile`.
    Rename the former for named implementation selection while retaining its
    behavior. Add a runnable pair of implementations over one data type, a
    trait over two unrelated argument types, and a no-data machine bundle.
  - Own parsing, resolution, typing, checking and conformance publication in
    `omega-rust/psi/compilation/pipeline/01_tokens-to-syntax-trees` through
    `07_lowered-psi-to-terminal-psi`, their representations and
    `omega-rust/psi/compilation/checking`; apply the same source contract to
    the product producer in `source/psi/`. Carry implementation identity,
    complete ordinary trait arguments, rows and laws through Terminal codecs,
    interpretation, package review metadata and independent verification.
    Consumers already scheduled for PCC deletion are deleted, not migrated
    or restored; retain source checking and authored mathematics proof checking.
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
    operator wrappers, placed-access families, proof bundles and test-group
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
  Depends on **NAMED-IMPLEMENTATION-CONFORMANCE-MIGRATION** for closed maps.
  The implementation selector and every data argument have distinct identities;
  no argument is an erased subject and no caller requires an attachment.
  - Customers: rewrite `pass/traits/runtime_dyn_two_impl_dispatch_exit`,
    `runtime_local_named_dyn_devirtualized_exit`,
    `runtime_local_named_dyn_pass_through_exit`, and `dyn_finite_family_dispatch`
    to select named implementations and pass data separately. Keep runtime
    branching, descriptor rebinding/forwarding, mutation and result oracles;
    do not reduce all calls to known static selections.
  - Own Psi dynamic checking/facts and stages 04/05/07, Terminal selection/call
    representation and codec, and corresponding `source/psi/` producers.
    Delete the `structural.is_empty()`/caller attachment requirement in
    `composed_control/dynamic_result`, synthetic `is_self` parameters in
    `dynamic_composed_unit`, data-place-to-dyn coercions and the one-`&Self`
    eligibility rule. Bind all types through ordinary trait applications.
    Zero-data calls, two same-type data arguments, two different data types,
    and concrete owned results use the same ordinary call machinery.
  - Forward and store selector identity independently from explicit argument
    loans and places. Entry establishment must see ordinary context parameters
    in dynamic callers rather than recover a hidden receiver. Preserve exact
    rows, contracts, finite families, proof actuals and private selection
    visibility. Package review and independent verification consume the same
    new records; reject obsolete records rather than translate them silently.
  - **RESTORE-DYNAMIC-DESCRIPTOR-AND-TABLE-CUSTODY** owns consuming this model
    in Omega lowering and native emission, including `source/omega/` parity.
    Remove the instance half of descriptors and receiver adapters end to end.
    Apply the breaking-migration authorization above. Hiding input/output types
    is separate work, not a prerequisite and not permission to restore `Self`.
  Acceptance: `python tools/corpus_gate.py --filter traits/` covers the migrated
  programs with interpreter/native result agreement on matching supported hosts.
  A selector changes implementation at runtime while operating on the same
  explicit counter; separate coverage uses zero and multiple data arguments.
  A mismatched trait application rejects, and copying/rebinding the selector
  neither captures data nor changes its borrow lifetime. No legacy dynamic
  subject path remains. Native completion depends on the custody task below.

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
  - Source remainder: boundary-trait requirements in
    `samples/gui/{window_app,window_demo,windowed_calculator}/main.omg`
    (**TRAIT-IMPLEMENTATION-SOURCE-MIGRATION** and **STATELESS-PROVIDERS**) and the two memory canaries
    (**STATIC-BOUNDARY-REQUIREMENT-CALL-FORM**). Build receiver calls remain
    for facets such as test groups. Move their producers and consumers to
    namespace-qualified calls with explicit arguments, following `Build::bind`,
    `Build::select_provider`, `Build::exclude_service`, and
      `Build::select_representation`, `Build::accept_boundary`, and
    `Build::require_wire_compatibility`, and `Build::assume_binding_contract`.
    Do not restore
    implicit builder insertion.
  - Compiler remainder: typed parameters now have only explicit subject
    identities; checking, interpretation, provider arity and review capture
    consume those identities without an implicit receiver parameter. The
    checked structural signatures, Terminal parameters, provider signatures,
    retained-borrow roots and Omega artifacts no longer carry receiver flags.
    Retire the remaining attachment helpers, inert receiver-retention arguments,
    and checked suspension `Persistent` discovery; suspension field origins
    need an explicit parameter-place join rather than a hidden receiver.
    Preserve independent rejection of malformed evidence while removing the
    obsolete representation; do not restore hidden argument insertion. Keep
    unrelated receiver-side artifact admission and mathematical subject
    binding.
  - Domain subjects: `domain Player::Alive(player) requires player.health > 0;`
    parses
    ([domains](wiki/spec/language/domains.md#declaration-and-membership)), but
    `omega-rust/psi/compilation/pipeline/01_tokens-to-syntax-trees/src/declarations/domain.rs`
    rewrites the binder's uses to `ExpressionNode::SelfValue`. Give the binder
    its own identity, separate from generic indices and runtime parameters.
    Add corpus fixtures for generic families, lexical scope and
    binder-renaming identity.
  - Entry context: provision the exact ordinary mutable context parameter of
    the [entry shape](wiki/spec/build/entry_roots.md#entry-shape-and-arrival-bridge),
    preserving ZII eligibility, selected bindings, root lineage, storage
    partitioning and cleanup. Entry provisioning now joins explicit source
    positions; remove remaining attachment-driven discovery without changing
    provider or storage authority.
  - Scratch fields become ordinary locals; pass only live values across
    states. The glider sample now keeps only bindings and its grid in Main;
    its local-buffer migration still needs native and interpreted validation
    on a host supplying its Clock provider. Temporary-scoping design is
    deferred and not a prerequisite.
  - A case dispatch over a computed call subject evaluates it once
    (`pass/control_flow/runtime_computed_case_subject_single_evaluation_exit`),
    but the state graph still declines it when the subject's operands call or
    the subject is a boundary call.
  - Open sample legs: `samples/cli/collections/heat_grid` now carries its running
    maximum as a state parameter rather than a `Grid` field. On Windows at
    `c252be7fa2d1`, package review and `omega --check` succeed; the native gate
    stops at stage 05's integer certificate search capacity (65,536), also red
    before this scratch migration. Follow **CHECKER-DISCHARGES-OPERATION-OBLIGATIONS**
    and **PCC-CERTIFICATES-DELETED**, not another certificate-search extension;
    native and interpreted exit 45 remain required.
    `samples/cli/basics/generic_counters` (exit 16) needs matching-host runs
    on its other declared targets.
  Acceptance: static customers execute with their authored exit/output; the
  migrated corpus and sample gates pass; no accepted receiver syntax,
  implicit receiver argument or receiver-specific signature authority
  remains. Keep the local-owner loan fixtures under `pass/calls/local_*`,
  `pass/calls/static_*`, `pass/borrows/static_*`, `pass/text/local_bounded_text_*`
  and `pass/storage/forwarded_context_byte_field_read_store_exit` at their
  recorded exits.

Prioritize unchanged customer programs reaching native execution over
additional evidence without an exercising program. The
[Rust compiler completion plan](wiki/drafts/reference/rust_compiler_completion.md)
defines the complete product bar.

- **REFERENCE-BEARING-RECORD-LOCAL-FILL.** (split-of:STATE-LOCAL-VALUE-FRONTIER)
  Generalize borrowed-view descriptor stores to an ordinary root plus path,
  including an empty path selecting a whole local. This is delegated Psi
  representation work under [the implementation policy](omega-rust/pipeline.md#psi-implementation-and-deferred-human-audit),
  not an open `borrowed-view-whole-slot-store` language decision.
  - Customers: `pass/text/runtime_string_stored_suffix_exit` (exit 193) and
    `pass/text/runtime_local_struct_string_field_concat_exit` (exit 188).
    The recorded first failure assigns into the uninitialized borrowed-view
    local `out_line`; an assignment of a literal alone exposed the same gap.
    Reproduce that first, rather than conflating it with concatenation backing.
  - Stage 04's `structural_scalar_store/destination.rs::owned_local_root`
    still limits declared structural homes to named/generic/fixed-array forms;
    the downstream descriptor path also requires a final field. Replace the
    root/field split through checked plans, stage 05, Terminal encoding,
    interpretation and Omega lowering. Reuse the ordinary store path instead
    of adding a view-local recognizer. Follow
    [store vocabulary](wiki/spec/terminal-psi/structural_access.md#store-vocabulary).
  - Then resolve concatenation backing and lifetime under ordinary bounded
    storage rules. The current operation materializes an owned result while
    these customers declare only a borrowed view. `Utf8` grants no storage;
    neither an arbitrary maximum nor a literal-flow exception supplies it.
    The recorded initialized-view variant instead reached Omega's control-flow
    observations refusal; reproduce it against the current compiler before
    treating that historical diagnostic as the frontier.
  - Retain descriptor lifetimes, overlapping-write rejection and the controls
    `pass/text/declared_record_view_field_fill_exit`,
    `pass/wire/runtime_wire_utf8_edge_verdicts_exit`, and
    `pass/domains/runtime_qualified_view_field_parameter_store_exit` (exit 70).
    Do not revive Terminal verification scheduled for deletion by the PCC work.
  Acceptance: `python tools/corpus_gate.py --filter text/runtime_string_stored_suffix_exit,text/runtime_local_struct_string_field_concat_exit,text/declared_record_view_field_fill_exit,domains/runtime_qualified_view_field_parameter_store_exit`
  covers first initialization, reassignment and the two authored exits in native
  and checked-interpreter execution, with invalid lifetime/read-before-establishment
  controls. The closure must preserve backing through surviving copies.

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
    `EstablishStructuralValue` route. Stage 05's
    `omega-rust/psi/compilation/pipeline/05_checked-trees-to-lowered-psi/src/machine_lowering/machine_dispatch.rs`
    still refuses "scalar and Unit dynamic joins compete for one caller";
    `.../src/unit/structural_unit_control.rs` still refuses checked
    expressions in its scalar-successor route. Remove those through ordinary
    argument evaluation, not another whole-body plan. Calls inside a record
    literal's tail fields (`Handle { name: Item::name(item), .. }`) decline at
    `outer calls: ordered statement call`
    (`.../04_typed-trees-to-checked-trees/src/execution/terminal_unit/control/call_occurrences.rs`);
    add a corpus fixture. Selected expression-owned calls need edge-specific
    cleanup when transferring earlier caller owners. Structural values with nominal
    cleanup hooks or reference debt need the ordinary disposal schedule;
    nontrivial cleanup must not become a trivial affine discard.
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
    requires an owned activation home and rejects reference-bearing, linear,
    claimed or qualified replacement values; complete those cases, including
    values arriving as owned parameters/block arguments, without double
    disposal or a fabricated move window. Verify empty-record locals both
    forwarded and discarded. Unbounded `UInt`/`Int` fields have no runtime
    representation and stay rejected. `pass/filesystem/windows_canonicalize_exit`
    is recorded rejected ("performed write has unknown destination origins")
    and needs its stored UnitResult; coordinate **FILESYSTEM-RELEASE-CONTRACT**,
    **WRITE-ONLY-BORROW** and **NOMINAL-FIELD-FLOW**. Keep:
    `pass/collections/case_array_literal_field_store_exit`,
    `pass/expressions/borrow_carrying_data_field_exit`,
    `pass/borrows/stored_reference_projection_exit`,
    `pass/borrows/nested_reference_crossings_exit`,
    `pass/borrows/runtime_reference_array_pressure_exit` (70; call-free
    frames preserve allocated link-register writes as well as call effects;
    at `870548dcfe2` on macOS ARM64, the direct command
    `omega run --both tests/omega/pass/borrows/runtime_reference_array_pressure_exit/main.omg`
    also exits 70 in both engines after ordinary `omega update` review and
    acceptance. The earlier missing `MacosApplication` was an unlocked
    package's absent accepted binding, not missing compiler implementation.
    A changed source still rejects with `omega update` guidance; restoring
    the original source restores execution. Follow the
    [CLI fixture workflow](omega-rust/omega/compilation/driver/tests/README.md#running-a-fixture-through-the-cli)),
    `pass/collections/runtime_whole_array_value_copy_exit` (70),
    `pass/collections/{empty_record_array_borrow_exit,nested_empty_record_array_borrow_exit}` (0),
    and rejections `fail/borrows/{stored_reference_live_source_write,nested_reference_live_source_write}`.
  - **Claim-bearing results and native receiving.** The checked state planner
    refuses nonempty call claim transfers ("state graph: operation custody:
    call claim transfers" in `.../execution/terminal_unit/state_graph/mod.rs`).
    Carry claim transfers on calls before a composed structural return;
    extend returned custody to extracted projections, fresh claims and claims
    from distinct owned inputs. Omega stage 02 still rejects entry claims and
    observed owned-linear parameters, and wider/nested structural results
    (indirect/stack ABI slots such as Windows x64's 12-byte Msg) do not reach
    target lowering. Add corpus fixtures for a qualified linear structural
    result with entry claims and for two distinct result owners with ordered
    residual cleanup; `pass/effects/structural_callback_reach/projected.omg`
    is only a control.
  - **Saved observations and arithmetic.** Stable unwritten mutable carriers
    contribute saved facts in
    `omega-rust/psi/compilation/checking/src/proof_contracts/contract_entailment/ranking_range/saved_arguments.rs`;
    finish snapshots across writes and exclusive exposure, retaining
    case/index/generic/reference/float identity. Share capture with
    **CRASH-CONTRACT**; loop invariants must cover backedges. Exclusive computed
    ranges use ordinary view formation and call/state-edge evaluation:
    the four `pass/slices/runtime_call_endpoint_*` fixtures return 70 in both
    engines on macOS ARM64 (stored view, direct argument, ordered endpoint
    effects and projected state transfer). Keep
    `python3 tools/corpus_gate.py --both --filter runtime_call_endpoint`.
    Keep the same route's inclusive controls with
    `--filter runtime_inclusive_call_endpoint` (both return 70), plus
    `fail/slices/inclusive_*` for bounds and overlapping loans. Public acceptance is
    `omega run tests/omega/pass/slices/runtime_inclusive_call_endpoint_view_exit/main.omg`
    after the project's package review. Explicit reborrows
    now preserve address origins across shared-value and write-prefix queries;
    `layouts/runtime_reborrow_preserves_store_origin_exit` returns 70 in both
    engines. The public stored-view command checks the standard library, then
    awaits 52 fresh project package-review decisions, not a compiler waiver.
    Direct collection extents now compose through borrow and range formation
    in `omega-rust/psi/compilation/checking/src/proof_contracts/contract_entailment/ranking_range/lengths.rs`.
    At the `45782ade0cb`-based repair on macOS ARM64,
    `pass/slices/runtime_direct_view_requires_index_exit` passes source
    checking and interprets to 70 unchanged, but `python3 tools/corpus_gate.py
    --both --filter runtime_direct_view_requires_index_exit` still fails native
    production with `OperationProofUnavailable(ObligationId(9223372036854775808))`.
    This customer depends on **PCC-CERTIFICATES-DELETED**, after
    **CHECKER-DISCHARGES-OPERATION-OBLIGATIONS**; do not extend late certificate
    search. Its expected native exit remains 70. Carry exact
    anonymous arithmetic through generic, boundary, aggregate, parameter,
    float and proof uses with result carrier/policy and warning origins.
    **OPERATOR-MACHINE-SUPPLY** owns executable selected helpers.
  - **Patterns and dispatch.** Record/case destructuring executes bind-all,
    `as _` waivers, `__` field names, renamed fields and rest opt-out, with
    non-place subjects evaluated once. Unverified: native exit of
    `pass/control_flow/runtime_nonplace_record_pattern_single_evaluation_exit`
    now that it passes its context explicitly. Execute a consumer of a
    selected case payload transferred on an edge
    (`.../state_graph/emission/successor_edge.rs`). Keep
    `pass/data/record_pattern_let_exit`,
    `pass/control_flow/{ordered_unconditional_arm_exit,record_pattern_arm_rename_guard_exit,arm_pattern_rest_optout_exit,case_pattern_rename_waive_exit}`,
    `pass/borrow/runtime_exclusive_loan_local_write_exit` and
    `pass/calls/runtime_value_call_nested_entry_call_exit` (exit 70; needs
    matching-host coverage).

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
  - Where: `OpenWindows::replace` in
    `omega-rust/psi/compilation/pipeline/04_typed-trees-to-checked-trees/src/execution/terminal_unit/borrowed_windows.rs`
    separates exact-place restoration debt from detached value custody;
    direct and named call results already use ordinary owned result
    bindings.
  - Missing: multi-block transport (composed-control emission requires
    closure after each operation sequence), keeping evaluation order,
    disjoint siblings, exact open-place agreement and contained loans.
    Indexed paths work; whole-root, referent, scalar-field and nominal-cleanup
    restrictions stay until their evidence exists.
  - Consumers: the product parser and **BUMP-ALLOCATOR-CANARY**.
  Acceptance: consuming transforms, replacement values and repair on both
  branches execute with caller-visible updates and exact-once custody in the
  checked interpreter and supported native targets, without staging caller
  storage. Keep `pass/ownership/runtime_borrowed_record_transform_exit`
  (exit 42) and `pass/ownership/move_keyword_field_assignment` (exit 41).
  Missing repair, early return, stale reads, overlapping loans, wrong-place
  repair, repeated extraction and whole-owner cleanup reject, including
  `fail/ownership/borrowed_storage_boundary_call`. Cover recoverable failure,
  suspension/resume/cancellation and crash/process-exit abandonment without
  invented rollback.

- **CLEANUP-HOOK-SELECTION-AND-ERASED-OWNERSHIP.** Run nominal cleanup for
  dying local owners at return, composed with contextual hooks and ordinary
  consuming `drop<T>`, under
  [nominal cleanup](wiki/spec/terminal-psi/ownership.md#nominal-cleanup).
  - Where: stage 04's
    `omega-rust/psi/compilation/pipeline/04_typed-trees-to-checked-trees/src/execution/terminal_unit/control/checked_machine.rs`
    still declines locals whose type needs a nominal drop
    (`type_graph_requires_nominal_drop`) at return. Stage 05 already retains
    exact caller/hook prerequisites through ordinary consuming helpers.
  - Keep exact selected-exit, named-transition and operand-time ownership,
    exact-place premises without invented caller demands, and outgoing result
    homes. **CML4** owns residual partitions and mixed-root order;
    **PROOF-RELEVANCE-MIGRATION** owns erased formal/actual transport.
  - Cleanup prerequisites are call contracts, distinct from primitive
    operation conditions. **CHECKER-DISCHARGES-OPERATION-OBLIGATIONS** owns
    the remaining operation gate; add no cleanup-specific proof machinery.
  Acceptance: add a corpus fixture where a local owner with a nominal hook
  dies at return and at a named transition, running its hook once in order
  natively and in the checked interpreter. Keep the passing controls
  `pass/drops/{core_drop_explicit_consume,core_drop_owner_hook,core_drop_owner_hook_body}`,
  `pass/drops/erased_record_initializer_exit` (70),
  `pass/drops/{contextual_cleanup_exit,contextual_core_drop_exit,conditional_structural_return_cleanup_exit}`
  (`BA`, exit 0), and source rejection of unmet hook prerequisites and
  duplicate consumption.

- **CML4.** Complete edge-local residual cleanup under the
  [ownership contract](wiki/spec/terminal-psi/ownership.md). Materialize
  outgoing values before transfer/disposal; give every occurrence one
  disposition. Clean dying roots in reverse declaration order and abandoned
  construction prefixes in
  [reverse establishment order](wiki/spec/language/ownership.md#construction-and-disposal-order).
  Crash/abort/process exit has no cleanup successor. Whole-root conditional
  scalar returns preserve reverse parameter order, covered by
  `pass/borrows/runtime_affine_local_case_return_exit` (42) and
  `pass/borrows/runtime_case_return_parameter_order_exit` (0), both native and
  interpreted. The remaining frontier is residual cleanup:
  - Transport `build_projected_edge_plan`
    (`omega-rust/psi/compilation/pipeline/04_typed-trees-to-checked-trees/src/execution/terminal_cleanup.rs`)
    through `.../execution/terminal_unit/state_graph/` and stage 05's
    attached Unit graph. Composed `Pair.left` transition programs lack
    admitted plans. Extend to partial construction, a second partial
    parameter and mixed dying roots; keep machine parameter custody distinct
    from edge cleanup. No runtime liveness flags, untouched-array leaf
    expansion or deferral to final return.
  - Add residual cleanup to conditional/structural-case successors and
    structural returns: `SuccessorEdge`, `StructuralCaseSuccessorEdge` and
    `ReturnStructural`
    (`omega-rust/psi/compilation/representations/terminal-psi/src/terminal_module/control_flow/termination.rs`)
    carry only whole-root discards; `ReturnUnitPartialAffine` exists. Change
    producer, representation, interpreter and native consumers together.
  - Generalize temporary cleanup beyond empty effect-free Unit consumers
    (other argument effects, non-Unit results, borrowed/owned mixtures,
    claims) on **STATE-LOCAL-VALUE-FRONTIER**'s evaluation path.
  - Native: `accepts` in
    `omega-rust/omega/compilation/pipeline/02_abstract-operations-to-target-operations/src/lowering/unobserved_owned.rs`
    suppresses arrival homes with a whole-function gate; replace it with
    per-place storage/action evidence. **CLEANUP-HOOK-SELECTION-AND-ERASED-OWNERSHIP**
    owns executable hooks.
  Acceptance: add corpus fixtures where native runs and the checked
  interpreter preserve values and hook order across composed edges, including
  a projected-argument jump edge closing residuals in order. Missing,
  duplicate, reordered, overlapping, wrong-root and post-transfer cleanup,
  and transfer after disposal, reject. Fuel exhaustion commits no disposal;
  retry cannot repeat one. No affine occurrence outlives its required edge.

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
  - Source/Terminal event production for bounded atomic fields, fences and
    single-attempt exchange: abstract fence/single-attempt carriers exist but
    have no Terminal producers. A fence stays an event even when realization
    emits no instruction.
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
  samples (`samples/uefi/uefi_hello`) need their own runner. `omega
  refresh-samples` publishes but does not run. Reproduce with the current CLI
  and the sample's own package/entry inputs; a stale binary, an earlier review
  refusal or an effect-free reduction does not exercise the same route. A trace
  phase records the last route entered and can misidentify the declining
  operation.

  Samples that build no native program on macOS ARM64:
  - `games/dungeon_crawler_cli`: checks, then stops at `ControlCycleIneligible`
    in Terminal verification; native build cost is
    **STATIC-DUNGEON-NATIVE-BUILD-COST**, field domains **NOMINAL-FIELD-FLOW**.
  - `systems/file_journal`: package review exits 200 because `Main::fs` /
    `Main::files::host` have no Darwin `FilesystemHost` provider
    (`omega-rust/omega/compilation/realization/provider-planning/src/provider_settlement/canonical_filesystem_host.rs`
    covers limited Linux x86-64 leaves). Owned by
    **FILESYSTEM-PLATFORM-SERVICES**. Acceptance: the seven-step workflow,
    exit 7.
  - `systems/note_vault`: stops in `Filesystem::create_dir_all` ->
    `Filesystem::mkall_walk` state 0 with "qualified non-linear parameter":
    stage 04 `forwards_qualified_loan`
    (`omega-rust/psi/compilation/pipeline/04_typed-trees-to-checked-trees/src/execution/terminal_unit/state_graph/mod.rs`)
    refuses a qualified loan (`path: &[u8] in Path`) re-entering state 0. Then
    the same `FilesystemHost` provider. Result custody and nested-field
    initialization stay with **STATE-LOCAL-VALUE-FRONTIER**.
  - `gui/{image_viewer,window_app,window_demo,windowed_calculator}`: see
    **MACOS-APPLICATION-PUBLICATION**.

  Host coverage still owed:
  - std `Clock` on Linux (`rendering/{bouncing_ball_2d,mandelbrot_zoom,bouncing_particles}`):
    `tests/omega/pass/host/std_clock_sleep_exit` exits 70 on macOS ARM64 and
    builds for both Linux targets; it needs actual 200 ms waiting and exit 70 on
    each Linux host, as does `tests/omega/pass/providers/kernel_import_result_chain_exit`.
  - Clock-bearing samples pass on macOS ARM64; verify the other declared hosts:
    `algorithms/{binary_search_viz,sort_visualizer}`,
    `rendering/{bouncing_console,ripple_field,tick_marquee}`,
    `simulation/{cellular_automaton,game_of_life_glider,langtons_ant,random_walk}`.
  - `arithmetic/stats_compute` runs on macOS ARM64 with its PASS line and exit
    70 (`5094be283192`, optimized CLI, closed stdin). Windows x86-64 and both
    Linux hosts still need the same [runtime oracle](samples/cli/arithmetic/stats_compute/README.md#runtime-acceptance);
    non-tail self recursion is not a reproduced blocker.
  - Text/index/match programs: missing operations belong to
    **NOMINAL-FIELD-FLOW**, **WRITE-ONLY-BORROW**, **STATE-LOCAL-VALUE-FRONTIER**,
    **MATCH-SELECTIVE-LOWERING**, **CML4** and **OPERATOR-MACHINE-SUPPLY**. Keep
    `Utf8` and ordinary domain evidence; recognized function names or weakened
    invariants cannot replace it.

  Distinct product oracles (paths under `samples/cli`); each needs Windows
  x86-64 and both Linux hosts while retaining macOS ARM64:
  - `cli_mvp`, `euclid_gcd`, `generic_counters`, `number_guess`: exact output,
    exits 0/12/16/70; interactive EOF/Enter behavior, both lines, prompt timing,
    empty stderr.
  - `basics/print_squares`: rows `001,004,...,081` in order, exit 0, EOF/Enter
    completion; the banner's `081` alone is not an oracle.
  - `recursive_sum`: exact 144-byte output, exit 70.
  - `systems/framed_payload`: the [exact-output oracle](samples/cli/systems/framed_payload/README.md), exit 60.
  - `simulation/calendar`: the 277-byte/exit-0 oracle in
    [its README](samples/cli/simulation/calendar/README.md); a banner containing `30` is insufficient.
  - `longest_run`, `parse_int`, `roman_numeral`, `string_hash`, `substring_search`:
    exits 4/150/70/40/4 per the [scanner](samples/cli/algorithms/longest_run/README.md)
    and [text](samples/cli/text/README.md) READMEs.
  - `proofs/math_proofs`: ordinary multiset extraction and checked proof terms;
    the false twin rejects. `source/library/core/seq.omg` is not a Bag.
  - Bounded Console input under [bounded input](wiki/spec/resources/bounded_input.md):
    exact prefix/destination, once-only effects, LF/EOF/Full, zero-capacity
    non-consumption, untouched tails, alias rejection.

  Unverified: `gui/windowed_calculator --check` took 20-55 s; reproduce on a
  current binary and locate the cost before attributing it.

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
  - Reconcile added/removed fixtures with the check and per-host native
    goldens after attributing their outcomes. Example: `pass/collections/runtime_shared_array_{dot_product,polynomial}_exit`
    have only `corpus_both_macos_arm64.txt` rows.
  - Keep one portable check golden without hiding semantic differences:
    host-selected providers can change a refusal; attribute it rather than
    repin one host over another. Keep missing rows, wrong-reason negatives,
    crashes and timeouts visible.
  - Fixture package locks and admissions stay local; a committed foreign
    `omega.lock` blocks the ordinary command
    (`pass/calls/local_records_mixed_record_result_arguments_exit` README).
  - Restore runtime promises lost with deleted per-feature tests.
    `pass/filesystem/native_wrapper_write_all_result` has no run suffix or exit
    file and is rejected natively on all three hosts; on macOS ARM64 it must
    store and dispatch the write result (nonexistent parent -> Error, `/tmp` ->
    Ok, stdout PASS and no FAIL). Distinct from **FILESYSTEM-RELEASE-CONTRACT**.
  - Separate proof/declaration fixtures from executable customers:
    `pass/terminal_psi/content_custody_exit` invokes `Root::enter` but has no
    `build.omg` or oracle; `pass/core/content_{conservation_contract,retained_custody_round_trip}`
    need an invoking customer (routed establishment: **BOUNDARY-ISSUANCE**);
    `pass/proofs/quotient_{define,lift}_managed_compile` stay proof-only, with
    the runtime witness owed by **QUOTIENT-RUNTIME-REALIZATION**.

  Diagnostic acceptance:
  - **IMPLICIT-STORE-CONVERSION** owns duplicate conversion/weakening reports in
    `fail/calls/terminal_return_type_mismatch_rejected`,
    `fail/expressions/bool_numeric_operand_mixing_rejected`,
    `fail/arithmetic/{suffix_type_disagrees,undeclared_bare_name}_rejected` and
    `fail/range/indexed_field_range_runtime_write_rejected`. Emit stable source
    subjects instead of arena IDs in
    `fail/expressions/token_bound_machine_duplicate_shape_rejected` and quotient
    rejections; attribute `fail/operators/selected_crash_invocation` to the
    authored operator invocation.
  - **ENTRY-SLOT-EXACT-SCHEMA-SELECTION** owns the remaining accepted-package
    physical-convention mismatch customer. The local-schema negative now
    rejects its invalid register architecture; it does not select the target
    entry contract or prove the selected-convention comparison.
  - `fail/providers/checked_boundary_operator_const_application_unselected`:
    recorded diagnostic and `expected.txt` disagree; it must reject for missing
    exact authored selection (**OPERATOR-BOUNDARY-CALLS**).
  - Keep `fail/arithmetic/narrowing_anonymous_expression_unproven` (range
    obligation) distinct from `narrowing_signedness_rejected` (carrier), and
    the `fail/modules/duplicate_data_keeps_guard_facts` control.

  Shared failures, by owner (rerun the unchanged fixture after each repair):
  - **OPERATOR-BOUNDARY-CALLS**: `pass/float/{x86_fma_plan_association,build_runtime_semantics_twins_x86_baseline,build_runtime_semantics_twins_windows_x64,named_provider_directed_divide_exit,named_provider_fused_multiply_add_exit,named_provider_multiply_then_add_exit}`,
    `pass/expressions/match_float_{interpretation,patterns,subjects}`,
    `pass/providers/checked_boundary_operator_const_application_alternate_exit`;
    `pass/float/{float_saturating_arithmetic_exit,float_trapping_overflow_traps,float_trapping_propagated_nan_traps}`
    need their authored policies (`as f32` erases the subject). Exact-cast
    guards in `pass/float/float_to_int_exact_proofs_exit` need one comparison
    recognizer shared by Binary expressions and token-bearing calls.
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
  - **PROOF-CONTRACT-MIGRATION**: the local theorem repair checks
    `pass/proofs/cauchy_predicates_compile` -- note its frontier below
    describes an UNPUBLISHED checkpoint; on main that fixture is `rejected`,
    and its second diagnostic names the blocker the compiler itself cannot
    satisfy: an `ensures` fact over proof-only `Nat` that "no entailment tier
    judges yet", directing the reader to "wait for the structural extraction
    tier (math roster N3)". **That roster is undocumented.** N1 through N4 are
    cited in six places in code
    (`checking/src/proof_contracts/contract_entailment.rs:413,859`, its
    `stand_downs.rs:46` and `citations.rs:743`, stage 03's `equality.rs:271`,
    `structural_equality.rs:346` and `lowerer.rs:33`) and in
    `samples/cli/proofs/structural_proofs/README.md`, but the tiers appear
    nowhere in `wiki/spec/` or on this board, so a user who reads that
    diagnostic has nothing to look up and no tracked item to follow. Either
    document the roster where the diagnostic can cite it, or stop naming an
    internal taxonomy in user-facing text. Until then this fixture is blocked
    on an untracked tier, not on the repair this bullet describes; publication
    still needs the native
    `pass/proofs/runtime_static_result_projection_citation_exit` witness and
    generalized fact-only citation erasure. See the owning item's frontier.
    Keep exact bounds in
    `pass/proofs/proof_inductive_climbing_sum` and its unbounded negative.

  Native customers (under `tests/omega/pass`):
  - `text/runtime_local_struct_string_field_concat_exit` (188),
    `text/runtime_string_stored_suffix_exit` (193): rejected on all hosts;
    re-measure under **OMEGA-READS-TERMINAL-UNVERIFIED**, then
    **REFERENCE-BEARING-RECORD-LOCAL-FILL**. Keep
    `traits/equatable_string_field_equality_exit` and
    `text/runtime_mixed_byte_field_equality_exit` (exit 70 on macOS ARM64).
  - `host/runtime_console_bounded_line_exit` and its
    `_{lf,full,partial_eof,middle_window}_exit` siblings: exits 20/13/34/22/70
    with exact stdout on macOS ARM64; remaining Windows and Linux.
    **WINDOWS-CONSOLE-INPUT** owns the Windows input dependency; output
    remains under the process-exit contract's Windows byte-leaf gap.
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

  Measured frontier at `ca03765f46c` on Windows: the interpreter exits 70;
  native realization rejects the selected Console `read_byte` intrinsic
  because it has no Windows native catalog identity. The archived draft at
  `cfe60a3a242` runs a checked adapter over private DLL requirements natively
  with exit 70 but makes the interpreter decline. That draft cannot land.
  Implement the input provider
  in `source/library/std/targets/windows_x86_64/console_impl.omg` and supply
  interpreter input operations from exact selected imports through
  `omega-rust/omega/compilation/realization/provider-planning/` and
  `omega-rust/psi/interpreter/checked-interpreter/`. Execute the checked adapter;
  a provider spelling or matching signature cannot bypass its body or grant
  host behavior. Distinguish input bytes, EOF, transient empty pipe reads and
  failures without treating every successful zero-byte read as EOF.
  The unchanged ordinary Loader `omega run --both` route exits 200 at this
  revision with 14 pending std package-review decisions; explicit package
  acceptance is also required before that route can establish execution.

  Acceptance: `python tools/corpus_gate.py --both --filter
  calls/runtime_mutable_call_before_transition_args_exit,host/runtime_console_bounded_line`
  preserves the Loader's exit 70 and the five interpreted exits
  20/13/34/22/70 with authored stdout. Attribute their existing native output
  refusals separately. Exercise empty input, raw bytes and invalid input
  handles, and establish the Loader's ordinary `omega run --both` route;
  unresolved package admission is a named dependency, not runtime success.
  Shared interpreter integration waits for the active accepted-Build claims
  to clear, as directed by the owner.

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

  Compiler-side frontier:
  - macOS ARM64 integration at Omega candidate `8b9cb8dcc9a`, published
    application `43d56a7`: the ordinary check command rejects the version-3
    lock. Local application checkpoint `ffee88a796f1` migrates 105 Build calls
    in 28 files and pins std to published Omega `944ee0efb38f`; layout retains
    17 packages and 37 internal edges. With the historical lock preserved,
    `omega update --project samples/apps/squalr/squalr-tests --target
    linux_x86_64` now reaches source checking and rejects `&self` at
    `squalr-engine-api/src/commands/command_invocation.omg:53`. The lock was
    restored exactly. Continue machine/state declaration and call migration
    under **STATIC-MACHINE-MIGRATION**, then perform fresh ordinary review;
    never enable implicit receivers or transplant approvals. The candidate is
    retained in `.codex/worktrees/squalr-build/samples/apps/squalr` and is not
    published: `git push --dry-run origin HEAD:refs/heads/main` still receives
    HTTP 403 for the configured identity. Repository write access is required
    before updating the gitlink. Source checking is not native acceptance.
  - The explicit-parameter form of `ProcessInfo::set_name` now checks and
    executes on macOS ARM64: `pass/slices/runtime_guarded_view_copy_cycle_exit`
    exits 70, checking copied bytes, untouched tail, zero limit, and input
    extent. Its guarded `requires ... position < name.len` arrival is not a
    reproduced compiler blocker. This control does not establish execution of
    the unmigrated application; rerun its command after source migration.
  - Unverified: the earlier result-store stop in stage 04
    `omega-rust/psi/compilation/pipeline/04_typed-trees-to-checked-trees/src/execution/terminal_unit/control/call_results.rs`
    (`EnginePrivilegedState` task slots holding `Binding<WaitWake>`); empty
    slots need no fabricated binding and transport must keep routed authority.
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
  a satisfier declares no parameter its requirement lacks). The `self`
  migration turned receiver state into a hidden `&mut Provider` parameter
  that the requirement does not declare. For example,
  `samples/gui/window_demo/main.omg`'s `data DemoGuiProvider { window, view,
  app, space, … }` with
  `DemoGuiProvider::window_create(demo_gui_provider: &mut DemoGuiProvider, …) satisfies Gui::window_create`.
  - Redesign `Gui` (`source/library/std/macos_gui.omg` and the samples'
    boundary traits) so `window_create` returns a `Window` record holding the
    handles, selectors and autorelease pool; `blit`, `msg_peek` and the rest
    take it explicitly, and teardown consumes it. Migrate `window_app`,
    `window_demo`, `windowed_calculator` and `image_viewer`, plus any other
    satisfier with provider fields (find them by the extra leading provider
    parameter).
  - Make conformance checking reject a satisfier parameter the requirement
    lacks, and delete the provider-subject provisioning: the installation
    adapter and checked interpreter call the satisfier with exactly the
    requirement's arguments. Stage 05's "checked provider receiver requires
    installation storage or qualification provisioning" refusal
    (`omega-rust/psi/compilation/pipeline/05_checked-trees-to-lowered-psi/src/unit/attached_unit/providers.rs`)
    goes with it.
  - A foreign-bound provider whose fields hold vtable addresses
    (`ForeignBinding::VtableField`) takes those from its foreign binding plan,
    not from provider data. **TRAIT-IMPLEMENTATION-SOURCE-MIGRATION** owns
    the requirement conversion under the settled named-implementation contract.

  Acceptance: no satisfier declares a parameter its requirement lacks (a fail
  fixture pins the rejection); the four GUI samples check;
  `tests/omega/pass/providers/runtime_boundary_primitive_array_argument_exit`
  still exits 70 in both engines.

- **MACOS-APPLICATION-PUBLICATION.** Publish and run `samples/gui/window_app`,
  `window_demo` and `windowed_calculator` under the
  [macOS application contract](wiki/spec/build/macos_application.md). Reuse
  `omega-rust/omega/compilation/backend/artifacts/native-artifact/src/package_publication.rs`
  for package assembly and signing, and
  `omega-rust/omega/compilation/products/src/compilation_products.rs` for
  package/inner-executable accessors. The samples declare nominal
  GUI/clock/input providers and macOS `provider_defaults`; preserve them.

  All three check (`python tools/sample_gate.py --filter
  gui/window_app,gui/window_demo,gui/windowed_calculator`). Native production
  stops inside `*GuiProvider::window_create`:
  - `window_demo`, `windowed_calculator`: stage 05
    `omega-rust/psi/compilation/pipeline/05_checked-trees-to-lowered-psi/src/unit/attached_unit/structural_calls/result_uses.rs`
    ("invalid claim-free custody") at `objc_get_class(&cbuf1[0])`, a shared
    loan of a local array's fixed element. Behind it:
    `unit/attached_unit/parameters.rs` ("invalid checked custody"); the
    by-value parameter element `&title[0]` ("primitive array parameter requires
    exact whole owned transport"); and Omega's normalized foreign call, which
    admits only caller-parameter-rooted loans. The Terminal verifier's
    local-array element-loan refusals go away with
    **OMEGA-READS-TERMINAL-UNVERIFIED**.
  - `window_app`: the stage 05
    `omega-rust/psi/compilation/pipeline/05_checked-trees-to-lowered-psi/src/unit/attached_unit/call_catalog.rs`
    fence on write-only stores and atomic events in opaque provider candidates.
    Admit them only with a checked judgment preserving non-observation and
    exact access to places exposed by the requirement's mutable parameters.
  - Behind both: **STATELESS-PROVIDERS**. The GUI providers keep window
    state in provider fields, which the static-machine contract forbids.
  General lowering goes through **ENTRY-CONTENT-ROOTS**, **TR3-TR8** and
  **STATE-LOCAL-VALUE-FRONTIER**.

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

  Current native witness:
  `omega run tests/omega/pass/termination/runtime_saved_rank_cycle_exit/main.omg`
  exits 0 natively and interpreted on macOS ARM64 with saved scalar values,
  named-state requirements, unrelated nominal operators and disjoint writes.
  Base `abaa2a1889d` rejected the saved operand when an unrelated subtraction
  declaration was visible; early operator selection now reads its declared
  type through the shared binder lookup. This supplies no saved-value proof.
  Unproved arrivals, overwritten requirement inputs and actual `u32` overrides
  still reject; machine-entry requirements are not imported at internal arrivals.

  Remaining:
  - Replace unique-carrier discovery with explicit arrival correspondence,
    reusing nested carriers, moved scalar/slice/record copies and produced rank
    facts; a same-typed or merely decreasing copy is not the ranked value.
    Mixed ranged/unranged components need correspondence and endpoint
    conservation. Recheck formation at every arrival: cancellation, equal
    intervals or unoriented disequality do not prove divisor validity, overflow
    freedom or identity. The interval-only fallback still needs one state.
  - Execute selected-signature write frames for boundary/requirement/admitted/
    external calls and disjoint frames for direct, nested and composed
    initializers through the same provider contracts; add corpus fixtures for
    requirement reads versus exclusive writes in prefix `let` call bindings.

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
    plus follow-ups. Record original/current spelling, owning position and
    restoration or reason for retention, including bare fields whose bounds
    were removed because inferred loop bounds made execution pass. A suffix
    grep or passing replacement does not establish preservation.
  - Field/case propositions through construction, nonliteral and indexed
    writes, reads, calls, state joins, mutable recasts (the
    [representation judgment](omega-rust/psi/compilation/checking/recasts.md))
    and returns (**NOMINAL-FIELD-FLOW**, **STATE-LOCAL-VALUE-FRONTIER**). Re-drive
    `pass/core/zii_default_composite_exit`,
    `pass/collections/runtime_dutch_flag_partition_exit`,
    `pass/calls/runtime_mutable_call_before_transition_args_exit`, the
    `pass/recast/runtime_*_mutable_write_exit` family and the sample field
    migrations; reproduce before attributing failures to proposition checking.
    `pass/float/float_to_int_exact_proofs_exit` (data `where`, exit 78 on macOS
    ARM64, rejected natively on Linux/Windows) keeps its invalid-write control.
    Constructor bounds now use the shared occurrence environment in
    `checking/src/value_custody/struct_literals/construction_bounds.rs`, not
    the retired immutable-operand filter or obsolete local initializers.
    `python3 tools/corpus_gate.py --native --filter data/runtime_constructor_observation_exit`
    executes with exit 70 on macOS ARM64; stale-local and earlier-operand
    mutation controls reject. Ordinary CLI package preparation still requires
    this new fixture's local package review; no acceptance policy is committed.
    Continue with the nonliteral/indexed writes and consumer transports above;
    this does not close the complete field/case migration.
  - Guarded immutable-local, guarded-field, nested-call-result and
    destructured-payload facts into `requires`; entry premises into `ensures`
    formation; state results into enclosing postconditions. Customers:
    `pass/calls/runtime_dispatch_binary_call_argument_exit`,
    `pass/calls/nested_value_call_arg_compile`,
    `pass/arithmetic/anonymous_rational_arguments`,
    `pass/proofs/accumulator_guarantee_exit`,
    `pass/termination/ranked_callee_projected_receiver_compile` (still has
    suffixes). A mutable parameter's `requires` constrains arrival where the old
    suffix was store-enforced: re-prove dependent reads after a write
    (`pass/termination/rank_range_mutable_bounded_endpoint` plus negative
    mutation controls).
  - Library: `source/library/core/numeric_conversion.omg` (suffixed widening
    results), `source/library/std/console.omg` (`ByteRead::Byte` payload and
    state parameters), `source/library/std/time.omg` (subsecond fields/locals,
    constants), `source/library/core/layout.omg` (fuel/index),
    `source/library/std/calling.omg` (indexed fields) and target-entry
    `field_range` parameters; prove computed `fuel - 1` / `index + 1`
    arguments and equivalent guards (`< 255` vs `<= 254`). Keep
    `pass/host/runtime_console_bounded_line_exit` behavior.
  - Migrate `fail/wire/wire_policy_plan_disagrees` keeping its plan/schema
    disagreement rejection.
  Canonical controls to keep: `pass/collections/runtime_argmax_index_exit`,
  `pass/collections/runtime_nested_const_row_indexed_read_exit`,
  `pass/arithmetic/runtime_guard_narrowed_transition_arg_exit`,
  `pass/arithmetic/runtime_exact_guarded_shift_count_exit`,
  `pass/termination/rank_range_{declared_step_copies,call_declared_step_copy}`.

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

  The Dungeon Crawler is migrated to explicit parameters; its native build is
  blocked on **STATIC-DUNGEON-NATIVE-BUILD-COST** and its cycle on
  **GENERAL-CYCLIC-EXECUTION**.

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
    `pass/text/nested_mutable_receiver_byte_result_exit`. Guarded and
    closed-case returns keep physical view cleanup and semantic custody.
  - The migrated application runs closed stdin and `help`/`exit` per
    [its README](samples/cli/games/dungeon_crawler_cli/README.md#runtime-acceptance)
    with native exit 0 and the authored room, prompt, command and goodbye output.
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
  and expression preparation. The optimizer board's same-name task owns native
  receiving.

  Executing controls: `pass/text/runtime_number_to_decimal_exit` and
  `pass/text/runtime_bounded_carrier_write_read_exit` (exit 70);
  `pass/collections/runtime_shared_fixed_array_cycle_exit` (42),
  `runtime_shared_array_dot_product_exit` (32),
  `runtime_shared_array_polynomial_exit` (70) with exact stdout, mirroring
  `samples/cli/{collections/array_max,arithmetic/dot_product,algorithms/horner_eval}`;
  acyclic control `pass/collections/runtime_borrowed_fixed_array_state_reads_exit`.

  Remaining cycle shapes are refused by the Terminal verifier's fences in
  `omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/control_flow/unranked_cycles.rs`
  (deleted by **TERMINAL-VERIFIER-DELETED**): broader qualified/partial
  ownership, reference-bearing or claim-carrying structural call results,
  moving claims, content reshuffles/partitions, broader structural-result
  custody, boundary completion receipts, and claim transfers through Unit/scalar
  calls. Each needs stage 04 to check the arrival/custody relation and stage
  05 to lower it; deleting the fence alone is not the repair.
  `samples/cli/games/dungeon_crawler_cli` stops at `ControlCycleIneligible`.
  Squalr's earlier `ProcessInfo::set_name` arrival failure is no longer
  reproduced by its explicit-parameter copy graph:
  `pass/slices/runtime_guarded_view_copy_cycle_exit` checks and runs with exit
  70 on macOS ARM64 at `6ba9c43af356`. The loop transports the shared input,
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

  - Selected-use joins beyond IEEE/integer comparisons: structural telescopes
    and call carriers need exact operand mapping and provider commitment;
    other intrinsic spellings and structural signatures still hit the custody
    gate's refusals; float ordering, generic and structural-formal routes need
    portable predicates and execution, keeping the original guard.
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
    exceptions, and private/local roots never become public parameters. Verify
    generic-field and case-payload composition.
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

  Mixed fixed-integer comparisons, Wrapping/Saturating casts and operation-level
  Trapping arithmetic exist (`pass/expressions/mixed_integer_comparison_{widths,places}`,
  `mixed_integer_guard_comparison`, `pass/core/numeric_cross_signed_conversion_surface`
  execute on all three hosts; `pass/core/numeric_trapping_conversion_overflow`
  traps). Guarded Trap ceilings work:
  `pass/core/runtime_guarded_trapping_conversion_exit` exits 70 and
  `fail/core/guarded_trap_ceiling_wider_than_primitive_rejected` rejects.
  Preserve operation site, operands, carrier, cause and normal-only result
  equation; coordinate **CRASH-CONTRACT** and never invent a Crash edge.

  Remaining: hosted acceptance for the public
  `source/library/core/numeric_conversion.omg` surface — same-sign narrowing,
  sign changes, widening and all four policies for every sign/width pair —
  by adding corpus fixtures (only two `core/numeric_*` fixtures exist), with
  exact-site and altered-cause/carrier/ceiling fail controls, dead results and
  overflowing shifts. Float policy/conversion customers belong to
  **OPERATOR-BOUNDARY-CALLS** / **FLOAT-PROVIDERS**.

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
[embedding](wiki/spec/build/embedding.md); current PCC-removal instructions take
precedence over older references to mandatory proof bytes or Terminal replay.

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
replay under the existing PCC tasks rather than downgrading it into a new
warning pass. Do not turn arbitrary errors into unimplemented or warn and run
code whose required semantic checks failed.

Claim individual assignments and paths. Each restoration lands one operation
or caller family, removes its superseded mechanism, and names an unchanged
program plus an invalid control. Reuse existing capability owners rather than
duplicate their representation or lowering work. Native/corpus success does
not establish ordinary CLI package admission; report both routes accurately.
Delete these items as their acceptance closes. The order is:
gap reporting -> core entry -> early jobs -> development seed -> tree deletion,
then independent claimed restoration slices -> final closure.

- **INTERPRETER-MIGRATION-GAPS.** (new-scope) Make intentional missing
  execution coverage visible before the tree cutover. The current corpus walks
  `tests/omega/{pass,fail,run}/`; simply moving canaries to `pending/` drops them
  from its results. Extend the existing runner/records/scoreboard with explicit
  migration-gap reporting and a narrow, reviewed designation tied to the owning
  capability task. Exercise designated pending canaries and retain their real
  authored exit/stdout oracles; count unimplemented separately from success,
  rejection, infrastructure failure, timeout and disagreement. Use an actual
  unsupported operation/entry diagnostic, not a blanket catch or a source-shape
  skip. A migrated slice exits the gap roster only after the unchanged oracle
  executes; no re-pinning wrong results as accepted behavior.
  Owners: `omega-rust/omega/compilation/driver/tests/corpus_runner.rs`,
  `tools/corpus_gate.py`, `tools/corpus_records.py`, `tools/corpus_scoreboard.py`.
  Acceptance: a deliberately unsupported canary reports its exact missing
  capability/task, a working canary still meets its oracle, a wrong result still
  fails, and an unrelated checker rejection or timeout cannot satisfy the gap.
  Pending artifact-only Build fixtures remain separately attributed to their
  existing harness dependency; do not relabel that as interpreter coverage.
  Current frontier: reviewed pending roots now enter the ordinary runner's
  filtering, sharding, staging and execution routes without walking all pending
  experiments. Exact selections reject missing, escaping or malformed paths;
  restored execution must meet the original exit/stdout oracle. Explicit gap
  records remain separate unresolved debt and never count as successful runs.
  A real unsupported-operation preflight producer/canary and a Terminal runtime
  driver carrying typed trap outcomes remain missing. The current checked-tree
  runner cannot supply those facts; arbitrary checker failures or host signals
  cannot substitute for them.

- **INTERPRETER-CORE-ENTRY.** (new-scope) Make the existing Terminal engine's
  program/instance lifecycle the common execution entry, without checked-tree
  dependencies, compiler-owned source lookup or a separate trusted execution
  engine. Reuse one core for direct interpretation and embedding; drivers own
  exact entries, supplied values, effects, resource provisions and observations.
  Its current `TerminalProgram::load` requires proof bytes and Terminal
  verification: coordinate the interpreter part of
  **OMEGA-READS-TERMINAL-UNVERIFIED** and its prerequisite PCC/checker removals,
  rather than synthesize empty proofs or replay the producer. Keep genuine
  foreign-package/build confinement at its existing trust boundary.
  Owners: `omega-rust/psi/interpreter/terminal-interpreter/src/program.rs`,
  `src/instance.rs`, `src/execution.rs`, `src/adapters.rs` within that crate.
  Acceptance: a produced Terminal program executes through the common lifecycle;
  missing/wrong-instance imports fail before effects, and pause/resume does not
  repeat an effect. Source constructors and byte loaders reach that same core.
  A current `--both` result alone cannot close this item while it uses trees.
  Current frontier: direct and embedded artifact entries already share program
  projection and execution construction. The missing source-program constructor
  and production execution callers depend on
  **CHECKER-DISCHARGES-OPERATION-OBLIGATIONS** -> **PCC-CERTIFICATES-DELETED** ->
  **OMEGA-READS-TERMINAL-UNVERIFIED**; do not add a constructor that bypasses
  those obligations or another wrapper around the existing engine.

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
  Psi stages. Acceptance: `pass/comptime/runtime_const_array_length_transitive_exit`
  computes 16 through Terminal and its unchanged application exits 70;
  `pass/build/const_index_dependency_order_exit` respects leaf-before-outer
  ordering. `fail/comptime/{negative_const_array_length,effectful_const_array_length}`
  still reject. Record a precise dependency if the demanded closure itself
  cannot yet check; do not establish a permanent typed-tree execution exception.
  Current frontier: the shared stage 05-07 production operation is now owned by
  `omega-rust/psi/compilation/terminal-production/`; the Omega product adapter
  delegates to it and retains product timing/report vocabulary. Early demand
  closure checking, prerequisite ordering and common-core execution remain
  unfinished. Wire them to this producer rather than copy its lowering body.

- **TERMINAL-BUILD-SEED.** (new-scope) Establish the minimum package-aware
  Build activation that lets agents keep building/checking/running the compiler
  after tree execution is removed. Depends on **EARLY-TERMINAL-JOBS**. Inventory
  the actual transitive Build/core/stdlib requirements of the seed before
  selecting its scope; a minimal source file does not imply a trivial Build.
  Run guest computation in the core and bind exact compiler-service operations
  for its required application, dependency, provider and entry declarations.
  Product description queries do not execute product bodies. Preserve
  **BUILD-EVALUATES-ONCE**, pre-effect admission and all-target authored rows;
  do not retain a feature/fixture-specific Build evaluator.
  Owners: `omega-rust/omega/compilation/build-evaluation/src/`, Psi compile-time
  admission, exact Build catalog contracts and Terminal lowering/adapters.
  Acceptance on Windows and macOS: `mbx check -p omega`; ordinary source check
  of the valid seed; rejection of
  `tests/omega/fail/arithmetic/array_constructor_divisor_requires_nonzero/main.omg`;
  package-aware `omega run tests/omega/pass/float/named_provider_directed_divide_exit/main.omg`
  after normal local package preparation produces 70. Freeze its actual closure,
  not the whole passing corpus. Unrelated unsupported features may become gaps.
  The seed's actual Build closure needs `Build::application`,
  `Build::depend(Source::Path)` and all four authored target `Build::bind` rows;
  the std dependency executes `Build::package`. Exact original Build storage,
  declaration coordinates and owned structured results must survive the common
  instance. These compiler catalog bindings and result transport remain missing;
  native execution through the existing tree route does not establish this seed.

- **CHECKED-INTERPRETER-DELETED.** (new-scope) Delete the tree execution
  engine immediately after **INTERPRETER-MIGRATION-GAPS** and
  **TERMINAL-BUILD-SEED** close; full feature parity is not a dependency.
  Remove `omega-rust/psi/interpreter/checked-interpreter/` and production
  dependency edges, expression walkers, call/frame/store machinery and worker
  entrypoints. Inventory every consumer first: compile-time and Build
  evaluation, CLI/corpus, package snapshots/metadata, observation codecs,
  provider selection and trust records. Move still-required data interfaces
  and genuine host helpers to their semantic/tooling owners; retire receipt
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
  Owners: `omega-rust/omega/application/cli/src/execution/compilation.rs`,
  `omega-rust/omega/compilation/driver/tests/corpus_runner.rs` and exact host
  adapters. Acceptance: `omega run --both` for
  `pass/float/{named_provider_directed_divide_exit,runtime_float_integer_conversion_boundaries_exit}`
  returns native/interpreter 70; restore **WINDOWS-CONSOLE-INPUT**'s Loader and
  five bounded-line controls through the common core. Missing/wrong-instance
  imports reject before effects; stdout/stderr and exit oracles are independent.
  A native compilation failure or comparison decline cannot count as agreement.

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
  existing `pending/build/artifact_only_*` witnesses need their separate corpus
  source-metadata harness dependency; do not waive grants to promote them.

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

- **REFERENCE-LOCAL-BINDING-ROUTES.** (new-scope) Reference-valued locals from
  every admitted producer keep their loan identity, backing and access through
  reads, calls and explicit state transfers, with exact backing and no implicit
  copies.
  - Spec: [ownership: borrows and aliases](wiki/spec/language/ownership.md#borrows-and-aliases),
    [loans](wiki/spec/terminal-psi/loans.md), record re-views under
    [the representation judgment](omega-rust/psi/compilation/checking/recasts.md).
  - Owners: `omega-rust/psi/compilation/pipeline/04_typed-trees-to-checked-trees/src/execution/terminal_unit/control/statement_sequence/local_data.rs`,
    `.../terminal_unit/receiver_aliases/`, stage 05
    `expression_preparation/bindings/` and
    `scalar_graph/scalar_computations/fields.rs`. Reuse the shared source-place
    and view-narrowing operations; do not add local-specific call or range routes.
  - State: exact field/fixed-array descriptor retirement preserves disjoint
    siblings and transfers child retirement to the receiving constructor leaf.
    On 2026-10-05 macOS arm64, the stored-leaf call and child-constructor
    regressions return 70 in both engines; public `omega run --both
    samples/cli/systems/descriptor_walk/main.omg` returns 50 in both engines
    after local package review. Ordinary owned scalar/record helper arguments
    use their published storage bindings. The encoded `IntegerAt` call ABI
    remains owned by **CHECKER-DISCHARGES-OPERATION-OBLIGATIONS**.
    Remaining: `reference_result_custody::aggregate_construction_sources`
    transports reference origins from record/array constructors but not from
    `match` results. Carry each alternative's exact source-to-result paths so
    retirement can distinguish possible transfers from transfers on every arm.
    Exercise both outcomes when all arms transfer the same child and when only
    one arm transfers it; retain rejection while the receiving leaf is live.
    Calls and stores without a retained receiving-holder association still
    require their checked correspondence before parent retirement can be proved.
  - Constraints: shared loans cannot gain mutation authority by widening the
    exclusive `receiver_aliases` match. Keep the local symbol as the loan
    subject and compose its captured projection at use; substituting the root
    loses borrow/write-frame/release identity. A known-place loan need not
    become a fresh affine `EstablishReference` result. Descriptor cleanup stays
    separate from backing permission; returned shared views cannot grant
    mutation or escape activation-owned storage. Named-state forwarding and Unit
    completion must not regain source-shape gates.
  - **STATE-LOCAL-VALUE-FRONTIER** owns integration with the composed caller.

  Acceptance: every producer form, reads through scalar/record loan names,
  borrowed calls and state forwarding execute natively and agree with the
  interpreter. Keep these controls at their oracles:
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
  fixtures reject. Reject conflicting loans, source/path substitution and
  invalid escapes. A constrained referent such as `u8 [0..=1]` borrowed as
  `&mut u8` keeps the original storage's obligations. The original sample
  `samples/cli/systems/descriptor_walk/main.omg` runs through
  `omega run --both` and returns 50 in both engines after local package review.

- **TRANSITION-TAIL-CALL-TO-MACHINE.** Terminal machine calls in attached Unit
  transition arms complete the caller for every result and operand form, per
  [state contracts](wiki/spec/language/state_contracts.md#terminal-calls-in-transition-arms):
  a terminal call is an ordinary invocation whose normal result completes the
  caller; an internal state target remains a jump.
  - Where: host trap-exit validation for
    `calls/runtime_guarded_reference_return_crash` remains unavailable on this host.
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
    - validate the native trap oracle for
      `calls/runtime_guarded_reference_return_crash` on a host that can reap trap
      exits. The current macOS host leaves both that program and system-clang
      `int main(void) { __builtin_trap(); }` in uninterruptible exit. Checking
      admits the fixture; the interpreter leg declines, so no cross-engine trap
      agreement has been established.
  - Not this item: `filesystem/dir_walk_wrappers_exit` and
    `samples/cli/systems/note_vault` depend on `Filesystem::create_dir_all`.

  Acceptance: establish the crash fixture's native trap oracle on a host that can
  reap trap exits. Keep `pass/calls/runtime_linear_guarded_owner_disposition_exit`
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
    remaining signatures. Value-dependent float representability and
    target-predicate casts need lowering that keeps the predicate; a rounding
    conversion cannot stand in for an exact cast. Exact float-to-integer casts
    depend on **FLOAT-PROVIDERS** (the conversion's `requires` reads neither
    declared float ranges nor dominating guards). Unverified:
    `float/float_to_int_exact_proofs_exit` executes at 78 on macOS ARM64 but
    its Linux/Windows rows record rejection; rerun there.
  - **GUI sample stores.** `samples/gui/{window_demo,windowed_calculator}`
    stopped at a borrowed byte-array element argument and
    `samples/gui/window_app` at a structural field store destination
    parameter; rerun them on Windows now that stage 05's storage-read check is
    deleted.
  - **Specialized providers.** Retain the exact closed requirement, provider
    instance, static tuple and application occurrence through selected-plan
    settlement into native realization
    (`omega-rust/omega/compilation/pipeline/00_terminal-psi-to-abstract-operations/src/boundary_application_settlement.rs`,
    `omega-rust/omega/compilation/backend/artifacts/native-artifact/src/physical/derivation/evidence.rs`).
    `ClosedReachApplication`, a template alias or equal runtime signatures
    cannot identify the specialization.
  - **Directed float arithmetic.** Install the remaining named directed
    arithmetic as Omega builtins through stage 00
    `provider_admission/compiler_builtins.rs` (one
    `NearestIeeeFloatArithmetic` operation per execution), stage 02
    `lowering/control_flow/ieee_float_arithmetic.rs`, and each ISA's
    `selected_form_encoding/ieee_float_arithmetic.rs`. Selected-form tag 230
    carries the operation/format ordinal; append identities, never renumber.
    Nearest FMA (ordinal 44) stays AArch64-only until
    **X86-FMA-PROVIDER-TRANSPORT**. Re-drive
    `tests/omega/pending/arithmetic/float_computation_no_route` with and
    without the float-operations import across arithmetic/comparison,
    field/local/guard positions and f32/f64. Preserve declared operator
    selection; no spelling-based fallback.
  - **Receiver-bearing requirements.** Complete a member-call witness through
    borrowed-field argument planning, provider signature refinement and native
    execution, keeping exact receiver/argument correspondence (no blanket
    `is_self` exemption). Reproduce the parameter-source/refinement stops
    before assigning repairs.
  - **Borrow rules.** Operators spelled as calls keep ordinary call borrow
    rules; an exclusive loan forbids a second alias reading the old contents.
    With **BORROW-PROOF-CONVERGENCE**, give plain-read, call-argument and
    float-comparison forms of one live-loan conflict the same rejection.
  - Checked interpretation (`omega-rust/psi/interpreter/checked-interpreter/src/interpreter/evaluator/execution/operator_applications.rs`)
    declines a requirement with several checked satisfiers because checked Psi
    does not retain the Build's provider choice; this closes with
    **PROVIDER-SELECTION-AFTER-TERMINAL**.

  Acceptance: `float/named_provider_directed_divide_exit` and the
  `pending/arithmetic/float_computation_no_route` matrix execute natively.
  Recover and execute the deleted customers from `089bd3290c`:
  `specialized_{boundary,fixed}_operator_physical_custody` and
  `specialized_{structural_fixed,mixed_structural_fixed}_operator_hosted_native`,
  covering structural-input/scalar-result, mixed-input/scalar-result and
  mixed-input/structural-result hosted cases with the owned affine
  operand/result disposition; an unused helper beside an empty entry is not
  coverage. Keep at their oracles: `providers/{checked_fixed_operator_dispatch,token_machine_family_override,checked_boundary_operator_const_application_alternate}_exit`,
  `providers/{checked_boundary,checked_fixed}_operator_physical_custody`,
  `providers/specialized_mixed_structural_result_operator_hosted_native`,
  `comptime/provider_boundary_range_endpoint_exit`,
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
  - The gap: `select_product_target` swaps bodies before checking
    (`omega-rust/omega/compilation/driver/src/checked/checking/execution_settlement.rs`),
    and `prune_target_siblings` (`.../04_typed-trees-to-checked-trees/src/product_pruning/`,
    called from the same file and from
    `omega-rust/omega/compilation/build-evaluation/src/admission/checking.rs`)
    drops siblings before Terminal.
  - Carry every body's checked semantics, supply mode and conformances through
    lowering and codec; `omega inspect-terminal` exposes each target tag.
    Entry-reached bodies alone do not satisfy retention.
  - **PROVIDER-SELECTION-AFTER-TERMINAL** owns Omega's selection from the same
    artifact; independent ownership moves can land first.

  Acceptance: keep `fail/targets/sibling_body_type_error_rejected` and
  `pass/targets/target_machine_gating_exit` (70). Add a fixture whose target
  bodies return distinct results, realized from one Terminal artifact, with
  same-target sibling calls. Invalid unrealized bodies and duplicate family
  members reject; a missing called body fails only the affected realization.

- **BUILD-EVALUATES-ONCE.** (new-scope) `build.omg` evaluates once per
  compilation, observes no realized target, and produces checked rows for every
  target: providers, entry, subsystem, application intent, opaque
  representation, x86 scalar FMA provider, grants, behavior exclusions and wire
  demands.
  - `BuildConfig` is still largely global. Finish authored rows,
    retain/check unrequested rows, and complete shared preliminary and final
    product checking (final checking is per target today).
  - Remove the duplicate preliminary whole-program check by moving its
    obligations into the owning stages; the gate is
    `omega-rust/omega/compilation/build-evaluation/src/admission/checking.rs::prepare_checked_build`.
    Preserve [admission before execution](wiki/spec/build/execution.md#admission-before-execution):
    freeze authored selections and admit the exact Build entry before effects,
    then incorporate generated source and finish product checks.
  - `PreparedBuilds` reuse already keys on the exact `build_snapshot`. The
    output-free Pure Build carrier does not authorize sharing effects or
    output obligations; keep separate Build/Product purposes and current-pass
    logical charges.

  Acceptance: a two-target request counts one Build activation and one final
  product check (timing rows alone are insufficient); std's six rows come from
  one evaluation. Malformed unrequested rows reject; per-target missing-row
  outcomes stay per target. Nested generated-dependency and dual-purpose
  custody, pre-effect rejection and snapshot/required-output settlement keep
  their behavior; their CLI Rust tests were cut, so add corpus or CLI-level
  coverage for them.

- **PROVIDER-SELECTION-AFTER-TERMINAL.** (new-scope) One target-neutral Psi
  product; per-target provider installation consumes Terminal Psi plus that
  target's Build rows in Omega stage 00. Follow
  [multi-target compilation](wiki/spec/build/configuration.md#multi-target-compilation)
  and the [pipeline](omega-rust/pipeline.md#one-driver-one-pass-per-target-realization).
  - Stage 00's `boundary_application_settlement.rs` and
    `comparison_settlement.rs` still borrow checked trees, source identities
    and frontend-derived selected evidence. Feed them from the Terminal
    artifact's occurrence scope, without another parallel registry.
  - `omega-rust/omega/compilation/driver/src/checked/checking/execution_settlement.rs`
    still joins frontend-dependent `provider-planning` and `selected-dispatch`
    decisions: target-family body substitution, calling plans, program-entry
    selection, fused-provider settlement, selected legacy generic-operator and
    boundary-family requests, selected-dispatch execution, callback
    materialization, task activation, component progress, x86 FMA associations
    and fused entry establishments. Move their per-target realization to Omega,
    keeping source-semantic contracts and all authored candidates in Psi. Every
    family body and Build row is checked even when `--target` excludes it.
  - Remove `provider-planning`'s `typed-trees`, `validation` and
    `typed-trees-to-checked-trees` dependencies
    (`omega-rust/omega/compilation/realization/provider-planning/Cargo.toml`)
    and enforce the layering boundary.
  - Const exception: provider-dependent constants belong to the one checked
    program. Every realized target must select the same provider for that
    application; evaluate it once in Psi through `SelectedConstantEvaluation`
    (`omega-rust/omega/compilation/realization/selected-dispatch/src/constant_evaluation.rs`),
    rejecting disagreement with the diverging targets named. Do not move a
    layout-determining fold into per-target realization.
  - `BoundaryMachineDeclaration` has no field for general scalar `ensures`;
    add it and validate provider/call agreement without narrowing the public
    contract to the selected body.

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
  - Today `compile` in `omega-rust/omega/compilation/driver/src/compiler.rs`
    clones prepared source per target and runs checking and production per
    target. Retire the per-target clones and the reuse structures that mask
    repeated compilation (`PreparedBuilds`, `compile_checked_reusing_builds`,
    package-manager's `CandidateSourcePreparation`) as their work moves into
    the shared stages.
  - Package review calls the same 00–03 prefix directly; remove alternate
    compile modes (`compile_to_checked` beside `compile`). Source preparation
    supplies stage 00; build-time evaluation, dynamic binding, const evaluation
    and semantic pruning belong inside stages 02–04. Carry discovery, grants,
    sponsors and snapshots in the owning stage inputs/outputs.
  - Keep each stage's independent representation and transformation crate,
    single-target command behavior, Build-effect custody and per-target
    publication.

  Acceptance: the compiler owns only composition, reports and publication; a
  two-target native compile shows one Psi chain and one Terminal identity, then
  one Omega chain per target, with native results and isolated realization
  failures as in **PROVIDER-SELECTION-AFTER-TERMINAL**. Prefix consumers stop at
  their checkpoint; stage counters distinguish execution from reused reports.

- **DRIVER-EARNS-ITS-LINES.** (new-scope) The product driver reads as
  "`build.omg` in, program out": prepare sources, run the stages, publish.
  `omega-rust/omega/compilation/driver/src` plus
  `driver/terminal-production/src` are mostly plumbing around a stage chain of
  a few dozen lines. This item's own work, which can land now:
  - ~~One timing system~~ — done at `f413234e5a4`: `TerminalProductionTimings`
    was a byte-for-byte duplicate of `CompileTimings` (same enabled flag, same
    aggregate-onto-existing-label rule, same `record_result`), and production
    now records its legs into the caller's single ladder through
    `TerminalProductionStage::stage_meta()`. `merge_terminal_production_timings`
    is deleted. The aggregate row is opened at zero before production and the
    elapsed added afterwards with `add_completed`, because `record_result`
    would hold the accumulator borrowed across the call that now writes to it,
    and because the ladder orders phases by first insertion — do not "simplify"
    that back into a wrapper.
  - ~~Plain results~~ — done at `53a409f5e9b`: the always-unit generic `C`, the
    `callback_custody` field and `CallbackCustodyTerminalArtifactProductionError<C>`
    are deleted.
  - No self re-validation: the checked `ProgramEntry` receipt is produced,
    carried beside the artifact and rejoined in `terminal_artifact.rs`. The
    producing stage returns the receipt it validated once.

  The rest leaves with other items: checked-tree sidecars past Terminal
  (source-call, IEEE and integer comparison occurrences,
  `CheckedBoundaryOperatorApplicationScope`) with
  **PROVIDER-SELECTION-AFTER-TERMINAL**; reuse caches in `driver/src/checked`
  with **ONE-DRIVER-PER-STAGE**; product re-admissions with
  **COMPILE-REPORT-PRODUCTS**.

  Acceptance: driver plus terminal-production production code is about 200
  lines or fewer, every remaining line a stage call, report construction or
  publication; a caller compiles a package root without assembling per-target
  configurations itself (the CLI builds `TargetCompileConfiguration`s in
  `omega-rust/omega/application/cli/src/compilation/mod.rs` today).

- **GUI-CANARY-HAS-NO-PROVIDER.** (new-scope)
  `tests/omega/pass/host/runtime_gui_window_lifecycle_exit` and
  `runtime_gui_foreground_window_exit` each declare their own
  `pub boundary trait Gui` over four user32 operations (`window_create`,
  `msg_peek`, `is_window`, `window_destroy`) and declare **no satisfier at
  all** -- `grep "satisfies Gui::"` finds zero rows repository-wide. Both
  reject with "selected ProgramEntry Binding field `Main::gui` requires a
  selected Fused provider for boundary `Gui`". They are the only two members of
  that diagnostic's cluster that are not `FilesystemHost` (19 of the 21 are).

  **Why they appear to work anyway, and why that is the real finding.** The
  checked interpreter dispatches these on the **bare operation name**:
  `interpreter/evaluator.rs:300` documents "the virtual window system:
  `window_create` mints opaque non-zero handle" and
  `evaluator/execution/expressions_and_value_calls.rs:1087,1136` matches the
  string `"window_create"`. So the interpreted leg satisfies the trait without
  any provider while terminal production demands a Fused one. That is the same
  bare-name-fallback class as the dead `BuildOutput` facet family repaired by
  `d37ea22ec59`, and it is why nothing caught the missing provider: a fixture
  can read as working interpreted while the route that would ship it has no
  provider.

  **The template is `pass/capabilities/windows_provides_import_exit`**, which is
  green: it declares `pub boundary trait Beeper`, a
  `windows_x86_64 machine beeper_binding() -> ForeignBinding<10, 3, 0>`
  returning `ForeignBinding::DllImport { import: DllImport::PeByName { library,
  export } }`, and `machine beeper_abs(...) satisfies Beeper::beep via
  beeper_binding();`. Every green fixture that declares its own boundary trait
  declares its satisfier in the same fixture; the two Gui fixtures are the
  exception.

  **Measured, so the next lane does not repeat it:** restricting
  `Build::bind` to `windows_x86_64` alone does **not** fix them -- the Fused
  demand applies to the declared target, not only the realization host, so the
  template is green because it has a provider rather than because Windows is
  not the host. A provider is required. That said the four-target bind is also
  wrong on its own terms: a user32 trait cannot be deployable on Linux or
  macOS, and the template binds Windows only.

  Acceptance: both fixtures declare a `windows_x86_64` user32 provider in the
  fixture, bind only `windows_x86_64`, and reach `checked`; their authored exit
  70 is verifiable only on a Windows host, so a macOS lane states that leg as
  unverified rather than claiming it.

- **COMPILE-REPORT-PRODUCTS.** (new-scope) Product admission is part of
  report construction in
  `omega-rust/omega/compilation/products/src/compilation_products.rs`, and
  every compiler and package route reaches it through one report-settlement
  call.
  - Frontier: `CompileReportSettlement` owns construction for all five stops
    (`finish_terminal`, `finish_native`, and the Check and artifact-only arms
    inside `prepare_product_report`), carrying root path, source count,
    production subject, `pcc_requests` and native application metadata
    captured before lowering (`33d5557dc4f`). `terminal_report`,
    `native_report`, `check_report`, `CompileReport::verify_native_source_metadata`
    and the CLI's hand-derived Check rollback predicate are deleted. The
    coverage half is now closed. Artifact-only admission is authorable,
    verified through the CLI, and exercised by the corpus. `d37ea22ec59`
    repaired the
    prerequisite nobody knew was broken: `BuildOutput::require`/`complete`/`fail`
    and `RequiredOutput::path` selected by RECEIVER SHAPE and became silently
    unreachable when the receiver-to-explicit-argument migration retired that
    spelling, so `Build::artifact_only` could never complete a required output
    and nothing diagnosed the loss. Proof rather than inference: two `require`
    calls reserving the same name, which the evaluator rejects as a collision,
    compiled clean before the repair and reject after it. The four entrypoints
    now read their facet from the first argument, the shape `try_build_path_call`
    and the three `BuildProduct` queries already used -- which is why
    `resolve`/`create`/`write`/`close`/`include_source` survived the migration
    and the obligation family did not. The receiver route and `output_facet_root`
    are deleted.

    The three witnesses now run in `pass`/`fail` under sealed fixture source
    staging: `pass/build/artifact_only_required_output_completion` records
    `checked`, `fail/build/artifact_only_rollback_overlay_refused` pins
    "artifact-only builds execute no product optimization stages to roll back",
    and `fail/build/artifact_only_unsealed_output_refused` pins the sticky
    required-output failure -- measured over 1962 fixtures with 0 other rows
    moved and 0 fail fixtures failing to reject. What remains is the
    acceptance's second half: **no automated check asserts that the refused
    overlay publishes no partial product set.** The corpus runner records
    diagnostics, not build-directory contents, so "the refused invocation
    leaves only the build-directory owner stamp" is still only the manual CLI
    observation written in that fixture's README; closing it needs a
    build-directory assertion the runner does not have today.
    Note also that an `OptimizationRollback`
    cannot be authored in Omega at all: the only two non-test construction sites
    are the CLI's `--disable-optimization <ExactName>` and the corpus runner's
    `rollback.txt` sidecar, which 15 fixtures already use.

    Correction for whoever picks this up: there is **no**
    `RequestedCompileProduct::ArtifactOnly`. That enum has exactly `Check`,
    `TerminalArtifact` and `NativeArtifact`; `artifact_only` is a `bool` on
    `EvaluatedBuild` selecting `CompileOutputKind::BuildArtifacts` inside
    `admit_build_requirements`, and the spec adds no fourth project role.
  - `admit_terminal_production` and `terminal_native_proposal_request` stay
    where they are: the first is the single pre-production head every
    Terminal-producing route enters, including the package route whose
    settlement is `Native`, so folding it into admission would skip it there;
    the second is a borrow projection into stage 00's request with no
    admission in it.
  - The [rollback](wiki/spec/build/optimizations.md#release-rollback-and-promotion)
    refusals now live at two placements, down from three: request admission
    before source acquisition, and `prepare_product_report` as the
    product-owned boundary the directly constructed package routes cross since
    they build no `CompileRequest`. Honest reachability: of the three package
    routes two request `NativeArtifact` (whose phase filter rejects nothing)
    and `check_project` passes an empty overlay, so the `CompileRequest`/CLI
    route is the only one that can carry a nonempty overlay today.
  - The item's premise that the corpus runner has no Terminal-request mode is
    **false**: `produce_terminal_semantic`
    (`omega-rust/omega/compilation/driver/tests/corpus_runner.rs:325`) compiles
    the 15 `tests/omega/pass/optimizer/rollback_*` fixtures twice with
    `RequestedCompileProduct::TerminalArtifact` and compares rolled against
    unrolled semantic modules, and `driver/tests/terminal_decode_once.rs`
    drives the Terminal product through full native realization. Terminal
    admission is covered end to end; only artifact-only is not.
  - Note for whoever takes this: `compilation-products` has **zero** Rust
    tests, so the corpus and real-CLI probes are the entire evidence for that
    crate.

  Acceptance: an [artifact-only](wiki/spec/build/scoped_execution.md) corpus
  fixture compiles and refuses a nonempty release-rollback overlay without
  publishing partial outputs.

- **NATIVE-GOLDEN-CANNOT-RECORD-A-REGRESSION.**
  (split-of:CORPUS-GOLDEN-HAS-NO-HOST-DIMENSION)
  The check golden's host dimension is settled. `check_leg_realization_target`
  reads the fixture's own `build.omg` -- the first of `macos_arm64`,
  `linux_x86_64`, `windows_x86_64` it binds, else its first declared target --
  and never the recording host, and every realized row names the result as a
  `check-target:<target>` fact. The four self-identifying rows the diagnostic
  detector found are recorded at their deterministic target, as is
  `native_close`, which that detector could not see: its false green carried no
  diagnostic at all, so a detector reading diagnostics finds host-dependent
  refusals and never a host-dependent acceptance. What remains is the other half
  of the same investigation.

  **Eight macOS rows claim native `built` for a fixture whose check leg
  refuses**, so a native verdict outlives a check-leg regression, and six of
  them still name an authored exit (four `exit:70`):
  `pass/capabilities/acquires_through_helper_return`,
  `pass/memory/secondary_processor_canary`,
  `pass/providers/specialized_mixed_structural_result_operator_hosted_native`,
  `pass/text/local_bounded_text_{case_result,scalar}_loan_exit`,
  `pass/text/runtime_number_to_decimal_exit`,
  `pass/traits/boundary_trait_effects_host_call` and
  `pass/wire/wire_preserving_decode_relay_exit`.

  **They cannot be corrected today.** `corpus_gate.py` exits `regressed` and
  refuses `--record` when an authored exit or stdout oracle fails
  (`tools/corpus_gate.py:109`), which is right -- recording would pin the
  regression as the new truth -- but it leaves no way to say "this used to build
  and now does not", so the row stays false until the fixture is fixed. These
  eight do not change the red count; the check golden has them right. What they
  change is that a native row cannot be read as evidence of health without its
  check row. Acceptance: the native leg can record a regression distinctly from
  pinning it as expected, and the eight stop claiming an exit they do not meet.

  Separately, the **Linux-side bound is now measurable rather than unknowable.**
  It could not be taken from a macOS host because a row green here and red there
  left no local trace; with the realized target deterministic, one Linux
  recorder running the corpus and diffing answers it, and any row that still
  differs is a genuine Linux/macOS compiler difference rather than a recording
  artifact.

- **BUILD-FACET-DEAD-SELECTION-RESIDUE.** (split-of:BUILD-FACET-RECEIVER-RESIDUE)
  No build operation is classified by receiver shape any more: stage 04's
  `BuildOperationForm`, both of its receiver predicates, and the `State`
  payload on `StaticArgumentCall::Statement` that existed only to answer the
  receiver question are deleted. All four classified operations declare their
  facet as an explicit first parameter in the build prelude
  (`BuildLog::write_line`, `BuildOutput::include_source`, and the generated
  `Optimizations::enable`/`emit_report`), so every authored use resolves that
  declaration and the resolved-target arm answers it first; Omega harvests the
  optimization selections from the evaluated `Build.optimizations` counter
  fields, never from an intrinsic fact.

  What remains is one unconstructed variant,
  `ExecutedProviderSelectionSite::Statement`, and it is unconstructed for an
  upstream reason rather than as a property of the published Psi record.
  Stage 02 retains every non-discarding provider-selection statement as an
  expression (`lowering/statement/statement_nodes.rs`), and the only statement
  spelling left, `_ = Build::select_provider<..>(builder)`, is refused at
  checking with "qualified call `select_provider` has no resolved callable
  declaration" because `select_provider` is the one operation with no prelude
  declaration. The expression retention cannot absorb that spelling, because
  `TableCallExpression` carries no `discards_result` field. The sibling record
  beside it, `ExecutedBehaviorExclusionSite`, constructs both of its variants
  -- `Build::exclude_crash(builder, cause);` takes the statement arm -- so the
  two-variant site is a live shared shape, not provider-only ceremony.
  `try_provider_selection_statement` stays called unguarded, so any spelling
  that does reach it works. Acceptance: either `TableCallExpression` carries
  the discard marker so stage 02 retains that spelling too and the statement
  arm becomes structurally impossible, or checking admits
  `_ = Build::select_provider<..>` and the arm becomes constructed. Deleting
  the arm by itself would bake the upstream gap into the Psi/Omega record
  contract.

- **CORPUS-BUILD-LOG-STREAM-COLLISION.** (new-scope)
  No corpus fixture can write a build log line. The compiler emits the retained
  build log on its standard output and the corpus runner multiplexes its
  fixture records on that same stream, so one authored line aborts the whole
  measurement with `corpus_gate: runner emitted unreadable records: line 1: not
  a fixture record: <the authored line>`. It is a harness property, not a
  property of any one source: every fixture writing any build log line breaks
  identically. That is why the authored `BuildLog::write_line` customer is
  `tests/omega/pending/build/build_log_line_written` rather than a `pass` row,
  verified instead through the real CLI. Acceptance: a fixture that writes a
  build log line records an ordinary outcome, and that pending case moves into
  `pass/` with its golden row.

  Sequencing: this is the same `corpus_runner.rs` and shared-golden surface as
  **CORPUS-SEALED-SOURCE-STAGING**, so the two coordinate rather than race. A
  row it adds carries a `check-target:<target>` fact when the fixture binds a
  program entry.

## Lowering simplification

Owner ruling 2026-10-04: stage 05 is cut down to a mechanical lowering of
stage 04's plans. Three things go:
- **Re-checking.** Stage 05 re-reads the typed program and re-validates the
  plan it is handed. That is the in-process self-validation the
  [Trust ceremony](#trust-ceremony) ruling removes.
- **Parallel routes.** Bodies are lowered by 13 leaf routes, chosen in
  `omega-rust/psi/compilation/pipeline/05_checked-trees-to-lowered-psi/src/machine_lowering/machine_dispatch.rs`,
  and each route re-implements calls, stores and transitions.
- **Shape-specific plans and rungs** where a general operation would work.

Stage 05 is 178k lines with 5,684 refusal sites; only 3 of its 5,250
distinct refusal messages appear in any recorded corpus rejection. An audit
sampling 42 refusals in `unit/attached_unit` found 34 checking stage 04's
invariants, 7 route-specific shape gaps and 1 unsupported feature.

Order: the deletions (first three items) shrink the surface, then the shared
operations, then the route retirements. The vocabulary items can run at any
time after the deletions. New capability work (**STATE-LOCAL-VALUE-FRONTIER**
and others) lands in the shared operations, never as a new per-route case.
Each item lands in one commit, deletes the code it supersedes, and keeps the
check corpus and the native corpus unchanged on the slices it names.

- **STAGE05-SOURCE-REREAD-DELETED.** (new-scope) Stage 05 re-reads the
  authored typed program to obtain values its stage 04 plan rows do not
  carry. The validators that only re-confirmed a row against its source are
  deleted. What remains:
  - `omega-rust/psi/compilation/pipeline/05_checked-trees-to-lowered-psi/src/expression_preparation/source_custody/`
    (3.9k lines): structural operand and place lookups, parameter storage,
    primitive references, computation calls and constructed values (the
    control readers moved: dispatch subject kinds and dispatch call roles
    are plan rows, guarded exits come from the plan's arm ordinals);
  - `.../src/emission/call_source_custody/` (2.7k lines): the nested call
    occurrence walk, named terminal arguments and projected receivers.

  Calls already show the pattern: `FlowCallFact` carries each call's
  authored site, callee, arguments, receiver, result use and closing-arm
  status, recorded where stage 04 joins the call to its source, and
  `src/call_sites.rs` reads those rows without touching source tables.

  Move each value into the stage 04 plan and delete its reader. Some readers
  also refuse shapes stage 05 cannot lower (the nested call walk refuses
  runtime receivers and boundary results that are not direct plain-owned
  affine operands); those refusals stay until the route items admit the
  shapes. Then add a source-scanning guard in stage 05 that lists the few
  files allowed to read `statement_table`, `expression_table`,
  `StatementNode::` or `ExpressionNode::`; today there are 766 such
  references across 73 files. Acceptance: both modules are gone, the guard
  passes, and the full check corpus and the native corpus are unchanged.

- **STAGE05-PLAN-REVALIDATION-DELETED.** (new-scope) Depends on
  **STAGE05-SOURCE-REREAD-DELETED**. Delete stage 05's validation of the plan
  it receives:
  - `.../05_checked-trees-to-lowered-psi/src/unit/attached_unit/admission/`
    and `admission.rs`;
  - `unit/dynamic_composed_unit/plan_validation.rs`;
  - the terminator, edge and case checks against source in
    `unit/attached_unit/composed_control/state_graph/admission.rs` (the
    state body rejoin is gone; the terminator ordinal is a plan field);
  - every refusal whose message asserts a stage 04 invariant ("disagrees
    with its checked …", "omits or duplicates", "declared more than once",
    "not canonical source order", "substituted", "rejoin"). The standalone
    `if <pure condition> { return unsupported(..) }` forms are gone; 50 remain
    in 29 files as lookup refusals (`ok_or(..)?`), `let … else` patterns or
    conditions with effects, each needing its value traced before removal.
    Of the "drifted", "changed its", "differs from its" and "exchanged its"
    refusals, 39 remain in 21 files: 33 produce a value emission uses
    (lookups, `let … else` bindings, route-selecting fallbacks, for example
    the partial-affine cleanup lookups in `unit_cleanup/partial.rs`), 3
    refuse a shape stage 05 has no route for, 1 has a real effect
    (`place_observations.rs`), and 2 compare backing homes stage 05 allocates
    itself (`loan_chains.rs`, `successor_edge.rs`).

  A plan that violates an invariant is a stage 04 bug, found by the corpus.
  Refusals for genuinely unsupported constructs stay until the route items
  below remove them. Acceptance: the listed files are gone; a source-scanning
  guard rejects new refusal messages matching those phrasings; the full check
  corpus and the native corpus are unchanged.

- **RECAST-OFFSET-EQUALITY-TRANSPORT.** (new-scope) An interior recast's offset
  bound is derived from three candidates and **none of them applies a proved
  scalar equation**, so a bound transported through equality is lost.
  `checking/src/value_custody/recasts/raw_byte_region.rs:~192` takes the `min`
  of a declared-range high, `composite_offset_upper_bound`'s structural walk,
  and `occurrence_integer_expression_bounds`. A boundary `ensures size == cap
  && cap <= 8` therefore proves nothing about `size`.

  Customer: `tests/omega/pass/dependent/boundary_equality_recast_witness_compile`,
  whose header states exactly this -- "a boundary contract may transport a
  symbolic bound through equality between out-parameters. `size == cap && cap
  <= 8` proves the four-byte view beginning at `size` fits in the twelve-byte
  region". The needed chain is `size == cap`, `cap <= 8`, so `size <= 8`, and
  `8 + 4 <= 12`.

  **Not design-blocked.** `wiki/spec/terminal-psi/verification.md:255-264`
  specifies `ValueEqualityTransport`: it "carries a nonempty ordered list of
  proved scalar equations, each oriented from an exact scalar value identity to
  its term", and "the bounded equality-transport owner applies only those
  carried equations to that proposition and the desired conclusion". The rule
  exists; the recast offset derivation simply has no equality source among its
  three candidates. The fixture's own "R1/R4" citation has no counterpart in
  `wiki/spec/`, so do not look for one.

  **A second, separable defect in the same path, worth fixing first because it
  is small and makes the real one legible.** When no route bounds the offset,
  `declared_high` can still resolve to the carrier's maximum, and
  `InteriorByteRegion::Bounded { offset: 4294967295 }` reports that as a
  *proven* bound: "the recast target `u32` needs 4 bytes at offset 4294967295,
  but the region holds 12". A bound equal to the carrier's full width conveys
  no proof, so the honest classification is `OffsetUnproven`, whose message
  (`push_offset_unproven`) already says the offset is not bounded. Admission
  does not change -- both paths refuse -- only the diagnostic stops asserting a
  fact it does not have. That u32::MAX sentinel is also against the house ZII
  rule, where absence is the zero value rather than a maximum.

  Acceptance: the fixture compiles, with the offset bound derived through the
  specified equality transport; and a genuinely unbounded offset reports
  `OffsetUnproven` rather than a sentinel offset. Note `checking/src` is inside
  "Codex / static heat grid"'s whole-directory lease, so claim with
  `--allow-overlap` and name the files.

- **UNIT-PLAN-GAPS-REPORTED-AT-SOURCE.** (new-scope) Today stage 04 drops an
  unsupported body silently:
  - its builders return `Option` (`return None` appears 1,459 times under
    `omega-rust/psi/compilation/pipeline/04_typed-trees-to-checked-trees/src/execution/terminal_unit/`);
  - `OmissionLedger` (`.../terminal_unit/mod.rs`) records the drop;
  - fallback builders retry with a different plan;
  - the compile finally fails at entry establishment with "the machine's
    unit plan was omitted at <phase>".

  Make the builders return `Result` carrying the unsupported construct and its
  source span, report it from `omega --check`, and delete `OmissionLedger`,
  `CheckedUnitPlanOmissionStage` and the retry paths. Acceptance:
  - `pass/text/runtime_local_struct_string_field_concat_exit`,
    `pass/traits/runtime_dyn_two_impl_dispatch_exit` and
    `pass/ownership/linear_transparent_record_frontier` fail `--check` with a
    diagnostic at the construct's span instead of an entry-establishment
    error;
  - no corpus outcome changes otherwise.

- **TRANSPARENT-RECORD-RESULT-CARRIES-MEMBER-CLAIMS.** (split-of:PROJECTED-OWNED-LET-CONTINUATION)
  `pass/ownership/linear_transparent_record_frontier` now lowers both
  `let extracted_left: Receipt = forwarded.left;` statements and rejects in
  Terminal verification instead: `terminal-artifact production failed:
  Lowering(InvalidTerminalModule(RecordResultMismatch(OperationId(5))))`.
  OperationId(5) is the `Pair { left: left, right: right }` `EstablishRecord`,
  and the refusing condition is `complete_root_custody` in
  `omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/record.rs:224-231`
  (measured with a per-condition probe, not inferred): that match requires an
  `Affine` or `Unrestricted` record result to be **claim-free**, while a
  transparent affine record over two linear members publishes one claim per
  member at that member's path. Only the `Linear` arm admits claims, and it
  admits exactly one whole-root claim.
  This is the same mismatch the contract paragraph below already names, so the
  amendment and this check move together: `fields` must admit an affine or
  unrestricted record result whose claim roster is exactly the type's linear
  claim frontier, each at its own path, and keep refusing a whole-root claim on
  a non-linear result. Acceptance: that fixture reaches `checked`, with a
  full-corpus before/after showing 0 regressions, and
  `fail/ownership/linear_transparent_record_member_scope_loss` keeps rejecting
  with ``linear value `pair.right` reaches scope exit without being consumed or
  transferred``.

  What landed at __LAND__, so it is not re-derived. An owned projection of an
  already established place needs no structural continuation: the authored
  binding establishes no storage, so stage 05 binds it as a projected alias
  (`StructuralBinding::ProjectedAlias`) over the root's place, emits no
  operation and no edge, and each later use projects the composed path at its
  real consuming operation -- the lane
  `validation/frontier/operations.rs:218-303` admits by recording the hole in
  `partial_custody_paths` while the root stays live.
  **Do not rebuild the continuation.** The Jump lane categorically refuses a
  projected owned `Linear` leaf
  (`terminal-psi/verification/src/validation/frontier/block_parameters.rs:46-53`,
  `validation/affine_cleanup/continuation.rs:58`, `validation/block_views.rs:567`),
  and `complete_value`'s residual schedule reaches
  `unit/unit_cleanup/partial/residuals.rs`, which never consults multiplicity
  and would emit a `StructuralAffineDiscard` for the live linear sibling
  `forwarded.right`. `wiki/spec/terminal-psi/ownership.md:41` confines the
  residual-complement derivation to a dying affine root, which this is not.

  Settled by measurement, against the earlier reading of this family: stage 04
  already admits the statement, so there is **nothing to fix upstream**. The
  recorded row carries only a stage-05 `Lowering` error, which means the
  checker's linear validation discharged both path-indexed obligations. The
  ordinary branch in
  `04_typed-trees-to-checked-trees/src/checks/multiplicity/linear_validation/permission_production.rs:138-185`
  records the `Transfer` with the leaf's own `Linear` multiplicity and claim
  identity, because `move_selects_claim` tests
  `claim.path.starts_with(event_path)`.
  `checks/multiplicity/projected_affine.rs::append_transfer` is the disjoint
  fallback taken only when `matching.is_empty()` -- an affine root with no
  linear members at all -- so its three row-shape guards are not on this
  customer's path, and generalizing them would record the transfer twice.
  Leave that module alone until a customer needs it.

  Latent, same family, no customer yet:
  `structural_values/emission/completed_storage.rs:52` keys a block-parameter
  arrival's claim requirement on the ROOT's multiplicity, so an affine arrival
  with linear children is not required to carry them across a join. There are
  four `ValueContinuation` builders, not three (`materialize_place` at
  `emission.rs:1498`, `direct_borrow` at `:1586`, `projected_case_place` at
  `:2386`, `dispatch` at `dispatch.rs:124`); adding a fifth is a sign of
  rebuilding the refuted mechanism.

  Contract text still owed, to be written with the `record.rs` repair above:
  `wiki/spec/terminal-psi/calls_and_outcomes.md:105-106` says record results are
  "unqualified, claim-free, and affine or unrestricted. Erased fields and
  linear custody remain outside this operation." That reads as "not modelled
  by it" rather than "reject" -- the same sentence says it of erased fields,
  which `record.rs` skips rather than refuses -- and it is already contradicted
  by `operation_contracts/records.rs:131-133`, which admits a `Linear` result
  with one whole-root claim.

- **UNIT-PLAN-OMISSION-MKALL-WALK-PARAMETER-CUSTODY.** (new-scope)
  `tests/omega/pass/filesystem/dir_walk_wrappers_exit` compiles no unit plan
  for `Filesystem::mkall_walk`, so `Filesystem::create_dir_all` is unavailable
  and the entry rejoins 0 Terminal context identities. The omission is at state
  0, phase `state graph: state signature: parameter custody shape: qualified
  non-linear parameter`, raised at
  `omega-rust/psi/compilation/pipeline/04_typed-trees-to-checked-trees/src/execution/terminal_unit/state_graph/mod.rs:809`.
  This is the second of two stacked gaps on that fixture; the first, a returned
  case payload from an owned structural state parameter carrying no consumption
  plan, landed at `1f6a0c3c325` and is pinned by
  `tests/omega/pass/errors/returned_case_payload_from_parameter_exit` (checked,
  native exit 70). **Not** blocked on FilesystemHost — that provider cluster is
  a separate owner question and this chain does not reach it. Acceptance:
  `python3 tools/corpus_gate.py --native --filter filesystem/dir_walk_wrappers_exit`
  reaches exit 70.

- **UNIT-PLAN-OMISSION-PROJECTED-INITIALIZER-ROOT.** (new-scope) A structural
  local initialized by a field projection gets no structural-value root, so the
  planner has no producer for the statement. `abbd563760a` generalized the
  `construction_destination` arm in
  `04_typed-trees-to-checked-trees/src/values/scalar/computations.rs` from a
  whole owned place to an owned place (`Name | Member | Indexed`), narrowing
  admission by the destination's owning lane
  (`primitive_type_reference(expected).is_none()`) rather than by the shape of
  the place expression, and deleted the superseded `is_view_element_copy`
  eligibility disjunct. Two disjuncts were deliberately kept:
  `is_scalar_case_place_value` and `needs_copied_place_root` are not subsumed,
  because `canonical_place_from_expression` also yields canonical places for
  `Borrow` and recast `Cast` nodes. What remains open is the same question for
  an **array-literal element that is a call**
  (`Envelope { elements: [first, make_point(40)] }` in
  `pass/structs/runtime_nonzero_array_elements_exit`), which still records no
  root. Acceptance: that fixture advances past its array-literal statement with
  a full-corpus before/after showing 0 regressions.

- **UNIT-PLAN-OMISSION-SCALAR-REQUIRES-MEMBERSHIP.**
  (split-of:UNIT-PLAN-GAPS-REPORTED-AT-SOURCE)
  `checked_structural_signature_contract_supported`
  (`04_typed-trees-to-checked-trees/src/types/signature_contracts.rs`) resolves
  a `Requires`/`Membership` place only against **structural** parameters, so
  `machine make_point(value: u64 in Positive) -> Point` stops at `state
  contracts` on the implicit `requires value in Positive` of a scalar
  parameter. Found behind `f935b0f4e6c`, which unified the owned-structural-
  result question onto `has_structural_construction_contents` at the binding
  classifier, the ordinary call route and the callee's return-type admission.
  Blocks `pass/structs/runtime_nonzero_array_elements_exit` together with
  **UNIT-PLAN-OMISSION-PROJECTED-INITIALIZER-ROOT**. Acceptance: a machine with
  a domain-qualified scalar parameter and a structural result plans its state
  contracts.

  `pass/core/content_conservation_contract` is **design-blocked** and is not
  part of these: `reconstruct_structural_parameter_return_claims`
  (`checking/src/machine_calls/structural_call_custody.rs:641`) replays only a
  whole-input claim continuation, and the wrapper's returned claim is
  `Established` by a boundary partition. The fixture's own header already
  records this as owed via
  `ContentPartitionCompositionFact::source_derivation_depth`. Do not widen that
  recognizer; it is the conservation check.

- **ONE-TRANSITION-LOWERING.** (new-scope) Successor transitions with
  parameter transfer are lowered in about six places:
  - `unit/attached_unit/composed_control/state_graph/edges.rs` and
    `state_graph/emission/successor_edge.rs`;
  - `unit/structural_unit_control.rs`;
  - `scalar_graph/scalar_graph_module/state_emission.rs`;
  - `unit/dynamic_composed_unit/join.rs` and `continuation.rs`.

  Stage 04 builds successor plans in `execution/terminal_unit/state_graph/mod.rs`,
  `control/structural_unit_machine.rs`, `execution/terminal_scalar/successors.rs`,
  `composed_control/topology.rs` and `state_graph/closed_sum.rs`. Make one
  successor-plan builder in stage 04 and one lowering in the shared
  `unit/attached_unit/operation_frame.rs`, used by every route. Acceptance:
  a source-scanning guard finds a single constructor and a single lowering
  of successor transfers; corpus `--filter control_flow/,transitions,termination/,calls/`
  is unchanged in check and native.

- **ONE-CALL-LOWERING.** (new-scope) Calls are emitted by about ten routes:
  - `unit/attached_unit/operation_frame/calls.rs`, plus its ordinary,
    structural and scalar-structural call modules;
  - `composed_control` `emit_call_operations` and `emit_call_leaf`;
  - `scalar_graph_lowering/call_lowering.rs`;
  - `scalar_computations/calls.rs`;
  - both `boundary_calls.rs`;
  - the return routes.

  Structural arguments are assembled in 24 files. Stage 04 shares
  `calls/call_operations.rs::build_call_operation` at 9 sites but keeps
  separate builders in `terminal_scalar`, `dynamic_scalar_calls` and
  `returns/guarded_call_returns`. Make one argument preparation and one call
  emitter, fed by `build_call_operation` alone. Include the claim-transfer
  parity the ordinary route lacks:
  - `ClaimBindings::Fixed` (`operation_frame/calls.rs`) cannot mint a result
    claim;
  - `establish_structural_result` (`argument_evaluation.rs`) accepts linear
    locals only from ordinary structural-call producers.

  Acceptance: a source-scanning guard finds one call emitter and one
  `StructuralArgument` constructor in stage 05; corpus
  `--filter calls/,boundary/,providers/` is unchanged;
  `pass/calls/runtime_value_call_literal_len_arm_guard_exit` exits 70.

- **STRUCTURAL-UNIT-CONTROL-RETIRED.** (new-scope) Depends on
  **ONE-TRANSITION-LOWERING**. Fold
  `.../05_checked-trees-to-lowered-psi/src/unit/structural_unit_control.rs`
  into the composed state-graph route and delete it. That includes the
  `lower_ranked_structural_unit_countdown` rung, which requires exactly a
  header and an exit state with one scalar parameter. Ranked loops then
  lower through the general rank plans, without the
  `CheckedStructuralRankedArgumentPlan::UnsignedParameterMinusOne` special
  case. Acceptance: the file is gone; add corpus fixtures for ranked loops,
  transfer and cleanup frontiers, diamond joins and resumable execution
  (their Rust tests were cut); each executes natively with its oracle.

- **RETURN-ROUTES-RETIRED.** (new-scope) Depends on **ONE-CALL-LOWERING**.
  Replace the six return rosters in
  `.../05_checked-trees-to-lowered-psi/src/returns/` (structural-scalar,
  boundary-scalar, structural, claim-free affine, trait-operator and guarded
  payloadless) with ordinary body operations plus result completion, keeping
  outcome-selected evidence and proof-only tail arguments. Coordinate operator
  returns with **OPERATOR-BOUNDARY-CALLS**.
  `pass/providers/specialized_structural_fixed_operator_terminal_custody` has
  an unused `exercise` beside an empty entry, so its build proves nothing:
  give it an entry that executes the operator. Acceptance: `returns/` holds
  no route of its own, `CheckedReturnPlan` is gone, and corpus
  `--filter returns/,operators/,providers/` is unchanged in check and native.

- **DYNAMIC-UNIT-ROUTE-RETIRED.** (new-scope) Depends on
  **ONE-CALL-LOWERING**. Lower dynamic dispatch as an ordinary call operation
  inside the body route and delete
  `.../05_checked-trees-to-lowered-psi/src/unit/dynamic_composed_unit/`
  (4.9k lines, Scalar and Unit variants across four binding kinds). Embed
  dynamic-call custody in the ordinary call plan instead of
  `DynamicCallCustody`
  (`omega-rust/psi/compilation/pipeline/04_typed-trees-to-checked-trees/src/execution/terminal_unit/dynamic_scalar_calls/call_plans.rs`)
  and its separate join views. Relaxing the dynamic join's `_` check alone
  does not admit a two-case sum dispatch. Acceptance: the directory is gone;
  `pass/traits/` and the dyn dispatch run fixtures are unchanged in check and
  native.

- **SCALAR-GRAPH-ROUTE-RETIRED.** (new-scope) Depends on
  **STRUCTURAL-UNIT-CONTROL-RETIRED**, **RETURN-ROUTES-RETIRED** and
  **DYNAMIC-UNIT-ROUTE-RETIRED**. Lower scalar machines through the same body
  route as Unit machines and delete
  `.../05_checked-trees-to-lowered-psi/src/scalar_graph/` (19.4k lines) and
  the plan-presence matrix in `machine_lowering/machine_dispatch.rs`. Account
  for:
  - continuation-arm custody (`CheckedStructuralControlSuccessorPlan` lacks
    continuation identity);
  - later-state mutable scalar stores;
  - crash predicates, float prefix initializers and debug maps.

  Acceptance: `machine_dispatch.rs` selects no route by plan presence; distinct
  exits returning 20 and 30 under `ensures result >= 20` still hold;
  constant-stack mutual tail fusion and `pass/calls/mutual_cycle_tail_admitted_exit`
  (exit 70) still hold; and the full check corpus and the native corpus are
  unchanged.

- **UNIT-PLAN-ASSEMBLY-ONCE.** (new-scope) Stage 04 assembles the Unit plan
  twice in places:
  - nominal cleanup is built both in the Unit candidates and in
    `finalize_execution`;
  - each roster keeps its own `ShapeCollector` namespace;
  - `UnitPlans::with_staged` (`.../05_checked-trees-to-lowered-psi/src/unit/attached_unit/bodies.rs`)
    unions them by hand.

  Build nominal cleanup once and keep one type namespace in the existing
  representations. Acceptance: `with_staged` and the per-roster namespaces are
  gone; corpus `--filter drops,ownership/,cleanup` is unchanged.

- **STORE-OPERATIONS-GENERALIZED.** (new-scope) The checked plan vocabulary
  splits stores by shape into nine variants
  (`omega-rust/psi/compilation/representations/checked-trees/src/checked_trees/flow/terminal/unit_effect_plans.rs`):
  - `WriteOnlyPrimitiveStore`;
  - `StructuralScalarFieldStore`, `StructuralCaseFieldStore` and
    `StructuralCaseLiteralFieldStore`;
  - `ByteSequenceStore` and `StructuralByteSequenceFieldByteStore`;
  - `ElementViewWrite`, `RepresentationViewStore` and `StoreStructuralField`.

  Each variant has its own stage 04 builder and stage 05 `operation_frame`
  arm. Replace them with one store operation over a place path and a value,
  where the representation decides the emitted bytes. Acceptance: one store
  variant remains; corpus `--filter structs/,collections/,slices/,text/,layouts/`
  is unchanged in check and native.

- **LOCAL-BINDINGS-GENERALIZED.** (new-scope) Local data bindings are split
  into `EstablishPrimitiveLocal`, `EstablishScalarLocal`,
  `EstablishStructuralLocal` and `TrivialAffineLocal`, plus byte-sequence,
  scalar-array and structural-value forms. Stage 05 handles them at about
  eight sites (`operation_frame::establish_scalar_local` and
  `establish_structural_local`, `primitive_locals`, `ordinary_machine/locals`,
  `structural_values/locals`, `byte_locals`, `scalar_graph_lowering/bindings.rs`,
  `emission::emit_scalar_binding`). Replace them with one local-binding
  operation keyed by the value's representation. Acceptance: one binding
  variant and one lowering remain; corpus `--filter expressions/,borrows/,calls/`
  is unchanged.

- **SHAPE-RUNGS-GENERALIZED.** (new-scope) Replace these stage 04 shape
  recognizers with their general operation:
  - `exact_two_field_record_projection`
    (`omega-rust/psi/compilation/pipeline/04_typed-trees-to-checked-trees/src/execution/terminal_unit/mod.rs`,
    used by `execution/terminal_cleanup.rs`), "the first checked
    projected-transition cleanup rung";
  - `first_unsupported_statement` in
    `.../execution/terminal_unit/control/statement_sequence.rs`, which admits
    a trailing expression only through about ten shape disjuncts;
  - the per-shape completion producers in the same file (`returned_parameter`,
    `returned_named_view`, `returned_subslice`, `returned_mutable_view`,
    `returned_byte_sequence_literal`).

  Acceptance: these functions are gone. New corpus fixtures cover a
  three-field projected-transition cleanup and a trailing expression outside
  the old disjuncts; both check and execute. Existing outcomes are unchanged.

## Checker simplification

Owner ruling 2026-10-04: remove the checker's re-checking, duplicate analyses,
shape gates and review/certificate support. The checker is stage 04
(`omega-rust/psi/compilation/pipeline/04_typed-trees-to-checked-trees/src`,
about 253k lines) plus the `validation` crate it calls
(`omega-rust/psi/compilation/checking`, about 169k lines). An audit at
`c252be7fa2` estimates 40–60k of those lines decide nothing new. The rulings
in [Trust ceremony](#trust-ceremony), [PCC removal](#pcc-removal) and
[Lowering simplification](#lowering-simplification) apply here. Related items
already filed elsewhere: **LOWERING-READS-CHECKED-TYPES**,
**LOWERING-READS-CHECKED-BORROW-CUSTODY**, **TYPED-FROZEN-BEFORE-CHECKING**
and **CHECKING-FROZEN-PROGRAM-CONTEXT**.

Each item lands in one commit, deletes what it replaces, and keeps every
check-corpus verdict and diagnostic family unchanged unless the item says
otherwise. Paths below are relative to the stage 04 `src/` (written `omega-rust/psi/compilation/pipeline/04_typed-trees-to-checked-trees/src/`) or
the validation `src/` (written `omega-rust/psi/compilation/checking/src/`) unless spelled in full.

- **BORROW-CERTIFICATES-DELETED.** (new-scope) Remaining work is the
  lowering-consumed reborrow resource certificates, not borrow admission.
  Stage 04 now checks compatibility, mutation and calls directly over one
  `BorrowFacts`, without source reconstruction, selector snapshots or
  certificate replay. On macOS ARM64, the `borrows/,borrow/,references/`
  check slice retains all 176 baseline outcomes (114 checked, 62 rejected);
  the positive native slice executes 91 programs with matching authored
  oracles.

  Retire `checks/borrows/resources/{resource_reconstruction,certificate_planning}.rs`
  only with their remaining consumers: stage 04
  `execution/terminal_unit/{receiver_aliases,calls/structural_arguments.rs}`
  and stage 05 `retention/{reborrow_root_handoff,reborrow_restored_call_use}.rs`
  and `machine_lowering/reborrow_handoffs.rs`. These consume
  `reborrow_containment_certificates` and
  `reborrow_restored_call_use_certificates`; deleting their construction
  alone removes required lowering inputs. Coordinate those currently assigned
  paths with **REFERENCE-LOCAL-BINDING-ROUTES**. Keep direct loan resources and
  access attenuation, parent/child lifetimes, conflict, writability, escape
  and persistence judgments in the single production route. Acceptance:
  no checker self-replay or obsolete resource-certificate consumers, one
  `build_borrow_facts` call per check, unchanged borrow check corpus and
  native restoration results.

- **SPECIALIZATION-LOOP-ONCE.** (new-scope) The specialization fixed point in
  `omega-rust/psi/compilation/pipeline/04_typed-trees-to-checked-trees/src/checking.rs` re-runs five whole-program validations
  (`validate_conformance_applications`,
  `validate_static_machine_selections_with_facts`,
  `validate_generic_machine_contract_entailment`, `infer_operational_may`,
  `validate_static_machine_call_contracts`) before the loop and on every
  iteration, and repeats `bind_selected_operator_calls` (3 + N runs),
  `normalize_open_index_identities` (2 + N), `resolve_projected_receiver_calls`
  (2) and `land_float_literal_destinations` (2–3). Make the loop specialize
  and bind only; run each validation, receiver resolution and float landing
  once after it converges. Coordinate with **TYPED-FROZEN-BEFORE-CHECKING**,
  which moves the loop to its own stage. Acceptance: each named pass runs once
  per check except the per-iteration specialize/bind step; the
  generic-value-call fence and call-contract diagnostics are unchanged; corpus
  `--filter generics,operators,traits/` unchanged.

- **CHECK-ONCE-ACROSS-TARGETS.** (new-scope) The driver re-checks the whole
  program for each selected target
  (`omega-rust/omega/compilation/driver/src/checked/checking/execution_settlement.rs`
  replaces the selected target's bodies and calls `lower_typed_trees` per
  compile; inferred from reading, not measured). Psi already checks every
  target-scoped body in one compilation (AGENTS.md: targets are data). Check
  once and have each target's realization read the shared checked trees.
  Coordinate with **PSI-TARGET-FAMILIES**. Acceptance: a two-target build
  calls `lower_typed_trees` once (add a counter or trace); native corpus on
  the host target unchanged.

- **ONE-INTEGER-BOUNDS-ENGINE.** (new-scope) At least ten separate engines
  answer "what range can this integer take":
  - stage 04: `omega-rust/psi/compilation/pipeline/04_typed-trees-to-checked-trees/src/checks/ranges/facts` (`RangeFacts`),
    `omega-rust/psi/compilation/pipeline/04_typed-trees-to-checked-trees/src/checks/contracts/intervals.rs`, `omega-rust/psi/compilation/pipeline/04_typed-trees-to-checked-trees/src/checks/contracts/evaluator`,
    `omega-rust/psi/compilation/pipeline/04_typed-trees-to-checked-trees/src/values/bounds` (`PlaceIntegerBounds`), `omega-rust/psi/compilation/pipeline/04_typed-trees-to-checked-trees/src/checks/termination/ranking`
    ranges (`Bounds`), `omega-rust/psi/compilation/pipeline/04_typed-trees-to-checked-trees/src/checks/borrows/overlap/indexes.rs`
    (`NormalizedBound`), `omega-rust/psi/compilation/pipeline/04_typed-trees-to-checked-trees/src/checks/ranges` `MergedBound` and the loop
    invariant `Bounds`;
  - validation: `omega-rust/psi/compilation/checking/src/proof_contracts/arithmetic_domains/interval.rs`,
    `omega-rust/psi/compilation/checking/src/proof_contracts/contract_entailment/arithmetic_judgment.rs`,
    `omega-rust/psi/compilation/checking/src/proof_contracts/arithmetic_domains/invariant_bounds.rs`,
    `omega-rust/psi/compilation/checking/src/proof_contracts/arithmetic_domains/integer_ranges.rs`,
    `omega-rust/psi/compilation/checking/src/value_custody/struct_literals/construction_bounds.rs`;
  - `omega-rust/psi/compilation/proof/src/checker/integer_ranges.rs`.

  Keep one interval domain and one premise-chaining oracle in `validation`
  (`arithmetic_domains`), and make every listed site a client. Float intervals
  (`omega-rust/psi/compilation/checking/src/value_custody/expression_types/float_cast_proofs.rs`) stay
  separate. This is large: land it per client, each commit deleting that
  client's private engine. Acceptance: no private integer interval type
  remains outside `arithmetic_domains`; corpus `--filter ranges,arithmetic/,
  collections/,slices/,termination/,contracts` unchanged.

- **ONE-ENSURES-PROVER.** (new-scope) Exit guarantees are decided twice:
  `omega-rust/psi/compilation/checking/src/proof_contracts/contract_entailment` (26k lines) judges
  requires⇒ensures, and `omega-rust/psi/compilation/pipeline/04_typed-trees-to-checked-trees/src/checks/contracts/{exits,prover}` independently
  proves exit `ensures` from flow. The two are bridged by "stand-downs":
  `collect_contract_entailment_stand_downs` re-runs whole-program entailment
  and discards its diagnostics, called from
  `omega-rust/psi/compilation/pipeline/04_typed-trees-to-checked-trees/src/checks/contracts/exit_membership.rs`, `omega-rust/psi/compilation/pipeline/04_typed-trees-to-checked-trees/src/proof/contract_entailment.rs`
  (twice) and
  `omega-rust/omega/compilation/realization/selected-dispatch/src/execution_settlement.rs`.
  Make the stage 04 prover the one decision over bodies and have it call the
  entailment engine for proof-only and contract-to-contract judgments; delete
  the stand-down bridge and the selected-dispatch call. Acceptance:
  `collect_contract_entailment_stand_downs` is gone; corpus
  `--filter contracts,proofs/,ensures` unchanged.

- **QUOTIENT-CORRESPONDENCE-ROWS-DELETED.** (new-scope) Delete the Terminal
  correspondence-row and certificate production for quotients:
  `omega-rust/psi/compilation/checking/src/proof_contracts/quotients/relation_plan/correspondence_certificate.rs`,
  the row emission in `relation_plan/terminal_bridge.rs` and
  `omega-rust/psi/compilation/checking/src/proof_contracts/quotients/terminal_bridge.rs`,
  and `extract_non_executable_quotient_correspondences*` (the review capture
  that read them is already gone). The stage 05 side is deleted by **PCC-CERTIFICATES-DELETED**. Keep quotient
  formation and request checking (`equivalence_selection`,
  `formation_collection`, `carrier_fence`) and the relation/theorem schema
  judgments that **QUOTIENT-THEOREM-LIFT** and
  **QUOTIENT-RUNTIME-REALIZATION** need; rewrite **QUOTIENT-THEOREM-LIFT** to
  build on those judgments instead of the deleted bridge in the same commit.
  Acceptance: `pass/proofs/quotient_*` and `fail/proofs/quotient_*` keep
  their verdicts; no quotient code emits Terminal rows or certificates.

- **CHECKER-RECORDS-ONCE.** (new-scope) Values the checker decides are
  recomputed by later consumers instead of read from checked trees:
  - `omega-rust/psi/compilation/checking/src/machine_calls/machine_specialization_identity.rs` and
    `omega-rust/psi/compilation/checking/src/machine_calls/static_machine_call_contracts.rs` hash
    commitments that stage 05, selected-dispatch, provider-planning and
    compile-time each recompute and compare;
  - Omega stage 00 still reads Psi's typed trees in
    `omega-rust/omega/compilation/pipeline/00_terminal-psi-to-abstract-operations/src/comparison_settlement.rs`
    and in `IntrinsicRequirement::by_symbol` inside
    `boundary_application_settlement.rs`.

  Replace the commitments with a plain identity key where one is needed and
  delete the hashing; record what stage 00 reads in checked trees or Terminal
  Psi. Acceptance: those modules hold no replay; no Omega crate references
  `checked.typed`; corpus unchanged.
  (**LOWERING-READS-CHECKED-BORROW-CUSTODY** already covers the
  `reference_result_custody` recompute in stage 05.)
- **LOWERING-GATES-OUT-OF-CHECKER.** (new-scope) The checker carries lowering
  capability gates and per-shape query variants:
  - 16 `*_supported` functions (for example
    `result_initializer_call_is_supported`, which matches
    `if let [state] = … && matches!(…last(), Some(StatementNode::Expression(e)) …)`);
  - 12 `has_*_contents` variants in
    `omega-rust/psi/compilation/checking/src/value_custody/storage_contents.rs` (such as
    `has_plain_owned_contents_with_numeric_constraints` and
    `…_with_represented_constraints`);
  - about 50 `…_with_…` function variants (for example
    `land_anonymous_integer_expression{,_with_selected_match_arms,_with_warning}`).

  Delete the `*_supported` gates: the checker decides legality and lowering
  reports its own gaps (**UNIT-PLAN-GAPS-REPORTED-AT-SOURCE**). Replace each
  variant family with one parameterized query. Acceptance: no `*_supported`
  function remains in `omega-rust/psi/compilation/checking/src/` or `omega-rust/psi/compilation/pipeline/04_typed-trees-to-checked-trees/src/`; one `has_contents` query takes
  the constraint kind as a parameter; corpus unchanged.

- **VALUES-SCALAR-PLANS-MERGED.** (new-scope) `omega-rust/psi/compilation/pipeline/04_typed-trees-to-checked-trees/src/values/scalar` (about 17k
  lines) builds scalar and computation plans for lowering and checks nothing
  ("Independent lowering replays that occurrence…"). It likely overlaps the
  `omega-rust/psi/compilation/pipeline/04_typed-trees-to-checked-trees/src/execution/terminal_unit` plan routes (unverified). Merge it into the
  single plan builder that **ONE-CALL-LOWERING** and
  **ONE-TRANSITION-LOWERING** establish, deleting duplicated plan
  construction. Depends on those two items. Acceptance: one plan builder
  produces scalar and Unit computations; native corpus unchanged.

## Data-flow defects

Each item removes one verified copy, rescan or repeated validation in the
compiler's flow from source bytes to image bytes. Re-read the cited symbol
before starting; paths drift. Each item lands alone with the cited corpus
slice unchanged (check plus `--native` where the code is backend-visible).
Re-checking stays allowed at trust boundaries; these items remove repetition
of a check the same flow already performed, not the check itself.

- **CHECKED-LEDGER-VERIFY-ONCE.** (new-scope) Stage 05's
  `omega-rust/psi/compilation/pipeline/05_checked-trees-to-lowered-psi/src/machine_lowering/checked_ledger.rs::verify`
  runs once per `lower_machine` call, so once per entry and callback artifact
  over the same whole-program ledger; it should run once per `CheckedTrees`.
  Do not hoist the call into `lower_machine`'s callers: three sites construct a
  `TerminalProductionRequest`
  (`omega-rust/omega/compilation/driver/src/terminal/terminal_artifact.rs`,
  `omega-rust/omega/compilation/driver/src/terminal/native_proposal/mod.rs`,
  `omega-rust/omega/compilation/products/src/checked_compilation/terminal.rs`),
  and hoisting turns a guaranteed invariant into an obligation each must
  remember. Verify the ledger where the `CheckedTrees` is frozen, so it holds
  by construction before any artifact is produced; land with
  **CHECKING-FROZEN-PROGRAM-CONTEXT**. Acceptance: one `verify` call per
  `CheckedTrees`, and the check corpus is unchanged.

- **CALLABLE-IDENTITY-INTERNED.** (new-scope) `NormalizedNamedCallableIdentity`
  (`omega-rust/psi/compilation/representations/typed-trees/src/typed_trees/type_system/type_identity/normalized_identities.rs`)
  holds its `path` and `parameters` as rendered, escaped `String`s, and
  `TypedTrees::machine_by_normalized_overload_identity(&str)` finds a machine
  by rendering every machine's identity and comparing strings. In one commit:
  - intern each machine, requirement and operator overload identity once
    during typing, in a `TypedTrees` table mapping a `CallableIdentityId`
    handle to its declaration;
  - replace the string identity type and the `&str` lookup with that handle
    and an O(1) table read;
  - render text only for diagnostics and at artifact encoding.

  Acceptance: `NormalizedNamedCallableIdentity` holds no `String`, no function
  looks a machine up by a string identity, and full check plus native corpus
  are unchanged.

- **TYPE-IDENTITY-INTERNED.** (new-scope) `NormalizedTypeIdentity(String)` (same
  file) is rendered afresh on every `TypedTrees::normalized_type_identity` call
  across stages 04, 05 and checking, and compared as text. Keep a hash-consed
  table of structural type keys in `TypedTrees` that issues one
  `TypeIdentityId` handle per distinct type, and switch every comparison and
  map key to that handle in one commit. Rendering stays only for diagnostics
  and artifact encoding. Acceptance: `NormalizedTypeIdentity` holds no
  `String`, type identity equality is a handle comparison, and full check plus
  native corpus are unchanged.

  An unfinished attempt exists and is NOT on any branch: 125 modified files,
  reaching layout, Omega stages 00 and 03, and provider planning. It was
  stopped mid-task to relieve a load average of 140, with the handle
  conversions for its regenerated files, formatting and the workspace check
  still outstanding, so it is not known to compile. Preserved as a patch at
  `<session scratchpad>/PRESERVED_type_identity_interning.patch` against
  `dfb4587153c`; ask the coordinator for it rather than restarting from zero,
  but treat it as a sketch, not a candidate. Note how far the blast radius
  reaches: a change described as "one commit" in this item touched 125 files,
  so plan the measurement before the edit.

- **CHECKED-IDENTITY-IDS.** (new-scope) Depends on
  **CALLABLE-IDENTITY-INTERNED** and **TYPE-IDENTITY-INTERNED**.
  `checked-trees` declares many `*identity: String` fields, concentrated in
  `omega-rust/psi/compilation/representations/checked-trees/src/checked_trees/flow/dynamic_scalar_calls.rs`
  and `.../flow/terminal/structural_type_plans.rs`, with others in
  composed-unit, affine-cleanup, result-binding, termination and
  service-parameter plans. Replace each with the callable or type identity
  handle (or the existing symbol handle) in one commit. Acceptance: no
  `String`-typed identity field remains in `checked-trees`, enforced by a
  source-scanning guard in that crate; full check corpus unchanged.

- **TERMINAL-IDENTITY-TABLE.** (new-scope) Depends on **CHECKED-IDENTITY-IDS**.
  Terminal Psi is the persisted boundary, so its identities stay
  content-derived, but they should be stored once. `terminal-psi`'s module
  repeats rendered text in `*identity: String` fields
  (`omega-rust/psi/compilation/representations/terminal-psi/src/terminal_module/boundary/dynamic_dispatch.rs`,
  `.../terminal_module/ownership/borrows.rs`, placement, conformances,
  outputs, recursion). Add one identity table section to the module and the
  codec; fields hold indices into it, and decoding checks the table once. Bump
  the codec version in the same commit. Acceptance: no `String`-typed identity
  field remains in `terminal-psi` outside the identity table (source-scanning
  guard); the native corpus is unchanged.

- **OMEGA-IDENTITY-INDICES.** (new-scope) Depends on
  **TERMINAL-IDENTITY-TABLE**. Omega consumes those identities as owned text:
  - `*identity: String` fields under `omega-rust/omega/compilation/backend/plans`
    and `omega-rust/omega/compilation/backend/runtime`;
  - the `BoundaryNominalIdentity(String)` and `BoundaryTypeIdentity(String)`
    newtypes in
    `omega-rust/omega/compilation/backend/artifacts/boundary-applications/src/boundary_applications.rs`.

  Carry Terminal identity-table indices instead, in one commit. Acceptance:
  none of those crates declares a `String`-typed identity (source-scanning
  guard in each), and the native corpus is unchanged.

- **PROVIDER-SCHEMA-IDENTITY-IDS.** (new-scope) Depends on
  **CALLABLE-IDENTITY-INTERNED**. The `effects` crate's
  `ServiceSchema`/`ServiceMethod`
  (`omega-rust/omega/compilation/backend/plans/provider-evidence/src/provider_evidence/providers/provider_plan/service_schemas.rs`)
  carry `trait_name` and `requirement_identity`, and
  `ProviderBinding::CheckedAdapter` carries `machine_identity`, all as rendered
  `String`s that selection, dispatch and digest code compare as text. Hold
  callable identity handles in-process; only the plan's digest encoder writes
  the canonical content. One commit. Acceptance: those schema and binding
  fields hold no `String`, plan digests (and so package-review records) are
  unchanged, and the provider and native corpus slices are unchanged.

- **PRE-ALLOCATION-BATCH-COMMIT.** (new-scope) Stage 04's
  `omega-rust/omega/compilation/pipeline/04_selected-instructions-to-selected-instructions/src/rewrites/pre_allocation/execution.rs`
  commits the first admissible candidate and restarts the whole sweep. Commit
  every non-conflicting candidate in a sweep and restart only when a commit
  enables another family. Acceptance: the same fixed point is reached in at
  most as many sweeps as family interactions require, and native corpus
  `--filter optimizer/` is unchanged.

- **CHECKING-FROZEN-PROGRAM-CONTEXT.** (new-scope)
  `omega-rust/psi/compilation/checking/src/frozen_program.rs` keeps its memos
  in a `thread_local!` `RefCell` slot keyed by the program's address, which
  AGENTS.md rules out as an ownership escape. Replace it with an explicit
  context value passed to its consumers. The global `ENVIRONMENTS` mutex cache
  with a linear find in
  `omega-rust/omega/compilation/pipeline/03_target-operations-to-selected-instructions/src/selection/mod.rs`
  goes the same way: build the environment once per realization and pass it.
  Acceptance: neither the thread-local nor the static mutex remains, and the
  check corpus is unchanged.

- **TYPED-FROZEN-BEFORE-CHECKING.** (new-scope) `CheckedTrees` embeds the
  typed program, but stage 04 rewrites it in place first, so the embedded
  value is a checking-time variant rather than typing's output.
  `omega-rust/psi/compilation/pipeline/04_typed-trees-to-checked-trees/src/checking.rs::lower_typed_trees`
  takes the program by value and then:
  - runs the specialization fixed point
    (`specialize_static_machine_calls_with_selections`,
    `generate_dynamic_family_specializations`,
    `bind_selected_operator_calls`, with `normalize_open_index_identities`
    every round);
  - runs the binding and landing passes `resolve_projected_receiver_calls`
    (twice), `resolve_dynamic_call_targets`,
    `resolve_named_result_overloads`, `bind_float_conversion_casts`,
    `land_float_literal_destinations` (twice), `refine_default_domain_zero`,
    `bind_pre_specialization_authored_selections`;
  - after checking, writes conclusions back through
    `finalize_checked_authored_selections(&mut program, &facts)`
    (`src/authored_selections/finalization.rs`).

  In one commit, move the specialization fixed point and its rebinding passes
  into a new `typed-trees-to-typed-trees` stage between stages 03 and 04,
  calling each pass once at its required point; record finalized authored
  selections in `CheckFacts` instead of the typed program; delete the moved
  calls from stage 04 with no fallback route. The new stage takes the same
  selected-provider inputs stage 04 takes, so land it before or with
  **PROVIDER-SELECTION-AFTER-TERMINAL**. Acceptance: `lower_typed_trees`
  performs no `&mut` pass over the program and nothing writes to
  `CheckedTrees.typed` after construction (source-scanning guard), and the
  full check corpus plus native corpus `--filter
  generics,operators,providers/` are unchanged.

- **LOWERING-READS-CHECKED-TYPES.** (new-scope) Stage 05
  (`omega-rust/psi/compilation/pipeline/05_checked-trees-to-lowered-psi/`)
  recomputes place and expression types that checking already resolved, via
  `validation::declared_place_type_raw`, `validation::unwrapped_type_reference`
  and `validation::expression_result_type_reference`. Have checking record
  each resolved place and expression type once, in a dense `CheckFacts` table
  indexed by expression handle, and replace every stage 05 call with a read of
  that table in one commit. Acceptance: stage 05 non-test sources call none of
  the three functions (stage 05 source-scanning guard); check and native
  corpus unchanged.

- **LOWERING-READS-CHECKED-BORROW-CUSTODY.** (new-scope)
  `CheckFacts::reference_custody` records one `ReferenceCustodyRow` per type
  reference, and stage 05 reads it at the 34 sites keyed by a type reference
  alone (`001397f340a`). The item's original premise was wrong in a way worth
  keeping: none of these functions recomputes a checking *analysis* — checking
  and lowering call the same pure functions over the frozen `TypedTrees`. The
  query could not simply move to its data's owner because the family pulls in
  `value_custody/storage_contents.rs`, whose own content classification is now
  recorded beside it, and because the return types live in `checked-trees`,
  which `typed-trees` cannot depend on. So a recorded row is right here even
  though the query is pure.

  `CheckFacts::state_reference_custody` records one `StateReferenceCustodyRow`
  per declared state, dense by the state symbol's arena index, and stage 05
  reads it at the 7 sites keyed by a state alone (`e1ca328f3ce`). **The plan
  roster this item predicted was not needed**: every stage-05 reader of
  `returned_reference_sources`, `source_parameter` and `source_leaf` consumes
  presence only (`.is_none()` in a refusal guard), and `tail_return_expression`
  returns one ZII handle, so a copy row is the whole answer. Recording the
  roster would have been facts with no reader -- the ground on which
  `001397f340a` declined to record `local_owes_record_loans`. Check what the
  readers consume before choosing a row shape. Population walks `machines()` by
  `machine_states()`, the structure `state_declaration` already resolves a
  symbol through (and which refuses a symbol naming two states).

  **15 sites remain, and the surface splits 6/6/3, not 6/6** -- this item twice
  stated the count wrong. 6 are keyed by a `Handle<BorrowLoanFact>` that the
  query also filters against a machine and state (`release_boundary` 4,
  `release_statement` 2), where a row must record the pair it was computed for
  or a mismatched pair reads a boundary the recompute denies. 6 are keyed by a
  state and an expression (`initializer_source` 3,
  `borrowed_initializer_source`, `mutable_return_source`, `record_argument`),
  and `record_argument` additionally takes a
  `CheckedUnitStructuralResultBindingPlan` that stage 05 holds, so nothing
  checking can enumerate keys it until that query is decomposed. 3 are keyed by
  a state **and a local ordinal** (`primitive_local_source`), which a
  state-alone row does not close.

  **This item now conflicts with a newer owner ruling, and that must be settled
  before the next family is recorded.** 4 of the 7 sites just converted were
  pure refusal guards, otherwise unused. `03710d6b5d9` cites the owner ruling
  of 2026-10-04 under [Lowering simplification](#lowering-simplification), and
  **STAGE05-SOURCE-REREAD-DELETED** says "the validators that only re-confirmed
  a row against its source are deleted". Converting a guard-only site to a
  recorded-fact read *preserves* the re-checking that ruling removes, so those
  4 rows may be short-lived and the remaining families may want deletion rather
  than rows. Classify each remaining site as guard-only or value-consuming
  first, and record only the value-consuming ones.

  Cost named rather than guessed: the classification pass is a second
  whole-program walk, and unlike the type-reference walk it is not uniformly
  cheap -- `returned_reference_sources` iterates every transition of every
  state and recurses through callees comparing normalized type identities. If
  it needs narrowing, the early-out belongs **inside that query**, where the
  row and the recompute cannot drift; hand-deriving a narrower predicate in the
  builder is a recurring defect class here.

  Population must not depend on checking's traversal: `001397f340a` walks the
  whole type-reference table after monomorphization and authored-selection
  finalization, so no visited-call hole is possible, and the table is
  append-only so no index goes stale. Before deleting a recompute, prove
  equivalence with a differential probe that reads the row and recomputes
  beside it over the full corpus — that is the only evidence these crates have,
  since they carry zero Rust tests after `55a502d211b`. Acceptance: stage 05
  non-test sources do not name `reference_result_custody` (same source-scanning
  guard); corpus `--filter borrows,references/` check and native unchanged.

- **IMAGE-SELF-REPLAY-DELETED.** (new-scope) Direct image emission, the
  fragment and container stages and `NativeArtifact::from_emitted_parts`
  now build and seal an image once. One rebuild remains, on a path this
  macOS host cannot run:
  `omega-rust/omega/compilation/backend/images/image-emission/src/dynamic_elf.rs`
  `validate_dynamic_elf_image_emission` builds the final image a second time
  (`image::build_final_image`) to compare against the writer bytes that
  `emit_admitted_dynamic_elf_image` accepts from its caller. Pass the
  emitted image through instead of rebuilding it, under
  [verification placement](omega-rust/pipeline.md#verification-placement);
  the comparison stays only where a retained artifact is read back.
  Acceptance: dynamic ELF emission builds its image once, and the
  `linux_x86_64` native corpus leg on a Linux host is unchanged.

## Trust ceremony

Owner ruling: keep security where it decides something, delete it where it
only performs. Re-read the cited symbol before starting.

These boundaries stay:
- accepting new or changed foreign code: install, update, review and audit
  commands;
- authenticating fetched git objects against pins, and the lock-pin match;
- `build.omg` confinement in the checked interpreter: `FsGrants`
  authorization before each host call, fuel and cell limits, rejecting
  runtime boundary services, and output alias and size checks;
- receiver admission of authority (owned by
  **TWO-AXIS-TERMINAL-AUTHORITY-REVIEW**);
- Psi's checking of capability and effect reach, and of authored behavior
  exclusions.

Everything else is deleted: a component re-checking data the same process
just produced, a guard against a writer who could equally rewrite the lock or
the compiler, and re-deriving a fact that is already established. Each item
lands alone with its corpus slice unchanged and no fallback path.

- **SOURCE-READ-ONCE.** (new-scope) An ordinary warm compile reads and hashes
  the root project's tree many times, more when the bound pass runs, and each
  dependency is re-hashed by its own occurrence and by every consumer's scope.
  The extra reads are:
  - `omega-rust/omega/packages/package-manager/src/review/candidate/custody.rs::verify_selected_source_custody`,
    before and after each occurrence
    (`review/candidate/compilation/package_pass.rs`);
  - `selection_evidence().revalidate()`;
  - `omega-rust/omega/packages/package-manager/src/admission/evidence/validation.rs::revalidate_source_custody`;
  - `omega-rust/omega/packages/compilation-inputs/src/source_snapshot.rs`
    `require_stable_recapture`, plus `validate_current` straight after
    `capture`;
  - `BuildSourceCustody::seal` → `verify_current_files`
    (`omega-rust/omega/packages/compilation-inputs/src/source_consumption/checked_sources.rs`);
  - `CheckedCompilation::verify_current_source_consumption`
    (`omega-rust/omega/compilation/products/src/checked_compilation.rs`);
  - the prepare-time `package_compilation_inputs` that
    `into_target_review_parts` discards;
  - `omega-rust/omega/packages/source-acquisition/src/local/snapshot.rs`
    `verify_local_snapshot` and `verify_live_source_unchanged`, and the
    persistent snapshot copy of the root project made on every edit;
  - `omega-rust/omega/packages/compilation-inputs/src/toolchain_library.rs::validate`,
    the compiler hashing its own bundled library.

  Each source file is read and hashed once per invocation, and the compile
  uses those bytes. The root compiles from its live tree; git snapshots are
  verified at fetch, not on every compile. Delete the re-checks in one commit.
  Acceptance: a read counter on the file-read seam, run over a package corpus
  fixture, shows one read per consumed file per invocation, and the check
  corpus is unchanged.

- **BUILD-SESSION-RECEIPTS-DELETED.** (new-scope) Delete the in-process
  bookkeeping around build evaluation and keep only the real confinement:
  - `omega-rust/omega/packages/package-manager/src/review/candidate/compilation/session_accounting.rs::verify_build_session_accounting`
    (usage reconciliations against `BuildEvaluationSponsor`);
  - `FilesystemSponsor`'s mirror namespace, its sponsor/host divergence halt,
    and its quiescence snapshots (the dangling-symlink escape the mirror used
    to be the only guard against is now refused by
    `omega-rust/psi/interpreter/checked-interpreter/src/interpreter/evaluator/filesystem/real.rs`
    resolution);
  - the observation identity and production-subject chain
    (`omega-rust/omega/packages/compilation-inputs/src/build_execution/observation_identity.rs`,
    `omega-rust/omega/packages/compilation-inputs/src/production_subject.rs`,
    `omega-rust/omega/compilation/products/src/checked_compilation/production_subject.rs`,
    the admitted-checkpoint symbol check in
    `omega-rust/omega/compilation/build-evaluation/src/build_evaluation.rs`);
  - `omega-rust/omega/compilation/build-evaluation/src/completed_pure_build.rs`
    logical charges;
  - re-reading the materialized source copy (`verify_materialized_source`)
    and written outputs (`verify_materialized_tree`,
    `omega-rust/omega/packages/compilation-inputs/src/build_execution/completed_outputs.rs::publish`).

  Also in this commit:
  - pure builds skip source capture;
  - the captured source inventory is served from memory, which
    `wiki/spec/build/scoped_execution.md` allows, instead of a temp-directory
    copy;
  - `PackageBuildSession`
    (`omega-rust/omega/packages/package-manager/src/build_session.rs`)
    creates its write root once per invocation, not once per pass.

  Acceptance: none of the named functions exists; the
  `filesystem/real/confinement_tests.rs` tests pass; grant refusal, fuel
  exhaustion, and alias and size rejection still reject (add a corpus fixture
  for each case the build corpus does not already cover); the build and
  filesystem corpus slices are unchanged.

- **RESTRICTED-REQUEST-GATE-ONCE.** (new-scope) `grants.admit(request)` in
  `omega-rust/omega/compilation/build-evaluation/src/build_evaluation.rs` is
  the gate. After it,
  `omega-rust/omega/packages/package-manager/src/review/candidate/compilation/package_pass.rs::reject_ungranted_restricted_requests`
  re-projects `PackagePolicyAcceptance::from_policy(review.policy())` and
  compares the same requests again, including for cache-carried occurrences
  where no build ran. Delete the post-hoc pass. Separately, with
  `restricted_build_checkpoint: None` (install/update review, audit,
  inspection, `--check --verify`), `if let Some(grants)` admits everything.
  Make `None` refuse real-host scoped requests: `wiki/spec/packages/review.md`
  says "An audit must not perform the very action it is asking the owner to
  approve". Acceptance: one gate call per request; an audit of a package whose
  build requests scoped filesystem execution performs no host effect; corpus
  `--filter build` is unchanged.

- **REVIEW-SINGLE-PROJECTION.** (new-scope) `review-evidence`
  (`omega-rust/omega/packages/review-evidence/`) projects every
  `CheckedCompilation` twice and then validates itself:
  - two projections: the legacy `project_checked_package_review` and
    `project_checked_package_policy`, both covering callables, dangerous
    authority, providers and terminal permissions;
  - `src/capture/package/validation.rs::validate_review_compilation`
    re-derives operator-realization contracts and entailment stand-downs and
    compares;
  - `validate_canonical_structure` runs after each component capture and
    again inside every encoder;
  - `src/capture/providers/policy/replay.rs::validate` replays the plans;
  - `src/ledger/results/reconstruction.rs` re-checks discharge certificates
    the same compile produced
    (`recheck_contract_entailment_assumption_discharge`);
  - the obligation ledger and its results have no consumer: the
    package-manager `review/reconstruction/` bijection that read them is
    deleted.

  Keep the policy projection: it feeds lock rows and review documents. In one
  commit, delete the legacy projection, the self-validation calls, the
  discharge re-check and the ledger, and re-base `whole_review_commitment` and
  the review cache on the policy projection. Then delete what only these
  consumers read in
  `omega-rust/psi/compilation/pipeline/04_typed-trees-to-checked-trees/src/`:
  `checks/contracts/exit_membership.rs` (caller
  `src/capture/contracts/stand_downs.rs`),
  `recheck_contract_entailment_assumption_discharge`,
  `facts/canonical_encoding.rs`, the byte `callable_shape` and
  substitutable-contract identities in `facts/contract_plan_facts.rs` and
  `facts/crash_plan_facts.rs`, and the review-only exports of
  `package_review.rs` (keep `derive_pre_flow_value_origins`,
  `infer_machine_termination_summary` and `typed_build_provider_selection`
  for compile-time callers). Acceptance:
  `project_checked_package_review` and the ledger module are gone; review
  command output for the package corpus is unchanged apart from removed
  legacy-only rows, which are listed in the commit.

- **REVIEW-BOUND-PASS-RECHECKS.** (new-scope) Package review's bound pass
  (`omega-rust/omega/packages/package-manager/src/review/candidate/compilation.rs`)
  recompiles every occurrence whose inputs the discovered semantic bindings
  touch and checks typed programs the discovery pass already checked; the
  exact-input memo used to serve those repeats and is gone. On macOS ARM64
  (debug build), `OMEGA_REVIEW_TIMINGS=1 omega update --project <copy of
  samples/cli/basics/cli_mvp> --target macos_arm64 --offline` reports
  `occurrences=2 compiled=2 settled_checks=4` for both the discovery and the
  bound pass (`carried=0`), and the update takes 280 s where the memo made it
  157 s. Bindings choose providers after the language judgment; let the bound
  pass keep an occurrence's checked trees when the bindings leave its typed
  program unchanged and redo only the binding-dependent settlement.
  Acceptance: the bound pass reports `settled_checks=0` for an occurrence
  whose typed program is unchanged, and review output for that command is
  unchanged.

- **REVIEW-CACHE-PLAIN-DECODE.** (new-scope) Depends on
  **REVIEW-SINGLE-PROJECTION**.
  `omega-rust/omega/packages/package-manager/src/review/candidate/review_cache.rs::into_review_and_facts`
  decodes every row, recovers the projection, `recover_canonical`s the policy,
  requires byte-identical re-encoding, re-encodes the ledger, and re-derives
  the source-consumption commitment. The SHA-256 trailer already rejects
  corruption, and a writer to the local cache could equally write the lock.
  Decode once, with no re-encode comparison. Stop lowering each fresh
  dependency to Terminal (`publish_checked_component`) just to store a cache
  description. Acceptance: no re-encode or re-derive on a cache hit; corpus
  `--filter packages` is unchanged.

- **ADMISSIONS-FILE-DELETED.** (new-scope) Delete the `omega.admissions`
  file and the `trust-ledger` crate. Owner ruling 2026-10-03: `omega.lock` is
  the one acceptance record; a second admissions file was never sanctioned
  (`wiki/spec/packages/locks.md` already says accepted policy needs "no second
  native acceptance channel"). The lock already retains admission-claim and
  external-realization rows (`omega-rust/omega/packages/package-manager/src/lock/README.md`).
  - Delete `omega-rust/omega/application/trust-ledger/` and its callers:
    `omega-rust/omega/application/cli/src/compilation/mod.rs`
    (`read_trust_admissions`, `accept_trust_admissions`) and
    `omega-rust/omega/application/cli/src/execution/compilation.rs`.
  - Remove the `accepted_trust_admissions` request field and
    `with_accepted_trust_admissions` from
    `omega-rust/omega/compilation/products/src/compile_request.rs`,
    `omega-rust/omega/packages/package-manager/src/operations/check_project.rs`
    and `.../operations/compile_project.rs`. Root grants that need acceptance
    are accepted through the lock's existing rows; a project with no lock
    keeps the guide's development behavior (own-package boundary machines
    active with a standing warning, dependency ones inert until accepted).
  - `omega-rust/omega/compilation/realization/trust-model/src/admission_settlement.rs`
    `reconstruct_trust_settlement` compares obligations against the lock
    rows, not a ledger; with **TRUST-REPORT-DELETED**, nothing else in
    `trust-model` reads the ledger.
  - Drop the `omega.admissions` exclusion in
    `omega-rust/omega/packages/source-acquisition/src/tree/capture/traversal.rs`
    and every mention in `wiki/spec/packages/locks.md`,
    `wiki/spec/packages/resolver_security.md`, `omega-rust/pipeline.md`, the
    application, CLI, source-acquisition and trust-model READMEs, and
    `tools/benchmark/` (`benchmark.py` and README).

  Acceptance: `rg omega.admissions` finds nothing outside history notes; no
  crate depends on `trust-ledger`; a package fixture whose root grants a
  dependency's boundary claim compiles once that claim is accepted in
  `omega.lock` and rejects with a review instruction when it is not.

  Unlocked native compilation uses the ordinary checked package route without
  inventing acceptance. On macOS ARM64, `omega run
  tests/omega/pass/calls/machine_self_call_recursion_compile/main.omg` exits 0
  without a lock; `python3 tools/tests/test_development_run.py --omega <binary>`
  covers native compile/run, current and invalid locks, stale source, missing
  target sections and an unreviewed dependency binding. The latter still
  prevents unlocked `pass/text/runtime_number_to_decimal_exit` from running:
  its package-owned physical entry contract needs explicit acceptance. The
  admissions-file deletion and lock-row settlement above remain unfinished.

  The end-to-end reproducer is
  `tests/omega/pending/proofs/accepted_dependency_claim_exit` (macOS ARM64,
  `9952e21837a`); move it into `pass/` with an accepting `omega.lock`. Its
  foreign axiom establishes the entry's postcondition. The current root grant
  is rejected by `selected-dispatch/src/provider_checking.rs` before package
  review can settle it. Diagnostic removal of the grant reaches the separate
  stage-04 execution-planning refusal: boundary `establish` has no boundary
  plan. Preserve the citation: this needs an admitted fact-only invocation,
  not an invented executable provider. The static `Build::accept_boundary`
  spelling also still rejects at generic-callee resolution; coordinate with
  **STATIC-MACHINE-MIGRATION** and the execution-planning owner. These are
  implementation dependencies, not unanswered language decisions.

- **BEHAVIOR-EXCLUSIONS-CHECKED-ONCE.** (new-scope) An authored exclusion is
  checked by `trust_model::verify_entry_behavior_exclusions` on the
  unoptimized module, again per callback thunk, three more times inside
  proposal validation, and again in the physical check. Service exclusions
  re-walk Terminal for boundary reach that Psi's entry reach summary already
  answers. Within one compile, check each exclusion once, on the module that
  is realized, and read service reach from Psi. Retained or received artifacts
  are checked by their receiver (**BUILD-EXCLUSION-REALIZATION**), which must
  not add in-process re-checks. Acceptance: one exclusion check per realized
  entry, and the `tests/omega/{pass,fail}/build/*exclusion*` fixtures keep
  their verdicts.

## Red tests

Rust tests cover only units with no Omega source input; language behavior is
the corpus gate's job ([testing](tools/testing.md#running-one-test)). Board
items that cite a deleted Rust test as acceptance mean the corresponding
corpus fixture, or a new one.

- **RED-WINDOWS-HOST.** (new-scope) Two `package-source` tests
  (`omega-rust/omega/packages/source-acquisition/`) fail on Windows x86-64:
  - `local::recovery::tests::byte_identical_content_does_not_cross_local_origins`
    (`src/local/recovery/tests.rs`) recovers a cached local source across
    origins where it should reject;
  - `tree::filesystem::read_tests::missing_file_identity_rejects_without_panicking`
    (`src/tree/filesystem/read_tests.rs`) does not return
    `LocalSourceChanged`.

  Confirm each one on macOS, then fix the Windows behavior or gate the test by
  platform. Acceptance: `mbx nextest run -p package-source --no-fail-fast`
  reports zero failures on Windows and macOS.

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

Each item lands alone with its corpus slice unchanged, except for intended
deletions, which its commit lists.

- **CHECKER-DISCHARGES-OPERATION-OBLIGATIONS.** (new-scope) Checking must
  discharge every primitive operation condition and lowering must consume
  those decisions; today PCC machinery is the compile-time soundness gate.
  Stage 05 reconstructs each Exact operation's obligation through the verifier
  (`omega-rust/psi/compilation/pipeline/05_checked-trees-to-lowered-psi/src/proofs/operation_proofs.rs`,
  `validate_module` + `reconstruct_execution_terminal_obligations`) and
  searches for a certificate (`src/proofs/nonzero_divisor_certificate/`);
  when none is found, `OperationProofUnavailable` fails the compile. This
  lands first.

  Conditions in scope: nonzero divisor, Exact representability, shift counts,
  Boolean landing, byte/element indices and ranges, representation-region
  geometry, capacities and bounded field stores. Source checking already
  decides most of these (Exact division/remainder definedness from intervals,
  unequal relations, scalar predicates and integer constraints; stored integer
  writes via `CheckFacts::stored_integer_writes`; shift counts, overflow,
  index/range bounds, recast geometry, domain field writes, Boolean recast
  admission). Remaining work:
  - retain each successful occurrence decision in `CheckFacts` and have stage
    05 consume it without proof search; do not add duplicate provers. Arithmetic
    expression analysis now returns exact-cast decisions from its branch-local
    walk; extend that output to the remaining primitive conditions, rather than
    rebuilding decisions with a separate syntax traversal. Concrete contract
    arithmetic uses that same formation walk; abstract declaration contracts
    still have a separate carrier-resolution path to unify;
  - cover nested expressions, compiler-generated semantic-to-storage casts,
    assignment targets, abstract proposition carrier resolution, and the
    formation audit across aggregate and proposition positions before
    claiming full traversal;
  - replace conditional cleanup proof search before deleting any late gate;
  - preserve policy distinctions: non-Exact division may still owe a nonzero
    divisor; keep call contracts, cleanup prerequisites and mathematical
    proofs distinct from primitive operation conditions; opaque operators
    cannot substitute a final value contract for evidence about every
    intermediate write; public layout literals remain separate unsupported
    syntax.
  - Encoded-storage references:
    `tests/omega/pass/layouts/runtime_callee_integer_at_fitting_write_exit`
    (helpers write -128 and 127 through a legal `&mut i64` loan to signed
    eight-bit storage; neighboring byte 85 must survive) fails in stage 05
    call emission: stage 04 plans `set(leaf, -128)`'s argument as a
    `StructuralLocal` naming `leaf`, which has no storage home because it is
    an exclusive place loan derived from the recast `view`, so
    `attached_unit/parameters.rs` refuses "Unit structural argument is
    neither a parameter nor a supported literal". Reference formation
    and transport must carry the actual backing address together with its
    canonical storage encoding, correlated through calls, joins, returns,
    recursion and reference-bearing records (specialization alone cannot
    handle a runtime choice of encodings). Extend the shared reference
    representation, its internal ABI, native accesses, the checked
    interpreter and the terminal interpreter consistently, preserving logical
    carrier type and permissions. A raw carrier-width pointer or delayed
    copy-out cannot establish this contract; foreign raw-pointer boundaries
    need a compatible checked adapter.
  - `tests/omega/pass/arithmetic/runtime_nested_record_observation_exit`: the
    interpreter exits 70, but the second division still hits
    `OperationProofUnavailable` despite a retained nonzero requirement and
    divisor alias. Do not extend certificate search for this arrangement.

  Acceptance: every corpus fail fixture that rejects with
  `OperationProofUnavailable` rejects from `omega --check` with a checking
  diagnostic, and the check corpus is otherwise unchanged;
  `python tools/corpus_gate.py --both --filter
  arithmetic/runtime_nested_record_observation_exit` and `--filter
  layouts/runtime_callee_integer_at_fitting_write_exit` exit 70 and 0
  respectively in native and interpreter.

- **PCC-CERTIFICATES-DELETED.** (new-scope) Depends on
  **CHECKER-DISCHARGES-OPERATION-OBLIGATIONS**. Delete certificate production
  and transport in one commit:
  - stage 05
    `omega-rust/psi/compilation/pipeline/05_checked-trees-to-lowered-psi/src/proofs/`:
    `nonzero_divisor_certificate`, `operation_proofs`,
    `scalar_block_invariants`, `scalar_result_guarantees`,
    `entry_requirement_certificates`, `proof_recursion`,
    `control_cycle_proofs`, `retained_evidence`, `quotient_correspondence`,
    `float_meaning_projection` and `mathematical_declarations`, plus the
    certificate half of `evidence_lowering`;
  - `proof-admission`
    `omega-rust/psi/compilation/proof/admission/src/certificate_search/`;
  - `ProofBundle` on `lowered-psi`;
  - stage 06's `reconstruct_optimizable_terminal_obligations` and
    `transport_*` evidence calls.

  Keep the IR lowering in `proofs/crash_routes`, `proofs/contract_predicates`,
  `proofs/integer_predicates`, `proofs/content_conservation` and
  `src/retention/`. Delete `src/retention/operation_crash_contracts.rs` if a
  search finds no backend reader. Stage 06 rewrites keep their own validity
  checks. Acceptance: no `ProofBundle` is produced, and the check and native
  corpus are unchanged.

- **OMEGA-READS-TERMINAL-UNVERIFIED.** (new-scope) Depends on
  **PCC-CERTIFICATES-DELETED**. Stop every consumer from verifying Terminal:
  - stage 00
    `omega-rust/omega/compilation/pipeline/00_terminal-psi-to-abstract-operations/src/artifact_admission/native.rs`
    `verify_module`: decode only;
  - stage 00 `src/provider_installation/admission.rs`;
  - the terminal interpreter
    (`omega-rust/psi/interpreter/terminal-interpreter/src/terminal_interpreter.rs`,
    `src/program.rs`);
  - `component-description`
    (`omega-rust/omega/compilation/backend/artifacts/component-description/`);
  - `omega-rust/psi/compilation/representations/terminal-psi/fuel/certification/`,
    which computes fixed-fuel bounds from `TerminalModule`;
  - `validate_module_representation` on codec decode
    (`omega-rust/psi/compilation/representations/terminal-psi/codec/src/sections/semantic_module.rs`;
    encode no longer runs it).

  Re-source the optimizer's facts:
  - `AcceptedObligationFact`
    (`omega-rust/omega/compilation/representations/abstract-operations/optimization-unit/src/optimization_unit/proof.rs`)
    and
    `omega-rust/omega/compilation/pipeline/01_abstract-operations-to-abstract-operations/src/rules/proof_check_elision/`
    read "an Exact operation's canonical goal holds", which
    **CHECKER-DISCHARGES-OPERATION-OBLIGATIONS** makes true;
  - move `substitute_crash_routes` into `terminal-psi`;
  - move `control_cycle_members`/`identity` and `control_graph` into
    `terminal-psi` semantics;
  - move the structural ownership frontier reconstruction
    (`omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/frontier.rs`
    and `frontier/`) into an Omega analysis owned by stage 01;
  - move `validate_boundary_crash_outcome` into the interpreter.

  Acceptance: `terminal-verifier`
  has no dependent outside its own tests, and the check corpus and native plus
  interpreter (`--both`) corpus are unchanged.

- **TERMINAL-VERIFIER-DELETED.** (new-scope) Depends on
  **OMEGA-READS-TERMINAL-UNVERIFIED**. Delete the `terminal-verifier` crate
  (`omega-rust/psi/compilation/representations/terminal-psi/verification/`)
  and its tests. This also retires its `trusted_surface` ledger and stale
  `TEST_ONLY_SOURCES` inventory. `trusted-surface-ledger-drift` is superseded
  by this deletion, not a reason to restore the removed verifier test suite.
  Remove `tools/trusted_surface_digests.py` and the landing guard/documentation
  that refers to this retired ledger once its consumers are gone; retain the
  independent mathematics kernel and bootstrap proof obligations. Until removal,
  changes to still-bound production sites require the documented revalidation,
  not permission from their original author. Historical drift is not a blanket
  authorization to refresh hashes without checking the citing entries.
  Acceptance: workspace `mbx build --lib --tests` succeeds, the corpus is
  unchanged, and no maintenance or landing command reads the deleted ledger.

- **PCC-WIRE-DELETED.** (new-scope) Depends on **PCC-CERTIFICATES-DELETED**.
  Delete the proof transport in one commit and bump the codec version:
  - in `omega-rust/psi/compilation/representations/terminal-psi/codec/src/`:
    `sections/proof_bundle/` (including `signature_codec.rs`),
    `sections/proof_sidecar.rs`, `artifact_admission.rs`, `trust_graph.rs`,
    `sections/obligation_ledger.rs`, `sections/terminal_trace_v1_profile/`,
    `publication.rs`, `bin/psi-terminal-publish.rs`, the proof section of
    `canonical_artifact.rs`/`sections/artifact_manifest.rs`, the
    `ProofBundle` in `sections/optimization_execution.rs`, and
    `sections/semantic_module/quotient_correspondence_wire/` and
    `sections/semantic_module/mathematical_certificate_wire/`;
  - `omega-rust/psi/compilation/representations/terminal-psi/src/artifacts/proof_bundle/`:
    move `nodes.rs` and `witnesses.rs`, with the certificate envelope, into
    `proof-admission`; delete crash and recursion evidence;
  - the PCC-only parts of `terminal-psi/src/terminal_module/proof/`:
    `quotient.rs`, `recursion.rs`, `scalar_block_invariants.rs`, and the
    declarations/outputs/values sections;
  - `proof-admission` `src/admission/recursion.rs`,
    `src/admission/normalization.rs` and the PCC half of
    `src/admission/denotation.rs`;
  - the stage 07 `encode_mathematical_signature` transport;
  - `terminal-psi/src/terminal_module/observation/` TraceV1, after checking
    the use in
    `omega-rust/psi/interpreter/terminal-interpreter/src/semantic_value_comparison.rs`.

  Acceptance: the `proof-admission` math tests and the check and native
  corpus are unchanged; no codec section carries proof evidence.

- **PCC-PRODUCTS-DELETED.** (new-scope) Depends on
  **TERMINAL-VERIFIER-DELETED**. Delete the requestable PCC products and
  receiver policy:
  - the `Pcc*` products in `omega-rust/omega/compilation/products/`
    (`src/pcc.rs`);
  - `omega-rust/omega/compilation/backend/artifacts/native-artifact/src/proof.rs`;
  - object-file `decode_proof_section_for`;
  - `omega-rust/omega/application/cli/src/inspection/evidence.rs`;
  - the `builder.pcc.psi` / `builder.pcc.native` build surface;
  - `AdmissionProfile`, threaded through about 20 crates (driver, products,
    package-manager, interpreter, stage 00). Keep `verify_obligation`,
    `Obligation` and `ObligationClass` in `proof-admission`.

  Acceptance: `rg -i "pcc|AdmissionProfile"` finds nothing in `omega-rust`
  outside history notes; the CLI, package and native corpus are unchanged.

- **PCC-DOCS-RETIRED.** (new-scope) Remove PCC from the instructions,
  specification and board text in one commit:
  - AGENTS.md: reword the opening "proof-carrying systems language", and the
    hard rule's "and independent verification" link.
  - `omega-rust/pipeline.md`: delete compositional lowering's "Independent
    verification must reconstruct..." rule and the Terminal verification
    package and admission rows.
  - Delete `wiki/spec/proofs/publication.md` after moving "## Outcomes" to
    `wiki/spec/build/compiler_request.md` and the bootstrap/foreign-producer
    paragraphs to `wiki/spec/proofs/foundation.md`.
  - Reduce `wiki/spec/terminal-psi/verification.md` to the fact-validity
    rules the checker and optimizer use ("Premise availability", "Entry facts
    and crash coverage") under a new name, and delete the rest.
  - Delete `wiki/spec/terminal-psi/encoding.md` "Proof bundle", "Obligation
    ledger" and "PCC proof sidecar", and the canonical artifact proof section.
  - Delete "Shipping independently checkable proofs" in
    `wiki/language_guide/chapter_10_compile_time_proofs.md`,
    "Proof-carrying output" in `wiki/spec/build/configuration.md`, and the
    `builder.pcc.*` lines in
    `wiki/language_guide/chapter_15_modules_imports_visibility.md`.
  - Trim the PCC sentences in `wiki/spec/terminal-psi/product.md`,
    `wiki/spec/proofs/contracts.md`, `wiki/spec/proofs/foundation.md`
    (migration bullets), `wiki/spec/proofs/quotients.md` ("Published quotient
    correspondence"), `wiki/spec/build/permissions.md`,
    `wiki/spec/build/executable_installation.md`,
    `wiki/spec/build/macos_application.md`,
    `wiki/spec/build/component_publication.md`,
    `wiki/spec/build/compiler_request.md`, `wiki/spec/build/optimizations.md`,
    `wiki/language_guide/chapter_19_capabilities_effects_boundaries.md` and
    `wiki/language_guide/packages.md`. Reframe "independent verification"
    wording in the other `wiki/spec/terminal-psi/` docs.
  Acceptance: `rg -i "pcc|proof-carrying|proof bundle|independent(ly)?
  verif"` over `AGENTS.md`, `wiki/`, `omega-rust/pipeline.md` and the boards
  returns only history notes.

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

- **SAME-NAME-REQUIREMENT-CALL-TARGETS.** (new-scope) Replace the remaining
  record-name-matched attachment lookups with the retained attachment identity
  (including closed applications):
  - stage 04 (`omega-rust/psi/compilation/pipeline/04_typed-trees-to-checked-trees/src/`):
    `execution/terminal_unit/calls/signatures.rs`
    (`structural_signature_with_partial_affine`), `operators/receiver.rs`, the
    contained-target filter in `facts/carry.rs`,
    `checks/contracts/evaluator/collections.rs`,
    `monomorphization/body_cloning.rs`;
  - stage 05: `omega-rust/psi/compilation/pipeline/05_checked-trees-to-lowered-psi/src/returns/boundary_scalar_return/source_custody.rs`.

  No current fixture reaches these sites;
  `pass/providers/same_name_requirement{,_right}_dispatch_exit` and
  `pass/modules/same_name_requirement_field_calls_exit` already exit 70.
  Extend a runtime customer through the affected storage, borrow or return path
  before replacing each lookup. Unverified: re-read each site; some may already
  select by attachment symbol.

  Acceptance: a same-name-module pass fixture reaches each replaced site and
  exits with its oracle; a forged target from a different requirement rejects.

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
through source-produced Terminal Psi. Core/std declarations, checked-interpreter
recognition and native `SelectedProcessExit` custody exist. The Terminal
[terminator](omega-rust/psi/compilation/representations/terminal-psi/src/terminal_module/control_flow/termination.rs)
has no external-completion variant, and the
[trace codec](omega-rust/psi/compilation/representations/terminal-psi/codec/src/sections/terminal_trace_v1_profile.rs)
rejects a nonempty external-termination roster. A native nonreturning
operation does not fill this gap.

- Native gaps (need not wait for the Terminal representation):
  - [normalized foreign-call custody](omega-rust/omega/compilation/backend/images/image-emission/src/object_artifact/normalized_foreign_calls.rs)
    accepts scalar homes only from call-result producers; transport a general
    computed status argument;
  - Windows Console output needs `WriteFile` (its `CompilerIntrinsic`
    declaration has no Windows hosted bundle); **WINDOWS-CONSOLE-INPUT**
    owns `ReadFile` input and its interpreter dependency;
    `ProcessExit` already selects kernel32 `ExitProcess`;
  - replace the fixed 256 KiB foreign-call reserve (`SAME_STACK_BYTES`) in
    [source-evaluated bindings](omega-rust/omega/compilation/pipeline/00_terminal-psi-to-abstract-operations/src/provider_admission/source_evaluated_bindings.rs)
    with per-callee stack demand.
- Carry the exact canonical requirement, domain-bound authority, status and a
  closed terminal invocation with no normal result/successor through checking,
  lowering, Terminal, codec and interpreter. Console reach and provider
  selection do not grant exit authority. Keep the semantic `i32` status and
  preceding output independent of the host's status mapping.
- Conditional helpers keep results, cleanup and postconditions on returning
  paths. Exit paths abandon in-domain obligations without discharge receipts.
  Automatic cleanup must return; normal root completion settles outstanding
  task custody. Survivor crossing contracts account for domain death without a
  whole-process resource census.

Acceptance: unconditional and conditional/helper exits run in the interpreter
and natively. Missing authority/reach/progress, lookalikes, returning/aborting
substitutes, cleanup exits and unsupported survivor contracts reject.
`tests/omega/pass/host/process_exit_i32_status_ordered` exits 70 after one
stdout line and `process_exit_i32_status_mapped` presents status -44 as 212
(interpreter keeps -44), matching their `expected_native_exit.<target>.txt`
on macOS ARM64 and on Linux x86-64/AArch64 hosts; on Windows `ordered` still
rejects at build and `mapped` needs a Windows full-i32 measurement.
**TWO-AXIS-TERMINAL-AUTHORITY-REVIEW** owns further receiver admission.

## P1 - Authority, roots, and entry

- **ENTRY-CONTENT-ROOTS.** Provisioned receivers are not yet activated and
  completed end to end under [entry roots](wiki/spec/build/entry_roots.md) and
  [binding validity](wiki/spec/build/component_publication.md#bindings-and-era-entry).
  Reuse the opaque affine `Binding<R>` in `source/library/core/binding.omg`;
  `ForeignBinding` is a separate locator. No authored `Bound` qualification,
  source-created binding or provider lookup may stand in for missing
  establishment.
  - Borrowed-service customers: carry `samples/cli/games/dungeon_crawler_cli`'s
    `&mut Binding<Console>` command flow (frontier owned by
    **NOMINAL-FIELD-FLOW**) to native execution. Make
    `tests/omega/pass/capabilities/uses_caller_folder` (checks only) lend the
    caller's field to `Librarian::archive(folder: &Binding<Folder>)` and
    execute its service effects. Run the original nested binding/helper-return
    chain of `tests/omega/pass/capabilities/acquires_through_helper_return`
    (builds natively; the corpus harness does not execute it) and verify
    propagated acquires/provenance. Keep
    `tests/omega/pass/text/runtime_stdin_line_buffering_exit` (exit 0, exact
    `first\nsecond\n`) as the control.
  - Activation/result/out-parameter joins must preserve the exact closed
    requirement, occurrence and selected plan through erased fields to native
    execution. Reuse `omega-rust/omega/compilation/backend/plans/program-entry-plan/`,
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
    a retained cleanup marker and `ProgramLocalRootInstallationLedger`
    (`external-roots`) exist, but no authored destructor executes. Compose
    application, bridge, provider and callback stack demand, not loader
    stack-size metadata or another storage profile.

  Acceptance: published `samples/cli/basics/{number_guess,cli_mvp,generic_counters}`
  keep their documented behavior without test-supplied receivers or
  qualifications (**SAMPLE-CORPUS** owns full sample coverage); the customers
  above execute natively. Reject missing/incompatible supply, forged zero or
  literal construction, lookalikes, redirected continuations, non-ZII state,
  invalid backing/alignment, overlapping partitions, stale occurrence/epoch,
  bare boundary-trait value formals, and moving an affine binding out of
  borrowed storage without replacement. Forwarding keeps distinct applications
  and affine custody; erasure removes neither initialization nor cleanup.
  Targetless checks select no physical entry; deployment cannot replace the
  physical adapter with a semantic continuation.

- **CONSERVATION-CONTRACT / TERMINAL-CONTENT-CLAIMS.** Nonempty content claims
  do not execute through source, Terminal, selected providers and native code
  under [content conservation](wiki/spec/resources/content_custody.md):
  partition an established input under an authored theorem, pass the exact
  residual to a boundary, retain/return the rest and consume it in the caller,
  for direct calls and checked wrappers using canonical
  `source/library/core/content.omg` identities.
  - `tests/omega/pass/core/content_conservation_contract` invokes
    `partition_and_retain` but rejects natively: its build selects no
    `RegionSplit`/`RegionSink` providers. Supply executable providers, rename it
    to an `_exit` fixture and add `expected_native_exit.txt`. Core supplies
    proof vocabulary, not a splitter: implement the partition primitive with
    its theorem.
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
    requires whole-root content arguments and accepts structural results only
    when affine, without claims or projected qualifications. Complete the
    provider/caller content join for partition results and projected residuals.

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
    schemas and the occurrence/capacity/epoch ledger in
    `omega-rust/omega/compilation/backend/runtime/external-roots/src/program_local/program_local_roots/`;
    wire real entry/provider observations into the installed account.
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
  `Main::main` is empty and `build.omg` declares only a package. Bind an
  admitted backing, invoke the accessors and observe a runtime result.
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
    supplies only a direct-entry pointer ABI for a parameterless one-block Unit
    function without access events; join the selected placement and accessor
    operations into normal native lowering.
  - Native admission and interpretation both reject non-entry roster rows; add
    call-bound custody from the caller. A pointer or roster alone stays
    insufficient.

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
  fixtures for interleaved fields with nested indices, computed selectors,
  field-held loop counters, leaf copies and shared receiver reads
  (`cells[i].get()`), and fail fixtures for invalid indices. Use
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
  Plain scalar and record loans, projected record subloans and literal-indexed
  record subloans work: `write_only/state_forwarded_scalar_store_exit`,
  `borrow/write_only_composed_forward_exit` (exit 70) and
  `borrow/runtime_write_only_record_array_argument_exit` (exit 0) agree
  natively and in the interpreter on macOS ARM64.
  Remaining:
  - general aggregate and `[copy]` sum replacement beyond scalar-field record
    literals, preserving displaced custody and whole-value validity;
  - runtime domain-qualified byte-field replacement with encoding evidence;
  - computed IEEE store/readback (through **FLOAT-PROVIDERS**; the mutable
    controls stop in caller guard planning);
  - computed-index writes and local affine backing through the complete state
    and cleanup route. Checking already admits closed copy sums, and computed
    integer selectors and scalar-computation store sources have plans; reuse
    them.

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
  stage 04 `execution/terminal_scalar/`, `execution/terminal_unit/receiver_calls/`
  and `execution/terminal_unit/calls/computation_arguments/`; stage 02
  `validation/structural_call_arguments.rs`; image-emission `argument_custody`.

  Remaining: local-rooted indexed receivers, owned-root/construction-local
  admission and runtime-indexed write-only receivers. Reuse `Field`,
  `FixedIndex` and `RuntimeIndex { index, obligation }`; coordinate range
  spelling with **CANONICALIZE-SCALAR-RANGE-CONTRACTS**. Declared receiver
  custody stays independent of field observations and state count.

  Controls (keep passing): `calls/unread_multistate_receiver_exit` (native exit
  70 on macOS, Linux x86-64 and Windows without a dummy read),
  `borrows/returned_record_field_read_exit` (70) and
  `borrows/static_record_reference_return_exit` (0) in both engines.
  Unverified: whether `unread_multistate_receiver_exit` agrees in the
  interpreter (no `--both` record).

  Acceptance: corpus fixtures execute caller-visible projected/forwarded
  writes, owned-field mutable/write-only subloans, synchronized shared
  observations and register/stack reference passing on matching hosts
  including Linux AArch64 and Windows; a callee observing staged bytes does not
  prove caller writeback. Fail fixtures reject copied borrowed homes, missing
  dereference, changed access and duplicate overloads.

- **NORMALIZED-ABI-LOWERING.** Borrowed descriptor arguments and native
  indirect results are not transported across foreign calls under the
  [calling-plan contract](wiki/spec/build/calling_plans.md). Owners: Omega
  stages 02 and 03, backend call-frame emission. Owned whole-place arguments
  and register-returned aggregate results execute:
  `providers/foreign_owned_record_exit`, `providers/foreign_record_result_exit`
  and `providers/foreign_multiregister_result_exit` exit 70 on macOS ARM64.
  Foreign imports share one mechanism template per target ABI, derived from
  the evaluated plans.
  - Extend borrowed descriptor transport using evaluated descriptor words,
    without substituting the descriptor's address.
  - Execute indirect (memory-returned) aggregate results natively.

  Acceptance: corpus fixtures pass a borrowed descriptor and receive an
  indirect aggregate result from C with exact values, register/stack placement
  and formal order on each matching host. Terminal Psi stays target-neutral.
  **RESTORE-DYNAMIC-DESCRIPTOR-AND-TABLE-CUSTODY** owns dynamic descriptor
  semantics; **CALLBACK-PRIVATE-MATERIALIZATION** owns private callbacks.

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

- **C2L-PROOF-SEARCH-BLOWUP-CONTAINMENT.** (new-scope) Proof search cost grows
  with scope size and its exhaustion is not reported from checking. Today the
  bounded search and its `ProofSearchCapacityExhausted` -> `Incomplete`
  publication (codes 27-35 in [compiler-request](wiki/spec/build/compiler_request.md))
  live in stage 05's certificate search
  (`omega-rust/psi/compilation/pipeline/05_checked-trees-to-lowered-psi/src/proofs/mod.rs`),
  which **PCC-CERTIFICATES-DELETED** removes; `samples/cli/simulation/cellular_automaton`
  spends most of its compile in that pass, re-proving each roster arrival
  every round under a growing axiom set. Depends on
  **CHECKER-DISCHARGES-OPERATION-OBLIGATIONS**.

  When checking owns the discharge, its search must not re-prove under every
  round's axioms or scale with unrelated scope, and every named provision must
  decline into an `Incomplete` outcome rather than a rejection. Re-scope or
  retire codes 27-35 to match the surviving provisions, and project them into
  `tests/bootstrap/ocreq-tables.sh` / `bootstrap/5_omega` outcome tables.
  [Kernel budgets](wiki/spec/proofs/kernel_metatheory.md) stay implementation
  policy, not language semantics.

  Acceptance: `cellular_automaton` checks in time dominated by ordinary
  checking rather than proof rounds (measure with `OMEGA_PROOF_MEASUREMENTS`);
  a corpus fixture that exceeds a provision reports `Incomplete` (`omega` exit 2,
  `omega run` status 202), distinct from invalid source; invalid proofs still
  reject.

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

  Current frontier (unpublished `repair-theorem` checkpoint `8b9cb8dcc9a`,
  macOS ARM64): `omega --check
  tests/omega/pass/proofs/cauchy_predicates_compile/main.omg` checks 17 source
  files. The proof/proposition/induction/Nat/Rat/contract/relevance corpus
  checks 64 and rejects 81 without diagnostic regressions; a separate abstract
  mathematical-call control rejects reuse of a linear receipt. Exact callable
  substitution, nested field identity and isolated-parameter preservation now
  reach checking. `omega run
  tests/omega/pass/proofs/runtime_static_result_projection_citation_exit/main.omg`
  requires accepted package inputs; `omega update --offline --project
  tests/omega/pass/proofs/runtime_static_result_projection_citation_exit`
  cannot produce them because the theorem call enters runtime structural
  argument planning. `checking/src/machine_calls/fact_only_citations.rs` still
  recognizes only empty nongeneric bodies. Replace that source-shape rule with
  checked termination, effect and custody eligibility, settled once before
  execution planning. Coordinate the citation skip in stage 04's live-owned
  `execution/terminal_unit/control/statement_sequence.rs`; a narrow handoff was
  requested on claim `03f901cb007048ba9c57d569fbdef49c`. This is an integration
  dependency, not an unanswered language decision. Next acceptance: the same
  native witness exits 70, preserving effectful/nonterminating and linear
  ownership refusals; then publish the complete repair.

- **PROOF-RELEVANCE-MIGRATION.** Erased arguments are not carrier-independent
  under [explicit erased bindings](wiki/spec/proofs/contracts.md#explicit-erased-bindings).
  Owners: stage 04 call plans and stage 05 contract/proof-term lowering. Static
  scalar contract arguments and record/sum constructions with
  `ProofTerm::Scalar` leaves work; `pass/relevance/erased_record_parameter_exit`
  covers signature stripping, not a contract consuming its witness. Array-field
  carriers refuse, and dynamic dispatch rejects erased formals in
  `checks/contracts/dynamic_erased_lane.rs`. Extend the existing routes to keep
  exact erased subjects/actuals, witness identity, scope, multiplicity and
  assumptions through static and dynamic calls, without runtime storage or ABI
  arguments. Erased-field custody/cleanup belongs to
  **CLEANUP-HOOK-SELECTION-AND-ERASED-OWNERSHIP**.

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

- **CALLBACK-PRIVATE-MATERIALIZATION.** Private callbacks only realize a single
  scalar-ABI function natively under
  [private callbacks](wiki/spec/build/private_callbacks.md). Psi production and
  Omega's `callback_settlement` validate requirement selection and inbound
  interfaces. Extend native callback bodies, call/import closures and
  signatures beyond `omega-rust/omega/compilation/backend/machine-emission/src/private_callback.rs`
  using ordinary lowering with ABI and call evidence, not body recognizers;
  preserve callback custody through optimization; integrate ordinary product
  publication and the hosted private stack. Owners: `callback_settlement`,
  `omega-rust/omega/compilation/driver/src/native/native_realization/callback_thunks.rs`,
  machine emission, selected-call ABI transport.

  **A resultless callback cannot carry an ABI record at all**, verified
  directly rather than inferred: `ScalarFunctionAbi`
  (`representations/target-operations/src/target_operations/calls/abi.rs:27-31`)
  declares `result: ScalarAbiValue`, not an `Option`, so there is no
  representation for a callback entry that returns nothing. A thunk for a
  resultless procedure therefore refuses at
  `backend/machine-emission/src/private_callback.rs:82` with "callback thunk
  emits no scalar ABI for its boundary entry signature", and both the
  materializer and its signature-drift check read that same mandatory record.
  Extending this needs a resultless entry in the ABI representation, not a void
  branch in the materializer.

  Reached while giving `pass/calls/callback_private_materialization`'s
  `WindowRegistrar::register` a provider; the obvious way around it does not
  work, since declaring the procedure `-> u64` instead fails two stages earlier
  in target lowering with `TranslationValidation(StructuralCallArgumentMismatch)`.
  The provider change that reaches this wall is **not landed and is under
  review** -- it resolves the schema with a `DllImport::PeByName` naming a
  library and export that exist nowhere in the repository, justified as a
  never-called compile-time coordinate, which is a question about what a
  compile-only fixture may assert rather than an implementation detail.

  Acceptance: a Windows x64 corpus program registers a private callback with a
  general body and inbound signature, and the callback executes with the
  authored result. Missing, duplicate, overlapping or incompatible
  materializations reject; private slots stay inaccessible to source; the
  package-local calling-vocabulary duplicate rejection holds. Registration
  outcome and lifetime belong to **REGISTERED-CALLBACK-LIFETIME**.

- **REGISTERED-CALLBACK-LIFETIME.** An authored registrar lifecycle does not
  execute under [registration and lifetime](wiki/spec/build/private_callbacks.md#registration-and-lifetime)
  and [opaque retention](wiki/spec/build/component_publication.md#opaque-retention-and-quarantine).
  Carry the success/rejection sum through checked result planning, Terminal
  case-conditional qualifications/claims and installed-provider
  interpretation: `omega-rust/psi/interpreter/terminal-interpreter/src/calls.rs`
  rejects projected result qualifications and affine results carrying
  claims/qualifications. A valid roster must come from authored evidence, not
  removal of the gate (reuse interpreter `live_claims`). Join success to the
  live-registration capacity, external root and code/component leases through
  `omega-rust/omega/compilation/backend/runtime/component-publication/src/callback_registration.rs`.
  Rejection creates no root and keeps capacity; unregister and quiescence
  precede lease release and return the same capacity occurrence; capacity
  counts live registrations, not thunks.

  Acceptance: an authored provider/customer rejects, retries, replaces with
  returned capacity and unregisters; dropped, repeated, stale or
  cross-occurrence registrations, spent-capacity reuse and wrong qualification
  rosters reject. **CALLBACK-PRIVATE-MATERIALIZATION** supplies the native
  entry; **FFIVAL** owns Windows foreign invocation (unavailable elsewhere).

- **FOREIGN-RETAINED-ARGUMENT-BACKING.** Authored retention is not connected to
  native argument backing under
  [outbound custody](wiki/spec/build/foreign_storage.md#outbound-custody).
  Owners: checked content/call planning, stage 05
  `retention/retained_borrow_custody.rs`, native marshaling and
  `omega-rust/omega/compilation/backend/runtime/external-roots/src/program_local/program_local_extents/retained_foreign_arguments.rs`,
  whose `retain_foreign_argument_under_custody` has only test callers. Complete
  the source-to-native connection including installed-provider custody for
  retained-borrow guarantees. Materialize pointers only from exact stable-root,
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
  Registration leases belong to **REGISTERED-CALLBACK-LIFETIME**.

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
  - range-qualified scalar and erased-field `[copy]` view elements under
    [ownership](wiki/spec/language/ownership.md); record copies already work
    (`slices/runtime_qualified_record_copy_local_exit`, exit 70).
    Unverified: which range-qualified/erased-field cases still fail.
  - consuming mutable-view forwarding on the Unit-graph route with exact
    reborrow places at successors;
  - `omega-rust/psi/interpreter/terminal-interpreter/src/element_views.rs`
    stores only scalar elements and rejects record paths.

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
ordinary satisfaction/entry checking, Terminal Psi execution and embedding
custody; no test attribute, special machine name, calling permission or
parallel compiler.

- **BUILD-TEST-GROUPS.** (new-scope) Build has no test registration:
  `Build` in `omega-rust/omega/packages/build-declarations/src/prelude.rs` has
  no `tests` facet, and `EvaluatedBuildSettings` in
  `omega-rust/omega/packages/compilation-inputs/src/build_execution/build_value/configuration.rs`
  retains no groups. Build evaluation must carry
  `builder.tests.group<ExactRequirement>()` registration and enablement;
  compiler orchestration (`omega-rust/omega/compilation/driver/`) owns
  package-local satisfaction discovery, separate runner roots, outcomes and
  publication. Keep exact product-reference identity, private scope and
  source/target provenance; never discover by strings or infer generic
  applications.

  Acceptance: an ordinary build discovers two tests without per-test
  registration; a failing mandatory check prevents publication; disabling its
  group reports not-run; a second requirement in the same trait stays
  distinct. Cover duplicate/inherited identities, non-runnable signatures,
  unresolved generics, dependency non-discovery and exhaustion distinct from
  failure. Runs without std; an optional std testing requirement needs no
  compiler recognition. Application Psi/native output excludes test-only roots;
  a native harness may explicitly call visible test machines. Receiver/service
  execution rejects until **BUILD-TEST-AUTHORITY**.

- **BUILD-TEST-AUTHORITY.** (split-of:BUILD-TEST-GROUPS) Add group-scoped
  providers, provisioned receivers and fresh mock state. Extend the restricted
  requests in `omega-rust/omega/compilation/build-evaluation/src/admitted_build_program.rs`
  and `omega-rust/omega/packages/package-manager/src/review/restricted_build_grants.rs`
  (which join build-host requests to lock acceptance) to carry test/group,
  fixture and provider attribution before execution. Keep policy out of
  constant-evaluator admission and use the existing review flow.

  Acceptance: two tests receive fresh virtual filesystems and established
  `Binding` fields; missing bindings reject. Child filesystem/network/process
  requests cannot widen root grants or escape virtual backing. Install/update
  surfaces new test requests; locked builds cannot approve them; acceptance
  without an executor grant rejects. Cover provider/target mismatches,
  disabled-to-enabled rechecking, unsupported execution without native
  fallback, and crash containment without invented unwind. Automatic tests
  reject assumed-binding execution, including through fixtures, helpers and
  providers. Mock configuration leaves production providers and roots
  unchanged.

## Embedding and interpreted components

Implement the [embedding contract](wiki/spec/build/embedding.md) as an ordinary
library, starting with one source-to-host customer. Generic embedding does not
wait for Cathedral's instruction coverage. The interpreter is
`omega-rust/psi/interpreter/terminal-interpreter/`.

- **EMBEDDING-SOURCE-TO-HOST.** (new-scope) Deliver compile -> load -> bind ->
  invoke -> host result -> guest result -> close from an authored interface
  package, host implementation and script, connecting public entry
  descriptions, the interpreter and checked host adapters through `Binding<R>`.
  `src/lib.rs` exposes artifact execution; reusable public programs,
  reclaimable instances, import-slot bindings and close are missing.
  - Share immutable admitted programs across instances; two same-trait slots
    address different host objects.
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
  waiting/completion protocol, and raw effect sinks let a handler be
  substituted on resume: pin admitted bindings and keep custody across pauses.
  `RunnableComponentEraLedger` in
  `omega-rust/omega/compilation/backend/runtime/component-publication/` has
  registration/teardown; runtime lifecycle integration is missing.

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
  import/export and era contracts in
  `omega-rust/omega/compilation/backend/runtime/component-publication/`; no
  second module manager. Root/nested build composition produces the
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
  to actual arrival and scoped providers through
  `omega-rust/omega/compilation/backend/runtime/external-roots/src/platform_bringup/uefi_bootstrap/`.
  Exact entry schema selection is split into
  **ENTRY-SLOT-EXACT-SCHEMA-SELECTION**; consume that selected identity here.
  Needs a source customer and a firmware or controlled-provider execution
  harness. Repair general capabilities in their owners; no firmware-protocol
  intrinsics.

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
  - Keep allocation/status/retry policy in target code and delete the Rust
    sequencing in `uefi_bootstrap/os_handoff_cycle.rs` once covered.

  Acceptance: emitted code under firmware or a controlled provider covers
  growth, stale-key retry, bounded exhaustion with the target error, rejected
  allocation/release, malformed maps, stale/foreign evidence, lost custody and
  forbidden Boot Services use after exit; success enters the selected OS entry
  without returning. A bodyless provider, multi-call intrinsic or fallible
  cleanup destructor cannot replace the cycle.

- **AP-BRINGUP.** Execute Cathedral-owned secondary-processor startup under
  [external roots](wiki/spec/build/external_roots.md#secondary-processor-startup).
  Cathedral owns discovery, dispatch, acknowledgement, retry and cancellation;
  the compiler checks installed entry, placement and resource custody.
  `tests/omega/pass/memory/secondary_processor_canary` declares entry,
  acknowledgement and roster but has an empty `Main::main`. Connect it to the
  trampoline in
  `omega-rust/omega/compilation/backend/machine-emission/src/startup_trampoline.rs`,
  the ledger in
  `omega-rust/omega/compilation/backend/runtime/external-roots/src/platform_bringup/secondary_processor.rs`
  and provider receipt ingress through **BOUNDARY-ISSUANCE**.

  Acceptance: on a matching platform or emulator, the emitted trampoline enters
  the installed entry with dedicated, nonoverlapping stack/state, bound to the
  provider profile, arrival regime and vector/low-memory geometry. Definite
  nondispatch permits withdrawal; unconfirmed dispatch holds resources and
  admits a later exact arrival; cancellation needs evidence of no future
  arrival; retirement needs quiescence; timeout releases nothing. Exercise
  timeout/late arrival, cancellation before confirmation, races,
  foreign/stale/replayed receipts and resource conflicts. No compiler-owned
  APIC driver.

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

  Check-leg status before any of this backend work is reachable: the fixture
  rejected in 6ms on a bundled-`std` import spelling until `cf06f90068e`, then
  on `DescriptorTableValidation::run`'s `terminates by fuel` ranking until
  `0b98b2fc46a`. It now stops at "direct call `complete` names public boundary
  requirement `InterruptAcknowledgement::complete`, which has no selected
  provider". **STATIC-BOUNDARY-REQUIREMENT-CALL-FORM** owns the missing
  implementation/selection repair under ordinary domain discipline. No
  `satisfies` row for this requirement was present at the recorded failure.
  Retain the argument's checked domain evidence and exact installation
  relationships; provider selection remains a separate implementation duty.
  Resolve that compiler/source dependency before the backend joins below.
  - Join the stub and member-call relocation from
    `omega-rust/omega/compilation/backend/machine-emission/src/entry_exit_stub.rs`
    to image placement, installation and external-root admission;
    `X86_64DeriverStubEntryEmission` has no production producer. Bind entry/body
    coordinates, bytes, stack overhead and final state, keeping stack, fuel and
    state separate.
  - Publish the authored table through the package validator and generic
    writer, and move the remaining gate/IST policy out of `InterruptTableGateDescriptor`
    and `InterruptTableProfile` in
    `omega-rust/omega/compilation/backend/runtime/external-roots/src/interrupts/interrupt_table/gate_descriptors.rs`,
    keeping the installed-root, publication-authority and checked-`lidt` joins.
  - Implement fatal halt, timer/controller setup, tick recording and wake.
    **BOUNDED-INSTALLATION-REACH-ROWS** / **TOP-LEVEL-BOUNDARY-REQUIREMENTS** own
    acknowledgement lineage; integrate **UEFI-PHYSICAL-SEMANTIC-ENTRY** /
    **UEFI-OS-HANDOFF** and **TR3-TR8** provisioning with Cathedral's QEMU
    harness. Keep interrupts disabled until the exception floor exists.

  Acceptance: QEMU reports ticks through owned output, halts between ticks and
  routes a deliberate fault to its dedicated-stack fatal handler without
  resuming ordinary work. Missing members, wrong writer/realization, changed
  bytes, replayed publication/acknowledgement, forgotten/double completion and
  retirement while the table names a root reject.

- **BOUNDED-INSTALLATION-REACH-ROWS.** Complete
  [installation-bound reach](wiki/spec/build/external_roots.md#installation-bound-reach)
  for the shipped acknowledgement in `source/library/core/interrupt.omg`:
  `InterruptAcknowledgement::complete` still declares `reaches PortIo`; migrate
  it to `reaches <= MachineControl + PortIo` with a selected satisfier that
  consumes the linear receiver. **TOP-LEVEL-BOUNDARY-REQUIREMENTS** owns
  selected dispatch/discharge. Reuse
  `omega-rust/omega/compilation/realization/provider-planning/src/provider_planning/installation_reach.rs`
  and `omega-rust/omega/compilation/backend/runtime/external-roots/src/root_entry/root_validation.rs`;
  `external-roots/src/tests/interrupt_entries.rs` checks completion
  coordinates from constructed facts, not the shipped carrier. Component
  deployment stays with **COMPONENT-SUBSTRATE**.

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
  - Remaining defect: duplicate conversion/weakening reports and cascades after
    a primary type, suffix or unresolved-name error. Coordinate
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
  conversion error. The shared carrier check in
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
    dynamic indexes, ranges, computed roots, join-place reads/projections,
    primitive-local origins and borrowed-subject successor lifecycle. End loans
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
    declaration-local identities. Package review projects exact call arguments
    but refuses `Match`; extend the canonical projection.

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

  - Call-containing scalar requirements: `covered_requires` in
    `omega-rust/psi/compilation/pipeline/05_checked-trees-to-lowered-psi/src/scalar_graph/scalar_contracts.rs`
    and `omega-rust/psi/compilation/pipeline/05_checked-trees-to-lowered-psi/src/scalar_graph/scalar_graph_lowering/graph_preparation.rs`
    refuses a clause such as `requires observe(left) == observe(right)` on a
    call `restricted(saved, value)`. Lower it with interpreter/native agreement
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
  - Extend `omega-rust/psi/compilation/pipeline/04_typed-trees-to-checked-trees/src/operators/call_binding.rs`
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
    `monomorphization::refresh_closed_domain_instance_identities`. Missing,
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
    content record before build execution; keep supplied package/core owners;
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
    nested record/array/case and whole-aggregate subjects, open applications and
    abstract-carrier `self` constraints. Keep alias-owned bounds/index tuples
    through `omega-rust/psi/compilation/checking/src/proof_contracts/domains.rs`.
    Computed payloads cannot establish qualifications; routed constraints need
    establishment even for unused initializers.
  - Constant invocations share **CRASH-CONTRACT** / **OPERATOR-MACHINE-SUPPLY**
    premise transport through
    `omega-rust/psi/compilation/pipeline/04_typed-trees-to-checked-trees/src/facts/crash_entry_values.rs`.
    Beyond lossless widening and proven indices, extend only with exact live
    evidence; value-changing casts, call-produced projections and unknown
    origins keep conservative crash ceilings.

  Acceptance: `tests/omega/pass/modules/{qualified_declarations,qualified_constants,match_constant_indices,nominal_constant_bodies,module_array_constant_indices,runtime_foreign_generic_scoped_constant_exit,runtime_qualified_static_mutation_exit}`
  and `python tools/corpus_gate.py --filter modules/` stay green. Anonymous
  owners: one std owner in a diamond; distinct root/core/library owners; stable
  identity after relocation, rebuild and cache deletion; changed semantic inputs
  change identity; equal-content named lineages stay distinct; missing or
  mismatched provenance and real visible-name conflicts reject. Add fixtures for
  the new constant forms preserving exact float bits/signed zero, field/index
  projections and operator widths/policies, all-arm obligations without
  evaluating skipped subjects, and the stale-operand, wrong-owner,
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
  [static type equality](wiki/spec/language/generics.md#static-type-equality),
  [structural equations](wiki/spec/language/generics.md#structural-type-equations-and-inference)
  and [canonical domain indices](wiki/spec/language/generics.md#canonical-domain-index-matching)
  through specialization, layout and execution. The running customer is
  `tests/omega/pass/generics/omitted_data_binder_range_equation` (infers
  Capacity = 256 from `u64::AtMost<256>`, exit 92 native and interpreted);
  extend from it. Reuse
  `omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/preparation/type_equations/`
  and `.../preparation/machine_equations.rs`; array, anonymous reference/slice
  and integer/Boolean application matching exist.

  - Combine equations with argument/result inference and keep obligations
    through forwarding, retained extensions, late-selected receivers,
    operator/conformance supply and machine/evidence/value binders. Machine
    preparation still requires closed explicit calls; keep unsolved-equation
    rejection until each route discharges them. Scalar return-domain inference
    from a typed parameter is separate.
  - Domain matching by exact package/declaration and generic-carrier identity:
    `DomainHead` joins `u64::AtMost<256>` with `u64 in AtMost<256>`, but its
    local head lookup declines generic carriers. Add named-lifetime identity
    and remaining const-index kinds; **WRITE-ONLY-BORROW** owns write-only
    admission.
  - Check unspecialized machine/requirement bodies under every admitted
    type-equality alternative, including finite disjunctions and branch-local
    equalities that cannot escape joins.
  - Extend the typed const-expression and invocation evaluators to remaining
    equation/index combinations, keeping carrier, policy, selected operation and
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
  operation; no handwritten suffix methods.

  - Runtime roster membership: `validate_dynamic_family_call_tuple`
    (`omega-rust/psi/compilation/checking/src/declarations/traits/dynamic.rs`)
    and `dynamic_family_tuple`
    (`omega-rust/psi/compilation/pipeline/04_typed-trees-to-checked-trees/src/execution/terminal_unit/dynamic_scalar_calls/realization_callables.rs`)
    require closed static tuples. Admit proved membership reusing
    **RUNTIME-VALUE-GENERICS**' scalar argument/guard machinery (not general
    value-indexed data). Extend the self-only, Unit/i32/bool callable subset to
    scanner inputs/results. Unproved membership rejects; no JIT or fallback.
  - Native dispatch through **RESTORE-DYNAMIC-DESCRIPTOR-AND-TABLE-CUSTODY**'s
    indirect-call mechanism: rebound `CallDynamicScalar` lacks common-graph
    legalization, and selected construction rejects `DynamicParameterCall`.
    Tuple tables must be source-produced.

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
  identity stay. No synthesized receiver-policy file.

- **FILESYSTEM-PLATFORM-SERVICES.** (new-scope) Implement the
  [platform services](wiki/spec/build/platform_services.md) shape for the
  filesystem (owner ruling 2026-10-04). Today `source/library/std/filesystem_host.omg`
  declares one `FilesystemHost` trait mixing POSIX and Windows-only operations
  (`close_handle` takes a Windows `HANDLE`; `find_next` uses Windows
  enumeration), so no provider covers it: macOS has none, and
  `omega-rust/omega/compilation/realization/provider-planning/src/provider_settlement/canonical_filesystem_host.rs`
  supplies partial Linux x86-64 rows.
  - Split it into `PosixFilesystemHost` and `WindowsFilesystemHost` with
    per-family handle types, and bind a complete checked-adapter provider
    per target (macOS and Linux over `PosixFilesystemHost`, Windows over
    `WindowsFilesystemHost`), as the macOS `TimeHost` provider does.
  - Rewrite `source/library/std/targets/<target>/filesystem_impl.omg` to the
    ZII portable shape: `File` stores descriptor + 1 on POSIX and the `HANDLE`
    on Windows, zero `OpenOptions` means defaults, results fail closed, and an
    operation a target lacks returns `ErrorKind::Unsupported`.
  - Delete the partial compiler fallback `canonical_filesystem_host.rs` and
    the mixed trait. Update `samples/cli/systems/file_journal/main.omg`,
    which uses the raw service directly, to the family trait for its targets.

  Frontier measured at `72a43a5bee1`: the split is a hard prerequisite for any
  macOS provider, and the portable wrapper half of it is blocked on
  **Q2** (`portable-service-per-family-binding-carrier`). The trait declares
  50 machines; 12 are Windows-only (`find_first`, `find_next`, `find_close`,
  `create_hard_link`, `open_path_handle`, `close_handle`, `get_osfhandle`,
  `final_path_name_by_handle`, `set_file_time`, `lock_file_ex`, `unlock_file`,
  `get_last_error`), 11 are POSIX-only (`open_at`, `unlink_at`, `read_dir`,
  `set_file_permissions`, `set_file_times`, `sync_data`, `duplicate`,
  `lock_file`, `change_owner`, `change_owner_no_follow`, `change_file_owner`),
  and `errno` is libc-backed-target-only — bound on Darwin (`___error`) and
  Windows (`_errno`), unbound on direct-Linux-syscall targets, which carry
  `-errno` in the result instead. The remaining 26 are portable by name over
  POSIX `fd: i32` handles. A provider must bind every schema method exactly
  once (`covers_schema`,
  `omega-rust/omega/compilation/backend/plans/provider-evidence/src/provider_evidence/providers/provider_plan.rs:881`)
  against the whole trait, because `service_schema::from_typed` collects every
  trait method with no demand filtering — so no honest macOS provider exists
  while the Windows family stays in the trait, and sentinel rows are exactly
  what the spec forbids.

  Call-site confinement is better than the item assumed. All 35 Windows-only
  call sites sit in `targets/windows_x86_64/filesystem_impl.omg` plus exactly
  one fixture, `tests/omega/pass/filesystem/windows_set_file_time_exit`; the
  portable `filesystem.omg` calls none. But `filesystem.omg` does call 7
  POSIX-only operations (`change_owner`, `change_owner_no_follow`,
  `change_file_owner`, `duplicate`, `errno`, `set_file_permissions`,
  `sync_data`), so those 7 portable wrapper machines become target-scoped
  families whose Windows body returns `ErrorKind::Unsupported`. Omega
  consumers that name the trait: `filesystem.omg`, 5 target impls (4724 lines,
  242 `.host.` call sites), 17 fixtures under `tests/omega/pass/filesystem`,
  `tests/omega/pass/time/runtime_fs_mtime_system_time_interop_exit`,
  `samples/cli/systems/file_journal` and `samples/gui/image_viewer`; four more
  (`pass/filesystem/{dir_walk_wrappers_exit,windows_wrapper_results_exit,wrapper_lock_metadata_exit,wrapper_open_with_exit}`)
  reach it only through `Filesystem`, so they are the ones Q2 holds.

  Rust consumers the split must move together, none of them optional:
  `omega-rust/psi/interpreter/checked-interpreter/build.rs` code-generates
  `FilesystemHostOperation` by parsing `filesystem_host.omg` and asserting
  every `machine` line ends `reaches FilesystemHost;`; the evaluator's
  `execution/boundary_console.rs:24` falls back to the trait's bare name;
  `packages/package-manager/src/review/candidate/semantic_bindings.rs:314`
  maps the name to `AcceptedSemanticBindingRole::FilesystemHostService`, which
  `packages/compilation-inputs/src/review_record_encoding.rs:85` wire-encodes;
  plus `review-evidence/src/capture/{authority.rs:188,
  providers/policy/toolchain_settled.rs:30}`,
  `realization/selected-dispatch/src/{intrinsic_review.rs:43,
  execution_settlement.rs:217}`, and the driver test support in
  `tests/support/{fixture_package_inputs.rs:776,
  dangerous_service_acceptance.rs:139}`.

  The macOS binding payloads are already catalogued, so the provider rows are
  transcription rather than discovery: `HostOperation::from_name`
  (`omega-rust/omega/compilation/backend/calling-conventions/src/host_operations.rs:641`)
  names every libc operation the seam needs, `darwin_import_library`
  (`omega-rust/omega/compilation/backend/calling-conventions/src/hosts/darwin.rs`)
  resolves the `_`-prefixed symbol to `/usr/lib/libSystem.B.dylib`, and
  `ProviderBinding::Import` is what an authored
  `via ForeignBinding::DllImport { MachODylibSymbol { .. } }` row becomes. The
  msvcrt mirror in `hosts/windows.rs:53` lists which operations have no clean
  CRT equivalent and so must answer `Unsupported` on Windows.

  Per-target state: macOS and `linux_arm64` have no `FilesystemHost` rows at
  all; `linux_x86_64` has the 11-leaf toolchain mint; `windows_x86_64` has
  none. The mint already yields to an already-selected plan
  (`canonical_filesystem_host.rs:224`), so an authored per-target provider may
  land before the fallback is deleted — the fallback is retired by the change
  that gives `linux_x86_64` its own authored provider, not before.

  Acceptance: `samples/cli/systems/file_journal` exits 7 on macOS, Linux and
  Windows; the `tests/omega/pass/filesystem/` fixtures run natively on each
  host; a fixture that opens a file with stdin closed (descriptor 0) is not
  treated as Closed.

- **FILESYSTEM-RELEASE-CONTRACT.** Run
  `tests/omega/pass/filesystem/windows_canonicalize_exit` on Windows:
  canonicalize a file and a directory, keep missing-path `NotFound`, clean up,
  exit 70; and derive the occurrence-bound close proof that lets a proved
  close carry empty authority.

  - Current state: Windows rejects with "selected ProgramEntry Binding field
    `Main::fs::host` requires a selected Fused provider for boundary
    `FilesystemHost`"; the corpus earlier recorded a conditional state-parameter
    transfer refusal. Re-drive the whole program before naming the blocker;
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
  permutations keep finite write sets; `omega --check source/psi/gates/parser/main.omg`
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
  sealed call graphs, argument images, lease accounting and routed `Task<T>`
  claims exist. Park/resume today changes ledger state only; no worker or native
  stack switch runs. `tests/omega/pass/tasks/task_runtime_machine_selection_compile`
  checks a probe beside an empty, unbound `Main::main`.

  - Stacks: join final physical frames/spills, alignment, entry overhead and
    live chains to the exact `StackPlan` and a real nonmoving backing.
    `task_frame_summary` (`task_plans/stack_graphs.rs`) measures checked locals
    and parked arguments; `TaskRuntimeAdmission` stores slot shapes, not stacks.
    Keep unresolved-call refusal and same-stack/separately provisioned demand.
  - Suspension production in
    `omega-rust/psi/compilation/pipeline/05_checked-trees-to-lowered-psi/src/retention/suspension_call_plan.rs`:
    receiver, persistent/structural places, live claims and Unit-call scalar
    environments need exact joins. Boundary targets fail identity because
    `symbol_identity` in
    `omega-rust/psi/compilation/representations/checked-trees/src/checked_trees/facts/carry.rs`
    resolves only machine/state symbols; boundary-call suspension must keep
    rejecting until that join exists. Keep storage lifetime, pinning, aliasing,
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

- **QUOTIENT-RUNTIME-REALIZATION.** Execute quotient value and call operations
  under the [representative realization contract](wiki/spec/proofs/quotients.md#representative-layer-and-executable-realization)
  and [ordinary checked-body elaboration](wiki/spec/proofs/quotients.md#execution-through-ordinary-checked-bodies).
  Start with the existing bounded correspondence forms; **QUOTIENT-THEOREM-LIFT**
  is not a prerequisite.
  - Carriers: `omega-rust/psi/compilation/checking/src/proof_contracts/quotients/formation_collection.rs`
    requires a proof-only carrier; replacing the managed fixtures'
    `Representative` with `data Representative [copy] { value: i32; }` rejects
    with "quotient carriers must be proof-only". Admit representable,
    custody-free carriers; keep `carrier_fence.rs`'s rejection of affine/linear
    content and routed custody.
  - Undemanded metadata: `pass/proofs/quotient_{define,lift}_managed_compile`
    reject with `NonExecutableQuotientCorrespondence` from
    `omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/quotient_correspondence.rs`,
    which refuses every nonempty correspondence table. Mere presence of
    proof-only metadata must not reject an executable module; a demanded
    operation without a realizable representative still rejects.
  - Execution: carry the representative ABI through construction, arguments,
    results, calls, Terminal operations and native realization. Preserve public
    operation identity; generated wrappers use a stable derived identity and
    ordinary machines; correctness must not depend on inlining. Do not turn the
    sealed `quotient_operation` proof requests into runtime calls by deleting
    guards. No backend-specific quotient engine. Axiomatic operations supply no
    algorithm.
  Acceptance: the managed fixtures compile with their proof-only metadata;
  new run fixtures execute direct `define` and transport-backed `lift`
  operations natively with observable exits, without structural observers or
  representative extraction; demanding an unrealizable operation and
  mismatched arguments/results/relations reject.

- **QUOTIENT-THEOREM-LIFT.** Extend quotient correspondence beyond public
  parameter positions under [lifting operations](wiki/spec/proofs/quotients.md#lifting-operations)
  and [published correspondence](wiki/spec/proofs/quotients.md#published-quotient-correspondence).
  `omega-rust/psi/compilation/checking/src/proof_contracts/quotients/relation_plan/terminal_bridge.rs`
  already handles parameter selection/permutation/repetition/omission,
  immutable result aliases and finite state forwarding; reuse it.
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
  unproved termination. **QUOTIENT-RUNTIME-REALIZATION** owns execution.

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

- **FLOAT-PROVIDERS.** Complete the `FloatSemantics` obligations of the actual
  `Float`, `F32`/`F64` and conversion slots under
  [FloatMeaning](wiki/spec/terminal-psi/mathematical_values.md#floatmeaning).
  Closed Meaning arithmetic works: `add(BINARY32, meaning32(1), meaning32(2))
  == meaning32(3)` checks and `== meaning32(4)` rejects. Reuse that route.
  - `omega-rust/psi/compilation/pipeline/04_typed-trees-to-checked-trees/src/proof/float_meaning.rs`
    and `omega-rust/psi/compilation/checking/src/proof_contracts/float_projection_bindings/semantic_values.rs`
    accept only Meaning results with Format/Meaning operands. Closed true
    `is_finite`, `to_i32` and `from_integer` ensures stop at unsupported proof
    entailment. Add Boolean, classification and integer results and integer
    operands through their own proof carriers; non-Meaning results must not
    use the FloatMeaning carrier.
  Acceptance: corpus fixtures where slot contracts and imported
  call/operator results discharge those obligations; false claims, lookalike
  declarations and changed operands/formats/results reject. IEEE Boolean
  comparison stays distinct from structural meaning equality (NaNs, signed
  zeros). **OPERATOR-BOUNDARY-CALLS** owns executable provider installation;
  **X86-FMA-PROVIDER-TRANSPORT** owns native FMA.

- **RESTORE-DYNAMIC-DESCRIPTOR-AND-TABLE-CUSTODY.** Consume the selector/data
  separation from **DYNAMIC-IMPLEMENTATION-SELECTION-MIGRATION** in Omega
  lowering and native execution under [dynamic dispatch](wiki/spec/terminal-psi/dynamic_dispatch.md).
  Start with a helper that forwards a selector, accepts explicit data arguments,
  and uses the result across calculations and branches. The breaking-migration
  authorization in **NAMED-IMPLEMENTATION-CONFORMANCE-MIGRATION** applies.
  - Own Omega pipeline representations, lowering, instruction selection,
    register allocation, ISA emission and image table/relocation custody,
    plus corresponding `source/omega/` code. Replace the old `{instance, table}`
    descriptor and receiver adapters with implementation selection plus the
    requirement's ordinary operands. No subject slot, forced parameter zero,
    hidden receiver or fallback legacy call route.
  - Known old-path gaps to resolve in the new route: instruction selection
    rejects `LegalizedScalarInstructionKind::DynamicParameterCall`, and
    `omega-rust/omega/compilation/backend/images/image-emission/src/function_fragments/production.rs`
    emits empty dynamic rosters. Implement indirect calls, immutable complete
    tables, relocations and final-image evidence through the ordinary owners.
  - Preserve exact table/slot, ABI, clobber, effect and explicit argument-loan
    identities. Unit calls have no result home; selector and result storage
    cannot overlap. Tables retain trap slots and private implementation
    identities. Forwarding, storage and result composition must accept ordinary
    effects and control flow; remove superseded one-call/scalar recognizers.
  Acceptance: migrated `runtime_local_named_dyn_pass_through_exit` and
  `runtime_dyn_two_impl_dispatch_exit` run on matching supported hosts, including
  Windows and macOS. Their results and writes to explicitly supplied data agree
  with interpretation. Add zero-data and multiple-data-argument calls and
  preserve producer-private selection on returned selectors. Run
  `python tools/corpus_gate.py --filter traits/` and affected backend checks.
  A `dyn` selector owns no implicit data; explicit owners retain the ordinary
  cleanup obligations covered by **CLEANUP-HOOK-SELECTION-AND-ERASED-OWNERSHIP**.

- **TARGET-SEMANTIC-APPLICATIONS.** Make compile-time evaluation consume the
  [target-semantic capsule](wiki/spec/language/evaluation.md#target-semantic-capsule)
  of the selected target, and close package review over
  [operator applications](wiki/spec/terminal-psi/boundary_calls.md#operator-applications-and-physical-children).
  - `size_of`/`align_of` already fold through native layout authority in
    `omega-rust/omega/compilation/realization/provider-planning/src/data_layout_queries.rs`,
    but targetless settlement chooses the host. Observations must name their
    subject (address space, data layout, format, profile) from target closure,
    never compiler-host facts. Reuse typed arguments and compiler-materialized
    target data; a new projection needs a concrete customer, not a general
    `TargetSemantics::*` source API (its spelling is unsettled per the spec).
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

- **TOP-LEVEL-BOUNDARY-REQUIREMENTS.** Complete execution and identity for
  explicit `boundary requirement Owner::name(...);` under
  [machine supply](wiki/spec/language/machines.md#supply),
  [provider selection](wiki/spec/build/provider_selection.md) and
  [requirement identity](wiki/spec/terminal-psi/boundary_calls.md#call-and-requirement-identity).
  - Dispatch: `omega-rust/omega/compilation/realization/selected-dispatch/src/boundary_dispatch/adapter_rows.rs`
    admits owned/shared/mutable receivers, erased lifetime telescopes and
    closed generic rows. Extend the remaining generic/family and projected
    receiver combinations through both engines; write-only and qualified
    receivers still reject. Keep `pass/providers/checked_boundary_requirement_dispatch_exit`
    (exit 70) and `lifetime_boundary_requirement_dispatch_exit` (exit 41).
  - Interpreter providers: `pass/providers/external_boundary_requirement_via_exit`
    (native exit 21) and `runtime_foreign_leaf_borrowed_byte_argument_exit`
    (native exit 70, libSystem `_strlen`) fail in the checked interpreter,
    which rejects bodyless external calls without a provider. Restore the
    mixed borrowed-record customer (initialized fields, reused returned
    value). Keep missing-import rejection; ordinary foreign exit is distinct
    from canonical ProcessExit.
  - Identity: the package-qualified operation, static telescope, signature,
    visibility and selected provider/adapter need a canonical kind distinct
    from trait requirements; reuse package review's top-level requirement kind.
    Coordinate with **PROVIDER-SELECTION-AFTER-TERMINAL**.
  Acceptance: value, statement, receiver and external-provider customers run
  in the interpreter and natively under one selected plan; unselected,
  ambiguous and substituted providers, unauthorized private/foreign
  declarations and same-spelled cross-package declarations reject.
  `Task::finish`/cancellation coordinates with **BOUNDED-INSTALLATION-REACH-ROWS**
  and **COMPONENT-SUBSTRATE**; **OPERATOR-MACHINE-SUPPLY** owns retiring
  `MachineSupplyMode::Boundary`.

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
  Acceptance: initial, added, widened and transitive requests stop before any
  effect, show their coordinates and leave review incomplete; resuming the
  exact candidate with accepted requests runs and reviews the generated
  source; a filesystem sentinel shows missing, refused or cross-purpose grants
  perform nothing. Unchanged requests need no reapproval; interrupted or stale
  candidates keep the prior lock; locks hold decisions, never secrets or host
  paths. No recursive build API or hidden post-compilation callback.
  **BUILD-SNAPSHOT-OUTPUTS** owns committed outputs.

- **OPTIONAL-STDLIB-SEMANTIC-BINDINGS.** Remove the broad standalone std/alloc
  `Toolchain` provenance from
  `omega-rust/omega/packages/compilation-inputs/src/source_imports/frontier.rs`
  under the [toolchain/library contract](wiki/spec/packages/toolchain.md).
  Package-aware compilation already limits the magic mount to core.
  - Migrate the remaining fallback consumers to ordinary dependency edges and
    the [semantic binding roles](omega-rust/omega/packages/compilation-inputs/semantic_bindings.md)
    (Console, host services, target physical entries). Directory relabeling
    grants nothing. Order: executable host/objc customers, then proof/fail
    consumers and nested member imports (the interrupt/secondary-processor
    fixtures' std calling contracts). Route exposed call/ownership/entry/proof
    failures to their owners. **UEFI-PHYSICAL-SEMANTIC-ENTRY** owns the
    wrong-policy canary's duplicate-schema rejection;
    **PROOF-CONTRACT-MIGRATION** and **PROOF-KERNEL-CORE** own private core
    proofs.
  - The old build-file census test was deleted with the Rust tests. Its
    replacement must inventory actual library imports, including build
    sources, fail-tier fixtures and imports below member directories. A root
    without `build.omg` is not migration debt; std needs no edge to itself.
  Acceptance: after removing the fallback, every corpus consumer keeps its
  outcome; removing a dependency edge rejects its imports and provider
  selections; aliases, paths and same-spelled substitutes confer nothing; a
  fail fixture pins missing-edge and alias rejection. Std stays replaceable,
  splittable or absent.

- **COMPONENT-SUBSTRATE.** Execute an ordinary two-package call with
  root-selected `Independent` composition under the
  [component publication contract](wiki/spec/build/component_publication.md),
  then stable-slot replacement. Runtime packages or Cathedral own deployment
  and update policy.
  - Package review publishes the dependency description
    (`omega-rust/omega/packages/package-manager/src/build_session/component_publication.rs`),
    and provider planning checks it, but
    `omega-rust/omega/compilation/realization/provider-planning/src/provider_settlement/composition_modes.rs`
    rejects every `Independent` edge before production. Replace that fence
    only with real independent products carrying symbolic imports/exports,
    entry/leave resources and installation obligations; never silently fuse.
  - Complete the entry, outgoing-authority, service, mapping, lease, stack and
    retained-provider inventories; connect `component-candidate`'s
    `describe_component` native stack and realization identity to
    publication. Join deployment to the description contract used by
    **TOPOLOGY-PLAN-VERIFICATION**. `omega-rust/omega/compilation/backend/runtime/component-deployment`
    rejects registries with live external roots; finish their teardown.
  - Replacement reuses `component-publication`'s era entries: compatible
    imports/exports, new calls enter the new era, existing sessions keep
    theirs, candidate resources fit the frozen envelope, and retirement waits
    until every activation, registration and returned object is disposed.
    Rejected operations preserve custody. Stronger continuity needs its
    explicit contract.
  Acceptance: a two-package corpus build with `Independent` composition
  emits separate products and executes the crossing call; descriptions grant
  no install authority; omitted, duplicate or unaccepted-assumption cases
  reject; a replacement customer shows new-era routing and old-era drain.
  **BOUNDED-INSTALLATION-REACH-ROWS** consumes this; **WIRE-RUNTIME-AND-INSTALLATION**
  owns platform invocation; **PSI-COMPONENT-REPLACEMENT** the interpreted
  customer.

- **FFIVAL.** Run the Windows user32 customer at
  `tests/omega/pending/host/user32_window_procedure_registration` through its
  authored build and the [private-callback protocol](wiki/spec/build/private_callbacks.md).
  - `omega --check tests/omega/pending/host/user32_window_procedure_registration/main.omg`
    rejects the by-reference outcome: both `Registered` paths lack
    `Registration::Live`, and the success/rejection payloads stay unestablished
    linear values. Carry the registrar's outcome and capacity through ordinary
    qualification and custody rules.
  - Supply the package-owned user32 adapter, exact ABI/layout and per-window
    context protocol. The fixture's retry branch neither forces rejection nor
    re-registers the returned capacity; complete those steps.
  - **REGISTERED-CALLBACK-LIFETIME** owns the lifecycle;
    **CALLBACK-PRIVATE-MATERIALIZATION** owns native callback realization.
  Acceptance on Windows: registration, synchronous `WM_NCCREATE`, dispatched
  callback entry, deterministic rejection/retry, teardown and re-registration
  with the returned capacity; quiescence precedes code-lease release. No raw
  callable address, implicit closure or Win32 compiler escape. Move the fixture
  out of `pending` into a run tier the corpus gate executes; other hosts report
  the runtime leg unavailable.

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

- **X86-FMA-PROVIDER-TRANSPORT.** Connect selected x86 FMA plans to Terminal
  occurrences and ordinary target lowering, instruction selection, register
  homes and image emission.
  - `pass/float/x86_fma_plan_association` rejects: "compiler float arithmetic
    for `operator::F32::fused_multiply_add` ... has no local target catalog
    realization". The encoder exists in
    `omega-rust/omega/compilation/backend/machine-emission/src/x86_fma.rs`;
    `omega-rust/omega/compilation/pipeline/00_terminal-psi-to-abstract-operations/src/comparison_settlement.rs`
    rejects FMA without selected occurrence custody; stage 02 rejects FMA;
    stage 03 admission and function-fragment emission are missing.
  - Keep AVX+FMA3 feature admission, XMM homes and MXCSR save/restore.
    `omega-rust/omega/compilation/backend/images/image-emission/src/object_artifact/replay/x86_fma.rs`
    handles only constant-load operands; extend to runtime values.
  - Nested calls and named-provider installation belong to
    **OPERATOR-BOUNDARY-CALLS**; direct `F64::fused_multiply_add` lacks a
    requirement-level Terminal route.
  Acceptance: `x86_fma_plan_association` runs on a matching host; both formats
  distinguish fused cancellation from separately rounded multiply/add,
  including repeated FMA on a prior result. Missing or wrong-profile feature
  admission and operand/provider substitution reject. Report cross-target
  byte checks separately from hardware execution.

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
  consumer `source/library/topology/verify.omg`. Reuse `omega-rust/library/topology/`'s
  normalization, policy, certificate and codec logic and
  `VerifiedComponent::export_contracts()`; derived contracts, not surface-name
  hashes, join demands to supplies. The Rust composition test is deleted and
  `verify.omg` has never been driven; component admission must come from
  ordinary component-description production, not Rust-built Terminal modules.
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
  after **TOPOLOGY-PLAN-VERIFICATION**. Reuse
  `omega-rust/library/topology/src/topology_installation.rs` and its mediation,
  frame, schema, pipe and supervisor modules; `LocalProcessSupervisor` is
  Unix-only. Add explicit Windows handle transfer and macOS validated/signed
  member images over the existing descriptor-transfer path. Disable general
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
  `RC-REPRESENTATIVE-PROGRAMS` (`refresh-samples`) builds samples but does not
  run their exit/output oracles; **SAMPLE-CORPUS** owns that execution.

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
  `resolver-execution`) at the release commit; **RED-WINDOWS-HOST** owns the
  current `package-source` Windows failures. These commands alone do not
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
    supplied-custody positive. **CALLBACK-PRIVATE-MATERIALIZATION** owns
    callback calling/fragment custody.
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
  - **Host-provider execution:** every hosted target has a reviewed TimeHost
    provider at `source/library/std/targets/<target>/time_host_impl.omg`.
    Only the two Darwin targets are EXECUTED:
    `pass/time/runtime_time_host_native_darwin_exit` exits 70 on macOS ARM64
    and `runtime_time_host_native_darwin_x86_64_exit` exits 70 under Rosetta.
    For linux_x86_64, linux_arm64 and windows_x86_64 the providers only make
    the artifact REALIZABLE: `corpus_gate.py --native-target <target> --filter
    time/runtime_time_elapsed_since_exit` moves that fixture from `rejected`
    to `built` for all three at 22d7c734257, measured against the same command
    with the four provider files removed. Nothing executes them, so their
    `clock_gettime` numbers (228 on x86-64's own table, 113 on arm64's
    asm-generic table) and their kernel32 exports rest on the kernel and API
    sources each file cites, not on a run. The lent-storage read all three
    depend on is pinned on Darwin by
    `pass/providers/foreign_out_parameter_write_observed_exit`, which exits 72
    if a caller observes the pre-call initializer instead of the kernel's
    write. Next acceptance: `runtime_time_elapsed_since_exit` exits 70
    natively on a Linux x86-64, a Linux arm64 and a Windows x86-64 host. Those
    three native goldens still read `rejected` for it, and `--record` cannot
    refresh them from this host: the exit-oracle promise is weighed before
    recording and a foreign-target build supplies no runtime evidence. The
    check-leg row for that fixture is separately stale at `rejected` — it
    measures `checked` with the providers REMOVED, so the closure-review
    change owns that row, not this provider work. Remaining beside that:
    filesystem adapters with real syscall marshalling and result transport,
    beyond the Linux x86-64 rows in
    `omega-rust/omega/compilation/realization/provider-planning/src/provider_settlement/canonical_{filesystem,time}_host.rs`.
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
  `tests/omega/pass/filesystem/windows_set_file_time_exit` rejects today and
  its oracle is only a comment: add `expected_native_exit` so the fixture must
  call SetFileTime then `_stat64`, read back Unix time 1,500,000,000 and exit
  70.

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
  `source/psi/` and `source/omega/` (product roots `source/omega/build.omg` and
  `source/omega/main.omg`). Psi owns parsing, resolution, typing, checking,
  proof and Terminal Psi; Omega consumes Terminal Psi for provider selection,
  realization, ABI and emission. The product reads source, lexes, parses
  supported `use`/`data` roots and exits without an artifact. Extend that one
  pipeline through the full language in compositional slices; no private
  dialect, file/AST allowlist, source-shape-only lowering or parallel
  source-to-native route.

  Current blocker: `omega --check source/omega/main.omg` and
  `omega --check source/psi/gates/parser/main.omg` reach `Parser::parse` and
  omit its unit plan at state 7 (`conditional successors: guard expression`).
  Minimize the nested `Token::Keyword { keyword: KeywordKind::Data }` guard in
  `source/psi/parse/parser.omg` before assigning the lowering repair to
  **STATE-LOCAL-VALUE-FRONTIER**. Unverified: recheck both roots on current
  main. Then build and run `source/psi/test-parser.sh` with explicit freshly
  built `OMEGA_CLI` and `OMEGA_TARGET`, keeping the parser gate's
  acceptance/rejection, lexical handoff, structural identity, capacity and
  determinism controls. Keep range-proven narrowing casts.

  Acceptance: the package-resolved closure implements the
  [compiler product contract](wiki/spec/build/compiler_request.md), passes the
  shared product suite and Rust/Omega differential checks, and publishes a
  deterministic manifest of all transitive compiler/build inputs. Epsilon-written
  D and `omega0 -> omega` construction stay on `TASKS_BOOTSTRAP.md`.
