# Unit 101 notes

Unit: 101
Files covered (all paths relative to checkout `<checkout>/`):
- `omega-rust/psi/compilation/validation/src/machine_parameters_and_bindings.rs` (1-8309)
- `omega-rust/psi/compilation/validation/src/operators.rs` (1-2910)
- `omega-rust/psi/compilation/validation/src/places.rs` (1-8920)

Method: every line of the three ranges was read with the Read tool, in order. Counts below come from Grep (`-o` content mode, `head_limit 0`, uncapped) over `omega-rust`; where a count says "identifier hits" it is a text match on the name, not a resolved call graph. "read" = I read the code; "grepped" = a Grep result; "inferred" = my reading of intent.

All three files are `include!`-style module fragments (they start with `/* ... */` header comment then `pub mod ...` / `pub(crate) mod ...`), re-exported through `lib.rs` (grepped: lib.rs lines 1446-1447, 1477, 1662-1674, 1762-1796, 1874, 2052-2065 re-export names from these files).

---------------------------------------------------------------------
# File 1: machine_parameters_and_bindings.rs (8309 lines)
---------------------------------------------------------------------

## 1. Purpose
Admission checks for static (compile-time) arguments to generic machines/requirements: when a generic is called with a callable passed as a `Machine`-kind parameter, this checks the chosen callable fits the demanded signature (parameter and return types, borrow regions, effects, contracts, termination, crash routes, nominal satisfaction rows). It also checks authored static arguments (type / const / value / implementation operands) and the stored "static machine binding" tuples (type bounds, const formation, conformance bindings). Header comment (1-5): "Static machine parameter, static argument, and static binding admission and type-argument inference".

## 2. Shape
Three top-level modules.
- `pub mod machine_parameters` (6-5644)
  - `callable_shape` (7-1231): `binder_scope::CallableBinderScope{static_bindings, lifetime_frames}` (30-62; `reversed()` swaps required/actual 41-60); `nominal_inputs::validate` (70-122); `CallableParts` (126-136); `validate_selected_callable_shape` (161-303); `validate_applied_formation` (306); `validate_callable_shape` (335); `validate_callable_parts` (385); `validate_callable_formation` (431-654); `with_callable_region_contracts` (656-812); `complete_callable_type_matches` (815-844); `validate_proposition_parameter_formation` (847); `validate_selected_callable_effects` (893-940); `validate_callable_effects` (943-1015); `validate_trait_callable_parameter_refinement` (1017-1065); `validate_callable_type_parameters` (1068-1177); `validate_callable_input` (1180-1230).
  - `family_demands::form` (1232-1303): for the head of a target-machine family, re-runs `form_call_selection` once per non-head member with type bindings remapped by parameter position (1274-1288).
  - `parameter_contract_tests` (1305-1504, `#[cfg(test)]`): two tests of `machine_parameter_contract_definition`.
  - `selected_dependencies` (1505-1638): `machine_selection_dependency_symbols` (1512), `call_selection_dependency_symbols` (1531), `specialization_selection_dependency_symbols` (1565); visitor struct `MachineSelectionDependencies` (1615).
  - `closed_types::closed_static_call_type_bindings` (1652-1831, `pub fn`).
  - `type_selections::selected_types` (1839-1864).
  - `contract_facts` (1868-2568): `CallableProofScope` (1879), `validate_contract_facts` (1910), `validate_callable_contract_refinement` (1965), `validate_crash_contract_refinement` (2067), `normalized_crash_bucket` (2128), `normalized_facts_in` (2181), `single_guarantee_left_prefix` (2262), `same_guarantee_expression` (2378), `substitute_type_actuals` (2456), `alpha_normalize` (2517).
  - `nominal_admission` (2569-3356): `AdmittedNominalSelection` (2579), `validate_nominal_machine_selection` (2621-3268), `receiving_application_identity` (3270).
  - `reflection_coverage::is_open_field_query` (3362-3415), `has_open_operand` (3416).
  - `type_refinement` (3501-3933): `TypeBinding{symbol,actual}` (3508), `BinderBinding{required,actual}` (3514), `required_type_matches` (3519), `required_type_matches_exact` (3537), `required_type_matches_inner::<EXACT>` (3554-3783), `corresponding_types_match` (3785), `fixed_array_lengths_match` (3812), `infer_call_type_arguments` (3861).
  - `installed_interrupts` (3941-4216): `PreparedInstalledInterruptRegistration` (3947), `form` (3963), `machine_control_service` (4098), `registrar_identities` (4143).
  - Row types (4232-4310): `MachineSuspensionRow`, `MachineBlockingRow`, `ValidatedNominalMachineUseSite{Statement,Expression}` (4245), `ValidatedNominalMachineUse` (4251), `ValidatedRequirementCallTypeBinding` (4266), `ValidatedRequirementCallMachineSelection` (4272), `ValidatedRequirementCallSpecialization` (4280), `PreparedStaticMachineSelections` (4288), `FormedStaticMachineSelections` (4296), `StaticMachineContractObligation` (4302) with `from_applied_callable` (4313), `supplies` (4345), `bounds_static_call` (4359), `required_signature` (4386).
  - Drivers: `form_static_machine_selections` (4453-4567), `admit_static_machine_effects` (4569-4608), `validate_static_machine_selections` (4610), `validate_call_selection` (4659), `CallSelection` (4707), `form_call_selection` (4716-5059), `validate_data_machine_selection` (5061), `validate_declaration_machine_selection` (5088), `validate_conformance_machine_selections` (5356-5511), `provider_tuple_matches_source` (5521-5643).
  - Lookups: `machine_parameter_contract_definition[_in]` (5177, 5232), `machine_parameter_signature` (5259), `MachineStateIndex = SymbolKeyMap<SymbolHandle,(u32,u32)>` (5269), `machine_state_index` (5271), `static_machine_input_parameters` (5294), `declared_type_parameters` (5309), `machine_and_state` (5336).
- `pub mod static_arguments` (5646-7040): `invocation_parameters` (5666), `product_operands` (5736-5910), `requirement_predicates` (5911-6138; `CallScopes` 5920), `validate_source_const_recipes` (6141), `validate_authored_static_arguments` (6195), `source_telescope` (6328), `source_requirement` (6374), `expression_call_states` (6405), `validate_call` (6462), `validate_types` (6572), `spelling` (6646), `forwarded_type` (6653), `is_runtime_value_expression` (6672), `is_runtime_value_subject` (6685), `validate_structural_type_operand` (6701), `ArgumentScope` (6721), `validate_arguments` (6726-7023), `const_expression_argument` (7025).
- `pub mod static_bindings` (7042-8309): `accessor_primitives::validate` (7050), `forwarded_evidence::validate` (7145), `StaticDeclaration` + `find` (7228, 7245), `RetainedCallTarget` (7236), `is_declared_source_tuple` (7302-7462), `dynamic_selector_scope` (7464), `selector_scope` (7525), `authored_tuple_is_refuted` (7542), `validate_static_machine_bindings` (7560-7936), `validate_const_binding` (7938), `validate_conformance_bindings` (8055), `index_identity_arguments` (8164), `lifetime_identity_rows` (8173), `conformance_arguments_match` (8211).

