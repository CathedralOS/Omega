# slice_0071

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0545 | recasts.rs:2818-2820 (digest rc:4818-4820) | REACHABLE | Missing `storage_index` forces `shared_mutation` false and can skip the writable-qualification-erasure lender block when the recast view is not writable. |
| A0546 | recasts.rs:1028-1050, 4660-4664 | REACHABLE | Non-`Domain` constraints make `declared_qualifications` return `None`, so qualification mismatch is not diagnosed as `Some(false)`. |
| A0547 | reference_result_custody.rs:3354-3356 | UNDECIDED | `Image` carriers are treated as absent at a leaf, but no ordinary-program witness that needs a loan was identified. |
| A0548 | reference_result_custody.rs:8795-8799 | REACHABLE | Invocation-rooted residual return sources satisfy `transient_call_record_loans` without comparing call arguments to parameters. |
| A0549 | reference_result_custody.rs:7020-7022 | REACHABLE | Data definitions whose `supply_mode` is not `CheckedShape` make `is_reference_record` false even if fields could hold references. |
| A0550 | reference_result_custody.rs:7685-7687, 7742-7747 | REACHABLE | `NumericOwned` and activation-byte carriers exit `declared_carrier_references` without scanning nested references. |
| A0551 | structural_call_custody.rs:642-656 | REACHABLE | Unspellable `where` facts in `requirement_rows` emit no predicate row and do not error on the production `state_declaration_predicates` path. |
| A0552 | structural_call_custody.rs:438 | UNDECIDED | `is_none_or` at 438 is only reached via `declaration_facts_carried`, which has no production caller in `omega-rust`. |

## A0545

**Verdict:** REACHABLE

**Digest entry:** `rc:4818-4820` — no storage index means "no atomic storage", which disables the qualification-erasure check.

**Re-location:** Digest lines `rc:4818-4820` at commit `457adf4a11f` do not match this checkout; the same logic is in `type_has_atomic_storage` at `recasts.rs:2818-2820` (read).

**Cited code (read):** At `recasts.rs:2818-2819`, when `program.normalized_type_identity(reference).storage_index()` is `None`, `type_has_atomic_storage` returns `false` immediately. At `recasts.rs:1601`, `form_scalar_recast` sets `shared_mutation = type_has_atomic_storage(program, cast.target_type)`. At `recasts.rs:1611-1655`, the writable qualification-erasure / lender-validity refusal runs only when `qualification_compatible == Some(true)` and `(writable_view || shared_mutation)` and the reverse compatibility test fails.

**Callers (search `\btype_has_atomic_storage\b` in `omega-rust`, count mode):** 3 matches, all in `recasts.rs` (import at 1435, use at 1601, definition at 2811) (read).

**Upward trace (read):** `type_has_atomic_storage` is used only from `form_scalar_recast` (`recasts.rs:1601`). `form_scalar_recast` is called from `judge_scalar_recast` (`recasts.rs:1461`). `judge_scalar_recast` is reached from `validate_recasts` (`recasts.rs:5324`, 5376). `validate_recasts` is a prologue validator in `program_validation.rs:573`, which appends to `diagnostics` during `validate` (inferred from prologue list at `program_validation.rs:568-574`).

**Input that reaches the branch:** A machine with a scalar reference recast (`&x as &T` or similar) where `qualification_view_compatible` is `Some(true)`, the recast view is not writable (`writable_view` false), the target type’s normalized identity has no `storage_index` (so `shared_mutation` is false), and erasing or narrowing qualifications on the target would fail the lender-implication test at `recasts.rs:1636-1647` if that block ran.

**Plain words:** A program that performs a shared (non-writable) qualified recast on a type the layout pass never gave a storage index, where the target’s domain qualifications are not preserved by the source lender, should be refused for unsafe qualification erasure, but validation accepts it because the lender check is skipped when both `writable_view` and `shared_mutation` are false.

## A0546

**Verdict:** REACHABLE

**Digest entry:** `rc:1028-1050` with `rc:4660` — non-`Domain` constraints yield `None`, read as "no qualification issue"; `ArithmeticDomain` is ignored.

**Cited code (read):** In `declared_qualifications` (`recasts.rs:1028-1050`), each `TypeConstraintNode::Domain` is collected; any other constraint kind hits `return None` at `1039-1040` (including invalid domain handles). In `mutable_scalar_representation_facts` (`recasts.rs:4653-4664`), `ArithmeticDomain(_)` is skipped (`4660`), while invalid `Domain` or `Named` constraints return `None` from the whole function (`4662-4663`).

**Callers for `declared_qualifications` (search `\bdeclared_qualifications\b` in `omega-rust`, count):** 6 matches — 5 in `recasts.rs` (definition and uses inside `qualification_views`) plus 2 in `execution_terminal_unit_types.rs` in pipeline 05 (read, count from grep).

