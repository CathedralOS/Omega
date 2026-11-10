# Unit 70 notes

Files covered (all under `omega-rust/psi/compilation/pipeline/06_checked-trees-to-lowered-psi/src/`, checkout `...\.agents\worktree\worker-23`, main at 457adf4a11f):

1. `scalar_lowering.rs` lines 1-3065 (3065)
2. `shared.rs` lines 1-2371 (2371)
3. `unit.rs` lines 1-3822 (3822)
4. `unit_arguments.rs` lines 1-7017 (7017)

Total 16,275 lines, every line read. All four files are one flat file each that contains many inline `mod` blocks (not separate files). `unit_arguments.rs` re-exports `crate::unit::*` (unit_arguments.rs:6) and extends `attached_unit` (unit_arguments.rs:8-9), so the "attached_unit" module is split across `attached_unit.rs` and `unit_arguments.rs` (read: unit.rs:7, unit_arguments.rs:8-9).

Paths below are short names; every file is in the directory above.

---

## 1. scalar_lowering.rs

### Purpose
Shared helpers for lowering "scalar" Unit bodies (bodies whose result is one scalar) from checked trees to Lowered Psi: the intermediate branch-state graph, how a call to a scalar callee is lowered, contract/proof-term lowering, and emission of "effects" attached to branch states. Header comment says: branch_states, call_closure, contracts, effects (scalar_lowering.rs:1-5). The file also re-exports a large prelude (`pub(crate) use ...`, 7-31).

### Shape
- `branch_states::call_lowering::lower_scalar_call` (55): validates a call to a scalar callee (arg count, types, entry state, result type, structural custody, erased value/proof args, crash-refinement row, contract identity) and returns a `LoweredDirectCallBinding` (230-243).
- `branch_states::guards::is_computed` (261), `guards::evaluate` (274): builds a `Conditional` terminator; when the guard is a computed root it pushes an extra state and returns a `Jump` (326-352).
- `branch_states::prepared`: `LoweredScalarBranchTerminator` (374) with variants CompleteReturn, Qualify, Capture, Jump, Conditional, CaseDispatch, CaseDispatchSplit; `CaseDispatchArm` (461); `LoweredCrashExit` (471); `LoweredScalarBranchState` (478: structural_parameters, parameter_types, erased_formal_types, erased_proof_formals, bindings, structural_effects, terminator); `scalar_state_emission_order` (591, iterative DFS, entry first then positions 0..n, post-order reversed per component); `LoweredScalarEffect` (637, 17 variants incl. ObserveStructuralValue, CompleteSnapshots, CaptureStructuralPlace, CopyStructuralValue, ProduceStructuralValue, CaptureDispatchSubject, BindCompletedValueAlias, EstablishElementView, EstablishScalarCase, EstablishByteSequenceLiteral, ViewSubslice); `CompletedSnapshot` (631).
- `branch_states::structural_values::ValueRequest` (714) with `prepare` (721), `complete_snapshot` (749), `snapshot_completion` (768), `finish`/`finish_with_alias` (801/815); `selected_copies` (850).
- `call_closure::callee::CheckedScalarCallee` (901; wraps `UnitBody`): `find` (904, rejects callees with structural params or entry claims), `find_for_unit_call` (915; looks in `terminal_unit_effects.machines`, `terminal_partial_affine_unit_cleanups.machines`, `composed_machines`; rejects duplicates, 938-943).
- `call_closure::embedded::discovery::checked_scalar_call_closure_with_structural_roots` (1041): worklist closure of scalar callees reachable from roots. `EmbeddedScalarCalls` (1135) is a bag of catalogs; `emission_context` (1215) turns it into a `CallEmissionContext`.
- `contracts`: `identity_formations` (1288), `indexed_theorems` (1344), `namespace::validate` (1426), `proof_scope::bind_invocation_proof_references` (1634), `ContractStructuralResult`/`ContractViewNamespace` (1709/1730), `clauses` (1748), `projected_guarantees` (1783), `canonicalize_requires` (1794), `erased_formal_declarations` (1802), `erased_proof_formal_declarations` (1818), `LoweredProofTerm`/`LoweredMathematicalArgument` (1837-1882), `checked_proof_term` (1884), `lowered_proof_term(s)` (2072/2175), `proposition` (2187), `erased_actual_term` (2206), `ScalarContractTerms` (2221, impl of `PredicateTerms`), `strict_result_bound` (2469), `mathematical_carriers` (2497).
- `effects::emit` (2557): one big match over `LoweredScalarEffect`, mutating operation buffer, blocks, evaluation.
- `scalar_carriers` (3063).

### Inputs and outputs
Reads `CheckedTrees` facts (`checked.facts.flow`, `.values`, `.proof`, `.contract_plans`), `PreparedScalarQualifications`, `TerminalIdentityTableBuilder`. Produces `Lowered*` structures, `terminal_psi::ProofTerm`, `Proposition`, `TerminalMathematicalCarrier`, `TerminalIndexedTheorem`, `TerminalIdentityFormations`; `effects::emit` appends `Operation`s and `Block`s through `OperationBuffer`.

### How it decides
- Callee lookup is keyed on `SymbolHandle` identity of the machine (905-972); "duplicate checked body ownership" is a refusal (942).
- Call lowering compares callee entry state to `target_state` by handle (101), types by `QualifiedScalarType` equality (104, 113), contract identity by `report_fingerprint` and `commitment` (211-214).
- Proof-term lowering maps formals by position in the caller's roster found via `parameter_symbol ==` (1949, 2059). Construction/field/case identities are emitted as `checked.symbols.display_path(symbol, "::")` strings (1995, 2000, 2040) - source spelling used as the identity on the wire.
- `mathematical_carriers` dedups by interned `type_identity` and compares `carrier_key` bytes (2524-2531).
- `ContractStructuralResult` namespace check keys on structural parameter `position` (1447-1470).
- `strict_result_bound` rewrites `<` to `<=` with +-1 only when one side is an integer literal term (2469-2495).
- `scalar_state_emission_order` is keyed on state position/target index (591-628).

### Cases by shape
- `contracts::clauses` (1756-1779): only `Predicate`, `Boolean`, `Integer` closed values accepted; `FloatRange` -> "scalar floating entry range is not a proposition clause" (1764); `SourceCheckedGuarantee` and `None` -> unsupported (1767-1772).
- `namespace::inspect` (1487-1538) / `scalar` (1540-1598): explicit allowlist of checked expression forms; `RepresentationView`, `Local`, `StorageRead`, `IeeeFloatComparison`, `StructuralIdentity`, `ByteSequenceEqual` refused. Comment text: "closed scalar contract predicate requires a scalar namespace" (1534).
- `lower_scalar_call` (87-98): structural args empty -> `find`, else `find_for_unit_call`; "computed scalar call requires exact structural custody".
- Dispatch split (the control shapes live in unit_arguments.rs, see below).
- `effects::emit` `CaseMembership`/`StructuralField` only for `ObserveStructuralValue` (2597-2657): other node kinds -> "retained value observation has no structural observation".
- `checked_scalar_call_closure...` accepts only `Eligible | FreeUnitEffect` signatures (1067-1076).