## 3. Inputs and outputs
Reads `&dyn crate::CheckingInput`: machines, states, traits, conformances, data definitions, type-reference table, expression and statement tables, `static_machine_application*` rows, service-reach rows, operational plan. Writes `Vec<Diagnostic>` in place and mutates `bindings: &mut Vec<TypeBinding>` while matching. Produces `PreparedStaticMachineSelections` (nominal uses, requirement-call specializations, crash refinements, installed-interrupt registrations) consumed by program_validation.rs (grepped: program_validation.rs:4125-4146, 4862; preparation.rs:2097-2100; immutable_checking_input.rs:44, 97).

## 4. How it decides
- Callable shape (431-654): positional zip of parameters; calling mode equality `is_mutable/is_const/relevance` (486-489); types by `corresponding_types_match` (3785-3809), which compares `type_identity` of required and actual under `$formation{N}` binder labels (3794) and substitutions. Identity-keyed. Structural matching with generic capture in `required_type_matches_inner` (3554+): `Named` required symbol found in `binder_bindings` -> exact actual symbol (3568-3576); found in `bindings` -> recursive; found in `generic_types` -> push binding (3615-3624).
- Contract facts (1965-2064, 2181-2260): keyed on SOURCE SPELLING. Each fact is rendered to a string (`display_name` for expressions 2208, `"{} in {}<..>"` for memberships 2210-2229, `name[binders](args)` for propositions 2230-2251), then `substitute_type_actuals` (2456; replaces a type-parameter name by the rendered actual only when preceded by `<` or `,`, 2506) and `alpha_normalize` (2517; replaces a word equal to a parameter `name` by `$index`, except after `.`/`:` or before `:`/`(`/`{`, 2544-2545). Requires: `actual.iter().all(|fact| required.contains(fact))` (2013). Ensures: every required fact is in actual, or `single_guarantee_left_prefix` (2017). Crash routes compared as sorted `Vec<String>` (2103-2110, 2152-2154). Only on string mismatch does it call the typed entailment `CallableContractRefinement::prepare/proves` (2037-2043).
- Nominal admission (2621-3268): identity-keyed via `applied_type_identity` / `applied_index_identity` (2848-2866, 2986-3014); the satisfaction row is matched by trait symbol and requirement NAME string: `conformance.requirement ... name.as_str() == requirement.name.as_str()` (3132-3137). Requires exactly one matching row (3188, 3202).
- Reflection special case (3362-3415): keyed on source path and names: `source.origin == Toolchain`, package-relative path equals `reflect.omg` (3397-3399), declaration name in {`visit_runtime_fields`, `visit_runtime_fields_mut`, `inspect_record_fields`, `visit_active_fields`, `visit_active_fields_mut`} (3400-3407). Effect: skips the `Requires` comparison (1991-1994).
- Installed-interrupt control service (4098-4134): toolchain-relative path equals `assembly.omg` (4114-4116) AND normalized hermetic symbol identity equals `StaticInterruptOperation::Install.required_service_identity()` (4119-4122).
- Build-operation skip (4770-4785): when target symbol invalid, `BuildOperation::from_call_target(target_name)` (a STRING of the call target) in a 7-item list returns without checks.
- Static argument kind (6726-7023): classification by `SymbolKind` (`BuiltinType|Data|TypeParameter|State|MachineParameter`), type-reference validity, `const_literal` presence, `evidence_projection` presence.
- Static tuple dedupe (7793-7806): skip when same (declaration, owner) was judged with equal bindings and lifetimes.

## 5. Cases by shape
- `closed_static_call_type_bindings` (1652-1831): returns `None` unless every type parameter is `Type` or `Machine` (1661-1667); each machine selection must have no `application`, `const_literal`, `evidence_projection`, requirement with zero type parameters (1702-1709), machine with zero type parameters (1713); argument must not be a `Borrow` (1754); a non-name argument without declared type is accepted only if `Boolean|Integer|Float` or a `StructLiteral` of non-generic, lifetime-free data (1790-1809, `_ => return None` 1809).
- `selected_types` (1839-1864): a name is a type only if symbol kind is `BuiltinType|Data` or a `TypeParameter` of kind `Type`.
- `validate_selected_callable_shape` (176-302): three accepted selected-symbol shapes (concrete machine/state, forwarded in-scope `MachineParameter`, applied callable view), else error at 300-302.
- `is_declared_source_tuple` (7408-7457): accepts open tuples only for `RuntimeValue` into a `Value` slot, `Machine` binding naming a `MachineParameter` that the declaration has, `Conformance` binding naming a declared bound.
- `validate_const_binding` (7938-8053): three accepted routes (carrier formation, forwarded const binder with matching carrier, closed const).
- `provider_tuple_matches_source` (5539-5558): operand categories Type / Const / Machine / Implementation by symbol-kind chain.

## 6. Same job elsewhere
- Four full walks of every Call expression plus every Call statement in this one file: `form_static_machine_selections` (4461-4565), `validate_source_const_recipes` (6174-6190), `validate_authored_static_arguments` (6220-6249), `validate_static_machine_bindings` (7570-7619). Read.
- Sorted (expression -> machine/state) tables built twice: `CallScopes::build` (5925-5969) and `expression_call_states` (6405-6430), both collect call expressions per state and sort by (arena_index, generation). Read.
- `IndexIdentityArgument` built from binding values in at least four places: `index_identity_arguments` (8164), `nominal_admission` 2748-2761, 2945-2959, `receiving_application_identity` 3288-3305. Read.
- Type-parameter kind pairing: `validate_callable_type_parameters` (1068-1177) vs `StaticDeclaration` parameter checks in `is_declared_source_tuple` (7433-7452). Inferred.
- Machine/trait/conformance parameter signature lookup: `machine_parameter_contract_definition` (5177-5230: linear scan of machines, traits, conformances, data, aliases) vs `program.machine_parameter_signature(...)` used at 4793, 5301, 5328. Read; the two lookups serve overlapping needs.

## 7. Unused / one-caller (greps)
Command: Grep `-o`, `head_limit 0`, over `omega-rust`, names alternated.
- `validate_data_machine_selection` (5061): 1 caller outside the file, type_references_and_wire.rs:1526.
- `validate_source_const_recipes` (6141): 1 caller, 04_typed-trees-to-typed-trees/src/monomorphization.rs:3775.
- `static_machine_input_parameters` (5294): 3 callers, all contract_entailment.rs (lines 83888, 84731, 84768).
- `machine_selection_dependency_symbols`: 1 caller (machine_calls_core.rs:2832). `call_selection_dependency_symbols`: 3 (call_cycles.rs:3666, 3717; machine_calls_core.rs:2847). `specialization_selection_dependency_symbols`: 1 (call_cycles.rs:3498).
- `infer_call_type_arguments` (3861): 2 callers (calls.rs:4697, contextual_and_requirement_calls.rs:2418). `closed_static_call_type_bindings` (1652): 4 callers (calls.rs:4110, checks_borrows.rs:53 and 217, borrow.rs:6811).
- `validate_trait_callable_parameter_refinement` (1017): 3 callers (traits.rs:1099, 2477, 2497). `validate_callable_contract_refinement` (1965): traits.rs:1143 plus own file.
- `admit_static_machine_effects` (4569): 2 callers in program_validation.rs (4146, 4862) plus 4617 in this file. `form_static_machine_selections`: program_validation.rs:4138 plus 4614 here.
- `spelling`, `forwarded_type`, `is_runtime_value_expression`, `is_runtime_value_subject`, `validate_structural_type_operand`: not measured (names collide with many other `spelling(` hits; I did not resolve per-function).
- No comment in this file calls a function THE single/shared/canonical way.

