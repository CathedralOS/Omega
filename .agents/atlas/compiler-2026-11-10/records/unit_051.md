# Unit 51 notes

Files covered by the unit (paths relative to checkout `<checkout>/`; all under `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/`):
1. `checks_contracts_exits.rs` (1-9918) - read in full.
2. `checks_contracts_prover.rs` (1-8071) - ONLY lines 1-1700 read; 1701-8071 NOT read.
3. `checks_contracts_writes.rs` (1-2313) - read in full.

Unit total 20302 lines. Read in full: 13931. Not read: 6371 (checks_contracts_prover.rs 1701-8071). Nothing below describes the unread range.

---------------------------------------------------------------------
## File 1: checks_contracts_exits.rs (1-9918, read in full)

### Purpose
Checks at every normal return of a machine: each `ensures` clause, the declared domain of the result (scalar and structural fields), and the domains of exclusive borrowed referents handed back, must be proved from the facts live at that exit (header 1-32). It is one file made of deeply nested inline modules; the driver is `contracts::check_flow_call_contracts` in `checks_contracts.rs` (calls at `checks_contracts.rs:11592` import, `:11634` `CyclicHeaderInvariants::new()`, `:11635` `BoundaryRefutations::new`, `:11812` `check_exit_ensures`; grepped).

### Shape (main types and entry points)
- Top-level entries: `check_exit_ensures` (9641), `live_contexts_prove_scalar_postcondition` (9582), `collection_premises_prove_exit` (9559); re-exports at 9542-9551 (`BoundaryRefutations`, `CyclicHeaderInvariants`, `check_exclusive_referent_field_domains`, `check_result_field_domains`, `check_scalar_result_domains`, `check_state_transfer_result_domains`, `exclusive_referent_type`, `is_readable_mutable_reference`, `value_provable_domain`, and `exact_scalar_membership` at 4763).
- `mod bindings` (42-189): `proves` (60-177), inherited-contract aware exit proof; `same_type` (179).
- `mod cases` (190-555): `CaseObservation` (302): `for_requirement` 212, `for_exit` 259, `observe` 328, `result_path_is_inactive` 318, `returned_case` 351, `live_case` 503, `stable_place` 539.
- `mod cyclic_headers` (556-2627): loop-header-invariant prover. Submodules `completions` (616-1377; `proves` 971, `proves_value` 1241, `return_value` 1304, inner `referents::ReferentGoal` 675-968), `components` (1378-1632; `admitted` 1400, `discover` 1483, `proves_component` 1559), `demands` (1633-1765; `TRANSPORT_WORK=16_384` 1649, `SUBSTITUTION_DEPTH=64` 1650). Types `CyclicHeaderInvariants` (1774; per-machine cache, `proves` 1799), `AdmittedGuarantee` 1785, `Candidate` 1856, `Arrival` 1866, `RosterParameter` 1873, `Header` 1880 (`new` 1915, `arrivals` 2074, `candidates` 2020, `caller_atoms` 2291, `term_reference` 2514, `comparison_is_admitted` 2436), `fixed_integer` 2615.
- `mod result_domains` (2628-4762): `proofs` (2645-3484), `scalar_exits` (3485-3947), `field_domains` (3961-4628), plus `exact_integer_reference` 4630 and `domain_predicate_meaning` 4648.
- `mod scalars` (4764-9541): `admitted_domains::BoundaryRefutations` (4799-4936), `deferred_recipes` (4938-5358), `calls` (5363-5831), `predicates` (5832-6097), `result_bounds` (6098-7701 with `guarded_returns` 6114-6481, `reconstruction` 6482-7154, `proves_bounds` 7157, `current_bounds_comparison` 7175, `result_bounds_comparison` 7264, `result_range_condition` 7475, `interval_comparison` 7627, `exact_integer_carrier` 7665), `result_fields` (7702-8770: `descriptor_lengths` 7740-7859, `proves_return_arithmetic` 7892, `result_projection` 8037, `result_field_value` 8072, `result_collection_lengths` 8297, `bind_current_scalar` 8512, `preserved_contract_subject` 8636), `static_calls` (8771-8980), then `proves` (8982-9071), `ExitScalars` struct (9073), helpers (9094-9540).