### Same job elsewhere
- Three-plus lowerers of contract predicates to `Proposition` through the same `PredicateTerms` trait: `ScalarContractTerms` (scalar_lowering.rs:2227), `RuntimeRequirementTerms` (unit.rs:1368), `FieldSubject` (shared.rs:1534), and `RouteTerms` in proofs.rs:3661 (grep `PredicateTerms for` across omega-rust: exactly 4 impls). Each re-implements `structural_identity`, `case_membership`, `scalar`, `boolean` with different leaf handling.
- Three namespace validators over `CheckedBooleanExpression`: `contracts::namespace::inspect` (scalar_lowering.rs:1487), `runtime_requirements::validate_namespace` (unit.rs:1162), `crash_interface::contract_scoped` (unit.rs:159, returns bool), and `validate_namespace` in proofs.rs:904 (grepped, not read). They differ: `ImplementationSelection` is accepted by `contracts::namespace::scalar` (1574) but rejected by `contract_scoped_scalar` (unit.rs:203).
- Two proof-term lowerings in two steps: `checked_proof_term` (1884) then `lowered_proof_term` (2072); a third `Formal -> InvocationFormal` rewrite in `proof_scope` (1683).
- `identity_formations` and `indexed_theorems` both encode `MathematicalCertificate` with `encode_mathematical_certificate` (1321, 1392) and `checked_proof_term` does it again (1909, 1976): same job three times with different error strings.

### Unused or one-caller code (greps in omega-rust, *.rs)
- `projected_guarantees` (1783): 1 external call site (attached_unit.rs:10206).
- `erased_actual_term` (2206): 1 call site (attached_unit.rs:13050).
- `lowered_proof_term` singular: 2 outside uses (attached_unit.rs:13032, 13187) + unit_composed_control.rs:9706 + 3 inside the file.
- `erased_formal_declarations` (1802): 2 uses (unit_composed_control.rs:1598 import, 10522 call).
- `mathematical_carriers`, `identity_formations`, `indexed_theorems`: 1 call site each, attached_unit.rs:17217-17222.
- `ValueRequest` (714): ~15 uses, all in scalar_computations.rs, unit_structural_values.rs:3522, unit_composed_control.rs:2563. `selected_copies`: scalar_computations.rs:5780, unit_composed_control.rs:3472.
- `LoweredScalarBranchTerminator::CompleteReturn/Qualify/Capture`: constructed in scalar_computations.rs (313, 5174, 5808, 7131), consumed once at unit_arguments.rs:3687-3760.
- `EmbeddedScalarCalls | checked_scalar_call_closure_with_structural_roots | CheckedScalarCallee | lower_scalar_call`: 62 matches across 13 files (count mode, grep over omega-rust *.rs), 16 in scalar_lowering.rs itself.
- `LoweredCrashExit` (471): not grepped; no use seen inside this unit (suspected unused; not confirmed).

### Stated limits
- "scalar callee requires structural call custody" (909); "scalar floating entry range is not a proposition clause" (1764); "source-checked guarantee has no runtime requirement projection" (1769); "nonintegerliteral carrier" etc. at FieldSubject (shared.rs).

### Fail-open risks
- `contracts::clauses` 1761-1763: `ClosedScalarContractValue::Boolean(_) | Integer(_)` become `Proposition::Truth` and the value is dropped. A closed Boolean `false` clause would be lowered as no requirement. No comment at the site. Status: open (I did not trace what produces `Boolean(_)` in clauses).
- `projected_guarantees` 1783-1792: filters out `None` and `SourceCheckedGuarantee` guarantees silently, so such `ensures` entries are not projected into the terminal contract. Status: open (name suggests intent: source-checked ones are checked elsewhere; no comment).
- `erased_proof_formal_declarations` 1827-1831 and `lowered_proof_term` Construction/Array/Projection (2129-2167): `type_identity_text(..).unwrap_or_default()` turns a missing identity into an empty string that is interned/emitted, while the sibling `Mathematical` arms (2083) refuse. A missing identity would yield an empty-string type identity. Status: open.
- `mathematical_carriers` 2506-2508: carriers with domains or level_arity != 0 are `continue`d (not published, no error). Status: open.
- `effects::emit` `CompleteSnapshots` 2815-2818: a produced value whose binding is not `fresh_storage` is skipped (`continue`), so no discard is emitted for it. Status: open (traced only locally).
- `ValueRequest::snapshot_completion` 793: returns `Ok(None)` (no snapshot completion) when `captured` or not intrinsic copy. Status: open.
- `guards::evaluate`, `lower_scalar_call`, `namespace::*`: refusals, none seen fail-open.

---

## 2. shared.rs

### Purpose
Cross-cutting vocabulary for the whole stage: `LoweringError`, id constructors and dense id allocation, call-site lookup, declaration lookup, the structural-type lowering helpers, the direct-call operation constructor, producer-result wrappers, checked-fact accessors, and a test module of "source guards" that grep the crate's own source (shared.rs:2081-2371). Header comment lists these (1-6).

