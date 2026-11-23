# Record: unit_057 gap_028 (area 02-source-stages-00-04, crate 03 symbol-resolved-trees-to-typed-trees)

Root of paths: `source-rust/subsystems/compiler/source-to-psi/pipeline/03_symbol-resolved-trees-to-typed-trees/src/` (written `T/`).
Checkout `4f3c255d05599c11b1ea8ac668635f15f2a6eb86`. Read-only. Every claim was read unless marked "grepped" or "inferred".
This is the second-pass record for the ranges the first reader of unit 057 did not read: `T/equality_and_exhaustiveness.rs` 1-3068 and `T/expression_lowering.rs` 1-2932. (`expression_lowering.rs` is 4,221 lines; 2933-4221 belong to the first record's unit and are not covered here.)

---

## T/equality_and_exhaustiveness.rs (3,068 lines, read in full)

The file has four parts: module `equality` (:7-298), module `exhaustiveness` (:300-2551, including a test module :314-722), module `proposition` (:2553-2967), and a test module `equality_identity_tests` (:2969-3068).

### Module `equality` (:7-298)

1. **Purpose.** Refuses `==` / `!=` (and value-pattern `match` arms) whose operand is a bare payload-bearing case name such as `Command::Move`. The message tells the author to use `in` (:292-294). A constructed brace form is accepted here. Comment: `==` is always value equality; a bare case "denotes no value -- only its domain" (:9-16).
2. **Shape.** `validate_equality_operands_in` (:36, pub(crate)), `scan_proof_facts` (:67), `scan_statement` (:84), `scan_transition_target` (:150), `scan_expression` (:169), `check_equality_operand` (:256).
3. **Inputs and outputs.** Reads `SymbolResolvedTrees` (machines, states, domain definitions, expression and statement tables) and a `TypingCohort`. Returns `Result<(), Diagnostic>`; mutates nothing.
4. **How it decides.** Keyed on symbol identity: the operand must be a `Name` whose `path.symbol` is `SymbolKind::Variant` (:261, :267) or a `StructLiteral` whose `case_symbol` is set (:262-264). The case is then found in its data definition by `variant.symbol == case` (:278). It refuses only when the case has `payload.count() != 0` (:286) and the operand is not constructed (:291). The error text uses `data.name` and `case.name` display strings (:289-293), but only for the message.
5. **Cases by shape.** `scan_expression` is an exhaustive `match` over `ExpressionNode` (:178-253). The check fires at `Binary` `Equal`/`NotEqual` (:200-207) and at `Match` arms whose pattern is `MatchPattern::Value` (:189-193). Membership scans only its subject (comment :230-231). `Match` arms of other pattern kinds are not checked for operands.
6. **Same job elsewhere.** The walk of statements and expressions is a second traversal of the resolved bodies. `exhaustiveness` below walks the same machine/state lists (:728-767). Typed-side equality is done in `T/equality_preparation.rs` (grepped as a file match for `checked_with_equality`; not read here).
7. **Unused or one-caller.** `validate_equality_operands_in`: 2 grep lines across `source-rust/` (definition plus one call at `T/lowerer.rs:1577`). Command: `grep -rw validate_equality_operands_in --include=*.rs source-rust | wc -l` gave 2.
8. **Stated limits.** None beyond the comment that the check "runs on the RESOLVED trees, BEFORE membership lowering" (:18-19).
9. **Fail-open risks.**
   - `:267-269`, `:270-272`, `:283-285`: a `Name` with an invalid symbol, a non-variant symbol, a variant whose parent has no data definition, or a variant not found among members returns `Ok(())`. Traced to the extent that these are unresolved-name cases that resolution should already have refused (inferred; I did not trace the resolver). Wrong program that could pass: bare payload case compared by `==` when its parent lookup fails. Open.
   - `:173-175`, `:154-156`: invalid handles return `Ok`. Stated by structure only (optional positions). Open.
   - `:41-43`: machines before `cohort.machines` that the cohort does not complete are skipped. Stated by the cohort concept (those are typed in an earlier cohort); not traced.

### Module `exhaustiveness` (:300-2551)

1. **Purpose.** Checks that each transition dispatch (a run of transition arms) covers every input, or has a `_` arm. It builds a small matrix of finite "axes" (a boolean subject, or the cases of one data type) from the arm guards and searches for an uncovered assignment. It refuses with "can fall through" or "does not cover ..." errors.
2. **Shape.**
   - Entry: `validate_case_dispatch_exhaustiveness_in` (:724, pub(crate)). `closed_boolean_guard` (:1764, pub(crate)). `data_definition_index` (:2072, pub(crate)).
   - Types: `DispatchScope` (:811; program, machine, state, bindings), `ArmShape { Default, Finite, UncountedCase, Compare, Opaque }` (:772), `FiniteConstraint { Case, Bool }` (:784), `CaseClaim` (:793; data_index, subject, epoch, covered), `LeafContribution` (:800), `FiniteAxis` (:992), `MatrixCoverage { Complete, Uncovered, TooLarge }` (:1005), `CoverageBinding { symbol, primitive, snapshot, shared }` (:1089).
   - Functions: `check_dispatch_run` (:818), `guard_may_change_storage` (:1011), `coverage_bindings` (:1096), `unchanged_subject` (:1246), `shared_subject` (:1305), `shared_read_type` (:1350), `first_uncovered_assignment` (:1586), `classify_arm` (:1794), `classify_membership_union` (:1877), `classify_declared_domain` (:1954), `find_case` (:2036), `same_coverage_subject` (:2153), `subject_declared_type` (:2253), `type_references_equal` (:2520).
3. **Inputs and outputs.** Reads the resolved program, each state's parameters, locals, machine-owned data, type parameters, data definitions and proof facts, and `const_declaration` (`application_identity::const_value` decode at :1747-1759). Output is `Result<(), Diagnostic>` only.
4. **How it decides.**
   - A run is a maximal sequence of `Transition` statements up to one with `ends_dispatch` (:749-765). A `_`/Always guard, or a guard that is the closed boolean `true`, returns Ok at once (:855, :1811-1816).
   - A guard that "may change storage" (any `Call`, `Atomic`, `Match`, `Block`, non-builtin operator, non-builtin cast, non-live member, non-builtin index; :1011-1085) bumps an epoch and is treated as opaque (:843-853). Subjects that are provably unchanged keep epoch 0 (`unchanged_subject`, :1246).
   - Subject identity is `same_coverage_subject` (:2153-2240): symbol handles for names (:2173-2189), `member_symbol` for members, structural recursion for index/unary/binary/cast/membership, saved-subject capture handles. Two `Call` nodes are never the same subject (:2237, comment :2234-2236).
   - Case and boolean claims are keyed on resolved symbols (`case_type_symbol`, `case_symbol`, `domain_symbol`; :2050-2060). Two fallbacks use spelling: `find_case` falls back to the `domain` name path `[type_name, case_name]` and matches `variant.name.as_str() == case_name.as_str()` (:2061-2069); `data_definition_index` falls back to `definition.name.as_str() == type_name.as_str()` when the name is not source-backed (:2089-2092), otherwise looks up a top-level symbol by name+kind (:2076-2088). `declared_field_in` matches fields by `field.name == member.member` (:2367) but requires uniqueness and, when a symbol is present, that it equals the field symbol (:2370-2377).
   - Location text for the error compares `machine.name.rsplit("::").next() == state.name` (:743), message-only.
   - Work bound: `MAX_MATRIX_WORK = 65_536` (:1591); exceeding it gives `TooLarge`, which becomes a refusal "too large for the exhaustiveness proof; add a `_` arm" (:919-924).
   - A variant is "impossible" when one of its `requires_facts` is literal integer `0` or boolean `false` (:1559-1573); impossible variants are dropped from the axis (:1553-1555), and an axis with zero possible values counts as Complete (:1599-1601).
5. **Cases by shape.** `classify_arm` handles: one conjunct that is a membership union, a closed boolean, a `Binary` boolean-equality, or an `Equal`/`NotEqual` compare; a multi-conjunct `&&` where each conjunct is a membership claim, constant `true`, or a boolean constraint (:1794-1875). Anything else is `Opaque`. A compare pair closes coverage only for `==` plus `!=` over the same two subjects at equal epochs (:890-913). `classify_declared_domain` counts a domain as tags only when its facts are exactly one expression that is an `||` union of case memberships on the domain's own subject (:1986-2016); otherwise `Predicate` (uncountable). `builtin_conversion_type` accepts only a plain value-form cast to a primitive with no semantic domain (:1458-1474). `builtin_index_element_type` handles only fixed arrays and slices whose element is primitive and whose index is a builtin operand (:2303-2330). `shared_read_type` comment: "never across an exclusive reference or an opaque/atomic/external storage carrier" (:1347-1349).
6. **Same job elsewhere.** `same_coverage_subject` (:2153) and `type_references_equal` (:2520) compare structure for subjects/types; `data_index_for_type` (:2466) and `T/expression_lowering.rs:341` both map a type name to a data definition through `data_definition_index`. `resolved_data_definition_for_type` (`expression_lowering.rs:2690`) does a similar type-to-definition lookup, matching by symbol or, when the symbol is invalid, by `definition.name == name` (:2730-2735). Suspected duplicates of the type-to-definition lookup outside this unit were not read.
7. **Unused or one-caller.**
   - `validate_case_dispatch_exhaustiveness_in`: 15 grep lines (definition, 1 production call at `T/lowerer.rs:1582`, plus test-module calls). Command: `grep -rw validate_case_dispatch_exhaustiveness_in --include=*.rs source-rust | wc -l`.
   - `closed_boolean_guard`: 4 lines; one use outside the file, `T/statement_lowering.rs:1187` (import at :1098).
   - `data_definition_index`: 5 lines: 2 uses inside the file (:1977, :2065), 1 in `expression_lowering.rs:341` (import at :48).
8. **Stated limits.** "add a `_` arm" refusals when relying on "arms the compiler cannot count (predicate domains, guarded patterns, or value compares)" (:958, :974). "A field depending on unresolved T supplies no access proof" (:1427-1428).
9. **Fail-open risks.**
   - `:1599-1601`: `arities.contains(&0)` returns `Complete`. Reached when an axis has zero possible values: every variant of the data type has a literal-zero/false `requires` fact (:1559-1573), or a `Case` axis whose data definition has no variants. Wrong program that could pass: a dispatch over a type with no inhabitable case. This is semantically correct for an uninhabited type; stated only by structure (no comment). Open.
   - `:1604-1613`: an arm whose constraint cannot be bound to an axis is dropped ("A malformed/unbound arm contributes no coverage", :1602-1603). This fails closed (less coverage), stated.
   - `:855`, `:1811-1813`: a guard that is the closed boolean `true` is Default and ends the check. `closed_boolean_guard` reads a const's evaluated value (:1753-1759); if a const is mis-evaluated as `true`, the run is accepted. Open (value comes from the const evaluator, not read).
   - `:1972-1989`/`:1963-1969`/`:1959-1961`: unknown domain, non-`Named` target type or missing data definition returns `NotCase`, which makes the claim uncountable (fails closed).
   - `:2240-2242` `_ => false` in `same_coverage_subject` fails closed (distinct subjects).
   - Function-level: `validate_case_dispatch_exhaustiveness_in` skips machines before `cohort.machines` that the cohort does not complete (:728-731). Stated by cohort design; not traced.
   - `:1244`: `unchanged_subject` treats `Boolean/Integer/Float/String` literals as unchanged (:1250-1253); correct by construction.

### Module `proposition` (:2553-2967)

1. **Purpose.** Types a call to a proposition (a named proof predicate) or to a generic proposition parameter. It checks the proof-static binder arguments (type, const, machine) and the value argument count, and lowers the value arguments in fact position.
2. **Shape.** `is_proposition_call_target` (:2564), `lower_proposition_application` (:2575), `resolve_proposition_static_path` (:2789), `classify_proposition_static_symbol` (:2806), `validate_const_literal_argument` (:2855), `proposition_const_symbol_type` (:2878), `find_data_type_parameter` (:2913), `proposition_const_types_match` (:2927), `integer_literal_fits` (:2949).
3. **Inputs and outputs.** Reads `lowerer.source_trees` (propositions, `proposition_binders`, `data_type_parameters`, symbols) and writes `typed::proposition::PropositionApplication` plus new `Named` type references and integer expression nodes into `lowerer.typed_trees` (:2703-2727).
4. **How it decides.** The proposition is found by linear scan `propositions.iter().find(|p| p.symbol == call.target_symbol)` (:2579-2583) or by scanning `data_type_parameters` for a matching symbol of kind `Proposition` (:2584-2597). Binder kind (Type/Const/Machine) is checked against the argument's symbol kind (:2806-2853). Static paths resolve by `argument.symbol`, else by `find_descendant_by_path` over the path spelling from the symbol root (:2789-2804), a source-spelling lookup. Const type match is primitive equality, else `Named` symbol equality, else `==` on the type reference (:2927-2947). Literal fit uses `carrier.admits` (:2963-2965).
5. **Cases by shape.** Three binder kinds only (:2624-2702). Integer-literal const arguments only (`IntegerLiteral`; `validate_const_literal_argument` refuses non-primitive and non-integer binder types, :2862-2874). `integer_literal_fits` returns false for `Bool`, `F32`, `F64`, `Selector` (:2955-2961). `PrimitiveType::Addr` is checked as `U64` (:2957, comment :2954).
6. **Same job elsewhere.** Scanning `data_type_parameters` and `proposition_binders` linearly by symbol appears four times in this module (:2579, :2584, :2821-2827, :2882-2889, :2917-2924). Dense-id table lookup would be the usual route (AGENTS.md data-oriented rule); suspected, not read elsewhere.
7. **Unused or one-caller.** `lower_proposition_application`: 4 grep lines: definition, `T/contracts_and_signatures.rs:1111`, `T/declarations.rs:1665` (import), `T/declarations.rs:1741`. `is_proposition_call_target`: 9 lines (grepped; used also in `expression_lowering.rs:588,1558`).
8. **Stated limits.** The call message at `expression_lowering.rs:1561` says a proposition application "must be the complete fact in this implementation slice" (that string is in the next file; cited for context).
9. **Fail-open risks.** None seen. Every mismatch returns `Err` (:2598-2603, :2614-2621, :2757-2764). `resolve_proposition_static_path` returns an invalid handle when the path is not found (:2803), and the invalid handle then fails `classify_proposition_static_symbol` (:2810-2812), so it is refused (traced within the function). `proposition_binders` empty when only a parameter exists (:2613): then any `machine_arguments` non-empty is refused by the length check (:2614).

### Test modules

- `exhaustiveness::tests` (:314-722): 14 `#[test]` functions (grep of `#[test]` between those lines: counted by reading). They pin that separate call invocations do not prove coverage (:338), captured values or a `_` close coverage (:353), cast targets and builtin policies distinguish subjects (:367), mutable pointees are versioned across calls (:388), unchanged fields and shared paths stay covered (:408, :421), exclusive references and mutable storage do not (:440), opaque/atomic carriers get no shared stability (:464), borrowed case captures keep their storage version (:486), constants and builtin indices keep coverage (:524), an authored indexer is effectful (:542), a complete prefix does not enumerate later versions (:551), matrix search returns exact witnesses and bounds work at 65,537 constraints (:561), and dispatch continuation does not supply a missing arm or borrow the next dispatch's wildcard (:681, :704). Negative twins are inside the same tests (`expect_err` beside `expect`). One test (`partial_matrix...`, :561-609) builds trees by hand with `DispatchScope` and calls `coverage_bindings`, `finite_axes`, `first_uncovered_assignment` directly. Another (:486-521) mutates the expression table to insert a `SavedSubject`.
- `equality_identity_tests` (:2969-3068): 5 tests that read fixtures through `include_str!` of `tests/fixtures/equality/...` (paths :2973-2983, :3004-3014) and call `crate::typing::qualified_operator_home_tests::checked_with_equality` (in `T/qualified_operator_home_tests.rs`, not read). They pin: alias exact leaf declarations on both sides of `==` (:2972), closed generic fields select their comparison (:3003), user trait named `Equatable` or otherwise never grants structural synthesis (:3027), qualified record operands keep exact domains (:3039), and only payloadless sums have implicit data equality (:3054). The two fixture-driven tests have accept and refuse twins; the `include_str!` paths hard-code the repo-relative path from `CARGO_MANIFEST_DIR` with six `..` segments.

---

## T/expression_lowering.rs lines 1-2932 (read in full)

The file is a single `expression` module with nested modules `domain_membership` (:44-384), `name_paths` (:385-437), `native_code_layout` (:438-511), `operators` (:512-570), `table` (:571-...), with `table::lowerer` (:572-2754) and `table::member_binding` (:2755-...). Lines 2933-4221 are outside this record.

### Header and entry-point doc (:1-42)

Comment lists the entry points: `lower_expression_handle`, `lower_expression_handle_from_table`, `..._with_symbol_substitution`, `..._in_fact_position`, `lower_static_machine_argument` (:8-35). Those functions are defined after line 2932 and are not read here (inferred from the comment; not verified).

### `domain_membership` (:44-384)

1. **Purpose.** Lowers `x in Domain` and `x in Type::Case` into executable boolean expressions: case membership becomes a `CaseMembership` binary node; a named domain expands to the conjunction of its proof facts with the subject substituted.
2. **Shape.** `lower_case_membership_expression` (:56), `replayed_case_membership_test` (:82), `case_membership_test` (:101), `domain_definition` (:157), `is_pending_data_subject` (:171), `lower_domain_membership_expression` (:203), `lower_atomic_domain_membership_expression` (:232), `case_symbols_for_domain_fact` (:334), `combine_conjunction` (:360).
3. **Inputs and outputs.** Reads the resolved program and a source expression table; writes typed `Name`, `Binary` and `Boolean` nodes into `target` (`typed::TypedTrees`). Records `ExecutableDomainTest { subject, domain }` on the result via `retain_executable_domain_test` (:324-330).
4. **How it decides.**
   - `case_membership_test` needs the last two domain path members to exist and both `type_symbol` and `case_symbol` valid (:110-115), and the case's variant symbol to be a member of the type's data definition (:117-124). The path member names are only copied into the new node; identity is the two symbols (:126-143). Otherwise returns `None`.
   - `lower_domain_membership_expression` refuses when the subject is a "pending data subject": a name whose symbol is the data definition's `predicate_subject_symbol` while the definition has `requires_facts` (:171-201, message :212). `SavedSubject`, `Borrow` and `Cast` are followed through to the subject (:189-196).
   - A domain expands through `expand_domain_reference` into atoms (:215-228). Each atom's facts are lowered with the domain's `subject_symbol` substituted by the value (:250-257, :281-287, :292-298).
   - `case_symbols_for_domain_fact` finds the case by spelling: the domain path must be exactly `[type_name, case_name]` (:338), the type by `data_definition_index` (:341), the case by `variant.name.as_str() == case_name.as_str()` (:350). This is a name lookup (source spelling) keyed on display names.
5. **Cases by shape.** `case_symbols_for_domain_fact` accepts only a two-segment path (`let [type_name, case_name] = ...`, :338). `is_pending_data_subject` loop only follows `Name`, `SavedSubject`, `Borrow`, `Cast` (:180-197), other node kinds return false.
6. **Same job elsewhere.** Case-by-spelling lookup duplicated in `T/equality_and_exhaustiveness.rs:2061-2069` (`find_case`) and here at :338-356. Both use `data_definition_index`. `combine_conjunction` builds a left-fold of `And` nodes (:360-383); the same conjunction build for patterns is at :783-788.
7. **Unused or one-caller.** `lower_case_membership_expression`: 3 grep lines (definition, import :578, use :2661). `lower_domain_membership_expression`: 4 lines (definition, import, uses :311 and :2652). `replayed_case_membership_test`: private, 2 uses (:258, :301).
8. **Stated limits.** None beyond the error "domain's own default domain is still pending admission" (:212).
9. **Fail-open risks.**
   - `:317-318` `case_membership_test` returning `None` for a `ProofFact::Membership` falls through to `lower_domain_membership_expression` with `membership.domain_symbol` (:311-316). If `domain_symbol` is invalid, `domain_definition` returns `Err` (:161-168). Fails closed. Traced.
   - `:365-368` `combine_conjunction` of an empty list returns `Boolean(true)`. Reached for a domain with no facts (every value is a member). Stated by semantics: a domain without proof facts constrains nothing (inferred; no comment). Wrong program that could pass: none if no-fact domains are truly unconstrained. Open.
   - `:200` `is_pending_data_subject` returns `value.is_valid()` after the loop; for a visited-cycle exit it returns true (refuses). Fails closed.

### `name_paths` (:385-437)

Copies a resolved name path's member names and member symbols into the typed expression table (`lower_name_path_members_into_table` :390, `lower_table_name_path_node_into_table` :404). No judgment. `lower_name_path_members_into_table`: 5 grep lines; `lower_table_name_path_node_into_table`: 3 lines (definition, import :581, use :1986). `reserved_result_of` is copied (:413).

### `native_code_layout` (:438-511)

1. **Purpose.** Lowers a `code_layout<M>()` call into a `NativeCodeLayoutRequest` and finds the type of its result.
2. **Shape.** `lower_request` (:445), `result_type` (:478).
3. **How it decides.** `lower_request` requires exactly one static machine argument and no value or evidence arguments (:453-457). The operand's symbol must have kind `Machine`, `State` or `MachineParameter` and carry no const literal or evidence projection (:460-471). `result_type` finds the data definition named `"CodeLayout"` whose `core_source` equals `CoreSource::CodeLayout` (:485-493): a name plus core-source identity, not name alone. Missing definition is an error (:494-498).
4. **Cases by shape.** Exactly one operand (:453).
5. **Fail-open risks.** None seen. A `program` of `None` makes `valid` false (:460-466) and refuses.

### `operators` (:512-570)

`lower_binary_operator` (:516) maps each resolved `BinaryOperator` to its typed counterpart one-to-one (18 arms). No judgment. 5 grep lines for the name; one production use (:1341).

### `table::lowerer` (:572-2754)

**Purpose.** Turns one expression from a resolved expression table into typed nodes. Most work is lowering a call into the right typed call form (ordinary call, value call, builtin query, build selection, quotient operation, layout query, atomic operation).

**Shape.**
- `ExpressionTableLowerer<'program, 'target>` (:897) with fields `program: Option<&SymbolResolvedTrees>`, `source`, `target_trees`, `symbol_substitution`, `source_owner`, `fact_position`, `admits_policy_operators`, `lowered_expressions` (a Vec of `(source handle, fact_position, lowered handle)`, :914-918), `match_captures`, `pending_blocks`.
- `new` (:930), `lower` (:1110), `lower_value` (:1271), `lower_node` (:1282), `lower_binary_node` (:1303), `lower_nonbinary_node` (:1348), `lower_block` (:973), `lower_match` (:602), `lower_destructure_pattern` (:677), `lower_transition_pattern` (:815), `lower_saved_subject` (:834), `lower_quotient_operation_request` (:2159), `lower_private_layout_operation_request` (:2268), `lower_data_layout_query` (:2312), `layout_query_operand_type` (:2396), `nullary_erased_initializers` (:2515), `synthesize_nullary_initializer` (:2596), `lower_application_type` (:2623), `lower_membership_expression` (:2641), `quotient_zero_value_target` (:2681), `resolved_data_definition_for_type` (:2690), `lower_unary_operator` (:2739). Also `NullaryErasedInitializer` (:888).

**Inputs and outputs.** Reads the resolved program (optional: `program` may be `None`, used by callers that type table fragments without the program; several branches then refuse), the source expression table, and mutates `typed::TypedTrees` (expression table, type reference table, authored declaration selections).

**How it decides.**
- Memoisation key is `(source handle, fact_position)` (:1117-1125). Linear scan over a `Vec` (`lowered_expressions.iter().find`), so cost is quadratic in lowered expressions per lowerer (inferred; not measured).
- If the lowered handle equals the substituted subject, the handle is returned without stamping span/selections (:1137-1141, comment :1132-1136).
- Call selection ownership: among the attached selection occurrences of a call, it picks the one of kind `Call` whose `source_span` equals `source_call.target.source_span()` (:1159-1160) (a source-position key); two different matches is an error (:1161-1166). A generated call owns the occurrence only if `bind_generated_call_selection` returns true when `build_selection.is_valid()`, else only if `!names_replaced_prelude_operation(...)` (:1176-1188).
- Post-lowering result types for: layout queries (`u64`, :1199-1205), native code layout (`CodeLayout`, :1206-1215), `AddressOf` builtin with no receiver, no machine arguments, one argument (`Address`, :1217-1226), and constants used as fixed arrays (:1229-1248).
- Policy operators: a binary node with `policy != Exact` is refused unless `admits_policy_operators` (:1290-1299), message "supported only in executable machine code, contracts and mathematical definitions".
- `lower_binary_node` lowers `And/Or/Equal/NotEqual` children without clearing `fact_position`, all others through `lower_value` (:1307-1323).
- Call lowering order in `lower_nonbinary_node` for `Call` (:1473-1851): (1) atomic call selection `atomic_calls::select_expression`; (2) callee is a value (`runtime_callables::callee_is_value`) gives a runtime call; (3) `IntegerEmbed` by `call.target.as_str() == BuiltinFunction::IntegerEmbed.name()` AND symbol check (:1545-1553) retains the `Int` type; (4) proposition targets are refused (:1557-1565); (5) test-group facet, product description requests, product query operations and product-selection requests (:1566-1701); (6) quotient, private-layout, installed-interrupt, data-layout-query and native-code-layout requests (:1702-1736); (7) an ordinary call with receiver-method fallback by `declared_receiver_method_target` then `computed_receiver_method_target` when `target_symbol` is invalid (:1743-1765); (8) refusal if the late-selected method's owner machine has a structural type equation (:1766-1777).
- `lower_data_layout_query` decides by `DataLayoutQueryKind::from_target_name(call.target.as_str())` (:2318-2321), a source-spelling match on the call's written name; it applies only when `call.target_symbol` is invalid and there is no receiver (:2325-2327). `Alignment` with one value argument is a `LocationAlignment` request (:2329-2334).
- `nullary_erased_initializers` synthesizes a value for each omitted erased (proof) field of a struct literal when the field's type is a checked-shape, non-generic data type (`!name.contains('<')`, :2561 — a spelling test on the display name), with no fields, and exactly one payloadless variant (:2557-2582).
- `resolved_data_definition_for_type` finds a data definition by symbol, or by `definition.name == *name` when the symbol is invalid (:2730-2735). A `LayoutStorage` generic constructor is recognised by `constructor.name.as_str() == "LayoutStorage"` plus `BoundaryOpaque` supply mode plus `SourceOrigin::Toolchain` plus one conformance bound plus two arguments (:2699-2706): name plus origin plus shape.
- `lower_membership_expression` uses `membership.domain_symbol` if valid, else case membership, else errors "unknown domain" (:2650-2677).

**Cases by shape (quoted).**
- Quotient operation: `Define` takes exactly one theorem; `Lift` takes 1 or 2 (`selected_theorems.len() == 1 || ... == 2`, :2194-2205); theorem roles are fixed by position: first `Congruence`, second `ForwardPreconditionTransport` (:2240-2244). Error text :2201 "requires `F, Congruence` or `F, Congruence, Transport` in canonical role order".
- `layout_plan_place_private` takes exactly one static argument that is a `Conformance` (:2287-2301).
- `code_layout` takes one machine operand (see above).
- `String` literal always lowers to a shared `&[u8]` slice type (:2068-2090).
- `Block` only where `pending_blocks` is set and not in fact position (:979-984).
- `lower_match` handles `Value`, `Guard`, `Wildcard`, `Destructure`; a `Destructure` pattern that has a `subject_binding` and no guard lowers to `Wildcard` (:638-643).
- `lower_destructure_pattern`: for a head whose symbol is a `Variant`, condition is `CaseMembership(saved, head) && predicate`; otherwise the predicate only (:765-791).

**Same job elsewhere.**
- `callee_is_value` is defined in two places: `T/runtime_callables.rs:170` (a wrapper that calls `member_binding::callee_is_value`, grepped at `runtime_callables.rs:176`) and `expression_lowering.rs:2881`. 8 grep lines in the crate. Used at `expression_lowering.rs:1500`, `:3156`, `statement_lowering.rs:214, 371, 1632`.
- The match-field action lowering (`Bind/Equal/Waive`) is written twice in a row, once for `placed` rows (:711-730) and once for `written` rows (:731-750) with the same `match action`.
- `retain_builtin_type_reference` (:1015) and the ad-hoc `find_named_type_reference(...).unwrap_or_else(insert Named)` pattern repeat at :499-509 (`native_code_layout::result_type`) and :1030-1039.
- Quotient `TypeParameter`/`Data`/`BuiltinType` operand to type reference in `layout_query_operand_type` (:2396-2464) overlaps with type-reference lowering in `T/type_reference.rs` (suspected; not read).

**Unused or one-caller.**
- `lower_proposition_application` is used elsewhere (see above), not here.
- `resolved_data_definition_for_type`: 10 grep lines (`grep -rw resolved_data_definition_for_type --include=*.rs source-rust | wc -l`); callers in `expression_lowering.rs` at :2556, :2685, :2708, :2721, :2726 and at `member_binding` import (:2756).
- `lower_transition_pattern`: 4 grep lines; `lower_application_type`: 3 lines.
- `declared_receiver_method_target` is re-exported at `:3753` and `:3837` (outside my range; grepped).

**Stated limits.**
- "this position cannot use a suffixed operator yet" (:1294-1295).
- "a proposition application must be the complete fact in this implementation slice" (:1561).
- "late-selected receiver method retains an undischarged structural type equation; use a closed explicitly selected receiver" (:1775).
- "conformance or structural proof discovery is not permitted" (:2229); "slot discovery is not permitted" (:2299).
- Comment on `nullary_erased_initializers`: "Open or applied evidence types still supply no implicit inhabitance" (:2546-2547).

**Fail-open risks.**
- `:1019-1021` `retain_builtin_type_reference` returns an invalid handle (not an error) when `program` is `None`. Callers continue with an unset type reference. Reached by table fragments lowered without the program. Open: no comment; the typed result has no type for the literal or builtin, which later stages must detect (not traced).
- `:1054-1058`, `:1060-1068` `retain_data_type_reference` silently returns when there is no program, no data definition, a generic owner, or a reference already exists. Fails neutral (only skips adding a type reference). Open.
- `:2519-2521`, `:2522-2528` `nullary_erased_initializers` returns an empty list when the holder is not found or not checked-shape, so an omitted erased field is not synthesised; the later stage then sees a missing field (fails closed, inferred).
- `:2566` `filter_map` returns `None` for evidence types that are generic, have fields, or have more than one nullary variant: no implicit value (fails closed).
- `:1148-1150` in `lower`: with `self.program == None` the loop `continue`s and `own` stays `None`: the call gets `authored_call_selection = None` (:1193-1197). Open: a call with source selection occurrences but no program loses its selection link silently.
- `:2318-2327`/`:2325` `lower_data_layout_query` returns `Ok(None)` when `call.target_symbol` is valid or a receiver exists, even if the spelling matches a layout query (a user machine named like a query is an ordinary call). Stated by structure (resolution has bound it); not traced.
- `:2175-2181` quotient: any builtin function that is not Lift/Define returns `Ok(None)`: normal.
- `:2273-2281`, `:2283-2286`: private layout op returns `Ok(None)` unless the symbol is a `State` whose owner is the sealed intrinsic `PrivateLayoutPlacement`: identity keyed, fails closed for a user machine.

### `table::member_binding` lines 2755-2932

1. **Purpose (this range).** Finishes the authored member-access selection attached to a lowered member expression, and answers "is this call's callee a value or a method" for receiver-method calls.
2. **Shape.** `finalize_authored_member_selection` (:2772), `declared_receiver_method_target` (:2870), `callee_is_value` (:2881), `ExpressionTableLowerer::positional_member` (:2908), start of `selected_member` (:2937; body continues past :2932, not read here).
3. **How it decides.**
   - `finalize_authored_member_selection` looks at the lowered node; if it is not a `Member`, returns `Ok` (:2784-2788). It finds the single attached `MemberAccess` selection occurrence (:2797-2812); more than one is an error. No occurrence means "Generated projections with no authored member" and returns `Ok` (:2813-2817).
   - For a `CollectionMeasure` selection the target must already be the matching `Intrinsic` or a `LateBound(CheckedMember)` that is then finalized to the intrinsic (:2823-2837); anything else errors.
   - For a field selection, the selected symbol must be valid and of kind `Field` (:2843-2855); an already resolved target must name the same symbol (:2856-2862); otherwise the selection is finalised with `CheckedMember` (:2863-2867). Keyed on symbol identity.
   - `callee_is_value`: true if `callee_uses_value_binding` says so (:2889-2895), or if the callee is a `Member` for which `declared_receiver_field(... , 0)` finds a field (:2896-2900). `declared_receiver_field` (:3015) is outside my range.
4. **Same job elsewhere.** `callee_is_value` duplicates the name of `T/runtime_callables.rs:170`; the latter forwards here (grepped, 1 forwarding site).
5. **Unused or one-caller.** `finalize_authored_member_selection`: 12 grep lines. `callee_is_value`: see above. `positional_member`: used at `expression_lowering.rs:1913` (read).
6. **Stated limits.** "An unselected member stays late-bound. Its demanded declaration still refuses; there is no inferred target." (:2844-2845).
7. **Fail-open risks.**
   - `:2813-2817` a member with no attached `MemberAccess` occurrence returns `Ok` without checking its selection. Stated: "Generated projections with no authored member carry no new authored selection." Reaches: compiler-generated member nodes. Stated.
   - `:2840` `Target::Intrinsic(_) if !selected.is_valid()` returns `Ok`. Open: an intrinsic-bound occurrence with an unselected field is accepted without checking that the intrinsic belongs to the member (not traced).
   - `:2846-2849`: unselected member with `LateBound(CheckedMember)` returns `Ok` (stated by comment :2844-2845).

### Test code

`expression_lowering.rs` lines 1-2932 contain no `#[test]` (grepped: `grep -n "#\[test\]" T/expression_lowering.rs` would show only later lines; I did not run it over the whole file, so this is inferred from reading 1-2932).

---

## Coverage

- `T/equality_and_exhaustiveness.rs`: lines 1-3068 read in full (3,068).
- `T/expression_lowering.rs`: lines 1-2932 read in full (2,932). Line 2933-2940 were also returned by the read but are not part of the unit and are not described.
- Skimmed: none. Not read: none within the unit file.
- Total read in full: 6,000 lines. Greps: see the commands cited inline plus a loop over 19 symbol names.

## Five things a maintainer most needs to know about this unit

1. The exhaustiveness check (`T/equality_and_exhaustiveness.rs:724-990`) is a bounded matrix search over finite axes (booleans, or cases of one data type), capped at 65,536 steps (:1591). Larger matrices are refused, not accepted (:919-924).
2. Coverage depends on proving a subject is unchanged between guards. Any call, atomic, match or block in a guard bumps a storage epoch (:843-853, :1011-1085); only copied scalars and shared-loan paths keep epoch 0 (:1246-1345). Two separate call nodes are never the same subject (:2237).
3. Case and domain lookups mix symbol identity and spelling: `find_case` (:2061-2069), `data_definition_index` (:2072-2093), `case_symbols_for_domain_fact` (`expression_lowering.rs:334-358`) and `resolved_data_definition_for_type` (:2730-2735) fall back to display names when symbols are missing.
4. `ExpressionTableLowerer::lower_nonbinary_node` for `Call` (`expression_lowering.rs:1473-1851`) is one long ordered chain of 8 special forms (atomic, value call, proposition refusal, product queries/selections, quotient, private layout, interrupts, layout queries, code layout, ordinary). The order decides which form wins.
5. `program: Option<&SymbolResolvedTrees>` lets table fragments lower with no program; in that mode several branches silently skip (invalid type handles :1019-1021, no selection ownership :1148-1150), others refuse (e.g. :1403-1407, :2645-2649).

## What you are unsure about

- Whether the early `Ok(())` returns in `check_equality_operand` (`:267-285`) can be reached by a program that resolution accepted: I did not read the resolver.
- Whether `retain_builtin_type_reference` returning an invalid handle when `program` is `None` (:1019-1021) is detected downstream.
- Whether `callee_is_value` in `runtime_callables.rs` adds any check beyond forwarding; I read only its first lines via grep.
- The number of `#[test]` functions in `exhaustiveness::tests` was counted by reading, not by a grep count command; I listed 14 in the order they appear.
- The text of `declared_receiver_field` and the rest of `selected_member` (past :2932) was not read.
