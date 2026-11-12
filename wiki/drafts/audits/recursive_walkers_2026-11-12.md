# Recursive walker inventory (Psi stages 01–06)

Curated, hand-verified inventory at `0835b3aaf803ed97f905be42d94727ff58459263`
for [expression-tree-depth-provision](OWNER_QUESTIONS.md). Lists only walkers
whose **Rust stack recursion** scales with **source** expression/type/pattern/
statement nesting or operator/projection chain length. Stage **05** spans
`typed-trees-to-checked-trees` and `validation`; stages **07–08** are out of scope.

## Shared flat-table groups (one table retires the whole group)

### `G-syntax-expr` — Syntax expression tables

- **Flat representation:** `SyntaxTrees.expressions` (arena `ExpressionTable`)
- **Stages:** 01–02
- **Walker IDs:** `W01-1`, `W01-6`, `W01-7`, `W02-1`, `W02-2`
- **Notes:** One indexed child-edge list per syntax expression node retires the mutual parse/lower expression SCCs.

### `G-syntax-types` — Syntax type references

- **Flat representation:** `SyntaxTrees.types` / `TypeReferenceTable`
- **Stages:** 01
- **Walker IDs:** `W01-2`, `W01-5`
- **Notes:** Shared with pattern/type grammars that call `parse_type_reference_handle`.

### `G-syntax-stmt` — Syntax statements and machines

- **Flat representation:** `SyntaxTrees` statement and transition tables
- **Stages:** 01–02
- **Walker IDs:** `W01-8`, `W01-9`, `W02-4`
- **Notes:** Machine bodies, transitions, and statement lowering share statement handles.

### `G-resolved-expr` — Symbol-resolved expression table

- **Flat representation:** `SymbolResolvedTrees` expression table (stage-02 output)
- **Stages:** 02
- **Walker IDs:** `W02-1`, `W02-2`, `W02-5`
- **Notes:** Const-eval and match lowering read the same resolved expression storage as the main lowering SCC.

### `G-typed-expr` — Typed expression table

- **Flat representation:** `TypedTrees.expressions` (`ExpressionTable`)
- **Stages:** 03–05
- **Walker IDs:** `W03-1`, `W04-1`, `W05-CORE`, `W05-P01`, `W05-P02`, `W05-V01`, `W05-V02`, `W05-V03`, `W05-V04`, `W05-V08`
- **Notes:** `validation::queries::expression_traversal` is the shared child dispatcher; most stage-05 walkers call into it or duplicate its match.

### `G-typed-stmt` — Typed statement / machine bodies

- **Flat representation:** `TypedTrees.statements` (`StatementTable`)
- **Stages:** 03–05
- **Walker IDs:** `W03-2`, `W05-V02`, `W05-V05`, `W05-V06`, `W05-P03`
- **Notes:** Statement-indexed walks scale with state body length, not a fixed enum.

## Measured source depths

Lexically scanned **50111** `.omg` files under `tests/omega`,
`samples`, `source/library`, and `source/psi` for parenthesis tests and
prefix chains. **Type nesting** and **binary operator chains** use the
real Psi stage-01 parser (`tokens-to-syntax-trees`) over every parsed file
(see `wiki/drafts/audits/measure_depths/`). Expression **tree depth** is the
maximum parse-tree height (parenthesis proxy).

**Deliberate depth-limit fixtures** (excluded from “real program” column): `fail/parser/nesting_exceeds_max_depth`, `pass/parser/deep_nesting_within_limit`, `pass/expressions/deep_paren_nesting`. `pass/expressions/deep_paren_nesting` is annotated `// Deep parenthesis nesting within the compile path's recursion budget.`

| Dimension | All corpus (max) | Excluding depth fixtures (max) | Deepest real program (file:line) |
| --- | ---: | ---: | --- |
| Expression tree depth (parse) | 135 (`tests/omega/fail/build/installed_arrival_wrong_controller/main.omg`) | 135 (`tests/omega/fail/build/installed_arrival_wrong_controller/main.omg`) | `tests/omega/fail/build/installed_arrival_wrong_controller/main.omg`:1 |
| Type nesting (parse) | 6 (`tests/omega/fail/facts/open_index_distributivity_commutative_semiring_rejected/main.omg`) | 6 (`tests/omega/fail/facts/open_index_distributivity_commutative_semiring_rejected/main.omg`) | `tests/omega/fail/facts/open_index_distributivity_commutative_semiring_rejected/main.omg`:1 |
| Binary op chain (parse, left spine) | 132 (`tests/omega/fail/build/installed_arrival_wrong_controller/main.omg`:1585) | 132 (`tests/omega/fail/build/installed_arrival_wrong_controller/main.omg`:1585) | `tests/omega/fail/build/installed_arrival_wrong_controller/main.omg`:1585 |
| Lexical `(` depth (sanity / fixtures) | 1100 (`tests/omega/fail/parser/nesting_exceeds_max_depth/main.omg`) | 9 (`tests/omega/pass/terminal_psi/integer_control_contract/main.omg`) | `tests/omega/pass/terminal_psi/integer_control_contract/main.omg` (parser uses tree depth above) |
| Prefix `~` chain (lexical, in parens) | 1 | 1 | `tests/omega/fail/resources/owned_extent_erased_label_rejected/main.omg` |

Parser pass rate: **49755** parsed, **356** failures (some stress fixtures need larger host stack to parse).

**Stage-01 integration tests** (Rust, not `.omg`): 4096 unary `~` repetitions; 1024 seven-operator groups (binary chains use `parse_binary_chain_handle` with an explicit `pending` stack — no Rust recursion on chain length).

## Kept walker counts (verified)

| Stage | Kept walkers |
| --- | ---: |
| 01 — parsing | 10 |
| 02 — resolution | 9 |
| 03 — typing | 7 |
| 04 — typed elaboration | 3 |
| 05 — checking | 16 |
| 06 — lowering | 2 |
| **Total** | **47** |
| **Shared-tree groups** | **6** |

## 01 — parsing

