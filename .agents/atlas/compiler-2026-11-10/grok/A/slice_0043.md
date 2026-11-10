# slice_0043

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0325 | omega-rust/psi/compilation/validation/representation-planning/src/representation_selection.rs:136-142 | UNDECIDED | `validate_type` returns `Ok` on any `primitive_type_reference` with no further carrier rule read here |
| A0326 | omega-rust/psi/compilation/validation/src/access_views_and_borrows.rs:27-189 | REACHABLE | AccessCustody conformance whose arguments name no matching plan/schema pair hits `continue` with no agreement diagnostic |
| A0327 | omega-rust/psi/compilation/validation/src/access_views_and_borrows.rs:1294-1305 | REACHABLE | Use of an atomic accessor place skips accessor rules when `declared_place_type` fails |
| A0328 | omega-rust/psi/compilation/validation/src/access_views_and_borrows.rs:1444-1463 | REACHABLE | Statement call with no accessor target match and unresolvable receiver skips `validate_binding_private_use` |
| A0329 | omega-rust/psi/compilation/validation/src/access_views_and_borrows.rs:2993-2996 | REACHABLE | Out-of-range or inverted fixed-array range on write-only array returns `true` with no diagnostic |
| A0330 | omega-rust/psi/compilation/validation/src/access_views_and_borrows.rs:3365-3418 | REACHABLE | Unresolved call target returns before flagging `&write` inputs |
| A0331 | omega-rust/psi/compilation/validation/src/arithmetic_domains.rs:5658,5717-5718,7791-7793 | UNDECIDED | Representability/division gates need `Some(primitive)`; did not trace typecheck guaranteeing that on reachable binaries |
| A0332 | omega-rust/psi/compilation/validation/src/arithmetic_domains.rs:838-864 | REACHABLE | Range/narrowing diagnostics pushed into `throwaway` are discarded and can skip `check_validated_value_narrowing` |

## A0325

- **read** `representation_selection.rs:129-142` (`CarrierClosure::validate_type`): after `scopes.head`, if `program.primitive_type_reference(type_reference).is_some()` the function returns `Ok(())` with no other inert-carrier test on that node.
- **read** Same function also returns `Ok(())` for `TypeReferenceNode::Named` when `SymbolKind::BuiltinType` (`160-164`); non-primitive named types recurse through `validate_definition` (`165-225`).
- **read** `validate_inert_carrier` (`32-51`) calls `validate_type` on the carrier type handle; **read** `form_opaque_representation_request` (`357-374`) requires the representation argument to resolve to a concrete `CheckedShape` data declaration before `validate_inert_carrier` runs.
- **grep** `\bvalidate_inert_carrier\b` over `omega-rust`: count **2** (definition `representation_selection.rs:32`, call `representation_selection.rs:370`); no other production call sites.
- **inferred** A program that should be rejected would need a carrier tree containing a primitive that the representation contract forbids as an inert leaf; that rule was not read in this slice.
- **Verdict UNDECIDED**: the accepting branch at `136-142` is **read**; what would decide reachability is a specification or earlier-stage check naming primitives invalid inside opaque carriers (digest notes `primitive_type_reference` not traced [088]).

## A0326

