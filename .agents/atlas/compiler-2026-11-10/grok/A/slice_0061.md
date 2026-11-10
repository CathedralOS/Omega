# slice_0061

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0465 | omega-rust/psi/compilation/validation/src/float_and_mathematical_contracts.rs:491-498 | REACHABLE | Any float/integer/boolean literal is treated as input-only in semantic-term checks. |
| A0466 | omega-rust/psi/compilation/validation/src/float_and_mathematical_contracts.rs:557-609 | REACHABLE | `float_scalar_semantic_contract` returns on the first matching ensures equality. |
| A0467 | omega-rust/psi/compilation/validation/src/float_and_mathematical_contracts.rs:1451-1459 | REACHABLE | Binary `Equal` is recorded only when both sides are already collected projections. |
| A0468 | omega-rust/psi/compilation/validation/src/float_and_mathematical_contracts.rs:2381-2383 | REACHABLE | Cyclic `logical_body` recursion returns admitted without re-checking body contents. |
| A0469 | omega-rust/psi/compilation/validation/src/float_and_mathematical_contracts.rs:2633 | REACHABLE | `logical_expression` default arm admits unlisted nodes including bare `Name`. |
| A0470 | omega-rust/psi/compilation/validation/src/expression_types_and_fields.rs:305-312 | REACHABLE | Missing operand or target primitive type skips float-coercion proof and reports success. |
| A0471 | omega-rust/psi/compilation/validation/src/expression_types_and_fields.rs:36-38 | REACHABLE | Recast casts exit `validate_cast_types` before `validate_exact_float_coercion`. |
| A0472 | omega-rust/psi/compilation/validation/src/expression_types_and_fields.rs:484-492 | REACHABLE | Mixed-format binary and destination checks silently skip when operand primitives are unknown. |

## A0465

**Verdict:** REACHABLE

**Code (read):** In `semantic_term_uses_only_inputs` (`float_and_mathematical_contracts.rs:475-531`), after budget decrement at 481-484, arms at 491-493 return `true` for `ExpressionNode::Float`, `Integer`, and `Boolean` without requiring the literal to be an operator parameter (contrast `Name` at 494-502, which requires parameter symbol match or format const).

**Callers (search `\bsemantic_term_uses_only_inputs\b` in `omega-rust`, count mode):** 4 hits, all in `float_and_mathematical_contracts.rs` — definition at 475; recursive call at 522; production uses at 422 (`float_result_semantic_contract` path, read 380-429) and 599 inside `float_scalar_semantic_contract`.

**Upstream (read):** `float_scalar_semantic_contract` (543-610) selects a ensures-equality predicate only when `semantic_term_uses_only_inputs` passes (598-606). `machine_calls_core.rs:5384` (read) treats `float_scalar_semantic_contract(program, operator).is_some()` as evidence a float operator is sealed/semantic (`is_sealed_toolchain_float_operator` context, 5380-5385).

**Input that reaches the branch:** An operator whose `ensures` facts include an equality tying the reserved result to a semantic call whose arguments embed a numeric or boolean literal (not a parameter name) can satisfy `semantic_term_uses_only_inputs` via the literal arm while still using a non-input constant in the contract spine.

**Smallest program (plain words):** A sealed float operator contract whose ensures equality mentions a literal coefficient inside the admitted semantic predicate, where that literal is not one of the operator’s parameters, should not count as input-only but is accepted.

## A0466

**Verdict:** REACHABLE

**Code (read):** `float_scalar_semantic_contract` (557-610) iterates `ensures_expression_facts` in collection order. For each fact it may `return Some(predicate)` at 606 on the first equality that passes reserved-result, call shape, result kind, and `semantic_term_uses_only_inputs` checks. Later ensures equalities are never consulted once 606 runs.

**Callers (search `\bfloat_scalar_semantic_contract\b` in `omega-rust`, count mode):** 4 hits — `float_and_mathematical_contracts.rs:543, 539, 613, 615`; production admission at `machine_calls_core.rs:5384` (read).