| ID | Function(s) | Location | Walks | Depth driver | Existing flat table |
| --- | --- | --- | --- | --- | --- |
| `W01-1` | `parse_expression_handle_in`, `parse_or_expression_handle`, `parse_bitwise_or_expression_handle`, `parse_membership_expression_handle`, `parse_unary_expression_handle`, `parse_postfix_expression_handle`, `parse_primary_expression_handle`, `parse_index_or_range_expression_handle`, `parse_struct_literal_handle` | `omega-rust/psi/compilation/pipeline/01_tokens-to-syntax-trees/src/expressions.rs:407` | expressions | parenthesis nesting (`Input::deepen`); operator chains use `parse_binary_chain_handle` Vec stack | `SyntaxTrees.expressions` — inserts handles; child dispatch still recursive |
| `W01-2` | `parse_type_reference_handle`, `parse_type_reference_handle_inner`, `parse_named_type_reference`, `parse_borrow_type_reference`, `apply_in_domain_suffix` | `omega-rust/psi/compilation/pipeline/01_tokens-to-syntax-trees/src/type_syntax_and_patterns.rs:583` | types | generic/type bracket nesting via `Input::deepen` | `SyntaxTrees.types` / type reference table |
| `W01-3` | `parse_pattern`, `parse_match_expression_handle` | `omega-rust/psi/compilation/pipeline/01_tokens-to-syntax-trees/src/expressions.rs:111` | patterns | pattern and match-arm nesting | `SyntaxTrees.expressions` / pattern payloads in expression table |
| `W01-4` | `parse_item`, `parse_declared_item`, `parse_conformance` | `omega-rust/psi/compilation/pipeline/01_tokens-to-syntax-trees/src/declarations.rs:2305` | declarations | nested top-level declaration sequences | `SyntaxTrees` root/item tables |
| `W01-5` | `parse_mathematical_type`, `parse_mathematical_type_inner`, `parse_arrow_domain` | `omega-rust/psi/compilation/pipeline/01_tokens-to-syntax-trees/src/declarations.rs:1490` | types | mathematical type parenthesis nesting (`deepen`) | type reference table |
| `W01-6` | `rewrite_expression`, `rewrite_expression_span`, `rewrite_optional_expression` | `omega-rust/psi/compilation/pipeline/01_tokens-to-syntax-trees/src/type_syntax_and_patterns.rs:49` | expressions | expression nesting inside type/pattern positions | `SyntaxTrees.expressions` |
| `W01-7` | `is_place`, `copy_expression_as_place`, `copy_compound_assignment_target` | `omega-rust/psi/compilation/pipeline/01_tokens-to-syntax-trees/src/statements_and_assembly.rs:334` | expressions (places) | member/index path chain length | `SyntaxTrees.expressions` |
| `W01-8` | `expression_handle_identifier_depth`, `copy_expression_to_statement_identifier_path` | `omega-rust/psi/compilation/pipeline/01_tokens-to-syntax-trees/src/bodies_and_transitions.rs:1302` | expressions / statements | identifier path depth in machine bodies | expression + statement tables |
| `W01-9` | `parse_signature_clauses`, `parse_machine_parameter_contracts` | `omega-rust/psi/compilation/pipeline/01_tokens-to-syntax-trees/src/contracts_and_parameters.rs:1372` | contracts + types | nested contract clauses and type syntax | syntax declaration + type tables |
| `W01-10` | `try_parse_static_argument`, `parse_static_symbol_application` | `omega-rust/psi/compilation/pipeline/01_tokens-to-syntax-trees/src/expressions.rs:1649` | expressions | nested static argument / generic actual lists | `SyntaxTrees.expressions` |

## 02 — resolution

| ID | Function(s) | Location | Walks | Depth driver | Existing flat table |
| --- | --- | --- | --- | --- | --- |
| `W02-1` | `lower_expression_into_table`, `lower_expression_node_into_table`, `lower_nonbinary_expression_node_into_table`, `lower_binary_expression_into_table` | `omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/lowering.rs:1140` | expressions | expression tree depth | symbol-resolved `ExpressionTable` (flat inserts; recursion on handles) |
| `W02-2` | `lower_match`, `lower_match_pattern`, `lower_destructure_pattern` | `omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/lowering.rs:864` | expressions + patterns | match dispatch and pattern nesting | resolved expression + pattern tables |
| `W02-3` | `lower_child_type_references` | `omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/lowering.rs` | types | nested type references in syntax | resolved type reference table |
| `W02-4` | `lower_statement_node`, `lower_statement_into_pending`, `hoist_into_temp`, `rewrite_children` | `omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/lowering_statement.rs:1066` | statements | statement list and nested expression hoists | `SymbolResolvedTrees` statement table |
| `W02-5` | `expression_child_handles`, `expression`, `collect` | `omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/constant.rs:1550` | expressions | const-eval expression tree depth | syntax expression handles (read-only walk) |
| `W02-6` | `visit_expression_type_inputs` | `omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/preparation_generic_uses.rs:1015` | types in expressions | type application nesting in generic uses | syntax type references |
| `W02-7` | `lower_lexical_machine_into`, `lower_lexical_machine_parts` | `omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/lowering_machine.rs:1347` | machines / statements | nested machine body structure | syntax/resolution machine tables |
| `W02-8` | `expression_contains`, `subject_contains_hoistable` | `omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/lowering_statement.rs:765` | expressions | expression subtree queries for hoisting | syntax expressions |
| `W02-9` | `generic_parameter_roles`, `application_arguments` | `omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/preparation.rs:929` | types + expressions | nested generic applications in preparation | syntax/type tables |

## 03 — typing

| ID | Function(s) | Location | Walks | Depth driver | Existing flat table |
| --- | --- | --- | --- | --- | --- |
| `W03-1` | `lower_expression_handle`, `lower_expression_handle_from_table`, `lower_expression_node` | `omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/expression_lowering.rs:3891` | expressions | resolved → typed expression tree depth | `TypedTrees.expressions` |
| `W03-2` | `lower_statement`, `lower_statement_node` | `omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/statement_lowering.rs:1283` | statements | statement and transition nesting | `TypedTrees.statements` |
| `W03-3` | `lower_type_reference`, `lower_child_type_references` | `omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/type_reference.rs` | types | type reference nesting | `TypedTrees.types` |
| `W03-4` | `lower_expression_fact`, `lower_statement_fact` | `omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/contracts_and_signatures.rs:531` | expressions + statements | contract fact expression depth | typed expressions |
| `W03-5` | `lower_declaration`, `lower_machine` | `omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/declarations.rs` | declarations | nested declaration bodies | `TypedTrees` machine/declaration tables |
| `W03-6` | `lower_match_expression`, `lower_pattern` | `omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/expression_lowering.rs` | patterns + expressions | match/pattern nesting | typed expressions |
| `W03-7` | `lower_statement_path_members` | `omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/statement_lowering.rs:39` | expressions (paths) | path projection chain length | typed expressions |

## 04 — typed elaboration

| ID | Function(s) | Location | Walks | Depth driver | Existing flat table |
| --- | --- | --- | --- | --- | --- |
| `W04-1` | `visit_post_order`, `validation::expression_traversal::visit_post_order` | `omega-rust/psi/compilation/pipeline/04_typed-trees-to-typed-trees/src/lib.rs:227` | expressions | typed expression tree depth | `TypedTrees.expressions` via shared `expression_traversal` |
| `W04-2` | `clone_specialized_machine`, `rewrite_clone_transition_subjects` | `omega-rust/psi/compilation/pipeline/04_typed-trees-to-typed-trees/src/body_cloning_and_rewriting.rs:669` | machines / statements | specialized machine body size (statement count) | `TypedTrees` machines + statements (cloned tables) |
| `W04-3` | `visit_lifetime_bindings`, `visit_lifetime_bindings_in_scope` | `omega-rust/psi/compilation/pipeline/04_typed-trees-to-typed-trees/src/conformance.rs:801` | types | nested lifetime/type binder scopes | typed signature / type tables |

## 05 — checking