### Shape (inline mods)
- `activation_storage` (8): `identity(bound)` = `format!("compiler(activation-byte-backing:{bound})")` (18-20), `backing_type` (22), `prepare_operation` (44: adds a `StructuralTypeDeclaration` for a `ByteSequence(BoundedOwned{capacity})` backing carrier when missing, new id = max id + 1, 56-77; recurses into `structural_value_evaluation` dependencies, 81-87).
- `call_operands` (91): one re-export `terminal_psi::call_structural_arguments as structural_arguments`.
- `call_sites` (95): `AuthoredCall` (104), `locate` (111), `ModuleCallTarget`/`selected_module_target` (164/169), `checked_invocation` (178), `checked_source_call` (230), `source_site` (242), private `checked_call` (275), `planned_result` (308), `discards_result` (323), `initializes_immutable_local` (331).
- `checked_facts` (339): `flow_facts`, `value_facts` (two one-line accessors).
- `content_classes` (351) and `reference_custody` (1463): thin readers of `checked.facts.content_classes` / `.reference_custody`.
- `declarations` (391): `state_declaration` (398), `TargetSignature` (421) with `authored_parameter_position` (434), `target_signature` (449), plus a `#[cfg(test)]` module comparing `state_declaration` to an `original_scan` (531-741).
- `direct_calls` (743): `DirectCall`, `ModuleInvocationArguments`, `emit_direct_call` (806), `DirectCallResult`, `into_operation_kind` (846), choosing among `CallUnit`, `CallStructuralScalar`, `CallStructural`, `CallStructuralWithScalarArguments`.
- `lowering_error` (916): `LoweringError` enum (976-1243; I count ~100 variants including many `Content*` unit variants), helper constructors, `diagnostics::debug_display!`, `unsupported` (1320), `invalid_suspension` (1324), `unit_machine_plan_error` (1328).
- `producer_result` (1351): `LoweredSelectedMachine`, `SourceMappedLowered::new` (1402) which checks that source owners are 1:1 with emitted machines and boundary machines (1410-1453).
- `structural_types` (1490): `scalar_refinements::lower` (1517) with `FieldSubject`; `lower_inline_carrier` (1623); `terminal_structural_field_type` (1643); `terminal_byte_sequence_carrier` (1653); `lower_field_type` (1674); `lower_mixed_fields` (1724); `lower_mixed_cases` (1752); `unique_declaration` (1783).
- `terminal_identities` (1799): constants `BYTE_VIEW_ELEMENT_TYPE_IDENTITY = "named(name(u8))"` (1810) and `TERMINAL_UNIT_CALL_OBLIGATION_BASE = 1<<63` (1812); `id_constructor!` macro for 15 id types (1824-1846); lookup functions (`lookup_type_id` 1879, `lookup_domain_id` 1934, `lookup_domain_id_for_carrier` 1950, `lookup_service_id` 1972, `lookup_machine_id` 1996, `lookup_claim_id` 2017); `allocate_dense` (2037); `mint`/`mint_with` (2045/2053); `fresh_field_snapshot` (2065).
- `source_guards` tests (2081): grep the crate's `src/` for forbidden substrings and for count-exactly-one constructors.

### Inputs and outputs
Pure helpers over `CheckedTrees`, `TerminalIdentityTableBuilder`, `&[StructuralTypeDeclaration]`. No global state; `prepare_operation` mutates the `types` Vec.

### How it decides
- `state_declaration` (398-419): looks up by `SymbolHandle`; errors if zero or more than one declared state matches.
- `target_signature` (449-529): tries in order: dynamic slot by symbol, machine state by symbol (first state of a machine if `source_target` is a machine), boundary operator by `symbol` (487-498), then a trait requirement via the caller's callable-parameter signature (499-529). `ParameterCoordinates::ServiceOperands` changes positions (441-443).
- `call_sites::locate` (111-162): matches `statement_index` and `call_ordinal` (293-295); primitive vs structural classification by `program.primitive_type_reference(parameter.type_reference)` (139); refuses `const` primitives and non-bool/non-fixed-integer `mut` primitives (148-153).
- `source_site` keyed on `FlowCallSite` variant (249-272).
- Domain/carrier binding keyed on `semantic_domain` and, when needed, `carrier` (1934-1970).
- `lower_mixed_fields/cases` reject duplicate or invalid identities via a `BTreeSet` keyed on `field.identity` (1731-1737, 1760-1766).
- Source spellings: `identities.intern(&field.identity.spelling())` (1744, 1770); activation backing identity string (18-20); `BYTE_VIEW_ELEMENT_TYPE_IDENTITY` string (1810, used attached_unit.rs:3567 via `type_identity_by_text`).

### Cases by shape
- `DirectCall::into_operation_kind` (861-911): `Structural{Linear}` with no scalar/erased/proof args -> `CallStructural`; every other structural -> `CallStructuralWithScalarArguments`; fixed list of four operation kinds.
- `lower_field_type` (1680-1721): exactly nine `CheckedUnitStructuralFieldType` variants (match is exhaustive).
- `scalar_refinements::FieldSubject` (1530-1620): only `Parameter{position:0}` of the same carrier, integer/float literals, Boolean constants/not/equal; else "scalar field predicate names a value outside its formal subject" (1581).
- `terminal_structural_field_type` (1643-1651): `F32`/`F64` special-cased, all others `terminal_scalar_type`.
- `source_guards::calls_have_one_emitter_and_one_argument_constructor` (2321-2370) lists six files by name (`attached_unit.rs`, `unit_operation_frame.rs`, `unit_arguments.rs`, `operation_emission.rs`, `scalar_lowering.rs`, `scalar_computations.rs`; 2323-2330); other stage files are not checked for free `StructuralArgument {` mints.

### Same job elsewhere
- `lookup_machine_id`, `lookup_type_id`, `lookup_service_id`, `lookup_claim_id`, `lookup_domain_id` are all linear `find_map` over `&[(key, id)]` slices (1879-2026). Same job: unit.rs:478 builds the `(TypeIdentityId, StructuralTypeId)` table again by string text (`type_identity_by_text(module.identities.text(..))`), and unit.rs:468-472 builds `(source, machine.id)` owners again.
- `unique_declaration` (1783) vs `structural_declaration_by_identity` (1890) vs `terminal_psi::structural_type_by_identity` (activation_storage 28): three type-lookup helpers with different error handling.
- `lower_mixed_fields/cases` called from unit.rs re-export (unit.rs:71-73) and attached_unit.rs:3753/3846 (inline duplicated `inline_carrier: lower_inline_carrier(...)`).
- `SourceMappedLowered::new` (1402) verifies 1:1 machine/source mapping, an invariant also checked by `lower_dynamic_dispatch_catalog` (unit.rs:465-467, length equality only).

### Unused or one-caller code (greps, omega-rust *.rs)
- `call_operands::structural_arguments`: 2 uses (emission_cleanup.rs:571, attached_unit.rs:10388).
- `primitive_scalar_identity` (1858): called only from `byte_view_element_identity` (1876) in shared.rs; `byte_view_element_identity`: 2 call sites (attached_unit.rs:1973, 16078).
- `unique_declaration`: ~14 call sites (emission.rs, expression_bindings.rs).
- `lower_inline_carrier`: 2 outside (attached_unit.rs:3753, 3846) + 1 inside (1741).
- `terminal_structural_field_type`: used only inside shared.rs (1697).
- `fresh_field_snapshot`: 3 uses (unit_composed_control.rs:12118/12283/12318, emission.rs:10708).
- `hex_text`: 6 uses in retention.rs. `dense_identity`: many uses in attached_unit.rs/retention.rs/proofs.rs. `lower_service_reach_sorted`: 3 uses in attached_unit.rs. `invalid_suspension`: 10 uses in retention.rs. `LoweringError::plan_gap`: 1 use (psi-compiler/terminal-production/src/lib.rs:540).
- `lookup_domain_id` (1934): exactly 1 use outside definition (unit.rs:739) plus the unit.rs re-export (unit.rs:76).
- `mint_with`: used in operation_emission.rs (5+ places). `mint` used operation_emission.rs, emission.rs.
- `TERMINAL_UNIT_CALL_OBLIGATION_BASE`: 1 real use (attached_unit.rs:17002).
- `call_sites::discards_result`/`initializes_immutable_local`: 1 use each (unit_operation_frame.rs:1226, 1212). `selected_module_target`: 2 uses. `checked_invocation`: 1 outside use (attached_unit.rs:12949).
- `ContentClass` accessors `plain_owned_with_numeric_constraints`, `plain_owned_with_represented_constraints`, `bounded_byte_capacity`, `stored_value_contents` and `reference_custody::*`: not individually grepped (the module-path count shows ~95 module-path references in 33 files across several crates, not usable per function).