### Inputs and outputs
Inputs: `&dyn validation::CheckingInput`, `&CheckFacts` (`flow`, `semantic`, `proof`, `operators`, `values`, `termination`), a `FlowExitFact`, optional `validation::CallFrameResolver`. Outputs: boolean verdicts; `Diagnostic::error(format!(...))` pushed (examples 3584, 3698, 3702, 3819, 4337, 4615, 4928, 9901); `Vec<CheckedScalarReturnQualification>` decisions (3660); conditional-proof debts `ConstantRecipeObligation` (4955) and `IntegerRangeObligation` (5008, 7475) appended by `check_exit_ensures` (9866, 9880).

### How it decides (keys)
- `check_exit_ensures` (9641-9918): for each fact in the exit's ensures contexts, skips (continues) when: the contract has `qualification_authorization` and owner is this machine/state (9676-9688, comment-free; "owner-authorized boundary result" in header 14-16); `entailment.expressions.contains(&expression)` (9690-9698); a proposition-application fact whose Machine-owned Ensures contract has an `evidence_term` (9699-9715). Otherwise `satisfied` is an OR over: `is_proved()`, `result_domains::proves_result_domain`, `content_preservation::proves_exit`, `proves_composed_exit`, `integer_embeddings::proves_embedded_literal_exit`, `entailment::transparent_proposition_proves_exit`, `collection_premises_prove_exit`, an authorized establishment route, then `|| cyclic_headers.proves` (9777-9852). Everything but the cyclic route is gated by `missing_origins.is_empty()` (9777); the cyclic route is NOT inside that conjunction (9852). Keyed on contract handle / expression handle / symbol identity, not names.
- `CyclicHeaderInvariants::proves` (1799): keyed on `(machine symbol, state symbol, statement_index, transition_target, contract handle)` membership in the `admitted` vec (1822-1832), then on `completions::proves` for the contract handle (1840).
- Header atoms use string `identity`s built by `format!` of debug-printed handles: `"\0{scope}:{:?}"` (2237), `"completion:{:?}:{returned:?}"` (1368), `"return-place:{subject:?}"` (2877, 3315). Compared by `ScopedArithmeticValue` equality (1709); handle debug text is the identity (read).
- Const-qualification machines are recognised by machine NAME: `machine_name(...).starts_with("@const-qualification-")` (3690-3692) chooses the diagnostic wording only.
- Field lookup by spelling: `field_of_data` falls back to `field.name.as_str() == member.member.as_str()` when `member.member_symbol` is invalid (8394-8398). Grepped `member\.member\.as_str\(\)` across the crate: 37 hits in 20 files (not read except this one); same spelling-compare pattern is common.
- Scalar contract proofs are keyed on place identity (`place_at`, `CanonicalPlace`), origin pairs `(entry_parameter, state_parameter)` from `exit_parameter_origins` (9094-9109), and statement ordinal.
- Work bounds: `TRANSPORT_WORK` 16_384 (1649); `EXPANSION_LIMIT=8` (6514); `4096` literal budgets (5937, 6205); depth caps 64 (5119, 5233), 128 (3149, 3270, 4656, 7670, 8324), 8 (3891).

### Cases by shape (quoted/cited)
- Cyclic header doc: "Saturating and Trapping stay outside the language" (594-595); `fixed_integer` accepts only Exact/Wrapping (2624). Residue terms only under `==` (2475-2484, doc 2417-2423).
- `ReferentGoal::new` (683) accepts only guarantees with builtin operators (693), no `Call` or `result` leaf (710-723), exclusive-reference parameters whose paths are only `Field` segments (751-756) and fixed-integer leaves (768), stable reference binding (771).
- `completions::proves` call forwarding: callee must be the same machine unless `result_only`, and fixed-integer return types must match (1180-1186).
- `Header::arrivals` returns `None` (admit nothing) on non-empty `evidence_arguments` (2108) or any non-admitted argument term (2112-2116).
- `closed_domain_indices` only decodes `Integer` and `Boolean` canonical const values (3112-3116).
- `exact_computations` (3183): only Binary/Unary(LogicalNot)/Name/Integer/Boolean.
- `check_state_transfer_result_domains` (3753): only Ordinary transitions with no authored expression, only same-machine transfers (3768-3796).
- `closed_call_value` (5671): fixed-integer result, callee with no `const` parameters (5627), no receiver/evidence, nominal-route-only (5570-5574).
- `reconstruction` (6482-7154): pattern `(dividend / divisor) * divisor` only (6796, 6820); nonnegative dividend and divisor >= 1 (6727-6729); doc 6484-6498 says "Wrapping or saturating carriers ... stay on the interval path". Linear forms over `i64` with `checked_*` (6646-6672).
- `result_bound_operand` (7607): doc 7601-7605 "Longer member paths stay unproven."
- `static_calls` (8771): only integer/bool result types (8845-8849); guarantees of form `result == <const param | literal>` (8855-8892).
- `result_collection_lengths` family (8297-8493): `result.field.len` compared with an integer literal only (8171-8173); every match arm must satisfy.
- `returned_descriptor_length` (7758): sources must be owned immutable non-const parameters, no recasts/representation/image storage (7799-7822).

