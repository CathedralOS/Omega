# slice_0127

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0976 | S02/lowering.rs:2057 | REACHABLE | Source-free domain family lookup keys only on path text, so distinct packages can share one closed-index family string. |
| A0977 | S02/lowering_machine.rs:1859 | UNDECIDED | Non-range `ranking_range` syntax becomes `RankRange::default()`; whether that wrongly accepts a program was not traced through validation. |
| A0978 | S02/lowering_statement.rs:401 | UNDECIDED | Call-argument lowering skips indexed-read hoisting; digest marks this as a contract gap, not a traced unsound accept. |
| A0979 | S02/lowering_statement.rs:1113 | REACHABLE | Assignment of two bare names drops the statement when either side is a current evidence term, even if the other is an ordinary local. |
| A0980 | S02/preparation.rs:1267 | REACHABLE | Several preparation paths skip expansion, defaults, or package disambiguation instead of rejecting. |
| A0981 | S02/preparation_const_evaluation.rs:438 | REACHABLE | Closed generic prep and non-ProveSyntax admission modes skip work without rejecting; downstream may still accept. |
| A0982 | S02/preparation_generic_data.rs:699 | UNDECIDED | Unsupported type-reference shapes yield `None` identity; whether leaving the application untouched accepts invalid programs was not traced. |
| A0983 | S02/preparation_generic_uses.rs:245 | UNDECIDED | Machine signatures are keyed by name text across modules; whether a missed relabel causes wrongful acceptance was not traced. |

## A0976

**Cited site (read):** `omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/lowering.rs:2057` — in `lower_item_with_exposure`, `Item::Capability(_) | Item::Package(_) => {}` emits no symbol-resolved declaration rows.

**Callers (search `\blower_item\b` in `omega-rust`, count mode):** 4 matches — definitions/calls in `lowering.rs` (3) and `resolution.rs:2213` via `lowering::lower_items` (1). **read**

**Domain identity (read):** Digest cites `S02/preparation.rs:578-587`; at this checkout the matching pooled-domain comment is `preparation.rs:576-581`. With `selection: None`, `canonicalize_closed_domain_indices` builds `ClosedDomainFamily.path` at `preparation_const_evaluation.rs:1277-1279` (module-qualified name or bare declaration name) and resolves uses at `1371-1381` with `families.iter().find(|family| family.path == name)` — no package key on the lookup string. Duplicate-family guard at `1282-1287` uses `selection.is_none_or(|selection| selection.same_checked_package(...))`, so without a source map the guard is path-only inside one syntax forest. **read**

**Input (inferred from digest):** A probe or merged syntax forest with no `SourceMap`, two packages each declaring the same indexed domain family name reachable under the same module path spelling, and a type position that applies that family by bare path text — the second declaration’s indices can be canonicalized against the first family’s identity string.

**Capability/Package (UNDECIDED within row):** Digest secondary cites `lowering.rs:1119-1121` and `:1144` (match-subject lowering); not read for whether missing Capability/Package lowering accepts invalid programs.

**Verdict:** **REACHABLE** for the domain identity string collision; Capability/Package silent lowering not shown to close acceptance either way.

## A0977

**Cited site (read):** `lowering_machine.rs:1855-1860` in `build_termination_plan` — when `machine.ranking_range` is valid but `syntax_trees.expressions.expression(machine.ranking_range)` is not `ExpressionNode::Range`, the arm returns `language_semantics::RankRange::default()`.

**Enclosing function (read):** `build_termination_plan` `1812-1874` fills `MachineTerminationPlan.implementation_witness` including optional `rank_range`.

**Callers (search `\bbuild_termination_plan\b` in `omega-rust`, count mode):** 2 matches — definition and call at `lowering_machine.rs:1486-1487` during machine lowering. **read**

**Downstream (read, partial):** `contract_entailment.rs:26443-26446` compares custody `rank_range` to witness/member range when checking ranking calls; full path from a silently defaulted witness to compile acceptance was not followed.

**Input:** A machine with non-empty ranking subjects and a `ranking_range` expression that parses but is not a range literal (for example a bare name or integer).

**Verdict:** **UNDECIDED** — silent default is **read**; whether validation still rejects the program is **not traced**.

## A0978

