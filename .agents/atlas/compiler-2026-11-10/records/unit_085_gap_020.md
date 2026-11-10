Second pass for unit 85

Ranges covered (all under `omega-rust/psi/compilation/representations/typed-trees/src/`, 5450 lines):
1. `callable_applications.rs` 6301-7775 (1475 lines)
2. `calls_and_control_flow.rs` 1-3975 (3975 lines)

Checkout `<checkout>/`. I read every line of both ranges in order. Counts below come from Grep content/count runs over `omega-rust`; where a count is for a whole name including the definition and own-file uses I say so.

Where range 1 starts: line 6301 is inside `impl TypedTrees` of the inner module `trait_scopes` (module begun at 6131 per the earlier reader; I read from 6285 and the function header at 6301 is `resolve_requirement_call_signature`). The inner module closes at 7001 (`}` then `pub(crate) use table::AppliedCallableTable;` at 7002). From 7004 to 7775 the code is file-level (not in a `pub(crate) mod`).

Contradictions with the earlier notes (`unit_085.md`): none found. The earlier notes said the core types (`CallableApplication`, `AppliedCallableOwner`, `AppliedCallableSignature`, `CallableSubstitution`, `AppliedMachineCallable(Id)`) were "defined outside the range" they read. They are all defined in my range: `AppliedCallableSignature` 7167-7171, `AppliedCallableOwner` 7191-7239, `CallableSubstitution` 7241-7245, `AppliedMachineCallableId` 7247-7254, `AppliedMachineCallable` 7256-7269, `CallableApplication` 7271-7290, `CallableLifetimeBinding` 7292-7297, `CallableLifetimeOrigin` 7299-7304. `ClosedConformanceApplication` is not defined here (only referenced at calls_and_control_flow.rs:1394).

---------------------------------------------------------------------------
## File 1: `callable_applications.rs` 6301-7775

### 1a. Requirement call-signature lookup, 6299-6613 (tail of `trait_scopes` module)

**Purpose.** Given a call that targets a trait requirement, find the `StateSignature` the caller must check against: either a "closed" public view, a prepared per-invocation applied signature, or an outer (evidence-bound or trait-declared) signature. Also create the per-invocation applied signature when a requirement has its own generic telescope.

**Shape.**
- `resolve_requirement_call_signature` 6301 (Result with reason; calls `requirement_invocation_signature` then falls back to `named_conformance_call_signature(call).ok_or(reason)`, 6306-6313).
- `requirement_statement_call_signature` 6316 (Option; `.ok()` at 6328, so a refusal becomes `None`; passes `Default::default()` for the requirement application, 6325).
- `requirement_invocation_signature` 6331-6427 (private).
- `requirement_outer_signature` 6429-6498 (private).
- `prepare_requirement_invocation` 6500 and `prepare_requirement_statement_invocation` 6520: two copies that differ only by `TableCallExpression` vs `TableCall`; both resolve target as `dispatch.contract_symbol()` or `call.target_symbol` (6507-6510, 6527-6530) and call `prepare_requirement_invocation_application` 6540-6613.
- `applied_callable_signature` 6615-6645.

**Inputs/outputs.** Reads `closed_requirement_application`, `static_machine_application`, `evidence_bound_requirement`, `applied_requirement_scope`, `trait_machine_signature`, `selected_requirement_contract_scope`. `prepare_requirement_invocation_application` builds a `CallableApplication` (6562-6566), runs `application.signature(self, selected)` (6589), and either `replace_signature`s an existing row or `insert`s a new `AppliedCallableSignature` with owner `RequirementInvocation { declaration, application, lexical_owner }` (6590-6611).

**How it decides.**
- Closed-view path (6340): if `closed_requirement_application(target)` finds a closed view, it is used. A matching requirement application scope is checked only if `requirement_application.is_valid()` (6386-6393, `requirement_application_matches_scope`).
- Evidence-bound refinement shortcut (6341-6385): only when `dispatch.is_none()`, the authored application is valid, its `lexical_owner == lexical_owner`, and the registered operation's parent is a `ConformanceParameter` symbol (6344-6353). It walks `declaration_scope_parent` upward with a visited list (6354-6383) and returns the applied signature only if it has no type or lifetime parameters (6372-6378).
- Prepared-row path (6396-6417): `find_requirement_invocation` with a closure keyed on handle equality of `application`, `lexical_owner`, and `prepared.signature.symbol == target` or (`source.registration_operation == target` and `source.binding_declaration == prepared.signature.symbol`) (6399-6413).
- Outer signature path: evidence bound located by walking owners (6439-6451); compares `bound.application.same_application(self, &written)` (6459); otherwise trait-declared signature with dispatch (6475-6484) or written application (6485-6494) or declared non-generic (6495-6497).
- `prepare_requirement_invocation_application` dedupes existing rows by `previous.owner == owner && previous.signature.symbol == signature.symbol` (6598-6600), a linear scan over every row (not via an invocation index).
- `applied_callable_signature` (6615-6645): first the closed-view shortcut (6620-6624), else key lookup `callable_contract_key` then first row whose owner is `Trait` with equal declaration and application identity and `signature.symbol == requirement` (6629-6643). Linear scan; `find_map` returns the first of any duplicates.

**Cases by shape.**
- `prepare_requirement_invocation_application` binding kinds: Type/Const go to `application.types`, Machine/Conformance go to `application.static_arguments`, `RuntimeValue { .. } => {}` is dropped (6573-6587).
- Returns `Ok(None)` (nothing prepared) when the target is closed, the authored application is invalid, the selected signature is not a trait member signature, or its `type_parameters` is empty (6548-6559).

**Same job elsewhere.**
- The "walk owners with visited list through `declaration_scope_parent`" loop appears at 6354-6383, 6437-6451 (this range), and in my other range at calls_and_control_flow.rs:691-716 (`finite_value_binder_in_scope`) and 1711-1727 (`source_target_context`, which uses `lifetime_declaration_owner` first). Four hand-written copies seen (read).
- `prepare_requirement_invocation` / `prepare_requirement_statement_invocation` are the same body for two call node types (6500-6538).

**Unused / one-caller (grep, whole `omega-rust`).**
- `resolve_requirement_call_signature`: definition 6301, one use in this file (6251, before my range), one outside: `build-time-evaluation/src/const_generic_expressions.rs:446`.
- `requirement_statement_call_signature`: definition 6316; 3 outside call sites: `validation/src/calls.rs:3937`, `calls.rs:43994`, `pipeline/04.../contextual_and_requirement_calls.rs:3493`.
- `prepare_requirement_invocation`: 2 outside call sites (`contextual_and_requirement_calls.rs:3189`, `:3413`); `prepare_requirement_statement_invocation`: 2 outside (`:3258`, `:3427`).
- `applied_callable_signature`: not counted separately.

**Stated limits.** None in this stretch beyond error strings ("closed public view does not match its source application" 6391, etc.).

