# Unit 95 notes

Unit file: units/unit_095.tsv. Covers one range:

- `omega-rust/psi/compilation/validation/src/contract_entailment.rs` lines 36425-54636 (18,212 lines), read in full.

All paths below are relative to `omega-rust/psi/compilation/validation/src/` and refer to `contract_entailment.rs` unless stated. The range starts mid-function inside `scoped_arithmetic::calls::result_observations` (line 36425) and ends mid-function inside `structural_judgment::integer_predicates` (`project_integer_premises`, line 54636). It is a run of nested `pub(crate) mod` blocks. The module boundaries inside it are:

| Lines | Module |
| --- | --- |
| ..37119 | tail of `scoped_arithmetic::calls` |
| 37120-37207 | `formation` |
| 37208-37282 | `predicates` |
| 37283-37788 | rest of `scoped_arithmetic` |
| 37789-37878 | `self_induction` |
| 37879-38188 | `shared_proof_walks` |
| 38189-43107 | `specification_calls` (with `applied_bindings`, `collection_operations`) |
| 43108-43630 | `static_applications` |
| 43631-43633 | re-export of `captured_predicates` |
| 43634-43719 | `strict_arithmetic` |
| 43720-47150 | `structural_case_arms` |
| 47151-54636+ | `structural_judgment`, a series of sub-modules (see section 7) |

---

## 1. `scoped_arithmetic::calls` tail (36425-37119) and `formation` (37120-37207)

**Purpose.** This code gives a call to a function with an `ensures` contract an integer meaning that the arithmetic prover can reason about. The call result becomes a bounded atom. The callee's `ensures` facts become hypotheses over that atom, but only when the callee is "independent" of the caller.

**Shape.**
- `result_uses_are_bound` (36432): walks an expression and returns false if any node names the reserved result and no `Occurrence` binding covers it.
- `CallOccurrence { Expression, Statement{state, ordinal} }` (36458). `post_atom` (36464) builds an atom identity string with `format!("\0scoped-call-formal:{scope}:{expression:?}:{owner:?}:{formal:?}")` (36473) or the `\0scoped-statement-formal:` form (36476). The identity is a debug-format string of handles.
- `ScopedCalls` (36483). Fields: `program`, `context`, `remaining` (step budget, 4096 at 36508), `depth`, `scopes` (memo of original to prepared expressions), `guarantees`, `lengths`.
  - `value` (36521) and `expression` / `expression_inner` (36575 / 36588). `expression` fails past depth 64 (36579); every step spends the budget (`step`, 36516).
  - `completed_guarantees` (36716), `formed_call_guarantees` (36827), `completed_bindings` (36860), `transport_guarantees` (36931), `postorder` (37042), `independent` (37070), `hypotheses` (36555).
- `formation` module: `scoped_declared_integer_application_formation` (37131) and `scoped_integer_expression_formation` (37172). Each builds an `Engine::strict`, installs hypotheses and returns `(IntegerApplicationFormation, ScopedArithmeticValue)`. The first accepts only a `measures()` entry whose body is exactly that one expression (37146-37150).

**Inputs and outputs.** Reads the `CheckingInput` tables (expression, proof facts, contracts) and the selected callee contract (`selected_call_contract`, 41958). Produces `ScopedArithmeticExpression`s whose `bindings` gain an `Occurrence(call)` entry. The entry is a `BoundedInteger { Atom, interval }`. `ScopedCalls.guarantees` accumulates hypotheses, which `hypotheses()` then filters through `Engine::install_scoped_expression` (36568).

**How it decides.**
- A call is bound as a bounded atom only if the callee resolves (`selected_call_contract`, 36623), is not a `DynamicSlot` (36627) and has a fixed-integer result (36631-36637).
- The interval comes from `call_result_interval` only when the callee is "independent"; otherwise it is the primitive's full range (36647-36659).
- `independent` (37070-37117) returns false when the callee is the context machine itself (37082), when it is already active (recursion, 37081), or when any nested call in its `ensures` is not independent. Active depth over 64 also returns false.
- Guarantees are transported only for `Ensures` contracts with `contract.binding.is_none()` and `ProofFact::Expression` facts (36939-36948).
- `And` is split only if `has_builtin_decomposed_guard_meaning` or `has_builtin_parameter_bound_expression_meaning` holds (36952-36981). A guarantee is kept only if `has_builtin_bound_expression_meaning` or the parameter-bound variant holds (36986-37012).
- Keyed on: symbols and handles, plus the strings in `post_atom` (36473, 36476) and the atom identity `format!("\0scoped-call:{scope}:{expression:?}:{:?}")` (36661).

**Cases by shape.**
- Only fixed-integer-result calls get atoms (36631).
- `completed_guarantees` accepts only `CallSite::Expression` or `CallSite::Statement`. `TransitionNamed` returns `None` (36757, 36764).
- For a statement site, the result and result observations must be absent (36747).
- For an expression site, `result` or `result_observations` must be present (36734).
- A `DynamicSlot` callee is refused (36768).
- `declared_carriers_match` and an equal `arithmetic_domain_for_type_reference` are required (36788-36797).

**Same job elsewhere.** `completed_bindings` and `formed_call_guarantees` overlap in what they do (both call `transport_guarantees`, 36856, 36823). `ScopedCalls::independent` (37070) and `requirements_are_acyclic` (42849) are both active/complete-set DFS walks over callee contracts that return false on a cycle. They are different predicates (independence over `ensures`, acyclicity over `requires`). `static_applications::StaticFacts::judge_alternatives` (43451) and `scoped_arithmetic_implication_for_declaration` (37530) both judge arithmetic goals under hypotheses; the former delegates to the latter at 43573.

**Unused or one-caller code (grep).**
- `ScopedCalls::new` / `for_declaration`: 12 in-file uses. They are all inside this file (grep of `ScopedCalls::(new|for_declaration)`: lines 4115, 4364, 31127, 37152, 37181, 37358, 37511, 37536, 37634, 37657, 37732, 37751).
- `scoped_integer_expression_formation` and `scoped_declared_integer_application_formation`: one call site each outside the file, `05_typed-trees-to-checked-trees/src/checks_current_scalar_values.rs:5793` and `:5811`.
- `scoped_arithmetic_implication`: callers in `checks_current_scalar_values.rs` (5845, 5870), `checks_contracts_prover.rs:1764`, `checks_contracts_exits.rs` (2011, 2998) and `domains_and_predicates.rs` (11009).
- `scoped_arithmetic_comparisons_unsatisfiable`, `scoped_arithmetic_value_bounds`, `scoped_arithmetic_values_relation`, `scoped_completed_call_guarantees`, `scoped_call_result_observations`: 30 occurrences across 7 files, counted with Grep count mode on the alternation of those names.

**Stated limits.**
- Budgets: `remaining: 4096` (36508), depth 64 (36579, 37077).
- Comment at 36502: none; the numbers are unexplained.

**Fail-open risks.**
- `scoped_completed_call_guarantees` (37506) does `.unwrap_or_default()`, so failure yields no guarantees. This is fail-closed.
- `scoped_arithmetic_implication_for_declaration` (37530) returns `Proven` if `engine.requires_unsatisfiable` (37554). That is vacuous truth: a contradictory hypothesis set proves anything. There is no comment at the site. Traced: the engine flag is set by `install_hypotheses`; a wrong program would get through only if the hypotheses were falsely judged unsatisfiable. Classification: open.
- The same pattern recurs at 37706 (`values_relation`: `engine.requires_unsatisfiable || match operator`) and 43707 (`strict_arithmetic_expression_implication`). Classification: open.
- `install_hypotheses` (37587) silently skips a negated hypothesis (`holds == false`) whose root is not a complementable binary (37594-37601), and a negated multi-comparison (37606). Skipping a hypothesis only weakens premises, so it is fail-closed.
- `predicates::matches_asserted` (37221) proves a goal if any asserted hypothesis normalizes to a structurally equal `Predicate` (37235-37239). It is used only after `engine.judge` returns `Unknown` (37566). Traced: this is syntactic match of a normalized comparison, an equal-premise lookup.

---

## 2. `self_induction` (37789-37878)

