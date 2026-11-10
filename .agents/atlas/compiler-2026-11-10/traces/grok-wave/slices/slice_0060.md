slice_0060

| entry id | file:line | verdict | one-line reason |
|----------|-----------|---------|-----------------|
| A0457 | lib.rs:1056-1098 | CLOSED | `None` only drops optional native-binding width defaults; caller falls back without accepting a bad struct literal |
| A0458 | fz:1751-1754 | CLOSED | Provenance-less symbols are kept only inside Build projection; projected slice is re-checked via `CheckRequest` |
| A0459 | fz:2090-2091 | CLOSED | Removing materialized product operands from the descriptor set is intentional; late-bound sources still error when not descriptor-eligible |
| A0460 | fz:2450-2452 | CLOSED | Invalid handles returning `true` only affect prefix traversal completeness, not validation acceptance |
| A0461 | fz:3334-3336 | CLOSED | Non-`TargetInputs` (`Invariant`) layouts have no per-target roster to verify |
| A0462 | fz:4458 | REACHABLE | Mismatched type-reference node kinds can satisfy region comparison via the default arm |
| A0463 | fz:4537-4539 | REACHABLE | Under `CallableRefinement`, a required domain with no actual counterpart is skipped instead of failing |
| A0464 | fz:4147-4149,4165-4167 | CLOSED | `Ok(None)` means no retained lifetime slot; mismatches return `Err` and reject `at_lifetime` |

### A0457 — `lib.rs:1056-1098`

1. Read `unused_coordinate_widths` in full at `omega-rust/psi/compilation/validation/src/lib.rs:1051-1099` (read).
2. Search: `rg '\bunused_coordinate_widths\b' /agent/repos/omega/omega-rust` → 3 hits: definition/re-export in `lib.rs`, one production call in `constructed_results.rs:945` (read).
3. Read caller `retain_constructor_result_type` at `omega-rust/psi/compilation/pipeline/04_typed-trees-to-typed-trees/src/constructed_results.rs:935-1120` (read): `defaults = unused_coordinate_widths(...)`; on `None`, `zero` stays invalid and the `let Some(defaults) = defaults` branch at 1101-1109 is never taken; empty proposals yield `None` from `proposals.first()?` at 1111 (read).
4. Read upward `retain_constructed_result_types` at 806-807: `None` omits a constructed result type for that expression rather than marking compile success (read).
5. Input reaching accepting branch: a struct literal whose `DllImport`/`ForeignBinding` vocabulary lookup or shape validation fails, so every `.ok()?` short-circuit yields `None` instead of width metadata.

**Verdict: CLOSED** — check at `lib.rs:1009-1048` (`application_widths` / `binding_widths`) emits diagnostics on the strict path; `unused_coordinate_widths` is a soft helper and does not decide program acceptance.

### A0458 — `fz:1751-1754`

1. Read `project_build_program` at `frozen_program_and_queries.rs:1683-1768` and closure at 1748-1755 (read).
2. Search: `rg '\bproject_build_program\b' /agent/repos/omega/omega-rust` → 4 hits: definition plus three calls in `omega/build-system/build-settings/src/admission/checking.rs:78,94,171` (read through 179).
3. Read `project_declarations` at 1929-2023: `retain_reference(symbol)` true forces parent symbols into `synthesized` during reference closure (read).
4. Input: symbols referenced in the Build slice whose `symbol_provenance_source_span` is `None` (compiler-generated or internal), so the third disjunct of the closure is true.
5. `project_build_program` already errors when entry or selected deps are outside Build context (1700-1712, read).

**Verdict: CLOSED** — extra retention does not skip validation; admission runs `psi_compiler::CheckRequest` on the projected program (`checking.rs:179-179`, read).

### A0459 — `fz:2090-2091`

1. Read `descriptor_selection_occurrences` at `frozen_program_and_queries.rs:2079-2093` and `materialized_product_operand_occurrences` at 2268-2313 (read).
2. Search: `rg '\bdescriptor_selection_occurrences\b' /agent/repos/omega/omega-rust` → 3 hits: definition, `declarations_core.rs:957`, `shared.rs:1384` (read both callers).
3. Read `checks_authored_selection` at `typed-trees/src/lib.rs:1561-1576` (read): false when occurrence is excluded or (with product frontier) source is outside Build/Toolchain.
4. Read `shared.rs:1385-1422`: unresolved late-bound selections error only when **not** in `descriptor_occurrences` (read).
5. Input: occurrence in the descriptor set because it is a product-request operand or a non-checked authored selection, after materialized operands are removed.

**Verdict: CLOSED** — skipping materialized operands and product-context selections is by design; main-source late-bound gaps still fail at `shared.rs:1406-1422` (read).

### A0460 — `fz:2450-2452`

