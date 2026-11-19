# Unit 44 notes

Reader 44 of 113. Checkout: `<checkout>/` (main at 457adf4a11f).

Files covered (all under `omega-rust/psi/compilation/pipeline/`, abbreviated below as `03/` = `03_symbol-resolved-trees-to-typed-trees/src/` and `04/` = `04_typed-trees-to-typed-trees/src/`):

1. `03/call_and_product_selections.rs` (2227 lines)
2. `03/contracts_and_signatures.rs` (972)
3. `03/declarations.rs` (2269)
4. `03/equality_and_exhaustiveness.rs` (2446)
5. `03/expression_lowering.rs` (3905)
6. `03/lib.rs` (347)
7. `03/lowerer.rs` (2214)
8. `03/statement_lowering.rs` (1422)
9. `03/type_reference.rs` (3051)
10. `04/assembly_and_operators.rs` (2179)

Total 21,032 lines, all read in full. All "read" claims below are from the Read tool; all counts say which Grep produced them.

Stage context (read, `03/lib.rs:1-22`): stage 03 takes `SymbolResolvedTrees` and produces `TypedTrees`; stage 04 (file 10) rewrites `TypedTrees` to `TypedTrees`.

---

## 1. `03/lib.rs` (347 lines)

**Purpose.** Declares the crate's modules, re-exports the entry points, and holds two small helper modules: `access_borrow_factory` (recognises sealed toolchain "View" machines) and `tables` (field accessors).

**Shape.**
- Module list `lib.rs:26-33`; re-exports `lib.rs:35-47` (`contracts`, `signatures`, `expressions::{...}`), and `lib.rs:342-347` (`access_borrow_factory`, `access_borrow_factory_of`, `lower_symbol_resolved_trees`, seeded-continuation items).
- `access_borrow_factory(program, target_symbol) -> Result<Option<AccessBorrowOperationRequest>, Diagnostic>` `lib.rs:55`; finds the machine by symbol (or by scanning machines and their states, `lib.rs:59-71`), then calls `access_borrow_factory_of` `lib.rs:78`.
- `resident_backing` `lib.rs:214` builds the `Resident { extent, granted, resident }` backing.
- `tables` `lib.rs:303-340`: five accessors (`expression_table`, `expression_table_mut`, `statement_table_mut`, `resolved_expressions`, `resolved_statements`, `type_constraints`, `type_references`).

**Inputs/outputs.** Reads resolved machines, symbols, domain definitions, type constraints. Output is an optional request record or a `Diagnostic` ("sealed borrowed View prototype: ..." `lib.rs:105-106`).

**How it decides.** Keyed on spelling and file origin:
- Leaf of the machine name must be `view`, `view_mut` or `view_resident_mut` (`lib.rs:82-87`, via `rsplit("::")`). Anything else returns `Ok(None)`.
- "Sealed" = symbol's source file has `origin == Toolchain` and its path relative to the package root is exactly `layout.omg` (`lib.rs:88-98`); the attached data symbol must be sealed and named `"Access"` (`lib.rs:99-104`). Not sealed returns `Ok(None)`.
- For the resident form the referee must be `Constrained` over a `Named` data called `"Extent"` in toolchain `extent.omg` (`lib.rs:221-240`), with domain definitions `Granted` and `Resident` found by leaf name and unique (`lib.rs:243-253`), and exactly two constraints (`lib.rs:255`).
- Once the name/origin gate passes, every other mismatch is an `Err` (`lib.rs:107-206`), i.e. fail-closed for a sealed-looking machine that is malformed.

**Cases by shape.** `lib.rs:107`: `let [policy, schema] = ...machine_type_parameters` "must declare exactly two type parameters"; `lib.rs:144` exactly one state; `lib.rs:148` exactly one source parameter; `lib.rs:193` exactly two result arguments. Huge conjunction of "must have no X" at `lib.rs:110-138`.

**Same job elsewhere.** `typed-trees/src/access_views_and_commitments.rs:1019` defines a method also named `access_borrow_factory` (grep: `access_borrow_factory\(` across `omega-rust`; I did not read it, "suspected" different: it is a lookup on `TypedTrees`). Callers of the `03` function: `declarations.rs:607` (1 in this crate), `build-time-evaluation/src/layouts.rs:1705` calls `access_borrow_factory_of` (1). `validation/src/recasts.rs:5114,5209` and `calls.rs:3684` call the `TypedTrees` method (3).

**Unused / one-caller.** `access_borrow_factory` (the by-symbol form) has 1 caller outside its definition (`declarations.rs:607`); `access_borrow_factory_of` has 1 outside caller (`layouts.rs:1705`) plus the by-symbol wrapper. `tables::expression_table` (`lib.rs:313`) is used by `call_and_product_selections.rs` (`expression_table(program)` at `1502`, `2033`, `2072`) so it has users. Comment `lib.rs:9-10` calls `type_resolved_cohort`/`close_typed_cohort` "two shared cohort operations": grep `type_resolved_cohort|close_typed_cohort` shows all code uses are in `lowerer.rs` (3 call sites of each, inside this crate, via three entry paths `lowerer.rs:791/807`, `864/878`, `1658` + `2138`). Outside `lowerer.rs`: 0 uses (only `README.md:36,59` and the `lib.rs` comment mention them).

**Stated limits.** `lib.rs:170` message "resident source must retain exact core Extent, Granted and Resident<P, T> declarations".

**Fail-open risks.** `Ok(None)` for a machine that merely is not named `view`/... or is not in toolchain `layout.omg` (`lib.rs:86,99-103`): a non-toolchain machine named `view` is simply not treated as a factory, so no `AccessBorrowOperationRequest` is recorded. Traced: that is the intended gate (user code must not mint a factory); not an accept path. Otherwise none seen.

---

## 2. `03/call_and_product_selections.rs` (2227 lines)

**Purpose.** A bag of seven helper modules used by expression/statement lowering: lowering `Build` selection calls to a prepare/commit pair of calls against a toolchain protocol, resolving declared call types, capturing static operands into a "product operand graph", landing construction destinations (struct/array/string/match initialisers), installed-interrupt requests, Product queries, and semantic qualification casts.

**Shape (each module with entry function).**
- `build_selections::lower` `:15`. Takes a typed `BuildSelectionRequest`, argument handles, and an acknowledgement; emits two synthesized `Call` expressions (prepare on arg 0, commit with all args) on a receiver naming the toolchain trait `BuildBindingOperations` (`:88-106`, `:145-221`), plus optional per-position value-helper calls (`:121-132`, `:180-204`).
- `call_results` `:226`: `declared_call_state` `:232`, `declared_call_return` `:248`, `declared_call_type_parameters` `:277`, `declared_call_parameters` `:299`, `peel` `:320`, `computed_receiver_method_target` `:337`, `declared_type_method_target` `:416`.
- `capture_product_operands::capture` `:477` with `struct Capture` `:496` and methods `operand` `:688`, `ty` `:768`, `expression` `:932`, `pattern` `:1129`; free fn `binary` `:1185`. Converts a resolved `StaticMachineArgument` into a `ProductStaticOperandGraph` (names, types, constraints, expressions, match arms).
- `construction_destinations::prepare` `:1226`, `needs_destination` `:1444`, `land_one` `:1455`, `callee_owns_nominal_lifetimes` `:1560`.
- `installed_interrupts::lower_request` `:1598`.
- `product_queries::operation` `:1759`, `lower` `:1790`.
- `product_selections::request` `:1862`.
- `qualification_casts::normalize_qualification_casts` `:2028`.

**Inputs/outputs.** Resolved trees (read), typed trees (mutated: expression table, type reference table, selection ledger). `capture` produces a graph stored inside a request; `prepare` mutates `construction_required_type_reference`/`application_type` on struct literals; `normalize_qualification_casts` sets `result_type`, `semantic_domain_symbol`, `semantic_domain_id` on cast nodes and finalizes late-bound selection occurrences (`:2156-2224`).

**How it decides (keys).**
- `build_selections::lower`: operation string chosen by `(request variant, arguments.len())` match `:52-87` ("await_provider", "exclude", "bind", "bind_description", "bind_native", "select", "select_provider"); any other pair is `Err("Build selection has unexpected ordinary operands")` `:81-86`. Protocol trait found by `definition.is_boundary && name == "BuildBindingOperations"` and source file origin Toolchain with path exactly `<build-prelude>` (`:88-102`). Operations found by `signature.name.as_str() == name` (`:107-118`: "prepare" and the chosen operation). Value helpers found by machine name `"build_selection_target"` / `"build_selection_independent"` / `request.vocabulary.helper()` and `is_build_prelude_symbol` (`:123-128`, `:181-189`).
- `product_queries::operation` `:1759`: both namespace and target must be build-prelude symbols (`:1765-1768`); keyed on `(symbols.name(namespace), name)` pairs `("BuildProduct","entry"|"schema"|"provider")`, `("ProductTypeSchema","path")`, `("ProductProviderRef","path")` (`:1770-1775`); protocol found by `is_build_protocol(..., "BuildProductOperations")` `:1776-1779`.
- `product_selections::request` `:1862`: names `"exclude_crash"`, `"exclude_physical_authority"` (`:1879-1883`); `BuildOperation::from_call_target(name)` filtered by `has_designated_operands()` (`:1899-1903`); namespace must be a data definition named `"Build"` in the build prelude (`:1904-1913`).
- `installed_interrupts::lower_request` `:1598`: callee symbol name `"run"` whose owner attached data is named `"X86StaticInterruptScope"` and both have Toolchain origin (`:1619-1628`); otherwise `Ok(None)`.
- `declared_type_method_target` `:416`: builds the string `format!("{}::{}", name(owner), target)` and looks it up with `lookup_signature_free_top_level_from_source_matching` (`:427-440`); then picks the single state in that machine with `state.name == target` (`:448-460`); on 0 or >1 returns invalid.
- `computed_receiver_method_target` `:337`: walks `Member`/`Indexed`/`Borrow` receivers down to a `Call` (any other node returns None, `:357`), then replays field/index projections by field *name* (`field.name == member.member`, `:391-398`) and requires a single field.
- `construction_destinations::prepare` `:1226`: scans only expressions with arena index `> expression_frontier` (`:1236`) and machines at/after `machine_frontier` or in `completed_machines` (`:1289-1296`); destination-needing nodes are exactly `StructLiteral | String | ArrayLiteral | Match` (`:1444-1453`).
- `normalize_qualification_casts`: casts with empty `semantic_domain` get `result_type` from arithmetic domain (`:2050-2071`); others require exactly one `DomainMembership` occurrence (`:2083-2088`) and exactly one candidate domain from `select_domain_candidates` (`:2128-2136`).