**Purpose.** For a recursive proof machine, installs the machine's own `ensures` as induction hypotheses on each self-application inside the value being proved, provided the application's `requires` are established.

**Shape.** `intake_available_self_induction_hypotheses` (37800).

**How it decides.**
- Gives up (returns) if any machine `requires` fact is a `Membership` or `Proposition` (37819-37823, 37832): `requires_out_of_language`.
- Otherwise, for each self-application (found by `StructuralJudge::self_applications`, 37837) it checks that every `requires` fact is established via `instantiated_fact_established` (37847).
- If established, `retain_induction_value` and the instantiated `ensures` predicates are installed.
- Keyed on the machine entry state symbol and parameter-name strings (`parameter_names` zipped with arguments, 37842).

**Cases by shape.** Only `ProofFact::Expression` requires and ensures are used (37816, 37825).

**Same job elsewhere.** Called from `finalize_structural_case_arm` (46927, 46958).

**Fail-open risks.** none seen (every failure path skips the hypothesis).

---

## 3. `shared_proof_walks` (37879-38188)

**Purpose.** Caches, per machine, the set of call-site requirements that were proven by one batched structural walk over a group of requirements with the same site and premises. This avoids re-walking each requirement individually.

**Shape.**
- `SiteCall`, `ProvenSite` (37895, 37901; fields are state, stored_state address, statement index, call, required expression).
- `SharedProofWalks { stored_machine: usize, proven: Vec<ProvenSite> }` (37910).
- `BuildingWalk = (SymbolHandle, ThreadId)` and `BuildingMark` (37924-37955), a per-thread re-entrancy marker held in `memos.building_shared_proof_walks`.
- `proven` (37957): the only entry.
- `build` (38054), `walk_group` (38118), `shares_walk` (38040), `required_expressions` (38161).

**Inputs and outputs.** Reads the machine, `RequirementWalk`s from `call_requirements`, and `CheckingMemos`. Writes the finished walk into `memos.shared_proof_walks` (37996-38003).

**How it decides.**
- `proven` returns false unless `shares_walk` is `Some`, memos exist, and the `machine` pointer is the stored machine: `std::ptr::eq` (37969-37974). The cache is therefore keyed on memory address (`address`, 38012: `std::ptr::from_ref(row) as usize`) and on `stored_machine == address(machine)` (38007).
- `shares_walk` rejects `runtime_values`, `case_facts_only` and inherited requirements (38041) and non-local sites (`reads.local_site()`, 38045).
- `build` only builds for proof machines (`is_proof_machine`, 38060). It groups requirements by `GroupKey = (SharedChoiceReads, bool, Vec<ExpressionHandle>)` (38010). It walks only groups with at least 2 members (38111).
- `walk_group` records a site as proven only if `recognize_structural_case_arms_batched` returns `Some` with no diagnostics (38149) and `has_positive_proof()` holds (38153).

**Cases by shape.** Only local-site, non-runtime, non-inherited requirements of proof machines in groups of two or more are shared. Every other requirement takes its ordinary proof path.

**Unused or one-caller code (grep).**
- `shared_proof_walks::proven`: 1 call site, `contract_entailment.rs:8383`.
- `SharedProofWalks` and `BuildingWalk` are also named in `frozen_program_and_queries.rs:1265, 1270` (memo field types).
- The comment at 37915-37923 calls the marker the way the "same machine" is handled across threads; it is internal to this module.

**Fail-open risks.** none seen. The default is false (not shared), so the requirement takes its ordinary proof.

---

## 4. `specification_calls` (38189-43107)

This is the largest block. It owns: (a) selecting which contract a call is held to, (b) building the type, const, runtime, machine and implementation bindings for a call application, (c) validating that every call or operator inside a specification expression has its `requires` established, and (d) helper predicates.

### 4a. `applied_bindings::operations` (38203-38832)

**Purpose.** Builds identities and bindings for requirement operations selected through evidence (conformance parameters) or closed requirement dispatch.

**Shape.**
- `operation_identity` / `operation_identity_in_regions` (38208 / 38217) build a text id: `format!("{}{identity}:{operation}", SELECTED_OPERATION_IDENTITY_PREFIX)` (38246).
- `selected_operation` (38252), `selected_application_operand` (38374), `selected_call_operation_identity` (38392).
- `AppliedProofBindings::retain_requirement_operations` (38483), `retain_requirement_operation` (38629), `retain_operation` (38669), `contract_machine_bindings` (38688), `predicate_machine_bindings` (38727).

**How it decides.**
- Every lookup that fails returns `Err(&'static str)` (for example "predicate implementation has ambiguous selected rows", 38320) or `None`.
- `retain_operation` refuses a second binding for the same source with a different value (`return (old == &binding).then_some(())`, 38682).
- `predicate_machine_bindings` returns early with the existing machines if `implementations` and `regions` are both empty (38735).
- It decides whether a region substitution changes a call's operation identity by comparing the id text `applied == original` (38798).
- Keyed on: symbols and the identity text built by `format!`.

**Cases by shape.** `selected_operation` handles two shapes only: a conformance (named, closed application with exactly one matching row) or a forwarded implementation namespace (38270-38356). A conformance whose application has a `member` is refused (38275-38277).

**Fail-open risks.** none seen (all `None` or `Err`).

### 4b. `applied_bindings` proper (38834-40428)

**Purpose.** `AppliedProofBindings` (38923) is the retained static application of a call or operator. Its fields are `types`, `constants`, `runtime_parameters`, `machines`, `implementations`, `implementation_owner` and `regions`. It projects declared parameter types through the application so a contract's `requires` can be instantiated at the call.

**Shape.**
- `canonical_result_reference` (38834) / `result_reference_for_identity` (38858): finds a type reference with a given normalized identity by a linear scan over all `type_reference_table.references()`. It is memoized in `memos.effective_result_references` keyed by the identity string (38865-38891). If no identity or no match exists, it returns the input reference (38853-38855).
- `PredicateMachineBinding` (38895); `MachineBindings` (38920).
- `for_operator` (39014), `from_selected` (39069), `for_call` (39126), `for_call_site` (39143), `retain_call_regions` (39201), `for_retained_call_site` (39233), `source_value_application` (39390), `transport_parameters` (39526), `for_application` (39565), `runtime_result_presentation` (39917), `runtime_result_type` (40121), `project_type` (40223), `retains_parameter` (40235), `constant_integer` (40266).
- Free helpers: `exact_parameter_bindings` (40282), `enclosing_type_reference` (40332), `signature_declared_by_machine` (40371), `implementation_operand` (40392), `integer` (40420).

**How it decides.**
- `for_application` requires an exact one-to-one correspondence of retained bindings to declared parameters plus conformance bounds (39671-39684) and returns `None` otherwise.
- A binding value must match its parameter's kind: `Type` with `Type`, `Const` or `Value` with `Const`, `Value` with `RuntimeValue`, `Machine` with `Machine`, `Implementation` with `Conformance` (39744-39892). Anything else is `_ => return None` (39892).
- `exact_parameter_bindings` (40282) resolves a machine's type parameters only for `TypeParameterKind::Type` (40295) and with no conformance bounds (40290). When the argument has only a single path segment it resolves by name, `parameter.name.as_str() == name` (40340, 40358), via `enclosing_type_reference`. This is source-spelling keyed.
- `runtime_result_presentation` for a `DynamicSlot` performs about 15 identity and parent checks before accepting (39951-39996). It builds `format!("\0public-result-formal:{position}")` ids (39997).
- `source_value_application` (39390) accepts only a checked-body, non-requirement-dispatch, concrete machine with exactly one specialization, no machine/conformance/lifetime parameters, and with equal static-binding identity tuples between call and specialization (39400-39511).

**Cases by shape (quotes).**
- `from_selected` handles `Type` and `Const` parameters only; `Value`, `Machine`, `Implementation`, `Proposition` kinds return `None` (39117-39120).
- `for_application` operator branch: operator with lifetime parameters or non-Type/Const parameters returns `None` (39660-39670).
- `exact_parameter_bindings` is for `Type`-only parameter lists.
- `retains_parameter` returns false for `Proposition` (40262).

