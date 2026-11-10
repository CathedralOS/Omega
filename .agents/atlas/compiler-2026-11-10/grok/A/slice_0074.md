# slice_0074

Area digest preamble (read): Area 14 is the back half of `psi/compilation/validation` plus `terminal-installation`; `vcc` is `omega-rust/psi/compilation/validation/src/value_custody_core.rs`. Citations pinned at commit `457adf4a11f`; cited `vcc` line ranges matched in this checkout (read).

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0569 | vcc:2025-2033 | REACHABLE | Missing generic definition or argument-count mismatch makes `type_graph_requires_nominal_drop` return false, so nested drop needs can be ignored. |
| A0570 | vcc:2142-2149 | CLOSED | Unresolved selection targets are rejected when checked trees finalize authored selections (shared.rs:1416-1422). |
| A0571 | vcc:2399-2403 | REACHABLE | Owned-delivery validation exits with no diagnostic when the source type has no resolved cleanup application. |
| A0572 | vcc:2838-2839,2848-2850 | REACHABLE | `receiver_regions_match` returns true when lifetime bindings are empty or region lists cannot be read. |
| A0573 | vcc:2785-2787 | REACHABLE | A cleanup selection whose static application has no matching machine skips receiver-shape checking entirely. |
| A0574 | vcc:2971-2973,2997 | REACHABLE | Parameter/argument count mismatch ends the function silently; a `Value` argument with no carrier skips formation checks. |
| A0575 | vcc:3418-3445 | REACHABLE | Array-literal elements are not walked when the destination carrier is not `FixedArray`. |
| A0576 | vcc:4052,4068 | REACHABLE | Unresolved `Named` data is treated as not needing establishment while unresolved `Generic` is treated as needing it. |

## A0569

**Cited site (read):** `omega-rust/psi/compilation/validation/src/value_custody_core.rs:2021-2032` — in `type_graph_requires_nominal_drop`, the `TypeReferenceNode::Generic` arm returns `false` when `data_definition_by_symbol` is `None` (2024-2025) or when `arguments.len() != parameters.len()` (2031-2032), instead of treating the type as needing nominal drop.

**Enclosing function (read):** `type_graph_requires_nominal_drop` (vcc:1959-2047) walks constrained/named heads, then matches on the resolved reference; `false` means “does not require nominal drop” for downstream consumers.

**Callers (search):** pattern `\btype_graph_requires_nominal_drop\b` in `omega-rust` — 1 public definition (vcc:1959), 4 internal recursive calls in vcc, 1 re-export (`validation/src/lib.rs:1751`), 5 production uses in stage-05 execution (`execution_terminal_unit_calls.rs:57,79`, `execution.rs:772,842`, `execution_terminal_unit.rs:8157`, `execution_terminal_unit_types.rs:784`, `execution_terminal_unit_state_graph.rs:6805`); 14 total identifier occurrences (read count).

**Upward trace (read):** Prologue `validate_cleanup_machine_declarations` (vcc:2092-2107) scans type references with `selected_application` but does not re-derive graph drop need from generics. Stage-05 `execution_terminal_unit_calls.rs:57-63` treats a local as ineligible for a trivial affine structural plan when `type_graph_requires_nominal_drop` is true; a false negative lets that plan proceed. Separately, generic arity on many surface type positions is diagnosed in `type_references_and_wire.rs:5251-5256` (read), but `validate_type_reference_handle` is not invoked for every type reference in the program (grep: 15 call sites across validation, read count); embedded member types can reach drop graph analysis without hitting that arity check.

**Input that reaches the branch:** A stored type reference node `Generic { base_symbol, arguments, .. }` still present in the typed program where `data_definition_by_symbol(program, base_symbol)` is `None`, or where argument handle count differs from `data_type_parameters` length, while nested members would need nominal drop if the instance were expanded.

**Smallest program (plain words):** A data type holds a generic instance whose argument list length does not match its template parameter list (or whose base symbol has no data definition row in this checking view), and an inner field would require an owner cleanup machine; validation reports no nominal-drop requirement and stage-05 may treat a local of that type as not needing the nominal-drop path.

## A0570

**Cited site (read):** `vcc:2142-2144` — in `collect_reserved_cleanup_selection_diagnostics`, `selection_target.as_resolved()` `None` causes `continue` with no diagnostic.

**Enclosing function (read):** `collect_reserved_cleanup_selection_diagnostics` (vcc:2124-2160) loops `authored_declaration_selections`, skips domain-cleanup kinds, and errors when a resolved target names a reserved cleanup machine.

**Callers (search):** `\bcollect_reserved_cleanup_selection_diagnostics\b` — 1 definition (vcc:2124), 2 calls: `validate_reserved_cleanup_selections` (vcc:2201) and `program_validation.rs:371` prologue.

