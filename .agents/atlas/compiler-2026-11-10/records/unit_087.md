# Unit 87 notes

Files covered (all under `omega-rust/psi/compilation/representations/typed-trees/src/`):
1. `service_declarations.rs` (1-3089)
2. `type_identity.rs` (1-8305)
3. `type_system.rs` (1-5245)
4. `values.rs` (1-5262)

## 1. service_declarations.rs (3089 lines)

**Purpose.** Projects the Psi-side "boundary service" declarations (boundary traits, boundary operators, top-level bodyless requirements, sealed scalar accessors) out of `TypedTrees` into target-neutral `ServiceSurface` rows (type `language_semantics::service_surface::ServiceSurface`), and prepares a catalog of them (`PreparedServiceCatalog`) plus a census of boundary signatures with their calling-policy (ABI) rows (`PreparedBoundarySignatures`). It is read by provider planning downstream (omega crate) so service requirements do not depend on target calling plans (header comment, lines 1-5).

**Shape.**
- Four inline modules: `provider_declarations` (7-140), `projection` (142-1075), `catalog` (1077-1692), `signatures` (1693-3066), plus `ProviderSchemaDeclaration` enum (3072-3078: BoundaryTrait / BoundaryRequirement / BoundaryOperator / SealedAccessor, each one SymbolHandle) with `.symbol()` (3081).
- `provider_declarations::context::ProviderPlanningContext{typed, scope: DependencyScope}` (14-29); `contains(symbol)` asks `symbol_is_in_dependency_scope` (24-28).
- `independent_satisfactions` (33-44) drops the one conformance edge that is the "owned requirement clause" (46-69) of a machine realizing an inline conformance row.
- `provider_target_declarations` (71-93): machines in scope with `authored_target`, in Product scope or with `target.is_none()`.
- `satisfaction_provider_declaration` (95-108), `boundary_satisfaction_identity` (110-139).
- `projection::ProjectedServiceSurface{program, declaration: Arc<ServiceSurface>, schema, argument_owner, requirements: Vec<ServiceRequirementSource>}` (341-347); `ServiceRequirementSource{declaring_trait, symbol}` (350-353).
- Entry points: `from_typed` (376), `from_typed_application` (383), `from_typed_operator` (429), `from_typed_boundary_requirement` (494), `project_boundary_machine_schema` (512), `collect_service_methods` (595), `service_entry_claims` (778), `service_result_claims` (822), `for_each_establishing_domain` (846), `service_reach_names` (960), `boundary_invocation_names` (1050).
- `projection::applied_receivers::{collect_scopes (148), receiver_applications (167)}`; `projection::correspondence::validate` (247-331).
- `catalog::PreparedServiceCatalog` (1166) with Arenas of `ServiceRecord` (1121), `PreparedServiceRequirement`, `TypeReferenceHandle`, `SealedDeclaration`; `ServiceOrigin` enum (1107: Declaration / Application / Named). `ServiceCatalog<'c>` (1178) is a bound view; `RetainedServiceSurface` (1183) holds an `Arc` to the catalog + record handle. `exact_identity` (1678).
- `signatures::PreparedBoundarySignatures` (2777) = Arena<BoundarySignatureDeclaration> (2753) + `calling_applications::PreparedCallingApplications` (2270) + `abi_signatures: Vec<PreparedBoundaryAbiSignature>` (1725) + `boundary_abis: Vec<(SymbolHandle, Result<CallingSignatureId,String>)>`. Preparation entry `PreparedBoundarySignatures::prepare` (2788) then `abi_signatures::prepare` (1739).
- `prepare_trait_signature` (2920) and `prepare_top_level_signature` (2874) build `PreparedBoundarySignature{telescope, callback_sources, parameter_types, return_type}` (2746).

**Inputs/outputs.** Reads `TypedTrees` (traits, conformances, machines, operators, `applied_trait_roots()`, `captured_machine_applications`, `access_view_plans`, `service_reaches`, `service_reach_rows`, `semantic_domains`, `type_reference_table`) via many accessors; emits `ServiceSurface` values with an interning `ProviderIdentityTable`, and the prepared catalogs/census. No mutation of `TypedTrees`; catalogs are owned structs.

**How it decides (keys).**
- Dependency scope membership by symbol (`ProviderPlanningContext::contains`, 24-28) gates every projection (389, 434, 498, 1269, 1284, 1311, 1345, 1358).
- Method deduplication in `collect_service_methods` is keyed on the interned *requirement callable identity text* (604-610), i.e. a string identity, not a symbol.
- Parent-trait application ambiguity: `collect_scopes` keys on `declaration.symbol`; if two scopes of the same declaration are not `same_application`, returns `None` (155-164) so the whole projection is dropped (from_typed_application `?` at 400).
- `for_each_establishing_domain` (846-892) matches `DomainEstablishmentRoute::BoundaryRequirement` with equal `boundary_trait` and `requirement` symbols (877-885); only walks `Reference` and `Constrained` type nodes (857-891, `_ => {}`).
- Entry claims are made only for parameters whose multiplicity is `Linear` (791); result claims only for Linear result (828-831) and only when `predicate_presence == Absent` (917).
- `domain_claim_name` (894-902): the claim is keyed on the semantic-domain name string (falls back to `domain.name`).
- `boundary_trait_name_for_type` (1036-1048) and `boundary_invocation_names` (1050) resolve boundary traits by linear scan of `program.traits()` comparing symbols; names are `trait_declaration_path` strings.
- Catalog `record()` (1413-1425) returns a default (invalid) handle when zero or more than one record matches an origin (1421), i.e. duplicates collapse to "not found".
- `has_origin` (1532-1540): `Application` origins with invalid application identity are never treated as present (1533-1536), so such roots can be inserted repeatedly.
- ABI `insert` (2116-2183) merges rows keyed on `CallingSignatureId::for_application(requirement_identity, declaring_application, operands)` plus equality of those three; merges `default_eligible |=` (2142) and marks a `physical_owner` disagreement as Err (2143-2146).
- `physical_owner` (2185-2210): trait owner must be boundary, or machine must be `TopLevelRequirement`; else Err string.
- `prepared_policy_machine` (2521-2679): requires exactly one implementation application whose source scope has the same `argument_owner` and `same_application` as the Calling scope (2544-2556); the policy trait must equal `calling_policy.symbol` and the conformance must have no arguments (2579); policy trait must declare exactly one requirement named `"plan"` (2587-2594, string compare on `requirement.name`); exactly one row (2603); and no type parameters or conformance bounds on the realized machine (2673-2677).
- Telescope rules in `prepare_trait_signature` (2920-3065): static machine binders only for `TypeParameterKind::Machine` with `MachineParameterContract::Nominal` (2930-2944; other machine contracts are skipped via `continue` but still consume an ordinal at 2933-2936); direct callbacks are matched to binder by *name string* `parameter.name == direct.binder` (3005); native ordinals checked against `total`, duplicates by ordinal or by name (2981-3001).