**Fail-open.**
- 6386-6394: if the target has a closed view and `requirement_application` is invalid and the evidence shortcut did not fire, `closed.signature` is returned with no scope check at all. Which wrong program: a call written without a requirement application whose target happens to have a closed view for another application. Status: open (callers of `closed_requirement_application` not traced).
- 6573-6587: a `RuntimeValue` binding is silently ignored when building the invocation application. Open.
- 6548-6559: four conditions that return `Ok(None)` ("nothing prepared"); callers at contextual_and_requirement_calls.rs:3189, 3258, 3413, 3427 not traced. Open.
- 6228-6296 is before my range. Nothing else seen.

### 1b. Applied trait signature preparation, 6647-6999

**Purpose.** Walk every applied trait scope in the program (plus scopes created by refinement invocations and by conformance bounds of those scopes) and make sure each requirement that needs it has a retained applied signature row.

**Shape.**
- `prepare_applied_callable_signatures` 6647 (wrapper) and `..._reporting_change` 6654-6809 (returns the produced signatures and a `wrote` flag).
- `refinement_invocation_scopes` 6811-6913 (private).
- `materialize_trait_application_arguments` 6915, `materialize_trait_application` 6928 (pub(crate)), `trait_application_arguments` 6972.

**How it decides.**
- `..._reporting_change` builds `pending` tuples from each root scope (6676-6723). A requirement is skipped (`continue` 6681-6687) only when it has no type parameters, no substitutions in the scope, no lifetime parameters on the declaration, and no refinement clauses. Otherwise it is queued. Closed requirement applications whose template equals the signature, that match the scope by `requirement_application_matches_scope` but not by `..._matches_contract_scope`, are queued too (6692-6712).
- Dedup: sorted vector of `((arena_index, generation) of requirement, same of declaration, NormalizedTypeIdentity)` (6724-6750), binary searched (6757-6761). Rebuilt when `applied_callable_signatures.revision()` changes (6754-6756) or the single-insert revision assumption fails (6799-6806).
- Row creation: closed view's signature cloned (6764-6767), else `materialize_trait_application` then `CallableApplication::nominal` (6769-6778); `nominal` returning `Ok(None)` makes the code `continue` (6779-6781), silently dropping the pending entry. Then `apply_callable_refinements` (6784) and insert with owner `Trait { declaration: owner, application: identity, scope }` (6789-6797).
- `refinement_invocation_scopes`: for each conformance application occurrence (from `collect_program_application_occurrences`, 6817-6819), only machines with a conformance bound whose trait has some refinement clause with non-empty contracts are considered (6824-6840); other cases `continue` (6821-6823, 6838-6840).
- `materialize_trait_application` walks `scope_starts` frames (6942-6968) and, for frame ordinal 0, assigns `argument_owner` as lifetime owner to any applied type reference that has none (6954-6963).

**Cases by shape.** In `refinement_invocation_scopes` the binding match (6847-6874) accepts Type, Const, Machine (creating a type reference if absent, 6851-6858), Conformance (error if no retained `type_reference`, 6862-6865); `RuntimeValue` is an `Err` (6869-6873, "refinement postcondition invocation has an unresolved value argument").

**Same job elsewhere.** The three-way binding-kind match here (6847-6874) resembles the one in 6573-6587 but they differ on RuntimeValue (Err vs ignored). Read both.

**Fail-open.** 6779-6781 (`nominal` returns None, requirement left without a row) and 6821-6823 (machine not found, invocation skipped): both skip rather than refuse. Open (not traced).

### 1c. Table view helpers, 7002-7157

- `pub(crate) use table::AppliedCallableTable;` 7002; `pub use requirement_instances::{ClosedRequirementApplication, RequirementLocalArgument};` 7004. Imports 7006-7009.
- `applied_trait_signatures` 7012 (wraps `applied_trait_contracts` 7020-7041). `applied_trait_contracts` keeps rows whose owner is `Trait` or `RequirementInstance` with matching declaration (7027-7040).
- `requirement_invocation_signatures` 7043-7058 (owners `RequirementInvocation` or `Realization` whose `lexical_owner == caller`).
- `prepared_callable_signatures` 7060 and `prepared_callable_contracts` 7068-7096: every live row's signature, plus for `MachineSelection` owners the `required` (if Some) and `source_required` signatures (7073-7080, 7088); `trait_scope` is taken only from `Trait` and `RequirementInstance` owners (7081-7087).
- `callable_application_owner_is_live` 7098-7106: `RequirementInvocation` and `Realization` rows are live only if `machine_by_symbol(lexical_owner)` finds a machine; every other owner is live (`_ => true`, 7104).
- `retain_live_trait_callable_applications` 7108-7132: builds a `live` list of `(declaration, application)` keys from `applied_trait_roots` visits, frees `Trait`-owner rows whose key is not in `live` (7117-7131). Linear `Vec::contains` per row.
- `remove_trait_declaration_applications` 7134-7156: frees rows of four owner kinds whose `declaration` satisfies the caller's predicate.
- Doc at 7065-7067: "Logical declaration symbols deliberately remain unchanged when preparation substitutes a concrete outer application."

**Unused / callers (grep).**
- `applied_trait_signatures`: whole-repo count 1 (its own definition at 7012). No callers. (grep `\bapplied_trait_signatures\b` count mode.)
- `applied_trait_contracts`: callers `references.rs:2734` (1 outside) plus the wrapper.
- `requirement_invocation_signatures`: 1 outside, `references.rs:3335`.
- `prepared_callable_contracts`: outside `validation/src/proof_contracts_core.rs:906`, `pipeline/04.../contextual_and_requirement_calls.rs:3287`; `prepared_callable_signatures`: outside `proof_contracts_core.rs:1018`, `completed_storage_and_conservation.rs:4710`, `declaration_tables_and_index.rs:4528`, `preparation.rs:1597`, `contextual_and_requirement_calls.rs:4047, 4067`.
- `retain_live_trait_callable_applications` and `remove_trait_declaration_applications`: one caller each, `validation/src/frozen_program_and_queries.rs:2209` and `:2205`.
- `AppliedCallableSignature::requirement_instance_symbol` 7174 and `requirement_template_symbol` 7179: name counts show 2 uses in `declaration_tables_and_index.rs` and 2 in this file (definitions); I did not separate which are call sites.
- `AppliedMachineCallableId`: 6 uses in this file, 1 in `lib.rs`, 1 in `validation/src/machine_parameters_and_bindings.rs` (grep count; not split into definition and use).

**Fail-open.** `callable_application_owner_is_live` `_ => true` (7104) keeps every `Trait`, `RequirementInstance`, `MachineSelection`, `Absent` row; liveness of those kinds is decided elsewhere (`retain_live_trait_callable_applications` for `Trait` only). `RequirementInstance` and `MachineSelection` rows are never dropped by this function. Status: traced within the file only.

### 1d. Types and `CallableApplication` core, 7159-7527

**Purpose.** Type definitions for retained applied callable rows and the per-substitution working state, plus the lifetime-scoping and nominal-requirement application methods.