### Stated limits
- "Bounded context ... retains no declaration graph or literal bytes and grants no alternate address path" (doc on `ArgumentProjectionFailure`, 951-952).
- Source guard comments are the test names (`lowering_refusals_do_not_revalidate_the_plan`, 2226): refusal strings must not contain ten phrasings such as "disagrees with its checked" (2227-2238).

### Fail-open risks
- `call_sites::planned_result` 308-321: a missing call row returns `FlowCallResult::default()` which is `Operand` (grep: flow.rs:309-311 `#[default] Operand`). `discards_result` and `initializes_immutable_local` then both return false, so a call with no checked row is treated as an ordinary operand call (not discarded, not a local initializer). Status: open (callers unit_operation_frame.rs:1212/1226 not traced).
- `lowering_error::unit_machine_plan_error` and the `lookup_*` helpers fail closed (error). `lower_service_reach_sorted` (1983-1994) sorts and dedups silently: duplicates are collapsed. Status: stated by name only.
- `fresh_field_snapshot` 2065-2079: if no qualification row matches `field`, uses the given plain `scalar_type` with empty qualifications (no error). Status: open.
- `activation_storage::prepare_operation` 55-79: creates a missing backing type on demand, then `backing_type` verifies its shape (31-40). Fail-closed on conflict.
- `mint` (2045-2051) panics via `expect` on overflow; `id_constructor!` panics on zero (1827). Not accept paths.
- Otherwise none seen.

---

## 3. unit.rs

### Purpose
Hub module for lowering "Unit" machine plans: re-exports the prelude for the stage (7-81), dispatches a `CheckedUnitPlan` to its lowerer (`lower_unit_plan_machine`, 3777), and contains four substantive inline modules: crash interface, dynamic dispatch tables, runtime requirements, and nominal/partial affine cleanup (`unit_cleanup`). Header (1-5) lists them.

### Shape
- `crash_interface::effective_crash_routes` (94): if plan crash interface is `PublishedCeiling` return the published routes (101-103); otherwise compute buckets per cause from checked sites, possible trap, checked calls, and checked operators (104-123) via `contribute` (126).
- `crash_interface::contract_scoped` (159) / `contract_scoped_scalar` (199): decide if a guard predicate mentions only contract-visible names.
- `dynamic_dispatch`: `DynamicDispatchIndex` (271: interfaces, tables, slots; `new` 278; `interface`, `table`, `interface_slots`, `slot`, `resolve_slot`, `resolve_tuple`, `family_roster`), `lower_dynamic_dispatch_catalog` (454), `same_checked_slot_contract` (655), `lower_slot` (700), `realizes_slot` (818), `lower_result_reference_regions` (890).
- `runtime_requirements`: `source::validate_runtime_memberships` (1009), `lower_structural_runtime_requirement` (1141), `validate_namespace` (1162) / `validate_float_namespace` (1193) / `validate_integer_namespace` (1214), `RuntimeRequirementTerms` (1289; impl `PredicateTerms` 1368), `substitute_runtime_requirement_scalar_values` (1454).
- `unit_cleanup` (1581): `contexts` (`SelectedCleanupSubjects` 1606, `SignatureContext` 1649, `prepare_signature` 1718, `field_clause` 1694, `validate_unconditional_contextual_requirements` 1672); `dispositions::outstanding_actions` (2001; tracks which planned cleanup actions have been superseded by later operations via `CurrentHomes`); `expression_owners::expression_cleanup_regions` (2114); `ordered::lower_ordered_nominal_affine_unit_cleanup_machine` (2190); `reference_retirement::lower_reference_completion_regions` (2221); partition types `CleanupCaseCondition`, `CleanupPartitionLeaf`, `CleanupPartitionNode` (Scope/Leaf/Cases), `CleanupPartitionArm`, `CleanupRegionAction`, `CleanupRegion` (2349-2441); `lower_cleanup_conditions` (2455); `select_cleanup_partition_sequence` (2553); `prepare_cleanup_partitions` (2679) / `prepare_cleanup_partition_node` (2813); `prepare_checked_cleanup_regions` (3000); `lower_checked_cleanup_regions` (3194); `retain_cleanup_descendants` (3243); `computation_cleanups` (3265); `selected_cleanups` (3302); `retained_call_cleanups` (3345); `lower_nominal_cleanup_action` (3375); `bind_consumed_claims` (3402); `materialize_nominal_cleanup_action` (3429); `PreparedNormalCleanupAction` (3469: Resolved, PartitionOnly, InvokeNominal); `emit_prepared_cleanup_regions` (3501); `validate_cleanup_partition` (3531); `materialize_prepared_cleanup_regions` (3681); `bind_nominal_cleanup_subjects` (3752).
- `lower_unit_plan_machine` (3777): three arms: `NominalAffineCleanup`, `PartialAffineCleanup`, `ComposedControl`; each wraps the result with `DebugPublication::Omit` (3793-3820).

### Inputs and outputs
Reads `checked.facts.contract_plans`, `.dynamic_conformances`, `.flow.terminal_unit_effects` (dynamic_slots, cleanup_control, machines, composed_machines), `.contextual_cleanup`, `.proof`. Produces `Vec<CrashRouteBucket>`, `TerminalDynamicDispatchCatalog` (written into `module.dynamic_dispatch`, 651), `Proposition`s, `Vec<CleanupRegion>`/`PreparedNormalCleanupAction`, `NominalAffineCleanup`, mutates `TerminalMachine.attachment`/`nominal_cleanup_subject` (3769-3771).