### Same job elsewhere
- `exit_return_expression` / `exit_return_call` / `completed_return_operand` come from `contracts::return_values` (imported at 209, 1768, 7722-7724; defined outside unit; not read).
- Place-segment "stable" test appears twice in this file: `CaseObservation::stable_place` (539-553) and `stable_segments` (9533-9540); the former also allows `FixedIndex` and checks root symbol validity; both accept Field/Case/FixedIndex. Read.
- Type-reference unwrapping loops over `Constrained`/`Reference`: `result_domain_type` (4354), `is_readable_mutable_reference` (4391), `exclusive_referent_type` (4415), `field_of_type` (8352), `exact_integer_carrier` (7665): five similar loops in this file (read).
- Interval/bounds comparison: `interval_comparison` (7627) vs `ScalarPredicateJudgment` and `StrictArithmeticImplicationJudgment` use elsewhere; "suspected" parallel scalar-judgment vocabularies, not traced.
- Domain-membership-by-place lookup `exact_scalar_membership` (3906) vs `assigned_values::carried_membership_at` in `checks_contracts_prover.rs:171` (read both): both scan context facts for a `domain_membership()` payload at a covering place; the first requires an exact `(domain, instance)` match and `segments_cover_subject`, the second takes a `satisfies` closure and `covers_canonical_subject` (prover.rs:198).
- `scalar_value`/`evaluate_scalar` come from `prover::scalars` (re-export of `checked_trees::facts::scalar_predicates`).

### Unused or one-caller code (grepped)
- `exact_scalar_membership`: 14 occurrences in 5 files (Grep count over `omega-rust`): exits.rs 6 (definition + re-exports + internal uses), writes.rs 4, `checks_multiplicity.rs` 2, `checks_contracts_calls.rs` 1, `checks_contracts.rs` 1. Used outside its file.
- `value_provable_domain` | `is_readable_mutable_reference`: 12 occurrences in 6 files (exits.rs 4, contracts.rs 1, content 2, flow_call_phases 2, flow_retained_borrows 1, flow_reference_places 2).
- `exclusive_referent_field_requirements` | `primitive_parameters(` | `scalar_result_domains(`: 9 occurrences, 8 in exits.rs and 1 in `checks_contracts.rs`.
- `check_exit_ensures`: one external call site `checks_contracts.rs:11812`. `CyclicHeaderInvariants::new`: one, `checks_contracts.rs:11634`. `BoundaryRefutations::new`: one, `checks_contracts.rs:11635`. `live_contexts_prove_scalar_postcondition`: its one non-definition use is `check_exit_ensures` (9732); the doc at 9577-9581 says "Ordinary exit checking and package exit evidence share these routes" - grep found no second caller of this name in `omega-rust`, so the "package exit evidence" sharing is not visible by this name (other callers may use a differently named path; not traced).
- `pub(crate) mod calls` (5363) is `pub(crate)` inside a private module chain; its items are `pub(super)` or inherent on `ExitScalars`. Not grepped further.
- Comments claiming a single shared path: "`scalar_result_domains` ... Shared by the exit census and its consumer" (3500-3503): grep shows only 1 use outside exits.rs (`checks_contracts.rs`). "These are exactly the rows the state-transition arrival check re-proves (checks/contracts/arrivals.rs); no second obligation vocabulary exists." (4441-4443): arrivals.rs not read; not verified.