## 8. Stated limits
- 4763-4765: "Constant and layout preparation can arrive before mathematical terms are checked." then early return for mathematical declarations (4766-4768).
- 7084-7087: "generic callers must declare an explicit operation requirement".
- 6781-6782: "A retained source type describes an implementation operand".
- 4856-4857: "erased evidence cannot eliminate into runtime computation".

## 9. Fail-open risks
- `form_call_selection` 4770-4785: invalid target symbol + target-name STRING in the BuildOperation list returns with no check. Open (matched by string; I did not trace what produces such unresolved targets).
- `form_call_selection` 4822-4840: no generic callee resolved and `arguments.is_empty()` returns silently. Traced: errors only when arguments are non-empty.
- `form_call_selection` 4941-4943: `if selected.symbol == parameter.symbol { continue; }` skips all validation for an argument that names the demanded parameter itself. Open.
- `form_call_selection` 4766-4768: mathematical declaration returns early. Stated (comment 4763-4765).
- `installed_interrupts::form` 4028-4033: returns silently when the registrar application is invalid and the owner has open parameters. Open (inferred: generic template deferral).
- `validate_static_machine_bindings` 7679 `if bindings.is_empty() && lifetimes.is_empty() { continue; }` and 7923 (`if complete { validate_conformance_bindings }`): incomplete tuples are not conformance-checked. Open.
- `validate_conformance_bindings` 8085 `Err(_) if open => continue;` and 8101-8103 `if open && matched.is_none() { continue; }`. Traced: `open` is true only when some const binding is not closed (7911-7914); a closed wrong tuple still errors (8104-8110).
- `is_declared_source_tuple` 7396-7401: `require_declared_application_operands(...).is_err()` returns `true` (accept as declared source tuple). Open.
- `requirement_predicates::validate_tuple_requirements` 6132: `Unknown if entry_proof_follows => {}` accepts an unproven requires judgment. Stated by parameter name `entry_proof_follows`; callers pass `true` at 6101 and `false` at 6321 (read).
- `requirement_predicates::validate` 6092-6094 `let Some(signature)=... else { return; }` skips when the applied callable signature cannot be rebuilt; 6085-6088 returns when a finite-family closed tuple is not a member. Open / stated by name `closed_non_member`.
- `validate_arguments` 6912-6947: when no const declaration, no forwarded type and no spelling, it `continue`s after limited checks. Open.
- `provider_tuple_matches_source` 5527-5529 returns `true` when the source operand has no `application`. Traced: nothing to compare.
- `validate_contract_facts`: Requires skipped for open reflection field query (1991-1994; keyed on strings, see 4); Ensures accepted via `single_guarantee_left_prefix` (2017). Stated by function names.
- `normalized_facts_in` 2195-2205 drops literal-`true` facts and type facts proved in scope before comparing. Stated by structure.
- `machine_and_state` 5345-5352 uses `expect` on index positions (panic, not accept).

---------------------------------------------------------------------
# File 2: operators.rs (2910 lines)
---------------------------------------------------------------------

## 1. Purpose
Validates operator declarations (duplicate signatures, overlapping spellings, crash-route form) and builds the checked-tree operator facts: for each executable expression it records which operator (declared, domain-scoped, trait-bound, trait-selected, or builtin) a binary/index/named-call use resolves to, with a status (Resolved, Ambiguous, Missing, Inadmissible, BuiltinFallback, DomainPending). Header (1-5): "Operator declaration validation, namespace resolution, pre-specialization and pre-flow operator selections, and boundary operator applications".

## 2. Shape
- `applications` (6-509): `ValidatedBoundaryOperatorApplication` (24), `ValidatedBoundaryOperatorApplicationArgument::{Type,Const,TypeBinder}` (43), `validate_named_operator_application` (67-144), `landed_integer_literal_type_reference` (146), `validated_boundary_operator_application` (165), `validated_symbolic_boundary_operator_application` (207), `retain_validated_boundary_operator_application` (241), `validate_named_statement_operator_application` (260), `validate_closed_operator_application` (343), `static_argument_matches` (417), `static_const_argument_identity` (435), `static_type_argument_matches` (458).
- `contracts` (510-644): `derive_pre_flow_operator_selections` (516), `build_operator_facts` (523), `derive_checked_operator_realization_contracts` (534), `derive_checked_operator_crash_contracts` (572).
- `namespaces` (645-727): `is_named_operator_namespace` (646), `expression_call_has_operator_namespace` (693).
- `receiver` (728-1070): `expression_type_reference_for_origin` (737), private `expression_type_reference_in_state` (755-906), `symbol_type_reference_in_state` (924), `local_type_reference_before_statement` (950), `data_binding_field_type_reference` (970), `field_type_reference` (993), `data_field_type_reference` (1056).
- `requirement_uses` (1071-1160): `directly_callable_requirement` (1082), `named_requirement_use_fact` (1100), `token_bound_machine_call_target` (1141).
- `scoped_applications` (1161-1508): `OperatorApplicationScope` (1174), `ScopedOperatorApplication::{Declared,BoundTrait,Trait,SelectedTrait}` (1181), `ScopedOperatorResolution` (1205), `scoped_operator_application` (1227-1483), `occurrence_requires` (1485).
- `selection` (1509-1827): `select_pending_domain_operator_meanings` (1523), `finalize_pending_use` (1547), `cast_selects_domain` (1620), `local_initializer_selects_domain` (1645), `signature_facts_select_domain` (1692), `scoped_mint_selects_domain` (1707), `select_scoped_candidates` (1728-1770), `builtin_meaning_exists` (1788), `type_is_primitive` (1807).
- `signatures` (1832-1859), `uses` (1860-2677): `collect_pre_specialization_operator_selections` (1887-1967), `OperatorUseSink` (1970), `build_operator_uses` (1978), `collect_expression_operator_use` (2002), `named_operator_use_fact` (2107), `named_float_policy_adapter` (2192), `operator_use_fact` (2426-2527), `trait_operator_use_fact` (2538), `arithmetic_policy_adapter` (2600), `checked_operator_candidate` (2655).
- Top level: re-exports (2679-2699), `comparison_operator_spelling` (2708), `validate_operator_declarations` (2725), `validate_operator_types` (2763), `validate_spelling_overlap` (2824), `validate_duplicate_operator_signatures` (2853).

## 3. Inputs and outputs
Reads `CheckingInput` (operators, domains, machines, statements, expressions, trait conformances) and `CheckedValueFacts`. Produces `CheckedOperatorFacts` (uses, named uses, named requirement uses, candidates, crash contracts, realization contracts), `ValidatedBoundaryOperatorApplication` rows, selection targets `(ExpressionHandle, AuthoredSelectionResolutionTarget)`, diagnostics.