**Cases by shape.**
- `project_boundary_machine_schema` (518): `let [entry] = program.machine_states(requirement) else { return None };` exactly one state.
- `project_boundary_machine_schema` (521-525): requirement path must contain `::` with non-empty owner and method (string split `rsplit_once("::")` on `display_path`).
- `prepare_top_level_signature` (2880-2885): "compatibility calling plans require one nongeneric bodyless top-level boundary requirement" (no lifetime or type parameters).
- `BOUNDARY_NATIVE_PARAMETER_CAPACITY` cap on native parameter count (2887, 2972): Err "calling policies currently support at most ...".
- `prepare_policy` (`PreparedCallingApplications::prepare`, 2306-2320): exactly one `Calling<Policy>` relationship per boundary trait (2307) and exactly one policy argument (2316).
- `boundary_policy_instances` (2481-2519): non-generic boundary => one instance; generic boundary => one instance per conformance of that trait (2494-2516); no instance if no conformance exists.
- `prepared_policy_machine` 2673-2677: "generic calling-policy realization requires an explicitly specialized machine before policy evaluation".
- `abi_signatures::prepare` closed-application loop (1761-1874): static operands need parameters/bindings/portable args all equal length (1779-1790).
- ABI inheritance for machines (1961-2000): machines only inherit a boundary ABI if `target_family_interface` (all family members agree, 1969-1986), or `supply_mode` is `ExternalRealization`/`Boundary` (1987-1991), and exactly one governing application (1955-1958).
- `default_scope` (2070-2090) marks a declaration "open" (so its default ABI is Err "requires its complete declaring and non-callback local application", 1745) when the trait has type parameters, lifetime parameters, or non-callback type parameters on the signature.
- `receiver_applications` (167-240): returns `Some(Vec::new())` (empty, not None) if any trait parameter/lifetime lacks a portable identity (184, 191).

**Same job elsewhere.**
- `service_reach_names` (960-988) vs `service_reach_row_names` (990-1004): same collect-names-sort-dedup; the first also extends the closure for the boundary trait's own service. Both in this file.
- `boundary_invocation_names` has two near-identical `traits().iter().find(is_boundary && symbol==)` + `trait_declaration_path` lookups (1043-1047 and 1064-1068).
- `PreparedServiceCatalog::retain` vs the `signatures` census both iterate `typed.traits()`/boundary machines/operators; ABI census and service catalog are separate arenas over overlapping sets (inferred from 1268-1372 vs 2791-2828).
- Matching `ProviderIdentityTable` interning pattern repeated through `from_typed_operator` (453-480) and `project_boundary_machine_schema` (554-589): operator and boundary-requirement ServiceMethod construction are almost identical field by field (read).
- Outside the unit: provider-planning has its own `service_declarations.rs` (`omega-rust/omega/compilation/realization/provider-planning/src/service_declarations.rs`) using this catalog (grepped, 2 call sites of `named_application|retain_*`; not read).

**Unused / one-caller.** Grep (count mode, `independent_satisfactions|provider_target_declarations|satisfaction_provider_declaration|boundary_satisfaction_identity|is_product_declaration`) over omega-rust: 31 occurrences in 14 files (5 in this file are definitions/uses). Grep for `.retain_declaration(|.retain_named_application(|.named_application(|.sealed_accessor(|prepare_frozen(|.exact_requirements()`: 12 occurrences in 9 files. None looked unused; I did not count per-function.

**Stated limits.**
- 2884: "compatibility calling plans require one nongeneric bodyless top-level boundary requirement".
- 2556: "unresolved static binders cannot execute".
- 2676: "requires an explicitly specialized machine before policy evaluation".
- 2889/2974: "calling policies currently support at most {N}" native parameters.
- 2309: "exactly one concrete policy may define a boundary contract".

**Fail-open risks.**
- `from_typed_application` / `collect_scopes` return None on ambiguous parent applications (164, 400): the service is silently omitted from the catalog rather than reported (`retain` is still called with `projected = None` at 1277/1302, so a record with `declaration: None` is stored and `view` returns None at 1525). Downstream sees "no surface". Fail-closed-by-absence; "open" (callers not traced).
- `receiver_applications` 184/191: returns `Some(Vec::new())` (empty receiver applications) when a portable identity is missing, so the projection succeeds with no receiver applications. Wrong program: a generic boundary whose type parameter has no portable identity produces a surface with zero `receiver_applications`. "open".
- `type_identity_text(...).unwrap_or_default()` for parameter/result identities in `collect_service_methods` (632-635, 646-652) and `append_routed_entry_claims` (948-950): a missing identity becomes the empty string and is interned; two different parameters with missing identity collide on `""`. "open".
- `service_entry_claims`/`service_result_claims`: non-Linear parameters make no claims (791, 828-831): stated by multiplicity, not a risk.
- `catalog::record` returns default handle on ambiguity (1421-1423): ambiguity reads as absence (`view` of default handle: `records.get` of an invalid handle; inferred it yields a default `ServiceRecord` whose `declaration` is None). "open".
- `has_origin` 1533-1536: invalid application never counts as present, so duplicates are retained; then `record()` of that origin finds >1 and returns absent. "open".
- `retain_sealed` (1646-1674): `ScalarAccessorDeclaration::authenticate` errors are stored as `Err` in `sealed` (1670), looked up by `sealed_accessor` which returns the Err (1507-1511): fail-closed. Filtered by `context.contains(machine)` (1649); an out-of-scope accessor becomes `Ok(None)`, same as "no accessor" (1499-1506 returns `Ok(None)` when no record). Ok(None) is indistinguishable between "not sealed" and "sealed but out of scope". "open".
- `abi_signatures::prepare` loops `continue` on missing declarations (1762-1767, 1890-1895, 1903-1909) so a closed requirement application with no declaration row yields no ABI row and no error. "open".
- `owned_requirement_clause` (46-69) returns `None` unless exactly one matching edge (68): with zero or two matching edges, no edge is excluded, so `independent_satisfactions` keeps all edges (including the owned clause). "open".
- `satisfaction_provider_declaration` (105-107) returns None when no attached data; "stated" none.
- `.expect(...)` panics: 692-696, 724-727, 743-747 assume normalized premises; a bad premise panics (fail-closed by crash, not accept).

## 2. type_identity.rs (8305 lines)

**Purpose.** Turns a `TypeReferenceHandle` (or an index expression, lifetime, callable declaration) into a canonical text identity (`String`) and interns it as a `TypeIdentityId` in `program.type_identities`. The text is a parenthesized `tag(part,part)` encoding (`atom`/`compound`, 874-927) in which `\ ( ) ,` inside atom values are backslash-escaped. Callable (machine / trait requirement / operator) identities are built from the same texts plus a path and parameter string and interned in a `CallableIdentityTable`. Identity can be "ordinary" (source paths for package-less symbols) or "package qualified" (owner digest encoded).

