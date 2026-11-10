# Unit 85 notes

Files in unit (all under `omega-rust/psi/compilation/representations/typed-trees/src/`):
1. `callable_applications.rs` (7775 lines) - read lines 1-6300 only
2. `calls_and_control_flow.rs` (3975 lines) - NOT read
3. `declaration_tables_and_index.rs` (6481 lines) - NOT read
4. `declarations.rs` (3445 lines) - NOT read

I ran out of working room. Only part of file 1 was read. Everything below is about `callable_applications.rs:1-6300` unless stated. Paths below are relative to the typed-trees `src/` folder.

## callable_applications.rs (lines 1-6300 read)

File is one header comment (1-5: "closed callable applications ... applied machine instances, trait and conformance scopes, domain scopes, and requirement instances") and a set of `pub(crate) mod` blocks. Lines 6301-7775 not read. The struct `CallableApplication` and enums `AppliedCallableOwner`, `AppliedCallableSignature`, `CallableLifetimeBinding`, `CallableLifetimeOrigin`, `CallableSubstitution`, `AppliedMachineCallable(Id)`, `ClosedConformanceApplication` are used here but defined outside the range I read (probably in lines 6300+ or in other files; inferred, not read).

### Purpose of the whole file (inferred from the read part)
"Apply" a generic callable/type/trait/domain to a concrete tuple of arguments: build a substituted copy of a signature, type reference, expression or proof fact (`CallableApplication`), retain the result as a row (applied-callable table, `StaticMachineApplication` rows, specialization/capture receipts), and answer later queries that ask "is this the same application as that one".

### Module `call_targets` (7-442)
- Purpose: when a copied expression calls a Machine-typed parameter, replace the target by the machine the caller supplied.
- Shape: `CallableApplication::retain_selected_call_application` (applications, 14), `retain_prepared_call_application` (173), `retained_application_arguments` (243), `AppliedCallTarget` struct (304: parameter, target, name, attached_data, supplied, provider), `selected_call_target` (314), `apply_call_target` (409).
- Inputs/outputs: reads `self.static_arguments`, `self.selected_providers`, `self.types`, machine/state tables; mutates a `TableCallExpression` (`machine_arguments`, `authored_machine_application`, `target_symbol`, `target`, `receiver`) and inserts new `StaticMachineApplication` rows (`retain_static_machine_application`, 150, 215).
- How it decides: keyed on symbol identity: `parameter` must have `SymbolKind::MachineParameter` (320); the selected symbol must be Machine, State or MachineParameter (361-366); if the selected symbol equals the parameter and has no application it returns `Ok(None)` (369-375). A machine selection resolves to the machine's first state (379-386). Argument count must equal `parameters.len() + evidence.len()` (88).
- Cases by shape: kind of each parameter is matched: Machine, Const, Value (refused: "unclosed runtime operand", 105-107), everything else becomes a Type binding (108). Two argument lists for one declaration refused (56-60).
- Fail-open: `selected_call_target` returns `Ok(None)` (leaves call alone) when the parameter is not a MachineParameter, or when no static argument/provider/type is found for it (346-353), or when selected == parameter with no application (369-375). Status: open (I did not trace callers). A call to an unsubstituted machine parameter stays as it was instead of being refused.
- Errors are `&'static str`, all fail-closed except the above.

### Module `closed_operands` (443-695)
- Purpose: check that the operands of a requirement instance contain no open binders.
- Shape: `ClosedOperands` (447), `OperandClosure` (453, visitor implementing `TypedReferences`), `TypedTrees::require_closed_application_operands` (525), `require_declared_application_operands` (537), `require_owned_application_operands` (545), `require_receiving_trait_operands` (553), `require_declared_trait_operands` (560), private `require_trait_operand_bindings` (567) and `require_application_operand_bindings` (586).
- How it decides: collects symbols and type references reachable from the arguments, then rejects if any symbol kind is TypeParameter, MachineParameter, ConformanceParameter, PropositionParameter, PropositionMachineParameter, Local or Parameter (637-646), except Type/Const data parameters whose parent is an ancestor of `lexical_owner` when `allow_declared_binders` (617-636). Also rejects a named data type that still has type or lifetime parameters ("omits a nominal telescope", 651-659) and requires an exact-region identity or a region-free identity (661-691).
- Same job elsewhere: the three `require_*` public wrappers differ only in two flags; 5 of them have callers in this file (525-560) - not grepped for outside users.
- Fail-open: line 612-616 filters `symbol.is_valid()` so invalid symbols are skipped (traced within the function only; stated: no). `exact_regions` false with invalid `lexical_owner` falls to the region-free check (673-678).