**Same job elsewhere.**
- `applied_bindings::for_application` and `specification_calls::call_parameter_scope` (42184) both substitute retained static bindings into parameter types. The second does it by `type_identity_with_simultaneous_substitutions` followed by a scan for the reference with that normalized identity (42278-42296); the first uses `project_type`.
- `structural_judgment::domain_premises::data_application_type_bindings` (51205) and the free `data_application_static_bindings` (51678) re-derive generic-data parameter-to-argument lists. Suspected duplicate of the same extraction at `domain_premises::selected_case_indices` (50554), which I read: it starts from the same `Generic`/`Named` reference to `generic_instance` step (50569-50577).

**Unused or one-caller code (grep).**
- `AppliedProofBindings::for_call(`: 1 call (41526). Also reachable as `for_call_site`.
- `exact_type_selection`: 3 uses (40299, 42726, and `contract_entailment.rs:82188` outside the range). It is `pub(crate)`.
- `exact_parameter_bindings`, `enclosing_type_reference`, `signature_declared_by_machine`: private; called from `for_retained_call_site` only (39262-39270 region), not grepped separately.

**Stated limits.**
- 42579-42581, `identity_carrier_is_closed`: "Value substitution does not project a proposition's stored type identity. Keep that route closed until its exact source carrier needs no type or region substitution".
- 38567: comment "Authenticate the called member before mapping its public siblings."

**Fail-open risks.**
- `canonical_result_reference` (38847-38855) returns the unnormalized reference if identity computation or the scan fails. Classification: traced; the caller treats the return only as a presentation reference, and the carriers are re-compared elsewhere (`applied_declared_carrier_identity`, 40191-40196). I did not trace every caller.
- `retain_call_regions` returns `Ok(())` with no regions when there is no authored application (40211-40213). That is a no-op, not an accept.
- `runtime_result_type` returns `declared` unchanged when the identity with static scope equals the identity without (40209-40219). That is a closed-identity shortcut.

### 4c. `collection_operations::validate_builtin_index` (40429-40491)

Emits an error unless `proves_observed_bounds` returns `Some(true)` (40443, 40481). Fail-closed.

### 4d. Specification validation (40515-41879)

**Purpose.** Walks a specification expression (a `requires`, `ensures`, `where` or proof body) and checks that every call, operator and index inside it has its own `requires` contract established by earlier facts. It also checks match coverage, pending-subject misuse, payload projection cases and address-cast fit.

**Shape.**
- `SpecificationCaller { Machine(&Machine, Option<&State>), Declaration(SymbolHandle, &[StateParameter]) }` (40516) with helpers `parameters`, `expression_type`, `selection_requirements`, `term`.
- `SpecificationPremises { asserted, denied, captures }` (40879) with `arithmetic_hypotheses` and `structural_judge` (40907).
- Entry points: `validate_declaration_expression_formation` (40647), `validate_declaration_call_requirements` (40764), `validate_specification_call_requirements` (40801), `validate_proof_body_formation` (40829).
- Core walker: `validate_specification_expression_in_scope` (41256-41879).
- Helpers: `validate_declaration_value_arguments` (40694), `declaration_anonymous_fit` (40979), `pending_data_subject` (41003), `validate_pending_subject_materialization` (41033), `validate_specification_payload_projection` (41067), `sole_conjunctive_case` (41129), `conjunctive_case` (41147), `record_membership_case` (41166), `membership_variant` (41184), `case_predicate_owner` (41218).

**Inputs and outputs.** Reads expression tables and contracts; pushes `Diagnostic`s and `IntegerRangeObligation`s (`formation_obligations`).

**How it decides.**
- `And` / `Or` (41282): the left operand is validated; if it produced no diagnostics and no obligations, it is added to `asserted` (for `And`) or `denied` (for `Or`) before the right operand is validated (41299-41310).
- `Match` (41333): validates coverage, subject, then each arm. A destructure arm condition is assumed in its own arm; a value pattern is denied for later arms (41372-41423).
- For a call (41491) the `requires` are proved by: (1) a structural proof through `instantiated_selected_fact_judgment` (41793-41847) or (2) `arithmetic_requirement` (41849). Each is attempted only if the requirement is a `ProofFact::Expression` and `arguments_select_plainly` (41790). If neither succeeds, the diagnostic "cannot prove requires contract" is pushed (41873).
- For a `Binary`/`Indexed` the operator is resolved by `scoped_operator_application` (41570). `BuiltinFallback` goes to the builtin comparison landing check or `validate_builtin_index` and returns (41573-41605). No selected meaning gives the "no exact selected meaning" error (41641).
- `case_predicate_owner` (41218) compares the owner string for equality: `owner == format!("data `{data_name}` default-domain predicate")` (41219-41222). It decides which `owner` label the right side of an `And` gets. It is keyed on a display string of the diagnostic owner. It has one caller (41315).
- Name matching in `names_are_unambiguous` (43065) uses `display_name` spelling and head names (43083-43101); `structural_static_binders_are_exact` (42909) compares `formal == name.as_str()` (42934).

**Cases by shape.**
- 41529-41533: a `Binary` with `static_type_predicate_operands` returns without checks.
- 41537-41540: a `Binary` whose operator has no `binary_operator_spelling` returns without checks.
- 41659: `_ => return` for every other expression node kind.
- 41492-41497: a mathematical call returns early; comment: "Mathematical definitions own checked dependent applications, not machine contract telescopes."
- 41519-41524: if the argument slice for the owner's leading operands is out of range, return.
- 41720: no requirements returns.
- 41770-41784 `has_named_evidence_lane`: a `Proposition` requirement tied to named evidence is skipped (`continue`) when the owner is `Concrete`, `named_evidence_count != 0` and the call has no dispatch, quotient op or machine arguments, and `evidence_arguments.len() == named_evidence_count`. No comment at the site. Classification: open (skipped without proof here; presumably proved at the evidence argument).
- `validate_specification_payload_projection` skips data with a single variant (41105).

**Same job elsewhere.**
- `names_are_unambiguous` / `structural_static_binders_are_exact` are name-spelling checks in the same style as `exact_parameter_bindings`.
- `SpecificationPremises::arithmetic_hypotheses` (40886) and `scoped_arithmetic::...::hypotheses` (36555) both turn asserted/denied expressions into `ScopedArithmeticHypothesis` lists.

**Unused or one-caller code (grep).**
- `invocation_is_fact_only`: 1 outside call (`05_typed-trees-to-checked-trees/src/flow_mutation.rs:3879`), plus the `pub use` at `lib.rs:1459`.
- `call_contract_parameter_position`: 2 outside calls (`checks_contracts.rs:1908, 1936`), plus the `pub use` at `lib.rs:1969`.
- `case_predicate_owner`: 1 call (41315).
- `validate_declaration_expression_formation` and the other entry points: not grepped.

**Fail-open risks.**
- `validate_declaration_value_arguments` (40706-40708) returns silently if `owner_parameter_scope` is `None`. It is `None` only for a `DynamicSlot` whose declared and instantiated parameter counts differ (42169). A wrong program would skip the argument type check at that site. Classification: open.
- 41537-41540 (operator with no spelling) and 41659 (other node kinds) return with no diagnostic. Classification: open.
- 41529-41533 (type-predicate binaries skipped) and 41492-41497 (mathematical calls skipped): the second is stated in a comment; the first has none, open.
- 41770-41784 named-evidence `Proposition` skip: open.
- 41730-41732: `owner_parameter_scope` `None` returns without proving `requires`. The same `DynamicSlot` mismatch input reaches it. Classification: open.

### 4e. Selection and parameter helpers (41881-42562)

