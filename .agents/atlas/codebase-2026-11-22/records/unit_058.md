# Record: unit_058 (source stages 03, 04, 05; selected files)

Checkout `C:/Development/Omega/.agents/worktree/worker-2/` at `4f3c255d`. All paths below are relative to
`source-rust/subsystems/compiler/source-to-psi/pipeline/`; `<03>` = `03_symbol-resolved-trees-to-typed-trees`,
`<04>` = `04_typed-trees-to-prepared-typed-trees`, `<05>` = `05_prepared-typed-trees-to-lowered-program`.
Every file in the unit was read in full (see "Coverage" at the end).
Counts are `grep -rwn --include=*.rs NAME source-rust | wc -l`, so each count includes the definition line.

---------------------------------------------------------------------------------------------------

## <03>/src/type_reference.rs (3923 lines, read in full)

**Purpose.** Lowers a resolved written type (`TypeReference` / `TypeReferenceNode`) into the typed trees' type-reference
table, and holds the checks that must happen while types are first formed: domain-constraint selection and alias
expansion, type-alias applications, constant arguments to generic applications, integer-width and array-extent checks,
and validation of "generated generic application origins". It is one file made of ten inline `pub(crate) mod` blocks
plus free functions.

**Shape.**
- `domain_aliases::expand_domain_reference` (`:17`) expands a domain alias into its non-alias constituents, with a cycle
  check (`:34`) and an empty-alias check (`:45`). Returns `Vec<ExpandedDomainReference{symbol,path}>` (`:12`).
- `domain_constraints::normalize_domain_constraints` (`:82`, frontier-filtered) and `..._for_type` (`:103`) call
  `normalize_constraint_span` (`:127`), which selects the one visible domain for each authored constraint
  (`select_domain_candidates` `:369`, `select_domain_candidates_where` `:397`), retains the domain application
  (`program.retain_domain_application` `:234`), and either rewrites the constraint in place or, for an alias, replaces the
  span with the expanded constituents (`:304-317`). `spell_domain_range` (`:322`) / `with_domain_candidate_index` (`:357`)
  fill a `DomainCandidateIndex` (`by_qualified`, `by_relative` maps keyed by spelling).
- `alias_uses::retain_application` (`:475`) records a `TypeAliasApplication`; `check_applications` (`:540`) judges each retained
  application once the cohort has complete declarations (arity, lifetime arguments, type-vs-value, bounds).
- `arithmetic_results::retain_arithmetic_result_type` (`:649`) inserts a `Constrained{ArithmeticDomain}` reference node for a
  builtin integer/float carrier so later readers find the policy-qualified type; `computed_results::retain_computed_result_types` (`:727`)
  calls it and `retain_collection_result_types`.
- `const_arguments::closed_applications::normalize` (`:754`, re-exported as `normalize_const_argument_carriers` `:2109`) walks every
  generic application (data, trait, conformance, machine/trait conformance edges, refines, conformance bounds, services) and, for
  each `const` parameter, replaces an argument that is a closed literal or named constant by a canonical `Named{symbol: invalid, name: "<value>"}` node.
  Helpers: `named_argument_in_carrier` (`:1020`), `defer_to_holders` (`:1170`), `builtin_carrier` (`:1235`), `argument_in_carrier` (`:1264`),
  `parameter_declarations` (`:1381`), `needs_admission` (`:1438`). Inline tests `:1465-2106`.
- `constraints::lower_type_constraint_node_span_*` (`:2120`, `:2163`) and `classify_domain_constraint_subject` (`:2248`).
- `direct::lower_type_reference_handle_with_context` (`:2336`) lowers the tree-shaped resolved type; `lower_fixed_array_type` (`:2517`);
  `integer_family_width` (`:2294`).
- `generic_origins` (`:2661-3327`): `validate_computed_arguments` (`:2668`), `application` (`:2838`), `region_application` (`:2910`),
  `application_scanned` (`:2935`, fallback when the index is malformed), `validate_origin` (`:2992`), and an `Equality` struct (`:3071`) that compares two resolved type references.
- `table::lower_type_reference_handle_from_table_with_context` (`:3362`) is a second lowering of the same node kinds from the handle/table
  form of the resolved types.
- Free functions: `retain_supplied_operand_owner` (`:3569`), `lower_type_reference_into_table` (`:3610`), `retain_new_type_owners` (`:3630`),
  `lower_return_type` (`:3650`), `lower_type_reference_into_trees_with_exposure` (`:3660`), `lower_type_reference_handle_from_table` (`:3674`),
  `retain_type_reference_recipe_selection` (`:3692`), `retain_type_reference_selection` (`:3752`), `lower_named_type_reference` (`:3806`),
  `retain_static_path_selection` / `retain_path_selection` / `retain_path_selection_occurrence` (`:3855-3923`).

**Inputs and outputs.** Reads `SymbolResolvedTrees` (`source.symbols`, `source.tables.*`, `source.domain_definitions`, `source.data_definitions`,
`source.type_aliases`) and mutates `TypedTrees` (`type_reference_table`, `expression_table`, `authored_declaration_selections`,
`deferred_refusals`, `applied_domain_applications`). Normalization functions return `Result<(), Diagnostic>`; the lowering functions return new `TypeReferenceHandle`s.

**How it decides.**
- Domain constraint selection is keyed on the authored spelling plus visibility from the referencing source span:
  `top_level_candidates_from_source(authored_name, [Domain], span)` (`:406`), else a spelling index `by_qualified`/`by_relative` over
  `display_path(symbol,"::")`, `domain.name` split on `::` (`:330-352`), then `domain_name_reaches` (`:441`) and `domain_accepts_carrier` (`:385`). That is source-spelling
  based lookup with identity filtering afterwards. Two or more matches with a source reference reject as "ambiguous domain" (`:165-183`).
  Zero matches is not an error here: it pushes the constraint back with `names_no_visible_domain` (`:188-204`) when no domain of that name is visible for any carrier.
- Const-argument admission is keyed on the receiving parameter's declared carrier identity: `type_identity_with_simultaneous_substitutions` compared
  with `normalized_type_identity(declaration.declared_type)` (`:1092-1098`); the canonical value text is compared with `value.encoding` (`:1105`).
  A named-constant argument is recognized by `CanonicalConstValue::from_atom(name.as_str())` (`:1041`, `:1105`, `:1451`), i.e. by parsing the argument's name text as a canonical atom.
- `generic_origins::Equality::reference` (`:3111`) compares two resolved type references structurally; names compare by symbol when valid, else by canonical atom or `parse::<i128>` of the name string (`:3135-3147`).
  `Equality::domain` (`:3287`) matches a domain constraint by `find_top_level_by_name_and_kinds_from_source` and, failing that, by `domain.name.rsplit("::").next() == Some(name)` (`:3315-3317`) (spelling).
  `instance_identity_matches` (`:3051`) keys an origin on `symbol` and on `source_span` equality of names (`:3061`).
- `integer_family_width` (`:2294`) accepts a `u<N>`/`i<N>` whose argument is a `const` type parameter or a `Const` symbol; refuses any other named argument (`:2329`).
- `lower_fixed_array_type` (`:2517`) refuses a `Parameter`/`Local` extent (`:2537-2547`); for a `Const` symbol with an evaluated value it requires an integer fitting `usize` (`:2573-2590`); an unevaluated constant is kept as `ConstParameter` (no check).
- `retain_type_reference_selection` (`:3752`) skips when exposure is `None`, the name is not source-backed, the symbol invalid or a builtin (`:3760-3767`), or the symbol kind is a parameter/local (`:3778-3789`).
- `arithmetic_results::retain_arithmetic_result_type` returns silently (adds nothing) on any shape it does not recognize (`:660-683`).