### How it decides
- Crash routes: grouped by `CrashCause` in a `BTreeMap` (104); a guard predicate that is `Constant(false)` is dropped, `Constant(true)` or any non-contract-scoped guard widens to `Truth` (137-150).
- Dynamic dispatch: interfaces identified by `TypeIdentityId` sorted/deduped (281-287); tables by (interface identity, implementation symbol arena_index) (288-296); slots by `(CallableIdentityId, Vec<String>)` family tuple (274); `resolve_tuple` compares `Vec<String>` tuples by equality (417); `lower_slot` requires scalar_parameters[0].source_position == 0 as the selector (714-719). `realizes_slot` (818) compares scalar types, structural positions/types/access/multiplicity, result and claim paths.
- Family tuple strings are parsed in unit_arguments.rs:6640 (see there).
- `realizes_slot`/table rows only validated when `selected_here` = the selection's machine is among `source_machines` (608-628); otherwise `rows.push(None)`.
- Runtime requirements: walk source contracts in order, matching `retained` expression clauses and `subjects` by `fact_ordinal` and `membership` equality (1057-1137).
- Cleanup: `outstanding_actions` decides a planned action is still outstanding by operation-order simulation using `same_effect`, `same_storage` (homes compared by `SourcePlan` equality, or result binding ordinal against local binding, 1977-1998). `select_cleanup_partition_sequence`, `prepare_cleanup_partitions`, `validate_cleanup_partition` are keyed on arena handle `(arena_index, generation)` (2558-2560, 2700, 3058).
- `prepare_checked_cleanup_regions` picks the "selected region" for an action as the region in `selected_regions` with the same `source` and a path prefix, longest path wins (3081-3087); a `Discard` with empty path and no conditions becomes `RetireHome` if the carrier multiplicity is `Unrestricted` else `DiscardRoot` (3132-3142).
- `bind_nominal_cleanup_subjects` keyed on `cleanup.cleanup_machine` symbol -> `MachineId`, and `cleanup.type_identity` -> structural type (3758-3771).

### Cases by shape
- `lower_unit_plan_machine`: three plan kinds only (3784).
- `validate_integer_namespace` (1214-1287): accepted integer binary kinds listed explicitly (1237-1247); `Addr`-typed operands refused (1248, 1259, 1265); comment string "runtime requirements admit only fixed integer parameters, members, and literals" (1284).
- `validate_float_namespace` (1193): only `F32`/`F64` parameters, fields, indexed reads, literals; "runtime IEEE requirement needs exact float entry parameters or literals" (1209).
- `substitute_runtime_requirement_scalar_values` (1518-1578): handles Truth/Falsehood, math comparisons, ordered comparisons, IEEE comparison, StructuralCaseMembership/StructuralIdentity, Implication, Conjunction/Disjunction; `_ =>` "unsupported proposition" (1575).
- `CleanupPartitionNode::Cases` requires a `Sum`/`Mixed` carrier (3611-3619), arms must equal the declared cases in order (3620-3629), `Selected` arm implies exactly one reachable case (3637-3644).
- `prepare_signature` (1718): handles only (a) a nominal dispatcher plan and (b) a hook; `nominal` and `hook` both present -> "nominal cleanup dispatcher is its own hook" (1757).
- `validate_unconditional_contextual_requirements` (1672): refuses any entry-requirement row for the machine whose antecedent is not `Constant(true)` - "guarded contextual cleanup requires an exact edge cleanup plan" (1688).

### Same job elsewhere
- `unit_cleanup::lower_checked_cleanup_regions` (3194) and `emit_prepared_cleanup_regions` (3501) both call `prepare_...`/`materialize_...` with two different closures for the nominal step (the first takes a caller closure, the second reads `calls.cleanup_receivers`, `calls.requirement_counts`, `calls.claims`). Same job, two call-closure surfaces.
- `expression_owners::expression_cleanup_regions` (2114) and `reference_retirement::lower_reference_completion_regions` (2221) each call `prepare_checked_cleanup_regions`/build regions with different storage closures.
- `selected_cleanups` (3302) and `computation_cleanups` (3265) and `retained_call_cleanups` (3345) are three enumerations of the nominal cleanup plans, differing by scope (all vs one machine vs call schedule).
- The normal-discard residual computation `Evaluation::normal_discard_regions` (unit_arguments.rs:594) does a similar "remaining partition after moved paths" job as `reference_retirement` (2285-2292) and `partial_affine_residuals`.
- `crash_interface::contract_scoped*` (159-251) vs `runtime_requirements::validate_*namespace` (1162-1287) vs scalar_lowering `contracts::namespace` - see scalar_lowering section.
- `dynamic_dispatch::lower_slot` result lowering calls `attached_unit::lower_boundary_result` (768), the same as boundary machine lowering (attached_unit.rs:360 uses `lower_result_reference_regions` too).

### Unused or one-caller code (greps)
- `effective_crash_routes`: 6 call sites outside the definition and re-exports (unit_composed_control.rs:210, scalar_lowering.rs:216, proofs.rs:4505, emission.rs:11558, attached_unit.rs:10827, 13127).
- `DynamicDispatchIndex`: ~12 uses (unit_composed_control.rs, operation_emission.rs, scalar_computations.rs:1245, attached_unit.rs). `lower_dynamic_dispatch_catalog`: 1 call (machine_lowering.rs:2035). `lower_result_reference_regions`: 1 outside call (attached_unit.rs:360).
- `lower_structural_runtime_requirement`: 2 calls (attached_unit.rs:6257, 6279). `validate_runtime_memberships`: 1 call (attached_unit.rs:6253). `substitute_runtime_requirement_scalar_values`: 1 outside call (attached_unit.rs:13319).
- `lower_checked_cleanup_regions`: 2 calls (unit_operation_frame.rs:4659, attached_unit.rs:6177). `lower_nominal_cleanup_action`: 2 (unit_operation_frame.rs:4710, attached_unit.rs:6198). `lower_reference_completion_regions`: 1 (attached_unit.rs:6032). `bind_nominal_cleanup_subjects`: 1 (attached_unit.rs:17131).
- `lower_nominal_affine_unit_cleanup_machine` (2519) is a pure forwarding wrapper around `lower_ordered_nominal_affine_unit_cleanup_machine` (2526); re-exported at unit.rs:81 and called from `lower_unit_plan_machine`.
- `PreparedNormalCleanupAction::discard_root` (3488): not grepped.

### Stated limits
- "A later completion can select only part of an earlier cleanup schedule. Detach that partial schedule explicitly; it cannot close the full carrier." (doc, 2551-2552).
- Error strings: "guarded contextual cleanup requires an exact edge cleanup plan" (1688), "nominal cleanup dispatcher is its own hook" (1758).