**Shape.**
- `SelectedCallContract { Concrete(&Machine,&State), Signature(&StateSignature), DynamicSlot{slot,signature}, Operator(&OperatorDefinition) }` (41882).
- `direct_named_member_contract` (41898), `selected_call_contract` (41958), `selected_call_site_contract` (41973), `selected_contract_by_symbol` (42012), `selected_call_contract_for_parameters` (42050), `signature_by_symbol` (42080).
- `owner_symbol` (42093), `owner_is_proof` (42102), `invocation_is_fact_only` (42118), `owner_name` (42130), `owner_parameters` (42146), `owner_parameter_scope` (42160), `call_parameter_scope` (42184), `call_contract_parameter_position` (42301), `owner_leading_operands` (42318), `owner_return_type` (42327), `owner_contracts` (42338), `applicable_requirements_of` (42364).
- `RuntimeScalarSubject`, `call_contract_scalar_substitution` (42386), `call_scalar_substitution` (42415), `call_machine_substitution` (42480), `selected_machine_binding` (42514), `selected_machine_result_type` (42527), `exact_type_selection` (42541), `plain_machine_selection` (42563).

**How it decides.** `selected_contract_by_symbol` (42012) tries, in order: dynamic slot, requirement signature, entry-state machine's signature, operator declaration, then the concrete machine (42019-42047). `selected_call_site_contract` (41973) tries direct named member, requirement contract signature, then bails if a static requirement dispatch is present (41988), then falls to the symbol lookup or operator resolution.

**Cases by shape.** `owner_is_proof` is true only for `Concrete` and `Signature`; `_ => false` covers `DynamicSlot` and `Operator` (42114). `exact_type_selection` accepts a selection only when the symbol kind is `BuiltinType`, `TypeParameter` or `Data` (42555-42560).

**Unused or one-caller code (grep).** `selected_call_contract(`: 24 occurrences (23 in this file, 1 in `machine_calls_core.rs`), counted with Grep count mode.

**Fail-open risks.** none seen (all `None` on failure).

### 4f. Substitution completeness and requirement checks (42579-43106)

**Shape.**
- `identity_carrier_is_closed` (42582), `structurally_substitutable_fact` (42611), `term_has_complete_substitution` (42651), `constructor_fields_are_complete` (42777), `fact_roots` (42835), `requirements_are_acyclic` (42849), `requirement_parameters_are_bound` (42879), `structural_static_binders_are_exact` (42909), `selected_machine` (42939), `arithmetic_caller_bindings` (42952), `arithmetic_requirement` (42984), `names_are_unambiguous` (43065).

**How it decides.**
- `term_has_complete_substitution` (42651) accepts a fixed list of term shapes. `CapturedValue`, `BoundProjection`, `MathematicalBinary`, `IntegerEmbedding` and `Opaque` are not complete (42686-42690). `ObservedValue` is `unreachable!` because `semantic_value` strips it (42657). `Application` requires a selected signature or concrete machine with matching parameter and selection counts.
- `structurally_substitutable_fact` (42611) accepts: a static-type-predicate fact, a call term of `Identity`-with-closed-carrier or `Bool`-result `Application` form, a case observation, `And`/`Or` of such, or a comparison (`==, !=, <, <=, >, >=`) of complete terms (42638-42648).
- `arithmetic_requirement` builds a goal with bindings for dependency symbols only (43008-43048) and requires `Proven`.
- Atom identities: `format!("\0specification:{:?}", parameter.symbol)` (42976).

**Fail-open risks.** none seen. Every unknown returns false.

---

## 5. `static_applications` (43108-43630)

**Purpose.** Judges whether a machine's static (compile-time) parameter requirements hold at a static application: a generic machine selected with constant arguments.

**Shape.**
- `static_application_requirement_judgment` (43126).
- `generic_tuple_requirements` (43227): `requires` facts that mention none of the parameter symbols or non-const type-parameter symbols.
- `immutable_caller_requires` (43276), `immutable_fact` (43365), `constant_value` (43418), `StaticFacts` (43441) with `judge_alternatives` (43451), `judge` (43513), `boolean_value` (43585), `conjunction` (43619).

**How it decides.**
- `static_application_requirement_judgment` returns `Unknown` if the signature or owner is invalid or the argument count differs from the type-parameter count (43137-43141), or the caller state is not in the owner machine (43143-43153).
- `constant_value` (43418) decodes a canonical const atom, or else `spelling.parse::<i128>()` (43436). It is keyed on the source spelling of a const argument.
- Booleans are judged by `boolean_value`; `==`/`!=` of booleans requires `has_identity_equality_meaning`, else `Unknown` (43557-43564).
- `judge_alternatives` splits `And`/`Or` hypotheses (fuel 4096 at 43219, depth 128). `Or` yields `Proven` only if both branches prove (43496), `Refuted` only if both refute.
- `immutable_fact` returns false for `Call`, `Atomic` or `Borrow` and caps at 4096 nodes (43379, 43406).

**Cases by shape.** In the argument loop only `Const` and `Value` parameters are bound (43168-43173). Other parameter kinds are skipped (`continue`), so their requirements are not judged here.

**Unused or one-caller code (grep).**
- `static_application_requirement_judgment`: 3 uses outside the definition (`machine_parameters_and_bindings.rs:6119, 7550`, and its own file). `generic_tuple_requirements`: `machine_parameters_and_bindings.rs:5913, 5930, 6010`, plus 43218 here.

**Fail-open risks.**
- If an unbound parameter (`None` from `constant_value` and no matching supplied binding, 43184-43206) is not bound, the requirement is still judged with that symbol unbound. The judge then returns `Unknown`, not `Proven`. Traced: `Proven` requires the arithmetic engine to prove with no value for that parameter, which only succeeds if the requirement holds for all values. Classification: traced.
- `judgment` starts at `Proven` and is conjoined (43217-43223). With zero `generic_tuple_requirements` the result is `Proven`. Reading: a signature with no such requirements has nothing to fail. Classification: stated by structure, no comment.

---

## 6. `strict_arithmetic` (43634-43719)

**Purpose.** A public wrapper around `Engine` that judges `hypotheses implies goal` over expression handles with explicit symbol and argument bindings.

**Shape.** `StrictArithmeticSymbolBinding` (43640), `StrictArithmeticBindingValue` (43646), `StrictArithmeticExpressionBinding` (43674), `StrictArithmeticImplicationJudgment { Proven, Refuted, Unknown }` (43680), `strict_arithmetic_expression_implication` (43686).

**How it decides.** `unsigned_integer_atom` (43652) marks an atom unsigned only for `U8|U16|U32|U64` (43661-43667), not `Usize` or `Addr`. `Judgment::ConstantFalse | Refuted` maps to `Refuted` (43713). `Proven` if `requires_unsatisfiable` (43707-43709).

**Unused or one-caller code (grep).**
- `strict_arithmetic_expression_implication`: `quotients.rs` (1124, 1233), `checks_contracts.rs:4519`, `lib.rs:1976`.
- `unsigned_integer_atom`: 2 callers, both in this file (33253, 33925).

**Fail-open risks.** `requires_unsatisfiable => Proven` (43707): open, same as section 1.

---

## 7. `structural_case_arms` (43720-47150)

**Purpose.** Walks the statements of a proof (theorem) machine, state by state, and produces its "leaf arms". An arm is one terminal path with the facts it carries: case hypotheses, equations, citations (lemma uses) and the completed value. The same traversal also serves as a prefix observer: it lets `StructuralPrefixVisitor`s watch the judge's state at a call site on the way to a leaf.

### 7a. `arrivals` (43722-43835)

- `ArrivalBucket` (43730) and `StructuralArrival` (43737). `bucket()` (43753) hashes the environment as `name.hash` plus `format!("{:?}", term.semantic_value()).hash` (43757). It is used only as a hash-bucket key; membership uses `same_inputs` (43767) with payload equality.
- `StructuralArrivalLog { completed: HashMap<ArrivalBucket, Vec<StructuralArrival>> }` (43809): `contains_completed` (43818) and `complete` (43826). The key is not a dense id. It is a hash of the debug string.
- Purpose: when a state lies on a prefix cycle (`state_lies_on_prefix_cycle`, 46673), a second arrival with the same inputs from a different parent returns an empty arm list (45467-45472).

### 7b. `call_entries` (43836-44234)

