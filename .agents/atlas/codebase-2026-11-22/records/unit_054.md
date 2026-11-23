# Record: unit_054 (area 02-source-stages-00-04)

Checkout `C:/Development/Omega/.agents/worktree/worker-2/`, detached at `4f3c255d`. All paths below are under
`P/` = `source-rust/subsystems/compiler/source-to-psi/pipeline/02_syntax-trees-to-symbol-resolved-trees/`
unless they start with `source-rust/`. All of this is stage 02 (syntax trees to symbol-resolved trees). Every file
was read in full ("read"); counts come from the Grep tool (count or content mode, uncapped) and are quoted as
"grepped". The unit has 41 files, 21,174 lines.

Pipeline position (from the crate map, not repeated): `declarations/` rewrites syntax to syntax before any symbol
exists; `lowering/` turns syntax nodes into the resolved carrier (`SymbolResolvedTrees`) with every symbol handle
left invalid; `names/` later fills symbols in.

---

## declarations/generic_data/uses.rs (1809 lines)

1. Purpose. After generic data (`Box<T>`) has been instantiated into closed copies, this file rewrites authored
   struct literals, qualified case names, `x in Base::Case` memberships and destructure heads so they name the
   closed copy that the surrounding expected type selects. It also holds the syntax-expression inventory walk that
   other generic-data passes reuse.
2. Shape. Seven inline modules, re-exported at the bottom (`uses.rs:1750-1768`):
   - `alias_constructors::select` (`:21`) infers a qualified alias constructor from a complete expected application; returns `AliasConstructor {template, case, application, carrier}` (`:14`).
   - `assignments::relabel_closed_data_uses_in_constants` (`:107`) and `..._in_exact_assignments` (`:162`).
   - `calls::relabel_closed_data_uses_in_exact_calls_and_returns` (`:241`), `exact_expression_receiver_owner` (`:408`), `call_context_parameter_types_agree` / `call_context_types_agree` (`:434`, `:446`).
   - `constructors::expected_instance` (`:550`), `selected_constructor` (`:572`), `selected_case_value` (`:597`), `constructor_path_name` (`:615`), `constructor_carrier_span` (`:631`), `closed_constructor_carrier` (`:646`), `relabel_data_literal_for_expected_type` (`:671`, recursive core).
   - `expressions`: `ExpressionMarks` trait (`:942`), `ExpressionInventory` (`:954`), `statement_type_inputs` (`:969`), `visit_expression_type_inputs` (`:998`), `visit_static_argument_type_inputs` (`:1044`), `ConcreteExpressions` (`:1063`), `selected_machine_expression_handles` (`:1082`), `selected_machine_state_handles` (`:1096`), `concrete_machine_declarations` (`:1112`), `collect_statement_expression_handles` (`:1176`), `collect_expression_handles` (`:1273`), `ConstructorFrontier {parameters, prior_statements}` (`:1367`).
   - `patterns::relabel_closed_sum_memberships_from_local_types` (`:1412`), `named_type_name` (`:1585`), `closed_sum_path` (`:1600`), `subject_annotation` (`:1621`), `relabel_pattern_head` (`:1686`).
   - top-level `concrete_state_local_types` (`:1777`), "the shared prelude every local-typed relabel pass walks".
3. In/out. Reads `SyntaxTrees` (mutably, rewrites expressions in place via `replace_expression`), the instance roster `&[Instantiation]`, an optional `ConstantSelection`, and declaration handles. Writes: replaced `Name`/`StructLiteral`/`Match`/`Membership` expression nodes (e.g. `:827-837`, `:851-856`, `:876-878`, `:1560-1563`, `:1739-1745`) and `application_type` on patterns.
4. How it decides.
   - Which instance a literal gets: the expected type's `ClosedArgumentIdentity::Instance(template, arguments)` is matched against `Instantiation.template` and `.argument_identity`; exactly one match or none (`:560-569`). Identity-keyed.
   - Whether a call argument gets a context: call-target source spelling `String` keyed in `HashMap<String, Option<Vec<TypeReferenceHandle>>>` (`:247`, `:356`) and `HashMap<(String,String),..>` keyed by (attached data name, method leaf) (`:248`, `:354`); an entry becomes `None` when same-name overloads disagree on parameter type structure (`:283-292`, `:295-304`). Spelling-keyed.
   - Receiver owner for a method call: `local_owner_types: HashMap<String,String>` from parameter/local names to type names (`:312-326`, `:421`). Spelling-keyed; a bare single-segment name only (`:418`).
   - Assignment target type: bare single-segment local name looked up in `local_types` (`:194-198`). Spelling-keyed.
   - Whether a name is a captured local rather than a constructor: `ConstructorFrontier::captures_name` compares `local.name` / `state_parameter.name` spellings (`:1380-1385`).
   - Sum membership subject type: a local's declared type via `subject_annotation` (`:1621`), which also follows `DispatchSubject` locals by name (`:1652-1653`).
   - `call_context_types_agree` (`:446-524`) compares `Named` by `as_str` equality (`:456`), generic base by spelling (`:507`), lifetimes by spelling (`:472`, `:508-511`).
5. Cases by shape. The comment at `:233-240` lists the supported shapes: "Free-machine overloads and concrete attached-machine overloads each contribute a context only when every same-name candidate on the exact owner agrees ... Computed, chained, and dynamic receiver selection remains resolver-owned and fails closed here." `call_context_types_agree` handles Named, Unit, Reference, Slice, FixedArray, Generic; `Constrained` and `ConstExpression` return `false` (`:518-521`), every other pair `false` (`:522`). Assignment targets: only a bare local name (`:189-201`). Constants: a `Const` item's value and its normalized `authored_expression` (`:113-130`, `:135-154`).
6. Same job elsewhere. `collect_expression_handles` (`:1273`) is a full syntax-expression walker; `expression_contains` (`P/lowering/statement/value_call_hoisting.rs:138`) is a second walker over resolved expressions; `collect_expression_binder_mentions` (`P/declarations/type_equations.rs:2473`) a third (type-binder mentions); `collect_contract_expression_handles` / `collect_type_expression_handles` (`P/declarations/generic_data/const_evaluation/templates.rs:367,375`, grepped `fn (collect_[a-z_]*expression..)` over `source-to-psi/`) and `collect_expression_subtree` (`P/selections/authored_selections.rs:1201`) are further expression walks (suspected, not read). `visit_operands` exists on syntax statements (`representations/syntax-trees/statements/control_flow.rs:431`) and resolved statements (`representations/symbol-resolved-trees/nodes/control_flow_and_calls.rs:587`).
7. Unused / one-caller. `collect_statement_expression_handles|collect_expression_handles|ExpressionMarks` grepped: 76 occurrences in 5 files (59 in uses.rs itself, 9 `declared_types.rs`, 6 `const_evaluation/templates.rs`, 1 `domain_heads.rs`, 1 `generic_data/mod.rs`). `statement_type_inputs`: callers `P/declarations/declared_types.rs:1395`, `const_evaluation/templates.rs:896`, `P/lowering/state.rs:126` (the last test-support only). `visit_expression_type_inputs`: callers in `declared_types.rs` (3), `templates.rs` (5), `domain_heads.rs:183`.
8. Stated limits. "computed targets remain fail-closed" (`:160`); "Advisory constructor rewriting" (`:1362`); "fails closed here" (`:239`); "Unsupported or expression-bearing details fail closed" (`:432`).
9. Fail-open risks. Every `return` in `relabel_data_literal_for_expected_type` when the owner, name, instance or field cannot be matched leaves the authored literal untouched (e.g. `:748`, `:753`, `:756`, `:768`, `:798`, `:801`). Stated: "A captured Name remains authored syntax for the ordinary resolver; no binding is chosen here" (`:1362-1365`); leaving syntax alone means a later stage still has to resolve it. No input reaches an accept-and-skip-a-check path here. Classification: stated.

---

## declarations/generic_data/work_domain.rs (91 lines)

1. Purpose. Bounds which declarations and type positions generic-data synthesis may mutate: it reads all declarations as lookup context but only owns declarations of the selected domain plus anything appended after the work began.
2. Shape. `GenericDataWorkDomain {type_roots, declarations}` (`:23`), `TypePositionPurpose {Identity, Materialization}` (`:29`), `SynthesisWork {domain, declaration_frontier: usize, expression_frontier}` (`:34`) with `new` (`:41`), `owns_new_expression` (`:54`), `declarations` (`:58`), `type_positions` (`:68`).
3. In/out. Reads `SyntaxTrees` root handle count and expression frontier; returns `Vec<ItemHandle>` and `Vec<TypeReferenceHandle>`; mutates nothing.
4. How it decides. Positional: a declaration is owned when it is in `domain.declarations` or its root-handle index is at/after `declaration_frontier` (`:60-64`); an expression is owned when it is inside the arena suffix since the frontier (`:55`).
5. Cases by shape. For `Identity` it uses all owned declarations; for `Materialization` only `domain.declarations` plus the stored positions of the appended suffix (`:74-85`).
6. Same job elsewhere. none seen in unit.
7. Unused. `SynthesisWork|TypePositionPurpose|GenericDataWorkDomain` grepped: 21 occurrences in 5 files (9 here, 7 `synthesis.rs`, 3 `const_evaluation/domains.rs`, 1 `generic_data/mod.rs`, 1 `machine_equations.rs`).
8. Stated limits. none.
9. Fail-open risks. none seen. `declarations()` uses `Vec::contains` (`:61`), quadratic in the appended suffix.

---

## declarations/integer_widths.rs (270 lines)

1. Purpose. Rewrites a type like `u<32>` into the same named carrier as `u32` before any symbol exists, and refuses malformed width applications.
2. Shape. `family_sign` (`:34`), `enum Width {Closed(i128), TooWide, Named, Runtime, Unevaluated}` (`:42`), `normalize(&mut SyntaxTrees) -> Result<Vec<FixedInteger>, Vec<Diagnostic>>` (`:54`). Tests `:178-270`.
3. In/out. Reads every `Generic` type reference whose base spelling is `u` or `i`; rewrites to `TypeReferenceNode::Named(carrier.to_string())` (`:170-173`); returns the carriers it created. Diagnostics for zero, over-`u64`, missing, repeated and runtime widths (`:99-111`, `:140-161`).
4. How it decides. Family: base-name spelling `"u"`/`"i"` (`:35-38`). Width: argument is a `Named` or `ConstExpression` whose spelling is digits/underscores (`:77-86`) or an integer literal (`:117-120`).
5. Cases by shape. "A width written as a name is left as written" (`:17`): `Width::Named | Width::Unevaluated => {}` (`:162`). A `ConstExpression` that is not an integer literal or a single-segment name is `Unevaluated` (`:124-127`); any other argument node (e.g. a type) is `Runtime` and refused (`:130`, `:158-161`).
6. Same job elsewhere. `P/declarations/mod.rs:82` and `:92` call it twice in `prepare` (grepped: 2 call sites plus the test helper in this file).
7. Unused. `integer_widths::normalize` is `pub(crate)`; grepped in `declarations/mod.rs` only (2 callers).
8. Stated limits. "A width written as a name is left as written ... resolution reads the name in that scope" (`:17-23`).
9. Fail-open risks. `Width::Named | Width::Unevaluated` accepted unchanged (`:162`). Stated (`:17-23`): resolution and typing own those; wrong program: `u<N+1>` with an unevaluated expression passes this stage and must be caught by typing ("typing accepts a `const` parameter ... and refuses a runtime width", `:21-23`). Classification: stated, not traced.