**Cases by shape.**
- `argument_in_carrier` serves only `Carrier::{Integer, Boolean}` (`:1220-1224`); comment at `:1229-1233` says other carriers "stay as written". Call-valued arguments are judged only by the declared return type of a single-state machine (`:1344-1376`); `_ => Ok(None)` for anything else.
- `named_argument_in_carrier` serves integer/boolean (scalar re-materialization `:1112-1149`) and Record/Variant/Array canonical values (`:1151-1155`), otherwise errors.
- `closed_applications::normalize` refuses a record or case const argument: error text "no product route yet: a record or case const argument is not typed against its declared data type" (`:982-985`) via `defer_to_holders`.
- `parameter_declarations` (`:1381`) handles `Data`, `Trait`, `Conformance`, `TypeAlias`, `Machine|State`; `_ => None` (`:1434`).
- `alias_uses::check_applications` judges only `TypeParameterKind::Type` formals for type-vs-value and bounds (`:608-610` skips others).
- `generic_origins::Equality::reference` handles `AliasUse`, `Named`, `Reference`, `Slice`, `FixedArray`, `Generic`, `Constrained`, `ConstExpression`, `Unit`; `_ => false` (`:3226`; Callable is never equal).
- `closed_materialized_value` (`:3255`) is a stub: it records an `unavailable_judgment("closed const argument replay")` failure and returns `None`, so two distinct const expressions in `Completed` phase never compare equal; in `Declaration` phase they compare equal whenever both have a materialization source (`:3220`).

**Same job elsewhere.**
- `direct::lower_type_reference_handle_with_context` (`:2336`) and `table::lower_type_reference_handle_from_table_with_context` (`:3362`) lower the same twelve node kinds from two representations; the bodies are near copies (Generic, ConstExpression, Callable, Named, AliasUse, Unit arms are textually parallel, read). Also `constraints::lower_type_constraint_node_span_from_table` (`:2120`) vs `..._with_context` (`:2163`).
- `generic_origins::application` (`:2838`) and `application_scanned` (`:2935`) do the same selection through an index vs a scan.
- `select_domain_candidates_where` (`:397`) vs `Equality::domain` (`:3287`) both pick a domain by name+carrier+visibility, with different rules (suspected duplicate selection logic; both read).
- Domain candidate selection by spelling also exists in `<03>/src/expression_meanings` (suspected, not read): `select_domain_candidates_where` has 12 line-hits across the repo (`grep -rwn ... | wc -l`).

**Unused / one-caller code** (counts are line hits incl. definition):
- `retain_type_reference_recipe_selection` 2 (one caller: `<03>/src/call_and_product_selections.rs:732`).
- `lower_type_reference_handle_from_table` 3; `lower_type_reference_into_table` 54; `lower_return_type` 5; `retain_static_path_selection` 6; `retain_path_selection_occurrence` 3; `normalize_domain_constraints_for_type` 7; `validate_computed_arguments` 4; `normalize_const_argument_carriers` 4; `expand_domain_reference` 7; `select_domain_candidates` 6.
- No `pub`/`pub(crate)` item in this file counted as 0 callers.

**Stated limits.**
- "no product route yet: a record or case const argument is not typed against its declared data type" (`:983`).
- `type alias argument ... requires a type` etc. are refusals, not limits. `closed_materialized_value` stub: "closed const argument replay" (`:3266`).
- "A width written as an expression is left for the specialization that would have to compute it." (`:2291-2292`).
- Comment `:1229-1233`: a carrier with a domain or arithmetic policy "asks whether the value is a member, which is a proof question"; argument "stays as written".

**Fail-open risks.**
- `argument_in_carrier` `_ => Ok(None)` (`:1377`) and `(None, Call, _) ... _ => Ok(None)` (`:1374`): a const argument that is a call returning a non-scalar, or an unevaluated expression, is not rejected here. **stated** (`:1335-1343` comment: "A result of any other type is not judged here"); whether a later stage judges it is not traced.
- `normalize` skips (`continue`) when `parameter_declarations` is `None` or its length differs from the argument count and no closed demand exists (`:910-933`); an application to an unknown declaration is then not checked. **open**.
- `builtin_carrier` returns `None` for domain/policy carriers (`:1226-1233`) so those arguments are neither admitted nor refused. **stated** ("a proof question").
- `retain_type_reference_selection` returns `Ok(())` without recording for builtin/param/local symbols (`:3763-3789`). A selection that should have been recorded but has `exposure == None` is silently not recorded (`:3760-3762`). **open**.
- `arithmetic_results::retain_arithmetic_result_type` returns on unknown shapes (`:660-683`): result is absence of the policy-qualified row, not acceptance. **traced?** no; **open**.
- `integer_family_width` returns `Ok(())` if the base is not an integer family, or the arguments are not exactly one `Named` (`:2299-2307`): a width written as a non-`Named` argument skips the check. **stated** ("A width written as an expression is left for the specialization").
- `lower_fixed_array_type` for a `Const` symbol with `evaluated_value == None` skips the extent check (`:2573`). **open**.
- `generic_origins::validate_computed_arguments` returns `Ok(())` if there are no origins or no materialized expression (`:2671-2687`). **stated** only implicitly; skip is on absence of generated origins.
- `normalize_domain_constraints` with `[domain] = matches` unmatched and not "ambiguous" keeps the constraint with `names_no_visible_domain` instead of failing (`:184-205`). **stated** (`:185-187` comment).

---------------------------------------------------------------------------------------------------

## <03>/src/type_reference_domain_exposure_tests.rs (184 lines, read in full)

**Purpose.** Unit tests for `domain_constraints::select_domain_candidates_where`: that the pool of candidate domains follows import visibility from a referencing source and not the module list.
**Shape.** Fixture builder `fixture`/`fixture_importing_into` (`:18-85`) builds three sources, two `u64::Mark` domains (sources 1 and 2) and registers imports; five `#[test]`s (`:87-184`): broad import ambiguity (`:88`), no invented source scope (`:118`), retained selection without import (`:134`), loaded modules not exposed (`:143`), local domain competes with an import (`:166`).
**How it decides / cases.** Tests assert selection keyed on source import paths and spelling `"u64::Mark"`; `reference(source)` spans use `SourceSpan(SourceId, Span(0,4))`.
**Unused/limits/fail-open.** Test-only; none.

---------------------------------------------------------------------------------------------------

## <03>/src/typing_queries.rs (3299 lines, read in full)

**Purpose.** Read-only queries over already-typed declarations: what type an assignment destination or contract value has, which parameters a call target takes, what the declared type of an operand is; plus the pass `defer_unbound_calls` that records, on the declaration holding it, each call typing could not bind. Header comment (`:1-5`): "They establish no contract, value inhabitation, or body judgment. A missing required type is unavailable rather than reconstructed".

**Shape.**
- `call_target_parameters` (`:39`), `selected_construction_field_type` (`:85`), `collection_element_type` (`:150`), `collection_field_type` (`:166`), `selected_member_value_type` (`:186`), `declared_place_type` (`:222`), `generic_value_type` (`:246`), `declared_callable_value_type` (`:285`), `declared_projection_type` (`:493`), `declared_projection_field` (`:599`), `declared_field_type` (`:669`), `land_owned_fixed_byte_literal` (`:681`, mutates), `scoped_expression_type_reference` (`:767`).
- `DeclaredCallTargets` (`:955`) with `declares` (`:978`), `named_call` (`:1061`), `named_expression_call` (`:1080`), `unserved` (`:1112`).
- `defer_unbound_calls` (`:1172`): scans every expression and every machine statement for unbound names/calls and records `DeferredRequest` rows. Helpers `unbound_call_path` (`:1139`), `complete_byte_domain_selection` (`:1513`), `case_call` (`:1574`).
- `declared_expression_type` (`:1609`) and `DeclaredExpressionTypes` (`:1620`, a memo `HashMap<(SymbolHandle, ExpressionHandle), Option<TypeReferenceHandle>, BuildIndexHasher>`), `core_integer_result` (`:1838`).
- Inline tests `:1884-3299` (read in full).

