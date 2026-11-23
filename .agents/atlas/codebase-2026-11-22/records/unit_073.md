# Record: unit_073 (area 05-source-representations, crate `typed-trees`)

Checkout `4f3c255d0559`. All paths are under
`source-rust/subsystems/compiler/source-to-psi/representations/typed-trees/` (written `TT/`). Every file was read in full.
Counts below come from the Grep tool in count mode over `source-rust/` (commands noted where a number is given). Where one grep
joined several names, the number is the combined total and says so. "Seen" means read; "inferred" means not read at the other site.

---

## 1. `TT/definitions/product_operand/formation/applications.rs` (948 lines)

1. **Purpose.** A Build can choose a provider whose slot and implementation are applied to static arguments, for example `Reading<Tag>`.
   The request carries each argument as a name graph. This file forms the application among the Product program's declarations: the
   declaration plus one closed argument per parameter (`applications.rs:1-24`).
2. **Shape.** `enum Stop { Open, Unserved(Cow<str>), Rejected(String) }` (`:45`); `OperandSource { Authored(&ProductSourceLookups), Retained(SymbolHandle) }` (`:62`);
   `enum Closed { Type, Integer, Boolean }` (`:68`); `struct Lower` (`:79`, fields `program`, `source`, `graph`, `environment`, `helper`, `selecting_operand`, `nodes`);
   `Lower::{builtin_type :97, symbol :149, parameter :195, supplied :242, argument :277, bare_name :328, applied_arguments :348, ty :429}`;
   `fn bound` (`:504`); entry `pub(super) fn form` (`:579`); `prepare_boundary_requirements` (`:832`). Test at `:899`.
3. **I/O.** Reads a `ProductDeclarationOperand`, its `ProductStaticOperandGraph`, an optional `ProductStaticEnvironment`, the `TypedTrees` symbols, data
   definitions, trait/conformance tables. Mutates `TypedTrees`: type reference table, symbol table (`install_integer_types` `:100`), expression table
   (`retain_static_machine_application` `:778`), and retains a `ProductOperandApplication` (`:810`).
4. **How it decides.** Declaration lookup keys on a source span anchor (`rejoin_product_occurrence` then `symbol_provenance_source_span == declaration`, `:172-178`),
   not on a display name, when the name has an anchor. Without an anchor it resolves by path string and source span through `declarations.resolve(path, span, role)` (`:165-167`).
   Builtin type names are decided by spelling: `is_builtin_type_spelling(&name.text)`, `from_decimal_spelling`, `atom.symbol_name() == name.text` (`:111-123`);
   `true`/`false` are decided by string match (`:333-336`). Role is decided by `ProductOperandRole` and by which table holds the declaration (trait definition, conformance) (`:641-675`).
   Parameter kind is decided by `TypeParameterKind` match in `bound` (`:510-568`); literals must fit the landed type (`:526-539`).
5. **Cases by shape.** Comment `:19-24`: served are type arguments that are a builtin type, plain or applied data, or a fixed array with a literal extent, and value arguments
   that are an integer literal or a Boolean; "Every other form is refused by name". Refused by `unserved(...)`: projected argument (`:287`), lifetimes or member on an application (`:314`,`:617`),
   computed array extent (`:471`), `CanonicalConst/ConstExpression/Reference/Slice/Constrained/Dynamic/Invalid` types (`:486-494`), `Value/Machine/Implementation/Proposition` parameters (`:559-567`),
   roles other than `BoundarySubject|RootGrantSubject|Provider|Requirement` (`:595-604`), declarations with lifetime parameters (`:676`). Node cap `NODES = 4096` (`:77`) is an arbitrary size limit.
6. **Same job elsewhere.** `static_environment.rs` (this unit) captures the same graph in the opposite direction (`capture_closed_operand`). `bound` argument-landing logic resembles
   `static_operand_binding` (`prepare_requirement_application` called at `:796`, not read). Suspected.
7. **Unused/one-caller.** `prepare_product_operand_application|prepare_retained_product_application|prepare_product_requirement_operand` joined: 18 occurrences in 9 files (grep, count mode; 8 of them in `formation/mod.rs`). `form` has 2 callers, both in `formation/mod.rs:47,73` (read).
8. **Stated limits.** `Stop::Unserved` doc: "A form this producer does not serve yet." (`:50`). Message prefix "no product route yet" is in `formation/mod.rs:96,100`.
9. **Fail-open.** `Stop::Open` with `ProductApplicationDemand::FormationOnly` returns `Ok(None)` (`formation/mod.rs:56-58`): the request as authored inside a generic helper forms nothing and is accepted. Stated at `formation/mod.rs:34-37`; a wrong program would be a helper call whose environment never arrives (that is checked when `Selected` demand is used; traced only in the two entries I read). `form` returns `Ok(None)` when `operand.application` is `None` (`:592-594`): stated "applies nothing". Otherwise none seen.

## 2. `TT/definitions/product_operand/formation/mod.rs` (242 lines)

1. **Purpose.** Public entries for preparing Product operands: plain declaration path vs applied slot/provider; error shaping.
2. **Shape.** `PreparedProductRequirement` (`:27`, `application`, `requirement`, `closed_requirement`, `trait_scope`); `prepare_product_operand_application` (`:38`), `prepare_retained_product_application` (`:67`),
   `prepare_product_requirement_operand` (`:110`), `served` (`:89`), `product_error` (`:165`). Two tests (`:175`,`:212`).
3. **I/O.** Calls `applications::form`; converts `Stop` to `Diagnostic` with the operand span (`:105-107`); for requirements calls `product_requirement_application` (`:130`, not read).
4. **How it decides.** Plain operand: resolves by `declarations.resolve(&operand.path, span, Requirement)` (`:141-147`), keyed on path string and source span.
5. **Cases by shape.** Retained operands must come back `Some` or error "lost its closed application" (`:82-86`).
6. **Same job elsewhere.** Authored and retained entries are two calls of one `form` with different `OperandSource`.
8. **Stated limits.** Test comment: a damaged application is refused, "rather than becoming an empty tuple" (`:173`).
9. **Fail-open.** See item 1. `served` never turns `Open` into success when demand is `Selected` (`:56-59`).

## 3. `TT/definitions/product_operand/input_selections.rs` (175 lines)

1. **Purpose.** Records which authored declaration-selection occurrence picked a given input of a retained Product operand, and re-derives the selected symbol on demand.
2. **Shape.** `ProductArgumentStep` (`:11`), `ProductTypeSelectionSlot` (`:19`), `ProductInputSelectionTarget` (`:25`), `ProductInputSelection` (`:36`, fields `occurrence`, `argument_ordinal`, `target`), `selected_symbol` (`:43`).
3. **I/O.** Reads `program.product_input_argument_steps`, the actual `StaticMachineArgument` slice, type reference table. Returns the `authored_origin` of the selected symbol (`:173`).
4. **How it decides.** Follows an index path through the argument tree (`:57-83`); for a type target, collects the argument's reachable types with a local `TypedReferences` visitor and requires the retained reference to be among them (`:88-132`), then matches `Named|Generic|FixedArray{ConstParameter|ConstCall}` by slot (`:136-167`). Keyed on identity (handles), not names.
5. **Cases by shape.** Only declaration slot on `Named`/`Generic` and array-extent slot on `ConstParameter`/`ConstCall` (`:136-167`); others return an error string.
9. **Fail-open.** None seen; every mismatch returns `Err(&'static str)`. Uses a `std::collections::HashSet<..., BuildIndexHasher>` (`:90`) for handle sets (data-oriented ratchet; coverage by the ratchet file not checked).