**Shape (inline modules, in file order).**
- `callable_identity` (7-629): `CallableIdentityId/PathId/ParametersId/ResultDispatchId` (u32, 0 = invalid, `interned_handle!` macro 33-54); `CallableIdentityTable` (88-107, interned by (path, parameters, result_dispatch) triple, plus `boundary_operator_slots` keyed by (path, parameter_types, result_type)); `CallableIdentityMemo` (110-115) keyed on `structural_fingerprint()`; `TypedTrees::machine_callable_identity` (217), `trait_requirement_callable_identity` (224), `operator_callable_identity` (245), `boundary_operator_requirement_callable_identity` (255), `machine_by_callable_identity` (493), `machines_by_callable_identity` (519), `sweep_callable_identities` (545-627) which eagerly derives identities for every machine with `target.is_none()` (552) and every boundary operator that has no machine (566).
- `constraint_identity` (630-972): `normalize_constrained_base`, `normalized_constraints` (667), `NormalizedConstraint` (796) enum {Arithmetic, CompilerDomain, DeclaredDomain, Named} (ordered, sorted+deduped at 2457-2458), `normalize_array_length` (820), `atom` (874), `byte_atom` (896, hex with length prefix), `compound` (912); `#[cfg(test)]` byte-atom equivalence test (929-971).
- `coordinates::CoordinateTag` (973-1032): 23 tag names with `Spelled` impl.
- `derived_declarations::nominal_name` (1043-1134): names a "derived placed" declaration (data, accessor data, accessor machine/state) from `program.access_view_plans`; returns `Err(&'static str)` when the retained origin is not an exact generic application of template/policy/schema.
- `identity_context` (1136-2763): `TypeIdentityContext<'b>` (1721-1748, 26 fields: portable_machine_applications, storage_value_indices, declared_carrier, application_scopes, application_owner/argument_owner, resolved_lifetimes, lifetime_binders, `invalid_application`/`encountered_regions`/`invalid_index_argument`/`missing_exact_nominal_owner` (all `Option<&Cell<bool>>` side-channel flags), binders, substitutions, substitution_scope_starts, substitution_semantics, active_const_substitutions, index_substitutions, match_subjects, exact_toolchain_sources, qualification, static_context). `normalize_type_reference` (2308), `normalize_index_expression` (2534), `generic_reference_identity` (2133), `static_nominal_argument_identity` (2238), `normalize_const_or_nominal_name` (2738), `match_dispatch::{normalize_match (1183), normalize_saved_subject (1302)}`. Test module `argument_roles_tests` (1339-1718).
- `lifetime_parameters` (2764-2990): `LifetimeParameterIdentity{declaration, ordinal}`; `declaration_lifetime_parameters` (2950) is a chain of `or_else` lookups over machine / type alias / conformance / trait / data / trait requirement / retained static machine contract.
- `normalized_identities` (2991-3072): `TypeIdentityId`, `TypeIdentityTable` (wraps `arena::OwnedTextInterner`), `NormalizedResultDispatchSet`, `NormalizedDomainTerm`.
- `open_index_identity` (3073-3464): canonicalizes selected open-index operations (associativity flatten, identity removal, commutative sort, bounded distribution; `MAX_EXPANDED_SUMMANDS = 64` at 3082) via `IndexTerm` (3188).
- `package_policy_reading_tests` (3465-4861, `#[cfg(test)]`): `Fixture` (3523), `kinds` macros `node_kinds!` (3926), `witnesses` (4035) and 5 tests (4760-4860) that render one witness per node kind and read back package owners with `visit_type_identity_package_owners`.
- `portable_machine_applications` (4862-5712): `PortableMachineApplications` (5436, RefCell `active` stack and `error`), `captures` (4864-5046, `capture()` validates a captured nested machine), `conformances::project` (5059-5285), `static_binding_operand` (5670).
- `result_dispatch_terms` (5713-5862): `collect_result_dispatch_terms`.
- `static_operand_identity` (5863-6025): `StaticOperandIdentityRequest`, `DeclaredStaticOperandIdentityRequest`, `TypedTrees::{declared_static_operand_identity, static_operand_identity, ..._with_simultaneous_substitutions}`; one test (5948-6024).
- `storage_identity::independent_value_index` (6032-6051).
- `substitution` (6086-6629): `index_argument` (6105), `static_callable_argument_identity` (6243), `index` (6428), `index_name` (6352), `array_length` (6486), `rejected` (6525), `TypedTrees::closed_array_extent` (6576).
- Top-level (6631-8305): many `applied_*_identity` / `portable_applied_*_identity` wrappers (6693-6980), `local_trait_application_identity` (6982), `local_trait_contract_identity` (7008), `portable_trait_application_identity` (7035), `IndexIdentityArgument` enum (6674: TypeArgument, TypeReference, Expression, Subject, FieldProjection, StaticArgument), `AppliedTypeIdentityScope` (6687), `TypeIdentityRequest` (7137), `ExactOwnerTypeIdentityRequest` (7165), and a large `impl TypedTrees` (7172-8253) holding data-constructor lookups (`closed_data_definition` 7194, `data_constructor_definition` 7262, `closed_data_constructor_case` 7305, `data_constructor_case` 7342, `data_constructor_field` 7371, `closed_data_member_field` 7434, `data_field_declaration_symbol` 7489, `normalized_hermetic_symbol_identity` 7588) and the `type_identity` entry points (7836+), `machine_callable_identity_parts` (8056), `trait_requirement_callable_identity_parts` (8134), `operator_callable_identity_parts` (8185), `named_callable_identity_parts` (8211). Free helpers `callable_type_binders` (8255, binders named `$Self`, `$T{index}`), `intern_boxed_text` (8273), `intern_owned` (8289).

**Inputs / outputs.** Reads `TypedTrees` tables (`type_reference_table`, `expression_table`, `symbols`, data / trait / conformance / machine definitions, `access_view_plans`, `semantic_domains`, `captured_machine_applications`, static machine applications). Mutates only the interners `type_identities` (via `program_intern_type_identity`, 6657), `callable_identities` and `callable_identity_memo` (interior-mutable `with_mut`).

**How it decides.**
- Whether two types are "the same" is decided by text equality of the identity string. Binder substitution is keyed on `SymbolHandle` (binders 1990-1996, substitutions 2381-2396, applied scopes 1823-1840). A symbol-less `Named` reference falls back to matching a binder by *spelled name* (`program.symbols.name(candidate) == fallback`, 1997-2006), rejecting if two binders with that name disagree.
- Const values: a symbol-less name is parsed by spelling: `CanonicalConstValue::from_atom(spelling)` then `spelling.parse::<i128>()` (2745-2760); booleans by `boolean_literal_spelling` (6544); `integer()` parse (6556-6573). So an invalid-symbol type reference named "7" is treated as the integer 7.
- Ordinary vs package-qualified naming: `qualify_non_binder_name` (2017-2079): Ordinary + (builtin atom, or symbol with no package and not toolchain) returns the raw `display_path` (2038-2045); otherwise encodes `package-owner(32-byte digest)` / `toolchain-source(digest)` / `toolchain` / `unresolved` (1765-1780). Toolchain source digests come from the caller's `exact_toolchain_sources` list matched by `SourceId` of the symbol's provenance span (2057-2065); a miss sets `missing_exact_nominal_owner` and uses bare `toolchain` (2066-2071).
- Generic references of a no-argument data definition that has a `generic_instance` are rewritten to the generic origin (2142-2166) and `Named` data with a generic instance is rewritten at 2372-2380.
- `static_context.is_valid()` => `statically_refined_type_reference` replaces the reference if the refined one is a closed index argument (2397-2405).
- Lifetime identity: owner declaration + ordinal (Ordinary uses `arena_index:generation` of the declaration handle, 1955-1968; package-qualified uses kind + callable/path text, 2790-2834 with kinds "machine" / "requirement" / "implementation" / "trait" / "type-alias-parameter" / "data").
- `open_index_identity::identity_operand_is_removable` (3416-3431) decides removal by comparing the rendered operand text to `atom(IndexForm::Integer, literal)` (3412-3414).
- `data_constructor_*` functions (7253-7432) match original and instance members by `name` and `identity` and `relevance` equality plus ordinal position (e.g. 7328, 7365-7367, 7391-7394).
- `closed_data_definition` for a generic reference with arguments finds the instance by `normalized_type_identity(origin) == identity` and `lifetime_parameters ==` and requires exactly one (7229-7243).
- `machine_by_callable_identity` (493-517) filters positions by `declarations.contains(machine.symbol)` and requires a unique one.
- `independent_value_index` (6032-6051): only `CheckedShape` data without `quotient` whose fields never mention the parameter (`independent_type` 6053-6084 recursion cap 128, 6059).