**Closing check (read):** `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/shared.rs:1416-1422` rejects any authored declaration selection whose target is still `LateBound` with “remained unresolved after successful checking”. `checking.rs:1231-1232` runs `validate_reserved_cleanup_selections` on the finalized typed program before checked trees are accepted. That path decides program acceptance after validation prologue.

**Why every path is closed:** A source attempt to select a reserved cleanup with an unresolved target is either excluded from `checks_authored_selection` (typed-trees `lib.rs:1561-1576`, read) or remains `LateBound` and fails finalization at shared.rs:1416-1422 before the program is admitted to checked trees; the vcc `continue` is harmless for shipped acceptance.

## A0571

**Cited site (read):** `vcc:2399-2403` — in `validate_cleanup_delivery`, `selected_application(program, source_type)` yielding not `Ok(Some(_))` executes `return` with no push to `diagnostics`.

**Enclosing function (read):** `validate_cleanup_delivery` (vcc:2380-2416) peels casts, compares normalized identities, and errors when target cleanup disagrees with source (vcc:2412-2414).

**Callers (search):** `\bvalidate_cleanup_delivery\b` — 1 definition (vcc:2380), 2 production calls: `expression_types_and_fields.rs:40-48` (explicit qualification cast), `proof_contracts_core.rs:2045-2053` (`validate_domain_delivery`).

**Upward trace (read):** Cast validation runs during per-statement validation with live `machine` and `state`; `validate_domain_delivery` guards on valid value/type then calls the same helper. Neither caller adds a diagnostic when this function returns early at 2397 or 2402.

**Input that reaches the branch:** A qualification cast or domain delivery site where the operand’s result type normalizes successfully but `selected_application` on the source type returns `Ok(None)` or `Err` (read: early return treats both like “no source cleanup”).

**Smallest program (plain words):** A cast or domain delivery erases from a type with no declared cleanup selection into a type that would retain a different cleanup machine; validation returns before the incompatible-cleanup error at vcc:2412-2414 and the program is accepted through validation.

## A0572

**Cited site (read):** `receiver_regions_match` (vcc:2831-2857) returns `true` at vcc:2838-2839 when `static_machine_application_lifetimes` is empty, and at vcc:2848-2850 when `region_arguments` cannot supply both receiver and subject lifetime lists (`(Some, Some)` pattern fails).

**Enclosing function (read):** `validate_declaration_owners` (vcc:2757-2828) checks domain cleanup package authority and, for each cleanup selection, compares receiver identity and `receiver_regions_match` to the subject (vcc:2809-2815).

**Callers (search):** `\bvalidate_declaration_owners\b` — 1 definition (vcc:2757), 1 call from `validate_cleanup_machine_declarations` (vcc:2096); prologue `program_validation.rs:426`.

**Upward trace (read):** When `shape == Some(true)` at vcc:2815, no receiver mismatch diagnostic is emitted (vcc:2816-2827). Empty lifetime bindings or uncomputable region arguments force `receiver_regions_match` true, so mismatched lifetime-parameter alignment between cleanup receiver and owned subject can pass.

**Input that reaches the branch:** A `dropped by` cleanup application whose static machine has lifetime binding rows but whose receiver type and selected subject type use region arguments that this helper cannot pair, or uses no lifetime bindings at all.

**Smallest program (plain words):** An authored cleanup selection pairs a mutable receiver reference with an owned subject whose lifetime parameters do not map through the application’s lifetime bindings; validation treats regions as matching and accepts the selection.

## A0573

**Cited site (read):** `vcc:2785-2787` — in `validate_declaration_owners`, `program.machine(application.registration_operation)` `None` causes `continue` without pushing the receiver-shape error at vcc:2816-2826.

**Enclosing function (read):** same as A0572.

**Callers (search):** same as A0572.

**Upward trace (read):** The shape test at vcc:2788-2814 is never reached for that selection. Other cleanup diagnostics (reserved machine declaration, per-type `selected_application` scan at vcc:2102-2107) do not substitute for this receiver/subject shape check on the specific selection row.

**Input that reaches the branch:** A cleanup selection record remains in `cleanup_selections()` but its static machine application’s `registration_operation` symbol has no `Machine` row in the typed program.

**Smallest program (plain words):** A `dropped by` selection points at a machine symbol the program table cannot resolve, so validation never emits “receiver does not match its selected owned subject” and the selection survives validation.

## A0574

**Cited sites (read):** `validate_argument_recipes` (vcc:2962-3027): at vcc:2971-2972, `parameters.len() != arguments.len()` returns immediately; at vcc:2986-2997, `Value`/`Const` parameters with no `has_carrier` hit `continue` after optionally diagnosing only the `Const` exact-carrier case (vcc:2987-2996).