**Cases by shape.**
- `:52-87` fixed list of 7 (request, arg count) shapes, rest rejected.
- `capture_product_operands::ty` `:830-832`: `Source::AliasUse` => `Err("captured Product static type cannot yet retain transparent alias application obligations")`.
- `construction_destinations::land_one` `:1455-1558`: handles `Match` (recurse per arm `:1464-1470`), `StructLiteral` (only if `exact_template` or closed data constructor `:1490-1501`; otherwise `Ok(())` and nothing landed), `ArrayLiteral` with `FixedArray` carrier (`:1541-1556`), then delegates to `validation::land_owned_fixed_byte_literal` `:1557`.
- `product_selections::request` `:1929-1949`: per-operation operand counts, message tables by operation ("one exact requirement operand", "four schema declaration operands and compatibility facts" `:1938`).
- `product_selections` `:1970-1977`: operands must be bare declarations; applications only allowed for roles `BoundarySubject | Provider` (`:1971`).

**Same job elsewhere.**
- Resolve-a-method-target-for-a-receiver is done three ways: `call_results::computed_receiver_method_target` `:337`, `call_results::declared_type_method_target` `:416`, and `expression_lowering.rs:2360` `declared_receiver_method_target` (which calls `declared_type_method_target`). Called together at `expression_lowering.rs:1381-1403`, and `declared_receiver_method_target` again at `statement_lowering.rs:259`.
- Peel references/constraints to a named data: `call_results::peel` `:320` vs `value_type_base_name` `equality_and_exhaustiveness.rs:704`, `table_type_base_name` `:724`, `data_index_for_type` `:1948`, `named_data_of` `statement_lowering.rs:809`, `resolved_data_definition_for_type` `expression_lowering.rs:2288`. Five peelers in this unit. Another `peel` exists in `02_syntax-trees-to-symbol-resolved-trees/src/symbols_expression_paths.rs:168` (grep `fn peel`; suspected duplicate, not read).
- "Lower a static machine argument" exists as `expression_lowering.rs:3773` (typed form) and as `capture_product_operands::operand` `:688` (graph form with its own `ty`/`names`/`expression` copies of the type and expression vocabulary).
- `retain_coordinate` `:512-549` and `expression_selections` `:887-930` both repeat the "get occurrence, `source_coordinate_occurrence`, compare with original, build `ProductOperandSelection`" sequence; the same sequence is in `build_selections::lower` `:25-50`.

**Unused / one-caller.** grep (Grep, content) results:
- `declared_call_state`, `declared_call_return`, `declared_call_type_parameters`, `declared_call_parameters` (`fn` greps): each defined once; `declared_call_return` is used in `equality_and_exhaustiveness.rs:1864`, `statement_lowering.rs:267`, `expression_lowering.rs:2432,...` (not counted exactly); `declared_call_state` is used inside this file `:256,307,360`. No unused claim made.
- `closed_boolean_guard`: defined `equality_and_exhaustiveness.rs:1282`, used 5 times in that file and once in `statement_lowering.rs:1041`.

**Stated limits.** `:830-832` "cannot yet retain transparent alias application obligations" (quote 9 words: "cannot yet retain transparent alias application obligations").

**Fail-open risks.**
- `construction_destinations::prepare` `:1379-1381` (`binders.len() != static_arguments.len()` => `continue`) and `:1389-1391` (`call_target_parameters` None => `continue`): the call's literal arguments never get a landed receiving type from here. Input reaching it: a call where static argument count differs from declared type-parameter count. Not an accept by itself (the later stage checks the call), but the destination is not prepared; **open**.
- `land_one` `:1495-1501` returns `Ok(())` without landing when the literal is neither the exact template nor a closed data constructor; **open** (comment `:1485-1489` explains only the runtime-index case, "Stage 04 snapshots a computed operand...").
- `normalize_qualification_casts` `:2128-2136`: zero or multiple domain candidates push an update with an invalid symbol instead of erroring; later `domain_definition(invalid)` returns None so `result_type` becomes invalid (`:2191-2214`). A cast `x as T in D` where `D` is unknown/ambiguous gets no error here. **open** (error presumably raised by a later check, not traced).
- `installed_interrupts::lower_request` `:1609-1630`: returns `Ok(None)` for anything not exactly named `run` / `X86StaticInterruptScope` / toolchain. Intended gate; **traced** as name+origin gate, not an accept.
- `product_selections::request` `:1899-1913`: unmatched shapes return `Ok(None)` and the call stays an ordinary call. **traced**: ordinary call then fails resolution if the name does not exist.

---

## 3. `03/contracts_and_signatures.rs` (972 lines)

**Purpose.** Lowers authored contract invocations (`invokes`), `where` proof facts (expression facts and domain-membership facts), parameter domain membership contracts, callable signatures, conformance bounds, enclosing-machine captured arguments, parameters and type parameters.

**Shape.**
- `contracts::invocations::lower_authored_invocations` `:14`.
- `contracts::parameter_domains` `:57`: `lower_state_parameters_collecting_domain_constraints` `:74`, `push_domain_membership_contracts` `:99`.
- `contracts::proof_facts` `:125`: `domain_selections::normalize` `:136`, `lower_proof_facts` `:297`, `intern_proof_membership_instances` `:442`, `lower_contract_kind` `:482`, `lower_signature_contract` `:514`, `lower_expression_fact` `:531`, `lower_statement_fact` `:546`, `check_membership_argument_count` `:425`.
- `signatures::callable_signature::lower_state_signature` `:570`; `evidence::lower_conformance_bounds` `:668`; `lexical_context` `:697` (`enclosing_parameters` `:719`, `enclosing_bounds` `:728`, `call_arguments` `:735`, `captured_arguments` `:751`); `parameters::{lower_state_parameter, state_parameter}` `:799,813`; `type_parameters::{lower_type_parameters, lower_type_parameter, lower_type_parameter_kind}` `:835,851,866`.

**Inputs/outputs.** Reads resolved signatures/contracts; appends typed signatures, proof facts, contracts. `domain_selections::normalize` rewrites `membership.domain_symbol` in place and finalises late-bound selection occurrences (`:246-276`).

**How it decides.**
- Invocation target: first parameter whose `name == declaration` string (`:22-26`), else a top-level `Trait` symbol found by name and source span (`:36-42`); else `AuthoredInvocationTarget::Unresolved` (`:43-46`).
- Membership fact normalisation `:136-278`: only memberships with invalid `domain_symbol` (`:151`), a carrier type found by `parameter_expression_result_type_reference` (`:154-163`), an authored occurrence (`:166`), exactly one candidate (`:193`), not an alias (`:196`) are updated. Conflicting updates for one occurrence => `Err` (`:248-256`).
- Termination guarantee on a signature: `terminates_guarantee` true => `Terminates { premises: Vec::new() }` else `NoGuarantee` (`:607-613`); premises are filled in later by `lowerer.rs:57` (`normalize_trait_progress_premises`).
- Captured arguments `:751-790`: when the callee (machine or state) has an enclosing machine parent, passes all enclosing type parameters, then enclosing conformance-bound binders, as static arguments named by `lower_name(&parameter.name)`.

**Cases by shape.** `:895-899` `MachineParameterContract::AuthoredNominal` => `Err("an unresolved nominal machine-parameter requirement reached typed lowering")`; `:906-916` nominal requirement path must be `[trait_path.., requirement_name]` with non-empty trait path.

**Same job elsewhere.** `intern_proof_membership_instances` `:442` and `domain_constraints::normalize_constraint_span` (`type_reference.rs:147-309`) both compute `indexed_domain_instance_name` + `semantic_domains.intern` for domain instances (`type_reference.rs:212-222`, `contracts_and_signatures.rs:462-472`), and `qualification_casts` `call_and_product_selections.rs:2146-2152` does it a third time. Three copies of "arity check, instance name, intern id".

**Unused / one-caller.** Grep `lower_statement_fact`: defined `:546`, used 4 times in `statement_lowering.rs` (`1103,1119,1146,1152,1307`). `lower_fact_expression` has 1 outside user (`contracts_and_signatures.rs:544` itself) and its definition `expression_lowering.rs:3893`. `lower_expression_fact` `:531` is used at `:309`, `:551`.

