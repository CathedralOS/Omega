# slice_0013

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0093 | omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/execution_terminal_unit.rs:5116-5164 | UNDECIDED | Unlisted `OperationPlan` / `BindingPlan` arms contribute no cleanup transfers; whether a program that depends on those moves can still be accepted was not traced past transfer planning. |
| A0094 | omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/execution_terminal_unit_calls.rs:5678-5691 | UNDECIDED | Projected mutable borrow at an expression root can return `StructuralResult` without `exact_structural_argument_access`; stage-05 flow/borrow refusal before this planner was not read. |
| A0095 | omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/execution_terminal_unit_calls.rs:6172-6174 | UNDECIDED | Unknown-domain qualification rows are dropped in `retain`; whether an obligation to reject the parameter is enforced earlier in domain publication was not read. |
| A0096 | omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/execution_terminal_unit_cleanup.rs:929-991 | UNDECIDED | `consumed_claims` returns `Some(empty)` when no matching `Consume` event; whether hooks without recorded linear claims are rejected elsewhere in cleanup planning was not read. |
| A0097 | omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/execution_terminal_unit_control.rs:398-402 | CLOSED | `reference_outputs` is skipped only when `checked_family` or `preserves_descriptors`; other boundaries still call `reference_outputs` at the same site. |
| A0098 | omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/execution_terminal_unit_control.rs:2546-2563;1670 | REACHABLE | Byte-sequence predicates with no matching flow call and transition guards with no Guard computation root both let collection/scheduling succeed without recording those computations. |
| A0099 | omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/execution_terminal_unit_control.rs:1881-1898 | REACHABLE | Sibling statement calls are allowed when marked `retired` in flow facts; a call wrongly listed in `retired_calls` is not required to be scheduled here. |
| A0100 | omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/execution_terminal_unit_control_statement_sequence.rs:1350;5502-5506;6770-6783;7208-7221;9224-9228 | UNDECIDED | Multiple intentional early-continues/skip paths (erased dispatch subject, future loans treated retired, no release without loans/completion, fact-only citations); digest names no wrong program and accept-vs-reject upstream was not traced for each. |

## A0093

**Cited site (read):** `execution_terminal_unit.rs:5103-5166` in `computations::Query::operation_computations` (`omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/execution_terminal_unit.rs`). Lines match the digest cite at this checkout (not re-diffed with `git show 457adf4a11f:`).

The function always ends with `Some(output)` (`:5165`). For `OperationPlan::EstablishLocal` and `OperationPlan::Store`, only listed binding/store value variants call `scalar`, `bytes`, or `argument_computations`; all other variants use `_ => {}` (`:5130`, `:5145`). A final `_ => {}` on the outer `match operation` (`:5163`) ignores every other operation kind.

**Callers (read):** `rg 'operation_computations\(' omega-rust` → 2 matches: definition at `:5103` and `Query::operation` at `:5805`, which appends the result and continues planning transfers.

**Upward (read):** `Query::operation` is used from completion/cleanup transfer builders (for example `outgoing_operation` at `:4969-4970`, `through_prefix` at `:5637`, `operation_sequence` at `:5050`). Failures propagate as `Option`/`Result` only when a invoked helper returns `None`, not when an operation kind is silently skipped.

**Input that reaches the branch (read):** A machine body whose checked effect plan includes an operation or local-binding kind outside the matched arms (for example structural local binding forms other than the listed scalar/primitive/array/byte variants, or store values other than computation/place/bytes), during cleanup transfer graph construction.

**Verdict:** UNDECIDED — skipped arms omit transfer edges; whether compilation is still rejected by flow/ownership facts before or after this query was not traced (digest: “no wrong program named”).

## A0094

**Cited site (read):** `execution_terminal_unit_calls.rs:5677-5691` inside `argument_contract` (`:5256-5691+` same file). Cite matches.

For `CheckedStructuralAccess::MutableBorrow | WriteOnlyBorrow`, when `projected` is true, `expressions(program).borrow_expression(expression)` is `None`, and `place.root` is `PlaceRoot::Expression(_)`, the function returns `Some(CheckedUnitStructuralArgumentPlan { source: StructuralResult { .. }, .. })` without calling `exact_structural_argument_access` (`:5678-5690`). The adjacent arm at `:5712-5717` requires `exact_structural_argument_access(...) == access`.

**Enclosing API (read):** `result_arguments::argument` (`:5207-5252`) wraps `argument_contract` and is reached from structural call/result argument planning (`:6564`, `:8243` in the same file; `rg 'result_arguments::argument\(' omega-rust` → 2 production call sites).

**Callers of `argument_contract` (read):** Direct definition call from `argument` at `:5223`; `argument_contract` name appears only in this file for stage-05 planning (other `argument_contract` hits are unrelated provider-admission/optimization symbols).