**Cases by shape (fixed lists that refuse the rest).**
- `independent_type` (6063-6083): accepts only Named / Reference / Slice / FixedArray with `FixedArrayLength::Literal` / Generic / Unit; `_ => false` (6082) for ConstExpression, DynamicTrait, Constrained, other array lengths.
- `static_binding_operand` (5670-5710): fixed kind x value matrix (Type/Type, Const/Const, Value/Const, Value/RuntimeValue with invalid subject and expression, Machine/Machine, Implementation/Conformance), `_ =>` Err "specialized callable argument differs from its original parameter kind".
- `normalize_array_length` (820-868) covers 5 `FixedArrayLength` shapes; `FixedArrayLength::Literal` and `ConstParameter` forms are different when `context.invalid_application.is_some()` (826-833) vs not (852-862).
- `derived_declarations::nominal_name` (1077): `let [policy, schema] = program.type_reference_handles(*arguments) else { Err("derived placed origin lost its policy/schema operands") }` exactly two operands.
- `open_index_identity` distribution capped at 64 produced summands (3313-3315); beyond that it silently leaves the term unexpanded (`expanded` returns None).
- `closed_array_extent` (6576-6627): resolves only `Literal` and `ConstParameter` lengths; other lengths return None (6586).
- `scoped_fixed_array_length_identity` (7742-7801): binder collection limited to symbol kinds TypeParameter / Parameter / Local / MachineParameter / ConformanceParameter / PropositionParameter / PropositionMachineParameter (7766-7777).
- `collect_result_dispatch_terms` (5724-5777): descends only through `Reference` and `Constrained`; all other nodes ignored (5769-5775).
- `substitution::open_selected_substitution!` (6408-6426): depth cap 64 on `active_const_substitutions` (6414).
- `static_callable_argument_identity` (6243) handles only Machine / State symbols.

**Same job elsewhere.**
- `type_identity.rs` index_argument (`substitution::index_argument`, 6105) vs `identity_context::static_nominal_argument_identity` (2238) vs `substitution::static_callable_argument_identity` (6243): three places that turn a `StaticMachineArgument` into identity text, tried in order inside `index_argument` (6204-6213). (read)
- `TypeIdentityContext::lifetime` (1844), `resolved_lifetime` (1891) and `encode_lifetime` (1914) plus `lifetime_parameters::portable_lifetime_parameter_identity_in_context` (2790) are two renderings of the same region identity (Ordinary vs PackageQualified).
- The `type_identity()` family: `type_identity` (7836), `type_identity_with_simultaneous_substitutions` (7891), `scoped_type_identity` (7902), `storage_type_identity` (7871), `exact_owner_type_identity` (7990), `scoped_type_identity_with_index_arguments` (7914), `region_free_scoped_type_identity` (7940), plus ~12 free `applied_*` / `portable_applied_*` wrappers (6693-6980) each build a `TypeIdentityContext` literal with a slightly different field subset and call `normalize_type_reference`. Same job, many entry points (read).
- Domain-name resolution `semantic_domains.name(id)` else `domain.name` is done in `constraint_identity::declared_domain_identity` (786-793) and `result_dispatch_terms::declared_domain_name` (5850-5861) and in service_declarations.rs `domain_claim_name` (service_declarations.rs:894-902) (read, three copies).
- Data-constructor correspondence checks (`data_constructor_field` 7371 and `closed_data_member_field` 7434 and `data_field_declaration_symbol` 7489) each re-check "original and selected member agree on name+identity+relevance" (read).
- Outside the unit, suspected: `structural_fingerprint()` is used as memo revision (grep not done).

**Unused / one-caller code (grep -rwo name omega-rust counts include the definition).**
- `callable_path_by_text`: total=1 (definition only, zero call sites).
- `region_free_scoped_type_identity`: 2 (one call site). `portable_applied_index_identity_in_region_scope`: 2 (one call site). `type_identities_match_in_static_context`: 2 (one call site). `local_trait_contract_identity`: 2. `machines_by_callable_identity`: 2. `applied_lifetime_identity`: 3. `callable_identity_by_text`: 6. `closed_array_extent`: 3. `scoped_fixed_array_length_identity`: 3. `normalized_hermetic_symbol_identity`: 76 (widely used). `type_identity_with_simultaneous_substitutions`: 29.
- Command used: `for n in ...; do grep -rwo --include=*.rs "$n" . | wc -l; done` from `omega-rust`. A total of 1 means no callers; 2 means one caller (tests included).
- Comment claims of a single canonical way: none found in this file besides the header.

**Stated limits.** 1075 "derived placed origin is not an exact generic application"; 1099 "contains an unowned nominal operand"; 6535 marker `UnsupportedConstSubstitution` for unsupported const substitution; 3082 `MAX_EXPANDED_SUMMANDS = 64`; 6414 cap of 64 active const substitutions; 6059 depth cap 128 in `independent_type`. Comments in this file are sparse; no migration-debt comments seen.

**Fail-open risks.**
- `rejected()` (6525-6536) and `unavailable()` (1170-1181) only *set* optional `Cell<bool>` flags; when the caller supplies none they return a marker string (`unsupported-const-substitution`, `unbound-match-subject`) as if it were a normal identity. `TypedTrees::type_identity` / `normalized_type_identity` / `storage_type_identity` / `package_qualified_type_identity` (7629, 7836, 7871, 7693) build contexts with no flags (7964-7987), so they always return an interned identity even for rejected input. Wrong program: two distinct unsupported const arguments (e.g. two unresolvable const substitutions) both render the same marker and compare equal. "traced" (I read the context construction at 7975-7985, did not trace every caller).
- `type_identities_match_in_static_context` (7844-7869) returns `true` immediately when `static_type_context_is_unreachable(expression)` (7850); no comment at the site. Effect: in a statically unreachable context any two types match. Source of "unreachable" is `static_type_contexts.is_unreachable` (type_system.rs:1683). "open" for the reachability definition; the single caller was not read.
- `callable_identity` constructors return the default invalid id when derivation returns None (326-330, 275-279); `callable_identities_share_*` return false and `callable_identity_text` returns "" for invalid ids (377-382, 408-415). An invalid id compared with an invalid id by `==` is equal; two unrelated callables whose derivation failed collide at `CallableIdentityId::default()`. "open".
- `normalized_type_identity_text` / `package_qualified_type_identity_text` / `type_identity_request_text` use `.unwrap_or_default()` (7654-7657, 7700-7711): an uninterned identity becomes "" text. "open".
- `normalized_nominal_type_identity` (7663-7691) calls `derived_declarations::nominal_name(...).ok()?` (7670-7671) and drops the Err reason, returning None. Fail-closed by None, "traced" (read).
- `callable_identities_share_family` compares path+parameters but not result dispatch (377-382): by design, "stated" by name only.
- `result_dispatch_terms` ignores `TypeConstraintNode::Named` (5765) and every type node below Reference/Constrained, so result dispatch ignores constraints nested in generics/arrays. "open".
- `collect_declared_result_dispatch_terms` (5823-5828, 5837-5842) skips a declared domain that has a predicate, no semantic roles and no establishment routes: such a domain contributes no dispatch term. "open".
- `closed_data_member_field` condition `(!retained.is_valid() || retained == ...)` (7485): an invalid `retained` handle is accepted. "open".
- `identity_context::name` fallback by name (1997-2006) matches binders by spelling when symbol is invalid. "open".
- `sweep_callable_identities` skips machines with `target.is_some()` (552) so target-body machines never reach `machines`/`by_text` memo. "open".

## 3. type_system.rs (5245 lines)

**Purpose.** Holds (a) a small constant evaluator for closed integer expressions (`closed_numeric`), (b) the proven static type-context machinery (`type_predicates`): worlds of type equations built from `==`/`!=` type predicates and case-membership facts, used to refine type references, mark code unreachable and elide arms, and (c) the type-reference table itself (`types`): the `TypeReferenceNode` enum, `TypeReferenceTable` arenas with side columns, copy/remap/substitute/instantiate operations, `PrimitiveType`, domain constraints. Header comment (1-5) lists only the three module names.