---

## declarations/machine_equations.rs (2117 lines)

1. Purpose. After provisional name resolution, completes a generic machine call's static arguments from the machine's structural `requires` equations (like `Backing == [Element; Count]`) and from the declared types of the actual arguments; rewrites the authored call with the completed arguments.
2. Shape. Inline `mod argument_types` (`:36-741`) with `projections::member_types` (`:92`), `closed_nominal_owner` (`:232`), `DeclaredTypeOrigin {Parameter, Result, Field, Payload, Constant}` (`:263`, `pub(crate)`), `array_lengths` (`:310`), `closed_type` (`:371`), `declared_type` (`:451`), `CallTypeObservations` (`:499`), `expression_types` (`:504`), `constraints` (`:623`), `call_constraints` (`:666`), `statement_constraints` (`:719`); inline `mod completion` with `complete_arguments` (`:750-1041`). Outside: `SourceLinks {declared_types, declarations, data_declarations, expressions, statements}` (`:1061`), `required` (`:1092`), `machine_type_equations` (`:1107`), `validate_declarations` (`:1119`), `selected_machine` (`:1165`), `complete` (`:1178`), `completes_call` (`:1391`), `reject_machine_arguments` (`:1455`), `ArgumentClosure` (`:1514`), `BinderClosures` (`:1534`), `classify_*` (`:1565-1759`), `binder_closure` (`:1767`), `complete_authored_arguments` (`:1790`), `inferred_const_argument` (`:1832`), `supplied_const_argument_reference` (`:1886`), `scalar_const_argument` (`:1917`), `application_argument_reference` (`:1975`).
3. In/out. Reads `SymbolResolvedTrees` (provisional), `SourceLinks` (links from resolved handles back to authored syntax, recorded by lowering), a `ConstantSelection`. Mutates `SyntaxTrees`: replaces authored `Call` expressions' `machine_arguments` (`:1282-1285`, `:1343-1346`), appends type parameters (`:864`), and finally runs `desugar_generic_data_instances_in` on the new type roots (`:1353-1361`).
4. How it decides.
   - Whether a call needs completion: `completes_call` (`:1391-1417`) keys on `trees.machines[index].has_structural_type_equations` or the original `Machine` syntax's equations; selected machine found by symbol (`selected_machine`, `:1165-1176`). Identity-keyed.
   - Closedness of arguments: `BinderClosures` indexed by symbol arena index (`:1534-1563`); symbol kind (`TypeParameter`, `Parameter`, `MachineParameter`, `ConformanceParameter`) classifies a binder (`:1767-1784`). Identity-keyed.
   - Which formal/actual pairs feed the solver: `DeclaredTypeOrigin` links, with `sources.declared_types.iter().find(...)` linear scans (`:461`, `:645-648`, `:684-697`). Identity (handle) keyed.
   - Binder positions inside the solver and inside `signature_mentions` are by parameter-name `String` (`:792-816`; the solver is in `type_equations.rs`).
   - Machine parameter kinds: "non-type binder `{name}`" refusal at `:828-837`, keyed on position/kind.
   - Struct field of a projected receiver: `trees.symbols.find_child_by_name_and_kind(owner, member.member.as_str(), SymbolKind::Field)` (`:152-156`). Spelling within an identity-selected owner.
