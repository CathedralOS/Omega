# Unit 42 notes (complete: all six files read in full)

Files covered by this unit (paths under omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/):
1. preparation_const_evaluation.rs (6545 lines)
2. preparation_generic_data.rs (4877)
3. preparation_generic_uses.rs (1843)
4. preparation_machine_equations.rs (1899)
5. preparation_type_equations.rs (3156)
6. resolution.rs (2889)

## 1. preparation_const_evaluation.rs (read in full, 1-6545)

**Purpose.** Evaluates compile-time constants for generic arguments, closed domain indices, proof facts ("where"/domain facts) and structural const values, before name resolution. It also canonicalizes const values into a string encoding (`CanonicalConstValue`) that later becomes generic-instance identity. It is one file holding nine inline modules (`mod anonymous`, `arguments`, `domains`, `fact_values`, `facts`, `remainder`, `templates`, `values`) re-exported at the bottom (6492-6525). (read)

**Shape.**
- `mod anonymous` (5-398): `AnonymousNumericValue{value: BigRational, fractional}` (23); `evaluate_anonymous_numeric_expression` (44, explicit stack, only Integer/Float/Binary + - * /); `anonymous_numeric_expression` (354, predicate "wholly anonymous, unlanded literal arithmetic"); `has_builtin_const_operator` (198) and `requires_const_operator_selection` (174) decide whether a binary node keeps builtin meaning; `authored_operator_can_bind` (315) / `parameter_binds` (340).
- `mod arguments` (399-1089): `prepare_closed_application` (438) turns `Base<Args..>` into `ClosedApplication`; `const_arguments_fit_declarations` (735); `evaluate_const_argument_expression` (784) -> `EvaluatedConst::{Concrete(i128),Symbolic(String)}` (1057); `evaluate_declared_width_operation` (982) (shifts/bitwise at declared width); `validate_declared_integer` (944).
- `mod domains` (1090-1548): `ClosedDomainFamily` (1132), `canonicalize_closed_domain_indices` (1180), `canonicalize_closed_domain_application` (1403), `domain_index_parameters` (1153), `integer_literal_value` (1115).
- `mod fact_values` (1549-1879): `ConstFactValue::{Anonymous,Integer,DeclaredInteger,Boolean}` (1563), `ConstScalarValue` (1581), `ConstScalarSpelling::{Canonical,Integer,Boolean}` (1637) with `from_bare_name` (1644), `evaluate_const_fact_binary` (1676), `evaluate_declared_integer_fact` (1773).
- `mod facts` (1880-3666): `evaluate_const_fact_expression` (1938, pub(crate)), `evaluate_const_fact_expression_at` (1961), `evaluate_const_membership_fact` (2177), `ConstSubjectBinding` (2231) / `ConstFactSubject` (2247), `prove_const_data_predicates` (2395), `evaluate_selected_const_fact` (2522), `evaluate_named_const_domain` (2668), `evaluate_selected_domain_facts` (2797), `evaluate_indexed_const_domain` (2964), `evaluate_domain_index_argument` (3115), `prove_const_leaf_domain_constraints` (3323), `const_type_requires_declaration_proof` (3413), `prove_const_literal_predicates` (3509), `evaluate_const_domain_expression` (3547).
- `mod remainder` (3667): `validate_anonymous_remainder` (3678) rejects builtin `%` between two anonymous operands.
- `mod templates` (3691-4510): `RuntimeTemplateCaptures` (3734), `capture_machine_runtime_template_names` (3745), `replace_*_const_expression_names_from` (3716, 3854), `normalize_generic_template_const_expressions` (4057), `collect_*_type_reference_positions` (4220-4447), `visit_callable_type_inputs` (4454).
- `mod values` (4511-6490): `CanonicalConstNode::{Integer,Boolean,Float,Array,Record,Variant}` (4561) with `encoding()` (4587, length-framed string via `framed` 4674) and `display()` (4625); `canonicalize_selected_const_definition[_in_application]` (4696, 4711); `selected_data` (4879), `selected_constructor` (4942), `selected_type_label` (5009), `generic_application_label` (5064), `same_carrier[_in_application]` (5232, 5284), `validate_selected_const_type` (5570), `canonicalize_const_expression[_with_admission]` (5779, 5796), `canonicalize_selected_data_const_expression_with_admission` (6143), `canonicalize_named_fields_with_admission` (6317), `closed_fixed_array_length` (5492), `ConstPredicateAdmission::{ProveSyntax,PendingDestination,CheckedDestination}` (5373), `validate_canonical_rat` (6385), `const_integer_in_envelope` (6475).
- Top level: `module_constant_selection_gate` (6532).

**Inputs/outputs.** Inputs: `&SyntaxTrees` (usually read-only; `prepare_closed_application`, `canonicalize_closed_domain_*`, template replace fns take `&mut` and rewrite type references in place via `replace_type_reference`, e.g. 527, 566, 624, 657, 1496, 1533); `const_values: &HashMap<String,i128>`, `const_definitions: &HashMap<String,ConstDefinition>`; optional `ConstantSelection` (from `crate::preparation::declaration_selection`); `GenericApplicationSubstitution`. Outputs: rewritten type-reference nodes whose names are generated atoms (`Identifier::generated(value.atom())`), `ClosedApplication`, `CanonicalConstValue` (type label + encoding + display), or `Result<Option<..>,String>` where `None` means "undecided, leave fenced".

**How it decides.**
- Anonymous vs typed arithmetic is keyed on literal landing: `literal.landing().is_none()` (373-377). An int/float literal with a suffix is not anonymous.
- Whether a binary operator keeps builtin meaning is keyed on whether ANY root item authors a machine/operator/domain-operator/trait-signature with the same `OperatorSpelling` (236-273): `has_no_authored_spelling` scans `syntax.root_items()` every call (no cache).
- `authored_operator_can_bind` (315) refines by carrier NAME STRING: `parameter_binds` (340) compares the parameter's spelled type name to `integer_type.name()` / `LandedIntegerType::from_name(name)` / `"bool"` (346-350). Binder names of the machine's own type params are excluded (298).
- Const argument spelling is recognized by bare-name parsing: `ConstScalarSpelling::from_bare_name` (1644) tries `CanonicalConstValue::from_atom`, then `name.parse::<i128>()`, then boolean spelling. So a generic argument is "a value" when its identifier TEXT parses (source spelling).
- Carrier compare for const values (`same_carrier`, 5284) uses declaration identity (`std::ptr::eq` of selected data, 5315), builtin-scalar names by string (5311-5313), fixed arrays by length+element, and `closed_argument_identity` for generics. Labels (`selected_type_label`, 5009) are metadata; comment at 5085 says identity is owned by declaration + recursive argument keys.
- Rat canonical check keyed on the label string `required == "Rat"` (4999) and record field names `"num"`, `"den"`, `"neg"`, `"pos"` and Nat cases `"Zero"`/`"Succ"`/`"prev"` (6391-6459).
- `module_domain_key` (2735) builds `module::Name` string; `visiting` recursion guard keys on (source span, that string) (2809-2812).
- `evaluate_named_const_domain` without a selection looks up domains by name text `authored` or `"{carrier}::{authored}"` (2688-2695) and requires a unique match. With selection it uses `selection.domain(...)`.
- Carry permissions are keyed on name: `authored == "Carry::Portable" || CarryPermission::from_name(authored).is_some()` (2699) -> error that const declarations cannot supply boundary-grant evidence.
- Declared integer binary ops: carriers must be the same `LandedIntegerType` unless shift (1792); mixing declared with anonymous/Integer adopts the declared type (1801-1808).

**Cases by shape.**
- Anonymous evaluation: only Integer, Float, and Binary with `Add|Subtract|Multiply|Divide` (378-393); `%`, bitwise, shifts, comparisons not anonymous. Error text 104: "operator has no anonymous const meaning".
- `evaluate_const_argument_expression` (784): accepts only integer literals, names, and binary ops among arithmetic, shifts, bitwise; others -> "expression is not a symbolic integer const expression" (929). Comment 776-783 describes the "symbolic integer subset".
- `prepare_closed_application` (438): leaves untouched (returns `Ok(None)`) when base is not a selected generic data, wrong arity, a const argument is a `Call` expression (630-635), argument not sluggable, or not fully monomorphizable. Comment 429-437 "left UNTOUCHED ... (skip, never reject)".
- Array eligibility: `closed_fixed_array_length` (5492) handles Literal, ConstParameter (substitution or selected const that must be an `Integer` literal expression with one of eight int builtin carriers, 5530-5542), Expression (only already-named parseable usize, 5557), ConstCall -> Err "array extent call requires completed constant evaluation" (5564).
- `validate_selected_const_type` (5570): accepts structural scalar carriers, f32/f64 only for Declaration purpose, nominal data (non-generic, not BoundaryOpaque, no quotient), closed lifetime-free generic applications, fixed arrays, Unit; rejects Reference/Slice/DynamicTrait/ConstExpression (5768-5775).
- `canonicalize_const_expression_with_admission` (5796): integer literal for int carriers, boolean literal for `bool`, float for `f32`/`f64`, struct literal or `Enum::Case` name for data, array literal for fixed arrays; `Unit` -> Err "unit const values do not yet have a source literal" (5055).
- `ConstSubjectBinding` (2231): Scalar / Record / Elements / Variant; comment 2227: "Aggregate carriers never bind a whole-subject scalar".
- `evaluate_const_fact_expression_at` (1961): handles Integer, Boolean, Name, Member (only on `self` with no case variant, 2091), Indexed (only `Elements`, 2102), Unary LogicalNot (BitwiseNot -> None, 2132), Binary; everything else `_ => Ok(None)` (2164).
- `evaluate_domain_index_argument` (3115): only integer or `bool` const binder types (3133).
- `carrier_satisfies_declared_bounds` (2749): builtin scalar carriers treated as `Unrestricted` + `CarryPolicy::PERMISSIVE`; named data answers from `properties`; "Any other carrier declines rather than guessing" (2747).
- `evaluate_selected_domain_facts` (2797): domains with `authored_routes` non-empty -> `Ok(None)` (2815); aliases evaluated constituent by constituent (2858-2884).
- Bitwise/shift in `evaluate_const_fact_binary` for non-declared ints: only when `left >= 0` (and `right >= 0` for bitwise) else Err "signed shifts and bitwise operators require declared-width semantics" (1744-1769).