**Stated limits.** None as comments. Error text `:1511` in lowerer is separate.

**Fail-open risks.**
- `domain_selections::normalize` `:154-198` has four `continue`s (no carrier, no authored occurrence, not exactly one candidate, alias). The membership keeps `domain_symbol` invalid. What gets through: a contract `x in D` that cannot be matched to exactly one domain. **open**; at `lowerer.rs:1870` it runs after `normalize_machine_progress_premises` (`lowerer.rs:1854`), so progress-premise extraction (`lowerer.rs:322-324`) sees the unnormalised `domain_symbol`.
- `lower_authored_invocations` `:43-46` keeps `Unresolved` invocations. **open** (consumer not read).

---

## 4. `03/declarations.rs` (2269 lines)

**Purpose.** Per-declaration-family lowering from resolved to typed: conformance, data, domain, machine (with states, owned data, conformance edges, ranking custody, two-phase body completion), mathematical definitions, measures, operators, propositions, provider-default call finalisation, states (with proof-output binding packages), traits (with refinement clauses), type aliases, wire schemas.

**Shape (one function per family).** `conformance::lower_conformance` `:15`; `data::lower_data_definition` `:145`, `lower_data_member` `:320`; `domain::lower_domain_definition` `:383`, `lower_cleanup_selection` `:473`; `machine::{generic_data::is_derived` `:564`, `lower_machine` `:603`, `lower_token_binding_view` `:626`, `attached_domain_symbol` `:683`, `lower_machine_contents` `:727`, `retain_machine_ranking_expressions` `:996`, `complete_machine_body` `:1032`, `lower_machine_implementation` `:1045`, `settle_satisfied_declarations_from` `:1153}`; `mathematical::lower_mathematical_definition` `:1227`; `measure::lower_measure_definition` `:1323`; `operator::lower_operator_definition` `:1379`; `proposition::lower_proposition_definition` `:1439`; `provider_defaults::finalize_provider_default_calls` `:1546`; `state::{lower_state_declaration` `:1642`, `complete_state_body` `:1679`, `proof_output_call_requires_execution` `:1765`, `proof_output_runtime_call` `:1793}`; `trait_definition::lower_trait_definition` `:1922`; `type_aliases::lower_type_alias_definition` `:2187`; `wire::lower_wire_schema` `:2218`.

**Inputs/outputs.** Reads resolved declarations; pushes typed declarations (`push_*`). Machine lowering is two-phase: declaration first (`body_status: Deferred` `:761`), then `complete_machine_body` fills owned-data initialisers, `via_expression`, state statements, termination witness (`:1045-1150`), with consistency checks that the declaration was not changed (`:1050-1063,1086-1095,1112-1116`).

**How it decides.**
- Conformance to a refinement trait is rejected: `trait_definition(...).refines.is_some()` => error "a transparent refinement ... is not implemented directly" (`:21-30`).
- `is_derived` `:564-600`: "valid" is a conjunction of source-span equality (`template.name.source_span() == origin.template_source.source_span()`, `:584-585`), public/supply mode equality, and `owner.generic_instance` base symbol equals template's attached data; failure is `Err("generic-data method lost its exact template and closed-owner derivation")` `:595-598`.
- Token-binding view `:626-681`: only for machines with `spelling` and not derived; `home_domain` via `attached_domain_symbol` `:683-702` which matches by `domain.name.as_str() == attached.as_str()` and requires exactly one match (string name key).
- Provider defaults `:1546-1561`: any machine whose name `ends_with("::provider_defaults")` gets `purpose = ProviderDefaults` (`:1550`); calls are recognised via `is_unresolved_provider_selection_call(call.target.as_str(), call.target_symbol)` (`:1597`) or `BuildOperation::from_call_target(call.target.as_str()) == Some(ProviderSelection) && !call.target_symbol.is_valid()` (`:1607-1609`).
- Proof-output binding `:1700-1750`: runtime-call-statement index is chosen when any binding has `output_field == "value" && binding != "_"` (`:1709`); otherwise `proof_output_call_requires_execution` (`:1765`): true when the call node is not a `Call`, or the target state is not found (`:1766-1781`); else `state.return_type.is_none() && (supply_mode != CheckedBody || state count != 1 || !statement_nodes.is_empty())` (`:1783-1790`).
- Trait refinement clause `:1994-2173`: narrowing-only checks (`suspends`, `blocks`, `reaches`) with messages "a refinement narrows, it cannot add an axis" (`:2070,2079`); named requirement matched by `machine.name == *requirement` (`:2037`).

**Cases by shape.** `:1994` refinement base must be a trait; `:2033-2065` clause either names exactly one base requirement or covers all; `:2102-2106` wildcard `reaches _;` cannot combine with named services; `:2143-2165` reach is `Inherited`, `IndependentBounded` (wildcard) or `Concrete`.

**Same job elsewhere.**
- `lower_state_declaration` `:1642` repeats `lower_state_parameters_collecting_domain_constraints`/`push_domain_membership_contracts` also used by `lower_state_signature` (`contracts_and_signatures.rs:619-657`); both follow the same four steps (parameters, authored contracts, domain membership contracts).
- Retaining type reference selections: `retain_type_reference_selection` is called at `:31`, `:208`, `:732`, `:849`, `:1952`, `:2003`.
- `trait_definition` uses `std::collections::HashSet` (`:2030`) for `seen_named`; repository rule says dense IDs, not hash maps (`AGENTS.md`), this is a set of strings of requirement names (small).

**Unused / one-caller.** `lower_token_binding_view` called 1 time (`lowerer.rs:2064`). `lower_cleanup_selection` is called 2 times in this file (`:303`, `:438`). `settle_satisfied_declarations_from` called once (`lowerer.rs:1853`). `finalize_provider_default_calls` once (`lowerer.rs:1896`).

**Stated limits.** `:2093` message "declares no reach set to narrow" is an error, not a limit. None.

**Fail-open risks.**
- `proof_output_call_requires_execution` `:1765-1781` returns `true` (i.e. keep the runtime call) when unsure: conservative (keeps more code). **traced**: not an accept.
- `finalize_provider_default_calls` `:1550`: name suffix `::provider_defaults` marks purpose, so a user machine named `foo::provider_defaults` gets `ProviderDefaults` purpose and its provider-selection calls get `Intrinsic::BuildProviderSelection` custody (`:1572-1577`). What gets through: a user-authored machine with that suffix is treated as a provider-defaults machine. **open** (no origin check; whether a user machine can have that name was not traced).
- `settle_satisfied_declarations_from` `:1165-1169`: if `resolve_satisfied_declaration` is None, `continue` leaves `requirement_symbol` invalid; **open**.
- `attached_domain_symbol` `:683-702` returns invalid when 0 or >1 name matches; used only as `home_domain` of a token-binding view. **open**.

---

## 5. `03/equality_and_exhaustiveness.rs` (2446 lines)

**Purpose.** Four pre-erasure checks on the resolved trees: (a) reject `==`/`!=` against a bare payload-bearing case name or a brace form with no `Equatable` implementation (`equality`), (b) validate synthesized `Equatable` conformances (`equatable`), (c) case-dispatch exhaustiveness for transition runs (`exhaustiveness`), and (d) lower proposition applications (`proposition`).

**Shape.**
- `equality::validate_equality_operands_in` `:38`, `scan_expression` `:169`, `check_equality_operand` `:250`, `payload_bearing_case` `:301`.
- `equatable`: consts `EQUATABLE_TRAIT_PATH = "Equatable"` `:334`, `EQUALS_REQUIREMENT = "equals"` `:335`; `DataEqualityShape` `:338`; `EqualitySelection` `:405`; `equality_selection` `:456`; `field_equality` `:515`; `validate_equatable_conformances_in` `:582`; `validate_equatable_data` `:618`; `byte_slice_text_view` `:651`; `byte_sequence_text_carrier` `:672`; `value_type_base_name` `:704`; `table_type_base_name` `:724`; `EqualityScope` `:746` (`value_types: Vec<(String,String)>`, `synthesized_row_data`), `for_state_interface` `:752`, `for_state` `:786`.
- `exhaustiveness::validate_case_dispatch_exhaustiveness_in` `:829`; types `ArmShape` `:879`, `FiniteConstraint` `:891`, `CaseClaim` `:899`, `LeafContribution` `:905`, `DispatchScope` `:916`; `check_dispatch_run` `:922`; `first_uncovered_assignment` `:1171`; `classify_arm` `:1319`; `classify_membership_union` `:1451`; `expressions_structurally_equal` `:1726`.
- `proposition::lower_proposition_application` `:2057` and helpers to `:2445`.

**Inputs/outputs.** Read-only over `SymbolResolvedTrees` for the three validators (return `Result<(), Diagnostic>`); `lower_proposition_application` mutates typed trees and returns a `PropositionApplication`.