**Inputs and outputs.** Input: `&TypedTrees` (`expression_table`, `statement_table`, `symbols`, `type_reference_table`, `machine_*`, `domain_definitions`, `proof_facts`, `authored_declaration_selections`). `defer_unbound_calls` mutates the program: `record_deferred_refusal`, `record_missing_name`, and finalizes late-bound authored selections (`:1456-1499`). `land_owned_fixed_byte_literal` rewrites a `String` expression into `ArrayLiteral` and sets its constructed type (`:732-735`).

**How it decides.**
- `call_target_parameters`: keyed on the target's symbol identity, trying in order closed requirement application, state, machine, machine parameter signature, operator definition, a single trait signature matching `signature.symbol == target` (`:66-75`), evidence requirement (`:39-83`).
- `declares` (`:978`): keyed on symbol kind (`MathematicalDefinition`, `BuiltinFunction`, `MachineParameter`) and receiver symbol; `FixedCoreDeclaration::Identity` is the only fixed core declaration that is callable (`:1012-1019`).
- `defer_unbound_calls` classifies a call as `UndeclaredCallName` when the request is `UnresolvedCallTarget|ReceiverCallSelection` and the recorded resolution outcome (`bare`) equals `UnresolvedName::NoVisibleDeclaration` (`:1439-1448`): keyed on resolution's retained outcome, not on spelling. `case_call` (`:1574`) uses `find_child_by_name_and_kind(owner, written, Variant)` on the call's written name text `call.target.as_str()` (`:1590`): a spelling lookup.
- Byte-domain primitive calls are exempted by membership of the call's expression handle in `byte_domain_clauses` (`:1227-1235`), built from `primitive_byte_slice_domain` of each domain (`:1189-1204`).
- `unbound_call_path` (`:1139`) builds a `a::b` path by splitting `Identifier` text on `"::"` (`:1148-1151`), returned only when the resolution outcome is `NoVisibleDeclaration`.
- `declared_expression_type` read path (`:1647-1833`): a retained constructed type first; then requirement-signature or call-return types; then by `ExpressionNode` kind.
- `DeclaredExpressionTypes::get` inserts `None` before reading (`:1641`) so a cyclic query returns `None`.

**Cases by shape.**
- `declared_callable_value_type` serves: suffixed literals, `old(x)` chains, named constants, then data-declaration clause values, then machine/state/trait-signature contract values (`:285-491`); everything else errors with `unavailable_judgment`.
- `declared_projection_type` accepts a spine of `Member`, `Unary::Dereference`, `Indexed` (non-range) ending in a `Name`; `_ => break` then error "assignment storage expression" (`:507-597`).
- `scoped_expression_type_reference` handles `Name, Member, Indexed (non-range), Cast, SavedSubject, Atomic, Borrow, Block (with tail), Match, Call`; `_ => None` (`:795-930`).
- `DeclaredExpressionTypes::read` handles `SavedSubject, Member, Integer/Float/Boolean/Name/Indexed/Call, Cast, Unary Dereference/BitwiseNot, Binary arithmetic`; `_ => None` (`:1718-1832`).
- `core_integer_result` handles shifts, equal carriers, policy combination (`:1838-1881`).

**Same job elsewhere.**
- `scoped_expression_type_reference` (`:767`) and `DeclaredExpressionTypes::read` (`:1647`) both compute the declared type of an expression by walking expression kinds, with different coverage (Block/Match/Atomic/Borrow only in the first; Binary/Unary only in the second). Both read.
- `declared_callable_value_type` also appears as the leaf of `read` (`:1732`).
- `Site`/call-site enumeration in `defer_unbound_calls` vs `call_sites` in `<04>/passes/specialization.rs:362` (suspected related; both read; they enumerate expression and statement calls in different ways).

**Unused / one-caller code** (counts are line hits incl. definition):
- `land_owned_fixed_byte_literal` 2 (one caller `<03>/src/call_and_product_selections.rs:2210`).
- `scoped_expression_type_reference` 4; `declared_projection_field` 4; `collection_field_type` 4; `float_literal_atom` 4; `call_target_parameters` 5; `defer_unbound_calls` 5; `exact_declared_field` 6; `declared_expression_type` 7; `selected_construction_field_type` 7; `collection_element_type` 10; `declared_place_type` 18; `declared_callable_value_type` 20.
- None at 0 callers.

**Stated limits.**
- "A missing required type is unavailable rather than reconstructed with the retired checker." (`:3-4`)
- `defer_unbound_calls` doc: "A request in an expression no declaration owns has no reader to raise it and still refuses here." (`:1169-1171`); error "no product route yet: an expression outside every declaration is one ..." (`:1428-1431`).
- `declared_expression_type` comment (`:1599-1608`): "`None` is 'typing does not read this form', never 'no domains': the caller defers the run."
- "A terminal call is examined for the machines of this cohort only, as before." (`:1168-1169`, and `in_cohort` `:1299`, `:1369`).

**Fail-open risks.**
- `defer_unbound_calls` records a deferred row rather than refusing, so a program holding an unbound call is accepted unless some reader later demands that declaration. **stated** (`:1161-1166`: typing "does not know what an entry reaches"). Wrong program that gets through: an unbound call inside a declaration no entry reaches. Refusal is later and keyed on the holder's `DeferredRequest` row.
- `DeclaredExpressionTypes::read` returning `None` and `scoped_expression_type_reference` returning `Ok(None)` mean "not read here"; callers defer. **stated** (`:1599-1608`).
- `defer_unbound_calls` skips terminal-call targets for machines before the frontier unless `completed` (`:1299`, `:1369-1371`). **stated** ("as before"); not traced for stale machines.
- `declares` returns `false` (so "unserved", not accepted) by default, so that direction is fail-closed.
- `call.runtime_call.is_some()` skips target checks after a `declared_callable_expression_type_reference` query (`:1236-1241`). **open**: a runtime call with an unbound callee value is judged only by that query.
- `call.layout_query.is_some()` skipped as a call needing no target (`:1247-1249`). **stated** (`:1242-1246`).
- Names not marked `NoVisibleDeclaration` and with invalid symbol are not recorded (`:1210-1223`): an ambiguous or otherwise unresolved value name is not deferred here. **open** (only `NoVisibleDeclaration` is recorded; other outcomes produce no row from this loop).
- `complete_byte_domain_selection` is an authenticating check that returns `Ok(())` only after `byte_predicate_call` matches (`:1555-1566`); fail-closed.

---------------------------------------------------------------------------------------------------

## <03>/tests/block_values.rs (74 lines), positional_patterns.rs (166), subjectless_match.rs (48), support/positional_fields.rs (224)  (read in full)

**Purpose.** Handoff tests of what stage 03 gives stage 04: block-valued match arms own their statements and tail names the block's local (`block_values.rs:19`); positional patterns get the declared field at each place and a roster with one row per field (`positional_patterns.rs:29`, `:57`), and unplaceable patterns are refused at the pattern with exact messages (`:98-166`); a subjectless `match` has `Guard` arms over parameters (`subjectless_match.rs:20`). `support/positional_fields.rs` provides `typed(text)` (`:51`), `field_reads` (`:114`), `let_roster` (`:183`), `arm_roster` (`:205`), and the program `MAIN` (`:28`).
**Inputs/outputs.** Parse -> resolve -> `typing::lower_symbol_resolved_trees` via the `source_compiler::test_support` re-exports (`positional_fields.rs:24`); `block_values` and `subjectless_match` reuse `<05>/tests/support/dispatch_arms.rs` through `#[path]` (`block_values.rs:12`, `subjectless_match.rs:12`).
**How it decides.** Assertions are keyed on field symbols and positions (`positional_patterns.rs:41-53`) and on exact message substrings plus the source text span of the refusal (`:162-164`).
**Cases by shape.** Table of eleven refusal bodies (`:103-155`): arity too small/large, sum subject, non-record, wrong record name, `Shape { radius }` on a sum, unknown field with `..`.
**Same job elsewhere.** Stage 05's side: `<05>/tests/block_values.rs`, `<05>/tests/subjectless_match.rs`, `<05>/tests/positional_field_reads.rs` (named in comments, not read).
**Stated limits / fail-open.** None seen; `Box::leak` of format strings in `positional_patterns.rs:156` is test-only.