**Same job elsewhere.**
- Two separate integer evaluators for const arguments: `evaluate_const_argument_expression` (784, i128 + symbolic) and `evaluate_const_fact_binary` (1676, ConstFactValue). Both implement add/sub/mul/div/mod/shift/bitwise with the "u64 envelope" check (`const_integer_in_envelope` 6475 vs `checked_evaluated_const` 1062 which wraps it). Shift-amount range check `< u64::BITS` appears at 863 and 1746/1753 (read).
- Two anonymous evaluators: `evaluate_anonymous_numeric_expression` (44, stack-based) and `AnonymousNumericValue::binary` (147, used from fact path), same four operators duplicated at 97-105 and 153-162.
- Type-reference "name atom" replacement (`Identifier::generated(...)` + `replace_type_reference`) occurs at five sites in `prepare_closed_application` (527, 566, 589, 624, 657) and three in `canonicalize_closed_domain_application` (1456, 1496, 1533).
- Const-definition selection + `retain_const_argument_normalization` with `ConstArgumentOrigin` appears at 551-565 and 1484-1495 (near copy).
- Cross-file: `closed_fixed_array_length` name appears in 13 files (grep count mode, `\bclosed_fixed_array_length\b`: validation, typed-trees, interpreter, provider-planning, build-time-evaluation, preparation_type_equations (2), preparation_generic_data (4)); I did not read the other sites, so whether they are the same function is "suspected" (they may call this one or define their own).

**Unused / one-caller.**
- `const_type_requires_declaration_proof` (pub(crate), 3413): grep `\bconst_type_requires_declaration_proof\b` over omega-rust: 9 hits in this file, 1 in preparation_generic_data.rs (counts include definition, recursive calls and re-export).
- `evaluate_const_membership_fact` (2177): 2 hits here (definition + re-export), 3 in preparation_generic_data.rs.
- `prove_const_literal_predicates` + `evaluate_const_fact_expression` combined grep: 4 here, 4 in preparation_generic_data.rs, 2 in constant.rs.
- `validate_const_index_type` (5353) has an unused `_visiting: &mut HashSet<String>` parameter (read).
- `canonicalize_selected_index_expression` (4743) re-exported at 6522; I did not grep its callers.
- Comment 1550 "not a proof IR" and comment 1880 "Declaration-site domain discharge shares the closed fact evaluator used by generic data" claim sharing; both are inside the same file/crate so not checked further.

**Stated limits.**
- 5648 "quotient data ... not eligible as a structural const index until quotient-backed canonical representatives land".
- 5055/6056 "unit const values do not yet have a source literal".
- 4049-4056 "A symbolic const expression cannot cross that boundary yet" (templates kept as placeholders).
- 5637 "generic data const carriers require closed template normalization".
- 5773 "const index types require finite structural values with decidable equality and one canonical form".
- 3272 `CONSTRAINED_CONST_FENCE` "constrained const declarations require declaration-site proof checking before they can publish compatibility identity".
- 5629 f32/f64 "not eligible as a const index: runtime floating/text identity is not canonical structural data".
- 5938 "floating declaration identity requires explicit NaN representation bits".

**Fail-open risks.** Most `None` returns mean "decline, keep fenced", and the callers I read at 2498-2512 (`prove_const_data_predicates`) turn `None` into an error ("requires declaration-site ... proof checking"), as does `prove_const_domain_constraint` (3309). So those are fail-closed ("traced" at those two callers).
- `prepare_closed_application` returns `Ok(None)` for many shapes (e.g. 454, 459, 469, 634, 701-713): the type reference is left unchanged and the comment (429-437) says it goes to "the existing type-check-only path (skip, never reject)". A wrong program (for instance a generic application whose const argument is a call) is not rejected here; whether the later path rejects it was not traced ("open").
- `evaluate_const_membership_fact` (2177) returns `Ok(None)` for any non-single-name value, missing parameter, or domain arguments non-empty (2186-2201): the fact stays on the record (stated at 2172-2176 "Machine-call facts stay on the concrete record"). Callers in preparation_generic_data.rs not read yet here.
- `canonicalize_closed_domain_indices` `continue`s when the family is not found (1383-1385) or a const-expression argument contains a name / needs operator selection (1504-1508): the argument is left unrewritten. "open" (not traced to a later check).
- `parameter_binds` (340): a parameter with unreadable carrier (`None`) binds anything -> `authored_operator_can_bind` true -> fact declines (the safe direction: returns None). Not accepting.
- `has_builtin_const_operator` (198): `binary_operator_spelling` None (And/Or/bitwise/shift) -> returns true (builtin) at 207-209: a user-authored overload for `&`/`|`/`<<`/`&&` is never consulted in this pre-resolution pass. Wrong program: a program that authors its own operator spelling for those would still get builtin const semantics here. Spelling enum has no entries for those operators (213-234) so this is "stated" by structure only; not traced further.
- `ConstFactValue::into_integer` (1656): Boolean -> `Ok(None)`, upstream returns Err "incompatible types".
- `const_field_projection`/`evaluate_const_fact_expression_at` for Member and Indexed return `Ok(None)` for unmatched shapes, which then fences (error at callers above).
- `canonicalize_named_fields_with_admission` (6317) rejects missing/duplicate/unknown fields: fail-closed.
- Admission modes `PendingDestination` and `CheckedDestination` (5373-5377) skip `prove_const_data_predicates` and leaf domain constraints (5311, 5853). Comment 5367-5370 says "Neither encoding mode proves a fact" and only the compile-time owner may use them. Callers using those modes are outside this file ("open").

**Coverage (file 1):** read in full 1-6545 (6545 lines).

---

## 2. preparation_generic_data.rs (read in full, 1-4877)

**Purpose.** Pre-name-resolution synthesis of closed generic data. It finds `Base<Args..>` type spellings of generic `data` templates whose arguments are closed, creates one concrete `Item::Data` per distinct (template, argument identity) pair, rewrites the spelling to the instance name, and relabels uses. It also owns "closed const argument" discovery (which type positions are const arguments and what parameter type receives them), domain-head normalization, and thin entry points (`normalize_generic_data`). The file is the parent of `const_evaluation` (includes `preparation_const_evaluation.rs` via `#[path]` at 986) and `uses` (`#[path = "preparation_generic_uses.rs"]` at 3820). (read)

**Shape.**
- `mod arguments` (40-985): `complete_argument_tuple` (79, delegates to `type_equations::complete_equation_arguments`); `type_reference_slug` (282) / `constraint_slug` (375); `closed_argument_identity` (475) returning `ClosedArgumentIdentity`; `closed_parameter_argument_identity` (432); `domain_index_telescope` (711); `selected_domain_family` (736); `indexed_domain_identity` (774); `DomainHead`/`DomainCarrier` (802, 826); `domain_application_head` (863); `normalize_domain_head_positions` (908) rewriting `Carrier::Name<idx>` generic spelling into `Constrained{base, Domain}`; `canonicalize_lifetime_bearing_type_argument` (166), `exact_synthesized_lifetime_instance` (260).
- `mod discovery` (988-1145): `GenericData` template record (1000), `PendingRewrite` (1020), `ClosedApplication` (1030), `Instantiation` (1045), `GenericDataShape` (1054), `selected_data_item[_in]` (1065, 1087), `ClosedArgumentIdentity` (1116: Builtin, Nominal(ItemHandle), RetainedNominal(SymbolHandle), Instance, Implementation, Constant(String), Array, Reference, Slice, Constrained) and `ClosedConstraintIdentity` (1134).
- `mod domain_heads` (1146-1371): `normalize` (1173) rewrites carrier-qualified domain names (`T::Marked`) on generic data fields and domain aliases into the "exposed generic-carrier family" address from `selection.generic_carrier_domain_address`.
- `mod eligibility` (1372-1733): `base_is_fully_monomorphizable` (1407), `generic_data_is_recursive` (1465), `generic_data_shape` (1564), `type_reference_handle_is_substitutable` (1602).
- `mod module_constants` (1736-1846): `lexical_integer_const_values` (1757), `reject_module_constant_selection` (1780), `module_path` (1834), `is_module_constant` (1746).
- `mod static_argument_positions` (1847-2024): `collect` (1867) finds static-call and nested-implementation const arguments as (argument, destination type, is_public) triples.
- `mod substitution` (2025-2953): `substitute_member` (2094), `substitute_type_reference` (2517), `substitute_constrained_type_reference` (2706), `decide_instance_type_equalities` (2184), `fold_impossible_case_memberships` (2276), `fold_instance_boolean_node` (2338), `variant_fact_verdict` (2400) -> `VariantFactVerdict::{Holds,Refuted,Undecided}`, `closed_name_identity` (2481), `type_reference_mentions_parameter` (2849), `substitute_cleanup_selection` (2915).
- `mod synthesis` (2954-3819): `desugar_generic_data_instances` (3011), `desugar_generic_data_instances_in` (3043: builds the `GenericData` roster at 3075-3169, runs a fixed-point loop at 3259-3397), `synthesize_instance` (3436).
- `mod work_domain` (3822-3944): `GenericDataWorkDomain::{All, Selected}` (3845), `SynthesisWork` (3861).
- Public API at file end: `ClosedConstArguments` (3994) + `closed_const_arguments` (4000), `closed_data_const_argument_expressions` (4088), `closed_array_extent_expressions` (4163), `retain_deferred_array_extent` (4216), `closed_machine_const_arguments` (4247), `closed_declaration_constant_uses` (4282), `collect_closed_const_arguments` (4328), `const_destination_requires_proof` (4508), `canonicalize_pending_initializer_payload` (4535), `canonicalize_declared_const_definition` (4587), `canonicalize_selected_declared_const_definition` (4594), `GenericDataRequest` (4641), `normalize_generic_data` (4683), `normalize_initializer_probe_data` (4699), `normalize_generic_data_in_mode` (4713), `validate_direct_const_arguments` (4771), `deduplicate_generic_warnings` (4859).