### Module `conformance_scopes` (696-969)
- Purpose: compare and prepare "closed conformance applications" (a chosen implementation plus its arguments).
- Shape: `portable_conformance_application_identity` (704), `..._in_owner` (719), `closed_conformance_applications_match` (750), `prepare_closed_conformance_scopes` (780), `prepare_closed_conformance_occurrence_scope` (792), `conformance_selection_region_supplier` (810), `closed_conformance_region_supplier` (828), `prepare_closed_conformance_scope` (889).
- How it decides: `closed_conformance_applications_match` is true if `left == right`, else needs same selection metadata, both trait scopes retrievable, same valid `argument_owner`, same application, and equal portable identity in that owner (755-777). Identity is built from a `StaticMachineArgument` and `portable_applied_index_identity` (729-747). Region supplier for an Invocation site checks the retained specialization receipt field by field (843-882).
- Callers of `closed_conformance_applications_match` (grep, content mode): `declaration_tables_and_index.rs:2591`, `monomorphization.rs:1050`, `monomorphization.rs:1281` (3 outside call sites plus definition 750).
- Fail-open: none seen; every miss returns false/Err. Line 755 `left == right` shortcut is structural equality.
- One `format!` of Debug values appears only in an error message (927-933, 953-964).

### Module `data_cleanup_scopes` (970-1053)
- `prepare_closed_data_cleanup_application` (976): for a generic data type whose definition has a `cleanup_selection`, builds a `CallableApplication` with the type arguments and calls `apply_cleanup_selection` (1043), then pushes `AppliedDataCleanupSelection`.
- Returns `Ok(false)` (do nothing) when the reference is not Generic, no unique definition, no cleanup_selection, parameter/argument count mismatch, any non-Value argument not closed (`domain::index_argument_is_closed`, 1003-1010), or already prepared (1013-1018). Not fail-open in the accepting sense: "false" means "no cleanup application prepared". Status: open (what a caller does with `false` not traced; the one caller is `monomorphization.rs:1504` by grep).

### Module `domain_scopes` (1054-1384, tests from 1208)
- `CallableApplication::domain_identity` (1060): returns `original` unchanged if no arguments or declaration invalid or no domain definition (1066-1071), else interns a canonical instance name. This is a silent pass-through: an unknown domain keeps its old identity. Status: open.
- `prepare_closed_domain_application` (1084): skips if same (declaration, identity) already stored (1091-1095); errors on generic carrier (1100) or incomplete telescope (1104); copies and substitutes the domain definition, clears `type_parameters` and `index_arguments`, rewrites `semantic_id` and the two semantic roles to the new identity (1145-1154).
- `apply_cleanup_selection` (1168): copies a `StaticMachineApplication` row with substituted arguments.
- Two unit tests (1217, 1285) about domain carrier matching and domain constraint identity.

### Module `machine_instances` (1385-1689)
- Purpose: walk instance receipts (`machine_specializations`, `captured_machine_applications`).
- Shape: `MachineInstantiation` enum (1394: Specialization or Capture), `EnclosingInvocationFrame` (1399), `enclosing_invocation_frames` (1422), `machine_instantiation` (1508), `machine_instantiation_path` (1530), `implementation_capture` (1550), `instantiated_machine_state_matches` (1569), `static_machine_binding_selects_target` (1631).
- How it decides: all keyed on symbol handle equality and receipt `instance`/`template` fields. More than one receipt for an instance is an error (1524). Cycles are detected by a visited list (1428, 1539, 1651). State matching is by ordinal position in the machine's state list; lengths must be equal (1589).
- `static_machine_binding_selects_target` (1631): returns `Ok(false)` if the binder has no retained contract (1637) or if the owner has no specialization receipt (1659-1661). It compares machine *entry states* of the argument and the target (1682-1683), requires `argument.application.is_none()` (1681).
- Fail-open: none seen (miss gives false).

