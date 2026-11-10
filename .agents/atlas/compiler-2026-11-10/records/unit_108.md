# Unit 108 notes

Files covered (14), all under `omega-rust/psi/foundation/`:
1. language-semantics/src/declaration_selection.rs (2903)
2. language-semantics/src/lib.rs (2524)
3. language-semantics/src/numeric_intrinsics.rs (775)
4. language-semantics/src/service_surface.rs (444)
5. language-semantics/src/tests.rs (1007)
6. language-semantics/src/type_identity.rs (1321)
7. layout-plans/src/layout_reports.rs (547)
8. layout-plans/src/lib.rs (1598)
9. layout-plans/src/materialization.rs (2042)
10. layout-plans/src/symbolic_materialization.rs (942)
11. layout-plans/src/tests.rs (3046)
12. mutation-matrix/src/lib.rs (113)
13. numerics/src/bignum.rs (1520)
14. numerics/src/float_semantics.rs (2523)

(Paths below are relative to `omega-rust/psi/foundation/language-semantics/src/` unless a crate is given.)

---------------------------------------------------------------------------
## 1. declaration_selection.rs (2903 lines, read in full)

**Purpose.** One file holding several unrelated ledgers/vocabularies for the Build-selected declaration machinery: (a) the per-occurrence ledger of "authored declaration selections" (which symbol a source occurrence resolved to), (b) the `Build` operation vocabulary and product-request shapes (what `select_provider`, `bind`, etc. mean), (c) a static-operand graph for Build product operands, (d) a generic provider-choice algorithm, (e) the product target catalog. Header comment (1-6) names these as "build-selected declaration ledgers, target and policy predicates, product operand bindings, provider choice resolutions".

**Shape.**
- `ResolvedAuthoredCallBinding {selected, original}` :11 (plain pair of SymbolHandle).
- mod `build_selection_origin` :17-91: `BuildSelectionOrigin {activation, author: RetainedDeclaration, occurrence: Span, event_ordinal}`; `capture` :30, `from_retained_parts` :45. Re-exported :1213.
- mod `product_policy` :92-137: `AuthoredRootGrant {selector: String, selecting_machine, source_span}` :97; `AuthoredBehaviorExclusionKind {CrashCause, PhysicalAuthorityClass, Service{identity,name}}` :104; `AuthoredBehaviorExclusion` :120 with `authored_label` :127.
- `pub mod provider_choices` :138-778: `ApplicationKey = (Option<PackageKeyIdentity>, String, Option<String>)` :140; `selected_application_slots` :146; generic `choose<CandidateView, DeclarationView>` :169-404 returning `Outcome{choices: Vec<Choice>, errors: Vec<Error>}` :500; traits `Candidate` :408 (slot, provider, target, build_keyed, has_requirements, covers_requirements, receiver_count, supplies_receiver, shares_receiver) and `Declaration` :423; `Tier{Override,Default}` :430; `Reason{Override,Default,UniqueCoveringCandidate}` :436; `Error` enum 11 variants :449-497; tests :505-777 with a toy `Map` candidate.
- mod `product` :782-1212: `BuildSelectionRequest<Requirement>` {ProviderReadiness, Entry, Declaration(Arc<ProductDeclarationRequest>), Exclusion} :786; `ExclusionVocabulary {CrashCause, PhysicalAuthorityClass}` :868 with `cases()` :888 (hard-coded name lists: "Trap","Abort" and 14 authority-class names); `BuildOperation::product_operand_role` :935 and `product_operand_count_valid` :955; `ProductDeclarationRequest {source_span, operation, operands: Box<[ProductDeclarationOperand]>}` :971 with `validate_shape` :985; `ProductDeclarationOperand {path: String, source_span, application: Option<Arc<ProductStaticOperandGraph>>}` :978; executed-result types `ExecutedBuildSelection`/`ExecutedProductSelection`/`ExecutedNativeBinding`/`ExecutedEnumExclusion`/`ExecutedRootBinding` :1037-1106; `NativeBindingFormation` :1139 with `seal` :1150; `CompositionMode {Fused, Independent}` :1030.
- `pub mod product_operands` :1214-1575: arena-based graph `ProductStaticOperandGraph` :1230 (16 arenas), `ProductOperandType` :1268 (11 variants), `ProductExpressionKind` :1342 (about 25 variants incl. Match, Atomic, Cast, Call), match pattern types, `ProductStaticEnvironment` :1499; methods `remap_source_spans` :1525, `authored_operand_span` :1553.
- mod `product_target` :1576-1651: `ProductTarget` 9 variants :1578, `ALL` :1591, `from_code` :1603, `code` :1616, `canonical_name` :1623, `source_case` :1637.
- Top-level ledger :1661-2479: `AuthoredDeclarationSelectionExposure` :1662, `...Kind` (15 variants) :1669, `...LateBinding` (11 variants) :1688, `...Target {Resolved, Intrinsic, LateBound}` :1720, `...Intrinsic` (27 variants) :1885, `CollectionMeasure {Length, Capacity}` :1736, `BuildOperation` (13 variants) :1766 with `ALL` :1807, `authored_spelling` :1823, `from_call_target` :1841, `intrinsic` :1847; `AuthoredDeclarationSelectionOccurrenceId(u64)` :1920; `CompilerDerivedSelectionPartition(u64)` :1929; `AuthoredDeclarationSelection` row :2021; `AuthoredDeclarationSelections {rows: Vec, by_span: SpanIndex}` :2102 with `SpanIndex(HashMap<SourceSpan, Vec<u32>>)` :2117; suffix-rebase type :1984.

**Inputs/outputs.** The ledger is append-only: `record_resolved*` :2215/:2226 and `record_late_bound*` :2249/:2260 push a row whose occurrence id is its row index (`next_occurrence_id` :2457); `finalize_late_bound` :2389 and `finalize_intrinsic` :2419 flip a `LateBound` row to `Resolved`/`Intrinsic` exactly once; `all_finalized` :2447. `replace_prefix_and_rebase_suffix` :2143 builds a new ledger = destination prefix + source rows after `source_frontier`, shifting ids. `choose` consumes candidate/declaration views and a target string and returns choices plus errors; it mutates nothing outside its locals.

**How it decides.**
- `choose` :169 keys on slot (generic `Ord` type), provider, target string (`row.target().is_none_or(|scope| scope == target)` :193; candidate applies when `target().is_empty() || == target` :225), and receiver contracts. Order: explicit Override beats Default beats inherited-receiver beats "unique build-keyed covering candidate" (:251-385). Slots are processed in order authored-first, then more receivers, then slot order (:243-249).
- `source_coordinate_occurrence` :2282 looks up a ledger row by source span + exposure + kind; returns `Err(CompilerCopyRequiresOwner)` if any matching row has a compiler partition, `Ambiguous` if more than one row matches. `resolved_occurrence` :2313 keys on span+exposure+kind+selected symbol and skips partitioned rows (:2328).
- `BuildOperation::from_call_target` :1841 decides by string compare of the call target against `authored_spelling` (:1823-1838 e.g. "bind", "select_provider"). `is_unresolved_provider_selection_call` :1781 is that string match plus an invalid target symbol.
- `validate_shape` :985 decides by operand count per operation (:955-966) and rejects empty path or empty span; for `WireCompatibilityFact` operands it parses the operand `path` string with `crate::wire::WireCompatibilityFact::from_name` :1004 and rejects repeats :1008-1022.
- `NativeBindingFormation::seal` :1150 refuses unless `root_scope` and (`author_package` is None or equals the sealing package).
- `ProductTarget::from_code` :1603 indexes into `ALL` by position; `code()` :1616 is the position in `ALL`, so reordering `ALL` changes the wire code (inferred from :1603-1621).
- `BuildSelectionOrigin::from_retained_parts` :45 requires both declarations to be `SymbolKind::Machine` and a non-empty in-bounds occurrence span.
- `replace_prefix_and_rebase_suffix` :2143 compares the shared prefix row by row on occurrence id, span, exposure, kind, partition, static-argument position (:2162-2168) and target; a source `LateBound` row may become `Resolved`/`Intrinsic` in the destination (:2172-2180), nothing else may differ.

**Cases by shape.**
- `product_operand_role` :935-953: fixed table by (operation, position): ProviderSelection 0 BoundarySubject, 1 Provider; Service*/BoundaryAcceptance 0 BoundarySubject; AssumedBindingContract 0|1 Requirement; NativeBinding 0 Requirement; RepresentationSelection 0 Data, 1 Representation; WireCompatibilityRequest 0..=3 Data, 4.. WireCompatibilityFact; everything else `None`.
- `product_operand_count_valid` :955: exact counts (1 or 2, `>= 5` for wire) and `_ => false` for other operations (OptimizationSelection, OptimizationReportRequest, IncludedSourceHandoff, LogWriteLine, ProviderReadiness): a product request for them is rejected.
- `requires_lexical_subject` :800 lists seven operations by name.
- `ExclusionVocabulary::cases` :888 hard-codes the case names as strings; `helper()` :881 names Build-prelude helper functions `build_exclusion_crash_code`/`build_exclusion_physical_code`.
- `ProductTarget` is a closed list of 9 targets.

**Same job elsewhere.** The prefix-compatible-rebase idea (retain base, shift extension) is also used by symbol-resolved-trees (callers :295 in `symbol-resolved-trees/src/lib.rs`, grepped). Operation spelling tables: `BuildOperation::authored_spelling` :1823 and `intrinsic` :1847 are two parallel 13-arm matches over the same enum; `CollectionMeasure` :1741 repeats the pattern (ALL / authored_spelling / from_authored_spelling) as `BuildOperation` :1807-1845 (and `ProductTarget` :1591-1637 has ALL/canonical_name/from_canonical_name/source_case). `ProductTarget::canonical_name` vs `source_case` are two spellings of the same 9 names (:1623, :1637).

**Unused / one-caller code (grep `omega-rust`, excluding this file).**
- `selected_application_slots`: 2 callers (`build-time-evaluation/src/product.rs:1304`, `omega/compilation/realization/provider-selection/src/choice/selection.rs:31`).
- `is_unresolved_provider_selection_call`: 2 callers (`03_.../declarations.rs:1597`, `build-time-evaluation/src/product.rs:3079`).
- `replace_prefix_and_rebase_suffix`: 2 callers (`symbol-resolved-trees/src/lib.rs:295`, `control_flow_and_calls.rs:1345` which sits in a test-like position; read not verified).
- `authored_operand_span`: 1 caller (`03_.../call_and_product_selections.rs:1986`). `remap_source_spans`: 5 call sites in 4 files.
- `ProductTarget::from_canonical_name`: 0 callers outside the file (grep `from_canonical_name` over omega-rust returns only a different type in `filesystem-authority/src/lib.rs:115`). `ProductTarget::from_code`: 5 callers. `source_case`: 3 uses in `build-declarations/src/prelude/targets.rs`.
- `AuthoredBehaviorExclusion::authored_label`: grep `authored_label()` found no caller outside the file.
- `provider_choices::choose`: grep `provider_choices` hits `provider-selection/src/choice/selection.rs` (6) and `product.rs` (12 for the combined pattern); the generic is implemented by test `Map` here and used by the one `choice/selection.rs` site (not read).

**Stated limits.** None with "unsupported/remaining" wording seen in the file.

**Fail-open risks.**
- `choose` :330-368 when a slot has no explicit/default and no keyed covering candidate (`keyed` empty, :375 `[] => {}`) pushes neither a choice nor an error: the slot is silently left unchosen. Reachable when candidates exist but none is `build_keyed`. Whether the caller later reports an unresolved slot is not traced ("open").
- `choose` :341-348 `supplies_receiver` returning `None` marks `unresolved` rather than accepting; produces an error (fail-closed).
- `shares_receiver` `None` -> `UnresolvedOverlap` error :396 (fail-closed).
- `selected_application_slots` :158-160: if no available slot matches, returns the subject itself (accepts an unknown slot). Open, not traced.
- `SpanIndex::eq` :2120 always returns `true`, so two ledgers with different indexes compare equal; the index is derived from rows and rebuilt in `replace_prefix_and_rebase_suffix` :2211 (stated by structure only; traced: `record_*` always add to the index).
- `product_operand_role` returns `None` for out-of-range positions :951, `validate_shape` only enforces the fact role; validated by count first :986.
- `AuthoredDeclarationSelections::get` :2338 returns None for unknown ids (not accepting).