**Inputs/outputs.** Consumes `SyntaxTrees` (owned or `&mut`), optional `ConstantSelection` (from `crate::preparation::declaration_selection`), `SourceMap`, `top_level_bindings`, `retained_base: &SymbolResolvedTrees`. `synthesize_instance` clones the whole forest as a `snapshot` (3353 `syntax.clone()`) once per fixed-point round that has new instantiations, then appends new `Item::Data` (3708) and cloned attached `Item::Machine`s (3815) to the same `SyntaxTrees`. Rewrites type references in place: spelling `Base<..>` becomes `Named(instance)` while the original is kept via `retain_generic_application_origin` (3378-3382). Warnings are printed with `eprintln!` (4740, 4761) after dedup by source span (4859).

**How it decides.**
- Instance sharing is keyed on identity, not name: `prior` lookup compares `instance.template == application.template && instance.argument_identity == application.argument_identity` (3293-3299, 3447-3450). `ClosedArgumentIdentity::Nominal(ItemHandle)` / `RetainedNominal(SymbolHandle)` are declaration identities; `Constant(String)` is keyed on the argument's TEXT (decimal value string or the single-segment name text, 550-566). Builtin types keyed on `builtin_type_atom_by_symbol_name(name)` (569, a name-to-atom table).
- Synthetic instance names are derived from `format!("{}<{}>", template name, argument_names.join(", "))` with `__N` suffix on collision (3306-3318); the comment at 1111-1113 and 2-17 says names are only lookup/diagnostic metadata.
- Which templates are eligible (3076-3134) is keyed partly on source spelling: `definition.name == "IntervalSet" | "CountedQuantity"` with no module path are skipped (3086-3092). Grep `"IntervalSet"|"CountedQuantity"` across omega-rust: only 3088 here, plus `completed_storage_and_conservation.rs:6538,6541,6600,6601` (validation crate, not read).
- Other eligibility skips: `quotient.is_some()` or non-empty `conformance_bounds` (3083); any parameter of kind `Value` or `Machine` (3095-3102); attached machines that bind a type parameter not on the data (3103-3134).
- Monomorphizability is shape-based: `base_is_fully_monomorphizable` (1407) rejects recursive templates (via `generic_data_is_recursive` 1465), and every field must be `type_reference_handle_is_substitutable` (1602): Named -> true; Generic -> head not a parameter and `selected_generic_data` exists and args recursively substitutable; arrays require element and length substitutable.
- `variant_fact_verdict` (2400) decides case possibility keyed on source spelling: membership `self in Type::Case` matched by `head.as_str == type_name` and `case == variant_name`, and integer literal `0` => Refuted, other integer => Holds (2408-2414).
- `fold_impossible_case_memberships` (2276) matches `self` by name text and `[head, case]` by text (2284-2297).
- `decide_instance_type_equalities` (2184): `==`/`!=` between two single-name expressions, one of them a listed type binder, is decided by comparing `ClosedArgumentIdentity` of both (2231).
- `module_path` (1834) finds the module name by scanning every root item for an `Item::Module` whose first path member has the same `source_id`.
- `reject_module_constant_selection` (1780) matches a spelling against modules constants by string forms: local name, `full_name`, `ends_with("::{full_name}")`, and `use` imports (1795-1824).

**Cases by shape.**
- `GenericDataShape` (1054): Record / PureSum / MixedSum; `(false,false)` (no members) counts as Record (1578).
- `canonicalize_lifetime_bearing_type_argument` (166): comment 131-140 "This first exact cohort is deliberately positional ... Broader permutations remain on the unnormalized path." Handles Reference (lifetime None), Slice, Generic with lifetime arguments and no type arguments, FixedArray with Literal length; all else returned unchanged (256).
- `type_reference_slug` (282): handles Reference (lifetime None), Slice, Named, ConstExpression (Integer or single Name only), Generic with no args that is a synthesized lifetime instance, FixedArray (length must be nonzero, 344), Constrained; others `None` (366). Comment 279-280: "Unsupported or open shapes remain on the ordinary generic path."
- `closed_parameter_argument_identity` (432): Type, Const/Value, Implementation; `Machine` and `Proposition` parameters => `None` (471).
- `closed_argument_identity` Constrained arm (598-698): ArithmeticDomain, indexed Domain, named Domain/Named constraint. Without a selection it resolves a constraint name by leaf-name scan of all root domains (`rsplit("::").next() == name`, 670-673).
- `static_argument_positions::collect` (1867): only `ExpressionNode::Call` with const-expression machine arguments whose receiver is a name path (1885-1889), and `StatementNode::Call`.
- `canonicalize_closed_domain_indices`/`closed_const_arguments`: array extents only `FixedArrayLength::Expression` containing a `ConstExpression` (4054-4064), destination type hardcoded as generated `u64` named type (4049-4051, 4189).
- `substitute_domain_index_argument` (2792): bare Named, ConstExpression (copied only if it mentions a parameter), else general recursion.
- `expression_mentions_in` (2158): leaf literals (Integer/Boolean/Float/String) false; Name single-segment looked up; Binary/Unary recursed; `_ => true` (2179) "conservatively counts as a mention".
- `type_reference_mentions_parameter` (2849): `_ => true` for unhandled nodes (2906) "so the caller rejects rather than silently sharing a parameter-bearing type".

**Same job elsewhere.**
- `domain_index_telescope` (generic_data 711) and `domain_index_parameters` (const_evaluation 1153) both implement "drop a leading `Type` parameter whose name equals the target type name" (comments: 703-710 says "the one rule shared by identity, argument normalization and the structural matcher"; const_evaluation 1149-1152 repeats the rule). Grep `\bdomain_index_telescope\b|\bdomain_index_parameters\b` (count): 2 in preparation_type_equations.rs, 6 in preparation_generic_data.rs, 6 in preparation_const_evaluation.rs; no other files. Two implementations of the same rule inside this crate (read both bodies).
- `selected_data_item` (1065) and `selected_data` (const_evaluation 4879) both select a data declaration by `selection.data(..)` or by unique name text; the first returns an `ItemHandle` by pointer-equality scan of root items (1072-1074), the second returns `&DataDefinition` (read both).
- `closed_name_identity` (2481) duplicates the `Named` arm of `closed_argument_identity` (568-579) (the comment at 2475-2479 says it mirrors it).
- `type_reference_slug` (282) and `syntax_type_identity` (const_evaluation 4806) and `selected_type_label` (const_evaluation 5009) are three renderers of closed type spelling (slug / identity / label).
- Const-argument value acceptance in `closed_argument_identity` (553-567) accepts only Integer or single Name: `Constant(String)`; `const_evaluation::syntax_type_identity` (4841-4863) has the same two cases.
- Fixed-array "length expression" handling: `closed_array_extents_at` (4184) and the loop inside `closed_const_arguments` (4052-4072) are near-copies (read).
- `static_argument_positions::collect_application` and `collect_closed_const_arguments` both zip const parameter carriers with arguments and push `(argument, carrier, is_public)` triples (1979-1996 vs 4344-4354).
- Suspected duplicates outside the unit: `closed_const_arguments`-family functions (11 pub entry points) are called from `build-time-evaluation/src/const_evaluation.rs` (10 grep hits across the family), `syntax_probes.rs` (3), `const_generic_expressions.rs` (2), `const_initializers.rs` (1), plus `lib.rs` (4), `resolution.rs` (6), `constant.rs` (3), `preparation.rs` (2), `lowering.rs` (1). Grep: `\b(closed_const_arguments|closed_data_const_argument_expressions|closed_array_extent_expressions|retain_deferred_array_extent|closed_machine_const_arguments|closed_declaration_constant_uses|canonicalize_declared_const_definition|normalize_initializer_probe_data|validate_direct_const_arguments|canonicalize_pending_initializer_payload|const_destination_requires_proof)\b` count mode: 44 total across 10 files, 12 of them in this file (definitions + re-uses).

**Unused / one-caller.**
- The `pub fn` set above is used from the files listed in the grep (10 files). I did not break down per-function counts except as aggregated.
- `canonicalize_monomorphizable_argument_handles` (141) is `pub(super)`; used at const_evaluation.rs:695 (read).
- `exact_synthesized_lifetime_instance` (260) used inside this file at 210 and 325 (read).

**Stated limits.**
- 131-140 "deliberately positional ... Broader permutations remain on the unnormalized path".
- 279 "Unsupported or open shapes remain on the ordinary generic path."
- 3083 skip quotient / conformance-bound data (no comment; read).
- 4049-4056 (in const_evaluation) comparable "cannot cross that boundary yet".
- 2841-2847 conservative `true` for unknown node shapes (stated).