---------------------------------------------------------------------------------------------------

## <04>/passes/mod.rs (6 lines), prepare.rs (188), domain_applications.rs (212), record_patterns.rs (466)  (read in full)

**Purpose.**
- `prepare.rs`: the stage entry `prepare_typed_trees(program, values, selected_providers)` (`:33`): runs specialization, `prepare_conformance_requirement_headers`, `settle_applied_service_lineage`, `domain_applications::prepare`, `record_patterns::prepare` (`:38-46`), then freezes with `PreparedTypedTrees::new(PREPARATION, program, values)` (`:47`). The pass list is hard-coded in this function; the `mod.rs` doc comment says "Their order is the list in `preparation.rs`" (`passes/mod.rs:2`), but no `preparation.rs` file is in this unit (grep not run; file list shows only `prepare.rs`).
- `domain_applications.rs`: for each retained domain application, builds the applied `DomainDefinition` by substituting arguments into its carrier, facts and cleanup selection (`apply` `:24`, `apply_cleanup_selection` `:80`).
- `record_patterns.rs`: gives each `let { a, b as _ } = subject;` marker the subject record's symbol and each roster row its field symbol (`prepare` `:19`).

**Shape.** `prepare_typed_trees` returns `Result<PreparedTypedTrees, Vec<Diagnostic>>`. `domain_applications::prepare(&mut TypedTrees)` loops over `applied_domain_applications` by position (growth-tolerant, `:9-21`) and stores `theory: Result<DomainDefinition, &'static str>` per row via `set_theory` (`:16-18`). `record_patterns::prepare` collects `resolutions: Vec<(StatementHandle, SymbolHandle, Vec<TableMatchField>)>` first, then writes (`:20-63`).

**How it decides.**
- `domain_applications::apply` refuses (stores an `Err` string on the row) when the declaration is missing (`:32`), the index telescope is not direct (`has_direct_index_telescope` `:34`), or argument count differs from the telescope length (`:37`). Errors become row state, not diagnostics; a later reader sees the `Err`.
- `record_patterns::subject_record` (`:88`) decides the subject record from, in order: `declared_binding_type(state, initial_value)`, `field_path_type` (member with `case_variant.is_none()` and a unique declared field by symbol, `:117-134`), then the marker's own `type_reference` (`:93-101`). It then requires `closed_data_definition`, no type parameters, no quotient, and all members `DataMember::Field` (`:102-110`). Row field symbols are found by `named_field`: field `name.as_str() == name` and exactly one (`:67-77`): this is a lookup by field **spelling**. Rows whose name matches none or two fields get `SymbolHandle::invalid()` (`:74`).
- An unresolved subject pushes `(handle, invalid, [])` (`:35`); the write phase sets `pattern_subject = invalid` and leaves `pattern_fields` unchanged (`:54-61`).

**Cases by shape.** `record_patterns` serves plain records only: "A call result, an open application, a quotient and a sum stay unresolved" (`:84-87`). `domain_applications::apply` serves domains with a direct type-parameter index telescope.

**Same job elsewhere.** `record_patterns.rs` names a field by spelling while stage 03 already gave positional patterns field symbols (`<03>/tests/positional_patterns.rs:41-53`, `:81-83`: "Preparation gives them their symbols from these names"). Crate map and `OWNER_CODE_CONCERNS` call this a duplicate (map `02-...:63,73`, not re-read by me). `data_applications::prepare_from` (used in `<04>/passes/specialization.rs:206`, `:1830`) is a related pass defined outside this unit.

**Unused / one-caller code.** `record_patterns::prepare` and `domain_applications::prepare` are each called once from `prepare.rs:45-46` (grep not run beyond reading).

**Stated limits.** `record_patterns.rs:7-9`: "Neither is an error here: the checker refuses both where a program reaches the pattern."

**Fail-open risks.**
- `record_patterns::prepare`: an unresolved subject, a field name that matches no field or two fields, all leave the marker/row unresolved and **accept** (`:7-9`, `:35`, `:74`). **stated** ("the checker refuses both where a program reaches the pattern"); the checker's refusal was not traced.
- `domain_applications::prepare`: an `Err(&str)` is recorded on the row and not raised (`:16-18`); acceptance depends on a later reader checking `theory`. **open**.

---------------------------------------------------------------------------------------------------

## <04>/passes/specialization.rs (2375 lines, read in full)

**Purpose.** The static-argument specializer. For each call `f::<A,...>(...)` it pairs arguments with the callee's binders (`bind`), and if closed it copies the callee declaration tree with fresh symbols and substitutions, generates one machine per distinct closed application, and points the call at it. Header comment `:1-51` lists "Served today": type parameters given plain named types, implementation parameters given plain named implementations, structural callable parameters given exact named machines, and value parameters given one integer literal or `true`/`false`, on single-state checked bodies with lexical children.

**Shape.**
- `prepare_with_providers` (`:91`): worklist loop over `program.machines()` (growing); per machine: `reflected_fields::elaborate` (`:117`), then each `call_sites` entry through `named_conformances::serve` else `serve` (`:140-149`), then selected-requirement preparation (`:150-169`). Results mapped on `Stop::{Unserved, Rejected, Defect}`: `Unserved` records `DeferredRequest::StaticApplication` on the holder (`:172-179`); `Rejected` records `InvalidStaticApplication` if the holder is "conditional" (active-case branch or descendant) else returns an error (`:181-193`); `Defect` returns an error (`:194`). Ends with `data_applications::prepare_from(program, first_type)` (`:206`).
- `GeneratedMachines` (`:228`): `BTreeMap<Reverse<ApplicationKey>, Generated>`; `ApplicationKey{template, requirement, arguments: Vec<Identity>}` (`:212-218`). One application is one machine.
- `Site` (`:262`) = expression or statement call; methods `call`, `applied`, `source_span`, `requirement_application`, `enter` (`:280-358`). `call_sites` (`:362`). `entered_machine` (`:398`).
- `serve` (`:410`), `specialize_application` (`:561`), `close_machine_argument` (`:636`), `remaining_arguments` (`:742`), `enter_generated` (`:845`), `bound_arguments` (`:934`), `bind_requirement` (`:1000`), `apply_requirement` (`:1061`), `Selection`/`selections` (`:1135`, `:1144`), `template` (`:1227`), `copied_parts` (`:1293`), `copy_expression` (`:1460`), `has_unserved_type_expression` (`:1478`), `CopiedDeclarations` (`:1494`), `copy_progress` (`:1504`), `copy_declarations` (`:1543`), `generate` (`:1842`), `record_symbol` / `copied_symbol` (`:1925`, `:1941`), `substitute_widths` (`:1957`), `substitute_values` (`:2041`), `select_requirements` (`:2072`), `copied_record_receivers` / `substitute_record_receivers` (`:2120`, `:2185`), `substitute_arguments` (`:2272`).
- Submodules: `callable_binder`, `named_conformances`, `provider_methods`, `reflected_fields` (`:81-84`); test modules `tests`, `implementation_tests`, `lifetime_tests` (`:2368-2375`).