1. Read `expression_has_inert_prefix` and inner `visit` at `frozen_program_and_queries.rs:2432-2484` (read).
2. Search: `rg '\bexpression_has_inert_prefix\b' /agent/repos/omega/omega-rust` → 3 hits: `places.rs:6419`, `checks_contracts_exits.rs:8451`, definition (read call sites; did not read full `checks_contracts_exits` module).
3. Callers use the result to add literal-arm bindings or collection-length facts, not to finish `validate` (read `places.rs:6416-6423`).
4. Input: invalid `ExpressionHandle` during subtree visit returns `true` for `complete`.

**Verdict: CLOSED** — acceptance is not decided here; downstream type/place checks still apply (inferred for invalid-handle rarity in typed IR).

### A0461 — `fz:3334-3336`

1. Read `layout_family_is_complete` at `frozen_program_and_queries.rs:3330-3352` (read).
2. Search: `rg '\blayout_family_is_complete\b' /agent/repos/omega/omega-rust` → 3 hits: definition, `value_custody_core.rs:7329`, internal `discover_layout_checking_targets` at 3410 (read both).
3. Read `validate_plans` at `value_custody_core.rs:7313-7332`: incomplete family pushes diagnostic and `continue`s (read).
4. Input: `PlanLaidRealizations::Invariant(_)` or any non-`TargetInputs` variant.

**Verdict: CLOSED** — returning `true` matches invariant layouts having no target-input roster (`access_views_and_commitments.rs:1235-1236`, inferred from grep).

### A0462 — `fz:4458`

1. Read `type_reference_region_applications` at `frozen_program_and_queries.rs:4227-4460` in full (read).
2. Search: `rg '\btype_reference_regions_match\b' /agent/repos/omega/omega-rust` → production uses in `borrow_and_reflection.rs:3049,3071`, `traits.rs:2903`, `contract_entailment.rs:42600` (read `borrow_and_reflection.rs:3031-3093`; skimmed entailment self-comparison).
3. After domain processing, paired arms compare references/generics/traits; `_ => true` at 4458-4458 runs when node kinds differ (read).
4. Input: `actual` and `required` handles whose resolved `TypeReferenceNode` variants are not one of the paired cases, after `type_structure` at 4253-4261 succeeds.

**Verdict: REACHABLE** — smallest rejecting case (plain words): a callable region check where one side’s parameter type is stored as a reference node and the required side as an unrelated node kind (e.g. generic application), so lifetime/child comparison is skipped and region matching reports success despite incompatible shapes (`borrow_and_reflection.rs:3049-3067`, read).

### A0463 — `fz:4537-4539`

1. Read `domain_region_applications` at `frozen_program_and_queries.rs:4468-4546` (read).
2. Caller is `type_reference_region_applications` at 4277-4284 with `TypeRegionRelationship::CallableRefinement` from `type_reference_regions_match` at 4214 (read).
3. Input: required-side domain obligation in `required_domains` with no matching entry in `actual_domains` (`found == false`).
4. `continue` at 4537-4538 skips that obligation without clearing applications; only under `CallableRefinement` (read).

**Verdict: REACHABLE** — smallest rejecting case (plain words): required callable type carries a domain constraint the actual callable type does not, and region refinement still succeeds because the missing obligation is skipped (`fz:4537-4539`, read).

### A0464 — `fz:4147-4149,4165-4167`

1. Read `retained_lifetime_identity` at `frozen_program_and_queries.rs:4139-4174` (read).
2. Search: `rg '\bretained_lifetime_identity\b' /agent/repos/omega/omega-rust` → 2 hits: definition and `borrow_and_reflection.rs:1445` (read `carried_region` 1442-1454); also used from `TypeRegionScope::at_lifetime` at 4064-4067 (read).
3. `Ok(None)` when `regions.is_empty()` (4147-4148) or declaration invalid (4165-4166); `Err` on slot/length/spelling mismatch (4159-4171, read).
4. In Reference matching, `at_lifetime` returning `None` from `Err` path becomes `return false` at 4315-4325 (read).

**Verdict: CLOSED** — `Ok(None)` is an explicit “no retained constraint at this slot”; hard mismatches reject via `Err`/`false`, and `carried_region` falls back to lexical lookup when `None` (1445-1454, read).

### What I did not read

- Full bodies of `validate_dll_import_shape`, `validate_binding_shape`, and `apply_declaration_sidecar_pruning` beyond cited spans.
- Full `checks_contracts_exits.rs` module around the `expression_has_inert_prefix` call.
- All trait/conformance callers of `type_reference_regions_match` beyond `traits.rs:2903`.
- Every caller of `layout_checking_targets` past `discover_layout_checking_targets` error paths.

SLICE COMPLETE