**Shape.**
- `closed_numeric` (7-593): `evaluate_value<const ALLOW_DECIMAL_LITERALS>` (180) is an explicit-stack evaluator (`Step::{Enter, Leave, Binary, Cast}` 188-193) over `BigRational` returning `NumericValue{value, primitive, type_reference, fractional_origin}` (25-30); `apply_numeric` (353) applies Add/Sub/Mul/Div (anonymous) or Add/Sub/Mul/Div/Mod typed by arithmetic domain Exact|Trapping / Wrapping / Saturating (430-477); `land_integer` (490), `integer_carrier` (499), `literal_carrier` (518); `has_anonymous_operator_meaning` (534), `has_builtin_anonymous_operands` (552). Public via `TypedTrees::closed_integer_expression_value[_in]` (105-131), `closed_integer_value_in` (133), `integer_expression_value_with_bindings` (153).
- `type_predicates` (595-1957): `StaticTypeContexts` (707-715: boolean_equalities, completed_case_values, admitted_match_cases, case_memberships, rows: per-expression `Vec<TypeWorld>`, statement_rows, transition_rows, all sorted Vecs binary-searched by (arena_index, generation)); `TypeWorld{bindings, exclusions}` (718-721) with `resolve` (828) and `constrain` (854); `condition` / `condition_inner` (902-1090), `visit_context` (1092-1184), `TypedTrees::prepare_static_type_contexts` (1187-1366), `elide_unreachable_case_arms` (1368-1420), `elide_unreachable_transition_rows` (1422-1454), `static_type_requirement_entailed[_at_statement]` (1577-1617), `static_boolean_value` (1695), `retain_proven_type_contexts` (1814) with `ProvenTypeContext` (1805), `common_static_type_reference` (1921), `CaseMembershipTypeFacts` (700).
- `types` (1959-5243): `TypeReferenceNode` enum (4350-4385: Reference, Constrained, FixedArray, Slice, Generic, ConstExpression, DynamicTrait, Named, Unit(default)); `FixedArrayLength` (4404: Literal, Expression, ConstParameter, ConstCall); `TypeConstraintNode` (4520: Named, ArithmeticDomain, Domain); `DomainConstraint` (4552, 10 fields); `PrimitiveType` (4591, 13 variants incl. `Selector`, `Addr`); `TypeReferenceTable` (3243: 11 arenas + 4 lazy index caches `NamedRowIndex`, `ArithmeticRowIndex`, `SliceViewIndex`, `PolicyShellIndex`, all with `PartialEq` always `true` (3285-3347) and no-op `Record` impls (4841-4895)). `copies` (1969-2414: `copy_from`, `copy_with_expressions`, `remap_symbols_in`), `display` (2542-2693), `substitution` (2811-3222: `substitute_node`, `substitute_from`, `substitute_applied_from`, `substitute_shared_children`, `instantiate`), tests `alias_tests` (2418-2541), `extent_custody_tests` (2695-2810), `tests` (4898-5242).

**Inputs/outputs.** Reads `expression_table`, `type_reference_table`, `symbols`, machines/states/statements/contracts, `proof_facts`, `declared_type_equations`, `machine_specializations`, `authored_declaration_selections()`; writes (`&mut self`) `static_type_contexts`, `expression_table` match arms (`elide_unreachable_case_arms` 1405-1419), state statement lists (1450-1453), and the type table (insert / substitute). Constant-evaluator output is `Option<BigInt>` / `ClosedIntegerValue`.

**How it decides.**
- Type predicate truth is `normalized_type_identity(left) == normalized_type_identity(right)` after refinement (693-694, 650, 863), i.e. identity text equality; a predicate is evaluated only if both operands are closed (`index_argument_is_closed*`, 687-688, 639-640).
- `TypeWorld::constrain` (854-899): equal identities => keep iff `equal`; a `TypeParameter`-kind Named side becomes a binder (866-873: keyed on `SymbolKind::TypeParameter`); a binding conflicting with a recorded exclusion kills the world (887-893); two non-binder closed unequal sides => keep iff `!equal` (879-883); otherwise keep unchanged (884).
- Reachability: an expression/statement/transition is "unreachable" iff a row exists for it with zero worlds (760-767, 1513-1545). No row means reachable.
- `refined_in_worlds` (739-758): returns the common resolved reference only if all worlds agree by identity text, else the original.
- `elide_unreachable_case_arms`: drops a match arm only if its pattern is `Destructure` and `is_unreachable(arm.value)`, and never drops all arms (1378-1388); keeps the arm list unchanged if `live.is_empty()`.
- `elide_unreachable_transition_rows` (1422-1454): drops a transition only if guard "opens with a refuted case" (`guard_opens_with_refuted_case` 1456, follows `And` left and `==true` boolean-equality) and the target's context is unreachable, and only if at least one other transition remains (1442-1449).
- `closed_numeric`: a Name is resolved only through `const_declaration_by_symbol` (290); binary operators on typed values require `has_builtin_spelled_expression_meaning` (400) and equal primitives (406-411) and equal arithmetic domain (420-425); `Modulo` is not allowed for anonymous values (366-372).
- `PrimitiveType::from_name` (4648-4676) maps by spelling: "bool","f32","f64","i8".."u64","addr","AtomicBool/U32/U64", and anything `starts_with("AtomicU32#AccessField<")` etc. `TypeReferenceNode::primitive_type` for `Named` uses that spelling, not the symbol (4448); `type_symbol` returns an invalid symbol for a Named or Generic whose spelled name is a primitive name (4492-4505). `anonymous_numeric_literal_value` (3722-3733) treats a symbol-less `Named` whose spelling parses as `usize` as a numeric literal.
- `find_named_type_reference` returns the first `Named` row with that symbol (4101-4118) (lowest arena index kept by `note_named_row` 3609-3619).
- `find_arithmetic_result_type_reference` (4120-4168): only `Constrained` rows whose constraint span is exactly one `ArithmeticDomain` element; first match per (carrier, domain) wins.
- `instantiate` (3060-3175): rebuilds only rows that mention a binder; refuses (Err) a binder inside a domain argument (3098-3102) or as an array length const parameter (3113-3119).

**Cases by shape.**
- `evaluate_value` handles: Match (needs a `selected_arms` entry, 221-238), un-landed Integer, Float (only if `ALLOW_DECIMAL_LITERALS`), landed Integer/Cast/Name/Binary only when `typed_owner.is_some()` (252-304), `_ => return None` (305).
- `apply_numeric` arithmetic table (430-477): 5 operators x 3 domains listed explicitly; `_ => return None` (476), so other operators or `Addr`/float carriers are refused (`integer_carrier` supports only i8..u64, 499-506).
- `literal_carrier`: `LandedIntegerType::Addr => return None` (531).
- `land_integer` (490-497) goes through `to_i64` / `to_u64`, so only values that fit 64 bits are admitted.
- `has_builtin_anonymous_operands` (552-592): fixed list of selection targets (Machine/State/Operator for Call; Domain for DomainMembership; BuiltinOperator intrinsic; CheckedOperator late bound; Const) `_ => false` (589). Note parameter `_spelling` unused (555).
- `has_anonymous_operator_meaning` (534-550): only Add/Subtract/Multiply/Divide (`_ => return false`, 547).
- `visit_context` (1092-1184): explicit arms for every `ExpressionNode`; `SavedSubject | TypeExpression | literal_expression_pattern!()` visit nothing (1179-1181).
- `condition_inner` (968-1090): recognises type predicates, Local names with immutable initializer, case memberships, `==`/`!=` against a Boolean literal, Boolean, `!`, `&&`/`||`; `_ => worlds.to_vec()` (1088).
- `contains_type_predicate` (914-966): same shapes; Local names only if immutable with an ordinary initializer (956-957).
- `static_boolean_value_inner` (1699-1770): Boolean, immutable Local, `!`, `&&`, `||`, boolean-equality rows; `_ => None`.
- `static_type_requirement_entailed_in_worlds` (1619-1680): refuses (false) when formals repeat (1643-1649), a formal is not `TypeParameter`, or an actual mentions another formal (1650-1658), or any world's bindings/exclusions mention a formal (1659-1672).
- `slice_view_accesses` (3991-4028): tracks only Shared and Mutable slice views (`WriteOnly => continue`, 4014).
- `element_inherits_collection_arithmetic_policy` (4247-4282): element kinds I8..U64, Address, F32, F64 builtin atoms; FixedArray/Slice inherit (returns true); `_ => false`.
- `PrimitiveType` queries: `integer_bounds`, `accepts_range_constraint`, `accepts_finite_constraint` (F32/F64 only), `scalar_byte_size` (4729) enumerate variants. `is_signed_integer` (4722-4727) is `!matches!(U8|U16|U32|U64|Addr|Selector)`, so it is true for `Bool`, `F32`, `F64`.
- `TypeReferenceNode::display_name` prints `dyn {name}<N argument(s)>` for 0/1/many (2594-2600).