| ID | Function(s) | Location | Walks | Depth driver | Existing flat table |
| --- | --- | --- | --- | --- | --- |
| `W05-CORE` | `queries::expression_traversal::children`, `queries::expression_traversal::visit_post_order` | `omega-rust/psi/compilation/validation/src/frozen_program_and_queries.rs:2724` | expressions | expression tree depth (shared child dispatch) | `TypedTrees.expressions` — table already flat; walkers should loop edges |
| `W05-V01` | `visit_post_order::inner` | `omega-rust/psi/compilation/validation/src/frozen_program_and_queries.rs:2838` | expressions | expression tree depth | same as W05-CORE |
| `W05-V02` | `visit_expression`, `visit_statement`, `visit_transition_target` | `omega-rust/psi/compilation/validation/src/effect_inference.rs:179` | expressions + statements | expression / transition tree depth | `TypedTrees` expression + statement tables |
| `W05-V03` | `walk_expression` | `omega-rust/psi/compilation/validation/src/float_and_mathematical_contracts.rs:1496` | expressions | expression nesting | typed expressions |
| `W05-V04` | `visit_result_values_inner`, `visit_destination_trees` | `omega-rust/psi/compilation/validation/src/literals_and_struct_literals.rs:2183` | expressions + types | result-type / nested match expression depth | typed expressions + type references |
| `W05-V05` | `walk_state_write_prefix_inner`, `walk_state_write_prefix_body` | `omega-rust/psi/compilation/validation/src/calls.rs:27589` | statements / places | state body and write-prefix path depth | `TypedTrees.statements` |
| `W05-V06` | `walk_state` | `omega-rust/psi/compilation/validation/src/domains_and_predicates.rs:13525` | machine states | state body statement count (source machine size) | statement table indexed by state |
| `W05-V07` | `visit_proof_fact_expressions` | `omega-rust/psi/compilation/validation/src/declarations_core.rs:469` | expressions | proof fact expression nesting | typed expressions |
| `W05-V08` | `visit_type` | `omega-rust/psi/compilation/validation/src/value_custody_core.rs:2614` | types | type reference nesting | `TypedTrees.types` |
| `W05-P01` | `TerminationBodyVisitor::visit_expression` | `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/checks.rs:5585` | expressions | expression tree depth | typed expressions |
| `W05-P02` | `RangeFactsVisitor::visit_expression` | `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/checks_ranges.rs:4002` | expressions | expression tree depth | typed expressions |
| `W05-P03` | `visit_field_child`, `visit_owner`, `visit_array` | `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/flow.rs:11708` | places / ownership | projection path length (field/index chain) | checked flow place graph (partial) |
| `W05-P04` | `FlowCalls::visit_qualified_ensures` | `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/flow_calls.rs:909` | expressions | contract / expression nesting in ensures | typed expressions |
| `W05-P05` | `is_state_independent_borrow_source` | `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/checks_borrows.rs:1656` | expressions | expression tree depth for borrow-source queries | `TypedTrees.expressions` |
| `W05-P06` | `visit_statement` | `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/checks.rs:5526` | statements | statement tree in checking passes | typed statements |
| `W05-EXT` | `typed_trees::references::visit_program` | `omega-rust/psi/compilation/representations/typed-trees/src/references.rs:1726` | programs | declaration + expression tree depth | `TypedTrees` (representation crate, not stage src) |

## 06 — lowering

| ID | Function(s) | Location | Walks | Depth driver | Existing flat table |
| --- | --- | --- | --- | --- | --- |
| `W06-1` | `integer_predicates::lower` | `omega-rust/psi/compilation/pipeline/06_checked-trees-to-lowered-psi/src/proofs.rs:7763` | checked scalar expressions | checked scalar expression tree depth | `CheckedTrees` scalar expression IR (match-shaped, not yet edge list) |
| `W06-2` | `lower_scalar_term` | `omega-rust/psi/compilation/pipeline/06_checked-trees-to-lowered-psi/src/proofs.rs:3789` | checked scalar expressions | scalar proof-term tree depth | checked scalar expression nodes |

## Appendix A — Recursion dropped (not source-depth-driven)

| ID | Representative | Reason |
| --- | --- | --- |
| `D01` | `starts_following_item` | Declaration-boundary lookahead over tokens, not syntax-tree child edges. |
| `D02` | `parse_binary_chain_handle` | Already iterative (`pending` stack); operator chain length does not use Rust recursion. |
| `D03` | `parse_unary_expression_handle (prefix loop)` | Prefix operators collected in `Vec`; chain length is not stack-recursive. |
| `D04` | `contract_entailment::visit_* / fold_*` | Proof polynomial / entailment graphs with fixed internal depth caps, not source AST nesting. |
| `D05` | `checks::Application::compare (depth > 128)` | Type-application comparison bounded by constant 128, not source tree depth. |
| `D06` | `projection_climb (max_depth parameter)` | Member/index chain helper with explicit caller-bound `max_depth`, used for bounded projection search. |
| `D07` | `collect_reachable_nodes (stage 06)` | Explicit `pending`/`visited` loops over scalar computation graph — not recursive walkers. |
| `D08` | `follow_reference_endpoint` | Reference-hop loop uses `while`, not Rust recursion on path length. |
| `D09` | `render_ranked_subject` | Self-recursion formats diagnostic strings, not source structure depth. |
| `D10` | `reject_machine_arguments` | Fixed preparation validation over machine signatures, not AST depth scaling. |

## Appendix B — Mechanical SCC candidate disposition

Cross-crate Tarjan SCCs flagged `keep:review` by `analyze.py`: **305** candidates (**24** map to curated walker IDs, **281** dropped).

| Stage | Candidates | Kept | Dropped |
| --- | ---: | ---: | ---: |
| 01 | 11 | 10 | 1 |
| 02 | 71 | 6 | 65 |
| 03 | 39 | 1 | 38 |
| 04 | 22 | 3 | 19 |
| 05 | 150 | 3 | 147 |
| 06 | 12 | 1 | 11 |