**How it decides (keys).**
- `check_equality_operand` `:250-299`: a `Name` operand counts only if its path is exactly two members `[type_name, case_name]` (`:257-259`); the type is found by name (`data_definition_by_name_from`, `:343-365`: by symbol-from-source if the span is non-empty, else the unique definition with that `name`). A `StructLiteral` with a case name that has payload and no Equatable selection is an error (`:286-296`).
- `is_equatable_implementation` `:417-422`: `display_path(trait_symbol, "::") == "Equatable"` (string compare of spelled path).
- `field_equality` `:515-580`: primitive by `PrimitiveType::from_name(&base_name)` (`:534`), text by `byte_sequence_text_carrier` (`:522`), data looked up by name (`:538-545`).
- Exhaustiveness arms are classified by shape: membership unions (`classify_membership_union` `:1451`), boolean compares against literals (`:1346-1383`), `Equal`/`NotEqual` pairs (`:1384-1393`), everything else `Opaque` (`:1395`, `:1447`). Subjects identified by `expressions_structurally_equal` (`:1726-1820`), which compares `Call` by `target.as_str()` and receiver/arguments (`:1805-1816`), `Member` by member string (`:1773`), `Cast` by `value` only (ignores `target_type`, `:1801-1803`), `Name` by symbol plus path strings (`:1765-1771`).
- Matrix size cap `MAX_MATRIX_CELLS: usize = 65_536` `:1176`; over-cap => "transition pattern matrix ... too large ... add a `_` arm" (`:973-977`).
- `variant_is_impossible` `:1144-1158`: a variant is excluded from the axis when a `where` fact is integer literal `0` or boolean `false`.
- Proposition binder arguments are classified by `SymbolKind` (`classify_proposition_static_symbol` `:2281-2328`); literal fit by `integer_literal_fits` `:2416-2445` (i8..i64, u8..u64, Addr).

**Cases by shape.**
- `:257` "let [type_name, case_name] = members else { return Ok(()) }": only two-segment case paths are checked.
- `:1635` same two-segment restriction in `find_case`.
- `:1328` single conjunct vs `:1398-1448` multi-conjunct arms; `:1559-1563` a declared domain is "countable" only if its `facts` is exactly one expression of an `Or` of memberships.
- `:2096-2103` proposition binder count must equal machine argument count; `:2232` value argument count must equal parameter count.
- Comment `:639`: "recursive Equatable implementations are not supported yet" (same text at `expression_lowering.rs:3468`).

**Same job elsewhere.**
- `validate_equatable_data` `:618-649` and `structural_equality::field_compare` `expression_lowering.rs:3427-3493` both walk the data shape through `field_equality` and both keep a `visiting: Vec<String>` stack of type names for recursion rejection (`:612,637` and `expression_lowering.rs:3049,3466`). Two recursion guards over the same relation.
- Looking up a data definition by name: `data_definition_by_name_from` `:343`, `data_definition_index` `:1645`, `attached_field_type` `statement_lowering.rs:849-869` (by `data.name == data_name`), `resolved_data_definition_for_type` `expression_lowering.rs:2288`. Four name-keyed lookups.
- "Declared type of a name": `name_declared_type` `:1873`, `declared_symbol_type` `expression_lowering.rs:2611`, `parameter_type`/`local_type` `statement_lowering.rs:871-903`.
- `expressions_structurally_equal` also exists at `typed-trees/src/values.rs:4006` (grep `fn expressions_structurally_equal`; **suspected** same job on typed trees, not read).
- `symbol_key`: this unit defines it twice (`lowerer.rs:1724`, `type_reference.rs:1854`); Grep `fn symbol_key` finds 10 definitions in `omega-rust` in total (9 files).

**Unused / one-caller.** (Grep content)
- `table_type_base_name` `:724`: 3 occurrences in this file (def + recursion + `:794`); no outside caller.
- `byte_slice_text_view` `:651`: 2 uses, both in this file (`:676` and its def).
- `data_definition_index` `:1645`: used by `expression_lowering.rs:46,339` (1 outside).
- `closed_boolean_guard` `:1282` has 1 outside user (`statement_lowering.rs:1041`).

**Stated limits.** `:639` "recursive Equatable implementations are not supported yet"; `:974-976` "too large for the exhaustiveness proof".

**Fail-open risks.**
- `check_dispatch_run` `:932-934`: `if has_continuation { return Ok(()) }` skips the exhaustiveness check for any run containing a transition with a continuation. No comment. What gets through: a transition run with a `continuation` target and no `_` arm. **open**.
- `check_dispatch_run` `:943`: a `Default` arm returns OK; `classify_arm` returns `Default` for a closed-true guard (`:1336-1340`, `:1354-1356`), reading `const` initialisers through names (`:1304-1317`). A named constant whose initialiser is literally `true` is a catch-all. **traced** (the const is compile-time known).
- `compare_pair_closes` `:955-967`: `x == y` plus `x != y` pair counted as exhaustive when both sides compare equal under `expressions_structurally_equal`; that function compares `Call` by target *name* string and ignores `target_symbol` (`:1805`) and ignores `Cast.target_type` (`:1801`). So two calls with equal spelled names but different resolved targets, or casts to different types, would be treated as the same expression. **open**.
- `check_equality_operand` `:297`: `_ => Ok(())` and `:257-259` `Ok(())` for any name path that is not exactly 2 members: a three-segment qualified case path (`module::Type::Case`) is not flagged. **open** (inferred from the match; not run).
- `lower_proposition_application` `:2160-2162`: a `Machine` binder with an `evidence_projection` argument is accepted with no further check. **open**.

---

## 6. `03/expression_lowering.rs` (3905 lines)

**Purpose.** Lowers any resolved expression table entry to the typed expression table: all node kinds, `match` and destructuring patterns, executable domain membership, structural equality expansion, special call forms (Product queries, Build selections, `Quotient::lift/define`, `LayoutPlan::place_private`, layout queries, `code_layout`, `address_of`), member/field binding, and static machine arguments.

**Shape.**
- `expression::domain_membership` `:44` (`lower_case_membership_expression` `:56`, `lower_domain_membership_expression` `:201`, `is_pending_data_subject` `:169`, `combine_conjunction` `:358`); `name_paths` `:382`; `native_code_layout` `:432` (`lower_request` `:439`, `result_type` `:472`); `operators::lower_binary_operator` `:515`.
- `table::lowerer::ExpressionTableLowerer` `:839` (fields: `program: Option<&SymbolResolvedTrees>`, `source`, `target_trees`, `symbol_substitution`, `equality_scope`, `fact_position`, memo `lowered_expressions: Vec<(handle, bool, handle)>`, `match_captures`). Core: `lower` `:952`, `lower_value` `:1049`, `lower_node` `:1060`, `lower_binary_node` `:1071`, `lower_nonbinary_node` `:1120`, `lower_quotient_operation_request` `:1698`, `lower_private_layout_operation_request` `:1819`, `lower_data_layout_query` `:1904`, `nullary_erased_initializers` `:2112`, `lower_membership_expression` `:2238`; `match_dispatch` `:593` (`lower_match` `:600`, `lower_destructure_pattern` `:655`, `lower_saved_subject` `:776`).
- `member_binding` `:2347` (`declared_receiver_method_target` `:2360`, `declared_member` `:2372`, `declared_symbol_type` `:2611`); `structural_equality` `:2713` (`try_lower_synthesized_equatable_call` `:2736`, `try_lower_structural_equality` `:2871`, `expand_structural_equality` `:3043`, `structural_equality` `:3213`, `sum_equality` `:3291`, `field_compare` `:3427`).
- Entry points `:3654-3710` (table level) and `:3714-3904` (public wrappers, `lower_static_machine_argument` `:3773`, `lower_fact_expression` `:3893`).

**Inputs/outputs.** Reads a resolved expression table (+ optional program) and writes into `TypedTrees.expression_table`. Memo is a linear `Vec` keyed `(source handle, fact_position)` (`:849-853`, search `:959-967`).

**How it decides (keys).**
- Call special forms keyed by spelled target and shape:
  - `address_of`: `!call.target_symbol.is_valid() && !call.receiver.is_valid() && target.as_str() == "address_of" && machine_arguments.is_empty() && arguments.len()==1` (`:1023-1026`).
  - `code_layout`: `target == "code_layout" && !target_symbol.is_valid() && !receiver.is_valid()` (`:1360-1362`); statement form at `statement_lowering.rs:1319`.
  - Quotient: `target` is `"lift"`/`"define"` and receiver is a one-segment name `"Quotient"` (`:1703-1719`); if `receiver.head_symbol` or `receiver.symbol` or `call.target_symbol` is valid, error "cannot be shadowed by an authored declaration" (`:1728-1735`).
  - `place_private` with receiver `"LayoutPlan"` (`:1824-1835`), exact owner found by attached data name `"LayoutPlan"` (`:1866-1875`), toolchain origin of target and owner required (`:1881-1892`).
  - `native_code_layout::result_type`: data named `"CodeLayout"` with Toolchain origin and source path `ends_with("core/code_layout.omg")` (`:483-490`).
  - `resolved_data_definition_for_type` special case: `"LayoutStorage"`, `BoundaryOpaque`, Toolchain, one conformance bound, two arguments (`:2297-2305`).
- Structural equality `try_lower_structural_equality` `:2871`: operand data types are *names* (`operand_data_type_name` `:3132-3182` returns `String` from struct literal type name, declared symbol type, `EqualityScope.value_type(name)` for a one-segment name `:3154-3159`, or field type by name). Case-reference operands (a `Name` path with ≥2 members) skip expansion after a name compare (`:2880-2902`). Different type names => error (`:2889-2900`, `:2906-2917`). `left_type.or(right_type)` picks the type (`:2918`); no name => `Ok(None)` (ordinary binary).
- Domain membership `case_membership_test` `:100`: needs the last two path segments, both symbols valid, and the case symbol to be a variant of the type (`:109-123`); else `None`.