**Same job elsewhere.**
- Resolving a named local to its immutable initializer is written three times in this file: `contains_type_predicate` (946-961), `condition_inner` (985-1001), `static_boolean_value_inner` (1725-1738) (read).
- Binary-search by `(arena_index, generation)` is hand-written ~15 times in `type_predicates` (e.g. 730, 762, 773, 787, 1031, 1473, 1519, 1536, 1555, 1583, 1604, 1712, 1877, 1909, 1927) instead of one helper (read).
- Match arms `ExpressionNode::Match` selection in `evaluate_value` (221-238) vs `has_anonymous_numeric_results::visit` (83-93) both walk match arms of anonymous numeric expressions (read).
- The `domain_name` rule (`semantic_domains.name(id)` else `domain.name`) also appears in type_identity.rs:786 and service_declarations.rs:894.
- `PrimitiveType::from_name` spelling match vs `PrimitiveType::from_builtin_atom` (4618-4630, symbol atom based; only integers) vs `closed_numeric::literal_carrier`/`integer_carrier`: three tables mapping integer kinds to carriers (read).
- Type-identity equality as predicate: `type_identity.rs:7844 type_identities_match_in_static_context` (`==` of normalized text) vs this file's 650/693/863 (read).

**Unused / one-caller code (grep -rwo; total includes the definition).**
- `const_expression_sites`: total=1 (definition only, zero call sites).
- Single call site (total=2): `integer_expression_value_with_bindings`, `closed_integer_range_endpoint`, `static_type_requirement_entailed_at_statement`, `statically_refined_transition_type_reference`, `static_boolean_normal_form`, `static_type_contexts_agree`, `retain_proven_type_contexts`, `elide_unreachable_case_arms`, `closed_nominal_symbol`, `constrained_type_references`, `unconstrained_named_symbol`, `canonical_integer_range_endpoint` (both occurrences in one file).
- Widely used: `peeled_type_reference` 74, `accepts_integer_literal` 82, `static_type_context_is_unreachable` 29, `has_anonymous_numeric_results` 41.
- Command: same `grep -rwo --include=*.rs name . | wc -l` loop from `omega-rust`.
- Documented "single" ways: none stated in comments.

**Stated limits.** Only error strings: 3100 "a trait parameter inside a domain argument has no instantiation"; 3117 "a trait parameter as an array length has no instantiation"; 3855 "array extent custody has no exact array and argument rows"; 3599/3604 "stale receiving type / occurrence span"; 3494 "substituted operand has no registered type row". No TODO/limit comments seen.

**Fail-open risks.**
- `is_unreachable` (760-767) and its wrappers return true only for an explicit empty-world row, so a missing row means reachable (fail-closed). But downstream consumers treat "unreachable" as permission to skip checks: `type_identities_match_in_static_context` (type_identity.rs:7850) returns `true` and `common_static_type_reference` skips unreachable expressions (1948), `elide_unreachable_*` delete code (1380, 1431-1432). What wrong program gets through: if `prepare_static_type_contexts` / `retain_proven_type_contexts` wrongly produce an empty world list (e.g. contradictory equations from `declared_type_equations`, 1208-1218, or `retain_proven_type_contexts` setting `worlds.clear()` when `!context.reachable`, 1889-1890), the construct is treated as dead and any type mismatch there is not reported. "traced" for the definition of unreachable (read 760, 1213-1217, 1889), "open" for the producers of `ProvenTypeContext.reachable` (outside this file; 2 callers per grep).
- `retain_proven_type_contexts` join (1822-1855): when the same expression has several contexts and only some are reachable, `previous.reachable` can stay false and later contexts overwrite equations/case_value only when previous is unreachable (1845-1849); when both are reachable equations are intersected (1842-1844) and differing `case_value` becomes `None` (1839-1841). Wrong program: a context unreachable in the first visit but reachable in a later one is revived correctly; the reverse (first reachable then unreachable) is ignored (the `if context.reachable` guard at 1827 skips unreachable later contexts), so a later unreachable context cannot mark a reachable one unreachable. "traced" (read).
- `TypeWorld::constrain` line 884 (`return Some(self)`) keeps a world unchanged when neither side is a type-parameter binder and the pair is not closed on both sides: an equation between two open non-binder types constrains nothing. "stated" no; "open".
- `condition_inner` fallback `_ => worlds.to_vec()` (1088) and the `else { worlds.to_vec() }` at 999 and 1053: unknown predicate shapes do not narrow worlds (sound direction: keeps more worlds). "traced" (read).
- `elide_unreachable_case_arms` removes arms only for `Destructure` patterns (1379), `Value`/`Wildcard` arms are kept even if unreachable.
- `evaluate_value`/`apply_numeric` return `None` on every unknown, so callers treat "not closed". Where `None` is then interpreted by callers was not traced.
- `PrimitiveType::from_name` spelling (4648-4676), used for `Named` (4448) with no symbol check: a user data type spelled `u32` or `AtomicBool#AccessField<...>` is classified primitive; a symbol-less Named spelled `bool` is Bool. Wrong program: a user-declared type named like a primitive in a scope where the symbol is not the builtin would be given primitive semantics. "open" (whether the front end forbids shadowing builtin names was not traced).
- `PrimitiveType::is_signed_integer` true for Bool/F32/F64 (4722-4727): callers using it as "is a signed integer" without checking `is_integer` would misclassify. Callers: `land_integer` (491) only reaches it after `integer_carrier(primitive)?`... note `land_integer` evaluates `is_signed_integer()` BEFORE `integer_carrier` (491-496), so for `Bool` it takes the signed branch then `integer_carrier(Bool)` returns None => None. Fail-closed here. "traced".
- `NamedRowIndex`, `ArithmeticRowIndex`, `SliceViewIndex`, `PolicyShellIndex` implement `PartialEq` as always-true (3285-3347) so two tables equal by `==` regardless of cache state (stated by tests 5089, 5154 "assert_eq!(table, without_index)").
- `substitute_node` clears `operand_lifetime_spans` unless lifetimes unchanged (2842-2847) and clears `array_extent_arguments` for non-FixedArray (2852-2860): intentional.

## 4. values.rs (5262 lines)

**Purpose.** Defines the expression storage of `TypedTrees`: the arena-backed `ExpressionTable` with `ExpressionNode` (21 variants, 4199-4221), the owned tree form `Expression` (2817-2838) and its conversion into the table (`insert_tree`, 2603), the copy/remap machinery that duplicates or renames symbols inside expression graphs, a tiny closed-integer classifier (`const_integer`), the "saved subject" capture lookups (`captures`), a pattern-component query (`pattern_components`) and a generic subtree visitor (`subtrees`).

