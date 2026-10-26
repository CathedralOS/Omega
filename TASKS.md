# Tasks

This board supplies capability priorities and remaining acceptance. Compiler
work follows [the sample loop](AGENTS.md#the-sample-loop): select an outcome,
make its required witnesses run, land complete repairs, and continue until
the outcome's acceptance is met. `python3 tools/sample_queue.py` helps find
witnesses; smallest-first is not the release priority. The
[completion contract](wiki/drafts/reference/rust_compiler_completion.md)
defines release acceptance. Squalr is a customer, not the destination or a
reason to special-case source; all accepted language scope stays required.

## Working the board

The sections below describe capability gaps, release requirements and design
context under stable IDs that other documents link to. Tasks enter and live on
this board through one lifecycle:

1. **An issue or missing capability is discovered** (from a customer program,
   fact witness, specification requirement, or architectural gap).
2. **It is framed as an end-to-end sample outcome** (`samples/cli/facts/<fact>/`
   across all nine transport places and their reject twins, or a named customer
   program plus invalid controls) carried through the entire pipeline (`Psi 00–04`
   checking/proof → `Psi 05–07` Terminal Psi lowering and interpretation →
   `Omega 00–09` target realization, layout/ABI plans, instruction selection,
   register homes, encoding, and native execution via `omega run --both`).
3. **It is merged by similarity into one single-agent loop outcome** so one
   agent can carry related compiler, library, and fixture repairs together in
   one continuous loop without cross-task handoffs (see
   [When to fold subagents into your main agent loop](https://claude.com/resources/articles/building-agents-when-to-fold-subagents-into-your-main-agent-loop)).

Read the whole selected item and check its frontier against current main before
assigning work. Choose the next executable witness by the remaining requirement
it establishes, not because its failure is small. Samples and fixtures prove
capability outcomes; they do not replace the item's broader acceptance.

After each complete landed repair, subtract what its evidence establishes and
continue the same item's next required witness. Delete the item when nothing
remains. A passing first witness is evidence to reconcile, not an instruction
to keep changing it until it fails. External prerequisites, release gates and
measured development-health incidents retain their own non-sample acceptance.
Keep owner decisions in [OWNER_QUESTIONS.md](OWNER_QUESTIONS.md). Do not turn
this board into one entry per witness, a stage-sliced handoff list, or a
history of successful runs.

Optimizer and bootstrap work keep their own boards
([TASKS_OPTIMIZER.md](TASKS_OPTIMIZER.md), [TASKS_BOOTSTRAP.md](TASKS_BOOTSTRAP.md))
and acceptance; Cathedral structures stay Cathedral's.

A claim note carries the parent item, remaining acceptance, witness and oracle,
the requirement it discharges, current failure, next necessary action and
prerequisites. After compaction or a checkpoint, reread
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
  `samples/cli/scientific/w100/` end to end through `omega run --both`. Reuse
  preserved candidate sources and frozen mathematical oracles; the integration
  owner implements all required interfaces without active-worker prerequisites.
  Current frontier: connect descriptor-backed moments to the shared layout,
  canonical float slice access and numeric policy; qualify both complete
  consumers and separate invalid controls, then continue the retained rank-two
  kernels. Preserve unrelated work and land coherent qualified chunks through
  the normal queue. Candidate preservation in IMPORTS.json is not capability
  acceptance.

## Updating the boards

Update by subtraction, not accumulation. This applies to all three task boards.

- Read the entire owning item before editing. Remove completed work rather than
  replacing it with a success report; delete the item when no work remains.
- Retain the remaining failure, owning mechanism, necessary context, dependencies
  and end-to-end sample/fixture acceptance (`omega run --both`). Keep passing
  regression commands only where they constrain the remaining repair. Run
  histories and success totals belong in commit messages and test records, not
  the board. Keep a revision only when needed to reproduce or qualify an
  unresolved observation.
- Search the boards for the repaired capability and affected program; reconcile
  stale claims elsewhere. A passing narrower example does not close a wider task.
- Fold newly discovered issues and adjacent sub-tasks into the existing capability
  outcome that shares their subsystem and sample family rather than creating
  stage-sliced or `split-of:` shards. Unrelated findings enter the board only
  with a distinct required end-to-end sample outcome and acceptance. No mining
  to fill slots.
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
  dependencies' bodies (`std` is checked once at package review, then re-judged
  in dependents). Dependents must consume that judgment through compiler-private
  checked package reuse bound to exact source, dependency and compiler identities,
  retaining the same facts and body-dependent queries at the same precision.
  - Immutable declaration pages and their fact-context index are shared across
    independent flow groups; local facts, fixed-point joins and unknown-input
    fallback remain per-group. Whole-program reuse requires the original typed
    input and complete preparation admission to match; a changed opaque
    application takes fresh checking.
  - Keep frozen dependency bodies available to `CallFrameResolver` (returned-reference
    origins and binding stability), denotational admission (observation freedom),
    and `return_ranges::infer_return_interval` (captured arrivals under the
    caller's actual intervals). Reuse the existing checked graph and fact owners
    with one declaration/state/expression correspondence, while new local bodies
    and generic substitutions retain their own obligations.
  - Same-invocation reuse now skips repeated Stage 05 contract entailment for a
    dependency body whose correspondence matches a strict retained check;
    persisted compiler-private reuse bound to exact source/dependency/compiler
    identity and native Terminal product reuse remain open.
  Acceptance: fixture median under 2 s and a 10-line `std` probe check under 3 s,
  with `corpus_gate --compare` over the touched groups losing nothing and
  `tests/omega/fail/proofs/structural_ensures_unjudged_rejected` still rejecting.

- **CHECKED-ASM-LABEL-CONTRACTS.** (also covers **ASM-INSTRUCTION-CATALOG-EXPANSION**,
  **PRIVILEGED-PORT-EFFECT-SETTLEMENTS**, and **INTERPRETED-ASSEMBLY**)
  Implement the ratified checked-assembly control-flow, catalog, effect-settlement,
  and interpreted/native execution model in
  [assembly](wiki/spec/language/assembly.md) and the
  [inline assembly catalog](omega-rust/psi/foundation/language-core/inline_assembly.md)
  end to end (`Psi 00–07` checking/lowering/interpretation and `Omega 00–09`
  target realization, effect settlement, and byte encoding).
  - Explicit labels are basic blocks, each label may carry ordinary
    `requires`/`ensures`, and every forward edge and backedge is proved against
    exact modeled register, flag, memory, place, authority and ownership state.
    Owners: `01_tokens-to-syntax-trees/src/bodies/statements/inline_assembly.rs`,
    `language-core/src/inline_assembly/`, stage 04 CFG/state transfer and
    `execution/terminal_unit/calls/call_operations.rs`, stage 05 lowering,
    Omega stage 02 `lowering/control_flow/operations.rs`, stage 03 scalar-graph
    admission, `image-emission/src/function_fragments/{production,validation}.rs`,
    `native-artifact/src/physical/derivation/provider_custody.rs`, and the
    shared Terminal/native execution paths.
  - Established fact rows (`checked_asm_countdown`, `checked_asm_flag_snapshot`,
    `checked_asm_zero_branch`, `checked_asm_view_move`, `checked_asm_flag_restore`,
    `checked_asm_rank_decrease`, `checked_asm_add_carry`, `checked_asm_arm_load`)
    and `pass/asm/scratch_snapshot_survives_input_write_exit` remain regression
    controls.
  - Remaining control, catalog, settlement, and interpretation obligations:
    - Other flag-producing instructions, AArch64 countdown/register-view parity,
      and checked hardware-effect retirement of exact linear obligations: a
      checked machine satisfying a consuming boundary-trait member may retire a
      linear argument only when instruction semantics and label contracts prove
      the required effect on every normal exit (`nop`, wrong port/place/value,
      missing ordering, unmatched installation/token lineage, or a bypass path
      leaves the argument live and rejects). `asm where` and label `ensures` are
      proof goals, never admissions.
    - Catalog and transitive plans: `call_operations.rs` plans only constant
      `AsmPortOut`; `pass/inline_asm/asm_runtime_port_msr_final_validation` and
      `asm_aarch64_system_registers_compile` reject with missing checked
      transitive machine plans. Complete checked plans, lowering, Terminal
      encoding, target realization and final-span evidence across x86 fences,
      interrupt control, flags, MSR/control registers, pipeline/cache directives,
      and AArch64 system registers.
    - Privileged port effect settlement (**PRIVILEGED-PORT-EFFECT-SETTLEMENTS**):
      connect `TargetUnitOperation::PortWrite` and port read (`in`) through
      Omega stage 02/03 lowering, register homes, `function_fragments` port
      rosters, and `provider_custody.rs` consumer-owned settlement
      ([consumer-owned settlement](wiki/spec/terminal-psi/boundary_calls.md#consumer-owned-settlement))
      for an installed adapter owning the exact selected `PortIo` requirement;
      keep `pass/inline_asm/asm_port_out_final_validation` rejecting as a direct
      root without provider requirement custody.
    - Interpreted assembly (**INTERPRETED-ASSEMBLY**): deliver
      [checked instruction interpretation](wiki/spec/language/assembly.md#interpreted-execution)
      through source → Terminal Psi → the ordinary interpreter with a supplied
      target environment across device, interrupt-state, idle and external-entry
      sequences and fuel pauses without repeating completed I/O.
  - Integration customers: `pass/memory/interrupt_table_canary` and
    `pass/memory/secondary_processor_canary` under
    **TOP-LEVEL-BOUNDARY-REQUIREMENTS** (one-member boundary traits and checked
    implementations whose `asm` proves LAPIC/PIC completion and startup
    handshake, preserving `Pending` domain preconditions, exact installation/token
    identity, bounded reach and linear consumption).
  Acceptance: nine-place `samples/cli/facts/checked_asm_*` witnesses and labeled
  straight-line, diamond and loop bodies execute identically through Terminal
  and native paths (`omega run --both`); `asm_runtime_port_msr_final_validation`,
  `asm_aarch64_system_registers_compile`, and an authored selected `PortIo`
  adapter emit exact native bytes and settlements; the interrupt and
  secondary-processor canaries check and run with no fake `CompilerIntrinsic`
  provider; wrong-edge invariants, unmodeled memory, wrong controller/port/target
  coordinates, missing effects/order/clobbers, live linear exits, and
  forged/replayed/double-completion controls reject (existing `fail/inline_asm/*`
  stay rejected).

- **STATIC-MACHINE-MIGRATION.** (also covers
  **NAMED-IMPLEMENTATION-CONFORMANCE-MIGRATION**,
  **TRAIT-IMPLEMENTATION-SOURCE-MIGRATION**, and **STATELESS-PROVIDERS**)
  Complete the owner-authorized migration to
  [static machine declarations](wiki/spec/language/machines.md#static-declarations-and-explicit-state),
  [named implementations](wiki/spec/language/conformances.md#data-and-code-are-independent)
  (`NormalCounter satisfies CounterOperations<CounterData>` names code, not data;
  no `Self`, no data-subject satisfaction, and no `Trait::member = Existing::machine;`
  reference rows), stateless providers (**STATELESS-PROVIDERS**), and
  [domain-selected automatic cleanup](wiki/spec/language/domains.md#domain-selected-automatic-cleanup)
  (`dropped by` on named domains and data default domains) end to end across
  `omega-rust/`, `source/psi/`, `source/omega/`, `source/library/`, `samples/`,
  and `tests/omega/`.
  - **Named implementations and source migration
    (NAMED-IMPLEMENTATION-CONFORMANCE-MIGRATION,
    TRAIT-IMPLEMENTATION-SOURCE-MIGRATION, STATELESS-PROVIDERS):**
    - Complete const-expression closure and package projection outside the
      reference producer's qualified scalar inline member path, and calling-policy
      or member kinds beyond the closed inline calling-policy path. Complete
      matching-host execution for `samples/cli/facts/positional_implementation_binder`,
      `samples/cli/facts/open_const_implementation`,
      `samples/cli/facts/open_record_const_implementation`,
      `samples/cli/facts/named_conformance_dispatch`, and
      `samples/cli/facts/generic_calling_policy` across remaining hosted targets,
      and extend `source/psi/` product-producer parity for evidence binders across
      data, conformance, domain, proposition, and machine telescopes.
    - Retire authored `Trait::member = Existing::machine;` reference rows in both
      producers, bundled policies/providers, boundary roots (`interrupt_table_canary`,
      `secondary_processor_canary`), application ports, and fixtures in favor of
      inline member declarations calling shared helpers, preserving exact entry
      establishment authority and foreign trust dependencies.
    - Finish removing implicit provider state in installation
      (**STATELESS-PROVIDERS**): `ForeignBinding::VtableField` takes vtable
      addresses from its foreign binding plan, not from provider data; no
      installation or interpretation route supplies a provider value or receiver
      its requirement did not author.
    - Move remaining Build facet receiver calls (such as test groups) to
      namespace-qualified calls with explicit arguments (`Build::bind`,
      `Build::select_provider`, `Build::exclude_service`,
      `Build::select_representation`, `Build::accept_boundary`,
      `Build::require_wire_compatibility`, `Build::assume_binding_contract`).
  - **`Binding<R>` retirement and compiler cleanup:**
    - Delete the remaining compiler paths and data shapes whose retired
      `Binding<R>` carrier branch can no longer fire:
      `declarations/trait_definition/applications/source_fields.rs`,
      `declarations/invocations.rs`, `shape_collector.rs`,
      `omega-rust/psi/compilation/checking/src/machine_calls/boundary_calls.rs`,
      `omega-rust/psi/compilation/checking/src/value_custody/type_references.rs`,
      `omega-rust/psi/compilation/representations/checked-trees/src/checked_trees/service_parameter.rs`,
      and selected-dispatch `service_custody` with
      `program_entry_service_requirements`.
    - Add fail controls for ambiguous provider supply; unpark
      `inherited_applied_binding_ambiguous_ancestors`,
      `occurrence_region_binding_conflicting_inputs`, and
      `private_binding_authority_contract`.
    - Retire remaining attachment helpers in checking/lowering and complete
      checked per-value loan carry evidence for suspension production without
      restoring hidden argument insertion.
  - **Code/data separation and domain-selected `dropped by` cleanup:**
    - Retire `machine Data::operation` and reserved `T::drop` spellings across
      compiler/toolchain helper namespaces, operator/proof declarations, and
      fixtures.
    - Implement `dropped by` on named domains and data default domains: parse,
      resolve independent machine applications, and preserve through domain
      normalization, call transport, erasure, Terminal cleanup, and native
      execution (zero selections retain structural eligibility; repeated
      identical selections invoke once; distinct inherited/conjoined selections
      reject).
    - Red place on main: `samples/cli/facts/domain_selected_cleanup/erased_predicates`
      (explicit predicate erasure) refuses native compilation since its landing
      (#165, `f8645229c43`); the probe at the landing commit fails too, so the
      place never ran green. Terminal Psi lowering reports `InvalidUnitMachinePlan`
      — "resident membership home has no exact source projection"
      (`pipeline/05_checked-trees-to-lowered-psi/src/unit/attached_unit/structural_values/emission/current_memberships.rs`).
  Acceptance: a nine-place fact row for `dropped by` with observable cleanup side
  effects passes `omega run --both` (with reject twins for conflicting machine
  applications, erasure bypass, missing inherited obligations, unauthorized
  selection, direct hook calls, and hooks that fail/block/suspend/crash/exit);
  `samples/cli/basics/generic_counters` (exit 16), clock samples, and
  `samples/gui/{window_app,window_demo,windowed_calculator,image_viewer}` compile
  and run on their supported hosts with their authored results;
  `python tools/corpus_gate.py --filter traits/,layouts/,progress/,proofs/named_witness_static_trait_plural_compile`
  and `tests/omega/pass/providers/runtime_boundary_primitive_array_argument_exit`
  (exit 70 in both engines) pass; no active Omega source or compiler path retains
  `self`/`Self`, data-subject satisfaction, implicit receiver arguments,
  data-owned machine declarations, conformance reference-row assignments, or
  source-visible `Binding<R>`.

- **RESTORE-DYNAMIC-DESCRIPTOR-AND-TABLE-CUSTODY.** (also covers
  **DYNAMIC-IMPLEMENTATION-SELECTION-MIGRATION**)
  Complete runtime selection of named implementations (`dyn`) with separated
  selector and data lanes under
  [dynamic dispatch](wiki/spec/terminal-psi/dynamic_dispatch.md) end to end
  across Psi checking/lowering/interpretation, package review, Omega 8-byte
  dispatch-table realization, and `source/psi/` + `source/omega/` product parity.
  - **Psi checking, contracts, and package review
    (DYNAMIC-IMPLEMENTATION-SELECTION-MIGRATION):**
    - Extend requirement-contract checking at selector calls beyond exact fact
      correspondence to transparent proposition aliases, supported integer
      arithmetic, and literal IEEE-bound weakening while preserving the
      requirement's public contract independently of stronger implementation
      guarantees (preserving `samples/cli/facts/dynamic_float_refinement` across
      all nine places).
    - Support suspending requirements at planning, crash-refinement rows when
      operands lead with the selector, and ranked recursion cycles crossing a
      dispatch edge (`fail/traits/dyn_selector_{reentry,self_reentry}_unranked`
      until ranked).
    - Extend public slot metadata with returned-borrow origins and parameter
      qualification preconditions, consume the new records in package review,
      and implement `source/psi/` and `source/omega/` producer/consumer parity.
  - **Omega realization and cross-host native execution:**
    - Preserve complete 8-byte tables, zero trap slots, exact private callable
      identities, and patched image targets across all supported hosts, including
      matching-host Windows x86-64 execution.
  Acceptance: `python3 tools/corpus_gate.py --both --filter traits/` (including
  `runtime_dyn_rebound_selector_forwarded_exit`,
  `runtime_dyn_two_impl_dispatch_exit`,
  `runtime_dyn_{structural,nested,array,guarded,linear}_result_exit`,
  `runtime_dyn_data_argument_counts_exit`, `runtime_dyn_joined_selector_exit`,
  and `runtime_dyn_selector_record_field_exit`) passes with Terminal and native
  agreement on all matching supported hosts (including Windows and macOS), and
  copying or rebinding a selector neither captures data nor changes its borrow
  lifetime.

Prioritize unchanged customer programs reaching native execution over
additional evidence without an exercising program. The
[Rust compiler completion plan](wiki/drafts/reference/rust_compiler_completion.md)
defines the complete product bar.

- **STATE-LOCAL-VALUE-FRONTIER.** (also covers **REFERENCE-LOCAL-BINDING-ROUTES**
  and **TRANSITION-TAIL-CALL-TO-MACHINE**)
  Complete ordinary expression, call, reference-local, tail-transition, and
  storage composition end to end through `samples/cli/facts/` rows and customer
  programs (`omega run --both`), preserving the
  [evaluation schedule](wiki/spec/language/expressions.md#evaluation-schedule),
  [argument/result custody](wiki/spec/terminal-psi/calls_and_outcomes.md#argument-and-result-ordering),
  [loans](wiki/spec/terminal-psi/loans.md), and
  [terminal calls in transition arms](wiki/spec/language/state_contracts.md#terminal-calls-in-transition-arms).
  Owners: `omega-rust/psi/compilation/pipeline/04_typed-trees-to-checked-trees/src/execution/terminal_unit/`,
  `.../terminal_unit/control/statement_sequence/local_data.rs`,
  `.../terminal_unit/receiver_aliases/`, stage 05 argument evaluation,
  `expression_preparation/bindings/`, `scalar_lowering/computations/fields.rs`,
  and attached Unit emission; Omega stage 02 owns target admission. Extend
  common operations and remove superseded whole-machine recognizers as you go.
  - **Computed calls and state transfers:** finish selected mixed scalar/structural
    operands, effectful arguments/returns, nested mutable-argument calls,
    dependent/public-trait results, cyclic/opaque/provider/dependent result
    composition with exact guard and operand completion cuts (copied returns
    through ranked identities and public local-dyn slots, and returned
    window extents as mathematical endpoint differences), and runtime-dispatched
    value construction on the common `EstablishStructuralValue` route through
    ordinary argument evaluation. Preserve once-only field/operand order,
    selective short-circuit effects, computed-result float format, selected
    affine cleanup, and occurrence-bound routed memberships through completed
    children, copies, and nested projections (retaining
    `samples/cli/facts/copied_call_argument_snapshot` including
    `copied_call_argument_snapshot/*/{indexed_byte_view,inferred_indexed_byte_view,post_write_indexed_byte_view,public_post_write_indexed_byte_view,ranked_indexed_byte_view,computed_ranked_indexed_byte_view,captured_window_indices}`
    and `samples/cli/facts/matched_field_windows` across all nine places).
    - **Const-argument recipe boundary (red on main):** since `a600523a236`
      (#91) stage 01 retains every generic value argument as a
      `ConstExpression` recipe without parser-side evaluation, so a layout built
      from raw 01→03b output degrades a generic-capacity field to `Unit` and the
      layout crate's two `full_width_*_rejects_placement_instead_of_panicking`
      tests are red on main (bisected 2026-10-26; red at every base since #91,
      including `79804c48963`-good → `a600523a236`-bad). The value closes in the
      driver's execution settlement
      (`omega/compilation/driver/src/execution_settlement/constant_recipe_closure.rs`),
      whose fold rewrites application sites to canonical literal spellings —
      the representation the layout build consumes at realization. Any 01→03b
      consumer that needs closed extents (the layout tests included) must be
      migrated to that closed representation or have its contract restated.
  - **Loan locals and projected storage (REFERENCE-LOCAL-BINDING-ROUTES):**
    reference-valued locals from every admitted producer keep their loan
    identity, backing, and access through reads, calls, and explicit state
    transfers with no implicit backing copies. Qualify the remaining routes on
    Linux x86-64 and Windows x86-64, and carry
    `samples/cli/facts/scalar_tag_field_loan` through remaining matching hosts.
    Keep the local symbol as the loan subject and compose its captured
    projection at use; shared loans cannot gain mutation authority by widening
    the exclusive `receiver_aliases` match.
  - **Terminal calls in transition arms (TRANSITION-TAIL-CALL-TO-MACHINE):**
    terminal machine calls in attached Unit transition arms complete the caller
    for every result and operand form. Validate matching-host native trap and
    normal-return oracles for `calls/runtime_guarded_reference_return_crash` and
    `pass/calls/runtime_guarded_reference_return_before_crash_exit` on Linux
    ARM64, macOS ARM64, and Windows x86-64 while keeping linear/reference/guarded
    terminal-call controls (`pass/calls/runtime_linear_*_owner_disposition_exit`,
    `pass/calls/runtime_guarded_terminal_*_exit`,
    `pass/calls/runtime_*_reference_*_result_exit`, and
    `fail/calls/terminal_call_result_type_mismatch_rejected`) at their oracles.
  - **Aggregate construction, replacement, and claim-bearing results:** finish
    remaining runtime/case compositions and additional inner-loan capture or
    escape through opaque incoming referents, preserving `samples/cli/facts/replaced_field_loan`,
    `samples/cli/facts/case_payload_stored_loan`,
    `samples/cli/facts/descriptor_replacement_order`, and
    `samples/cli/facts/stored_exclusive_byte_view` across all nine places, plus
    the `pass/ownership/linear_*` and `fail/ownership/linear_*` controls.
  - **Saved observations, arithmetic, and patterns:** preserve the pinned float
    fixture pairs for `samples/cli/facts/exclusive_observation_exposure` and
    complete remaining case/generic/reference composition, preserving
    `samples/cli/facts/saved_observation_snapshot`,
    `samples/cli/facts/narrowed_integer_write`, and single-evaluation pattern
    destructuring (`pass/control_flow/runtime_nonplace_record_pattern_single_evaluation_exit`,
    `runtime_scalar_case_arm_evaluation_exit`).
  Acceptance: a composed caller tolerates renamed/reordered states and inserted
  computations; operands execute left-to-right once, skipped calls never run,
  and saved values, reference locals, and terminal transition calls agree
  between the Terminal interpreter and native execution (`omega run --both`).
  Explicit state transfers preserve custody without mandatory copies; implicit
  cross-state use, conflicting loans, source/path substitution, invalid escapes,
  missing provider authority, and stale snapshots reject. Keep `7 / 2 * 2 == 7`
  and `4097 / 4096 * 4096 == 4097` with their warnings, reject fractional
  integer landing, and keep typed truncation (`4097u32` gives 4096), signed
  quotient/remainder laws, and zero-divisor rejection.

- **BORROWED-STORAGE-RESTORATION.** (split-of:OMEGA-PRODUCT-COMPILER-SOURCE)
  Complete consuming-transform and replacement execution end to end under
  [ownership windows](wiki/spec/language/ownership.md#borrowed-storage-invariant-windows)
  and [borrowed-storage restoration](wiki/spec/terminal-psi/ownership.md#borrowed-storage-restoration)
  through `samples/cli/facts/recoverable_replacement_window` and the corpus
  `pass/ownership/runtime_*` / `pass/drops/runtime_*` suites (`omega run --both`).
  Owner: `OpenWindows::replace` in
  `omega-rust/psi/compilation/pipeline/04_typed-trees-to-checked-trees/src/execution/terminal_unit/borrowed_windows.rs`.
  - Qualify borrowed byte-storage state joins, referent windows, and
    `samples/cli/facts/recoverable_replacement_window` on Windows x86-64,
    Linux x86-64, and remaining bound native hosts without treating bounded
    byte-owner loans as pointer/length views.
  - Keep evaluation order, pre-RHS destination capture, disjoint siblings, and
    exact open-place agreement across recoverable failure,
    suspension/resume/cancellation (with **TR3-TR8**), and process-exit or trap
    abandonment (`pass/ownership/runtime_{borrowed,whole_borrowed}_window_crash_exit`
    and `fail/ownership/borrowed_window_crash_does_not_repair_return`); do not
    invent rollback.
  - Consumers: the product parser and **BUMP-ALLOCATOR-CANARY**.
  Acceptance: consuming transforms, replacement values, and repair on both
  branches execute with caller-visible updates and exact-once custody in the
  common Terminal core and all supported native targets (`python tools/corpus_gate.py --both --filter borrowed,window,carried_loan,recoverable`),
  without staging caller storage. Missing repair, early return, stale reads,
  overlapping loans, wrong-place repair, repeated extraction, and whole-owner
  cleanup reject (`fail/ownership/{whole_borrowed_*,borrowed_window_*,borrowed_storage_*}`).

- **ATOMIC-MEMORY-MODEL.** Complete atomic/fence semantics and native execution
  under [concurrency](wiki/spec/language/concurrency.md) and the
  [atomic vocabulary](omega-rust/psi/foundation/language-core/atomic_operations.md).
  Serial load/store/fetch/swap/decisive exchange already execute as single
  Terminal events (`omega-rust/psi/compilation/representations/terminal-psi/src/terminal_module/control_flow/atomic.rs`);
  Omega stage 00 retains location, orderings and coherence edges
  (`omega-rust/omega/compilation/pipeline/00_terminal-psi-to-abstract-operations/src/lowering/machine/operation/atomic_events.rs`),
  but its unique-reaching-write restriction is not a concurrent memory model.
  `samples/cli/facts/atomic_compare_exchange_once` completes all nine standard
  places (`omega run --both`, exit 0), while `pass/atomics/shared_receiver_atomic_store`
  is still rejected on Linux x86-64 and Windows x86-64 and
  `pass/atomics/atomic_global_order_operations` has no executable entry.
  - Target operations, selection/encoding and checked refinement for every
    admitted width, alignment and ordering. Swap/fetch return the
    instruction-observed prior, never a separate load. Preserve
    observing/non-observing and decisive/single-attempt behavior, failure
    ordering, `Uncommitted` custody and target retry-work attribution.
  - Source/Terminal event production for bounded atomic fields, non-observing
    single-attempt exchange, and the location-free fence event and its ordering
    through target refinement.
  - Checked `sequenced_before`, `reads_from`, `modification_order`,
    `synchronizes_with`, `happens_before` and `global_sequential_order`,
    fence synchronization and outcome-sensitive RMW obligations, keeping
    existing serial coherence. **TR3-TR8** gates the concurrent publication,
    fence-pair, global-order and compare-exchange contention witnesses.
  Acceptance: `shared_receiver_atomic_store` and `atomic_global_order_operations`
  (given an entry) execute via `omega run --both` on every matching host;
  missing or substituted relations, locations, orderings or outcomes reject in
  checking; concurrent witnesses run once **TR3-TR8** lands.

- **SAMPLE-CORPUS.** Close every maintained `samples/cli|gui|uefi` project
  through checked semantics, native publication for its authored targets, and
  documented exit/output on matching hosts under the
  [release matrix](wiki/drafts/reference/rust_compiler_completion.md#release-matrix).
  **SQUALR-HEADLESS** owns `samples/apps/squalr`; **CANARY-CORPUS** owns language
  fixtures; GUI product acceptance is **MACOS-APPLICATION-PUBLICATION**. This
  item keeps sample integration until each full program works, even when the
  capability repair has another owner.

  Gates: `python tools/sample_gate.py` (`--filter`, `--cli`, `--write-results`)
  and `python tools/sample_native_gate.py --cli <binary> --target <host> --jobs 1`.
  Host coverage still owed:
  - `std` `Clock` on Linux (`rendering/{bouncing_ball_2d,mandelbrot_zoom,bouncing_particles}`
    and `tests/omega/pass/host/std_clock_sleep_exit`,
    `tests/omega/pass/providers/kernel_import_result_chain_exit`) and remaining
    declared hosts for `algorithms/{binary_search_viz,sort_visualizer}`,
    `rendering/{bouncing_console,ripple_field,tick_marquee}`, and
    `simulation/{cellular_automaton,game_of_life_glider,langtons_ant,random_walk}`.
  - `arithmetic/stats_compute`, `cli_mvp`, `euclid_gcd`, `generic_counters`,
    `number_guess`, `basics/print_squares`, `recursive_sum`,
    `systems/framed_payload`, and `simulation/calendar` on both Linux hosts;
    `longest_run`, `parse_int`, `roman_numeral`, `string_hash`, and
    `substring_search` on Windows x86-64 and both Linux hosts.
  - `proofs/math_proofs`: ordinary multiset extraction and checked proof terms
    with a rejecting false twin.
  - Bounded Console input under [bounded input](wiki/spec/resources/bounded_input.md):
    exact prefix/destination, once-only effects, LF/EOF/Full, zero-capacity
    non-consumption, untouched tails, alias rejection.
  Acceptance: every maintained sample checks, every applicable native product
  builds, and documented runtime oracles pass on every required matching host.

- **CANARY-CORPUS.** (also covers **WINDOWS-CONSOLE-INPUT**,
  **MACOS-NATIVE-LOST-SUCCESSES**, **NATIVE-GOLDEN-LINUX-BOUND-UNMEASURED**, and
  **CORPUS-ROUTE-BLIND-SPOTS**)
  Close `tests/omega/{pass,fail,run}` at each fixture's promised stage under the
  [release matrix](wiki/drafts/reference/rust_compiler_completion.md#release-matrix),
  with `python tools/corpus_gate.py` (`--filter fail/` for diagnostics,
  `--native` / `--both` on each matching host). Harness:
  `omega-rust/omega/compilation/driver/tests/corpus_runner.rs`; attribution per
  [testing](tools/testing.md#attributing-a-move).
  - **Production-only runner and route coverage (CORPUS-ROUTE-BLIND-SPOTS):**
    the runner invokes production application/compiler/package/interpreter
    facades; delete `driver/tests/support/fixture_package_inputs.rs`,
    `console_acceptance.rs`, `dangerous_service_acceptance.rs`, and
    `process_exit_acceptance.rs`. Package fixtures run the real isolated
    update/review/lock workflow through package-manager APIs before ordinary
    compilation, and product requests/reports expose the selected targets
    without harness-side `build.omg` projection. Restore `--both` coverage on
    blind-spot routes (`pass/filesystem` and `pass/targets/per_target_import_satisfies`).
  - **Windows Console input and native customers (WINDOWS-CONSOLE-INPUT):**
    complete `omega run --both` package admission and execution on Windows
    x86-64 for `pass/calls/runtime_mutable_call_before_transition_args_exit/main.omg`
    and `pass/host/runtime_console_bounded_line*` (exits 20/13/34/22/70 with
    authored stdout), plus `pass/host/runtime_gui_foreground_window_exit`,
    `pass/control_flow/runtime_{integer,string}_literal_dispatch_exit`, and
    `pass/wire/runtime_wire_{exact_array_without_count,policy_authored_plan}_exit`.
  - **macOS lost successes and Linux golden measurement
    (MACOS-NATIVE-LOST-SUCCESSES, NATIVE-GOLDEN-LINUX-BOUND-UNMEASURED):**
    restore remaining `regressed-from:` check/build/runtime rows from
    `python3 tools/corpus_gate.py --list-pinned-regressions` (including
    `pass/arithmetic/closed_call_transition_return_proves_exact_sum_exit`,
    `pass/blockexec/blocking_executor_custody_claims_compile`,
    `pass/capabilities/windows_provides_import_exit`,
    `pass/modules/same_name_requirement_calls_exit`,
    `pass/concurrency/runtime_retained_reference_boundary_exit`, and
    `pass/text/runtime_mutable_string_parameter_concat_write_line`), and record
    `tests/omega/corpus_native_linux_x86_64.txt` on a Linux host at one
    identified revision (`python3 tools/corpus_scoreboard.py --cross-golden`).
  Acceptance: the complete roster reaches promised stages, negatives reject for
  their intended reasons, and exact runtime oracles (`--both` and `--native`)
  pass on matching hosts with observed (never copied) per-host golden records.

- **SQUALR-HEADLESS.** Run the independently versioned
  [Squalr application](samples/apps/README.md) (`samples/apps/squalr`
  submodule) through its ordinary nested packages to native geometry,
  supplied-byte scanning and repeat filtering. The application's board owns the
  1:1 Rust port; this item owns compiler integration and unchanged application
  acceptance.
  - Application-side gaps (in the submodule): connect expansion, ordering,
    string parsing, the six scalar `FixtureKit` scan/repeat checks, and
    production new-scan, element-scan, repeat, delete and list in
    `squalr-tests/main.omg` and `squalr-tests/src/command_driver.omg` before
    printing `PASS`; use raw-byte storage for true multi-byte overlap; restore
    the foreign-package `Optional<T>` APIs for `ScanFunctionScalar` and
    `SnapshotRegionFilter`; and fix live reads to add distinct recent-read
    presence/value.
  - Compiler integration frontier: `squalr-tests/main.omg` exits 0 with
    `Squalr geometry: PASS` under `omega run --both` on Windows x86-64 and
    under WSL Ubuntu. Remaining compiler repairs:
    - Reject scalar-cast to one-field record result mismatches in the checker;
    - Retire `Binding<WaitWake>` fields on `EnginePrivilegedState` task slots
      into exact `WaitWake<Role>` static slot demands in stage 04
      `execution/terminal_unit/control/call_results.rs`;
    - Windows byte output in `HostedWriteByteI32Realization`
      (`omega-rust/omega/compilation/backend/artifacts/native-artifact/src/physical/derivation/hosted_builtins.rs`);
    - Connect **BUMP-ALLOCATOR-CANARY** (**VEC-NATIVE-GROWTH**) and
      **PLAN-LAID-VIEWS** for partitioned scan output and repeat filtering.
  Acceptance: on Linux x86-64 with Python 3.11+, `python3
  samples/apps/squalr/tools/verify.py native --target linux_x86_64 --timeout 600
  --omega <binary>` after ordinary package review exits 0 with `Squalr
  geometry: PASS` after the restored checks, and scan/repeat addresses and
  ranges match pinned Rust and the independent expected cases (no/all matches,
  tails, empty input, overlapping candidates), keeping the 17-package/37-edge
  layout.

- **MACOS-APPLICATION-PUBLICATION.** Publish and run `samples/gui/window_app`,
  `window_demo` and `windowed_calculator` under the
  [macOS application contract](wiki/spec/build/macos_application.md). Reuse
  `omega-rust/omega/compilation/backend/artifacts/native-artifact/src/package_publication.rs`
  for package assembly and signing, and
  `omega-rust/omega/compilation/products/src/compilation_products.rs` for
  package/inner-executable accessors. Keep provider selection as named
  implementations and window state as explicit owned data.
  Acceptance: all three publish a validated `.app` and execute on macOS ARM64
  with observed windows/rendering, input/close behavior and completion
  (`window_demo`: 60 frames, exit 0), plus a separate Finder launch, in a
  desktop session with Screen Recording, event-posting and Accessibility access.

- **TERMINATION-RANKING-CHECKS.** (also covers **GENERAL-CYCLIC-EXECUTION** and
  **TPR6**)
  Carry exact rank/endpoint evidence, general cyclic control, and progress-subject
  provenance through ordinary calls, nested machines, state arrivals, and native
  execution (`omega run --both`) under the
  [termination contract](wiki/spec/language/termination.md) and
  [cyclic control](wiki/spec/terminal-psi/control_flow.md).
  Owners: stage 04
  `omega-rust/psi/compilation/pipeline/04_typed-trees-to-checked-trees/src/checks/termination/{ranking,progress}/`,
  `src/flow/value_origins.rs`, `src/execution/terminal_unit/`,
  `omega-rust/psi/compilation/checking/src/proof_contracts/contract_entailment/ranking_range/`,
  `omega-rust/psi/compilation/checking/src/machine_calls/call_cycles/runtime_ranking/`,
  stage 05 `src/unit/attached_unit/`, and
  `omega-rust/psi/interpreter/terminal-interpreter/src/terminators/`.
  - **Ranking and endpoint transport:** re-run the external-result and `std`-loop
    execution controls (`external_result_ranked_arrival_compile` currently
    refuses with `provider candidate requires beyond its boundary's declared
    obligations`) once **ASSUMED-BINDING-CONTRACTS**' Build/root
    trust-to-installation join lands. Keep quotient/remainder, shift, bitwise-AND,
    and mixed IEEE-requirement/integer-rank controls
    (`pass/termination/runtime_*_exit`, `pass/control_flow/termination_index_distance_compile`)
    at their oracles; rankless exits returning to recurrence, nondecreasing
    cycles, stale premises, overwritten requirement inputs, and unproved
    arrivals reject.
  - **General cyclic execution (GENERAL-CYCLIC-EXECUTION):** complete remaining
    cycle shapes in stage 04 arrival/custody checking and stage 05 lowering
    (broader qualified/partial ownership, content reshuffles/partitions, broader
    structural-result custody, boundary completion receipts, and claim transfers
    through Unit/scalar calls). Keep `pass/control_flow/runtime_cyclic_reference_and_claim_rotation_exit`,
    `pass/slices/runtime_guarded_view_copy_cycle_exit`,
    `pass/text/runtime_{number_to_decimal,bounded_carrier_write_read}_exit`, and
    `pass/collections/runtime_shared_{fixed_array_cycle,array_dot_product,array_polynomial}_exit`
    plus `fail/borrows/cyclic_arrival_duplicates_exclusive_reference` and
    `fail/ownership/{cyclic_arrival_restores_consumed_field,conditional_scalar_return_preserves_live_claim}`
    at their oracles across the hosted matrix.
  - **Progress-subject preservation (TPR6):** extend `src/flow/value_origins.rs`
    beyond single-origin tracing so projected/stored helper results, extra
    reference boundaries, and state arrivals with distinct local owned backing
    homes retain exact provenance for every selected origin (building on
    `samples/cli/facts/progress_selector/` across all nine places). Retire shape
    gates in `omega-rust/psi/compilation/checking/src/machine_calls/calls/expression_scanning/result_realization.rs`
    as real producers preserve custody.
  Acceptance: `python tools/corpus_gate.py --filter termination/,control_flow/`
  with `--both` and `--native` on matching hosts, `samples/cli/facts/progress_selector/`,
  and the cyclic samples (`array_max`, `dot_product`, `horner_eval`) pass with
  Terminal and native agreement; changed endpoints, stale copies,
  protected-carrier writes, overflowing intermediates, nondecreasing cycles,
  corrupt arrivals, and missing/stale/substituted progress provenance reject.

- **NESTED-MACHINE-UNIFICATION.** Implement the owner-ratified
  [single machine declaration model](wiki/spec/language/machines.md#nested-machines-and-identity)
  and [invocation contracts](wiki/spec/language/state_contracts.md), replacing
  source `state` with arbitrarily nested named machines. The spec/guide/sample
  README sweep precedes implementation; renamed prose is not compiler acceptance.
  This is one end-to-end outcome, not separate parser and backend handoffs.

  Resolve recursive lexical scopes and forward references; keep ordinary
  overload/shadowing rules and exact declaration/generic identities. Nested
  helpers are visible only in their enclosing declaration scope, not exported
  by a parent or accessible by qualification through a sibling. Reject runtime
  capture, enclosing-local use, hidden context parameters and executable code
  after trailing declarations. Permit value/Unit completion before declarations,
  not only transitions. Elaborate static declaration context, then use flat
  machine identities and explicit arguments downstream; no runtime nesting tree.

  Unify ordinary-call and terminal-transfer checking around the complete callee
  contract. Ordinary calls resume; terminal calls forward completion. Preserve
  exact actuals/snapshots, live facts and write invalidation, result/Unit
  postconditions, effects/markers, loans, invariant/hole restoration, cleanup
  and linear custody. Parent entry facts never become ambient helper premises.
  Remove state-only return, effect, ownership and progress shortcuts; joint SCC
  ranking covers nested/sibling/ancestor/top-level edges without self-justifying
  recursive evidence. Preserve proof-only non-tail recursion versus runtime
  tail-only cycles. Update Terminal production, codecs/checking, both interpreters,
  native lowering and WCSU together: nesting does not mandate a frame, and
  runtime cycles remain constant-stack with all live backing accounted for.

  Migrate affected bundled/product Omega source, samples and fixtures with each
  semantic change; remove the retired parser/declaration/dispatch paths, not a
  permanent compatibility mode. Add explicit result contracts where old states
  implicitly used their enclosing result. Retained IR basic blocks are not
  legacy source states.

  Acceptance: source-to-Terminal/native execution agreement (`omega run --both`)
  for three-level nesting, ordinary helper call with continuation,
  sibling/ancestor terminal cycles, generic enclosing declaration context and
  explicit borrowed/local data, across the applicable nine-place row
  (`other_state/` becomes the nested-machine place) and fact-sensitive reject
  twins. Reject inaccessible descendants, accidental capture, stale parent
  facts, mismatched results, missing exact call markers, unclosed restoration,
  invalid loan escape, lost linear custody, unjustified recursion and retired
  `state` syntax. Demonstrate bounded physical stack on a long cycle, including
  live borrowed backing and completion cleanup, and finish the full affected
  sample/corpus migration and both compiler implementations before closure.

- **DOMAIN-DEFAULT-PREDICATE-UNIFICATION.** Implement the owner-ratified
  [predicate subject](wiki/spec/language/domains.md#declaration-and-membership)
  and [case-dependent default domains](wiki/spec/language/data_and_literals.md#case-constraints).
  `self` denotes the exact constrained value only in data `where` and domain
  `requires`. Remove explicit domain subject binders and case-level `where`,
  including their parallel internal predicate paths; machine/state receivers
  and `Self` stay rejected. No data subject-parameter syntax is introduced.
  Migrate library, samples and fixtures in the same implementation change.

  Carry one general predicate representation through formation, construction,
  calls, matching, coverage, writes and erasure. Case membership plus the
  established domain must recover type equality and guarded payload bounds;
  merely recognizing a constructor or accepting a declaration is insufficient.
  Do not assume a predicate's own goal to form its payload access or helper
  call. Keep recursive/indexed data support and its checked encoding obligations,
  exact subject identity, first-case zero gating and mutation restoration.

  Acceptance: execute a generic typed extractor over Integer/Boolean and a
  guarded Range payload consumer through native and Terminal Psi (`omega run --both`),
  exercising the same predicate as a default domain and a named generic domain
  through all nine fact places. Reject mismatched case/type construction,
  unguarded inactive payload access, reversed bounds, stale facts after writes,
  invalid zero establishment and the retired syntax.

- **CANONICALIZE-SCALAR-RANGE-CONTRACTS.** Complete the settled
  [proposition-based scalar range contract](wiki/spec/language/domains.md#declaration-and-membership)
  and [generic inference rule](wiki/spec/language/generics.md) end to end:
  common fields use data `where` (including guarded payload predicates),
  machine/state parameters `requires`, results `ensures`, locals the facts from
  initializers, guards and calls. Remove integer and float `T [lo..hi]` /
  `T [lo..=hi]` suffixes after their customers migrate; ordinary ranges, slices,
  arrays and bracketed multiplicity stay.
  - **Design constraints:** a domain kept by a migration needs an independently
    meaningful nominal identity, semantic contribution or establishment route
    (endpoint-named or interval-only replacement domains are invalid); flow
    narrowing never selects specialization or layout identity; do not rebuild
    `proven_predicates_grant_domain`
    (`omega-rust/psi/compilation/pipeline/04_typed-trees-to-checked-trees/src/checks/contracts/calls.rs`).
  - **Field/case propositions and recasts:** carry field/case propositions
    through construction (`checking/src/proof_contracts/contract_entailment/construction_predicates.rs`),
    nonliteral and indexed writes, reads, calls, state joins, mutable recasts
    ([the representation judgment](omega-rust/psi/compilation/checking/recasts.md)),
    and returns, preserving the audited inventories in
    [range_suffix_migration.md](wiki/drafts/designs/range_suffix_migration.md)
    and running `pass/float/named_integer_conversion_source_proofs_exit` (exit
    78) on Linux/Windows.
  - **Provider-dependent clause endpoints and remaining customers:** close the
    three remaining `requires <param> <= call()` range-suffix sites
    (`pass/providers/provider_const_endpoint:26`,
    `pass/comptime/provider_boundary_range_endpoint_exit:33`,
    `fail/comptime/recipe_endpoint_membership_refused_at_closure:26`) by adding
    a contract-expression work-root class and fold-publication site in the
    constant-recipe machinery feeding `constant_recipe_obligations.rs` against
    `closed_constant_recipe_value` (without widening `ConstantPredicateObligation`).
    Migrate the 16 recursive filesystem parameter suffixes with
    **FILESYSTEM-PLATFORM-SERVICES**, and qualify remaining collection/slice
    view and capture compositions (`samples/cli/probes/slice_view_whole/main.omg`,
    `pass/dependent/runtime_symbolic_slice_{cell_theorem,predicate_store}_exit`,
    and `samples/cli/facts/ieee_guard_i32_from_f64`).
  Acceptance: spec/guide/compiler docs, bundled libraries, samples, `source/psi`,
  the corpus, `bootstrap/5_omega/parser.epsilon` and
  `tests/bootstrap/omega-parser` are migrated and each customer's native/Terminal
  oracle runs via `omega run --both`; then delete suffix parsing
  (`omega-rust/psi/compilation/pipeline/01_tokens-to-syntax-trees/src/type_syntax/parse_type.rs`),
  `TypeConstraintNode::Range`, range-shell inference, suffix-recommending
  diagnostics, and the migration recipe, turning
  `fail/proofs/mathematical_constrained_scalar_formal:2` into a parse-refusal pin.

- **DOMAIN-ACCESS-AUTHORITY.** (also covers **DOMAIN-ISSUER-ROUTES**)
  Implement the settled
  [domain read/write policy](wiki/spec/language/domains.md#domain-owned-read-and-write-authority)
  (`read by` and `written by`, independent of `established by`) and
  [requirement, exact-machine and boundary establishment](wiki/spec/resources/authority.md#requirement-and-exact-machine-routes)
  end to end through `samples/cli/facts/` witnesses and `omega run --both`.
  - **Read/write authority and controlled cells:** carry normalized exact route
    identities and subject-bound access restrictions through the common
    permission/loan model without extra runtime permission fields or a changed
    carrier layout. The guide's
    [controlled cell](wiki/language_guide/chapter_8_domains.md#read-and-write-authority)
    is the first witness (public structure, authorized read and write-only
    update, writer delegating to an unlisted helper).
  - **Qualification restoration, temporary views, and scalar reads:** complete
    post-update qualification restoration under
    [invariant-window rules](wiki/spec/language/dependent_values.md#invariant-windows)
    and [membership evidence rules](wiki/spec/resources/authority.md#membership-and-boundary-evidence)
    (retaining `samples/cli/facts/additional_input_domain_mutation` and
    `samples/cli/facts/non_owning_reference_erasure`),
    [temporary qualified-view retirement](wiki/spec/language/domains.md#temporary-qualified-views)
    across loan cohorts, and scalar reads/results under
    [copy and publication rules](wiki/spec/language/domains.md#establishment-transport-and-helper-calls).
  - **Portable issuer routes and forwarding (DOMAIN-ISSUER-ROUTES):** extend
    same-project checked ordinary issuance/forwarding to cross-package private
    routes and selected structural payloads in
    `omega-rust/psi/compilation/pipeline/05_checked-trees-to-lowered-psi/src/unit/attached_unit.rs`
    and `.../attached_unit/catalog.rs`, retaining ordinary callable and
    selected-conformance identities with exact invocation/result correspondence
    (preserving `samples/cli/facts/scalar_domain_call_guarantee`,
    `samples/cli/facts/structural_issuer_forwarding`, and
    `samples/cli/facts/linear_routed_label`). Bind an executable entry for
    `tests/omega/pass/domains/bodyless_owner_establishment` and exercise
    admitted provider issuance for core's protected `Vacant`/`Resident` routes
    with **BUMP-ALLOCATOR-CANARY**.
  Acceptance: the controlled cell, `bodyless_owner_establishment` (issuer →
  forwarder → consumer), and the nine-place fact rows execute via
  `omega run --both` across nested fields, whole-place replacement, requirement
  dispatch, helper calls, reflection, and codecs; unauthorized reads/writes,
  missing/forged/rebound issuer evidence, same-spelled issuer substitution,
  writer-only routes reading through `&mut`, wrong-subject or escaped helper
  grants, and bypass through bare helper parameters, explicit domain erasure,
  byte views, or atomic recasts reject.

- **NOMINAL-FIELD-FLOW.** Close the Dungeon Crawler's field-domain execution and
  [representation-erased domain qualification](wiki/spec/language/domains.md)
  end to end under
  [mutable arguments and declared field domains](wiki/spec/language/dependent_values.md#mutable-arguments-and-declared-field-domains).
  Owner: `omega-rust/psi/compilation/pipeline/04_typed-trees-to-checked-trees/src/semantic/facts/field_domains.rs`
  and `src/flow/state_values/`.
  - Restore representation-erased domain qualification: `[u8; N] in D` is the
    same exact-length fixed array as `[u8; N]`, with `.len == N`. Delete the
    `has_domain => BoundedOwned` source-shape classifier in
    `checking/src/value_custody/reference_shells.rs` and every downstream
    assumption that domain presence supplies live-length storage; migrate text
    and Dungeon Crawler values needing inline capacity plus live length to
    `FixedVec<u8, N> in Utf8` (or `Vec<u8> in Utf8` where growth is intended).
  - Qualify the unchanged Dungeon Crawler and remaining domain/proof
    compositions across all declared hosts while retaining
    `pass/domains/{qualified_mutable_field_exit,threaded_subject_keeps_established_domains}`,
    `pass/borrows/{indexed_record_field_replacement_exit,qualified_byte_owner_exclusive_call_exit,branch_selected_record_composed_receiver_exit,returned_record_case_field_store_exit,interleaved_parameter_field_replacement_exit}`,
    and `fail/text/byte_field_snapshot_domain_widening`.
  Acceptance: `samples/cli/probes/{write_only_domain_store,ascii_indexed_write}`,
  the 71 migrated text fixtures, and the Dungeon Crawler execute via
  `omega run --both` on all declared hosts with their authored content/output
  oracles; invalid arrivals, unclosed invariant windows, stale aliases, and
  wrong UTF-8/ASCII writes reject.

- **CRASH-CONTRACT.** (also covers **OUTCOME-SPECIFIC-CALL-EVIDENCE**)
  Complete invocation-specific crash obligations and outcome-specific call
  evidence through selected operators, nested values, calls, cycles, and package
  review end to end (`omega run --both`).
  Owners: stage 04 `omega-rust/psi/compilation/pipeline/04_typed-trees-to-checked-trees/src/facts/{operator_crashes,crash_entry_values}.rs`,
  `src/values/scalar/contract_entry.rs`, `src/flow/expression.rs`, stage 05
  lowering of crash routes, and the native consumers.
  - **Selected-use joins and entry-operand normalization:** extend selected-use
    joins for remaining intrinsic spellings, generic telescopes, open const
    guards in generic provider bodies, and provider-backed structural calls
    (building on `samples/cli/facts/selected_crash_formal`,
    `samples/cli/facts/indexed_crash_guard`, and
    `samples/cli/facts/crash_entry_{representation_view,boolean_view,ieee_view,signed_zero,nan_equality}`
    across all nine places, plus `pass/operators/crash_routes` and
    `pass/contracts/runtime_float_*_crash_guard_exit`). Complete nonempty-path
    lengths, policy casts, and entry provenance for opaque/content leaves,
    dynamic indices, and whole-collection reads after writes with proven origins.
  - **Outcome-specific call evidence (OUTCOME-SPECIFIC-CALL-EVIDENCE):** extend
    outcome-specific guarantees beyond scalar/result/reference-free structural
    projections (`run/calls/outcome_specific_selected_guarantee_*_exit` and
    `fail/calls/outcome_specific_witness_on_nonmatching_exit`) to entry claims,
    other contract rows, crash routes, borrowed/reference-bearing structural
    inputs, computed dependent argument substitution, and broader witness scope
    transport.
  - **Executed outcomes and ordered completion:** ensure actual outcome,
    substituted guards, abandoned claims, staged writeback, no-result/no-cleanup
    behavior, remaining qualified results, state/control contracts, compound
    arithmetic, and nonliteral wrapping operands execute identically in both
    engines.
  Acceptance: nine-place fact rows and corpus fixtures for each extended crash
  and outcome-specific route execute via `omega run --both` through package
  review; changed guards, captures, substitutions, nonmatching exit witnesses,
  and stale writes reject.

- **ARITHMETIC-POLICY-REALIZATION.** (also covers **IMPLICIT-STORE-CONVERSION**)
  Close remaining [numeric-policy](wiki/spec/language/numeric_values.md)
  realization and the implicit-store-conversion rejection and migration under
  [explicit coercion/erasure](wiki/spec/language/domains.md#exact-coercion-and-erasure)
  end to end (`omega run --both`).
  Owners: `omega-rust/psi/compilation/checking/src/proof_contracts/arithmetic_domains/{assignments,operand_reports}.rs`,
  stage 04 scalar checking, stage 05 expression preparation, and
  `omega-rust/psi/compilation/representations/terminal-psi/src/terminal_module/control_flow/trapping_integer.rs`
  with its interpreter and native consumers.
  - **Numeric conversions and Trapping realization:** qualify the 16
    `pass/core/numeric_*` fixtures reaching all 170 public machines of
    `source/library/core/numeric_conversion.omg` on Linux x86-64 (preserving
    Windows and macOS execution and `pass/core/runtime_guarded_trapping_conversion_exit`
    / `fail/core/guarded_trap_ceiling_*_rejected`).
  - **Implicit store conversion and source migration (IMPLICIT-STORE-CONVERSION):**
    every cross-carrier/policy store in maintained pass fixtures, samples
    (**SAMPLE-CORPUS**), bundled libraries, and `source/omega/main.omg` with
    `source/psi/{lex,parse,source}` spells its conversion explicitly (`as`
    requires unchanged denotation and representability; wrapping/clamping use
    `core/numeric_conversion.omg`), producing one actionable diagnostic per
    authored conversion error.
  Acceptance: comparisons, conversions, and policy-explicit stores agree between
  the interpreter and matching-host native execution across all six relations,
  operand orientations, widths, signedness, and extrema above `i64::MAX`
  (`python tools/corpus_gate.py --both --filter numeric_,runtime_explicit_call_,runtime_shift_count_domain_exit,runtime_comparison_signedness_exit,runtime_wrapping_operand_truncation_exit`);
  `tests/omega/fail/arithmetic/implicit_{store_conversion,store_widening,store_call_result_conversion,call_operand_conversion,call_result_policy_change,integer_policy_erasure}_rejected`
  and `fail/slices/signed_slice_index_upper_only` each report their intended
  refusal.

## Universal interpreter cutover

Move executable Omega computation to one Psi-owned core consuming Terminal Psi
(`omega-rust/psi/interpreter/terminal-interpreter/`), governed by the
[pipeline](omega-rust/pipeline.md),
[semantic evaluation](wiki/spec/language/evaluation.md), and
[embedding](wiki/spec/build/embedding.md). Do not introduce a third IR, a third
engine, a tree-input execution mode, or a wrapper around both old engines.
Establish the minimum build, check, and run development seed, delete tree
execution (`omega-rust/psi/interpreter/checked-interpreter/`), and restore
compile-time, Build, CLI/corpus, and embedded execution through the common core.

- **EARLY-TERMINAL-JOBS.** (new-scope; also covers **INTERPRETER-CORE-ENTRY**,
  **TERMINAL-CONST-SCALARS**, **TERMINAL-CONST-PROVIDERS**,
  **TERMINAL-CONST-STRUCTURES**, and **TERMINAL-LAYOUT-WIRE-EVALUATION**)
  Route direct interpretation, embedding, and demanded compile-time evaluation
  (const scalars, provider-selected constants, owned structural snapshots, and
  layout/wire/proof-value queries) through one shared Terminal program/instance
  lifecycle (`TerminalProgram::load`) and the shared Psi 05-07 production
  operation end to end (`omega run --both`).
  Owners: `omega-rust/psi/interpreter/terminal-interpreter/src/{program,instance,execution,adapters}.rs`,
  `omega-rust/psi/compilation/compile-time/src/{machine_execution,const_evaluation,layouts}/`,
  and `omega-rust/psi/compilation/driver/terminal-production/src/`.
  - **Common core entry (INTERPRETER-CORE-ENTRY):** expose one interpreter
    facade accepting the borrowed Terminal artifact view, exact provisions, and
    host resources without checked-tree dependencies or compiler-owned source
    lookup; reject missing, wrong-signature, and wrong-instance imports before
    effects and preserve fuel pause/resume without repeating effects.
  - **Closed early jobs and constant evaluation (EARLY-TERMINAL-JOBS,
    TERMINAL-CONST-SCALARS, TERMINAL-CONST-PROVIDERS,
    TERMINAL-CONST-STRUCTURES, TERMINAL-LAYOUT-WIRE-EVALUATION):** fixed-array
    jobs now use the shared producer and execute through a Terminal instance with
    admitted primitive numeric imports, call-local result policy, and Terminal
    semantic fuel; remove remaining private arena cloning/rechecking and
    admission projections, and migrate the remaining early-evaluation consumers
    (scalar lengths/ranges/initializers, provider-selected constants via
    **PROVIDER-SELECTION-AFTER-TERMINAL**, owned record/case/array/reference
    snapshots, and layout/placed-view/wire consumers via **WIRE-CODEC-LIBRARY**).
  Acceptance: source guards forbid `SyntaxTrees`/`TypedTrees` cloning and
  private `lower_typed_trees` calls in compile-time evaluation; peak memory
  drops on a package with shared constant dependencies;
  `pass/comptime/{runtime_const_array_length_transitive,runtime_const_measured_recursion,range_endpoint_requires_discharge,selected_float_array_length,provider_boundary_range_endpoint}_exit`,
  `pass/build/const_index_dependency_order_exit`,
  `pass/generics/runtime_const_data_machine_call_exit`,
  `pass/providers/checked_boundary_operator_const_application_alternate_exit`,
  `pass/layouts/{runtime_plan_laid_value_field,runtime_plan_laid_nested_fixed_array_mutable_write}_exit`,
  and `pass/wire/runtime_wire_policy_authored_plan_exit` execute through the
  Terminal core (`omega run --both`), while
  `fail/comptime/{negative_const_array_length,effectful_const_array_length,recursive_accumulator_requires_coupling}`
  and escaping-reference/malformed-policy controls reject.

- **TERMINAL-BUILD-SEED.** (new-scope; also covers **TERMINAL-BUILD-QUERIES**,
  **TERMINAL-BUILD-EFFECTS**, **BUILD-PRODUCT-REFERENCES**,
  **BUILD-SNAPSHOT-OUTPUTS**, and **BUILD-ADMISSION-CHECKPOINT**)
  Execute package-aware `build.omg` activation, product-reference selection,
  build queries, restricted-build consent, and captured-input/committed-output
  staging once in the Terminal interpreter under
  [scoped build execution](wiki/spec/build/scoped_execution.md) and
  [restricted-build acceptance](wiki/spec/packages/acceptance.md#restricted-build-acceptance).
  Owners: `omega-rust/omega/compilation/build-evaluation/src/`,
  `omega-rust/omega/packages/{build-declarations,compilation-inputs,source-acquisition,build-output,package-manager}/`,
  and Psi compile-time admission.
  - **One-entry Terminal Build activation and provider suspension:** retire
    static syntax projection in `omega-rust/omega/packages/build-declarations`
    as a second Build language; designated `Build::bind` rows now lower through
    ordinary boundary calls, settings readback projects admitted settings through
    the instance catalog, `receiver_view_writeback_exit` executes mutable
    boundary calls across fuel pauses, Terminal's fuel meter pays invocation and
    shared sponsor accounts, and byte backing owners reserve before allocation
    with shared view leases and copy-on-write/staging transfers. Scalar and
    structural runtime tables reserve new rows and overlapping copies, carry
    leases with frames/joins, and retire removed cells; source-produced Build
    exercises exact and short cell budgets across fuel pauses. Root and saved
    call headers now reserve before creation; one frame-capture owner preserves
    scalar/structural/cleanup custody and retires saved headers on return.
    Packed scalar-array backing now retains element reservations through guest
    snapshots and result inspection, including after instance close; plain
    projection copies share that snapshot operation. Frame-local descriptors now
    retain row reservations through call staging, suspension, joins and returns;
    refused return installation retains outgoing backing and continuation custody.
    Shared scalar record-field rows now retain reservations through constructor,
    case and subtree staging, owned call copies, entry binding and retirement;
    exact/short-budget source controls preserve independent copied contents and
    sticky refusal. Shared primitive backing and entry lookup rows now retain
    reservations through construction, mutation, subtree staging and retirement;
    exact/short-budget controls preserve original borrowed entry writeback.
    Shared reference, byte-field, byte/view descriptor, backing, case and host-atom
    rows now use the same owner through execution and constructor/transplant
    staging. Publication funds and validates receiving rows before retiring
    sources, including activation split/merge result cases. Exact/short-budget
    source controls exercise cases, byte fields and claim division through fuel
    pauses, sticky refusal and teardown. Activation ledgers, live claims and
    descriptor/owner rows now retain reservations through construction, transport,
    division and retirement; division stages both outcomes before source custody
    changes. Provider allocation/resident records and descriptor/profile rows
    now reserve through entry binding, constructor transport and lifecycle
    completion before byte writes or source retirement. Source-produced resident
    initialization/extraction preserves original backing through borrowed/loaded
    execution, exact/short budgets, fuel pauses and handback. Compiler-owned
    initialization/extraction image payloads now reserve before allocation through encoding,
    nested/array staging and decoding; caller-owned codec scratch removes the
    hidden duplicate image. Exact/short payload controls separately refuse
    initialization and extraction without publishing custody. Provider field-access
    transfer windows use the same payload owner through the single external
    write; caller-private scalar staging removes the duplicate codec image and
    scalar decoding uses bounded inline storage. External read/write source
    controls cover exact/short budgets, borrowed/loaded artifacts, fuel pauses
    and sticky refusal. Root result publication now charges the shared cumulative
    allowance from actual retained rows/backings, preserving failed custody and
    avoiding repeat charges on cached observations. Scalar-case inspection shares
    funded payloads; byte/view results retain their backing until instance close.
    Source-produced result controls cover exact/short quotas, sibling invocations,
    shared/distinct byte backing, literal windows, cases and packed arrays.
    Explicit host loan returns still require an admitted loan-carrier interface;
    accounting does not bypass that existing refusal.
    Connect those admitted operations, remaining live-storage accounting, remaining
    host operations, and `suspend Build::await_provider<R>(builder, alias)`
    park/resume to the production Build scheduler, replacing checked-tree Build
    evaluation.
  - **Non-executing product references and Build queries
    (BUILD-PRODUCT-REFERENCES, TERMINAL-BUILD-QUERIES):** keep `bind`,
    `select_provider`, `assume_binding_contract`, test registration, and
    product description/schema queries inert during Build and resolve them at
    the final join (`pass/build/{described,same_leaf}_runner_entry_{left,right}_exit`,
    `pass/build/runtime_depend_mapping_exit`, and
    `fail/build/{product_entry_*,program_entry_binding_outside_build,root_binding_foreign_helper_private_entry}`).
  - **Granted Build effects, admission checkpoint, and snapshot outputs
    (TERMINAL-BUILD-EFFECTS, BUILD-ADMISSION-CHECKPOINT,
    BUILD-SNAPSHOT-OUTPUTS):** join restricted-build requests and lock
    decisions before any Build effect (`omega-rust/omega/packages/package-manager/src/review/restricted_build_grants.rs`),
    and stage generated sources, nested dependencies, and committed outputs
    with atomic capture/recapture isolation (`pass/build/artifact_only_required_output_completion`
    and `fail/build/artifact_only_{rollback_overlay,unsealed_output}_refused`).
  Acceptance: on Windows, macOS, and Linux, `mbx check -p omega`, valid seed
  check, rejection of
  `tests/omega/fail/arithmetic/array_constructor_divisor_requires_nonzero/main.omg`,
  and package-aware
  `omega run tests/omega/pass/float/named_provider_directed_divide_exit/main.omg`
  (exit 70) succeed through the Terminal core; helper/conditional dependency
  requests, `await_provider` park/resume, recursive dependency Builds, product
  description queries, restricted-request stop/resume, and generator-package
  snapshot outputs pass their positive and negative controls without executing
  product bodies during Build or publishing partial outputs.

- **UNIVERSAL-INTERPRETER-CLOSURE.** (new-scope; also covers
  **CHECKED-INTERPRETER-DELETED** and **TERMINAL-RUNTIME-DRIVERS**)
  Delete `omega-rust/psi/interpreter/checked-interpreter/` immediately after
  **TERMINAL-BUILD-SEED** closes, restore hosted CLI/corpus/embedded Terminal
  runtime drivers (`omega-rust/omega/application/terminal-runtime`), and close
  the universal cutover with one execution core and no executable tree visitor
  end to end (`omega run --both`).
  Owners: `omega-rust/psi/interpreter/`,
  `omega-rust/omega/application/terminal-runtime/src/execution.rs`, and
  `source/psi/` + `source/omega/`.
  - **Tree-interpreter deletion (CHECKED-INTERPRETER-DELETED):** remove
    `omega-rust/psi/interpreter/checked-interpreter/` and all production
    dependency edges, expression walkers, and frame/store machinery; report any
    remaining unimplemented caller as an explicit capability gap before effects.
  - **Source-free Terminal runtime drivers (TERMINAL-RUNTIME-DRIVERS):** finish
    the source-free Terminal artifact execution boundary in
    `omega-rust/omega/application/terminal-runtime` (consuming retained
    provisions from `native-artifact` without recompiling source or rerunning
    Build), completing scalar entry arguments, placed/aggregate/atomic host
    loans, case replacement, clock/descriptor imports, explicit standalone
    `process_exit` delegation, and architectural trap presentation across
    Windows, macOS, and Linux.
  - **Release and product-source closure:** audit retired crate consumers,
    verify `source/psi/` and `source/omega/` against the same job/value/effect
    contract, and promote all migration-designated canaries in
    `tests/omega/interpreter_migration.json`.
  Acceptance: source/dependency audits find one dispatcher/call/store/resource
  implementation and no `checked-interpreter` or executable tree visitor;
  public `omega run --both`, source-free Terminal artifact execution, and
  corpus/sample controls pass on Windows, macOS, and Linux with independent
  stdout/stderr/exit/trap oracles and pre-effect import rejection.

## Pipeline route

Follow one connected stage chain under the
[pipeline](omega-rust/pipeline.md#one-driver-one-pass-per-target-realization)
and [multi-target contract](wiki/spec/build/configuration.md#multi-target-compilation):
Psi checks every target's bodies once, Build evaluates once, and `--target`
narrows realization only.

- **PIPELINE-ARCHITECTURE-ENFORCEMENT.** (new-scope; also covers
  **BUILD-EVALUATES-ONCE** and **ONE-DRIVER-PER-STAGE**)
  Enforce the connected single-pass Psi (`00–07`) + single-pass Build +
  per-target Omega (`00–09`) pipeline architecture and workspace dependency
  topology in [the pipeline contract](omega-rust/pipeline.md) end to end.
  Owners: `omega-rust/omega/compilation/driver/src/compiler.rs`,
  `omega-rust/psi/compilation/driver/`,
  `omega-rust/omega/compilation/native-production/`, and workspace architecture
  gates (`tools/check_native_boundary.py` and `cargo metadata` guards).
  Current migration frontier: authored Build prepares its own import closure
  before Product imports, retaining the captured namespace and parse prefix for
  a distinct post-join Product context. Application package compilation and review
  now begin, step and complete opaque Build invocations without holding a compiler
  session borrow; interleaved compilation preserves the paused activation. The
  activation retains its full request ledger and completion checks its exact code
  owner. Acquired unbound source can now run Build before package identity exists,
  then join a matching named owner before Product typing. Normal return retains
  the executed declaration; package management binds its name to captured source
  lineage without a syntax projection or provisional key. The SDK runs a
  helper-computed declaration through native and Terminal execution after live
  source deletion, with an unselected missing dependency never requested.
  Actual returned dependency requests now drive package management's retained
  source traversal. The two-package SDK witness acquires and executes only the
  selected local dependency, joins both captured source owners and completes
  native/Terminal execution after live-source deletion without replay. Missing,
  wrong-scope/ordinal/package selections, substituted graphs and repeated joins
  reject; private dependency machines remain inaccessible. Captured dependency
  bytes no longer imply root-only Build metadata authority. Reconciliation keeps
  its scope/alias/cycle/limit checks; compiler completion refuses any unjoined
  request. Pre-acquired source inputs retain the exact request value, purpose,
  ordinal and selected source; bound compilation rejoins those receipts against
  the executed ledger rather than treating an alias graph as settlement. The
  same validation serves post-execution acquisition, and changed locators still
  refuse with unchanged aliases. Project preparation retains recovery, lock selection
  and concurrent-edit checks across application-owned source execution; its
  begin/finish boundary accepts the actual acquired graph without parsing Build
  declarations. Application source discovery now drives the executed traversal
  through the same execution loop as compilation/review, retaining invocations
  for final completion rather than replay. Repeated local requests reuse their
  first acquired source and Build while every graph edge still reconciles. The
  SDK proves two aliases share one dependency activation, including after its
  live files are deleted, and runs the application route through native/Terminal
  completion with private-call rejection. Discovery, compilation and review share
  one application Build session; the compiler remains application-owned while
  package management retains staging and aggregate sponsors. Review callbacks
  borrow that compiler across passes instead of storing it in package resources.
  Source work cannot replenish a spent result allowance or start after closure.
  The captured executed graph passes subsequent admission with the same sponsor
  identity after live deletion. Completed Builds can select multiple targets
  with equal source/semantic inputs, retaining the completed configuration and
  each occurrence's execution observations. The SDK proves one Build and one
  Product source completion produce two portable targets plus host native/Terminal
  execution, isolating a missing-entry target refusal. Ordinary application
  execution and focused review/preparation checks pass; transaction exclusion
  and refusal ordering remain enforced. Production CLI discovery still consumes the
  static graph; it must consume these retained Builds across review and target
  occurrences. Final Product context now chooses its target at source join;
  the SDK completes a foreign-target dependency graph after host discovery,
  then selects and executes the host product. Pre-admitted targets remain fixed.
  Build settlement is now a separate opaque result before Product preparation:
  source-request/output settlement consumes the actual return, while independent
  Product contexts resolve and type from the same captured input under their own
  admitted semantic bindings. The SDK exercises accepted review and executable
  contexts after live-source deletion, refusing missing entry acceptance and a
  replaced request graph without replay or additional Build fuel. Default Builds
  reprepare changed compilation purposes from retained parsing. The ordinary
  composite route consumes this same sequence; this phase boundary does not yet
  complete the production retained-discovery cutover above.
  Execution-time profile, staging, snapshots/outputs, sponsors and grant guards
  now travel as one owned `BuildExecutionInputs` from package admission through
  the compiler facade into Build evaluation; repeated field-by-field forwarding
  is removed and compilation defaults resolve in one place. The sliced generator
  retains its refusing grant guard through normal return and releases it only
  after settlement, before Product checking. Production admission still needs
  the retained scheduling cutover: issue those inputs before Build, never drop
  them while converting old occurrence requests to post-return Product work.
  Terminal Build now executes captured Source reads,
  private Output writes/sealing, required-output completion and generated-source
  handoff through source-issued ordinary boundary calls. The generator SDK
  retains exact bytes and six I/O observations across fuel pauses; forged facets,
  unsealed outputs and pre-effect budget exhaustion refuse. Filesystem vocabulary
  and output records no longer belong to the checked-tree interpreter, and
  filesystem reach follows the exact protocol rather than helper-name guesses.
  Anonymous source custody now survives frontend preparation into Build's
  execution scope, without a provisional declaration owner or a second live
  capture. The same generator executes after its live source files are deleted,
  then joins its executed package declaration and checks Product source. Explicit
  narrowing still excludes unselected members and refuses an omitted requested
  root or Build import before effects; caller-required outputs remain mandatory
  at settlement.
  The application source-resolution route retains this generator through Product
  completion without replay. Its single request now carries the root snapshot
  before execution, preserving scoped source selection, named immutable inputs
  and caller-required outputs. Missing slots and omitted outputs still refuse.
  Dependency input assignment remains unimplemented: the removed edge-map carrier
  had no writer or constructor input, so its routing and validation only ever
  processed empty maps. Implement the specified pre-execution assignment and
  shared-node conflict checks at actual dependency activation admission; never
  inherit the root's slots. Required-output path byte results remain explicitly
  refused.
  Ordinary one-shot compiler calls still own their execution loop.
  Root Git requests now use package-owned acquisition before executing their
  declarations, retaining authenticated lineage/commit/tree rather than treating
  snapshots as external-local packages. Acquisition policy belongs to its exact
  originating project request; two aliases share the first Git source and Build.
  The Git SDK executes native/Terminal exit 0 after moving the selector and
  deleting the live project sources, with offline-unaccepted, foreign-policy and
  private-call refusal controls. Executed and transitional static acquisition
  now share the accepted-pin/offline selection rule. Complete accepted-lock
  round-trip qualification, named workspace and Git-relative Path acquisition;
  preserve
  review/lock/offline recovery while replacing static Build projection,
  provider readiness/resumption, and demanded Product-description queries; this
  is not yet the completed production cutover.
  - **One Build evaluation and one Psi chain per compilation
    (BUILD-EVALUATES-ONCE, ONE-DRIVER-PER-STAGE):** `build.omg` evaluates once
    per compilation, observes no realized target, and produces checked rows for
    every target (candidate review now completes opaque Build continuations,
    compiler evidence owns semantic-binding nomination, and designated bindings
    lower to ordinary boundary calls; production description queries and Terminal
    Build/helper activation scheduling remain to migrate); `compiler.rs` runs Psi
    `00–07` once and produces one canonical Terminal Psi artifact, then runs
    Omega `00–09` and emission once per requested target without
    checked-tree/lowered-Psi sidecars.
  - **Topology, facade, and producer-record enforcement:** enforce via
    `cargo metadata` and source guards that Psi never depends on Omega, Omega
    pipeline/realization never depends on source/typed/checked Psi or
    interpreters, numbered stages are called only inside the Psi/Omega
    coordinators and stage tests, `name.as_str() == "string"` spelling checks
    are replaced by representation classes, and 256 MiB thread stacks are
    replaced by arena-handle worklists on platform default stacks.
  Acceptance: a two-target integration fixture proves with invocation counters
  that source/Psi `00–07`, Build, and Terminal publication each execute once and
  Omega `00–09` executes once per requested target, producing bytes equal to
  single-target compilation while isolating target-specific realization
  failures (`tests/omega/pass/build/nested_generated_dependency_dual_purpose`,
  `scoped_build_stops_before_unaccepted_effect`,
  `artifact_only_unsealed_output_refused`, and
  `captured_source_generated_constant` stay green); workspace architecture and
  stack-size guards pass.

- **PSI-TARGET-FAMILIES.** (new-scope) Target-scoped machine declarations
  survive every Psi stage as one family with bodies keyed by target, including
  unrealized and unreachable bodies, under
  [multi-target compilation](wiki/spec/build/configuration.md#multi-target-compilation)
  end to end (`omega run --both`).
  Owners: `omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/symbols/target_siblings.rs`,
  Psi stages `03–07`, and Omega stage `00` target-family installation.
  - **Complete family catalog and target-sibling dispatch:** retain all target
    member bodies and opaque contracts in Terminal family rows, join selected
    plans and entry/callback backing from the source-free Build companion, and
    exercise portable consumer calls (including filesystem
    `read_dir_count`, `read_dir_nth`, `remove_dir_all`, `open_with`, and `lock`
    once supplied by **FILESYSTEM-PLATFORM-SERVICES**).
  - **Generic, lifetime, and callback conjunction preservation:** preserve
    explicit/universal lifetimes, captured operands, open const scopes,
    evidence telescopes, and per-member callback requirement/guarantee
    conjunctions across all family members (`pass/targets/runtime_family_*_exit`,
    `pass/targets/runtime_target_*_exit`, `pass/targets/portable_family_entry_installations_exit`,
    `pass/targets/selected_family_exclusion_closure_exit`,
    `pass/providers/{inherited_applied_binding_*,applied_entry_nested_binding_context_exit,applied_binding_field_telescopes_exit,occurrence_region_binding_*}`,
    and `pass/traits/{conformance_unused_lifetime_scope,default_row_*}_exit`).
  Acceptance: `fail/targets/sibling_body_type_error_rejected`,
  `pass/targets/target_machine_gating_exit` (70), and the `pass/targets/*`
  family/callback/provider controls pass in both engines; distinct tagged
  native realizations execute from one unchanged Terminal artifact and Build
  companion, while invalid unrealized bodies and duplicate family members
  reject and a missing called body fails only the affected target realization.

- **PROVIDER-SELECTION-AFTER-TERMINAL.** (new-scope; also covers
  **OPERATOR-BOUNDARY-CALLS** and **TARGET-SEMANTIC-APPLICATIONS**)
  Complete source-free per-target provider settlement, legacy operator and
  float boundary-call realization, target-semantic capsule evaluation, and
  package review over canonical Terminal Psi plus target Build rows in Omega
  stage `00` end to end (`omega run --both`).
  Owners: `omega-rust/omega/compilation/driver/src/execution_settlement.rs`,
  `omega-rust/omega/compilation/pipeline/00_terminal-psi-to-abstract-operations/src/{boundary_application_settlement.rs,provider_admission/compiler_builtins.rs}`,
  and `omega-rust/omega/compilation/backend/artifacts/native-artifact/src/physical/derivation/evidence.rs`.
  - **Source-free Omega-00 settlement:** convert the remaining install-phase
    joins in `execution_settlement.rs` (selected dispatch, task activation,
    component progress, intrinsic review, fused entry establishments, and
    deferred constant recipes for arrays/const-generics) from typed/checked
    records to canonical Terminal Psi plus target Build rows, and replace
    provisional completion-role nomination so unused external candidates do not
    make selected returning implementations fail closed.
  - **Operator boundary calls, specialized providers, and IEEE float builtins
    (OPERATOR-BOUNDARY-CALLS):** complete float/receiver signatures, directed
    FMA builtins, member-call borrowed-field planning, Windows GUI provider
    selection (`samples/gui/{window_app,windowed_calculator}`), and specialized
    provider custody (`specialized_{boundary,fixed}_operator_physical_custody`
    and `specialized_{structural_fixed,mixed_structural_fixed}_operator_hosted_native`),
    preserving `samples/cli/facts/float_comparison_route` (all nine places) and
    `pass/float/float_computation_route_exit` on matching hosts.
  - **Target-semantic capsule and package review (TARGET-SEMANTIC-APPLICATIONS):**
    connect provider-free observation receipts, deferred layout/constant
    predicates in call preconditions, and symbolic boundary-application
    closures in package review across reachable specializations.
  Acceptance: a two-target request runs Psi `00–07` once and produces one
  Terminal artifact whose per-target native outputs equal single-target outputs
  byte-for-byte; `pass/float/float_computation_route_exit`, recovered
  `specialized_*_operator_*` fixtures, `pass/providers/*`, `pass/targets/single_target_internal_*`,
  and `sensor_min_max`, `vector_distance`, `temperature_convert`, and
  `fibonacci_golden` pass `omega run --both` on their matching hosts; package
  review reports the full reachable application set and rejects missing, extra,
  or cyclic mappings.

## Lowering, checker, data-flow, and PCC simplification

Owner rulings (2026-10-04): stage 05 is a mechanical lowering of stage 04's
plans (no re-checking, parallel leaf routes, or shape gates), stage 04 and
`validation` (`omega-rust/psi/compilation/checking`) own one integer bounds
engine over frozen typed trees (`FrozenTypedTrees`), proof-carrying code (PCC)
receiver replay is retired while keeping the mathematical kernel and checker
contracts, and every supported primitive operation carries its checking-time
decision (`SourceChecked`) into stage 05 and stage 00.

- **CHECKER-CARRIES-OPERATION-DECISIONS.** (new-scope; also covers
  **STAGE-05-REFUSAL-REVALIDATIONS** and **RETIRE-EXPORTED-AUDIT-FACADE**)
  Complete checking-time discharge of primitive operation conditions into
  `SourceChecked` across stage 04, stage 05, and stage 00, remove the remaining
  12 stage-05 plan-revalidation refusal sites so
  `lowering_refusals_do_not_revalidate_the_plan` passes, and retire or wire
  unreached audit facades end to end (`omega run --both`).
  Owners: stage 04 `CheckFacts`,
  `omega-rust/psi/compilation/pipeline/05_checked-trees-to-lowered-psi/src/source_guards.rs`,
  and stage 00 optimization-unit construction.
  - **Stage-05 refusal revalidations removed (STAGE-05-REFUSAL-REVALIDATIONS):**
    publish needed carrier/root invariants in stage 04 and delete the 12
    plan-revalidation comparisons in
    `omega-rust/psi/compilation/pipeline/05_checked-trees-to-lowered-psi/src/`
    so `lowering_refusals_do_not_revalidate_the_plan` and
    `mbx nextest run -p checked-trees-to-lowered-psi` pass.
  - **Complete `SourceChecked` decision transport:** carry exact checking
    decisions (`ArithmeticDecisions`, `proven_scalar_indexes`,
    `proven_byte_writes`, `decided_call_requires`) through inner element-path
    runtime selectors (`CheckedRuntimeIndex`), view-range segments, record-loan
    element reads, synthesized widenings, scalar-graph casts, bounded stores,
    affine aggregate entries, and invocation-entry/arrival obligations, and
    connect `accept_source_checked_operation` / `verify_target_operation_plan`
    (**RETIRE-EXPORTED-AUDIT-FACADE**).
  Acceptance: `lowering_refusals_do_not_revalidate_the_plan` is green; every
  supported primitive operation reaches stage 05 and stage 00 with its exact
  `SourceChecked` decision; corpus failures for missing decisions reject from
  `omega --check` with checking diagnostics while preserving all
  `arithmetic/*`, `collections/*`, `slices/*`, `calls/*`, `float/*`, and
  `domains/*` positive and negative controls.

- **ONE-INTEGER-BOUNDS-ENGINE.** (new-scope) Consolidate all private integer
  interval engines in stage 04
  (`omega-rust/psi/compilation/pipeline/04_typed-trees-to-checked-trees/src/checks/{ranges,contracts/evaluator,termination/ranking,borrows/overlap/indexes.rs}`),
  `validation`
  (`omega-rust/psi/compilation/checking/src/proof_contracts/{arithmetic_domains,contract_entailment/arithmetic_judgment.rs}`
  and `src/value_custody/struct_literals/construction_bounds.rs`), and
  `omega-rust/psi/compilation/proof/src/checker/integer_ranges.rs` into clients
  of one interval domain and premise-chaining oracle in
  `validation::arithmetic_domains` (`BigIntInterval` / `ExpressionInterval`).
  Acceptance: no private integer interval type remains outside
  `arithmetic_domains`; `python tools/corpus_gate.py --filter ranges,arithmetic/,collections/,slices/,termination/,contracts`
  is unchanged, preserving signed-minimum remainder, full-width unsigned
  quotient, bitwise/shift, and unbounded zero-product bounds and rejection
  controls.

- **TYPED-FROZEN-BEFORE-CHECKING.** (new-scope) Complete qualification of the
  immutable `FrozenTypedTrees` boundary between
  `omega-rust/psi/compilation/pipeline/03b_typed-trees-to-typed-trees/src/preparation.rs`
  and stage 04 (`CheckFacts`), with no checking-time mutation of
  `CheckedTrees.typed` and no `thread_local!` or `RefCell` working state in
  checking/stage-04 production paths.
  Acceptance: the immutable-boundary and `thread_local!`/`RefCell` source
  guards pass; the full check corpus and focused native cohort (including
  `runtime_computed_backing_reference_copy_exit`,
  `runtime_boundary_range_view_exit`,
  `runtime_case_qualified_reference_storage_exit`,
  `runtime_linear_constructor_instances_exit`,
  `runtime_qualified_byte_array_storage_exit`,
  `runtime_inherited_composite_requirement_exit`, and
  `runtime_applied_default_domain_setter_exit` with their negative controls)
  pass without regression.

- **IMAGE-SELF-REPLAY-DELETED.** (new-scope) Direct and dynamic ELF image
  emission build their final image once (only retained/admitted replay entrypoints
  rebuild and compare).
  Acceptance: the `linux_x86_64` native corpus leg on a Linux host, with
  execution, passes unchanged.

- **WIRE-CODEC-LIBRARY.** (new-scope; also covers **SEMANTIC-REFLECTION**)
  Connect [semantic reflection](wiki/spec/language/reflection.md) in
  `omega-rust/psi/compilation/compile-time/src/reflection/` to authored Omega
  queries, field subloans, and active-case dispatch, implement the
  `compact_binary` codec in Omega in `source/library/std/wire.omg` over the
  reflection serializer, and delete the Rust source-text wire generator end to
  end (`omega run --both`).
  Owners: `omega-rust/psi/compilation/compile-time/src/reflection/`,
  `source/library/std/wire.omg`,
  `omega-rust/omega/packages/compilation-inputs/src/wire_codec_sources/mod.rs`,
  `omega-rust/omega/packages/compilation-inputs/src/source_assembly.rs`,
  `omega-rust/psi/compilation/checking/src/machine_calls/calls/write_frames/wire_codecs.rs`,
  and `omega-rust/psi/compilation/pipeline/04_typed-trees-to-checked-trees/src/authored_selections.rs`.
  Acceptance: the reflection inspector/serializer and `source/library/std/wire.omg`
  compile from Omega source and run in both engines (`omega run --both`) across
  the 40-field/five-type record, three-package delegated visitation,
  `tests/omega/{pass,fail}/wire/*`, and `samples/cli/systems/wire_protocol`
  with no generated `.omega/generated/wire-codec.omg` source and all
  unauthorized/ambiguous/loan-escape controls rejecting.

## P1 - Authority, roots, and entry

- **ENTRY-CONTENT-ROOTS.** (Also covers **`PROCESS-EXIT-CONTRACT`**.) Activate
  and complete provisioned receivers and process-exit boundary contracts end to
  end (`Psi 00–07` and `Omega 00–09`) under
  [entry roots](wiki/spec/build/entry_roots.md),
  [provider-slot realization](wiki/spec/build/component_publication.md#provider-slots-and-era-entry),
  and [process exit](wiki/spec/language/process_exit.md).
  - Delete `source/library/core/binding.omg`; `ForeignBinding` remains a
    separate compile-time locator. Checked boundary calls demand exact static
    slots (`omega-rust/psi/compilation/checking/src/machine_calls/boundary_calls.rs`) and no
    authored qualification, source-created routing value, or provider lookup may
    stand in for missing/ambiguous supply.
  - Former borrowed-service customers: make
    `tests/omega/pass/capabilities/uses_caller_folder` call the exact `Folder`
    slot and carry only folder protocol data; migrate the nested
    binding/helper-return chain of
    `tests/omega/pass/capabilities/acquires_through_helper_return` to named role
    applications or protocol handles and verify equivalent reach/provenance;
    keep `tests/omega/pass/text/runtime_stdin_line_buffering_exit` (exit 0,
    exact `first\nsecond\n`) as control.
  - Preserve exact closed slot applications, call occurrences, and selected
    plans across activation/result/out-parameter joins to native execution via
    `omega-rust/omega/compilation/backend/plans/program-entry-plan/`,
    `omega-rust/omega/compilation/driver/terminal-production/`,
    `omega-rust/omega/compilation/pipeline/00_terminal-psi-to-abstract-operations/src/entry_settlement/`,
    and `native-artifact`'s `publish_fragment_object`. Connect nominal cleanup
    and callback/signal occupancy to actual receiver completion
    (`omega-rust/omega/compilation/backend/images/image-emission/src/hosted_receiver.rs`),
    composing application, bridge, provider, and callback stack demand.
  - Complete **`PROCESS-EXIT-CONTRACT`** (`source/library/std/process.omg`):
    remove `Never` return-type assumptions, treat process termination as a
    non-returning `ProcessExit` boundary call with affine cleanup before
    termination, and keep `tests/omega/pass/host/process_exit_{clean,nonzero}`
    and `tests/omega/fail/host/process_exit_requires_service` passing across
    `omega run --both`.

  Acceptance: `samples/cli/basics/{number_guess,cli_mvp,generic_counters}`,
  `tests/omega/pass/capabilities/{uses_caller_folder,acquires_through_helper_return}`,
  and `tests/omega/pass/host/process_exit_{clean,nonzero}` execute via `omega
  run --both` without service fields or test-supplied receivers, and all
  missing/ambiguous supply, non-ZII state, invalid backing/alignment,
  overlapping partition, and unauthorized exit controls reject.

## P2 - Materialization and placed access

- **DEFERRED-RECAST-ESTABLISHMENT.** Implement the ratified
  [exclusive storage-cast window](wiki/spec/layouts/recasts.md#deferred-establishment-on-exclusive-storage-casts)
  through checking, Terminal Psi and native execution. Reuse exact place/version,
  loan, invariant-window and partial-initialization custody; do not add a header,
  IPC or source-shape-specific cast exception. Source: the existing recast owner
  and [implementation note](omega-rust/psi/compilation/checking/recasts.md).
  - Prove geometry/access/exclusion before the cast; track pending target facts,
    occupied source obligations and source handback separately. Promote only after
    complete authorized establishment; preserve strict shared/placed-view rules.
  - Establish a restrictive boundary-data shape in exclusive byte backing, use it,
    and safely return/reuse backing. Cover checked validation and initialized-field
    reasoning without reading invalid tags/references or assuming whole-domain facts.
  - Reject premature typed use/call/publication, bad geometry, surviving aliases,
    stale facts, unauthorized minting/access and widened atomic authority. Exercise
    failed initial establishment with partial linear fields and occupied-source
    restoration; no whole-value destructor, silent rollback or lost resource.

  Acceptance: same establishment fact across the nine places under
  `samples/cli/facts`, matching `tests/omega/{pass,fail}/facts` controls and actual
  `omega run --both` agreement. Pending views cannot cross valid-value interfaces;
  only established results travel. No implementation completion is claimed by the
  documentation ratification.

- **PLAN-LAID-VIEWS.** Complete externally established placed-view entry
  lending, subordinate placed-input reborrowing, and retirement end to end (`Psi
  04–07` and `Omega 00–09`) under
  [placed access](wiki/spec/resources/placed_access.md#establishment-and-retirement).
  - Reuse `add_placed_accessor_attachment` in
    `omega-rust/psi/compilation/pipeline/04_typed-trees-to-checked-trees/src/execution/terminal_unit/types/shape_collector.rs`,
    `omega-rust/omega/compilation/pipeline/00_terminal-psi-to-abstract-operations/src/artifact_admission/establishment.rs`,
    and `omega-rust/psi/interpreter/terminal-interpreter/src/structural_inputs/placed_views.rs`.
  - Extend native publication in
    `omega-rust/omega/compilation/backend/artifacts/native-artifact/src/{fragment_object_publication,entry_receiver}.rs`
    and target-operation lowering in
    `omega-rust/omega/compilation/pipeline/02_abstract-operations-to-target-operations/src/lowering/placed_view_inputs.rs`
    so direct-entry and subordinate placed-view loans establish, access, and
    retire with exact original backing and qualifications across Terminal Psi
    and native execution.

  Acceptance: `tests/omega/pass/layouts/placed_view_source_access_exit`
  (upgraded from `_compile`) establishes, accesses, and retires a placed view
  under `python3 tools/corpus_gate.py --both --filter placed_view_source_access`,
  and stale/substituted plan, backing, range, rights, occurrence, or overlapping
  exclusive controls reject.

- **DEVICE-EXTENT-ACCESS.** Execute provider-backed lending, completion, and
  restored CPU access end to end (`Psi 04–07` and `Omega 00–09`) in
  `samples/cli/device/device_extent_access` under
  [device custody and ordering](wiki/spec/resources/device_access.md).
  - Admit `BoundaryOpaque` types (`DeviceLoan`, `DeviceOrderingEvent` in
    `source/library/core/device.omg`) in reachable Terminal unit state and
    callee signatures (`shape_collector.rs`), and provide runtime/native bodies
    for the ten `LoopbackDeviceLoanProvider` crossings.
  - Connect `omega-rust/psi/foundation/extents/src/{external_loans,ordering_events,mapping}/`
    (`admit_acquisition_after_completion`, `MappedExtent::stable_loan`,
    `ProgramLocalExtentRegistry::remap`) to opaque runtime occurrences,
    role-specific ranges/mappings, device, request, and scope through target
    realization.

  Acceptance: `omega run --both` executes `samples/cli/device/device_extent_access`
  in both directions, and fail controls for missing/extra/duplicate roles,
  substituted coordinates, stale publication, wrong request/device/scope/mapping,
  and premature restoration reject.

- **SYMBOLIC-MATERIALIZATION.** Connect checked source geometry for recursive
  [derived consumers](wiki/spec/layouts/plans.md#derived-consumers) to the
  generated post-handoff writer and its installed native invocation under
  [writer custody](wiki/spec/build/hardware_materialization.md#generated-writers-and-publication).
  Reuse `omega-rust/psi/foundation/layout-plans/src/symbolic_materialization.rs`
  and `omega-rust/omega/compilation/backend/plans/program-entry-plan/src/post_handoff_writer/mod.rs`
  across x86-64 and Linux AArch64 while rejecting open templates, non-closed
  member applications, and runtime counts in static layout.

  Acceptance: a registered corpus/sample customer executes the generated writer
  natively on Linux AArch64 and matching hosts, matching reference destination
  bytes and surrounding guards across empty arrays, sibling writes, and
  nested/generic cases.

- **ADDRESS-EXPOSURE.** (Also covers **`NATIVE-CODE-LAYOUT-QUERIES`**.) Complete
  [`address_of` queries](wiki/spec/language/counts_and_addresses.md#address-exposure)
  and [`code_layout<Machine>()` queries](wiki/spec/language/counts_and_addresses.md#native-code-layout-queries)
  end to end (`Psi 00–07` and `Omega 00–09`):
  - Retain selected machine applications (`address_of<Selected<Arguments>>()`),
    bodyless target-family applications (`code_layout<Machine>()`), and
    aggregate/re-view/write-only storage referents (`address_of(reference)`)
    through specialization/inlining, Terminal Psi encoding, and physical
    selection.
  - Ensure disjoint code/storage encodings in the Terminal interpreter
    (`StructuralRuntimePlace`, `LocalStructuralIdentities`) without truncation,
    collision, or zero for live entries, and carry explicit entry/frame custody
    through split/shared native realization and relocations. Keep
    `samples/cli/facts/selected_native_code_layout` passing across all nine
    places.

  Acceptance: corpus and fact witnesses execute `address_of<Selected<Arguments>>()`,
  `address_of(reference)`, and target-family/split-fragment `code_layout<Machine>()`
  with `omega run --both` agreement, while unresolved selections, proof-only
  entities, invalid loans, nonaddressable projections, and constant/type/layout
  eval leaks reject.

- **ASSUMED-BINDING-CONTRACTS.** Complete root-only
  [`Build::assume_binding_contract<Required::operation, Supplied::operation>(builder)`](wiki/spec/build/assumed_binding_contracts.md)
  end to end beyond `samples/cli/facts/assumed_binding_result_bound` and
  `samples/cli/facts/assumed_binding_input_authority`: carry result
  qualifications, content guarantees, program-local root introductions, broader
  invocation/lifetime correspondence, exact entry/exit assumptions, authorized
  issuer/subject mapping, and selected implementation closure through erasure,
  inlining, lock rows, and audit (`Extremely high risk` disclosure and the
  [refutable-connection exception](wiki/spec/build/assumed_binding_contracts.md#known-contradictions-at-the-trust-crossing)).

  Acceptance: a corpus caller executes through the root-selected link with
  caller-facing qualification including input-side authority under `omega run
  --both`, and fail fixtures reject carrier/ABI/lifetime/custody mismatches,
  competing/cyclic supply, unapproved/stale roots, and use from helpers,
  generators, const/layout eval, or automatic tests.

## P3 - Terminal Psi, proofs, and observation

- **WRITE-ONLY-BORROW.** Complete caller-visible `&write T` mutation end to end
  (`Psi 04–07` and `Omega 00–09`) under
  [write-only authority](wiki/spec/terminal-psi/structural_access.md#write-only-authority):
  - Admit non-observation evidence for opaque foreign declarations and
    boundary-trait requirements bound to the selected implementation and receipt
    before original-backing foreign output transport (keeping
    `fail/borrow/opaque_write_only_boundary` for unproved boundaries).
  - Support aggregate replacement carrying conserved displaced custody without
    observing old contents through write-only access, and computed IEEE
    store/readback.
  - Keep all existing `borrow/runtime_write_only_*_exit` and
    `borrows/runtime_reference_aggregate_*_replacement_exit` controls passing at
    exit 0.

  Acceptance: `python3 tools/corpus_gate.py --both --filter write_only` passes
  on Windows, macOS, and both Linux targets, and fail fixtures reject reads,
  bare forwarding, readable widening, and overlapping exclusive arguments.

- **STRUCTURAL-BORROW-IDENTITY.** (Also covers **`PLACED-ACCESS-NATIVE-OPS`** and
  **`GENERIC-FIXED-ARRAY-VIEWS`**.)
  Preserve original caller storage and exact-once computed/indexed selector
  semantics through every projection, call preparation, and native placement
  end to end (`Psi 04–07` and `Omega 00–09`) under
  [structural access](wiki/spec/terminal-psi/structural_access.md). Owners:
  stage 04 `execution/terminal_scalar/`,
  `execution/terminal_unit/calls/computation_arguments/`, and
  `execution/terminal_unit/structural_scalar_store/selectors.rs`; stage 05
  `emission/runtime_elements.rs`; stage 02
  `omega-rust/omega/compilation/pipeline/02_abstract-operations-to-target-operations/src/lowering/{coordination,structural_layout}.rs`
  and `lowering/control_flow/scalar_sources.rs`; image-emission
  `argument_custody`.
  - Complete cross-host native and Terminal Psi execution (`omega run --both`)
    for `samples/cli/facts/computed_nested_state_loan`,
    `collections/runtime_indexed_struct_field_rmw_exit`,
    `collections/runtime_parameter_nested_literal_index_read_exit`,
    `collections/runtime_double_indexed_rmw_exit`,
    `storage/runtime_machine_owned_indexed_integer_write_exit`,
    `collections/runtime_hoisted_index_write_exit`,
    [the selector regression family](tests/omega/pass/collections/runtime_computed_nested_borrow_call_exit/README.md),
    [the constrained shared-field witness](tests/omega/pass/layouts/runtime_indexed_restricted_field_read/README.md),
    and `borrows/runtime_indexed_{stored_reference_call,scalar_descriptor_call,reference_call_regions}`
    across Windows x86-64, macOS ARM64, Linux x86-64, and Linux AArch64.
  - Carry ordinary generic fixed-array views from `&[T; Count]` to `&[T]`
    through the range protocol, preserving symbolic extent and backing custody.
    Distinct extents 3 and 4 must check in all four target bodies and return 0
    in both engines; wrong extents, stale descriptors and conflicting backing
    writes reject.
  - Qualify the nine-place
    [reference recast backing-footprint row](samples/cli/facts/reference_recast_backing_footprint/README.md)
    on the other three declared hosted targets, preserving the original address
    and full backing loan alongside its fact-sensitive reject twins.
  - Keep `pass/ownership/runtime_{captured_exclusive_argument,exclusive_argument_before_descriptor_retarget}_exit`
    and `runtime_{fixed_selector_collection_capture,index_before_descriptor_retarget,fixed_selector_owned_argument}_exit`
    at 0 in both engines: capture the collection before its selector, execute
    the selector once and retain completed arguments before later siblings.
    `fail/ownership/fixed_selector_owned_argument_second_consume` must reject.
  - Keep all `calls/unread_multistate_receiver_exit` and `borrows/*_exit`
    snapshot, subloan, and linear-owner controls passing.

  Acceptance: all indexed/projected borrow witnesses above and
  `samples/cli/facts/computed_nested_state_loan` execute via `omega run --both`
  on Windows, macOS, and both Linux targets, modifying or reading the exact
  selected caller element while leaving neighbors untouched, and fail fixtures
  reject copied borrowed homes, missing bounds/dereference, changed access, and
  live-loan exclusion violations.

- **BORROW-PROOF-CONVERGENCE.** Complete borrow-disjointness establishment in
  stage 04 `checks/borrows/` under [loans](wiki/spec/terminal-psi/loans.md)
  across remaining callee/domain predicates, runtime projections, and
  same-statement/theorem establishment beyond
  `samples/cli/facts/computed_subslice_extent`, using exact call substitution
  and the shared contract/range readers.

  Acceptance: corpus pass fixtures and fact witnesses accept disjoint loans,
  writes, and exclusive call operands established by each remaining premise kind
  under `omega run --both` (keeping
  `pass/slices/disjoint_mut_subslice_windows_compile` and
  `fail/slices/overlapping_mut_subslice_windows_rejected`), while missing,
  stale, or insufficient premises and overlapping mutations reject.

- **TERMINAL-SLICE-VIEW-VOCABULARY.** (split-of:SLICE-VIEW-LOCAL-ENTRY-ESTABLISHMENT)
  Complete consuming non-byte `&[T]` calls and state transfers end to end (`Psi
  04–07` and `Omega 00–09`) under
  [view custody](wiki/spec/terminal-psi/byte_views.md#element-typed-slice-views).
  Reuse stage 04 `execution/terminal_unit/calls/view_subslice.rs` and stage 05
  `unit/attached_unit/view_ranges.rs` and
  `emission/operation_emission/view_subslice.rs`.
  - Restore default-domain windows for narrowed record-element slice arguments
    in source checking while keeping `samples/cli/facts/as_slice_views`,
    `samples/cli/facts/computed_subslice_extent`, and all `slices/runtime_*_exit`
    controls passing on `omega run --both`.

  Acceptance: consuming views for scalar and record elements,
  `samples/cli/text/fletcher_checksum` (exit 56), and
  `samples/cli/arithmetic/recursive_sum` (exit 70) execute on `omega run --both`
  across hosts, and bounds, element-type, alias, and access violations reject.

- **NORMALIZED-ABI-LOWERING.** (Also covers **`OPAQUE-BY-VALUE-BOUNDARY-ABI`**
  and **`FOREIGN-RETAINED-ARGUMENT-BACKING`**.) Complete foreign and
  cross-component calling plans, opaque by-value representation agreement, and
  retained outbound argument backing end to end (`Psi 04–07` and `Omega 00–09`)
  under [calling plans](wiki/spec/build/calling_plans.md),
  [opaque representations](wiki/spec/build/opaque_representations.md), and
  [outbound custody](wiki/spec/build/foreign_storage.md#outbound-custody):
  - Qualify remaining borrowed descriptor carriers, original scalar-field
    backing through thin foreign export
    ([foreign_scalar_backing](samples/cli/facts/foreign_scalar_backing/README.md)),
    and C record-result host/ABI lanes beyond the exact bindings covered by
    [foreign_scalar_record_result](samples/cli/facts/foreign_scalar_record_result/README.md)
    and the macOS ARM64 indirect floating result in
    [foreign_indirect_float_record_result](samples/cli/facts/foreign_indirect_float_record_result/README.md)
    across Windows, macOS, and both Linux targets in Omega stages 02–03 and
    backend call-frame emission,
    keeping
    `providers/foreign_{owned_record,record_result,multiregister_result,indirect_record_*}_exit`
    and `host/runtime_descriptor_*_exit` passing.
  - Connect `omega-rust/omega/compilation/realization/provider-planning/src/calling_policy_plans/boundary_signatures.rs::opaque_representation_movement`
    to provider call-frame transport and backend emission for the remaining
    qualified and view-backed opaque inputs with exact composition-edge
    drift rejection. The direct and independent closed Affine rows in
    `samples/cli/facts/opaque_closed_carrier_exchange` now pass all nine places
    on all four hosted targets, including Linux ARM64. The
    [checked Copy continuation](samples/cli/facts/opaque_closed_carrier_exchange/independent_copy/README.md)
    also has nine-place native/Terminal agreement and copy-only rejects on all
    four targets. Its README retains the original Windows timeout and the
    independently reviewed finite diagnostic; that original run remains red.
    The [Linear continuation](samples/cli/facts/opaque_closed_carrier_exchange/independent_linear/README.md)
    carries actual whole ownership claims through independent inputs and returns
    across all nine places on all four targets. Restore their current Build
    selection path across the deferred Product source/dependency join: the
    unchanged baseline now refuses the first `select_representation` Data
    operand before execution. Preserve the qualified source contracts and
    exact representation/provider authority through that repair.
  - Complete installed-provider custody and native marshaling for protocol-owned
    backing, permitted snapshots and the remaining resident/view codecs.
    Finite embedded ordinary records now retain original borrowed backing
    through [nested record codecs](samples/cli/facts/foreign_nested_record_backing/README.md)
    in all nine places on all four hosts; arrays, tagged/dynamic carriers,
    independently qualified descendants and nested owned/result publication
    still require their own codecs. Preserve the complete lifetime, range and
    revision/lease records through stage 05
    `retention/retained_borrow_custody.rs` and the selected provider; private
    per-call captures cannot supply original backing.

  Acceptance: `samples/cli/facts/opaque_closed_carrier_exchange`,
  `samples/cli/facts/foreign_retained_shared_borrow`,
  `samples/cli/facts/foreign_retained_mutable_borrow`,
  `samples/cli/facts/foreign_nested_record_backing`, and the `providers/foreign_*`
  corpus fixtures pass `omega run --both` across matching hosts with exact
  register/stack placement, occurrence preservation, and retention leases, while
  carrier/target/lifecycle/multiplicity drift and unauthorized retention reject.

- **PROOF-KERNEL-CORE.** (Also covers **`PROOF-CONTRACT-MIGRATION`**,
  **`PROOF-RELEVANCE-MIGRATION`**, **`QUOTIENT-THEOREM-LIFT`**, and
  **`PROOF-SURFACE-DECLARED-NOT-BUILT`**.) Complete source-to-kernel proof
  checking, mathematical bindings, quotient theorem lifting, and carrier-free
  erasure end to end (`Psi 00–07` and `Omega 00–09`) under
  [proof foundation](wiki/spec/proofs/foundation.md),
  [inductive profile](wiki/spec/proofs/inductive_profile.md),
  [contracts](wiki/spec/proofs/contracts.md),
  [mathematical bindings](wiki/spec/proofs/mathematical_bindings.md), and
  [quotients](wiki/spec/proofs/quotients.md). Owners:
  `omega-rust/psi/compilation/proof/admission/src/mathematical_core/`,
  `omega-rust/psi/compilation/proof/src/{proof_surface.rs,mathematical_declarations/fixed_core_vocabulary.rs}`,
  and stage 04–05 proof/contract lowering.
  - Replace remaining `bounded_denotation.rs::rule_instance` assumptions with
    checked derivations and extend source-produced indexed families beyond
    `samples/cli/facts/{closed_integer_bound,source_indexed_family,source_indexed_binary_tree,source_indexed_type_parameter}`
    to general runtime-dependent carrier/telescope transport, mutual/nested
    families, and higher type/level indices.
  - Complete body denotation, member-call resolution, `ForAllInRangeFact` /
    `QuantifiedRangeFact` sequence/range entailment, higher-order
    `TraitRequirement` predicate/function terms, and ordinary contract/bundle
    replacement of retired `proposition`/hidden-witness routes; remove
    `omega-rust/psi/compilation/proof/src/mathematical_declarations/fixed_core_vocabulary.rs`'s
    unresolved-spelling fallback and either wire or delete unused entries in
    `omega-rust/psi/compilation/proof/src/proof_surface.rs`.
  - Complete **`QUOTIENT-THEOREM-LIFT`** (`source/library/core/quotient.omg`,
    `tests/omega/{pass,fail}/proofs/quotient_*`) so representative bodies lift
    to public `Quot<T, R>` theorems through `Quot::ind`/`Quot::lift` with exact
    respectfulness and transitive assumption closure.
  - Complete **`PROOF-RELEVANCE-MIGRATION`** beyond
    `samples/cli/facts/erased_reference_contract`: extend erased arguments to
    record/enum/array-bearing formals and dynamic trait calls
    (`fail/relevance/dynamic_erased_formal_lane` becomes positive) with stripped
    runtime signatures and zero ABI storage.

  Acceptance: all [migration examples](wiki/spec/proofs/foundation.md#migration-acceptance),
  `samples/cli/facts/{closed_integer_bound,negated_mathematical_order,source_indexed_*,erased_reference_contract}`,
  and `tests/omega/{pass,fail}/proofs/{kernel_theorem_equality_*,quotient_*}`
  pass `omega run --both` with false-twin rejections, exact transitive
  assumption closure, and a refreshed
  [metatheory evidence ledger](wiki/spec/proofs/kernel_metatheory.md#evidence-ledger).

## Build-level behavior exclusions

- **BUILD-SEMANTIC-EXCLUSIONS.** (Also covers **`BUILD-EXCLUSION-REALIZATION`**.)
  Deliver the [behavior exclusion contract](wiki/spec/build/behavior_exclusions.md)
  across the full admitted entry/call closure, native artifact custody,
  installation, and replacement (`Psi 04–07` and `Omega 00–09`). Reuse
  `omega-rust/omega/compilation/backend/plans/provider-evidence/src/provider_evidence/executable_scopes/behavior_exclusions.rs`,
  `omega-rust/omega/compilation/realization/trust-model/src/behavior_exclusions/selections.rs`,
  `omega-rust/omega/compilation/pipeline/00_terminal-psi-to-abstract-operations/src/retained_terminal.rs`,
  `omega-rust/omega/compilation/native-production/src/realization.rs`,
  and `omega-rust/omega/compilation/backend/artifacts/native-artifact/src/native_artifact.rs`.
  Cover generated entries and remaining dispatch forms, and join the exclusion
  policy union to the exact semantic/target/provider selection through
  legalization, image emission, and component replacement.

  Acceptance: `tests/omega/{pass,fail}/build/{crash_exclusion_*,*service_exclusion*}`
  (`no-op-app` and `quiet-logger-app` using `tests/omega/packages/{assert-kit,logger-kit}`)
  pass `python3 tools/corpus_gate.py --both` with optimization off and on across
  Windows, macOS, and Linux, and prohibited behavior, insufficient coverage, or
  substituted providers reject before publication.

## Requirement-based tests

- **BUILD-TEST-GROUPS.** (Also covers **`BUILD-TEST-AUTHORITY`**.) Implement the
  [testing contract](wiki/spec/build/testing.md) and
  [authored source shape](wiki/language_guide/chapter_23_testing.md) end to end
  (`Psi 00–07` and `Omega 00–09`):
  - Add `builder.tests.group<ExactRequirement>()` registration and enablement to
    `Build` (`omega-rust/omega/packages/build-declarations/src/prelude.rs`) and
    `EvaluatedBuildSettings`
    (`omega-rust/omega/packages/compilation-inputs/src/build_execution/build_value/configuration.rs`),
    with package-local satisfaction discovery, separate runner roots, and
    Terminal Psi + native execution in `omega-rust/omega/compilation/driver/`.
  - Extend `omega-rust/omega/compilation/build-evaluation/src/admitted_build_program.rs`
    and `omega-rust/omega/packages/package-manager/src/review/restricted_build_grants.rs`
    to carry group-scoped providers, ordinary fixture storage, fresh mock state,
    and lock-reviewed test authority without automatic mutable context
    provisioning or assumed-binding execution.

  Acceptance: pure and virtual-filesystem test groups discover and execute on
  both Terminal Psi and native engines with identical deterministic observations
  while application outputs exclude test-only roots; failing mandatory checks,
  missing/ambiguous mock supply, widened child requests, and assumed bindings in
  automatic tests reject.

## Embedding and interpreted components

- **EMBEDDING-SOURCE-TO-HOST.** (Also covers **`EMBEDDING-RESOURCE-LIFETIMES`**
  and **`PSI-COMPONENT-REPLACEMENT`**.) Implement the
  [embedding contract](wiki/spec/build/embedding.md) end to end in
  `omega-rust/psi/interpreter/terminal-interpreter/` (`src/{program,instance,adapters,results}.rs`)
  and `omega-rust/omega/compilation/realization/provider-planning/src/provider_settlement/composition_modes.rs`:
  - Deliver `compile -> load -> supply -> invoke -> host result -> guest result -> close`
    from an authored interface package, host implementation, and script through
    exact static import/provider slots.
  - Extend instance-owned adapters with borrowed views, owned host resources,
    asynchronous waiting/completion across fuel pauses, and contracted
    callbacks.
  - Support live interpreted subcomponent replacement (`v1 -> v2`) without
    returning from `main`, keeping paused calls on `v1` and retiring `v1` only
    after every pin releases.

  Acceptance: end-to-end embedding fixtures execute full host-import invocation,
  fuel-pause/resume with invariant-window protection, and live subcomponent
  replacement, while missing/unauthorized bindings, schema mismatches,
  conflicting inspection during pauses, and premature retirement reject.

- **INTERPRETED-CATHEDRAL.** Run Cathedral startup, memory establishment,
  visible device work, live event delivery, and guest-owned component
  replacement in the Terminal interpreter (`omega-rust/psi/interpreter/terminal-interpreter/`),
  building on **`EMBEDDING-SOURCE-TO-HOST`** and **`CHECKED-ASM-LABEL-CONTRACTS`**.
  Migrate Cathedral's `tools/x86-legacy-bootstrap-composition-canary/main.omg`
  and `tools/{extent-root-canary,timer-root-work-canary}/run.sh` from retired
  debug JSON dumps to observed runtime behavior.

  Acceptance: the full Cathedral startup, device, event, and replacement
  sequence runs in the Terminal interpreter with reported partial slices and
  shared native/interpreted algorithms.

## P5 - Cathedral over general Omega primitives

- **EXTENT-LIBRARY-TRANSFORMS.** (Also covers
  **`CONSERVATION-CONTRACT / TERMINAL-CONTENT-CLAIMS`** and
  **`BOUNDARY-ISSUANCE`**.) Complete checked n-to-m content conservation,
  provider boundary issuance, and core `Extent` split/merge operations end to
  end (`Psi 04–07` and `Omega 00–09`) under
  [content custody](wiki/spec/resources/content_custody.md) and
  [external roots and issuance](wiki/spec/resources/authority.md#external-roots-and-issuance).
  - Carry general n-to-m claim transformations and match-selected wrapper
    conservation guarantees through separately checked calls, result
    correspondence, and Terminal/native execution using canonical
    `source/library/core/{content,extent}.omg` identities (keeping
    `tests/omega/pass/core/content_conservation_wrapper_match_exit` and
    `tests/omega/fail/core/content_conservation_wrapper_match_unproved_alternative`).
    Only proved bodies and admitted declarations supply theorem roots; unproved
    guarantees and recursive cycles cannot seed their own evidence. Keep every
    reachable return alternative, capture epoch and invocation correlation.
  - Carry per-invocation geometry, backing, and live supply through provider
    planning and native settlement in
    `omega-rust/omega/compilation/realization/provider-planning/src/{service_schema/mod.rs,provider_planning/qualification_grants.rs}`
    and stage 05 `unit/attached_unit/catalog.rs` so
    `tests/omega/pass/core/content_retained_custody_round_trip` receives an
    authorized initial claim from its selected admitted `InitialSupply::acquire`
    invocation and executes its submit/complete cycle. Fresh native allocation
    and constructor shape alone do not establish the initial claim.
  - Complete canonical execution qualification of the hosted-entry programs
    `tests/omega/pass/terminal_psi/{content_custody_exit,structural_content_passthrough}`
    alongside the custody path, using bundled Content and quantity identities.
  - Implement usable `Extent` split and merge machines in
    `source/library/core/extent.omg` that consume and account for all inputs,
    preserve vacancy independently of the backing grant, and return Cathedral's
    retired vacant storage to allocator custody.

  Acceptance: a nine-place fact row under `samples/cli/facts/` for actual
  split/recombine content transformation, a non-Extent conservation resource,
  and `python3 tools/corpus_gate.py --both --filter core/content_conservation_contract,core/content_retained_custody_round_trip`
  pass `omega run --both` with fact-sensitive reject twins, and Cathedral's
  retirement/recomposition path executes over retained backing with readback.

- **BUMP-ALLOCATOR-CANARY.** (Also covers **`VEC-NATIVE-GROWTH`**.) Deliver an
  executable package allocator (`source/library/alloc/bump.omg`) and native
  growable `Vec` (`source/library/alloc/vec.omg`) over qualified `Extent` and
  **`PLAN-LAID-VIEWS`** end to end (`Psi 04–07` and `Omega 00–09`) under
  [allocation](wiki/spec/resources/allocation.md), needed by Squalr and
  Cathedral.
  - Complete `ExtentPartition` conservation and every-retired-extent `reset`
    recomposition over real `Vacant`/`Resident` backing in
    `tests/omega/pass/memory/bump_allocator_canary`, replacing fixture-local
    `ResidentStorage` with **`PLAN-LAID-VIEWS`** establishment, element
    transfer, and retirement.
  - Connect `Vec` growth/reallocation/drop to the real backing allocator so
    dynamic `Vec` growth executes natively across hosts.

  Acceptance: `tests/omega/pass/memory/bump_allocator_canary` and `Vec` growth
  witnesses execute via `omega run --both` with real backing, coexisting
  allocations, preserved prefix elements on growth, and full reset after
  release, while `fail/memory/bump_allocator_*` and live-reset/double-placement
  controls reject.

- **UEFI-PHYSICAL-SEMANTIC-ENTRY.** (Also covers **`UEFI-OS-HANDOFF`**.) Execute
  the [two-surface UEFI bootstrap](wiki/spec/build/uefi_entry.md#authored-firmware-definitions-and-adapters)
  and [Boot Services-to-OS handoff](wiki/spec/build/uefi_entry.md#returning-application-versus-os-handoff)
  end to end (`Psi 04–07` and `Omega 00–09`).
  - Complete returning semantic-entry lifecycles (single context cleanup, fresh
    context on repeated invocation, provider-owned reference outputs, and
    `pass/build/uefi_selected_physical_entry`) and preserve Cathedral's
    `source/kernel/boot/uefi` QEMU/OVMF boot oracle (`tools/boot-harness/run.py`).
  - Realize `UefiOsHandoffTermination::{transfer,firmware_return}` in
    `omega-rust/omega/compilation/realization/selected-dispatch/src/compiler_intrinsic.rs`
    and complete descriptor validation, stale-key retry, and allocation lineage
    in `source/library/std/targets/uefi_x86_64/handoff.omg` and
    `tests/omega/pass/build/uefi_os_handoff_invocation`.

  Acceptance: `tests/omega/pass/build/{uefi_selected_physical_entry,uefi_os_handoff_invocation}`
  and Cathedral's `source/kernel/boot/uefi` execute under QEMU/OVMF or a
  controlled provider, covering returning cleanup, stale-key retry, and
  non-returning OS entry handoff while rejecting malformed maps, stale/foreign
  evidence, and Boot Services use after exit.

- **AP-BRINGUP.** Execute Cathedral-owned secondary-processor startup end to end
  under [external roots](wiki/spec/build/external_roots.md#secondary-processor-startup).
  Connect `tests/omega/pass/memory/secondary_processor_canary` to the target
  trampoline emission in
  `omega-rust/omega/compilation/backend/machine-emission/src/`, generic
  installed-entry/resource custody, and provider receipt ingress without a
  compiler-owned APIC driver or processor lifecycle state machine.

  Acceptance: on a matching platform or emulator, `secondary_processor_canary`
  enters the installed entry with dedicated nonoverlapping stack/state,
  handling unconfirmed dispatch, late arrival, cancellation with quiescence
  evidence, and rejecting foreign/stale/replayed receipts or resource conflicts.

- **ADDRESS-TRANSLATION-CANARY.** Execute Cathedral-owned multi-page mapping
  installation and teardown end to end in
  `tests/omega/pass/memory/address_translation_canary` under
  [mapping and reclamation](wiki/spec/resources/extents.md#mapping-and-reclamation),
  joining selected authority, hardware, and shootdown providers to
  `extents::mapping`'s source/destination, identity, era, and
  activation/release receipts (`FRAME_MASK = 0x000FFFFFFFFFF000`).

  Acceptance: QEMU installs and tears down multi-page mappings with Extent/TLB
  custody and `FRAME_MASK` walk verification, while source use after map,
  premature unmap (`fail/core/translation_unmap_before_activation`), lost
  shootdown, forged carriers, and stale-era receipts reject.

- **EXCEPTION-ROOTS-AND-TIMER.** (Also covers
  **`BOUNDED-INSTALLATION-REACH-ROWS`**.) Execute Cathedral fatal entries on
  dedicated critical stacks and a timer that acknowledges, records, and wakes
  work end to end in `tests/omega/pass/memory/interrupt_table_canary` under
  [interrupt obligations](wiki/spec/build/interrupt_obligations.md),
  [installation-bound reach](wiki/spec/build/external_roots.md#installation-bound-reach),
  and [hardware materialization](wiki/spec/build/hardware_materialization.md).
  - Migrate `InterruptAcknowledgement::complete` in
    `source/library/core/interrupt.omg` to `reaches <= MachineControl + PortIo`
    and carry `InstalledInterruptCompletionRoute` through
    `omega-rust/omega/compilation/realization/provider-planning/src/provider_planning/installation_reach.rs`,
    **`TOP-LEVEL-BOUNDARY-REQUIREMENTS`**, and **`CHECKED-ASM-LABEL-CONTRACTS`**.
  - Join the interrupt entry/exit stub and member-call relocations in
    `omega-rust/omega/compilation/backend/machine-emission/src/` to image
    placement, installation, and checked `lidt` admission.

  Acceptance: in QEMU, `interrupt_table_canary` and Cathedral report timer ticks
  through owned output, halt between ticks, select `PortIo` (PIC) or
  `MachineControl` (LAPIC/x2APIC) completion through the installation-bound
  route, and route a deliberate fault to the dedicated-stack fatal handler,
  while missing members, changed bytes, or forgotten/double completion reject.

## Parallel language and compiler lanes

- **MATCH-SELECTIVE-LOWERING.** Lower [value dispatch](wiki/spec/language/patterns.md)
  for owned, borrowed, and linear results, structural/case/domain patterns, and
  coverage end to end (`Psi 04–07` and `Omega 00–09`), preserving once-only
  subject evaluation, authored first-match order, selected-arm-only execution,
  and exact result/loan origins and death edges. Owners:
  `omega-rust/psi/compilation/checking/src/value_custody/expression_types/{match_dispatch,result_type}.rs`,
  checked value continuations, Terminal production, and
  `omega-rust/omega/packages/review-evidence/src/capture/contracts/expressions/projection/`.
  - Carry affine custody through branch-local calls/receivers, effectful
    subjects, projected fields inside record-construction arms, heterogeneous
    residual custody, and claim-bearing body plans.
  - Extend borrowed joins in
    `omega-rust/omega/compilation/representations/abstract-operations/src/abstract_operations/control_flow/address_joins.rs`
    and `omega-rust/omega/compilation/representations/abstract-operations/optimization-unit/semantics/src/current_ownership/address_joins.rs`
    to write-only joins, case/reference/descriptor-bearing referents,
    runtime-sized element-view selections, ranges, computed roots, and
    join-place reads/projections, ending loans at last use.
  - Carry combined Psi rewrite provenance across `branch_edges` in
    `omega-rust/omega/compilation/pipeline/03_target-operations-to-selected-instructions/src/legalization/scalar_graph_input/control.rs`
    and instantiate composite/constrained result shells and scoped
    captured-subject identities for indexed predicates/theorems.

  Acceptance: corpus and fact witnesses for owned call-product arms,
  interleaved/projected sources, linear child carriers, borrowed joins, and
  [float Match customers](wiki/drafts/reference/float_realization.md#operation-and-control-custody)
  execute via `omega run --both` (keeping
  `tests/omega/pass/expressions/{match_anonymous_result_landing,numeric_operand_destinations}`
  and `tests/omega/pass/collections/runtime_dutch_flag_partition_exit` at exit
  70), while wrong qualifications, ownership, or incompatible arms reject.

- **OPERATOR-MACHINE-SUPPLY.** Run calls to declared operations and their
  premises end to end (`Psi 00–07` and `Omega 00–09`) under
  [declaration-owned executable supply](wiki/spec/language/expressions.md#executable-supply),
  [call preconditions](wiki/spec/language/machines.md#call-preconditions), and
  [licensed normalization](wiki/spec/proofs/contracts.md#licensed-normalization).
  - Extend formation checks in
    `omega-rust/psi/compilation/checking/src/proof_contracts/contract_entailment/specification_calls.rs`
    and `omega-rust/psi/compilation/pipeline/04_typed-trees-to-checked-trees/src/checks/contracts/evidence.rs`
    to static callable/evidence arguments, generic dispatch, calls in abstract
    signatures, domain/default predicates, receiver/projection terms, and
    citation/induction guarantees. The `abstract_signature_call` constant
    production now preserves selected-call custody through the nine-place row,
    statement and terminal calls, and calls inside the supplied body; broader
    formation acceptance remains open.
  - Extend `OpenIndexOperationSelection` in
    `omega-rust/psi/compilation/checking/src/value_custody/type_references/open_index_expressions.rs`
    with distributivity and zero/one law evidence surviving
    `monomorphization::finalize_domain_instance_identities`.
  - Migrate library/corpus/embedded declarations off the `operator` introducer
    ([retirement inventory](wiki/drafts/audits/operator_introducer_retirement_inventory.md))
    and remove legacy parser/IR consumers and undifferentiated
    `MachineSupplyMode::Boundary`.
  - Controls (keep passing across all nine places and `omega run --both`):
    `samples/cli/facts/{body_bearing_comparison,generic_comparison_body,projection_call_premise,abstract_signature_call,static_evidence_premise}`
    and `tests/omega/{pass,fail}/facts/*`.

  Acceptance: `tests/omega/pass/expressions/declared_operator_match_result/`
  (exit 70), the nine-place fact rows above, and new fixtures for
  generic/stateful bodies, private helpers, qualifiers, ordered operands, and
  receiver/result loans pass `omega run --both` on every host, while missing
  bodies, duplicate shapes, unproved laws, and missing premises reject.

- **MODULE-NAMESPACE-RESOLUTION.** Complete exact declaration selection through
  checking, constant evaluation, and publication end to end (`Psi 00–07` and
  `Omega 00–09`) under the [module/name contract](wiki/spec/language/modules.md)
  and [default builds and anonymous owner identity](wiki/spec/build/declarations.md#default-builds-and-anonymous-owner-identity).
  - Assign versioned content owners to nonempty authored anonymous `Build`
    activations after dependency requests join and before product declaration
    identities are interned (extending
    `samples/cli/facts/nominal_declaration_owner`), and complete
    `tests/omega/pending/modules/direct_file_import_exit` (keeping
    `pass/modules/direct_file_import_process_exit` at exit 7).
  - Extend normalization and exact declaration selection for indexed constraints
    and operator homes in
    `omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/preparation/`,
    and extend constant evaluation in
    `omega-rust/psi/compilation/compile-time/src/const_evaluation/const_initializers/`
    and `omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/preparation/generic_data/const_evaluation/{facts,values}.rs`
    to specialized provider/target applications, exact authored NaN bits, nested
    array/case and whole-aggregate subjects, open applications, and
    abstract-carrier constraints over explicit named subjects (keeping
    `samples/cli/facts/computed_constant_array_extent` passing across all nine
    places).
  - Carry constant-invocation crash premises through
    `omega-rust/psi/compilation/pipeline/04_typed-trees-to-checked-trees/src/facts/crash_entry_values.rs`.

  Acceptance: `python3 tools/corpus_gate.py --both --filter modules/`,
  `samples/cli/facts/{nominal_declaration_owner,computed_constant_array_extent}`,
  and fixtures for nonempty anonymous `Build` owners and new constant forms pass
  `omega run --both`, while mismatched provenance, visible-name conflicts, and
  unproved constant indices reject.

- **RUNTIME-VALUE-GENERICS.** Support runtime-capable value binders on data end
  to end (`Psi 00–07` and `Omega 00–09`) under
  [value binders](wiki/spec/language/generics.md#value-binders-and-const-requirements)
  and [runtime index identity](wiki/spec/language/generics.md#runtime-index-identity-and-storage)
  beyond `samples/cli/facts/{runtime_index_application,selected_value_application}`:
  - Extend `validate_const_data_argument` in
    `omega-rust/psi/compilation/checking/src/value_custody/type_references/generic_arguments.rs`,
    `omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/lowering/data.rs`,
    `omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/preparation/generic_data/`,
    and stage 04 `monomorphization/` so executable value indices pass as
    ordinary data/descriptors while proof-only uses erase after discharge.
  - Substitute callee types in inferred caller hoists for forwarded
    value-indexed data results and enforce exact index identity across
    reassignment (`Index<9>` cannot pass as `Index<7>`).

  Acceptance: `<Count: u32>` accepts static and runtime arguments under proved
  obligations while `<const Count: u32>` stays static, and
  `samples/cli/facts/{runtime_index_application,selected_value_application}` plus
  `tests/omega/pass/generics/{runtime_value_generic_subject_exit,runtime_forwarded_value_contract_exit}`
  pass `omega run --both` across hosts with invalid construction/bounds/retag
  twins rejecting.

  Linux x86-64 now has native/Terminal agreement for both existing nine-place
  rows, the named acceptance fixtures, and the forwarded descriptor production
  under `runtime_index_application/forwarded` (including same-index mutable
  loans and reference forwarding). Construction/bounds/retag and static-kind
  controls reject. Keep this outcome open: general computed-data formation,
  independently proved runtime/result index equality, selected-operator result
  guarantees and nested composition still require whole-item qualification.
  CI candidate `7ad3806d0f45` additionally observed 57 native/Terminal agreements
  and 97 intended rejects on Linux ARM64 and macOS ARM64, using the existing
  hosted runners. Windows execution and the later capture/nested-result source
  changes remain unqualified. Explicit-target image production supplies no
  native/Terminal execution evidence on another host.

- **STRUCTURAL-GENERIC-MATCHING.** Complete
  [static type equality](wiki/spec/language/generics.md#static-type-equality),
  [structural equations](wiki/spec/language/generics.md#structural-type-equations-and-inference),
  and [canonical domain indices](wiki/spec/language/generics.md#canonical-domain-index-matching)
  through specialization, layout, and native execution (`Psi 02–07` and `Omega
  00–09`). Reuse
  `omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/preparation/{type_equations/,machine_equations.rs}`.
  - Extend equation and argument/result composition beyond closed generic
    results in `samples/cli/facts/inferred_array_equation_call/forwarded_result`
    to open schemas, retained extensions, late-selected receivers, operator/conformance supply,
    machine/evidence/value binders, named-lifetime identity, and unspecialized
    body checking under finite type-equality disjunctions.
  - Extend typed const-expression and invocation evaluators to selected computed
    calls and structured const value/equation combinations, and unblock the
    Std-backed customers
    `tests/omega/pass/generics/{omitted_data_binder_range_equation,runtime_generic_carrier_equation_exit}`.

  Acceptance: `samples/cli/facts/inferred_array_equation_call` (all nine places)
  and `tests/omega/pass/generics/{omitted_data_binder_range_equation,runtime_generic_carrier_equation_exit}`
  pass `omega run --both`, while repeated/explicit conflicts, missing indices,
  occurs cycles, kind mismatches, and unmet conformances reject.

  Closed mixed callback/value/conformance selections now complete the
  `inferred_array_equation_call/mixed_binders` nine-place row on Windows x86-64,
  with explicit-capacity conflicts and lexical/kind/carrier controls. The same
  selected value index composes through formal and result types. This covers
  closed mixed selections only; the broader alternatives above and matching
  native observations on the other hosted targets remain open.

- **FINITE-GENERIC-DISPATCH.** Select a finite generic instance at runtime end
  to end (`Psi 02–07` and `Omega 00–09`) under the
  [finite specialization contract](wiki/spec/language/generics.md#finite-specialization-boundary)
  and [dynamic method families](wiki/spec/terminal-psi/dynamic_dispatch.md#finite-generic-method-families).
  Owners:
  `omega-rust/psi/compilation/pipeline/03b_typed-trees-to-typed-trees/src/monomorphization/dynamic_families.rs`
  and `omega-rust/psi/compilation/checking/src/machine_calls/calls/dynamic_requirement.rs`.
  - Complete public slot-relative returned-loan origin contracts for borrowed
    descriptor results, structural field reads rooted in entry content claims,
    and target-ineligible/const-only bridge qualification while keeping
    `tests/omega/run/dyn_finite_family_{runtime_dispatch,owned_carrier,correlated}`,
    `tests/omega/pass/traits/runtime_finite_family_*_result_exit`, and
    `tests/omega/fail/traits/dyn_finite_family_{unproved_membership,wrong_tuple,narrowed_provider}`
    passing.

  Acceptance: pass/fail/run finite-family fixtures execute via `omega run --both`
  preserving effects, index identity, and once-only moves/cleanup across
  selection, forwarding, and storage, while unproved membership, missing/wrong
  rows, and escaping variable-shaped results without an authored sum/descriptor
  reject.

- **TWO-AXIS-TERMINAL-AUTHORITY-REVIEW.** Produce reviewed native artifacts
  without receiver input under the
  [production/admission split](wiki/spec/build/permissions.md#artifact-production-versus-receiver-admission)
  and [critical service audit policy](wiki/spec/packages/review.md#critical-service-audit-policy):
  - Carry core's critical service catalog and root `Build::audit_service<R>`
    selections through install/update review and compact lock acceptance.
  - Settle demanded Linux x86-64 filesystem `BoundaryCall`s (`set_len`) in
    `omega-rust/omega/compilation/realization/provider-planning/src/provider_settlement/canonical_filesystem_host.rs`,
    replace equality-based receiver checks in
    `omega-rust/omega/compilation/pipeline/00_terminal-psi-to-abstract-operations/src/provider_admission/terminal_authority_permissions.rs`
    with demand-based containment, and replace the broad filesystem arm in
    `omega-rust/omega/packages/review-evidence/src/capture/authority.rs` and
    `omega-rust/omega/packages/package-manager/src/review/audit/triage/` with
    exact cohort classifications from
    `omega-rust/omega/compilation/pipeline/00_terminal-psi-to-abstract-operations/src/provider_admission/settlements/source_imports.rs`
    and `omega-rust/omega/packages/package-manager/src/review/candidate/semantic_bindings.rs`.

  Acceptance: `tests/omega/pass/host/console_exit_process_exit` (exit 70),
  Cathedral native smoke, `samples/cli/basics/cli_mvp`, and Squalr build and run
  without a synthesized receiver-policy file; denying policies reject and
  sufficient policies with surplus rows admit.

- **FILESYSTEM-PLATFORM-SERVICES.** (Also covers
  **`FILESYSTEM-RELEASE-CONTRACT`**.) Complete portable and raw
  [platform services](wiki/spec/build/platform_services.md) and
  [bounded occurrence-specific release proofs](wiki/spec/build/permissions.md#bounded-occurrence-specific-release-proof)
  end to end (`Psi 04–07` and `Omega 00–09`):
  - Validate the Windows portable `FilesystemService` implementation natively
    (anonymous SQOS on owned path opens, HANDLE storage, explicit permissions and
    data sync), run `file_journal` (exit 7), `note_vault` (exit 14), Windows raw
    breadth/set-file-time/metadata fixtures (exit 70), and Linux
    `write_all_full_device_exit`, and provide trusted Terminal adapters for the
    exact authored foreign bindings.
  - Run `tests/omega/pass/filesystem/windows_canonicalize_exit` (exit 70 on
    Windows) and wire the occurrence-bound close proof helpers in
    `omega-rust/omega/compilation/pipeline/00_terminal-psi-to-abstract-operations/src/provider_admission/terminal_authority_policy/filesystem.rs`
    so a proved close earns an empty authority classification (keeping
    `tests/omega/pass/filesystem/native_close` as the unconstrained control).

  Acceptance: `file_journal`, `note_vault`, `windows_canonicalize_exit`, and the
  Windows/Linux filesystem fixtures pass `omega run --both` on their matching
  hosts with nested raw-provider closure checks and occurrence-bound close
  classifications, while failed acquisition, handle escape, or unproved close
  retains conservative authority.

- **TR3-TR8.** (Also covers **`BLOCKEXEC`**.) Execute ordinary named machines
  through a selected task runtime and deliver the `source/library/blocking-executor/`
  package end to end (`Psi 04–07` and `Omega 00–09`) under
  [task activation](wiki/spec/build/task_runtime.md),
  [suspension](wiki/spec/terminal-psi/calls_and_outcomes.md#suspension), and
  [foreign execution](wiki/spec/build/foreign_bindings.md#operational-coherence).
  - Join physical frames/spills, `StackPlan`s, and incoming-referent/loan-leaf
    lifetime joins in
    `omega-rust/omega/compilation/realization/provider-planning/src/task_plans/`,
    `omega-rust/omega/compilation/backend/runtime/task-plans/README.md`, stage
    04 `checks/carry.rs`, and stage 05 `retention/suspension_call_plan.rs` to a
    source-selected task runtime performing start, park/resume, cancellation,
    and settlement (upgrading
    `tests/omega/pass/tasks/task_runtime_machine_selection_compile` and
    `pass/core/task_lifecycle_operations` while keeping all
    `pass/concurrency/runtime_*_exit` and `fail/concurrency/*` controls passing).
  - Complete `source/library/blocking-executor/{queue,executor}.omg` (`[T; N]`
    FIFO queue, conserved submission/settlement claims, worker park/wake across
    suspension, and process-isolated hung-worker recovery), upgrading
    `tests/omega/pass/blockexec/blocking_executor_custody_claims_compile` and
    keeping `tests/omega/fail/blockexec/blocking_executor_ticket_scope_loss`.

  Acceptance: task-runtime and `blocking-executor` witnesses execute concurrent
  start/park/resume/finish, FIFO linear-payload queueing, and isolated
  hung-worker recovery via `omega run --both`, while foreign/stale tickets,
  dropped custody, early reclaim, and unbacked suspension borrows reject.

- **TOP-LEVEL-BOUNDARY-REQUIREMENTS.** Retire standalone `boundary requirement
  Owner::name(...);` declarations and automatic containing-trait reach across
  `source/library/`, `samples/`, `tests/omega/`, `omega-rust/`, and
  `source/{psi,omega}/` under the
  [machine-supply rule](wiki/spec/language/machines.md#supply) and
  [explicit boundary reach](wiki/spec/language/effects.md#implementation-boundaries-and-capability-identities).
  - Migrate the remaining 84 std raw platform, 2 interrupt, and 126 float rows
    plus 78 test files to boundary traits with explicit reach clauses, lift stage
    04's template-dependent result borrow-elision fence for `core/task.omg`'s
    generic `request_cancel` and `finish`, and preserve source-selected trait
    tokens (`samples/cli/probes/operator_requires_effects`).
  - Delete the standalone declaration kind from parsing, AST/IR representations,
    checking, Terminal codec, provider planning, and both compilers, keeping one
    parser fail fixture for the retired spelling.

  Acceptance: no accepted source or IR contains standalone `boundary requirement`
  declarations; migrated customers execute through `omega run --both` under
  boundary-trait selection; and unselected, ambiguous, or reach-exceeding
  providers reject.

- **COMPONENT-SUBSTRATE.** (Also covers **`WIRE-RUNTIME-AND-INSTALLATION`**.)
  Execute two-package `Independent` composition, stable-slot replacement, and
  receiving-provider executable installation end to end (`Psi 04–07` and `Omega
  00–09`) under
  [component publication](wiki/spec/build/component_publication.md) and
  [executable installation](wiki/spec/build/executable_installation.md).
  - Rejoin physical stack demand, provider plans, root imports, and runtime
    products before publication across Linux ELF, macOS Mach-O dylib, and
    Windows PE, extending `samples/cli/facts/opaque_closed_carrier_exchange`
    and replacement-era draining.
  - Connect `OwnedImageProvider` in
    `omega-rust/omega/compilation/backend/runtime/executable-installation` and
    artifact binding in
    `omega-rust/omega/compilation/backend/images/image-emission/src/` to an
    authored/package provider and executor join that installs, invokes at
    distinct placements, and retires with quiescence and W^X enforcement.

  Acceptance: a two-package `Independent` build and an installed-image customer
  execute crossing calls, placement-independent invocation, new-era routing, and
  old-era drain on matching hosts, while omitted/duplicate assumptions,
  substituted bytes, and premature retirement reject.

- **RUNTIME-SIZED-ACTIVATION-CONTRACT.** Complete target execution for
  source-authored [bounded activation claims](wiki/spec/resources/activation_storage.md)
  beyond `samples/cli/facts/bounded_activation` (all nine places passing `omega
  run --both`): keep co-live bounds, exclusive branches, `claim_split` covering
  splits/merges, and across-suspension caller-home preservation working across
  all four hosted targets while `committed > bound` returns checked failure and
  open demands, escaping claims, or non-LIFO releases reject.

## Checked boundary topology

- **TOPOLOGY-PLAN-VERIFICATION.** (Also covers
  **`TOPOLOGY-PRIVATE-PIPE-INSTALLATION`**.) Deliver the
  [checked boundary topology contract](wiki/spec/packages/topology.md) end to
  end:
  - Update `tests/omega/pending/build/build_scope_topology_plan` to current
    `Build` facets and drive `source/library/topology/verify.omg` over ordinary
    component-description outputs (`payments.plan`) against a supplied
    `TopologyRequest`.
  - Run the [three-process payment customer](wiki/spec/packages/topology.md#first-executable-realization)
    on Windows and macOS with an Omega-authored installer, private-pipe
    adaptation, explicit Windows handle transfer, macOS validated/signed member
    images, and generation replacement.

  Acceptance: `build_scope_topology_plan` and `verify.omg` admit and verify
  `payments.plan` and execute the three-process payment topology on Windows and
  macOS with schema-checked private pipes and quiesced generation replacement,
  while missing/substituted components, cycles, bypasses, and ungranted
  endpoints reject.

## Rust compiler release closure

- **RC-RELEASE-RECORD-AND-CLOSURE.** (Also covers **`RC-REPOSITORY-CLOSURE`**,
  **`IND19-SPECTRUM-VALIDATION-RED`**, **`RC-NATIVE-MATRIX-LINUX-X86-64`**,
  **`RC-NATIVE-MATRIX-LINUX-ARM64`**, **`RC-NATIVE-MATRIX-MACOS-ARM64`**, and
  **`RC-NATIVE-MATRIX-WINDOWS-X64`**.) Produce the same-commit release record
  across all four required hosts (`linux_x86_64`, `linux_arm64`, `macos_arm64`,
  `windows_x86_64`) via `tools/release/release_record.py` under the
  [completion contract](wiki/drafts/reference/rust_compiler_completion.md):
  - Repair the `ind19-spectrum-validation` API drift
    (`TerminalRuntimeRequest.x86_flags`, `CheckedCompilation::into_program`) and
    pass `python tools/fmt.py --check`, workspace all-target Clippy, workspace
    all-target check, and `mbx nextest run --workspace --lib --no-fail-fast` at
    one clean commit.
  - Pass `RC-NATIVE-MATRIX` and `RC-REPRESENTATIVE-PROGRAMS`
    (`tools/sample_native_gate.py`) with direct native execution on Linux
    x86-64, Linux ARM64, macOS ARM64, and Windows x86-64 (including Windows
    provider rows or scoped target bindings for POSIX-only fixtures and
    `tests/omega/pass/filesystem/windows_set_file_time_exit`).

  Acceptance: four same-commit host records pass `tools/release/release_record.py
  close` with zero crate or fixture exclusions.

- **RC-BUILD-AND-PACKAGES.** Pass nextest and doctests for `build-declarations`,
  `package-source`, and `resolver-execution` at the release commit, and pin
  end-to-end corpus/package witnesses for:
  - Package and selection identity in
    `omega-rust/psi/compilation/pipeline/04_typed-trees-to-checked-trees/src/authored_selections/finalization.rs`
    (`allow_unresolved_toolchain`);
  - Private progress premises in stage 04 `checks/contracts/calls.rs` and
    `checks/termination/progress/components.rs`;
  - Demanded imports and callback custody in
    `omega-rust/omega/compilation/native-production/src/realization.rs`
    and stage 00 `source_evaluated_bindings.rs`;
  - Cross-package foreign-domain mutable recasts;
  - Borrowed suspension and terminal transition arms; and
  - Host-provider execution (`source/library/std/targets/<target>/time_host_impl.omg`,
    `pass/time/runtime_time_elapsed_since_exit`,
    `pass/providers/foreign_out_parameter_write_observed_exit`, and
    `tests/omega/pass/objc/`).

  Acceptance: all six integration witnesses execute via `omega run --both` on
  their target hosts and the `RC-BUILD-AND-PACKAGES` gate passes at the release
  commit.

- **MACOS-X64-HOST-PROFILE.** Record `tests/omega/corpus_native_macos_x86_64.txt`
  on an Intel Mac via `python3 tools/corpus_gate.py --native --record`, compare
  with the Rosetta leg (`--native-target macos_x86_64`), and record `omega
  inspect-terminal --target macos_x86_64` per
  [host observations](wiki/drafts/measurements/macos_x64_host_profile.md).

## Omega-written compiler (after Rust completion)

- **OMEGA-PRODUCT-COMPILER-SOURCE.** Finish the Omega-written product compiler
  in `source/psi/` and `source/omega/` (rooted at `source/omega/{build,main}.omg`)
  under the [compiler product contract](wiki/spec/build/compiler_request.md) and
  [the Psi source guide](source/psi/README.md) so one `OCREQ V1` request
  produces one `OCOUT V1` outcome across the full language:
  - Complete request semantics (`source/omega/request/` phases 3, 4, 5, 7, and 8
    with shared package-origin identity and `tests/product/request/test-request.sh`);
  - Complete parsing, resolution, typing (`Equatable` vs. proof-relevant `Id`
    under [data identity in facts](wiki/spec/proofs/contracts.md#data-identity-in-facts)),
    checking/proof (`embed`, storage origins, payload loan paths, and
    destination-first stores), and Terminal Psi + Omega realization;
  - Close bundled-library and entry refusals (`std/calling.omg`,
    `core/slice.omg`, `std/process_exit.omg`, `core/option.omg`, `std/build.omg`,
    and `Console::read_byte()`); and
  - Publish scalar-resource capacity refusals and pass
    `tests/psi/{parser,resolver,typing}/` and Rust/Omega differential checks on
    Linux, Windows, and macOS.