| Stage | Representative | Disposition | Detail |
| --- | --- | --- | --- |
| 01 | `expression_handle_identifier_depth` | kept (W01-8) | `omega-rust/psi/compilation/pipeline/01_tokens-to-syntax-trees/src/bodies_and_transitions.rs`:1301 |
| 01 | `copy_expression_to_statement_identifier_path` | kept (W01-8) | `omega-rust/psi/compilation/pipeline/01_tokens-to-syntax-trees/src/bodies_and_transitions.rs`:1383 |
| 01 | `parse_satisfies_type_argument` | kept (W01-2) | `omega-rust/psi/compilation/pipeline/01_tokens-to-syntax-trees/src/contracts_and_parameters.rs`:191 |
| 01 | `parse_capability_definition` | kept (W01-4, W01-9) | `omega-rust/psi/compilation/pipeline/01_tokens-to-syntax-trees/src/declarations.rs`:20 |
| 01 | `type_reference_target_label` | dropped — Name/label helper over a type reference; no recursive type-child walk. | `omega-rust/psi/compilation/pipeline/01_tokens-to-syntax-trees/src/declarations.rs`:1210 |
| 01 | `parse_mathematical_type` | kept (W01-5) | `omega-rust/psi/compilation/pipeline/01_tokens-to-syntax-trees/src/declarations.rs`:1489 |
| 01 | `try_parse_static_argument` | kept (W01-10) | `omega-rust/psi/compilation/pipeline/01_tokens-to-syntax-trees/src/expressions.rs`:1648 |
| 01 | `is_place` | kept (W01-7) | `omega-rust/psi/compilation/pipeline/01_tokens-to-syntax-trees/src/statements_and_assembly.rs`:334 |
| 01 | `copy_compound_assignment_target` | kept (W01-7) | `omega-rust/psi/compilation/pipeline/01_tokens-to-syntax-trees/src/statements_and_assembly.rs`:538 |
| 01 | `copy_expression_as_place` | kept (W01-7) | `omega-rust/psi/compilation/pipeline/01_tokens-to-syntax-trees/src/statements_and_assembly.rs`:2415 |
| 01 | `rewrite_expression` | kept (W01-6) | `omega-rust/psi/compilation/pipeline/01_tokens-to-syntax-trees/src/type_syntax_and_patterns.rs`:48 |
| 02 | `retain_value_carrier` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/constant.rs`:93 |
| 02 | `has_nominal_carrier` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/constant.rs`:182 |
| 02 | `same_resolved_carrier` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/constant.rs`:204 |
| 02 | `validate_literal_initializer` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/constant.rs`:726 |
| 02 | `lower_type_parameters` | dropped — Mis-link or auxiliary walk: SCC from cross-crate name resolution, not a curated source-depth walker. | `omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/lowering.rs`:290 |
| 02 | `lower_mathematical_type` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/lowering.rs`:2310 |
| 02 | `extend_name_positions` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/lowering_machine.rs`:642 |
| 02 | `lower_type_operand` | dropped — Token stream helper without syntax-tree child dispatch. | `omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/lowering_machine.rs`:1189 |
| 02 | `render_ranked_subject` | dropped — Diagnostic/string rendering — not source AST nesting depth. | `omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/lowering_machine.rs`:1882 |
| 02 | `hoist_into_temp` | kept (W02-4) | `omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/lowering_statement.rs`:154 |
| 02 | `rewrite_children` | kept (W02-4) | `omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/lowering_statement.rs`:316 |
| 02 | `subject_contains_hoistable` | kept (W02-8) | `omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/lowering_statement.rs`:764 |
| 02 | `expression_contains` | kept (W02-8) | `omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/lowering_statement.rs`:1887 |
| 02 | `constructor_prefix_is_value` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/preparation.rs`:1267 |
| 02 | `scalar_literal_tree` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/preparation.rs`:2099 |
| 02 | `collect_requirement_instances` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/preparation.rs`:2806 |
| 02 | `substitute_statement_static_arguments` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/preparation.rs`:3217 |
| 02 | `substitute_expression_static_arguments` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/preparation.rs`:3355 |
| 02 | `rewritten_static_arguments` | dropped — Mis-link or auxiliary walk: SCC from cross-crate name resolution, not a curated source-depth walker. | `omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/preparation.rs`:3478 |
| 02 | `static_argument_for_actual` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/preparation.rs`:3552 |
| 02 | `into_integer` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/preparation_const_evaluation.rs`:1659 |
| 02 | `const_type_requires_declaration_proof` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/preparation_const_evaluation.rs`:3421 |
| 02 | `named_data` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/preparation_const_evaluation.rs`:3432 |
| 02 | `collect_type_expression_handles` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/preparation_const_evaluation.rs`:3953 |
| 02 | `normalize_template_type_reference` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/preparation_const_evaluation.rs`:4120 |
| 02 | `canonicalize_lifetime_bearing_type_argument` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/preparation_generic_data.rs`:165 |
| 02 | `normalize_domain_head_node` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/preparation_generic_data.rs`:917 |
| 02 | `reaches` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/preparation_generic_data.rs`:1471 |
| 02 | `type_reference_handle_is_substitutable` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/preparation_generic_data.rs`:1601 |
| 02 | `expression_mentions_in` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/preparation_generic_data.rs`:2157 |
| 02 | `decide_instance_type_equalities` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/preparation_generic_data.rs`:2184 |
| 02 | `fold_impossible_case_memberships` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/preparation_generic_data.rs`:2276 |
| 02 | `variant_fact_verdict` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/preparation_generic_data.rs`:2399 |
| 02 | `substitute_type_reference` | dropped — Mis-link or auxiliary walk: SCC from cross-crate name resolution, not a curated source-depth walker. | `omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/preparation_generic_data.rs`:2516 |
| 02 | `type_reference_mentions_parameter` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/preparation_generic_data.rs`:2849 |
| 02 | `substitute_cleanup_selection` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/preparation_generic_data.rs`:2915 |
| 02 | `select` | kept (W02-1, W02-2, W02-3, W02-5, W02-6, W02-9, W03-3) | `omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/preparation_generic_uses.rs`:20 |
| 02 | `call_context_parameter_types_agree` | dropped — Mis-link or auxiliary walk: SCC from cross-crate name resolution, not a curated source-depth walker. | `omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/preparation_generic_uses.rs`:474 |
| 02 | `relabel_data_literal_for_expected_type` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/preparation_generic_uses.rs`:725 |
| 02 | `visit_static_argument_type_inputs` | dropped — Iterative `while` loop — not Rust call-stack recursion on depth. | `omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/preparation_generic_uses.rs`:1051 |
| 02 | `subject_annotation` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/preparation_generic_uses.rs`:1670 |
| 02 | `reject_machine_arguments` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/preparation_machine_equations.rs`:1331 |
| 02 | `classify_selected_arguments` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/preparation_machine_equations.rs`:1440 |
| 02 | `classify_type_reference` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/preparation_machine_equations.rs`:1471 |
| 02 | `defines_structure` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/preparation_type_equations.rs`:1761 |
| 02 | `reject_unclosed_names` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/preparation_type_equations.rs`:1846 |
| 02 | `retarget_statement_span` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/selection.rs`:174 |
| 02 | `retarget_expression` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/selection.rs`:325 |
| 02 | `collect_type_reference_domain_symbols` | dropped — Mis-link or auxiliary walk: SCC from cross-crate name resolution, not a curated source-depth walker. | `omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/selection.rs`:2112 |
| 02 | `mutable_reference_parameter` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/selection.rs`:2383 |
| 02 | `expand` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/selection.rs`:2403 |
| 02 | `collect_type_domain_homes` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/selection.rs`:2830 |
| 02 | `type_references_match` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/selection.rs`:2955 |
| 02 | `collect_authored_statement_call_candidates` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/selection_authored.rs`:358 |
| 02 | `collect_statement_static_sites` | dropped — Token stream helper without syntax-tree child dispatch. | `omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/selection_authored.rs`:991 |
| 02 | `collect_statement_static_argument_candidates` | dropped — Token stream helper without syntax-tree child dispatch. | `omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/selection_authored.rs`:1026 |
| 02 | `collect_expression_subtree` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/selection_authored.rs`:1086 |
| 02 | `collect_static_argument_candidates` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/selection_authored.rs`:1427 |
| 02 | `assign_contract_call_symbols` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/symbols.rs`:336 |
| 02 | `collect_case_selections` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/symbols.rs`:674 |
| 02 | `same_case_subject` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/symbols.rs`:735 |
| 02 | `zero_fold` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/symbols.rs`:1062 |
| 02 | `collect_requirements` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/symbols.rs`:3291 |
| 02 | `has_type_role` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/symbols.rs`:5068 |
| 02 | `materialize_type_operand` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/symbols.rs`:5091 |
| 02 | `type_operand_nodes` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/symbols.rs`:5151 |
| 02 | `resolve_expression_table_call_target_symbol` | kept (W02-4, W02-7, W03-2, W06-1) | `omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/symbols_expression_paths.rs`:27 |
| 02 | `receiver_path_symbols` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/symbols_expression_paths.rs`:485 |
| 02 | `stamp_receiver_path_symbols_in_table` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/symbols_expression_paths.rs`:587 |
| 02 | `spelled_receiver_chain` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/symbols_expressions.rs`:109 |
| 02 | `insert_generic_parameter_children` | dropped — Mis-link or auxiliary walk: SCC from cross-crate name resolution, not a curated source-depth walker. | `omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/symbols_symbol_table.rs`:538 |
| 03 | `type_name` | kept (W02-5) | `omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/call_and_product_selections.rs`:560 |
| 03 | `land_one` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/call_and_product_selections.rs`:1464 |
| 03 | `flatten` | dropped — Iterative `while` loop — not Rust call-stack recursion on depth. | `omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/call_and_product_selections.rs`:2504 |
| 03 | `lower_mathematical_type` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/declarations.rs`:1277 |
| 03 | `proof_output_receiver_parts` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/declarations.rs`:1887 |
| 03 | `scan_statement` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/equality_and_exhaustiveness.rs`:85 |
| 03 | `scan_expression` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/equality_and_exhaustiveness.rs`:168 |
| 03 | `data_definition_by_symbol` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/equality_and_exhaustiveness.rs`:366 |
| 03 | `validate_equatable_data` | dropped — Iterative `while` loop — not Rust call-stack recursion on depth. | `omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/equality_and_exhaustiveness.rs`:617 |
| 03 | `value_type_base_name` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/equality_and_exhaustiveness.rs`:703 |
| 03 | `table_type_base_name` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/equality_and_exhaustiveness.rs`:723 |
| 03 | `flatten_binary` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/equality_and_exhaustiveness.rs`:1707 |
| 03 | `expressions_structurally_equal` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/equality_and_exhaustiveness.rs`:1725 |
| 03 | `subject_declared_type` | dropped — Mis-link or auxiliary walk: SCC from cross-crate name resolution, not a curated source-depth walker. | `omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/equality_and_exhaustiveness.rs`:1836 |
| 03 | `data_index_for_type` | dropped — Proof polynomial / entailment — not source AST nesting. | `omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/equality_and_exhaustiveness.rs`:1947 |
| 03 | `type_arguments_equal` | dropped — Proof polynomial / entailment — not source AST nesting. | `omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/equality_and_exhaustiveness.rs`:1987 |
| 03 | `lower_domain_membership_expression` | dropped — Mis-link or auxiliary walk: SCC from cross-crate name resolution, not a curated source-depth walker. | `omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/expression_lowering.rs`:201 |
| 03 | `new` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/expression_lowering.rs`:870 |
| 03 | `layout_query_operand_type` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/expression_lowering.rs`:2038 |
| 03 | `resolved_data_definition_for_type` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/expression_lowering.rs`:2333 |
| 03 | `declared_receiver_field` | dropped — Mis-link or auxiliary walk: SCC from cross-crate name resolution, not a curated source-depth walker. | `omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/expression_lowering.rs`:2446 |
| 03 | `classifier_reference_operand` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/expression_lowering.rs`:3216 |
| 03 | `classifier_reference_type` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/expression_lowering.rs`:3234 |
| 03 | `operand_data_type_name` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/expression_lowering.rs`:3269 |
| 03 | `lower_transition_pattern` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/expression_lowering.rs`:3856 |
| 03 | `lower_expression_handle_from_table_with_symbol_substitution` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/expression_lowering.rs`:3920 |
| 03 | `lower_expression_handle_from_table_in_fact_position` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/expression_lowering.rs`:3939 |
| 03 | `lower_static_machine_argument` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/expression_lowering.rs`:3949 |
| 03 | `subject_path` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/lowerer.rs`:347 |
| 03 | `compatible_contracts` | dropped — Mis-link or auxiliary walk: SCC from cross-crate name resolution, not a curated source-depth walker. | `omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/lowerer.rs`:1211 |
| 03 | `infer_declared_type_arguments` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/statement_lowering.rs`:616 |
| 03 | `substitute_call_type_arguments` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/statement_lowering.rs`:709 |
| 03 | `collection_type_reference` | dropped — Mis-link or auxiliary walk: SCC from cross-crate name resolution, not a curated source-depth walker. | `omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/statement_lowering.rs`:752 |
| 03 | `named_data_of` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/statement_lowering.rs`:809 |
| 03 | `expand` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/type_reference.rs`:23 |
| 03 | `normalize_domain_constraints_for_type` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/type_reference.rs`:122 |
| 03 | `with_domain_candidate_index` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/type_reference.rs`:346 |
| 03 | `lower_type_constraint_node_span_from_table` | dropped — Mis-link or auxiliary walk: SCC from cross-crate name resolution, not a curated source-depth walker. | `omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/type_reference.rs`:1261 |
| 03 | `with_generic_origin_index` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/type_reference.rs`:1888 |
| 04 | `name` | kept (W02-5) | `omega-rust/psi/compilation/pipeline/04_typed-trees-to-typed-trees/src/assembly_and_operators.rs`:164 |
| 04 | `target_reads` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/04_typed-trees-to-typed-trees/src/assembly_and_operators.rs`:250 |
| 04 | `copy_type_contexts` | dropped — Mis-link or auxiliary walk: SCC from cross-crate name resolution, not a curated source-depth walker. | `omega-rust/psi/compilation/pipeline/04_typed-trees-to-typed-trees/src/body_cloning_and_rewriting.rs`:42 |
| 04 | `copy_static_argument_type_payloads` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/04_typed-trees-to-typed-trees/src/body_cloning_and_rewriting.rs`:1980 |
| 04 | `realized_receiver_type` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/04_typed-trees-to-typed-trees/src/body_cloning_and_rewriting.rs`:2241 |
| 04 | `closed_bool` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/04_typed-trees-to-typed-trees/src/body_cloning_and_rewriting.rs`:4029 |
| 04 | `collect_evidence_requirement_closure` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/04_typed-trees-to-typed-trees/src/body_cloning_and_rewriting.rs`:4245 |
| 04 | `collect_static_argument_type_roots` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/04_typed-trees-to-typed-trees/src/body_cloning_and_rewriting.rs`:4901 |
| 04 | `visit_lifetime_bindings_in_scope` | kept (W04-3) | `omega-rust/psi/compilation/pipeline/04_typed-trees-to-typed-trees/src/conformance.rs`:822 |
| 04 | `collect_conformance_lifetime_bindings` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/04_typed-trees-to-typed-trees/src/conformance.rs`:968 |
| 04 | `retain_access_result_type` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/04_typed-trees-to-typed-trees/src/constructed_results.rs`:190 |
| 04 | `close` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/04_typed-trees-to-typed-trees/src/contextual_and_requirement_calls.rs`:1833 |
| 04 | `elaborate_value_fields` | dropped — Iterative `while` loop — not Rust call-stack recursion on depth. | `omega-rust/psi/compilation/pipeline/04_typed-trees-to-typed-trees/src/data_and_product_operands.rs`:194 |
| 04 | `instantiate_type` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/04_typed-trees-to-typed-trees/src/data_and_product_operands.rs`:595 |
| 04 | `visit_post_order` | kept (W04-1, W05-CORE) | `omega-rust/psi/compilation/pipeline/04_typed-trees-to-typed-trees/src/lib.rs`:226 |
| 04 | `contains_reachable_construction` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/04_typed-trees-to-typed-trees/src/preparation.rs`:553 |
| 04 | `fill` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/04_typed-trees-to-typed-trees/src/preparation.rs`:880 |
| 04 | `instantiate_requirement_applications` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/04_typed-trees-to-typed-trees/src/preparation.rs`:2151 |
| 04 | `collect_machine_proposals_for_callee` | dropped — Mis-link or auxiliary walk: SCC from cross-crate name resolution, not a curated source-depth walker. | `omega-rust/psi/compilation/pipeline/04_typed-trees-to-typed-trees/src/selection_and_reflection.rs`:34 |
| 04 | `state_by_symbol` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/04_typed-trees-to-typed-trees/src/selection_and_reflection.rs`:343 |
| 04 | `infer_static_bindings` | dropped — Mis-link or auxiliary walk: SCC from cross-crate name resolution, not a curated source-depth walker. | `omega-rust/psi/compilation/pipeline/04_typed-trees-to-typed-trees/src/selection_and_reflection.rs`:1094 |
| 04 | `completed_subject_type` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/04_typed-trees-to-typed-trees/src/selection_and_reflection.rs`:3357 |
| 05 | `reads_extracted_copy` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/borrow.rs`:196 |
| 05 | `collect_place_evaluation_reads` | dropped — Mis-link or auxiliary walk: SCC from cross-crate name resolution, not a curated source-depth walker. | `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/borrow.rs`:648 |
| 05 | `append_read_access` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/borrow.rs`:757 |
| 05 | `collect_argument_accesses` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/borrow.rs`:1290 |
| 05 | `collect_expression_borrow_calls` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/borrow.rs`:1345 |
| 05 | `reference_local_borrow_loans` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/borrow.rs`:5779 |
| 05 | `binding_borrow_loans` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/borrow.rs`:7766 |
| 05 | `actual_erased_identity_at` | dropped — Hard depth cap (128) — bounded comparison, not source tree depth. | `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/checks.rs`:3537 |
| 05 | `check_received_value` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/checks.rs`:3958 |
| 05 | `collect_clean_conjuncts` | dropped — Iterative `while` loop — not Rust call-stack recursion on depth. | `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/checks.rs`:9185 |
| 05 | `establishes` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/checks.rs`:9607 |
| 05 | `collect_structural_guard_consequences` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/checks.rs`:11131 |
| 05 | `collect_integer_order_relations` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/checks.rs`:11217 |
| 05 | `expression_is_integer_typed` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/checks.rs`:11563 |
| 05 | `is_state_independent_borrow_source` | kept (W05-P05) | `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/checks_borrows.rs`:1656 |
| 05 | `captured_storage_index` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/checks_borrows_overlap.rs`:2874 |
| 05 | `append_sources` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/checks_content.rs`:5903 |
| 05 | `type_size` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/checks_content.rs`:6097 |
| 05 | `append_type_domains` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/checks_content.rs`:6614 |
| 05 | `recipe_requirements` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/checks_contracts.rs`:108 |
| 05 | `proves_with` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/checks_contracts.rs`:2402 |
| 05 | `proves_scoped` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/checks_contracts.rs`:2538 |
| 05 | `conjunct` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/checks_contracts_calls.rs`:1652 |
| 05 | `equivalent` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/checks_contracts_calls.rs`:1696 |
| 05 | `assignment_target_mentions_name` | dropped — Token stream helper without syntax-tree child dispatch. | `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/checks_contracts_calls.rs`:1962 |
| 05 | `collect_expression_self_fields` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/checks_contracts_calls.rs`:2278 |
| 05 | `assignment_target_mentions_field` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/checks_contracts_calls.rs`:2357 |
| 05 | `render_projected_requirement` | dropped — Diagnostic/string rendering — not source AST nesting depth. | `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/checks_contracts_calls.rs`:2817 |
| 05 | `guard_conjunct_matches` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/checks_contracts_calls.rs`:2909 |
| 05 | `negated_guard_conjunct_matches` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/checks_contracts_calls.rs`:2947 |
| 05 | `premise_conjuncts` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/checks_contracts_calls.rs`:3085 |
| 05 | `current_bindings` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/checks_contracts_exits.rs`:3262 |
| 05 | `domain_predicate_meaning` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/checks_contracts_exits.rs`:4671 |
| 05 | `semantic_context_proves_instantiated_boolean_expression` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/checks_contracts_prover.rs`:834 |
| 05 | `seed_zero_values` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/checks_multiplicity.rs`:1724 |
| 05 | `terminal_arm_route` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/checks_multiplicity.rs`:4384 |
| 05 | `expression_establishes_obligation` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/checks_multiplicity.rs`:10882 |
| 05 | `collect_frames` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/checks_multiplicity_borrowed_windows.rs`:708 |
| 05 | `state_frontiers` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/checks_multiplicity_claim_outcomes.rs`:903 |
| 05 | `claim_outcomes_for_expression` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/checks_multiplicity_claim_outcomes.rs`:4157 |
| 05 | `positive_expression_proves` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/checks_operators.rs`:497 |
| 05 | `contexts_prove_boolean_expression` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/checks_operators.rs`:2223 |
| 05 | `closed_clause_value` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/checks_operators.rs`:2488 |
| 05 | `closed_scalar_value` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/checks_operators.rs`:2548 |
| 05 | `expression_tail_eligible` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/checks_ranges.rs`:1303 |
| 05 | `guard_bounds` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/checks_ranges.rs`:1556 |
| 05 | `parse_counter_relational_upper` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/checks_ranges.rs`:2042 |
| 05 | `collect_authored_upper_relations` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/checks_ranges.rs`:2217 |
| 05 | `parse_counter_upper_bound` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/checks_ranges.rs`:2717 |
| 05 | `seed_index_proofs_from_expression` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/checks_ranges.rs`:3425 |
| 05 | `positive_guard` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/checks_termination.rs`:4225 |
| 05 | `collect_source_operation_spans` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/execution.rs`:1295 |
| 05 | `collect_dependency_paths_from_expression` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/flow.rs`:6657 |
| 05 | `mathematical_captures_are_inline` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/flow.rs`:6872 |
| 05 | `collect_nominal_symbols` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/flow.rs`:10538 |
| 05 | `indexed_statements` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/flow_context.rs`:1171 |
| 05 | `classify_operator_result_ownership` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/flow_ownership.rs`:1654 |
| 05 | `profile_sources` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/flow_retained_borrows.rs`:564 |
| 05 | `bound_point_rank` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/flow_state_values.rs`:1572 |
| 05 | `value_proves_predicate` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/flow_transfers.rs`:113 |
| 05 | `convert_through` | dropped — Mis-link or auxiliary walk: SCC from cross-crate name resolution, not a curated source-depth walker. | `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/flow_transfers.rs`:3081 |
| 05 | `typed_operator_authored_selection_candidates` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/products.rs`:52 |
| 05 | `derive_checked_nominal_call_target` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/products.rs`:127 |
| 05 | `infer_checked_recursive_machine_terminations` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/products.rs`:202 |
| 05 | `append_trait_contract_facts_for_machine` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/published.rs`:1390 |
| 05 | `bind_semantic_applications_in_expression` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/published.rs`:4036 |
| 05 | `append_evidence_interface_reference_regions` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/published.rs`:4721 |
| 05 | `contract_expression_place` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/published.rs`:7474 |
| 05 | `collect_zero_initialized_field_domains` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/published.rs`:9145 |
| 05 | `carry_constraint_permissions` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/published.rs`:9519 |
| 05 | `instantiate_contract_expression_place` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/published.rs`:10073 |
| 05 | `expression_receiver_head` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/published_facts.rs`:461 |
| 05 | `expression_path_head` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/published_facts.rs`:475 |
| 05 | `assignment_target_is_field` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/published_facts.rs`:539 |
| 05 | `known` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/published_facts_boolean_predicates.rs`:2505 |
| 05 | `boolean_view_observation` | dropped — Mis-link or auxiliary walk: SCC from cross-crate name resolution, not a curated source-depth walker. | `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/published_facts_crash_calls.rs`:2020 |
| 05 | `member_symbol_from_type_reference` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/shared.rs`:1792 |
| 05 | `operand_carrier` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/shared.rs`:2516 |
| 05 | `logical_match_operand` | dropped — Mis-link or auxiliary walk: SCC from cross-crate name resolution, not a curated source-depth walker. | `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/values_scalar.rs`:2964 |
| 05 | `resolved_subject` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/values_scalar.rs`:3770 |
| 05 | `shared_reference_chain` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/values_scalar.rs`:10948 |
| 05 | `contribute` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/validation/src/arithmetic_domains.rs`:203 |
| 05 | `project_possible` | dropped — Iterative `while` loop — not Rust call-stack recursion on depth. | `omega-rust/psi/compilation/validation/src/arithmetic_domains.rs`:1881 |
| 05 | `bitwise_known_unsigned` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/validation/src/arithmetic_domains.rs`:8378 |
| 05 | `condition_belongs_to_state` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/validation/src/arithmetic_domains.rs`:13570 |
| 05 | `match_subtract_floor` | dropped — Mis-link or auxiliary walk: SCC from cross-crate name resolution, not a curated source-depth walker. | `omega-rust/psi/compilation/validation/src/arithmetic_domains.rs`:20431 |
| 05 | `domain_place_type` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/validation/src/arithmetic_domains.rs`:24987 |
| 05 | `direct_operand` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/validation/src/arithmetic_domains.rs`:25165 |
| 05 | `narrow_abstract_specification_environment` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/validation/src/arithmetic_domains.rs`:25269 |
| 05 | `validate_state_signature` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/validation/src/arithmetic_domains.rs`:25720 |
| 05 | `applied_region` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/validation/src/borrow_and_reflection.rs`:1009 |
| 05 | `input_write_only_access` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/validation/src/borrow_and_reflection.rs`:3499 |
| 05 | `collect_nominal_expectations` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/validation/src/borrow_and_reflection.rs`:3731 |
| 05 | `composes` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/validation/src/calls.rs`:1766 |
| 05 | `first_nested_call` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/validation/src/calls.rs`:1802 |
| 05 | `signature_result_candidate_origins` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/validation/src/calls.rs`:8741 |
| 05 | `graph` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/validation/src/calls.rs`:12922 |
| 05 | `operand_profiles` | dropped — Mis-link or auxiliary walk: SCC from cross-crate name resolution, not a curated source-depth walker. | `omega-rust/psi/compilation/validation/src/completed_storage_and_conservation.rs`:1364 |
| 05 | `judge_mathematical_term` | dropped — Proof entailment graph with internal caps — not source AST nesting. | `omega-rust/psi/compilation/validation/src/contract_entailment.rs`:4616 |
| 05 | `runtime_predicate_fact` | dropped — Proof entailment graph with internal caps — not source AST nesting. | `omega-rust/psi/compilation/validation/src/contract_entailment.rs`:8988 |
| 05 | `runtime_fact_for_parameters` | dropped — Proof entailment graph with internal caps — not source AST nesting. | `omega-rust/psi/compilation/validation/src/contract_entailment.rs`:9049 |
| 05 | `split_conjuncts` | dropped — Proof entailment graph with internal caps — not source AST nesting. | `omega-rust/psi/compilation/validation/src/contract_entailment.rs`:15865 |
| 05 | `pair_conjuncts` | dropped — Proof entailment graph with internal caps — not source AST nesting. | `omega-rust/psi/compilation/validation/src/contract_entailment.rs`:17089 |
| 05 | `refine` | dropped — Proof entailment graph with internal caps — not source AST nesting. | `omega-rust/psi/compilation/validation/src/contract_entailment.rs`:35938 |
| 05 | `value_index_names` | dropped — Proof entailment graph with internal caps — not source AST nesting. | `omega-rust/psi/compilation/validation/src/contract_entailment.rs`:40386 |
| 05 | `prepare_expression` | dropped — Proof entailment graph with internal caps — not source AST nesting. | `omega-rust/psi/compilation/validation/src/contract_entailment.rs`:47974 |
| 05 | `detach_returned_nominal_value` | dropped — Proof entailment graph with internal caps — not source AST nesting. | `omega-rust/psi/compilation/validation/src/contract_entailment.rs`:63027 |
| 05 | `same_current` | dropped — Proof entailment graph with internal caps — not source AST nesting. | `omega-rust/psi/compilation/validation/src/contract_entailment.rs`:66953 |
| 05 | `type_carries_predicate_context` | dropped — Proof entailment graph with internal caps — not source AST nesting. | `omega-rust/psi/compilation/validation/src/contract_entailment.rs`:87429 |
| 05 | `has_produced_subject` | dropped — Proof entailment graph with internal caps — not source AST nesting. | `omega-rust/psi/compilation/validation/src/contract_entailment.rs`:89840 |
| 05 | `callable_receiver_type_name` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/validation/src/declarations_core.rs`:2591 |
| 05 | `collect_call_summaries` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/validation/src/domains_and_predicates.rs`:8332 |
| 05 | `scan_expression_reads` | dropped — Mis-link or auxiliary walk: SCC from cross-crate name resolution, not a curated source-depth walker. | `omega-rust/psi/compilation/validation/src/domains_and_predicates.rs`:8686 |
| 05 | `composed_value_established` | dropped — Mis-link or auxiliary walk: SCC from cross-crate name resolution, not a curated source-depth walker. | `omega-rust/psi/compilation/validation/src/domains_and_predicates.rs`:9270 |
| 05 | `write_place_spelling` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/validation/src/domains_and_predicates.rs`:10080 |
| 05 | `literal_value` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/validation/src/domains_and_predicates.rs`:11515 |
| 05 | `expression_symbolic_value` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/validation/src/domains_and_predicates.rs`:11940 |
| 05 | `visit_calls` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/validation/src/domains_and_predicates.rs`:13361 |
| 05 | `restore_parameter_write` | kept (W05-V06) | `omega-rust/psi/compilation/validation/src/domains_and_predicates.rs`:13378 |
| 05 | `origin_for_expression` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/validation/src/effect_inference.rs`:895 |
| 05 | `self_field_path` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/validation/src/expression_types_and_fields.rs`:7868 |
| 05 | `type_reference_data_name` | dropped — Iterative `while` loop — not Rust call-stack recursion on depth. | `omega-rust/psi/compilation/validation/src/expression_types_and_fields.rs`:8447 |
| 05 | `source_operations_have_same_meaning` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/validation/src/float_and_mathematical_contracts.rs`:3730 |
| 05 | `inner` | kept (W05-V01) | `omega-rust/psi/compilation/validation/src/frozen_program_and_queries.rs`:2838 |
| 05 | `has_active_recast_formation` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/validation/src/frozen_program_and_queries.rs`:3385 |
| 05 | `typed_program` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/validation/src/frozen_program_and_queries.rs`:3419 |
| 05 | `expression_interval` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/validation/src/frozen_program_and_queries.rs`:3449 |
| 05 | `land_float_value_for_type` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/validation/src/literals_and_struct_literals.rs`:631 |
| 05 | `land_float_tree` | dropped — Mis-link or auxiliary walk: SCC from cross-crate name resolution, not a curated source-depth walker. | `omega-rust/psi/compilation/validation/src/literals_and_struct_literals.rs`:729 |
| 05 | `collect_static_selections` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/validation/src/machine_calls_core.rs`:325 |
| 05 | `boundary_requirement_symbol` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/validation/src/machine_calls_core.rs`:7131 |
| 05 | `machine_specialization_matches_template_identity` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/validation/src/machine_calls_core.rs`:7245 |
| 05 | `record_expression_owner` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/validation/src/machine_calls_core.rs`:7486 |
| 05 | `push_expected_call` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/validation/src/machine_calls_core.rs`:8624 |
| 05 | `array_element_type` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/validation/src/machine_calls_core.rs`:8692 |
| 05 | `collect_expression_bindings` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/validation/src/machine_calls_core.rs`:8839 |
| 05 | `canonical_closed_operator_realization_bytes` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/validation/src/operators.rs`:29 |
| 05 | `collect_expression_operator_use` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/validation/src/operators.rs`:2001 |
| 05 | `is_mutable_place_handle` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/validation/src/places.rs`:1184 |
| 05 | `expression_root_name_handle` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/validation/src/places.rs`:1196 |
| 05 | `unwrapped_type_reference` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/validation/src/places.rs`:4762 |
| 05 | `value_argument_phrase` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/validation/src/program_validation.rs`:3120 |
| 05 | `contract_argument_root` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/validation/src/proof_contracts_core.rs`:6 |
| 05 | `is_boolean_asm_fact_expression` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/validation/src/proof_contracts_core.rs`:7539 |
| 05 | `descend` | dropped — Mis-link or auxiliary walk: SCC from cross-crate name resolution, not a curated source-depth walker. | `omega-rust/psi/compilation/validation/src/structural_call_custody.rs`:511 |
| 05 | `collect_call_demands_in` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/validation/src/traits.rs`:5771 |
| 05 | `collect_dynamic_expression_call_updates` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/validation/src/traits.rs`:6073 |
| 05 | `normalize_open_index_expression_operations` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/validation/src/type_references_and_wire.rs`:3447 |
| 05 | `validate_boundary_completion_declarations` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/validation/src/value_custody_core.rs`:5370 |
| 05 | `expression_origin` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/validation/src/value_custody_core.rs`:7277 |
| 06 | `collect_value_roots` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/06_checked-trees-to-lowered-psi/src/attached_unit.rs`:2929 |
| 06 | `establishes_element_view` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/06_checked-trees-to-lowered-psi/src/attached_unit.rs`:3065 |
| 06 | `returning_exits` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/06_checked-trees-to-lowered-psi/src/attached_unit.rs`:16745 |
| 06 | `byte_record_observation` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/06_checked-trees-to-lowered-psi/src/expression_bindings.rs`:4974 |
| 06 | `name` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/06_checked-trees-to-lowered-psi/src/projections.rs`:894 |
| 06 | `lower_content_projection_scalar` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/06_checked-trees-to-lowered-psi/src/proofs.rs`:110 |
| 06 | `retain_services` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/06_checked-trees-to-lowered-psi/src/retention.rs`:1702 |
| 06 | `backing_type` | kept (W02-5, W06-1, W06-2) | `omega-rust/psi/compilation/pipeline/06_checked-trees-to-lowered-psi/src/shared.rs`:21 |
| 06 | `prepare_operation` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/06_checked-trees-to-lowered-psi/src/shared.rs`:43 |
| 06 | `classes` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/06_checked-trees-to-lowered-psi/src/shared.rs`:356 |
| 06 | `custody` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/06_checked-trees-to-lowered-psi/src/shared.rs`:1469 |
| 06 | `successors` | dropped — Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth. | `omega-rust/psi/compilation/pipeline/06_checked-trees-to-lowered-psi/src/unit_composed_control.rs`:12943 |