**Input that reaches the branch:** An operator with multiple ensures equalities where an earlier, weaker predicate matches first and a later equality would impose a stricter semantic requirement.

**Smallest program (plain words):** Two ensures equalities on the same operator’s result; the first match uses a looser semantic call and is chosen permanently, so a program that should be rejected for failing the second equality is accepted as having a scalar semantic contract.

## A0467

**Verdict:** REACHABLE

**Code (read):** In `walk_expression` (`1435-1539`), for `ExpressionNode::Binary` at 1448-1474, `float_meaning_operands` (1451-1459) requires `projections.iter().find` for both `binary.left` and `binary.right`. Only then, if operator is `Equal`, is `ValidatedFloatMeaningEqualityProposition` pushed (1460-1472). If either side is not yet in `projections`, the closure returns `None`, the `if` at 1460 is skipped, and execution returns `Ok(())` at 1474 with no diagnostic. Lines 1523-1525 are the non-`Binary`/`Call` arm closing without recording equalities (read).

**Callers (search `\bcollect_float_meaning_projection_invocations\b` in `omega-rust`, count mode):** 2 hits — definition `float_and_mathematical_contracts.rs:1542`; prologue `program_validation.rs:4613` (read) during `validate` fact collection.

**Input that reaches the branch:** A proof fact expression containing `projection == non_projection` (or projection on one side only) under a program-level proof root walked by `collect_float_meaning_projection_invocations`.

**Smallest program (plain words):** A proof equality comparing a validated float-meaning projection to a bare name or other non-projection expression is ignored rather than rejected, so an ill-formed float-meaning equality can remain in the accepted validation bundle.

## A0468

**Verdict:** REACHABLE

**Code (read):** `logical_body` (2375-2388) returns `true` immediately when `stack.contains(&machine.symbol)` at 2381-2382, before `logical_body_contents` runs. The stack is pushed at 2384 and popped at 2386 on the non-cycle path.

**Callers (search `\blogical_body\b` in `omega-rust/psi/compilation/validation/src/float_and_mathematical_contracts.rs`, count mode):** 5 hits — definition 2375; uses at 2289 (`is_indexed_logical_machine` filter), 2446, 2520, 2609 inside `logical_body_contents` / `logical_expression` (read 2268-2308, 2390-2417, 2595-2610).

**Upstream (read):** `is_indexed_logical_machine` (2268-2308) fixes a candidate theorem machine set and retains symbols only when `logical_body` returns true; used in validation and checked-pipeline gates (e.g. `proof_contracts_core.rs:10147`, read grep listing only).

**Input that reaches the branch:** Mutual recursion among candidate logical machines where the call graph revisits a machine symbol already on `stack`.

**Smallest program (plain words):** Two theorem-indexed machines that call each other’s symbols form a cycle; the second visit admits the body as logical without re-validating statements inside the cycle.

## A0469

**Verdict:** REACHABLE

**Code (read):** `logical_expression` (2574-2643) sets `admitted` via `match node` (2586-2633). `ExpressionNode::Name` has no dedicated arm. The default `_ => true` at 2633 admits the expression before child traversal at 2638-2642. Explicitly rejected kinds include `Atomic`, `Float`, `UndeterminedNaN`, `String`, `ZeroValue` (2587-2591); `Indexed` is false (2627).

**Callers (search `\blogical_expression\b` in `omega-rust/psi/compilation/validation`, count mode):** 7 hits in `float_and_mathematical_contracts.rs` only (2574 definition; statement/transition walks at 2454, 2468, 2476, 2488, 2527, 2641).

**Upstream (read):** Admitted expressions are required throughout `logical_body_contents` state/transition scans feeding `logical_body` and thus `is_indexed_logical_machine`.

**Input that reaches the branch:** A checked logical machine body statement or transition carrying a bare name reference that is neither ruled out by another arm nor validated as a logical parameter/local in this function.

**Smallest program (plain words):** A logical theorem machine whose body references an unbound name expression is admitted because the default arm treats it as allowed logical syntax.

