slice_0056

| entry id | file:line | verdict | one-line reason |
|----------|-----------|---------|-----------------|
| A0425 | domains_and_predicates.rs:1478 | REACHABLE | `constant_subtree_meaning` returns `None` for non-integer/borrow/binary nodes and `unwrap_or(true)` treats them as builtin-folded constants in arithmetic narrowing. |
| A0426 | domains_and_predicates.rs:582-588 | REACHABLE | `binary_expression_meaning` returns true for And/Or/bitwise/shift without operand-type or resolution checks, so `has_builtin_bound_expression_meaning` can admit whole subtrees. |
| A0427 | domains_and_predicates.rs:683 | REACHABLE | `bound_subtree_height` marks every non-binary expression as `SubtreeMeaning::Valid(1)` with no builtin-meaning probe on leaves. |
| A0428 | domains_and_predicates.rs:877-892 | CLOSED | Vacuous `.all` on zero selection occurrences only applies after single-candidate resolved boundary sealed-float checks at dp:852-876. |
| A0429 | domains_and_predicates.rs:1035 | REACHABLE | When subject has no closed data reference, `selected_case_in_subject_namespace` still returns `Some(selected)` after prior exact-membership gates. |
| A0430 | domains_and_predicates.rs:5043;6250-6253 | CLOSED | `Comparison` literals are built only via `compare()` with equality/ordering operators that always have `complement()` (typed-trees values.rs:4779-4788). |
| A0431 | domains_and_predicates.rs:6605-6611 | REACHABLE | Integer actual with `actual_type == None` skips landing/primitive mismatch when `landing()` is absent and still proceeds to bounds. |
| A0432 | domains_and_predicates.rs:7502-7504 | CLOSED | Accepted programs reach entailment with `call_frames` `Some` because `validate` already runs `MachineSymbols::build` per machine (program_validation.rs:4191). |

### A0425

1. Read `folded_constant_has_builtin_meaning` and `constant_subtree_meaning` in full at `omega-rust/psi/compilation/validation/src/domains_and_predicates.rs:1473-1504` (read).
2. Search `folded_constant_has_builtin_meaning` in `omega-rust`: 3 matches (definition plus `arithmetic_domains.rs:24760`).
3. Search `folded_constant_is_builtin` in `omega-rust`: 7 matches; production uses include `domains_and_predicates.rs:1408-1409` and `arithmetic_domains.rs:13579-13580`, `14101-14102` (read call sites; not every arithmetic_domains caller).
4. Read `abstract_specification_constant_has_builtin_meaning` and `abstract_specification_interval` at `arithmetic_domains.rs:24755-24764`, `24903-24912` (read); read bound check at `25159-25166` (read).

**Input on accepting branch:** Any expression handle whose subtree is not exclusively integer/borrow/binary (e.g. a name or call under a fold check), fed into `folded_constant_has_builtin_meaning`, which maps failed subtree classification to `true` at dp:1478.

**Verdict:** REACHABLE — A numeric bound or abstract-spec coercion path that should refuse a non-constant operand can treat it as a builtin-folded constant because `None` from `constant_subtree_meaning` becomes acceptance.

**Unsure:** Whether every production path pairing this with `literal_i64` would exclude non-literals before the `unwrap_or`; `abstract_specification_constant_has_builtin_meaning` is used without that guard at dp:25159-25164 (read).

### A0426

1. Read `binary_expression_meaning` in full at `domains_and_predicates.rs:547-590` (read); entry description matches `And/Or/Bitwise*/Shift* => true` (read).
2. Search `binary_expression_meaning` in `omega-rust`: 0 external references (internal only).
3. Search `has_builtin_binary_expression_meaning` in `omega-rust`: 46 matching lines (grep output; did not read every caller).
4. Read `has_builtin_binary_expression_meaning` at `domains_and_predicates.rs:489-511` (read).
5. Read `bound_subtree_height` binary arm at `domains_and_predicates.rs:625-649` (read).
6. Read `machine_calls_core.rs` closed binary branch at `4296-4310` (read) where `has_builtin_bound_expression_meaning` gates closed expressions.

**Input on accepting branch:** A binary expression with operator And, Or, or a bitwise/shift spelling whose operands have user-authored operator meaning, when any caller only consults `has_builtin_binary_expression_meaning` or `has_builtin_bound_expression_meaning`.

**Verdict:** REACHABLE — A program using a trait-provided bitwise or logical operator spelling could be classified as having builtin binary meaning and admitted as a bounded/closed expression without verifying operands resolve to builtin operators.

**Unsure:** Stage-04/05 operator-resolution diagnostics for every such spelling; assembly-only check at `assembly_and_operators.rs:1387-1396` does not cover non-assembly uses (read).

### A0427

1. Read `bound_subtree_height` in full at `domains_and_predicates.rs:599-701` (read).
2. Search `bound_subtree_height` in `omega-rust`: 7 matches, all in `domains_and_predicates.rs` (read).
3. Read `has_builtin_bound_expression_meaning` at `domains_and_predicates.rs:430-440` (read).

**Input on accepting branch:** Leaf expressions (names, calls, literals other than the binary recursion path) reached as children of a binary node that already passed `has_builtin_binary_expression_meaning` at dp:627.

**Verdict:** REACHABLE — Combined with A0426, a custom binary parent plus arbitrary leaf children yields `SubtreeMeaning::Valid` without per-leaf meaning checks.

**Unsure:** None material beyond caller-specific downstream filters not fully traced.

### A0428

1. Read `sealed_float_comparison_declaration_for_operands` in full at `domains_and_predicates.rs:834-894` (read).
2. Search `sealed_float_comparison_declaration_for_operands` in `omega-rust`: 4 call sites (`domains_and_predicates.rs`, `machine_calls_core.rs:4307`, `contract_entailment.rs:86694`; did not read ce caller body).
3. Read prerequisite guards at dp:841-876 (read).