**Cited site (read):** `lowering_statement.rs:401-425` — `rewrite_children` hoists `Binary`, `Unary`, `Cast`, `Membership`, `Range`; `ExpressionNode::Borrow(_) => {}` leaves borrows untouched; `ExpressionNode::Indexed(_) => { capture_address_operands(...) }` without general operand hoisting.

**Header contract (read):** Module comment `lowering_statement.rs:4-8` states runtime-indexed operand reads are hoisted into synthetic locals.

**Call path (read):** `StatementNode::Call` at `1159-1174` lowers arguments via `lower_statement_expressions` (`1575-1599`), which calls `lower_private_expression_into_table` per argument without `hoist_operand_indexed_reads`. Indexed hoisting on calls appears only on transition targets at `1629-1642`, not ordinary call statements.

**Callers of hoisting (read):** `hoist_operand_indexed_reads` used from assignment values (`1145-1149`), returns, transitions, etc.

**Digest:** area_07 labels this “a possible gap” in the stated contract, not a soundness accept.

**Verdict:** **UNDECIDED** — hoisting omission is **read**; whether any program that should be rejected is accepted because indexed reads remain nested in call arguments, struct/array literals, match arms, or member receivers was **not traced** to stage 03+ acceptance.

## A0979

**Cited site (read):** `lowering_statement.rs:1112-1139` — for `StatementNode::Assignment`, if `evidence_forwarding_names` returns `Some`, an `EvidenceForwarding` row is pushed and the function returns `Ok(Vec::new())`, dropping the assignment statement.

**Helper (read):** `evidence_forwarding_names` `1529-1541` requires two bare `Name` paths and succeeds when **either** target or source spelling is in `lowerer.current_evidence_term_names`.

**Evidence names (read):** `lowerer.current_evidence_term_names` is populated during machine/state lowering (`lowering_machine.rs:1383`, cleared `1426`, scoped `1727-1737`).

**Callers (search `\bevidence_forwarding_names\b` in `omega-rust`, count mode):** 2 matches — definition and single use in assignment lowering. **read**

**Binding (read):** `selection.rs:3033-3086` `bind_evidence_forwarding_owners` fills symbols on forwardings but does not recreate dropped assignments.

**Wrong program (digest, quoted):** Machine state with evidence term `evidence` in scope and assignment `x = evidence` where `x` is an ordinary local — only `evidence` is an evidence term, yet the assignment is removed.

**Verdict:** **REACHABLE**

## A0980

**Sites read:**

1. `preparation.rs:1267-1277` — `constructor_prefix_is_value` calls `symbols::constructor_prefix_is_value` and `.unwrap_or(true)` on resolver `Err`, so ambiguous constructor prefix errors count as value. **read**
2. `preparation.rs:1008-1016` — `generic_parameter_roles` returns `None` when header lookup returns `Err(())` (ambiguous duplicate headers), skipping role discovery. **read**
3. `preparation.rs:5116-5193` — alias-expansion `item` match ends with `_ => {}` for items other than those listed; `Item::Domain` not handled (Domain alias expansion skipped). **read**
4. `preparation.rs:2835-2837` — `collect_requirement_instances` `continue`s when `traits.get(&requirement.symbol)` is missing, skipping nested requirement default synthesis. **read**
5. `preparation.rs:2186-2258` — `target_families::normalize` with `sources: Option<&SourceMap>`; at `2209-2210` `file` is `None` when `sources` is `None`; `compare_family` `2151-2177` orders by `package_identity` then falls back to `package_root` paths when package is absent, merging members across packages in one forest at `2239-2258`. **read**

**Callers:** `constructor_prefix_is_value` — 7 matches in `omega-rust` (symbols, preparation, preparation_generic_uses). `collect_requirement_instances` — 3 matches in `preparation.rs`. `target_families::normalize` — 2 matches (`preparation.rs:5372`, `resolution.rs`). **read counts**

**Inputs:** (1) Ambiguous `Const::` prefix in a qualified constructor path treated as value reference. (2) Ambiguous generic head → no telescope roles → nested alias not expanded. (3) Domain declaration relying on alias expansion in this pass. (4) Conformance whose trait map lacks a required trait symbol → missing default requirements. (5) Target-family normalization without source map → cross-package machines grouped by module/name text.