**Upward trace via `qualification_view_compatible` (read):** `qualification_view_compatible` (`recasts.rs:972-985`) returns `None` if either `declared_qualifications` call fails (`977-978`). In `form_scalar_recast` (`recasts.rs:1602-1604`), `qualification_compatible` is `Option` from that helper. Only `qualification_compatible == Some(false)` pushes a refusal (`1605-1609`). The erasure lender block at `1611+` requires `Some(true)`. When the result is `None`, neither refusal nor lender check runs.

**Input:** A recast whose source or target type reference is wrapped in constrained types where spellable qualifications include only `ArithmeticDomain` and/or constraints that force `declared_qualifications` to return `None`, while the actual domain qualification set on the carrier is incompatible with the recast target.

**Plain words:** A program whose qualified reference types use constraint shapes the reader does not interpret as `Domain` constraints should be rejected when qualifications do not carry across the recast, but validation treats the situation as having no qualification issue because compatibility stays `None` instead of `Some(false)`.

## A0547

**Verdict:** UNDECIDED

**Digest entry:** `rrc:3354-3356` with `rrc:4516-4535` — `Image` sources count as not present so the leaf needs no loan; reader: "none identified for ordinary programs".

**Cited code (read):** In `expression_leaf_present` (`reference_result_custody.rs:3352-3356`), `StoredCarrierSource::Image { .. }` sets `supplied = false`. At `4523-4534`, when a return leaf is not loan-captured, `expression_leaf_present` is consulted; if `present` is false, the function does not return `None` (only `if present { return None }` at 4532-4533).

**Callers for `expression_leaf_present` (search in `omega-rust`, count):** 6 matches, all in `reference_result_custody.rs` (definition and internal uses including `4523`) (read).

**Upward trace (read):** Return-origin planning for reference-record loans uses this path when matching uncaptured leaves (`reference_result_custody.rs:4516-4535` region). Structural call custody ties reference-record loan obligations through `local_owes_record_loans` and related helpers (`structural_call_custody.rs:3595-3610`).

**What would decide:** Whether ordinary typed programs can place a returned reference-record leaf whose only `stored_carrier_sources` entry is `StoredCarrierSource::Image`, such that `present` is false but a loan is still semantically required. The area digest reader reported no ordinary-program witness (read, `area_14.md` GROUP 13 bullet).

## A0548

**Verdict:** REACHABLE

**Digest entry:** `rrc:8795-8799` — invocation-rooted sources accepted without comparing any argument.

**Cited code (read):** In `transient_call_record_loans` (`reference_result_custody.rs:8794-8799`), for each filtered return source, `CheckedReferenceOriginRoot::Invocation { .. }` makes the closure return `true` immediately. Parameter-indexed sources continue to compare borrow kinds and type identity (`8801-8824`).

**Callers (search `\btransient_call_record_loans\b` in `omega-rust`, count):** 2 matches — definition at `8769`, production use at `structural_call_custody.rs:3600` (read).

**Upward trace (read):** At `structural_call_custody.rs:3595-3610`, when `local_owes_record_loans` holds and `reception.residual_paths` is non-empty, failure of `transient_call_record_loans` yields `Err("reference record result has no exact captured leaf loans")` and blocks the structural call plan. Success allows the call to proceed.

**Input:** A call whose return type is a reference record, with non-empty residual paths, where a residual leaf’s origin root is `Invocation` but the actual argument at the corresponding parameter would not justify the child borrow.

**Plain words:** A program that returns a reference record from a call with invocation-rooted residual leaves should be rejected when actual arguments do not reborrow correctly, but custody accepts those leaves without inspecting arguments.

## A0549

**Verdict:** REACHABLE

**Digest entry:** `rrc:7020-7022` — boundary/opaque data storing a reference would not be a 'reference record'.

**Cited code (read):** In `is_reference_record` (`reference_result_custody.rs:6988-7043`), inner `visit` at `7016-7021` loads the data definition and returns `None` when `data.supply_mode != language_semantics::DataSupplyMode::CheckedShape`. The outer function treats `visit(...) == Some(true)` as the only true case (`7043`); `None` yields false.

**Callers for `is_reference_record` (search in `omega-rust`, count):** multiple production sites in validation and pipeline 05 (read: `reference_result_custody.rs`, `checked-trees/facts.rs`, `execution_terminal_unit_*` modules).

**Upward trace (read):** `local_owes_record_loans` (`reference_result_custody.rs:8012-8018`) requires `is_reference_record(program, reference)`. Reference-record call-source analysis at `8762-8763` also gates on `is_reference_record`.

**Input:** A return or parameter type referencing a data definition with non-`CheckedShape` supply mode (boundary/opaque) whose stored fields include references.

**Plain words:** A program that stores references inside boundary or opaque data should trigger reference-record loan rules, but validation classifies the type as not a reference record, so loan obligations attached to that shape are not applied.