---------------------------------------------------------------------------
## 2. lib.rs (2524 lines, read in full)

**Purpose.** Crate root of `language-semantics`. Header comments (1-23) say it holds target-neutral semantic identities and vocabularies shared by symbol-resolved, typed and checked Psi. In practice it is a grab-bag of about 20 inline modules, each a small data vocabulary or table, plus `pub mod` declarations of the sibling files (`const_value` :1091, `content` :1092, `declaration_selection` :1093, `numeric_intrinsics` :1419, `service_surface` :2232, `type_identity` :2267) and `#[cfg(test)] mod tests;` :2524.

**Shape (inline modules, each with file:line).**
- `boundary_values` :28-89 `BoundaryValueRecipes` (3 arenas), `BoundaryValueRecipe` enum (Absent/Rejected/Integer/Float/Reference/FixedArray/Record/TargetLayouts/Opaque) :39. Data only.
- `callback_declarations` :90-203 `PrivateCallbackSlotSelection` :94 (identity strings + `application_commitment: [u8;32]` + `application_encoding: Arc<[u8]>`), `has_complete_identity` :106, `validate_identity_encoding` :116; `BoundaryTelescope` :152 and friends; consts `BOUNDARY_NATIVE_PARAMETER_CAPACITY = 32` :147, `BOUNDARY_CALLBACK_CAPACITY = 64` :149.
- `opaque_representations` :204-294 `selected_application_commitment` :225 (digest over schema version, origin byte, lifecycle byte, copy-disposition byte, conformance commitment); `SelectedOpaqueApplication` :248 with `validate_identity_encoding` :263.
- `boundary_bodies` :296-318 `BoundaryBodySelection` (10 fields) and `CheckedBodyCandidate<Body>`.
- `build_requests` :319-343 `is_project_request_protocol` :320, `is_build_protocol` :327.
- `boundary_provider_review` :345-697 borrowed review structs (`BoundaryProviderReview<'facts>` :692 etc.) and a private `retained` module :349-634 that copies them to owned form (`retain` :444 / `borrow` :484) with one test :530.
- `byte_predicates` :698 re-export of semantic_vocabulary types.
- `conformance_identity` :703-1090 `ConformanceIdentityDescription` :708, `encoded_bytes` :768 (hand-rolled byte framing with tags 0xff/0xfd/0xfe), `fingerprint_and_commitment` :851, `source_identity_commitments` :857 (FNV-1a "report" fingerprint plus domain digest `omega.psi.closed-conformance-application.v4\0`); 3 tests incl. a pinned snapshot (545 bytes, hash literal) :981-996.
- `direct_calls` :1094-1159 `DirectCallDeclarations<Declaration>` :1097 with a `HashMap<Declaration, usize>` index and `called_machine_position` :1155 (`new` :1137 keeps the FIRST position per declaration via `entry().or_insert`).
- `external_bindings` :1160-1247 `ExternalBindingMechanism` (5), `ExternalBindingIdentity`, `ExternalBindingTable::intern` :1221 (linear scan interning, ids start at 1).
- `machine_termination` :1248-1418 `MachineSupplyMode` :1253, `TerminationGuarantee` :1280, `ProgressPremise`, `RankingWitness` :1307, `TerminationInterface`, `MachineTerminationPlan` :1339, `RankingViewDeclaration` :1348, and `impl RankingViewId` :1363 with four consts and `canonical` :1369 / `canonical_path` :1379 / `catalog_declaration` :1389 / `from_catalog_declaration` :1403.
- `permissions` :1420-1488 plain enums `PermissionEventKind/Access/EventSource/Provenance/ClaimIdentity`.
- `provider_declarations` :1489-1541 plain data structs.
- `quotient_correspondence` :1542-1787 about 30 plain data types for the quotient correspondence judgment (`QuotientCorrespondenceJudgment` :1776). No functions.
- `semantic_domains` :1788-1967 `SemanticDomainTable` :1792 (names Vec, pre-seeded "Wrapping","Saturating","Trapping" :1799-1803, consts WRAPPING/SATURATING/TRAPPING = 1/2/3 :1817-1819), `DomainSemanticRoles`, `DomainEstablishmentRoute` :1882 (3 variants), `BoundaryQualificationIssuer`, `QualificationEvidenceOrigin` (8 variants) :1941.
- `semantic_identities` :1968-1990 macro `semantic_id!` making `SemanticDomainId`, `ServiceReachId`, `ServiceReachRowId`, `ExternalBindingId`, `RankingViewId` as `u32` newtypes with NULL = 0.
- `service_reach` :1991-2231 interface/plan/summary enums for reach, synchronous invocation, suspension, blocking; `ServiceReachRowTable` :2072 (rows Concrete(Vec<ServiceReachId>) or AbstractBounded(row); `intern` sorts+dedups :2091), `ServiceReachTable` :2159 (definitions with parents; `extend_closure` :2219 recursive).
- `trust_declarations` :2233-2266 plain structs.
- `value_domain` :2268 `ValueDomain` with a single variant `Finite` :2270.
- `wire` :2295-2337 `WireCompatibilityFact` (5 variants, `from_name` :2306 string match), `WireCompatibilityDemand` :2319, `WireScalarRange` :2332.
- `native_bindings` :2370-2521 trait `NativeLocatorValue` :2374; `decode_native_locator` :2392 turns an evaluated `ForeignBinding` value into `NativeBindingLocator`.
- Re-exports :2339-2368 (language_core items, termination, permissions, domains, ids, reach).

**Inputs/outputs.** Mostly types. Real logic: the identity byte encoding/commitments (:768-864, :225-245), the validate_identity_encoding checks (:116-131, :263-292) which recompute commitments from the stored encoding and compare, the interning tables, and `decode_native_locator`.

**How it decides.**
- `is_build_protocol` :327 decides by symbol name string equal to `name` AND source origin `Toolchain` AND source path equal to `<build-prelude>` (:332-341). Keyed on spelling plus a source path literal.
- `RankingViewId::canonical` :1369 maps the path strings "Nat::Descending", "Nat::BoundedDistance", "Slice::Length", "Nat::IncreasingTo" to ids (spelling keyed). `catalog_declaration` :1389 returns a declaration only for `NAT_DESCENDING`; the other three ids return `None` (:1399).
- `ServiceReachTable::intern` :2164 keys on `SymbolHandle`; `id_for_name` :2190 keys on display name string.
- `SemanticDomainTable::intern` :1821 keys on name string.
- `decode_native_locator` :2392 keys on the case name strings "Syscall", "DllImport", "PeByName", "PeByOrdinal", "ElfVersioned", "MachODylibSymbol" (:2397-2451) and on exact field-name lists (`exact_fields` :2456).
- `PrivateCallbackSlotSelection::validate_identity_encoding` :116 recomputes commitments and compares to stored (identity by content).

**Cases by shape.**
- `decode_native_locator` :2411-2451: four DllImport cases; unknown -> `Err("unknown compiler-owned DllImport case ...")` :2451. Width arguments (`widths: [u64;3]`) must be zero for unused slots (`require_unused_width` :2516), e.g. Syscall needs all zero :2398-2400.
- `RankingViewId` has exactly four canonical views :1364-1367.
- `ExternalBindingMechanism` closed 5-way list :1164.
- `ValueDomain` accepts only the name "Finite" :2275-2279.
- `WireCompatibilityFact` 5 names :2306-2314.
- Concrete `ServiceReachRow::AbstractBounded` chains resolved recursively in `services` :2128.

**Same job elsewhere.**
- Interning tables by linear `position` scan: `ExternalBindingTable::intern` :1221, `SemanticDomainTable::intern` :1821, `ServiceReachRowTable::intern` :2091, `ServiceReachTable::intern` :2164 (four copies of the same "scan, else push, id = index+1" pattern in this file). The AGENTS rule on dense id tables is relevant: these use Vec scan, not maps.
- `selected_application_commitment` (free function :225) has a second implementation in provider-planning `calling_policy_plans/opaque_representations.rs:72` and `psi/.../representation-planning/selections/src/lib.rs:83,140` (grepped; the latter two define `selected_application_commitment`/`rederived_...` methods; whether they call this function was not read, "suspected" overlap).
- Two separate commitment preimage domains exist for conformance applications: v4 here :861 and a "terminal v7" domain string only in a test :1038 (the terminal one is implemented elsewhere, not read).

**Unused / one-caller code (grep over omega-rust, excluding this file).**
- `build_requests::is_build_protocol`: callers `terminal-production/src/build_bindings.rs:661`, `build_admission.rs:92,127`, `call_and_product_selections.rs:1778` (4 external sites). `is_project_request_protocol`: 1 external caller (`build_admission.rs:89`).
- `RankingViewId::canonical`: external callers `checks_termination.rs:2689,2751`, `lowering_machine.rs:1925` (plus tests.rs). `canonical_path`: callers in contract_entailment.rs:26451, call_cycles.rs:1521,1549,1582, calls.rs:5313, product.rs:1092,1101 (and others). `catalog_declaration`: `checks_termination.rs:3083` (1). `from_catalog_declaration`: `lowering.rs:2631` (1).
- `DirectCallDeclarations`: used by typed-trees lib.rs, checked-trees providers_and_trust.rs, provider-selection target_bodies.rs, provider-planning (5+ files); well used.
- `native_bindings::decode_native_locator`: 1 external caller (`omega/build-system/build-settings/.../native_bindings.rs:123`); `decode_binding_value`: re-exported in `provider-planning/src/evaluated_via_bindings.rs:30` and used `binding_evaluation.rs:123` (1 real call).
- `boundary_provider_review` / `BoundaryProviderReview`: 15 external references in 4 files (`06_.../projections.rs:8`, `checked-trees/operators_and_proof.rs:2`, `selected-dispatch/provider_binding.rs:1`, `provider-selection/boundary.rs:4`).
- `BoundaryValueRecipes`/`TrustDeclarations`/`BoundaryValueRecipe`: 94 references in 9 files (mostly `backend/layout/src/boundary_values.rs:40` and `typed-trees/src/lib.rs:36`, counted together; counts not split by name).
- `ValueDomain`: not grepped separately by name beyond the broad pattern; `ValueDomain::from_name` external callers not individually confirmed ("suspected" used by typed/checked stages).
- `ServiceReachTable::id_for_name` and `SemanticDomainTable::lookup` external caller counts not grepped.

**Stated limits.** None with unsupported/remaining wording. Comment :146-151 constants are plain capacities with no comment.

**Fail-open risks.**
- `ServiceReachRowTable::services` :2121-2131 returns an empty slice for an unknown or null row id (`None => &[]`). An unknown row reads as "no services", i.e. empty reach. A bad/stale `ServiceReachRowId` would therefore read as no service reach. Not traced to callers ("open").
- `ServiceReachTable::set_parents` :2208 silently does nothing when `id` is out of range (`if let Some`). "open".
- `ServiceReachTable::extend_closure` :2219 tolerates unknown ids (adds the id, no parents). "open".
- `SemanticDomainTable::name` :1831 returns None for NULL/out-of-range (fail-closed for callers that check).
- `PrivateCallbackSlotSelection::has_complete_identity` :106 treats `application_commitment == [0;32]` as incomplete (fail-closed).
- `DirectCallDeclarations::new` :1143 duplicates are resolved by keeping the first position; a second machine with the same declaration key is unreachable via `called_machine_position`. Not traced ("open").
- `is_build_protocol` :332 returns false on any lookup failure (fail-closed).
- `ExternalBindingTable::identity` :1233 returns None on out-of-range (fail-closed).

