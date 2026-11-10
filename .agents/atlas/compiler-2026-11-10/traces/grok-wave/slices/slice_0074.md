slice_0074

| entry id | file:line | verdict | one-line reason |
|----------|-----------|---------|-----------------|
| A0569 | value_custody_core.rs:2025-2033 | CLOSED | `type_graph_requires_nominal_drop` returns false on ill-formed generics only after `trw` type-reference validation already errors the same program (read). |
| A0570 | value_custody_core.rs:2142-2149 | CLOSED | Unresolved selection targets are skipped here but rejected when selections finalize or package authority is checked (read). |
| A0571 | value_custody_core.rs:2399-2403 | CLOSED | Silent early return applies only when source type or cleanup resolution is missing; conflicting cleanup on type references is diagnosed earlier in the same validation pass (read). |
| A0572 | value_custody_core.rs:2838-2850 | REACHABLE | Non-empty lifetime bindings can pass when either side lacks extractable region arguments because the helper returns true (read). |
| A0573 | value_custody_core.rs:2785-2787 | REACHABLE | Cleanup selection whose static application names a missing machine skips owner validation with no diagnostic (read). |
| A0574 | value_custody_core.rs:2971-2997 | REACHABLE | `Value`/`Const` arguments without a static carrier skip further checks except the narrow `Const`+eligible branch (read). |
| A0575 | value_custody_core.rs:3418-3445 | REACHABLE | Array literal initializer against a non-array destination carrier falls through and the walk still returns `Ok(())` (read). |
| A0576 | value_custody_core.rs:4052;4068 | REACHABLE | Entry description matches: unresolved `Named` yields no establishment; unresolved `Generic` via `is_none_or` yields establishment required (read). |

### A0569

1. Read `type_graph_requires_nominal_drop` in full at `omega-rust/psi/compilation/validation/src/value_custody_core.rs:1959-2047` (read).
2. Search: `rg '\btype_graph_requires_nominal_drop\b' omega-rust` — 15 hits in 8 files (read).
3. Callers traced: internal recursion at vcc:2004-2007,2041; `data_requires_nominal_drop` at vcc:2049-2081; re-export `lib.rs:1751`; checked-tree consumers in `execution.rs`, `execution_terminal_unit*.rs` (names only; did not read those call sites’ full bodies).
4. `validate_program` → prologue → `validate_cleanup_machine_declarations` (`program_validation.rs:426`) does not call `type_graph_requires_nominal_drop`; acceptance is decided via `finish_diagnostics` (`program_validation.rs:4819-4826`) after type-reference validation elsewhere.
5. Generic `Named`/`Generic` arms: missing `data_definition_by_symbol` → `return false` at vcc:2024-2025; argument-count mismatch → `return false` at vcc:2031-2032 (read).
6. Input to accepting branch: a `TypeReferenceNode::Generic` handle whose base symbol does not resolve to a data definition, or whose argument list length does not match the definition’s parameters, during nominal-drop graph walk (typically on programs that already failed or will fail type-reference validation).
7. **CLOSED** — `validate_type_reference_node` errors unknown generics at `type_references_and_wire.rs:5207-5209` and argument-count mismatch at `5251-5256` (read); those diagnostics fail `validate_program` before a malformed generic can be accepted solely because vcc:2025/2032 returned false.
8. Unsure: whether any harness invokes `type_graph_requires_nominal_drop` on typed programs that bypass `trw` prologue validators (not traced).

### A0570

1. Read `collect_reserved_cleanup_selection_diagnostics` in full at vcc:2124-2161 (read).
2. Search: `rg '\bcollect_reserved_cleanup_selection_diagnostics\b' omega-rust` — 3 hits (read).
3. Callers: `validate_reserved_cleanup_selections` vcc:2197-2202 (`finish_strict_diagnostics`); `program_validation.rs:371` prologue (aggregates into `finish_diagnostics` at end of `validate`).
4. Accepting branches: `selection_target.as_resolved()` is `None` → `continue` vcc:2142-2143; `reserved_cleanup_selected_by` returns `None` → `continue` vcc:2147-2148 (read).
5. Input: an `authored_declaration_selections` entry that names a reserved cleanup in source but whose target is `LateBound` or does not map to a reserved cleanup machine symbol.
6. **CLOSED** — Unresolved late-bound targets are rejected at `declarations_core.rs:1009-1013` (read) and during `finalize_checked_authored_selections` in `checking.rs:1215-1218` / `shared.rs:1417` (read, coordinator path not read in full); the `continue` only skips selections that cannot yet name a symbol, not ones that should pass checking.
7. Unsure: every selection kind that can point at a reserved cleanup while staying `LateBound` through the whole of `validate_program` (not enumerated).