**Shape.**
- `CallableContractScope<'a>` 7161-7165 (`signature`, `trait_scope`). Comment 7159-7160: "This is source typing context, never a conformance or checked proof."
- `AppliedCallableSignature` 7167-7171 (`owner`, `signature`).
- `AppliedCallableOwner` 7191-7239: `Absent` (default), `Realization`, `Trait`, `RequirementInvocation`, `RequirementInstance` (largest: 10 fields incl. `trait_application_identity`, `portable_local_arguments`, `local_arguments`, `invocation_contract_parameters`, `application`), `MachineSelection` (14 fields: `invocation`, `ordinal`, `demand`, `selected`, `provider`, `required: Option<StateSignature>`, `source_required`, `caller_operands`, `caller_bindings`, `caller_lifetimes`, caller/provider lifetime declaration and owner, `provider_lifetimes`).
- `CallableApplication` 7271-7290: 14 fields incl. `owned_signature`, `runtime_values`, `region_scope`, `trait_region_application: Option<(TraitApplicationEnvironment, usize)>`, `lifetimes`, `elided_lifetimes`, `types`, `static_arguments`, `static_argument_owner`, `selected_providers`, `symbols` (formal -> replacement pairs), `active_requirements`, `source_binders` (name -> symbol).
- `CallableLifetimeOrigin` 7299-7304: `Lexical`, `Rigid`, `Captured`.
- Methods: `scoped_lifetime` 7313-7398, `lifetime` 7400, `symbol` 7408, `nominal_contract` 7416-7477, `nominal` 7479-7526.

**How it decides.**
- `scoped_lifetime` (7313): first by exact region identity from the type reference's operand lifetimes at `slot` (7320-7325): finds a `lifetimes` binding whose parameter resolves to the same identity (7327-7336), else resolves through the trait environment (7337-7344) and marks `Captured` if substituted. If no identity, falls back to a binding whose `parameter == original` (a name compare on `Identifier`, 7358-7363). If there is no trait environment it returns the original name with `Default` region identity and origin `Lexical` (7370-7376). Otherwise `environment.lifetime_identity(...)` by name (7377-7385).
- `lifetime` (7400-7406) and `symbol` (7408-7414): reverse-search a list; if no replacement found, return the original (pass-through). Keys are `Identifier` string equality for lifetimes (7404) and handle equality for symbols (7412).
- `nominal` (7479): refuses recursion with `Err("recursive nominal callable application is not yet represented by cycle-aware applied telescopes")` when `(trait_symbol, requirement)` is already in `active_requirements` (7503-7510). Returns `Ok(None)` if trait definition missing (7486-7488), parameter count differs from argument count (7494-7496), or no declared signature for the symbol (7497-7502). Pushes parameter-name/symbol pairs into `source_binders` and types, runs `self.signature`, and truncates both after (7511-7524).
- `nominal_contract` (7416): `Ok(None)` for `applied` when `arguments.is_empty() && environment.is_none()` (7459-7460); when the declaring trait differs from the selected trait it builds an `AppliedTraitScope`, finds the unique declaring scope or errors (7437-7451) and rematerialises arguments (7456). Result `MachineParameterContract::Nominal { receiving_trait, receiving_arguments, trait_definition, requirement, trait_arguments, instantiated }` (7469-7476).

**Cases by shape.** `AppliedCallableOwner` is the fixed list of row kinds; code that matches owners lists subsets explicitly (e.g. 7027-7033, 7049-7052, 7073-7087, 7099-7105, 7120-7125, 7141-7146); several use a `_ =>` arm.

**Stated limits.** 7507-7509: "recursive nominal callable application is not yet represented by cycle-aware applied telescopes" (error string).

**Fail-open.**
- 7370-7376: a lifetime name with no resolved region identity, no matching binding and no trait environment passes through unchanged with a `Default` region. What gets through: a result-type or operand lifetime name that was never bound; later stages compare by name. Status: open.
- 7400-7414 pass-through of unmapped lifetime names and symbols. Open.
- 7459-7460/7494-7502: `instantiated: None` stored on the machine parameter contract (7475) when nothing could be applied. `instantiate_nominal_machine_parameter_contracts` only revisits rows with `instantiated: None` (7750). Whether `None` is later refused: not traced.

### 1e. Public application entry points, 7529-7775

**Purpose.** Thin public wrappers that construct a `CallableApplication` with the right fields and call a single engine method (`type_reference`, `parameter_kind`, `nominal_contract`, `facts`).

**Shape and decisions.**
- `apply_refinement_postconditions` 7530-7608 (pub(crate)): requires equal counts of parameters, type parameters and lifetime parameters between the declared and receiving signature (7540-7544, else Err "refinement postcondition lost its inherited callable telescope"). Builds a positional symbol map (declared symbol -> receiving symbol, then params, then type params, 7546-7559), source binders by name (7560-7568), and `Rigid`-origin lifetime bindings positional by ordinal (7569-7583). Then rebuilds `receiving.contracts` and appends only contracts of kind `Ensures`; any other kind is `Err("refinement postconditions cannot add caller obligations")` (7599-7602). Facts are copied through `application.facts` (7603).
- `apply_callable_result_type` 7610-7633: lifetimes with `Some(argument)` become `Lexical` bindings with default region; `None` goes to `elided_lifetimes` (7620-7631).
- `apply_callable_type` 7635-7657 (types and lifetimes given directly; default region, `Lexical`).
- `apply_static_application_type` 7659-7692: takes lifetimes from `static_application_lifetimes_for_declaration`, each resolved with `lifetime_parameter_identity` against `source.lexical_owner`; errors if missing (7677).
- `apply_captured_callable_type` 7694-7720: `trait_region_application` set; local lifetimes become `Rigid` bindings with identity from `declaration` (7707-7713).
- `copy_callable_parameter_kind` 7722-7734, `instantiate_nominal_machine_parameter_contracts` 7736-7774: picks data type parameters from `first_parameter` on whose kind is `Machine { contract: Nominal { instantiated: None, .. } }` (7743-7752) and rewrites the contract via `nominal_contract` (7765-7771); the lexical owner is `declaration_scope_parent(parameter)` (7764).

**Callers (grep).**
- `apply_callable_type`: 2 uses in this file (1344, 4687) plus 14 outside call sites (`type_system.rs:2770, 2797`, `preparation.rs:821, 1277, 1303`, `data_and_product_operands.rs:103`, `contextual_and_requirement_calls.rs:1563`, `constructed_results.rs:177, 343`, `03.../type_reference.rs:909`, `call_and_product_selections.rs:1402`; counted from the content listing: type_system 2, preparation 3, data_and_product_operands 1, contextual 1, constructed_results 2, type_reference 1, call_and_product_selections 1 = 11 outside plus 2 inside).
- `apply_callable_result_type`: 1 outside (`constructed_results.rs:281`).
- `apply_static_application_type`: 1 outside (`selection_and_reflection.rs:4038`).
- `apply_captured_callable_type`: 1 outside (`body_cloning_and_rewriting.rs:1489`).
- `copy_callable_parameter_kind`: 1 outside (`body_cloning_and_rewriting.rs:1027`).
- `instantiate_nominal_machine_parameter_contracts`: 1 outside (`03.../lowerer.rs:1861`).
- `apply_refinement_postconditions`: 1 outside (`product_and_trait_declarations.rs:2707`).

**Same job elsewhere.** `apply_callable_type`, `apply_callable_result_type`, `apply_static_application_type`, `apply_captured_callable_type` are four constructors for the same engine call (`type_reference`) differing in how the lifetime bindings are formed.