**Input that reaches the branch (read):** A projected mutable or write-only borrow argument whose canonical place is rooted at an expression handle, where the typed expression table has no `borrow_expression` node for that handle, during call-result argument planning for a structural parameter.

**Verdict:** UNDECIDED — the bypass is explicit in the planner, but whether an invalid borrow would already be rejected in flow or borrow checking before this `Option::Some` plan row is retained was not read.

## A0095

**Cited site (read):** `execution_terminal_unit_calls.rs:6167-6183` in `parameter_signatures` (`:6093-6190+`). At `:6172-6173`, `matching.next()` failing on `shapes.domains` causes `retain` to drop the row (`return false`); planning continues and still pushes `CheckedUnitStructuralParameterPlan` (`:6184-6188`).

**Callers (read):** `rg 'parameter_signatures\(' omega-rust` → 7 symbol occurrences across `execution_terminal_unit_calls.rs` (definition plus `free_structural_scalar_signature` path at `:6072`), `execution_terminal_unit_control.rs` (`:485`, `:721`, `:904`), and `declarations_core.rs` (5 uses, not opened).

**Input that reaches the branch (read):** A structural formal type whose shell adds `additional_input_qualifications` rows referencing a `domain` id that does not appear in `ShapeCollector::domains`, during state or boundary signature planning.

**Verdict:** UNDECIDED — qualifications are silently omitted rather than failing `parameter_signatures`; whether domain establishment or contract checking rejects the program earlier was not read.

## A0096

**Cited site (read):** `execution_terminal_unit_cleanup.rs:929-991`, function `consumed_claims` in module `actions`. Cite matches.

The loop at `:970-989` selects permission events matching machine, state, source, root, `Consume`, linear owned access, and path. If no event matches, `claims` stays empty and the function returns `Some(claims)` at `:990` (not `None`). `None` is returned only when a matched event has unknown identity/provenance, live obligation, or duplicate claim (`:981-987`).

**Caller (read):** `rg 'consumed_claims\(' omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees` → 2 matches: definition `:929` and assignment at `:456-458`, which requires `consumed_claims(...).ok_or("cleanup hook has no exact consumed claims")?` — an empty vector still passes `ok_or`.

**Input that reaches the branch (read):** A nominal cleanup hook leaf on a path where flow ownership has no linear `Consume` event matching disposition root/segments, during `DispositionActions` construction for contextual cleanup (`:402-459` region read).

**Verdict:** UNDECIDED — empty consumption is explicitly allowed at this site; whether flow cleanup disposition construction or earlier permission publication refuses hooks without consumable claims was not read.

## A0097

**Cited site (read):** `execution_terminal_unit_control.rs:384-402` inside `build_boundary_machine` (`:296-416+`). Cite matches.

`checked_family` is true when the machine symbol is a canonical target family whose members all `supplies_checked_body()` (`:384-392`). `preserves_descriptors` comes from `static_interrupt_declaration` when entry matches the state and `preserves_parameter_descriptors()` (`:393-397`). When either holds, `reference_outputs` is set to `Vec::new()` (`:398-399`); otherwise `reference_outputs(...)?` runs (`:401`).

**Callers (read):** `rg 'build_boundary_machine\(' omega-rust` → 2 matches: definition `:296` and `execution_terminal_unit.rs:7104` (boundary machine planning in unit builder, caller body not fully read).

**Closing behavior (read):** Only the two named predicates skip output planning; all other boundary machines still evaluate `reference_outputs` (`:1019+` function read through signature list mapping). This is gated at the single assignment before `CheckedBoundaryMachinePlan` construction (`:416`).

**Verdict:** CLOSED — for boundaries outside checked families and without descriptor-preserving static interrupts, reference output rows are still computed at `:401`; the empty vector is limited to the two documented machine classes at this site.

## A0098

**Cited sites (read):** Both in `execution_terminal_unit_control.rs`, module `collecting`.

1. **`collect` / `ComputationKind::ByteSequencePredicate` (`:2527-2563`):** After `collect_subject!`, byte predicate metadata is validated (`:2534-2540`). Flow calls matching `authored_expression` are enumerated (`:2542-2545`). If `occurrences.next()` is `None`, the block `:2546-2562` is skipped entirely; `collect` does not return `None` for that reason and proceeds to finish the computation kind handling.

2. **`statement_computation_prefix` (`:1645-1736`):** For `TransitionGuardNode::When(expression)` (`:1660-1665`), guard roots are taken from scalar computation plans (`:1668-1669`). The inner block runs only `if let Some(root) = roots.next()` (`:1670`). If there is no guard root, control falls through without returning `None`, and the function continues to `collect_constructor_calls` / assignment collection and returns `Some((consumed, guard_maximum))` with `guard_maximum` still `None` (`:1710-1735`).