**Cases by shape.**
- `:3207-3209` structural `==` operands must be `StructLiteral`, `Borrow` of those, or `Name | Member | Indexed`; others: "cannot synthesize structural `==` for this `{type_name}` operand: bind the value to a local first".
- `:3303-3406` `sum_equality` handles (Literal,Literal), (Place,Literal)/(Literal,Place), (Place,Place), and rejects record-literal vs sum.
- `:3443-3449` text field against a literal is rejected.
- `:2145-2150` comment: "Open or applied evidence types still supply no implicit inhabitance" for the nullary-erased initialiser (`:2112-2191`): an omitted erased field is filled only when the evidence data is `CheckedShape`, nongeneric, no fields, with exactly one nullary variant.

**Same job elsewhere.**
- Two parallel entry-point layers: `table::lower_expression_handle_from_table_with_symbol_substitution` `:3654` and `expression::lower_expression_handle_from_table_with_symbol_substitution` `:3744` (wrapper); `table::lower_expression_handle_from_table_in_fact_position` `:3679` and the wrapper at `:3762`.
- `lower_transition_pattern`: method `:758` and wrappers `:3702` (table) — 1 caller `statement_lowering.rs:975`.
- `lower_expression_handle_from_table_in_scope` `:3668`: grep shows 1 caller (`:3725`). `lower_scoped_fact_expression` `:3690`: 1 caller (`:3897`). `lower_expression_handle_from_table` `:3734`: 1 caller (`type_reference.rs:2374`).
- Method-target resolution: see section 2 ("Same job elsewhere").
- `lower_static_machine_argument` `:3773` and `capture_product_operands::operand` `call_and_product_selections.rs:688` (see section 2).

**Unused / one-caller.** See above (greps). Comment `:15-17`: "`lower_expression_handle` reads the package's body expression table ... Declaration and statement lowering use it" — Grep `lower_expression_handle(` is used widely in `declarations.rs`/`statement_lowering.rs` (not counted).

**Stated limits.** `:1413` "late-selected receiver method retains an undischarged structural type equation; use a closed explicitly selected receiver" (error); `:1677` quotient `zero_value` "cannot observe or choose a retained quotient representative".

**Fail-open risks.**
- `try_lower_structural_equality` `:2918-2921`: when neither operand's data type name can be derived (call result, array element of unnamed type, etc.) returns `Ok(None)` and the `==` is lowered as a plain `Binary Equal` over data operands. What gets through here: `f() == g()` over a data type. Whether a later stage rejects it was not traced. **open**.
- `classifier_reference_operand` `:3079-3095` + `:2880-2902`: any `Name` with ≥2 path members on either side makes the compare return `Ok(None)` after a type-name check, regardless of whether the 2-member path is a case or a module-qualified value. **open**.
- `nullary_erased_initializers` `:2112-2191`: implicit population of omitted erased fields; **stated** (comment `:2141-2144`: "Each erased field must still name the nongeneric, accessible unique nullary declaration accepted below").
- `lower` `:1404-1415` evaluates `program.symbols.get(target_symbol).parent` where `target_symbol` is the late-selected local that may be invalid (`:1381-1403` ends in `.unwrap_or_default()`), but only inside `.any()` over machines with `has_structural_type_equations`; whether `symbols.get(invalid)` panics was not traced. **open (unsure)**.
- `lower_quotient_operation_request` `:1708-1719`: if the receiver is not a one-segment `Quotient` name, returns `Ok(None)` and the call is an ordinary call (not an accept of a Quotient op). **traced**.

---

## 7. `03/lowerer.rs` (2214 lines)

**Purpose.** The stage entry point and its orchestration: `lower_symbol_resolved_trees` runs validators then types every declaration family, then settles/normalises (`close_typed_cohort`). Also: terminating-record progress premise normalisation, seeded continuation (append-only extension and authored body completion), target-machine family telescope validation, `Lowerer` state.

**Shape.**
- `name::{lower_name, lower_names}` `:36-47` (`Identifier::generated(name.as_str())`).
- `progress` `:49`: `normalize_trait_progress_premises` `:57`, `normalize_machine_progress_premises` `:95`, `settle_machine_ranking_default` `:161`, `pending_ranking_default` `:174`, `inherited_guarantee` `:247`, `authored_premises` `:308`, `subject_path` `:348`, `resolve_subject_member_symbol` `:398`, `replay_subject_position` `:430`.
- `seeded_continuation` `:505`: `SeededContinuationError` `:522`, `SeededTypingBase { resolved: Arc<..>, typed: Arc<..> }` `:529`, `lower_symbol_resolved_trees_to_seeded_base` `:744`, `lower_seeded_extension` `:755`, `lower_body_completion` `:817`, retention checks `:889-1050`, `join_resolved_semantics` `:1052`.
- `target_machine_families` `:1184`: `telescopes::validate` `:1362`, `callable_carriers::compatible_contracts` `:1207`, `append` `:1517`.
- `validate_resolved_semantics_in` `:1611`; `lower_symbol_resolved_trees` `:1622`; `TypingCohort` `:1663` (counts per family: `const_declarations, data_definitions, ... typed_machines, typed_expressions`, `completed_keys`); `type_resolved_cohort` `:1728`; `close_typed_cohort` `:1849`; `Lowerer` `:1999` (`typed_trees`, `source_trees`, `equality_scope`, `type_reference_exposure`); `finish` `:2134`; `rebuild_fresh_typed_trees` `:2143`.

**Inputs/outputs.** Input `&SymbolResolvedTrees` (+ retained `SeededTypingBase` for continuations). Output `TypedTrees`.

**How it decides.**
- Cohort frontier: each declaration family is processed with `.skip(cohort.<family>)` (`:1733-1821`); machines below `cohort.machines` are skipped unless `cohort.completes(symbol)` (`:1789-1795`). `completes` is a binary search over `(arena_index, generation)` keys (`:1717-1721`, `:1724`).
- `close_typed_cohort` order (`:1853-1908`): `settle_satisfied_declarations_from`, `normalize_machine_progress_premises`, `retain_static_argument_contexts`, `instantiate_nominal_machine_parameter_contracts` + `prepare_applied_callable_signatures`, `normalize_const_argument_carriers`, `normalize_domain_constraints`, `normalize_membership_selections`, `intern_proof_membership_instances`, `normalize_qualification_casts`, `construction_destinations::prepare`, (conditional) `validate_computed_arguments`, `finalize_provider_default_calls`, `target_machine_families::append`, `normalize_terminal_transition_calls`, `bind_dynamic_call_targets`.
- Ranking default `pending_ranking_default` `:174-245`: the single subject must be a plain name that is a state parameter (or const/value machine type parameter); after peeling `Constrained`/`Reference` (≤128 steps `:221`): `Slice | FixedArray` => `RankingViewId::SLICE_LENGTH` (`:225-227`); builtin `U8|U16|U32|U64` => `NAT_DESCENDING` (`:228-240`); anything else None.
- Progress premises `authored_premises` `:308-346`: only `Requires` contracts; only `Membership` facts whose `domain_symbol` is a declared domain with classification `ProgressProfile` (`:322-329`); subject must be a one-segment name (matching a parameter by symbol or, failing that, by *name string* `:363-371`) or a field chain (`:348-396`).
- Seeded continuation is validated by structural prefix equality on every retained family (`:1149-1167`, `:889-1050`).

**Cases by shape.**
- `:1383-1396` target family consistency messages: "unimplemented: target-family member `...` requires {detail}" (`:1509-1514`) for generic property demands, bound conjunctions and refinements; `:1546` "seeded typing cannot append a body to a retained target-machine family".
- `:1883-1895` conditional skip of `validate_computed_arguments` (see below).

**Same job elsewhere.**
- Three entry paths each call `type_resolved_cohort` then `close_typed_cohort` and copy the same evidence-forwarding loop: `lower_seeded_extension` `:794-806`, `lower_body_completion` `:865-877`, and `lower_symbol_resolved_trees` `:1652-1656` / `finish` `:2135-2138`. The seeded two copy the loop verbatim.
- `append_authored_service_reach_rows` `:1929` vs `lower_authored_service_reach_row` `:1911` and the initial mapping `:1640-1644`.
- `symbol_key` duplicates: `:1724` vs `type_reference.rs:1854` (see section 5).
- `exact_top_level_data_symbol` `:1966` and `exact_field_symbol` `:1976` are used by `expression_lowering.rs` (`:2352, 2510, 2538, 2559, 2693`).

**Unused / one-caller.** `activate_body_completion` has 1 caller (`psi-compiler/src/source_compilation.rs:904`); `lower_extension` 1 (`source_compilation.rs:1161`); `append_deferred` 3 (`source_compilation.rs:823`, `syntax_probes.rs:150,241`); `lower_symbol_resolved_trees_to_seeded_base` 3 (`source_compilation.rs:851`, `syntax_probes.rs:153,245`); `lower_seeded_extension` free fn 2 uses (`lowerer.rs:564,684`) plus `lib.rs` re-export. `lower_const_initializer_evidence`, `Lowerer::finish` called once each. All from Grep content results.

**Stated limits.** `:1511` "unimplemented: target-family member `{}` requires {detail}" (printed text; no source comment). `:1627-1628` comment: "Type arguments are classified while expressions are lowered, so the initial pass needs the resolved symbols from the start."