### Fail-open risks
- `effective_crash_routes` 98-100: no contract plan for the machine -> `Ok(Vec::new())`, i.e. no crash routes. A callee without a contract plan is treated as crash-free. Status: open (callers not traced).
- `contribute` 147-149: unknown/non-scoped guard -> `Truth` (widens). This is the safe direction (more crashes assumed). Status: traced locally, not fail-open.
- `lower_dynamic_dispatch_catalog` 625-628: for tables not `selected_here`, rows are `None` and no realization check runs. Status: open (no comment; the rest of the module presumably validates elsewhere, not traced).
- `validate_runtime_memberships` 1074-1080: `restates_structural_parameter_qualification` facts are `continue`d without matching a retained clause. Status: open.
- `substitute_runtime_requirement_scalar_values` 1560-1561: `StructuralCaseMembership`/`StructuralIdentity` returned unchanged with no scalar substitution; also dead `match scalar_type` (1487-1490) with identical empty arms. Status: open.
- `bind_nominal_cleanup_subjects` 3759-3761: `let Ok(target_id) = lookup_machine_id(..) else { continue }` silently skips a cleanup whose machine is not in the emitted closure; the subject is not bound. Status: open.
- `prepare_signature` 1800-1802: `Ok(None)` when neither nominal nor hook; fine by design (not a cleanup callable).
- `dispositions::outstanding_actions` 2061-2072: a `Store` into the same storage sets `retained = true` again (re-opens an action); other unmatched operations leave state (`_ => {}`, 2093). Heuristic, keyed on storage-home equality. Status: open.
- `lower_cleanup_conditions` 2469-2471: empty selection -> no conditions (unconditional cleanup). Intended.

---

## 4. unit_arguments.rs

### Purpose
The "evaluation" machinery for Unit bodies that call things: emits cleanup control flow (case tests, partitions, selected-storage bypass blocks), carries the per-body `Evaluation` state (structural homes, locals, claims, retired homes), turns scalar branch-state graphs into terminal blocks (`emit_state`), completes scalar computations and call arguments, and schedules the order of argument evaluation (`argument_schedule`). Header (1-4).

### Shape (all inside `attached_unit::argument_evaluation` unless noted)
- `cleanup_control` (54): `case_observations` (65-404: `CleanupEmission::case_subject`, `declared_single_case`, `capture_transfer_cases`, `observe_case`, `retained_case`, `preserves_case_observation`, `fresh_inline_home`, `inline_case_path`), `CleanupEmission` struct (406), `Evaluation::cleanup_emission/carrier_multiplicity/emitted_carrier/emitted_carriers/completed_carrier_inventory/descriptor_retirement/normal_discard_regions/capture_structural_arguments/emit_cleanup_regions/continue_cleanup_regions` (424-776), `CleanupEmission::{capture_arguments 779, emit 860, completed_partition_edge 931, partition_node 967, partition_cases 1108, select_storage 1358, proof_arguments 1402, continue_actions 1416, cleanup_scope 1446, cleanup_cases 1621, cleanup_jump 1793}`.
- `ScalarArgumentCaptures` (1837).
- `qualification_edge::emit` (1866).
- `record_captures`: `Evaluation::prepare_storage_before` (2015).
- `scalar_control`: `ScalarReturnCleanup` (2235), `completion_target`, `cleanup_target`, `result_targets`, `ScalarCaseDispatchArm` (2331), `push_arm_target`, `scalar_case_dispatch_terminator`, `Evaluation::scalar_case_dispatch` (2403), `case_dispatch_split` (2453), `scalar_control_result` (2543).
- `source_values`: `receiving_value` (2926), `retained_observation` (3007), `branch_guard` (3067), `guard_value` (3121), `source_value` (3185), `direct_expression_tests_whole_root_case` (3337).
- `state_emission::emit_state` (3394, ~870 lines): lowers one `LoweredScalarBranchState` to blocks, handling Boolean short-circuit bindings, dynamic family calls, other bindings, effects, then the terminator (CompleteReturn, Qualify, Capture, Jump, Conditional, CaseDispatch, CaseDispatchSplit); helpers `dispatch_arguments` (4265), `completion_roster` (4293).
- `straight_line::complete_straight_line` (4322).
- `Evaluation` struct (4513: ~40 fields), `StructuralBindingEnd`, `StructuralHomeEndings`, `RetiredStructuralBinding`, `StructuralSourceFrontier`, `SelectionCleanup`, `CapturedSelectionSource`, `StructuralValueOwner`; `Evaluation` methods for home/binding lifecycle (4652-5630), `arguments`/`argument_at`/`call_selector`/`arguments_slice` (5633-5982), `field_assignment_value` (6009), `single_completed_expansion`/`complete_expansion` (6147/6184), free fns `declarations` (6376), `validated_values` (6392), `BlockEmissionSpan` (6412), `emit_dynamic_family_dispatch` (6423), `emit_tuple_test`, `family_tuple_integer` (6636), `dispatch_successor_edge`.
- `argument_schedule` (6659): `Step` enum (6665: CaptureBefore, Ordinary, Begin, Argument, Subslice, Selector, Structural, Operation, Call, End), `build` (6692), `build_operations` (6705), `call_group_root` (6805), `append` (6840), `append_completions` (6993).

### Inputs and outputs
Reads checked plans (`CheckedUnitEffectOperationPlan`, `CheckedUnitStructuralArgumentSourcePlan`, `CheckedScalarComputationRoot`), `Evaluation` state, `OperationBuffer`. Writes: `Block`s pushed to `self.blocks`/`blocks`, `Operation`s into `OperationBuffer`, allocator counters (`next_value`, `next_block`, `next_edge`, `next_place`), `Evaluation` mutation, `ScalarQualificationCoercion`s. Output of `argument_schedule::build` is `Vec<Step>`.

### How it decides
- Cleanup case dispatch keyed on `CleanupCaseCondition{place, path, structural_type, case}` equality (1640-1643). Reuse of an earlier Boolean case test (`retained_case`, 240-293) is keyed on `(source place, path, structural_type, case)` and verified by scanning later operations with `preserves_case_observation` (295-350; allows unconditionally total scalar ops, stores into fresh inline homes with inline-only paths, read-only borrows) and by checking no cleanup action in existing block terminators retires the source (269-291).
- `select_storage` decides when a "bypass" block is needed: `selected_region != selected_scope` (1470-1472, 1025-1027).
- Home retirement is keyed on `PlaceId` and `PlaceRoot` (5446-5482, `retire_structural_sources`).
- `scalar_case_dispatch` (2403-2450): a guard is a case dispatch only if its lowered form is `StructuralCaseMembership` with empty path whose `source` has a sum in `structural_cases`.
- `scalar_control_result` `Guarded` arm (2690-2878): decides "one_subject" if every arm's dispatch has same `source` and same `cases`; otherwise takes trailing same-subject arms into a closing split (2729-2788).
- `branch_guard` (3067-3118): returns an expression only if it has a short-circuit or (`successor_reads_payloads` and tests whole-root case).
- `argument_schedule::append` (6840) orders by `authored.evaluation_order` and positions from `call_parameters::{scalar_positions, structural_positions}`; consumers of a result binding are found by `source_structural_result_binding_ordinal` (6745-6748, 6824).
- Dynamic family dispatch (6423): tests integer equality against constants parsed from tuple strings `"named(integer-const(N))"` (6640-6643) - the tuple identity is a source/identity spelling that is parsed with `strip_prefix`/`strip_suffix`/`parse::<i128>`.

