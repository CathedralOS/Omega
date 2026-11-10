# slice_0028 (area_10 ACCEPT, A0208–A0215)

| id | file:line | verdict | one-line reason |
|----|-----------|---------|-----------------|
| A0208 | expression_bindings.rs:4434 | REACHABLE | Equal root and observed root extend `source.path` without the reference-shape gate used on the unequal branch |
| A0209 | expression_preparation.rs:661-662,743-747 | CLOSED | Empty `match` arms omit node-local scalars; closure still walks `value_dependencies` children and `call_schedule` operands |
| A0210 | expression_preparation.rs:414-415 | REACHABLE | Missing or ambiguous `call_schedule` yields an empty call list, so effect operands are skipped in closure walks |
| A0211 | expression_preparation.rs:2144-2148 | REACHABLE | Ill-formed required-membership closure clears predicate data and lowering continues into the domain catalog |
| A0212 | expression_preparation.rs:1682-1685 | CLOSED | `UnresolvedStructuralParameterField` is rejected in validation before `operation_emission.rs:269-271` |
| A0213 | expression_preparation.rs:3787 | REACHABLE | `ArithmeticDomain` constraints contribute no qualification atoms in `type_atoms` |
| A0214 | expression_preparation.rs:3796-3813 | UNDECIDED | Three `Domain` guard arms skip atom collection; downstream predicate enforcement not fully traced |
| A0215 | expression_preparation.rs:4449 | REACHABLE | Default arm does not enqueue child values; several kinds rely only on `qualification_decisions` or prior `call_schedule` steps |

## A0208

**read** Digest area_10 preamble (`digests/area_10.md:1-25`). **read** Row cites `expression_bindings.rs:4434` in `ScalarBindings::structural_observation_path` (`expression_bindings.rs:4422-4452`).

When `root == observed_root` (`expression_bindings.rs:4434-4436`), the function clones `source.path`, appends the observation suffix, and returns without the unequal-root reference check at `expression_bindings.rs:4438-4447` (which may return `unsupported("structural observation changes its established root type")`).

**Caller search** `\bstructural_observation_path\b` in `omega-rust`: 6 matches — 1 definition in `expression_bindings.rs`, 1 method call in `scalar_computations.rs:4341` (`resolved_observation_path`), 4 in `proofs.rs` (separate local function, not the `ScalarBindings` method).

**read** `scalar_computations.rs:4331-4349` calls the method when `captured_type`, `loan_source`, and `receiver` are all absent. **read** `scalar_computations.rs:4404-4410` (`observe`) requires `SharedBorrow.admits(home.access)` before using the path; **read** `scalar_computations.rs:4360-4368` (`atomic_observation`) checks load ordering and readable access.

**Verdict REACHABLE:** A program whose home established root equals `observation_root_type` but whose suffix path is only sound under the unequal-root reference guard can take the early branch at `4434` and still reach observation lowering with only custody-level shared-borrow admission at `scalar_computations.rs:4404`, not the reference compatibility test at `4438-4447`.

## A0209

**read** `expression_preparation.rs:606-751` (`structural_dependencies`).

**read** Lines 661-662: `Value::NativeCodeLayout` and `Value::QuotientCall` arms are empty (no node-local scalars).

**read** Lines 743-747: `QuotientAdaptation`, `FixedArray`, `Call`, `ByteLiteralView`, and `ZeroedStructural` arms are empty.

**read** Lines 624-627 and 409-415: each visited value also iterates `call_schedule(value).map(...).unwrap_or(&[])` for effect operands (same pattern as A0210).

**read** `values_and_views.rs:1342-1395` (`value_dependencies`): `QuotientCall` and `FixedArray` enqueue structural children; `QuotientAdaptation` enqueues `source`; `Call` has no child enqueue (`_ => {}` at 1392) but remains in the `values` vector from the root closure.

**Caller search** `structural_dependencies(` in `omega-rust`: 9 matches, all in `expression_preparation.rs` (definition at 606; calls at 164, 402, 454, 477, 504, 527, 550, 583).

**Verdict CLOSED:** The empty arms only skip scalars embedded directly in those node kinds; the `while` loop still visits handles from `value_dependencies` (e.g. `QuotientCall` arguments at `values_and_views.rs:1357-1358`, `FixedArray` elements at 1384-1385, `QuotientAdaptation` source at 1381-1382) and scalars from scheduled calls at 624-657. Leaf kinds (`NativeCodeLayout`, `ByteLiteralView`, `ZeroedStructural`) add no scalars at the node, consistent with their shapes in `values_and_views.rs:1099-1178`.

## A0210

**read** `expression_preparation.rs:371-418` (`value_calls`) and `410-415`: `call_schedule(value).map(|(_, calls)| calls).unwrap_or(&[])`.

**read** `values_and_views.rs:1398-1416`: `call_schedule` returns `None` when schedules are missing, duplicate for a value, or the node is invalid (`1410-1411`).

**read** The same `unwrap_or(&[])` appears in `structural_dependencies` at `624-627` and in `collect_domains` at `4312-4315`.

**Caller search** `value_calls` in `omega-rust`: 11 matches (definition in `expression_preparation.rs`; uses in `value_call_operations`, `reachable_structural_values` pipeline; stage-05 `execution_terminal_unit_control.rs` and related files build schedules).

**Verdict REACHABLE:** If a checked structural value has effect operations but `call_schedule` returns `None` (no row, ambiguous duplicate, or invalid node per `values_and_views.rs:1410-1411`), dependency and `value_calls` collection behave as if there are zero calls, so scalars reachable only through those operations are omitted while lowering continues.