### Module `machine_selections` (1690-2738)
- `queries` (1691-2200): `same_selected_provider` (1706), `same_selected_bindings` (1727), `machine_selection_lifetimes_are_current` (1773), `has_applied_machine_selection` (1813), `applied_machine_callable` (1828), `applied_machine_callable_at` (1850), `applied_machine_callable_matches` (1888) / `applied_machine_callable_mismatch` (1914), `same_caller_operand` (2076), `same_callable_index` (2098), `same_callable_operand` (2120), `applied_machine_callable_origin` (2132), `same_registered_machine_operation` (2172), `applied_machine_callables` (2186).
  - How it decides: `applied_machine_callable_mismatch` finds a stored row for the invocation and runs a list of 18 named equality checks (1961-2065); the first failing check's text is returned. Operand equality is `left == right` or equal `applied_type_identity` / `applied_index_identity` in the owner (2098-2130). Identity of operands is therefore by normalized type identity, not by spelling.
  - Same job twice: `same_selected_bindings` (1727-1771) and the inline provider-bindings check inside `applied_machine_callable_mismatch` (2026-2046) both compare binding lists by parameter and then by Type/Const identity; the inline one falls to `previous.value == current.value` for Machine and Conformance values (2041) while `same_selected_bindings` uses identity for those (1745-1767). So Machine/Conformance values are compared structurally in one and by identity in the other (read).
  - `applied_machine_callable_origin` (2132) returns `Some(invocation)` itself when it has no `callable_contract_source` (2144-2146).
  - Callers of `applied_machine_callable_matches` (grep): `machine_parameters_and_bindings.rs:5446`, `contextual_and_requirement_calls.rs:174`, `:526`, `:1615` = 4 outside call sites.
  - `same_registered_machine_operation` (2172): left and right count as the same operation if equal, or one is a machine head and the other is its unique entry state (2177-2183).
- `prepare_applied_machine_callable` (2214), `prepare_selected_machine_contracts` (2442), `applied_machine_provider` (2567): build a `StateSignature` for the selected machine from machine fields (2313-2333) or from a forwarded Machine-parameter signature (2341), substitute through `CallableApplication::signature`, and store a `MachineSelection` row, replacing a previous matching row (2393-2436). `prepare_selected_machine_contracts` errors when two rows give conflicting providers for one demand (2473-2476); rows are skipped (`continue`, 2462-2469) when stale (lifetimes no longer current or ordinal/entry no longer in the selected list). Skipped stale rows are silently left out, not refused. Status: stated by structure only; not traced.
- Termination guarantee: only `Published` interface is copied; anything else becomes `NoGuarantee` (2326-2331, and again at 3832-3837).
- `record_tests` (2599): one test round-tripping the table through `compiler_storage`.

### Module `namespaces` (2739-2975)
- `CallableApplication::namespace_symbols` (2747): for an evidence (conformance) binder, maps binder and its State children to the chosen implementation's rows. `selected_implementation_namespace_symbols` (2830), `implementation_namespace_symbols` (2847), `named_conformance_call_signature` (2904), `resolve_named_conformance_call_signature` (2916).
- Decision keys: symbol identity and the unique matching row; "no unique exact requirement row" error (2894-2896). `resolve_named_conformance_call_signature` returns `Ok(None)` (falls through to ordinary dispatch) when a requirement application is already valid or the receiver is not a conformance (2920-2935). `named_conformance_call_signature` (2904) turns any Err into `None` (2908-2910).
- Cases by shape: only whole-declaration, non-generic implementations (2938: "named public view has no complete selected application"); needs a "complete qualified call" for every use of the binder (2798-2803).

### Module `payloads` (2977-3764)
- Purpose: the substitution engine. `CallableApplication::type_reference` (2983), `type_reference_with_owner` (3033), `type_payload` (3061), `type_arguments` (3395), `expression` (3413), `static_arguments` (3710).
- `type_payload` matches every `TypeReferenceNode` variant (Named, Reference, Constrained, FixedArray, Slice, Generic, DynamicTrait, ConstExpression, Unit) and rewrites it; no catch-all `_ =>` arm in the node match (3072-3348).
- `expression` (3413): copies an expression tree, then patches names by *source spelling*: a one-member unresolved name is looked up in `self.source_binders` by `name == member` (3438-3446). This is a decision keyed on identifier text for names with no resolved symbol (guarded by `path.head_symbol.is_valid() || path.symbol.is_valid()` skip at 3435). Then it rewrites only these expression kinds: Name, StructLiteral (with application_type), Match (Destructure patterns with application_type), Cast, TypeExpression, ZeroValue, Call; everything else is `_ => None` and kept as copied (3660). Comment-free list of cases; a node kind not listed is copied without substituting types inside it. Status: open (I did not check which other expression kinds carry type references).
- Fixed array lengths: only literal, const-parameter, and expression lengths handled; a `Named` const argument that is neither a numeric literal nor an integer const becomes `ConstParameter` again (3195-3199); a non-const-index argument errors (3211-3215).
- Proof-fact copy `facts` is in `signatures` module (5298).
- Stated limits: "compound nominal callable proposition operands are not yet represented" (5397, error string); "nominal callable array extent argument is not a const index" (3213).

