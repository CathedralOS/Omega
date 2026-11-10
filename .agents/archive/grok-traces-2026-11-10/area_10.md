# Area 10 open entries

Read-only trace of every entry classed "open" in `.agents/atlas/compiler-2026-11-10/digests/area_10.md`. Citations were checked in the working tree on `work/grok-traces-2026-11-10`. Stage 06–08 sources match the atlas commit for the lines opened here (the digest's `file:line` sites were still the sites described). Lines below were read unless a sentence says inferred. Nothing was built or run.

Paths: a bare file name is under `omega-rust/psi/compilation/pipeline/06_checked-trees-to-lowered-psi/src/`.

| # | Site | Verdict | Reason |
| --- | --- | --- | --- |
| 1 | `attached_unit.rs:541, 547` | UNDECIDED | A structural-result field ensures is dropped from the boundary declaration; body discharge was not read. |
| 2 | `attached_unit.rs:6290` | UNDECIDED | Any unprojected authored requires drops the whole list; caller discharge was not read. |
| 3 | `attached_unit.rs:7101` | UNDECIDED | An unclassified boundary symbol contributes no provider candidates and lowering continues. |
| 4 | `attached_unit.rs:6711-6717` | UNDECIDED | An invalid receiving handle becomes a blank application and the requirement is still emitted. |
| 5 | `attached_unit.rs:8375-8377, 8431-8434, 8467-8472` | UNDECIDED | Unreferenced, already-valued, and landed rows are skipped; completeness of the reference set was not read. |
| 6 | `attached_unit.rs:8071-8073, 8087` | UNDECIDED | A failed description is stored as Unavailable and the codec allows a non-empty reason. |
| 7 | `attached_unit.rs:9345` | UNDECIDED | Open families are omitted, and the completeness check counts only the closed set. |
| 8 | `attached_unit.rs:13701` | UNDECIDED | A returned byte or element view is left out of the origin list. |
| 9 | `attached_unit.rs:15256-15268` | UNDECIDED | `Ok(false)` is ignored at every caller; later argument lowering was not read. |
| 10 | `attached_unit.rs:11984-11992` | UNDECIDED | Normal and scalar returns emit the deferred release; the invocation-completion path does not. |
| 11 | `attached_unit.rs:11666-11670` | CLOSED | Later literals are not emitted, and a missing prebuilt place is an error. |
| 12 | `attached_unit.rs:8792-8802, 8827-8833` | CLOSED | The fallback is the same expression or recipe with an invalid owner, never another owner's row. |
| 13 | `attached_unit.rs:12242-12244` | UNDECIDED | A quotient whose type was not emitted is omitted from the catalog. |
| 14 | `attached_unit.rs:4491-4531` | UNDECIDED | A missing link omits the establishment; consumers of the shorter list were not read. |
| 15 | `emission.rs:1355-1357` | UNDECIDED | No transfer rows return an empty claim list; two of three callers were not read. |
| 16–93 | see digest | not reached | Tracing continues. |

## 1. Scalar ensures that name a structural result

Verdict: UNDECIDED.

Read `lower_scalar_ensures` in full (`attached_unit.rs:503-552`). A row whose `proposition` returns `UnavailableContractResult` is omitted (`545-548`). Any other error is returned (`548`). The implication arm turns one unavailable side into that same error (`527-541`), so the whole implication is omitted.

Search `lower_scalar_ensures(`: 2 hits, the definition and the only call at `321`, inside `lower_declarations` (`236-448`, read through the call and the declaration push at `425`). When `plan.result.structural_identity()` is `Some`, the ensures view is `ContractStructuralResult::Unavailable { position }` with `position` equal to `signature.parameters.len()` (`304-317`). Otherwise the result view is `Absent` (`318-319`). `structural_identity` is `Some` only for `CheckedBoundaryMachineResultPlan::Structural` (`flow_terminal.rs:662-667`, read).

`proposition` (`scalar_lowering.rs:2187-2203`, read) returns `UnavailableContractResult` only from `namespace::validate` (`1426-1438`, read) after `inspect` sets the flag. The flag is set only when a structural observation's position equals the unavailable result position (`1461-1465`) or the predicate names the result while the view is unavailable (`1473-1478`). A name that matches neither a parameter nor that position is `Unsupported` (`1467-1469`), and that error is not swallowed.

`target_signature` (`shared.rs:449-528`, read) returns the state, operator, or trait-signature parameter slice. For a machine, `authored_entry_parameters` (`values_scalar.rs:4014-4065`, read) returns that same state-parameter slice. Stage 05 encodes a scalar field of the result, when ensures are allowed, as `StructuralParameterField` at `parameter_position = parameters.len()` (`values_scalar.rs:6934-6978`, read). That is the unavailable position. `lower_clause` stores a successful projection as `ClosedScalarContractValue::Predicate` (`published_facts.rs:3153-3179`, read). `derive_scalar_boundary_guarantees` copies those predicates into `scalar_ensures` (`proof_contracts_core.rs:10839-10867`, read). Machine plans use that function (`execution_terminal_unit_control.rs:436-438`, read). Trait plans copy the same predicates (`807-819`, read). Operator plans store an empty `scalar_ensures` (`973`, read).

`CheckedBoundaryScalarGuarantee::Implication` is built only by `meaning_consequence` (`10870-10998`, read). Those expressions are float parameter comparisons, not structural-result observations, so they do not set the unavailable flag. The digest's implication drop is real code and is not reached by that producer.

Search `.scalar_ensures` in `omega-rust`: the pipeline writes the field and does not read it again. `validate_provider_scalar_contract` (`foundation_validation.rs:2095-2104`, read) rejects a provider whose ensures do not cover each boundary `scalar_ensures` row, and an empty list passes. It is called from `validate_provider_candidate` (`1868-1927`, read), which `validate_structural_foundation` runs for every candidate (`10256`, `10398-10399`, read). Callers of `validate_structural_foundation` were not read. `host_imports.rs:113-121` and `compiler_builtins.rs:528-530` (both read) reject a foreign or builtin boundary that still has scalar ensures; an emptied list does not trip those rejects. `windows_handle_contracts.rs:361` (read) requires empty ensures for one Windows signature that is a unit result, so it is not this case.

Not read: the stage 05 check that a body establishes an ensures predicate about a field of a structural result. What would decide it: that check. If a body can return the field false and still produce checked trees, this branch accepts that program, because the lowered declaration and the provider-contract validator no longer contain the clause. The digest's description of the drop matches.

## 2. Authored requires, all or nothing

Verdict: UNDECIDED.

Read `entry_requirements::lower` in full (`attached_unit.rs:6230-6327`). `authored_requires()` is the prefix of `closed_scalar_values.requires` of length `authored_requires_len` (`contract_and_obligation_facts.rs:3534-3536`, read). If any slot is `None`, the `if` at `6290` is skipped and `requires` stays empty, then only `runtime_requirements` are appended (`6321`). `source_checked_requires` is a clone of that same vector before the append (`6319-6320`).

`None` is what `lower_clause` returns for a requires clause that is neither a builtin equality of two identical boolean or integer constants nor a predicate `lower_callable_scalar_contract_predicate` can project (`published_facts.rs:3099-3150` and `3153-3234`, read). The `SourceCheckedGuarantee` fallback in that function is only for ensures (`3181-3182`).

Search `entry_requirements::lower(`: 2 calls. Read `attached_unit.rs:9288-9303`: the result is stored on the signature, and `validate_contract` sees only the emitted list. Read `unit_composed_control.rs:6784` through the call arguments only; the enclosing function was not read from its start. `prepare_requirement_obligations` (`attached_unit.rs:13233-13248`, read) allocates an obligation only for each emitted requirement.

Not read: stage 05's check of a caller against a requires clause that `lower_clause` cannot project. What would decide it: whether such a clause still rejects a caller that does not satisfy it before checked trees exist. If it does not, this branch emits a signature with no authored precondition and the call obligation loop has nothing to record. The digest is right that `None` was the unknown; it means the clause did not project.

## 3. No catalogue entry, no candidates

Verdict: UNDECIDED.

Read `boundary_candidates` from `7058` through `7104`, and `checked_unit_provider_candidates` in full (`6959-7056`). Search `boundary_candidates(`: the definition and the one call at `7020`. Search `checked_unit_provider_candidates(`: the definition and the one call at `956`.

A boundary symbol that is not one trait signature, not a top-level requirement machine, and not a boundary operator returns `Ok(Vec::new())` (`7083-7101`). The caller appends that empty list and still returns `Ok` (`7020-7055`). The closure loop (`941-969`, read) uses candidates only to add provider machines that are not yet in the closure. An empty list adds none.

The symbol list is boundary calls, retained requirements, `boundary_targets`, and demanded trait signatures (`6966-7007`, read). Not read: `boundary_targets`, `call.is_boundary`, and the later lowering of a boundary call whose symbol took the empty return while `provider_instance_publication.candidates` still names it. What would decide it: whether that combination occurs for a program that should have failed provider selection. The digest matches the empty return.

## 4. Invalid receiving handle

Verdict: UNDECIDED.

Read `intern_prepared_receiving_application` in full (`6706-6723`) and `intern_receiving_description` (`6726-6744`). An invalid handle returns `Ok` with three default identity indexes and no error (`6711-6716`).

Search `intern_prepared_receiving_application(`: the definition, the wrapper `intern_receiving_application` (`6694-6704`, read), and `instance_requirements` (`6834-6912`, read through the push). The wrapper forwards `candidate.receiving`. `instance_requirements` still pushes a `RequiredProviderInstance` when the handle is invalid, with those default indexes (`6878-6906`).

Not read: when a receiving handle is invalid, and `TerminalIdentityIndex::default` / `is_absent`. What would decide it: whether an invalid handle is only the "no application" sentinel, or a real obligation whose application was lost. In the second case the requirement is emitted with a blank application. The digest matches the default return.

## 5. Unreferenced recipes, evaluated constants, landed layouts

Verdict: UNDECIDED.

Read `SourceScalarApplications::prepare` from `8355` through `8507`. Three skips:

- A constant recipe whose position is not in the collected reference set is skipped (`8365-8376`). `RetainedConstantPosition::None` is never retained (`8373`).
- A const declaration that already has `evaluated_value`, or whose evaluation entry is invalid, is skipped (`8430-8434`). A valid entry that is not a machine id is an error (`8436-8444`).
- A layout observation whose expression is not in the reference set, or that already landed an integer constant, is skipped (`8467-8472`).

Search of the function name was not finished; the only production of this `prepare` was not traced past the definition. Not read: `collect_program_references` and the consumers of a missing application row. What would decide it: whether a live recipe, const, or layout observation can be absent from that reference set and still be required later, with the missing row treated as success. The digest's "by reference set" matches the three `continue`s. It does not mention that `RetainedConstantPosition::None` is always skipped, which was read at `8373`.

## 6. Proof-scope description failure and invalid symbols

Verdict: UNDECIDED.

Read `proof_scope::publish` from `8058` through `8132`, including the `identity` closure (`8069-8091`), and `describe` in full (`8288-8316`). Search `proof_scope::publish(`: one call, `SourceScalarApplications::publish` at `8894` (read `8880-8901`).

An invalid symbol returns `ProofDeclarationId::default()` and is not inserted (`8071-8072`). A valid symbol gets an id starting at 1 (`8078-8088`). `describe`'s `Err` becomes `ProofScopeDeclaration::Unavailable` (`8086-8087`). `describe` returns that error from `boundary_identity` or `semantic_identity`; those two functions were not read.

`foundation_validation.rs` `validate` for a proof scope (`2893-2921`, read) rejects `Missing` and an empty Unavailable reason, and accepts `Unavailable` when the reason is non-empty. It rejects a referenced id with no declaration row (`2896-2900`).

Not read: whether a checked symbol can make `describe` fail, and whether an invalid symbol's default id is stored in a call edge. What would decide it: those two producers. If `describe` can fail for an accepted symbol, the scope keeps the symbol as Unavailable and the codec check above does not reject it. The digest matches both branches.

## 7. Open target families skipped

Verdict: UNDECIDED.

Read `runtime_families` in full (`9321-9349`) and `target_families::lower` from `9376` through `9411`. A family whose head has specialization parameters is not pushed (`9332`, `9345-9347`). A cohort that mixes open and closed members is an error (`9339-9342`). `has_specialization_parameters` is a non-empty type-parameter or conformance-bound list (`calls_and_control_flow.rs:1801-1803`, read).

Search `runtime_families(`: the definition, the call inside `lower` (`9382`), the root walk at `869` (read `863-880`), and the count check at `machine_lowering.rs:1962` (read `1962-1964`). That check requires the emitted family list to have the same length as `runtime_families`, so it does not demand the open families that were filtered out.

Not read: whether an open family can be the selected runtime entry. What would decide it: the entry-binding consumer of `target_machine_families`. The digest matches the skip.

## 8. Returned byte and element views

Verdict: UNDECIDED.

Read `lower_reference_result_sources` from `13658` through `13776`. A result path whose field carrier is a byte-sequence view, or whose leaf is an element view or a byte-sequence view, is skipped (`13689-13701`). A leaf that is not a reference is an error (`13703-13709`). The kept rows record a parameter or invocation origin (`13711-13775`).

Search `lower_reference_result_sources(`: the definition, `attached_unit.rs:11624` (read the result-declaration builder through `11624`), and `unit_composed_control.rs:13555` (read `13547-13566`). Both store the returned vector on the structural result.

Not read: any consumer of `reference_sources`, and the stage 05 check that a returned view's origin is legal. What would decide it: a consumer that is the only record of that origin. If it never sees the skipped row, a returned view of storage that should not escape is accepted. The digest matches the skip.

## 9. Immutable-local borrow returns false

Verdict: UNDECIDED.

Read `prepare_immutable_local_borrow` from `15243` through `15317`. It returns `Ok(false)` when the root is not a symbol (`15256-15257`), the state has no local of that name (`15260-15261`), the local is not a primitive (`15263-15264`), or the local is mutable (`15266-15267`). The `bool` is otherwise unused by the callers below.

Search `prepare_immutable_local_borrow(`: the definition and three calls, all discarding the bool with `?`:

- `prepare_borrowed_argument` (`15136-15240`, read). The scalar-parameter arm prepares a home. The other arm only calls this function and returns `Ok(())`.
- `establish_reference` (`unit_operation_frame.rs:7905-7969`, read through the call at `7930`).
- the place-loan loop (`unit_arguments.rs:2076-2091`, read).

Search `prepare_borrowed_argument(`: four calls. Read `emit_call` (`unit_operation_frame.rs:1046-1070`), which calls it for every structural argument, and `unit_arguments.rs:2056-2059`, which calls it for borrowed computation roots. `unit_operation_frame.rs:1723` and `unit_structural_values.rs:4263` were not read. The rest of `emit_call` after `1070` was not read.

The digest's "one caller" is wrong. There are three direct calls, and the bool is ignored at each. What would decide it: the argument lowering after `1070`, specifically whether a structural argument that took the `Ok(false)` arm is rejected later or passed through with no home.

## 10. Deferred reference release emits nothing in the step

Verdict: UNDECIDED.

Read `MachineEmission::lower_step` from `11764` through `11992`. `ReleaseReference { defer_to_exit: true }` returns `Ok(())` without pushing an operation (`11984-11991`). Search `lower_step(`: one call, the ordinary schedule loop at `10795` (read `10789-10798`).

`lower_reference_completion_regions` (`unit.rs:2221-2292`, read) is the pass that turns `defer_to_exit: true` into a cleanup region. Its only call is `for_planned_actions` (`attached_unit.rs:6032`, read `5942-6036`). That function is reached from:

- `for_exit_regions` (`5910-5938`, read), called for an ordinary normal return at `11061` with `plan.operations` (`11065-11069`, read).
- `scalar_returns` (`5765-5847`, read), which calls `for_planned_actions` at `5816`, used by the scalar-control arm at `10910` (read `10883-10929`).

The invocation-completion arm (`10871-10881`, read) sets `completes_normally = false`, pushes the block, and does not call either function. `take_invocation_completion` (`operation_emission.rs:1683-1720`, read) builds `InvokeNoNormal` or starts an `External` outcome. Not read: the rest of that match, and whether a `defer_to_exit` release can sit in a plan that takes this arm. What would decide it: that combination. On the normal and scalar-return arms the release is emitted later; the digest's "not traced" is the invocation arm.

## 11. Literals after an external completion

Verdict: CLOSED.

Read `establish_call_literals` from `11637` through `11672`. Literals are collected, then an `External` completion breaks the loop (`11666-1170`). The external call's own literals are collected before the break. Search `establish_call_literals(`: one call, the ordinary emitter at `10595`.

The schedule loop (`10789-10798`, read) stops after a step that sets `invocation_completion`. An external call does that and returns (`unit_operation_frame.rs:604-630`, read). Later steps, including later literal calls, do not run. `literal_argument_places` (`14125-14148`, read) is what consumes the prebuilt places, via `argument_places` (`12869-12894`, read) and `emit_call_through_frame` (`12131-12135`, read). A request past the prebuilt slice returns `Unsupported` ("byte-sequence literal argument place is absent") at `14139-14143`. The digest is right that later literals are not pre-established. They are also not emitted, and a use of a missing place is an error.

## 12. Scoped lookup falls back to an unscoped row

Verdict: CLOSED.

Read `expression_in` (`8783-8802`) and `recipe_in` (`8818-8833`). After the owner match fails, both look for the same handle with an invalid scope. A valid owner does not receive another owner's scoped row. `expression` (`8770-8780`) and `recipe` (`8805-8815`) return an id only when every match is the same id.

Search `expression_in(`: the definition, `retain_expression_in` (`8593-8599`, read), and `scalar_computations.rs:7576-7580` (read), which returns `Unsupported` when the lookup is `None`. Search `recipe_in(`: the definition, `scalar_lowering.rs:2376` (read `2363-2405`), and test asserts at `9049` and `9084`. On `None`, the contract leaf requires an admitted constant value or returns `Unsupported` (`2388-2395`). The test `constant_entries_keep_shared_expression_scopes_distinct` (`9043-9058`, read) expects two owners to keep different ids and expects the unscoped `recipe` lookup to be `None`. The digest matches the fallback. It does not cross owners.

## 13. Quotient type missing from `type_ids`

Verdict: UNDECIDED.

Read `quotients::retain` from `12215` through `12244`. A closed data definition with a quotient whose stored type identity is not in `type_ids` is skipped (`12242-12244`). Search `quotients::retain(`: the definition and the call at `17200`. That call was not read. Not read: a consumer that lowers a quotient operation for a type absent from `type_ids`. What would decide it: whether such an operation can be reached. The digest matches the skip.

## 14. Establishment omitted when a link is missing

Verdict: UNDECIDED.

Read `call_result_qualification_establishments` from `4455` through `4517`. A domain with no structural-domain plan (`4491-4493`), a plan with no routes (`4495-4496`), a domain not established at this call (`4509-4510`), or a callee with no trait signature (`4514-4516`) adds no row. Search of the function name: the definition and `Prepared::emit` at `1765` (read `1730-1774`), which calls it only when `issuer.is_valid()`.

Not read: the lines after `4517`, and any consumer of `qualification_establishments`. What would decide it: whether a missing row is treated as "no obligation" by a checker that should have rejected the result. The digest's "omitted on any missing link" matches the four `continue`s. The "conservative" part was not established.

## 15. Owned operand with no transfer rows

Verdict: UNDECIDED.

Read `lower_owned_operand_claims` from `1320` through `1357`. No matching `owned_transfer_paths` row returns `Ok(Vec::new())` (`1355-1356`). An invalid expression is an error (`1334-1335`). Search `lower_owned_operand_claims(`: the definition and three calls. Read only `lower_owned_transfer_claims` (`5145-5174`), which returns that vector unchanged. Not read: `unit_structural_values.rs:8008`, `emission_cleanup.rs:2898`, and whatever consumes the empty vector. What would decide it: those consumers. The digest matches the empty return.

## Entries not reached

16 through 93.

## What I did not read

- Stage 05 discharge of ensures and requires against bodies and callers.
- Callers of `validate_structural_foundation`.
- `boundary_targets`, `is_boundary`, and provider-candidate publication for an unclassified symbol.
- Who sets a receiving handle invalid, and what the default identity means.
- `collect_program_references` and consumers of a missing scalar-application row.
- `declarations::semantic_identity`, `declarations::boundary_identity`, and `try_map` on the proof-scope identity closure.
- Entry binding of `target_machine_families`, and consumers of `reference_sources`.
- `emit_call` after line `1070`, and `prepare_borrowed_argument` at `unit_operation_frame.rs:1723` and `unit_structural_values.rs:4263`.
- The remainder of `PendingInvocationOutcome` after `operation_emission.rs:1720`.
- `quotients::retain` after `12244`, and its call at `17200`.
- `call_result_qualification_establishments` after `4517`, and consumers of `qualification_establishments`.
- `lower_owned_operand_claims` after `1357`, plus the calls at `unit_structural_values.rs:8008` and `emission_cleanup.rs:2898`.
- Entries 16–93. Background traces were not copied in.