### A0571

1. Read `validate_cleanup_delivery` in full at vcc:2380-2416 (read).
2. Search: `rg '\bvalidate_cleanup_delivery\b' omega-rust` — 3 hits (read).
3. Callers read: `proof_contracts_core.rs:2045` inside `validate_domain_delivery` (guards invalid value/local at 2036-2042, read); `expression_types_and_fields.rs:40` in `validate_cast_types` when `state` is `Some` (read). Upstream: statement validation and cast type checking feed `diagnostics` → `validate_program` → `finish_diagnostics`.
4. Accepting branch: `let Ok(Some(source)) = selected_application(...) else { return; }` at vcc:2399-2402; also `expression_result_type_reference` `None` → `return` vcc:2392-2397 (read).
5. Input: cast or domain-delivery site where the expression’s result type cannot be computed, or cleanup selection on the source type is absent/`Err` (not `Ok(Some(_))`).
6. **CLOSED** — `validate_cleanup_machine_declarations` (`vcc:2092-2107`, prologue `program_validation.rs:426`) runs `selected_application` on every collected type reference and pushes `Err` diagnostics (read), so a conflicting/malformed cleanup graph cannot reach program acceptance while `validate_cleanup_delivery` silently returns on `Err`.
7. Unsure: expression-only types never appearing in `collect_program_references` but still participating in casts (not verified).

### A0572

1. Read `receiver_regions_match` and enclosing `validate_declaration_owners` at vcc:2757-2858 (read); `region_arguments` at vcc:2860-2872 (read).
2. Search: `rg '\breceiver_regions_match\b' omega-rust` — 2 hits (vcc only) (read).
3. Caller: `validate_cleanup_machine_declarations` → `validate_declaration_owners` at vcc:2096 (read) → prologue `program_validation.rs:426`.
4. Accepting branches: empty lifetime bindings → `return true` vcc:2838-2839; `(receiver, subject)` region lists not both `Some` → `return true` vcc:2848-2849 (read).
5. Input: `CleanupSelection` with non-empty `static_machine_application_lifetimes` where the receiver type handle peels to a shape without `Generic { lifetime_arguments }` while the subject supplies lifetime parameters (or the converse), so region comparison is abandoned.
6. **REACHABLE** — A program could pass `receiver_regions_match` with mismatched authored lifetime wiring that should be rejected, because incomparable region metadata is treated as success; downstream only requires `shape == Some(true)` at vcc:2815-2826, which does not re-derive regions.
7. Unsure: whether `trw` lifetime validation on the same application always supplies both sides before this runs (not fully traced).

### A0573

1. Read `validate_declaration_owners` at vcc:2757-2828 (read).
2. Search: `rg '\bvalidate_declaration_owners\b' omega-rust` — 2 hits (read).
3. Caller chain same as A0572 (vcc:2096, `program_validation.rs:426`).
4. Accepting branch: `program.machine(application.registration_operation)` is `None` → `continue` vcc:2785-2787 (read).
5. Input: row in `program.cleanup_selections()` whose `static_machine_application.registration_operation` does not resolve to a `Machine` in the typed program.
6. **REACHABLE** — That cleanup selection never receives the “receiver does not match” error at vcc:2816-2826 because the whole selection is skipped; no alternate validator for the same orphan selection was found in callers read.
7. Unsure: whether table invariants guaranteed by an earlier pipeline stage make this unreachable in ordinary source compilation (inferred only, not read).

### A0574