## 4. How it decides
- `select_scoped_candidates` (1728-1770), identity-keyed on (operator symbol, domain symbol): candidates with a valid domain that is active -> exactly one Resolved, several Ambiguous; else exactly one domainless candidate Resolved; none and `builtin` -> BuiltinFallback; else Missing (no candidates) / Inadmissible; several domainless Ambiguous.
- Domain active (1365-1386, 1563-1593): an operand's type selects the domain (`type_selects_semantic_domain`), or a cast "mint" selects it, or a requires-fact Membership on the same direct binding symbol (1701-1704).
- `cast_selects_domain` (1620-1643): compares `domain.name.as_str() == authored` or last `::` segment `== authored` where `authored` is the joined cast domain path: SOURCE SPELLING.
- `local_initializer_selects_domain` (1645-1680): local found by symbol OR `local.name.as_str() == binding_name` (1671), `binding_name` = `display_name` of the expression (1613).
- `symbol_type_reference_in_state` (924-948) and `local_type_reference_before_statement` (950-968): by symbol when valid, else by name string (941-943, 964-965).
- `named_operator_use_fact` (2147-2166): float format keyed on operator path members compared to the strings "F32", "F64", "Float", and integer names with `from_f32|from_f64` (which map to `None`).
- `type_is_primitive` (1807-1826): `PrimitiveType::from_name(name.as_str())` on a `Named` type reference's name string (1816-1818).
- Duplicate checks (2853-2910, 2824-2851): string equality of `operator_signature_key` (`"{name}({operand signature})"`, 1839-1844) and `operator_operand_key`, unless `source_scopes_separate`.
- Static-argument equality (417-508): type by symbol/application shape; const by `CanonicalConstIdentity::integer(primitive.name(), value)` (455).

## 5. Cases by shape
- 76-90: only `Type|Const` parameters without lifetimes get explicit application; "has a lifetime, machine, or proposition telescope whose explicit application is not yet supported" (87-89), but `Ok(None)` if no static arguments (84-86).
- `static_const_argument_identity` (435-456): integer literals (`value_i64`/`value_u64`) with a primitive carrier only.
- `static_type_argument_matches` (458-508): `Named` and `Generic` (no lifetime args) only; `Reference|Constrained|FixedArray|Slice|ConstExpression|DynamicTrait|Unit => false` (500-506).
- `expression_type_reference_in_state` (765-905): SavedSubject, Match (first arm that resolves, 776-786), Atomic, Name, Member, Borrow/Indexed, Cast, Call, Binary, NaN/Float, ZeroValue, StructLiteral, ArrayLiteral; `Boolean|Integer|Range|String|Unary => None` (900-904).
- `named_requirement_use_fact` (1100-1139): boundary traits with no trait or signature type parameters and a static receiver (1108-1117) or top-level requirement machines with no type parameters, no body, one entry state (1082-1098).
- `derive_checked_operator_realization_contracts` (534-570): conformances whose `conformance.symbol` is NOT a trait definition but has an operator declaration (543-554).
- Index/Range paths (1337-1344, 2449-2457); builtin indexing recognised only for `FixedArray`/`Slice` first operand (1414-1419, 2470-2481).

## 6. Same job elsewhere
- Operator resolution runs three times for binary uses: `scoped_operator_application` (1227), `operator_use_fact` (2426: `selected_trait_operator_meanings`, then `resolve_spelling_for_operands`), and `collect_pre_specialization_operator_selections` (1887-1967: `bound_trait_operator_meanings`, `selected_trait_operator_meanings`, `resolve_spelling_for_operands`). Read.
- Building a trait-backed candidate fact with signature appears twice: `scoped_candidate_fact` SelectedTrait arm (2341-2377) and `trait_operator_use_fact` (2547-2575). Read.
- F32/F64 policy-adapter mapping appears three times: `named_requirement_use_fact` (1124-1132), `named_operator_use_fact` (2167-2181), `arithmetic_policy_adapter` (2600-2636). Read.
- Expression-to-type for an origin: `receiver::expression_type_reference_in_state` (755) is a partial reimplementation beside `crate::expression_result_type_reference` (called 818, 827, 1924); and places.rs has a same-named public function `expression_type_reference_in_state` (places.rs:7457) built from canonical places (read both; different implementations, same name).
- `origin_machine_symbol` (2529) vs inline `CheckedValueOrigin::StateStatement{..}` destructuring at 1489, 1652, 2252. Read.

## 7. Unused / one-caller (grep across omega-rust excluding operators.rs)
- `derive_checked_operator_realization_contracts` (534, pub): 0 call sites outside; used only at 529 inside `build_operator_facts`.
- `derive_checked_operator_crash_contracts` (572): lib.rs:1446 re-export only; also used at 528 here.
- `scoped_operator_application` (1227): lib.rs:1447 re-export and contract_entailment.rs:41570 (1 real call).
- `validated_symbolic_boundary_operator_application` (207): 1 call (calls.rs:3276). `validated_boundary_operator_application` (165): 1 outside call (calls.rs:3263).
- `is_named_operator_namespace` (646): 1 outside call (calls.rs:776). `expression_call_has_operator_namespace` (693): 3 (value_custody_core.rs:5488, calls.rs:2686, 3228).
- `token_bound_machine_call_target`: 1 outside call (05/shared.rs:1235) plus lib.rs re-export.
- `canonical_closed_operator_realization_bytes` (30): wrapper over `typed_trees::operator::canonical_closed_operator_realization_bytes`; outside callers machine_calls_core.rs:3572 and 06/projections.rs:1531.
- `type_is_primitive` (1807), `arithmetic_policy_adapter` (2600, pub(crate)): 0 outside.
- `derive_pre_flow_operator_selections`: widely used (build-time-evaluation const_evaluation.rs, machine_execution.rs, syntax_probes.rs, const_initializers.rs; omega/build-system build-settings admission/checking.rs:158). `comparison_operator_spelling` and `landed_integer_literal_type_reference`: used in more than 10 files each.

## 8. Stated limits
- 88-89 "explicit application is not yet supported".
- 2779-2780 "domain memberships and proposition applications are proof facts, not runtime-refinable crash routes".

## 9. Fail-open risks
- `validate_named_operator_application` 84-86 and 107-109 return `Ok(None)` (no closed application recorded, no error) when the operator has a lifetime/machine/proposition telescope or the operand application is open, and no static arguments were supplied. Traced to the call site calls.rs:3255 only by name; effect of `None` there not read. Open.
- `validate_named_statement_operator_application` 269-274 returns silently if the target is not an operator or has a spelling; 300-312 retains only boundary operators. Stated by structure.
- `scoped_operator_application` returns BuiltinFallback with no candidates for typed case-membership (1234-1244) and anonymous numeric operands (1245-1261). Stated by helper names `has_exact_typed_case_membership_meaning`, `has_anonymous_operator_meaning`.
- `select_scoped_candidates` 1753-1756 `[] if builtin => BuiltinFallback`; `builtin` for pending uses = `builtin_meaning_exists` (1788-1805) = left operand type is a primitive BY NAME (`type_is_primitive` 1816). A user type spelled like a primitive name with no declared candidate would take this path if it reaches a `Named` node with that name. Open (not traced whether `Named` names can be shadowed).
- `collect_pre_specialization_operator_selections` 1939-1943: `continue` (no selection, no diagnostic) when no trait meaning exists and any operand type is non-primitive. Open.
- `build_operator_uses` 2075: binary uses whose operand types are all `None` create no fact. Open; `receiver::expression_type_reference_in_state` returns `None` for many shapes (900-904, 886 `operands[0].or(operands[1])`).
- `domain_is_active_at_use` 1570 `unwrap_or_default()` on operands => domain inactive (fail-closed).

---------------------------------------------------------------------
# File 3: places.rs (8920 lines)
---------------------------------------------------------------------

## 1. Purpose
The "canonical place" layer: turns expressions into root + segment paths (`CanonicalPlace`), says whether two paths overlap or contain one another, projects declared types along a path, resolves member symbols, recognises "builtin coordinates" (plain fixed-array/slice indexing), and recognises structural scalar fields (a scalar reached by field/index steps from a local/parameter). Header (1-4): "Canonical places, storage coordinates, collection extents, and projected receiver and storage-path relations (`value_custody::places`)".