**Fail-open risks.**
- `type_reference_is_substitutable` default arm (1730) returns `!type_reference_mentions_parameter(...)`; and `type_reference_mentions_parameter` returns `true` for unknown shapes (2906) => fails closed (rejects). Stated at 2841.
- `expression_mentions_in` `_ => true` (2179) conservative (stated).
- `closed_argument_identity` returns `None` for unsupported shapes (699), which means "not closed": the application is left alone (`prepare_closed_application` continues) rather than rejected. Same pattern as file 1: wrong program stays on "ordinary generic path"; downstream behavior not traced ("open").
- `closed_argument_identity` for `Constant`: `Constant("N")` where N is an unresolved single-segment name text is accepted as an identity (550-566) with no check that the name is a value; two different declarations with the same spelled constant name would compare equal as `Constant("N")`. Whether a closed-shape gate earlier rules this out: the comment at 402-404 says "Classify only arguments the existing closed-shape gate has admitted ... no unresolved name is an atom." (stated; I did not trace the gate).
- `decide_instance_type_equalities` (2184) replaces the equality with a boolean when both sides are names and one is a type binder; if the other side is not a closed type it errors (2225-2230). If neither side is a listed binder it recurses; a non-name comparison is left alone (fail-open only for undecided facts that stay symbolic).
- `variant_fact_verdict` `Undecided` for unknown leaves keeps the case possible (comment 2386-2391, stated: "establishment and observation still own those terms").
- `synthesize_instance` (3577-3607): a const fact that evaluates to anything other than `Some(true)` (including `None`/undecided, or `Some(false)`!) is copied onto the instance as a retained fact (3609-3619) rather than rejected; `Some(false)` is retained too (so false facts remain as obligations on the record). So undecided or false facts do not cause an error here, they become predicate obligations ("traced" within this function: copied into `where_facts`).
- `normalize_generic_data_in_mode` (4726-4743): when `sources` is None and `retained_base` is Some or bindings exist -> error; otherwise normalizes without header selection (stated at 4644-4647 "A source-free forest has none; its instances then synthesize without header selection"). Selection-less lookups use name-text scans (e.g. 1076-1080, 665-689): in a source-free forest a same-named declaration pair resolves to "ambiguous -> None".
- `module_path` and `reject_module_constant_selection` look only at `Item::Module` entries by first-member `source_id`; a file that is not under a `module` item yields `None`, so its constants are treated as NON-module constants (1746-1748) and enter `lexical_integer_const_values` (1757). Wrong program: a constant declared in a file without a `module` header cannot be ambiguous by module, so this is by design (inferred).

**Coverage (file 2):** read in full 1-4877 (4877 lines).

---

## 3. preparation_generic_uses.rs (read in full, 1-1843)

**Purpose.** After generic data instances exist (see file 2), this module rewrites authored USES (struct literals, bare case names, `self in Type::Case` memberships, destructure patterns) from the generic template spelling (e.g. `Box { .. }`, `Maybe::Some`) to the synthesized closed instance's name (e.g. `Box<u64>::..`). It only relabels where an exact expected type is available locally (const declared type, local/param type, call parameter type, return type). It also holds the shared expression/statement walkers (`collect_expression_handles`, `collect_statement_expression_handles`) and the "concrete machine" inventory. (read)

**Shape.** Child modules under `preparation_generic_uses.rs` (included from file 2 via `#[path]`, 3820):
- `alias_constructors` (3-91): `AliasConstructor{template, case, application, carrier}` (14), `select` (21).
- `assignments` (92-213): `relabel_closed_data_uses_in_constants` (110), `relabel_closed_data_uses_in_exact_assignments` (164).
- `calls` (214-582): `relabel_closed_data_uses_in_exact_calls_and_returns` (239), `exact_statement_receiver_owner` (439), `exact_expression_receiver_owner` (451), `call_context_parameter_types_agree` (474), `call_context_types_agree` (486).
- `constructors` (583-950): `expected_instance` (605), `selected_constructor` (627), `selected_case_value` (652), `constructor_path_name` (670), `constructor_carrier_span` (686), `closed_constructor_carrier` (701), `relabel_data_literal_for_expected_type` (726, the recursive core).
- `expressions` (951-1432): `ExpressionMarks` trait (963), `statement_type_inputs` (990), `visit_expression_type_inputs` (1015), `visit_static_argument_type_inputs` (1052), `ConcreteExpressions` (1067), `selected_machine_expression_handles` (1092), `selected_machine_state_handles` (1106), `concrete_machine_declarations` (1122), `collect_statement_expression_handles` (1183), `collect_expression_handles` (1266), `ConstructorFrontier{parameters, prior_statements}` (1353), `statement_call_inputs` (1382).
- `patterns` (1433-1782): `relabel_closed_sum_memberships_from_local_types` (1457), `membership_subject_type` (1616), `named_type_name` (1633), `closed_sum_path` (1648), `subject_annotation` (1669), `relabel_pattern_head` (1731).
- top level: `concrete_state_local_types` (1811) returns (State, name->TypeReferenceHandle map, statements) for every concrete state.

**Inputs/outputs.** Inputs: `&mut SyntaxTrees`, the synthesized `&[Instantiation]` roster, optional `ConstantSelection`, and the declaration cohort `&[ItemHandle]`. Outputs: expressions replaced in place (`replace_expression`) — `ExpressionNode::Name`/`StructLiteral`/`Membership` retargeted to the instance's name (e.g. 860-867, 876-879, 897-899, 1608-1611, 1777-1779); destructure patterns get `application_type` set (1513, 1552). No return value (all `()`/handles).

**How it decides.**
- Which instance a literal belongs to: `expected_instance` (605) computes `closed_argument_identity` of the expected type and finds the single `Instantiation` with equal `(template, argument_identity)` (620-624) — identity, not name. If zero or two instances match -> `None`.
- Which template a constructor name selects: `selected_constructor` (627) uses `selection.constructor(..)` and finds the owner ItemHandle by pointer-equality scan (634-635); without selection it falls back to unique name text, then `rsplit_once("::")` carrier/case with a check that the carrier has a variant named `case` (641-649) — name-text keyed.
- A relabel proceeds only if `owner == instance.template` (833-835): "a matching shape or name cannot substitute an unrelated template" (comment 106-107).
- Call contexts: free machines are keyed by `machine.name` TEXT in `signatures: HashMap<String, Option<Vec<TypeReferenceHandle>>>` (245), attached methods keyed by `(attached-data name text, method tail after last "::")` (246-247, 267-278). If two same-name machines disagree on parameter types (via `call_context_types_agree` 486, textual comparison of names, access, lifetime names, array lengths, generics), the entry becomes `None` and no relabel occurs (285-287, 297-299). Comment 232-238.
- Receiver owner for attached calls: only a single-segment local/param name whose declared type is a plain `Named` (or Constrained over Named) (`named_type_name` 1633, `exact_*_receiver_owner` 439, 451). Comment 236-237: "Computed, chained, and dynamic receiver selection remains resolver-owned and fails closed here."
- Pattern relabel: `subject_annotation` (1669) finds the subject's declared type through a name in `local_types`, or through a `DispatchSubject` local's initializer, `SavedSubject`, `Borrow`, or `StructLiteral.application_type`; recursion guarded by `active` stack (1676-1679).
- `ConstructorFrontier::captures_name` (1366): a `Name` is "captured" (left alone) if it equals the name of a prior `LocalData` statement or a state parameter — keyed on name text (1367-1370). Comment 1348-1352: "A captured Name remains authored syntax for the ordinary resolver".
- `concrete_machine_declarations` (1122): a machine is "concrete" when it has no type parameters and its `attached_data` name text is not in the set of generic data names (1126-1141, keyed on data name TEXT). Visits root machines, inline conformance (with no type params) members, and nested machines (1143-1179). Only machines owned by the supplied `declarations` (or enclosing owned) are returned (1150, 1155).
- `alias_constructors::select` (21): requires the expected type to have exactly one `type_alias_applications` origin matching the alias declaration (45-52), and for a case, exactly one variant with that name (56-70); payload-free constraints (68-73).

**Cases by shape.**
- Statement kinds relabeled in calls pass: `StatementNode::Expression` only as the FINAL statement (373), `Call`, `Transition` value targets (417-432). Others (`_ => {}` 433) are skipped.
- Assignment pass (164): comment 159-163 "Relabel a bare generic literal only when that type is available directly from a local or parameter; computed targets remain fail-closed." Target must be `Name` with one segment (192-197).
- Constant pass (110): constants only (Item::Const) and their retained `normalization.authored_expression`.
- `relabel_data_literal_for_expected_type` (726): handles `Match` (recurse into arm values, 734-747), `Name` not captured, `ArrayLiteral` against `FixedArray`, `StructLiteral`, payload-free `Name` case values; other expression kinds fall through to `authored_name == None -> return` (800-802).
- `call_context_types_agree` (486): Named, Unit, Reference, Slice, FixedArray, Generic, DynamicTrait; `Constrained` vs `Constrained` and `ConstExpression` pairs return false (575-578), everything else false (579). Comment 467-472: "Unsupported or expression-bearing details fail closed".
- `collect_statement_expression_handles` (1183) enumerates every `StatementNode` variant (Assembly, RootBinding, AssemblyFact, Assignment, Call, ProofOutputBindingStatement, Expression, LocalData, Transition); `collect_expression_handles` (1266) enumerates every `ExpressionNode` explicitly (no `_` arm, 1337-1344 lists the leaf variants).
- `statement_call_inputs` (1382): retains a `Call` expression or an enclosing `Match` with `capture` valid that contains a call (1392-1414); comment 1374-1381.

**Same job elsewhere.**
- `named_type_name` (1633) and `membership_subject_type` (1616) strip Constrained/Reference to a Named; the same peel loop appears in `closed_fixed_array_length` carrier peel (const_evaluation 5524-5529), `type_equations`/`generic_data` `named_carrier` (generic_data 1362-1370). Several "peel Constrained to the base" loops exist (read).
- Signature-agreement `call_context_types_agree` (486) is a structural type equality by name text; `same_carrier` (const_evaluation 5284) and `closed_argument_identity` (generic_data 475) are other type equality notions in the same crate (read all three; different keys: text vs identity).
- `collect_expression_handles` is a hand-written expression-child walker; the same walker shape appears in `const_evaluation::templates::collect_*` (const_evaluation 3916-4018 uses it) and `generic_data::substitution::expression_mentions_in` (2158) walks only Name/Binary/Unary. Whether other crates have their own full ExpressionNode walkers is "suspected" (not read).
- Re-export count: `concrete_machine_declarations` / `concrete_machine_expression_handles` grep count: 4 in this file, 3 in preparation_generic_data.rs, 2 in preparation_const_evaluation.rs; no other files.