**Inputs and outputs.** Reads and mutates `TypedTrees`: appends machines, generated symbols (`insert_generated_root_from` `:1595`), `machine_specializations` receipts (`:1904`), `captured_machine_states`, `ranking_expression_custody` (`:1536`), `deferred_refusals`; rewrites call `target_symbol` and `machine_arguments` (`enter` `:342-358`). Uses `typed_trees::static_operand_binding::{Operand, bind, Served, Stop, ...}`.

**How it decides.**
- Generated-machine identity is keyed on `(template symbol, requirement symbol, Vec<Identity>)` where `Identity` is an operand identity (`:212-218`, `:630`); symbol order by `(arena_index, generation)` (`:255-257`), not by spelling. The generated machine's name is a `format!("{}$specialized${spelled}${}", ...)` (`:1590-1594`) but it is a label; lookup is by key.
- `serve` decision order: skip if no application or the call is "selected otherwise" (`:414-416`); closed-requirement calls are validated and return (`:417-442`); a bodyless boundary machine takes `prepare_bodyless_callable_application` (`:447-459`); a trait requirement goes to `apply_requirement` (`:460-462`); empty args return (`:463-465`); non-machine target returns (`:470-472`); a machine with a template and no callable parameters enters generated (`:473-482`); evidence requirement -> `bind_requirement` (`:483-490`); otherwise specialize.
- `copied_parts` decides whether a machine is copyable (`plain` `:1308-1337`): `body_is_present`, `CheckedBody` supply mode, no owned data, no `satisfies`, `invokes` span present, no attached data, no `target`, no pending structural equations, not installation-bound, `termination_plan.checked_summary == NoGuarantee`, no installation-bound service reach rows, no proof-output calls, no evidence forwardings. Anything else -> `unserved("the declaration copier does not yet retain this machine's complete interface")` (`:1338-1342`).
- A copied statement is accepted only for kinds `Assignment|Expression|Evaluate|Transition`, `Call` (without installed interrupt or reflected field origin) and `LocalData` (without reflected query origin); `Assembly|AssemblyFact` refused (`:1395-1411`).
- Copied expression metadata check: `Call` with `native_code_layout`, `quotient_operation`, `private_layout_operation` or `installed_interrupt_operation` is refused; `Cast` with non-empty `semantic_domain_arguments` is refused (`:1434-1446`).
- `substitute_values` keys on `path.symbol` being the value parameter, no static application and exactly one path member (`:2052-2058`). `substitute_widths` keys on `integer_family(base_symbol)` and argument `Named{symbol == operand.parameter}` (`:1978-1994`).
- `select_requirements` rewrites calls whose `target_symbol == placeholder` (`:2080-2101`).
- `remaining_arguments` identifies the original binder by `program.symbols.get(parameter.symbol).generated_from` (`:821`) and `symbol_order` binary search (`:795-808`).

**Cases by shape.** Quoted: "Served today: type parameters given plain named types, ..." (`:24-29`). `selections` (`:1144`): a requirement of an implementation operand must have only `Const` type parameters, no lifetimes, no conformance bounds, and exactly one implementation row (`:1176-1202`). `bind_requirement` only when all requirement type parameters are `Const` (`:1015-1022`). `template` (`:1227`) refuses a nested declaration whose independent binders are not `Type` (`:1252-1268`).

**Same job elsewhere.** `copy_declarations` is the "one declaration copier" (comment `:1492`); `provider_methods.rs:106` and `named_conformances.rs:148` both call into it through `specialize_application` / `copy_declarations`; reflected_fields/active_cases build synthetic machines by a separate path `GeneratedMachine::install` (`active_cases.rs:528`) rather than the copier. Binder satisfaction is checked via `callable_binder::{satisfies_binder, binder_shape}` (`:1873`, `reflected_fields.rs:201`, `active_cases.rs:126`).

**Unused / one-caller code.** The only `pub(crate)` entry is `prepare_with_providers` (`:91`); `prepare` (`:87`) is `#[cfg(test)]`. `symbol_order` is shared by sibling modules (`provider_methods.rs:4`).

**Stated limits.** `:21-29` ("An application this pass does not serve keeps the call as typing left it and records a deferred refusal"); `:37-39` "an argument that a machine would have to compute is unserved"; `:1339-1341` "does not yet retain this machine's complete interface"; `:1250-1251` "Other independent binder kinds need their full interface copied."

**Fail-open risks.**
- `Stop::Unserved` becomes a deferred refusal on the holder only (`:172-179`); a closed application that is unserved leaves the call as typing wrote it and the program is accepted if nothing demands the holder. **stated** (`:21-23`: "only a program that demands that machine is refused").
- `serve` returns `Ok(())` early for: no application; `selected_otherwise`; empty arguments; target not a machine; `parameters.is_empty()`; a declaration-query call (`:414-416`, `:463-465`, `:470-472`, `:495-497`, `:501-507`). Each leaves the call unchanged. **stated** in comments at `:466-469` ("Only an application of a declared machine with parameters is a request to specialize ...") and `:498-500`.
- `specialize_application` returns `Ok(None)` when `open_map || open_callback || any operand open` (`:610-615`), leaving the call to be closed later in generated machines. **stated** (`:605-609`).
- `Stop::Rejected` in a "conditional" holder is downgraded to a deferred `InvalidStaticApplication` row instead of failing the program (`:127-134`, `:181-188`). **stated** (`:97-99` comment: "Keep a rejection with its branch").
- `copy_progress` `symbols.lookup(*field).unwrap_or(*field)` (`:1519`): a projection field with no rebinding is kept as is. **open**.
- `substitute_arguments` leaves calls with `build_selection.is_valid()` untouched (`:2315`), by design (`:2262-2271`). **stated**.

---------------------------------------------------------------------------------------------------

## <04>/passes/specialization/callable_binder.rs (455 lines, read in full)

**Purpose.** Structural comparison of a callback machine against a callback binder (the published callable contract of a static machine parameter), used by specialization and reflection expansion. Header: "Both use the published contract even when a particular generated call could meet a stronger requirement. A comparison needing a proof stays deferred." (`:2-3`).
**Shape.** `enum Bound{Type(TypeReferenceHandle), Parameter(SymbolHandle)}` (`:20`); `satisfies_binder` (`:32`), `binder_shape` (`:124`), `properties_match` (`:238`), `admits_types_only` (`:264`); two tests (`:298`, `:419`).
**How it decides.** `binder_shape` refuses/defers: non-empty conformance bounds, `invokes`, native callback parameters or non-`Type` binder parameters -> `unserved` (`:140-150`); static-parameter count/kind mismatch -> `Rejected` (`:151-167`); parameter count mismatch (`:181-187`); then `compare_callback_signature_types` and maps `CallableShapeMismatch` to messages (`:202-230`). `satisfies_binder` then: any non-`Crashes` binder contract -> `unserved` (`:60-67`); a callback `requires` clause that is not types-only (`admits_types_only`) or, when the binder has no type parameters, not statically true under substitutions -> `Rejected` (`:73-94`); callback crash route vs binder `declares_no_crash_route` -> `Rejected` (`:98-102`); explicit binder crash routes -> `unserved` (`:103-110`); suspend/block excess -> `Rejected` (`:111-113`); binder termination promise -> `unserved` (`:114-118`).
**Cases by shape.** `admits_types_only` accepts a fact only if every node is `TypeExpression|Boolean|Binary|Unary` (`:280-286`). Stated: "A binder contract that only a refinement proof could discharge is unserved." (`:30-31`).
**Fail-open risks.** `properties_match` returns `true` if the callback does not demand stronger properties (`:244-246`). `satisfies_binder` ignores callback `ensures`/other contracts and binder `ensures`: binder contracts other than `Crashes` refuse (`:60-67`) but callback-side extra postconditions are not looked at. **traced** partially (read only this file): extra callback guarantees cannot weaken a caller. The callback's own `reaches`/service reach, `blocks` aside from `suspends/blocks`, is not compared here (binder `invokes`/native callbacks cause `unserved` at `:140-143`; callback-side `reaches` not read here). **open**.
**Same job elsewhere.** `compare_callback_signature_types` is in `typed_trees` (not read). Callers: `specialization.rs:1873` (binder_shape), `reflected_fields.rs:201`, `active_cases.rs:126-149` (satisfies_binder).
**Unused/limits.** Both `pub(super)` functions have callers in sibling modules (above). Comment `:95-97`: private callback's inferred routes "exist after lowering, so Terminal Psi closure compares them".