**Fail-open.** `apply_callable_result_type` lets elided lifetimes through into `elided_lifetimes` without an error (7629). What this does to a type reference containing them: not traced (`type_reference` is outside my range). Open.

---------------------------------------------------------------------------
## File 2: `calls_and_control_flow.rs` 1-3975

The file is three inline modules: `calls` (7-1686), `control_flow` (1688-3793) and `names` (3795-3975). The header comment (1-5) says it holds typed call signatures and dispatch families, executable machines, states, statement control graphs, and symbol/identity names. The range starts at line 1; no enclosing function.

### 2a. `calls::application_graph` 8-131
- **Purpose.** Build a `ProofApplicationPlan<SymbolHandle>` (a vocabulary type from `semantic_vocabulary::proof_applications`) from the typed program.
- **Shape.** `source_application_plan` 17-130.
- **Reads/produces.** Reads program declarations (`references::collect_program_declarations`), trait signatures and refinement clause signatures, declaration dependencies (`collect_declaration_symbol_dependencies`), machines/states/binders, target machine families, conformance rows, machine trait conformances. Produces the plan (symbols table, declarations, dependencies, machines, provider candidates).
- **Decides.** Machines are removed from `declarations` (20, 49); a dependency on a trait requirement is rewritten to its declaring trait (44-47); dependencies kept only if not a machine and in `declarations` (48-51). Provider candidates are all `(row.requirement, row.realization_machine)` of every conformance (114-119) and `(clause.requirement_symbol, machine.symbol)` for machine clauses with a valid requirement symbol (120-127). `product_selected: true` is hard-coded for every machine and `dynamic_realizations: Default::default()` (109-110).
- **Callers (grep).** 2 outside: `build-time-evaluation/src/layouts.rs:6187`, `pipeline/05.../published_facts.rs:7192`.
- **Fail-open.** None seen.

### 2b. `calls::boundary` 132-152
- `called_boundary_signature` 137-151: returns the requirement signature if the called target (or dispatch contract symbol) resolves to a trait requirement of a trait with `is_boundary`; else `None`. Callers: `proof/src/obligations.rs:484`, `pipeline/05.../checks_ranges.rs:8278` (2 outside).

### 2c. `calls::dynamic_traits` 153-636
- **Purpose.** Decide whether a trait requirement can be offered through a dynamic trait (a `dyn`-style object) and record dynamic slot signatures.
- **Shape.** `DynamicNonSignatureIneligibility` 164-168 (`PrivateContractIdentity`, `UnprojectableResultLifetime`, `UnfittableEnvelope`); `DynamicSlotSignature` 170-179 (`symbol`, `interface_identity`, `declaring_trait`, `requirement`, `call_requirement`, `parameters`, `return_type`) with `data_parameters` (skips index 0, 181-185); methods `dynamic_slot` 188, `dynamic_slot_requirement` 199, `dynamic_slot_by_symbol` 207, `dynamic_signature_eligibility` 217, `dynamic_non_signature_eligibility` 237; private collectors 255-635.
- **Decides.**
  - `dynamic_signature_eligibility` (217-235): `Err(BoundaryRequirement)` if the trait is boundary (222); `Err(RequirementLocalGenerics)` if the requirement has type parameters and `finite_signature_family` is not `Finite` (225-231). `dynamic_slot` finds by linear scan on `call_requirement == requirement` and `interface_identity` (193-196).
  - `dynamic_non_signature_eligibility` (237-253): rejects if any symbol named in the contract facts is known and not public (241-243, 255-268); rejects if a result lifetime is not among the input lifetimes (244-246, 270-294); rejects if `service_reach_is_installation_bound` or `native_callback_parameters` non-empty (247-250).
  - Lifetime comparison is by name string (`Vec<String>`; `push_lifetime_name` 630-635 skips empty names).
- **Cases by shape.** `collect_expression_symbols` 327-448 matches every `ExpressionNode` variant with no wildcard (the last arm lists Boolean/Float/UndeterminedNaN/Integer/String as empty, 442-446). `collect_type_reference_symbols` 518-585 and `collect_type_reference_lifetimes` 587-628 also match all variants; in the symbols version `TypeConstraintNode::Named(_) | ArithmeticDomain(_) => {}` (545-546) contributes no symbols; in the lifetimes version `ConstExpression`, `DynamicTrait`, `Named`, `Unit` contribute nothing (623-626).
- **Same job elsewhere.** `contract_identity_is_public` 467-516 does nine linear scans across tables (data definitions, machines, traits, domains, propositions, conformances, consts, mathematical definitions, operators). Name-grep shows no other function with the names `collect_type_reference_symbols`, `collect_expression_symbols`, `contract_identity_is_public` in typed-trees `src` (grep). Whether `references.rs` has a similar walker under another name: suspected, not read.
- **Callers (grep, counts include own file).** `dynamic_slot*`/eligibility names appear 69 times in 22 files including this one, mostly `validation/src/calls.rs` (19). Not split by name.
- **Fail-open.**
  - 267: `contract_identity_is_public(..) == Some(false)` is the only rejection. A symbol that is invalid or not found in the nine tables gives `None` (468-470, 471-515) and is accepted as public. Which wrong program: a contract fact naming a private declaration of a kind not in those nine tables (e.g. a state, field, type parameter symbol) or a `TypeConstraintNode::Named` constraint (545-546, not collected) would not block dynamic eligibility. Status: open (I did not check which kinds can appear in a contract fact).
  - 441-446: literal expression kinds contribute nothing: expected.

### 2d. `calls::finite_family` 637-1015
- **Purpose.** Decide whether a requirement or machine with value binders (Const/Value type parameters) declares a finite family via a `requires` clause that is a disjunction of complete `Binder == literal` equalities, and enumerate the tuples.
- **Shape.** `FamilyValue` 647-650 (`identity: String`, `display: String`), `FamilyTuple` 653-656, `FamilyProbe` 659-665 (`NotFinite(String)` or `Finite { arity, tuples }`); `finite_signature_family` 668, `finite_machine_family` 677, `finite_value_binder_in_scope` 686, `finite_declaration_family` 720, `explicit_signature_family` 777, `binary_chain_leaves` 828, `equality_assignment` 847, `binder_operand` 878, `literal_operand` 897, `value_fits_carrier` 969; free functions `integer_const_identity` 989, `canonical_const_identity` 993, `primitive_integer_range` 1001.
- **Decides.**
  - `finite_declaration_family` (720-775): `NotFinite` if lifetime parameters exist (727-732) or any type parameter is not Const/Value (733-748). Each `Requires` contract fact that is an expression is examined (752-773). First finite candidate is stored; a second finite candidate gives `NotFinite("...one explicit disjunction...")` (763-768).
  - `explicit_signature_family` (777-826): each alternative is an `And` chain of equalities; each equality must bind a distinct binder (796-803); each alternative must bind every binder (804-810); tuples sorted and deduplicated by identity strings (820-821).
  - `equality_assignment` (847-876): operator must be `Equal`; exactly one side a binder and the other a literal (859-868); carrier must be the binder's `type_reference`; `value_fits_carrier` (969-986) parses the identity string by prefix `named(integer-const(` and suffix `))` (977-980) and compares to the primitive's integer range; any other identity passes only if the primitive is `Bool` (985).
  - `binder_operand` (878-895): a single-member name; if `path.symbol` is valid compares symbol, else compares `parameter.name.as_str() == member.as_str()` (888-893): source-spelling match.
  - `literal_operand` (897-967): Integer literals via `value_i64`/`value_u64` (904-907); Boolean (913-920); a one-member Name resolves to a const either by `path.symbol` or by looking up a top-level `Const` symbol by name `find_top_level_by_name_and_kinds_from_source(member, [Const], span-of-owner)` (924-932) and decodes its evaluated encoding into integer or boolean (937-963), others `None`.
  - Identity strings are built with `format!` (990, 994-998) and later parsed back by `value_fits_carrier` (977-980). Keyed on string form.