**Input on accepting branch:** A float comparison expression with zero `authored_selection_occurrences` after passing empty trait-meaning check, exactly-one candidate resolution, boundary semantics, and sealed-operator check.

**Verdict:** CLOSED — Every path to the `.all` at dp:877 goes through dp:852-876; with no occurrences, vacuous truth does not bypass operator resolution or sealed-float identity requirements.

**Unsure:** Whether an expression can reach dp:877 with zero occurrences yet wrong runtime operator; prior filters require resolved single boundary candidate (read dp:852-860).

### A0429

1. Read `selected_case_in_subject_namespace` at `domains_and_predicates.rs:1011-1036` (read); entry matches fallback `Some(selected)` when closed definition lookup fails (read).
2. Search `selected_case_in_subject_namespace`: 3 matches, all in `domains_and_predicates.rs` (read).
3. Read callers `applied_case_membership_variant` and `applied_declaration_case_membership_variant` at `970-1008` (read); both require exact membership predicates first (read).
4. Read downstream `applied_case_classifier` at `contract_entailment.rs:47606-47622` (read) which also accepts when `subject_type` is `None`.
5. Read pipeline use at `values_scalar.rs:2629-2631` and `flow_place.rs:458-466` (read; not full lowering paths).

**Input on accepting branch:** Case-membership comparison that passed `has_exact_*_case_membership_meaning` but whose subject yields no `TypeReferenceHandle` or no `closed_data_definition`, so dp:1035 returns the RHS case symbol unchecked against subject namespace.

**Verdict:** REACHABLE — A case tag on a subject whose closed data type cannot be resolved could still yield a variant symbol for lowering/flow observation even though the case is not in that subject's namespace.

**Unsure:** How often `has_exact_declaration_case_membership_meaning` passes without resolvable subject type (read dp:1038-1126 only partially).

### A0430

1. Read `implies_clause` at `domains_and_predicates.rs:5031-5057` and `clauses` at `6221-6257` (read).
2. Read local `compare` at `6088-6094` (read).
3. Read `BinaryOperator::complement` at `typed-trees/src/values.rs:4779-4788` (read).
4. Sampled all `compare(` call sites in `collection_predicates::implication` module: dp:4903-4940, 5515-5546, 5366-5370, 6222-6246 use only equality/ordering operators (read); no `compare(Add, …)` found in that module (grep).

**Input on accepting branch:** Would be a `Comparison` literal whose `operator` has `complement() == None`, negated via `unwrap_or(operator)` at dp:5043 or dp:6250-6253.

**Verdict:** CLOSED — Inferred from read call sites: implication `Comparison` values are only constructed with operators that have complements, so the `unwrap_or` branch is not reachable on stored paths.

**Unsure:** `exploration` submodule `compare` at dp:2664 (different type) not exhaustively cross-checked with implication `Comparison` (partial read only).

### A0431

1. Read `closed_integer_predicate_value` at `domains_and_predicates.rs:6575-6622` (read); entry matches skip when landing absent (read dp:6605-6612).
2. Search `closed_integer_predicate_value` in `omega-rust`: 2 matches (`type_references_and_wire.rs:412`).
3. Read `evaluate` integer branch at `type_references_and_wire.rs:343-350` (read) showing `None` carrier for integer literals.

**Input on accepting branch:** Predicate application argument that is an integer expression without landing metadata, paired with `(value, None)` in the values slice, where parameter expects a fixed integer primitive.

**Verdict:** REACHABLE — An unlanded literal that does not match the declared primitive would not be rejected by the landing block yet can still produce a decided predicate truth if within numeric bounds.

**Unsure:** Whether earlier landing validation always rejects such literals before wire evaluation.

### A0432

1. Read `handle_assignment` at `domains_and_predicates.rs:7355-7504` and early return at 7502-7504 (read); entry description matches silent skip (read).
2. Search `handle_assignment(` in `omega-rust`: 2 matches (definition and `walk_state` call at 13409; read).
3. Read `walk_state` signature and call at `13326-13423` (read).
4. Read `validate_default_domain_writes` and `judge_contract_entailment` at `program_validation.rs:4668-4679` (read).
5. Read `CallFrameResolver::new` at `calls.rs:15337-15348` and `CallFrameTables::build` at `14075-14093` (read).
6. Read per-machine `MachineSymbols::build` in main validation at `program_validation.rs:4191` (read).

**Input on accepting branch:** `call_frames == None` passed into `walk_state` / `handle_assignment`, skipping origin-based field-store tracking after `element_store` (read dp:7487-7504).

**Verdict:** CLOSED — For program acceptance, `judge_contract_entailment` runs after `validate`, which already builds machine symbols at pv:4191; `CallFrameTables::build` uses the same `MachineSymbols::build` path (calls.rs:14086-14089, read), so accepted programs should have `call_frames` `Some` when default-domain assignment origins run.

**Unsure:** Alternate entry points that invoke `validate_default_domain_writes` with `None` without prior `validate` (only pv:4675 found in grep).

### What I did not read

- Full bodies of all 46 `has_builtin_binary_expression_meaning` production call sites.
- `contract_entailment.rs` callers of `sealed_float_comparison_declaration_for_operands` beyond line header.
- Entire `exact_case_membership_meaning` selection loop (dp:1103-1125) and all case-membership lowering paths.
- `exploration` submodule predicate evaluation (`domains_and_predicates.rs:1664-2810`) beyond sampled `compare` sites.
- `handle_field_store` and remainder of origin loop after dp:7560.

SLICE COMPLETE