### Module `realization_scopes` (3765-3886)
- `prepare_realization_contracts` (3771): for a generic conformance, builds a `Realization` row for each realization state owned by the conformance (3792-3797, 3802-3847). Returns `Ok(())` immediately if the conformance has no type or lifetime parameters (3780-3783): non-generic ones need no row.
- `realization_call_signature` (3851): requires exactly one selection (3868-3870), else `None`.

### Module `requirement_instances` (3887-4486)
- Types: `RequirementLocalArgument` (3896: Static(handle) or RuntimeValue{carrier}), `ClosedRequirementApplication` (3923, a view with 11 fields).
- Functions: `closed_requirement_applications` (3964), `closed_requirement_application` (3972, via `closed_requirement_signature`, defined outside the part I read), `requirement_application_matches_scope` (4017), `requirement_realization_specialization` (4059), `requirement_realization_matches` (4150), `instantiate_requirement_application` (4197).
- `instantiate_requirement_application` dedupes: it searches existing requirement instances with a long conjunction of equal declaring trait, template, trait-application identity, portable identities, lifetime rows and per-binding identity (4272-4371) and returns the existing symbol. Comment at 4347-4349: without the Machine/Conformance arm "each get their own instance under one canonical identity, which lowering rejects as a duplicate". The final `(left, right) => left == right` arm (4368) compares remaining binding kinds structurally.
- Note: line 4283-4289 compares local arguments only by `std::mem::discriminant` (variant kind, Static vs RuntimeValue), not by value; value identity is covered by `portable_local_arguments` (4282). Read.
- `requirement_realization_matches` (4150): if the requirement has no local arguments, accepts only when the path has no specialization and the provider machine has no type parameters (4173-4179).
- Cases by shape: `instantiate_requirement_application` accepts only these (parameter kind, binding kind) pairs: (Type, Type), (Const|Value, Const) with valid actual, (Machine, Machine), (Implementation, Conformance), (Value, RuntimeValue); `_ =>` error "no represented complete static operand" (4413-4447). Fail-closed.

### Module `selected_operands` (4487-4906)
- `static_operand_region_supplier` (4492), `static_operand_from_type_reference` (4545), `static_const_literal_from_type_reference` (4635), `static_operand_type_reference` (4670), `static_operand_type_reference_in_scope` (4677), `selected_value_expression` (4709), `selected_argument_reference` (4777). One test (4858).
- Decision by source spelling: `static_const_literal_from_type_reference` (4649-4665) parses the *name string* of an invalid-symbol `Named` type reference: leading '-', prefixes "0b", "0o", "0x", else decimal; a parse failure gives `None`. Also `selected_value_expression` decodes a `Named` with invalid symbol as a canonical const atom string (4723-4727). These are numeric/const values carried as a type-reference name (spelling-keyed carrier).
- `static_operand_type_reference_in_scope` (4677): after copying, every newly created type reference with no lifetime owner gets `owner` (4694-4703); this is by arena index range `>= start` (4685-4691), an allocation-order dependency.
- `static_operand_region_supplier` returns `lexical_owner` when the operand has no type reference (4500-4502) or when supplier is invalid and it has no lifetimes and Lexical origin (4519).

### Module `signatures` (4907-5431)
- `applied_signature_authored_parameters` (4913), `applied_signature_parameter` (4949), `applied_signature_contract_fact` (4972), `CallableApplication::signature` (5002), `parameter_kind` (5189), `parameters` (5267), `facts` (5298).
- `signature` (5002): temporarily extends `self.symbols`, `self.lifetimes`, `self.source_binders` and truncates them after (5009-5012, 5182-5185). It copies the declared `StateSignature`, substituting type parameters, bounds, parameters, return type, contracts, invokes, and termination premises. Fresh symbols are generated as children of the owned signature symbol or as fresh roots (5065-5083).
- `applied_signature_contract_fact` (4972): matches a fact by position offset in the contract list; requires equal contract counts, kinds, and fact counts (4981-4986).
- `facts` (5298): handles Expression, Membership, Proposition. Proposition head replaced only if the replacement is a `Named` type reference (5357-5364); binder arguments accept Named or Integer ConstExpression; anything else errors (5395-5399).
- `parameter_kind` (5189) matches all 7 kinds of `TypeParameterKind`.

