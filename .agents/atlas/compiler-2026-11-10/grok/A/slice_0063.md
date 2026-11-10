# slice_0063

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0481 | lsl:3557-3559 | CLOSED | Only callers pass `data_definition.symbol` from an already-resolved `DataDefinition`; the early return is unreachable on consistent typed input. |
| A0482 | lsl:2776-2778 | REACHABLE | Suffixed integer literal assigned into a non-primitive declared type returns after float-only checks and never compares suffix to destination. |
| A0483 | lsl:714-726 | REACHABLE | `land_float_value_for_type` skips landing for non-float named destinations and silently ignores several other destination shapes. |
| A0484 | lsl:5259-5261 | REACHABLE | `array_window_shape` returning `None` exits `validate_array_window_elements` with no diagnostic and no element checks. |
| A0485 | mcc:617-633 | REACHABLE | Checked-flow call with empty boundary edges and a concrete target gets `Ok(None)` from direct-boundary lookup and is dropped from approval coordinates. |
| A0486 | mcc:366-400 | REACHABLE | `approved_boundary_traits` approves by boundary-trait identity only; `validate_boundary_calls` never reads provider installation tables. |
| A0487 | mcc:161-164 | REACHABLE | Generic-data machines that never enter the selection set skip the bound-satisfaction loop entirely. |
| A0488 | mcc:1931-1933 | REACHABLE | `context_is_closed` ignores invalid dependency symbols instead of treating them as open, so closure can be reported incorrectly. |

## A0481

Cited `omega-rust/psi/compilation/validation/src/literals_and_struct_literals.rs:3557-3559` (read): inside `validate_literal_facts` (`lsl:3541-3674`, read), `program.data_definition_by_symbol(owner)` returning `None` returns immediately and skips all `requires` fact judging.

Search `\bvalidate_literal_facts\b` in `omega-rust`: count **3** (1 definition, **2** call sites, both in `literals_and_struct_literals.rs`).

Callers (read): `validate_literal_default_domain` (`lsl:3421-3436`) passes `data_definition.symbol` with `data_definition` from `selected_construction_definition`; `validate_literal_case_constraints` (`lsl:3483-3498`) does the same with variant facts. Upstream construction validation (`lsl:5871-5898`, read) already holds `&DataDefinition` from `selected_construction_definition` before those calls.

**CLOSED:** On a consistent typed program, `owner` is always the symbol of the `DataDefinition` already in hand; `data_definition_by_symbol` must resolve it (`declarations_core.rs:28-33`, read). No path supplies a dangling owner, so the skip branch does not accept programs that should fail construction `requires` checking here.

## A0482

Cited `lsl:2761-2778` (read): in `validate_suffix_landing` (`lsl:2711-2814`, read), after `unwrapped_type_reference`, a fixed-array path recurses (`lsl:2766-2774`). Then `primitive_type_reference(unwrapped)` at `lsl:2776-2778` returns when the destination is not a primitive.

Search `\bvalidate_suffix_landing\b` in `omega-rust`: count **11** (9 in `literals_and_struct_literals.rs`, 1 in `expression_types_and_fields.rs`, 1 in `calls.rs`).

Prologue `validate_suffix_landings` (`lsl:2816-2909`, `program_validation.rs:380`, read) calls this for assignments, locals, returns, and struct fields. For a non-primitive unwrapped destination, only `validate_numeric_tree_destination` runs (`lsl:2779`), which is float-only (`lsl:156-164`, read); suffixed integer agreement at `lsl:2780-2794` is never reached.

**REACHABLE:** A program that assigns a width-suffixed integer literal (for example `42i64`) into a struct field or local whose declared type is a named non-primitive aggregate should be rejected for suffix/destination mismatch but can be accepted because the function returns at `lsl:2777-2778` without emitting a diagnostic.

## A0483

Cited `lsl:350` anchors `land_float_literal_destinations` (`lsl:350-566`, read). Pairs are landed at `lsl:560-561` via `land_float_value_for_type` (`lsl:632-728`, read).

Search `\bland_float_literal_destinations\b` in `omega-rust`: count **4** (definition, re-export in `lib.rs`, call in `preparation.rs:2061`, read).

In `land_float_value_for_type`, `TypeReferenceNode::Named` at `lsl:714-720` calls `land_float_tree` only for `F32`/`F64`; `_ => return` at `lsl:718` leaves other named primitives unlanded. `Slice`, `Generic`, `DynamicTrait`, `ConstExpression`, and `Unit` at `lsl:722-726` are no-ops.

**REACHABLE:** A program with a float literal assigned to a named integer primitive or other non-float named destination still compiles through preparation: the pair is collected but landing is skipped with no diagnostic at `lsl:718` or `lsl:722-726`.