### Stated limits (quotes < 20 words)
- "Candidate discovery remains incomplete" (577-578).
- "Saturating and Trapping stay outside the language." (594-595, 2613-2614).
- "Proof-only predicate roots remain outside this storage judgment, as before" (3975-3976).
- "Longer member paths stay unproven." (7604-7605).
- "Unknown dispatch, missing flow and an unproved producer refuse the whole closure." (622-623).
- "Unsupported substitutions and any failed arrival retire the whole component proposal." (576-577).
- "unknown effects stand down" (9557-9558).
- "Disagreeing extents do not become one value." (8173).

### Fail-open risks
- Skips in `check_exit_ensures` that accept without a proof at that site: owner-authorized boundary result (9687-9689), entailment-proved expression (9696), evidence-term proposition application (9713). "stated" by header 14-16 ("skips an owner-authorized boundary result, which is established at the admitted crossing"); the entailment and evidence-term skips depend on `entailment.expressions` and `contract.evidence_term` set elsewhere (not traced: "open").
- `check_exit_ensures` first line (9655-9659): `static_type_context_is_unreachable(returned expression)` returns with no check. Wrong program that would get through: an exit the type context calls unreachable. "open" (defined outside the unit).
- `check_scalar_result_domains` (3551): `domains.is_empty()` return (3563); vacuous domain `continue` (3592-3596). A vacuous domain imposes nothing: "stated" only by the name `domain_is_vacuous`.
- `check_result_field_domains` (4000): returns without checking when `!returned.is_valid()` (4042) or when `paths.is_empty()` (4037). Open: not traced which exits have an invalid returned expression (e.g. exits without a value).
- `check_state_transfer_result_domains` (3753): returns early for authored calls (3759), non-ordinary transitions (3768-3773), non-local transfers (3794); `continue`s when the entry state promises the domain (3803) or the call result has it (3815). "stated" (3744-3751): "The common machine promise is checked at leaf exits".
- `check_exclusive_referent_field_domains` (4530): `continue` on a requirement whose case path is inactive (4568-4590) or whose payload is not a `DomainMembership` (4556-4566). "stated" by doc 313-317 style ("An absent payload owes no field membership").
- `domain_predicate_meaning`/`value_satisfies_bound_domain_predicates_in_contexts` analog in writes (see file 3) returns `true` for empty predicate lists.
- `establishes_scalar_predicates_visited` (2752): an empty `domain.facts` leaves `judgment = Proven` (2887, loop over empty span). A domain with no facts therefore proves; this matches the vacuous-domain rule. Traced locally only.
- `BoundaryRefutations::check` (4831) only adds diagnostics; silent returns (4839-4850) mean "no refutation", not acceptance of anything it would have rejected.
- `cyclic_headers`: every error path returns `false`/`None`/empty `admitted` (e.g. 1419, 1426). None seen that accepts on `None`.
- `Header::arrivals` (2079-2088): `find_call_site(...)` returning `None` does `continue` (2086), and a non-`TransitionNamed` site `continue`s (2096). An arrival whose call site cannot be found is therefore NOT counted as an arrival, so a header could be admitted without checking that arrival. Wrong program: a transition into the header whose site lookup fails. Open (not traced; `find_call_site` is outside the unit).
- `proves_component` (1559): `arrivals[destination].is_empty() && header != entry` returns false (1569) - closed.

---------------------------------------------------------------------
## File 3: checks_contracts_writes.rs (1-2313, read in full)

### Purpose
Write-enforcement for domain-qualified fields and constructions (header 1-24): an assignment `self.f = X`, a constructed `T { f: X }`, an array literal element, a local initializer with a domain-annotated type, and an `as ... in D` cast must establish membership of the value in the declared domain. Successful judgments are recorded as `CheckedValueQualification` rows so later stages can see which occurrence was proven.