**Fail-open risks.**
- `close_typed_cohort` `:1883-1895`: `validate_computed_arguments` is skipped when `cohort.generic_application_origins > 0 && == source origin count && cohort.operators == source.operators.len() && cohort.data_definitions == source.data_definitions.len()`. No comment. What gets through: a continuation that appends only machines/other declarations whose generic origins are unchanged is not re-validated; a computed-argument mismatch introduced in new machine bodies would be unchecked at this point. **open**.
- `authored_premises` `:322-324`: `domain_definition(membership.domain_symbol)` None => `continue` (premise dropped). Because `normalize_machine_progress_premises` (`:1854`) runs before `normalize_membership_selections` (`:1870`), a membership whose `domain_symbol` was still invalid is silently skipped. What gets through: a `requires x in ProgressProfileDomain` fact left unresolved becomes no premise. **open**.
- `subject_path` `:363-371`: name-string fallback if symbols differ. **open**.
- `inherited_guarantee` `:251-255`: `resolve_satisfied_declaration` None => `continue` (no inheritance). Conservative (no guarantee gained). **traced**.

---

## 8. `03/statement_lowering.rs` (1422 lines)

**Purpose.** Lowers state-body statements: calls (with Product query/Build selection rewrites), root bindings, transitions (guards, targets, patterns), inline assembly windows, assignments, locals with hoist-temporary type inference.

**Shape.**
- `statement::arguments` `:7`; `calls::lower_call_statement` `:66`, `lower_ordinary_call` `:142`; `hoist_temp_type::infer_hoist_temp_type` `:211` (helpers `declared_projection_type` `:369`, `static_conformance_requirement_return` `:489`, `instantiate_declared_call_result` `:551`, `infer_declared_type_arguments` `:616`, `substitute_call_type_arguments` `:709`, `collection_type_reference` `:752`, `member_chain_type` `:782`, `attached_field_type` `:849`, `parameter_type` `:871`, `local_type` `:883`); `root_bindings::lower` `:914`; `transitions::lower_transition_statement` `:966`; `assembly::lower` `:1130`; `lower_statement` `:1267`; `lower_statement_node` `:1282`.

**Inputs/outputs.** Resolved statements in, typed `StatementNode`s out. `assembly::lower` returns `Entry`, then per-block `Label`, requires facts, instructions, ensures facts, optional transfer (`:1130-1264`), and patches jump targets (`:1228-1250`).

**How it decides.**
- `lower_call_statement` `:66-139`: first `product_queries::operation(source, call.receiver_symbol, call.target_symbol, call.target)`, then `product_selections::request`, else ordinary call.
- Local type: when the resolved declared type is `Unit` and `type_is_inferred` and the role has an ordinary initialiser (`:1378-1390`), calls `infer_hoist_temp_type`; `None` falls back to lowering the declared (Unit) type (`:1391-1396`).
- `infer_hoist_temp_type` `:211-367` handles a fixed list: `Borrow` (recursive, `:219-232`); data layout query spelling => `u64` (`:235-243`); byte-sequence predicate call by name `ByteSequencePredicate::from_name` with one argument => `bool` (`:244-254`); `Call` via declared return, late-selected receiver method, `static_conformance_requirement_return`, or a machine type parameter's contract (`:255-348`); `Name|Member|Indexed` via `declared_projection_type`; `Binary` with boolean result => `bool`; everything else `None` (`:363-366`).
- Name-keyed lookups: `parameter_type` by `parameter.name == name` `:879`; `local_type` by symbol, or by name only when both symbols are absent (`:897-900`); `attached_field_type` by `data.name == data_name` `:857`; `collection_type_reference` only for `Member` chains or a one-segment `Name` (`:759-777`).
- Guard `When(expr)` with `closes_run && closed_boolean_guard(...) == Some(true)` becomes `Always` (`:1039-1045`).
- Statement `Call` named `code_layout` with invalid symbol and empty receiver is rewritten to an expression call (`:1318-1365`).
- Root binding paths joined with `"::"` into a `String` slot/implementation key (`:943-952`).

**Cases by shape.** `:160-164` installed-interrupt calls drop the first 2 machine arguments `[2..]` (the request guarantees ≥2 via `lower_request` pattern `[registrar, timer_slot, ..]`, `call_and_product_selections.rs:1640`); `:1170-1177` only blocks with `EnclosingTransition` exit pop a trailing transfer; `:1367-1370` `unreachable!("evidence packages are classified out before runtime statement lowering")`.

**Same job elsewhere.** `lower_call_statement` repeats the query/selection decision tree that `expression_lowering.rs:1268-1342` does for call expressions (product queries first, build selection second); `code_layout` statement form repeats `expression_lowering.rs:1360-1374,1013-1022`. Typed-name lookups: see section 5.

**Unused / one-caller.** `infer_hoist_temp_type` has 1 external caller (`statement_lowering.rs:1387`, plus recursion). `statement::assembly::lower` is called from `lower_statement` only. `lower_statement_expression` `:16` is a one-line forwarder to `lower_expression_handle`.

**Stated limits.** None as comments.

**Fail-open risks.**
- `infer_hoist_temp_type` returns `Ok(None)` for unhandled initialiser shapes (e.g. a literal, struct literal, indexed range, cast). The local keeps the declared type `Unit` (`:1391-1396`). What gets through: a `let x = <unhandled expression>` is typed as `Unit` here; whether stage 04 or 05 infers or rejects it was not traced. **open**.
- `transitions::lower_transition_guard` `:1039-1045`: `closed_boolean_guard == Some(true)` becomes `Always`. Only at `closes_run` and constant-true literal forms (`:1282-1302` of the other file). **traced**.

---

## 9. `03/type_reference.rs` (3051 lines)

**Purpose.** Lowers written type references (both the tree form `TypeReference` and the table form `TypeReferenceNode`), retains authored declaration selections for type names, expands domain aliases, normalises domain constraints, normalises closed const arguments into canonical named atoms, validates generated generic applications against their retained origins, and handles alias uses.

**Shape.**
- `domain_aliases::expand_domain_reference` `:18` (special path `"Carry::Portable"` `:30-49`; cycle check `:56-66`).
- `domain_constraints` `:92`: `normalize_domain_constraints` `:102`, `normalize_domain_constraints_for_type` `:123`, `normalize_constraint_span` `:147`, `spell_domain_range` `:311`, `with_domain_candidate_index` `:347`, `select_domain_candidates` `:359`, `prefer_local_domain` `:429`, `domain_exposed_to` `:464`.
- `alias_uses::retain_application` `:505`; `arithmetic_results::retain_arithmetic_result_type` `:564`; `computed_results::retain_computed_result_types` `:667`.
- `const_arguments` `:679`: `closed_applications::normalize` `:702` (with `#[cfg(test)] mod tests` `:979-1176`), `normalize_const_argument_carriers` `:1179`.
- `constraints` `:1234` (`lower_type_constraint_node_span_from_table` `:1243`, `lower_type_constraint_node_span_with_context` `:1284`, `classify_domain_constraint_subject` `:1367`).
- `direct::lower_type_reference_handle_with_context` `:1401`, `lower_fixed_array_type` `:1595`; `generic_origins` `:1729` (`validate_computed_arguments` `:1737`, `application` `:1912`, `region_application` `:1985`, `application_scanned` `:2010`, `Equality` `:2165`); `table::lower_type_reference_handle_from_table_with_context` `:2504`; top-level helpers `:2716-3051` (`retain_supplied_operand_owner`, `lower_type_reference_into_table`, `lower_return_type`, `retain_type_reference_recipe_selection`, `retain_type_reference_selection`, `lower_dynamic_trait_type_reference`, `lower_named_type_reference`, `retain_static_path_selection`, `retain_path_selection`, `retain_path_selection_occurrence`).

**Inputs/outputs.** Reads resolved type references and the symbol table; writes typed `TypeReferenceTable` rows, constraint rows, and authored declaration selection ledger entries. `closed_applications::normalize` rewrites const-argument nodes in place into `Named { symbol: invalid, name: <canonical spelling> }` (`:967-973`).

**How it decides.**
- Domain candidates: if the constraint has a source span, a top-level lookup by name+`SymbolKind::Domain`+span picks one (`:368-376`); else a per-`TypedTrees` index of qualified and relative spellings (`spell_domain_range` `:311-345`: full display path, the part after the first `::`, the leaf). Visibility via `source_reference_can_see_symbol` (`:378-382`); exposure via module import path string equality (`domain_exposed_to` `:464-486`); local preference `prefer_local_domain` (`:429-462`).
- `domain_constraints::normalize_constraint_span` `:180-183`: if candidates are not exactly one, the authored constraint is kept unchanged (`normalized.push(...)`; `continue`).
- Subject classification by spelling: `classify_domain_constraint_subject(name, argument_count)` `:1367-1387`: `CarryPermission::from_name(name)`, `ValueDomain::from_name(name)`, and `name == "OmegaLayout" && argument_count == 1`.
- Closed const args: only when the formal parameter is `TypeParameterKind::Const` (`:878`) and the argument is a `ConstExpression` that is closed (`:881-889`) or a `Named` atom with no symbol decoding to a `Record`/`Variant` (`:891-905`); declaration kinds considered: data, trait, conformance, type alias (`:817-854`), plus bound lists and trait requirements/refinements/machine conformances (`:733-815`). `parameters.len() != arguments.len()` => `continue` (`:855-857`); conformance/alias with duplicate headers => `continue` (`:832-834`, `:844-846`).
- Generic origin validation: instance identity is `symbol` plus equal source span (`instance_identity_matches` `:2145-2156`); `Equality` compares types structurally (`:2205-2338`), const expressions by replaying through the typed lowerer (`closed_materialized_value` `:2365-2412`), lifetimes by position remap (`:2414-2423`), constraints by re-resolving domains by name (`domain` `:2429-2467`, which matches `domain.name.rsplit("::").next() == Some(name)` for unqualified names).
- Fixed array length `lower_fixed_array_type` `:1595-1727`: literal, const parameter (evaluated integer or `ConstParameter` retained), expression (completed to literal only when materialised integer), or `ConstCall` (name from `source_text` of the span `:1697-1704`).