- **Cases by shape.** `finite_declaration_family` error text at 744-747: "the requirement carries non-value generic binders"; 805-808: complete tuples required; 790-793: "not explicit `Binder == literal` equalities". `primitive_integer_range` returns `None` for `Bool | F32 | F64 | Selector` (1004).
- **Callers (grep, counts incl. own file).** `finite_signature_family`, `finite_machine_family`, `finite_value_binder_in_scope`: 28 occurrences across 13 files, 5 in this file; outside users include `validation/src/value_custody_core.rs`, `traits.rs`, `machine_parameters_and_bindings.rs`, `calls.rs`, `06.../proofs.rs` (4), `monomorphization.rs`, `contextual_and_requirement_calls.rs`, compiler test support. `integer_const_identity` / `canonical_const_identity` calls: 6 in this file, 1 outside (`omega/compilation/compiler/tests/support/provider_instance_controls.rs`).
- **Fail-open (traced inside the function, callers not traced).**
  - 759-761: a `Requires` fact that is not an expression (membership or proposition) is skipped (`continue`).
  - 763-772: once a finite family has been found, a later `Requires` fact whose candidate is `NotFinite` is dropped (the `else if` only replaces `family` when `family` is not yet finite). So `requires` with one explicit finite disjunction plus another `requires` containing an opaque predicate or inequality still returns `Finite` with only the enumerated tuples. Wrong program that gets through: a requirement with an extra constraint that excludes some enumerated tuples (the family then lists tuples the extra clause forbids). Status: open; the comment at 766 says the family must be "one explicit disjunction", which this contradicts for the multi-clause case.