### Shape
- `check_domain_field_writes` (1580): per state; builds `ConstructionQualification` rows (1559; `site()` 1571) for structural values, walks statements with `ConstructionWalk` (1944; `scan` 1962), checks local initializers (1722-1819), assignments (1820-1872), and borrowed-`&[u8]` concatenation stores (1874-1909).
- `mod constructor_memberships` (57-335): `ConstructorMemberships` (91; `new` 96, `array_element_domains` 114, `at` 147; captures sorted by `(state, statement, expression)` handle triple (110, 168-180)); `ConstructionWalk::check_constructor_membership` (190-293); `live_bounds_satisfy_domain` (302).
- `mod domain_implications` (337-892): `value_proves_qualification` (362), `value_semantic_contexts` (406), `completed_operand` (440), `value_case_path_is_selected` (486), `value_proves_qualification_in_contexts` (515), `_with_domain_stack` (545), `value_proves_domain_in_contexts` (645), `recast_view_source_expression` (737), `value_result_type_carries_domain` (782), `completed_owned_call_carries_domain` (819), `result_type_carries_domain` (873).
- `mod domain_tests` (893-1025): `validate` (907), `retained` (959): executable domain tests on routed domains need retained provenance.
- `mod member_domains` (1026-1548): `initializer_satisfies_predicate_domain` (1049), `value_satisfies_domain_predicates_in_contexts` (1111), `..._instance_...` (1140), `value_satisfies_bound_domain_predicates_in_contexts` (1170), `captured_value_satisfies_indexed_domain` (1256), `predicate_expression_holds_on_value` (1327), `literal_member_value` (1417), `member_subject_satisfies_domain` (1446).
- Free fns: `statement_root_expressions` (1918), `construction_field_domain_identities` (2120), `judge_value_qualification_cast` (2161), `qualification_cast_refusal` (2248), `check_routed_cast_coverage` (2269), `record_qualification` (2293).

### Inputs and outputs
Reads `CheckFacts` (flow operands, semantic contexts, operators, qualifications), the `CheckingInput` program, `CallFrameResolver`. Mutates `decisions: &mut Arena<CheckedValueQualification>` (appended in `record_qualification` 2304) and `diagnostics`. Callers (grepped): `checks_contracts.rs:11749` (`check_domain_field_writes`) and `:11852` (`check_routed_cast_coverage`), one each.

### How it decides
- Membership of a value: keyed on `(domain symbol, SemanticDomainId)` identity and live fact contexts at the value's place (`place_at`), plus `place_cases_are_selected` (558-570).
- Provenance-gated domains (`domain_requires_provenance`, 571) need live membership (`exact_scalar_membership`, 573) or a completed owned call carrying the domain (577) or an instance-predicate check only when no `establishment_routes` and no alias (589-594). Predicate-only domains additionally accept result-type carry (609), symbol-only proofs (645), string-literal grant (669), concatenation of two proven halves for concat-preserving domains (677-705), recast source (707), and domain-at-place (721-724).
- `result_type_carries_domain` (873-891): symbol-only domains compare via `domain_membership_implies`; otherwise exact `source == domain_symbol && source_identity == identity` (885-887).
- Construction field domains (2120-2146): looks the field up through `construction_field_type(program, data_definition, case_name, field_name)` with NAME strings: `field.name.as_str()` and `case_name.as_str()` passed at 1999-2000 (source-spelling key; the lookup itself is outside the unit). Excludes generic data (`type_parameters.count() > 0` returns empty, 2129-2131).
- Cast qualification (2161-2245): early `return`s when `cast.semantic_domain.is_empty()`, when a predicate-only scalar-target domain (validation owns it), or when the domain is vacuous (2176-2184).
- Assignment targets (1820-1872): `exact_declared_domain_interval(...).is_some()` skips the value proof (1826-1828); then `value_proves_qualification` or, for predicate-only domains, `initializer_satisfies_predicate_domain`.
- Diagnostic target label uses `display_name(assignment.target)` (1851, 1877): spelling, output only.
- `check_routed_cast_coverage` (2269): any `Cast` expression anywhere in `expression_entries()` with a routed domain and no decision row for that expression gets a refusal (2278-2288). Keyed on expression handle.

### Cases by shape
- `scan` (1962-2084) is an exhaustive match over expression node kinds: SavedSubject, Atomic, Match, StructLiteral, ArrayLiteral, Binary, Cast, Call, Indexed, Member, Borrow, Range, Unary, TypeExpression/literals; no wildcard (read). Struct literals of generic data are skipped (2129).
- `statement_root_expressions` (1918): doc "The exhaustive set keeps the construction walk in step with the statement shapes" (1914-1916). `Transition` yields nothing (1931) but transitions are handled separately in `check_domain_field_writes` (1667-1706).
- Construction domains accepted only for `ScalarArray`, `FixedArray`, `Record`, `Case` structural values (1603-1612).
- Borrowed-bytes concatenation diagnostic only for `BinaryOperator::Add` with builtin operator on `is_borrowed_byte_slice` target (1893-1909).
- Indexed-domain captured check only for fixed-integer carriers, literal or captured scalar (1277-1309).