- **read** `access_views_and_borrows.rs:27-189` (`access_custody::validate_agreements`): loops `program.conformances()`; for core `AccessCustody` (`638-652`), six early `continue` paths omit diagnostics—unknown trait (`32-35`, but see traits pass below), not AccessCustody (`37-39`), wrong argument count (`44-45`), non-`Named` type arguments (`47-52`), missing access plan (`54-57`), missing schema/custody data (`59-69`).
- **read** Caller chain: `validate_plans` (`1168-1170`) calls `validate_agreements`; **grep** `\bvalidate_agreements\b` in `omega-rust`: count **2** (definition + single call). **grep** `access_views::validate_plans` in `omega-rust`: count **1** (`program_validation.rs:482`), scheduled after `validate_conformances` (`455-461`).
- **read** `traits.rs:6462-6498` (`validate_named_implementation`): unknown trait and wrong generic argument count emit errors and `return` before access validation.
- **read** `concrete_named_symbol` (`654-661`): only `TypeReferenceNode::Named` succeeds; generic or non-named arguments hit `continue` at `51-52` without error in `validate_agreements`.
- **Input in plain words**: an `AccessCustody` conformance with three named type arguments whose policy/schema pair does not match any entry in `program.access_view_plans` (plan lookup fails at `54-57`).
- **Verdict REACHABLE**: trait argument arity can be closed at `traits.rs:6493-6498`, but a conformance that passes trait typing yet fails plan agreement is accepted silently here (`continue` at `56-57`); nothing in the traced upward path reports that absence.

## A0327

- **read** `access_views_and_borrows.rs:1259-1305` (`validate_expression`): when `access_view_field_for_place` returns `Some`, runs binding-private and atomic-field rules (`1297-1303`); the whole block is skipped when `access_view_field_for_place` is `None`.
- **read** `access_view_field_for_place` (`1406-1421`): returns `None` if `access_view_plans` is empty (`1412-1413`) or `declared_place_type` fails (`1415-1420`).
- **read** Fallback for `ExpressionNode::Atomic` load (`1314-1333`) uses `access_atomic_field_for_place` or `place_is_atomic_storage`; it does not cover using an atomic accessor field as an ordinary non-atomic expression.
- **grep** `validate_expression` callees from `access_views` module: statement validation (`1189-1191`, `1353-1359`) and assignment paths (`1215-1220`) all funnel through this function (**read**).
- **Input in plain words**: a machine state expression that refers to an atomic accessor field as a normal value, on a place whose declared type cannot be computed at `1415-1420`.
- **Verdict REACHABLE**: no enclosing check in the traced callers substitutes for the skipped `1294-1305` branch when `declared_place_type` is `None`.

## A0328

- **read** `access_view_field_for_statement_call` (`1444-1462`): first tries `access_view_field_for_call_target` (`1424-1441`) matching `call.target_symbol` to an accessor state symbol; else resolves receiver member names and `declared_member_path_type` (`1451-1460`), returning `None` if the receiver path does not resolve.
- **read** `validate_statement` (`1183-1187`): calls `validate_binding_private_use` only when `access_view_field_for_statement_call` returns `Some`.
- **read** `validate_binding_private_use` (`1484-1501`): errors only for `AccessExposure::BindingPrivate` when machine and view policy differ in source package.
- **Input in plain words**: a statement call whose target symbol is not listed in any field’s `accessor_targets`, while the receiver path’s type cannot be resolved—so binding-private accessor policy is never consulted.
- **Verdict REACHABLE**: the `None` path from `1444-1463` bypasses the only binding-private check on statement calls (`1183-1187`); no alternate route was found in the read callers.

## A0329

- **read** `validate_write_only_fixed_array_range_assignment` (`2921-2996`): after static bound normalization (`2968-2991`), if bounds are known and `start > end || end > collection_len` (`2993-2994`), returns `true` without pushing a diagnostic (digest item 1 **read**, matches current lines).
- **read** Caller `validate_statement` write-only assignment branch (`2762-2777`): a `true` return takes the empty branch and does not run later index validation for that assignment shape.
- **read** `places.rs:8280-8287` rejects inverted fixed ranges during type projection, but that path was not traced into this write-only assignment entry.
- **Input in plain words**: a write-only machine assignment that replaces a fixed-array slice using a range literal (or alias) whose static start/end is inverted or extends past the array length.
- **Verdict REACHABLE**: the accepting path is **read** at `2993-2996`; no diagnostic is emitted on the traced call chain from `2762`.

## A0330