## A0550

**Verdict:** REACHABLE

**Digest entry:** `rrc:7685-7687`, `7742-7747` — `NumericOwned` contents and activation byte carriers count as holding no references.

**Cited code (read):** In `declared_carrier_references`’ inner `visit` (`reference_result_custody.rs:7685-7687`), `has_contents(..., NumericOwned)` returns `Some(())` without enumerating child references. At `7742-7747`, a `Named` type with `activation_claim_byte_carrier` also returns `Some(())` without descending into fields.

**Callers (read):** `declared_carrier_references` is used from reference-result custody and structural loan planning (`reference_result_custody.rs` grep: production uses at 4170, 4919, 6501, etc.).

**Upward trace (read):** Loan and origin planning for calls and locals consumes the leaf list from `declared_carrier_references`. Missing leaves mean downstream checks do not demand borrows for hidden references.

**Input:** A carrier type classified as `NumericOwned` or as an activation-byte carrier name, where executable layout still contains a reference field nested inside the represented contents.

**Plain words:** A program whose type is treated as numeric-owned or activation-byte storage but actually carries references should require borrow tracking on those references, but validation’s carrier scan stops early and treats the type as reference-free.

## A0551

**Verdict:** REACHABLE

**Digest entry:** `scc:642-656` — unspellable `where` facts get predicate rows silently missing; `declaration_facts_carried` has no production caller.

**Cited code (read):** In `requirement_rows` (`structural_call_custody.rs:642-656`), for each conjunctive proof fact, if `spelling.boolean` and `collection_predicates::data_fact_element_consequences` both fail, the loop `continue`s without pushing a row and without diagnostic. `RowEmitter::definition_rows` always calls `requirement_rows` for emitting definitions (`620`). `state_declaration_predicates` (`275-321`) builds rows through `RowEmitter` and is exported.

**Fence note (read):** `declaration_facts_carried` (`scc:361-373`) is separate; grep `\bdeclaration_facts_carried\b` in `omega-rust` gives 7 hits (1 definition, 1 `lib.rs` re-export, remainder tests at `scc:1600`, 2021, 2115, 2152, and test module export at 2349) — no production caller (read).

**Upward trace (read):** `state_declaration_predicates` is called from pipeline 05 (`published.rs:9910`, `execution_terminal_unit_state_graph.rs:9756`, etc.), not from `program_validation.rs` prologue (grep: no matches in `program_validation.rs`) (read). Missing rows still allow the checking pipeline to proceed; incomplete predicates are not surfaced as validation errors at emission time.

**Input:** A state parameter whose data definition carries a `where` fact expressible in the fact table but not translatable by `FactSpelling` or `data_fact_element_consequences`.

**Plain words:** A program that relies on a declaration `where` invariant that the spelling layer cannot encode should be rejected or carry an explicit predicate obligation, but row emission silently omits that fact and compilation continues without a diagnostic at this site.

## A0552

**Verdict:** UNDECIDED

**Digest entry:** `scc:438` — `is_none_or` returns `true` for a carrier with no closed data definition.

**Cited code (read):** In `complete_owned_storage` (`structural_call_custody.rs:438-446`), the default arm uses `owned_data_definition(program, carrier).is_none_or(|definition| complete_definition_storage(...))`; when `owned_data_definition` is `None`, `Option::is_none_or` yields `true`, so the function reports owned storage as complete without rows.

**Callers (read):** `complete_owned_storage` is called only from `complete_definition_storage` (`397-403`). `complete_definition_storage` is invoked from `declaration_facts_carried` (`366-372`) and tests (`1589-1596`). Grep shows `declaration_facts_carried` has no production caller outside tests and re-export (`structural_call_custody.rs`, `lib.rs`) (read).

**What would decide:** Whether any production acceptance path outside the test-only `declaration_facts_carried` fence calls `complete_owned_storage` with the `is_none_or` branch at 438. Production predicate emission uses `RowEmitter::descend`, which returns early when `owned_data_definition` is `None` (`539-540`) — a different mechanism (read).

**Plain words:** The cited `is_none_or` behavior affects only the unused `declaration_facts_carried` completeness fence; impact on whether `validate` accepts a program was not determined from production call sites.

## What I did not read

- Full bodies of `validate_recasts`, `validate`, and all `expression_leaf_present` callers beyond cited regions.
- Pipeline 05 consumers of `DeclarationPredicateRow` beyond grep-hit lines in `published.rs` and `execution_terminal_unit_state_graph.rs`.
- Commit `457adf4a11f` file snapshots via `git show` (re-location by symbol name only for A0545).
- Wiki/spec chapters for intended semantics of boundary data, `Image` carriers, or qualification erasure.
- `stored_carrier_sources` implementation defining when `Image` variants appear.

SLICE COMPLETE