**Verdict:** **REACHABLE** (multiple independent skip paths; digest-checked examples).

## A0981

**`prepare_closed_application` (read):** `preparation_const_evaluation.rs:429-437` documents skip-never-reject; returns `Ok(None)` for non-generic bases (`454-455`), wrong arity (`466-469`), const-operator shapes (`471-482`), call const expressions (`630-634`), failed canonicalization (`695-712`), etc.

**Callers (search `\bprepare_closed_application\b` in `omega-rust`, count mode):** 5 matches — definition, re-export, and production call `preparation_generic_data.rs:3280-3291` (`continue` on `None`). **read**

**Domain indices (read):** `canonicalize_closed_domain_indices` at `1371-1384` skips unknown families with `continue`; `TypeReferenceNode::ConstExpression` branch `1503-1507` `continue`s when expression needs operator selection or contains names — leaves argument unrewritten.

**Admission modes (read):** `ConstPredicateAdmission` `5372-5377`; `canonicalize_const_expression_with_admission` `5853-5860` and `6311-6313` call `prove_const_leaf_domain_constraints` / `prove_const_data_predicates` only when `admission == ProveSyntax`.

**Callers of admission (read):** `preparation_generic_data.rs:4546-4550` uses `PendingDestination`; `4623-4627` uses `CheckedDestination` — both bypass predicate proof gates present under `ProveSyntax`.

**Example input (digest):** Generic application `Base<...>` whose const argument is a call expression → `prepare_closed_application` returns `Ok(None)` and synthesis loop continues without closing the application at stage 02.

**Verdict:** **REACHABLE** for skip-without-reject and non-ProveSyntax predicate omission; full compile acceptance for every shape **inferred** not exhaustively traced.

## A0982

**Cited site (read):** `preparation_generic_data.rs:699` — final `_ => None` arm of `closed_argument_identity` for type-reference shapes outside the handled `match` arms.

**Enclosing function (read):** `closed_argument_identity` `475-700` returns `Option<ClosedArgumentIdentity>`; `None` means the argument is not closed for identity purposes.

**Callers (search `\bclosed_argument_identity\b` in `omega-rust`, count mode):** 46 matches across preparation_generic_data, preparation_const_evaluation, preparation_type_equations, preparation_machine_equations, preparation_generic_uses, constant.rs. **read count**

**Upstream:** `prepare_closed_application` treats failed canonicalization/`None` slugs as `Ok(None)` (`695-706`, `708-712`) — application left as authored syntax.

**Verdict:** **UNDECIDED** — “not closed” and leave-alone behavior is **read**; whether a program that should be rejected at stage 02 is still accepted later was **not traced**.

## A0983

**Cited site (read):** `preparation_generic_uses.rs:245-303` in `relabel_closed_data_uses_in_exact_calls_and_returns` — `signatures` map keyed by full machine name string; attached machines keyed by `(attached_data_string, method_segment)`.

**Enclosing function (read):** Full function `239-~400+` builds signature maps from all root machines, then relabels constructor uses in concrete machine states from `selected_machine_state_handles`.

**Callers (search `\brelabel_closed_data_uses_in_exact_calls_and_returns\b` in `omega-rust`, count mode):** 4 matches — definition, re-export, call `preparation_generic_data.rs:3407-3411`. **read**

**`concrete_machine_declarations` (read):** `1122-1141` excludes attached machines when attached data name matches a generic data template’s name string — ordinary attached machines on a same-spelled generic template are dropped from relabel inventory.

**Digest note:** Missed relabel may leave template spelling for ordinary resolver; rejection not traced.

**Verdict:** **UNDECIDED**

## What I did not read

- Full `lower_item_with_exposure` arms beyond `Capability`/`Package` and whether stage 02 symbols ingest package/capability syntax trees.
- Digest secondary sites `S02/lowering.rs:1119-1121` and `:1144` for A0976.
- Stage 03+ paths for indexed-read hoisting gaps (A0978), `closed_argument_identity` failures (A0982), and relabel misses (A0983).
- Corpus/tests/probes cited as “inferred reach” for A0976 domain collision.
- Entirety of `rewrite_children`, `lower_private_expression_into_table`, and validation ranking pipeline beyond cited lines.

SLICE COMPLETE