### Cases by shape
- `emit_dynamic_family_dispatch` (6440-6446): requires `arity != 0`, `tuples.len() >= 2`, every tuple arity equal; "dynamic family roster does not cover the call's binders". Each binder must have an integer carrier (6612-6617).
- `qualification_edge::emit` (1877-1990): requires exactly the scalar successor arguments (no structural arguments, nonempty), every argument a plain parameter or local; only the last argument may change qualification and only by pure introduction or pure erasure on a fresh value; coercion refused if the introduced domain has a predicate or establishment routes (1970-1976).
- `scalar_control_result` terminators: `Return`, `Conditional`, `Guarded` only; `_ => "ordered scalar completion requires a returning tail"` (2880-2883).
- `complete_straight_line` (4322-4490): only chains of `Jump` terminators with no structural params/effects/erased formals/cleanup, bindings limited to `Expression`, `SelectedComparison`, `DirectCall` without short-circuit args (4372-4385); anything else -> `Ok(None)` (falls back to blocks).
- `emit_state` Conditional with short-circuit and erased operands: "erased call operands do not stage through guard decisions" (3894).
- `argument_schedule`: `CheckedStructuralInvocation::BorrowedInstruction` special-cased as `(vec![0], [], [0])` (6900-6902); other `Step::Structural` skipped for `ByteSequenceLiteral | ByteSequenceSubslice | ElementViewSubslice` sources (6973-6978).
- `call_selector` (5709-5768): runtime index selectors accepted only as `StructuralField` or `StorageRead` (or a computation); other index kinds -> "scheduled selector has no evaluated operand". `prepare_storage_before` (2125-2142): loan coordinate sources `StorageRead`, `Parameter`, `Local`, `StructuralField`; `_ =>` "loan coordinate has no declaration-time scalar source".
- `case_dispatch_split`: all cases armed or a fallback required (2512-2517).

### Same job elsewhere
- `cleanup_cases` (1621) and `partition_cases` (1108) both emit "test case, branch, run arm, join" block chains over a sum; they duplicate the Block construction code with `Terminator::Conditional` (1263-1287 vs 1729-1751).
- Block construction of `Block { id: *self.current, parameters: take, structural_parameters: take, erased_scalar_formals: take, erased_proof_formals: erased_proof_formal_declarations(..), operations: operations[start..].to_vec(), terminator }` repeats in `cleanup_jump` (1806-1826), `cleanup_cases` (1729), `partition_cases` (1263), `complete_expansion` (6233-6260), `emit_dynamic_family_dispatch` (6486), `emit_state` (4253).
- `proof_arguments` (1402) and `Evaluation::proof_formal_forwarding` (5623) and `completion_roster` (4293) all build `ProofTerm::Formal{position}` forwarding lists.
- Three nearly identical scalar completion funnels: `source_value` (3185), `retained_observation` (3007), `receiving_value` (2926), `guard_value` (3121), `field_assignment_value` (6009) all create a `scalar_computation_expansion`, call a root-specific constructor, `finish`, and `single_completed_expansion`.
- Terminator lowering for `CaseDispatch` (4005-4132) and `CaseDispatchSplit` (4133-4251) duplicate the selected/fallback block emission.
- `retain_selection_sources` (5023), `capture_selection_sources` (4653), and `value_storage` (4844) vs unit.rs `unit_cleanup` storage closures.
- `normal_discard_regions` (594) vs `lower_reference_completion_regions` in unit.rs:2221 (residual discard computation).
- Dispatch tuple tests duplicate unit.rs `DynamicDispatchIndex::resolve_tuple` tuple spellings (`Vec<String>`).

### Unused or one-caller code (greps)
- `ScalarArgumentCaptures`: constructed 4 times (unit_structural_values.rs:5941, unit_composed_control.rs:604, attached_unit.rs:10480 field, 10720); methods `record`/`values` not individually grepped.
- `validated_values`: 4 uses (unit_operation_frame.rs:802, operation_emission.rs:379/753, attached_unit.rs:13068).
- `completed_source_binding`: 1 use (unit_structural_values.rs:4300). `capture_selection_sources`: 1 (unit_structural_values.rs:5045). `cleanup_source_is_retired`: 3 (attached_unit.rs:5971, unit_operation_frame.rs:8086, one more at 5971 inside unit_arguments? no: 2 outside + definition). `retire_consumed_structural_arguments`: 2 outside (unit_structural_values.rs:1728, 2526) + 1 inside. `retire_structural_operation_operands`: 5 outside (emission_cleanup.rs:3240, unit_operation_frame.rs:1178, unit_structural_values.rs:1326, 5871 and 1). `capture_structural_arguments`: 3 (unit_composed_control.rs:9886, unit_operation_frame.rs:4216 and its definition). `normal_discard_regions`: 6+. `emit_cleanup_regions`: 5. `continue_cleanup_regions`: 9. `completed_carrier_inventory`: 1 outside use (4837, same file). `descriptor_retirement`: 6.
- `Step` variants `Ordinary`, `Operation`, `Call`, etc. are consumed outside this file; not grepped.
- `StructuralHomeEndings::contains` (4577): used at 4732 in-file; external use not grepped.

### Stated limits
- "Cleanup already selected by source ownership, before scalar arm expansion. Obligation identities belong to this exit..." (2232-2234).
- "A runtime window argument evaluates its start and end exactly once, inside the step that forms the window ... would re-run any call in them" (6922-6927).
- "The first discriminator dominates every arrival at this dispatch's join. Observations made inside an arm ... do not" (1712-1714); "The tree covers the exact declared alternatives. After all earlier tests failed, the last reachable alternative is exhaustive" (1294-1296).
- "Reuse the completed Boolean, never a tag load on the current payload." (236-239).
- Doc on `completed_value_binding`: "Absence means that producer has not completed; a retired or out-of-frame name is an error, never permission to produce it again." (4711-4715).
- Debug hook: `std::env::var_os("OMEGA_COMPILER_CHECK_SITES")` printed from `retire_structural_home` (5420), also used in 3 other stage files and omega-rust trace_flags.rs:21.

