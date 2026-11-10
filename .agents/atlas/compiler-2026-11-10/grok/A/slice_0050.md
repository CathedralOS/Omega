# slice_0050

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0381 | cs:6880 | REACHABLE | `None` origin with no source metadata makes every symbol toolchain-owned, unlocking `content_algebra_application`. |
| A0382 | ce:5365 | REACHABLE | `And`/`Or`/`CaseMembership` make `selected_binary_meaning` true with no builtin overload check. |
| A0383 | ce:7083-7123 | REACHABLE | Empty selection list satisfies `unselected_or_builtin_arithmetic`, so `+ - *` fold as wrapping polynomials. |
| A0384 | ce:7338-7408 | REACHABLE | `value_preserving_integer_cast` omits width/range proof; `normalize` keeps the operand. |
| A0385 | ce:8120-8126 | REACHABLE | `static_type_requirement_entailed` can decide a call `requires` with no value judgment. |
| A0386 | ce:12080-12097 | REACHABLE | Matching `source_constructor` yields all `Proven` construction predicates without judging. |
| A0387 | ce:12610 | REACHABLE | `data_predicates_admit_zero` accepts via `data_fact_holds_at_zero` when the judge is not `Proven`. |
| A0388 | ce:15229-15239 | REACHABLE | Structural `ensures` skip `entailed` when `law_checked`; `check_law_conformance` is not on this path. |

## A0381

Digest [092] row 57; preamble area_13 (Toolchain declarations; cs file).

Opened `compiler_owned_symbol` at `omega-rust/psi/compilation/validation/src/completed_storage_and_conservation.rs:6871-6882` (read). On `symbol_source_origin(authored) == None`, the function returns `!program.symbols.has_source_metadata()` (`6880:6880`, read). `has_source_metadata` is `self.sources.is_some()` at `omega-rust/psi/foundation/symbols/src/table.rs:1716-1717` (read).

Search `rg '\bcompiler_owned_symbol\b' omega-rust` → **3** matches (read): definition `cs:6871`; production call `content_algebra_application` `cs:6527`; production call `is_content_projection_trait` `cs:6845`. No other production sites.

`content_algebra_application` (`cs:6514-6547`, read): if `!compiler_owned_symbol(program, *base_symbol)` at `6527`, returns `None`; otherwise user-looking generics named `IntervalSet` / `CountedQuantity` can yield a `ContentAlgebraApplication`.

**Input that reaches the branch:** A checking input whose symbol table has `sources == None` (no source metadata), so `has_source_metadata()` is false, and a user-declared symbol whose `symbol_source_origin` is `None`. That symbol is treated as compiler-owned at `6880`, so `content_algebra_application` proceeds instead of returning `None` at `6527`.

**Acceptance:** Validation uses content-algebra recognition (`cs:6527`) during checking; misclassified toolchain ownership accepts a program that should not get toolchain algebra treatment.

## A0382

Digest [093] row 58.

Read `selected_binary_meaning` in full `omega-rust/psi/compilation/validation/src/contract_entailment.rs:5314-5375` (read). For non-arithmetic operators, `spelled_binary_operator(binary.operator)` at `5361-5363`; on `None`, **`return true` at `5365`** without calling `has_builtin_spelled_expression_meaning`.

`spelled_binary_operator` at `omega-rust/psi/compilation/validation/src/proof_contracts_core.rs:5965-5986` (read): `BinaryOperator::CaseMembership | And | Or | … => None` at `5978-5985`.

Search `rg '\bselected_binary_meaning\b' omega-rust` → **11** matches, all in `contract_entailment.rs` (read). Production uses include `install_dispatch_selection` / condition decomposition `ce:5442-5449` (read): guard `if self.selected_binary_meaning(expression, &binary)` decomposes `And`/`Or` without a prior builtin overload check.

**Input:** A proof or dispatch expression whose binary node is `And`, `Or`, or `CaseMembership` where `has_builtin_spelled_expression_meaning` would fail if required, but `selected_binary_meaning` still returns true at `5365`.

