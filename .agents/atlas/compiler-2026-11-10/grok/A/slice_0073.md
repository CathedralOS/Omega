# slice_0073

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0561 | omega-rust/psi/compilation/validation/src/type_references_and_wire.rs:2186-2216 | REACHABLE | Ineligible structured carrier accepts a parsed atom when `type_name` matches carrier label, without encoding or identity validation. |
| A0562 | omega-rust/psi/compilation/validation/src/type_references_and_wire.rs:2114-2115 | REACHABLE | Const-parameter argument that is a pending range-endpoint expression returns with no diagnostic. |
| A0563 | omega-rust/psi/compilation/validation/src/type_references_and_wire.rs:3688-3697 | REACHABLE | Closed indexed-domain const expression treats `normalize_closed_const_expression_in_carrier` returning `Ok(None)` as success. |
| A0564 | omega-rust/psi/compilation/validation/src/type_references_and_wire.rs:3462-3484 | REACHABLE | Open-index normalization silently no-ops for non-listed operators, missing owner, or non-selectable index types. |
| A0565 | omega-rust/psi/compilation/validation/src/type_references_and_wire.rs:6771-6801 | REACHABLE | Adjacent wire-era check only tombstones dropped field numbers, not same-number type changes. |
| A0566 | omega-rust/psi/compilation/validation/src/value_custody_core.rs:521-584 | REACHABLE | Activation-claim walk does not descend through reference, slice, or dynamic trait shells. |
| A0567 | omega-rust/psi/compilation/validation/src/value_custody_core.rs:1521 | REACHABLE | `FixedArray` arms outside compile-time literal length add no linear claim frontier. |
| A0568 | omega-rust/psi/compilation/validation/src/value_custody_core.rs:1460-1461 | REACHABLE | Unresolved named type returns from claim walk without contributing frontier claims. |

## A0561

**Cited:** `trw:2153-2154` in the row; digest also names `2186-2217`. At `2153-2154` (read) the function pushes an error for a non-`Named` const actual and returns — that path rejects. The digest wording matches `2186-2216` instead.

**Function (read):** `validate_const_data_argument` (`type_references_and_wire.rs:2036-2279`).

**Branch (read):** After `CanonicalConstValue::from_atom` succeeds (`2186`), if `exact_structured_const_carrier_is_eligible` is false the code skips `validate_exact_const_identity_in_carrier` (`2187-2198`). If `validate_typed_const_index_type` succeeds (`2200-2208`), only `value.type_name` is compared to the carrier label (`2209-2215`); on match it returns (`2216`) with no encoding or range check.

**Callers:** Search `\bvalidate_const_data_argument\b` in `omega-rust`: **7** matches, all in `type_references_and_wire.rs` — definition `2036`, calls `173`, `1733`, `1778`, `5289`, plus `pub use` lines `4153`, `4160` (read).

**Upward (read):** Generic const/value arguments are validated through `validate_const_data_argument` during type-reference validation (`173`). `validate_closed_const_argument` (`1703`) forwards here (`1733`) and fails the program when diagnostics are errors (`1755`). A const generic actual that is a spelled atom with matching display type name but malformed canonical encoding, for a parameter whose carrier is index-legal but not structured-eligible (e.g. a small integer primitive per `validate_typed_const_index_type` at `2872-2878`), can compile if no other validator catches the encoding.

## A0562

**Function (read):** `validate_const_data_argument` (`2036-2279`).

**Branch (read):** For `TypeReferenceNode::ConstExpression(root)` (`2111-2113`), if `program.pending_const_range_endpoints.contains(root)` the function returns immediately (`2114-2115`) without pushing a diagnostic — skipping the “must be known during compilation” error at `2133-2137` and all later const checks in this function.

**Callers:** Same search as A0561: **7** matches in `type_references_and_wire.rs` (read).

**Related (read):** Generic argument validation also skips closed-const checking when the expression is a pending range endpoint (`141`).

**Upward (read):** A generic const or value parameter whose actual is still registered as a pending integer-range endpoint is accepted through this function during type checking; program acceptance then depends on whether later stages require a closed value (not traced here).

## A0563

**Function (read):** `validate_indexed_domain_argument_pack` (`3636-3727`).

**Branch (read):** For a closed application with a `ConstExpression` argument (`3662-3704`), after `validate_open_index_expression`, the code calls `normalize_closed_const_expression_in_carrier` (`3683-3688`). The `and_then` closure runs `validate_exact_const_identity_in_carrier` only when `value` is `Some` (`3690-3696`); `Ok(None)` from normalization yields `Ok(())` (`3697`) and no diagnostic is pushed.

**Callers:** Search `\bvalidate_indexed_domain_argument_pack\b` in `omega-rust`: **3** matches — definition `3636`, calls `3197`, `3227` in `validate_indexed_domain_arguments` / `validate_indexed_qualification_arguments` (read).

**Normalization (read):** `normalize_closed_const_expression_in_carrier` (`1097-1129`) returns `Ok(None)` when the carrier is not structured-eligible (`1103-1104`) or when the expression does not produce a value (`1112-1113`).

**Upward (read):** `validate_indexed_domain_arguments` is invoked from constrained type validation (`769`). Diagnostics accumulate in program validation; an indexed domain with a closed const-expression argument that normalizes to `None` can pass this pack without the extra identity check.