5. Cases by shape. `validate_declarations` refuses (`:1134`) "machine structural equations currently require explicit calls; operator and conformance supplies retain undisclosed equations" when a machine with `spelling` or `satisfies` has equations that require argument recovery. `complete` refuses a conformance row whose realization has equations (`:1189-1199`) and a noncall selection of such a machine (`:1287-1298`). `expression_types` handles only `Name` (without static application), `Borrow`, `Member`, `Call`, non-empty `Block` tail, `Match` (all arms), else `None` (`:512-619`). `classify_*` depth cap 128 returns `ConstBinder` (`:1631`, `:1664`).
6. Same job elsewhere. `P/declarations/type_equations.rs` `complete_equation_arguments_with_types` (`:2628`) is the solver this file calls; the data-side caller is `projections::member_types` here (`:200-217`). `lowering/state.rs:582-596` and this file both pick a data/machine by name; unrelated.
7. Unused / one-caller. `machine_equations::complete` grepped: 1 call site (`P/deferred_bodies.rs:423`). `required`: imported at `P/resolver.rs:22` (1 use). `validate_declarations` (this file's): called at `P/pre_resolution/mod.rs:206` and `P/resolver.rs:201`. `machine_type_equations`: used in `lowering/machine.rs:46` and here. `type_reference_parameter_mentions|template_type_equation_offsets|classify_machine_equations|materialize_completed_type|complete_equation_arguments` grepped: 20 occurrences in 6 files.
8. Stated limits. "Open applications remain obligations and reject" (`:9-10`); "machine structural equations currently require explicit calls" (`:1134`); "Early type observations are evidence for inference, not a second call admission boundary" (`:492-497`).
9. Fail-open risks.
   - `complete_arguments` warnings that are not errors are written with `eprintln!("{warning}")` and dropped (`:969-971`, `:1372-1374`; grepped `eprintln!` in `P/`: exactly these two sites). Wrong program: any non-error diagnostic the solver produces never reaches the user's diagnostics. Classification: open (comment at `:965` says "Landing errors reject; see report_preparation_diagnostics").
   - `completes_call` returns `Ok(false)` (call left untouched) when the callee has no equations (`:1409-1411`). Stated by the function doc (`:1378-1390`).
   - `constraints` returns an empty observation list when argument arity differs from parameter arity (`:641-643`) or the callee state is not found (`:633-639`). Stated (`:492-497`).
   - `let Some(index) = callee else { continue }` (`:1248-1250`, `:1309-1311`): calls with no selected machine are skipped (they have no equations to complete).

---

## declarations/mod.rs (107 lines)

1. Purpose. Entry for stage-02 syntax-to-syntax preparation; fixes the order of the passes.
2. Shape. `prepare_statement_bodies` (`:38`), `Prepared {syntax, constant_selection}` (`:48`), `prepare` (`:63`).
3. In/out. Takes `SyntaxTrees` by value, optional `SourceMap`, top-level bindings, `ConstResolutionMode`, optional retained base; returns `Prepared`. Order: assembly + dispatch preparation, `builtin_type_names::validate`, `target_families::normalize`, `ConstantSelection::with_shared_base`, `declared_types::normalize`, `integer_widths::normalize` + `install_integer_types`, `domain_heads::normalize`, (Complete mode only) `fold_closed_scalar_initializers`, a second `integer_widths::normalize`, a second fold when new widths appeared, `synthesize_trait_defaults_with_selection` (`:70-102`).
4. How it decides. Constant folding runs only when `constants == ConstResolutionMode::Complete` (`:89`, `:95-97`).
5. Cases by shape. none.
6. Same job elsewhere. `integer_widths::normalize` called twice (`:82`, `:92`); `fold_closed_scalar_initializers` possibly twice (`:90`, `:100`).
7. Unused. `prepare_statement_bodies` is `pub(crate)`; not grepped separately.
8. Stated limits. "Generic data normalization is the caller's earlier step, not part of this one." (`:55-56`).
9. Fail-open risks. none seen.

---

## declarations/target_families.rs (195 lines)

1. Purpose. Gives each target-specific machine of a product family a stable identity and adds one bodyless portable interface machine per family; refuses two implementations for the same target.
2. Shape. `Member {handle, name, target, file, module}` (`:23`), `compare_family` (`:32`), `normalize(&mut SyntaxTrees, Option<&SourceMap>)` (`:63`).
3. In/out. Reads each root machine with `authored_target` or `target`; sorts and chunks them; pushes a cloned bodyless interface machine (`:138-177`) and rewrites each member's `authored_target`/`target` (`:179-193`).
4. How it decides. Family = same package identity, same build/product scope, same module directory namespace, same machine name (`:32-55`); duplicates are `target.as_str` equality inside a family (`:104-106`). Name/target/module are source spelling. Build-scope families are skipped (`:114`).
5. Cases by shape. A family with no entry state is refused (`:139-144`). Missing source in the supplied map is refused (`:78-84`).
6. Same job elsewhere. none seen.
7. Unused. `target_families::normalize` is called once from `declarations/mod.rs:72` (grepped by reading mod.rs).
8. Stated limits. "No native target catalog participates in this pass." (`:10`).
9. Fail-open risks. When `sources` is `None`, `file` is `None`, so every machine shares `module = []` and package `None` (`:76-94`); wrong program: families from different packages merge if the caller passes no source map. Classification: open (caller `prepare` takes `Option<Arc<SourceMap>>`, `mod.rs:65`; not traced).

---

## declarations/trait_defaults.rs (798 lines)

1. Purpose. Before symbols exist, gives each named trait implementation (`X satisfies Trait { ... }`) a member for every omitted requirement that has a default body, and gives a closed `Equatable<T>` implementation a synthesized `equals` member.
2. Shape. `TraitDefaultsInput` (`:56`), `RequirementInstance` (`:93`), `synthesize_trait_defaults` (`:101`, test-support), `synthesize_trait_defaults_with_selection` (`:116`), `inherit_refinement_telescopes` (`:348`), `collect_requirement_instances` (`:465`), `replace_closed_members` (`:528`), `synthesized_equality_member` (`:566`), `implementation_member_name` (`:646`), `instantiate_default_signature` (`:659`), `supplied_arguments` (`:674`), `machine_from_signature` (`:713`).
3. In/out. Reads traits via `selection.trait_lookup(syntax, name).unique()` (`:126`, `:209`); mutates conformance items by appending `ConformanceMember::TraitDefault` and `Synthesized` members (`:307-311`, `:326`, `:330`).
4. How it decides.
   - Trait identity: `SymbolHandle` from the constant-header table, "never by leaf spelling" (`:17-21`); a non-unique lookup skips the trait or conformance (`:126`, `:209-214`).
   - Which defaults already exist: `(declaring_path String, requirement_ordinal)` key (`:269-272`, `:282-283`). Path spelling plus ordinal.
   - Which requirement is the equality row: `signature.name.as_str == "equals"` and no default body (`:574`, `:587-591`); an existing member named `equals`, a `Selection` for `equals`, or any `Synthesized` member blocks synthesis (`:576-586`). Spelling-keyed. The trait gate is `sealed_intrinsic(trait_symbol) == EqualityProtocol` (`:315-316`), identity.
   - Parameter substitution is a `HashMap<String, TypeReferenceHandle>` keyed on binder name (`:97`, `:241-246`).
5. Cases by shape. Refinement postconditions: exactly one base requirement by name (`:428-439`), complete base application (`:400-406`). Equality synthesis: "A trait without exactly one type parameter and one two-parameter `equals` requirement synthesizes nothing" (`:563-565`, code `:575`, `:592`).
6. Same job elsewhere. none seen.
7. Unused. `synthesize_trait_defaults` (test/test-support only, `:100-101`). `synthesize_trait_defaults_with_selection` called from `declarations/mod.rs:102`.
8. Stated limits. "an ambiguous authored trait name defers to ordinary resolution instead of guessing an owner" (`:19-21`); "ordinary row completeness reports it" (`:565`).
9. Fail-open risks. `instantiate_default_signature` and `collect_requirement_instances` call `syntax.clone()` on the whole forest per instantiation (`:504`, `:669`) (cost, not acceptance). `.expect("parsed named implementations carry their name")` (`:230`) panics for an unnamed conformance. Skips on non-unique trait lookup are stated (`:19-21`). Classification: stated.

---

## declarations/type_equations.rs (3270 lines)

1. Purpose. The solver for "type equations": `requires` facts that equate a type binder with type structure (`Backing == [Element; Count]`). It recovers omitted generic arguments from them, checks complete tuples against them, and classifies each equation.
2. Shape. Inline `mod type_structure` (`:55-1204`) with `mod applications` (`:92-1077`, `Application`, `application`, `match_application`, `construct_application`, `application_integer/boolean/constant`, `require_equal_application_structure`) and `collect_binder_mentions` (`:1079`); `impl Solver` (`:1206-1701`: `type_operand_reference`, `match_type_structure`, `construct_type_structure`, `closed_array_length`, ...). Outside: `TypeEquation {fact_offset, definition, shape}` (`:1722`) with `requires_argument_recovery`, `structural_definitions`, `defines_structure`, `mentions_parameter`, `validate_kind`, `validate_names`, `reject_unclosed_names` (`:1734-1888`); `template_type_equation_offsets` (`:1898`), `validate_closed_name_type` (`:1956`), `validate_materialized_type_equations` (`:1996`), `EquationShape {Boolean, Structural, KindMismatch}` (`:2022`), `Structure {TypeOperand, TypeReference, Name}` (`:2058`), `Side` (`:2099`), `classify_machine_equations` (`:2127`), `classify_type_equations` (`:2145`), `normalize_applied_operands` (`:2249`), `classify_side` (`:2338`), `equation_shape` (`:2398`), `EquationTemplate` (`:2588`), `complete_equation_arguments` (`:2599`), `..._with_types` (`:2628`), `materialize_completed_type` (`:2689`), `Binding {Type, NamedType, Integer, Boolean, Constant}` (`:2706`), `Solver` (`:2729`) with `solve`, `apply`, `predicate_value`, `recover_binder`, `match_structure`, `match_name`, `bind_integer` (`:2793-3246`), `normalized_boolean_argument` (`:3253`), `const_binder_envelope` (`:3267`).
3. In/out. Reads `SyntaxTrees` type references/expressions, `ConstantSelection`; the solver appends new type-reference nodes (`insert`, `:545`, `:571`, `:593`, `:606`) and returns completed argument tuples. `normalize_applied_operands` rewrites `Name{application}` operands into `TypeExpression` (`:2329-2334`).
4. How it decides.
   - Binder identification is by name: `binder_index` compares `parameter.name.as_str == name` (`:2109-2118`), `parameter_position` (`:1294-1303`), `collect_binder_mentions` pushes position of the first parameter name equal to the mentioned name (`:1085-1089`). Spelling-keyed on template-local names.
   - Nominal head identity: `closed_name_identity` / `selected_data_item` selection results compared by declaration (`:1001`, `:429`), never by layout/leaf (`:14`).
   - Closed identity equality: `closed_argument_identity(...)` pairs compared (`:1501-1516`).
   - Integer constants: `name.as_str().parse::<i128>()` (`:647`, `:2759`) and the builtin carrier `const_binder_envelope(type_name)` via `LandedIntegerType::from_name` (`:3267-3269`), a name lookup by builtin spelling (`:693-698`).
   - Kind (type vs value binder): `TypeParameterKind`.
   - Boolean arguments: `boolean_literal_spelling(text)` and `CanonicalConstValue::from_atom` (`:3253-3265`).
5. Cases by shape. The comment `:10-18` and `match_type_structure` (`:1310-1464`) list: references (elided actual region only, `:1325-1335`), slices, named, fixed arrays, generic applications, domain-constrained types with exactly one `Domain` constraint (`:1451-1458`); everything else goes to `require_closed_pattern` then `require_equal_type_structure` (`:1459-1462`). `application()` accepts `Generic` or `Constrained` with exactly `[Domain]`, else "requires an exact declared application" (`:251-267`). Constructor parameters: `Type` or `Const` only (`:340-350`). `Structure::Name` only with a closed or binder name (`:2058-2076`). `classify_type_equations`: operators `==`, `!=`, `&&`, `||`, `!` only (`:2163-2200`). Domain applications require lifetime-free arguments (`:200-208`, `:893-948`: "requires anonymous reference lifetimes until lexical lifetime identity is retained", `:907`).
6. Same job elsewhere. Binder-mention collection also appears in `machine_equations.rs:792-816` (`type_reference_parameter_mentions`) and in `callable_signatures::SignatureInputs` use (`:1174`). The Rust type-side of `closed_fixed_array_length` is used at `:1415`; `closed_array_length` (`:1643`) is a second extent reader on the same data (`ConstParameter` handled in both, `:1656-1688`). `normalize_applied_operands` (`:2249`) is called once, from `P/declarations/declared_types.rs:1177` (grepped).
7. Unused / one-caller. `normalize_applied_operands`: 1 call site. `template_type_equation_offsets` etc: see machine_equations item 7 (20 occurrences in 6 files).
8. Stated limits. "This is not an arithmetic solver or a new source of array-length evaluation." (`:69-70`); "Runtime contents and compatible storage sizes never supply an argument." (`:26`); "requires anonymous reference lifetimes until lexical lifetime identity is retained" (`:907`); "cannot decide an open or unsupported element type; supply explicit arguments" (`:991`, `:1486`, `:1513`).
9. Fail-open risks.
   - `validate_materialized_type_equations` returns `Ok(())` when the reference is not `Generic`, when `selected_data_item` finds none, or when the item is not data (`:2001-2011`). Wrong program: a `Generic` whose base name selects no data item (ambiguous or a domain head) passes this check. Classification: traced partly: its only caller `lowering/type_reference.rs:85` runs it only when `!lowerer.structural_type_equations_pending`; the Lowerer default is `true` (`lowering/lowerer.rs:307`) and only `resolver.rs:129` clears it (grepped `structural_type_equations_pending\s*=`). Open for the unselected case.
   - `predicate_value` for `Or` with one unknown operand gives `None` -> `Outcome::Deferred` (`:2995-3000`), later refused at `:2821-2829` ("cannot be decided from the supplied arguments"); fail-closed.
   - `unreachable!` at `:2675`, `:2834`, `:3025`, `:3096`, `:3149`, `:3161`, `:3205` assert solver invariants; a wrong binder kind slipping past classification would panic rather than accept.

---

## deferred_bodies.rs (650 lines)

1. Purpose. Holds the "frontier": a resolved program whose machine bodies are still unlowered syntax, and completes selected bodies on demand, transactionally.
2. Shape. `complete_callable_type_dependencies` (`:41`), `complete_initializer_receipt_dependencies` (`:108`), `DeferredResolvedFrontier` (`:152`: `syntax: Arc<SyntaxTrees>`, `trees: Arc<SymbolResolvedTrees>`, `pending: Vec<PendingMachineBody>`, `pending_type_inputs`, `supplied_operand_owners`, `declaration_inputs`, `selection`, `sources`, `bindings`, `hoist_counter`, `const_resolution_mode`, `structural_type_equations_pending`, `equation_sources`), `DeferredBodyScope {All, BuildAndToolchain, SelectedFiles}` (`:225`), `activate_sources` (`:259`), `with_prepared_allocations` (`:284`), `pending_machines` (`:298`), `pending_machines_in` (`:304`), `body_demand` (`:351`, test-support), `complete_machine_bodies` (`:372`), `seed_body_lowerer` (`:465`), `append_generated_declarations` (`:479`), `lower_selected_bodies` (`:496`), `ResolvedBodyCompletion` (`:513`).
3. In/out. `complete_machine_bodies(selected: &[SymbolHandle])` returns a successor frontier plus the predecessor `Arc` and an authored-selection frontier. It builds a fresh `Lowerer` seeded from the base (`seed_body_lowerer`), lowers the selected bodies, runs `finish_resolution`, replaces `self.trees` (`:434-456`).
4. How it decides.
   - Which pending body to complete: `pending_by_symbol` sorted by `(arena_index, generation)` and binary-searched by `partition_point` (`:376-408`). Identity-keyed; an unknown symbol is refused (`:401-405`).
   - Scope filter: `BuildAndToolchain` keys on the symbol's provenance source file's `origin == Toolchain || dependency_scope == Build` (`:316-324`); `SelectedFiles` on `sources.source_is_selected(span.source_id)` (`:336`). Source-file keyed.
   - Equation loop: while `equation_sources.is_some()`, lower provisionally, call `machine_equations::complete`, repeat until no new root item appears (`:412-433`).
   - Callable-type demand: a machine is demanded if any of its type parameters has pending callable inference or its lexical parent is demanded (`:53-74`).
5. Cases by shape. none (scope enum is the fixed list).
6. Same job elsewhere. `generated_source.rs` and this file both extend a base frontier: this file completes bodies, that one appends generated declarations (`generated_source.rs:28`).
7. Unused / one-caller. `with_prepared_allocations|resolve_deferred_extension|rebase_for_typed_continuation|ExtensionRequest` grepped: 40 occurrences in 13 files (incl. driver `resolve_and_type.rs` 11, tests). `into_complete_trees` and `body_demand` are test/test-support only (`:232`, `:350`).
8. Stated limits. "Completion is transactional. A failed body cannot partly advance the retained frontier" (`:368-371`).
9. Fail-open risks. `.retain(|body| !demanded.get(..).copied().unwrap_or(false))` (`:98`): a body whose ordinal is outside `demanded` stays pending (keeps, not drops). none seen that accepts a wrong program. `Arc::make_mut(&mut self.syntax)` (`:424`) clones the whole syntax forest if shared.

---

## generated_source.rs (94 lines)

1. Purpose. Resolves a later-stratum syntax forest appended to a retained resolved base (generated source), leaving bodies pending.
2. Shape. `ExtensionRequest {base: Arc<SymbolResolvedTrees>, syntax, sources, top_level_bindings}` (`:12`), `resolve_deferred_extension` (`:28`), `DeferredResolvedExtension` (`:51`) with `rebase_for_typed_continuation` (`:61`), test-support `resolve_extension` (`:83`).
3. In/out. Calls `resolve_deferred(ResolutionRequest{.. retained_base: Some(base)})` (`:38-43`); `rebase_for_typed_continuation` takes the successor's trees out with `mem::take` (`:66`) and rebases authored selections against the typed predecessor's selections (`:65-70`), then clones the trees again into the frontier (`:76`).
4. How it decides. none (delegates).
5. Cases by shape. none.
6. Same job elsewhere. `deferred_bodies.rs::activate_sources` / `with_prepared_allocations` also rejoin to a typed owner.
7. Unused. See `deferred_bodies.rs` item 7 (7 occurrences of the extension names in this file's group of greps).
8. Stated limits. "Must retain the base's exact source frontier." (`:19`).
9. Fail-open risks. none seen.

---

## lib.rs (17 lines)

1. Purpose. Crate root: re-exports `source_declarations::{declaration_selection, source_selections}`, `source_symbol_table::{source_inputs, source_symbols, source_tables}`, `symbol_resolved_trees::source_resolved`; names `resolver.rs` as `pub mod resolution` via `#[path]` (`:16-17`); in tests pulls `tests/support/frontend_operations.rs` as `frontend_fixture` (`:12-14`).
2-9. No types or decisions. `#![forbid(unsafe_code)]` (`:1`).

---

## lowering/data.rs (522 lines)

1. Purpose. Lowers a syntax `data` definition (fields, variants, quotient, type parameters, `requires` facts) into the resolved `DataDefinition` with invalid symbols.
2. Shape. `lower_data_definition` (`:30`), `retain_derived_const_argument_origins` (`:60`), `lower_data_definition_with_argument_origins` (`:116`), `lower_type_parameters` (`:266`, `pub(crate)`), `lower_data_member` (`:467`).
3. In/out. Appends to `declarations.data_members`, `data_payload_fields`, `data_type_parameters`; pushes `DeclaredTypeOrigin::Field/Payload` rows into `lowerer.equation_sources` (`:157-163`, `:504-508`); queues pending callable types and service reaches (`:416-461`).
4. How it decides.
   - Which `requires` facts to exclude from runtime facts: `template_type_equation_offsets` offsets (`:210-221`), excluded by span offset (`domain.rs:202`).
   - `zero_gated = !requires_facts.is_empty()` (`:222`).
   - Machine/proposition parameters without a contract are refused (`:299-304`, `:375-380`).
   - Value type parameters become synthetic data fields (`:124-141`).
5. Cases by shape. `TypeParameterKind` arms Type, Implementation, Const, Value, Machine{Inferred, RequirementIdentity, Structural, Nominal}, Proposition (`:273-394`). `lower_data_member` panics (`unreachable!`) on a `Retired` member (`:520`) — callers skip those (`:143-145`).
6. Same job elsewhere. `lower_type_parameters` is called from data, domain (`domain.rs:28`), item (conformance), machine, mathematical, operator, state (signature), trait_definition, type_aliases (grepped `lower_type_parameters(`: 20 occurrences in 11 files, including `03_.../src/declarations.rs` (8), which defines its own function of a similar name, suspected).
7. Unused. none.
8. Stated limits. The comment above `lower_type_parameters` (`:261-265`) reads "R2 rung 2 slice 1: fold one default-domain fact AT THE ZERO VALUE ..." and describes a function that is not in this file.
9. Fail-open risks. none seen.

---

## lowering/domain.rs (278 lines)

1. Purpose. Lowers domain definitions (`domain<..> carrier::Name`), aliases, member operators, and proof facts (`lower_proof_facts`).
2. Shape. `lower_domain_definition` (`:23`), `lower_domain_alias` (`:133`), `lower_domain_operators` (`:160`), `lower_proof_facts` (`:180`), `lower_proof_facts_excluding` (`:193`).
3. In/out. Interns a semantic-domain identity string into `symbol_resolved_trees.semantic_domains` (`:93-96`); sets `DomainSemanticRoles` (`:97-100`); proof facts append to `declarations.proof_facts` with source spans (`:256-266`) and queue `PendingAuthoredProofMembership` when an exposure is active (`:267-274`).
4. How it decides. The semantic domain identity is built from source: directory namespace joined with `::` plus the domain name, then prefixed by `package:<hex digest>:<scope>::`, `toolchain:...`, `toolchain-contract:...` or `unmanaged:...` by the owner file's package identity/origin (`:51-92`). String-keyed identity assembled from spelling plus provenance. `denotation_dimension` is set only when the domain has operators (`:98`).
5. Cases by shape. Facts are `Expression` or `Membership` only (`:214-253`).
6. Same job elsewhere. none seen.
7. Unused. `lower_proof_facts_excluding`: callers `data.rs:216`; `lower_proof_facts`: `data.rs:510`, `state.rs:694`, here.
8. Stated limits. none.
9. Fail-open risks. When `lowerer.sources` is `None`, `owner` is `None` and the identity is the bare name (`:57-66`, `:67`); wrong program: two same-named domains in a source-free resolution intern to one `semantic_id`. Classification: open (`lowerer.sources: Option`, `lowerer.rs:200`; callers not traced).

---

## lowering/expression.rs (966 lines)

1. Purpose. Lowers syntax expressions into the shared resolved `ExpressionTable`, including match dispatch, destructure patterns, saved subjects, calls and static machine arguments.
2. Shape. Inline `mod match_dispatch`: `lower_match` (`:54`), `with_transition_pattern_captures` (test, `:133`), `lower_saved_subject` (`:167`), `lower_destructure_pattern` (`:200`). `lower_expression_into_table` (`:335`), `retain_authored_expression` (`:360`), `lower_private_expression_into_table` (`:386`), `lower_expression_node_into_table` (`:410`), `lower_binary_expression_into_table` (`:421`), `lower_nonbinary_expression_node_into_table` (`:438-820`), `lower_applied_requirement_qualifier` (`:827`), `lower_static_machine_argument` (`:862`), `expression_table` (`:933`).
3. In/out. Appends to `lowerer.symbol_resolved_trees.tables.bodies.expressions`; records `(syntax handle, resolved handle)` in `equation_sources.expressions` (`:354-356`); records exposure/partition on each lowered expression (`:360-378`).
4. How it decides.
   - Reuse of a hoisted match subject: `lowerer.match_subject_temp(expression)` keyed on syntax `ExpressionHandle` (`:344`). Identity.
   - Saved-subject binding: stack search `expression_match_captures.iter().rev().find(syntax_capture == .. && syntax_head == ..)` (`:172-179`). Identity (handles); error text includes a debug dump (`:183-189`).
   - Exposure: a constant-extent probe root marked public makes `PublicInterface`, else `PrivateImplementation` (`:392-401`).
   - Applied-requirement call qualifier: `base_name.as_str().split("::")` becomes a name path (`:840-845`). Spelling.
5. Cases by shape. `Call` with `TypeExpression` receiver and `target_is_static` is lowered as an applied requirement qualifier; "Only a generic trait path qualifies" (`:822-826`, `:832-838`). Atomic: result custody must match the ordering plan (`:479-485`).
6. Same job elsewhere. none seen.
7. Unused. `lower_private_expression_into_table`: used by `statement.rs:35`, `:198`, `:546`, `:563`.
8. Stated limits. none. The code at `:512-513` says `unreachable!("binary expressions use the stack-bounded lowering path")` (grepped `stack-bounded`: only this site), but `lower_binary_expression_into_table` (`:421-436`) recurses through `lower_expression_into_table` on both operands (`:426-427`), so it is recursive, not stack-bounded.
9. Fail-open risks. none seen.

---

## lowering/item.rs (479 lines)

1. Purpose. Dispatches each root item kind to its lowering function, wrapping it in a callable scope, an authored-exposure context and type-input tracking.
2. Shape. `lower_item` (`:34`), `lower_item_with_exposure` (`:103`), `item_expression_exposure` (`:404`), `lower_closed_machine_row` (`:428`).
3. In/out. Pushes onto `symbol_resolved_trees.{type_aliases, data_definitions, domain_definitions, traits, conformances, measures, operators, propositions}`, `roots.const_declarations`, `machines` (via `lower_machine_into`). Records `declaration_inputs` for conformances and traits (`:80-93`) and `equation_sources.data_declarations` (`:94-98`).
4. How it decides.
   - A bodyless bare machine with no target counts as an operator scope owner (`:58-66`), keyed on shape (`is_bare_bodyless_signature`).
   - Constants: `pending` evaluation when not `Complete` mode and the initializer requires evaluation and has pending leaves (`:287-303`); `evaluated_value = None` for pending/target-observation/destination-proof constants, a hard error for public constants without canonical identity, else `.ok()` (silently drops an evaluation failure) (`:316-352`).
   - Conformance rows: `edge_trait` is found by matching `clause.requirement` spelling to the machine name (`:151-159`). Spelling-keyed. Row names are `format!("{conformance_name}::{declaring_trait}::{requirement}")` (`:438-452`), so realization machines are named by joined spellings.
6. Same job elsewhere. none seen.
5. Cases by shape. `Item::Use(File)`, `Capability`, `Package` are dropped (`:398`). Conformance members: Machine, Synthesized, TraitDefault, Selection (`:148-243`).
7. Unused. none.
8. Stated limits. none.
9. Fail-open risks. `evaluated_value ... .ok()` at `:346-352`: an evaluation error on a non-public, non-pending const yields `None` rather than an error. Wrong program: a private const whose canonical value fails to evaluate is not reported here. Classification: open (consumer not traced).

---

## lowering/lowerer.rs (628 lines)

1. Purpose. The `Lowerer` working-state struct: the carrier under construction plus pending side lists that later phases (symbol assignment, selection collection, service-reach binding) consume.
2. Shape. `ConstResolutionMode {Complete, ArgumentSelection, InitializerSelection}` (`:106`), `Lowerer` (`:112-250`, about 45 fields), `RootWatermarks` (`:252`) and `capture` (`:268`), `BaseSeed` (`:286`), `Lowerer::new` (`:292`), `seed_resolved_base` (`:351`), `with_authored_expression_exposure` (`:463`), `match_subject_temp` / `record_match_subject_temp` (`:478`, `:485`), `next_hoist_name` (`:495`), `into_trees` (`:530`), `pending_signature_reaches` (`:558`), `PendingSignatureLocation` (`:611`).
3. In/out. `seed_resolved_base` clones the whole base (`self.symbol_resolved_trees = base.as_ref().clone()`, `:439`) after checking the retained source map is a prefix of the new one (`:355-369`) and rebuilding service-reach side lists (`:374-436`). `into_trees` rebuilds tables from roots with `SymbolResolvedTrees::with_roots` (`:545`).
4. How it decides. Seeding refuses when sources are missing or do not retain the base frontier (`:355-369`). Hoist names are `__hoist_{counter}` (`:496`).
5. Cases by shape. none.
6. Same job elsewhere. `match_subject_temps` is a `std::collections::HashMap` keyed on `ExpressionHandle` (`:237-238`); the same file also uses `arena::BuildIndexHasher` maps (`:374-381`).
7. Unused / dead. Three fields `current_state_parameter_names` (`:226`), `current_state_parameters` (`:245`) and `current_state_return_type` (`:247`) are only declared (`:226`, `:245`, `:247`), initialised (`:335`, `:340`, `:341`), and saved/restored in `with_state_scope` (`state.rs:311-322`); grepped `current_state_parameter_names|current_state_parameters|current_state_return_type` across `source-rust/`: 14 matching lines, none reading them. The comments (`:219-225`, `:240-247`) say transition classification and hoist gates read them. `resolved_lexical_inputs` (`:172-173`) is `allow(dead_code)` outside tests and written at `names/mod.rs:600` (grepped: 3 lines total).
8. Stated limits. none.
9. Fail-open risks. `structural_type_equations_pending: true` default (`:307`) disables `validate_materialized_type_equations` in `type_reference.rs:84-90` for any lowerer not cleared by `resolver.rs:129`. Stated: "Only the complete route clears this." (`:133-134`).

---

## lowering/machine.rs (1065 lines)

1. Purpose. Lowers a machine (states, type parameters, trait conformances, ranking/termination plan, service reaches, supply mode) and completes deferred bodies.
2. Shape. Inline `mod type_equations::lower` (`:14-154`, retains classified equations as `DeclaredTypeEquation`), `MachineBodyScope` (`:191`), `with_machine_body_scope` (`:198`), `PendingMachineBody {machine_ordinal, source: syntax Machine, states: Vec<(Handle<State>, StateHandle)>}` (`:230`), `machine_interface_exposure` (`:260`), `lower_machine_into` (`:276`), `lower_lexical_machine_parts` (`:300-580`), `complete_machine_body` (`:586`), `lower_pending_lexical_inputs` (test, `:623`), `with_pending_machine_scope` (`:649`), `compiler_selection_partition` (`:688`), `lower_generic_conformance_bounds` (`:728`), `build_termination_plan` (`:758`), `render_ranked_subject` (`:817`), `authored_ranking_view` (`:850`), `external_binding_identity` (`:947`), `lower_machine_states` (`:968`), `sealed_intrinsic` (`:1016`), `is_sealed_float_provider_member` (`:1034`), `compiler_owned_boundary` (`:1056`).
3. In/out. Pushes a `Machine` onto `symbol_resolved_trees.machines` (`:509-569`); when `defer_machine_bodies && !machine.bodyless` pushes a `PendingMachineBody` and marks `MachineBodyStatus::Deferred` (`:348-368`); lowers nested machines recursively with `lexical_parent` (`:571-578`).
4. How it decides. The supply mode is a cascade (`:430-494`): compiler-provided bare bodyless signature; target-family interface -> Requirement; token-bearing boundary or sealed `FloatOperationRequirement` -> TopLevelRequirement; inferred float binding -> ExternalRealization; bodyless with `via` -> ExternalRealization; bodyless boundary -> Boundary or AdmissionClaim by whether an `ensures` has facts; boundary -> Boundary; else CheckedBody. Sealed intrinsic is looked up by `(source file, machine name spelling)` through `source_symbols::sealed_intrinsic_at(file, name)` (`:1016-1026`). Selection partition: a machine with non-empty `default_body_trait`, or non-source-backed with one compiler-made `satisfies` clause, gets a compiler ordinal (`:688-726`); `Identifier::is_source_backed()`.
5. Cases by shape. `is_sealed_float_provider_member` ("only in its exact bodyless single-relation shape", `:1029-1031`): bodyless, boundary, no attached data, exactly one `satisfies` edge with no `via`. `render_ranked_subject` renders Integer, bare Name, Member, `a - b`, otherwise `"value"` (`:821-842`); it is documented as private witness text for diagnostics (`:813-816`). Nested machines: "a nested machine is a private independent lexical declaration" (`:312-314`).
6. Same job elsewhere. `sealed_intrinsic` (`:1016-1026`) and the custody lookup in `machine_supply.rs:44-48` repeat the same `sources.get(source_id) -> sealed_intrinsic_at(file, name)` chain (grepped `sealed_intrinsic_at`: 6 occurrences, 4 files).
7. Unused. `lower_generic_conformance_bounds` is shared (callers `data.rs:235`, `type_aliases.rs:20`, `trait_definition.rs:30`, here). `with_pending_machine_scope` iterates `machines.iter().nth(ordinal)` (`:655-660`), O(n) per body.
8. Stated limits. "Neither stage establishes the checked termination guarantee." (`:755-756`); "Typed validation must still authenticate ..." (`:1031-1032`).
9. Fail-open risks. `build_termination_plan` leaves `checked_summary = NoGuarantee` (`:808`), so it records no guarantee unless authored; fail-closed. `render_ranked_subject` default `"value"` (`:841`) affects only diagnostic text (stated `:813-816`). none else seen.

---

## lowering/machine_supply.rs (871 lines)

1. Purpose. Decides whether a bodyless, non-boundary machine is a legitimate compiler-supplied primitive by matching it against closed catalogs and sealed toolchain source custody; refuses everything else with "has no body" guidance.
2. Shape. `is_token_bearing_boundary_signature` (`:16`), `is_bare_bodyless_signature` (`:28`), `require_catalog_signature` (`:37`), `CatalogFamily {Collection, ByteObservation, AtomicFence, FloatProjection, FloatSemantics, RankingView}` (`:122`), `CatalogFamily::naming` (`:141`), `primitive` (`:155`), `require_exact_ranking_view_row` (`:173`), `require_exact_semantic_row` (`:242`), `require_exact_byte_observation_row` (`:316`), `require_exact_atomic_fence_row` (`:409`). Tests `:438-871`.
3. In/out. Reads the machine syntax and `lowerer.sources` for the sealed custody; returns `Ok(())` or a diagnostic. Mutates nothing.
4. How it decides. Family comes from sealed custody first (`:49-60`), else from `(namespace, leaf)` spellings: `ATOMIC_FENCE_NAMESPACE`/name, `FloatProjectionOperation::from_source_identity`, `FloatSemanticOperation::names_a_row`, `RankingViewId::from_catalog_declaration` (`:141-153`). Acceptance requires `custody == Some(CompilerPrimitive(family.primitive()))` (`:61-63`), so a spelling alone selects nothing ("A family is selected by exact path; the leaf spelling alone selects nothing", `:118-119`). Header rows are checked by type spellings: `"u8"`, `"bool"`, `"FenceOrdering"` (`:398-402`, `:421`), `FloatSemanticValueKind::from_spelling` (`:268`), ranking `named_spelling` (`:195-202`).
5. Cases by shape. Closed list at `:115-138`. `require_exact_byte_observation_row` is a hand-written conjunction of ~35 header predicates (`:326-364`), with exactly one lifetime, one parameter `@'a [u8]` relevant, result `bool` (`:367-405`). Atomic fence: exactly `public machine fence(ordering: FenceOrdering);` (`:433`).
6. Same job elsewhere. `machine.rs:1016-1026` (see above).
7. Unused. `is_token_bearing_boundary_signature` callers: `machine.rs:266`, `:444`; `is_bare_bodyless_signature`: `item.rs:61`, `machine.rs:318`.
8. Stated limits. none beyond the shape rules.
9. Fail-open risks. `CatalogFamily::Collection(_) => {}` and `FloatProjection => {}` (`:97-98`) run no header check here. Stated in the comment at `:94-96`: "proceeds through the complete generic machine owner". Classification: stated; wrong program (a sealed-custody file with a drifted header for those two families) is left to typing.

---

## lowering/mathematical.rs (148 lines)

1. Purpose. Lowers proof-surface `let` / `boundary let` definitions (binders, parameters, a dependent result type, transparent or assumed body).
2. Shape. `lower_mathematical_definitions` (`:34`), `lower_mathematical_definition` (`:63`), `lower_mathematical_type` (`:112`).
3. In/out. Reads `syntax.root_mathematical_definition_handles()`; pushes to `roots.mathematical_definitions` and declaration arenas.
4. How it decides. none; arrow/application/ordinary node kinds are copied.
5. Cases by shape. `MathematicalTypeNode::{Ordinary, Arrow, Application}` (`:117-141`).
6-8. Same job elsewhere: none. Unused: called once, `resolver.rs:175` (grepped). Limits: "binder kinds stay the authored `Type`/`Const`/`Value` carriers" (`:9-11`).
9. Fail-open risks. none seen.

---

## lowering/measure.rs (48 lines)

1. Purpose. Lowers a `measure` definition (name path, optional parameter, optional return type, ranking body expressions).
2. Shape. `lower_measure_definition` (`:14`). 3. Reads `measure.body` expressions and pushes lowered handles (`:33-38`). 4-9. No decisions beyond valid-handle checks (`:23`, `:29`); none seen.

---

## lowering/mod.rs (46 lines)

Module list and `lower_items` (`:38-46`), which lowers every root item in source order and maps each single `Diagnostic` to a one-element vec (`:43`). No decisions.

---

## lowering/name.rs (52 lines)

`lower_name` (`:10`): `DiagnosticName::from_str(name.as_str(), name.source_span())`; `lower_names` (`:18`), `lower_operator_path_name` (`:25`), `lower_name_members` (`:42`). Every lowered name is spelling plus span; this is the only identifier-to-name conversion. Callers: used throughout `lowering/` (not counted).

---

## lowering/operand_owners.rs (106 lines)

1. Purpose. Gives type references copied from a supplying declaration their `OperandLifetimeOwner` handle, bridging "syntax root input" to "declaration symbol" once symbols exist.
2. Shape. `DeclarationInput {Conformance(usize), Trait(usize)}` (`:14`), `retain` (`:31`), `bindings` (`:62`), `bind_program` (`:87`).
3. In/out. `retain` linear-scans `lowerer.supplied_operand_owners` and `declaration_inputs` (`:35-47`), inserts an `OperandLifetimeOwner` (`:48-53`); `bind_program` writes `declaration` symbols into the owners (`:98-103`) and errors on an invalid symbol (`:93-97`).
4. How it decides. By `ItemHandle` identity; "no name, source span or matching type shape reconstructs the association" (`:4-5`). An input with no declaration entry yields `declaration = default` (invalid) (`:42-47`).
5-8. none. 9. Fail-open: `retain` returns an owner with an invalid declaration when `declaration_inputs` lacks the item (`:42-47`); `bind_program` later refuses unbound owners only for owners listed in `bindings`, which requires an entry (`:69-77`); an owner created for an input that is not in `declaration_inputs` is in `supplied_operand_owners`, so `bindings()` errors "supplied operand lost its exact declaration input" (`:73-77`). Classification: traced; fail-closed.

---

## lowering/operator.rs (37 lines)

`lower_operator_definition` (`:15`): copies flags, name path, type parameters, state parameters, return type, contracts, `spelling`, `token_count`. No decisions. Callers: `item.rs:273`, `domain.rs:168`.

---

## lowering/proposition.rs (99 lines)

1. Purpose. Lowers `proposition` definitions: binders, parameters, primitive / witness / transparent body.
2. Shape. `lower_proposition_definition` (`:21`), `lower_proposition_binders` (`:51`).
4. How it decides. Binder kinds accepted: Type, Const, Machine. Refused with messages: Value (`:63-68`), Implementation (`:70-75`), Proposition (`:76-81`).
9. Fail-open risks. none seen.

---

## lowering/state.rs (906 lines)

1. Purpose. Lowers machine states: signatures, parameters, contracts (including outcome-specific `ensures` over a result sum), and statement bodies (with run-of-transition reordering).
2. Shape. Inline `mod lexical_inputs` (`:11-159`): `StateLexicalInputs` (`:37`), `lower_local_declaration` (`:45`), `lower_state_lexical_inputs` (test, `:106`). Outside: `lower_state_node` (`:194`), `lower_state_parts` (`:212`), `lower_state_body` (`:256`), `with_state_scope` (`:272`), `lower_state_signature_node` (`:326`), `StateSignatureParts` (`:364`), `lower_state_signature_parts` (`:387`), `LoweredStateSignature` (`:490`), `lower_signature_invokes` (`:498`), `lower_effect_references` (`:518`), `lower_signature_contracts` (`:533`), `lower_machine_signature_contracts` (`:541`), `ResultSumContractContext` (`:624`), `lower_signature_contracts_with_result_sum` (`:629`), `lower_state_statements` (`:727`), `lower_statement_into_pending` (`:766`), `lower_state_parameters` (`:826`), `lower_state_parameter` (`:857`), `reference_struct_parameter_names` (`:882`).
3. In/out. Writes `declarations.state_statements`, `state_parameters`, `signature_contracts`; records `sources.statements` links `(syntax StatementHandle, resolved Handle)` keyed by pending-list offsets (`:755-761`); queues `PendingOutcomeSpecificContract` (`:711-720`).
4. How it decides.
   - Outcome-specific `ensures`: the result data is found by `data.name.as_str() == result_data_name.as_str()` among all `data_definitions`, filtered by `source_reference_can_see_declaration`, then `min_by_key((different_source_file, strata_separate))` (`:578-596`); the case must equal a variant spelling (`:661-671`); the path must be exactly `Result::Case` (`:647-652`). Spelling-keyed, ties broken by minimum key (first minimal element).
   - Reference-struct parameters: a shared (non-exclusive) reference whose referee is `TypeReference::Named` (`:891-903`); computed only for boundary machines (`:280-288`). Names stored as `String`s.
   - Transition runs: hoisted statements are inserted before the first transition of a run (`:783-808`).
5. Cases by shape. `lower_machine_signature_contracts`: rows only on "a top-level machine result" (`:643-646`), result must be `Named` or `Generic` (`:569-577`); "outcome-specific ensures requires a declared nominal sum result type". `lower_state_lexical_inputs` handles `LocalData` statements only.
6. Same job elsewhere. `names/contracts.rs:624-682` re-finds the data and case by name (`finalize_outcome_specific_contract_symbols`) after symbols exist.
7. Unused. `lower_state_lexical_inputs`: test/test-support only; its only caller is `machine.rs:634` (also test-only), reached via `names/mod.rs:570`. `reference_struct_parameter_names`: 1 caller (`:285`).
8. Stated limits. "Typing refuses ..." not here. The doc at `:877-880` ties `reference_struct_parameter_names` to `table: @EfiSystemTable`.
9. Fail-open risks. `with_state_scope` stores `current_state_parameter_names`, `current_state_parameters`, `current_state_return_type` that nothing reads (see lowerer.rs item 7); no acceptance effect. The name-and-visibility data pick at `:578-596` can select a different declaration of the same name; `names/contracts.rs:630-637` re-resolves by name and source span. Classification: open.

---

## lowering/statement.rs (770 lines)

1. Purpose. Lowers one syntax statement into one or more resolved statements, inserting synthetic `let __hoist_N` bindings for runtime-indexed operands and unconditional value calls; lowers block values.
2. Shape. Inline `mod statement_nodes` (`:29-679`): `lower_statement_node` (`:74`), `evidence_forwarding_names` (`:496`), `bare_syntax_name` (`:510`), `set_expression` (`:533`), `lower_statement_expression` (`:541`), `lower_statement_expressions` (`:549`), `lower_transition_guard_node` (`:576`), `lower_transition_target_node` (`:589`), `lower_statement_path_members` (`:661`). Outside: `lower_block_value` (`:709`), `lower_statement_handle` (`:756`).
3. In/out. Returns `Vec<Statement>` where hoisted lets precede the rewritten statement; pushes `EvidenceForwarding` rows into `symbol_resolved_trees.evidence_forwardings` (`:142-157`); mutates `machine_body.evidence_names` (`:262-269`).
4. How it decides.
   - Evidence forwarding: an assignment is removed from the body and recorded as `EvidenceForwarding` when both sides are bare single names and either name equals some `evidence_names` entry (`:134-159`, `:496-508`). Spelling-keyed (`Vec<String>`).
   - Proof output binding: `output_field.as_str() == "value"` and `binding.as_str() != "_"` (`:254-257`, `:263-264`). Spelling-keyed.
   - A `Call` statement is lowered to an `Evaluate` expression when the call target spelling names a build declaration operation (`BuildOperation::from_call_target(call.target.as_str())`, with `NativeBinding` excluded, `:180-186`) or a static call with no runtime receiver (`:186-193`).
   - Hoisting is conditioned by `unconditional = !has_preceding_transition && patterns.is_empty() && guard is Always` (`:413-415`).
5. Cases by shape. `AuthoredAssembly` and `Dispatch` nodes are errors here ("reached statement lowering before preparation", `:82-90`, `:359-362`). A destructure transition pattern that does not come back as `MatchPattern::Destructure` hits `unreachable!()` (`:385`).
6. Same job elsewhere. `hoist_*` family in `lowering/statement/` (see below); `lower_block_value` (`:709`) reuses `lower_statement_handle` and a block tail hoist (`:731-740`).
7. Unused. `lower_statement_expression` is `pub(super)`; used in `assembly.rs` and here. `__hoist_` grepped over `source-rust/`: 8 occurrences in 4 files, all in this lowering tree (`lowerer.rs` 3, `statement.rs` 1 (comment), `hoist_temps.rs` 3, `indexed_read_hoisting.rs` 1 (comment)); only `lowerer.rs:495-496` mints the name.
8. Stated limits. "A guarded arm's calls are never moved" (`:7`); comment at `:698` says "locals are bound by ORDER + NAME".
9. Fail-open risks. The evidence-forwarding branch returns `Ok(Vec::new())` (`:158`): the statement vanishes. Wrong program: an assignment such as `a = b` inside a machine with a capability-contract binding named `a` or `b` (or a proof output binding) is not lowered as an assignment, including if `a`/`b` is an unrelated local spelled the same. Classification: open (consumer of `evidence_forwardings` not traced; grepped `evidence_forwarding_names|bare_syntax_name`: 5 matches, all here).

---

## lowering/statement/assembly.rs (106 lines)

1. Purpose. Lowers a syntax assembly control-flow graph (blocks, requires/ensures, instructions, jump exits, scratch slots) into the resolved form.
2. Shape. `lower` (`:10`). 3. Lowers each instruction with `lower_statement_node`, each expression with `lower_statement_expression`; allocates blocks, then patches `Jump`/`ZeroFlag` exits to block handles (`:62-99`).
4. How it decides. Jump target block = the first block whose `label.as_str()` equals the jump label (`:80-91`); a missing label is an error "assembly jump has no local label" (`:87-90`). Spelling-keyed; duplicate labels resolve to the first.
5-9. Scratch slots limited to `ASM_SCRATCH_SLOT_COUNT` (`:68-74`). Fail-open: duplicate block labels silently choose the first (`:83-86`); open.

---

## lowering/statement/hoist_temps.rs (124 lines)

1. Purpose. The constructor for synthetic hoisted locals: pushes `let __hoist_N = <initial>` and returns a name reference expression.
2. Shape. `hoist_into_temp` (`:24`), `hoist_operand_into_temp` (`:37`), `mint_hoist_temp` (`:57`), `reference_temp` (`:110`).
3. In/out. Appends a `LocalData` with `type_is_inferred = annotation.is_none()` (`:82-96`). A match-subject temp shared by sibling arms is looked up through `lowerer.match_subject_temp(subject)` and reused instead of minted (`:65-67`).
4. How it decides. Sharing is by syntax subject `ExpressionHandle` (`:61`, `:78-80`).
6. Same job elsewhere. Header claim "The one constructor for synthetic `let __hoist_N` temps" (`:2`): grepped `__hoist_` and `next_hoist_name` over `source-rust/`: `next_hoist_name` has one caller (`hoist_temps.rs:77`) and the format literal one site (`lowerer.rs:496`), so the claim holds for this tree.
9. Fail-open risks. none seen.

---

## lowering/statement/indexed_read_hoisting.rs (273 lines)

1. Purpose. Hoists runtime-indexed reads that sit in operand position (children of binary, unary, cast, range, membership) into synthetic locals.
2. Shape. `OperandHoisting {Value, Guard, Computation}` (`:18`), `hoist_operand_indexed_reads` (`:35`), `rewrite_children` (`:49`), `is_hoistable_value_cast_call` (`:170`), `is_integer_embedding_call` (`:186`), `hoist_child` (`:203`), `is_member_of_runtime_indexed_read` (`:232`), `is_runtime_indexed_read` (`:264`).
4. How it decides. A read is "runtime-indexed" when it is `Indexed` and the index is not an `Integer` literal node (`:264-273`). `is_member_of_runtime_indexed_read` requires the collection to be `self.<field>` or a two-segment name whose first segment `is_self_receiver()` (`:247-260`). `is_integer_embedding_call` compares `call.target.as_str() == BuiltinFunction::IntegerEmbed.name()` and, when the symbol is already valid, also its builtin-function identity (`:186-195`); at lowering time the symbol is invalid, so spelling alone decides. For `And`/`Or`, the right operand is not hoisted (`:59-63`).
5. Cases by shape. `rewrite_children` handles Binary, Unary, Cast, Membership, Borrow (no-op), Range, Indexed; everything else `_ => {}` (`:161`).
7. Callers. `is_integer_embedding_call` + `expression_contains` grepped: 15 occurrences across 6 files (the walker is defined in `value_call_hoisting.rs:138`, used at `:79`, `place_operand_capture.rs:197`).
9. Fail-open risks. A user machine named like the integer-embed builtin, called without a receiver, is treated as proof-only at hoist time because the symbol is not yet resolved (`:186-195`); comment `:178-185` says "the later proof-only gate requires the exact builtin symbol and rejects ... any authored same-spelled replacement". Classification: stated.

---

## lowering/statement/match_subject_hoisting.rs (242 lines)

1. Purpose. Hoists membership and comparison match subjects once so all arms of a match share one subject (needed for exhaustiveness grouping), and hoists reads through shared-reference struct parameters.
2. Shape. `borrow_membership_subject` (`:25`), `hoist_membership_match_subject` (`:38`), `hoist_comparison_match_subject` (`:96`), `is_reference_struct_parameter_member` (`:176`), `subject_contains_hoistable` (`:200`), `guard_hoists_operands` (`:232`).
4. How it decides. Shared-subject key is the syntax subject handle (`:62`, `:137`). `is_reference_struct_parameter_member` matches the receiver's single-segment name string against `lowerer.reference_struct_parameters` (`:190-197`). Spelling-keyed: a local with the same name as a reference parameter is also treated as one. `guard_hoists_operands` is true only for a `Binary` root (`:232-242`).
5. Cases by shape. `hoist_comparison_match_subject` handles only `==`/`!=` whose right side is a Boolean literal and left side a comparison containing a hoistable (`:103-127`); the temp is typed by the name `bool` (`DiagnosticName::generated("bool")`, symbol invalid, `:139-143`).
7. Callers. `is_reference_struct_parameter_member` grepped: 7 occurrences in 3 files (statement.rs 3, indexed_read_hoisting 2, here 2).
8. Stated limits. The header comment of `is_reference_struct_parameter_member` ties it to "the flat fold reads frame garbage" (`:169-175`).
9. Fail-open risks. none seen (a wrong name match only changes hoisting).

---

## lowering/statement/place_operand_capture.rs (217 lines)

1. Purpose. Evaluates address operands (collection, selector) of reads, writes and loans once into inferred locals, in order, without extracting the pointee.
2. Shape. `SelectorCapture {BeforeWrites, Every}` (`:33`), `capture_reference_initializer_selectors` (`:44`), `capture_address_operands` (`:59`), `capture_subject_address_operands` (`:74`), `capture_formation` (`:95`), `capture_operands` (`:115`), `capture_selector` (`:162`), `operand_may_write` (`:196`), `is_numeric_literal_expression` (`:202`).
4. How it decides. `operand_may_write` is true when the operand contains a `Call` or `Atomic` node (`:196-200`); a numeric literal selector (Integer/Float through Unary/Binary) is never captured (`:168-170`, `:202-217`).
5. Cases by shape. `capture_operands` handles Name, Unary deref, Member, Indexed, Borrow, recast Cast; anything else is hoisted when capture is on or may write (`:122-159`). `capture_formation` handles Borrow and recast Cast only (`:101-112`).
7. Callers. `capture_address_operands`: statement.rs `:163`, indexed_read_hoisting `:159`, here.
9. Fail-open risks. none seen.

---

## lowering/statement/value_call_hoisting.rs (209 lines)

1. Purpose. In a guard comparison like `self.next() == expected`, evaluates the user call once into a shared local so both true/false arms reuse the result; also holds `expression_contains`.
2. Shape. `hoist_scalar_value_call_comparison` (`:19`), `expression_contains` (`:138`).
4. How it decides. Exactly one side of a comparison is a user call (calls to the integer-embed builtin excluded by spelling plus symbol, `:65-70`); the other side contains no call (`:79-83`); when the call is on the right the left must be a Boolean/Float/NaN/Integer literal (`:84-94`).
5. Cases by shape. Operators `==, !=, >, >=, <, <=` only (`:53-61`); the Boolean-compare wrapper `x == true` is peeled in a loop (`:30-52`).
6. Same job elsewhere. `expression_contains` is a general resolved-expression walker; see uses.rs item 6.
7. Callers. `expression_contains`: 5 occurrences in this file, 2 in `place_operand_capture.rs` (grepped).
9. Fail-open risks. none seen.

---

## lowering/trait_definition.rs (230 lines)

`lower_trait_definition` (`:21`), `lower_trait_requirements` (`:110`), `lower_trait_requirement_node` (`:155`), `lower_trait_machine_signatures` (`:188`). Lowers parents, `requires` names, conformance bounds, machine signatures, `refines` and refinement clauses; binds each signature's runtime-callable scope and queues its pending service reach (`:202-226`). A requirement reference must be `Named` or `Generic`, else "expected a named declaration, optionally with static arguments" (`:174-178`). Decision keyed on node shape. `lower_trait_requirement_node` has callers in `state.rs:528` (`lower_effect_references`) and here. Fail-open: none seen.

---

## lowering/type_aliases.rs (45 lines)

`lower_type_alias_definition` (`:13`): lowers parameters, bounds, and the expansion with `structural_type_equations_pending` forced true (`:21-25`); attaches the alias body found by `selection.alias_body(alias.name.source_span())` (source-span keyed, `:34-42`), erroring "missing authored type alias body" when absent. Fail-open: none seen.

---

## lowering/type_inputs.rs (106 lines)

1. Purpose. Records which declaration root each pending const-argument type input belongs to, using root-count watermarks (not names or spans), so typed probes can find their owners.
2. Shape. `LoweredTypeInputs {start, end: RootWatermarks, arguments, owner}` (`:15`), `TypeInputScope::{new, finish}` (`:26-57`), `bind` (`:59`).
4. How it decides. `bind` expands each scope to the declarations created between its watermarks (macro over 11 root kinds, `:75-98`); a scope that produced no declaration is an error (`:99-103`); an unbound owner symbol is an error (`:79-81`).
9. Fail-open risks. none seen; `wire_schemas` is in `RootWatermarks` (`lowerer.rs:264`) but not in `bind_roots!` (`:86-98`); schemas are derived, not authored.

---

## lowering/type_reference.rs (670 lines)

1. Purpose. Lowers syntax type references (named, generic, reference, slice, array, constrained, callable, const expression) and the const-argument custody (origins, selections, builtin operators, receiving-application slots) they carry.
2. Shape. `lower_type_reference_handle` (`:25`), `lower_type_reference_contents` (`:76`), `lower_alias_application` (`:261`), `lower_generic_application_origin` (`:304`), `lower_type_reference_node` (`:320`), `lower_fixed_array_length` (`:463`), `lower_type_reference_child` (`:500`), `lower_child_type_references` (`:512`), `lower_type_constraint_handles` (`:532`), `lower_indexed_application_arguments` (`:591`), `retain_const_argument_slots` (`:631`).
4. How it decides.
   - Alias use: wraps the expansion in `TypeReference::AliasUse` per alias application row (`:57-72`).
   - Materialized-equation check skipped while `structural_type_equations_pending` (`:84-90`).
   - Const-argument slots: a pending selection is linked to a slot when `pending.origin == *selected` (`:657`), a `ConstArgumentOrigin` value comparison; ordinal by argument position. Identity of origins.
   - Array extent: if a normalization has no materialized expression and the type reference is `Named`, the name is parsed as `usize`, else `Expression(child)` (`:471-486`).
5. Cases by shape. `StaticSelection` is an error ("a scoped type selection must resolve to an admitted declared type before typed lowering", `:392-394`). Callable types allocate a contract and bind a scope (`:401-447`).
6. Same job elsewhere. `lower_indexed_application_arguments` is the "one operation" (`:584-590`) for data applications, domain constraints and membership facts; grepped `lower_indexed_application_arguments|retain_const_argument_slots`: 7 occurrences across `domain.rs` (1) and this file (6): callers are `type_reference.rs` (alias application, generic node, domain constraint) and `domain.rs:238`.
9. Fail-open risks. `validate_materialized_type_equations` skipped in the pending state (stated, see lowerer.rs).

---

## lowering/wire.rs (73 lines)

1. Purpose. Derives a wire schema (field list with numbers) for a data definition only when the generated codec can check it end to end.
2. Shape. `derive_wire_schema` (`:21`). 4. Withheld unless no type/lifetime parameters, default properties, no `requires` facts, supply mode `CheckedShape`, only fields with `#n` identities, and (fields non-empty or retired identities non-empty) (`:22-39`). Caller: `item.rs:119`.
9. Fail-open risks. none seen (it withholds, "Those declarations still lower normally", `:15-18`).

---

## names/contracts.rs (682 lines)

1. Purpose. Assigns symbols to value references, call targets and static arguments inside contract clauses (`requires`/`ensures`/`crashes`) of machines, states, traits, operators, machine-parameter signatures and runtime-callable contracts, in the owning callable's scope.
2. Shape. `assign_contract_reference_symbols` (`:37`), `assign_callable_value_symbols` (`:305`), `assign_contract_call_symbols` (`:377`), `finalize_outcome_specific_contract_symbols` (`:624`).
3. In/out. Mutates the `ExpressionTable`, child type references, declared type equations of `SymbolResolvedTrees` (disjoint field borrows, `:60-73`); sets `Call.target_symbol` and static argument symbols (`:513-525`); sets `Name.reserved_result_of` (`:596-599`).
4. How it decides.
   - A one-segment name `result` is the reserved result when `language_core::is_reserved_result(name)` and `signature_reserves_result(parameter names)` (`:584-601`). Source comment: "the source rule is a spelling, decided here once. Record the decision as the contract owner's identity" (`:590-595`). Grepped `is_reserved_result`: 9 occurrences in 7 files (also `domain_facts.rs:956`).
   - Call target: `normalize_expression_module_call` first, else `resolve_expression_table_call_target_symbol` (`:488-503`).
   - Machine data definition lookup by `definition.symbol == machine.attached_data_symbol` linear (`:90-92`).
   - Outcome contract finalization: top-level data lookup by name and source span (`find_top_level_by_name_and_kinds_from_source`, `:630-637`), then variant by name equality (`:648-655`).
5. Cases by shape. Contract expressions: `Block` and `SavedSubject` are skipped (`:407-408`, `:430`) with comment "Typing refuses a block value in a contract"; the other node kinds are walked (`:406-617`).
6. Same job elsewhere. `domain_facts.rs` `ProofExpressionContext::assign` (`:711-1029`) is a parallel walker for declaration facts (see below).
9. Fail-open risks. `Block(_) => {}` (`:408`): stated ("Typing refuses a block value in a contract"), classification stated. `.unwrap_or_default()` at `:329` (contract result data) leaves an invalid symbol; wrong program: a result type with no nominal owner leaves `contract_result_data` invalid, so `result.field` stays unresolved. Open.

---

## names/domain_facts.rs (1069 lines)

1. Purpose. Assigns symbols inside domain and data declaration facts (`requires` expressions and memberships) and computes the `zero_gated` flag of each data definition.
2. Shape. `mod case_members` (`:35-144`: `CaseSelection`, `collect_case_selections`, `assign_guarded_case_member`, `same_case_subject`); `assign_domain_fact_symbols` (`:146`), `update_data_zero_gates` (`:365`), `zero_fold` (`:439`), `fact_expression_is_constant_false` (`:489`), `resolved_domain_byte_predicate` (`:500`), `ProofExpressionContext` (`:570`) with `assign_static_argument`, `assign_lexical_subject`, `assign` (`:604-1030`), `resolve_domain_symbol` (`:1038`), `proof_fact_membership_domain` (`:1056`).
3. In/out. Builds `proof_fact_scopes` of `(facts span, local_symbols: Vec<(String, SymbolHandle)>, type_parameters, logical_data_subject)` for each domain, signature contract, data definition and variant (`:184-282`); fills symbols into the expression table and facts; writes `definition.zero_gated` (`:424-436`).
4. How it decides.
   - Lexical subject binding: first path segment spelling matched against `local_symbols` pairs; one match binds, several leave it unresolved with `UnresolvedName::Ambiguous` (`:672-697`). Spelling-keyed.
   - `self`: the string `"self"` is a local in data scopes (`:243`) and the logical receiver of `self.field` (`:886`).
   - Descriptor parameters: a Value parameter pairs with a data field of the same name (`zip` over parameters and members) (`:213-222`).
   - Guarded case members: a selection only if exactly one variant is selected for the receiver subject, the variant has one field of that name; subject identity is `same_case_subject`: same head symbol, same resolved path symbols, or same member symbol chain (`:114-142`).
   - Domain: `membership_domain_lookup` then `membership_selection`; Domain wins, a Case or None or error gives invalid (`:1056-1069`).
   - Zero gate: `zero_fold` evaluates a fragment at the zero value (field names read 0, integer literals, `+ - *`, comparisons, `&&`, `||`, collection measures by spelling `CollectionMeasure::from_authored_spelling`) (`:439-481`); `expressions_gate_zero` = any fact folding to 0 or not foldable (`is_none_or`) (`:390-392`); a variant fact that is literally `0`/`false` gates (`:393-413`, `:489-498`); membership domains gate unless the domain's single fact is a sealed byte-observation call whose predicate holds for the empty byte slice (`:380-382`, `:500-562`).
5. Cases by shape. `resolved_domain_byte_predicate` accepts: domain with exactly one `Expression` fact that is a call with valid target, no machine/evidence arguments, sealed `ByteObservation` custody, one `@self`/`@self[..]` shared-borrow argument (`:505-561`).
6. Same job elsewhere. `contracts.rs::assign_contract_call_symbols` (`names/contracts.rs:377`) handles the same node kinds for contract clauses with different scope tracking.
9. Fail-open risks. `.expect("one alias-resolution set per domain")` (`:176`) and `.expect("one zero result per data definition")` (`:432`) are length-sync assertions. `zero_gated` errs toward gated (an unevaluable fact, an unknown domain, or an unknown predicate gate zero), i.e. fail-closed (comment `:360-364`: "conservative declaration gate").

---

## names/expression_paths.rs (689 lines)

1. Purpose. Symbol lookup for receiver paths, member accesses and call targets in an expression table: call target selection, projected receivers (indexed or case-payload), pattern field symbols, stamping resolved symbols back onto receivers.
2. Shape. Inline mods `calls` (`resolve_expression_table_call_target_symbol`, `:29`; `receiver_needs_result_type`, `:111`), `projected_receivers` (`needs_declared_projection`, `:146`; `ReceiverType`; `declared_nominal`, `:203`; `call_target`, `:211`), `receivers` (`pattern_field_symbol`, `:393`; `assign_pattern_field_symbols`, `:422`; `resolve_expression_table_member_symbol`, `:440`; `resolve_expression_table_receiver_path_symbols`, `:479`; `receiver_path_symbols`, `:506`), `stamping` (`stamp_receiver_path_symbols_in_table`, `:641`).
4. How it decides.
   - Call target: a callee that uses a value binding gives invalid (`:38-44`); a receiver that is (or projects from) a `Call` returns invalid because "needs result type" (`:46-48`, `:111-125`); indexed or case-payload receivers go through declared-type projection (`:49-62`); otherwise resolve the receiver path, then `resolve_call_target_symbol`; if that fails and the receiver is an invalid symbol or a Domain and a single-segment name, fall back to `call_target_for_attached_data(owner.as_str(), call.target.as_str(), span)` (`:83-96`). Spelling.
   - Projected receiver chain: walks `Indexed`/`Member`/`Borrow` down to a name, checks `declaration.parent == state_symbol && symbols.name(name.symbol) == spelling.as_str()` (`:253-257`), element type of `FixedArray`/`Slice` for each index hop (`:286-298`), field by name and optional case variant name equality, requiring exactly one candidate and `field.symbol.parent == owner|variant` (`:333-351`). Spelling within symbol-identity checks.
   - `pattern_field_symbol`: children of the head symbol that are `Field` with the same spelling; exactly one, else invalid (`:393-416`).
   - `Indexed` receivers: only an `Integer` index literal that fits `i64` (`:615-631`).
5. Cases by shape. Header comment (`:128-130`): "Projected method candidates follow exact indexed and case-payload declarations. Bounds, case reachability, effects, and access remain later-stage obligations." `RECEIVER_MEMBER_KINDS` is the fixed list Field, State, Parameter, Variant (`:383-388`).
6. Same job elsewhere. `names/targets.rs`, `names/lookup.rs` (not in unit) hold the other target-selection paths (callers imported at `:26`, `:373`).
9. Fail-open risks. `stamp_receiver_path_symbols_in_table` writes `head_symbol` into both fields for `Indexed` and `Member` receivers (`:654-668`), so the leaf symbol of those nodes is overwritten with the head; open (not traced to its readers). `resolve().unwrap_or_else(invalid)` (`:364`) turns every unmatched projection into an invalid target, which later reports an unresolved call.

---

## Coverage

Read in full (every line): all 41 ranges in `units/unit_054.tsv`, 21,174 lines:
uses.rs 1-1809; work_domain.rs 1-91; integer_widths.rs 1-270; machine_equations.rs 1-2117; mod.rs 1-107; target_families.rs 1-195; trait_defaults.rs 1-798; type_equations.rs 1-3270; deferred_bodies.rs 1-650; generated_source.rs 1-94; lib.rs 1-17; lowering/data.rs 1-522; domain.rs 1-278; expression.rs 1-966; item.rs 1-479; lowerer.rs 1-628; machine.rs 1-1065; machine_supply.rs 1-871; mathematical.rs 1-148; measure.rs 1-48; mod.rs 1-46; name.rs 1-52; operand_owners.rs 1-106; operator.rs 1-37; proposition.rs 1-99; state.rs 1-906; statement.rs 1-770; statement/assembly.rs 1-106; hoist_temps.rs 1-124; indexed_read_hoisting.rs 1-273; match_subject_hoisting.rs 1-242; place_operand_capture.rs 1-217; value_call_hoisting.rs 1-209; trait_definition.rs 1-230; type_aliases.rs 1-45; type_inputs.rs 1-106; type_reference.rs 1-670; wire.rs 1-73; names/contracts.rs 1-682; domain_facts.rs 1-1069; expression_paths.rs 1-689.
Skimmed: none. Not read: none. The inline `#[cfg(test)]` modules (integer_widths 178-270; machine_supply 438-871; deferred_bodies 571-650) were read but are summarized only by name.

## Five things a maintainer most needs to know about this unit

1. Stage 02 is only partly identity-keyed. Instantiation, trait and data selection use handles and `ClosedArgumentIdentity`, but the relabel passes in `generic_data/uses.rs` (`:247-248`, `:312-326`, `:1791-1805`), the type-equation solver's binder lookup (`type_equations.rs:1294-1303`, `:2109-2118`), evidence forwarding (`statement.rs:496-508`), the reference-struct-parameter hoist (`match_subject_hoisting.rs:190-197`), proof-fact local scopes (`domain_facts.rs:192-243`, `:672-697`) and assembly jump labels (`assembly.rs:83-86`) all key on `String` spellings.
2. `Lowerer.structural_type_equations_pending` defaults to `true` (`lowerer.rs:307`) and only `resolver.rs:129` clears it; while it is true, `validate_materialized_type_equations` is skipped in `lowering/type_reference.rs:84-90`; the function itself returns `Ok` for any head that does not select a data item (`type_equations.rs:2001-2011`).
3. Machine-argument completion (`machine_equations.rs`) re-lowers selected bodies in a loop until no new root items appear (`deferred_bodies.rs:412-433`), using `SourceLinks` vectors linearly scanned with `find` (`machine_equations.rs:461`, `:645`, `:684`), and prints non-error solver warnings to stderr with `eprintln!` instead of returning them (`:969-971`, `:1372-1374`).
4. Dead or misleading code: `current_state_parameter_names`, `current_state_parameters`, `current_state_return_type` (`lowerer.rs:226,245,247`) are written but never read (grepped, 14 lines, no reader); the `unreachable!` message at `expression.rs:513` calls binary lowering "stack-bounded" though `lower_binary_expression_into_table` recurses; a comment above `lower_type_parameters` (`data.rs:261-265`) describes a different, absent function.
5. Hoisting is a lowering-time rewrite that mints `__hoist_N` locals (only `lowerer.rs:495-496`, via `hoist_temps.rs:77`) and relies on later passes to bind locals "by ORDER + NAME" (`statement.rs:698`); `hoist_temps.rs` is the one constructor for them (grepped), but five files decide when to hoist (`statement.rs`, `indexed_read_hoisting.rs`, `match_subject_hoisting.rs`, `place_operand_capture.rs`, `value_call_hoisting.rs`).

## What you are unsure about

- Where `evidence_forwardings` rows are consumed and whether dropping the assignment (`statement.rs:158`) is safe when a same-spelled local shadows an evidence name; I grepped only the producer.
- Whether the non-error warnings dropped by `eprintln!` (`machine_equations.rs:969-971`, `:1372-1374`) can actually occur: `complete_equation_arguments_with_types` passes a `warnings` vec to `Solver::solve`/`recover_binder`/`match_structure`, but I saw no code in this unit that pushes to it (`_warnings` parameters are unused at `type_equations.rs:3068`, `:3111`).
- Whether `stamp_receiver_path_symbols_in_table` writing `head_symbol` into both fields for `Indexed`/`Member` receivers (`expression_paths.rs:654-668`) is intended; I did not read its readers.
- The "same job" duplicates in `selections/authored_selections.rs:1201` (`collect_expression_subtree`), `const_evaluation/templates.rs:367,375`, and `03_.../src/declarations.rs` (`lower_type_parameters`-named code, 8 grep hits) are suspected only; I did not read those sites.
- Whether `Lowerer::seed_resolved_base`'s full clone (`lowerer.rs:439`) and `Arc::make_mut` on the syntax forest (`deferred_bodies.rs:424`) actually copy in practice (depends on Arc sharing in the driver).