---------------------------------------------------------------------------
## 3. numeric_intrinsics.rs (775 lines, read in full)

**Purpose.** Closed catalog of the numeric operations the compiler gives meaning to without library source: named float operations (`F32::add_toward_zero`, `is_nan`, ...), float binary/compare operations, integer comparisons, negation, and numeric conversions. It gives each a signature (operand type, arity, result category) and a 4-byte canonical wire encoding.

**Shape.**
- Macro `tagged_vocabulary!` :10 defines a closed enum with `ALL`, `name()` and a tag function. Used for `NumericType` (10 variants, tags 0-9, :40), `PrimitiveFloatBinaryOperation` (10, :65), `PrimitiveIntegerComparisonOperation` (6, :84).
- `NumericIntrinsic` enum :97 {Builtin{function,format}, FloatBinary, IntegerComparison, Negation, Conversion{source,target,domain}}. `NumericResult` :119 {Primitive, Boolean, FloatClass}. Private `FloatResult` :126.
- `FLOAT_OPERATIONS` table :144-288: 29 rows of (name, arity, result kind, f32 BuiltinFunction, f64 BuiltinFunction) (count by reading; rows are `minimum` through `fused_multiply_add_toward_negative`).
- `NamedNumericIntrinsic {operand, operand_count, result, execution}` :290 with `resolve(namespace, name, domain)` :298 and `accepts(parameters, result)` :334.
- `NumericIntrinsic::signature` :348, `canonical_encoding` :424, `from_canonical_encoding` :478, `builtin_format` :524; `NumericType::scalar_type` :536; private `numeric_primitive` :569 and `conversion_operand` :585 (string -> NumericType).
- Tests :601-775.

**Inputs/outputs.** Pure functions on strings/enums. `resolve` takes the namespace spelling ("F32", "I8", ...) and member name, returns the signature or `None`.

**How it decides.**
- `resolve` :298: namespace spelling ("F32".."U64") picks the target type (:569-583); a name starting `from_` picks a conversion whose source is parsed from lowercase type spellings (:585-599); otherwise the namespace must be F32/F64 (:312-316) and the name is `negate` or a row of `FLOAT_OPERATIONS` found by string equality (:317-321). Keyed on source spelling.
- Conversion domain: float targets are forced to `ArithmeticDomain::Exact` regardless of the given domain (:305-309).
- `signature` :348 for `Conversion` returns `None` when source == target, both non-float, a float target with non-Exact domain, or domain Wrapping (:406-411). So integer<->integer conversions do not exist here, and Wrapping float->int is rejected.
- `accepts` :334 compares operand count, each parameter equals `Some(self.operand)`, result equals `Some(self.result)`. All operands must be the same type.
- `canonical_encoding` :424 builds `[kind, a, b, c]`; Builtin uses the row index in `FLOAT_OPERATIONS` as the byte (:432-442) so reordering that table changes the wire encoding (inferred).
- `from_canonical_encoding` :478 decodes then re-encodes and requires equality (:521).

**Cases by shape.** `FLOAT_OPERATIONS` is a fixed list; `Conversion` only permits int<->float or float<->float with different types; `IntegerComparison` rejects float types :392. `FloatBinary` result is `Primitive` for Add/Subtract/Multiply/Divide, else `Boolean` :378-388.

**Same job elsewhere.** `numerics::float_semantics_catalog::FloatSemanticOperation::named_call_arity` is checked against `FLOAT_OPERATIONS` arity by a test :614, i.e. the name/arity list is kept in two places (this file and `numerics/src/float_semantics_catalog`, not read; "suspected" duplicate catalog). `float_semantics.rs` in this unit (file 14) has its own float operation vocabulary (see there). Type spelling tables `numeric_primitive` :569 / `conversion_operand` :585 duplicate the `NumericType::name()` spellings (upper-case vs lower-case).

**Unused / one-caller code.** `NamedNumericIntrinsic::resolve`: 1 external caller (`typed-trees/src/declarations.rs:2566`). `from_canonical_encoding`: callers in `terminal-psi/codec/src/structural_wire.rs:1433` and `semantic_module.rs:1400` (2 non-test). `NumericIntrinsic` used by terminal-psi boundary.rs, retention.rs:2728 (grepped with head_limit 25; listing was truncated so the counts are lower bounds).

**Stated limits.** None.

**Fail-open risks.** none seen. Every unknown spelling/tag returns `None`; round trip check :521.

---------------------------------------------------------------------------
## 4. service_surface.rs (444 lines, read in full)

**Purpose.** Data description of a service (a trait whose implementations are operational providers): its method signatures, claims, receiver applications and progress premises, plus the structural equality and validity rules that compare two such descriptions across different identity tables.