### Module `table` (5433-6130)
- `AppliedCallableTable` (5487): wraps `Arena<AppliedCallableSignature>` plus per-invocation linked lists (`invocation_heads`, `invocation_groups`, `row_links`) so lookups visit only rows of one invocation. Module doc (5434-5438).
- Functions: `insert` (5606), `replace` (5612), `free` (5626), `replace_signature` (5634), `set_required` (5646), `machine_rows` (5780), `requirement_rows` (5787), `invocation_rows` (5795), `publish_row` (5691), `remove_row` (5739), `recover` (5587). `Record` impl (5520) rebuilds the links on decode and charges allocation for them (5533-5538).
- Only `MachineSelection` and `RequirementInvocation` owners are indexed (5455-5469); other owner kinds live only in the arena.
- Tests 5818-6129 (6 tests): ordering, generation reuse, record round trip, allocation budget.
- Note: `set_required` only works on MachineSelection rows (5654-5660).

### Module `trait_scopes` (6131-... read to 6300)
- `selected_requirement_contract_scope` (6139): requires exactly one supplying map application (6155-6157); if `dispatch.evidence_parameter` is invalid it returns the selected scope directly without cross-checking against any evidence bound (6163-6165). If valid, it requires a unique specialization receipt, rebuilds the required scope from the evidence bound and compares with `same_application` (6240).
- `requirement_call_signature` (6246) turns an Err into `None`. `requirement_call_signature_in_scope` (6259): resolves a named conformance call first; for ordinary positional calls with no requirement_application, exact_coercion_target, build_selection, suspend/block acknowledgement and no local arguments, it uses the supplying scope's signature filtered to `type_parameters.is_empty()` (6268-6296); anything else falls back to `requirement_call_signature`. Callers (grep): `expression_types_and_fields.rs:4988`, `contract_entailment.rs:42069` (2 outside call sites).
- Lines 6300 onward (comment "The same logical view with the reason its exact source cannot supply it") not read.

### Not-read part of callable_applications.rs: 6301-7775 (1475 lines).

## Other three files: not read
`calls_and_control_flow.rs` (3975), `declaration_tables_and_index.rs` (6481), `declarations.rs` (3445). I did not open them. No claim is made about them. The only thing I know is from grep: `declaration_tables_and_index.rs:2591` calls `closed_conformance_applications_match`.

## Coverage
- Read in full: `callable_applications.rs` lines 1-6300 = 6300 lines.
- Skimmed: none.
- Not read: `callable_applications.rs` 6301-7775 (1475); `calls_and_control_flow.rs` 1-3975 (3975); `declaration_tables_and_index.rs` 1-6481 (6481); `declarations.rs` 1-3445 (3445). Total unread 15376. Total unit 21676.

## Five things a maintainer most needs to know (for the part I read)
1. `CallableApplication` (the substitution state) is one engine for types (`type_payload`, 3061), expressions (`expression`, 3413), proof facts (`facts`, 5298), signatures (`signature`, 5002) and static arguments (`static_arguments`, 3710); expression copy only rewrites seven node kinds and copies the rest (3660).
2. Names with no resolved symbol are bound by source spelling in `expression` (3438-3446), and integer/boolean const values travel as the *name string* of an invalid-symbol `Named` type reference (4649-4665, 4723-4727).
3. "Same application" is decided by normalized type/index identity (`applied_type_identity`, `applied_index_identity`) with a `==` shortcut, in three separate places that are written out by hand: `same_selected_bindings` (1727), the inline check in `applied_machine_callable_mismatch` (2026-2046), and the dedupe in `instantiate_requirement_application` (4272-4371); Machine/Conformance values are compared by identity in two and structurally in the inline one.
4. Retained rows are stored in `AppliedCallableTable` with per-invocation linked lists that are rebuilt on decode; only MachineSelection and RequirementInvocation rows are indexed (5455-5469).
5. Several `pub fn ... -> Option` wrappers turn any refusal into `None` (`named_conformance_call_signature` 2904, `requirement_call_signature` 6246), so callers of those cannot tell "absent" from "refused".

## What I am unsure about
- Where `CallableApplication`, `AppliedCallableOwner` and the other core types are defined (probably in the unread 6301-7775 or another file).
- Whether the `Ok(None)` / `Ok(false)` early returns noted above can be reached with a wrong program; I did not trace callers.
- Everything in the three unread files.