- `exclude_target_calls` (43847), `CallEntryScope` (43892), `observe_calls` (43898), `expression_entries` (44107), `body_can_be_observed` (44184).
- `body_can_be_observed` returns false for a callee with no checked body or a `target_family_interface` machine (44190-44195). It returns true for a proof machine or when runtime values must be preserved (44196). Otherwise it asks `checked_value_call_candidate` using effect inference plans (44202-44232).
- `observe_calls`: a missing frame for a requested expression means that site is excluded, `excluded_body` (44022-44032). If any requested callee's body cannot be observed, the whole traversal returns `None` (43976-43983).
- `expression_entries` returns `None` for a `Transition` statement scope (44129-44134). It returns `None` if the prefix has `roots_unavailable` (44162).

### 7c. Arm recognition (44258-44555)

- `StructuralCaseArm` (44258): `normal_judge`, `parameter_names`, `case_hypotheses`, `case_equations`, `citations`, `normal_guarantees`, `value`, `value_source`.
- `recognize_guarded_structural_value_arms` (44276): a fixed program shape (below). 2 call sites: the `use` at 90300 and the call at 90474 (both in this file, outside the range).
- `recognize_structural_case_arms` (44428), `recognize_structural_case_arms_batched` (44450), `recognize_structural_state_prefix` (44496) and `_batched` (44516).
- `guard_expressions_equal` (44369): equality for `Boolean`, `Integer` (`value_i64`), `Name` (symbol and member lists) and `Binary`; every other node pair is false.
- `stage_citation_argument_terms` (44396).

**Cases by shape (quote).** `recognize_guarded_structural_value_arms` accepts only this shape (44281-44331): exactly one state; exactly two non-marker statements, both `Transition`s; first is `when (x == bool)` with no continuation and a valid target; the second is either `Always` (no continuation, valid target) or `when (x == !bool)` with the same condition expression. Anything else returns `None`.

**Note (read, line 44316).** In the `Always` arm the check is `!guard_expressions_equal(program, first_condition, first_condition)`, comparing `first_condition` with itself. It carries no information about the second statement; the `When` arm (44325) compares first with second. Grep of `guard_expressions_equal` shows 4 sites: 44316, 44325, 44389, 44390.

### 7d. Statement-local liveness (44557-44694)

- `expression_mentions_symbol` (44557), `local_symbol_is_dead_in_remaining_prefix` (44579), `local_symbol_is_never_branched_on_in_remaining_prefix` (44611).
- At 44685-44687 a local is forwarded only if its name `starts_with("__")` and it has an ordinary initializer that mentions the tracked local. This is a decision keyed on name spelling.

### 7e. `recognize_structural_state_prefixes` (44696-45363)

**Purpose.** For one state, runs the judge forward through the straight-line prefix and, at each visitor's site, hands it the judge's state. It returns `Vec<bool>` saying which visitors reached their site.

**How it decides.**
- Statement kinds handled: `LocalData`, `Assignment` (only with runtime body values), `Expression`, `Call` (value-preserving, citation, or runtime-completion), `Assembly`, `AssemblyFact`, and `Transition` collected for later (44895-45199).
- `_ => return valid` (45199) stops the walk for any other statement kind. Visitors not yet reached stay false.
- A path whose hypotheses are contradictory is dropped and `visitor.entered()` is called (44843-44850); so a contradictory (vacuous) path counts as entered.
- A `LocalData` with several outcomes may be rejoined into one pure outcome when the local is dead later (45040-45056) or a never-branched-on `Bool` (45057-45085).
- Transition handling (45233-45361): needs a `When` guard; `guard_observation` must succeed, else `return valid` (45301-45305). A guard judged `Proven` for the subject removes the path from `remaining_paths` (45337).

### 7f. `recognize_structural_state_leaves` (45366-46557)

**Purpose.** The main recursion. For a state, it walks statements, splits paths on multi-outcome statements (`StateLeafStep::Continue`), turns each transition into branches (guard taken or skipped), recurses into the target state or call body, and finalizes leaf arms.

**How it decides.**
- `path.contains(&state.symbol)` at `start_index == 0` returns `None` (45443): a recursive re-entry on the path is refused.
- Statement kinds: `LocalData`, `Assignment` (runtime values only), `Expression` (last statement becomes a synthetic `Always` transition, 45799-45811), `Call`, `Transition`, `Assembly`/`AssemblyFact`; `_ => return None` (45901).
- Transition with `continuation` valid or `exit == Ordinary` without a target returns `None` (45880-45885).
- Crash exits: `if matches!(transition.exit, TransitionExit::Crash(_)) { continue; }` (46198). A crash transition produces no leaf arm and no body entry.
- Guard judgments from `completed_guard_judgment`: `Refuted` excludes target calls and skips the branch (46084-46094). `Proven` removes the "guard not taken" continuation (46065).
- Body entry is through `terminal_body_transfer` (46782) and requires `body_can_be_observed`.

**Cases by shape.**
- `terminal_body_transfer` accepts a `Named` target or a `Value` that is a `Call` selected to a `Concrete` callee (46797-46867). Any other target returns `None`.
- Comment at 46364-46367: selected value targets can contain calls below a `Match`; they are observed instead of recursing.

**Fail-open risks.**
- 46198 (crash arms dropped when no visitors, i.e. in leaf collection). A wrong program: a crashing branch is not required to carry the theorem's conclusion. No comment at the site. Classification: open. I did not trace the callers of `recognize_structural_case_arms` to see whether `ensures` is meant to hold only on normal completion.
- 46656 and 46666 in `prefix_site_is_reachable_with_entry` return `true` ("reachable") when a state lookup or its destinations are unknown. Reachable only causes more observation, not acceptance. Classification: traced, conservative.
- `state_lies_on_prefix_cycle` (46673) returns `true` when destinations are unknown (46686, 46701, 46706). A cycle assumption turns arrival de-duplication on; it only prunes a repeat arrival with identical inputs from a different parent (45467). Classification: traced.
- 45467-45472: a repeated arrival with identical inputs returns `Some(Vec::new())` (no arms), so its leaves are not re-produced. The first arrival's leaves are kept by the first traversal. Classification: traced (stated in structure, no comment).

### 7g. Shared-read keys and reachability (46559-46772)

- `SharedChoiceReads { state, local_site, stops_at }` (46560), built by `of` (46567). `stops_at` lists machine symbols for which `visitor.stops_at_machine` is true.
- `site_is_local`, `all_sites_local`, `prefix_site_is_reachable` (46622), `prefix_site_is_reachable_from_entry` (46630), `state_lies_on_prefix_cycle` (46673), `state_prefix_reachability_destinations` (46720).
- Reachability destinations are memoized in `memos.state_prefix_reachability_edges` and `memos.state_prefix_cycle_membership`, both keyed by state symbol (46725-46729, 46678-46682).
- `state_prefix_reachability_destinations` skips `Operator` and `BuiltinFunction` targets (46740-46746) and returns `None`, caching `None`, when a callee cannot be resolved to a machine head (46748-46760).
- `SharedChoiceReads::of` has one outside user, `shared_proof_walks::shares_walk` (38044).

### 7h. Finalization (46774-47149)

- `TerminalArm` (46774), `terminal_body_transfer` (46782), `is_subordinate_machine` (46875), `finalize_structural_case_arm` (46909), `structural_arm_judge` (46981), `guard_observation` (47020), `completed_guard_judgment` (47058), `guard_case_value` (47093).
- `finalize_structural_case_arm`: `vacuous = arm_judge.hypotheses_contradictory` (46925); if not vacuous, pending citations are instantiated and their predicates taken in (46936-46967).
- `structural_arm_judge`: marks the arm contradictory if any premise `requires` fact is `Refuted` (47006-47016).
- `guard_observation` (47020): accepts `x == true|false` with builtin binary meaning, a case-membership guard, or any `Bool`-typed expression; otherwise `None` (47049-47055).
- `guard_case_value` builds induction variables `format!("\0__ih_{}_{}_{}", state.name.as_str(), branch_id, field)` (47138). Debug names embed the state's name string and a counter.
- `is_subordinate_machine` (46875) walks `lifetime_declaration_owner` and `declaration_scope_parent` ancestry with no numeric cap; it loops until a handle repeats or becomes invalid (46884, 46897).

