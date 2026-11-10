# slice_0056

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0425 | omega-rust/psi/compilation/validation/src/domains_and_predicates.rs:1478 | REACHABLE | `None` from `constant_subtree_meaning` becomes `true`, so non-constant operands pass `folded_constant_is_builtin`. |
| A0426 | omega-rust/psi/compilation/validation/src/domains_and_predicates.rs:582-588 | REACHABLE | `And`/`Or`/bitwise/shift binaries return `true` without `has_builtin_spelled_expression_meaning` on operand types. |
| A0427 | omega-rust/psi/compilation/validation/src/domains_and_predicates.rs:683 | REACHABLE | Any non-recursive expression leaf gets `SubtreeMeaning::Valid(1)` and passes `has_builtin_bound_expression_meaning`. |
| A0428 | omega-rust/psi/compilation/validation/src/domains_and_predicates.rs:877-892 | REACHABLE | Empty `authored_selection_occurrences` makes `.all` succeed on sealed toolchain float comparisons. |
| A0429 | omega-rust/psi/compilation/validation/src/domains_and_predicates.rs:1035 | REACHABLE | Missing subject data type still yields `Some(selected)` for the right-hand case path after exact-membership checks. |
| A0430 | omega-rust/psi/compilation/validation/src/domains_and_predicates.rs:5043,6250-6253 | CLOSED | Collection-prover `Comparison` literals only use ordering/equality operators that always have `complement()`. |
| A0431 | omega-rust/psi/compilation/validation/src/domains_and_predicates.rs:6605-6611 | REACHABLE | Integer actual with no `landing` skips the primitive mismatch test and proceeds to range bounds. |
| A0432 | omega-rust/psi/compilation/validation/src/domains_and_predicates.rs:7502-7504 | REACHABLE | `handle_assignment` returns with no diagnostic when `call_frames` is `None`, skipping origin-based stores. |

## A0425

**Cited site (read):** `domains_and_predicates.rs:1473-1504` in module `bound_expression_meaning`. `folded_constant_has_builtin_meaning` calls `constant_subtree_meaning(...).unwrap_or(true)` at `domains_and_predicates.rs:1478`. Cite matches this checkout.

**Enclosing logic (read):** `constant_subtree_meaning` returns `Some(true)` only for `Integer`, `Borrow`, or `Binary` nodes whose children succeed; any other node kind hits `_ => None` at `domains_and_predicates.rs:1502-1503`. `folded_constant_is_builtin` (`domains_and_predicates.rs:1458-1470`) requires `builtin_arithmetic_node` on the outer binary and `folded_constant_has_builtin_meaning` on each operand separately.

**Callers (read):** `rg '\bfolded_constant_has_builtin_meaning\b' omega-rust` → 3 matches (`domains_and_predicates.rs:1473`, `:1464`, `arithmetic_domains.rs:24760`). `rg '\bfolded_constant_is_builtin\b' omega-rust` → 9 matches (`domains_and_predicates.rs` plus `arithmetic_domains.rs`).

**Upstream (read):** `abstract_specification_constant_has_builtin_meaning` (`arithmetic_domains.rs:24755-24764`) forwards to this helper. `narrow_abstract_specification_environment` (`arithmetic_domains.rs:25159-25167`) returns early only when both comparison operands fail that test; a `true` from `unwrap_or` lets bound narrowing continue. `exact_bound_binary` (`arithmetic_domains.rs:14323-14327`) and `builtin_arithmetic` (`domains_and_predicates.rs:1398-1410`) also consult `folded_constant_is_builtin` on operands.

**Input (plain words):** An exact-integer bound or abstract-spec comparison whose one side is a non-constant expression (for example a parameter or place read) while the other side looks like builtin arithmetic: the non-constant side is treated as having builtin constant meaning and the bound or narrowing path proceeds.

**Verdict:** REACHABLE.

## A0426

**Cited site (read):** `domains_and_predicates.rs:547-590` in `binary_expression_meaning`. The `BinaryOperator::And` through `ShiftRight` arm returns `true` at `domains_and_predicates.rs:582-588` without operand-type or spelling checks. Cite matches.

**Enclosing function (read):** `has_builtin_binary_expression_meaning` (`domains_and_predicates.rs:489-511`) memoizes and delegates to `binary_expression_meaning`. Early exit at `domains_and_predicates.rs:559-561` when `static_type_predicate_operands` is set returns `true` for a different reason.

**Callers (read):** `rg '\bhas_builtin_binary_expression_meaning\b' omega-rust` → 62+ matches across validation, pipeline `05_typed-trees-to-checked-trees`, build-time evaluation, and related crates (count mode).

