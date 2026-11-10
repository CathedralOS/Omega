# slice_0062

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0473 | omega-rust/psi/compilation/validation/src/expression_types_and_fields.rs:1444-1462 | REACHABLE | Match arms with neither declared types nor value classes are treated compatible via `(None, None) => false` on data mismatch. |
| A0474 | omega-rust/psi/compilation/validation/src/expression_types_and_fields.rs:1270-1275 | REACHABLE | `common_static_type_reference` hit skips later per-arm `check_compatible_values` even when arm values may still disagree. |
| A0475 | omega-rust/psi/compilation/validation/src/expression_types_and_fields.rs:3078-3340 | REACHABLE | `carrier_conflict` / `delivered_type_conflict` return no-conflict on substitution failure, references, missing carriers, and unreachable context. |
| A0476 | omega-rust/psi/compilation/validation/src/expression_types_and_fields.rs:7030-7178 | REACHABLE | `Slice`, `Generic`, `DynamicTrait`, and plain `Named` arms accept many expression kinds by `matches!` before identity checking. |
| A0477 | omega-rust/psi/compilation/validation/src/expression_types_and_fields.rs:685-704 | REACHABLE | `MatchPattern::Value(_)` always yields `structural_match_pattern_is_total` true, inflating structural coverage. |
| A0478 | omega-rust/psi/compilation/validation/src/frozen_program_and_queries.rs:1097-1119 | UNDECIDED | After `program_mut`, cycle-cut hooks are false; whether that lets an ill-typed program through validation was not traced to accept/reject. |
| A0479 | omega-rust/psi/compilation/validation/src/literals_and_struct_literals.rs:4229-4231 | REACHABLE | Non–value-parameter data types skip `enforce_construction_field_obligations` when `construction_members_are_concrete` is false. |
| A0480 | omega-rust/psi/compilation/validation/src/literals_and_struct_literals.rs:5509-5547 | REACHABLE | Const struct scan keeps only `evaluates FALSE` diagnostics; with no machines, `const_declaration_probe` skips const literals entirely. |

## A0473

**Cited:** `expression_types_and_fields.rs:1444-1462` (read; lines match digest commit `457adf4a11f` at this path).

**Enclosing function:** `check_compatible_values` (`expression_types_and_fields.rs:1353-1470`, read).

**Callers:** ripgrep `\bcheck_compatible_values\b` under `omega-rust`, count 4 — all in `expression_types_and_fields.rs` at 1245, 1301, 1364, 1378 (read).

**Upward path:** `validate_match_dispatch` (`1210-1310`, read) calls `check_compatible_values` for live arms after optional early exit. Production caller: `calls.rs:2542-2548` (read) during statement validation; export `lib.rs:1839` (read).

**Branch input:** Reaches the `else` at 1428 when both `peer_type` and `value_type` are `None` (no `declared_value_type` and no assignment value type from 1398-1405). With both `peer_class` and `value_class` `None`, `class_mismatch` is false (1441-1443). With neither struct-literal data symbol, `(None, None) => false` at 1460-1461 (read).

**Verdict REACHABLE:** A typed `match` whose arms are opaque expressions without declared types or value classes should be rejected as incompatible arms but passes compatibility.

## A0474

**Cited:** `expression_types_and_fields.rs:1270-1275` (read).

**Enclosing function:** `validate_match_dispatch` (1210-1310, read).

**Callers:** ripgrep `\bvalidate_match_dispatch\b` under `omega-rust`, count 5 — definitions/re-exports in `expression_types_and_fields.rs:1210,6548`, `lib.rs:1839`, production `calls.rs:2542`, `build-time-evaluation/const_generic_expressions.rs:3624`, comment in `checks_ranges_facts.rs:1549` (read).

**Branch input:** `declared_arms` is `Some` and `program.common_static_type_reference(values)` is `Some` at 1270-1273; function returns at 1275 before the loop at 1297-1309 that calls `check_compatible_values` (read).

**Verdict REACHABLE:** Match arms that share one common static type reference but produce incompatible concrete values are not pairwise-checked.

## A0475

**Cited:** `expression_types_and_fields.rs:3078-3340` (read).

**Functions read in full:** `carrier_conflict` (3078-3241), `delivered_type_conflict` (3290-3340), `delivered_carrier_conflict` (3056-3076), `report_delivered_type_mismatch_with_component` (3361-3387).

**Silent no-conflict paths (read):**
- `substituted_reference` fails → `return false` (3103-3105).
- Either side `TypeReferenceNode::Reference` → `return false` (3138-3140).
- `carrier(...)` returns `None` → `return false` (3144-3147, 3170-3173).
- Foreign rigid parameter → `return false` (3151-3152).
- `delivered_type_conflict`: unreachable context → `None` (3296-3297).
- `delivered_carrier_conflict` only reports when inner `carrier_conflict` is true and `scope.is_valid()` (3065-3075).

**Callers:** `delivered_carrier_conflict` — ripgrep count 3, all `expression_types_and_fields.rs` (6929, 3326, 3056 pub). `report_delivered_type_mismatch*` — `program_validation.rs:1721,2866,3677`, `literals_and_struct_literals.rs:4299` (read).