## 4. `TT/definitions/product_operand/mod.rs` (650 lines)

1. **Purpose.** Lookups of declarations that a Build request names (Product operands), plus a few shared pieces: entry-declaration view, request author lookup, native-binding formation digest, same-source comparison.
2. **Shape.** `is_declaration_operand` (`:44`), `resolve_product_operand` (`:67`), `resolve_product_path` (`:79`), `ProductDeclarationLookup` (`:90`, `resolve :103`), `PRODUCT_ROLES` (`:143`, nine roles), `ProductDeclarationIndex` (`:168`),
   `product_candidates` (`:226`), `core_arrival_requirement` (`:357`), `ProductEntryDeclaration` (`:408`, `from_declared_owner :423`, `from_machine :457`), `resolve_build_request_author` (`:477`), `ProductRequestAuthors` (`:485`),
   `ProductSourceLookups` (`:492`), `native_binding_formation` (`:548`), `same_product_operand_source` (`:606`), `same_product_request_source` (`:633`).
3. **I/O.** Reads `TypedTrees` tables per role; builds a sorted `Vec<ProductDeclarationName>` per role (`:178-224`). `native_binding_formation` hashes relative path, source bytes, callable identity and operand path with SHA-256 (`:577-589`).
4. **How it decides.** `resolve` binary-searches rows keyed on a path **string** (`name` and `display_path(symbol,"::")` both inserted, `:193-206`), strips a leading `alias::` only when `product_dependency_target(occurrence, alias)` exists (`:112-120`), then
   hands rows to `find_build_subject_declaration_from_source` for visibility by source occurrence (`:125-139`). `core_arrival_requirement` decides by `definition.name.as_str() == owner` after splitting the string at `::`, plus `core_vocabulary == ProgramStorageEntry` and `dependency_scope == Product` (`:367-383`), and requires exactly one match (`:384-403`). `ProductRequestAuthors::resolve` keys on exact span, error if zero or more than one (`:531-545`).
   `is_declaration_operand` accepts a type reference beside a path only when it names the same symbol (`:44-65`, comment `:28-43`).
5. **Cases by shape.** `PRODUCT_ROLES` has nine fixed roles; `WireCompatibilityFact` has no candidates (`:266`) and no position in `PRODUCT_ROLES`, so `product_role_position` returns `None` and `resolve` returns `None` for it (`:110`).
   Comment `:28-30`: a Build selection operand "is never an applied type, a conformance application, a value or an evidence projection".
6. **Same job elsewhere.** `same_product_operand_source` compares operands after `remap_source_spans`, same job as span rejoin in `applications.rs:172`. Declaration lookup by path string and span also appears in `source_symbols::find_build_subject_declaration_from_source` (not read).
7. **Unused/one-caller.** Joined grep `resolve_product_operand|resolve_product_path|core_arrival_requirement|native_binding_formation|from_declared_owner`: 19 occurrences in 8 files (includes definitions and tests; per-name counts not separated).
8. **Stated limits.** `:208-209` comment: "Stable sorting retains the original representative of an operator family and every competing exact declaration."
9. **Fail-open.** `ProductEntryDeclaration::from_declared_owner` returns `None` on incomplete rows (`:429-449`); the caller sees "no entry" (fail-closed for a missing entry, traced by reading only this file). `retained_source_names_same_declaration` fail-closed. None seen.

## 5. `TT/definitions/product_operand/occurrences.rs` (101 lines)

1. **Purpose.** Walks every machine body to find typed Build selection requests, with their author machine, state and expression.
2. **Shape.** `ProductSelectionOccurrence` (`:12`), `collect_product_selection_occurrences` (`:19`), `visit_build_selection_occurrences` (`:42`), private `Census` visitor (`:67`).
3. **I/O.** Reads machine states, statement table, expression roots; calls `references::visit_expression` (`:60`). Emits via callback.
4. **How it decides.** Keeps only `BuildSelectionRequest::Declaration` in the collector (`:27`); `Census::call_arguments` fires for any call whose `build_selection` resolves (`:92-97`); dedup by expression handle per state (`:80-89`, `seen` cleared per state `:57`).
9. **Fail-open.** None seen.

## 6. `TT/definitions/product_operand/static_environment.rs` (763 lines)

1. **Purpose.** Copies a Build helper's static arguments (its specialization receipt) or a closed Product application into a portable `ProductStaticOperandGraph`, so a later program can re-form it.
2. **Shape.** `capture_static_environment` (`:8`), `capture_closed_operand` (`:122`), private `Capture` (`:144`) with `name :154`, `names :175`, `operand :188`, `type_arguments :298`, `ty :310`, `constraint :454`, `expression_selections :480`, `expression :516`.
3. **I/O.** Reads `machine_specializations`, static machine applications, type reference table, expression table, authored selection ledger. Produces `Arc<ProductStaticEnvironment>` or a graph.
4. **How it decides.** Exactly one specialization receipt per helper instance (`:13-18`); binding declaration must equal the application's (`:30-34`). Atoms like `true`/numerics are recognized by `CanonicalConstValue::from_atom(argument.path[0].as_str())` (`:198-202`, `:339-345`): spelling. Cycles are detected by an `active_types`/`active_expressions` list (`:327`,`:528`). A selection occurrence must rejoin to itself via `source_coordinate_occurrence` (`:494-506`).
5. **Cases by shape.** `ty` refuses `Callable` types (`:332`) and types with alias applications (`:315-322`). `expression` handles Integer/Boolean/Float/String/Name/StructLiteral/ArrayLiteral/UndeterminedNaN/ZeroValue/TypeExpression/Unary/Binary/Member/Indexed/Cast/Call; everything else returns "Build specialization has a non-static or unmaterialized selected value" (`:743-746`). Calls with `runtime_call` or `native_code_layout` are refused (`:677-682`).
6. **Same job elsewhere.** Inverse of `formation/applications.rs`. The `Box<Unary|Binary>` operator mapping (`:610-646`) is a hand-written copy of the `expr::BinaryOperator` list into `ProductBinaryOperator`; any new operator breaks compilation (exhaustive match), so it does not drop silently.
7. **Unused.** `capture_static_environment`/`capture_closed_operand` are re-exported (`product_operand/mod.rs:19`); callers not grepped.
8. **Stated limits.** `:321` "cannot yet retain transparent alias application obligations"; `:333` "the static operand format cannot retain it".
9. **Fail-open.** `ty` caches by linear `Vec` search (`:323`), not a fail-open risk. None seen.

## 7. `TT/definitions/service_declarations.rs` (3,727 lines)