**Callers (read):** `rg 'statement_computation_prefix\(' omega-rust` → 3 matches (definition `:1645`, uses inside `call_owns_statement_computations` at `:1775` and state-graph preparation at `:3239`). `collect_root_calls` / `collect` referenced 13+ times within `execution_terminal_unit_control.rs` (`rg 'collect_root_calls\(' omega-rust` → 10 symbol occurrences in that file).

**Consumer (read):** `call_owns_statement_computations` (`:1745-1899`) calls `statement_computation_prefix` when not using selected invocation accounting (`:1773-1782`), then later requires every other statement call to satisfy the disjunction at `:1881-1897` (including `is_retired`).

**Input (read):** (1) A byte-sequence predicate computation embedded in scalar plans whose source call expression has no corresponding `FlowCallFact` row at planning time. (2) A transition with `when` guard expression but no retained `ExpressionRole::Guard` scalar computation root.

**Verdict:** REACHABLE — both paths allow scheduling/accounting to succeed without binding guard or predicate call occurrences that might still exist in the statement or flow tables.

## A0099

**Cited site (read):** `execution_terminal_unit_control.rs:1881-1898` at the end of `call_owns_statement_computations` (`:1745-1899`). Cite matches.

After consuming nested and argument calls, the function requires every other call on the same `statement_index` to satisfy one of: same pointer as the scheduled call, `scheduled_subject`, `scheduled_by_active_arm`, `other_transition_arm`, `facts.flow.control.is_retired(state, candidate)`, builtin with `has_empty_write_frame()`, or already in `consumed` (`:1881-1897`). Success is `.then_some(())` (`:1898`).

**`is_retired` (read):** `flow_ownership_and_roots.rs:417-423` returns true when `(state, statement_index, call_ordinal)` appears in `retired_calls`.

**Callers (read):** `rg 'call_owns_statement_computations\(' omega-rust` → 2 matches: definition `:1745` and `execution_terminal_unit_state_graph.rs:7937` before `build_call_operation` on a terminal call arm.

**Input that reaches the branch (read):** A `FlowCallFact` on the same statement that was inserted into `retired_calls` during flow construction but still represents work that should execute on the active transition arm, when a composed terminator schedules the terminal call.

**Verdict:** REACHABLE — a falsely retired sibling call satisfies `:1889` and is not required to appear in the consumption ledger, so `call_owns_statement_computations` can return `Some(())` without scheduling it.

## A0100

**Cited sites (read):** All in `execution_terminal_unit_control_statement_sequence.rs` unless noted.

| Site | Behavior (read) |
| --- | --- |
| `:1350-1351` in `local_data::plan_inner` | `validation::dispatch_subject_of_erased_binding` (`proof_contracts_core.rs:9870-9904` read for predicate) → immediate `Some(LocalPlan::Planned)` with no initializer operations. |
| `:5502-5506` in `holder_retired_inner` | If `!order.precedes(loan.statement_index, self.statement_index)`, returns `true` (loan treated retired though formed at or after current statement). |
| `:6770-6782` in `append_reference_releases` | When borrow state missing/empty and no captured reference results in operations/prepared calls, `return Some(())` without appending releases. |
| `:7208-7221` in `append_exit_reference_releases` | Same empty-loan guard as above; additionally `:7230-7231` returns `Some(())` when `!prepared_completion.has_normal_completion()`. |
| `:9224-9228` in statement-sequence driver | Skips `Assembly` / `AssemblyFact` (`:9220-9224`) and `execution::citation_erasure::is_fact_only_citation` statements (`:9226-9228`; helper at `execution.rs:55-66` read). |

**Callers (read):** `plan_inner` via `local_data::plan` (`:1297-1309`). `append_reference_releases` called from the sequence driver at `:9204-9217` (`rg 'append_reference_releases\(' omega-rust` → 3 matches in this file). `append_exit_reference_releases` (`rg` → 2 matches in this file).

**Input (read):** Erased transition-subject locals; loans not yet established at the retirement statement; statements with no loans and no captured reference results; abnormal completion edges; assembly/fact-only citation statements — each path continues planning without emitting the corresponding operations.

**Verdict:** UNDECIDED — comments and digest treat these as intentional skips; whether any of them let an otherwise invalid program reach checked-tree acceptance without a separate diagnostic was not traced end-to-end.

## What I did not read

- `git show 457adf4a11f:` line-for-line diff for each cite (current checkout lines matched on spot checks only).
- Full flow/borrow checking paths that run before `argument_contract`, `parameter_signatures`, or cleanup dispositions.
- Corpus or sample programs witnessing REACHABLE cases.
- Stage-06 lowering behavior for empty `consumed_claims` or skipped reference releases.
- `declarations_core.rs` call sites of `parameter_signatures` beyond grep hit list.
- Complete `collect`/`collect_root_calls` fan-out beyond `call_owns_statement_computations` and state-graph terminator.

SLICE COMPLETE
