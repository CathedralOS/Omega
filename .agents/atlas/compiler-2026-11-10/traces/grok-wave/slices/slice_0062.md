slice_0062

| entry id | file:line | verdict | one-line reason |
|----------|-----------|---------|-----------------|
| A0473 | expression_types_and_fields.rs:1444-1462 | REACHABLE | `check_compatible_values` treats two expressions with no declared type and no `value_class` as compatible (`(None, None) => false`). |
| A0474 | expression_types_and_fields.rs:1270-1275 | REACHABLE | `validate_match_dispatch` returns before arm checks when `common_static_type_reference` finds a shared static type. |
| A0475 | expression_types_and_fields.rs:3078-3340 | REACHABLE | `carrier_conflict` / `delivered_type_conflict` return “no conflict” on failed substitution, missing carriers, and non-`Reference` `Reference` shapes without a later mandatory error on that path. |
| A0476 | expression_types_and_fields.rs:7030-7178 | REACHABLE | `argument_matches_type_reference_using` accepts `Slice`/`Generic`/`DynamicTrait`/plain `Named` operands from node kind alone before deeper checks. |
| A0477 | expression_types_and_fields.rs:685-704 | CLOSED | `MatchPattern::Value => true` only reaches flow paths that also require `structural_match_pattern_case` to return `Some` (variant-style heads). |
| A0478 | frozen_program_and_queries.rs:1097-1119 | CLOSED | After `program_mut()`, `memo_source` is `None`, so parent query cycle cuts are not inherited; main `validate` uses `CheckingContext`, not this projection path. |
| A0479 | literals_and_struct_literals.rs:4229-4231 | REACHABLE | Non–all-`Value` generic applications skip `enforce_construction_field_obligations` while name/shape checks elsewhere still apply. |
| A0480 | literals_and_struct_literals.rs:5509-5547 | REACHABLE | Const struct scan drops all diagnostics except those containing `evaluates FALSE`, including field-name errors from `validate_literal_field_names`. |

### A0473

1. Read `check_compatible_values` in full at `expression_types_and_fields.rs:1353-1470` (read).
2. Search `check_compatible_values\(` under `omega-rust`: 5 hits, all in `expression_types_and_fields.rs` (read); callers are `validate_match_dispatch` at `1245` and `1301` (read enclosing `validate_match_dispatch` at `1210-1310`, read).
3. `validate_match_dispatch` is exported (`lib.rs:1839`) and called from `calls.rs:2542` and build-time `const_generic_expressions.rs:3624` (grep, not full `calls.rs` read). I did not trace every statement validator that reaches those call sites.
4. Input on the accepting branch: two match arm value expressions where `declared_value_type` is `None` for both, `value_class` is `None` for both, and neither side is a typed `StructLiteral` with mismatched `type_symbol` (read `1428-1462`).
5. **Verdict: REACHABLE.** A match whose arms are differently typed literals or calls that never received static types or classes could pass arm compatibility while still disagreeing at runtime.
6. Unsure: whether typed-tree lowering always assigns `value_class` or a declared type before validation for every arm shape that can disagree.

### A0474

1. Read `validate_match_dispatch` at `1210-1310`, especially `1270-1275` (read).
2. Search `common_static_type_reference`: definition `typed-trees/src/type_system.rs:1921-1954` (read); use sites in validation at `etf:1272` and `etf:4922` (second site not read in full).
3. When `declared_arms` lists live arms with declared types and `program.common_static_type_reference` returns `Some`, the function returns without `check_compatible_values` (read `1270-1310`).
4. Input: live match arms that share a static type candidate from the static-type context but whose runtime/refined values could still disagree.
5. **Verdict: REACHABLE.** Arms that agree in the shared static typing world but not in normalized identity can skip pairwise compatibility checking.
6. Unsure: how often arm expressions lack refinements that would have been caught by the skipped loop.

### A0475

1. Read `carrier_conflict` at `3078-3241`, `delivered_type_conflict` at `3290-3340`, and `report_delivered_type_mismatch_with_component` at `3361-3387` (read).
2. Search `delivered_carrier_conflict` / `carrier_conflict(` under validation: 7 hits in `expression_types_and_fields.rs` (grep). Callers of `report_delivered_type_mismatch*` include `program_validation.rs:1721`, `2866`, `3677` and `literals_and_struct_literals.rs:4299` (grep; caller bodies not all read).
3. Early `return false` (“no conflict”) at failed substitution `3099-3104`, either `Reference` operand `3138-3140`, missing `carrier()` `3144-3147` / `3170-3173`, non-generic tail `3191-3192`; unreachable values return `None` at `3296-3297` (read).
4. Input: a value whose delivered type fails substitution or lacks a resolved carrier while the destination expects a concrete carrier identity.
5. **Verdict: REACHABLE.** A delivery mismatch can produce no `delivered_type_conflict` and thus no error from `report_delivered_type_mismatch` at `3380-3386` unless `report_delivery` at `3370-3378` already fired (read; did not trace all `report_delivery` outcomes).
6. Unsure: whether `report_delivery` always covers the substitution-failure shapes I did not read in full.

### A0476