## A0211

**read** `domain_predicates::complete_value_components` (`expression_preparation.rs:2139-2165`).

**read** Lines 2140-2148: precompute `required_memberships_are_well_formed` per domain; when false, set `required_memberships = None` and `predicate = None` on that declaration without `LoweringError`.

**read** `types_and_values.rs:203-205`: `required_memberships_are_well_formed` is true iff membership closure resolves.

**read** Caller: `complete_value_components` invoked at `expression_preparation.rs:3191` during scalar qualification catalog preparation (before sealing domain IDs).

**Verdict REACHABLE:** A domain whose required-membership graph is ill-formed is silently stripped and folded into the catalog instead of failing lowering, so compilation can succeed with a domain that no longer carries the intended membership predicate.

## A0212

**read** `lower_boolean` (`expression_preparation.rs:1573-1685`); `StructuralParameterField` path when `structural_fields` is empty (`1657-1685`) yields `LoweredBooleanReturnExpression::UnresolvedStructuralParameterField`.

**read** `operation_emission.rs:269-271`: `unreachable!(...)` for that variant in `emit_boolean_expression`.

**read** `emission.rs:9489-9491` and `9690-9692`: `validate_boolean_parameter_types` / `validate_short_circuit_expression` return `unsupported(...)` on that variant.

**read** `proofs.rs:2515-2517`: crash-predicate lowering also `unsupported(...)`.

**read** `scalar_lowering.rs:311-312`: guard branches call `validate_short_circuit_expression` then `validate_boolean_parameter_types` before using the lowered Boolean.

**Caller search** `emit_boolean_expression(`: production uses in `emission.rs` (e.g. 10305) and `unit_arguments.rs:3978`, after preparation/validation paths above.

**Verdict CLOSED:** Lowering may construct the variant at `1682-1685`, but emission and scalar-guard paths reject it at `emission.rs:9690-9692` (and related sites) before `emit_boolean_expression` runs; successful compilation does not reach `operation_emission.rs:269-271`.

## A0213

**read** `type_atoms` (`expression_preparation.rs:3743-3855`).

**read** Line 3787: `TypeConstraintNode::ArithmeticDomain(_) => {}` — no atom pushed.

**read** Default branch at 3833-3837 returns `unsupported("scalar qualification requires non-vacuous or unsupported evidence")` for other unsupported constraints.

**Caller search** `type_atoms(` in `omega-rust`: 12 matches (5 in `expression_preparation.rs` including definition; `unit_operation_frame.rs`; `validation/proof_contracts_core.rs`).

**Verdict REACHABLE:** A constrained carrier whose qualifying evidence is only `ArithmeticDomain` constraints can traverse `type_atoms` and produce an empty or incomplete atom list while lowering proceeds to build qualification sets from the returned atoms (`expression_preparation.rs:3111-3114`, `3644-3658`).

## A0214

**read** `type_atoms` constraint loop `expression_preparation.rs:3786-3837`.

**read** Three no-op guard arms for `TypeConstraintNode::Domain`:
- `3788-3795`: domain with non-empty `establishment_routes` and `scalar_type_refinement_predicates(current).is_some()`
- `3796-3800`: `validation::scalar_value_domain_predicate(...).is_some()`
- `3801-3808`: `validation::exact_declared_domain_carrier_interval(...)` with `minimum <= maximum`
- `3809-3813`: `predicate_presence.is_present()` and `scalar_type_refinement_predicates(current).is_some()`

Each arm uses `{}` — no `atoms.push`, no error.

**read** Contrasting arm `3814-3832` for declared domains without present predicate calls `declared_atoms` and may error.

**Verdict UNDECIDED:** I read the collection site but did not trace whether the skipped constraints are enforced exclusively via `scalar_type_refinement_predicates`, `validation::scalar_*` helpers, or later emission such that omitting atoms is harmless; opening those validation implementations and all `type_atoms` consumers beyond the catalog build at `3111` would decide REACHABLE vs CLOSED.

## A0215

**read** `collect_domains` (`expression_preparation.rs:4292-4453`).

**read** Lines 4324-4335: `qualification_decisions` on each node always contribute domains before the `kind` match.

**read** Lines 4312-4322: `call_schedule` operations may push child values for nested structural evaluations.

**read** Line 4449: `_ => {}` for all kinds not explicitly handled (includes e.g. `QuotientAdaptation`, `CopiedStructuralPlace`, `BorrowedSliceView`, `MutableSliceView`, plain `Place`, `Scalar`, `Call` without further pending pushes from the arm).

**Caller search** `collect_value_requirements` / `collect_operation_requirements`: `attached_unit.rs` (1875-1876, 4000, 4061, 16878), `scalar_computations.rs:5429`.

**Verdict REACHABLE:** For structural value kinds that fall through `_` without enqueuing children, domain requirements that live only on child structural values (not mirrored in `qualification_decisions` and not reached via the `call_schedule` operand loop) are not walked; lowering can still succeed with an incomplete domain requirement set for that subtree.

## What I did not read

- Stage-05 logic that populates or retains `call_schedule` rows beyond `values_and_views.rs:1398-1416`.
- Full bodies of `validation::scalar_value_domain_predicate`, `validation::exact_declared_domain_carrier_interval`, and `value_facts` refinement predicate tables (relevant to A0214).
- All upstream callers of `ScalarBindings::structural_observation_path` outside `scalar_computations.rs` (e.g. complete `proofs.rs` lowering paths).
- Corpus or specification text defining whether `ArithmeticDomain` must surface as qualification atoms (A0213).

SLICE COMPLETE