**Same job elsewhere.**
- `recognize_structural_state_prefixes` (44705) and `recognize_structural_state_leaves_from` (45421) duplicate the statement-kind dispatch for `LocalData` pure-outcome rejoin (44-line blocks at 45040-45085 and 45723-45773), `Assignment`, `Expression`, `Call` and citation handling. Both read; they are near-copies (the first uses `any_visitor_at_transition`, the second `all_sites_local`).
- The "local is never branched on, rejoin Bool as `CapturedValue`" block appears at 45057-45085 and 45744-45773 with the same `rejoined.capture_sequence += 1` and `CapturedValue { statement: invalid, ... }` construction.

---

## 8. `structural_judgment` sub-modules (47151-54636)

The `StructuralJudge` and `StructuralTerm` types are defined outside this range (in this same file). These sub-modules extend `StructuralJudge` with `impl` blocks.

### 8a. `algebraic_identity` (47168-47489)

**Purpose.** Decides equality of two terms up to the associativity and commutativity of licensed operations, with bounded use of hypotheses as exchange rules.

**Shape.** `AlgebraAtom` (47183). `StructuralJudge::ring_rearranged_equal` (47201), `licensed_algebra_equal` (47218), `owned_application` (47393), `additive_addends` (47421), `polynomial_normal_form` (47446, `MONOMIAL_CAP = 64`, 47451), `sorted_multiset_subtract` (47478).

**How it decides.**
- For each `ring_license` whose add machine appears, both sides flatten into addend multisets (needs at least 2 left addends, 47231).
- Multisets are sorted by `format!("{term:?}")` (47234-47235) and compared with `==`. The comment at 47180-47181 says: "Debug provides deterministic grouping only; equality always checks the complete payload" (`AlgebraAtom::eq` uses `same_resolution_payload`, 47186-47189).
- Hypothesis exchange: depth 2, frontier capped at 64 (47262-47285). The comment says over-refusal past the cap and never unsound (47250-47251).
- Semiring tier: polynomial normal form with scaled hypothesis exchange (47292-47386).

**Cases by shape.** `owned_application` accepts only a binary `Application` with no selections and exactly 2 arguments (47417), after peeling observation shells and refusing reference shells (47403).

**Fail-open risks.** none seen (all misses give `false`).

### 8b. `cases` (47490-48267)

**Purpose.** Case-membership facts about data values ("subject is variant V"): intake, judgment, and equations between case constructors.

**Shape.**
- `CaseEligibilityState`, `CaseEligibilityRow`, `CaseGuaranteeEligibility` (47504-47596): a dense `Vec` indexed by `expression.arena_index()` (47530, 47550), built once per program in `frozen_program_memos().case_guarantee_eligibility` (47626-47631).
- `CaseFact` (47599); `applied_case_classifier` (47606).
- `StructuralJudge` methods: `may_extract_contract_cases` (47634), `intake_case_condition` (47639), `is_case_membership` (47700), `instantiated_case_judgment` (47729), `instantiated_selected_case_judgment` (47739), `instantiated_case_guarantee` (47753), `instantiated_contract_case_guarantees` (47789), `extend_contract_case_guarantees` (47806), `intake_case_guarantee` (47935), `intake_observed_case` (47944), `observed_case_value` (47962), `judge_case` (47979), `intake_case_equation` (48050), `retain_case_equation_guarantees` (48102), `resolve_case_subject` (48179).

**How it decides.**
- `judge_case` (47979) is `Unknown` unless the resolved subject is a `Constructor` (exact `matches_case`) or one of seven variable-like shapes (47998-48009). Then it scans `case_facts`: a holding fact gives `Proven` for the same case and `Refuted` for a different one (48016-48022). Excluded cases give `Refuted` for the excluded one. If exactly one variant remains and it is the asked case, `Proven` (48031-48045).
- `extend_contract_case_guarantees` handles `Or` by extracting a case guarantee from an alternative only if the sibling is `Refuted` (47835-47868).
- `resolve_case_subject` returns `None` for `Identity`, `LogicalNot`, `ContentValue`, `Opaque`, `ScalarBinary`, `MathematicalBinary`, `IntegerEmbedding` and `CollectionMeasure` (48257-48264). Depth cap 32.

**Fail-open risks.**
- `may_extract_contract_cases` (47634) returns true when no memos exist (`is_none_or`), meaning "attempt extraction". It does not accept anything on its own.

### 8c. `conditional_guarantees` (48270-48653)

**Purpose.** An `ensures` can say "if condition C then conclusion D". These are held back until C is established, then D is assumed.

**Shape.** `ObservedCondition { Case, Predicate, Equal, Connective }` (48278); `ConditionalGuarantees` (48403) with `boolean_comparison` (48410) and `same_guarantee`; `retain_conditional_guarantees` (48454) and `activate_conditional_guarantees` (48465).

**How it decides.** `activate_conditional_guarantees` does nothing when `case_facts_only` (48466). A guarantee fires when every condition is `(Proven, true)` or `(Refuted, false)` (48473-48479). It loops to a fixed point (48469).

**Tests in the range.** 4 `#[test]` functions in `mod tests` (48507-48652): `unknown_condition_neither_establishes_itself_nor_escapes_a_branch`, `boolean_comparison_preserves_both_directions_without_self_support`, `consequence_cannot_supply_its_own_guard`, `independently_established_guards_activate_a_chain_once`.

### 8d. `constructor_identity` (48654-48915)

**Purpose.** `ConstructorIdentity { Boolean(bool), Nominal{carrier, reference, declaration, case, member} }` (48661), the identity of a data constructor.

**How it decides.**
- Custom `PartialEq` (48852) compares only `carrier` and `member` for nominal, and the bool for `Boolean`. `declaration`, `case` and `reference` are ignored by `==` and by `Hash` (48874-48884).
- `same_resolution_payload` (48761) is stricter: it also requires equal `declaration`, `case` and `reference`. `same_constructor` (48790) requires equal `declaration` and `case`, ignoring the reference.
- `matches_case` (48812): comment "Rejoin that exact case through the receiving application; spelling never selects its owner" (48810-48811).

### 8e. `data_identity` (48917-49883)

**Purpose.** Decides data-value identity (`Id`) between two terms, using established identity edges, record eta, algebraic rewrite and bounded search.

**Shape.** `IDENTITY_QUERY_WORK_LIMIT = 8192` (48929); `IdentityCorrespondence { Representation, Established }` (48932); `declaration_data_identity_terms` (48938), `intake_proposition` (48968), `retain_formed_proposition` (48977), `retain_data_identities_from` (49004), `judge_explicit_identity` (49023), `judge_identity_with_budget` (49033), `identity_edges` (49077), `typed_identity_path` (49095), `exact_identity_congruence` (49224), `identity_representation_correspondence` (49242), `identity_correspondence` (49276), `identity_application_arity` (49498), `identity_algebra_equal` (49521), `exact_record_eta` (49538), `judge_data_identity` (49652), `data_identity_forms` (49684), `collection_measure_identity` (49794), `data_identity_is_observed` (49842).

**How it decides.**
- Integers equal: `Proven` / `Refuted` (49353-49359).
- Floats: both must have the same landing format; compared by bits. Comment at 49021-49022: "Id uses value representation congruence. Executable numeric equality, including IEEE signed-zero equality, supplies no rule in this reader."
- Constructor pairs compare fields pairwise (49383-49420). Applications compare target, machine, selection payload, result types and arity (49421-49451).
- Anything else under `Established` tries `exact_record_eta` or `identity_algebra_equal` (49486-49489); under `Representation`, `Unknown` (49483).
- `exact_record_eta` rebuilds a record from its own projected fields. It builds a `HashMap<&str, _>` keyed on field-name spelling (49607-49610). The map is local to the call.
- Search caps: depth 32/64; `typed_identity_path` visits at most 64 values (49152); edge roster is `take(*remaining)` (49148).

**Fail-open risks.** none seen (all miss paths are `Unknown`/`false`).

### 8f. `domain_premises` (49884-51792)

**Purpose.** Brings declared data-type and domain predicates (`where` facts, domain membership, index equations) into the judge as established premises, for a value that is "admitted".