**Enclosing function (read):** `validate_argument_recipes` validates const/value expression arguments for static machine parameters and recurses into nested conformance applications (vcc:3013-3024).

**Callers (search):** `\bvalidate_argument_recipes\b` — 1 definition (vcc:2962), 1 recursive call (vcc:3016), 3 production calls in `machine_parameters_and_bindings.rs:5871,6164,6548`; 6 total (read count).

**Upward trace (read):** Call sites supply `parameters` and `arguments` from typed call/application tables (`mpb:5871-5877`, `6163-6171`). Length mismatch skips all per-argument work with no diagnostic. For `TypeParameterKind::Value`, missing carrier skips `validate_formation` without the const-only error path.

**Input that reaches the branch:** (1) A static machine argument vector whose length disagrees with the callee’s parameter list at a call site that still reaches `validate_argument_recipes`, or (2) a runtime `Value` parameter passed a const expression that `has_carrier` rejects while `exact_const_carrier_is_eligible` is false for the expected type (so the `continue` at 2997 avoids formation validation).

**Smallest program (plain words):** A direct or conformance-nested call supplies fewer static arguments than the machine’s parameter list declares, or supplies a non-carried expression for a value parameter; validation leaves formation and carrier obligations unchecked and accepts the call through validation.

## A0575

**Cited site (read):** `validate_constant_literal_fields` (vcc:3408-3447): for `ExpressionNode::ArrayLiteral`, vcc:3420-3428 pushes element work only when `carrier` is `TypeReferenceNode::FixedArray`; otherwise the literal arm does nothing and `_ => {}` at vcc:3444 leaves pending empty.

**Enclosing function (read):** Depth-first walk of struct/array literal fields for constant initializer shape checking; returns `Ok(())` if no forbidden pending mismatch.

**Callers (search):** `\bvalidate_constant_literal_fields\b` — 1 definition (vcc:3408), 1 production call `build-time-evaluation/src/const_initializers.rs:2123-2128` (maps `Err` to build failure).

**Upward trace (read):** Build-time constant replay requires this check before evaluating declarations. When destination type is slice, named array alias, or other non-`FixedArray` carrier, nested elements inside an array literal are not validated.

**Input that reaches the branch:** A `const` materialized initializer containing an array literal while `declaration.declared_type` (unconstrained) is not a `FixedArray` type reference node.

**Smallest program (plain words):** A compile-time constant whose type is a slice or other non-fixed-array carrier is initialized with an array literal; validation walks nothing under that literal and build-time evaluation proceeds.

## A0576

**Cited site (read):** `type_requires_establishment_inner` (vcc:3984-4076): for `TypeReferenceNode::Named` (vcc:4037-4058), `definition.is_some_and(...)` yields false when no `closed_data_definition` / `data_definition_by_symbol` / `data_definition_by_name_str` (vcc:4043-4051), so the named arm does not require establishment. For `TypeReferenceNode::Generic` (vcc:4060-4074), missing definition uses `.is_none_or(...)`, which is true when definition is `None` (read: opposite default).

**Enclosing function (read):** `type_requires_establishment` (vcc:3866-3884) memoizes `type_requires_establishment_inner`.

**Callers (search):** `\btype_requires_establishment\b` — 1 public definition (vcc:3866), 9 validation production uses (`literals_and_struct_literals.rs:5996`, `domains_and_predicates.rs` ×6, `contract_entailment.rs` ×2, read listing), plus stage-05 multiplicity/flow uses (grep count 19 total, read).

**Upward trace (read):** `literals_and_struct_literals.rs:5994-6006` omits a gated field diagnostic only when `type_requires_establishment` is false for the field type; unresolved `Named` references therefore behave like fields that need no explicit initializer.

**Input that reaches the branch:** A struct literal omits an authored field whose type is a `Named` reference with valid symbol or spelling but no resolvable data definition in this checking view, while the same program would treat an unresolved `Generic` with the same missing definition as requiring establishment (vcc:4068).

**Smallest program (plain words):** A construction omits a field typed as an unresolved named data reference; validation does not require explicit initialization for that field, whereas an unresolved generic with the same missing definition would still count as needing establishment.

## What I did not read

- Full bodies of `visit_type` in `cleanup_selection` beyond the excerpt used for A0569 context (vcc:2604+, not read in full).
- Every `validate_type_reference_handle` call site argument lists (only grep counts, read).
- Stage-05 execution modules beyond the cited `type_graph_requires_nominal_drop` call lines.
- Specification chapters beyond the area_14 digest preamble.
- `git show 457adf4a11f:.../value_custody_core.rs` line-by-line diff (current lines matched citations by symbol, read).

SLICE COMPLETE