---------------------------------------------------------------------------------------------------

## <04>/passes/specialization/named_conformances.rs (175), provider_methods.rs (132)  (read in full)

**Purpose.** `named_conformances::serve` (`:10`) closes each demanded generic member of a named conformance (an implementation map) at the arguments from an explicit or lexical call by copying the member's declarations (`copy_member` `:111`) and recording a `CapturedMachineApplication` (`:98-103`). `provider_methods::SelectedMethods` (`:11`) prepares, for each selected provider's generic boundary method, the application used by each closed requirement call (`prepare_applications` `:45`).
**Inputs/outputs.** Mutate `TypedTrees.captured_machine_applications`; call `copy_declarations`/`specialize_application` (`named_conformances.rs:148`, `provider_methods.rs:106`).
**How it decides.** `named_conformances::serve` returns `Ok(false)` (not handled) when the call is not a conformance member, the member's implementation has no type parameters (`:46-48`), or it is the "symbolic map scope" case with no authored application (`:104-107`); `Ok(true)` with no copy when the qualifier is open (`:57-59`). `provider_methods` keys on `symbol_order(closed.template)` binary search over providers sorted by requirement (`:56-61`) and `requirement_application_matches_contract_scope` (`:67-71`).
**Cases by shape.** Requires `member.row.source.owns_realization()` else `unserved` (`:50-54`); machine with its own type parameters refused (`:120-124`); argument count must equal conformance parameters (`:133-137`); a provider only counts if its realization machine has type parameters (`provider_methods.rs:30-35`).
**Fail-open risks.** `provider_methods::prepare_applications` `scope.is_some_and(...)` false silently skips a provider/requirement pair (`:67-72`). **open**. `.expect("the selection retained its member")` (`:79-81`) panics, not accepts. `named_conformances` `Ok(false)` falls through to the ordinary `serve` (`specialization.rs:141-148`).
**Unused / one-caller.** `named_conformances::serve` 1 caller (`specialization.rs:141`); `SelectedMethods::new`/`prepare_applications` each 1 caller (`specialization.rs:103`, `:107`).

---------------------------------------------------------------------------------------------------

## <04>/passes/specialization/reflected_fields.rs (796) and reflected_fields/active_cases.rs (717)  (read in full)

**Purpose.** Expand reflection visitor calls (`inspect_record_fields`, `visit_runtime_fields[_mut]`, `visit_active_fields[_mut]`) into ordinary calls before specialization continues. `elaborate` (`reflected_fields.rs:30`) loops over a machine's states, finds the first statement `Call` whose target is a core reflection declaration (`reflection_operation` `:83`, keyed on `program.core_vocabulary(machine.symbol)`), and replaces it (`replace` `:758`). Records: `close` (`:117`) emits one `__reflect_*` local and one callback call per included field. Sums: `active_cases::expand` (`active_cases.rs:54`) generates a dispatcher machine and one branch machine per case.
**Shape.** `RecordQuery`, `ReflectionOperation` (`:72-81`); `Record`/`record_fields` (`:469`, `:476`), `context_type` (`:534`), `Callback`/`callback` (`:601`, `:607`), `description_type` (`:637`), `FieldQuery`/`field_query_entry` (`:672`, `:678`), `local` (`:727`), `CallbackSite::{field, invoke}` (`:308-419`). `active_cases`: `Sum`, `ActiveCase`, `sum_description` (`:307`), `sum_application` (`:370`), `GeneratedMachine::{new, install}` (`:481-553`), `empty_dispatch` (`:555`), `case_arm` (`:579`), `case_literal` (`:639`).
**How it decides.**
- `record_fields` requires `subject.symbol` kind `Data`, no application/const literal, no type parameters, no lifetime parameters, no quotient (`:483-505`), all members `Field` (`:507-514`) and same source package as the holder (`:515-522`). Otherwise `unserved`.
- `context_type` compares the written context type with the borrowed binding's type via `descriptive_type_identity_in_region_scope` identity (`:574-592`); mismatch -> `Rejected` (`:590-592`).
- `field_query_entry` selects `reflect::field` as the unique machine in the **same source file** (`source_id` equality) as the inspection whose `core_vocabulary == Reflection(Field)` (`:679-695`).
- The local names are `format!("__reflect_{role}#{}.{}", site.arena_index(), statements.len())` (`:737-741`), generated labels. Generated dispatcher/branch names are `visit_cases#{site}` / `visit_case#{site}.{n}` (`active_cases.rs:176`, `:196`), labels only; identity is the symbol.
- `active_cases::expand` requires exactly 4 binders (`:77-81`), arguments 3 or 4 (`:88-98`), exactly two call operands (`:99-101`); `sum_description` requires same source package (`:315-322`), schema shape `Sum` else `Rejected("active-case visitation takes a sum. ...")` (`:327-332`). A per-case field-binder refusal is attached to that branch's generated machine as a deferred refusal (`:138-166`, `:199-201`).
- The dispatcher is a sequence of structural `Transition` statements with a `CaseMembership && true` guard (`case_arm` `:579-637`); an empty sum gets `empty_dispatch` (`:277-279`).
**Cases by shape.** Record visitation: "a sum is inspected with its active-case visitor" (`:510`) and an applied/quotient record "is not served yet" (`:503`). Erased fields are skipped for runtime visits (`:183-185`), included for `Inspect`; erased payload fields skipped in sums (`active_cases.rs:349`). `CaseInfo` must have exactly 7 fields (`:687`).
**Same job elsewhere.** Generated machines are created by `GeneratedMachine` (`active_cases.rs:481`) and by `copy_declarations` (`specialization.rs:1543`): two creation paths. The `local` helper in `reflected_fields.rs:727` and hand-built `TableLocalData` in `active_cases` share the `local` function only via the `CallbackSite::invoke`.
**Unused/one-caller.** `elaborate` 1 caller (`specialization.rs:117`). `active_cases::expand` 1 caller (`reflected_fields.rs:57`).
**Stated limits.** `:503` ("not served yet"), `:520` ("needs an owning operation"), `:198` ("needs a refinement proof for its full interface").
**Fail-open risks.** `case_refusals` turns a per-case `Stop::Rejected` into a deferred row on that branch (`active_cases.rs:156-158`), so a failing payload candidate is accepted unless the branch is reached. **stated** (`:135-137`). `reflection_operation` returns `None` for any other `core_vocabulary` (`:102`). In `replace`, `unreachable!` if site isn't a call (`:772`).

---------------------------------------------------------------------------------------------------

## <05>/lib.rs (10 lines), src/boundaries.rs (319), src/body.rs (1471)  (read in full)

**Purpose.** Stage 05's closed body representation and boundary publication. `lib.rs` sets the crate root: `#[path = "src/lib.rs"] pub mod lowering;` (`lib.rs:9-10`) re-exporting selected modules from other crates (`:3-7`). `body.rs` defines the closed body the reader produces and lowering consumes (header `:1-15`): `ValueType`, `Qualification`, `DomainSet`, `Policy`, `RecordType`/`StorageShape`/`CaseType`/`RecordField`, `Signature`, `Body`/`Statement`/`Expression`/`ExpressionKind`, `CallTarget`, `Selector`, `CaseSelector`. `boundaries.rs` holds `BoundaryNamespace` (rows of `BoundarySource`) and publishes each boundary requirement as a `BoundaryMachineDeclaration` into the Terminal Psi module.