**Shape.** `conditional_case_type_equations` (49897), `admitted_case_judgment` (49948), `intake_completed_match_case` (49979), `intake_selected_case_predicates` (50074), `has_admitted_nominal_subject` (50301-50552), `selected_case_indices` (50554), `intake_entry_domain_predicates` (50673), `intake_admitted_value_predicates` (50767), `intake_admitted_value_indices` (50811), `intake_completed_scalar_bounds` (50866), `intake_observed_integer_bound` (50903), `intake_atomic_observation_bounds` (50930), `intake_admitted_data_predicates` (50981), `nominal_type_has_data_predicates` (51105), `has_projected_field_equations` (51153), `data_application_type_bindings` (51205), `data_application_static_bindings` (51252, and free fn 51678), `intake_domain_predicates` (51259), `intake_instantiated_predicate` (51373), `intake_judged_instantiated_predicate` (51391), `intake_entry_collection_predicate` (51527), `entry_predicate_integer_operand` (51581).

**How it decides.**
- `has_admitted_nominal_subject` (50301) returns false unless `runtime_body_values` and a valid `declaration_state`, and the subject is not an unobserved descriptor (50307-50315). It accepts when the subject's `observation_identity_key` matches one of: a completed match admission, a completed nominal value, a body type parameter, an entry parameter, a projection of a readable-reference parameter, or a normal result (50316-50551). Each also requires `same_application` (equal normalized type identity). It is keyed on observation identity keys and normalized type identities.
- Synthetic parameter names are built from handles: `"\0case_index_{arena}_{gen}"` (50137), `"\0case_field_..."` (50167), `"\0data_subject_..."` (50263, 51007), `"\0value_..."` (51046).
- `has_projected_field_equations` (51153): for an `Opaque(text)` term it tests `text.starts_with(name)` and `text.as_bytes().get(name.len()) == Some(&b'.')` (51173-51179). This is a string-prefix test on the opaque term's text.
- `intake_domain_predicates` (51259) needs the domain's index arguments to be the domain's own `Named` parameters (51278-51282), and every `Const` index to have an integer literal argument, else returns (51312-51317). Kind `Type` is skipped; other kinds return.
- `intake_judged_instantiated_predicate` (51391): case observation, then `And` (both sides), then `Or` (the surviving side only if the other is `Refuted`, 51442-51479), then identity equality, then a general bound term equated to `true`.

**Cases by shape.**
- `selected_case_indices` (50554): accepts only `Generic` or `Named` carriers (50569-50577); index values only from integer literals, entry parameters of the same fixed-integer primitive, or machine value/const parameters of the same primitive (50598-50666); a state local that shadows the parameter name makes it `None` (50623, `local.name == parameter.name`).
- `data_application_static_bindings` (51678): only `Const`/`Value` parameters of fixed-integer type (51729-51734).

**Same job elsewhere.** `data_application_type_bindings` (51205-51250) and `data_application_static_bindings` (51678-51791) both open with the same ~25-line "peel reference, find `Generic` application, `unique_data_definition_by_symbol(base_symbol)`, zip parameters with arguments" block. A third start is in `selected_case_indices` (50569-50588).

**Fail-open risks.**
- All early `return`s in the intake functions lose a fact, not gain one. `intake_judged_instantiated_predicate` (51410-51417) sets `hypotheses_contradictory = true` when a predicate's case guarantee is already `Refuted`. That makes the arm vacuous, which is how an impossible path is discarded. Traced: this is only reached from instantiating a declared predicate whose own case claim contradicts known case facts.

### 8g. `equations` (51793-52154)

**Purpose.** Installs equations as substitutions, rewrites or contradictions.

**How it decides.** The doc comment (51828-51831) says: constructor pairs decompose, distinct cases of one data make the hypotheses contradictory (disjointness), and a variable side becomes a directed substitution, first binding wins. In code:
- Two constructors of the same carrier with unequal identity set `hypotheses_contradictory = true` (51991-51995).
- Two unequal `Integer`s set `hypotheses_contradictory` (52077-52079).
- `note_type_atom` (52084) is called when a variable is bound; it calls `open_closed_type_atom(name)` (52086), which parses the variable name: `strip_prefix("$static-type-predicate:")`, then length-prefixed segments (defined at 89429-89445, outside the range, read 17 lines). Two variables with that prefix pattern and swapped open/closed flags both bound to `true` make the hypotheses contradictory (52093-52106). This is keyed on variable-name text.
- A first-match-wins substitution; occurs-check by `term_contains`.

**Fail-open risks.** The two contradiction paths (51991, 52077) mark the premise set contradictory, which later makes every goal vacuously provable on that path (see `vacuous` at 46925 and the skip at 44843). Classification: traced for the two direct cases; both are sound if the terms really are distinct normalized values. I did not check whether `Integer` terms could be unequal because of an unresolved alias.

### 8h. `identity_elimination` (52155-52489)

**Purpose.** Installs an established `Id` as substitutions or value-identity rewrites, for owned values only.

**How it decides.**
- `identity_value_is_available` (52170): a fixed list of term shapes; `Application` must have a valid result type, and the operator's arity and return type must match when the target is an operator, and its arguments all available, and be an abstract value call, an induction value, or a `specification_application_candidate` (52178-52212). `_ => false` (52240).
- `eliminate_identity` (52269): constructors of different identity make the premise contradictory (52335-52338). Reference fields stop elimination (owned-subject check, 52292-52295). `if !left_owned && !right_owned { return; }` (52355).
- `identity_constructor_field_pairs` (52392) / `identity_constructor_fields` (52446): fields keyed by name; `HashMap<String, TypeReferenceHandle>` returned (52449).

### 8i. `integer_predicates` (52491-54636, partial: ends mid `project_integer_premises`)

**Purpose.** Turns term-level integer predicates and premises into polynomial comparisons for the arithmetic `Engine`, and proves goals against the premises. This is the integer/rank layer of the structural judge.

**Shape.**
- `operand_preparation::PreparedIntegerOperands` (52511): a per-goal cache keyed by `HashMap<u64, Vec<usize>>` with `fingerprint(value)` buckets and `same_resolution_payload` as the equality (52513-52523).
- `rank_comparison`: `CapturedIntegerRank` (52645), `CapturedIntegerInput` (52652), `FormedIntegerRank` (52664), `form_captured_integer_rank` (52672), `compare_captured_integer_rank_sequences` (52769), `observed_rank_input` (52867), `form_arriving_integer_rank` (52899).
- `IntegerObservations` (53006), `PreparedIntegerPremises` (53016), `PreparedIntegerPremisesCacheKey` (53023), `PreparedIntegerPremisesCache` (53046; at most 8 entries, drop oldest, 53072).
- `StructuralJudge` methods: `integer_premise_revision` (53347), `integer_premises_contradictory` (53351), `observed_integer_fits` (53365), `independent_integer_premises` (53378), `integer_term_fits` (53414), `completed_integer_range_recipe` (53425) with `recorded_` (53451) and `rewritten_` (53509), `integer_term_fit_judgment` (53574), `judge_observed_predicate` (53629) and `_uncached` (53639), `proves_observed_integer_boolean` (53738), `integer_boolean_alternatives` (53835), `prove_observed_integer_ordering_minimum` (53943), `prove_observed_predicates` (54094), `prepared_predicate_proofs` (54159), `cached_prepared_integer_premises` (54434), `prepare_integer_premises` (54474), `proves_observed_integer_predicate` (54508), `integer_predicate_observations` (54518), `project_integer_premises` (54566, continues past the end of the unit).