**Acceptance:** Downstream paths treating “selected meaning” as established (e.g. collecting comparisons at `5443`) run on an expression that should not count as resolved.

## A0383

Digest [093] row 59; predicate cross-check [094] `ce:29175-29187`.

Read `Engine::normalize` arm `ce:7083-7123` (read) and `unselected_or_builtin_arithmetic` `ce:29167-29187` (read). Predicate is `.authored_selection_occurrences(expression).all(|…| … BuiltinOperator …)`; empty iterator makes `.all` **true** (`29175-29187`, read).

Search `rg '\bunselected_or_builtin_arithmetic\b' omega-rust` → **4** matches in `contract_entailment.rs` (read): definition `29167`; uses `7091`, `29068`; re-export `29308`.

When `!selected_binary_meaning` for `Add`/`Subtract`/`Multiply` and the predicate holds, the engine builds a polynomial with `ArithmeticDomain::Wrapping` at `7118-7121` (read).

**Input:** An integer `+`, `-`, or `*` expression with **no** authored declaration-selection occurrences on that expression, failing the “selected builtin arithmetic” path on `selected_binary_meaning`’s first branch but entering `7083-7123`.

**Acceptance:** Polynomial normalization treats the operation as wrapping/builtin-folded in the proof engine when selection or domain policy should block that fold.

## A0384

Digest [093] row 60.

Read `normalize` cast arms `ce:7331-7340` and `value_preserving_integer_cast` `ce:7393-7408` (read). Cast acceptance requires target fixed integer and source either integer literal or fixed-integer embedding (`7400-7407`); **no** comparison of source and target widths.

Search `rg '\bvalue_preserving_integer_cast\b' omega-rust` → **2** matches (definition + single use in `normalize`, read).

**Input:** A cast to a narrower fixed integer type from a fixed-integer-typed subexpression whose value is not provably in range (e.g. wider binding cast to `u8`).

**Acceptance:** `normalize` returns `self.normalize(cast.value)` at `7338-7339`, so bound/entailment reasoning can treat the cast as value-preserving without a range proof.

## A0385

Digest [093] row 61; implementation outside unit in `typed-trees`.

Read `prepare_selected_value` `ce:8037-8127` (read): at `8120-8126`, `program.static_type_requirement_entailed(expression, type_context, &application.types)` → `PreparedRequirement::Decided(true)`.

Read `RequirementWalk` loop `ce:8774-8779` (read): same call causes `continue` without `instantiated_selected_fact_judgment`.

Read `static_type_requirement_entailed` / `static_type_requirement_entailed_in_worlds` `omega-rust/psi/compilation/representations/typed-trees/src/type_system.rs:1577-1679` (read): returns `true` when `condition_inner` is empty for all type worlds after predicate-shape checks; no runtime value judgment.

Search `rg '\bstatic_type_requirement_entailed\b' omega-rust` → **6** matches (read): `type_system.rs` defs `1577`, `1589`, `1612`, `1619`; validation `ce:8120`, `ce:8774`; plus `machine_parameters_and_bindings.rs:1898` (not opened).

Upstream: `selected_value_call_requirement_entailed` `ce:8011-8033` (read) maps `Decided(true)` to acceptance.

**Input:** A call-site `requires` predicate statically entailed in the type world at `type_context` but not true at runtime.

**Acceptance:** Call requirement checking returns true / skips structural proof (`8126`, `8779`).

## A0386

Digest [093] row 62.

Read `construction_predicate_judgments` `ce:12067-12097` (read). Early return: if `prepared_mathematics().source_constructor(expression)` matches machine/state/schema/predicate list (`12080-12094`), returns `vec![StructuralJudgment::Proven; predicates.len()]` at `12096` with **no** `instantiated_selected_fact_judgment`.

Search `rg '\bconstruction_predicate_judgments\b' omega-rust` → **4** matches (read): def `12067`; `use` `13157`; calls `literals_and_struct_literals.rs:3595`, `5123` (opened `3594-3613`, read).