## 2. Shape (module, lines; entry functions)
- `binding_observations` (5-395): `stable_slice_descriptor_place` (13), `stable_reference_descriptor_place` (24), `binding_is_exclusively_exposed` (51), `binding_is_written` (125), `binding_has_write_effects` (149), `call_receiver_is_mutable` (198), `borrow_root_symbol` (215), `binding_is_unchanged_value` (233), `place_has_immutable_denotation` (247), `value_reads_have_immutable_denotation` (279), `immutable_binding` (306), `immutable_read_place` (327), `immutable_value_origin` (356). `BindingObservation{WholeValue,Descriptor}` (120).
- `captured_ownership::ownership_place_from_expression_in_state` (396-441).
- `storage_path_relations` (442-794): `case_tag_segments_may_overlap` (443), `place_segment_has_static_identity` (459), `place_segment_has_unresolved_identity` (468), `static_place_segment_pair_equal` (476), `canonical_place_segments_equal` (505), `canonical_places_agree` (546), `PlaceRelation{Disjoint,Equal,Prefix,Extends,MayOverlap}` (571) with block comment (560-569), `place_relation` (587), `place_segments_relation` (598), `canonical_places_may_overlap` (620), `canonical_place_segments_may_overlap` (628), `canonical_place_joined_segments_may_overlap` (646), `canonical_place_segment_pair_may_overlap` (665), `index_expressions_may_overlap` (728), `binding_overwrite` (754), `contents_overlap` (761), test (768-793).
- `collection_operations` (796-861): `collection_measure_member` (806), `core_vector_measure_member` (826), `type_reference_is_collection` (849).
- `declaration_types` (862-1061): `declaration_type_equations` (867), `closed_requirement_equations` (899), `state_root_type` (940), `declaration_type_reference` (955-1060).
- Top-level 1066-1441: `assignment_targets_parameter_referent` (1077), `validate_assignment_target_handle` (1101-1183), `is_mutable_place_handle` (1185), `expression_root_name_handle` (1197), `direct_self_field_member` (1227), `machine_attached_data` (1248), `declared_place_type` (1255), `assignment_value_type` (1265), `declared_place_type_raw` (1275-1394), `declared_member_field_symbol` (1396), `bound_symbol_declared_type` (1411).
- `array_extents` (1443-1653): `ResolvedArrayExtent{Closed(BigInt),Bound{..}}` (1454), `resolved_array_extent_at` (1462), `resolved_projected_array_extent` (1490), `resolved_array_extent` (1506), `ExtentResolver` (1532).
- `attached_storage` (1654-1765): `attached_storage_root`, `exact_attached_storage_field`, `expression_names_attached_storage`, `canonical_storage_label`, `declaration_storage_label`.
- `builtin_coordinates` (1770-2499): `collection_length_receiver` (1778), `declared_collection_extent` (1795), `fixed_array_type_length` (1805), `fixed_array_type_extent` (1815), `subslice_expression` (1836), `has_builtin_subslice_meaning` (1861), `range_operator_establishes_endpoint_extent` (1882), `ensured_range_call_endpoints` (1926), `contracts_establish_endpoint_extent` (2002), `expression_establishes_endpoint_extent` (2034), `expression_names_source_parameter` (2098), `reference_has_builtin_collection_geometry` (2152), `indexed_operation_has_builtin_meaning` (2175), `whole_collection_view_source` (2223), `assignment_target_selectors/indexes` (2269, 2284), `place_has_builtin_coordinates` (2311), `value_element_has_builtin_coordinate` (2320), `value_has_builtin_coordinates[_with_types]` (2337, 2348), `mutable_byte_slice_parameter_element` (2398), `builtin_coordinates` (2425).
- `exact_self_field` (2500-2563): `single_member_receiver_field`, `exact_self_field`, `exact_attached_field`.
- `structural_scalar_field` (2564-3991): `StructuralScalarField` (2580), `structural_scalar_field_in_parameters` (2594), nested `coordinates` (2627-3094: `ScalarCoordinateProjection`, `StructuralCoordinate`, `StructuralScalarCoordinate`, `Endpoint`, `structural_scalar_coordinate[_in_type]` 2700/2721, `structural_coordinate[_in_type]` 2763/2784, `structural_scalar_field_at_coordinate` 2995, `..._at_element_coordinate` 3039), `structural_scalar_field` (3102), `..._of_evaluated_receiver` (3122), `structural_atomic_field` (3142), `structural_field_observation` (3169-3382), `atomic_scalar_carrier` (3384), `observable_scalar` (3396), `readable_projection_carrier` (3416), `selected_member_field` (3443), `Step` (3512), `scope` (3520), `StructuralSource` (3543), `saved_dispatch_subject` (3578), `saved_dispatch_call` (3608), `same_authored_call` (3697), `dispatch_case_subject` (3713), `structural_source` (3758-3914), `realized_scalar_carrier` (3916-3981).
- `member_paths` (3992-4217): `declared_member_path_type` (4001), `collect_member_path` (4037), `first_unknown_nested_field` (4058), `resolve_nested_member_path` (4123), `data_definition_for_type` (4144), `local_or_parameter_type` (4160), `data_field_or_payload[_type]` (4187, 4195).
- `parameter_scoped` (4218-4361): `parameter_scoped_type_reference` (4225), `parameter_source_type_application` (4234), `expression_type` (4274), `has_retained_builtin_index_meaning` (4338).
- `projected_members` (4365-4532): `applied_data_member_field` (4373), `applied_data_field` (4387), `contains_case_projection` (4414), `declared_case_projection_type/field` (4428, 4441), `exact_data_member_field` (4482).
- `result_shape::expression_result_is_reference` (4533-4688).
- 4716-4796: `declared_indexed_projection_type[_raw]`, `unwrapped_type_reference`, `peel_declared_constraints`, `collection_element_type`, `declared_reference_access`.
- `declared_carriers` (4798-5103): `Substitutions{Exact,DeclarationEquations,Application}` (4806), `under_equations` (4812), `under_application` (4826), `normalize_root` (4841), `compare` (4874-4994), `substituted_declared_carrier` (4996); tests (5024-5102). Wrappers `declared_carriers_match*` (5106-5130). `zeroed_structural_parts` (5132-5200).
- `declared_symbols` (5202-5444): `machine_symbol_from_type_reference_handle` (5208), `expression_root_symbol` (5235), `first_valid_name_path_symbol` (5265), `runtime_field_subject` (5283), `scoped_binding_type` (5302), `nearest_scoped_local` (5325), `structural_member_identity` (5355), `data_member_symbol_by_name` (5369), `declared_data_type` (5397), `state_scoped_name_symbol` (5418).
- `canonical_projection` (5450-6438): `CanonicalPlace{root: PlaceRoot, segments: Vec<PlaceSegment>}` (5465) with `compare_identity` (5471), `canonical_collection_element_place` (5544-5638), `index_place_segment` (5640), `element_place_segment` (5652), `exact_fixed_element` (5678-5739), `standing_fixed_element` (5741), `folded_coordinate` (5848), `canonical_place_from_expression` (5943-6031), `cast_preserves_place` (6033), `place_root_may_be_symbol` (6153), `name_materializes_value` (6178), `storage_operand` (6205), `canonical_place_from_expression_in_state` (6217), `syntax_queries` (6244-6437, memoised `ProjectionSyntaxQueries` via `frozen_memo` 6341).
- `contextual_projection` (6446-6649): `contextual_canonical_place_from_expression` (6459), `resolve_contextual_name_path_root` (6589), `resolve_member_symbol_from_place` (6605).
- `member_resolution` (6653-7442): `MemberPosition{Reference,Declaration,Sliced}` (6664), `container_member_type_position` (6753), `symbol_type_position` (6913), `whole_program_member_type_position` (6943), `expression_type_position` (7002-7103), `member_position_after_segments` (7123), `expression_place_type_reference[_observing]` (7174, 7182), `effective_member_symbol[_from_position]` (7227, 7240), `saved_subject_member_symbol` (7314), `resolve_*_member_symbol_from_type_symbol` (7369, 7390).
- `projection_types` (7447-8690): `expression_type_reference_in_state` (7457), `StorageObservationTypePath[Error]` (7477, 7487), `observed_storage_type_references` (7507), `applied_members::{corresponding_loan_owner_path, corresponding_place_projection}` (7590-7707), `qualification_paths::{qualification_storage_path, canonical_qualification_path}` (7710-7853), `ObservationTypeCursor` (7856), `canonical_place_type_reference[_observing]` (8063, 8098), `canonical_place_source_type_application` (8197), `source_type_application_from_segments` (8218), `project_type_reference_from_segments` (8261), `qualification_storage_type_reference` (8269), `owned_projection_type_reference` (8310), `loan_carrier_has_owned_storage` (8325), `ProjectionTypeCursor` (8387-8457), `symbol_type_reference_in_state` (8459), `field_type_reference` (8516), `indexed_element_type_reference` (8593), `substituted_type_reference` (8628), `unique_data_definition_by_symbol` (8659), `data_field_type_reference` (8666).
- `receiver_projections` (8702-8917): `root` (8719), `field` (8745), `expression` (8767), `check_symbol` (8858), `projected_statement_receiver_place` (8875), `projected_statement_receiver_type_reference` (8900).

