# slice_0060

Area digest preamble (read): Area 14 is the validation crate’s second half (entailment tail through terminal-installation); file abbreviations map `lib` → `omega-rust/psi/compilation/validation/src/lib.rs`, `fz` → `frozen_program_and_queries.rs`, etc. (`digests/area_14.md:1-23`). Rows here are the **lib, fz, fl, etf (26)** accept cluster (`digests/area_14.md:217-229`). Citations pinned at `457adf4a11f`; current checkout lines matched on read for each cited range.

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0457 | lib.rs:1056-1098 | REACHABLE | `None` from chained `.ok()?` skips zero-fill for unused native-locator coordinates during constructor typing in stage 04. |
| A0458 | fz:1751-1754 | REACHABLE | Build projection retains every symbol with no provenance span, not only build-scoped declarations. |
| A0459 | fz:2090-2091 | REACHABLE | Non-materialized descriptor selections stay in the skip set, so checked selection authority can be waived. |
| A0460 | fz:2450-2452 | REACHABLE | Invalid expression handles in the inert-prefix walk count as complete, so effectful match patterns can read as inert. |
| A0461 | fz:3334-3336 | REACHABLE | `layout_family_is_complete` is vacuously `true` when realizations are not `TargetInputs`, bypassing roster checks. |
| A0462 | fz:4458 | REACHABLE | Unpaired type-reference shapes fall through `_ => true` and region matching succeeds without structure pairing. |
| A0463 | fz:4537-4539 | REACHABLE | A required domain obligation with no actual candidate is skipped under callable refinement. |
| A0464 | fz:4147-4149, 4165-4167 | REACHABLE | `Ok(None)` from empty or invalid lifetime slots treats the slot as unconstrained in region scope entry. |

## A0457

**Cited site (read):** `omega-rust/psi/compilation/validation/src/lib.rs:1056-1098` — `unused_coordinate_widths` uses `.ok()?` and early `None` on any `exact_vocabulary`, `exact_width_parameters`, `validate_dll_import_shape`, `validate_binding_shape`, or ordinal lookup failure (e.g. `1056-1058`, `1073-1091`, `1093-1098`).

**Enclosing function (read):** `unused_coordinate_widths` (`lib.rs:1051-1099`) returns `Option<[bool; 3]>` only on full success; otherwise `None`.

**Callers (search):** pattern `\bunused_coordinate_widths\b` in `omega-rust` — 2 lines in `lib.rs` (definition `1051`, re-export `1432`), 2 in `constructed_results.rs` (import `16`, call `945`); count 4 token occurrences, 1 production call site.

**Upward trace (read):** `retain_constructor_result_type` (`constructed_results.rs:935-970`) sets `defaults` from `unused_coordinate_widths` (`945`); only when `defaults` is `Some` and any width unused does it synthesize a `"0"` named type for zero-fill (`947-960`). On `None`, `zero` stays `TypeReferenceHandle::invalid()` (`959-960`) and constructor result typing proceeds without coordinate zero defaults. Caller `806` (read) invokes this during stage-04 constructed-result retention. Separate path `vocabulary` (`lib.rs:973-1007`, read) emits diagnostics for binding vocabulary, but this caller does not invoke it when `unused_coordinate_widths` returns `None`.

**Input that reaches the branch:** A struct literal typed as a native-locator case where DllImport/ForeignBinding shape resolution would fail internally, while stage 04 still needs unused-coordinate defaults to complete the constructor result type.

**Smallest program (plain words):** A native-locator struct literal for a case that should demand implicit zero fields in unused width slots, while internal vocabulary or import-shape lookup fails silently; stage 04 keeps typing the constructor without injecting zeros and the program is not rejected on that path.

## A0458

**Cited site (read):** `frozen_program_and_queries.rs:1748-1755` — second predicate to `project_declarations` returns true when `is_generated(symbol)`, or `in_build_context(symbol)`, or `symbol_provenance_source_span(symbol).is_none()` (`1751-1754`).

**Enclosing function (read):** `project_build_program` (`fz:1683-1768`) builds `selected`, `is_build_scope`, and calls `project_declarations` with the retention closure (`1735-1758`).

**Callers (search):** pattern `\bproject_build_program\b` in `omega-rust` — definition `fz:1683`, production calls `omega/build-system/build-settings/src/admission/checking.rs:78`, `:94`, `:171` (3 call sites); count 4 matching lines.