**How it decides.**
- `compare_captured_integer_rank_sequences` (52769): arity must match; each destination component must be at least its minimum; for each position, source - destination must be at least 0, and if it is at least 1 the sequence is strictly decreasing (`Ok(true)`, 52860-52862); else `Ok(false)`. Failures are `Err(&'static str)` messages (for example "captured ranks do not prove lexicographic nonincrease", 52857).
- `form_captured_integer_rank` (52672) requires: observed inputs admitted, a declared measure or a scalar input, the value to land in its primitive's carrier bounds (52718), and `value - minimum >= 0` (52727). It then captures a denotation and every leaf atom (52751-52757).
- `install_premises` (53287) is a fixed-point loop: install admitted comparisons, refresh opaque intervals, then for each operation whose children are admitted either install its bounds or try `admit_selected_integer_operation`.
- `recorded_integer_range_recipe` / `rewritten_integer_range_recipe` only combine `ScalarBinary` terms whose both meanings are `ArithmeticDomain::Exact` (53496-53503, 53553-53561); `ScalarRepresentation` only when `source == target` (53563-53569).
- `proves_observed_integer_boolean` (53738) returns `None` if any cap is exceeded (64 fuel at 53747, at most 32 worlds at 53804, at most 64 comparisons at 53810); `Some(true)` when every world and alternative is unsatisfiable (53832).
- `judge_observed_predicate_uncached` (53639): negation, identity, mathematical `And`/`Or` of checked predicates are composed; otherwise `judge_equation(resolved, true)`, then float, then integer observations.
- `prepared_predicate_proofs` (54159-54432): about 270 lines of hand-ordered shortcuts that return `Some(false)` (disproof), `Some(true)`, or `None`, keyed on counts and registrations (`base_atoms`, `base_bounds`, `base_engine_reg`) and on atom-name text (`atom.starts_with("\0address-model-bound:")`, 54206).

**Cases by shape.** `prove_observed_integer_ordering_minimum` returns only `Some(1)`, `Some(0)` or `None` (53943-54088): it proves a strict or non-strict order, nothing else.

**Same job elsewhere.**
- `values_relation` (37666-37724) and `IntegerObservations::comparison_holds` (53242-53270) both map an operator to a `prove_at_least` of `right - left` or `left - right` with the same table (Less: at least 1; LessOrEqual: at least 0; Equal: both directions; NotEqual: nonzero). Both read.
- `strict_arithmetic_expression_implication` (43686) and `scoped_arithmetic_implication_for_declaration` (37530) differ in input form but both end in `engine.judge`.

**Fail-open risks.**
- `prepared_predicate_proofs` returns `Some(false)` (a definite disproof, which later reads as "not proven") in many branches, and `Some(true)` at 54304 (`operations` empty, operator in `Equal | LessOrEqual | GreaterOrEqual`, `left == right`), 54329 (`!holds || operations.is_empty()` returns `holds`), 54342, 54377 and 54419. These return true from polynomial comparison via the lower-bound queries, so they depend on the prepared premises. Classification: traced only for 54304 (reflexive comparison with no operations and no new opaque terms, `work.engine.integer_projection_registrations().1 == base_engine_reg.1`, no address bound); the others open.
- `is_contradictory` (53127): `requires_unsatisfiable || prove_at_least(-1, 0)` marks the premises unsatisfiable; that makes later goals vacuous (`has_premises && prepared.is_contradictory()` guards in disproof branches, for example 54227). Classification: traced for the guard use, open for the global effect.
- 54128-54131 (`prove_observed_predicates`): a `MathematicalBinary` with no disequalities resolves to `Unknown` without calling `judge_equation`. A skip, not an accept.

---

## 9. Same job elsewhere (cross-unit, suspected)

- The `Unknown`/`Proven`/`Refuted` three-valued enum appears twice: `StrictArithmeticImplicationJudgment` (43680) and `StructuralJudgment` (defined outside this range; used throughout 47979-54632). Both are matched in `static_applications::conjunction` (43619) and `StructuralJudgment::connective` (53685) with the same truth table. `connective` is defined outside the range (suspected, not read).
- Atom identity strings: `format!("\0specification:{:?}", ...)` (42976), `"\0static-const:{:?}"` / `"\0runtime-subject:{:?}"` (43308, 43336), `"\0scoped-call:..."` (36661), `"\0integer-operation:{identity}"` (53155). Five different `\0`-prefixed text identity families for atoms in this range; each is a debug-format string of handles.

---

## 10. Counts and greps used

- `selected_call_contract(`: Grep count, 24 occurrences in 2 files.
- `recognize_guarded_structural_value_arms`: 3 lines (44276 def, 90300 use, 90474 call).
- `shared_proof_walks::`: 3 lines (frozen_program_and_queries.rs 1265, 1270; contract_entailment.rs 8383).
- `open_closed_type_atom`: 3 lines (52086, 52095, def 89429).
- `invocation_is_fact_only`: 3 lines. `call_contract_parameter_position`: 4 lines.
- `static_application_requirement_judgment|generic_tuple_requirements|strict_arithmetic_expression_implication|unsigned_integer_atom`: 17 lines across 6 files.
- `scoped_declared_integer_application_formation|scoped_integer_expression_formation|scoped_arithmetic_implication`: 25 lines.
- `matches_asserted|guard_expressions_equal|case_predicate_owner|exact_type_selection|ScopedCalls::(new|for_declaration)|AppliedProofBindings::for_call\(`: 28 lines, all in contract_entailment.rs.

---

## Coverage

- **Read in full:** `contract_entailment.rs` 36425-54636 = 18,212 lines (read in 11 chunks: 1500 + 9x1800 + 512).
- **Skimmed:** none.
- **Not read:** none.
- Additional lines read outside the unit for the `open_closed_type_atom` definition: 89420-89449 (30 lines).

## Five things a maintainer most needs to know about this unit

1. **"Unsatisfiable hypotheses prove anything" is the standard escape hatch.** `engine.requires_unsatisfiable` (or `hypotheses_contradictory`) turns the goal into `Proven` at 37554, 37706 and 43707, and makes an arm vacuous at 46925 and 44843. The contradiction flags are set from `Integer != Integer` (52077), distinct constructors of one carrier (51991, 52335), and `open_closed_type_atom` name patterns (52093-52106). No site has a comment explaining why vacuity is sound.
2. **Specification validation skips several node kinds silently.** In `validate_specification_expression_in_scope`: type-predicate binaries (41529), operators with no spelling (41537), every other node kind (41659), `owner_parameter_scope` `None` (41730), and named-evidence `Proposition` requirements (41778). Only the mathematical-call skip (41492) has a comment.
3. **Text and debug-string keys appear in decisions.** Names in `exact_parameter_bindings`/`enclosing_type_reference` (40340, 40358), `names_are_unambiguous` (43097), `structural_static_binders_are_exact` (42934), `case_predicate_owner` (41219), `starts_with("__")` for locals (44686), `open_closed_type_atom` on variable names (52086), `starts_with` on `Opaque` text (51176), `parse::<i128>()` on const spellings (43436), plus the `format!("{:?}")` sort and hash keys at 43757, 47234 and following. Equality itself is still done on payloads in the algebra and arrivals code (47186, 43767).
4. **The prefix traversal is written twice.** `recognize_structural_state_prefixes` (44705) and `recognize_structural_state_leaves_from` (45421) both carry the `LocalData` rejoin, `Assignment`, `Expression`, `Call` and citation dispatch (45040-45085 versus 45723-45773). Crash exits are dropped from leaf collection at 46198, and the guarded-arm recognizer's `Always` branch compares `first_condition` to itself at 44316.
5. **Budgets and caps decide many outcomes.** Examples: 4096 steps (36508, 43219), depth 64 (36579), 8192 identity work (48929), 64 monomials (47451), 64 frontier (47276), 32 worlds and 64 comparisons (53804-53810), an 8-entry LRU of prepared premises (53072). Exceeding them gives `None`, `Unknown` or `false`, never acceptance.

## What you are unsure about

- Whether the dropped crash arms at 46198 are intended: I did not trace `recognize_structural_case_arms` callers (the main one is at 90474) to see how crash exits are handled in the theorem's conclusion.
- Whether the `first_condition` versus `first_condition` comparison at 44316 is deliberate (a no-op) or a typo for the second statement's condition. In the `Always` arm there is no second condition to compare, so it may be meant as a placeholder. I only recorded the code.
- Whether `selected_call_contract`-style resolution can disagree with the resolution used by the checker stage for the same call. I only counted uses (24).
- The 54159-54432 block of `prepared_predicate_proofs` shortcuts: I read all of it but did not verify that each `Some(false)` and `Some(true)` is sound.
- I did not read `StructuralJudge`/`StructuralTerm` definitions (outside the range), so claims about fields such as `rewrites`, `case_facts`, `runtime_body_values` rest on their uses here.
- The line 54636 cut: `project_integer_premises` continues past the end of my range.