**Shape.**
- `captures::saved_value_source` (11-47), `saved_argument_source` (49-67).
- `const_integer` (70-187): `ConstIntegerExpression{Closed(i128), Open}`, `classify` (80), `is_integer_expression` (112), private `fold` (140) over Integer / single-member Name / `+ - * / %` only.
- `expression::display` (208-782) display-name writers; `index_authority` (783-925): `IndexExpressionAuthority{normalization, operation}`, `OpenIndexNormalization`, `OpenIndexOperationSelection` (14 fields, 802-817) with `operation_contract_identity` (826); `shape` (926-950): `selects_only_nominal_route`, `carries_only_positional_arguments`, `is_ordinary_positional_call`; `static_members` (951-1165): copy/remap of `StaticMachineArgument`, `request_static_arguments[_mut]`, `visit_static_arguments_mut`; `table_copies` (1166-2509): `copy_graph` (1312), `copy_subtree_with_occurrences` (1848), `remap_symbols_in` (1567), `replace_with_existing` (1496), `copy_call_operands_from` (1740), `copy_match_subject` (2243), `CopyGraph{expressions, subjects}` (1182); `tree_conversion` (2510-2791).
- `ExpressionTable` (2861-2893): 28 columns (arenas plus per-expression `Vec`s indexed by `source_span_index(handle)` = arena_index - 1, 4185); constructor family (3147-3226), static-machine application columns (3260-3467), `insert` (3497), span/occurrence/domain-test/materialization-source columns, `expressions_structurally_equal` (4006), `constant_integer_value` (4154).
- Node structs 4223-4915: `TableCallExpression` (4330, 21 fields), `StaticMachineArgument` (4441: type_reference, path, application, const_literal, evidence_projection, symbol), `TableMatchExpression`, `CompletedValueCapture` (4834: subject, owner, definition, value_anchor), `ValueCaptureOwner` (Match / ExpressionArgument / StatementArgument), `MatchPattern` (Value/Destructure/Wildcard), `BinaryOperator` (4702, 19 variants) with `is_arithmetic`, `is_comparison`, `evaluate_comparison`, `mirrored`, `complement`.
- Macros `executable_leaf_expression_pattern!` (4918), `spelled_literal_expression_pattern!` (4930), `literal_expression_pattern!` (4942); `ieee_float_value` (4960).
- `pattern_components::Query::read` (5006-5090): `TypedTrees::pattern_component` (4988).
- `subtrees` (5093-5262): `SubtreeEdges{saved_sources, atomic_results, pattern_dependencies}` with consts `VALUES`, `STRUCTURAL`, `SAVED_SOURCES`; `SubtreeVisit`; `subtree_any` (5133), `subtree_for_each` (5142), `subtree_children` (5183).

**Inputs/outputs.** Reads/writes only the expression table and, for captures/pattern components, `statement_table`, `symbols`, local data. Copy functions allocate new rows in the destination table and return a `correspondence` list of (original, copy) pairs.

**How it decides.**
- Structural equality of two expressions (`expressions_structurally_equal` 4006-4100): same handle => equal (4011); different `direct_pattern_component` => unequal (4016); then per-variant field comparison. Names compare `head_symbol`, `symbol` and member text (4057-4064). Calls compare `target_symbol`, `exact_coercion_target`, target text, receiver and arguments only (4091-4097). Members compare `member_symbol` and member text and receiver (4065-4069), not `case_variant`. Match arms with `Destructure` patterns are compared with `left == right` on the pattern struct (handles) (4033-4036). Variants not listed (Cast, ArrayLiteral, Atomic, StructLiteral, TypeExpression, ZeroValue) fall to `_ => false` (4098).
- `names_parameter` (3990-4004): true if the name has one member and either its symbol/head symbol is the parameter's symbol, OR (line 4003) the spelled name equals the parameter's name. The spelling arm is not guarded by the symbol check.
- `const_integer::classify` (80-110): a symbol-less Name is closed if its spelling parses as `i128` (95-99), else Open if `is_binder(symbol, spelling)` is true; operations on Closed x Closed use checked arithmetic (None on overflow or division by zero, 177-186); anything with an Open side is Open (107).
- `authored_length_receiver` (3850-3864): recognises `.length` by spelling via `CollectionMeasure::from_authored_spelling` and requires an unresolved member symbol and no case variant.
- `names_reserved_result` (3977-3988): single-member Name with no symbols whose member `is_reserved_result()` (spelled).
- `pattern_components::Query::read`: a node's component is `direct_pattern_component`; `SavedSubject` requires `saved_value_source` to resolve (5014-5021); Name of a non-Local symbol => Fields (5031-5033); Local follows `initial_value` (5034-5041, None if `!role.is_runtime_binding()`); Borrow/Cast follow their operand; Match arms must all agree else None (5079-5085); any other node kind => Fields (5046).
- `saved_value_source` (11-47): by capture owner; for call-argument owners requires `source == capture.subject` and `!saved.pattern_head.is_valid()` (43-46).
- `selects_only_nominal_route` (930-937): true iff six optional request fields are all None.
- `copy_subtree_with_occurrences` (1848): memoizes (original, copy) pairs by linear scan of `copies.expressions` (1861-1867); `SavedSubject` copied from `CopySource::Own` with no copied capture reuses the original node (1927-1928).
- `replace_with_existing` (1496-1553): component becomes `Case` if either side is Case else `Fields` (1501-1509).

**Cases by shape.**
- `const_integer::fold` (146-174): Integer, single-member Name (`let [member] = ... else return None`, 149), Binary with Add/Sub/Mul/Div/Mod only; `_ => None` (173).
- `subtree_children` (5183-5260): lists every node variant. Call children are receiver and arguments only (5228-5232); `layout_query.nonempty_view`, static machine arguments and requests are not children. Atomic `result` only if `edges.atomic_results` (5216). `SavedSubject` descends only if `edges.saved_sources` (5192). `TypeExpression | literal_expression_pattern!()` (includes `Name`) have no children (5258).
- `constant_integer_value` (4154-4175): Integer (i64), Borrow, Binary Add/Sub/Mul/Div/Mod with checked i64 ops; `_ => None`.
- `expression_at_depth` (3839-3848): depth cap 128 returns None.
- `display_name`: separate `Expression::display_name` (274) and `ExpressionNode::write_display_name_with` (352) both enumerate all variants.
- `OpenIndexOperationSelection::operation_contract_identity` (826-833) builds text `trait::<path>::<requirement>(S,S)->S` with the same subject for both operands and the result.
- `StaticMachineArgument::display_name_with_owner` (708-745): priority type_reference, const_literal, evidence_projection, path.

**Same job elsewhere.**
- Three closed-integer evaluators: `const_integer::classify` (i128, values.rs:80), `ExpressionTable::constant_integer_value` (i64, values.rs:4154), and `closed_numeric::evaluate_value` (BigRational, type_system.rs:180). All three parse symbol-less Name spelling or fold + - * / % (read). A fourth integer parse appears at type_identity.rs:6556 (`integer`) (read).
- Owned `Expression` tree vs table `ExpressionNode`: parallel structures with parallel display, conversion (`insert_tree` 2603) and `Table*` mirrors for Binary/Unary/Cast/Call/Member/Indexed/Range/Match/StructLiteral (read).
- Symbol remapping appears in three flavours: `remap_symbols_in_inner` (1577-1726), `remap_static_argument_symbols` (998-1026), `remap_static_binding_symbols` (1052-1072), and in the type table `TypeReferenceTable::remap_symbols_in` (type_system.rs:2209) (read).
- Copy: `copy_static_argument_from` vs `copy_static_argument_from_self` (956-969) both route into `copy_static_argument_members(Some/None, ..)` (read). `copy_graph` (1312) vs `copy_from` (1555) vs `insert_copy` (1563) wrap the same `copy_subtree_with_occurrences`.
- `subtree_any` family vs the hand-written recursive walkers in type_system.rs (`contains_type_predicate` 914, `has_anonymous_numeric_results::visit` 73, `visit_context` 1092) and `open_index_operations` (884-905), each walking Expression children itself (read).
- `names_parameter` spelling match vs symbol match: also the `single_name_member` / `name_path_is_single_member` helpers (3929, 3971).

