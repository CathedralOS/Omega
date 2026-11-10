# slice_0070

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0537 | omega-rust/psi/compilation/validation/src/quotients.rs:5551-5556 | REACHABLE | Static type argument with no `data_definition_by_symbol` row passes when a single path segment spells a built-in primitive name. |
| A0538 | omega-rust/psi/compilation/validation/src/quotients.rs:7540-7549 | REACHABLE | `quotient_for_type` miss maps to `ExactRelationLookup::NotQuotient`, then positional planning uses exact equality instead of quotient relation. |
| A0539 | omega-rust/psi/compilation/validation/src/quotients.rs:424-426 | CLOSED | `symbol_toolchain_relative_path` is `None` for non-toolchain origins, so path `"relation.omg"` cannot be satisfied by authored symbols. |
| A0540 | omega-rust/psi/compilation/validation/src/recasts.rs:1483-1490 | UNDECIDED | Defer pass skips recast diagnostics when layout is pending; no in-pipeline re-invocation of `judge_scalar_recast` with defer off was traced. |
| A0541 | omega-rust/psi/compilation/validation/src/recasts.rs:1795-1800 | UNDECIDED | Pending-record recast formation returns without refusal here; not every consumption site was traced to `report_consumption`. |
| A0542 | omega-rust/psi/compilation/validation/src/recasts.rs:757-758 | REACHABLE | `pending_origins` drops recasts whose issuing machine differs from the consuming machine, so cross-machine pending views accrue no obligation. |
| A0543 | omega-rust/psi/compilation/validation/src/recasts.rs:1902-1907 | REACHABLE | Scalar recast with missing `scalar_byte_size` on source or target exits `judge_scalar_recast_alternative` with no diagnostic. |
| A0544 | omega-rust/psi/compilation/validation/src/recasts.rs:2307-2321 | REACHABLE | Unspelled reference pun check returns silently unless both initializer and referee resolve to exact scalar representation types. |

## A0537

**Cited site (read):** `quotients.rs:5548-5559` in `static_application::validate_closed_type_argument` (`omega-rust/psi/compilation/validation/src/quotients.rs`). Lines match the atlas cite at this checkout.

When `data_definition_by_symbol(program, argument.symbol)` is `None`, the `else` arm (`:5550-5559`) accepts the argument if `application` is `None`, `path.len() == 1`, and `PrimitiveType::from_name(argument.path[0].as_str()).is_some()` (`:5551-5555`); otherwise it errors with `RepresentativeStaticArgumentCategoryMismatch` (`:5557-5558`).

**Enclosing function (read):** `validate_static_application` (`:5440-5479`) calls `validate_closed_type_argument` for each `TypeParameterKind::Type` static argument (`:5467-5470`).

**Callers (read):** `rg '\bvalidate_closed_type_argument\b' omega-rust` → 2 matches, both in `quotients.rs` (definition `:5537`, call `:5469`). `rg '\bvalidate_static_application\b' omega-rust` → 5 matches in `quotients.rs` (`:5432`, `:5440`, `:5502`, `:5561`, `:5622`).

**Input that reaches the branch (read):** A quotient representative or theorem static machine argument that names a primitive type by one-segment path (for example a spelling recognized by `PrimitiveType::from_name`) without a resolved data-definition symbol row.

**Verdict:** REACHABLE — such an argument is accepted as a closed type binding even though no data definition backs the symbol.

## A0538

**Cited site (read):** `quotients.rs:7536-7563` in `relation_plan::exact_quotient_relation`. Cite range `7540-7549` matches.

`quotient_for_type(program, quotient_type)` (`equivalence_selection::quotient_for_type`, `quotients.rs:505-514`) returns `None` when the base data symbol is missing or no `data_definitions()` row has that symbol with `quotient.is_some()`. `exact_quotient_relation` then returns `ExactRelationLookup::NotQuotient` (`:7545-7546`).

**Downstream (read):** `derive_relation_and_representative` (`:7293+`) maps `NotQuotient` to `PositionalRelation::ExactEquality(argument_type)` for each argument (`:7351-7354`) and for the return type (`:7363-7366`).

**Callers (read):** `rg '\bexact_quotient_relation\b' omega-rust` → 3 matches in `quotients.rs` (definition `:7536`, uses `:7351`, `:7363`). `rg '\bderive_relation_and_representative\b' omega-rust` → 3 matches in `quotients.rs`.

**Input (read):** A `Quotient::lift` / `define` call whose argument or result type names ordinary data (no quotient metadata in `data_definitions()`) is planned with exact equality rather than `PositionalRelation::Quotient`.

**Verdict:** REACHABLE — types absent from the quotient registry are treated like non-quotient types in relation planning.

## A0539

**Cited site (read):** `quotients.rs:416-450` in `equivalence_selection::is_sealed_core_equivalence`. `CORE_EQUIVALENCE_SOURCE` is `"relation.omg"` (`quotients.rs:8046`). Path check at `:424-426`.

**Authority gate (read):** `validate_equivalence_selection` (`:17-51`) rejects when `!is_sealed_core_equivalence(...)` (`:46-51`).

**Path source (read):** `FrozenTypedTrees::symbol_toolchain_relative_path` (`omega-rust/psi/compilation/representations/typed-trees/src/lib.rs:1578-1587`) returns `None` when `file.origin != SourceOrigin::Toolchain` (`:1584-1585`). Then `source_matches` at `quotients.rs:424-426` is false.

**Callers (read):** `rg '\bis_sealed_core_equivalence\b' omega-rust` → 2 matches in `quotients.rs` (`:46`, `:416`). `rg '\bvalidate_equivalence_selection\b' omega-rust` → 3 matches in `quotients.rs` (`:17`, import `:523`, call `:603`).