### 2e. `calls::operand_order` 1016-1200
- **Purpose.** Compute the order in which call arguments are evaluated when some arguments are runtime values bound through static machine bindings.
- **Shape.** `CallArgumentEvaluationOrder` 1022-1032 (`positions`, `position`, `count`; empty `positions` = natural order, `is_natural` 1029); `call_argument_evaluation_order` 1048-1180; `derives_from` 1183-1199.
- **Decides.** Natural order when the application handle is invalid (1059-1061) or has no `RuntimeValue` bindings (1073-1075). Receiving parameters come from the first of: closed requirement application, state, machine (first state), dynamic slot, trait requirement, machine parameter signature, evidence-bound requirement (1076-1112); missing: `Err("runtime operand order lost its exact receiving declaration")` (1112). Parameter count must equal `count` (1113-1117). Each runtime binder is matched to a parameter by following `generated_from` links (`derives_from` 1183-1199, which also stops if the predecessor's arena index is not lower, 1193); two matches give Err (1125-1127).
  - Natural order is returned if every runtime binder has no matching parameter and (no closed view or all closed local arguments are static operands) (1130-1139). Otherwise any unmatched binder is an Err (1143). Duplicate positions Err (1146-1150). Ordering: explicit arguments (after stripping member projections 1154-1159) first at their positions, then non-runtime positions, then those whose expression has a `saved_argument_source` (1160-1171). Returns natural if resulting order equals 0..count (1172-1174).
- **Callers.** `call_argument_evaluation_order|CallArgumentEvaluationOrder`: 14 occurrences in 6 files incl. 8 in this file.
- **Fail-open.** 1130-1139: if no runtime binder maps to a formal and there is no closed view, the call gets natural order with no error even though binders carried runtime values. Traced within the function: only the mixed case errors (1143). Wrong program: a runtime-value binding whose binder is derived from no formal of the called declaration. Status: open.

### 2f. `calls::signature` 1201-1379
- Plain data. `StateSignature` 1257-1278 (20 fields: `symbol`, `name`, `spelling`, `lifetime_parameters`, `type_parameters`, `conformance_bounds`, `is_default`, `parameters`, `native_callback_parameters`, `return_type`, `invokes`, service reach fields, `suspends`, `blocks`, `contracts`, `termination_guarantee`; default `NoGuarantee` at 1301). `StateParameter` 1313-1321 (`is_const`, `is_mutable`, `relevance`). `SignatureContract` 1345-1352 and `SignatureContractKind` 1360-1372 (`Requires`, `Ensures`, `EnsuresForResultCase`, `Crashes{cause}`); `CrashCause` {Trap, Abort}. `AuthoredInvocation` 1228-1233 with `target: AuthoredInvocationTarget` {Unresolved, Parameter, Service}. `AuthoredServiceReachRow` 1213-1219.
- `state_parameter_position` 1323-1330: position by symbol equality. (Name counts too noisy to give a clean outside count.)
- `SignatureContract::is_crashes` 1355.

### 2g. `calls::static_applications` 1380-1568
- **Shape.** `StaticMachineApplicationHandle = Handle<StaticMachineApplication>` (1389); `StaticConformanceSelection` 1392-1395; `StaticMachineApplication` 1398-1411 (12 fields including `callable_contract_source`, `registration_operation`, `lexical_owner`, `target`, `arguments`, `input_selections`, `selected_machine_arguments`, `bindings`, `lifetime_bindings`, `lifetime_binding_declaration`, `binding_declaration`, `conformance_selection`); `StaticMachineBindingValue` 1425-1436: `Type`, `Const`, `RuntimeValue{type_reference, subject, expression}`, `Machine(arg)`, `Conformance(arg)`; default `Type(invalid)` (1438-1442).
- `project_type_operand` 1444-1456: if the reference is a `Named` type whose symbol matches a formal in `substitutions`, returns the actual; otherwise returns the reference unchanged (pass-through for any non-Named or unmatched reference). 6 outside uses (`contract_entailment.rs:5307, 39098, 39111, 40228`, `float_and_mathematical_contracts.rs:3570`, `operator_declarations.rs:101`).
- `static_application_lifetimes_for_declaration` 1459-1566: returns the bindings borrowed with no checks if empty (1471-1473). Otherwise: original declaration (`lifetime_binding_declaration`), registered declaration (`registration_operation`, via lifetime owner, else machine-parameter signature, else holding machine, else Err, 1477-1491), must equal original (1492-1496). If `original != receiving` it needs a requirement/owned-realizer correspondence either way (`owned_conformance_requirement_scope`, 1503-1519) and equal region telescope length (1520-1524); each binding's parameter must name the original declaration's own formal by `lifetime_parameter_identity` (1528-1536), no repeats (1537-1542), and its argument must resolve in `source.lexical_owner` (1543-1550); when declarations differ the formal is re-expressed by ordinal into the receiving telescope (1551-1559), else the bindings are returned borrowed (1561-1562).
- **Decides.** Everything keyed on lifetime-declaration owners and region ordinals; names of lifetimes are used as lookup input for `lifetime_parameter_identity(.., binding.parameter.as_str())` (1528-1531, 1543-1547).
- **Fail-open.** 1471-1473 (empty bindings return borrowed empty, nothing validated); expected for "no region group". Stated by structure only.

### 2h. `calls` module-level methods 1569-1686
- `call_has_no_runtime_receiver_for_target` 1574-1655, `call_has_no_runtime_receiver` 1657-1684.
- **Decides.** Returns `true` ("call needs no runtime receiver value") in these cases:
  1. Target belongs to a machine state and `call_has_no_runtime_receiver` holds (1575-1582): target symbol equals the state's, and receiver absent (1666), or a name path naming a `Conformance`/`ConformanceParameter` symbol (1675-1682), or the machine's attached data symbol (1683).
  2. `selected_named_expression_call_has_no_runtime_receiver` (external, 1583).
  3. Receiver is a `ConformanceParameter` name with an evidence-bound requirement for the target: true iff `call.target == requirement.signature.name` and argument count equals the requirement's parameter count (1586-1599). This compares `Identifier` strings (1596).
  4. Target's parent is a `ConformanceParameter` and receiver path symbol is that binder, same name/arity test (1600-1616).
  5. Trait requirement path: unique owning trait/requirement (1619-1630, 1639), arity equals operand count (1631-1642), name equals (1643, only when no dynamic slot), receiver absent (1647-1649) or a name path naming the trait symbol (1653-1654).
- **Fail-open.** `true` at 1647-1649: no receiver on a call whose target is a unique trait requirement with matching name and argument count. This is the intended "static requirement call" meaning; I did not trace what a user-written call with a wrong receiver arrives as. Open. Dispatch is keyed on spelling at 1596, 1613, 1643 (`call.target` vs `signature.name`).
- **Callers (grep, count incl. own file).** 24 occurrences across 11 files, 3 in this file; outside users in `validation/src/calls.rs` (8), `machine_calls_core.rs` (2), `contract_entailment.rs` (2), `05.../values_scalar.rs`, `flow_mutation.rs`, `flow.rs` (2), `borrow.rs`, `execution_scalar_computations.rs` (2), `build-time-evaluation/src/const_initializers.rs`, `const_evaluation.rs`.

### 2i. `control_flow::machine` 1689-2002
- **Shape.** `Machine` 1753-1792 (about 37 fields: `lexical_region_origin`, `assembly_target_requirement`, `symbol`, `name`, `attached_data(_symbol,_application)`, `generic_data_template`, `spelling`, `is_public`, `interface_exposure`, `supply_mode`, `purpose`, `target`, `authored_target`, `target_family_interface`, `body_is_present`, `body_status`, `structural_type_equations_pending`, `termination_plan`, service reach fields, `admitted_same_stack_bytes`, keyword spans, `lifetime_parameters`, `type_parameters`, `declared_type_equations`, `owned_data`, `satisfies`, `conformance_bounds`, `invokes`, `suspends`, `blocks`, `contracts`, `states`); `Machine::default` 1843-1885. Others: `MachineBodyStatus` {Deferred, Complete}, `MachinePurpose` {Runtime, ProviderDefaults}, `LexicalRegionOrigin` 1747, `DeclaredTypeEquation` 1795, `TargetMachineFamily` 1830 and member 1838, `GenericConformanceBound` 1888-1895, `OwnedData` 1898, `TraitConformance` 1906-1918, `SatisfiedDeclaration` 1921-1928 {Trait, TopLevelRequirement, Operator}.
- **Functions.** `machine_authored_target` 1700, `source_target_context` 1707-1729 (walks lexical owners with visited list, returns the first authored target; returns `None` when it reaches a machine symbol with no declaration for that lexical scope, 1722-1724), `has_specialization_parameters` 1801 (type params or conformance bounds), `supplies_checked_body` 1805-1811 (`CheckedBody` => true, `Boundary` => `body_is_present`, `_ => false`), `has_bodyless_signature` 1813, `symbol_spelling` 1817-1826 (`format!("{}::{}", name, target)`), `resolve_satisfied_declaration` 1940-1997, `machine_by_symbol` 1999-2001 (linear scan over a slice).
- **Decides.** `resolve_satisfied_declaration`: requirement name from `conformance.requirement` (None -> returns None, 1945). If the conformance symbol is a trait, matches trait requirements by `requirement.name == *requirement_name` (1953); if more than one, filters by equal `normalized_result_dispatch_set` of return types (1957-1964); exactly one required else `None` (1966-1968). If not a trait: any conformance arguments -> `None` (1975-1979); else the checked operator for the conformance, then a `TopLevelRequirement` machine with that symbol, else the operator (1981-1996). Keyed on requirement name spelling plus return-type dispatch set.
- **Callers.** `resolve_satisfied_declaration`, `state_position`, `state_parameter_position`, `AuthoredServiceReachRow`: 163 occurrences across 66 files in a combined name grep (too broad to split).
- **Fail-open.** `supplies_checked_body` `_ => false` is fail-closed. None seen.

### 2j. `control_flow::state` 2003-2087
- `State` 2012-2019 (`symbol`, `name`, `parameters`, `return_type`, `contracts`, `statement_nodes`). Free functions `state_by_symbol` 2021 and `state_position` 2025 (linear scans on slices).
- `parameter_domain_membership_contract` 2030-2085: builds a `Requires` contract with one `ProofFact::Membership` fact on a name expression for the parameter. The domain path is built by `domain_full_name.split("::")` (2060): source-spelling derived path members (`Identifier::generated(part)`, 2062). `authored_domain_selection: None` (2074). Callers: `04.../body_cloning_and_rewriting.rs:620`, `03.../contracts_and_signatures.rs:113` (2 outside).

### 2k. `control_flow::statement::assembly` 2094-2218
- `AssemblyControlFlow` 2100-2107 (`target_requirement`, `primitives`, `scratch` array of `ASM_SCRATCH_SLOT_COUNT` expression handles, `blocks`, `statement_count`), `AssemblyControlPoint` {Entry(graph), Label(block)}, `AssemblyBlock` {label, start, count, exit}, `AssemblyExit` {Fallthrough, Jump, ZeroFlag(block, bool), EnclosingTransition}.
- `StatementTable` accessors for assembly (2144-2180); `copy_assembly_from` 2181-2216 copies blocks and re-points `Jump`/`ZeroFlag` targets by index within the graph (2190-2208), using `.expect("assembly copy retains only graph-local edges")` (2198): a panic if an edge targets a block outside the graph.

### 2l. `control_flow::statement::control_graph` 2219-2523
- **Purpose.** Turn a statement span with inline assembly into a block graph and compute the finite execution order of statements.
- **Shape.** `StatementControlGraph` 2224, `StatementExecutionIter` 2228, `StatementExecutionOrder` 2233-2237 (`source_count`, `indices`, `positions`) with `position`, `prefix_end`, `precedes`, `at_or_before`, `latest`, `successor` (2239-2289), `StatementControlBlock` 2300, `StatementControlExit` {Continue(usize), SourceExit, End} 2306-2310. `StatementControlGraph::finite_execution_order` 2313-2345. `StatementTable::admitted_execution_order` 2349, `admitted_execution_statements` 2368, `finite_execution_order` 2382, `control_graph` 2390-2498. One test at 2506-2521 on `prefix_end`.
- **How it decides.**
  - `control_graph` (2390): `Err(String)` if the span has stale statement handles (2395); each `Assembly(Entry)` must be well-formed: label `Assembly(Label)` as first statement of each block matching the expected handle (2442), no independent control point inside a block (2448-2455), blocks partition the window (2430-2439, 2484-2488); an `Assembly(Label)` met outside an entry window is an Err (2406-2409); `ZeroFlag` exits are an Err "must be normalized before finite execution" (2457-2460); `Jump` to a non-graph block is an Err (2473); `EnclosingTransition` becomes `SourceExit` (2476).
  - `finite_execution_order` (graph, 2313-2345): follow `Continue` edges from block 0; revisiting a block returns `None` (2322-2324); in each block take only `reachable_state_statements`; stop after the block if the window was cut short or the last reachable statement is an unconditional transition (2327-2337).
  - `admitted_execution_order` / `admitted_execution_statements` (2349-2380): `.expect("typed statement control shape was admitted")` and `.expect("finite assembly control was admitted")`, so a graph error or a cyclic assembly graph panics instead of returning.
- **Cases by shape.** `StatementNode::Assembly` is handled at 2402; only `Entry`/`Label` shapes; `ZeroFlag` rejected.
- **Callers (grep count, names `admitted_execution_order|admitted_execution_statements|finite_execution_order|control_graph(`).** 92 occurrences across 33 files, 9 in this file; heavy use in `05_typed-trees-to-checked-trees`, `validation`.
- **Fail-open.** None seen; the panic path (2355-2356, 2374-2375) is stated by the `expect` text. `statements[block.start..block.start + block.count]` at 2326 is an unchecked slice (panic, not accept).

### 2m. `control_flow::statement::payloads` 2524-3149
- **Purpose.** Deep-copy a span of statements from another `TypedTreeTables` into this one, remap symbols in a span, list expression roots per statement, copy/remap proof facts.
- **Shape.** `copy_statement_nodes_deep_from` 2531, `copy_statement_nodes_with_type_expressions` 2543-2802, `remap_statement_symbols` 2803-2982, `expression_roots` 2983-3038, `proof_fact_expression_roots` 3039-3051, `copy_proof_fact_from` 3052-3115, `remap_proof_fact_symbols` 3116-3147.
- **Cases by shape.** The statement `match` in the copy (2557-2788), remap (2812-2980) and `expression_roots` (2989-3036) list all seven `StatementNode` variants with no wildcard.
- **Decides / notable behavior (read).**
  - Copy of `Assignment` sets `receiving_type` invalid (2613); remap of `Assignment` also resets it (2838-2843).
  - Copy of `Call`: `static_machine_parameter`, `receiver_root_symbol`, `receiver_symbol`, `target_symbol` copied as is (2668-2671); `evidence_arguments` cloned (2686); `installed_interrupt_operation` copied with `invalidate_preparation()` (2650-2664). Remap of `Call` remaps `receiver_symbol`, `receiver_root_symbol`, `target_symbol` (2875-2879) but not `static_machine_parameter`; `TransitionTargetNode::Named.static_machine_parameter` is also not remapped (3407-3415). Whether that is intended: not traced (open).
  - Copy of `Assembly(Label)` looks up the new block handle in `assembly_blocks`, `.expect("assembly window copying includes its owning header")` (2594): copying a span that begins mid-window panics.
  - Copying sub-statements for `reflected_field_origin.query` and `local.reflected_query_origin` uses a one-statement span `HandleSpan::from_parts(handle, 1)` (2640, 2718).
  - `expression_roots`: `LocalData` with role `AtomicObservedResult` contributes no initializer root (3010); `Call` contributes only argument handles (3000-3005), not `machine_arguments` or receiver; `Transition` contributes guard and the targets' arguments/values (3014-3035).
  - `copy_proof_fact_from`: copies expressions, domain path members, domain argument type references; for a proposition, only `arguments` are copied, `binder_arguments` and `proposition` are kept as is (3092-3104); source spans for facts are carried by index (3107-3113).
- **Callers (grep).** `remap_statement_symbols`: `pipeline/04.../lib.rs:224`, `body_cloning_and_rewriting.rs:1197`, `assembly_and_operators.rs:74, 1008, 1103, 1280` (6 outside). `copy_statement_nodes_deep_from`: `assembly_and_operators.rs:70, 690, 1005, 1102, 1172` (5 outside). `copy_statement_nodes_with_type_expressions`: `body_cloning_and_rewriting.rs:1670, 1678, 1683` (3 outside). `expression_roots` outside count not run.
- **Fail-open.** None seen (all copies are of existing nodes; panics on contract violations).

### 2n. `control_flow::statement::returns` 3150-3197
- `direct_state_return_values` 3157-3196: scans a state's statements. A `Transition` must be `TransitionExit::Ordinary` with a valid target (3165-3169); every target must be a `Value` target (3170-3178); returns after an unconditional transition or one with a continuation (3181-3185); a trailing `Expression` statement counts as a return value (3187-3190); any `Assembly` statement gives `None` (3191); every other statement kind (Assignment, Call, LocalData, AssemblyFact, Expression not in last position) is ignored (`_ => {}`, 3192).
- **Fail-open (traced inside).** 3192: non-return statements before the return are skipped, so "direct" means "return values reachable when ignoring other statements". If the scan runs out with no unconditional return, returns `None` (3195). Whether callers treat `Some` as "the state is only a return": not traced. Status: open. Callers (grep count with `reachable_*`): 19 occurrences across 9 files incl. 6 in this file; users in `05.../execution_terminal_unit_state_graph.rs`, `..._control_statement_sequence.rs`, `execution.rs`, `validation/src/reference_result_custody.rs` etc.

### 2o. `control_flow::statement` types and `StatementTable` 3198-3792
- Free functions `reachable_transition_prefix` 3210-3218 (cut after first `Always` transition), `reachable_state_statements` 3220-3229 (keep everything before the first transition, then the reachable prefix from there).
- `StatementTable` 3234-3243: eight arenas (`assembly_graphs`, `assembly_blocks`, `statements`, `expression_handles`, `name_path_members`, `transition_targets`, `transition_patterns`, `outcome_proof_selectors`). `revision()` 3515-3523 sums the revisions of six of them: `statements`, `expression_handles`, `name_path_members`, `transition_targets`, `transition_patterns`, `outcome_proof_selectors`; `assembly_graphs` and `assembly_blocks` are not included (read). Accessors 3245-3538 (`insert*`, `statements`, `statement_at`, `indexed_statements`, `iter_statements`, `transition_target*`, `named_transition_target`, `transition_targets`, `transition_target_count`). `copy_transition_target_deep_from` 3335-3387 and `remap_transition_target_symbols` 3389-3421 are per `TransitionTargetNode` variant (Named, Value, Terminal).
- `StatementNode` 3560-3568 (7 variants); `as_*` accessors 3570-3612; `TableCall` 3640-3662 (17 fields); `TableLocalData` 3671-3682; `TableTransition` 3716-3724 with `targets()` 3727 (target and continuation if valid); `TransitionGuardNode` {Always, When(expr)} 3762; `TransitionExit` {Ordinary, Crash(cause)} 3755; `TransitionTargetNode` {Named{...}, Value, Terminal(default)} 3768-3783; `TableNamePath` 3786-3791.
- `StatementRootRole` 3547-3557.
- **Callers (grep, two name groups).** `named_transition_target|transition_target_count|state_parameter_position`: 88 occurrences in 38 files (mixed, not split). `statement_at` and `indexed_statements` appear many times in `validation/src` (e.g. `value_custody_core.rs:4560, 4882`, `recasts.rs:788, 4430, 5304`) so they are not unused.

### 2p. `names::identity` 3796-3891
- `normalized_schema_report_fingerprint` 3800-3890: FNV-1a over `"omega.schema.v2"`, then fields and cases of a data definition. For each field/case the member name is hashed by identity when it has a stable `identity: Option<u64>` (`byte(1)`, `u64`), otherwise by position and name (`byte(0)`, position, name, 3804-3821). Fields and cases are sorted by identity only if every one has an identity (3840-3845); payload fields likewise (3859-3861). Types are hashed by the rendered string `typed.display_type_reference(..)` (3853, 3874). Retired identities included sorted (3876-3888). Returns `hash.finish().max(1)` (3889).
- **How it decides.** Display string of a type is the key for type identity here (3853, 3874). Position is part of the key if identities are missing.
- **Same job elsewhere.** `build-time-evaluation/src/layouts.rs:6749-6750` defines a `pub fn normalized_schema_report_fingerprint` that just calls this function (read via grep content). Callers of the typed-trees function: `validation/src/value_custody_core.rs:7565`, `layouts.rs:6728, 7043`, plus the wrapper.

### 2q. `names::name` 3892-3974
- `Identifier` 3898-3901 wrapping `IdentifierText` {Missing (default), Static(Cow<'static,str>), Generated(Arc<str>)}. `as_str` returns `""` for `Missing` (3927). `PartialEq` is by string content (3968-3973), so `Missing` equals an empty generated name. `generated`, `generated_static`, `is_self_receiver`, `is_reserved_result` (delegates to `language_core`). `Debug` prints the string.
- This type is the "name" carrier used as a key in many places seen in this unit (e.g. `Identifier` comparisons at 7362, 7404, 1596, 1643, 1953).

---------------------------------------------------------------------------
## Cross-cutting items

**Keys that are source spelling (read):** `binder_operand` name match when no symbol (calls_and_control_flow.rs:888-893); `literal_operand` const lookup by name (924-932); `call.target == requirement.signature.name` (1596, 1613) and `call.target.as_str() != signature.name.as_str()` (1643); `requirement.name == *requirement_name` (1953); `domain_full_name.split("::")` (2060); `value_fits_carrier` parses `format!`-built identity strings (977-980, 990); schema fingerprint hashes `display_type_reference` strings (3853, 3874); lifetime matching by name at callable_applications.rs:7362, 7404, 7327-7329 (`lifetime_parameter_identity(.., binding.parameter.as_str())`).

**Stated limits found in this unit (quotes under 20 words):** callable_applications.rs:7507-7509 "recursive nominal callable application is not yet represented by cycle-aware applied telescopes"; 6571 "requirement invocation lost its prepared member binder"; calls_and_control_flow.rs:2457-2459 "conditional assembly control must be normalized before finite execution"; 766 "the finite family must be one explicit disjunction of complete binder equalities".

**Panic-on-violation sites (not fail-open):** calls_and_control_flow.rs:813 (`expect("complete alternative tuple")`), 2198, 2355-2356, 2374-2375, 2594.

---------------------------------------------------------------------------
## Coverage

- Read in full: `callable_applications.rs` 6301-7775 = 1475 lines (I also read lines 6285-6300 as context for the enclosing function; those do not count). `calls_and_control_flow.rs` 1-3975 = 3975 lines. Total read in full = 5450 lines.
- Skimmed: none.
- Not read: none (0 lines).

## Five things a maintainer most needs to know about this unit

1. `AppliedCallableOwner` (callable_applications.rs:7191-7239) is the single list of retained row kinds, and every consumer re-lists a different subset of its variants by hand (7027-7033, 7049-7052, 7073-7087, 7099-7105, 7120-7125, 7141-7146). Liveness is defined only for `RequirementInvocation` and `Realization` (7098-7106); the other owner kinds are always live.
2. Requirement signature lookup has four independent routes (closed view, prepared invocation row, evidence-bound outer signature, trait-declared signature) in `requirement_invocation_signature`/`requirement_outer_signature` (6331-6498), and the closed-view route returns the closed signature without a scope check when no requirement application was written (6386-6394). The statement-call and expression-call entry points differ only in the arguments they pass (6301-6329, 6500-6538).
3. `finite_declaration_family` (calls_and_control_flow.rs:720-775) keeps a `Finite` result when a later `requires` fact is non-finite or non-expression (759-772), which differs from its own error text at 766 about "one explicit disjunction". Dynamic-trait eligibility (217-235) and value-binder custody checks depend on it.
4. `dynamic_non_signature_eligibility` rejects only when a symbol is positively found private (267, 467-516); unknown symbols and symbols from `TypeConstraintNode::Named`/`ArithmeticDomain` (545-546) are not rejected.
5. Statement copying/remapping (2543-2982) covers all seven statement kinds exhaustively but copies `static_machine_parameter` without remapping it (2668, 2875-2879, 3407-3414), resets `Assignment.receiving_type` (2613, 2838-2843), and panics on partial assembly windows (2198, 2594); `StatementTable::revision` omits the two assembly arenas (3515-3523).

## What you are unsure about

- Whether the `Ok(None)` / `continue` skips in 1a-1b (6548-6559, 6779-6781, 6821-6823) are reachable with a wrong program; I did not trace the callers at `contextual_and_requirement_calls.rs`.
- Whether the missing `static_machine_parameter` remap (2875-2879, 3407-3414) is intentional; I did not read what populates that field.
- Whether `references.rs` or `types.rs` already hold another symbol-collecting walker equivalent to `collect_type_reference_symbols`/`collect_expression_symbols` (calls_and_control_flow.rs:327-585); a name grep found none in typed-trees `src`, but other names may do the same job (suspected, not read).
- Exact outside call-site counts for `state_parameter_position`, `named_transition_target`, `transition_target_count`, `AppliedCallableSignature::requirement_*_symbol`, `expression_roots` and the dynamic-slot methods; the greps I ran either mixed names or included own-file uses, so only the counts stated above are reliable.
- What `CallableApplication::type_reference`, `signature`, `facts`, `parameter_kind` do with the `elided_lifetimes` and `Default` region values set at 7370-7376, 7625, 7629 (those engine methods are in the earlier reader's range, not mine).