## A0470

**Verdict:** REACHABLE

**Code (read):** `validate_exact_float_coercion` (295-399) at 305-308 returns `true` when `expression_result_type_reference` yields no primitive for the operand; at 310-312 returns `true` when the cast target is not a primitive. After `float_coercion_preserves_denotation` (313-353), lines 354-356 return `true` when `proven` is `None`, skipping diagnostics at 373-397.

**Callers (search `\bvalidate_exact_float_coercion\b` in `omega-rust`, count mode):** 4 hits — `expression_types_and_fields.rs:49` from `validate_cast_types` (read 27-58); definition 295; re-export 6578; `calls.rs:2644` on exact-coercion calls (read 2639-2653).

**Upstream (read):** `validate_cast_types` invoked from statement scan on non-recast casts (`calls.rs:2754-2762`).

**Input that reaches the branch:** A non-recast cast to a floating primitive whose operand has no inferable result type at validation time, or whose denotation proof cannot be formed.

**Smallest program (plain words):** An explicit qualification cast to `f64` whose operand’s type cannot be resolved in the state value environment passes float-coercion validation without error.

## A0471

**Verdict:** REACHABLE

**Code (read):** `validate_cast_types` (`expression_types_and_fields.rs:27-65`) returns immediately at 36-38 when `cast.form.is_recast()`, so `validate_exact_float_coercion` at 49-58 never runs for recasts.

**Callers (search `\bvalidate_cast_types\b` in `omega-rust`, count mode):** 3 hits — definition 27; re-export 6576; `calls.rs:2754` (read).

**Separate route (read):** `recasts.rs:61-79` handles recasts via `judge_scalar_recast` without calling `validate_exact_float_coercion` (grep `float_coercion` in `recasts.rs`: no matches).

**Input that reaches the branch:** Any recast cast form encountered during statement validation scan on casts.

**Smallest program (plain words):** A byte re-view recast between floating-related types that would fail exact float-denotation checking is never subjected to that check in the cast-validation path.

## A0472

**Verdict:** REACHABLE

**Code (read):** `validate_binary_float_formats` (459-512) returns without diagnostic when `operands` are not both `Some` at 484-486, when either side lacks a primitive at 487-492, or when the pair is not mixed F32/F64 at 493-498. `report_mismatch` (2096-2127) returns false at 2127 when `source_primitive` (2130+, read) yields `None`, so unknown-format values stored into a float slot are not diagnosed.

**Callers:** `validate_binary_float_formats` — search count 3 in `expression_types_and_fields.rs` (definition 459; `validate_float_expression_formats` 527; `validate_binary_operand_types` 1527, read 1512-1533). `report_mismatch` — grep `report_mismatch(` in validation: callee at `expression_types_and_fields.rs:2599` (read 2575-2609) on constant/slot delivery checking.

**Input that reaches the branch:** A builtin binary on expressions whose inferred operand primitives are missing while one operand is known float; or a slot/store whose value has no resolved float primitive but the target slot expects F32/F64.

**Smallest program (plain words):** A mixed float-format builtin operation where one operand’s type is still undetermined at validation, or a float destination receiving an expression with unknown float format, produces no mixed-format error.

## What I did not read

- Full `float_result_semantic_contract` and sealed-operator selection beyond the `semantic_term_uses_only_inputs` call at `float_and_mathematical_contracts.rs:422`.
- Entire `logical_body_contents` and `has_exact_theorem` predicates beyond the portions needed for caller tracing.
- Earlier typed-tree formation stages that might reject ill-typed operands before validation scan.
- Full `recasts.rs` and `judge_scalar_recast` rules for non-float recast obligations.
- Remaining `source_primitive` arms and `cross_class_conflict` after `report_mismatch` at `expression_types_and_fields.rs:2612+`.
- Digest spellings at commit `457adf4a11f` line-by-line diff against this checkout (symbols re-located by name; lines match on read).

SLICE COMPLETE