### Same job elsewhere
- `member_domains::value_satisfies_domain_instance_predicates_in_contexts` (1140) vs `exits::result_domains::proofs::establishes_scalar_predicates` (exits.rs 2705): both prove a domain's predicate facts on a value/returned value; the exits one uses the arithmetic engine plus `evaluate_scalar` and recurses on membership facts with a `visited` list; the writes one uses an `active` list and `completed_value_proves_bound_domain_predicate`. Both call `completed_value_proves_domain_predicate`-family in `validation`. Read both.
- `domain_implications::value_proves_qualification_with_domain_stack` (545) overlaps `exits::check_result_field_domains` root checks (exits.rs 4136-4174), which call `writes::value_proves_qualification_in_contexts` (exits.rs 4164, `value_satisfies_domain_predicates_in_contexts` 4143). Read both.
- `domain_is_vacuous` shortcut appears at writes 216-217, 1729, 2181 and exits 3592, 3808.

### Unused or one-caller code (grepped)
- `check_domain_field_writes`, `check_routed_cast_coverage`: one external call each (`checks_contracts.rs:11749`, `:11852`).
- `value_proves_qualification_in_contexts` is `pub(super)`; used by exits.rs (4164, 4143 via `value_satisfies_domain_predicates_in_contexts`).

### Stated limits
- "A row requests a judgment; it never supplies membership." (1555-1557).
- "indexed theories and aliases need their own exact instance substitution and cannot be discharged by this declaration-only reader" (1108-1109, paraphrase of a quoted comment).
- "Proof-only" / `ProofFact::Proposition(_) => false` (1232): propositions never discharge a domain predicate here.
- "Routed ... still require live provenance" (341-343).

### Fail-open risks
- `value_satisfies_bound_domain_predicates_in_contexts` (1170): `if program.proof_facts(domain).is_empty() { return true; }` (1190-1192) - a domain with no predicate facts and no alias is satisfied by any value. Wrong program that would get through: a predicate-free routed or unrouted domain; callers gate routed domains before reaching it (571-608 requires `establishment_routes.is_empty()`), predicate-free + unrouted means vacuous. Traced locally ("stated" by the vacuity rule at 216-217).
- `check_domain_field_writes` early return when `machine_state_by_symbols` is `None` (1590-1594): no checks for a state whose machine/state lookup fails. Open.
- `value_case_path_is_selected` (486-512): returns `true` when `place_at` gives `None` (493-495) or the place has no `Case` segment (496-502). A value with no place cannot carry a case path; "traced" locally.
- `judge_value_qualification_cast` (2161): returns with no diagnostic and no row when `domain_definition(domain_symbol)` is `None` (2172-2174) or for the three listed shapes (2176-2184); `check_routed_cast_coverage` (2269) then catches routed casts with no row. For a predicate-only scalar-target cast, nothing here checks; the comment (2155-2157) says "Scalar carriers keep the value-interval judgment in `validation`" (stated).
- `check_constructor_membership` (190): `publishes_formation_membership(...)` early return (205-214) skips the check when a formation membership was already published. Open (published by a different pass).
- `member_subject_satisfies_domain` (1446): when `capture.is_some() && child.is_none()` returns false (closed); when `capture` is `None` the literal member value is proved against the outer `contexts`. Fine.
- `live_bounds_satisfy_domain` (302): only claims proof when the interval is exactly contained in `exact_declared_domain_carrier_interval`; otherwise `false` (closed).

---------------------------------------------------------------------
## File 2: checks_contracts_prover.rs (1-8071; only 1-1700 read)

### Purpose (read, header 1-28)
Decides whether fact contexts prove a contract proposition. Three entries (header 1-28): `semantic_contexts_prove_boolean_expression` (used by `assembly`, `exits`), `call_entry_contexts_prove_boolean_contract_expression` (used by `calls`), `semantic_contexts_prove_contract_fact` (used by `arrivals`, `calls`, `exits`). The latter two are defined beyond line 1700 (NOT read); only their header doc was read.