**Cases by shape.**
- `:1639-1647` const array length must be an evaluated integer constant (else error "requires an evaluated integer constant").
- `:1560-1568` alias-use application must be a `Generic` or `Named` head.
- `:2123-2128` `validate_origin`: application base must be a `Data` symbol equal to the shared instance's base.
- `:2112-2114` `generic_instance` must be a `Generic` type reference.

**Same job elsewhere.**
- `direct::lower_type_reference_handle_with_context` `:1401-1593` and `table::lower_type_reference_handle_from_table_with_context` `:2504-2713` are two parallel lowerers of the same ten shapes (`Reference`, `Constrained`, `FixedArray`, `Slice`, `Generic`, `ConstExpression`, `DynamicTrait`, `Named`, `AliasUse`, `Unit`), one for the tree form and one for the handle/table form; they differ in how children are fetched. Likewise `constraints::lower_type_constraint_node_span_from_table` `:1243` and `lower_type_constraint_node_span_with_context` `:1284`.
- `generic_origins::application` `:1912` (indexed) and `application_scanned` `:2010` (linear scan) are the same selection; the scan is the fallback when `index.malformed` (`:1918-1920`).
- Domain instance interning duplicated in three places (see section 3).
- `symbol_key` `:1854` duplicates `lowerer.rs:1724`.
- `retain_arithmetic_result_type` `:564` and `typed-trees` `find_arithmetic_result_type_reference` (used `call_and_product_selections.rs:2059`, suspected related, not read).

**Unused / one-caller.** `lower_type_reference_into_table` and `lower_return_type` are used widely (not counted). `retain_type_reference_recipe_selection` `:2805`: 1 external caller (`call_and_product_selections.rs:556`). `computed_results::retain_computed_result_types` `:667` is used at `:1591` and `:2711` (2 uses, grep count mode returned 4 combined for 4 names; not separately counted).

**Stated limits.** `:1368` is in a test... no. Real ones: `:831` error text in `call_and_product_selections.rs` (not here). In this file: `:1639-1647`, `:2162` ("the normalized `end + 1` form" is in file 10). None as comments.

**Fail-open risks.**
- `normalize_constraint_span` `:180-183`: unresolved/ambiguous domain constraint stays as authored (`subject: classified by spelling`, `symbol` as authored). **open** (later checks not traced).
- `closed_applications::normalize` `:855-857, :832-834, :844-846, :853`: `continue` for mismatched arity, duplicate/absent header, or unknown declaration kind, so a const argument of such an application is not closed/checked here. **open**; unit test at `:997-1175` covers unique headers and "wrong width" failure only.
- `.expect("typed alias declaration must have a resolved source")` `:248-250` (panic, not accept).
- `validate_computed_arguments` `:1741-1757` returns `Ok(())` when there are no origins or no materialisation sources anywhere in the program (global shortcut): **traced** (nothing to validate).
- `retain_type_reference_selection` `:2878-2883`: names that are not source-backed, have invalid symbols, or primitive names return `Ok(())` without retaining; intentional exemptions; **traced** as bypass for builtins.

---

## 10. `04/assembly_and_operators.rs` (2179 lines)

**Purpose.** Stage 04 pieces: (a) normalise inline-assembly control flow that uses scratch registers into a generated helper machine with versioned register locals, derived `requires`/`ensures` contracts and exit transitions (`assembly_control`); (b) bind selected operator uses (binary/indexed/match-equality) to calls of the implementing machine entry (`operators::call_binding`).

**Shape.**
- `assembly_control::normal_exit_contracts::{derived_contract` `:20`, `append` `:33}`.
- `assembly_control::normalization`: `Binding` `:158`, `name` `:165`, `root` `:177`, `declaration_type` `:190`, `successors` `:214`, `target_reads` `:251`, `check_register_flow` `:268`, `parameters` `:412`, `target` `:455`, `exit` `:513`, `has_scratch` `:606`, `normalize` `:612`, `carry_ranking_witness` `:764`, `normalize_graph` `:822`.
- `register_versions::{RegisterVersion` `:1240`, `destination` `:1247`, `statement` `:1267}`.
- `admit` `:1346`, `validate_primitive_meanings` `:1377`.
- `operators::call_binding::bind_selected_operator_calls` `:1433`, `bind_body_bearing_match_equality` `:1728`, `rewrite_match_equality` `:1765`, `retain_operator_call_application` `:1879`, `SelectedOperatorCall` `:1894`, `selected_call_target` `:1950`, `indexed_collection_argument` `:2099`, `range_operands` `:2120`.

**Inputs/outputs.** Operates on `&mut TypedTrees`. `normalize` rewrites state statement windows (`:757-760`) and pushes generated helper machines (`program.push_machine(helper)` `:1219`). `bind_selected_operator_calls` overwrites expression nodes in place with `Call` nodes (`:1713`) and returns the bound count. Callers (grep): `preparation.rs:1946` (`validate_primitive_meanings`), `:1976`, `:2014` (`bind_selected_operator_calls`), `:2019` (`normalize`), `:2043` (`admit`).

**How it decides.**
- `normalize` `:612-762`: merges per-graph `target_requirement` with `intersect`; conflict => error (`:622-629`); authored target name must be accepted (`:632-640`). Graphs without scratch are left as-is (`:671-675`). Graphs with scratch and no `ZeroFlag` exit are rewritten as one linear sequence of register versions (`:678-744`); an unconditional cycle => error "unconditional assembly cycle requires an explicit checked progress/control model" (`:715-720`). Graphs with a `ZeroFlag` exit go through `normalize_graph` (`:822`) which builds a helper machine named `format!("{}$assembly${}${body_start}${}", owner.name, authored_target|"portable", state.name)` (`:935-943`).
- `check_register_flow` `:268-410`: fixed-point bit-set dataflow over ≤ `u64::BITS` scratch slots (`:299-303`); reading a scratch register or the zero flag before definition => error (`:377-385`, `:402-406`); scratch registers identified by `root(...)` symbol equality.
- Captures (`normalize_graph` `:847-924`): every `Local | Parameter` name read in reachable blocks becomes a helper parameter, `Mutable` reference when any statement writes it, else `Shared` (`:885-893`, `:905-909`).
- `carry_ranking_witness` `:764-820`: matches ranking subject text to helper parameter by `symbols.name(source) == text || name.rsplit('#').next() == Some(text)` (`:791-794`); no match => `return` (nothing carried).
- `normal_exit_contracts::append` `:33-133`: only the last block, only if `Fallthrough | ZeroFlag`; derived contract only when the `ensures` fact observes a captured reference and does not "escape the frame" via `SavedSubject`, name with invalid head, or `Local | Parameter` not captured (`:86-119`).
- `bind_selected_operator_calls` `:1433`: early `Ok(0)` when there are no token bindings, no operators, no top-level-requirement machines and no spelled trait requirements (`:1437-1451`). Occurrences = recorded operator uses plus binary nodes found by walking executable value origins (`:1463-1497`). `selected_call_target` `:1950-2089`: operand types from `expression_type_reference_for_origin`; bound trait operator meaning (exactly one) else explicit-selected closed meaning (exactly one) else a resolved token-binding machine; >1 => error "operator has multiple explicitly selected implementation meanings" (`:2018-2022`, `:2054-2058`). Only `Binary | Indexed` expression positions bind (`:1536-1549`); otherwise error "...position whose body supply is not implemented...".
- Named uses (`:1585-1650`): receiver symbol kind must be `Data | Domain | Module | BuiltinType` or invalid (`:1608-1618`); the binding's operator path must equal `namespace::member` by name strings (`:1619-1626`).
- `range_operands` `:2120-2175`: omitted end becomes `collection.len` (member named `"len"`, `:2131-2135`); omitted start needs an integer start parameter (`:2141-2151`); inclusive end becomes `end + 1` (`:2166-2173`), requires integer end operand (`:2155-2165`).

**Cases by shape.** `:243-247` `EnclosingTransition` exits error "scratch assembly with an enclosing-machine exit requires a lexical continuation"; `:831-835` "scratch assembly requires a closed callable telescope before CFG normalization"; `:1368` "cyclic assembly label control ... this control domain is not yet implemented" (stated limit, `admit`); `:1261` scratch destination must be a plain register name.

**Same job elsewhere.**
- `validate_primitive_meanings` is called twice for the same program: at the start of `normalize` (`:613`) and at `preparation.rs:1946` (grep).
- Synthesising an operator call node: `synthesized_operator_call` `:1918` and the `TableCallExpression { ... }` literal appears again at `03/call_and_product_selections.rs:155-176`, `1825-1846` and `03/expression_lowering.rs:1451-1483`, `3010-3039` (same 20-field struct written out each time).
- Register flow bit-sets in `check_register_flow` (`:268`) vs `successors` used again in `normalize` `:736` and `exit` `:513`.