**Upstream (read):** `exact_binary_fit_request` (`contract_entailment.rs:22301-22305`) returns `None` only when `has_builtin_binary_expression_meaning` is false; when true it may admit anonymous numeric operand carriers (`contract_entailment.rs:22315-22328`). `expression_types_and_fields.rs:1534-1551` runs `validate_builtin_comparison_operand_landing` only when this flag is true. Predicate exploration keeps ordering comparisons only if this flag is true (`domains_and_predicates.rs:2917-2930`).

**Input (plain words):** A binary `And`, `Or`, bitwise, or shift expression resolved to a user or trait operator for those operand types still counts as “builtin binary meaning,” so downstream validation that gates on this flag treats it like a toolchain operator.

**Verdict:** REACHABLE.

## A0427

**Cited site (read):** `domains_and_predicates.rs:599-701` in `bound_subtree_height`. Non-`Binary`/`Unary`/`Cast`/`Atomic` nodes fall through to `_ => SubtreeMeaning::Valid(1)` at `domains_and_predicates.rs:683`. Cite matches.

**Enclosing function (read):** `has_builtin_bound_expression_meaning` (`domains_and_predicates.rs:430-440`) is true exactly when `bound_subtree_height` is `SubtreeMeaning::Valid(_)`.

**Callers (read):** `rg '\bhas_builtin_bound_expression_meaning\b' omega-rust` → 62+ matches (validation, `arithmetic_domains.rs`, `places.rs`, pipeline, proof checker).

**Upstream (read):** For a `Binary` child, height computation first requires `has_builtin_binary_expression_meaning` (`domains_and_predicates.rs:626-628`). A single leaf such as a `Call` or `Name` never hits that guard and is `Valid(1)` at depth 0, so `has_builtin_bound_expression_meaning` is true (`places.rs:2381-2387` uses this to accept range bounds; `proof_contracts_core.rs:5309-5315` for bound leaf initializers; `contract_entailment.rs:35435` and related for goals).

**Input (plain words):** A slice or range bound, bound leaf initializer, or entailment goal that is a lone non-arithmetic expression (for example a call) is treated as a valid builtin-bound subtree of height one.

**Verdict:** REACHABLE.

## A0428

**Cited site (read):** `domains_and_predicates.rs:834-894` in `sealed_float_comparison_declaration_for_operands`. The closure at `domains_and_predicates.rs:877-892` requires every `authored_selection_occurrences(expression)` entry to satisfy operator selection targets; an empty iterator makes `.all` vacuously true. Cite matches.

**Enclosing function (read):** Full function read `domains_and_predicates.rs:834-894`. Preconditions include exactly one resolved candidate (`domains_and_predicates.rs:858-860`), boundary operator, matching `primitive_float_binary_semantics`, and `is_sealed_toolchain_float_operator` (`domains_and_predicates.rs:867-876`).

**Callers (read):** `rg '\bsealed_float_comparison_declaration_for_operands\b' omega-rust` → 5 matches (`domains_and_predicates.rs:824`, `:834`, `:772`, plus `machine_calls_core.rs`, `contract_entailment.rs`).

**Upstream (read):** `has_builtin_or_sealed_float_comparison_meaning` (`domains_and_predicates.rs:781-814`) ORs builtin spelling meaning with `sealed_float_comparison_for_operands`, which treats a `Some` declaration symbol as success (`domains_and_predicates.rs:807-814`, `824-831`).

**Input (plain words):** A sealed toolchain float comparison expression with no authored declaration-selection occurrences on that expression passes the selection-authority loop and is admitted as sealed float meaning.

**Verdict:** REACHABLE.

## A0429

**Cited site (read):** `domains_and_predicates.rs:1011-1036` in `selected_case_in_subject_namespace`. When `subject_type(subject)` is `None`, or peeling yields no `closed_data_definition`, control reaches `Some(selected)` at `domains_and_predicates.rs:1035` without calling `closed_data_constructor_case`. Cite matches.

**Enclosing callers (read):** `applied_case_membership_variant` (`domains_and_predicates.rs:970-983`) and `applied_declaration_case_membership_variant` (`domains_and_predicates.rs:985-1009`) call this only after `has_exact_case_membership_meaning` / `has_exact_declaration_case_membership_meaning` succeed.

**`exact_case_membership_meaning` (read):** `domains_and_predicates.rs:1073-1126` requires valid expressions, `exact_case_reference_owner` on the right, `subject_matches`, and paired authored selections for owner and case when operator is `Equal`.

**Callers (read):** `rg '\bselected_case_in_subject_namespace\b' omega-rust` → 3 matches (definition plus the two `applied_*` functions). `rg '\bhas_exact_case_membership_meaning\b' omega-rust` → 18 matches across validation, pipeline, and review evidence.

**Input (plain words):** A case-membership or equality guard whose subject expression has no resolved closed data type (or type not found by the supplied `subject_type` closure) still returns the right-hand name path symbol as the applied variant, after selection-shape checks passed.