## A0564

**Function (read):** `normalize_open_index_expression_operations` (`3448-3516`).

**Branch (read):** Returns without diagnostic when: expression is not binary (`3459-3460`), operator is not `+|-|*|/` (`3467`), `owner` is `None` (`3469-3470`), or index type is not `Named`/`Constrained` (`3483-3484`).

**Callers:** Search `\bnormalize_open_index_expression_operations\b` in `omega-rust`: **4** matches — definition `3448`, recursive calls `3500`, `3508`, call from `normalize_open_index_expressions` `3330` (read).

**Upward (read):** `normalize_open_index_expressions` (`3266-3372`) collects sites from domain constraints and indexed casts, runs normalization, and applies `OpenIndexNormalization` only when the local `diagnostics` vector is empty (`3348-3369`). An open index expression using an unlisted operator, lacking a resolvable machine owner, or using a non-selectable index type leaves `operations` empty but still allows `Ok(())` when no other diagnostic was recorded. Invoked from pipeline preparation `04_typed-trees-to-typed-trees/src/preparation.rs:2068` (read grep).

## A0565

**Function (read):** `validate_adjacent_eras` (`6771-6801`).

**Branch (read):** For each predecessor field, only when `successor_field` is missing (`6785`) does it require a tombstone (`6791-6798`). When `successor_field` is `Some`, there is no comparison of field names or type references.

**Callers:** Search `\bvalidate_adjacent_eras\b` in `omega-rust`: **2** matches — definition `6771`, call `6583` inside `validate_schema` (read).

**Upward (read):** `validate_schema` (`6541-6584`) chains version scopes and calls `validate_adjacent_eras` for each adjacent pair (`6582-6583`). `validate_wire_schemas` feeds program validation prologue (`program_validation.rs:576`). A wire protocol whose successor era keeps the same field number but changes the field’s type is not flagged here.

## A0566

**Function (read):** `carries_activation_claim` (`521-586`); public entry `type_carries_activation_claim` (`513-518`).

**Branch (read):** Recurses through `Named`, `FixedArray`, `Constrained`, and `Generic` record fields. `TypeReferenceNode::Reference`, `Slice`, and `DynamicTrait` (and other variants) hit `_ => false` (`584`).

**Callers:** Search `\bcarries_activation_claim\b` in `omega-rust`: **8** matches in `value_custody_core.rs` (definition and internal recursion) (read). Search `\btype_carries_activation_claim\b`: **3** matches — `value_custody_core.rs:513`, re-export `lib.rs:2094`, use `pipeline/05_typed-trees-to-checked-trees/src/execution_terminal_unit_state_graph.rs:1085` (read).

**Upward (read):** A type that stores an activation claim only behind a reference, slice, or dynamic trait is reported as not carrying a claim; downstream checks keyed on `type_carries_activation_claim` can treat the program as lacking activation storage obligations.

## A0567

**Function (read):** `append_linear_claim_frontier` (`1354-1527`).

**Branch (read):** `FixedArray` with `FixedArrayLength::Literal` is handled at `1384-1432` (element frontier expansion). In the outer `match`, other `FixedArray` shapes fall through `_` (`1450`). In the inner `match`, `TypeReferenceNode::FixedArray { .. } => {}` at `1521` is a no-op — no claims added for const-parameter or expression array lengths.

**Callers:** Search `\bappend_linear_claim_frontier\b` in `omega-rust`: **8** matches, all in `value_custody_core.rs` — including `linear_claim_frontier` / `type_carries_linear_obligation` entry points `1325`, `1342` (read).

**Upward (read):** `linear_claim_frontier` (`1313-1317`) memoizes this walk and is used across multiplicity and custody checks (pipeline `checks_multiplicity*` et al., grep counts only). A `[Linear; N]` type whose length is a const generic parameter or non-literal length gets no per-element linear frontier from this layer.

## A0568

**Function (read):** `append_linear_claim_frontier` (`1354-1527`).

**Branch (read):** For affine/multiplicity expansion on `Named`, `find_data_definition` (`1460`) returning `None` causes an immediate `return` (`1461`) without pushing claims — same for generic base at `1491-1494` when the base definition is missing.

**Helper (read):** `find_data_definition` (`1725-1734`) uses symbol index or name lookup; unresolved symbols/names yield `None`.

**Callers:** Same as A0567 — **8** `append_linear_claim_frontier` matches in `value_custody_core.rs` (read).

**Upward (read):** Linear obligation discovery for a type spelled as a `Named` reference that does not resolve to a data definition produces an empty subtree here, so nested linear fields inside that nominal are not enumerated at this layer.

## What I did not read

- Bodies of `exact_structured_const_carrier_is_eligible`, `validate_exact_const_identity_in_carrier`, and `check_closed_const_expression_in_carrier` beyond call sites (not required to decide the cited branches).
- Remainder of `validate_open_index_expression` after the binary branch (`3893+`).
- Full `validate_schema` field-type validation and other wire compatibility modules besides `validate_adjacent_eras`.
- `memoized_claim_frontier` implementation and downstream multiplicity check bodies beyond grep call sites.
- Historical file at commit `457adf4a11f` (current checkout line numbers matched cited symbols; read).

SLICE COMPLETE