**Unused / one-caller.**
- Grep `\b(statement_call_inputs|statement_type_inputs)\b` count: const_evaluation 1, preparation.rs 1, lowering.rs 4, this file 3, generic_data 1 (10 total). `statement_call_inputs` therefore has outside users in `lowering.rs`/`preparation.rs` (not read).
- Grep `\bcall_context_(parameter_)?types_agree\b|\bconstructor_prefix_is_value\b` count: symbols.rs 3, preparation.rs 2, this file 12. The `call_context_*` fns are `pub(in crate::preparation::generic_data)`; the matches in `symbols.rs`/`preparation.rs` are likely `constructor_prefix_is_value` (method on selection); not verified.

**Stated limits.**
- 236-238 "Computed, chained, and dynamic receiver selection remains resolver-owned and fails closed here."
- 159-163 "computed targets remain fail-closed".
- 467-472 "Unsupported or expression-bearing details fail closed rather than broadening contextual inference."
- 1-1 "Relabel authored uses only from the exact expected-type context available here."

**Fail-open risks.** None seen of the accepting kind: every unresolved lookup returns early without rewriting (`return`/`None`), leaving the authored (template-spelled) literal for the "ordinary resolver" (comment 1348-1352; "stated"). The consequence of a missed relabel is that a generic-spelled literal stays unrewritten; whether the later resolver rejects or accepts it was not traced ("open").
- One name-text hazard: `signatures` is keyed by machine name TEXT across all modules (245, 280): two same-named free machines in different modules with the same parameter type spelling would agree and be merged (`call_context_types_agree` compares names only), so the expected type for a call to either is the first's parameter handle. Wrong program: a call to the second machine gets relabeled using the first machine's parameter types; both have equal-spelled types by the agree check, so the relabel result would be the same spelling. Resolution of equal spelling to different declarations (module-qualified) is "open".
- `concrete_machine_declarations` treats a machine as generic-attached by `attached_data` NAME TEXT in the set of generic data names (1126-1141): a non-generic data in another module with the same name as a generic one causes its attached machines to be excluded ("open").

**Coverage (file 3):** read in full 1-1843 (1843 lines).

---

## 4. preparation_machine_equations.rs (read in full, 1-1899)

**Purpose.** For a call to a generic machine whose `requires` facts contain type equations (e.g. `Kind == FixedVec<i32, N>`), this pass solves the omitted static (type/const) arguments after provisional symbol resolution and writes the completed argument list back into the authored call syntax. It reads declared types of the call's actual arguments to feed the solver, and re-runs generic data synthesis for any new type roots the completion minted. (read)

**Shape.**
- `SourceLinks` (981): `declared_types: Vec<(DeclaredTypeOrigin, TypeReferenceHandle)>`, `declarations: Vec<(usize, Machine)>`, `data_declarations: Vec<(ItemHandle, usize)>`, `expressions: Vec<(syntax ExpressionHandle, resolved ExpressionHandle)>`, `statements: Vec<(StatementHandle, Handle<Statement>)>`. `same_syntax` (990).
- `mod argument_types` (37-662): `DeclaredTypeOrigin::{Parameter,Result,Field,Payload,Constant}` (268), `closed_type` (309), `declared_type` (382), `expression_types` (435; handles Name, Borrow, Member, Call, Match, `_ => None`), `constraints` (544), `call_constraints` (587), `statement_constraints` (640), `CallTypeObservations` (430), inner `mod projections` with `member_types` (93) and `closed_nominal_owner` (237).
- `mod completion` (663-962): `complete_arguments` (671), the core: splits explicit conformance evidence off the tail (695-701), computes equation positions (type/const/value parameters only, 761-778), builds argument references (819-836), calls `type_equations::complete_equation_arguments_with_types` (850) and `materialize_completed_type` (870), then appends the recovered arguments (905-958).
- Top level: `required` (1012), `machine_type_equations` (1027), `validate_declarations` (1039), `selected_machine` (1085), `complete` (1098, the pass driver), `completes_call` (1261), `requirement_equations` (1293), `supplies_complete_tuple` (1304), `reject_machine_arguments` (1313), `require_closed_selected_arguments` (1337), `ArgumentClosure::{Closed,TypeBinder,ConstBinder}` (1361), `BinderClosures` (1381), `classify_selected_arguments` (1412), `classify_type_reference` (1443), `binder_closure` (1549), `complete_authored_arguments` (1572), `inferred_const_argument` (1614), `supplied_const_argument_reference` (1668), `scalar_const_argument` (1699), `scalar_const_argument_reference` (1732), `application_argument_reference` (1757).

**Inputs/outputs.** `complete` (1098) takes `&mut SyntaxTrees`, `&SymbolResolvedTrees` (provisional), `&SourceLinks`, `&ConstantSelection`. For each resolved `Call` expression / `Statement::Call` whose selected machine declares equations, it clones the authored call, completes `machine_arguments`, and `replace_expression` / `replace_statement` at the authored handle (1159-1162, 1214-1216). It then runs `desugar_generic_data_instances_in(.. Selected{type_roots: completed_type_roots, declarations: &[]})` (1223-1231) and errors if any completed type position is still a `Generic` node (1232-1241). Warnings printed with `eprintln!` (889-891, 1242-1244).