**Input:** A struct literal whose expression handle has a matching prepared `source_constructor` record listing all attached predicates, while field values would refute those predicates under structural judging.

**Acceptance:** Construction gate sees `StructuralJudgment::Proven` for every predicate (`3605-3613` path) without evaluating predicates against operands.

## A0387

Digest [093] row 63.

Read `data_predicates_admit_zero` `ce:12532-12615` (read). Each `where` fact: accept if `instantiated_selected_fact_judgment` is `Proven` **or** `data_fact_holds_at_zero` at `12609-12614`.

Read `data_fact_holds_at_zero` `omega-rust/psi/compilation/validation/src/domains_and_predicates.rs:7095-7104` (read): unfolds with `zero_knowledge` (`7100-7101`); true when `unfolding.decided() == Some(true)`.

Search `rg '\bdata_fact_holds_at_zero\b' omega-rust` → **4** matches (read): `domains_and_predicates.rs:7095`; call `ce:12610`; build-time duplicate `const_evaluation.rs` (not traced to validation acceptance).

Search `rg '\bdata_predicates_admit_zero\b' omega-rust` → **6** matches (read): gate in `value_custody_core.rs:4126`, `4147`; pipeline `preparation.rs:344` (not fully opened).

**Input:** A `zero_gated` data definition whose `where` fact is `Unknown`/`Refuted` under `StructuralJudge` at zero bindings but `data_fact_unfolding` + `zero_knowledge` yields `decided() == Some(true)`.

**Acceptance:** `data_predicates_admit_zero` returns true; `value_custody_core` zero gate allows the type (`4126`, inferred from grep + partial read).

## A0388

Digest [093] row 64 (entry names `check_law_conformance` callers; site is `check_contract_refinement`).

Read `check_contract_refinement` loop `ce:15225-15246` (read). For each required `ensures` conjunct with `fact_has_source_builtin_index_meaning`, skip error (`continue` `15241`) when any of: `owes_through_edge` (`15229`); `boundary_qualification_authorization` (`15230-15236`); `conclusion.law_checked` (`15238`); `entailed(...)` (`15239`).

`law_checked` set in `ContractFacts::collect` `ce:15442-15459` (read): `is_law_conjunct` for expression facts (`15665-15688`, read) — structural equality of two structural terms marks law, not semantic entailment.

Search `rg '\bcheck_contract_refinement\b' omega-rust` → **2** matches (read): def `15122`; production caller `traits.rs:2523`.

Search `rg '\bcheck_law_conformance\b' omega-rust` → **7** matches (read): `traits.rs:1552`, `6614`; `quotients.rs:179`; defs/uses in `ce` — **not** called from `validate_applied_trait_signature` before/after `2523` (read `traits.rs:2183-2532`).

**Input:** A machine realizing a trait requirement whose member `ensures` includes a structural equation conjunct that does **not** entitle the requirement’s matching `ensures`, while `law_checked` is true on the requirement conjunct (structural-term equality shape) and one of `owes_through_edge` / boundary authorization applies.

**Acceptance:** `check_contract_refinement` emits no `does not guarantee its ensures` diagnostic (`15243-15246` skipped) on the `validate_applied_trait_signature` path (`traits.rs:2523`) without `check_law_conformance` on that same path.

## What I did not read

- Full bodies of `condition_inner`, `data_fact_unfolding`, `collect_equality_conjuncts`, and `check_law_conformance` past sketched portions (`ce:19110+`, `domains_and_predicates.rs` unfolding helpers).
- `machine_parameters_and_bindings.rs:1898` `static_type_requirement_entailed` call site.
- `build-time-evaluation` `data_fact_holds_at_zero` and whether it affects shipped validation acceptance.
- Corpus/test harnesses that set `has_source_metadata()` false (digest [092] notes synthetic/test reachability).
- Remaining eight call sites of `selected_binary_meaning` beyond `5443` and `7084`.
- Commit `457adf4a11f` pin re-show; line numbers matched current checkout on read.

SLICE COMPLETE