**Shape (body.rs).**
- `ValueType` (`:52`): `Scalar(ScalarType, Qualification) | Record(StructuralTypeId, Qualification, Regions) | Rigid(RigidType)`. `Regions(u32)` interned tuple (`:69`). `Qualification{policy, domains}` (`:86`). `DomainSet` (`:124`): inline sorted array of `CAPACITY = 4` `(arena_index, generation, identity)` triples (`:140`); `with` returns `None` when full or present (`:178-189`).
- `RecordType` (`:338`), `StorageShape` (`:375`: `Opaque, TypeParameter, Record, Sum, ByteSequence, Reference, ElementView, FixedArray, AtomicCell, PrimitiveScalar`).
- `Signature` (`:541`): parameters, `erased`, `borrowed`, `result`, `membership_ensures`, `requires`, `ensures`, `crashes: CrashCeiling`, `reaches`/`invokes: Ceiling`, `termination`, `ranking`. Helpers `formal_values` (`:841`), `formal_value_ids` (`:862`), `Signatures` (`:622`), `Axis` (`:874`).
- `Body{slots, statements}` (`:925`), `Statement` (`:930-1030`: `ParkedBuildLocals, Bind, CaptureReference, Witness, Declare, Decompose, Store, Evaluate, Dispatch, Arm`), `ExpressionKind` (`:1090-1319`, about 38 variants).
- `boundaries.rs`: `BoundarySource` (`:19`), `BoundaryNamespace{rows: IdTable<BoundaryMachineId, BoundarySource>}` (`:30`), `outcome` (`:43`), `publish` (`:65`).

**Inputs/outputs.** `body.rs` is data and small pure helpers. `BoundaryNamespace::publish` reads signatures/outcomes/records/callables/services/`declarations: Option<&read::Program>` and mutates `module.interfaces.boundary_machines` and `module.identities` (swapped through `qualifications.identities` `:86`, `:316`). Returns `Checked<()>`.

**How it decides.**
- `ValueType` equality includes qualification and regions, so type equality for every typing rule is `==` (`body.rs:51`, comment `:30-50`).
- `BoundaryNamespace::publish` (`:65`) refuses (via `RefusalReason::Unsupported`): a repeated publication (`:83-85`), an outcome-specific guarded guarantee (`:99-101`), retained erased actual lanes (`:102-105`), missing prepared lifetime context or unresolved result lifetime frontier (`:107-113`), a structural result with reference sources (`:120-124`), no unique native requirement association (`:241-244`).
- Execution kind is keyed on `reader.typed.symbols.sealed_intrinsic(source.source)`: `LayoutTargetFacts`, `FloatOperationRequirement` (numeric primitive), `HostedPrimitive(op)`, else `Provider` (`:156-194`). Compiler-owned executions go through `declare::boundary`; others through `service_declaration_commitment` (`:195-267`).
- `BoundaryNamespace::outcome` returns `None` if crashes are `Inferred` or reach/invokes `Inferred` (`:45-62`).

**Cases by shape.** `DomainSet::CAPACITY = 4`: comment `body.rs:121-123`: "a type naming more than `DomainSet::CAPACITY` declared domains is refused by the reader". The `Signature` doc says a clause naming a "record formal ... skipping" scalar formals uses a numbering convention (`:545-551`). `Statement::Declare` doc: "A constrained scalar type is refused by the reader" (`:969-971`). `StorageShape::length_operation` (`:421`) serves only `ByteSequence::BorrowedView` and `ElementView` with `None | SharedBorrow | MutableBorrow` access.

**Same job elsewhere.** Two numbering helpers `formal_values` (`body.rs:841`) and `formal_value_ids` (`:862`) both delegate to `terminal_check_tables::contracts::formal_value_ids`; `FormalValueShape` (`:856`) is used with the second. `Signature::static_signature` (`:889`) duplicates the type projection from `ValueType` to `StaticValueType` (once).

**Unused / one-caller code** (grep counts incl. definition):
- `Signature::runtime_parameter_position` 2 (one caller `<05>/src/read/services.rs:1044`).
- `Signature::structural_result_place` 2 (one caller `<05>/src/lower/calls.rs:612`).
- `formal_value_ids` 6, `FormalValueShape` 6, `source_parameter_position` 6, `canonical_contract` 6, `fixed_array_extent` 4, `length_operation` 8, `Ranking::without_range` 8, `source_multiplicity` 8, `SourceByteDomain` 4, `declared_crash_routes` 23, `self_place` 31.
- Comment "The one numbering of a machine's formal values, shared by the reader ... and lowering" (`:833-835`) is on `formal_values`; its users outside `body.rs`: not counted by me separately (the 6-hit count for `formal_value_ids` is a different name).

**Stated limits.** `body.rs:1-15` ("a construct the reader cannot express here is refused there, never approximated"); `Statement::Witness` doc: "the reader refuses any other initializer" (`:955-957`); `boundaries.rs:121-122` "boundary reference results need retained public region declarations"; `:223-225` n/a.

**Fail-open risks.**
- `BoundaryNamespace::publish`: `ExecutionKind::Provider` is the default for any intrinsic that is not one of three listed (`boundaries.rs:193`). **open**: a sealed intrinsic added later would silently be published as a Provider boundary.
- `DomainSet::with` returns `None` when full (`body.rs:180`) rather than silently dropping, with a test asserting it (`:1449`). fail-closed.
- `unconditional_termination` matched with `premises.is_empty()` (`boundaries.rs:289`); **open** only in the sense of what other termination values publish as false.
- `outcome` `None` when anything is inferred (`:45-62`): callers' handling not traced. **open**.

---------------------------------------------------------------------------------------------------

## <04> specialization test modules: tests.rs (3112), implementation_tests.rs (1270), lifetime_tests.rs (591)  (read in full)

**Purpose.** Hand-built typed-tree fixtures that run `prepare` (the `#[cfg(test)]` wrapper `specialization.rs:87`) and assert what specialization produced. They build the typed program directly (symbols, machines, calls) instead of parsing source, so they test the pass in isolation.
- `tests.rs` (header `:1-18`): `forward<T>`, `relay<T>`, `sized<const N: u64>`, `main`, `counted`. Fixture builders `source*` (`:226-593`). Tests cover: one machine per closed argument tuple (`:731`), a name like `schema` with no query identity still specializes (`:813`), copied pattern types owned per application (`:841`), refusal of missing/foreign/ambiguous receipts (`:934`), key built from bound arguments not generated names (`:985`), applications inside generated bodies (`:1049`), literal value parameters (`:1130`) and one machine per value (`:1202`), unserved application refused on its holder only (`:1232`; detail text "a static scalar requires its canonical closed value" `:1262`), nested/lexical child declarations (`:1365-1607`), reach rows copying (`:1677`, `:1733`), copied progress/ranking custody (`:1755`), `GeneratedMachines` key index with 300x3 requests (`:1878`), callback application closure through `close_machine_argument` (`:2117`), statement callbacks (`:2379`), closed callback requirements retained (`:2426`), invocation rows (`:2560`), layout queries (`:2641`), structural type operands (`:2710`), open callbacks (`:2785`, `:2976`), constructed type args (`:3045`).
- `implementation_tests.rs` (header `:1-18`): `trait Step`, conformances `Hold`, `Held`, `Empty`, `advance<Selected satisfies Step>`, `main`, `partial`. Tests: call through an implementation parameter enters the selected operation (`:552`), missing requirement row refused on its holder only (`:649`), typed Const record binding and exact named call incl. cycle rejection (`:673`), forwarded Const value and "another declared schema" error (`:974`), nested const records keep root owner and distinct callable identity (`:1033`), frames refuse foreign owners/prefixes/suppliers (`:1170`).
- `lifetime_tests.rs` (header `:1-2`): `borrowed<T>` with a child machine, `caller` with two applications at lifetimes `input`/`other`. Tests: copied lifetimes keep root/captured/child owners (`:310`), each call group preserved without specializing by region (`:389`), implicit groups left for the call checker (`:420`), malformed explicit groups refused with a deferred row and no new symbols (`:447`), borrowed type arguments share code (`:489`).
**Inputs/outputs.** In-memory `TypedTrees` built with `SymbolTableBuilder`; assertions read `machine_specializations`, `deferred_refusal(holder)`, `static_machine_application_bindings`, `ranking_expression_custody_for`, etc.
**How it decides (what the tests key on).** Identity (symbol handles, `generated_from`, receipts) rather than names, with two exceptions: the generated machine name `sized$specialized$4$...` is asserted by prefix (`tests.rs:1228`) and `program.symbols.name(...)` equality is used to show names differ between runs (`:1005-1008`).
**Same job elsewhere.** `tests.rs` and `implementation_tests.rs` each define their own `Source`, `call`, `push_machine`, `only_state`, `calls` helpers (`tests.rs:42-633` vs `implementation_tests.rs:43-549`); `only_state`/`calls` are textually duplicated in both files (read both).
**Stated limits asserted by tests.** `tests.rs:1232-1264` (unserved -> refusal on holder only); `:1733` (installation-bound reach row "remains unserved before copying"); `lifetime_tests.rs:447` (malformed group -> deferred refusal, nothing copied).
**Fail-open risks.** Test-only; the tests pin the deferral behavior in `specialization.rs` (that is, they assert that `prepare(...)` returns `Ok` while a deferred row exists: `tests.rs:1240`, `:1707`, `:1749-1751`, `implementation_tests.rs:657`).