## 3. Inputs and outputs
Reads `CheckingInput` (expression/statement/type-reference tables, state parameters and locals, data definitions, machines, symbols), plus `facts::{PlaceRoot, PlaceSegment}` and checked_trees types. Outputs: `CanonicalPlace` values, `TypeReferenceHandle`s along a path, booleans about overlap/immutability/builtin-ness, `StructuralScalarField`/`StructuralCoordinate` records, `ResolvedArrayExtent`, a few `Diagnostic`s (`validate_assignment_target_handle`, `receiver_projections::check_symbol`). Mostly pure queries; one memo (`syntax_queries`, 6341-6347).

## 4. How it decides
- Overlap (628-710): common-prefix zip only. A segment with an invalid symbol (`Field{symbol}` or `Case{variant}` invalid) -> overlap `true` (634-637). Pair rules (665-710): static pairs compare by symbol / variant / index (476-503); `OpaqueRange` overlaps anything (671); `FixedRange` vs `FixedRange` by interval (682-685); range vs index by containment (687-695); `Index` expression vs `FixedIndex` by constant value, unknown => overlap (696-699); `Index` vs `Index` via `index_expressions_may_overlap` (728-752: same handle or invalid or non-literal => overlap; two `Integer` nodes compare); any other kind pair `_ => false` (708). Keyed on symbol identity, handle identity, constant value.
- Containment (598-618): relation is `Disjoint` if no overlap, else `MayOverlap` if some common segment is not "equal" (equal = 505-541, false for unresolved identity), else Equal/Prefix/Extends by length. Roots compared by `left.root != right.root` (592) -> Disjoint.
- `case_tag_segments_may_overlap` (443-457): returns `false` when `written.len() > observed.len()` and every segment is resolved (448-455).
- Place construction (5943-6031): Name -> `PlaceRoot::Symbol(first valid symbol of path)` plus segments from `name_path_member_symbols` (5978-6004); `name_materializes_value` (6178: exact case reference or `SymbolKind::Const`) -> `PlaceRoot::Expression(expression)`; any expression kind not listed -> its own `PlaceRoot::Expression` (6026-6029).
- Dynamic vs fixed index (5652-5676): index becomes `Index{expression}` if the expression tree contains any `Call` and has no closed integer value (5656-5672); else `exact_fixed_element` folding (5678-5739, depth cap 64 at 5683): closed integer, closed record projection, match-arm literal bindings (5827-5846), `folded_coordinate` with standing bounds, immutable local initialiser, `Match` whose arms all fold equal (5721-5727), `Member` of a struct literal initialiser.
- Member symbol resolution is layered and partly by NAME: `exact_data_member_field` (4482-4531): `field.name.as_str() == member_name` and, if symbol valid, symbol equality; ambiguity -> `None`. `data_field_or_payload` (4195-4216), `data_member_symbol_by_name` (5369-5393), `resolve_member_symbol_from_type_symbol` (7390-7415), `resolve_case_member_symbol_from_type_symbol` (7369), `local_or_parameter_type` (4160-4185), `nearest_scoped_local` (5325-5353), `state_scoped_name_symbol` (5418-5443), `runtime_field_subject` (5283-5300), `declared_data_type` (5397-5416, `definition.name == *name`) all match by name string.
- `resolve_contextual_name_path_root` (6589-6603): `name = display_name(expression)` then `state_scoped_name_symbol(.., &name, ..)` if no valid symbol on the path: display-name string lookup.
- `carriers_match` (6096-6151): equal handles, else `Named`/`Generic` equal by symbol if both valid else by `name` equality (6124-6128, 6142-6146).
- `ExtentResolver::reference` (1602-1639): for `Named` with INVALID symbol, the extent is parsed from the name string `name.as_str().parse::<i128>()` (1619) or `CanonicalConstValue::from_atom(name.as_str())` (1623).
- `collection_measure_member` (806-824): `CollectionMeasure::from_authored_spelling(member.member.as_str())` (823).
- Endpoint-extent recogniser (2002-2096): requires exactly three parameters `[_, start, end]` (2008), both integer, `ArithmeticDomain::Exact` (2011-2017); an `ensures` fact of form `X.len == end - start` where member name is the string "len" (2071), result owner equals the callable, `Subtract` with left = parameter at position 2 and right = parameter at position 1 (2081-2094); name equality `name.as_str() == parameter.name.as_str()` (2141); conjunctions descended with depth cap 128 (2041).
- `effective_member_symbol_from_position` (7240-7312): returns invalid unless `program.pattern_component(receiver) == Some(PatternComponent::Fields)` (7246-7248); then saved-subject, closed generic, case-variant, struct-literal-field, type-symbol by name, retained symbol, in that order.
- `WHOLE_ELEMENT_EXTENT` sentinel: a `FixedRange` segment compared by value `*extent == crate::field_domains::WHOLE_ELEMENT_EXTENT` (7774, 8247, 8281, 8430); definition not in this unit (suspected: a constant range value in field_domains.rs, not read).
- Immutability (247-394): write detection scans EVERY statement of the state, not only those before the use (131-147, 159-163, 174-195); local declaration order checked only for the local's own declaration `position < before` (318, 384). Call receiver counted as written when the call target is not a machine state (198-200).