**Unused / one-caller code (grep -rwo name omega-rust | wc -l; total includes definition).**
- `copy_expression_handles_from_slice`: total=1 (definition only, zero call sites).
- `respell_single_member_names` 2, `match_pending` 2, `match_deep_any` 2, `display_name_resolving` 2, `struct_field_count` 2, `copy_transition_pattern_from` 2, `retain_expression_roots` 3, `subtree_for_each` 3, `is_integer_expression` 3 (const_integer: `grep const_integer::|ConstIntegerExpression` = 18 occurrences in 4 files, 8 of them in values.rs itself).
- Widely used: `expressions_structurally_equal` 65, `constant_integer_value` 43, `iter_expressions` 134, `saved_value_source` 47, `names_parameter` 20 (4 call sites of the `.names_parameter(` form at proof_contracts_core.rs:2162, 9317, 9587 and checks_content.rs:6215).
- Single-owner claim: header says the file holds subtree visitors; no "single canonical" comment in this file except the doc comments at 2867-2868 (`materialization_sources` "provenance edge") and 3482-3483 ("A component belongs to the produced occurrence, not the current node shape").

**Stated limits.** `.expect("expression occurrence spans cannot contain the arena dummy sentinel")` (2812); `.expect("copied expression retains its component row")` (1872); `.expect("qualified static member belongs to its source expression table")` (987-989); assertions in `retain_materialization_source` (3551-3553); doc comment at 4975-4981 ("This is a binding contract, not a claim that the initializer's contents or old referents remain live"). No TODO / unsupported comments.

**Fail-open risks.**
- `names_parameter` (3990-4004): the `|| name.as_str() == parameter.name.as_str()` arm (4003) accepts a one-member Name whose spelling equals the parameter's name even when its symbol is a different declaration (e.g. a shadowing local). Wrong program: a membership proof written against a same-spelled local rather than the parameter would be treated as about the parameter. The four callers (validation/proof_contracts_core.rs:2162, 9317, 9587; pipeline/05 checks_content.rs:6215) were not read. "open".
- `expressions_structurally_equal` ignores `Member.case_variant` (4065-4069) and, for Call, ignores `machine_arguments`, `static_requirement_dispatch`, `build_selection` and the request fields (4091-4097). Two calls that differ only in those compare equal; 65 occurrences in 14 files use it. "open" (callers not traced).
- `const_integer::classify` (95-99) and `is_integer_expression` (122-125) treat a symbol-less Name as a literal when its spelling parses as an integer. Wrong program: an unresolved name spelled `7`. "open" (callers in pipeline 04 and validation not read).
- `is_integer_expression` (112-130) accepts any `+ - * / %` shape including division by zero (operation closure `Some(())`, 127), unlike `classify` which rejects it via checked ops. "open".
- `subtree_children` (5183-5260) skips `layout_query.nonempty_view` and all static arguments, so `subtree_any` searches (18 occurrences in 13 files) cannot see an expression only reachable through them. "open".
- `pattern_components::Query::read` returns `Some(Fields)` for every node kind not listed (5046) and for Names whose symbol is not a Local (5031-5033). Wrong program: a case view reached through a non-Local name is classified as a Fields view. "stated" partly by the module doc (4975-4981); "open".
- `saved_value_source` returns `None` on any mismatch (fail-closed), `pattern_component` likewise.
- `copy_match_subject` sets `StatementArgument.call` to invalid in the copy (2279-2283), to be fixed up later by `rebind_statement_operand_captures` (1795-1812); a copy that is never rebound keeps an invalid owner. "open".
- `set_static_machine_application_bindings` (3349-3370) with differing lengths allocates new rows and leaves the old span unreferenced (arena growth), no error.
- `source_span_index` (4185-4190) panics on invalid handles (`expect`), fail-closed by crash.

---

## Coverage

- `service_declarations.rs` lines 1-3089 (3089 lines): read in full.
- `type_identity.rs` lines 1-8305 (8305 lines): read in full (large `#[cfg(test)]` blocks at 1339-1718 and 3465-4861 read but described briefly).
- `type_system.rs` lines 1-5245 (5245 lines): read in full (test modules 2418-2541, 2695-2810, 4898-5242 read but described briefly).
- `values.rs` lines 1-5262 (5262 lines): read in full (display writers 208-782 and test modules skimmed for shape while reading all lines).
- Total 21901 lines read, 0 not read.
- Greps run (all `grep -rwo --include=*.rs NAME . | wc -l` and `grep -rlw ... | wc -l` from `omega-rust`, plus Grep-tool count mode for service_declarations names).

## Five things a maintainer most needs to know about this unit

1. Type identity is a string. Everything that asks "same type?" ends in equality of the `tag(a,b)` text from `normalize_type_reference` (type_identity.rs:2308); the entry points that supply no `invalid_*` flag (`type_identity`, `normalized_type_identity`, `storage_type_identity`, 7836/7629/7871) never fail and render rejected const arguments as the shared marker `unsupported-const-substitution` (6525-6536, 7964-7987), while the `applied_*`/`exact_owner_*` family returns `None` on any flagged problem.
2. "Unreachable" is a recorded empty set of type worlds (type_system.rs:760-767); several consumers treat it as permission to accept or delete: `type_identities_match_in_static_context` returns `true` (type_identity.rs:7850), `common_static_type_reference` skips (type_system.rs:1948), and `elide_unreachable_*` remove arms/transitions (1368-1454).
3. Several decisions key on spelling, not symbol: `PrimitiveType::from_name` (type_system.rs:4648) incl. `starts_with("AtomicU32#AccessField<")`; symbol-less `Named` parsed as integer/`usize` (type_identity.rs:2745-2760, type_system.rs:3722); `names_parameter` spelling arm (values.rs:4003); binder lookup by name when symbol is invalid (type_identity.rs:1997-2006); direct-callback to binder match by name (service_declarations.rs:3005).
4. `service_declarations.rs` builds two parallel censuses over the same boundary traits/machines/operators: `PreparedServiceCatalog` (service surfaces, 1166) and `PreparedBoundarySignatures` (ABI/calling rows, 2777). Ambiguity is turned into absence in several places (`collect_scopes` 164, `catalog::record` 1421, `receiver_applications` returning `Some(vec![])` at 184/191) rather than an error.
5. There are four integer evaluators (values.rs:80 i128, values.rs:4154 i64, type_system.rs:180 BigRational, type_identity.rs:6556 i128 parse) and three expression-walking styles (`subtree_any`, hand-written recursion, explicit-stack `evaluate_value`); changes to what counts as a "closed integer" must be made in each.

## What you are unsure about

- I did not read the callers of `names_parameter`, `expressions_structurally_equal`, `static_type_contexts` producers (`ProvenTypeContext.reachable`), `classify`/`is_integer_expression`, or the single caller of `type_identities_match_in_static_context`, so every "open" item above is an unverified path, not a confirmed defect.
- Whether the front end forbids user types spelled like primitives (affecting `PrimitiveType::from_name`) was not traced.
- The correctness of the ABI census `abi_signatures::prepare` closure return types (`Result<_, &str>` vs `String`) was not checked; I only recorded behaviour.
- Counts from `grep -rwo` count textual occurrences including definitions, tests and comments; "zero call sites" for `callable_path_by_text`, `const_expression_sites`, `copy_expression_handles_from_slice` means no other textual occurrence in `omega-rust`, not proof of unreachability (macro or generated use not checked).
- The `service_declarations.rs` per-function usage counts were taken as grouped totals (31 and 12 occurrences), not per function.
- Interaction details of `structural_fingerprint()` (memo revision for callable identities) were not read.