---------------------------------------------------------------------------------------------------

## <04>/tests/*.rs and tests/support/*.rs (1437 lines, read in full)

**Purpose.** End-to-end handoff tests for stage 04 through the real front end (lexer, parser, resolver, typer, `prepare_source_for_test`). Each test program is built so a wrong pairing yields a valid program with a different answer.
- `computed_array_extents.rs` (203): array extent `[u8; east\N]` vs `[u8; west\N]`; typing leaves the extent without a length when the constant is a call; the source driver evaluates it before stage 04; a request reaching only one leaves the other unevaluated (`:107`, `:183`).
- `prepared_invocations.rs` (189): compile-time calls evaluated with their own operands (index, callback static arg, enclosing static arg, negation, call-valued array extent).
- `prepared_value_inputs.rs` (237): `PreparedTypedTrees::constants()` lists the constants a prepared value was computed from; refusals "prepared value inputs belong to another source program" (`:215`) and "source preparation lost a demanded constant declaration" (`:235`).
- `qualified_constant_patterns.rs` (72): a pattern naming `east\ON` keeps that declaration and value.
- `support/constant_array_extents.rs` (236), `support/prepared_package.rs` (304), `support/qualified_boolean_constants.rs` (196): shared builders (`prepare`, `extent_uses`, `Inputs`, `Prepared`, `constant_uses`), each with its own copy of the `compilation(...)`/`TerminalSourceCompilation` builder (read; three near-identical definitions at `constant_array_extents.rs:89`, `prepared_package.rs:53`, `qualified_boolean_constants.rs:78`) and a `#[path]` import of `tests/support/frontend.rs` four levels up (`:17`, `:9`, `:11`).
**How it decides.** Keyed on qualified constant paths (`display_path(symbol, "\\")`) and declaration symbols; two modules declare `N` / `ON` with the same last name to catch last-name lookup.
**Stated limits.** `prepared_invocations.rs:57-65`: index outside array is refused at the call with "cannot prove requires contract for call pick" and `2 < 2`.
**Fail-open risks.** Test-only; none seen.

---------------------------------------------------------------------------------------------------

## Coverage

Every range in the unit file was read in full (final state).

Read in full (line counts from the unit file):
- `<03>/src/type_reference.rs` 1-3923 (3923)
- `<03>/src/type_reference_domain_exposure_tests.rs` 1-184 (184)
- `<03>/src/typing_queries.rs` 1-3299 (3299)
- `<03>/tests/block_values.rs` 1-74, `positional_patterns.rs` 1-166, `subjectless_match.rs` 1-48, `support/positional_fields.rs` 1-224 (512)
- `<04>/passes/domain_applications.rs` 1-212, `mod.rs` 1-6, `record_patterns.rs` 1-466, `specialization.rs` 1-2375 (3059)
- `<04>/passes/specialization/callable_binder.rs` 1-455, `named_conformances.rs` 1-175, `provider_methods.rs` 1-132, `reflected_fields.rs` 1-796, `reflected_fields/active_cases.rs` 1-717 (2275)
- `<04>/prepare.rs` 1-188 (188)
- `<05>/lib.rs` 1-10, `src/body.rs` 1-1471, `src/boundaries.rs` 1-319 (1800)

- `<04>/passes/specialization/implementation_tests.rs` 1-1270, `lifetime_tests.rs` 1-591, `tests.rs` 1-3112 (4973)
- `<04>/tests/computed_array_extents.rs` 1-203, `prepared_invocations.rs` 1-189, `prepared_value_inputs.rs` 1-237, `qualified_constant_patterns.rs` 1-72 (701)
- `<04>/tests/support/constant_array_extents.rs` 1-236, `prepared_package.rs` 1-304, `qualified_boolean_constants.rs` 1-196 (736)

Total read in full: 15,240 + 4,973 + 701 + 736 = 21,650 lines (matches the unit's sum). Skimmed: 0. Not read: none.

## Five things a maintainer most needs to know about this unit

1. Stage 04's specialization is a deferral machine. Anything it cannot serve (`Stop::Unserved`) becomes a `DeferredRequest::StaticApplication` row on the holder and the call is left as typing wrote it (`<04>/passes/specialization.rs:172-179`); only a demanded holder is refused later. A rejected application inside an active-case branch is also downgraded to a row (`:127-134`, `:181-188`).
2. Stage 03's `typing_queries::defer_unbound_calls` does the same for unbound calls: it records a row on the owning declaration and refuses only a call outside every declaration (`typing_queries.rs:1172`, `:1428-1431`). Const-argument admission in `type_reference.rs` defers likewise (`defer_to_holders` `:1170`). So "typing accepted the program" does not mean its unbound calls or unserved const arguments are checked.
3. `type_reference.rs` lowers every type twice, from the tree form (`direct`, `:2336`) and the table form (`table`, `:3362`), with near-identical arms; and selects domains by authored spelling with visibility filtering (`:397-453`), separate from a second spelling-based domain match in `generic_origins::Equality::domain` (`:3287`).
4. `record_patterns.rs` (`<04>`) gives a record pattern's rows their field symbols by field **name** (`:67-77`) even though stage 03 gives positional patterns exact field symbols; unresolved subjects or ambiguous names stay unresolved and the checker is relied on to refuse (`:7-9`).
5. `<05>/src/body.rs` is a data-only closed-body vocabulary; `ValueType` equality (carrier + qualification + regions) is the typing-rule type equality (`:51`, `:30-50`), `DomainSet` holds at most 4 domains (`:140`), and boundary publication defaults any unknown sealed intrinsic to a `Provider` execution (`boundaries.rs:193`).

## What I am unsure about

- Whether the checker really refuses what `record_patterns.rs:7-9` says it refuses; I did not read the checker.
- Whether `preparation.rs`, named in `passes/mod.rs:2`, exists; I did not grep for it.
- The caller counts above are line-hit counts (definition included) and some names are generic enough that hits in other crates could be unrelated; I did not inspect each hit except the four named one-caller cases.
- I did not compare the `direct` and `table` type lowerings line by line; "near copies" is from reading both.
- The crate map's `OWNER_CODE_CONCERNS` claim about `record_patterns.rs` was not re-read.