## Appendix C — Limits and unverified edges

- W05-EXT lives in `representations/typed-trees`; stage-05 crates call it but it is outside the stage `src/` tree — include when retiring typed expression walks.
- Automated call graph (`analyze.py`) links callees by identifier across stage crates; it does not resolve traits, `mod` paths, or `use` renames perfectly. Curated table is authoritative.
- Binary operator chain maxima come from the stage-01 syntax-tree parse (longest left-spine `Binary` chain per expression, any site). Stage-01 Rust tests still pin 1024 seven-operator groups for the parser grammar.
- Could not mechanically prove absence of further stage-06 expression walkers hidden behind iterative queues without a rustc call graph; manual scan found no additional Rust-recursive expression walkers beyond W06-1/W06-2.

## Method

```bash
git checkout 0835b3aaf803ed97f905be42d94727ff58459263
mbx build --manifest-path wiki/drafts/audits/measure_depths/Cargo.toml
ulimit -s unlimited
wiki/drafts/audits/measure_depths/target/debug/measure-depths . \
  > wiki/drafts/audits/recursive_walkers_2026-11-12_parser_depths.json
python3 wiki/drafts/audits/recursive_walkers_2026-11-12_analyze.py
python3 wiki/drafts/audits/recursive_walkers_2026-11-12_disposition.py
python3 wiki/drafts/audits/recursive_walkers_2026-11-12_render.py
```

- **Call graph (appendix input):** stage-local union of stage crate `src/`
  trees; callee resolution by function name across crates in the same stage
  (may miss `mod` paths and trait calls). Curated list above overrides.
- **Depths:** `recursive_walkers_2026-11-12_analyze.py`.
- **Report body:** `recursive_walkers_2026-11-12_verified.json` (hand-verified).

## Host

Cursor Cloud Agent VM, Linux, `/agent/repos/omega`.