- **read** `validate_opaque_call_inputs` (`3352-3435`): resolves callee via `machine_state_by_symbol` (`3362-3377`) or boundary `selected_requirement_signature` (`3385-3408`); if both fail, `return` at `3416-3417` before inspecting inputs.
- **read** Write-only input test (`3419-3434`) runs only after a callee signature and bindings are obtained; unresolved targets never reach it.
- **grep** `\bvalidate_opaque_call_inputs\b` in `omega-rust`: count **3** (definition + `2829` statement call + `3446` expression call).
- **Input in plain words**: a write-only-borrow machine calling through a target symbol that does not resolve to a machine state or admitted boundary requirement, with a parameter typed as `&write`.
- **Verdict REACHABLE**: early `return` at `3417` accepts the call without the opaque-input error that `3420-3434` would emit for resolved targets.

## A0331

- **read** Binary analysis `analyze` (`5599-5806`): `division_formation` runs only when `primitive.is_some()` from `result_primitive` (`5656-5668`); `require_representable_result` / `warn_unconditional_trap` run only inside `if let Some(bridge) = policy_bridge && let Some(primitive) = primitive` (`5717-5738`).
- **read** `result_primitive` (`5897-5936`): without address operands, falls back to operand `primitive` fields, then bounded interval plus `walk.target_primitive`; if all missing, returns `None`.
- **read** Place operand analysis `place::analyze` (`7783-7793`): `declared_type` missing yields `NEUTRAL` (`7791-7792`), contributing no carrier to operand `Analysis`.
- **read** `ExpressionWalk::declared_type` (`7912-7925`): uses `declared_place_type_raw` in machine context or `result_type` otherwise.
- **Input in plain words**: an integer binary in a context where neither operand’s declared/result type resolves and the walk carries no `target_primitive`, so `result_primitive` stays `None`.
- **Verdict UNDECIDED**: the skip when `primitive` is `None` is **read** at `5717-5718`; whether such an expression can appear in a program that still reaches validation arithmetic was not established (type inference / `expression_types` not read).

## A0332

- **read** `check_value_narrowing` (`806-864`): for non-match arms, allocates `throwaway` (`838`), calls `validate_value_range` with `&mut throwaway` (`839-848`); only if `throwaway.is_empty()` runs `check_validated_value_narrowing` with the real `diagnostics` (`850-863`).
- **read** `validate_value_range` (`15011-15033`) forwards to `validate_value_range_with_decision_sink` with `ArithmeticDecisionSink::Discard` (`15032`), so obligations/decisions from the walk are dropped; diagnostics still append to the passed vector (`15141-15160`, **read**).
- **grep** `\bcheck_value_narrowing\(` in `omega-rust`: count **4** (definition, recursive match arm `819`, `program_validation.rs:1766`, `calls.rs:531`).
- **Input in plain words**: a narrowing store where `validate_value_range` records one or more errors into the side vector (for example range or arithmetic diagnostics during range proof), so the main diagnostic list stays empty and narrowing is not re-checked.
- **Verdict REACHABLE**: errors written only to `throwaway` are never merged into `diagnostics` and actively suppress `check_validated_value_narrowing` when non-empty (`850-863`).

## What I did not read

- Specification or language guide rules for which primitives may appear as inert opaque carriers (A0325).
- Full body of `validate_trait_application_obligations` and whether every ill-shaped `AccessCustody` argument is rejected before `validate_agreements` (A0326 partial).
- `expression_types` / typing pipeline guarantees that integer binaries always carry `Analysis.primitive` or `target_primitive` at validation (A0331).
- Whether write-frame, borrow, or call validation elsewhere rejects out-of-range write-only slice stores or unresolved-target `&write` calls (A0329, A0330).
- `git show 457adf4a11f:` paths (current checkout lines matched on spot-check; no relocation noted).
- Units 088–096 primary records beyond digest `area_13.md` preamble.

SLICE COMPLETE