## A0484

Cited `lsl:5247-5262` (read): `validate_array_window_elements` (`lsl:5234-5290+`, read) returns silently when `array_window_shape` is `None` (`lsl:5259-5261`). Earlier, if the indexed range node is missing, it returns at `lsl:5247-5252` with no diagnostic.

Search `\bvalidate_array_window_elements\b` in `omega-rust`: count **3** (definition, re-export, call `program_validation.rs:1605`, read).

`array_window_shape` (`lsl:5174-5231`, read) returns `None` when indexed/range nodes, collection type, unwrap, or length normalization fail.

**REACHABLE:** A program with a slice-window assignment on a place whose collection type or index bounds cannot be resolved should fail window element checking but can be accepted because validation exits at `lsl:5260-5261` without pushing a diagnostic.

## A0485

Cited `mcc:607-634` (read): inside `checked_boundary_call_coordinates` (`mcc:428-697`, read), for a call with valid `target_symbol` and empty `boundary_edges`, `exact_direct_boundary_requirement` (`mcc:699-730`, read) runs; `Ok(None)` at `mcc:630-633` `continue`s without recording a `BoundaryCallCoordinate`.

Search `\bchecked_boundary_call_coordinates\b` in `omega-rust`: count **2** (definition + call from `validate_boundary_calls` at `mcc:350`, read). `\bvalidate_boundary_calls\b`: count **3** (`mcc:348`, `lib.rs` re-export, `checking.rs:1394`, read).

When the target is a concrete state/machine symbol and not an exact boundary signature, `exact_direct_boundary_requirement` returns `Ok(None)` (`mcc:714`, read). That call never reaches `validate_boundary_calls` unapproved filtering (`mcc:352-362`, read).

**REACHABLE:** A checked-flow row for a concrete implementation call that lost its boundary-edge span should still be vetted as a boundary call but can be accepted because empty edges plus non-boundary target drop the call at `mcc:617-633`.

## A0486

Cited `mcc:366-400` (read): `approved_boundary_traits` marks boundary traits with unique symbol and name-in-module/package identity; no provider table lookup (read).

Search `\bvalidate_boundary_calls\b` as above. `approved_boundary_traits` is used only at `mcc:351-354` (read).

**REACHABLE:** A program that exercises a boundary capability through a trait that passes syntactic uniqueness approval but has no registered provider for that capability should be rejected at checking; it can be accepted because `validate_boundary_calls` only tests membership in `approved` and never consults provider installation data (`terminal-installation` is separate, digest preamble `area_14.md:50-51`, read).

## A0487

Cited `mcc:123-215` (read): `validate_selected_attached_method_bounds` (`mcc:120-303`, read) builds `selected` from const initializers, proof-output calls, and reachability over machines/states (`mcc:130-218`, read). Machines with `generic_data_template` that fail `method_is_selected` are skipped at `mcc:161-164` and never visited.

Search `\bvalidate_selected_attached_method_bounds\b` in `omega-rust`: count **3** (`mcc:120`, `lib.rs`, `checking.rs:1039`, read).

Bound checking runs only for machines in the filter at `mcc:221-224` where `method_is_selected` is true (`mcc:305-315`, read).

**REACHABLE:** A program defining a generic-data attached method that is never selected by the closure (never called and never reached from tracked expressions) should still have template argument bounds checked if instantiated, but validation accepts it with no bound diagnostics because the machine is never enqueued.

## A0488

Cited `mcc:1931-1933` (read): in `context_is_closed` (`mcc:1919-1953`, read), invalid symbols from `collect_type_symbol_dependencies` hit `continue` and do not return `false`.

Search `\bcontext_is_closed\b` in `omega-rust`: count **2** (definition + use `mcc:1650` in `static_interrupt_context`, read).

If an open type parameter or similar dependency were represented with an invalid handle, the walk would skip it and may return `true` at `mcc:1952`, allowing `static_interrupt_context` to proceed (`mcc:1657-1665`, read) when it should report an open context.

**REACHABLE:** A program whose installed-context referent depends on an invalid symbol entry in the typed graph can be treated as closed and admit static interrupt context when it should not.

## What I did not read

- Full bodies of `collect_type_symbol_dependencies` (`typed-trees/src/references.rs`) and provider admission in `terminal-installation` beyond digest preamble.
- Entire `validate_literal_facts` predicate tail below `lsl:3674`.
- Full `program_validation::validate` coordinator beyond prologue hook for suffix landings.
- Git object `457adf4a11f` (current checkout line numbers matched cited symbols on read).

SLICE COMPLETE