## 5. Cases by shape
- `observable_scalar` (3396-3414): fixed list `Bool, Addr, I8, I16, I32, I64, U8, U16, U32, U64, F32, F64, Selector`.
- `structural_field_observation` (3169-3382): steps are `Member` or `Element`; element steps only on `FixedArray` with `FixedArrayLength::Literal` (3276-3279, 3310-3313) and a constant integer index (3286-3288, 3320-3322).
- `structural_coordinate_in_type` (2784-2971): segments `Case`, `Field`, `FixedIndex|Index` (when no case selected); `FixedRange`/`OpaqueRange`/other `_ => return None` (2954).
- `declared_member_path_type` (4001-4035): path shapes `[name]`, `[self, field]`, `[root, field]`, `len >= 3`; else `None`.
- `mutable_byte_slice_parameter_element` (2398-2423): only `&mut [u8]` (2405-2422).
- `ensured_range_call_endpoints` (1926-1936): exactly three arguments `[_, start, end]`.
- `zeroed_structural_parts` (5132-5200): primitive; fixed array with literal length (length 0 stops, 5156); closed data with non-erased fields; for data with variants only the FIRST variant's payload is walked (5169, 5189-5194).
- `containing_...` `expression_result_is_reference` (4548-4655): handles Member/Indexed/StructLiteral/ArrayLiteral/Borrow/Cast; scalar literals only with empty projections (4639-4650); `_ => return None` (4651).
- `qualification_storage_path` (7713-7795): rejects `Index` (7771-7773) and any range other than the whole-element sentinel (7777-7779) with string errors.
- `canonical_collection_element_place` (5544-5638): range windows normalised to a fixed index only if bounds are closed integers (5573-5578).

## 6. Same job elsewhere
- Six separate "walk a place-segment list over type references" cursors in this file: `ProjectionTypeCursor` (8387), `ObservationTypeCursor` (7856), `member_position_after_segments` (7123), `structural_coordinate_in_type` (2784), `corresponding_place_projection` (7632), `source_type_application_from_segments` (8218). Read.
- Field lookup by symbol/name repeated: `exact_data_member_field` (4482), `data_field_or_payload` (4195), `data_member_symbol_by_name` (5369), `resolve_case_member_symbol_from_type_symbol` (7369), `selected_member_field` (3443), `data_field_type_reference` (8666), field matching inside `structural_coordinate_in_type` (2842-2878). Read.
- "Unique non-const, non-erased parameter by symbol" filter duplicated: 2706-2711 and 2769-2774. Read.
- `StructuralScalarField` built in five places: 3024, 3080, 3230, 3335, 3365. Read.
- Expression -> declared type: `declared_place_type_raw` (1275), `expression_type_position` (7002), `parameter_scoped::expression_type` (4274), `projection_types::expression_type_reference_in_state` (7457), plus operators.rs:755 `expression_type_reference_in_state` and `crate::expression_result_type_reference` (not in unit). Read.
- Name-to-type lookups: `local_or_parameter_type` (4160, by name), `symbol_type_reference_in_state` (8459, by symbol), `state_root_type` (940), `scoped_binding_type` (5302), `bound_symbol_declared_type` (1411, whole program scan), `receiver_projections::root` (8719), `symbol_type_position` (6913). Read.
- Peeling reference/constrained wrappers: `fixed_array_type_extent` (1815), `reference_has_builtin_collection_geometry` (2152), `readable_projection_carrier` (3416), `closed_generic_owner` (6683), `unwrapped_type_reference` (4763). Read.
- Overlap loop twice: `canonical_place_segments_may_overlap` (628-644) and `canonical_place_joined_segments_may_overlap` (646-663) are the same loop over `chain`. Equality: `static_place_segment_pair_equal` (476) used by both equality (515) and overlap (670).
- Same name twice: `expression_type_reference_in_state` here (7457) and operators.rs:755; also `data_field_type_reference` here (8666) and operators.rs:1056. Read.

## 7. Unused / one-caller / documented-as-shared (greps)
- The block comment at 560-569 says `PlaceRelation` is how two places relate so "every keep-or-kill decision gives the same answer for the same two places (wiki/drafts/designs/fact_transport.md)". Grep `\b(place_relation|place_segments_relation|PlaceRelation)\b` with `-o` over `omega-rust`: 22 hits, ALL in places.rs (lines 571-625, 777-790); zero users outside the file. Only `canonical_places_may_overlap` (620) calls `place_relation` inside this file; `canonical_places_may_overlap` is used outside (many hits in 05_typed-trees-to-checked-trees: checks_termination_progress.rs, checks_ranges_facts.rs, published_facts.rs, flow.rs, checks_contracts.rs). But `canonical_place_segments_may_overlap`, `contents_overlap`, `binding_overwrite`, `case_tag_segments_may_overlap`, `canonical_place_joined_segments_may_overlap`, and `canonical_places_agree` do not go through `place_relation` and are used widely outside (grepped listing: flow.rs, flow_place.rs, flow_mutation.rs, flow_operand_contexts.rs, flow_call_phases.rs, flow_exits.rs, checks_*.rs, domains_and_predicates.rs:10334, completed_storage_and_conservation.rs:294/332/338, 06/unit_structural_values.rs:2902). The `Prefix`/`Extends`/`Equal` distinction is consumed by no caller outside this file.
- `immutable_value_origin` (356): 5 outside callers (contract_entailment.rs:9177; calls.rs:5282, 5683, 5758, 5858).
- `core_vector_measure_member` (826): 1 outside caller (expression_types_and_fields.rs:5163). `collection_measure_member`: not separately counted.
- `qualification_storage_type_reference` (8269): 1 outside caller (06_checked-trees-to-lowered-psi/src/emission.rs:155).
- `zeroed_structural_parts` (5132): callers build-time-evaluation/layouts.rs:6254 and execution_scalar_computations.rs:71, 7358, 7448, 10678; execution_terminal_unit_control_statement_sequence.rs:1377 (6 outside).
- `structural_scalar_field_of_evaluated_receiver` (3122): 1 outside caller (execution_scalar_computations.rs:6895). `structural_atomic_field` (3142): 1 outside (execution_scalar_computations.rs:132). `saved_dispatch_call` (3608): 2 outside (flow_ownership.rs:53, execution_terminal_unit_control.rs:2719).
- `ensured_range_call_endpoints` (1926): 1 outside (checks_current_scalar_values.rs:1281). `mutable_byte_slice_parameter_element` (2398): 1 outside (checks_ranges_facts.rs:3841).
- `stable_slice_descriptor_place` (13): 4 outside (contract_entailment.rs:76960, arithmetic_domains.rs:18891, 20504, published_facts.rs:2428).
- `canonical_storage_label` (1731): 9 outside (all checks_ranges.rs); `declaration_storage_label` (1749): 2 outside (checks_ranges.rs:3082, 3098).
- `loan_carrier_has_owned_storage` (8325): 4 outside (checks_contracts_exits.rs:7825, checks_borrows_calls.rs:494, checks_borrows.rs:728, plus lib.rs re-export). `canonical_owned_place_type_reference` (8078): 5 outside (execution_scalar_computations.rs:2732, flow_place.rs:658, 685, borrow.rs:4861, plus lib.rs re-export).
- `owned_projection_type_reference` (8310): heavily used (>40 hits outside in 05_typed-trees-to-checked-trees, contract_entailment.rs, calls.rs, arithmetic_domains.rs, completed_storage_and_conservation.rs).
- Not counted: remaining pub functions in this file (not grepped individually).