1. **Purpose.** Projects boundary traits, operators, bodyless boundary requirements and sealed accessors into `ServiceSurface` rows (the contract a provider promises), keeps them in a catalog bound to one typed program, and prepares the calling-policy ABI signature census.
2. **Shape (inline modules).**
   - top: `service_declaration_scope :13`, `service_declaration_commitment :32`.
   - `provider_declarations` (`:50`): `closed_provider_requirements :59`, `closed_provider_leaf_applications :115`, `requirement_reach :159`, `ProviderPlanningContext :186`, `PreparedProviderInventory :208` (`trust_declarations :299`, `origins :267`), `provider_target_declarations :338`, `satisfaction_provider_declaration :364`.
   - `projection` (`:467`): `applied_receivers` (`collect_scopes :474`, `receiver_applications :524`), `correspondence` (`InheritedRequirements :625`, `validate :707`), `ProjectedServiceSurface :827`, `from_typed :876`, `from_typed_application :883`, `from_typed_operator :946`, `from_typed_boundary_requirement :1031`, `project_boundary_machine_schema :1054`, `collect_service_methods :1201`, progress premises `:1296-1409`, entry/result claims `:1411-1593`, `service_reach_names :1608`, `invocation_entries :1658`.
   - `catalog` (`:1691`): `PreparedServiceCatalog :1766` (`assemble :1848`, `retain :2179`, `retain_named :2230`, `retain_sealed :2278`), `ServiceCatalog :1778`, `RetainedServiceSurface :1784`, `exact_identity :2332`.
   - `signatures` (`:2347`): `abi_signatures :2364` (`prepare :2415`), `calling_applications :2843` (`PreparedCallingApplications :2904`, `boundary_policy_instances :3116`, `prepared_policy_machine :3212`, `collect_boundary_signatures :3313`), `PreparedBoundarySignatures :3410`, `prepare_top_level_signature :3508`, `prepare_trait_signature :3554`.
   - `ProviderSchemaDeclaration :3711` (four variants).
3. **I/O.** Reads `TypedTrees` (traits, machines, conformances, operators, access view plans, captured applications, service reach rows, closed requirement applications). Produces `service_surface::ServiceSurface` values and `Arc<PreparedServiceCatalog>`; uses `OnceLock` caches in `PreparedProviderInventory` (`:212-213`).
4. **How it decides.** Scope membership by `symbol_is_in_dependency_scope` (`:196-199`). Catalog identity: `owner_identity` and `owner_address` pointer compare (`:1996-1998`), the inventory by `Arc` pointer (`:277`). Records are located by linear scan on `origin` equality; if zero or two match, returns `Handle::default()` (`:2010-2022`). Requirement identity is the callable identity **text** (`trait_requirement_callable_identity_text`) used as dedup key (`:1211-1219`) and sort key (`:570`, `:1400`, `:1437`). Service names are `String`s collected into `BTreeSet<String>` (`:1613`, `:1627`). `trait_name` choice uses `requirement.spelling.is_some()` (`:1134`). Owner/method split by `rsplit_once("::")` on a display path (`:1071`). Claim domain name: `semantic_domains.name(id)` falling back to `domain.name` (`:1531-1537`). Operation kinds `"read"/"take"/"write"` are string-matched in `access_views` (see §25), not here.
5. **Cases by shape.** `type_parameter_count`-style tables absent here. Quoted: `prepare_top_level_signature` "compatibility calling plans require one nongeneric bodyless top-level boundary requirement" (`:3518`); `BOUNDARY_NATIVE_PARAMETER_CAPACITY` limit refusal (`:3521-3525`, `:3609-3613`); `Calling<Policy>` requires exactly one policy per boundary trait (`:2942-2947`) and one argument (`:2951`); policy realization must be a nongeneric machine with `plan` row (`:3272`, `:3302-3306`); `checked_service_scope` (in service_reach.rs) accepts only `Const` and `Type` binder kinds (§19).
   `from_typed_operator` hard-codes empty `entry_claims`, `result_claims`, `service_reach`, `synchronous_invocations`, `may_suspend:false`, `may_block:false`, `terminates_guarantee:false` (`:1002-1024`).
6. **Same job elsewhere.** `retain_service_applications` (`:2070`) and `declared_traits` (`:1974`) both enumerate catalog records. Three code paths build a `PreparedAbiStorage` from an applied signature with near-identical code: `abi_signatures::prepare` at `:2551-2563` and `:2672-2683` (inside this file, same text twice) and at `:2497-2504`. The same refinement-scope walk appears in `trait_definition/applications.rs` (`visit`) and here (`collect_scopes :474`) with different cycle handling: `collect_scopes` returns an error on a cycle (`:487-493`); `visit_inherited` silently skips (`applications.rs:863-865`).
7. **Unused/one-caller.** Joined grep `check_refined_service_ceilings|closed_provider_leaf_applications|closed_provider_requirements|service_declaration_commitment|service_declaration_scope`: 14 occurrences in 7 files (including 5 in this file). `.try_visit(` has 11 occurrences in 10 files.
8. **Stated limits.** `:1-5` "We keep those contracts independent of target calling plans". `:3305` "generic calling-policy realization requires an explicitly specialized machine before policy evaluation". `:3611` "calling policies currently support at most".
9. **Fail-open.**
   - `retain_sealed` ignores `Err` from `ScalarAccessorDeclaration::authenticate` (`:2297-2302`): only `Ok(Some(..))` retains; an `Err` is dropped silently, so a malformed sealed accessor yields no catalog record and no diagnostic here. Status: open (not traced to consumers).
   - `retain_named` returns silently when `root` is `None` (`:2239-2241`). Open.
   - `abi_signatures::prepare` `continue`s when no declaration row matches `closed.template` (`:2438-2443`) and when `scope.portable_application_identity` is empty (`:2537-2542`). Open.
   - `collect_boundary_signatures` uses `root.visit_parents_first`, which skips a parent whose application is incomplete (see §13 item 9). Open.
   - `projection::correspondence::validate` returns `Ok(())` when the schema is not `BoundaryTrait` (`:716-721`): operators and boundary requirements get no inherited-requirement check; they have one row by construction (`:965`, `:1129`). Traced by reading.
   - `service_progress_premises_for_parameters` and `service_progress_establishment_routes` call `.expect(...)` (`:1321-1331`, `:1356-1380`): a malformed premise panics instead of refusing. Stated by message text only.
   - `append_*_claims` hard-code `CarryPolicy::STRICT` (`:1558`, `:1588`); 11 occurrences of `CarryPolicy::STRICT` in 5 files (grep). Not a risk to accepting; the retired carry policy remains a constant.
   - `normal_memberships.rs:62` skips unrestricted domains (§9).

## 8. `TT/definitions/service_declarations/inventory_records.rs` (19 lines)

Re-exports `symbols::trust_declarations::{AcceptedContractDeclaration, AcceptedMachineDeclaration, AcceptedTemplateDeclaration, TrustDeclarationCardinality, TrustDeclarations}` (`:4-7`) and defines `SelectedTargetMachineOrigin` (`:10`) and `ProviderRequirementApplication` (`:16`, two `String` fields: `requirement`, `receiving_application`). Plain records; no decisions. Fail-open: none seen.

## 9. `TT/definitions/service_declarations/normal_memberships.rs` (155 lines)

1. **Purpose.** Collects `ensures` membership promises on a service's result into `ServiceNormalResultMembership` rows (domain identity plus a path into the result).
2. **Shape.** `NormalMemberships<'p>` (`:19`, `OnceCell` of toolchain source identities), `collect :32`, `result_path :98`.
3. **I/O.** Reads `signature_contracts`, `proof_facts`, domain definitions, expression table; produces a sorted, deduplicated `Vec` (`:93-95`).
4. **How it decides.** Only `SignatureContractKind::Ensures` and `ProofFact::Membership` (`:45`,`:53`). Skips a membership when the domain is `DomainIssuance::Unrestricted` **and** has no access policy (`:62-66`). Result path is a chain of `Member`/`Indexed` expressions ending at the reserved result of the owner (`:110-146`); field identity by symbol (`:118`), case by parent symbol kind (`:131`), index must be a closed integer (`:140-143`).
5. **Cases by shape.** Path forms allowed: reserved result name, `.field` (including variant payload field), `[closed index]`. Anything else: "normal membership guarantee does not name its exact result or result projection" (`:148`).
9. **Fail-open.** `:62-66` skips a membership with no comment; effect: an unrestricted domain with no policy is not published as a promise. Open (a program relying on this promise gets none; whether consumers need it is not traced).