**Verdict:** REACHABLE.

## A0430

**Cited sites (read):** `domains_and_predicates.rs:5031-5057` (`implies_clause`) uses `literal.operator.complement().unwrap_or(literal.operator)` at `domains_and_predicates.rs:5042-5046`. `domains_and_predicates.rs:6246-6257` (`clauses` on `Boolean::Comparison`) uses the same pattern when `holds` is false. Cités match.

**`BinaryOperator::complement` (read):** `typed-trees/src/values.rs:4779-4788` returns `Some` only for `Equal`/`NotEqual` and ordering operators; `_ => None` for all other operators.

**Where `Comparison` literals come from (read):** Internal `compare` builds `Comparison` values (`domains_and_predicates.rs:6088-6093`). Predicate exploration emits `Boolean::Comparison` only from `Candidate::Ordering` with operators already filtered to equality/ordering at `domains_and_predicates.rs:2917-2924` and `domains_and_predicates.rs:3238-3242`. Path initialization uses `GreaterOrEqual`/`LessOrEqual` compares (`domains_and_predicates.rs:4903-4911`, `:4935-4943`). `clauses` for `Boolean::Comparison` copies the same operator or its complement (`domains_and_predicates.rs:6246-6257`).

**Closing check (read):** Every `Comparison` literal that can reach `implies_clause` or the `holds == false` branch of `clauses` uses an operator for which `complement()` is `Some`, so `unwrap_or` never changes the operator on those paths.

**Digest note (read):** Area 14 digest records reader uncertainty (“not reachable from what I read”); code inspection shows unreachable non-complement operators on live `Comparison` paths.

**Verdict:** CLOSED.

## A0431

**Cited site (read):** `domains_and_predicates.rs:6575-6622` in `closed_integer_predicate_value`. In the `actual_type` is `None` branch, `integer_node(*actual)` is required; `literal.landing().is_some_and(|landing| …)` at `domains_and_predicates.rs:6606-6611` rejects only when landing exists and conflicts; absent landing skips the block. Cite matches.

**Enclosing function (read):** Full function read `domains_and_predicates.rs:6575-6622` through `unfold(...).decided()`.

**Callers (read):** `rg '\bclosed_integer_predicate_value\b' omega-rust` → 2 matches (`domains_and_predicates.rs:6575`, `type_references_and_wire.rs:412`).

**Upstream (read):** Closed predicate operand evaluation (`type_references_and_wire.rs:397-416`) maps integer actuals through this helper; failure becomes `"closed predicate call has no fully decided admitted integer application"`.

**Input (plain words):** A closed integer predicate call whose argument is an integer literal without a landed type carrier, but whose numeric value fits the declared bare integer parameter bounds, is decided successfully.

**Verdict:** REACHABLE.

## A0432

**Cited site (read):** `domains_and_predicates.rs:7355-7504` in `handle_assignment`. After `handle_element_store`, `let Some(frames) = call_frames else { return; }` at `domains_and_predicates.rs:7502-7504` exits without pushing diagnostics. Cite matches.

**Enclosing walk (read):** `walk_state` (`domains_and_predicates.rs:13326-13423`) invokes `handle_assignment` for every assignment (`domains_and_predicates.rs:13409-13423`).

**Callers (read):** `rg '\bwalk_state\b' omega-rust/psi/compilation/validation/src/domains_and_predicates.rs` → 5 matches (definition plus `domains_and_predicates.rs:12627`, `:12660`, `:12775`, `:13245`). `analyze_default_domain_writes_with_frames` (`domains_and_predicates.rs:12595-12600`) threads `call_frames: Option<_>` through those walks.

**Upstream (read):** `validate_default_domain_writes` (`domains_and_predicates.rs:12582-12593`) is called from `judge_contract_entailment` (`program_validation.rs:4672-4679`) with `calls::CallFrameResolver::new(program).as_ref()`. `CallFrameResolver::new` (`calls.rs:15337-15348`) returns `None` when `CallFrameTables::build` fails (`calls.rs:14075-14078`).

**Input (plain words):** Default-domain write analysis running without a call-frame resolver (resolver build failed or absent) still walks assignments but skips assignment-origin invariant checking for field stores that depend on `assignment_storage_origins`.

**Verdict:** REACHABLE.

## What I did not read

- Full bodies of every `has_builtin_binary_expression_meaning` consumer beyond the samples cited.
- Whether `CallFrameTables::build` failure always produces diagnostics that reject the program before entailment completes (A0432).
- Runtime behavior of `applied_case_membership_variant` consumers outside validation grep hits (A0429).
- `git show 457adf4a11f` paths for line drift (all cites matched at this checkout).

SLICE COMPLETE