### Shape (read part)
- `mod assigned_values` (35-592): `prove_domain` (71), `prove_domain_by_extent_enumeration` (94), `live_fact_at_place` (150), `carried_membership_at` (171), `covers_canonical_subject` (198), `carried_domain_membership_at_place` (218), `prove_domain_at_place` (233), `prove_indexed_domain_at_place` (267), `subslice_domain_at_place` (292), `byte_predicate_proven_at_place` (353), `AssignedValues` (326) with `domain` (431), `installed_case` (522), `boolean` (546).
- `pub(super) mod captured_values` (597-754): `scalar_at_path` (612), re-exported as `captured_scalar_at_path` (765).
- `mod scalars` (755-763): re-export of `checked_trees::checked_trees::facts::scalar_predicates::{ScalarValue, closed_boolean_value, evaluate, evaluate_for_atoms, has_builtin_operators}`; re-exports at 764-769.
- `mod booleans` (770-923): `semantic_context_proves_boolean_expression` (785), `semantic_context_proves_instantiated_boolean_expression` (830): direct proof, then structural recursion on Borrow / And (both) / Or (either), else domain-membership route.
- `mod place_predicates` (924-1315): `PlacePredicate` (932: `Bytes`, `Case`), `place_predicate` (937), `proven_for_capture` (956), `proven_for_call_projection` (1064), `trace_capture_refusal` (1130-1273; prints `CHECKSITE` lines when `trace_flags::check_sites()`), `proven` (1275).
- `pub(in crate::checks) mod call_guarantees` (1317-...): header 1318-1336; `arithmetic` submodule begins 1358 (`proves` 1402, `ExitArithmetic` 1445 with `value` 1476 and `proves` 1539, `at_exit` 1554, `append_store_hypotheses` 1617 ...). Read up to line 1700 (mid-`append_store_hypotheses`).

### Inputs and outputs (read part)
Reads `FactPlan` contexts, `CanonicalPlace`, `CheckFacts`, flow control rows (operands, copied_values, call_loan_resources). Outputs booleans / `Option<ScalarValue>`; the trace function only prints.

### How it decides (read part)
- Domain membership at a place: by declaration symbol (`domain_membership_implies`, 228) or, for indexed families, by exact `(domain symbol, SemanticDomainId)` (276-283, doc 251-265: "`BelowIndex<4>` and `BelowIndex<9>` share a symbol and have no variance relationship").
- Place coverage: `segments_cover_subject` (re-export of `crate::flow::segments_cover_subject`, 86): equal segments or containing `FixedRange`.
- Extent enumeration (94-142): proves every element index of a `FixedRange`/dynamic `Index` selector in `start..end`; for runtime `Index` takes the collection length from `closed_collection_length_at_place`; `start < end` required (136).
- `AssignedValues::domain` (431-516): string-literal grant, per-byte predicate, integer-domain membership by scalar value (Proven returns true, Refuted returns false, Unknown falls through), then `establishment_routes.is_empty()` required (484), then every `ProofFact` must hold; empty fact list returns `domain_is_vacuous` (487-490).
- `captured_values::scalar_at_path` (612): every captured value must give the same scalar (747-749).

### Cases by shape (read part)
- `prove_domain_by_extent_enumeration` (101-113): first `FixedRange` or non-range `Index` segment only.
- `subslice_domain_at_place` (292): strips exactly one trailing window selector (302-312), for domains `domain_is_subslice_preserving`.
- `proven_for_call_projection` (1064): only `PlacePredicate::Case` (1075-1077).
- `AssignedValues::domain` returns false for domains with type parameters or index arguments (456-458).

### Same job elsewhere
- See file 1 "Same job elsewhere": `carried_membership_at` (prover.rs 171) vs `exact_scalar_membership` (exits.rs 3906).
- `place_predicates::proven_for_capture` (956) and `captured_values::scalar_at_path` (612) both enumerate `captured_value_places` / `captured_observation_places` with an observation/operand pair loop (prover.rs 633-652 and 984-990; read).

### Unused or one-caller code
Not grepped for this file's public items (beyond `exact_scalar_membership` and friends in the file 1 section).

