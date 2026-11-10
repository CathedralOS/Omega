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
| 6–93 | see digest | not reached | Tracing continues. |

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

## Entries not reached

6 through 93. Entry 6 was opened (`proof_scope::publish` at `8058-8090` and `describe` at `8288-8316`) and is not given a verdict in this commit.

## What I did not read

- Stage 05 discharge of ensures and requires against bodies and callers.
- Callers of `validate_structural_foundation`.
- `boundary_targets`, `is_boundary`, and provider-candidate publication for an unclassified symbol.
- Who sets a receiving handle invalid, and what the default identity means.
- `collect_program_references` and consumers of a missing scalar-application row.
- Entries 7–93, except the opened but unfinished entry 6 noted above.
- The two background traces of entries 15–68 were not used. A background trace of later entries was not copied in; those sites are still "not reached" until read here.