## 10. `TT/definitions/source_names.rs` (215 lines)

1. **Purpose.** Diagnostics-only queries that explain unresolved names in a state body (`:1-2`: "they grant no body facts").
2. **Shape.** `declared_receiver_type_reference :13`, `receiver_member_chain :46`, `state_binding_type :73`, `value_root_is_known :96`, `report_unresolved_name_use :171`.
3. **I/O.** Reads state parameters, locals, machine owned data, const/data/machine/trait tables, lexical type parameters; pushes `Diagnostic`s.
4. **How it decides.** `value_root_is_known` is keyed on `SymbolKind` of the resolved head (`:106-121`), then on membership in data/machine/trait tables (`:122-131`), then on a lexical binder walk up `declaration_scope_parent` (`:134-153`). `report_unresolved_name_use` skips diagnostics when the **spelling** is a boolean literal (`boolean_literal_spelling(name)`, `:192`) and emits a message when a two-member path `scope::tail` has no symbol (`:207-214`).
6. **Same job elsewhere.** Resolution in pipeline 02 (`names/expressions.rs`, 1 grep hit) reports its own unresolved names; suspected duplicate message source.
7. **Unused.** Joined grep `report_unresolved_name_use|receiver_member_chain|declared_receiver_type_reference|rejoin_build_activation|source_subjects::`: 20 occurrences in 13 files (5 in this file).
9. **Fail-open.** `value_root_is_known` returns `true` for `Conformance | ConformanceParameter` roots (`:119`) and for any data/machine/trait/builtin symbol (`:122-131`), which suppresses the "not declared" error for those roots. Status: open. Wrong program: a path rooted at a conformance name is not flagged here; later stages are assumed to check. Only diagnostics are affected.

## 11. `TT/definitions/source_subjects.rs` (81 lines)

1. **Purpose.** Decides whether a source declaration belongs to a requested subject set (package identity) and purpose (Build vs Product), and rejoins a retained Build activation (`:1-3`).
2. **Shape.** `contains :10`, `nested_in :37`, `rejoin_build_activation :57`.
4. **How it decides.** `contains` keys on `file.package_identity` membership, or `None` subject with `origin == User` (`:28-29`), plus `symbol_source_is_selected`, not toolchain, and `dependency_scope == purpose` (`:30-33`). `rejoin_build_activation` allows the toolchain default Build only when `file.path`, `package_root` equal `DEFAULT_BUILD_PATH`, no package identity and scope Build (`:70-76`): a path compare.
9. **Fail-open.** `rejoin_build_activation` returns the symbol for the toolchain default Build without calling `contains` (`:77`); stated at `:68-69`. Traced: the declaration was already rejoined by exact source and span (`:62-67`).

## 12. `TT/definitions/trait_definition.rs` (129 lines)

Record types for traits and conformances: `TraitDefinition :27` (`is_boundary`, `type_parameters`, `requires`, `machines`, `refines`, `refinement_clauses`), `TraitRefinementClause :42`, `TraitRefinementReach :50` (`Inherited | Concrete(row) | IndependentBounded(rows)`), `Conformance :64`, `ImplementationValueIdentity :88`, `ConformanceRow :97`, `ConformanceRowSource :109` (`Inline|Reference|TraitDefault|Synthesized`; `owns_realization` is true for all but `Reference`, `:117-119`), `TraitRequirement :123`, `ConformanceCallMember :20`. Comment `:78-86` says what distinguishes the value of a named implementation. No logic. Fail-open: none seen.

## 13. `TT/definitions/trait_definition/applications.rs` (881 lines)

1. **Purpose.** Builds and queries `AppliedTraitScope`: a trait together with the substitutions (type arguments, lifetimes) under which it is read, including parent traits.
2. **Shape.** `TypedTrees::lexical_conformance_scope :23`, `machine_conformance_requirement_scope :53`, `applied_trait_roots :88`, `applied_requirement_scope :175`; `TraitApplicationArguments :228`; `AppliedTypeReference :236` (`parameter_argument`, `child`, `type_identity`, `multiplicity`); `project_type_reference :293`; `AppliedTraitScope :328` (fields `declaration`, `substitutions`, `substitution_scope_starts`, `argument_owner`, `region_scopes`) with constructors `from_type_reference :337`, `from_conformance_application :371`, `from_conformance :406`, `from_machine_conformance :433`, `new :463`, `abstract_scope :501`; `declaring_scope :724`, `bound_scope :744`, `preceded_by :767`, `parent_scopes :783`, `try_visit :819`, `visit :844`, `visit_parents_first :848`, `visit_inherited :856`, `named_implementation_argument :627`.
3. **I/O.** Reads trait/conformance tables, type reference table; builds scopes (borrowing the declaration). `applied_trait_roots` unions eight sources of scopes (machine conformances, conformances, bounds, abstract scopes of nongeneric traits, closed conformance applications, live product operand applications, service reach rows, requirement applications) (`:88-173`).
4. **How it decides.** Frames are keyed on integer positions in `substitution_scope_starts`; a parameter is found by symbol handle equality searching newest first (`:244-249`). Ambiguity of a declaring scope is decided by `same_application` which compares application identity values (`:616-622`, `:729-741`). `named_implementation_argument` requires an `Implementation` binder, a unique bound, closed arguments, a `Named` argument naming a conformance of the bound trait (`:632-676`).
5. **Cases by shape.** `applied_requirement_scope` handles only `Named` and `Generic` type references (`:180-199`); a generic conformance named without arguments returns `None` (`:186-188`, comment `:184-185`). `named_implementation_argument` refuses an applied implementation bound: "applied implementation bound needs closed source preparation" (`:649`, also `:661`, `:674`).
6. **Same job elsewhere.** Parent walk exists three times: `try_visit` (strict, `:819`), `visit_inherited` (lenient, `:856`) and `service_declarations.rs:480-518` (`collect`, strict with cycle error). Comment at `:817-818`: "Parent formation has one owner" (`parent_scope`, `:795`); grep of `.parent_scope(` was not run. `.try_visit(` count: 11 occurrences in 10 files (grep).
7. **Unused.** `bound_scope` (`:744`), `from_type_reference` (`:337`) not grepped.
8. **Stated limits.** "A missing generic tuple cannot become a zero-argument selection." (`:184`).
9. **Fail-open.** `visit_inherited` does `let Ok(scope) = self.parent_scope(..) else { continue }` (`:873-875`): a parent whose application is incomplete is skipped in `visit`/`visit_parents_first`, so the visitor sees a smaller graph. `parent_scopes` doc (`:780-782`) says a retained service must return missing declarations instead of "silently publishing a smaller graph", yet `visit` (used in `service_declarations.rs:1880`, `:2243`, `:2519`, `:3182`, `applied_trait_roots` consumers) has the lenient behavior. Status: open (no comment at the skip; callers not traced).

## 14. `TT/definitions/trait_definition/applications/enclosing.rs` (87 lines)

`data_application` (`:13`) resolves a type reference to a data declaration and its arguments/lifetimes, following `generic_instance` once and returning `None` if arity does not match (`:49-53`); `append_data_application` (`:61`) pushes a substitution frame. Fail-open: none seen (all failures return `None`).

