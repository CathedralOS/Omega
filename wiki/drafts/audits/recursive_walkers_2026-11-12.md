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

Scanned **50111** `.omg` files under `tests/omega`, `samples`,
`source/library`, and `source/psi`. Parenthesis and angle counts are
delimiter scans with comments/strings stripped. **Binary operator chain**
counts the longest run of binary punctuators inside a single `(...)`
region (parse-lexical; see limitations). **Prefix chain** counts
consecutive `~` / `&` / `*` / `!` at operand starts inside parens.

Excluded depth-test fixtures: `fail/parser/nesting_exceeds_max_depth`, `pass/parser/deep_nesting_within_limit`, `pass/expressions/deep_paren_nesting`.

| Dimension | All corpus (max) | Excluding depth fixtures (max) | Deepest real program |
| --- | ---: | ---: | --- |
| Parenthesis nesting | 1100 (`tests/omega/fail/parser/nesting_exceeds_max_depth/main.omg`) | 9 (`tests/omega/pass/terminal_psi/integer_control_contract/main.omg`) | `tests/omega/pass/terminal_psi/integer_control_contract/main.omg` |
| Type `<…>` nesting | 381 (`tests/omega/pass/terminal_psi/integer_control_contract/main.omg`) | 381 (`tests/omega/pass/terminal_psi/integer_control_contract/main.omg`) | `tests/omega/pass/terminal_psi/integer_control_contract/main.omg` |
| Binary op chain (in one paren group) | 8 (`tests/omega/pass/modules/match_constant_indices/main.omg`) | 8 (`tests/omega/pass/modules/match_constant_indices/main.omg`) | `tests/omega/pass/modules/match_constant_indices/main.omg` |
| Prefix operator chain | 1 (`tests/omega/fail/resources/owned_extent_erased_label_rejected/main.omg`) | 1 (`tests/omega/fail/resources/owned_extent_erased_label_rejected/main.omg`) | `tests/omega/fail/resources/owned_extent_erased_label_rejected/main.omg` |

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

## Appendix B — Limits and unverified edges

- W05-EXT lives in `representations/typed-trees`; stage-05 crates call it but it is outside the stage `src/` tree — include when retiring typed expression walks.
- Automated call graph (`analyze.py`) links callees by identifier across stage crates; it does not resolve traits, `mod` paths, or `use` renames perfectly. Curated table is authoritative.
- Binary operator chain measurement is parse-lexical (binary punctuators inside each `(...)` region), not a full Omega expression parse; treat as a lower bound. Exact chain depth is pinned in stage-01 Rust tests (1024 seven-operator groups).
- Could not mechanically prove absence of further stage-06 expression walkers hidden behind iterative queues without a rustc call graph; manual scan found no additional Rust-recursive expression walkers beyond W06-1/W06-2.

## Method

```bash
git checkout 0835b3aaf803ed97f905be42d94727ff58459263
python3 wiki/drafts/audits/recursive_walkers_2026-11-12_analyze.py
python3 wiki/drafts/audits/recursive_walkers_2026-11-12_render.py
```

- **Call graph (appendix input):** stage-local union of stage crate `src/`
  trees; callee resolution by function name across crates in the same stage
  (may miss `mod` paths and trait calls). Curated list above overrides.
- **Depths:** `recursive_walkers_2026-11-12_analyze.py`.
- **Report body:** `recursive_walkers_2026-11-12_verified.json` (hand-verified).

## Host

Cursor Cloud Agent VM, Linux, `/agent/repos/omega`.