## 8. Stated limits
- 560-569 (design comment on PlaceRelation): "A range that covers an index is `MayOverlap`, which a caller must treat as not contained."
- 4961-4964: "Reused declarations can retain different symbol handles for the same nominal owner."
- 6245-6249 (syntax_queries doc comment): "These rows select no executable arm and establish no numeric or flow fact".
- 6416-6418: "A failed effectful pattern cannot grant the captured subject's equality to a later storage read."
- Error strings stating limits: 1526 "array extent still requires selected constant evaluation"; 7772 "qualification storage requires a statically captured index"; 7778 "unsupported collection window".
- Hard caps: loop/depth 128 at 94, 220, 1712, 2041, 2156, 2290; 64 at 5683.

## 9. Fail-open risks
- `case_tag_segments_may_overlap` 448-455: a write path LONGER than the observed path, with all segments resolved, is treated as NOT overlapping the observed case tag. Open (no comment; callers flow_exits.rs:57, 768; flow.rs:11818; flow_place.rs:1821 not read).
- `canonical_place_segment_pair_may_overlap` 708 `_ => false`: segments of different kinds (e.g. `Field` vs `FixedIndex`, `Case` vs `Field`) count as disjoint. Open (inferred: well-typed paths do not mix kinds at the same depth, but `Case` segments precede `Field` payload segments, see `push_field_place_segments` 5932-5941).
- `canonical_place_from_expression` 6026-6029 and 6585: any expression that is not Name/Member/Indexed/Borrow/Cast/SavedSubject becomes its own `PlaceRoot::Expression`; `place_relation` 592 then reports `Disjoint` against any other root. Open (a call result or literal is a fresh value, so likely intended; not traced).
- `place_root_may_be_symbol` 6169-6171 `.is_none_or(|root| root == symbol)`: unresolved root => "may be" (conservative). Fail-closed for may-queries.
- `index_expressions_may_overlap` 733-740, 750 returns `true` for unknown (conservative).
- `validate_assignment_target_handle` 1141-1143: returns without error if `expression_root_name_handle` is `None`. Traced: `is_mutable_place_handle` (1185-1195) only admits Indexed/Member/Name chains, so `None` needs a Name with zero members; low reach.
- `validate_assignment_target_handle` 1131-1139 relies on `first_unknown_nested_field` (4058-4121) which returns `None` (no error) whenever a receiver type cannot be resolved (4071, 4072, 4106, 4109, 4118) and for `self.<machine-owned data>` (4099-4105). Open: unresolved typed receivers produce no "no field" diagnostic from this function (other checks not traced).
- `validate_assignment_target_handle` 1169-1171: write accepted when `writes_borrowed_storage` even if the root is not in `writable_roots`. Stated by variable name; inferred meaning: assignment through a `&mut` parameter.
- `binding_observations`: `selects_collection_contents` and `borrow_root_symbol` stop at 128 steps and return false/None (94-103, 220-230): fail-closed for write detection in `binding_has_write_effects`? `borrow_root_symbol == None` means "not this symbol" (e.g. 137, 179), so a chain longer than 128 nodes would be judged not to write the binding. Open (practically unreachable).
- `builtin_coordinates` 2436 `ExpressionNode::Name(_) => true`: any name is a valid builtin base; correctness relies on the Indexed arms above it (2444-2495). Stated by structure.
- `structural_field_observation` 3258 closure `value_coordinate = !fresh || ...`: when the source is not a fresh value the builtin-coordinate check is skipped here and done earlier at 3247-3256 (traced, read).
- `zeroed_structural_parts` 5169-5194 only inspects the first variant of a data type with variants. Open: a data type whose first variant is zero-representable and later variants are not would be judged zeroable by this function; callers (execution_scalar_computations.rs:71 etc.) not read.
- `carriers_match` 6124-6128 and 6142-6146: falls back to NAME equality when either symbol is invalid. Open.
- `ExtentResolver::reference` 1618-1626: a `Named` reference with invalid symbol is read as a number from its spelling. Open.
- `effective_member_symbol_from_position` 7246-7248 returns invalid for non-`Fields` receivers, so callers fall back (contextual_projection.rs path 6546-6557 resolves from the place). Traced within this file.
- `nearest_scoped_local` 5339-5351: with `scope_limit` it picks the LAST local of the same name (shadowing) but parameters first; `usize::MAX` used by `runtime_field_subject` (5290) = whole state. Stated by arguments.

---------------------------------------------------------------------
# Coverage
- `machine_parameters_and_bindings.rs` lines 1-8309: read in full (8309 lines).
- `operators.rs` lines 1-2910: read in full (2910 lines).
- `places.rs` lines 1-8920: read in full (8920 lines).
- Total read in full: 20139. Skimmed: 0. Not read: 0.
- Test modules (`parameter_contract_tests` 1305-1504; `relation_tests` places.rs 768-793; `declared_carriers::tests` places.rs 5024-5102) were read but I did not trace their assertions beyond what is stated above.

# Five things a maintainer most needs to know about this unit
1. Contract refinement for static machine arguments (machine_parameters_and_bindings.rs:1965-2064, 2181-2260, 2456-2558) compares `requires`/`ensures`/`crashes` as RENDERED STRINGS (display names, with textual type-parameter substitution and `$index` renaming of parameter names); typed entailment (`CallableContractRefinement`) is only a fallback after a string mismatch (2037-2043). Renaming alone is normalised away; other spellings of the same fact are not.
2. `PlaceRelation` / `place_relation` / `place_segments_relation` (places.rs:571-618) are documented as the single answer for keep-or-kill decisions, but grep shows no user outside places.rs; the wide consumers call `canonical_place_segments_may_overlap`, `contents_overlap`, `binding_overwrite`, `case_tag_segments_may_overlap`, `canonical_places_agree` or `canonical_places_may_overlap` directly.
3. Several lookups in places.rs resolve members, locals and constants by name string rather than symbol (4160-4216, 5283-5443, 6589-6603, 6124-6146, 1618-1626, 7390-7415); operators.rs does the same for primitive-type detection (1816) and cast domains (1620-1643). Symbol-based routes exist alongside (e.g. `symbol_type_reference_in_state` 8459, `exact_data_member_field` with symbol 4482).
4. Three families of special cases are keyed on source paths/names: `reflect.omg` + five function names (machine_parameters_and_bindings.rs:3397-3407), `assembly.omg` + hermetic identity (4114-4122), and the `len == end - start` range-endpoint recogniser requiring exactly three parameters with positions 1 and 2 (places.rs:2002-2096).
5. The same traversals are repeated inside each file: four full call-site walks in machine_parameters_and_bindings.rs (4461, 6174, 6220, 7570), three operator-resolution paths in operators.rs (1227, 2426, 1887), and six place-segment-to-type cursors in places.rs (2784, 7123, 7632, 7856, 8218, 8387).

# What I am unsure about
- Whether `None`/early-return results flagged "open" are reachable with a wrong program: I did not read callers outside the unit (calls.rs:3255, 4110; flow_exits.rs; flow_place.rs; execution_scalar_computations.rs).
- The definition of `crate::field_domains::WHOLE_ELEMENT_EXTENT` (sentinel `FixedRange`), `facts::PlaceSegment`/`PlaceRoot`, and `crate::expression_result_type_reference` were not read.
- Per-function call counts for the many remaining `pub` functions in places.rs and for `spelling`/`forwarded_type`/`is_runtime_value_*` in machine_parameters_and_bindings.rs were not measured.
- `contract_entailment.rs` line numbers in my greps (e.g. 84731, 88022) are as printed by Grep; that file is far larger than this unit and I did not open it.