## 15. `TT/definitions/trait_definition/applications/evidence.rs` (444 lines)

1. **Purpose.** Finds which trait requirement a call on a generic binder or a const record binder selects, and authenticates copied receiver metadata.
2. **Shape.** `GenericBoundRequirement :6`; `static_record_parameter_schema :14`, `static_record_bound_requirement :69`, `static_record_source_receiver :116`, `evidence_requirement_declaration :197`, `machine_callable_parameters :246`, `callable_conformance_bounds :267`, `evidence_bound_requirement :305`, `collect_requirement_namespace :422`.
4. **How it decides.** Keyed on symbol identity and parent relations: binder kind `TypeParameter` (`:22`), binder parent equals declaring owner (`:28`), lexical inheritance path walk (`:54-65`), requirement child position equals namespace position (`:239-243`). Matching a target requirement uses the closed application's `template` and `requirement_application_matches_scope` (`:84-88`), else `signature.symbol == target`.
5. **Cases by shape.** Const binders whose schema is a record type (`static_record_schema`, `:46`); everything else `Ok(None)` (`:23`, `:37`, `:43-45`).
9. **Fail-open.** `Ok(None)` for non-const or non-type-parameter binders means "not this kind of call" (`:19-24`), consumers treat `None` as not applicable; traced only by reading this file. `evidence_bound_requirement` `root.visit` (lenient parent walk, `:382`) sets `ambiguous` on conflicts but uses the lenient visitor. Open.

## 16. `TT/definitions/trait_definition/applications/refinement_views.rs` (287 lines)

1. **Purpose.** Applies refinement clauses (a trait refining another trait) to a callable's signature: postconditions, suspends/blocks, termination, service reach.
2. **Shape.** `refinement_prefixes :10`, `callable_contract_key :37`, `callable_refinement_clauses :60`, `check_refined_service_ceilings :90`, `refinement_service_keys :133`, `applied_refinement_row :158`, `apply_callable_refinements :216`.
4. **How it decides.** Service sets compared as `BTreeSet<String>` of **application identity text** (`:137-155`), subset check `narrowed.is_subset(&wider)` (`:121`). Matching clause by requirement **name** `name.as_str() == requirement` (`:77`) or `name == &requirement.name` (`:116`). `IndependentBounded` row chosen by `row.requirement == signature.name` (`:252`).
5. **Cases by shape.** Three reach modes (`Inherited`, `Concrete`, `IndependentBounded`) (`:247-256`). Termination copied only when clause `Terminates` and (premises empty or current `NoGuarantee`) (`:237-246`).
7. **Unused.** `check_refined_service_ceilings`: in joined grep (14 occurrences/7 files; callers `05.../declare.rs` appears in the file list).
9. **Fail-open.** `apply_callable_refinements` handles `Inherited` with `continue` (`:248`): inheriting the base's reach is the stated meaning of the variant. `suspends &= clause.suspends` narrows only (`:226`). None seen.

## 17. `TT/definitions/trait_definition/applications/regions.rs` (405 lines)

1. **Purpose.** Region (lifetime) frames for applied scopes, invocation frames, and portable/normalized identity of an application.
2. **Shape.** `AppliedDeclarationRegionScope :15`; `under_invocation :40`, `keeps_declared_signatures :106`, `portable_application_identity :122`, `portable_parameter_identity :179`, `portable_lifetime_arguments :213`, `parameter_application :233`, `argument_identity :250`, `with_identity_scopes :276`, `application_identity :322`, `invocation_region_scope :377`.
4. **How it decides.** `under_invocation` filters bindings by type and skips `RuntimeValue` bindings (`:75`) and bindings with invalid references (`:78`); comment `:61-62`: "Untyped bindings remain unresolved. They cannot close a demanded trait operand, but unrelated traits still form." Identity function chosen by parameter kind: const/value/machine use index identity, others type identity (`:144-155`, `:190-209`).
5. **Cases by shape.** `keeps_declared_signatures` counts leading frames whose declaration kind is `Machine` (`:107-114`).
9. **Fail-open.** `under_invocation` dropping `RuntimeValue` and unresolved bindings (`:63-81`) yields a scope in which that parameter has no substitution; stated at `:61-62`. Traced by reading: later `parameter_application` returns `None`, identity functions return `None`. Fail-closed downstream.

## 18. `TT/definitions/trait_definition/applications/retention.rs` (434 lines)

`TraitApplicationEnvironment :12` (owned copy of an `AppliedTraitScope`), `RetainedTraitScope :21` (handles into arenas `callable_substitutions`, `callable_lifetime_substitutions`, `trait_application_frames`), `retain :158`, `scope :252`/`:145`, `visit_references :211`, `substitute_service :47`, `lifetime_identity :67`, `resolved_lifetime_identity :120`. A test (`:293-433`) checks that formal and actual authors are kept. `retain` errors if `scope_starts.len() != region_scopes.len()` (`:159`). `RetainedTraitScope::scope` returns `None` only when the declaration is gone (`:284`). Fail-open: none seen.

## 19. `TT/definitions/trait_definition/applications/service_reach.rs` (205 lines)

1. **Purpose.** Ties service-reach rows (effects a callable may reach) to trait applications.
2. **Shape.** `service_reach_scope :11`, `service_effect_scope :40`, `checked_service_scope :55`, `settle_applied_service_lineage :116`, `intern_service_reach_environment :162`, `service_reach_application_identity :196`.
4. **How it decides.** Canonical service per `(symbol arena index, generation, identity text)` key via `BTreeMap` (`:117`, `:141-143`); interning looks up an existing row by `row.symbol == symbol && application identity == identity` (a linear scan over all definitions, `:175-180`).
5. **Cases by shape.** `checked_service_scope` accepts only `Const` binders (scalar carrier) and `Type` binders (not a value); any other binder: "the applied service has an unsupported binder kind" (`:108`). Row without application is valid only for a nongeneric boundary trait (`:19-25`).
9. **Fail-open.** None seen.

## 20. `TT/definitions/trait_definition/supply.rs` (100 lines)