**Verdict:** CLOSED — a non-toolchain `Equivalence` trait cannot satisfy the `"relation.omg"` relative-path test; selection fails closed at `:46-51`.

## A0540

**Cited site (read):** `recasts.rs:1483-1490` in `recast_judgments::judge_scalar_recast` (`:1448+`). After `form_scalar_recast` succeeds, if `program.defers_pending_layout_geometry()` and `pending_layout_geometry(program, roots)` (`frozen_program_and_queries.rs:3445+`), the function returns without pushing diagnostics.

**Defer flag (read):** `check_program` builds `CheckingContext` with `.with_pending_layout_geometry(admits_pending_inputs || !deferred_layout_applications.is_empty())` (`omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/checking.rs:1032-1036`).

**Callers (read):** `rg '\bjudge_scalar_recast\b' omega-rust` → 8 matches, all in `recasts.rs` (definition `:1448`; calls at `:67`, `:4465`, `:5324`, `:5376`, plus module re-exports).

**Re-run (read, not traced to completion):** `check_program` runs validation once per `lower_typed_trees_reusing` invocation. `deferred_layout_applications` is stored on checked facts (`checking.rs:1099`) and blocks Terminal lowering when non-empty (`machine_lowering.rs:1681-1684`). No automatic second validation pass that re-calls `judge_scalar_recast` with `defers_pending_layout_geometry() == false` was found inside the same `check_program` call.

**Verdict:** UNDECIDED — the defer branch clearly skips recast refusal during pending-layout checking; whether every accepted program is eventually re-checked under strict layout was not established.

## A0541

**Cited site (read):** `recasts.rs:1795-1800` in `judge_scalar_recast_alternative` (`:1742+`). When `exact_scalar_representation_type` is `None` for the cast target, `qualification_compatible.is_none()`, and `pending_views::pending_view(...)` is `Some`, the function returns with no diagnostic.

**Consumption hooks (read):** `pending_views::validate_operand_consumptions` (`recasts.rs:913-965`) calls `report_consumption` for qualifying operand sites (`:953-962`). Invoked from local-reference validation in the same module (`:5351-5357`). `report_consumption` (`:809-883`) uses `pending_origins` and may error on unestablished predicates (`:874-879`).

**Other consumers (read):** `rg '\breport_consumption\b' omega-rust` → 7 matches (`recasts.rs:2`, `program_validation.rs:3`, `literals_and_struct_literals.rs:1`, `calls.rs:1`).

**Verdict:** UNDECIDED — formation deferral at `:1795-1800` is real; whether every way a pending view value can flow to use is covered by `report_consumption` (or sibling checks) was not exhaustively traced.

## A0542

**Cited site (read):** `recasts.rs:731-769` in `pending_views::pending_origins`. For each completed recast source, if `owner_machine.symbol != machine.symbol` the loop `continue`s (`:757-758`).

**Callers (read):** `rg '\bpending_origins\b' omega-rust` → 4 matches in `recasts.rs` (`:731` def; uses `:803`, `:830`, `:904`).

**Downstream (read):** `withholds_default_predicate` (`:772-805`) treats empty origins as not withholding (`:803-805`). `report_consumption` with empty `views` returns `false` without diagnostic (`:840-841`).

**Input (read):** A reference in machine M that borrows a pending-record recast formed in a different machine’s body; obligation collection skips the foreign issuer.

**Verdict:** REACHABLE — cross-machine pending views are not charged in `pending_origins`.

## A0543

**Cited site (read):** `recasts.rs:1902-1907` in `judge_scalar_recast_alternative` after both source and target passed `exact_scalar_representation_type` (`:1877-1878`, `:1889-1900`). If either `scalar_byte_size()` is `None`, the function returns silently.

**Callers (read):** Invoked only from `judge_scalar_recast` (`:1499-1508`) inside the layout-alternative loop.

**Input (read):** A scalar recast where representation metadata exists but byte size is not fixed (`scalar_byte_size()` returns `None` for source or target).

**Verdict:** REACHABLE — size disagreement and other downstream checks are skipped with no diagnostic.

## A0544

**Cited site (read):** `recasts.rs:2298-2337` in `recast_judgments::report_unspelled_reference_pun`. Early `return` at `:2313-2314` and `:2319-2320` unless both declared place type and referee unwrap to `exact_scalar_representation_type`. Only then scalar inequality emits an error (`:2322-2335`).

**Callers (read):** `rg '\breport_unspelled_reference_pun\b' omega-rust` → 3 matches in `recasts.rs` (definition `:2298`, re-export `:5289`, call `:5339-5346` on non-cast reference initializers).

**Input (read):** A `let` binding with a reference initializer that is not a spelled `&x as &T` recast, where source or `&T` referee is not an exact scalar primitive type (for example record or pending view types).

**Verdict:** REACHABLE — non-scalar reference shape puns are not reported by this helper.

## What I did not read

- Full fan-out of `validate_static_application` / quotient relation planning into emitted diagnostics beyond `derive_relation_and_representative`.
- Complete `symbol_toolchain_relative_path` / `SourceOrigin` assignment for every package kind beyond `typed-trees/src/lib.rs:1578-1587`.
- End-to-end pending-layout workflow (re-preparation, second `lower_typed_trees_reusing`, strict admission) after `deferred_layout_applications` is populated.
- Every `report_consumption` call site in `program_validation.rs`, `calls.rs`, and `literals_and_struct_literals.rs` for pending-view operands.
- Which scalar representation kinds yield `scalar_byte_size() == None`.
- Corpus programs exercising cross-machine pending recasts or unspelled non-scalar reference puns.

SLICE COMPLETE