1. Read `validate_argument_recipes` in full at vcc:2962-3027 and `has_carrier` at vcc:3029-3070 (partial tail unread) (read).
2. Search: `rg '\bvalidate_argument_recipes\b' omega-rust` — 5 hits (read).
3. Callers read: `machine_parameters_and_bindings.rs:5871`, `6164`, `6548` via `validate_source_recipes` / `validate_source_const_recipes` (read `6163-6172`, `6155-6162`); results go to `finish_diagnostics` / `finish_strict_diagnostics` (`mpb:6325`, `program_validation.rs` integration not read line-by-line).
4. Accepting branches: `parameters.len() != arguments.len()` → `return` vcc:2971-2972; `!has_carrier(...)` → `continue` after optional `Const` diagnostic vcc:2986-2997 (read).
5. Input: static call/recipe invocation where a `TypeParameterKind::Value` (or non-eligible `Const`) argument expression has no provable static carrier under `has_carrier`.
6. **REACHABLE** — Such a `Value` argument is not diagnosed (only `Const` with eligible exact carrier gets vcc:2992-2995); the call’s recipe validation can complete without rejecting the argument.
7. Unsure: `source_telescope` always pairing equal-length parameter/argument slices in practice (read `mpb:6328-6371` only; not all call sites).

### A0575

1. Read `validate_constant_literal_fields` at vcc:3408-3447 (read).
2. Search: `rg '\bvalidate_constant_literal_fields\b' omega-rust` — 3 hits (read).
3. Caller read: `build-time-evaluation/src/const_initializers.rs:2123-2128` maps `Err` to failure but accepts `Ok(())` (read); not traced back to full compile acceptance in this slice.
4. Accepting branch: `ExpressionNode::ArrayLiteral` where `carrier` is not `FixedArray` — no `pending` pushes, loop continues vcc:3419-3429 (read).
5. Input: `const` declaration whose `declared_type` is not a fixed array while `materialized_initializer` is an array literal (mismatched shape).
6. **REACHABLE** — Walk ends with `Ok(())` vcc:3447 without diagnosing the mismatch; a program whose constants rely on this check alone could accept the wrong initializer shape.
7. Unsure: whether `validate_constants` / `eligible` at vcc:3395-3405 rejects the same constant earlier (not read for this pairing).

### A0576

1. Read `type_requires_establishment_inner` `Named` and `Generic` arms at vcc:4037-4074 (read); public wrapper `type_requires_establishment` at vcc:3866-3883 (read).
2. Search: `rg '\btype_requires_establishment_inner\b' omega-rust` — 7 hits (vcc only) (read); public `type_requires_establishment` used in validation and `05_typed-trees-to-checked-trees` (grep only).
3. Entry description: **correct** (read) — `Named` uses `definition.is_some_and(...)` (false when no definition); `Generic` uses `.is_none_or(...)` (true when no definition).
4. Input to branches: type reference handle where `closed_data_definition` / `data_definition_by_symbol` cannot supply a definition for `Named`, or for `Generic` base.
5. **REACHABLE** — For the same “unresolved nominal” situation, `Named` answers “no establishment required” while `Generic` answers “establishment required,” so multiplicity/flow code calling `type_requires_establishment` (`checks_multiplicity.rs:1748`, read grep) can treat spelling variants differently if both handles can coexist past earlier checks.
6. **CLOSED partial:** sourced programs with unknown type names still get `trw` errors at `type_references_and_wire.rs:5455-5458` (read) — asymmetry bites when symbol resolution succeeds without a closed data definition (e.g. metadata-scarce fixtures).
7. Unsure: prevalence of `Named` handles without definitions in programs that pass full `validate_program` (not run).

### What I did not read

- Full bodies of `type_graph_requires_nominal_drop` consumers in `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/execution*.rs`.
- `validate` coordinator in `program_validation.rs` beyond prologue list fragments and `finish_diagnostics`.
- `has_carrier` and `eligible`/`eligible_type` beyond cited ranges in vcc.
- `finalize_checked_authored_selections` / `shared.rs` finalize implementation in full.
- Contract-entailment and domain call sites of `type_requires_establishment` beyond grep.

SLICE COMPLETE