### Fail-open risks
- `completion_roster` 4293-4306: if the explicit proof actuals are empty and the callee roster is non-empty, it substitutes `Formal{position: proof_position}` for every roster entry (implicit forwarding) rather than refusing. A jump with no declared proof actuals gets the caller's own proofs forwarded. Status: open (callers `emit_state` 3754, 3802, 3969-3976 not traced to the producer of empty lists).
- `ScalarArgumentCaptures::values` 1849-1855: `filter_map` drops slots that are `None` or out of range instead of erroring. Status: open.
- `retained_case` 245-248, 257-268: returns `None` (re-observe) on any doubt: fail-closed.
- `capture_transfer_cases` 172-188: conditions whose path is not inline or whose place is not an owned argument prefix are `continue`d (no capture). Status: stated in doc comment 149-151 ("Other cleanup regions keep their ordinary selected scope").
- `emit_cleanup_regions` 744-753: if no region has partition leaf/conditions/selected region, `region.action.into_physical()` via `filter_map` drops `CheckedPartition` actions (which return `None`) silently. Status: open: a `CleanupRegionAction::CheckedPartition` with no partition leaf would be dropped; in practice such regions carry `partition_leaf` (unit.rs:3703), so traced as guarded by construction at unit.rs:3703-3716 but not confirmed for all producers.
- `cleanup_scope` 1532-1535: `region.action.clone().into_physical()` via `filter_map` likewise.
- `cleanup_cases` 1639: indexes `first.conditions[depth]` without bounds check (panics if shorter), but the caller guarantees `conditions.len() > depth` (1460, 1543). Panic path, not accept path.
- `retain_selection_sources` 5028-5057: compares by equality and refuses mismatches; fail-closed.
- `complete_straight_line` returns `Ok(None)` on mismatch: fall back to the general path.
- `Evaluation::completed_value_binding` 4730-4735: fail-closed.
- `record_owned_home` 5191-5207: if an existing owner has an invalid symbol and the new one valid, silently adopts; if both invalid, `Ok(())`. Status: open.
- `emit_dynamic_family_dispatch` 6569-6582: no tuple matches -> `Terminator::Crash{cause: Abort}`; fail-closed at runtime.

---

## Cross-file items

- Declared as shared and checked by tests: `source_guards` (shared.rs:2256-2286) asserts exactly one `CheckedStructuralControlSuccessorPlan {` construction and one `fn successor_edge(` across the compilation root (`../../..`), and (2321-2370) exactly one `DirectCall {` construction and one `fn structural_argument(` in this stage's `src`. Not run (read-only); counts not verified by me. grep: `DirectCall {` constructor sites live in `direct_calls::DirectCall::new` (shared.rs:773) and `into_operation_kind` destructure (847).
- Comment-says-canonical check: shared.rs header calls `LoweredSelectedMachine` ("producer_result") and `DirectCall` shared vocabulary (1-6). `LoweredSelectedMachine`/`SourceMappedLowered::new` users not grepped by me; `DirectCall::new` is the single constructor by test.
- Source-spelling keyed identities in this unit: `display_path(.., "::")` (scalar_lowering.rs:1995, 2000, 2040; unit.rs:647, 801), `format!("compiler(activation-byte-backing:{bound})")` (shared.rs:18-20), `"named(name(u8))"` (shared.rs:1810), `"named(integer-const(N))"` parse (unit_arguments.rs:6640-6643), `type_identity_by_text(module.identities.text(..).unwrap_or_default())` (unit.rs:476-482), `Vec<String>` family tuples (unit.rs:274, 417).

---

## Coverage
- Read in full: scalar_lowering.rs 1-3065 (3065), shared.rs 1-2371 (2371), unit.rs 1-3822 (3822), unit_arguments.rs 1-7017 (7017). Total 16,275 lines.
- Skimmed: none. Not read: none.
- Greps outside the unit (proofs.rs, attached_unit.rs, retention.rs, unit_*.rs, etc.) were limited to name searches; those sites were not read.

## Five things a maintainer most needs to know about this unit
1. Every file here is a flat file holding many inline `mod`s; `attached_unit` is split between `attached_unit.rs` and `unit_arguments.rs`, and `unit_arguments.rs` re-exports `crate::unit::*` (unit_arguments.rs:6-9). Names resolve through three layers of `pub(crate) use`.
2. Contract-predicate lowering exists in four `PredicateTerms` impls (scalar_lowering.rs:2227, unit.rs:1368, shared.rs:1534, proofs.rs:3661) and four hand-written namespace allowlists (scalar_lowering.rs:1487, unit.rs:159, unit.rs:1162, proofs.rs:904), which disagree on some forms (e.g. `ImplementationSelection`, scalar_lowering.rs:1574 vs unit.rs:203).
3. Cleanup control (unit_arguments.rs:54-1834 with unit.rs:2349-3750) is the largest decision surface: partition trees, selected-storage bypass blocks, and case-test reuse (`retained_case`) keyed on `(place, path, type, case)` plus an operation-prefix scan.
4. Several quiet drops rather than errors: `bind_nominal_cleanup_subjects` skips unmapped machines (unit.rs:3759-3761); `ScalarArgumentCaptures::values` drops empty slots (unit_arguments.rs:1849-1855); `contracts::clauses` maps closed Boolean/Integer values to `Truth` (scalar_lowering.rs:1761-1763); `unwrap_or_default()` on missing type identities (scalar_lowering.rs:1827-1831, 2129-2167).
5. Dynamic dispatch and family calls depend on string identities: tuples are `Vec<String>` compared by equality (unit.rs:274, 417) and decoded from `"named(integer-const(N))"` text at emission time (unit_arguments.rs:6640-6643); a single `OMEGA_COMPILER_CHECK_SITES` env read sits inside `retire_structural_home` (unit_arguments.rs:5420).

## What you are unsure about
- Whether `FlowCallResult::Operand` default (shared.rs:320 + flow.rs:310) is actually reachable from `discards_result`/`initializes_immutable_local` callers with a missing row; I did not trace unit_operation_frame.rs:1212-1230.
- Whether `ClosedScalarContractValue::Boolean(_)`/`Integer(_)` clauses can carry a falsifying value (scalar_lowering.rs:1761); I did not read the producers in checked-trees.
- Whether `LoweredCrashExit` (scalar_lowering.rs:471) and `PreparedNormalCleanupAction::discard_root` (unit.rs:3488) are used; I did not grep them.
- Exact `LoweringError` variant count (I estimated ~100 by eye; did not count with a command).
- Whether the `source_guards` tests (shared.rs:2081-2371) currently pass; I did not run anything.
- The behaviour of `dispositions::outstanding_actions` (unit.rs:2001-2102) in edge cases (Store re-opening an action) is read but not traced against callers.