**Upward trace (read):** `admission/checking.rs:78-82` projects the build slice before build-time evaluation and checking admission; projected `TypedTrees` feed `psi_compiler::CheckRequest` (`checking.rs:179-180`, read from prior chunk). Extra retained symbols enlarge the checked build program.

**Input that reaches the branch:** A symbol that should be pruned from the build slice but has no provenance source span (and is not classified generated or build-context file scope).

**Smallest program (plain words):** A build entry whose dependency closure includes a declaration handle with no provenance span; projection retains it and later validation runs against an oversized build program that should have been rejected as out-of-context.

## A0459

**Cited site (read):** `fz:2090-2091` — `materialized_product_operand_occurrences(program)` computed, then `occurrences.retain(|o| !materialized.contains(o))` so only non-materialized authored-selection occurrences remain in the returned set.

**Enclosing function (read):** `descriptor_selection_occurrences` (`fz:2079-2093`) unions product-request operands with failing `checks_authored_selection` rows, then subtracts materialized ones.

**Callers (search):** pattern `\bdescriptor_selection_occurrences\b` — definition `fz:2079`, `declarations_core.rs:957`, `pipeline/05.../shared.rs:1384`; 3 production call sites.

**Upward trace (read):** `checked_selection_requirements` (`declarations_core.rs:953-985`) builds `skipped` from this function (`957`); sets `excluded = skipped.contains(&selection.occurrence_id()) || selected_is_pruned` (`974`). Excluded selections produce no package-dependency error in the earlier validator loop (`921-948`, read). Stage 05 `shared.rs:1384-1388` also treats descriptor occurrences as already handled for late-bound selection diagnostics.

**Input that reaches the branch:** An authored declaration selection occurrence that is not in the materialized operand set (build/product operand not materialized for that occurrence).

**Smallest program (plain words):** A package selects a foreign declaration in a build descriptor position that never becomes a materialized product operand; the occurrence is skipped and cross-package selection authority is not enforced for that site.

## A0460

**Cited site (read):** `fz:2450-2452` — in `expression_has_inert_prefix`’s inner `visit`, `if !expression.is_valid() { return true; }`.

**Enclosing function (read):** `expression_has_inert_prefix` (`fz:2432-2484`) requires `complete && prefix.found && prefix.stable`; `Call`/`Atomic` before the endpoint mark effects (`2461-2466`).

**Callers (search):** pattern `\bexpression_has_inert_prefix\b` — definition `fz:2432`, `places.rs:6419`, `checks_contracts_exits.rs:8451`, plus import at `7731`; 1 definition, 2 production calls.

**Upward trace (read):** `places.rs:6416-6423` — match literal-arm bindings push `(subject, value)` only when `expression_has_inert_prefix` is true (guards effectful patterns). `checks_contracts_exits.rs:8451-8457` filters matched integer arms the same way for collection-length / return stability analysis.

**Input that reaches the branch:** A match pattern or arm path containing an invalid expression handle before the endpoint, in a dispatch that should be treated as effectful.

**Smallest program (plain words):** An integer match arm whose pattern subtree contains a stale invalid expression handle before a call; the walk treats the subtree as complete and inert, allowing a storage read binding that should be blocked as effectful.

## A0461

**Cited site (read):** `fz:3334-3336` — `let PlanLaidRealizations::TargetInputs(span) = layout.realizations else { return true; }`.

**Enclosing function (read):** `layout_family_is_complete` (`fz:3330-3352`) only compares `target_inputs` roster when realizations are `TargetInputs`; otherwise returns `true` immediately.

**Callers (search):** pattern `\blayout_family_is_complete\b` — definition `fz:3330`, call `value_custody_core.rs:7329`, internal `fz:3410`; 3 matching lines, 2 production checks (plan validation and layout-target discovery).

**Upward trace (read):** `plan_laid::validate_plans` (`vcc:7313-7332`) pushes error only when `!layout_family_is_complete`; non-`TargetInputs` plans always pass this gate. `layout_checking_targets` (`fz:3409-3412`) returns `Err(())` on incomplete families only when a layout exists and check runs — early `true` skips roster proof.

**Input that reaches the branch:** A plan-laid layout whose `realizations` variant is not `TargetInputs` but whose target-family roster is incomplete or inconsistent.

**Smallest program (plain words):** A plan-laid data type whose layout realizations are not recorded as target inputs; validation treats the family as complete without enumerating targets and admits the layout plan.