**Shape.**
- `identities::ProviderIdentityTable` :33 (`texts: Vec<Box<str>>`, `slots: HashMap<Box<str>, ProviderIdentityId>`), `intern` :39, `id_at` :50, `text` :54 (unknown id returns `""` through `unwrap_or_default` :59), `texts` :62; `ProviderIdentityId(u32)` :8 (0 = invalid).
- `ServiceSurface {trait_name, identities, trait_package_identity, receiver_applications, methods}` :70; `ServiceReceiverApplication` :79; `ServiceMethod` :133 (16 fields); claims `ServiceEntryClaim` :431, `ServiceResultClaim` :441; `ServiceProgressPremise` :384.
- Methods: `eq_in` on application/method/premise (:89, :154, :206), manual `PartialEq for ServiceSurface` :228 (compares by identity TEXT through each side's own table), `matches_receiving_application` :250, `selection_slot_identity` :288, `root_application_identity` :302, `validate_receiver_applications` :308, `identity_text` :378.

**Inputs/outputs.** Reads only its own tables; returns bool / Result<(), &'static str>.

**How it decides.**
- Equality keys on identity text, not id: `left.text(a) == right.text(b)` (:95-97, :164, :180, :188, :222, :129). An id that is invalid/out of range reads as `""` so two unknown ids compare equal to each other (inferred from :54-60).
- `matches_receiving_application` :250: finds the method by requirement identity text (:255-258); filters receiver applications by trait-name text == method.requirement_owner AND package identity equal AND listing the requirement (:262-272); if the surface has receiver applications but none match -> false (:273); with `receiving == None` it requires every matching view to have no type/lifetime arguments (:276-281); empty string receiving -> false (:282); else any view whose application text equals `receiving` (:285).
- `validate_receiver_applications` :308: root = first application must have the surface's package/trait; roster length == methods count and contains each method's requirement id (:317-322); each application must have non-empty trait/argument identity texts, requirements known and strictly increasing by text (`windows(2)` `>=` -> error, :345-351); no two applications equal by (package, trait text, arguments, lifetime args) (:352-373).

**Cases by shape.** `ServiceProgressSubject` has only `Parameter(usize)` :413; `ServiceEntryAuthorityFlow` only `Accepts` :418; `ServiceProgressEstablishmentRouteKind` two kinds :398.

**Same job elsewhere.** `ServiceMethod::eq_in` :154 repeats field-by-field the comparison the derived `PartialEq` on `ServiceMethod` would do if ids were comparable; both exist (derive at :132 and `eq_in`). Receiver-application matching by `root_application_identity` also appears in `psi/compilation/validation/src/borrow_and_reflection.rs:3719-3854` (grepped: parameter named `root_application_identity`, not read; "suspected" parallel).

**Unused / one-caller code (grep over omega-rust excluding file).**
- `matches_receiving_application`: 3 call sites (`provider-selection/src/binding.rs:245`, `binding/provider_instances.rs:91`, `backend-plan/src/target_providers.rs:779`).
- `selection_slot_identity`: 4 call sites + a wrapper in `provider_plan/service_schemas.rs:123`.
- `validate_receiver_applications`: 2 call sites + wrapper `service_schemas.rs:133`.
- `root_application_identity`: 5 external call sites + wrapper `service_schemas.rs:119`.
- `ProviderIdentityTable::id_at`: not separately grepped (count unknown).

**Stated limits.** None.

**Fail-open risks.**
- `ProviderIdentityTable::text` :54 returns `""` for an invalid or out-of-range id. In `eq_in`, two ids that are both unknown in their tables compare equal. Reachability depends on callers constructing ids from the same table; not traced ("open").
- `matches_receiving_application` :255-261 returns `false` when the method is absent (fail-closed); when `receiver_applications` is empty, it skips the view requirement (:273) and `receiving == None` returns `all(...)` over an empty iterator = `true` (:277). So a surface with no receiver roster matches any requirement named by one of its methods when no receiving application is demanded; and with `Some(non-empty)` it returns `views().any(...)` = false. Reachable for surfaces with no roster (traced by reading :273-285 only; whether rosters are always populated was not traced).

---------------------------------------------------------------------------
## 5. tests.rs (1007 lines, read in full)

**Purpose.** Unit tests for `language-semantics` (header :1-5 lists carry policies, external bindings, termination plans, semantic domains, service reach tables, array extent identities, type identity encoding).

**Shape.** Modules: `carry_policies` :7 (tests `CarryPolicy::intersect/permits/STRICT/PERMISSIVE` and `CarryPermission` round trip; those types live in `language_core`, not this crate), `external_bindings` :57, `machine_termination` :108 (round trip of the 4 ranking view ids), `semantic_domains` :128, `service_reach` :221, `array_extent_identity` :301 (visitor tests over `visit_type_identity_package_owners` with hand-built identity strings), `type_identity` :477, `type_identity_vocabulary` :565 (helper builders `atom`/`compound`/`owner`/`nominal` :604-637, tests including `vocabulary::audit()` :662).

**How it decides / what it pins.** Fail-closed expectations: unknown package owner -> `UnknownPackage` (:350-367), unknown arithmetic policy "Invented" -> `MalformedIdentity` (:384-389), `hidden-owner(7)` -> Malformed (:425, :462), region forms -> `Unadmitted::ApplicationScope` (:945-974), retired scalar interval tags refused (:995-1006). Also a test that `static-application` stale field `alias` is refused (:695-704).

**Same job elsewhere.** `typed-trees/src/type_identity.rs:4692-4726` defines a second `Owners` visitor and tests using `visit_type_identity_package_owners` (grepped, not read); here `Owners` is defined three times (:306, :571 plus `PackageOwnerCounter` :483).

**Unused / one-caller / stated limits / fail-open.** Not applicable for tests; none seen.

---------------------------------------------------------------------------
## 6. type_identity.rs (1321 lines, read in full)

**Purpose.** A hand-written recursive-descent parser for the canonical text form of type identities (strings such as `array(compiler-type(atom(u8)),const-expression(call(target(...),integer(3))))`). It never builds a tree. It walks the text and tells a visitor each package-owner digest it meets, while checking the text is in the known grammar. Callers use it to collect which packages a type identity depends on, and to find the byte offset where an identity is malformed.

**Shape.**
- `callable_display::callable_display_path` :12 (extract the authored path from a `named-callable(path(...))` string), private `atom_payload` :28.
- `framing::Reader` :114 (position over `&str`): `production::<F: Vocabulary>` :157, `peek*`, `raw_value` :201 (parses `(...)` with `\` escapes of `\ ( ) ,`), `opaque`, `leaf_opaque`, `atom`, `leaf_atom`, `leaf_byte_atom`/`hexadecimal_value` :312 (`<len>:<hex>` with len*2 == hex length, lowercase hex).
- `grammar` :338: `type_identity` :360, `canonical_const` :418, `region` :429, `name_atom` :446, `semantic_name` :463 (parses a nested name string as `nominal(...)`, `compiler-type`, `derived-application`, `placed-field`, `placed-operation` or else hands it to `visitor.embedded_name`), `owner` :524 (decodes 64-hex package digest to `[u8;32]`, refuses all-zero, calls `visitor.package_owner`), `constraint` :562, `declared_domain` :577, `compiler_domain` :596.
- `index` :618 (the const-expression/index grammar): `array_length` :630, `expression` :644, `pattern` :814, `static_argument` :898, `embedded_operation` :927, `embedded_algebra` :942.
- `lifetime_partition::normalize_requirement_lifetime_partition` :963 (renumber raw ordinals to first-appearance indices).
- `vocabulary` :981: traits `Spelled`, `Vocabulary`; macro `vocabulary!` :999 generating 12 enums (`TypeForm` 13, `NameForm` 6, `OwnerForm` 4, `ConstraintForm` 4, `DomainFamily` 3, `ArrayLengthForm` 5, `IndexForm` about 48, `RegionForm` 3, `PatternForm` 2, `FieldActionForm` 5, `Label` 39) with spelling <-> variant tables; test-only `audit` :1033.
- Public: `TypeIdentityVisitError` :1236, trait `TypeIdentityPackageOwnerVisitor` :1265 (enter/leave/reserve/package_owner/embedded_name), `visit_type_identity_package_owners` :1273, `malformed_identity_offset` :1282.

**Inputs/outputs.** Input: identity text. Output: `Ok(())` or `TypeIdentityVisitError`; side effects only through the visitor callbacks (`enter`/`leave` bracket every node so a visitor can enforce depth; `reserve` is called before allocating an unescaped copy).

**How it decides.** Everything is keyed on source spelling of tags: `Reader::peek_token` :148 takes the longest run of `a-z - _` and `F::parse` matches it against the vocabulary table (:1019-1024). Package owner decisions are made by the visitor (`UnknownPackage` is the visitor's judgment, not this file's). The cast domain check :726-729 accepts a name if `ArithmeticDomain::from_name` knows it or it equals `Exact`'s name.

**Cases by shape.**
- `type_identity` :360-416 handles 13 type forms; references :368-376 allow exactly one child type (a following `,` becomes `ApplicationScope` error if a region form follows, otherwise `MalformedIdentity`).
- `grammar::region` :429 and `region_operand` :437: every region form (`region`, `elided-region`, `unresolved-region`) is refused as `Unadmitted::ApplicationScope` (they can't appear in a canonical identity).
- `OwnerForm::Toolchain | Unresolved` refused as `InvalidIdentity` :556; `ToolchainSource` accepted if 64 hex chars but NOT reported to the visitor :550-555 (no package owner callback).
- `IndexForm::StaticApplication` at top-level `expression` -> `MalformedIdentity` :807 (only accepted through `static_argument` :905); `UnsupportedConstSubstitution`, `UnboundMatchSubject`, `InvalidExpression`, `InvalidDerivedOrigin` -> `Unadmitted::InvalidIdentity` (:640, :808, :488).
- `expression` binary operators accept an optional "licensed" prefix (`operation(...)`, `algebra(...)`) then two expressions, and extra expressions only when licensed (:681-698).
- `Named` type accepts `integer-const`, `canonical-const` or a name atom :401-413.

**Same job elsewhere.** Two near-identical escape scanners: `callable_display::atom_payload` :28 and `framing::Reader::raw_value`/`unescaped_value` :201-300. The visit traversal is also implemented a second time over a typed form in `typed-trees/src/type_identity.rs` (4692+ visitor tests there; the producer of these strings is that file, not read; "suspected" mirror: any new type form must be added in producer and in this grammar). The `package-owner` digest hex decode at :532-544 is hand-rolled; other hex decoders likely exist (not grepped).

**Unused / one-caller code (grep over omega-rust excluding this crate's tests/type_identity).**
- `visit_type_identity_package_owners`: 1 non-test external caller (`review-evidence/src/encoding/encode/membership/visitor.rs:63`); plus tests in `typed-trees/src/type_identity.rs:4726`.
- `malformed_identity_offset`: 1 external caller (`membership/visitor.rs:73`).
- `callable_display_path`: 2 external callers (`build-settings/src/calling_policy_execution.rs:104`, `provider-admission/src/provider_admission.rs:82`).
- `normalize_requirement_lifetime_partition`: about 10 external call sites (provider-planning x3, review-evidence x5, provider_plan.rs:819).
- `TypeIdentityVisitError::UnsupportedEmbeddedName`: only constructed outside this file? grep shows no construction in `language-semantics`; `membership/visitor.rs:70` matches on it. `ResourceLimitExceeded` and `AllocationFailed`: the latter is produced at :288; `ResourceLimitExceeded` is produced only by visitors in tests (e.g. tests.rs:315) (grep over the file).
- The header comment (:2-6) mentions `TypeIdentity` and "interned type identity tables (`index`)". No `TypeIdentity` type and no intern table exist in this file; `index` here is the const-expression grammar (read :618-961).

**Stated limits.** None with unsupported/remaining wording. Variant name `UnsupportedConstSubstitution` and `UnboundMatchSubject` mark identities the producer could not form (:808).

**Fail-open risks.**
- `grammar::owner` :550-555: a `toolchain-source-owner(...)` digest is accepted and never reported to the visitor, so a visitor that checks "every owner is a known package" does not see it. Stated by structure only; I did not trace whether toolchain source owners are meant to skip membership (the test at tests.rs:526-531 uses one and expects success with `owners` count not asserted). "open".
- `semantic_name` :463-471: a name that does not parse as a known compound form and does not look like a vocabulary compound is passed to `visitor.embedded_name(name)` and accepted if the visitor accepts (e.g. `type-parameter:0`, tests.rs:978-980). Whether `embedded_name` implementations reject arbitrary text is outside this file; `membership/visitor.rs` not read. "open".
- `malformed_identity_offset` :1300-1303 maps any non-Malformed/Unadmitted error to `None` (meaning "not malformed"); with the `Unbudgeted` visitor that never errors, other variants can't occur here (traced by reading :1283-1298).
- `framing::Reader::raw_value` :212-220 validates escapes; `unescaped_value` uses `expect("prevalidated escape")` :292 (panic if invariant broken; not reachable from validated text).

=========================================================================
# layout-plans crate (`omega-rust/psi/foundation/layout-plans/src/`)

---------------------------------------------------------------------------
## 7. layout_reports.rs (547 lines, read in full)

**Purpose.** Plain report types describing where each field of a record or sum type sits in memory (`LayoutPlanReport`, `ConventionalSumLayoutReport`, nested record/sum path reports), plus fingerprints of those reports and a "do these two reports agree" check used when a retained layout is replayed against a freshly computed one.

**Shape.**
- `IntegerInterpretation {Signed, Unsigned}` :11 with `stored_bounds(stored_width)` :17.
- `LayoutPlacementReport {At{offset}, IntegerAt{offset,stored_width,interpretation}, Bits{container,container_width,destination_lsb,source_lsb,width}}` :29; default `At{0}` :47.
- `LayoutFieldEntryReport {field: String, member_identity: Option<u64>, placement}` :54; `LayoutPlanReport {schema_report_fingerprint, entries, offsets: Option<Vec<u64>>, size: Option<u64>, align}` :61.
- Sum reports: `ConventionalSumLayoutReport` :87 (tag offset/size/align, common_fields, cases, size, align), case/payload field reports :70-84, array/field wrappers :99-112.
- Nested record/sum path reports :115-164 (`ConventionalNestedRecordSumPathLayoutReport`, `...PathsLayoutReport`, `ConventionalRecursiveRecordSumPathsLayoutReport` with `children`, `ConventionalRecordSumChildHop {Field, Index{count,stride}}`, `ConventionalRecordSumChildInterior {Sum, Record}`), `CONVENTIONAL_RECORD_PATH_DEPTH_LIMIT = 64` :166, `leaf_occurrence_count` :169.
- `PrivateCallbackLayoutDemandReport<SlotApplication>` :182, `NativeLayoutPlanReport<SlotApplication>` :191.
- Functions: `normalized_layout_plan_report_fingerprint` :196, `normalized_conventional_sum_layout_report_fingerprint` :269, `conventional_sum_layout_reports_match_for_replay` :308, `normalized_native_layout_plan_report_fingerprint` :424, `layout_plan_reports_match_for_replay` :461, `layout_placement_sort_key` :512; private `member_sort_key` :505, `conventional_sum_member_identities_are_unambiguous` :367, `hash_optional_member_identity` :406.

**Inputs/outputs.** Pure functions of the report values. Fingerprints are FNV-1a (`semantic_vocabulary::Fnv1a`) over domain strings `omega.layout-plan.v3` :208, `omega.conventional-sum-layout.v1` :273, `omega.native-layout-plan.v2` :435; every result is `.finish().max(1)` so 0 is never produced (:266, :305, :458).

**How it decides.**
- Plan fingerprint sorts entries by (member identity first, else field name) then placement (:196-205), so entry order does not affect it. The member key is the identity when present, else the field name (:505-510): entries WITHOUT an identity are keyed on field spelling.
- `layout_plan_reports_match_for_replay` :461 requires equal `schema_report_fingerprint`, `offsets`, `size`, `align`, equal entry count, then compares sorted entries on identity (and on field name only if identity is None) and placement. It first runs `validate_materialization_field_identities` on both (:465-466), so ambiguous identities make it return false.
- `conventional_sum_layout_reports_match_for_replay` :308 is positional (zip in given order, no sorting) and compares identity (and name only when identity is None), ordinal, offset/size/align; refuses if either side has duplicate identities (:367-404).
- `native` fingerprint sorts callback demands by (slot_identity, callback_requirement_identity) :428-433 and folds `slot_application` NOT at all (the `SlotApplication` generic is never read in the fingerprint; read :424-459).

**Cases by shape.** Three placement shapes only; sort keys tag At=0, Bits=1, IntegerAt=2 (:516-545) while the hash tags are At=0, Bits=1, IntegerAt=2 as well (:230, :253, :238) but the match arms are written in the order At, IntegerAt, Bits.

**Same job elsewhere.** Triple copies of the "member identity else field name" comparison: `hash_optional_member_identity` :406, `member_sort_key` :505, and the inline duplicate-detection in `conventional_sum_member_identities_are_unambiguous` :367-404 and in `validate_materialization_field_identities` (materialization.rs, read later in this unit). The payload field comparison closure appears twice verbatim (:330-337 and :356-363). The two fingerprint functions (:196, :269) and the replay matchers (:308, :461) are parallel for records vs sums.

**Unused / one-caller code (grep `omega-rust`; counted excluding layout_reports.rs and layout-plans tests.rs).**
- `normalized_conventional_sum_layout_report_fingerprint`, `conventional_sum_layout_reports_match_for_replay`, `leaf_occurrence_count`: grep for the three names across omega-rust returned only `layout-plans/src/tests.rs` (12 hits), `lib.rs` (2, the `pub use`) and `layout_reports.rs` (4). Zero production callers outside the crate.
- `normalized_layout_plan_report_fingerprint`: 1 outside caller (`access-plans/src/access_fields.rs:244`). `layout_plan_reports_match_for_replay`: 1 outside caller (`access-plans/src/plan_policy.rs:30`). `normalized_native_layout_plan_report_fingerprint`: 3 outside sites (`backend/layout/src/lib.rs:171`, `data_layouts.rs:172`, builder tests). `layout_placement_sort_key`: 4 outside callers (`layouts.rs:5141`, `foundation_validation.rs:2829`, `callback_catalog.rs:242`, plus in-crate). `IntegerInterpretation::stored_bounds`: 3 outside (`value_custody_core.rs:7651`, `layouts.rs:4785,6067`) + `materialization.rs:1136`.
- `CONVENTIONAL_RECORD_PATH_DEPTH_LIMIT`: used in this crate's lib.rs:1064, symbolic_materialization.rs:65,734; doc `build-time-evaluation/layouts.md:152` mentions it.

**Stated limits.** `CONVENTIONAL_RECORD_PATH_DEPTH_LIMIT = 64` :166 (bounds record path depth).

**Fail-open risks.**
- `stored_bounds` :17-24: `1i128 << (stored_width - 1)` panics for `stored_width == 0` (underflow) and shifts overflow for width 128 on the Unsigned arm (`1i128 << 128`). Callers pass `u64::from(stored_width_bits)` (materialization.rs:1136, read below) after a width validation; not traced for the other three callers ("open").
- `layout_plan_reports_match_for_replay` :461: compares `offsets` as `Option<Vec<u64>>` equality, so two reports that both have `None` offsets match regardless of placements agreeing with a geometry; fail-closed only on mismatch. Not a wrong-accept by itself (placements still compared).
- Native fingerprint ignores `slot_application` :424-459, so two reports differing only in slot application hash the same ("traced" by reading; whether callers rely on it was not traced).

---------------------------------------------------------------------------
## 8. layout-plans/src/lib.rs (1598 lines, read in full)

**Purpose.** Crate root. Three inline modules do real work: `aggregate_geometry` (pack a tagged sum into size/alignment/offsets), `placement` (where a materialization may be written: address range, alignment, phase, machine regime, installation scope), `symbolic_values` (typed ids and field-path carriers for symbolic fields), plus `private_callback_placements` (validate that a callback pointer is stored either whole or as a complete set of non-overlapping bit fragments) and `target_facts::LayoutTargetFacts`. Everything else (`layout_reports`, `materialization`, `symbolic_materialization`) is a separate file. A `test_support` module is exported under `cfg(any(test, feature = "test-support"))` :1188.

**Shape.**
- Macros `numbered_constructor!` :33 and `numbered_variant!` :48 stamp a `member_identity` onto a base constructor.
- `aggregate_geometry` :60-172: `StorageGeometry`, `ConventionalSumGeometry` :68, `conventional_sum_geometry` :84, private `pack_fields_at` :128, `align_up` :161.
- `placement` :175-466: `ByteOrder`, `ConsumptionInstant`, `PlacementPhase {Build, Load, PostHandoff}`, `PlacementAddressRange::new` :206 (rejects empty/reversed), `MachineRegimeId`, `ArtifactInstallationScopeId` (non-zero ids), `PlacementConstraints` :267 (`new` :276 rejects alignment 0, `joined_with_layout` :327, `validate_site` :360), `PlacementSite`, `MaterializationContext` :412, `MaterializationWrite` :420, `StoredIntegerFit` :432, `MaterializationAction {ResolvedWrite, NativePointerRelocation, RuntimeWriter}` :439, `checked_lcm`/`gcd` :453-465.
- `private_callback_placements` :467-723: `validate_private_callback_placements` :470 -> `Vec<(start_bit, end_bit)>` destination ranges; `compose_private_callback_placements` :565 (shift every placement by `base`).
- `symbolic_values` :725-1139: ids `EntryStubId`, `DataSymbolId`, `RelocationTarget {Data, Entry}`, `SymbolicFieldPathSegment` :776 (field, member_identity, element_index, inner), `SymbolicFieldValue` :824, `SymbolicFieldInterior {Record, Sum}`, `SymbolicFieldInteriorLayout`, `SymbolicFieldInnerLayout` :917 with constructors and `from_recursive_sum_paths` :1013 / `fold_recursive_sum_paths` :1019, `nonzero_identity` :1127.
- Re-exports :1141-1183; `test_support` :1188-1564; `target_facts::LayoutTargetFacts` :1566-1593 with `validate` :1577.

**Inputs/outputs.** Value constructors and pure validators; `PlacementConstraints::validate_site` reads a `PlacementSite` and returns `Err(MaterializationDiagnostic)` or `Ok`.

**How it decides.**
- `conventional_sum_geometry` :84: error on empty case set, zero alignment, overflow. Tag is always at offset 0 (:116); common fields follow the tag (`pack_fields_at(common_fields, tag.byte_size)` :97); payload offset = common size aligned to max payload alignment (:103); storage size = end aligned to max(tag, common, payload alignment) (:111-120). Alignment is not required to be a power of two, only non-zero (:153-159).
- `validate_site` :360 compares phase equality (:365), `base_address % alignment` (:371), range containment (:377), machine regime and installation scope equality only when the constraint has one (:385-398, `is_some()` guards).
- `validate_private_callback_placements` :470: pointer size 1..=8 and power-of-two alignment (:477); whole-pointer shortcut only for exactly one `At` placement (:481); otherwise only `Bits` fragments are accepted and `IntegerAt` is refused with the message at :513 ("narrowed IntegerAt storage requires an address-fit contract"); source bits must tile `[0, pointer_size*8)` exactly (:543-557) and destination ranges must not overlap (:559).
- `fold_recursive_sum_paths` :1019 picks a constructor by (hop, interior, member identity presence) across 6 shapes (:1032-1110); depth limit :1064.

**Cases by shape.** `validate_private_callback_placements` comment (error text :513): "private callback slots require one whole pointer or a complete set of Bits fragments". Pointer size above 8 bytes is rejected :477. `IntegerAt` unsupported there.

**Same job elsewhere.** `conventional_sum_geometry` is called from three other places that each assemble sum layouts: `build-time-evaluation/src/layouts.rs:6975`, `terminal-psi/semantics/src/references.rs:2761`, `calling-conventions/src/aggregate_layout.rs:49` (grepped; not read). `validate_private_callback_placements`: 8 call sites in 6 files (`review-evidence/.../callbacks.rs:166,217`, `backend/layout/src/terminal_callback_layouts.rs:230,245`, `callback_catalog.rs:181,196`, `provider-planning/.../callback_layout.rs:227,241`, `private_callback_closure.rs:360`). `compose_private_callback_placements`: 5 call sites. `validate_fragment` (materialization.rs, in this unit) is reused here :519.

**Unused / one-caller code.** `derive_symbolic_materialization_with_inner_layouts` has its only external hit inside this crate (`symbolic_materialization.rs:37` calls it from `derive_symbolic_materialization`); grep over omega-rust excluding lib.rs/tests.rs found no outside caller. `SymbolicFieldInnerLayout::from_recursive_sum_paths`: no caller outside lib.rs/tests.rs in the grep output (0 outside; tests.rs use not counted). `LayoutTargetFacts`: 53 references in 17 files (used widely; `.validate()` call sites not separately grepped). The `test-support` feature is enabled by `access-plans/Cargo.toml:20`, `post-handoff-writer-plan/Cargo.toml:20`, `executable-installation/Cargo.toml:21`.

**Stated limits.** Header :13-21 states that rejection of out-of-range fragments and unresolved targets happens before mutating destination bytes (checked in materialization.rs, see below).

**Fail-open risks.**
- `PlacementConstraints::validate_site` :385 and :391: if the constraint has NO machine regime / installation scope, any site passes. This is the designed "optional constraint". `unconstrained(phase)` :297 builds exactly that (stated by the constructor name).
- `joined_with_layout` :343-356: the length-versus-range check only compares lengths (`byte_len > range_len`), not alignment of the first aligned address inside the range; the real check happens in `validate_site`. Not a wrong-accept at the end because `validate_site` repeats containment.
- `conventional_sum_geometry` accepts non-power-of-two alignments (:153) so a bad alignment 3 yields a result; callers' trust not traced ("open").
- `align_up` with the generic remainder method is correct for any non-zero alignment.

---------------------------------------------------------------------------
## 9. materialization.rs (2042 lines, read in full)

**Purpose.** Writes field values into byte buffers according to a `LayoutPlanReport` and reads them back: scalar fields (possibly split into bit fragments or stored as narrower integers), whole aggregate fields (byte blobs, possibly repeated array elements), and pre-resolved symbolic plans. It checks identities, coverage, overlap and bounds before and during the write.

**Shape (inline modules + top-level items).**
- `field_codec` :8-576: `update_scalar_field` :14 (stages on a copy, then copies back: atomic), `stage_scalar_field` :30 (writes in place, NOT atomic on error; the test at :369-375 shows `stage_*` leaves partial writes), `decode_aggregate_field` :57, `decode_aggregate_field_into` :74, private `DecodedAggregateField` :84 and `prepare_aggregate_field` :140. Tests :289-575.
- `field_identities` :577-834: `MaterializationFieldKey {Numbered(u64), Positional(String)}` :587, `materialization_field_key` :592 (identity if present else field name), `planned_entries_by_key` :602 (BTreeMap), `SuppliedFields` and `index_supplied_fields` :652, `validate_supplied_field_coverage` :686, `validate_materialization_field_identities` :723, `validate_conventional_sum_materialization_identities` :750, symbolic path display helpers :784-833.
- `field_values` :835-1019: `ScalarFieldValue` :852, `AggregateFieldValue<Bytes>` :883, `AggregateFieldSchema` :889 with `AggregateFieldShape {Whole, Repeated{element_byte_size, element_align, element_count}}` :897, `ScalarFieldSchema` :998; width validator `validate_authored_width` :839 (1..=64).
- `stored_integer_writes` :1020-1427: `ScalarFragment`, `scalar_fragment` :1037, `materializer_scalar_window` :1052, `stored_value_fits` :1130, `validate_stored_integer_fit(_shape)` :1206/:1236, `apply_write` :1268, `apply_fragment` :1288, `validate_write` :1323, `validate_fragment` :1351, `read_container` :1396, `write_container` :1409, `low_mask` :1420.
- Top level: `SymbolicMaterializationPlan {byte_len, byte_order, placement, actions}` :1446 with `materialize_resolved_into` :1454; `fixed_layout_byte_len` :1503; `materialize_scalar_layout_into` :1523; `materialize_aggregate_layout_into` :1560; `stage_aggregate_layout` :1580; `decode_scalar_layout` :1893; `MaterializationDiagnostic(pub String)` :2034.

**Inputs/outputs.** Reads a `LayoutPlanReport` (must have `size: Some(_)`, :1503-1521) plus field schemas and values; writes into `&mut [u8]`. All diagnostics are free-form `String`s wrapped in `MaterializationDiagnostic`.

**How it decides.**
- Field matching is by `materialization_field_key`: stable member identity when present, otherwise the field NAME (:592-600). Both supplied value lists and schemas are indexed twice, by name and by key (:652-684); duplicate names error, duplicate identities error.
- Placement legality is decided by `materializer_scalar_window` :1052: container width must be 8..=64 and a multiple of 8, width non-zero, source bits within the access width, destination bits within the container (:1085-1105).
- Stored integers: `stored_value_fits` :1130 compares the semantic value (sign-extended from `source_width_bits` for Signed, raw for Unsigned) against `stored_bounds`. `validate_stored_integer_fit_shape` :1236 requires source width 1..=64, stored width byte-sized and `<=` source width.
- `stage_aggregate_layout` :1580 decides per field: if any placement is `IntegerAt`/`Bits` and any is `At` -> error (:1647-1655); scalar-placed fields must be `Whole` shape and at most 8 bytes (:1657-1667); `At`-only fields with several placements must be a `Repeated` schema with matching count, constant stride >= element size, and offsets aligned to `element_align` (:1752-1817); every written byte range is checked against an `occupied` vector for overlaps (:1713, :1833, :1866-1877), and Bits fragments are checked per bit in a `BTreeMap<usize,u8>` (:1842-1887).
- `decode_scalar_layout` :1893 requires fragments to tile the source width exactly (`covered == low_mask(width_bits)` :2017) and rejects overlapping fragments (:1994).
- `materialize_resolved_into` :1454: refuses if any action is not `ResolvedWrite` (:1465-1473), validates all writes first (:1475-1485), then applies on a staged copy and copies back (atomic).

**Cases by shape.**
- `prepare_aggregate_field` :140: "scalar placements require one field of at most eight bytes" (:171-175); "whole-field and scalar placements cannot mix" (:176-181); multiple `At` placements need `Repeated` shape (:241-248).
- A field with exactly one `At` placement is treated as one whole chunk even when its schema is `Repeated` (:238-240 and :1752-1753): the element alignment and stride checks apply only when there is more than one placement.
- Scalar widths are capped at 64 bits (:843).
- `AggregateFieldSchema::new_repeated` requires a power-of-two alignment (:937), unlike `conventional_sum_geometry` (lib.rs:153).

**Same job elsewhere.**
- "identity else name" key: `materialization_field_key` :592 is the canonical one in this crate; `layout_reports.rs` repeats the rule in `member_sort_key` :505 and `hash_optional_member_identity` :406, and `symbolic_materialization.rs` calls this one.
- Field-identity consistency check appears twice here (`validate_materialization_field_identities` :723 for records and `validate_conventional_sum_materialization_identities` :750 for sums) plus `conventional_sum_member_identities_are_unambiguous` (layout_reports.rs:367) for replay.
- Bit-range validity is checked in three places: `materializer_scalar_window` :1052, `validate_fragment` :1351, and again by `apply_scalar_entry` (symbolic_materialization.rs:914) which calls both.
- The array stride/element checks are duplicated between `prepare_aggregate_field` :254-267 (decode) and `stage_aggregate_layout` :1787-1816 (encode). The decode copy also checks alignment before calling, the encode copy checks it per element later.
- `read_container`/`write_container` byte-order loops :1396-1418; `DecodedAggregateField::copy_into` :103 has its own endianness slicing.

**Unused / one-caller code (grep `omega-rust`, excluding `layout-plans/src/*.rs`).**
- `stage_scalar_field`: 1 caller (`terminal-interpreter/src/storage_providers.rs:1596`). `decode_aggregate_field_into`: 3 callers (same file :1616, :3380, :3536). `stage_aggregate_layout`: 1 caller (:4183). `validate_fragment`: 4 callers (`foundation_validation.rs:2856`, `post-handoff-writer-plan/src/lib.rs:244`, `isa-x86_64/.../post_handoff_writer.rs:49`, `isa-aarch64/.../post_handoff_writer.rs:48`). `validate_write`, `validate_write_source_value`, `validate_stored_integer_fit(_shape)`: callers only in `post-handoff-writer-plan/src/lib.rs` (the `foundation_validation.rs:4076` hit is a different local function of the same name).
- No callers outside layout-plans (same grep) for: `update_scalar_field`, `decode_aggregate_field`, `materialize_scalar_layout_into`, `materialize_aggregate_layout_into`, `decode_scalar_layout`, `apply_write`, `SymbolicMaterializationPlan::materialize_resolved_into`. These are all `pub` re-exports in lib.rs:1158-1171; they are exercised only by this crate's tests/internal calls (`decode_scalar_layout` is called by `prepare_aggregate_field` :220; `materialize_aggregate_layout_into` is used in tests only). I did not grep method-call syntax for `materialize_resolved_into` beyond the name match, which is a name match anywhere.

**Stated limits.** None in comments. Hard limits: 64-bit scalar carrier (:843, :1662 "past the 64-bit scalar placement carrier" message).

**Fail-open risks.**
- `stored_bounds` / `signed_bits` :1259-1265 / `low_mask` :1420-1425 use unchecked shifts (`1 << (width-1)`, `1 << width`); callers validate widths first (`validate_authored_width` 1..=64, `byte_sized_width`), so a zero or >64 width does not reach them in the paths read (traced for `validate_stored_integer_bits` :1144-1193 and `validate_stored_integer_fit` via `_shape`). `decode_scalar_layout` calls `low_mask(width_bits)` with `ScalarFieldSchema.width_bits` validated by `validate_authored_width` :1009.
- `stage_aggregate_layout` :1752-1753: a `Repeated`-shaped schema whose layout has exactly one placement is accepted as a whole field without checking `element_align` or count. Wrong program: array field planned with a single `At` entry for an array of N elements; bytes are copied as one block (still within `byte_len` and not overlapping). Not harmful to memory safety; semantic acceptance is "open".
- `decode_scalar_layout` returns values in `BTreeMap` key order (Numbered before Positional, then by name), not the order of `fields` :1928-2030; callers that zip by position would misassign. Reachable from `prepare_aggregate_field` which expects exactly one value :221 (traced), no other callers outside the crate.
- `stage_aggregate_layout` zero-fills `destination[..byte_len]` before validating the fragment checks that run later (:1633 vs :1843+); a failing call leaves the destination partially written (stated by the name `stage_*`; the atomic wrapper is `materialize_aggregate_layout_into` :1560 which stages into a temp vec).
- `unreachable!` and `expect` calls (:1481, :1494, :1553, :1640, :1725, :1750) rely on earlier validation within the same function.

---------------------------------------------------------------------------
## 10. symbolic_materialization.rs (942 lines, read in full)

**Purpose.** Given a layout, a list of symbolic field values (fields whose value is "the address of entry stub X / data symbol Y"), optional nested inner layouts, and a materialization context, work out exactly which bytes must be written where and whether each write can be done now (target resolved), must be a native pointer relocation, or must be a runtime (post-handoff) writer. Output is a `SymbolicMaterializationPlan` or a diagnostic.

**Shape.**
- `derive_symbolic_materialization` :31 (delegates with no inner layouts) and `derive_symbolic_materialization_with_inner_layouts` :46 (the main routine, about 480 lines).
- Private `PreparedInterior {Record{layout, planned, nested, nested_order, repetition}, Sum{layout, repetition}}` :531, `PreparedInnerLayout {byte_len, interior, path_display}` :546 with `has_no_storage` :553.
- `prepare_inner_layouts` :566, `prepare_record_interior` :704, `prepare_sum_interior` :761, `first_untraversed_inner_layout` :819, `select_materialization_entries` :840, `write_from_entry` :875, `pub(crate) apply_scalar_entry` :914.

**Inputs/outputs.** Reads `LayoutPlanReport`, `&[SymbolicFieldInnerLayout]`, `&[SymbolicFieldValue]`, `MaterializationContext`, and a `resolve: FnMut(RelocationTarget) -> Option<u64>` callback. Produces `SymbolicMaterializationPlan {byte_len, byte_order, placement, actions}`. `apply_scalar_entry` mutates a byte slice and is called from materialization.rs (:52, :1554, :1718, :1888).

**How it decides.**
- Duplicate detection: two sets keyed on (field name, element index) paths and on (identity-or-name key, element index) paths (:60-96).
- Path walking (:134-446): for each hop it looks the field up by `materialization_field_key` in the current planned map; the leaf hop emits writes from the selected entries; non-leaf hops require a prepared inner layout carrier for that key, compute the enclosing offset (repeated: min `At` offset + index*stride, :249-266; otherwise exactly one whole `At` placement, :274-298) and descend; sum interiors consume the next one or two hops as case/payload or common field (:332-441).
- Action choice per write (:479-519): target resolved -> `ResolvedWrite`; unresolved and `AfterOmegaHandoff` -> `RuntimeWriter`; unresolved before Omega entry -> `NativePointerRelocation` only for an `At` placement whose symbolic width equals `context.native_pointer_relocation_bits` (:488-498), else error (At with no matching width :499, IntegerAt :505, Bits :511).
- Resolved values are checked with `validate_write_source_value` for every write that shares the same target (:464-475), and `resolve` is called once per distinct target (cached in a `BTreeMap` :457-478).
- Every supplied inner layout must be traversed by some symbolic path unless it `has_no_storage` (:448-455, `first_untraversed_inner_layout` :819-838).

**Cases by shape.** Messages spell the supported shapes: a sum path must spell "a selected case and payload field or a common field" (:338); "a case payload is the leaf of a sum path" (:360); "a common field is the leaf of a sum path" (:406); sum payload fields cannot carry element indices (:363-367); inner sum interiors cannot carry nested record carriers (:613-617); repeated elements stride must be at least the element size (:619-626, :722-728). Nesting is bounded by `CONVENTIONAL_RECORD_PATH_DEPTH_LIMIT` (:65, :734).

**Same job elsewhere.** `lib.rs` `SymbolicFieldInnerLayout::fold_recursive_sum_paths` (lib.rs:1019) walks the same nesting with the same depth limit. Empty-storage logic (`has_no_storage` :553, plus :641-651 and :301-313) parallels the layout rules in `layouts.rs` of build-time-evaluation (not read). `select_materialization_entries` :840 duplicates the "sort `At` entries by offset" step also done at materialization.rs:1748 and :237.

**Unused / one-caller code.** Grep `derive_symbolic_materialization` (count mode, all of omega-rust): `layout-plans/src/tests.rs` 30, `symbolic_materialization.rs` 3, `lib.rs` 3 (re-export and `test_support`), `post-handoff-writer-plan/src/tests.rs` 6. So both `derive_symbolic_materialization` and `..._with_inner_layouts` have zero non-test callers anywhere in omega-rust; the only non-test consumer of the plan TYPE is `post-handoff-writer-plan/src/lib.rs` (imports `SymbolicMaterializationPlan`, :30). `apply_scalar_entry`: 4 in-crate call sites above.

**Stated limits.** Messages quoted above ("record path bound", "empty interior", "no native {}-bit pointer relocation").

**Fail-open risks.**
- `has_no_storage` interiors are skipped by `first_untraversed_inner_layout` :825-827 and writes into them are rejected only when they are the leaf (:186-193). A supplied empty inner layout is accepted without any symbolic path naming it. Wrong program: an unused empty inner layout carrier. Harmless by construction ("stated" by the error text "cannot write an empty interior"; accepting an unused empty carrier is by design as no bytes exist).
- Empty inner layout placement check :643-651 allows `offset <= enclosing_byte_len` (equal to the end is accepted), by design for zero-sized fields.
- `resolve` returning `Some(v)` for a target is trusted; its range check is `validate_write_source_value`, which only runs for writes carrying `stored_integer_fit` (materialization.rs:1195-1204); for `At` / `Bits` writes the value is masked (`apply_fragment` :1315) without a fit error. A symbolic value wider than the field width is silently truncated to `width` bits for those placements. Reachable: `At` placement with `width_bits` 64 and a 64-bit value fits exactly; a `Bits` fragment takes bits `[source_lsb, source_lsb+width)` of the value by design (a fragment, not truncation). Traced by reading :1195-1204 and :1315.

=========================================================================
# mutation-matrix crate (`omega-rust/psi/foundation/mutation-matrix/src/`)

---------------------------------------------------------------------------
## 12. mutation-matrix/src/lib.rs (113 lines, read in full)

**Purpose.** Test helper. Given a record family that carries "custody" data, it replaces one field at a time with a value from a foreign (donor) record and asserts the family's independent checker either rejects with a named error or replays to a custody that differs from the substituted one. Header :1-24.

**Shape.** Macro `custody_field_inventory!` :28 (gated `#[cfg(any(test, feature = "test-support"))]`, generates an enum plus `INVENTORY` slice); `MutationOutcome<E> {ExactError(E), RebuiltCustodyDiffers}` :48; `OneFieldSubstitutionMatrix<R,F,C,E>` :53 (9 fields: family, fields, honest, donor, custody, substitute, check, outcome, joined_replay); `run_one_field_substitution_matrix` :65 (not cfg-gated, uses `assert!`/`panic!`).

**How it decides.** Pure test assertions: donor custody must differ from honest :73-78; each substituted record must change custody :83-87; outcome per field is declared by the caller via `outcome(field)` :88.

**Unused / one-caller code (grep `omega-rust`, excluding the file).** `custody_field_inventory!`, `run_one_field_substitution_matrix`, `OneFieldSubstitutionMatrix`, `MutationOutcome::`: 11 hits in 3 files: `omega/foundation/optimization-core/src/optimization_core/mutation_matrix.rs` (2), `optimization-core/src/lib.rs` (2), `omega/compilation/backend/images/image/src/final_image.rs` (7). `Cargo.toml` of mutation-matrix declares `test-support = []` and optimization-core enables it (grepped `test-support`).

**Stated limits / fail-open.** None; panics on mismatch (test code). The runner is compiled into non-test builds (no cfg gate on the function :65) while the macro is gated; stated by structure only.

=========================================================================
# numerics crate (`omega-rust/psi/foundation/numerics/src/`)

---------------------------------------------------------------------------
## 13. bignum.rs (1520 lines, read in full)

**Purpose.** Exact arithmetic substrate: arbitrary-size signed integers (`BigInt`), exact rationals (`BigRational`), and exact floating values with specials (`ExactFloat`), all independent of any IEEE format. Plus correct rounding of a rational to binary32/binary64 under four rounding directions. Header :1-7.

**Shape.**
- `IeeeRounding {NearestTiesToEven, TowardZero, TowardPositive, TowardNegative}` :13.
- `BigInt {negative: bool, magnitude: Vec<u64>}` :21 (little-endian limbs, no trailing zero limb, zero is `negative=false`); methods `from_canonical_words` :43 (rejects non-canonical), `shl_bits` :60, `from_u64/i64/u128/i128` :86-124, `to_i64/to_u64` :126-147, `add/sub/mul` :163-199, `div_rem` :201 (returns `None` for zero divisor; power-of-two divisor fast path :205), `gcd` :275, `from_str_radix` :298, `from_decimal_str` :324, `normalized` :347; `Ord` :364; `Display` :375 (19-digit chunks).
- `BigRational {numerator, denominator, negative_zero}` :403 with `from_decimal_str` :418 (accepts sign, `.`, `e/E` exponent as i64), `add/sub/mul/div` :456-502, `cmp_value` :485, `negate` :527, `to_f32/to_f64(_with_rounding)` :537-551, `new` (reduces by gcd, normalizes sign) :553, `signed_zero` :574, private `to_ieee_bits` :580.
- `ExactFloat {Finite(BigRational), Infinity{negative}, NaN}` :673 with `from_decimal_str` :680 (accepts `NaN`, `nan`, `inf`, `infinity` spellings), `from_f32/from_f64` :689/:693 via `from_ieee_bits` :835, `add/sub/mul/div/negate` :697-772, `equal_value`, `partial_cmp_value` :782-807, `to_f32/to_f64` :809-833.
- Free functions: `pow10` :885, `rounded_scaled_quotient` :900, `rounding_overflows_to_infinity` :938, `cmp/add/sub/mul_magnitudes` :947-1018, `mul_magnitudes_by_u64` :1020, `div_rem_magnitudes` :1027 (schoolbook BIT-AT-A-TIME long division for multi-limb divisors :1043-1063), `shift_left_one`, `short_div_rem` :1079, `DecimalChunks` :1094. Tests :1126-1519.

**Inputs/outputs.** Pure value computation. Output of `to_ieee_bits` is the raw bit pattern (sign|exponent|fraction) as `u64`.

**How it decides.**
- Rounding: `rounded_scaled_quotient` :900 scales numerator/denominator by a power of two, divides, and picks the increment from the remainder and the direction (:923-931). Overflow policy by `rounding_overflows_to_infinity` :938. Subnormal and carry handling in `to_ieee_bits` :603-655.
- NaN: `ExactFloat::from_ieee_bits` :843-848 collapses every NaN (any payload, any sign) to one `NaN`.
- Equality: `ExactFloat::equal_value` :782 / `partial_cmp_value` :793 give IEEE-style unordered NaN; `BigRational` derives `PartialEq` over all fields including `negative_zero` :402, but `cmp_value` :485 compares by cross-multiplication and ignores the zero sign. So `==` and `cmp_value` disagree on `-0` vs `+0` (read).
- Add result sign for exact zero: `ExactFloat::add` :709-712 sets the zero negative only when BOTH operands are negative; no rounding direction is consulted (see float_semantics notes).

**Cases by shape.** `ExactFloat::from_decimal_str` :680-686 accepts exactly the listed NaN/inf spellings; everything else goes through `BigRational::from_decimal_str`, which requires ASCII digits only in `whole+fraction` (:432), so `1_000` or `0x10` is `None`.

**Same job elsewhere.** The IEEE decode/encode of binary32/binary64 by exponent/fraction widths here (`from_ieee_bits` :835 and `to_ieee_bits` :580) is parameterised on (exponent bits, fraction bits). `float_semantics.rs` (this unit) hard-codes the two formats again by comparing the whole `FloatFormat` record to `BINARY32`/`BINARY64` constants (:306-312). Decimal parse into numeric literals also exists in `numerics/src/literals.rs` (3 grep hits for BigInt parse functions; not read; "suspected" parallel).

**Unused / one-caller code.** Grep for BigInt/BigRational parse and conversion names across omega-rust excluding this file returned 98 hits in 32 files (including validation, proof, interpreter and 05 pipeline), so the type is widely used; exact per-function counts not taken (the pattern included `.to_i64()` which matches other types).

**Stated limits.** `// Little-endian limbs, no trailing zeros; empty is zero.` :23 (invariant comment). Header says proof engines and constant evaluation share this (:4-5); not grepped.

**Fail-open risks.**
- `BigInt::from_str_radix` :298 calls `char::to_digit(base)` which panics when `base > 36` (std behaviour); base is a caller-supplied `u32`. Callers not traced ("open").
- `BigRational::from_decimal_str` :418-450 and `pow10` :885: an exponent such as `1e1000000000` is accepted (i64 parse, `usize::try_from(scale)`), then `pow10` builds a number of ~3.3e9 bits (time and memory). No bound here. "open" (a literal with a huge exponent could stall or exhaust memory in constant evaluation; whether callers cap it was not traced).
- `div_rem_magnitudes` :1043-1063 is O(bits x limbs) with an allocation per bit (`shift_left_one` :1066); large divisors are slow (stated nowhere).
- `to_ieee_bits` :594 casts `bit_length()` (usize) to `i32` with `as`; values over 2^31 bits would wrap. Not reachable at realistic sizes.
- `rounded_scaled_quotient` :921 `expect("IEEE significand fits in one u64")` relies on the caller's exponent normalization (:594-601). Traced for the two call sites at :605 and :616.
- `sub_magnitudes` :977 only `debug_assert`s `a >= b`; callers `add` :171-182 and `div_rem_magnitudes` :1055-1056 compare first (read).

---------------------------------------------------------------------------
## 14. float_semantics.rs (2523 lines, read in full)

**Purpose.** Defines what each floating-point operation MEANS, independent of the host CPU: decode operands to exact values, do exact arithmetic, round once to the chosen format. It also defines the closed catalog of 65 named operations that toolchain source declares under `FloatSemantics::<name>`, each with an identity (row ordinal + digest) and a kernel that can be evaluated ("discharged") on checked operands. Header :1-9.

**Shape.**
- Formats: `FloatFormat` :25 (radix, precision, exponent limits, specials, rounds_to_nearest flag) with consts `BINARY32` :134, `BINARY64` :149; `FloatSpecialValues` :17; `RoundingDirection` :36; `FloatClass` :55; `IntegerFormat {bits, signed}` :64 with `bounds` :92 / `contains` :106.
- Values: `FloatMeaning {FiniteNonZero(BigRational), Zero{negative}, Infinity{negative}, NaN}` :166 with `from_f32/from_f64`, `to_f32/to_f64`, `to_interpreter_value` :227, private `from_exact`/`to_exact` :261-284.
- Operations: `FloatSemantics` (unit struct) :287 with `from_decimal` :290, `round_exact(_directed)` :297-313, `convert` :315, `add/subtract/multiply/divide/negate` :319-395, `rounded_binary!` macro rows :322-335 (12 directed add/sub/mul/div), `square_root*` :397-411 / :619-667, `multiply_then_add` :413, `fused_multiply_add*` :423-436, `classify` :438, `apply_trapping_policy` :486, `apply_saturating_policy` :494, `apply_saturating_divide_policy` :506, comparisons :519-547, `maximum_finite` :549, `minimum/maximum` :561-575, integer conversions `to_integer_exact/trapping/saturating` :348-371, `from_integer(_toward_zero)` :337-346.
- `correctly_rounded_square_root_bits` :681 (binary search over bit patterns).
- Catalog: consts `FLOAT_SEMANTICS_CATALOG_VERSION = 1` :725, `FLOAT_SEMANTICS_NAMESPACE = "FloatSemantics"` :726, `FLOAT_FORMAT_CORE_SOURCE = "float_format.omg"` :727; `IntegerCarrier` :731, `FloatSemanticValueKind` :785, `FloatSemanticContractIdentity {row, catalog_version, commitment}` :816, `FloatSemanticOperation` :823, `FloatSemanticKernel` (11 variants, function pointers) :885, `FloatSemanticComparison` :900, `FloatSemanticPredicate` :938, operands/results :864-882, `FLOAT_SEMANTIC_OPERATIONS` (65 rows, pinned by the test at :1968) :1262-1333, `FLOAT_NAMED_CALL_LEAVES` (30 names) :1335-1366, impl methods :1368-1518 (`from_source_identity` :1373, `names_a_row` :1387, `named_call_arity` :1392, `kernel_discharge` :1409, `integer_operand_image` :1431, `ordered_interval_predicate` :1462, `for_contract_identity` :1478, `ordinal` :1492, `contract_identity` :1500).
- Tests :1520-2523 (about 1000 lines; read in full).

**Inputs/outputs.** `kernel_discharge(arguments)` takes `FloatSemanticOperand`s and returns `Some(FloatSemanticDischarge{contract, result})` or `None`; `contract_identity` is a domain digest of (version, `toolchain::FloatSemantics::<name>`, parameter spellings, `->`, result spelling) :1500-1517 plus the row ordinal.

**How it decides.**
- Format: every format-taking function decides by `format == FloatFormat::BINARY32` / `== BINARY64` (whole-record equality) and `panic!("unsupported floating-point format record")` otherwise (:233, :311, :455, :557, :665). The record fields other than as identity are never used for computation.
- `kernel_discharge` :1409: argument count and each operand's kind must match (`matches_kind` :969: a Format operand must equal BINARY32 or BINARY64; an Integer operand must lie inside its carrier's bounds); the result kind must match the declared one :1422.
- Row identity: `from_source_identity` :1373 requires namespace == "FloatSemantics" and exact name + parameter kinds + result kind; spelling alone selects nothing (test :2004).
- `for_contract_identity` :1478: version must equal 1, row index must exist, and the recomputed identity must equal the presented one.
- Policies: `apply_trapping_policy` :486 errors on NaN or infinity RESULT only; `apply_saturating_policy` :494 clamps an infinite result to the maximum finite value of the same sign only if all operands are finite; `apply_saturating_divide_policy` :506 leaves the result untouched when the divisor is zero.
- Integer conversions: truncation toward zero then range check (:600-617); `to_integer_exact` and `to_integer_trapping` are the same function (:352, :359).
- `minimum`/`maximum` :561-575: `if less(left,right) {left} else {right}`, so with a NaN or equal operands the right operand is returned (tests :1604-1607 pin the NaN case).
- Square root: exact binary search over the finite bit patterns with exact rational squaring :681-723; ties-to-even by comparing `4*input` with `(floor+ceiling)^2`.

**Cases by shape.**
- Only two formats are supported (BINARY32/64); the table of rows has fixed arities `BINARY`, `UNARY`, `TERNARY`, `PAIR`, `SINGLE` :1155-1159.
- Eight integer carriers (i8..u64) for `from_integer`, `to_*`, `to_*_saturating` rows.
- `ordered_interval_predicate` :1462 / `FloatSemanticPredicate::ordered_interval` :953 only for the three predicates `is_finite`, `is_nan`, `is_infinite`.

**Same job elsewhere.**
- The same trapping/saturating policy application is repeated in other crates: checked-interpreter `execution_scalars.rs:1182-1184, 1525-1533`, `execution_expressions.rs:1106-1117`, terminal-interpreter `engine_execution.rs:3308`, `05_typed-trees-to-checked-trees/src/published_facts_crash_calls.rs:4371-4381` (grepped hits; not read).
- `FLOAT_SEMANTIC_OPERATIONS` rows vs `language-semantics/src/numeric_intrinsics.rs::FLOAT_OPERATIONS` (29 named rows + `negate` = the 30 names in `FLOAT_NAMED_CALL_LEAVES`): two parallel name lists kept consistent by a test at numeric_intrinsics.rs:606-648 (arity check only). A third catalog, `FloatProjectionOperation` (`numerics/src/lib.rs:1058`), is used by validation (`float_and_mathematical_contracts.rs`) and was not read ("suspected" same job).
- `from_integer`/`to_f32` roundings overlap with `BigRational::to_f32_with_rounding` (bignum.rs:545); this file delegates to it.

**Unused / one-caller code (grep over omega-rust excluding this file, content mode).**
- `to_interpreter_value`: 3 external callers (`program_values.rs:870`, `interpreter.rs:242`, `literals_and_struct_literals.rs:756`).
- `to_integer_trapping`: 2 external callers (`execution_scalars.rs:156`, `terminal-runtime/.../host_imports/numeric.rs:264`). `to_integer_exact` as the named function was not in my pattern; row dispatch uses it internally (:1074).
- `apply_trapping_policy` 5 callers (checked-interpreter x2 files, terminal-interpreter, published_facts_crash_calls); `apply_saturating_policy` 4; `apply_saturating_divide_policy` 3.
- `named_call_arity`: 1 external caller (`checked-interpreter/src/execution_expressions.rs:1056`) plus the language-semantics test.
- `integer_operand_image`: 2 external (`float_and_mathematical_contracts.rs:154`, `checks_operators.rs:656`). `ordered_interval_predicate`: 1 external (`checks_operators.rs:620`). `for_contract_identity`: 3 external (`operators_and_proof.rs:1919`, `proof_contracts_core.rs:10910`, plus in-file). `same_meaning_truth`/`unordered_truth`: 1 external region (`contract_entailment.rs:78248-78250`).
- `from_source_identity` (row version): `lowering.rs:2772`, `float_and_mathematical_contracts.rs:835`; `names_a_row`: `lowering.rs:2626`, `float_and_mathematical_contracts.rs:852`. (The `FloatProjectionOperation::from_source_identity` hits are a different type.)
- `FLOAT_NAMED_CALL_LEAVES`: no outside use except via `named_call_arity` (grepped the constant name: only this file).
- `FLOAT_FORMAT_CORE_SOURCE`: 6+ uses in `float_and_mathematical_contracts.rs`.

**Stated limits.** `panic!("unsupported floating-point format record")` repeated at :233, :311, :455, :557, :665; the numerics README (`numerics/README.md:15-16`) describes the policy functions. Comment `// Little-endian ...` is in bignum.

**Fail-open risks.**
- `ExactFloat::add` zero sign (bignum.rs:709-712) combined with `round_exact_directed` (:301-313): `add_toward_negative(1, -1)` reads as +0 here, whereas IEEE 754 roundTowardNegative gives -0 for an exact zero sum of opposite signs. Inferred from the code; no test in this file pins it (the only zero-sign test is add(+0,-0) = +0 under nearest, :2245-2266, and add(-0,-0) = -0, :1611-1617). Wrong programs: float code using `add_toward_negative`/`subtract_toward_negative` that observes the sign of an exact-zero result. "open".
- `square_root_directed` :619-667 for `NearestTiesToEven`: if the input is larger than `max_finite^2` for the format (only possible when a meaning wider than the format is passed), `decode(lower + 1)` decodes the infinity bit pattern and hits `unreachable!` (:648/:659). With in-format operands this does not occur. "open": mixed-format callers not traced.
- `kernel_discharge` returns `None` for out-of-range or non-finite integer results (`to_i8(300.0)` test :2334-2338). A `None` is "no discharge", not an accept (fail-closed).
- `FloatSemanticOperand::matches_kind` :971-973 accepts a Format operand only if it equals one of the two constants, so a modified record is refused at discharge (test :2510-2521) but PANICS in the direct `FloatSemantics::*` functions (:306-312).
- `from_decimal` :290 returns `None` on parse failure (fail-closed). `ExactFloat::from_decimal_str` NaN-spelling acceptance: any of 6 NaN spellings becomes the single `NaN` (bignum.rs:682).

=========================================================================
# layout-plans tests (listed here out of numeric order)

---------------------------------------------------------------------------
## 11. layout-plans/src/tests.rs (3046 lines, read in full)

**Purpose.** Unit tests of the layout-plans crate (header :1-5): fingerprints and replay matching, materialization into byte buffers, symbolic derivation with inner record/sum layouts, writer fragments and placement constraints.

**Shape.** Modules `conventional_materializations` :14-1304 (fingerprint order-independence :44, sum fingerprint binds ordinals/geometry/unnumbered names :66, replay by numbered identity :121, identity aliases rejected before work :242, aggregate and scalar materializers: atomicity, overlap, byte-order cases :337-1204, resolver-order tests :1207-1303), `symbolic_materializations` :1305-1848, `symbolic_sum_materializations` :1849-2701 (empty interiors :1866, depth bound :1954, numbered case/payload join :2031, malformed sum paths :2070, malformed carriers :2192, sum arrays :2318, recursive fold by identity :2439, depth bound of the fold :2545, record arrays :2597), `writer_fragments` :2702-2909 (constant folding :2712, stored-integer fit before each consumption phase :2735, loader-consumed fragments rejected :2800, plan with unresolved action cannot partially materialize :2819, placement constraints over all axes :2827), plus helper builders `nested_layout` :2917, `record_interior` :2945, `sum_field_layout` :2952, `recursive_sum_report` :2969, `sum_array_layout` :3015, `reject_inner_layout_materialization` :3031.

**What it pins (decisions).**
- Rejections happen before destination mutation and, for symbolic derivation, before any resolver call (`resolutions == 0` at :1233-1236, :1391-1394, :281). The resolver is called once per distinct target and ALL retained writes for that target are fit-checked before the next target (:1279, :1292).
- Identity aliasing (one stable identity spelled with two names, or one name with two identities) is an error in scalar, aggregate, decode and symbolic paths (:259, :269, :280, :312, :469, :1027, :1048, :1060).
- A numbered field renamed in source rejoins by identity; an unnumbered field renamed does not (:194-238, :1000-1015).
- Empty-storage interiors are allowed without a write path but malformed ancestors/descendants still reject (:1866-1951).
- Depth bound 64 for path hops, carrier trees and report folds (:1954-2028, :2545-2594).

**Same job elsewhere.** `test_support` in lib.rs :1188-1564 provides many of the builders; the `nested_layout` (here :2917) and `deeply_nested_layout` (lib.rs :1322) overlap in shape; `sum_layout()` (lib.rs :1364) is shared.

**Unused / stated limits / fail-open.** Test code. Not applicable; none seen. Note: the tests are the only callers of `normalized_conventional_sum_layout_report_fingerprint`, `conventional_sum_layout_reports_match_for_replay`, `materialize_scalar_layout_into`, `materialize_aggregate_layout_into`, `decode_scalar_layout` (as public API), `derive_symbolic_materialization`, `derive_symbolic_materialization_with_inner_layouts`, `from_recursive_sum_paths` and `materialize_resolved_into` (counts in sections 7-10).

=========================================================================
# Closing sections

## Coverage

Command basis: every file was read with the Read tool in consecutive ranges with no gaps (offset/limit pairs chained: declaration_selection.rs 1-1000, 1001-2000, 2001-2903; language-semantics lib.rs 1-1300, 1301-2524; numeric_intrinsics.rs 1-775; service_surface.rs 1-444; language-semantics tests.rs 1-1007; type_identity.rs 1-1321; layout_reports.rs 1-547; layout-plans lib.rs 1-1000, 1001-1598; materialization.rs 1-1000, 1001-2042; symbolic_materialization.rs 1-942; layout-plans tests.rs 1-1000, 1001-2000, 2001-3046; mutation-matrix lib.rs 1-113; bignum.rs 1-800, 801-1520; float_semantics.rs 1-850, 851-1700, 1700-2523).

- Read in full: all 14 files, 21305 lines total (2903+2524+775+444+1007+1321+547+1598+2042+942+3046+113+1520+2523).
- Skimmed: none.
- Not read: none.

Grep caveats: several grep counts in this record are lower bounds or name matches only (marked in text). Broad greps that matched generic method names (`.to_i64()`, `.scalar_type()`) were discarded as noise and are not relied on.

## Five things a maintainer most needs to know about this unit

1. `language-semantics/src/declaration_selection.rs` is five different subsystems in one 2903-line file (provider-choice algorithm :138-778, Build request/operand vocabulary :782-1212 and :1766-1882, a 16-arena static-operand graph :1214-1575, the target catalog :1576-1651, and the occurrence ledger :1661-2479). The ledger is append-only with occurrence id = row index; `LateBound` rows are finalized exactly once and prefix rebasing refuses anything but LateBound -> Resolved/Intrinsic differences (:2143-2213).
2. Identity is by stable member number when present and by field NAME otherwise, in layout-plans (`materialization_field_key` materialization.rs:592, repeated in layout_reports.rs:406 and :505). Renaming a numbered field is presentation only; renaming an unnumbered field changes fingerprints and replay matching. The same crate rejects identity/name aliasing everywhere before writing.
3. Large parts of layout-plans' public surface have no production caller in omega-rust: `derive_symbolic_materialization(_with_inner_layouts)`, `materialize_scalar_layout_into`, `materialize_aggregate_layout_into`, `decode_scalar_layout`, `update_scalar_field`, `decode_aggregate_field`, `apply_write`, `SymbolicMaterializationPlan::materialize_resolved_into`, the sum fingerprint/replay pair and `from_recursive_sum_paths` (greps in sections 7-10). The interpreter uses only `stage_scalar_field`, `decode_aggregate_field_into` and `stage_aggregate_layout`; the post-handoff writer plan uses the validators.
4. Float semantics decides the format by whole-record equality with `BINARY32`/`BINARY64` and panics on any other record (float_semantics.rs:306-312 etc.); only the catalog discharge path (`kernel_discharge` :1409) refuses instead of panicking. The 65-row operation catalog's row index is part of each contract identity (:1500-1517), so row order is a compatibility surface; the same float name list also lives in `numeric_intrinsics.rs` (29 rows + `negate`) and in `FloatProjectionOperation` (not read).
5. `type_identity.rs` is a validating parser for canonical type-identity strings that never builds a tree; "unknown package" is decided by the visitor supplied by the caller (`review-evidence/.../membership/visitor.rs`, the only non-test caller), and `toolchain-source-owner` digests are accepted without being reported to the visitor (type_identity.rs:550-555).

## What I am unsure about

- Whether the three non-reporting accepts are intended: `toolchain-source-owner` not visited (type_identity.rs:550-555); `embedded_name` acceptance (:463-471, depends on caller); `selected_application_slots` returning the subject when no slot matches (declaration_selection.rs:158-160). I did not read the visitor or callers.
- Whether the `add_toward_negative` / `subtract_toward_negative` exact-zero sign (IEEE: -0) is handled elsewhere before reaching these functions; I read only this crate and found no direction-aware zero sign.
- Whether the many other trapping/saturating policy sites (checked-interpreter, terminal-interpreter, published_facts) share code with `apply_*_policy` or reimplement it; I saw only the call lines.
- Exact external caller counts for several `pub` items (BigInt functions, `ProviderIdentityTable::id_at`, `ValueDomain`, `LayoutTargetFacts::validate`) were not taken; broad patterns were noisy.
- Whether `choose` (declaration_selection.rs:169) "slot with candidates but no keyed covering candidate" (:375 `[] => {}`) is reported as unresolved by the caller in `provider-selection/src/choice/selection.rs`; not read.
- The suspected parallel implementations (conventional sum layout calculation in `build-time-evaluation/layouts.rs`, `terminal-psi/.../references.rs`, `calling-conventions/aggregate_layout.rs`; `FloatProjectionOperation`; `typed-trees/src/type_identity.rs`) were identified by grep only.