**Unused / one-caller.** `normal_exit_contracts::append` called once (`:1071`); `register_versions::destination` called at `:291` and `:1307`; `admit` once (`preparation.rs:2043`); `bind_selected_operator_calls` 2 call sites (`preparation.rs:1976,2014`). `HashSet` use (`std::collections::HashSet` `:1415`, `with_hasher(symbols::BuildSymbolHasher)` `:1470,1563`).

**Stated limits.** `:1368` "this control domain is not yet implemented"; `:2146` "write it explicitly as `0..`" (diagnostic text); `:1544` "a position whose body supply is not implemented".

**Fail-open risks.**
- `selected_call_target` `Ok(None)` => `continue` (`:1519-1520`): an operator use whose selected meaning is not a token binding stays a built-in `Binary`. Input: a resolved operator use with no token-binding machine and no bound/selected trait meaning. **traced** as the normal builtin case; whether any selected operator that should bind falls through here was not traced.
- `:1503-1512`: uses of a bodiless top-level-requirement operator with a selected float comparison are skipped (`continue`). No comment. **open**.
- `carry_ranking_witness` `:791-796`: name-text match failing returns without moving the witness; the owner keeps its `implementation_witness` and the helper has none. What gets through: loop with a ranking subject that cannot be mapped by name; **open**.
- `normal_exit_contracts::append` `:117-119` drops (does not derive) a helper contract when the fact is not provably frame-independent: conservative (less is assumed). **traced**.
- `check_register_flow` `:307`, `:311`, `:313` index `reachable[0]`/`next[0]` without an emptiness check of `blocks`; if a graph with scratch had zero blocks it would panic, not accept. **open (unsure)**.
- `range_operands` `:2166-2173`: inclusive range becomes `end + 1` with no overflow guard seen in this file for the maximum value of the integer type. **open (unsure; later stages not read)**.
- Panics: `.expect("scratch is a declaration name")` `:698`, `.expect("scratch carrier retained")` `:703`, `.expect("scratch declaration is a name")` `:851`, `unreachable!` `:1300`, `:1660`.

---

## Cross-file items

- **Name/spelling keys inside stage 03** (Grep for `as_str() == "..."` patterns, `03/src`): `"BuildBindingOperations"` (`call_and_product_selections.rs:93`), `"run"`, `"X86StaticInterruptScope"` (`:1619-1621`), `"Build"` (`:1910`), `"Access"` (`lib.rs:101`), `"Extent"` (`lib.rs:240`), `"CodeLayout"` + `core/code_layout.omg` (`expression_lowering.rs:483,490`), `"address_of"` (`:1025`), `"code_layout"` (`expression_lowering.rs:1360`, `statement_lowering.rs:1319`), `"Quotient"` (`:1719`), `"place_private"` (`:1824`), `"LayoutPlan"` (`:1835,1868`), `"LayoutStorage"` (`:2298`), `"::provider_defaults"` (`declarations.rs:1550`), `"value"`/`"_"` (`declarations.rs:1709`), `"_"` (`declarations.rs:2101,2109`), `"Equatable"`/`"equals"` (`equality_and_exhaustiveness.rs:334-335`), `"Carry::Portable"` (`type_reference.rs:30`), `"OmegaLayout"` (`type_reference.rs:1381`), `"layout.omg"`/`"extent.omg"` (`lib.rs:96,229`), `"<build-prelude>"` (`call_and_product_selections.rs:100`). Some are gated by Toolchain origin; many are not (quotient `lift`/`define`, `address_of`, `code_layout`, `provider_defaults`, `Equatable`, `OmegaLayout`).
- **Panics in stage 03**: Grep count of `.expect(|unreachable!|panic!|.unwrap()` over `03/src`: 17 occurrences in 6 files (`call_and_product_selections.rs` 4, `declarations.rs` 1, `type_reference.rs` 4 (includes the test module), `contracts_and_signatures.rs` 1, `statement_lowering.rs` 1, `expression_lowering.rs` 6).
- **Memo/lookup by linear scan**: `ExpressionTableLowerer.lowered_expressions` (Vec, `expression_lowering.rs:849`), `Capture.expressions/captures/active` Vecs (`call_and_product_selections.rs:501-509`), `seen_named: HashSet<String>` (`declarations.rs:2030`), `with_domain_candidate_index` uses a `TypedTrees`-level cache (`type_reference.rs:347-357`).

---

## Coverage

- Read in full (21,032 lines): `03/lib.rs` 1-347; `03/call_and_product_selections.rs` 1-2227; `03/contracts_and_signatures.rs` 1-972; `03/declarations.rs` 1-2269; `03/equality_and_exhaustiveness.rs` 1-2446; `03/expression_lowering.rs` 1-3905; `03/lowerer.rs` 1-2214; `03/statement_lowering.rs` 1-1422; `03/type_reference.rs` 1-3051; `04/assembly_and_operators.rs` 1-2179.
- Skimmed: none.
- Not read: none.
- Greps run (all over `omega-rust` unless noted): `access_borrow_factory_of|access_borrow_factory\(`; `lower_expression_handle_from_table_in_scope|lower_scoped_fact_expression|lower_expression_handle_from_table\b`; `table_type_base_name|byte_slice_text_view|byte_sequence_text_carrier|value_type_base_name`; `fn expressions_structurally_equal|fn symbol_key|fn peel...|fn closed_boolean_guard`; `type_resolved_cohort|close_typed_cohort`; `closed_boolean_guard`; `assembly_control::|bind_selected_operator_calls|register_versions::|...`; `declared_call_*`/`data_definition_*`/`exact_*` fn definitions; seeded-continuation method names; fact-lowering function names; `.expect(|unreachable!|panic!|.unwrap()` count within `03/src`; name-literal comparison pattern within `03/src`.

## Five things a maintainer most needs to know about this unit

1. Stage 03's entry is `lower_symbol_resolved_trees` (`lowerer.rs:1622`): validators (`:1611`), then per-family typing (`type_resolved_cohort` `:1728`), then `close_typed_cohort` (`:1849`), which is a fixed 15-step settlement pipeline whose order matters (e.g. progress premises at `:1854` run before membership selections at `:1870`). The same two functions serve the full pass and both seeded continuations, driven by `TypingCohort` frontiers (`.skip(n)` per family).
2. Many recognitions are by spelled name, not identity: `"Quotient"`, `"LayoutPlan"`, `"address_of"`, `"code_layout"`, `"::provider_defaults"`, `"Equatable"`, data type names in structural equality (`operand_data_type_name`), exhaustiveness expression equality (`Call` by target string, `Cast` ignoring type). Toolchain-origin gates exist for some (`lib.rs:88-98`, `expression_lowering.rs:1881-1892`, `call_and_product_selections.rs:1619-1628`) but not all.
3. Two parallel lowerers of type references (`type_reference.rs:1401` tree form and `:2504` table form) and two parallel constraint lowerers (`:1243`, `:1284`) implement the same ten shapes; likewise five different "peel to a named data" helpers and three "declared type of a name" helpers (sections 2, 5, 8).
4. Many validation branches silently `continue`/`Ok(None)` rather than error when they cannot decide (listed per file under fail-open risks), notably `check_dispatch_run` skipping runs with a continuation (`equality_and_exhaustiveness.rs:932`), domain-membership normalisation (`contracts_and_signatures.rs:154-198`), qualification casts with 0/many domain candidates (`call_and_product_selections.rs:2128`), const-argument normalisation (`type_reference.rs:832-857`), and `validate_computed_arguments` being skipped for some continuations (`lowerer.rs:1883-1895`).
5. File 10 (stage 04) turns scratch-register inline assembly with a zero-flag branch into a generated helper machine named `"{owner}$assembly${target}${body_start}${state}"`, with register-versioned locals; it only handles acyclic graphs (`admit` `:1368`, cycle check `:715`) and `EnclosingTransition` exits are rejected for scratch graphs (`:243`). The same file's operator binder rewrites `Binary`/`Indexed` expression nodes in place into synthesized calls and rewrites body-bearing match equality into nested matches (`:1765`).

## What you are unsure about

- Whether `symbols.get(invalid_handle)` panics (affects `expression_lowering.rs:1404-1409`); I did not read `symbols`.
- Whether later stages (04, 05) reject: `let x = <unhandled>` typed `Unit` (`statement_lowering.rs:1391-1396`), data `==` with no derivable operand type name (`expression_lowering.rs:2918-2921`), unresolved membership domains (`contracts_and_signatures.rs:193`), unresolved domain constraints (`type_reference.rs:180`), and exhaustiveness runs with continuations. I only traced within the unit.
- Whether a user-authored machine can be named with suffix `::provider_defaults` and what origin gates exist (`declarations.rs:1550`).
- Whether `bound_trait_operator_meanings` / `selected_trait_operator_meanings` (not read, `typed-trees`) can return the same meaning twice, and whether `end + 1` for inclusive ranges is overflow-guarded downstream (`04/assembly_and_operators.rs:2166`).
- The exact outside-caller counts for several `pub(crate)` helpers: I counted only where stated; where I wrote "used widely" I did not count.
- `symbol_key` finding: Grep `fn symbol_key` listed 10 definitions; I did not read the other 8 to confirm they are identical.