**Verdict REACHABLE:** A delivery site where actual/required types involve references, failed substitution, or unresolved carriers gets no mismatch diagnostic from these reporters.

## A0476

**Cited:** `expression_types_and_fields.rs:7030-7178` (read; digest also mentions 7168-7178, same `Named` fallback block).

**Enclosing function:** `argument_matches_type_reference_using` (6712-7181, read).

**Callers:** ripgrep count 6 in `expression_types_and_fields.rs` — 6662, 6693, 6702, 6751, 7402 plus definition (read).

**Upward path:** `argument_matches_type_reference_handle` (read from 6688) is used for assignment and call argument fitting during validation.

**Branch input:** For `TypeReferenceNode::Slice`, any `Call`, `Indexed`, `Member`, or `Name` matches at 7035-7041 without `value_carrier_matches_reference` (read). `Generic` and `DynamicTrait` accept similar kind lists (7043-7061). Plain `Named` non-primitive falls through to kind-only `matches!` at 7168-7178 (read).

**Verdict REACHABLE:** An argument required to be a `Slice` (or generic/trait/named carrier) can be a `Call` (including one whose constructed value type is unset) and still satisfy `argument_matches_type_reference_using`.

## A0477

**Cited:** `expression_types_and_fields.rs:685-704` (read).

**Enclosing function:** `structural_match_pattern_is_total` (685-705, read).

**Callers:** ripgrep count — `expression_types_and_fields.rs:851,1079,6546`; `lib.rs:1847`; pipeline `flow_expression.rs:1070,1182`; `flow_exits.rs:934` (read).

**Upward path:** `structural_patterns::validate_for_subject` returns `Some((case, true))` for non-destructure patterns at 794-795 (read). `validate_match_coverage` uses structural `validate` with `total` at 1166-1177 (read).

**Branch input:** Any `MatchPattern::Value(_)` hits `=> true` at 690 (read).

**Verdict REACHABLE:** A structural match with only a value-pattern arm can be treated as total for case coverage though it does not destructure all variants.

## A0478

**Cited:** `fz:1097-1119` → `frozen_program_and_queries.rs:1092-1118` (read; re-located by symbol `CheckingProjection` impl).

**Related read:** `program_mut` (1064-1071) sets `projected`; `memo_source` (1074-1076) is `None` once projected exists. Default `CheckingInput` hooks return false at 575-600 except `has_active_recast_formation` uses `depth >= 64` at 594-599 (read).

**Callers:** `has_active_*` — ripgrep under `psi/compilation/validation`, count 8 references in `frozen_program_and_queries.rs` (read).

**Observation:** After `program_mut()`, all four `has_active_*` on `CheckingProjection` delegate only when `memo_source()` is `Some`; otherwise numeric/operand/case return false and recast uses `depth >= 64` at 1097-1098 (read). Digest notes “Suspected unbounded recursion risk; not an accept.”

**Verdict UNDECIDED:** Cycle cuts are off on the mutated projection (read), but no path was traced from that to a program being accepted without diagnosis.

## A0479

**Cited:** `literals_and_struct_literals.rs:4229-4231` (read).

**Enclosing function:** `enforce_construction_field_obligations` (4211+, read). `construction_members_are_concrete` (4201-4208) is true only when every data type parameter is `TypeParameterKind::Value` (read).

**Callers:** ripgrep `\benforce_construction_field_obligations\b` under `omega-rust`, count 3 — definition, `use` at 5355, call at 5656 (read).

**Upward path:** Called from `scan_expression` during `validate_struct_literal_fields` prologue (`program_validation.rs:563`, read).

**Branch input:** Data definition with non-Value type parameters (e.g. type parameters) makes `construction_members_are_concrete` false; early `return` at 4229-4230 skips field obligation loop (read).

**Verdict REACHABLE:** A generic data struct literal with a resolved application type but non-concrete members skips construction field obligations in this pass.

## A0480

**Cited:** `literals_and_struct_literals.rs:5509-5547` (read).

**Enclosing function:** `validate_struct_literal_fields` (5388-5553, read). `const_declaration_probe` (5555-5564) requires `program.machines().first()` (read).

**Machine path (contrast, read):** Per-machine `scan_expression` → `validate_literal_field_names` (`5820-5918`) reports duplicate and unknown fields (`5837`, `5914-5918`).

**Const path:** Loop at 5509-5547 runs `scan_expression` on const struct initializers but `diagnostics.extend` only keeps messages containing `"evaluates FALSE"` (5542-5545, read). If `const_declaration_probe` is `None` (no machines), block skipped entirely (5509, read).

**Verdict REACHABLE:** A package with no machines, or a top-level `const` whose struct literal has duplicate/unknown/out-of-range fields, can pass this validator without those field diagnostics (machine-body literals are still scanned when machines exist).

## What I did not read

- Full bodies of every `CheckingInput` implementor beyond `CheckingProjection` and default trait (`frozen_program_and_queries.rs` beyond sampled ranges).
- Entire `program_validation.rs` prologue ordering beyond `validate_struct_literal_fields` cite.
- Earlier compiler phases (typed-trees) for duplicate struct-field reporting on consts.
- Runtime or build-time evaluation paths beyond `const_generic_expressions.rs:3624` caller cite.

SLICE COMPLETE