## A0462

**Cited site (read):** `fz:4458` — in `type_reference_region_applications`, the shape `match` ends with `_ => true` when actual and required `TypeReferenceNode` kinds are not one of the paired arms above.

**Enclosing function (read):** `type_reference_region_applications` (`fz:4227-4460`) runs `domain_region_applications` first (`4277-4296`), then structural `match` with `region_child!` recursion for paired kinds.

**Callers (search):** pattern `\btype_reference_regions_match\b` — definition `fz:4188`, calls `traits.rs:2903`, `contract_entailment.rs:42600`, `borrow_and_reflection.rs:3049`, `3071`; 1 definition, 4 production call sites (plus internal recursion via `region_child!`).

**Upward trace (read):** `borrow_and_reflection.rs:3049-3068` and `:3071-3093` — failed match yields `Err(...)` region-identity errors; success allows callable region preservation. Unpaired kinds that hit `_ => true` return success without comparing referees or lifetime slots.

**Input that reaches the branch:** Callable type refinement where actual and required parameter (or return) references resolve to different top-level type-reference kinds (e.g. named vs reference) that are not handled by a paired arm.

**Smallest program (plain words):** Two callable types that should disagree on region structure but pair unrelated type-reference shapes; region matching takes the default arm, succeeds, and callable region validation accepts the mismatch.

## A0463

**Cited site (read):** `fz:4537-4539` — in `domain_region_applications`, `if !found && relationship == TypeRegionRelationship::CallableRefinement { continue; }`.

**Enclosing function (read):** `domain_region_applications` (`fz:4468-4546`) walks required (or actual) obligations, narrows `applications` when candidates match, and returns false when `applications` becomes empty (`4541-4543`).

**Callers (read):** Only `type_reference_region_applications` (`fz:4277-4284`) with `DomainCoverage::Required` and `TypeRegionRelationship::CallableRefinement`.

**Upward trace (read):** Same callable-region path as A0462 in `borrow_and_reflection.rs:3049-3068`; domain failures surface only if the function returns false later; skipping a missing required actual leaves prior `applications` unchanged.

**Input that reaches the branch:** A required domain constraint on the callable type with no matching actual domain candidate under erased-identity pairing, during callable refinement (not exact application).

**Smallest program (plain words):** A callee type that should carry a domain obligation the caller type lacks; with no corresponding candidate the loop continues, refinement does not fail on that obligation, and the callable pair is accepted.

## A0464

**Cited sites (read):** `fz:4147-4149` — `operand_lifetimes` empty ⇒ `Ok(None)`; `fz:4165-4167` — lifetime declaration handle invalid ⇒ `Ok(None)` (after slot/expected checks at `4159-4164`).

**Enclosing function (read):** `retained_lifetime_identity` (`fz:4139-4174`) returns `Err` only on slot count mismatch or spelling mismatch (`4159-4171`).

**Callers (search):** pattern `\bretained_lifetime_identity\b` — definition `fz:4139`, internal `at_lifetime` `fz:4064`, `borrow_and_reflection.rs:1445`; 2 production entry paths (scope helper and carried-region).

**Upward trace (read):** `TypeRegionScope::at_lifetime` (`fz:4056-4068`) — `Ok(None)` yields `None => self` (`4066-4067`), so scope does not enter the lifetime operand. `carried_region` (`borrow_and_reflection.rs:1445-1454`) — `None` from identity falls back to lexical region (`1452-1454`).

**Input that reaches the branch:** A type reference whose operand lifetime table is empty, or a lifetime slot whose declaration handle is invalid, while region matching still proceeds through `at_lifetime`.

**Smallest program (plain words):** A callable or reference type with a lifetime slot that lost its declaration binding; identity returns no constraint, scope entry is a no-op, and region checking accepts the call without enforcing that slot.

## What I did not read

- Full bodies of `materialized_product_operand_occurrences`, `product_request_operand_occurrences`, `project_declarations`, and `checks_authored_selection` (only slice-local control flow and call edges).
- Entire `contract_entailment.rs` and `traits.rs` call sites beyond line numbers for `type_reference_regions_match`.
- Pipeline/fact-build code that would prove whether invalid expression handles, missing provenance spans, or non-`TargetInputs` plan-laid rows arise in normal compilation versus corrupted tables only (would decide fixture-only vs production reachability).

SLICE COMPLETE