**How it decides.**
- Which calls are completed: `completes_call` (1261): the selected machine must have equations (`has_structural_type_equations` on the resolved machine, or `requires_argument_recovery` on the source machine's equations, 1269-1278). Then by `ArgumentClosure`: complete tuple + const/value binder => error "open caller binder" (1283-1285); complete tuple forwarding only open TYPE binders => skipped (later predicate check; comment 1248-1260); otherwise complete.
- Binder closedness is keyed on symbol kind: `binder_closure` (1549): `SymbolKind::TypeParameter` looks up `BinderClosures` (data type params only: Type/Implementation => TypeBinder, other => ConstBinder; unknown => ConstBinder, 1559); `Parameter | MachineParameter | ConformanceParameter` => ConstBinder; all else Closed (1564). `BinderClosures::build` (1386) scans only `trees.tables.declarations.data_type_parameters` (1388) — i.e., machine type parameters are not in this table; a `TypeParameter` symbol not found there is treated as ConstBinder (1559).
- Depth limit: `classify_type_reference` returns `ConstBinder` at depth >= 128 (1450-1452).
- Which parameter positions participate in equations: only `Type`, `Const`, `Value` kinds (761-768); other kinds (machine/implementation/proposition) must be supplied explicitly (770-776) and error if mentioned in the signature or equations (738-759).
- Member projection owner (`member_types` 93): data field owner found via `sources.data_declarations` ordinal into `trees.data_definitions`, else `closed_nominal_owner`; field found by `find_child_by_name_and_kind(owner, member.member.as_str(), SymbolKind::Field)` (153-157) — keyed on field NAME text within the owner symbol.
- `declared_type` (382): finds the source type handle for a resolved symbol by scanning `sources.declared_types` linearly (392-402) and local statements (404-418); only if `closed_type(...)` holds. Linear scans each call (`O(n)` per lookup).
- `scalar_const_argument` (1699): a const argument must be a single-path name selected via `selection.select`; Boolean keyword/atom recognized by `normalized_boolean_argument(name)` (1716).

**Cases by shape.**
- `expression_types` (435) supports only `Name`, `Borrow`, `Member`, `Call`, `Match` expressions; `_ => None` (540). Comment 44-48: "An initializer, literal element, flow predicate or private postcondition supplies no type."
- `application_argument_reference` (1757): `Const` parameter must have a `Named` carrier that is a builtin integer (via `const_binder_envelope`) or `bool`, or a structural const (Array/Record/Variant) when the declared carrier matches (1791-1810); `Type` parameter, lifetime-free generic application with a complete tuple (1867-1877); other parameter kinds `_ => Err("requires type and integer const constructor parameters")` (1895).
- `validate_declarations` (1039): comment-by-error 1054: "machine structural equations currently require explicit calls; operator and conformance supplies retain undisclosed equations" — machines with `spelling` or `satisfies` AND argument-recovering equations are rejected.
- `complete` rejects: conformance realization rows whose realization machine/state has structural equations (1105-1119 "conformance realization retains an undisclosed machine structural equation"); a `Name` expression selecting such a machine without calling it (1164-1171); a static argument that is itself such a machine (1318-1323).

**Same job elsewhere.**
- `closed_type` (309, over resolved `TypeReference`) vs `classify_type_reference` (1443, over resolved `TypeReference`) vs generic_data `closed_argument_identity` (generic_data 475, over syntax `TypeReferenceNode`) — three "is this type closed" walks. Both of the first two are in this file: `closed_type` returns a bool using `binder(symbol)` kinds `TypeParameter | Parameter | MachineParameter | ConformanceParameter`; `classify_type_reference` returns a three-valued closure using nearly the same symbol-kind test (`binder_closure` 1549). Both walk Alias/Named/DynamicTrait/Reference/Slice/FixedArray/Constrained/Generic/ConstExpression/Unit.
- `scalar_const_argument_reference` (1732) and `type_equations::normalized_boolean_argument` plus `generic_data::synthesis` const-argument Boolean handling (generic_data 3529-3532) both convert a boolean const argument.
- `machine_type_equations` (1027) / `requirement_equations` (1293) both call `classify_*_equations` over `requirement_facts(contracts)`; one uses `classify_machine_equations` (with lifetimes), the other `classify_type_equations`.
- Outside the unit: `machine_equations::` has 12 hits (resolution.rs 7, lowering_machine.rs 2, lowering.rs 3); `machine_type_equations|SourceLinks|DeclaredTypeOrigin` 37 hits across resolution.rs (5), this file (25), lowering_machine.rs (2), lowering.rs (5). (grep count mode.)

**Unused / one-caller.**
- `required` (1012), `validate_declarations` (1039), `complete` (1098) are `pub(crate)`; I did not count per-function. The `machine_equations::` path has 12 users in 3 files (above).
- `complete_arguments` is called at 507 (via `super::complete_arguments`) and at 1583 (read).

**Stated limits.**
- 10 "Open applications remain obligations and reject rather than entering later inference unchecked."
- 1054 error text: "currently require explicit calls".
- 1114-1116 "conformance realization retains an undisclosed machine structural equation".
- 1168-1170 "noncall selection retains an unsolved equation".
- 755 "machine structural inference cannot replace non-type binder ... with a global declaration".

**Fail-open risks.**
- `completes_call` returns `Ok(false)` ("skip completion") when the machine declares no equations (1279-1281): fine (nothing to recover).
- `completes_call` returns `Ok(false)` when closure == TypeBinder and the tuple is complete (1286): the call is left unchanged and "stays with the later type-predicate check" (stated at 1248-1260). A wrong program whose forwarded open type binder violates the equation is checked later (not traced; "stated").
- `BinderClosures::get` returns `None` for an unknown TypeParameter and `binder_closure` maps that to `ConstBinder` (1559): fails closed (more restrictive).
- `classify_type_reference` `depth >= 128` => ConstBinder (1451): fails closed.
- `declared_type`/`expression_types` return `None` when no closed declared type is found: the call then has fewer ordinary type observations; the comment (423-428) says "Early type observations are evidence for inference, not a second call admission boundary ... an independently determined consumer may proceed to ordinary checking of the unchanged call." (stated: missing evidence does not reject here).
- `complete` loops `continue` when `selected_machine` is `None` (1125-1127, 1181-1183): calls to non-machine targets are untouched.
- Linear/no-op `return Ok(())` paths: none beyond above. Overall none seen that accepts a wrong program without a stated later check.

**Coverage (file 4):** read in full 1-1899 (1899 lines).

---

## 5. preparation_type_equations.rs (read in full, 1-3156)

**Purpose.** Solves declaration-level `where`/`requires` equations that equate a type binder with a type structure (`Backing == [Element; Count]`, `Kind == FixedVec<i32, 4>`, `T == u64`) so omitted generic arguments can be recovered from supplied ones, and so complete tuples are verified. It decomposes types (arrays, slices, references, nominal generic applications, declared-domain applications) and binds binders; it is used by data synthesis (file 2: `complete_argument_tuple`, `classify_type_equations` at generic_data 3159) and machine call completion (file 4). (read)

**Shape.**
- `mod type_structure` (56-1698): inner `mod applications` (95-1081) with `Application` (134), `DomainApplication` (146), `Solver::application` (160), `complete_application` (328), `domain_head` (371), `domain_constraint_application` (381), `match_application` (424), `construct_application` (520), `application_integer` (618), `application_constant` (711), `application_boolean` (811), `require_lifetime_free_argument` (898), `require_equal_application_structure` (988). Then `collect_binder_mentions` (1084), and `impl Solver` at 1183 with `type_operand_reference` (1214), `parameter_position` (1265), `match_type_structure` (1281), `require_closed_pattern` (1443), `require_equal_type_structure` (1464), `construct_type_structure` (1506), `closed_array_length` (1614).
- Top level: `TypeEquation{fact_offset, definition, shape}` (1718) with `requires_argument_recovery` (1730), `structural_definition` (1746), `defines_structure` (1761), `mentions_parameter` (1785), `validate_kind` (1802), `validate_names` (1817), `reject_unclosed_names` (1846); `template_type_equation_offsets` (1876); `validate_closed_name_type` (1934); `validate_materialized_type_equations` (1974); `EquationShape::{Boolean, Structural, KindMismatch}` (2000); `Structure::{TypeOperand, TypeReference, Name}` (2036); `Side` (2077); `classify_machine_equations` (2105); `classify_type_equations` (2123); `classify_side` (2224); `equation_shape` (2281); `type_operand_kind_error` (2324); `collect_expression_binder_mentions` (2353); `EquationTemplate` (2453); `complete_equation_arguments` (2464); `complete_equation_arguments_with_types` (2495); `materialize_completed_type` (2564); `Binding::{Type, NamedType, Integer, Boolean, Constant}` (2589); `Solver` (2612) with `solve` (2679), `apply` (2771), `predicate_value` (2868), `recover_binder` (2949), `match_structure` (2991), `match_name` (3020), `bind_integer` (3067); `normalized_boolean_argument` (3139); `const_binder_envelope` (3153).

**Inputs/outputs.** Input: `&mut SyntaxTrees`, an `EquationTemplate` (kind "data" or "machine", name, parameter span, parameter names, const parameter types, classified equations), supplied argument handles, optional `ordinary_types` formal/actual pairs, `const_values`, `ConstantSelection`. Output: a complete argument tuple `Vec<TypeReferenceHandle>`; recovered arguments are new type-reference nodes (`Named(Identifier::generated(value))` at 2540, 2543) inserted into the syntax forest. `materialize_completed_type` builds a closed type from a result pattern. No mutation of existing nodes other than appending.

**How it decides.**
- Equation classification keyed on SYNTAX SHAPE: only `ProofFact::Expression` with `==`/`!=` between sides (2170-2178), `&&`/`||`/`!` become Boolean shapes (2141-2158). Each side classified by `classify_side` (2224): `Borrow`/`ArrayLiteral` => `AmbiguousTypeOperand`; `TypeExpression` => `TypeStructure`; single-segment `Name` matching a parameter NAME => `TypeBinder` or `ValueBinder` by kind; other `Name` => `ClosedName` (full path); all else `Other("a value expression")`. A `!=` wraps the equation in `Not` (2207-2216).
- Binder lookup is by NAME text: `binder_index` (2087) and `parameter_position` (1265) compare `parameter.name.as_str == name`; `collect_binder_mentions` (1084) matches `name_mention(..)` on type names, lifetime names, const-param array lengths, domain constraint NAMES (1141-1146) and trait names (1172).
- Nominal head joins by declaration identity: `application` (160) gets `closed_name_identity(..)` and requires `ClosedArgumentIdentity::Nominal(declaration)` whose item is data (194-202), else tries a domain head. `match_application` compares `pattern.declaration != actual.declaration` (432).
- Equality of closed types uses `closed_argument_identity` identity (1473-1476); if identities are unavailable but either side has "application shape" it falls to `require_equal_application_structure` (1480-1482), which compares declaration + each index argument.
- Integers: `application_integer` accepts a decimal name text via `name.parse::<i128>()` (650) or a selected const (653-659); bounds by `const_binder_envelope(type_name)` from `LandedIntegerType::from_name` (3153-3155). Booleans via `normalized_boolean_argument` (3139): `boolean_literal_spelling(text)` or canonical atom of type "bool".
- Which constructor parameter kinds are allowed: only `Type` and `Const` (343-353); other kinds => "requires type and scalar const constructor parameters". `Const` parameter dispatch (456-473): `boolean_constructor_parameter` (803: carrier identity is `Builtin(Bool)`), `integer_constructor_parameter` (698: carrier name has a `const_binder_envelope`), else structural constant.
- Solver loop (2679-2695) iterates equations to a fixed point; then any unbound binder => error "cannot determine ... from its requires equations" (2703); a cycle check `defined_through_cycle` (2750) distinguishes the message.
- Reference lifetimes: `require_lifetime_free_argument` (898) rejects a `Reference` with `lifetime: Some(_)` in a recovered element "until lexical lifetime identity is retained" (912) and rejects data with lifetime params or `generic_instance` (938-948). Pattern references may name only declared invocation lifetimes (`require_invocation_lifetime` 1189).

**Cases by shape.**
- `match_type_structure` (1281) handles: Reference (actual must have `lifetime: None`, 1296-1306), Slice, Named (binder or closed), FixedArray (actual length via `closed_fixed_array_length`; expected length ConstParameter binder binds integer), Generic, Constrained with exactly one `Domain` constraint (1422-1429); `_ =>` requires closed pattern then equality (1430-1433).
- `type_operand_reference` (1214): `TypeExpression`, `Name`, `Borrow`, single-element `ArrayLiteral` (slice); else error "cannot use a value as a reference or slice element type" (1258).
- `domain_constraint_application` (381) and `application` Constrained arm (254-266): only ONE `Domain` constraint (`let [TypeConstraintNode::Domain(constraint)] = constraints` 259).
- `construct_type_structure` (1506) handles Reference, Slice, Named binder, FixedArray, Generic, Constrained-domain; otherwise requires closed pattern.
- `classify_type_equations` (2123) accepts only equalities where at least one side is a type binder or type structure (`_ => return None` 2200); a pure value equality like `Capacity > 0` stays ordinary (comment 2098-2101).
- `complete_application` (328): arity must equal declared telescope; kinds restricted to Type/Const (343).
- `type_reference_name`-based Named recognition (797-801): only `TypeReferenceNode::Named`.

**Same job elsewhere.**
- `collect_binder_mentions` (1084, over type references) + `collect_expression_binder_mentions` (2353) + `collect_static_argument_binder_mentions` (2420): "which declared binders does this tree mention" walkers; similar to `type_reference_mentions_parameter` (generic_data 2849) (returns bool, `_ => true`) and `expression_mentions_in` (generic_data 2158). Three/four near-duplicate "mentions parameter" traversals across files 2 and 5 (read all).
- Boolean const argument normalization: `normalized_boolean_argument` (3139) vs `ConstScalarSpelling::from_bare_name` Boolean arm (const_evaluation 1651) vs `boolean_literal_spelling` direct uses (const_evaluation 1651, here 3140): the first wraps the second plus canonical atom of type bool.
- Integer envelope check: `const_binder_envelope` (3153, BigInt bounds) vs `validate_syntax_integer_range` (const_evaluation 6370, i128 bounds) vs `validate_declared_integer` (const_evaluation 944): three range checks against `LandedIntegerType::bounds()`.
- Array length closure: `closed_array_length` (1614) duplicates part of `closed_fixed_array_length` (const_evaluation 5492): same ConstParameter handling (selected const via `canonicalize_selected_declared_const_definition`, integer only), plus binder lookup, plus the selection-less lexical `const_values` branch (1669-1683).
- `domain_index_telescope` use at 402; `domain_application_head` at 372: shared with generic_data (see file 2 notes).
- Grep `\b(normalized_boolean_argument|const_binder_envelope|classify_machine_equations|validate_materialized_type_equations|template_type_equation_offsets|type_reference_parameter_mentions)\b` count: see list in "Unused" below.

**Unused / one-caller.** (counts from the grep above, filled in below at end of notes).

**Stated limits.**
- 912 "requires anonymous reference lifetimes until lexical lifetime identity is retained".
- 894-897 "ClosedArgumentIdentity currently omits application lifetimes" (the identity type has no lifetime component: generic_data 1116-1131 read, confirms).
- 983-986 "Domain applications have no closed argument identity yet" (comparison by declaration + index arguments instead).
- 2001-2005 "Alternatives constrain an already determined tuple; they never select one possible type for an omitted argument".
- 1054 (file 4) machines with spelling/conformances and recovering equations are rejected.

**Fail-open risks.**
- `validate_materialized_type_equations` (1974): returns `Ok(())` when the reference is not `Generic`, when the data item is not selected, or when the selected item is not data (1979-1989). Wrong program: a generic spelling whose base cannot be selected passes this check. "open" (callers in lowering/resolution outside the unit; not traced). Counts below.
- `apply`/`solve` reject undecidable equations (2707-2728) and unknown predicates (`Deferred` leftover -> error "cannot be decided from the supplied arguments"): fail-closed.
- `unreachable!` sites (2550, 2720, 2911, 2982, 3035, 3047, 3091) assume solving invariants; they panic rather than accept.
- `complete_equation_arguments_with_types` (2514-2525): an `ordinary_types` pair is matched only if its formal mentions an omitted binder (`index >= supplied.len()`); pairs mentioning only supplied binders are NOT checked here (comment 2485-2494: "only patterns mentioning an omitted binder contribute inference"; later checks own compatibility). "stated".
- `require_equal_type_structure` (1464): neither identity nor application shape => error (1483-1486): fail-closed.
- `type_reference_name` accepts `Named` only; non-Named values in const positions error (e.g. 626).

**Coverage (file 5):** read in full 1-3156 (3156 lines).

**File 5 unused / one-caller counts (grep, count mode).** `\b(normalized_boolean_argument|const_binder_envelope|classify_machine_equations|validate_materialized_type_equations|template_type_equation_offsets|type_reference_parameter_mentions)\b`: preparation_type_equations.rs 13, preparation_machine_equations.rs 8, preparation_generic_data.rs 3, preparation_const_evaluation.rs 1, lowering.rs 2 (27 total, 5 files). `validate_materialized_type_equations` therefore has at most the `lowering.rs`/generic_data re-export users (not separated; lowering.rs not read).

---

## 6. resolution.rs (read in full, 1-2889)

**Purpose.** The entry point of symbol resolution (syntax trees -> symbol-resolved trees). It exposes the `resolve*` family of operations, each running the same route (prepare forest, lower items, assign symbols, settle selections) for a different product, and holds the carriers that let later phases continue from a retained base (deferred frontier, extension, seeded trees, const-initializer selection). It also holds the `Lowerer` working-state struct. (read)

**Shape.**
- Header comment 1-16 names the route: `resolve`, `begin`, `drive`, plus `resolve_const_argument_selection`, `resolve_numeric_probe`, `prepare_const_initializer_selection`, `resolve_extension`.
- `mod body_completion` (18-607): `complete_initializer_receipt_dependencies` (55), `DeferredResolvedFrontier` (102; fields: `syntax: Arc<SyntaxTrees>`, `trees: Arc<SymbolResolvedTrees>`, `pending: Vec<PendingMachineBody>`, `pending_type_inputs`, `supplied_operand_owners`, `declaration_inputs`, `selection: ConstantSelection<'static>`, `sources`, `bindings`, `hoist_counter`, `const_resolution_mode`, `structural_type_equations_pending`, `equation_sources: Option<SourceLinks>`), hand-written `PartialEq` (126), `into_extension` (176), `activate_sources` (210), `with_prepared_allocations` (236), `body_demand` (290), `complete_machine_bodies` (311, transactional; loops provisional lowering + `machine_equations::complete` until the root item count stops growing, 351-372), `seed_body_lowerer` (404), `append_generated_declarations` (418), `lower_selected_bodies` (435), `ResolvedBodyCompletion` (452), `RebasedResolvedBodyCompletion` (460), one `#[cfg(test)]` test (529).
- `mod continuations` (608-1103): `ConstInitializerSelection{frontier}` (635) with `retained_constant` (657), `retained_constant_custody` (679), `initializer_expression_dependencies` (698), `pending_leaf_dependencies` (719), `initializer_dependencies` (747), `selected_initializer_dependencies` (762), `initializer_authored_dependencies` (774), `declaration_dependencies` (782), `validate_initializer_leaves` (822), `pending_leaves` (849), `canonicalize_initializer_payload` (866), `initializer_requires_destination_proof` (878); `ConstInitializerPayload` (643); `SeededSymbolResolvedTrees` (888); `RebasedSeededSymbolResolvedTrees` (900); three `#[cfg(test)]` tests (940-1102).
- `pub(crate) mod lowerer` (1104-1749): `Lowerer` struct (1214-1367) with ~55 fields (pending sidecars: machine bodies, type inputs, const declarations/selections/argument slots, authored expressions, service reaches, outcome contracts, plus per-state scratch: `hoist_counter`, `reference_struct_parameters`, `current_state_parameter_names`, `current_machine_is_boundary`, `current_state_parameters`, `match_subject_temps`, etc.), `ConstResolutionMode::{Complete, ArgumentSelection, InitializerSelection}` (1208), `RootWatermarks` (1369), `BaseSeed` (1403), `Lowerer::new` (1409), `seed_resolved_base` (1471), `into_trees` (1640), `pending_service_reaches` (1667).
- `pub(crate) mod operand_owners` (1750-1857): `DeclarationInput::{Conformance(usize), Trait(usize)}` (1764), `retain` (1781), `bindings` (1812), `bind_program` (1837).
- `pub(crate) mod type_inputs` (1858-1962): `LoweredTypeInputs` (1873), `TypeInputScope` (1884), `bind` (1914).
- Public API (1964-2665): `ResolutionRequest` (2014), `ExtensionRequest` (2043), `resolve` (2056), `resolve_deferred_extension` (2065), `resolve_deferred` (2091), `DeferredResolvedExtension` (2129), `resolve_deferred_numeric_probe` (2163), `index_deferred` (2172), `index_declarations` (2211), `ConstArgumentSelection` (2227), `resolve_const_argument_selection` (2273), `resolve_numeric_probe` (2297), `resolve_deferred_initializer_probe` (2310), `prepare_const_initializer_selection` (2328), `prepare_const_qualification_selection` (2343), `finish_const_initializer_selection` (2400), `resolve_extension` (2444), `begin_complete` (2474), `begin` (2499), `drive` (2534), `drive_recorded` (2541), `finish_resolution` (2568), `settle_resolution` (2611), `finish_resolved_tables` (2636), `resolve_call_result_selection` (2657).
- Tests: `call_result_selection_tests` (2667-2812, 4 tests), `primitive_family_binding_tests` (2813-2889, 3 tests).

**Inputs/outputs.** Inputs: `ResolutionRequest{syntax (by value), sources: Option<Arc<SourceMap>>, top_level_bindings, retained_base: Option<Arc<SymbolResolvedTrees>>}`. Outputs vary per entry: `SymbolResolvedTrees` (`resolve`, `resolve_numeric_probe`, `resolve_call_result_selection`), `DeferredResolvedFrontier`, `ConstArgumentSelection`, `ConstInitializerSelection`, `SeededSymbolResolvedTrees`. `begin` (2499) calls `preparation::prepare(...)` (the generic data / const / equation preparation: files 1-5 sit under it), then `validate_declarations` (file 4), then builds a `Lowerer`; `drive_recorded` (2541) runs `lower_items`, `lower_mathematical_definitions`, `select_operator_homes`, `symbols::assign`, `finish_resolution`.

**How it decides.**
- Whether machine-equation completion runs: `required(&syntax, base)` (file 4, 1012): true if any root machine has `requirement_equations` or the base has a machine with `has_structural_type_equations`. When required, the code lowers a provisional copy in `ConstResolutionMode::ArgumentSelection`, calls `complete`, and repeats until the root item count stops changing (2099-2121 in `resolve_deferred`; `begin_complete` 2479-2490 does one provisional drive; `complete_machine_bodies` 351-372 loops).
- Which machines `finish_resolution` treats as new: not present in the predecessor (`previous.machine(machine.symbol).is_none()`) or in the completed set (2588-2595), keyed on symbol identity `(arena_index, generation)` (2578).
- Pending body selection is keyed on `symbol_key` sorted vector + `partition_point` (316-347).
- `prepare_const_qualification_selection` (2343): demanded if `definition.pending_qualification.is_some()` or `const_destination_requires_proof(...)` AND no pending initializer leaves (2369-2379); else returns `Ok(None)`.
- `seed_resolved_base` (1471): source map must retain the base's storage and every selected file (1480-1489).
- Operand owner association is keyed on declaration `ItemHandle` input (1781-1797), explicitly "no name, source span or matching type shape reconstructs the association" (1754-1755).

**Cases by shape.**
- `DeclarationInput` only `Conformance`/`Trait` (1764-1767): operand owners exist only for those two declaration kinds.
- `type_inputs::bind` (1914): `bind_roots!` lists 12 root kinds (const_declarations, data_definitions, type_aliases, domain_definitions, machines, operators, measures, propositions, mathematical_definitions, traits, conformances; `wire_schemas` is in `RootWatermarks` (1381) but NOT in the `bind_roots!` list (1941-1953)). A pending type input attributed to a wire-schema root would yield "pending type input lost its produced declaration owner" (1955) (inferred from reading both lists).
- `RootWatermarks` (1369-1382) enumerates 12 root kinds explicitly.
- `finish_const_initializer_selection` (2400) takes initializers from `const_declarations` only (2407-2419).
- Tests list operator introducers: `primitive_family_binding_tests` (2855-2867) enumerates 6 declaration spellings (e.g. `boundary operator -`, `boundary machine ==`, `operator +`, `machine +`) that must join the primitive family.

**Same job elsewhere.**
- `resolve_numeric_probe` (2297) and `resolve` (2056) have the same body shape `begin(...)` then `drive(...)`; the only difference is `begin_complete` vs `begin(request, Complete)` (2057 vs 2300): `resolve` uses `begin_complete` (which runs machine-equation completion) while `resolve_numeric_probe` calls `begin` directly, skipping equation completion (read; comment 2292-2296 "retaining machine equations as pending execution obligations").
- The "provisional drive then `complete` then loop until root count stable" pattern appears three times: `resolve_deferred` (2099-2121), `begin_complete` (2479-2490, single pass, no loop), `complete_machine_bodies` (351-372). The first and third loop, the second does not (read).
- `index_deferred` (2172) / `finish_const_initializer_selection` (2400): both end by building a `DeferredResolvedFrontier`; `resolve_deferred`, `resolve_deferred_numeric_probe`, `resolve_deferred_initializer_probe` and `prepare_const_initializer_selection` differ only in the `ConstResolutionMode` and equation pre-pass (read).
- `Lowerer::seed_resolved_base` and `DeferredResolvedFrontier::seed_body_lowerer` (404) both seed a lowerer from a base.
- Dependency collectors: `ConstInitializerSelection::{initializer_expression_dependencies, pending_leaf_dependencies, declaration_dependencies}` all call into `crate::constant::initializer_dependencies` (collect / collect_declaration / expression_at_source); not read.
- Outside the unit: grep `\b(resolve_deferred_numeric_probe|resolve_numeric_probe|resolve_deferred_initializer_probe|prepare_const_qualification_selection|resolve_call_result_selection|resolve_const_argument_selection|resolve_extension|resolve_deferred_extension|prepare_const_initializer_selection)\b` count: resolution.rs 20, lib.rs 6, psi-compiler/source_compilation.rs 2, build-time-evaluation layouts.rs 2, const_initializers.rs 3, const_generic_expressions.rs 3, const_evaluation.rs 6 (42 total, 7 files).

**Unused / one-caller.**
- `resolve_numeric_probe`: grep count 4 in resolution.rs (definition, test `use` line, two test call sites... the 4 hits are: definition 2297, import 2675, call 2731, and the header comment 9) and 1 in lib.rs. I did not read lib.rs; it is probably a re-export. So within this crate's code I can only find the test `compare` helper (2731) calling it (grepped, count only; callers outside omega-rust crates 0).
- `resolve_call_result_selection` / `resolve_const_argument_selection` grep count: resolution.rs 5, lib.rs 2, build-time-evaluation layouts.rs 2, const_evaluation.rs 1, const_generic_expressions.rs 1.
- `prepare_const_qualification_selection`, `resolve_deferred_initializer_probe`, `resolve_deferred_numeric_probe`, `resolve_extension`, `resolve_deferred_extension`: counted only in the aggregate above (42 hits / 7 files).
- `DeferredResolvedFrontier::with_prepared_allocations`, `activate_sources`, `into_pending_type_inputs`, `shared_trees`: not grepped.

**Stated limits.**
- 2063 "Their implementations remain pending" (deferred bodies).
- 2325-2326 "deferred unrelated machines cannot affect preparation or become executable through this carrier."
- 2007-2012 forest passed by value to prevent retaining "the unprepared sibling".
- 1250-1254 "Expressions absent from this list are either compiler-generated or belong to a source surface whose public/private disposition has not yet been classified by its owning declaration."
- 2825 test comment: "Every introducer answers the same rule" (primitive operator family).

**Fail-open risks.** None seen of the accepting kind in this file; it orchestrates and returns `Err(vec![Diagnostic])` on lost custody ("lost its exact declaration" 730, 794, 840; "no retained deferred implementation" 85-89, 300; "prepared allocation join does not match" 244).
- `prepare_const_qualification_selection` (2343): returns `Ok(None)` if no destination is demanded (2381-2383): the caller then does not run the qualification-judgment path; stated at 2337-2342 ("Selection and leaf classification only schedule work: every selected destination still returns through ordinary typed checking"). "stated".
- `Lowerer::source_reference_can_see_declaration` (1553): `self.sources.as_deref().is_none_or(...)` returns TRUE when there is no source map (comment at 2017-2019: "A source-free forest has none, and those checks stay permissive for it"). Wrong program: any source-free forest sees every declaration from everywhere. "stated" (source-free forests are the unit-test/probe case; callers of source-free `resolve` outside the unit were not traced).
- `source_resolution_strata_separate` (1563): `is_some_and` => false (no separation) when no sources.
- `finish_resolution` (2568-2605): `predecessor.is_none_or(...)` means with no predecessor every machine counts as new (intended).
- `type_inputs::bind` (1914-1961) for an `owner` that `is_valid()` skips the root-range binding entirely (1921-1929): attaches arguments directly to the owner symbol with no check that the owner exists in `trees` (open).

**Coverage (file 6):** read in full 1-2889 (2889 lines).

---

## Unit coverage

- **Coverage:** read in full, all six files: preparation_const_evaluation.rs 1-6545 (6545 lines), preparation_generic_data.rs 1-4877 (4877), preparation_generic_uses.rs 1-1843 (1843), preparation_machine_equations.rs 1-1899 (1899), preparation_type_equations.rs 1-3156 (3156), resolution.rs 1-2889 (2889). Total 21209 lines read; 0 skimmed; 0 not read. The Read tool refused one 1843-line read of preparation_generic_uses.rs for size, so that file was read in two ranges (1-950, 951-1843); no lines were skipped.
- **Five things a maintainer most needs to know about this unit.**
  1. The six files are one pre-resolution "preparation" subsystem: `preparation_generic_data.rs` includes `preparation_const_evaluation.rs` (`#[path]`, line 986) and `preparation_generic_uses.rs` (line 3820) as child modules and re-exports their items (3946-3991); `preparation_type_equations.rs` is re-exported as `equations` (1734); `preparation_machine_equations.rs` is a separate module used by `resolution.rs`. They rewrite SYNTAX to SYNTAX before symbols exist, so most judgments are keyed on identifier TEXT plus a `ConstantSelection` lookup, not on resolved symbols (the exception is file 4, which reads resolved trees).
  2. Generic data instance identity is `(template ItemHandle, Vec<ClosedArgumentIdentity>)` (generic_data 1116-1131, compared at 3293-3299). `ClosedArgumentIdentity` has no lifetime component (stated 894-897 of file 5), and const arguments are identified by the argument's TEXT (`Constant(String)`, generic_data 550-566). Names like `Box<u64>` are display metadata (generic_data 3306-3318).
  3. Const evaluation lives in several overlapping evaluators: i128 symbolic (`evaluate_const_argument_expression`), `ConstFactValue` with carriers (`evaluate_const_fact_binary`), exact-rational anonymous values (`AnonymousNumericValue`), and `CanonicalConstNode` structural values with length-framed string encoding (`framed`, const_evaluation 4674). A "None" from these means "decline and keep fenced", which callers turn into an error ("requires declaration-site proof checking") only on some paths.
  4. Failure mode pattern: most unsupported shapes return `None`/`Ok(None)`/`continue` and leave the authored syntax unchanged for "the ordinary resolver/checker path" (generic_data 279-280, generic_uses 236-238, 1348-1352, const_evaluation 429-437). Whether the later stage rejects each leftover was not traced in this unit.
  5. `resolution.rs` runs a provisional lowering in `ArgumentSelection` mode, calls `machine_equations::complete` and repeats until the root-item count stops changing in `resolve_deferred` (2099-2121) and `complete_machine_bodies` (351-372), but `begin_complete` (2479-2490) runs it once; `resolve_numeric_probe` (2297) skips equation completion entirely.
- **What you are unsure about.**
  - Whether the many `Ok(None)`/"leave untouched" paths in `prepare_closed_application` (const_evaluation 438-733) and `closed_argument_identity` (generic_data 475-701) always end in a later rejection for a wrong program; I did not read the downstream resolver/checker.
  - Whether `closed_fixed_array_length` in other crates (grep hits in validation, typed-trees, interpreter, provider-planning, build-time-evaluation) is the same function; I read only the definition at const_evaluation 5492.
  - Whether `lib.rs` re-exports `resolve_numeric_probe` (grep count showed 1 hit in lib.rs; lib.rs not read).
  - Whether the `IntervalSet`/`CountedQuantity` name skip (generic_data 3086-3092) has a counterpart that handles those types elsewhere (the only other hits are in the validation crate, `completed_storage_and_conservation.rs:6538-6601`, not read).
  - The exact outside user count for each `pub(crate)` function; I gave aggregate greps in count mode, not per function, in several places.