### Stated limits (read part)
- "Recorded facts supply membership, never the collection's extent" (91-92).
- "Per-byte predicates survive a window; UTF-8 and nonempty do not." (286).
- "Evidence and deferred payloads are never proved here." (22-23).
- "a prefix of writes can still leave later cells uninitialized" (92-93).

### Fail-open risks (read part)
- `AssignedValues::domain` returns `domain_is_vacuous(...)` when the domain has no proof facts (487-490); accepts a value in a predicate-free, unrouted domain. "stated" by the vacuity helper; not traced.
- `scalar_at_path` returns `None` rather than a value on disagreement (closed).
- `semantic_context_proves_*boolean_expression` (785, 830): `Or` is proved if either operand is proved (818, 893-895); `And` needs both (815, 890-892). Sound shape; no unknown accepted.
- No other fail-open seen in 1-1700.

---------------------------------------------------------------------
## Coverage
- `checks_contracts_exits.rs`: 1-9918 read in full (9918 lines).
- `checks_contracts_writes.rs`: 1-2313 read in full (2313 lines).
- `checks_contracts_prover.rs`: 1-1700 read in full (1700 lines); 1701-8071 NOT read (6371 lines).
- Total read in full: 13931. Not read: 6371. Skimmed: 0.
- Grep commands run (Grep tool, count/content over `omega-rust`): `exact_scalar_membership`; `value_provable_domain|is_readable_mutable_reference`; `exclusive_referent_field_requirements|primitive_parameters\(|scalar_result_domains\(`; `check_routed_cast_coverage|check_domain_field_writes`; `check_exit_ensures|CyclicHeaderInvariants::new|live_contexts_prove_scalar_postcondition|BoundaryRefutations`; `member\.member\.as_str\(\)` (crate src only).

## Five things a maintainer most needs to know about this unit
1. `checks_contracts_exits.rs` is a single file of about 9.9k lines containing at least six independent provers nested as inline modules (inherited-contract binding, case observation, cyclic-header induction, domain/field result checks, scalar postcondition evaluation, bounds/reconstruction). `check_exit_ensures` (9641) is the only external entry besides the re-exports at 9542-9551; it is called from one place (`checks_contracts.rs:11812`).
2. `check_exit_ensures` accepts a fact by OR-ing many routes; all but `cyclic_headers.proves` are conjoined with `missing_origins.is_empty()` (9777, 9852), and three kinds of facts are skipped before any proof (9687, 9696, 9713).
3. Identity of arithmetic atoms in the cyclic-header prover is a `format!` of debug-printed symbol/expression handles (2237, 1368, 2877, 3315); some field resolution falls back to field-name spelling when a member symbol is invalid (8394-8398), and const-qualification machines are recognised by name prefix `@const-qualification-` (3690).
4. Domain membership is proved in at least three places with similar shape: `prover.rs` `assigned_values` (35-592), `exits.rs` `result_domains` (2628-4762) and `writes.rs` `member_domains`/`domain_implications` (337-1548); they share helpers (`exact_scalar_membership`, `prove_domain_at_place`, `domain_is_vacuous`) but each carries its own recursion guard (`visited`/`active`/`active` vec).
5. Work is bounded by fixed numeric budgets and depth caps (16_384 transport work, depth 64/128/8, 4096 literal fuel) and by "refuse the whole closure on unknown" returns; limits are recorded in comments (e.g. 577-578, 594-595).

## What you are unsure about
- Whether `Header::arrivals` skipping a call site that `find_call_site` cannot resolve (exits.rs 2079-2088) can occur for a real transition into the header; `find_call_site` is outside this unit.
- Whether `static_type_context_is_unreachable` (exits.rs 9655) can mark a reachable exit unreachable; defined outside the unit.
- The correctness of the claim in comment 9577-9581 that package exit evidence reuses `live_contexts_prove_scalar_postcondition`: no second caller of that name found by grep, other callers may use another route.
- Everything in `checks_contracts_prover.rs` after line 1700 (`call_guarantees`, the rest of `arithmetic`, `field_actuals`, `semantic_contexts_prove_contract_fact`, `call_entry_contexts_prove_boolean_contract_expression`, `evaluator`, and anything else in 6371 lines) was not read; no statement here describes it.
- Unused-code check was done only for a handful of `pub(crate)`/`pub(super)` items in exits.rs and writes.rs; the many `pub(super)` helpers inside nested modules were not individually grepped.
