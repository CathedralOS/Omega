# Recursive walker inventory (Psi stages 01–03)

This is a checklist for the `TASKS.md` item that stores recursive terms as flat
tables. It lists the walkers whose **Rust stack recursion** scales with
**source** expression/type/pattern/statement nesting or operator/projection
chain length. Source depth is bounded only by memory and time
([compiler request](../../spec/build/compiler_request.md#validation-and-failures)),
so each of these walks must become a loop over flat-table edges.

The inventory was hand-verified at `0835b3aaf803ed97f905be42d94727ff58459263`.
Line numbers have moved since then; find each walker by its function names.

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
- **Stages:** 03
- **Walker IDs:** `W03-1`

### `G-typed-stmt` — Typed statement / machine bodies

- **Flat representation:** `TypedTrees.statements` (`StatementTable`)
- **Stages:** 03
- **Walker IDs:** `W03-2`
- **Notes:** Statement-indexed walks scale with state body length, not a fixed enum.

## Kept walker counts (verified)

| Stage | Kept walkers |
| --- | ---: |
| 01 — parsing | 10 |
| 02 — resolution | 9 |
| 03 — typing | 7 |
| **Total** | **26** |

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

## Appendix A — Recursion dropped (not source-depth-driven)

| ID | Representative | Reason |
| --- | --- | --- |
| `D01` | `starts_following_item` | Declaration-boundary lookahead over tokens, not syntax-tree child edges. |
| `D02` | `parse_binary_chain_handle` | Already iterative (`pending` stack); operator chain length does not use Rust recursion. |
| `D03` | `parse_unary_expression_handle (prefix loop)` | Prefix operators collected in `Vec`; chain length is not stack-recursive. |
| `D09` | `render_ranked_subject` | Self-recursion formats diagnostic strings, not source structure depth. |
| `D10` | `reject_machine_arguments` | Fixed preparation validation over machine signatures, not AST depth scaling. |

## Appendix B — Limits and unverified edges

- The curated table was built from a call graph that linked callees by identifier across stage crates; that graph did not resolve traits, `mod` paths, or `use` renames perfectly, so a walker reached only through those may be missing.
- Stage-01 Rust tests pin 1024 seven-operator groups for the parser grammar; binary operator chains use `parse_binary_chain_handle` with an explicit `pending` stack, so chain length does not use Rust recursion.