1. Read `argument_matches_type_reference_using` at `6712-7182`, focusing on `7030-7178` (read).
2. Search `argument_matches_type_reference_handle`: re-export `lib.rs:1835`; production uses in `machine_calls_core.rs:7312`, `machine_parameters_and_bindings.rs:1811`, `proof_contracts_core.rs:6154`, `type_references_and_wire.rs:2099`, `float_and_mathematical_contracts.rs:1505`, `validate_expression_type_handle` at `etf:7402` (grep; not all call sites read).
3. For `TypeReferenceNode::Slice`, any `Call`/`Name`/… node kind matches before declared-type checks (read `7030-7042`); similar kind-only arms for `Generic`, `DynamicTrait`, and plain `Named` (read `7043-7178`).
4. Input: a `Call` (or other listed kind) passed where a `Slice`/`Generic`/trait object/`Named` type is required, including when `constructed_value_type_reference` is unset.
5. **Verdict: REACHABLE.** An arbitrary call can satisfy the slice/generic/named correspondence hook on shape alone; rejection depends on later validators not read here.
6. Unsure: whether `validate_expression_type_handle` is always reached for the same sites.

### A0477

1. Read `structural_match_pattern_is_total` at `685-705` and `structural_match_pattern_case_for_subject` at `601-683` (read).
2. Search `\bstructural_match_pattern_is_total\b` in `omega-rust`: 8 hits — definitions/reexports plus production uses in `flow_expression.rs:1070`, `1182` and `flow_exits.rs:934` (grep); read those call sites (read `flow_expression.rs:1055-1183`, `flow_exits.rs:925-935`, `retain_owned_case_selection` signature at `895-903`).
3. `MatchPattern::Value(_) => true` at `690` (read). Production uses pair `is_total` with `structural_match_pattern_case` returning `Some(case)` (`flow_expression.rs:1068-1071`, `1173-1183`, `flow_exits.rs:925-934`).
4. Input: value patterns whose head is not a variant path, empty case struct, etc., yield `None` from `structural_match_pattern_case_for_subject` at `682` even when `is_total` is true.
5. **Verdict: CLOSED.** The total flag does not relax coverage or owned-case semantics unless a case symbol is already resolved; integer or opaque value patterns do not take the accepting branch.
6. Unsure: none material for this entry.

### A0478

1. Read `CheckingProjection::program_mut` at `1064-1071`, `memo_source` at `1074-1076`, and `has_active_*` overrides at `1092-1118` (read).
2. Search `program_mut(` in `omega-rust`: production uses in `omega/packages/review-evidence/...` (grep `frozen_program_and_queries.rs:162`, `238` tests); `program_validation.rs:4067` `validate` takes `CheckingInput` without requiring projection (read prologue only).
3. After `program_mut`, `memo_source()` is `None`, so `has_active_operand_fit_population` etc. always read false from the parent (read `1107-1118`). `CheckingQuery` constructors still self-cut on the active query kind (`frozen_program_and_queries.rs:360-395`, read).
4. Input: projected copy used while mutating type tables (review-evidence), not the frozen program under main validation.
5. **Verdict: CLOSED.** This disables inherited cycle cuts on an editable projection slice; it is not the gate that admits a whole program in `validate` (`program_validation.rs:4067`).
6. Unsure: whether any review-evidence path treats a failed cut as acceptance; not traced.

### A0479

1. Read `enforce_construction_field_obligations` at `4211-4231` and `construction_members_are_concrete` at `4201-4208` (read); `selected_construction_definition` at `4771-4781` (read).
2. Search `enforce_construction_field_obligations`: 2 hits — definition and `scan_expression` at `5656` (grep). Search `construction_members_are_concrete`: 4 hits in `literals_and_struct_literals.rs` (grep).
3. When `!construction_members_are_concrete` (any non-`Value` type parameter), the function returns before the per-field obligation loop (read `4229-4231`).
4. Input: generic data literal with a valid applied type (`construction_application` valid at `4748-4752`) but type parameters include `Type`/`Implementation`/etc.
5. **Verdict: REACHABLE.** Range, reference-access, and stored-write obligations for fields are skipped; `validate_literal_field_names` at `5820-5918` still checks names/duplicates on machine-scanned literals (read), not necessarily the same obligation set.
6. Unsure: whether another validator replays field obligations for generic constructions everywhere they are omitted.

### A0480

1. Read `validate_struct_literal_fields` tail at `5509-5547` and `const_declaration_probe` at `5555-5565` (read); `scan_expression` / `validate_literal_field_names` at `5645-5657`, `5820-5918` (read).
2. Search `validate_struct_literal_fields`: prologue hook `program_validation.rs:563` (grep, read line only).
3. Const branch runs only if `const_declaration_probe` finds a machine (`5509`); `scan_expression` may emit duplicate/unknown field errors, then `5542-5545` keeps only messages containing `evaluates FALSE` (read).
4. Input: top-level `const` whose initializer is a `StructLiteral` with duplicate or unknown fields, on a program with at least one machine (probe almost always succeeds if any machine exists, read `5563-5565`).
5. **Verdict: REACHABLE.** Those field errors are generated then discarded; `validate_constants` at `value_custody_core.rs:3395-3405` only checks copy eligibility (read), not field names.
6. Unsure: whether build-time `validate_constant_literal_fields` (`value_custody_core.rs:3408`, called from `build-time-evaluation/src/const_initializers.rs:2123` per grep) rejects the same bad literals before ship.

### What I did not read

- Full bodies of every `argument_matches_type_reference_*` caller beyond grep hits.
- `report_delivery` / `pattern_components` implementation behind `3370-3378`.
- `calls.rs` paths into `validate_match_dispatch`.
- `expression_types_and_fields.rs:4922` second `common_static_type_reference` use.
- Review-evidence conformance capture after `CheckingProjection::program_mut`.
- Build-time const initializer pipeline end-to-end.

SLICE COMPLETE