`Supply { slot, provider }` (`:5`): an exact requirement binding that "supplies no proof or execution authority" (`:1-3`). `requirements :12` (one requirement or every member of a trait), `realization :27` (decides, in order: provider's conformance rows for a standalone requirement, then `named_conformance_member`, then any machine whose `satisfaction_provider_declaration` equals the provider and that has a clause naming the requirement; requires exactly one, else `NO_UNIQUE_REALIZATION`). Comment `:84`: "Read retired data-owned sources far enough for the normal declaration check to report the attachment violation." Fail-open: none seen (all ambiguity returns the error constant).

## 21. `TT/examples/declaration_lookups.rs` (498 lines)

An example binary for performance measurement (not shipped code): generates synthetic `TypedTrees` payloads, runs lookups (`declared_binding_type_by_symbol`, `unique_declared_field_by_symbol`), counts allocations with a `#[global_allocator]` (`:21-44`), and writes `omega-performance-plan/1` JSON plans (`plans :317`). Subcommands `construction|audits|summary|plans|audit|run` (`:453-497`); `_ => panic!`. Compared against inlined baseline loops. Uses `serde_json`. No pipeline role. Fail-open: not applicable.

## 22. `TT/examples/declaration_owner.rs` (432 lines)

Same pattern: builds synthetic machines/states/locals (`payload :64`), measures `unique_local_data_for_symbol` and `declared_binding_type_by_symbol` in cold/frozen/mutable modes (`run :175`), allocation audit (`:258`), writes plans against two built revisions (`plans :307`). Seven fixed `Shape` cases (`:309-359`). Not pipeline code.

## 23. `TT/examples/support/mod.rs` (9 lines)

`typed_trees_directory()` finds the ancestor directory ending in `source-to-psi` and joins `representations/typed-trees` (`:3-9`); panics if absent. Used by the two examples.

## 24. `TT/exports.rs` (105 lines)

Re-export table (`use crate::{callables, definitions, expressions, proofs, storage, type_records}` then `pub use`/`pub(crate) use` for about 100 names), producing the `source_state::` namespace. One `#[macro_export]` is in `access_views_and_commitments.rs` (`digest_commitment!`, `:1089`). No logic.

## 25. `TT/expressions/access_views_and_commitments.rs` (1,780 lines)

1. **Purpose.** Four inline modules: `access_views` (sealed placement/accessor declarations and plan-laid layouts), `commitments` (closed conformance and specialization records), `constant_recipes` (array-length and contract constants), `wire_schema_queries`.
2. **Shape.**
   - `access_views` (`:8`): `access_atomic_observing_results :9`; `lifecycle_execution::AccessExecutionDeclaration :46` (`authenticate :104`); `primitive_selection::AccessorPrimitive :214` (`from_declaration :227`, `select_target :374`); `sealed_accessors::ScalarAccessorDeclaration :438` (`authenticate :493`); `AccessLifecyclePlan :690`, `AccessLifecycleStorage :708`, `AccessLifecycleOwner :719`, `AccessLifecycleOperation :731`, `access_borrow_factory :777`, `AccessViewPlan :815`, `AccessFieldPlan :829` (`scalar_projection_leaf :845`), `AccessAtomicResidentContract :890`, `AccessorTarget :907`, `PlanLaidLayout :914` and related (`:914-1081`).
   - `commitments` (`:1084`): `MachineSpecialization :1112`, `ClosedConformanceApplication :1124`, `ClosedConformanceConstArgument :1169`, `ClosedConformanceApplicationSite :1200`, `TraitImplementationApplication :1214`, `DataImplementationApplication :1224`, `CapturedMachineApplication :1235`, `StaticRequirementDispatch :1282`.
   - `constant_recipes` (`:1291`): `ConstantEvaluationEntry :1312`, `create_evaluation_entry :1357`, `RetainedConstantPosition :1401`, `RetainedConstantRecipe :1426`, `ConstantRecipeClosure :1483`, `DeclaredArrayExtent :1504`, `declared_array_extent :1528`, `retain_constant_recipes :1664`.
   - `wire_schema_queries` (`:1739`).
3. **I/O.** `authenticate` functions read machines, states, `access_view_plans`, sealed intrinsic classification (`symbols.sealed_declaration`), return a typed declaration or a `Diagnostic`.
4. **How it decides.**
   - `AccessExecutionDeclaration::authenticate` keys on the sealed intrinsic (`SealedIntrinsic::PlacementLifecycle`) of the machine (or its specialization template) and then checks a long envelope list: no body, `Boundary` supply mode, no spelling, no target, no conformances, etc. (`:152-166`), one entry state, exact parameter count by operation (`:175-185`), type parameter count by `(content, operation)` (`:87-101`).
   - `AccessorPrimitive::from_declaration` is keyed on sealed intrinsic `PlacedPrimitive(Read|Write)`, an envelope list (`:267-291`), reference shape and access polarity (`:298-325`), and the trait name by **string**: `conformance.name.as_str().rsplit("::").next() != Some("Readable"|"Writable")` and `requirement` name equals `"read"/"write"` (`:347-362`); the trait source file must equal the accessor machine's source file (`:359`).
   - `ScalarAccessorDeclaration::authenticate` matches the target by symbol equality in view plans (`:497-504`), requires unique match (`:514`), toolchain origin and `PlacedAccessorTemplate` of the template (`:517-533`), then **`match target.operation.as_str() { "read"|"take"|"write" => .., _ => return Ok(None) }`** (`:597-602`).
   - `declared_array_extent` decides by `FixedArrayLength` variant (`:1537-1551`); `declared_constant_extent` parses a `Named` type's **name** as `u64` (`:1587`).
5. **Cases by shape.** `type_parameter_count`: `(Resident,View)=2`, `(Resident,Initialize|MoveOut)=4`, `(Run,Initialize|MoveOut)=3`, `(Run,Relocate)=4`, others error "placement operation has no declared content/type-argument correspondence" (`:87-101`). Parameters expected: Initialize 3, MoveOut/View 2, Relocate 4 (`:175-179`). `scalar_projection_leaf` accepts only corresponding fixed arrays with nonzero equal length ending in a scalar accessor (`:857-885`).
6. **Same job elsewhere.** `AccessorPrimitive::from_declaration` (`:227`) and `ScalarAccessorDeclaration::authenticate` (`:493`) repeat the same envelope checks (body absent, boundary mode, no suspends/blocks, `service_reach_is_installation_bound`) with different field lists. `service_declarations.rs:2297` consumes `ScalarAccessorDeclaration::authenticate`.
7. **Unused.** Not grepped individually.
8. **Stated limits.** `:129-132` comment about borrow factories; `:1554-1559` doc comment on `named_constant_extent` (misindented `///` at `:1554`).
9. **Fail-open.**
   - `ScalarAccessorDeclaration::authenticate` returns `Ok(None)` when the field value is neither a primitive nor a `Region` access (`:546-550`) and when the operation string is unknown (`:597-602`): the accessor is treated as "not a scalar accessor" rather than refused. Open. Wrong program: a sealed accessor whose operation name is something else is not authenticated, and (via `service_declarations.rs:2301`) not cataloged.
   - `select_target` requires `authenticate(..)?.is_some()` (`:422-427`), fail-closed.
   - `retain_constant_recipes` merges contexts by `same_scope` (owner and result type, `:1453-1455`), so a recipe retained at the same position under a different evaluation entry is kept from the first (`:1685`): the later context's `evaluation_entry` is dropped when scope equals. Traced by reading; effect on consumers not traced.
   - `access_borrow_factory` returns `None` on any ambiguity (`:803`, `:806-808`).

## 26. `TT/expressions/canonical_const_carrier_tests.rs` (182 lines)

Two tests: `canonical_const_carriers_check_inactive_payloads_and_cycles` (an inactive variant with a borrowed or linear payload makes `canonical_const_carrier_is_supported` false; a cyclic array type is refused; an invalid handle is refused) and `a_materialized_float_keeps_its_exact_carrier_and_representation` (negative zero bits `0x8000_0000` for f32, signed signaling NaN `0xfff0_0000_0000_0123` for f64; wrong carrier gives `None`; flipped bit does not match).

## 27. `TT/expressions/canonical_values.rs` (524 lines)

1. **Purpose.** Turns a decoded canonical constant value back into a typed expression for its carrier (`materialize`) and checks that an existing expression still matches the value (`matches_materialized`).
2. **Shape.** `declared_domain_qualifications :27`, `materialize :64`, `materialize_carrier :118`, `aggregate_declaration :187`, `literal_application :208`, `named_case :218`, `record :234`, `float_format :284`, `integer_value :292`, `matches_materialized :314`, `matches_fields :469`, `TypedTrees::{canonical_const_materialization_matches :498, constant_domain_qualifications :510, materialize_canonical_const_value :517}`.
3. **I/O.** Reads `DecodedCanonicalConstValue`, carrier type, data definitions; builds `ExpressionNode`s in the expression table, `Cast` entries with `CastForm::Entry(Add)` for each declared domain (`:94-105`).
4. **How it decides.** Declared domains peeled from `Constrained` carriers; stops at an arithmetic-only constraint; any non-`Domain` constraint or a domain that is not `Declared` or has an invalid symbol returns `None` (`:42-56`). Integer: `integer_type == primitive.landed_landing()` (`:136`). Array: carrier's **display text** must equal `type_name` (`:152`, `:408`). Records: data **name** must equal `type_name` (`:197`, `:202`), field names must match positionally (`:270`, `:485`), case found by name and uniqueness (`:223-231`).
5. **Cases by shape.** `StaticCallable` values: `materialize_carrier` returns `None` (`:124`) but `matches_materialized` accepts a `MachineValue` expression after `canonical_static_callable_value` equality (`:349-366`). Value kinds: Float, Integer, Boolean, Array, Record, Variant.
6. **Same job elsewhere.** `constant_values.rs` decodes the same `DecodedCanonicalConstValue` variants for names (`:12-22`).
8. **Stated limits.** Doc `:14-26`: "the evaluator refuses a domain whose issuance is restricted before this point, since a constant grants no authority".
9. **Fail-open.** `matches_materialized` returns `false` (refuse) on every mismatch. `materialize` returns `None` for unsupported shapes (not accept). None seen.

## 28. `TT/expressions/constant_values.rs` (223 lines)

1. **Purpose.** Decode and inspect typed constants: `canonical_const_declaration_value`, `canonical_const_argument_value`, `canonical_const_carrier_is_supported`, `integer_const_declaration_extent`, `static_const_argument_spelling`, `static_const_argument_value`, `canonical_static_const_spelling`.
4. **How it decides.** `canonical_const_argument_value` accepts a `Named` reference by symbol, or by **spelling**: `CanonicalConstValue::from_atom(name)` or `boolean_literal_spelling(name)` (`:51-55`). `canonical_const_carrier_is_supported` walks the carrier with an explicit stack: builtin integers and `bool` pass (`:93-99`), `FixedArray` needs a closed length (`:104-108`), only `Named`/`Generic` data otherwise, and the data must have no lifetimes, `CheckedShape` supply, no cleanup, no quotient, and `Unrestricted` stored multiplicity (`:116-127`); cycles refuse (`:88`).
5. **Cases by shape.** Floats are not accepted as carriers here (only integers, bool, arrays and data); aggregates of floats would refuse because `F32` is not `Integer|Bool` and `closed_data_definition` fails. Inferred, not tested.
7. **Unused.** `canonical_const_carrier_is_supported` appears in exports and the test file; not grepped.
9. **Fail-open.** None seen (all `None`/`false` on failure).

## 29. `TT/expressions/mod.rs` (9 lines)

Declares `pub mod references; pub mod values; pub(crate) mod access_views_and_commitments; pub(crate) mod canonical_values; pub(crate) mod constant_values;` and `#[cfg(test)] mod canonical_const_carrier_tests`. `expressions/values.rs` (6,816 lines per the crate map) is not in this unit.

## 30. `TT/expressions/references.rs` (5,263 lines)

1. **Purpose.** The reference-visitor system: a `TypedReferences` trait with default no-op/accepting methods, and walkers that visit every symbol, type, expression, statement, proof fact and static application a program, declaration or expression reaches. Collectors built on it feed source evaluation, Build admission, type occurrence censuses and computation demand.
2. **Shape.** `trait TypedReferences :1300` (about 30 hook methods, defaults at `:1301-1410`); walkers `visit_program :2337`, `visit_declarations :2821`, `visit_machine :4075`, `visit_signature :3277`, `visit_type_reference :3372`, `visit_expression :3506`, `visit_statement :3873`, `visit_call_expression :3752`, `visit_static_argument :2527`, `visit_static_machine_bindings :2562`, `visit_closed_conformance :2653`, `visit_proof_fact :3043`, `visit_type_parameters :4020`, `visit_constant_evaluation_entries :4269`; collectors `ReferenceCollector :1563`, `TypeOccurrences :1829`, `ComputationDependencies :2132`, `ConstantUses :4330`, `TypeReferenceInputs :28`, `MachineSidecarIndex :2272`, `DeclarationSelection :1982` (`all`, `contains`, `from_computation_roots :2022`, `intersect`, `excluding`, `include`); public collectors `collect_program_references_with_roles :2215`, `collect_selected_program_type_occurrences :1705`, `collect_authored_program_type_occurrences :713`, `authored_type_occurrences :719`, `collect_program_application_occurrences :1814`, `collect_source_evaluation_references :1765`, etc. Inline module `source_occurrences::call_applications` (`:108`) holds `TypedTrees::retain_static_argument_contexts :115`, `retain_call_static_argument_context :262`, `retain_description_operand_sources :390`, `retain_static_operand_sources :526`, `retain_application_operand_sources :605`.
3. **I/O.** Reads the entire typed program; writes only through hooks, except `call_applications` which mutates the expression table, statement table, type reference table (retains source application rows, lexical owners, type references for static operands).
4. **How it decides.**
   - Membership in a walk is by a `DeclarationSelection` (sorted vec of symbol handles plus an `excluded` flag, `:1982-2008`) and by `retained.declaration(symbol)` (visitor decides).
   - `derived_declaration_references()` (default `true`, `:1361`) gates whether derived rows (layout plans, access views, specialization receipts, product operand inputs, ranking custody) are visited (`:2452`, `:2576`, `:2667`, `:2715`, `:3156`, `:3181`, `:3263`, `:4087`).
   - Static-argument role is decided by an injected classifier function pointer `StaticArgumentClassifier` (`:1298`, used `:1635-1637`); the default (None) means all arguments are `ValueDependency`.
   - `retain_static_operand_sources` classifies an argument as a type by `SymbolKind` in `{BuiltinType, Data, TypeAlias}` or a `Type` type parameter, or `names_implementation` (`:557-568`). `retain_static_argument_contexts` selects statement calls to retain by a six-way OR (`:176-189`).
   - Type name visiting: a `Named` type with invalid symbol that is a const literal or a canonical const argument visits nothing; any other named type visits its symbol (`:3489-3501`, comment `:3484-3488`). A generic type whose base is a universe sort and one argument is skipped when `visit_universe_levels()` is false (`:3465-3474`).
   - Cast domain symbol is visited when any domain coordinate is present or an `Exact` entry form (`:3595-3603`).
5. **Cases by shape.** Exhaustive `match` over `ExpressionNode`, `StatementNode`, `TypeReferenceNode`, `ProofFact`, `TypeParameterKind`. `visit_call_expression` visits many optional call attachments (`runtime_call`, `quotient_operation`, `installed_interrupt_operation`, `private_layout_operation`, `native_code_layout`, `layout_query`, `:3758-3854`).
6. **Same job elsewhere.** `visit_machine_value_dependencies` (`:1534`) and `visit_static_argument` (`:2527`) overlap with `visit_described_argument` (`:3721`) (the latter reports `declaration_description_dependency` instead of `extend`). `input_selections.rs:88-121` and `product_operand/occurrences.rs:67-101` and `references.rs:120-157` each define a private visitor struct re-implementing `Extend<SymbolHandle>` + `expression()` dedup. Three other typed-reference dedups use `Vec<Vec<..>>` indexed by arena slot (`retain_scoped_occurrence :1842`, `Census.seen product_operand/occurrences.rs:70`).
7. **Unused/one-caller.** Joined grep for `collect_program_type_occurrences|collect_source_evaluation_operand_owners|collect_expression_references|collect_program_operand_owners|collect_type_symbol_dependencies|collect_computation_symbol_dependencies`: 44 occurrences in 19 files (7 in this file). `retain_static_argument_contexts|retain_call_static_argument_context\b`: 11 occurrences in 5 files (6 in this file). `visit_product_operand_inputs` and the other `pub fn visit_*` were not individually grepped.
8. **Stated limits.** `:1362-1363` "A carried type belongs to the typed program. Inferring it never names its declaration in the receiver's source." `:531-532` "An unresolved child defers its enclosing application until this same context pass runs after ordinary resolution."
9. **Fail-open.**
   - `retain_static_operand_sources` sets `complete = false` and `continue`s for evidence projections, member applications, unresolved children (`:542-556`, `:594-600`); it returns `Ok(false)` and callers in `retain_call_static_argument_context_in_owner` ignore the returned bool (`:293`, `:213`). Effect: an incomplete operand keeps `type_reference` invalid, to be redone by a later pass (stated `:531-532`). Status: stated; whether a later pass always runs is not traced.
   - `retain_description_operand_sources` swallows `Err` from `prepare_callable_operand_application` with `unwrap_or_else(|_| invalid)` (`:411-415`). Open.
   - `retain_application_operand_sources` ignores `Err` from `prepare_callable_operand_application` (`if let Ok(..)`, `:652-658`); stated `:650-651` "Unsupported applications remain requests of their ordinary consumer."
   - `visit_declarations` (`:2983-2985`, `:2992`) `continue`s for an applied domain whose `theory` is `None` or `Err`; no diagnostic here. Open.
   - `ConstantUses` and the collectors are visitors; `visit_call_expression` visits an invalid `target_symbol` when `runtime_call` and `layout_query` are both `None` (`:3774-3778`), stated `:3771-3773`.
   - `authored_type_occurrences` skips an owner whose lexical description is inconsistent (`ambiguous` → `continue`, `:752-754`): the owner contributes no occurrences. Stated by tests (`:1199-1207`); caller effect not traced.
   - A `HashSet` type alias with `arena::BuildIndexHasher` is used throughout (`:1234`); `std::collections::HashSet` with `(SymbolHandle, ExpressionHandle)` keys at `:124` (`SourceCalls.visited`). Data-oriented ratchet coverage not checked.

---

## Coverage

All 30 files read in full, in order, in chunks, with no skimming:
- `formation/applications.rs` 1-948; `formation/mod.rs` 1-242; `input_selections.rs` 1-175; `product_operand/mod.rs` 1-650; `occurrences.rs` 1-101; `static_environment.rs` 1-763 (total 2,879);
- `service_declarations.rs` 1-3727 (read in chunks 1-1250, 1251-2500, 2501-3727); `inventory_records.rs` 1-19; `normal_memberships.rs` 1-155; `source_names.rs` 1-215; `source_subjects.rs` 1-81;
- `trait_definition.rs` 1-129; `applications.rs` 1-881; `enclosing.rs` 1-87; `evidence.rs` 1-444; `refinement_views.rs` 1-287; `regions.rs` 1-405; `retention.rs` 1-434; `service_reach.rs` 1-205; `supply.rs` 1-100;
- `examples/declaration_lookups.rs` 1-498; `examples/declaration_owner.rs` 1-432; `examples/support/mod.rs` 1-9; `exports.rs` 1-105;
- `access_views_and_commitments.rs` 1-1780 (1-900, 901-1780); `canonical_const_carrier_tests.rs` 1-182; `canonical_values.rs` 1-524; `constant_values.rs` 1-223; `expressions/mod.rs` 1-9; `references.rs` 1-5263 (1-1800, 1801-3550, 3551-5263).

Total read in full: 19,073 lines. Skimmed: 0. Not read: none.

## Five things a maintainer most needs to know about this unit

1. **Two parent-walk behaviors on one type.** `AppliedTraitScope::try_visit` returns an error when a parent application is incomplete or cyclic (`applications.rs:819-842`); `visit` and `visit_parents_first` silently skip such a parent (`applications.rs:873-875`). Service catalog assembly, ABI census, and `applied_trait_roots` consumers call the lenient form (`service_declarations.rs:1880`, `:2243`, `:2519`, `:3182`), so a smaller graph can be published without error.
2. **Product operand formation is a closed whitelist.** `formation/applications.rs` forms only builtin, data, fixed-array types with literal extents and integer/Boolean values; every other shape gets `Stop::Unserved` with the "no product route yet" message (`formation/mod.rs:99-102`). `Stop::Open` with `FormationOnly` demand returns `Ok(None)` instead of an error (`formation/mod.rs:56-58`).
3. **Several decisions key on spelling.** Builtin type names and `true`/`false` (`applications.rs:111-123`, `:333-336`), `Readable`/`Writable` and `read|take|write` operation names (`access_views_and_commitments.rs:347-362`, `:597-602`), array/record names in constant materialization (`canonical_values.rs:152`, `:197`), `parse::<u64>()` of a named type (`access_views_and_commitments.rs:1587`), `core_arrival_requirement` by owner name (`product_operand/mod.rs:367-371`), and service sets as `BTreeSet<String>` of identity text (`service_declarations.rs:1613`, `refinement_views.rs:137`).
4. **`service_declarations.rs` builds the same applied signature storage three times** (`:2497-2504`, `:2551-2563`, `:2672-2683`), and `retain_sealed`/`retain_named`/ABI census each silently drop an unauthenticated or incomplete input (`:2297-2302`, `:2239-2241`, `:2438-2443`, `:2537-2542`). Progress-premise code panics with `.expect` on malformed rows (`:1321-1380`).
5. **`references.rs` mixes a read-only visitor system with mutating passes.** The `call_applications` module (`:108-708`) writes source application rows, lexical owners and type references into the tables; its `retain_*` functions ignore the `bool`/`Err` outcomes of `retain_static_operand_sources` and `prepare_callable_operand_application` in several places (`:211-232` via callers, `:411-415`, `:652-658`) and depend on a later pass to finish incomplete operands.

## What you are unsure about

- Whether the lenient `visit` skip (finding 1) can be reached by a program that the earlier stages already refused; I did not read the producers of `closed_requirement_application` or the typing code that checks parents.
- Per-name call-site counts: my greps joined several names into one regex, so numbers are totals per group, not per function. `capture_static_environment`, `capture_closed_operand`, `bound_scope`, `from_type_reference`, `visit_product_operand_inputs` and most `pub` items in `references.rs` and `access_views_and_commitments.rs` were not individually grepped.
- Whether the later pass that completes a deferred static operand (`references.rs:531-532`) always runs before lowering.
- Whether `service_declarations.rs` ratchet exemptions exist for `std::collections::HashSet` (`input_selections.rs:90`, `references.rs:124`, `:1234`) and `BTreeMap`/`BTreeSet` (`service_reach.rs:117`, `:128`, `service_declarations.rs:1613`, `product_operand/formation/applications.rs:838`); I did not read `tools/ratchets/dense_id_maps.txt`.
- The effect of `retain_constant_recipes` dropping a later `evaluation_entry` when owner and result type match (`access_views_and_commitments.rs:1685`).
- `typed_tree_table!` macro use in `wire_schema_queries` (`:1745`) was not expanded.
