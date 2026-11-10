# Unit 84 notes

Files covered (all under `omega-rust/psi/compilation/representations/`):
1. terminal-psi/src/boundary.rs (1-1838)
2. terminal-psi/src/control_flow.rs (1-2736)
3. terminal-psi/src/identity.rs (1-532)
4. terminal-psi/src/lib.rs (1-1172)
5. terminal-psi/src/observation.rs (1-1307)
6. terminal-psi/src/ownership.rs (1-1144)
7. terminal-psi/src/proof.rs (1-808)
8. terminal-psi/src/terminal_module.rs (1-1288)
9. terminal-psi/src/types_and_values.rs (1-2763)
10. terminal-psi/work/src/lib.rs (1-437)
11. terminal-psi/work/tests/meter.rs (1-451)
12. tokens/src/lib.rs (1-476)
13. tokens/src/token_kind.rs (1-300)
14. typed-trees/src/access_views_and_commitments.rs (1-2521)

(Paths below are relative to `omega-rust/psi/compilation/representations/terminal-psi/src/` unless stated.)

---

## 1. boundary.rs (1838 lines, all read)

**Purpose.** Data declarations (no pipeline logic) for the boundary half of a `TerminalModule`: boundary machines (provider/callable declarations the program calls across a boundary), external provider conformances, closed conformance applications, dynamic dispatch tables, provider candidate/refinement rows, closed reach (service reach) applications, and service declarations. Plus a handful of validation/matching helpers and two hash/commitment encoders. The file is a sequence of private-ish `pub(crate) mod` blocks (`applications` 6, `conformances` 181, `declarations` 598, `external_providers` 1143, `non_observation` 1258, `numeric_intrinsics` 1358, `dynamic_dispatch` 1405, `providers` 1510, `reach_applications` 1601, `services` 1723, `result_regions` 1781) re-exported flat at 1747-1779, 1255, 1404, 1838 (read).

**Shape.**
- `BoundaryTypeIdentity(TerminalIdentityIndex)` 13; `BoundaryOperatorRequirement {declaration, overload: String, package}` 26, ctor rejects empty overload 38; `BoundaryApplication::{Empty, Exact(Vec<BoundaryApplicationArgument>)}` 72, `exact()` 78 requires non-empty and arguments in exact binder-ordinal order 82-88; `BoundaryApplicationArgument::{Type, Const}` 101; `TerminalBoundaryApplicationDemand` 149; `BoundaryApplicationCandidate` 62 (carries a `CheckedMachineDeclaration` body).
- `ClosedConformanceApplication` 186 (owner, telescope, trait identity, schemas, static_applications, rows, `report_fingerprint: u64`, `commitment`), `ClosedConformanceSchema` 209, `ClosedConformanceRow` 265, `ClosedConformanceStaticApplication` 223. Encoders: `closed_conformance_application_bytes` 289 (private), `closed_conformance_application_report_fingerprint` 351 (FNV-1a), `closed_conformance_application_commitment` 499 (domain digest `...closed-conformance-application.v7\0`).
- `BoundaryMachineDeclaration` 766 (id, `declaration: Option<CheckedBoundaryCallable>`, `numeric_intrinsic`, `role`, `kind`, `completion`, `unconditional_termination`, `identity`, `owner`, `attachment`, `parameter_order`, `scalar_parameters`, `structural_parameters`, `crash_routes`, `result: BoundaryMachineResult`, `result_reference_regions`, `requires`, `normal_qualifications`, scalar domain/proposition requirements, `reference_outputs`, `program_local_root_introductions`, `content_guarantees`, service reach fields). `BoundaryMachineResult::{Unit, Scalar(QualifiedScalarType), Structural(BoundaryStructuralResultDeclaration)}` 632. `CheckedBoundaryExecution` 804 (Provider, AccessInitialize/MoveOut/Relocate/View, EmptyRunRelease, four StaticInterrupt* variants).
- `ExternalProviderConformance` 1167 with `validate()` 1181. `TerminalDynamicDispatchCatalog {interfaces, tables}` 1413 with `interface/table/row` 1497-1507. `ProviderCandidateConformance` 1539, `ProviderSignature`, `ProviderRefinement` 1586. `ClosedReachApplication` 1607 etc. `ServiceDeclaration` 1740. Trait `ReferenceResultShape` 1789 implemented for `BoundaryMachineDeclaration` 1799 and `TerminalDynamicSlot` 1818.

**Inputs/outputs.** Pure data plus functions taking `&TerminalIdentityTable`, `&[StructuralDomainDeclaration]`, `&[StructuralTypeDeclaration]`. Nothing mutated. Encoders produce bytes/u64/`[u8;32]`.

**How it decides.**
- `BoundaryMachineResult::accepts_provider_result` 639: for structural results, keyed on structural_type id + multiplicity equality (648-649), provider qualifications being a subset of the declared ones (650-653), and for every declared qualification a matching `StructuralDomainDeclaration` (by id and carrier) whose `missing_membership_requirements` against the provider's qualifications are each either without `establishment_routes` or authorize this boundary requirement/declaration (654-673), and projected qualification rows contained (674-677). Non-structural: falls to exact `matches_machine_result` 679, 683. `matches_operation_result` 699 uses exact equality of qualifications and projected rows.
- `requires_checked_result` 880: returns false unless result is Structural (884-886); otherwise true if any result qualification's domain has no declaration (`is_none_or` 944) or its `required_domain_closure` is `None` (947), or some closure node has a content_projection other than itself (949-950) or (not consumed by an owned parameter) has establishment_routes and does not authorize this boundary (951-959). Keyed on domain ids and identity indices.
- `has_valid_parameter_order` 966: counts only.
- `requires_checked_non_observation` 1267: true if any structural parameter has `WriteOnlyBorrow` access (1271-1276) or, walking structural children of parameter types (visited set, 1283-1285), any non-erased field/Reference/ElementView/ByteSequence carries WriteOnlyBorrow (1305-1333). `FixedArray` of literal length 0 is not followed (1334-1336). A type id for which `lookup` returns `None` is skipped (1288-1290).
- `ExternalProviderConformance::validate` 1181: rejects when identity text is absent/empty via `TerminalIdentityTable::text` (1183-1187), numeric intrinsic without schema (1188), zero contract commitment (1217), zero specialization commitment (1229), empty VtableField/TableFunction field (1243-1249). Returns `Err(&'static str)`.
- `validate_numeric_intrinsic_signature` 1360: returns Ok when `boundary.numeric_intrinsic` is None (1365-1367). Otherwise checks role, completion NormalReturn, no attachment, no structural parameters, scalar parameter count/types equal to the intrinsic signature, and result type (1371-1399). The FloatClass result is matched via `is_float_class_shape` on the declaration found by id (1390-1395).
- `ClosedReachArgument::same_identity_as` 1694: compares identity *text* across two identity tables, both nonempty and equal (1716-1719); Machine arguments also require equal contract commitments (1713).
- `CheckedBoundaryCallable::same_identity_as` 861 delegates to `CheckedMachineDeclaration::same_identity_as` and compares execution and method_arguments.
- `TerminalDynamicSlot::matches_operation_result` 1454: result matches plus sorted claim paths equal `result_claim_paths`.

**Cases by shape.** `program_local_root_introduction_compatibility_report_identity` 1044 hashes `ContentProjectionScalar` variants with a fixed tag list 1060-1102 and `ContentProjectionExpression::{IntervalSet, CountedQuantity}` 1126-1139; `unreachable!()` at 1096 in a nested match. `callable_result_bytes` 363: `unreachable!("address carriers are unsigned")` 382 for (Address, Signed). `ClosedConformanceCallableResult::Scalar` only encodes Boolean/Integer/IeeeFloat (372-396). `BoundaryReferenceDestination` has only `Parameter(u32)` 748.

**Same job elsewhere.** Three separate FNV-1a/byte-hash encoders in this one file: `semantic_vocabulary::Fnv1a` at 355, an inline hand-rolled FNV (offset 14_695_981_039_346_656_037, prime 1_099_511_628_211) at 1050-1104, and `crate::digest_encoder::DomainDigestEncoder` at 503 (read). `matches_machine_result` 683 and `matches_operation_result` 699 are the same comparison over two result enums (`TerminalMachineResult` vs `OperationResult`). `ReferenceResultShape` impls 1799 and 1818 are near-identical bodies.

**Unused / one-caller / "shared".** Counts (Grep count over `omega-rust`, name alternations; counts are lines with any name so approximate per file): `find_boundary_machine(` 20 occurrences/15 files (several outside this file); `ReferenceResultShape` 7 occurrences/3 files (this file 4, `semantics/src/references.rs` 1, `semantics/src/descriptors.rs` 2); `accepts_provider_result|requires_checked_non_observation|validate_numeric_intrinsic_signature|program_local_root_introduction_compatibility_report_identity` 21 occurrences/9 files; `BoundaryApplicationCandidate|TerminalBoundaryApplicationDemand` 57 occurrences/14 files. No item here has zero outside users.

**Stated limits.** None in comments (file has only the header comment 1-4).

**Fail-open risks.**
- `identities.text(...).unwrap_or_default()` in the encoders 301, 339, 345, 419: a missing identity index hashes as the empty string, so two applications whose identities are missing hash equal (traced: input only from the table the application was built against; no check here). Status: open.
- `requires_checked_non_observation` 1288-1290: unknown structural type is skipped (treated as not requiring non-observation). Status: open (callers pass a lookup over the module's structural types; not traced).
- `accepts_provider_result` 654-673 `missing_membership_requirements(...).is_some_and(...)`: `None` makes the domain fail (fail-closed). `requires_checked_result` 944/947 uses `is_none_or`, so missing domain declaration or missing closure leads to returning `true` (requires checking: fail-closed direction).
- `validate_numeric_intrinsic_signature` 1365: boundary with `numeric_intrinsic == None` returns Ok without checking anything (by design: nothing to validate).
- `ExternalProviderConformance::validate` `match` 1243-1251: all other `ExternalBindingIdentity` variants return Ok without checking (open).

---

## 2. control_flow.rs (2736 lines, all read)

**Purpose.** Defines the executable body of a Terminal Psi machine: `TerminalMachine`, `Block`, `Operation`, the very wide `OperationKind` enum, `Terminator`, ranking witnesses for termination, atomic and register access events, float result policy, trapping integer operations, and target-machine family rows. Almost all code is data plus accessor/remap helpers. Header comment 1-5.

**Shape.** `pub(crate) mod` blocks re-exported flat at 2717-2736: `atomic` 7, `float_result_policy` 226, `invocations` 286, `machines` 626, `operations` 842, `ranking` 1967, `record_field` 2066, `register_access` 2093, `scalar_case_field` 2206, `target_families` 2216, `termination` 2250, `trapping_integer` 2630.
- `TerminalMachine` 638: id, `declaration: Option<CheckedMachineDeclaration>`, `parameters`, `parameter_order`, `structural_parameters`, `ranked_scc: Option<TerminalRankedScc>`, `result`, `structural_places`, `entry_claims`, `block_entry_claims`, `loan_completions`, service reach fields, `closed_reach_application`, content claim/reshuffle/composition tables, `entry`, `blocks: Vec<Block>`, `contract: MachineContract`.
- `Block` 665 (id, params, erased formals, structural params, `operations`, `terminator`).
- `Operation` 916 (`static_reach_binding`, `suspension_crossing`, `id`, `result: OperationResult`, `kind: OperationKind`); `OperationResult::{Unit, Scalar, Structural}` 941; `StructuralOperationResult` 964.
- `OperationKind` 1042-1571: about 110 variants (counted by reading, not by a command) covering task-register/segment/GDT service ops (1043-1079), references (1080-1091), activation claims (1095-1121), structural/record/case establishment, byte-sequence and element-view ops (1135-1309), calls (`CallDynamic` 1319, `Call` 1332, `CallUnit` 1340, `CallStructuralScalar` 1350, `CallStructural` 1360, `CallStructuralWithScalarArguments` 1368, `BoundaryCall` 1379), port/register access, integer/float/boolean ops (1395-1561), `AtomicAccess`/`AtomicFence` 1562-1570.
- `Terminator` 2261: `InvokeNoNormal`, `ExternalCompletion`, `Jump`, `Conditional`, `StructuralCase`, `Return`, `ReturnUnit`, `ReturnStructural`, `Crash`. `SuccessorEdge` 2594, `StructuralCaseSuccessorEdge` 2605.
- Ranking: `TerminalRankedScc::Natural(Vec<TerminalNaturalCycle>)` 1972; `TerminalNaturalCycle {observations: Arena<TerminalIntegerRankValue>, ranks, edges}` 1977; manual `PartialEq` comparing spans by contents 2009.
- `TargetMachineCallable::{Machine, Boundary}` 2221, `TargetMachineSupply` 2227, `TargetMachineFamily {canonical, head_contract, members}` 2244.
- Invocation views: `BoundaryInvocation` 296 (built from `Block::boundary_invocations` 316, which yields one per `BoundaryCall` op plus one for an `ExternalCompletion` terminator), `ModuleInvocationInputs` 375 via `OperationKind::module_invocation` 387 and `Terminator::module_invocation` 488.

**Inputs/outputs.** Reads `semantic_vocabulary` ids, `language_core` enums, `crate::StructuralArgument`, `CheckedMachineDeclaration`, etc. Mutating helpers: `map_scalar_uses` (OperationKind 1601, Terminator 2505, atomics 130, float policy 272, trapping 2686, register event 2199), `map_structural_uses` 2385, `structural_arguments_mut` 1863.

**How it decides.**
- Atomic ordering legality `ordering_is_legal` 90: Load/Store use `valid_for_load/store`, CompareExchange uses `failure.valid_compare_exchange_failure(success)`; RMW and Swap always legal (94).
- `FloatResultPolicy::accepts_operand_types` 259 keyed on operand scalar type lookup (raw result must be `IeeeFloat(format)`, saturating `eligible` must be Boolean).
- `TerminalNaturalCycle::rank_values` 1984: all rank vectors must be non-empty, same length as the first block's signature, each element non-address integer of the same `integer_type` (1990-1994). `None` if no ranks (`ranks.first()?` 1988).
- `module_invocation` 387 returns `None` for any OperationKind not in the five static call variants (472); note `CallDynamic` and `BoundaryCall` are excluded.
- `Terminator::edge` 2471 panics for Conditional/StructuralCase (2481).

**Cases by shape.**
- `OperationKind::map_scalar_uses` 1601-1831 is an exhaustive `match` with no wildcard (all variants listed, with a block of no-op variants 1802-1829) so new variants fail to compile; `module_invocation` 387 uses `_ => return None` (472).
- Five call variants carry overlapping field sets: `Call` has no structural arguments (1332), `CallStructural` has no scalar arguments (1360); `static_call_structural_argument_variants!` 1874 lists the five variants that carry `structural_arguments` (CallUnit, CallStructuralScalar, CallStructural, CallStructuralWithScalarArguments, BoundaryCall); `call_structural_argument_variants!` 1896 adds CallDynamic. `claim_transfers()` 1842 lists CallUnit/CallStructuralScalar/CallStructural/CallStructuralWithScalarArguments/CallDynamic (not BoundaryCall).
- `RegisterAccessEvent::inputs` 2147 returns at most two operands (array of two options).
- `expect_scalar` 1014 panics "operation has no scalar result".

**Same job elsewhere.** Operand remapping is implemented once per carrier: `AtomicAccessEvent::map_operands` 130, `FloatResultPolicy::map_operands` 272, `TrappingIntegerOperation::map_operands` 2686, `RegisterAccessEvent::map_operands` 2199, `OperationKind::map_scalar_uses` 1601, `Terminator::map_scalar_uses` 2505. `operands()` getters exist for atomics 112, float policy 250, trapping 2672 but `OperationKind` has no single `operands()` here (suspected elsewhere; not grepped). Pair-pattern macros (1927-1965, 2614-2628, 2705-2714) exist to avoid repeating or-patterns. `OperationKind::module_invocation` 387 and `Terminator::module_invocation` 488 repeat the same eight-field tuple. `Terminator::successor_edges` 2561, `successor_targets` 2585, `edges` 2486, `successor_block_arguments` 2539 and `cleanup_rosters` 2427 each re-enumerate the terminator variants.
- `TerminalMachine::structural_parameter` 808, `parameter` 817 (alias) and `signature_structural_parameter` 820 are three lookups over the same tables; `parameter` forwards to `structural_parameter`.

**Unused / one-caller / "shared".** Grep count of `\.signature_structural_parameter\(|\.expect_scalar\(\)|\.direct_callee\(\)|\.constructor_arguments\(\)|successor_block_arguments\(` : 86 occurrences/21 files (all five names together; not split per name). The ten exported pattern macros (names in the `call_structural_argument_variants!|integer_arithmetic_operations!|...` alternation): 93 occurrences in 36 files (including this file's definitions and `work/src/lib.rs` 8). `TargetMachineFamily\b`: 26 occurrences in 12 files. No evidence in this unit of zero users.

**Stated limits.** None in comments inside this file.

**Fail-open risks.**
- `TerminalNaturalCycle::rank_values` 1984 returns `None` for mismatched shapes (fail-closed for callers that require `Some`; callers not read).
- `module_invocation` 387 `_ => return None`: a future call-like variant that is not added here is silently not an invocation (traced only to this file; open for callers).
- `Terminator::storage_uses` 2328 lists projections for Jump/Invoke/External/Conditional/StructuralCase/ReturnStructural and cleanup actions; `Return`, `ReturnUnit`, `Crash` give none (2372); `StructuralCase` per-case `structural_arguments` are not included in `storage_uses` (2360-2365 only pushes source/path) although `map_structural_uses` 2414-2421 does remap them. Observed difference between the two functions; effect on callers not traced (open).
- `OperationKind::map_scalar_uses` 1610-1612: `ReleaseActivationClaim`/`MergeActivationClaim`/`EstablishNativeCodeLayout` have no scalar operands (traced: fields shown at 1095-1121 hold only structural args and claim ids). `ActivateTaskRegister` maps only `selector` (1766) and not `entries` (u16 constant) - fine.

---

## 3. identity.rs (532 lines, all read)

**Purpose.** Defines how Terminal Psi names things: an interned string table (`TerminalIdentityTable`) indexed by `TerminalIdentityIndex(u32)`, nominal declaration identities, the checked machine declaration record, the semantic fingerprint (32 bytes) and the vocabulary marker. Header (1-5) also names `TerminalArtifactManifest` and `OptimizationReceipt`, but neither is defined here: `TerminalArtifactManifest` is at `codec/src/sections.rs:65` (grepped `struct (TerminalArtifactManifest|OptimizationReceipt)`), and `OptimizationReceipt` has exactly 1 occurrence in the whole of `omega-rust`, this header comment (grepped, count).

**Shape.**
- `NominalDeclarationIdentity(TerminalIdentityIndex)` 11, `same_identity_as` 22.
- `CheckedMachineDeclaration` 44 (machine, state, semantic_declaration, parameter_types, callable, result_type, `contract: [u8;32]`, `specialization: Option<CheckedMachineSpecialization>`), `same_identity_as` 77, `callable_evidence_identity` 56 (formats `specialized-machine|callable=<len>:<text>|application=<hex>` when a specialization commitment exists). `CheckedMachineSpecialization` 137, `CheckedMachineCapture` 146 with `same_identity_as` 153.
- `SemanticFingerprint([u8;32])` 202; `TerminalPsiIdentity {vocabulary_marker, program_fingerprint}` 230.
- `TerminalIdentityIndex(pub u32)` 240 with `ABSENT = Self(0)` 249 and `Default` = ABSENT 242. `TerminalIdentityTable {entries: Vec<String>, invocation: Option<InvocationCodeScope>, invocation_local: Vec<TerminalIdentityIndex>, positions: memo}` 257. `TerminalIdentityTableBuilder` 374 (`intern` 423, `intern_invocation_local` 403, `for_invocation` 389, `build` 451, `snapshot` 447).
- `VocabularyMarker` 503 is a unit struct whose `get()` returns the constant `134` (519); `new(raw)` accepts only 134 (509).

**Inputs/outputs.** Table is built by interning strings; the lookup `index_of` 343 lazily builds a `HashMap<String,u32>` in a `OnceLock` (344-350), invalidated on `intern` by `positions.0.take()` (431). `Record::write` 469 refuses to serialize a table that has an invocation scope or local entries (473-476); `Record::read` 480 requires entry 0 to be the empty string and re-interns, failing on repeats or empty entries (484-494).

**How it decides.** Identity equality is by text, per owning table, never by index:
- `NominalDeclarationIdentity::same_identity_as` 22: if either side is invocation-local, defers to `same_invocation_declaration` 311 (both local, text equal, same `Arc` scope pointer via `InvocationCodeScope::eq` 292-296). Otherwise requires non-empty text on the left and text equality on the right (33-39). Absent (index 0) or out-of-range indices have `text` empty/None and compare false (test 190-194).
- `CheckedMachineDeclaration::same_identity_as` 77 compares text of semantic_declaration, parameter types, callable, result type (89-98) and the raw 32-byte `contract`; note these text comparisons at 89-98 use plain `!=` on `Option<&str>`, so two missing/out-of-range indices (both `None`) compare equal and two empty strings compare equal (unlike the `.filter(non-empty)` rule used for the machine/state identities and the specialization template fields at 114-124). Specialization requires equal commitment, matching capture presence/identity, and equal non-empty template callable/semantic declaration text and template identity.
- Display of `SemanticFingerprint` is lowercase hex (222-224).

**Cases by shape.** None (no fixed program-shape lists).

**Same job elsewhere.** `ClosedReachArgument::same_identity_as` (`boundary.rs:1694`) and `CheckedBoundaryCallable::same_identity_as` (`boundary.rs:861`) repeat the cross-table "non-empty text equal" rule; `CheckedMachineCapture::same_identity_as` 153 repeats it a third time (read). The `positions` HashMap is built in two places: `index_of` 344-350 and `into_builder` 359-365 (read).

**Unused / one-caller / "shared".** `\.same_identity_as\(`: 38 occurrences in 15 files (Grep count). `callable_evidence_identity`: 4 occurrences in 4 files (this definition + `06_checked-trees-to-lowered-psi/src/proofs.rs`, `semantics/src/calls.rs`, `codec/src/foundation_validation.rs`). `for_invocation\(\)|intern_invocation_local|is_invocation_local|same_invocation_declaration`: 30 occurrences in 8 files. `index_of` has the same name on table and builder (not grepped separately).

**Stated limits.** Error strings: "portable Terminal identities cannot contain invocation-local declarations" (408), "invocation-local identity cannot leave its retained owner" (475). Comment at 517-518 under `VocabularyMarker::get`: "Explicit Id formation retains its checked mathematical signature and subject telescope independently of executable comparisons." (the comment explains the constant 134 only by this sentence; read).

**Fail-open risks.**
- `CheckedMachineDeclaration::same_identity_as` 89-98: type/callable/semantic text compared with `!=` on `Option<&str>`; two indices that are both out of range or both empty compare equal. A declaration whose result_type or parameter type index is ABSENT in both tables would be considered the same on that field. Status: open (not traced to producers; `ExternalProviderConformance::validate` in `boundary.rs:1183-1220` separately requires non-empty text for those fields, but that is only for external providers).
- `TerminalIdentityTable::text` returns `None` for out-of-range and `Some("")` for ABSENT; `NominalDeclarationIdentity::same_identity_as` treats both as "not equal" (fail-closed).

---

## 4. lib.rs (1172 lines, all read)

**Purpose.** Crate root of `terminal-psi`. Declares the modules, re-exports, and defines several small data schemas inline: prepared roots, callback application, installed interrupt registration, calling ABI/policy declarations, native layout catalog, unoptimized-behavior capture, callback declaration roster, target layout applications, and direct-call accessors. Comments at 3-27 describe the crate as a "pure data-schema crate" separate from the codec and semantics crates.

**Shape.**
- Modules: `artifacts`, `boundary`, `control_flow`, `identity`, `observation`, `ownership`, `proof`, `terminal_module`, `types_and_values` (29-39; `artifacts` is in another file, not in this unit). `terminal_module::*` is glob re-exported at 958.
- `prepared_roots` 59: `PreparedNominalRoot` 82 (`new` 90 interns one declaration string; `resolve` 109 finds the unique machine whose `declaration.machine` is `same_identity_as`; errors `NominalRootError::TerminalEntryMultiplicity(n)` 70), `TerminalRootDeclaration::{Missing, Available, Unavailable}` 142, `PreparedTerminalRoots` 157 with `new` 169 and `retained_roots` 200.
- `callback_application::TerminalCallbackApplication` 339; `installed_interrupt::TerminalInstalledInterruptRegistration` 354.
- `calling_policies` 373: `CallingAbiDeclaration` 382 with `identity()` 401 (builds `semantic_vocabulary::CallingAbiIdentity`, validates, compares compact id), `CallingAbiStorage` 438, `CallingPolicyRequest` 461, `CallingSignatureTelescope` 482 etc.
- `native_layouts` 511: `NativeLayoutCatalog {records, fields, realizations}` arenas 520, `for_target` 636.
- `unoptimized_behavior` 669: `UnoptimizedMachineBehavior::capture` 710.
- `callback_declarations` 795: roster, use and requirement declarations with `Default` impls that hold `Err("... has no ...")` strings (820-829, 854-866, 884-901).
- `target_layouts` 960: `TargetLayoutApplication` 979, `TargetLayoutInput`, `TargetLayoutRealization` 1033, `StructuralRecordStorage` 970.
- `direct_calls` 1062: `direct_call_parts` 1083, `call_structural_arguments` 1154.

**Inputs/outputs.** `resolve` and `retained_roots` read `TerminalModule.machines` and `module.identities`. `capture` reads a `TerminalMachine` and returns an operations list. No mutation except `NativeLayoutCatalog::insert` 615.

**How it decides.**
- `PreparedTerminalRoots::new` 169: a request with an `Available` declaration is downgraded to `Unavailable("checked declaration has no exact package or toolchain owner")` if machine, state, semantic_declaration, or specialization template/semantic declaration is invocation-local (176-193). Keyed on identity index membership in `invocation_local`.
- `retained_roots` 200: exactly one machine whose declaration `same_identity_as` the root's; zero or more than one gives an `Err(String)` (221-226).
- `NativeLayoutCatalog::for_target` 636: if `policy_identity` is ABSENT and realizations exist, error (642-646); returns `Ok(None)` for natural layouts. Otherwise selects realizations whose `target` is `None` or whose `facts.target == target` (string compare, 648-653); exactly one required. A realization with `target: None` matches every target (so a record with one target-less realization plus one for the requested target errors with "multiple").
- `UnoptimizedMachineBehavior::capture` 710: records direct calls, boundary calls, instruction-service ops (a fixed list of six op kinds, 729-737), `InvokeNoNormal`, `ExternalCompletion`, `Crash`, and `InvokeNominal` cleanup actions. Keyed on variant.

**Cases by shape.** `direct_call_parts` 1083-1143: the five call kinds, `_ => return None` (1143). `DirectCallRole` 1067 maps `Call` -> Scalar (1089), `CallUnit` -> Unit, `CallStructuralScalar` -> Scalar, both `CallStructural*` -> Structural. `call_structural_arguments` 1154 handles `BoundaryCall` and `CallDynamic` itself and delegates the other five to `direct_call_parts`. `capture` only records the six service kinds at 729-737 plus `RegisterAccess` with `service: Some`; other `RegisterAccess` (service None) is not recorded.

**Same job elsewhere.**
- `direct_call_parts` 1083 vs `OperationKind::module_invocation` (`control_flow.rs:387`), `OperationKind::structural_arguments` (`control_flow.rs:1833`, via macro at 1874) and `direct_callee` (`control_flow.rs:1574`): all four enumerate the same five direct-call variants and extract (callee, arguments, structural_arguments). Read.
- `call_structural_arguments` 1154 vs `OperationKind::structural_arguments` 1833 (macro `static_call_structural_argument_variants!`, 5 variants) vs `call_structural_argument_variants!` (6 variants, `control_flow.rs:1896`): same set (CallUnit, CallStructuralScalar, CallStructural, CallStructuralWithScalarArguments, BoundaryCall, CallDynamic) three ways.
- `UnoptimizedMachineBehavior::capture` 710 also walks `direct_callee`, `cleanup_rosters`; `Block::boundary_invocations` (`control_flow.rs:316`) provides the boundary part of the same walk.
- `PreparedNominalRoot::resolve` 109 and `PreparedTerminalRoots::retained_roots` 200 are the same "find the unique machine by declaration identity" with different error types.

**Unused / one-caller / "shared".** `direct_call_parts\(|call_structural_arguments\(`: 12 occurrences in 7 files. `UnoptimizedMachineBehavior|UnoptimizedBehaviorOperation`: 41 occurrences in 7 files (this file 12, `artifacts.rs` 10, `executable_scopes.rs` 11). `PreparedNominalRoot|PreparedTerminalRoots`: 34 occurrences in 11 files. `NativeLayoutCatalog|\.for_target\(`: 31 occurrences in 16 files (the `.for_target(` alternation also matches unrelated methods). No zero-user finding.

**Stated limits.** The crate comment 12-20 states the crate is isolated so representation is "free of serialization cursors and verification engines". The test at 923-951 forbids fields named `identity: String`, `identity: Option<String>`, `identity: Vec<String>`, `identities: Vec<String>`, `identities: HashSet<String>`, `identities: BTreeSet<String>` across the crate source (`ALLOWED` list empty 930); it matches those literal text patterns only (read). Note several structs in this file still carry string-typed identities under other field names: `TerminalCallbackApplication.requirement: String` 343, `calling_owner_application: String` 344, `TerminalInstalledInterruptRegistration.registrar_application: String` 361 and `registrar_arguments: Vec<String>` 363 (read).

**Fail-open risks.**
- `CallbackRequirementDeclaration::default` / `CallbackUseDeclaration::default` / `CallbackEntryResources::default` 820-901 carry `Err(...)` defaults (fail-closed), but `requirement: Handle::default()` 891 and `CallbackDeclarationRoster::declaration` 904 does `self.requirements.get(usage.requirement).is_boundary` (910); an out-of-range handle's behavior depends on `Arena::get` (not read; open).
- `NativeLayoutCatalog::for_target` 642-646 returns `Ok(None)` when `policy_identity` is absent and there are no realizations: records with no policy are accepted as natural layout. Stated by the error text at 644. Traced: only reached through catalog rows inserted by `insert` 615.
- `UnoptimizedMachineBehavior::capture` is observation only, no accept/reject.

---

## 5. observation.rs (1307 lines, all read; lines 699-1294 are the in-file `#[cfg(test)] mod tests`)

**Purpose.** Describes a "trace profile" (what a run of the program is allowed to show an outside observer: root signature, crash sites, boundary calls, port writes, register accesses, external completions) and a builder that checks a runtime trace against that profile event by event. Header (1-4) also names `CrashCause`, `ObservationProfile`, `FloatContract`; `CrashCause` lives in `semantic_vocabulary` (re-exported at `control_flow.rs:2325`) and `ObservationProfile`/`FloatContract` appear nowhere else in `omega-rust` (grepped `ObservationProfile|FloatContract`: only this header line).

**Shape.** One `pub(crate) mod profile` 6-1295, re-exported 1297-1307. `TerminalObservationSchema::TerminalTraceV1` with `version()=2` 27-31 and `from_version` 33. Schemas: `TerminalTraceScalarSchema` 47, `TerminalTraceStructuralSchema` 53, `TerminalTraceResultSchema` 63. Profile rows: `TerminalTraceRootRow` 70, `...CrashSiteRow` 78, `...BoundaryCrashSiteRow` 86, `...OperationCrashSiteRow` 96 with `TerminalOperationCrashDenotation::{Integer, FloatNonFinite}` 105, `...OrdinaryEventRow` 134 (kinds BoundaryCall / PortWrite / RegisterAccess 117), `...ExternalCompletionSiteRow` 145; `TerminalTraceV1Rows` 156 and `TerminalTraceV1Profile` 166 (Deref to rows 172). Values: `TerminalTraceScalarValue` 224, `TerminalTraceStructuralValue {opaque_identity, structural_type, qualifications, path}` 244, `TerminalTraceV1Event` 269, `TerminalTraceV1Outcome` 279, `TerminalTraceV1RuntimeTrace` 312. Builder `TerminalTraceV1TraceBuilder` 408 with `push_event` 414, `finish_return` 437, `finish_edge_crash` 445, `finish_operation_crash` 476, `finish_boundary_crash` 511, `finish_prefix` 568. Private validators `validate_scalar_value` 582, `validate_structural_value` 608, `validate_result` 648, `validate_event_arguments` 672.

**Inputs/outputs.** Reads the profile (immutable borrow); produces `TerminalTraceV1RuntimeTrace` or `TerminalTraceV1ConstructionError` (20 variants, 326-398). `Display` for the error is `{self:?}` (402).

**How it decides.** Event sites are looked up by linear scan of `ordinary_events` / `crash_sites` / `operation_crash_sites` / `boundary_crash_sites` keyed on the id triple (machine, block, operation or edge) (200, 456, 487, 520-535). Scalar values: rejects any schema whose scalar type is an address-carrier integer (589-593), requires exact scalar type equality (594), and that the integer value is admitted by its type (600-604). Structural values: rejects non-empty `projected_qualifications` in the schema (615-617), type id equality (618), qualification list strictly increasing (624-630), every schema qualification present in the value (631-641), and path empty (642-644: "whole root" only). Crash outcomes: declared cause must equal actual (462, 497). Boundary crash: site must exist and the exact route (`CrashRouteBucket` equality) must be declared (520-543), then actuals are validated against the ordinary event row for the same operation (544-557).

**Cases by shape.** Structural trace values are accepted only as whole roots with no path and no projected qualifications (`NestedStructuralValue` 369, `UnsupportedProjectedStructuralSchema` 360, checked 615 and 642); address-sized integers are refused entirely (589). `validate_result` 668 ends with `unreachable!("equal result kinds pair the same variants")`.

**Same job elsewhere.** `validate_scalar_value`/`validate_structural_value` duplicate type/qualification rules found in the interpreter and validators (suspected: not read). `finish_edge_crash` 445 and `finish_operation_crash` 476 are the same lookup-and-compare over two row types.

**Unused / one-caller / "shared".** Builder entry points `begin_trace\(|finish_prefix\(|finish_boundary_crash\(|finish_operation_crash\(|finish_edge_crash\(|finish_return\(|TerminalTraceV1TraceBuilder`: 41 occurrences in 2 files: this file 38 (definitions and its own tests) and `psi/interpreter/terminal-interpreter/src/engine_terminators.rs` 3 (Grep count). `external_completion_sites|TerminalTraceExternalCompletionSiteRow|TerminalTraceV1Outcome`: 17 occurrences, all in this file (Grep count). The three error variants `UnknownExternalCompletionSite`, `ExternalCompletionContractMismatch`, `UnsupportedExternalCompletionSchema` appear only at their definition (`observation.rs:327-336`, grepped, 3 hits total): never constructed. `TerminalTraceV1Outcome::ExternalCompletion` (279-287) has zero constructions anywhere in `omega-rust` (grepped `TerminalTraceV1Outcome::ExternalCompletion`, no hits) and the builder has no `finish_external_completion` method; `TerminalTraceExternalCompletionSiteRow` rows exist in the profile but no code in this file reads them (read 156-163, 408-580).

**Stated limits.** None as comments; the in-code limits are the error variants above.

**Fail-open risks.** None seen that accept a bad trace on a `None`. One observation: `validate_scalar_value` 586-588 and `validate_structural_value` 612-614 `match schema.comparison { ExactSemanticValue => {} }` is an exhaustiveness hook with a single variant (traced: `TerminalTraceValueComparison` has one variant 42-44).

---

## 6. ownership.rs (1144 lines, all read)

**Purpose.** Vocabulary for who owns and may touch storage: multiplicity (Unrestricted/Affine/Linear), access capabilities (Owned/SharedBorrow/MutableBorrow/WriteOnlyBorrow), borrow-restoration rows, claim records, affine cleanup actions, access-view/lifecycle placement plans, and suspension (async carry) call plans with a digest. Header 1-5.

**Shape.** `pub(crate) mod` blocks: `access` 7, `borrows` 96, `claims` 219, `cleanup` 253, `placement` 309, `suspension` 591; re-exports 1121-1144.
- `StructuralAccess::admits(self, granted)` 62 and `admits_referent` 81 (two small truth tables over the four access kinds); five `#[macro_export]` or-pattern macros 23-59.
- Borrow rows: `TerminalReborrowRestoredCallUse` 153 (22 fields), `RetainedBorrowCustody` 179, `TerminalBorrowPlace` 147.
- Claims: `LoanCompletion` 224, `EntryClaim` 233, `ClaimTransfer` 240, `CompletionReceipt` 247 (the last two have identical field sets).
- Cleanup: `TerminalAffineCleanupAction::{DiscardRoot, DiscardResidual, InvokeNominal, RetireHome}` 277 with `path()`, `invoked_machine()`, `place()`.
- Placement: `TerminalAccessViewInput` 315, `TerminalAccessViewPlan` 343, `TerminalAccessLifecyclePlan` 355, `TerminalAccessStorage` 386 with `same_custody` 408, `owner_storage_path` 432, `owned_content` 459; `TerminalAccessLifecycleOperation::profile_operands` 499 (static table); `canonical_access_view_identity` 334 (`format!("access-view:{}:{policy}:{}:{schema}", len, len)`).
- Suspension: `TerminalSuspensionCallPlan` 613, `TerminalSuspensionLoans` (five arenas) 624, `TerminalSuspensionLoan` 633, `suspension_frontier_commitment` 781 (domain `terminal-suspension-frontier-v9\0`, 785) with private helpers `hash_handle` 889, `hash_span` 894, `hash_loans` 899, `hash_access` 1008, `hash_loan_path_segment` 1020, `hash_scalar_type` 1066, `hash_carry_policy` 1091, `hash_argument` 1107.

**Inputs/outputs.** Pure data; commitment function reads `TerminalIdentityTable` and a plan, returns `[u8;32]`.

**How it decides.**
- `StructuralAccess::admits` 62-79: `Owned.admits(_)` is `false` (77); `SharedBorrow` admits Owned/Shared/Mutable; `MutableBorrow` admits Mutable or Owned (via `mutating_access_pair_pattern!` 46-51); `WriteOnlyBorrow` admits Owned/Mutable/WriteOnly. `admits_referent` 81-93: Shared admits Shared/Mutable referents, Mutable admits only Mutable, WriteOnly admits Mutable/WriteOnly, Owned none.
- `TerminalAccessStorage::same_custody` 408: compares carrier, base, length, granted, vacant, resident and owner after erasing the `singleton` role of `Run` owners (409-422).
- `owner_storage_path` 432: for `Run` owners finds the record type and field by id; returns `None` if the type is not a Record or the field is missing (441-446).
- `owned_content` 459: `Err("owned access lifecycle lost its exact content qualification roles")` unless both `vacant` and `resident` are `Some`.
- Commitment hashing keys on ids and on identity text (`hash_loan_path_segment` 1031-1036 hashes the field identity text via `identities.text(*field).unwrap_or_default()`; a missing identity hashes as empty string).

**Cases by shape.** `profile_operands` 499-529 is a fixed table: Initialize {storage 0, profile 2, occupied false}, MoveOut {0,1,true}, View {0,1,false}, Relocate two operands {0,2,true} and {1,3,false}. `TerminalSuspensionCallTarget` has Machine/Boundary/Dynamic hashed with tags 1, 2, 5 (790-803; tag 3,4 unused in this function). `hash_carry_policy` 1091 reads four fields of `CarryPolicy` as booleans.

**Same job elsewhere.** Three byte-hash encoders for scalar types exist: `hash_scalar_type` 1066-1089 here, `callable_result_bytes` `boundary.rs:363`, and (suspected, not read) others in `codec`. Three copies of the structural-path-segment tag table: `hash_loan_path_segment` 1020 here (tags 1-8,10), plus codec wire encoding (suspected). `ClaimTransfer` 240 and `CompletionReceipt` 247 are field-for-field identical (read). `TerminalSuspensionPlace::visit_value_ids` 754, `TerminalSuspensionCallPlan::visit_value_ids` 858 repeat the "visit RuntimeIndex segments" loop four times (read 758-761, 862-866, 871-875, 879-883).

**Unused / one-caller / "shared".** `canonical_access_view_identity|suspension_frontier_commitment`: 22 occurrences in 4 files (`retention.rs` 3, `access.rs` 1, `module_wire.rs` 14, this file 4). The five access/multiplicity pair macros (`reading_borrow_pair_pattern!` ...): 210 occurrences in 62 files (Grep count; the count includes this file and widely used). `\.admits_referent\(|\.same_custody\(|\.owner_storage_path\(|\.profile_operands\(|\.owned_content\(`: 55 occurrences in 17 files. No zero-user finding.

**Stated limits.** None.

**Fail-open risks.**
- `admits` 62: `MutableBorrow` admits `Owned` grant (46-51); read as "an owned value may be lent mutably" (inferred; no comment).
- Commitment: `identities.text(..).unwrap_or_default()` at 1033 (open: unknown field identity hashes as empty; two plans differing only in missing field identities collide).
- `TerminalSuspensionLoan::default` 644 uses `StructuralAccess::SharedBorrow` and `Handle::invalid()`; defaults of `Unresolved` for capture/backing/resource (traced to the Default impls 646-656, 661, 674, 704). Whether validators reject `Unresolved` is not in this unit (open).

---

## 7. proof.rs (808 lines, all read)

**Purpose.** Plain data for the proof side of a Terminal module: content-conservation claim records, machine contracts (requires/ensures/crash routes), proposition/evidence declarations, proof-output call rows, recursion (termination) components, scalar block invariants, and float-meaning proof values. Header (1-4) names `TheoremObligation`, `ObligationPhase`, integer certificates; `TheoremObligation|ObligationPhase` occurs once in all of `omega-rust` (this header; grepped count 1), so those names are not defined here or elsewhere under those spellings.

**Shape.** `pub(crate) mod` blocks: `content` 6, `contracts` 175, `declarations` 313, `operation_crash_contracts` 434, `outputs` 447, `recursion` 508, `scalar_block_invariants` 578, `values` 596; re-exports 769-808.
- `MachineContract` 183 (id, crash_routes, erased formals, `requires`, `source_checked_requires`, `ensures: Vec<ContractClause>`, `outcome_specific_ensures`), `MachineContract::empty` 195. `CrashRouteBucket {cause, alternatives: Vec<CrashRouteGuard>}` 283, `CrashRouteGuard::{Truth, Predicate(CrashPredicateTerm)}` 289. `OutcomeSpecificCallEvidence` 232 (13 fields), `OutcomeSpecificEnsure` 216.
- `PropositionDeclaration` 318, `PropositionApplicationIdentity` 348, `EvidenceInterfaceIdentity` 379, `EvidenceContractLane` 418, `EvidenceArgumentSubject::{Parameter, Result, Closed, Unknown}` 399.
- `ProofOutputCall` 453, `StaticRequirementDispatch` 465.
- `TerminalProofRecursiveComponent` 570 with ranking relation `StructuralSubterm`/`StructuralSubtermCycles` 513.
- `FloatMeaningSource` 709 (ten variants), `FloatMeaningProjection` 740, `float_meaning_equality_proposition_id(index)` 754 (domain digest `omega.terminal_psi.float_meaning_equality_proposition.v1`, first 8 digest bytes OR 1, 760-765).
- `ContentTransformationDerivation::map_places` 114 (the only logic of size): rewrites place roots in conservation terms, bindings, boundary requirement substitutions and address transports, then re-sorts (144-171).

**Inputs/outputs.** Data only; `map_places` mutates in place with a caller closure.

**How it decides.** No accept/reject logic. `map_places` rebuilds `ContentConservation::new(algebra, left, right)` (144-148) and sorts children/inputs/outputs for canonical order (139, 161, 163, 168, 170-171).

**Cases by shape.** `FloatMeaningSource` enumerates ten fixed sources (709-720); `FloatMeaningProjectionOperation` has only `Meaning32`/`Meaning64` (621-624); `ProofOnlyValueType` has one variant `FloatMeaning` (627-629). `TerminalProofRankingRelation` has two relations (513-516).

**Same job elsewhere.** `ClaimTransfer`/`CompletionReceipt` style duplicates (`ownership.rs:240,247`); `CrashRouteBucket` comparisons in `observation.rs:530-535`. Two identically-shaped small structs `ContentIdentityReshuffle` 27 and `ContentEntryClaim` 20 differ only by the `output` field (read).

**Unused / one-caller / "shared".** `float_meaning_equality_proposition_id|FloatMeaningEqualityProposition|ProofPropositionId`: 42 occurrences in 11 files (proof.rs 7). `\.map_places\(|ContentIdentityReshuffle|ContentPartitionComposition` over `omega-rust/psi`: 48 occurrences in 14 files (mixed names; `.map_places(` alone not separated). Two empty inherent impls: `impl ContentIdentityReshuffle {}` at 34 and `impl ContentPartitionComposition {}` at 53 (read; zero methods).

**Stated limits.** None.

**Fail-open risks.** None seen (no decision code). `float_meaning_equality_proposition_id` maps the index into 63 bits of a digest so distinct indices could in principle collide (inferred from 760-765; not traced).

---

## 8. terminal_module.rs (1288 lines, all read)

**Purpose.** Defines `TerminalModule`, the root struct of the Terminal Psi IR (about 56 public tables, 1053-1106), plus small submodules that hold a few other schemas: folded constant closure, proof application scope, scalar applications, scalar conditions, quotients, task activation demands. Re-exports almost every other file in the crate (`pub use boundary::*`, `identity::*`, `observation::*`, `ownership::*`, `proof::*`, `values::*`, `task_activation::*`).

**Shape.**
- `TerminalModule` 1053: fields include `identities: TerminalIdentityTable`, `entry: Option<TargetMachineCallable>`, `structural_types`, `structural_domains`, `boundary_machines`, `machines`, `provider_candidates`, `external_providers`, `closed_conformance_applications`, `suspension_call_plan_count: u32` plus `suspension_call_sites` and `suspension_call_plans`, `native_layouts`, `task_activation_demands`. Constructors `for_entry` 1225, `declarations` 1232 (all-empty).
- Lookups on `TerminalModule` 1108-1223: `callback_application` 1109 (binary search by `application_index` in `callback_applications`, then in each `installed_interrupt_registrations[..].callbacks`), `calling_abi_identity` 1126, `blocks` 1147, `operations` 1151, `machine` 1157 (linear scan), `calling_abi_signature` 1161 (binary search by `id`), `unoptimized_machine_behavior` 1171, `service` 1180, `structural_type` 1184, `foreign_reference_descriptor` 1191, `structural_domain` 1207, `boundary_machine` 1216 (linear scan).
- `FoldedConstantClosure` 19 with `validate_plan_inputs` 40 and `validate` 75.
- `ProofApplicationScope` 207 (arenas of declarations, callable headers, associations, recursive components) with `callable_declaration` 233, `declaration` 252; `ProofCallableHeader::default` 188 holds `Err("checked callable header is absent")` etc.
- `ScalarApplications` 266 (`insert` 341 refuses `ScalarApplication::Missing` sentinel 345 and numbers from `len+1`), `ArrayExtent::{Literal, Application}` 299.
- `ScalarConditions` 435 with `retain_range` 624, `retain_predicate` 538, `retain_completion` 514; `ScalarPredicateCondition::evaluate` 476.
- `quotients` 798: `TerminalQuotientCatalog` 805. `task_activation` 848: `TerminalTaskActivationDemand` 951 (25 fields).
- Inline structs 1017-1050: `TerminalMathematicalCarrier`, `TerminalIdentityFormation(s)`, `TerminalIndexedTheorem`.

**Inputs/outputs.** `FoldedConstantClosure::validate(&TerminalModule)` reads `module.identities`, `module.provider_candidates`, `module.target_machine_families` and returns `Result<(), &'static str>`. Other methods are lookups.

**How it decides.**
- `validate_plan_inputs` 40: closure target must equal the requested target string (45); the set of plan digests for dependencies on that target must equal the set of digests of the supplied preimages, with no repeats (48-71) (digest is `DomainDigestEncoder::new(b"")` over the preimage, 62-64).
- `validate` 75: no dependencies requires target `None` (76-82); otherwise target non-empty (83), dependencies strictly increasing (86), each dependency target non-empty, plan not all-zero, requirement identity text non-empty (89-98); if a `candidate` is present, exactly one `provider_candidates` row matches (boundary, machine, provider_declaration, source_requirement_identity) (102-116); and no `target_machine_families` family that contains that callable lacks a member for `dependency.target` (117-129). A dependency with `candidate: None` is skipped with `continue` (99-101).
- `retain_range` 624: rejects inverted required bounds (631-635), absent steps, inverted captured bounds, stack underflow, final depth other than 1 (636-667). Keyed on a postfix step stack depth.
- `retain_predicate` 538: rejects Missing/Unknown nodes (548-555), computes kind per node in insertion order (556-566), requires root valid and Boolean (568-578). The `value_kind` closure for leaves always answers `Integer` (564).
- `ScalarPredicateCondition::evaluate` 476: Missing observation gives `None` (488), Literal gives integer, other observations must return an Integer `ScalarValue` or `None` (490-493); the `&|_, _| true` third callback passed to `evaluate_nodes` at 495 (meaning not visible here; open).
- `foreign_reference_descriptor` 1191: if the referent is a declared structural type, answer true only for `ByteSequence`/`ElementView` shapes; otherwise look the id up in `structural_referents` and return its flag; `None` if unknown.
- `callback_application` 1109: relies on both `callback_applications` and each `callbacks` Vec being sorted by `application_index` (binary search); sortedness is not checked here.

**Cases by shape.** `ScalarApplication` has only `LayoutObservation` and `Evaluate` besides `Missing` (273-286); `LayoutMeasurement` only Size/Alignment (289-292); `ScalarRangeRequirement::{Fixed, AddressModel}` 407-410. `TerminalQuotientAdaptationLicense::{Mint, Input, Result}` 830-834.

**Same job elsewhere.**
- `TerminalModule::boundary_machine` 1216 (linear find by id) and `boundary::declarations::find_boundary_machine` (`boundary.rs:1037`) do the same job. Counts: `\.boundary_machine\(` 77 occurrences in 35 files; `find_boundary_machine\(` 20 occurrences in 15 files (Grep counts).
- `proof_scope.declaration` 252 and `ScalarApplications::declaration` 357: same "1-based id to arena slot" translation (`checked_sub(1)` then `storage_slice().get`), plus `ScalarApplications::insert` 348-352 computing the next id as `len+1`.
- `blocks()` 1147 / `operations()` 1151 here and `TerminalMachine::operations` (`control_flow.rs:770`).
- `suspension_call_plan_count` 1097 duplicates `suspension_call_plans.len()`: set once at `06_checked-trees-to-lowered-psi/src/retention.rs:5080` (`u32::try_from(plans.len())`), serialized at `codec/src/module_wire.rs:4949`, initialised to 0 at `terminal_module.rs:1277` and `06_checked-trees-to-lowered-psi/src/attached_unit.rs:17291` (grepped).

**Unused / one-caller / "shared".** `\.callback_application\(|\.foreign_reference_descriptor\(|\.calling_abi_identity\(|\.unoptimized_machine_behavior\(`: 32 occurrences in 19 files (names combined). `validate_plan_inputs|FoldedConstantClosure|\.retain_range\(|\.retain_predicate\(|\.retain_completion\(`: 43 occurrences in 9 files (names combined). `TerminalModule::declarations`/`for_entry` not grepped.

**Stated limits.** Doc comments 1023-1024 ("These rows grant no identity witness and allocate no runtime value copy") and 1034-1035 ("One shared checked signature avoids repeating it for every Id occurrence."). `ScalarApplication::Missing` / `ScalarRangeObservation::Missing` etc are arena-sentinel variants (default) and are rejected at insert (345, 554, 640, 648).

**Fail-open risks.**
- `FoldedConstantClosure::validate` 99-101: a dependency whose `candidate` is `None` passes without any association or target-scope check. Status: open (not traced; the preceding checks 89-98 still apply).
- `FoldedConstantClosure::validate` 117-129 only errors when a family has the callable but no member for the dependency's target; a callable in no family is accepted (traced to the loop: no family match gives no error).
- `callback_application` 1109-1124: binary search over unsorted data returns `None` or a wrong row without error (open; not traced to where sorting is established).
- `ScalarPredicateCondition::evaluate` third callback `&|_, _| true` 495 (open).

---

## 9. types_and_values.rs (2763 lines, all read; tests at 1427-1502 and 2248-2504)

**Purpose.** Declares the type and place vocabulary of Terminal Psi: structural types (records, sums, arrays, byte sequences, views, atomic cells), field types, domain (qualification) declarations with their required-membership closures, scalar qualification catalog and ranges, structural path segments, operation-to-place helpers (which places each `OperationKind` reads or writes), reference origins, and machine/parameter/result signatures. Two top modules: `types` 7-1525 and `values` 1527-2763.

**Shape.**
- `types::atomic_outcomes` 9: `atomic_once_outcome_cases` 16 (returns the three-case sum only when the declaration is canonical: no quotient carrier, no linear claim, no scalar restriction, exactly 3 cases, payload cases carry exactly one non-erased scalar field of the resident type with no storage, 21-48) and `atomic_once_result_is_plain` 52.
- `types::domain_requirements` 69: private trait `RequirementNode` 74 implemented for `StructuralDomainDeclaration` 82 and `ScalarDomainDeclaration` 96; `resolve` 110 (unique row by semantic domain and carrier), `walk` 127 (explicit-stack DFS with `active` for cycle detection and `visited`), `closure` 168, `supplied_closures` 177; macro `requirement_queries!` 196 expands to `has_no_required_memberships`, `required_memberships_are_well_formed`, `required_domain_closure`, `missing_membership_requirements` for both declaration types (238-239).
- `types::qualifications` 241: `StructuralDomainDeclaration` 250, `StructuralEstablishmentRoute::{CheckedRequirement, ExactMachine, BoundaryRequirement}` 296, `StructuralPathQualification` 351 with `selected_suffix` 360, `select_restorable` 384, `append_under` 400, `select_rows` 422, `select` 431. `ScalarDomainEstablishmentRoute` is a `pub use` alias of the structural route type (528).
- `types::scalar_qualifications` 462: `ScalarQualificationCatalog` 471, `ScalarFloatRange` 546 (`decode` to f64, `ordered`, `contains`, `contains_range` 554-593), `ScalarIntegerRange` 597.
- `types::structural` 639: `StructuralTypeDeclaration` 647, `StructuralTypeShape` 684 (nine variants: Reference, PrimitiveScalar, ByteSequence, ElementView, Record, FixedArray, Sum, Mixed, AtomicCell), `StructuralFieldType` 896 (ten variants), `ByteSequenceCarrier::{BorrowedView, BoundedView, BoundedOwned}` 1154, many lookup helpers (`find_structural_type` 1195, `live_field` 1176, `live_field_by_identity` 737, `live_field_by_id` 1323, `live_field_entry` 1332, `field_by_identity` 730, `structural_type_by_identity` 1314, `find_structural_case` 1343, `structural_case_by_identity` 1307, `structural_case_position` 1384, `unique_structural_declaration` 746), reachability walks `structural_type_contains_reference(_by)` 1202/1209, `structural_type_has_borrowed_storage_by` 1221, private `structural_type_contains_by` 1251, `structural_case_initializer_path` 1350; presentation helpers on `StructuralFieldType`: `borrowed_presentation_type` 1018, `leaf_copy_type` 1072, `declares_leaf` 1110, `leaf_declaration` 1100, `shared_observation_type/shape`, `leaf_copy_shape`, `canonical_leaf_shape/type`.
- `values::places` 1529: `StructuralPathSegment` 1539 (eight variants, default = `Field(ABSENT)` 1559), `is_static_structural_path` 1632, `is_structural_scalar_store_path` 1639, `StructuralStorageUse` 1669, and four large `impl OperationKind` methods: `structural_observation_source` 1694, `map_structural_uses(_with_access)` 1713/1717, `structural_projections` 1886, `storage_uses` 1911, `structural_projection_paths_mut` 2125, `runtime_indexes` 2224.
- `values::reference_origins` 2506, `values::signatures` 2561: `ValueDeclaration` 2572, `TerminalMachineResult` 2594, `StructuralParameterDeclaration` 2619 (+ `is_cleanup_receiver` 2637, `is_unqualified_root` 2644, `admits_scalar_read_at` 2659), `StructuralResultDeclaration` 2668, `StructuralReferenceOrigin` 2684, `StructuralArgument {place, path, access}` 2734.

**Inputs/outputs.** Pure functions over slices of declarations. `map_structural_uses_with_access` mutates the operation through a closure `(PlaceId, StructuralAccess) -> PlaceId`; `TransformContent` additionally rewrites each derivation via `ContentTransformationDerivation::map_places` (`proof.rs:114`) using a binding list and re-sorts derivations (1740-1760).

**How it decides.**
- Domain closure `walk` 127-166: fails (`None`) on a cycle (148-150), on an unresolvable or non-unique domain row (154 via `resolve` 110-120), or if a `required_memberships` list is not strictly increasing (159-161). `required_domain_closure` 207 returns nodes only if every node has `required_memberships: Some(_)` (173, 212); `missing_membership_requirements` 215: for each supplied id requires exactly one local row for the carrier and a well-formed closure (186-191), then walks pruning at any domain already in a supplied closure (221-228) and requires every remaining node's requirements to be `Some` (229-232). Keyed on `DomainSemanticId` plus carrier (`StructuralTypeId` or `ScalarType`) equality.
- `StructuralDomainDeclaration::is_pure_predicate` 270, `authorizes_boundary_requirement` 277: matches a `BoundaryRequirement` route by `requirement_identity == requirement` and `Some(owner.identity()) == declaration` (index equality, not text).
- `StructuralPathQualification::selected_suffix` 360: `declared` path matches `selected` prefix segment by segment; a declared `AllElements` matches a selected `FixedIndex` or `RuntimeIndex` (372-378).
- `select_restorable` 384: refuses (returns `None`) when selecting a non-empty path while whole-value qualifications exist, or when a projected row is a strict prefix-extension that matches a prefix of the selection (389-396).
- `ScalarFloatRange::contains` 571 decodes both bounds to `f64` and compares (f32 values widened, 557); `ordered` 566 uses `<=` on decoded values. `ScalarIntegerRange::ordered` 606 refuses address-carrier integers.
- `structural_type_contains_by` 1251: DFS with `BTreeSet` visited; unknown type id is skipped (1263-1265); descends Record/Sum/Mixed fields (non-erased for `contains_field`), FixedArray unless length is `Literal(0)` (1293), ElementView element; does not descend `Reference` referents (1298).
- `live_field` 1176 requires exactly one field with that identity, not erased, and exactly one field with the same id.
- `StructuralParameterDeclaration::admits_scalar_read_at` 2659: Unrestricted always, Affine only when the path is non-empty, and no qualifications.
- `is_structural_scalar_store_path` 1639: Field must be non-absent; Case, FixedIndex, RuntimeIndex ok; AllElements, FixedRange, RepresentationView, Referent not ok.
- Access mapping in `map_structural_uses_with_access`: length/read ops map with `SharedBorrow`, stores with `WriteOnlyBorrow`, `MoveStructuralField` with `MutableBorrow`, `ReleaseReference` with `Owned`, atomic access by (observes, modifies): (true,true) Mutable, (false,true) WriteOnly, (_,false) Shared (1865-1869); other ops use the argument's own `access` field.

**Cases by shape.**
- `is_float_class_shape` 660 requires a Sum of exactly five cases in the order `[nan, infinity, normal, subnormal, zero]` (664), `nan` with no fields and each other case with exactly one non-erased Boolean field (667-671).
- `atomic_once_outcome_cases` 16: exactly three cases (31) with the shape described above; returns `None` otherwise.
- `borrowed_presentation_type` 1018: special cases for atomic cells, bounded-owned byte sequences, scalar restriction, SharedBorrow byte-view, else canonical leaf; `leaf_copy_shape` 1140 copies only `BoundedOwned` and `BorrowedView` byte carriers (not `BoundedView`).
- `OperationKind` is matched exhaustively (no wildcard) in `map_structural_uses_with_access` 1721-1883 and `storage_uses` 1913-2121 via the macros `call_and_scalar_constant_operations!` 1565 and `scalar_predicate_operations!` 1580; but `structural_projection_paths_mut` 2129-2220 ends with `_ => {}` (2219) and `structural_observation_source` 1694-1710 with `_ => None` (1709).

**Same job elsewhere.**
- `structural_type_contains_by` 1251-1305 and `BoundaryMachineDeclaration::requires_checked_non_observation` (`boundary.rs:1267-1343`) both do a visited-set DFS over structural types descending Record/Sum/Mixed fields and FixedArray (skip `Literal(0)`) and ElementView; the boundary version duplicates the walk with its own predicate (read both).
- The nine `OperationKind` operand/projection enumerations: `map_scalar_uses` (`control_flow.rs:1601`), `map_structural_uses_with_access` 1717, `storage_uses` 1911, `structural_projection_paths_mut` 2125, `structural_observation_source` 1694, `direct_call_parts` (`lib.rs:1083`), `module_invocation` (`control_flow.rs:387`): each enumerates operation variants separately (read).
- Atomic access kind determination: `AtomicAccessEvent::observes_resident/modifies_resident` (`control_flow.rs:104-110`) used at 1865.
- `StructuralFieldType::scalar_type` 972 vs `as_scalar` 943 (different: `scalar_type` also maps Atomic/BoundedInteger/Refined/IeeeFloat).
- Lookup-by-id helpers (`find_structural_type` 1195, `find_structural_case` 1343, `field_by_identity` 730, `structural_case_by_identity` 1307, ...) are linear scans; `unique_structural_declaration` 746 differs by requiring uniqueness and a `visiting` guard.
- Two `is_bounded_owned_byte_sequence` methods: one on `StructuralTypeDeclaration` 766 and a `const` one on `StructuralTypeShape` 824 (read).

**Unused / one-caller / "shared".** `unique_structural_declaration\(|structural_case_initializer_path\(|atomic_once_outcome_cases\(|atomic_once_result_is_plain\(`: 22 occurrences in 14 files (names combined; this file 3). `[^_a-z]live_field\(`: 5 occurrences in 4 files (this file 1, the definition). `structural_type_contains_reference|structural_type_has_borrowed_storage_by|required_domain_closure\(|missing_membership_requirements\(|required_memberships_are_well_formed\(`: 45 occurrences in 16 files. No zero-user finding for these. The five shape-pattern macros `record_or_mixed_shape_pattern!` 1392, `record_or_fixed_array_shape_pattern!` 1405, `byte_sequence_view_carrier_pattern!` 1413, `sum_or_mixed_shape_pattern!` 1421: not grepped individually (suspected used; two are used at 789-790 and 848 inside this file).

**Stated limits.** None as comments. `StructuralPathQualification::select_restorable` doc: none. Default of `StructuralPathSegment` is `Field(ABSENT)` 1559-1563 (rejected by `is_structural_scalar_store_path` 1641).

**Fail-open risks.**
- `structural_type_contains_by` 1263-1265: a type id with no declaration is skipped, so `structural_type_contains_reference` returns false for an unknown root (traced: callers pass `find_structural_type` over the module's own types; whether unknown ids occur is not traced; open). Same pattern in `requires_checked_non_observation` (`boundary.rs:1288`).
- `ScalarFloatRange::contains` 571-579 compares via `f64`; NaN bounds make `candidate >= minimum` false, so NaN bound rows contain nothing (read; fail-closed). `ordered` 566 is false for NaN bounds.
- `structural_projection_paths_mut` 2180-2182: `ElementViewWrite` pushes only `destination.path`, not the separate element `path` field (declared at 1302-1309), although `ElementViewRead` pushes its `path` (2185) and `storage_uses` returns the `path` for both (1977-1993). `OperationKind::map_scalar_uses` (`control_flow.rs:1602-1608`) remaps `RuntimeIndex` only through this function, so a `RuntimeIndex` inside `ElementViewWrite.path` would not be remapped; whether such a path ever holds a `RuntimeIndex` is not traced (open).
- `structural_projections` 1886-1909 for `ElementViewWrite` returns only the descriptor projection (1899-1901), so `runtime_indexes` 2224 does not list element-path indexes for writes (inferred from the code, not traced to callers; open).
- `StructuralTypeShape::is_composite_shape` etc. are pure predicates (no fail-open).

---

## 10. terminal-psi/work/src/lib.rs (437 lines, all read) - crate `psi_work`

**Purpose.** A logical work meter for the Terminal Psi interpreter: every operation and every taken control-flow edge is charged a fixed number of units before it runs; the meter records per-site execution counts and units, can stop at a local budget, and can also debit a shared evaluation-invocation account.

**Shape.** `WorkSchedule {identity}` 47 with `CURRENT` (identity `WorkScheduleIdentity::new(1)`, 52-57), `operation_units(&OperationKind) -> u64` 63 and `terminator_units(&Terminator) -> u64` 155. `WorkChargeSite::{Operation(OperationId), Edge(EdgeId)}` 170, `WorkAttribution {executions, units}` 176, `WorkUsage {schedule, total_units, attribution: BTreeMap<WorkChargeSite, WorkAttribution>}` 192, `WorkMeter {schedule, work_remaining: Option<u64>, usage, invocation: Option<EvaluationInvocationAccount>}` 225 with `unbounded` 233, `with_budget` 237, `with_invocation` 241, `replenish` 288, `charge_operation` 300, `charge_terminator` 307, `charge_edge` 322, private `charge` 336. `WorkBudgetExhausted` 391, `WorkMeterError` 414 (seven variants).

**Inputs/outputs.** Reads `terminal_psi::{Operation, OperationKind, Terminator}`, `evaluation_accounting::{EvaluationInvocationAccount, EvaluationMeasurementIdentity}`. Mutates `usage`, `work_remaining`, and (through `invocation.charge_work`) the shared account.

**How it decides.** `operation_units` is a single arm listing every `OperationKind` variant (65-151) returning `1`; `terminator_units` lists every `Terminator` variant returning `1` (157-164). Both matches are exhaustive without wildcard (read), so a new variant forces a decision at compile time. `charge` 336: refuses when `work_remaining < units` (337-347, `Exhausted`), then computes checked sums (349-362), then charges the shared account (364-368), and only then commits `usage` and decrements `work_remaining` (369-379), so a refused payment leaves usage unchanged. `with_invocation` 241 requires the account's measurement to be `EvaluationMeasurementIdentity::TerminalPsi` with the same work schedule identity (246-253). `charge_terminator` 307 returns `ConditionalEdgeNotSelected` for `Conditional`/`StructuralCase` (315-317); `charge_edge` 322 requires that the edge belongs to the terminator (327).

**Cases by shape.** Cost is uniform: every operation is 1 unit and every terminator is 1 unit (63-166). Branching terminators cannot be charged as a whole (315).

**Same job elsewhere.** The list of `OperationKind` variants appears again in `control_flow.rs:1601` (`map_scalar_uses`), `types_and_values.rs:1717/1911` and `lib.rs:1083` (this unit) (read).

**Unused / one-caller / "shared".** Header says `WorkScheduleIdentity::TERMINAL_UNIT_V1` and marker `"psi-work-unit-v1"` (lines 9, 29); grepped `TERMINAL_UNIT_V1|psi-work-unit-v1` over `omega-rust`: only these two comment lines match, and the code uses `WorkScheduleIdentity::new(1)` (53) with the test asserting `marker() == 1` (meter.rs:16), so the named constant and the string marker do not exist in the code. Users: `psi_work::|WorkMeter::` 170 occurrences in 49 files (Grep count; includes tests, interpreter, build-time evaluation).

**Stated limits.** Header 3-34 states the schedule is "explicit, deterministic" and that exhaustion "never partially executes an instruction". No unsupported-feature comments.

**Fail-open risks.** `replenish` 288-290: on an unbounded meter (`work_remaining == None`) it returns `Ok(())` without doing anything (stated by behavior; harmless). `charge` with `work_remaining = None` never refuses (by design: `unbounded`). Order in `charge`: the invocation account is debited at 364-368 before the local counters commit; the test at `meter.rs:362-399` checks the failure path leaves usage and budget unchanged; if `charge_work` succeeded and a later step failed there is none (the checked sums are computed before 364), so no partial debit is reachable (traced, read 349-380).

---

## 11. terminal-psi/work/tests/meter.rs (451 lines, all read)

**Purpose.** Integration tests of `psi_work`: schedule identity, one-unit costs for sampled operation kinds and terminators, exhaustion without mutation, replenishment overflow, shared invocation accounting, and refusal of non-Terminal measurement identities. Seven `#[test]` functions: 14, 24, 225, 255, 321, 363, 402. Helper builders 328-451 (`operation()`, `terminal_sponsor`, `*_id`).

**How it decides / what it pins.** Asserts exact error values: `WorkMeterError::Exhausted(WorkBudgetExhausted{...})` 233-239, the display text "Work budget exhausted: 1 unit used, 0 remaining; next charge requires 1 (schedule 1)" 243, `BudgetOverflow` 323, `InvocationCharge{reason: BudgetExhausted(Work)}` 383-386, `InvocationMeasurementMismatch` for a `CheckedTree` account 412-415. It samples 7 integer kinds (45-74), 5 other operation kinds (81-119) and 5 terminators (127-166) for cost 1; not every variant is sampled (exhaustiveness is enforced by the match in `lib.rs:63`).

**Same job elsewhere / unused / limits / fail-open.** Not applicable (test file). Cloning a `WorkMeter` shares the invocation account (test 376-378 shows an alias meter charging the same sponsor, read).

---

## 12. tokens/src/lib.rs (476 lines, all read) - crate `tokens`

**Purpose.** The lexical token stream representation: compact `Token {kind, decoded: Handle<DecodedRange>, span}`, `TokenStream` that owns or borrows the source text and a shared buffer for decoded string/byte literals, a scratch recycler, and `LexedSource(s)` records. Header 3-23 says it is produced by `Psi 00` and consumed by `Psi 01` and `package-manager`.

**Shape.** `sources` 34: `LexedSource {source_id, path, source: Arc<str>, origin, tokens: TokenStream<'static>, scope}` 42, `LexedSources {sources: Arena, batch: HandleSpan}` 52. `token` 57: `Token` 66 with classifier helpers `keyword` 77, `punctuation` 84, `integer_literal_kind` 91, `float_literal_kind` 98, `is_whitespace` 105, `comment` 109, `is_non_semantic` 116. `token_text` 122: `TokenText::{Source(&str), Bytes(&[u8])}` 126 with `as_bytes`, `try_as_str`, `as_str` (panics "token text is not valid UTF-8" 156). `TokenStream` 209 (`source: SourceBacking`, `tokens`, `decoded_ranges: Arena<DecodedRange>`, `decoded_bytes: Option<Arc<Vec<u8>>>`) with `with_capacity` 230, `with_scratch` 239, `recycle` 250, `push_source` 272, `push_decoded` 280, `retain_source` 298, `text` 323, `literal_bytes` 337. `TokenScratch` 217. Last lines 474-476: `pub mod token_stream { pub use crate::*; }`.

**Inputs/outputs.** Callers push tokens with `push_source(kind, span)` or `push_decoded(span, closure)`; `push_decoded` always records kind `TokenKind::StringLiteral` (292), so any decoded literal (string or byte) is the same kind. `text` returns decoded bytes when `token.decoded.is_valid()`, else the source slice `[span.start..span.end]` (323-334).

**How it decides.** `retain_source` 298 asserts pointer identity of the source text with the given `Arc<str>` (299-302, panics "retained source must be the exact text used to lex this stream"). `recycle` 250 clears buffers and keeps the decoded-bytes `Arc` only if uniquely owned (259-263). `PartialEq` for `TokenStream` 357 compares length, kind, span and text per token, not source ownership.

**Cases by shape.** `integer_literal_kind` / `float_literal_kind` match exactly the two `NumericLiteralKind` variants (91-103).

**Same job elsewhere.** `TokenKind::StringLiteral` is the only literal kind for decoded payloads; byte literals share it (inferred from 292 and the header 11-14 "decoded string or byte literals").

**Unused / one-caller / "shared".** `pub mod token_stream { pub use crate::*; }` (474-476): grepped `token_stream::|tokens::token_stream` over `omega-rust`: 0 matches, so the alias module has no users by path (a `use tokens::token_stream` wildcard-less form could not be matched by that pattern only if spelled differently; not otherwise grepped). Helpers `\.is_non_semantic\(\)|\.literal_bytes\(|\.source_owner\(\)|\.integer_literal_kind\(\)|\.float_literal_kind\(\)|\.is_string_literal\(\)`: 27 occurrences in 10 files (names combined; this file 3 are the definitions).

**Stated limits.** None. Panics: `retain_source` assert 299, `as_str` 156, `text`/`literal_bytes` `.expect("decoded payload has backing bytes")` 330, 345.

**Fail-open risks.** None seen. `text` slices the source by span without bounds checks beyond Rust's own slicing panics (333).

---

## 13. tokens/src/token_kind.rs (300 lines, all read)

**Purpose.** The closed vocabulary of lexical token kinds: keywords, punctuation, numeric literal shapes, comment kinds, and the `TokenKind` enum. Header 1-6.

**Shape.** `CommentKind::{Line, Block(default)}` 10; `KeywordKind` 18 (28 real keywords plus `Unknown` default) with a sorted `KEYWORDS: &[(&str, KeywordKind)]` table 51-80, `from_lexeme` 83 (binary search), `lexeme` 90 (linear scan, `"<unknown>"` fallback), `is_strict_identifier_keyword` 97 (true only for `SelfValue`). `NumericLiteralKind::{Integer(IntegerLiteralKind), Float(FloatLiteralKind)}` 124, `IntegerLiteralKind {base, empty_digits, has_suffix}` 136, `FloatLiteralKind {has_exponent, empty_exponent, has_suffix}` 143, `NumericBase` 150. `PunctuationKind` 160 (42 named variants plus `Unknown`) with `ordered_lexemes()` 208 (17 two/three-character lexemes first, then single characters) and `lexeme` 255. `TokenKind::{Identifier(default), NumericLiteral, StringLiteral, Keyword, Punctuation, Whitespace, Comment}` 291.

**How it decides.** Keyword lookup is by exact spelling via `binary_search_by` on `KEYWORDS`; correctness depends on the table being sorted, which the test at 106-119 asserts. Punctuation matching order is the order of `ordered_lexemes()` (longest first by construction: `..=` before `..`, `->` before `-`, 210-226); the lexer itself is in another crate (not in this unit).

**Cases by shape.** Fixed keyword list (28 spellings) and fixed punctuation list; anything else is `Unknown`/identifier. `PunctuationKind::Unknown` has no lexeme (maps to `"<unknown>"` 261); the enum has variants `Apostrophe`, `Hash`, `Tilde`, `Caret` with lexemes at 228, 230, 237, 251. Numeric literal kind records only base, whether digits are empty, suffix presence, exponent presence (no value).

**Same job elsewhere.** `KeywordKind::lexeme` 90 and `PunctuationKind::lexeme` 255 each do a linear reverse lookup over their table; the keyword table is a second copy of the enum list (the test at 106-119 guards agreement).

**Unused / one-caller / "shared".** Comment at lib.rs 21-22 says token_kind.rs holds these types; users are the lexer and parser crates (not grepped by name here). Not counted.

**Stated limits.** Test message 109: "the keyword table must stay sorted for `from_lexeme`'s search". `KeywordKind::Unknown` and `PunctuationKind::Unknown` are defaults with no lexeme.

**Fail-open risks.** None seen (`from_lexeme` returns `None` for unknown spellings; `lexeme` returns `"<unknown>"` for the Unknown variant).

---

## 14. typed-trees/src/access_views_and_commitments.rs (2521 lines, all read; path under `omega-rust/psi/compilation/representations/typed-trees/src/`)

**Purpose.** Four groups of data/queries on `TypedTrees`: (1) access views: recognizing the sealed toolchain declarations that implement placed-access lifecycle operations and scalar accessors, and recording laid-out view/field plans and plan-laid layout geometry; (2) commitments: machine specialization and closed conformance application records; (3) constant recipes: retained constant-expression recipes, evaluation entries and array extent queries; (4) wire schema queries (field carrier shapes and worst-case size). Header 1-6.

**Shape.** `access_views` 8-1445 with sub-modules `lifecycle_execution` 9, `primitive_selection` 445, `sealed_accessors` 671; `commitments` 1447-1691; `constant_recipes` 1693-2230 (sub-module `evaluation_entries` 1701); `wire_schema_queries` 2232-2521.
- `AccessExecutionDeclaration {operation, content: Resident|Run, machine, state, template}` 32 with `authenticate` 332, `backing_outcomes` 52 (static table of case names and parameter-to-field transfers, 55-144), `invocation_operands` 255, `profile_role` 281, `source_invocation_bindings` 167.
- `AccessorPrimitive::{Read, Write}` 456 with `from_declaration` 469 and `select_target` 613.
- `ScalarAccessorDeclaration` 677 with `authenticate` 732 (eleven private fields).
- `AccessLifecyclePlan::{Absent, Evaluated{..}}` 933, `AccessLifecycleStorage` 951, `AccessLifecycleOwner` 962, `AccessLifecycleOperation` 974, `AccessViewPlan` 1049, `AccessFieldPlan` 1063 (+ `scalar_projection_leaf` 1079), `PlanLaidLayout` 1162, `PlanLaidGeometry` 1292 (+ `authored_field_storage` 1326, `native_layout_report` 1305), `PlanLaidRealizations::{Invariant, TargetInputs}` 1218.
- `TypedTrees::access_borrow_factory` 1019.
- `commitments`: `MachineSpecialization` 1475 (template, instance, type/const argument strings and identities, `conformance_applications`, `normalized_template_identity: String`, `accepted_template_commitment: Option<String>`), `digest_commitment!` macro 1452 (exported) producing `MachineSpecializationCommitment` 1492, `MachineTemplateCommitment` 1497, `ClosedConformanceApplicationCommitment` 1564; `ClosedConformanceApplication` 1503 (declaration, trait scope, arguments, rows of symbol handles, `report_fingerprint`, `commitment`, `identity_encoding: Arc<[u8]>`) with `same_selection_metadata` 1522 and a `PartialEq` that adds `arguments` 1538; `StaticRequirementDispatch` 1654; `machine_specialization_matches_template_identity` 1676.
- `constant_recipes`: `RetainedConstantPosition::{None, ArrayLength, ContractExpression, ConstApplication}` 1847, `RetainedConstantRecipe` 1872, `ConstantExpressionContext` 1882, `ConstantRecipeClosure` 1929, `DeclaredArrayExtent::{Literal, Recipe}` 1950, `retain_constant_recipes` 2157, `create_evaluation_entry` 1803, `TypedTrees::{constant_evaluation_entry 1721, constant_evaluation_for_expression 1756, declared_array_extent 1981, declared_constant_extent 2005, closed_fixed_array_length 2103, retain_symbolic_constant_recipes 2128}`.
- `wire_schema_queries`: `record_wire_schema_plan` 2242, `wire_field_fixed_array` 2346, `wire_field_is_unbounded_slice` 2367, `wire_field_slice_element` 2382, `wire_field_repeated_carrier` 2411, `wire_field_repeated_encoding` 2480, `wire_schema_scalar_body_worst_case` 2495, `wire_nested_field_worst_case` 2516, `is_core_fixed_vec` 2299.

**Inputs/outputs.** Reads `TypedTrees` tables (symbols, machines, states, type reference table, access_view_plans, constant_recipes, wire_schemas, data definitions). Returns `Result<Option<..>, Diagnostic>` for authenticators, `Option` for most queries; `retain_constant_recipes` 2157 and `retain_symbolic_constant_recipes` 2128 mutate `constant_recipes`/`constant_expression_contexts`; `create_evaluation_entry` 1803 inserts generated symbols, statement table rows and returns a `Machine`.

**How it decides (keyed on source identity and source spelling).**
- `AccessExecutionDeclaration::authenticate` 332-442: a symbol is accepted as a sealed placement execution only if its declaration's source file has `origin == Toolchain`, no package identity (352-356), the path relative to the package root is exactly `layout.omg` and the file text is byte-equal to `include_bytes!("source/library/core/layout.omg")` (357-361, constants 16-23), or `run.omg` and byte-equal to `run.omg` (364-369); then the normalized identity string must be one of six exact spellings: `toolchain::Access::initialize_from_profile`, `toolchain::Run::initialize_from_profile` (Initialize), `toolchain::Access::extract_resident`, `toolchain::Run::extract_from_profile` (MoveOut), `toolchain::Access::view_from_profile`, `toolchain::Access::view_mut_from_profile` (View), `toolchain::Run::relocate_from_profiles` (Relocate) (377-391). Any other symbol returns `Ok(None)` (not sealed). Then a "declaration envelope" check (400-414): no body, supply mode Boundary, lifetime-parameter count equals 1 for View else 0, no spelling/target/suspends/blocks/trait conformances/invokes, one state, parameter count 3 (Initialize), 2 (MoveOut, View), 4 (Relocate), no const or erased parameters (423-434). Violations after the sealed match are `Err(Diagnostic)`.
- `AccessorPrimitive::from_declaration` 469: matches machine name `"read"`/`"write"` (479-483) whose source file is Toolchain with relative path `layout.omg` (does not compare file bytes, unlike the previous function) (488-493), then many shape checks (501-609: exactly two type parameters, one state, no body, boundary supply, empty contracts/invokes/owned data, first parameter a reference to the accessor parameter with Shared (read) or Mutable (write) access, signature arity 1 or 2, one conformance whose trait name last path segment is `Readable`/`Writable`, in the same source file, requirement named `read`/`write`, type arguments `[accessor, value]`). Failure after the name+file match is `Err`.
- `ScalarAccessorDeclaration::authenticate` 732: looks for a unique `(view, field, target)` over `access_view_plans` whose `machine_symbol` or `state_symbol` equals the symbol (736-754); requires the view template symbol to be Toolchain origin and in `layout.omg` (756-776), the field's access-plan entry to match the key and access (777-788), returns `Ok(None)` if the field value type is not primitive and the access is not `Region` (789-793) or the target operation name is not `read`/`take`/`write` (840-845), and `Err` for any envelope/signature substitution (801-907). Keyed on symbol handle equality, source file path and string operation names.
- `AccessorPrimitive::select_target` 613: finds the unique field whose `accessor_data_symbol` equals the accessor type's symbol (627-640), compares normalized type identities of accessor and value leaf (642-655), then picks the target whose `operation` string equals `read`/`write` (656-660).
- `scalar_projection_leaf` 1079: peels matching literal-length fixed arrays (equal non-zero counts, 1092-1103) until the accessor is `Named` with the field's accessor data symbol and the value is primitive (1104-1109); otherwise `Err`.
- `access_borrow_factory` 1019: follows `machine_specializations` from instance to template with a `visited` guard until a unique factory row is found (1023-1044); returns `None` on ambiguity or cycle.
- `AccessFieldPlan`-to-geometry: `authored_field_storage` 1326 prefers bit fields, then repeated, then integer, then scalar/direct by `scalar_field` flag.
- `PlanLaidLayout::geometry_for_target` 1240: invariant geometry for any target; target-input realizations are matched by string compare of the target name (1253).
- `machine_specialization_matches_template_identity` 1676: package identity equality plus `normalized_template_identity.split_once('|')` second half equal to the machine identity string (1686-1689); a template identity without `|` yields false.
- `declared_constant_extent` 2005: a `Named` type reference whose text parses as `u64` is a literal length (2010); otherwise a `ConstExpression` with a constant integer value; otherwise a recipe for the const application site.
- `wire_field_repeated_carrier` 2411: FixedArray with literal length, or a generic application of the toolchain `FixedVec` (by `is_core_fixed_vec` 2299: name equals `"FixedVec"` and source file is Toolchain `fixed_vec.omg`, 2315-2324) with a `Named` count argument parsed with `usize::parse` (2436), or a `Named` FixedVec data definition with fields named `"items"` (a fixed array) and `"length"` (u64) (2461-2472).
- `retain_constant_recipes` 2157: recipes with equal `position` merge their contexts; contexts are compared by `same_scope` (owner and result type only, 1899-1901).

**Cases by shape.**
- `backing_outcomes` 55-144: exactly Initialize x {Run, Resident}, MoveOut x {Run, Resident}, Relocate x Run, and returns `&[]` for View and for Relocate x Resident (142-143). Case-name strings "Ready", "Rejected", "Moved" and transfer paths ("view.storage", "returned.source", "view.run.storage", ...) are string literals (58-138).
- Parameter counts hard-coded: 4 for Relocate else 3 type parameters (213-217, 302-306); `role_position >= expected - 2` allows 1 or 2 role positions (307).
- `wire_field_*` helpers: only literal-length fixed arrays (`FixedArrayLength::Literal`, 2360), `Slice`, FixedVec; scalar-only worst-case sizing (`wire_schema_scalar_body_worst_case` 2495 returns `None` for any non-scalar relevant field 2507-2509).
- `DeclaredArrayExtent`: `ConstParameter` lengths give `None` (1994, 2114).

**Same job elsewhere.**
- Commitment/record types with the same names exist in the Terminal crate: `ClosedConformanceApplication` and `ClosedConformanceApplicationCommitment` here (1503, 1564) versus `boundary.rs:186` and `boundary.rs:233`; `StaticRequirementDispatch` here 1654 versus `proof.rs:465`; `ClosedConformanceRowIdentity` 1596 (symbol handles) versus `ClosedConformanceRow` (`boundary.rs:265`, identity indices) (read). They are separate typed-tree and Terminal copies of the same concept.
- `digest_commitment!` macro 1452: also used in `checked-trees/src/operators_and_proof.rs` and `checked-trees/src/contract_and_obligation_facts.rs` (1 each; grepped count `digest_commitment!`: 5 occurrences in 3 files, 3 here); `ClosedConformanceApplicationCommitment` in `boundary.rs:233` hand-writes the same `from_digest/as_bytes/is_zero` shape rather than using the macro (different crate).
- Sealed-declaration recognition by toolchain source path appears three times in this file with different strictness: bytes compared (`authenticate` 357-369), path only (`from_declaration` 488-493, `ScalarAccessorDeclaration::authenticate` 768-776), and path plus name (`is_core_fixed_vec` 2315-2324). `omega-rust/omega/compilation/realization/provider-planning/src/sealed_accessors.rs` is a further site (suspected; grep matched `ScalarAccessorDeclaration::authenticate` there, not read).
- Type-reference shell peeling (`Reference` / `Constrained` loops): `named_type_reference_through_shells` 1145 (recursive), `named_type_reference_name` 2327 (recursive), `wire_field_fixed_array` 2346, `wire_field_is_unbounded_slice` 2367, `wire_field_slice_element` 2382, `wire_field_repeated_carrier` 2419 (four loops written out) (read).
- `expected = if Relocate {4} else {3}` appears at 213 and 302.
- Constant recipe lookup by position: `constant_recipe_for_array_length` 2026, `constant_recipe_for_contract_expression` 2039, `constant_recipe_for_const_application` 2064 are three linear scans over `constant_recipes` with different predicates.

**Unused / one-caller / "shared".** Greps over `omega-rust` (count mode): `MachineSpecializationCommitment|MachineTemplateCommitment` 18 occurrences in 6 files (this file 2, `validation/src/machine_calls_core.rs` 9); `closed_conformance_applications_of|machine_specialization_matches_template_identity|\.is_core_fixed_vec\(|create_evaluation_entry\(|retain_symbolic_constant_recipes\(` 21 occurrences in 11 files (this file 6); `backing_outcomes\(|AccessExecutionDeclaration::authenticate|ScalarAccessorDeclaration::authenticate|AccessorPrimitive::from_declaration` 30 occurrences in 17 files (this file 2). `admits_fixed_index(`: 5 call sites outside the definition? Grep content shows definition at 1972 plus uses at `validation/src/value_custody_core.rs:6734, 6743, 6752` and `05_typed-trees-to-checked-trees/src/execution_terminal_unit_types.rs:965` (4 callers). No zero-user finding among the names checked.

**Stated limits.** Diagnostic strings only, e.g. "requires an exact compiler-issued field accessor; an ordinary or unconstrained generic type grants no operation" (635), "constant expression has multiple evaluation contexts; its lexical owner is required" (1793), "access field `{}` requires corresponding fixed-array dimensions with one exact scalar accessor leaf" (1112). No TODO or migration comments.

**Fail-open risks.**
- `DeclaredArrayExtent::admits_fixed_index` 1972-1977: for a `Recipe` extent returns `true` for any index (no comment). Traced: callers are `validation/src/value_custody_core.rs:6734-6752` (rejects when false) and `execution_terminal_unit_types.rs:965` (accepts when true); so a constant index into an array whose length is a not-yet-closed const-call recipe is admitted at those sites. Whether a later closed-length check re-validates was not traced (open).
- `AccessExecutionDeclaration::authenticate` returns `Ok(None)` for every non-matching symbol (333-393), i.e. "not a sealed access declaration", rather than an error; stated by the `Option` result; the two later failure modes are errors. Source-file mismatch (bytes differ from the shipped `layout.omg`/`run.omg`) is `Ok(None)` (370), so a toolchain file with the right name but different bytes is silently treated as an ordinary declaration (traced to the match at 357-370; consequence depends on callers, open).
- `AccessorPrimitive::from_declaration` 479-493 and `ScalarAccessorDeclaration::authenticate` 768-776 compare only the relative path `layout.omg` under a Toolchain origin, not file bytes (read); a toolchain-origin file at that path is trusted. Status: open.
- `ScalarAccessorDeclaration::authenticate` 789-793 returns `Ok(None)` when the value type is not primitive and the field access is not `Region` (the field is then not treated as a scalar accessor).
- `access_borrow_factory` 1036-1043: when no factory row matches and there is not exactly one specialization, returns `None` (fail-closed for a caller that needs a factory).
- `wire_schema_scalar_body_worst_case` 2507-2509 returns `None` on first non-scalar relevant field (fail-closed).
- `retain_constant_recipes` 2178: contexts that share `owner` and `result_type` but differ in `evaluation_entry` or `evaluation_expression` are treated as the same scope and the new one is dropped (read 1899-1901, 2178; test at 2196-2228 pins that a rebound result type is kept). Which entry is kept is the first retained one (open).

---

## Coverage

- Read in full (all lines of every row in the unit file): boundary.rs 1-1838 (1838), control_flow.rs 1-2736 (2736), identity.rs 1-532 (532), lib.rs 1-1172 (1172), observation.rs 1-1307 (1307), ownership.rs 1-1144 (1144), proof.rs 1-808 (808), terminal_module.rs 1-1288 (1288), types_and_values.rs 1-2763 (2763), work/src/lib.rs 1-437 (437), work/tests/meter.rs 1-451 (451), tokens/src/lib.rs 1-476 (476), tokens/src/token_kind.rs 1-300 (300), typed-trees/src/access_views_and_commitments.rs 1-2521 (2521). Total 17773 lines.
- Skimmed: none. Not read: none.
- Counts came from Grep tool calls in count mode over `omega-rust` (names combined where an alternation is shown; counts of "occurrences" are lines matched).

## Five things a maintainer most needs to know about this unit

1. `terminal-psi` is almost entirely data; the large logic sits in about eight `OperationKind` enumerations that each list variants separately: `map_scalar_uses` (`control_flow.rs:1601`), `module_invocation` (387), `map_structural_uses_with_access` (`types_and_values.rs:1717`), `storage_uses` (1911), `structural_projection_paths_mut` (2125, has a `_ => {}` arm unlike its siblings), `direct_call_parts` (`lib.rs:1083`) and `WorkSchedule::operation_units` (`work/src/lib.rs:63`). A new `OperationKind` variant needs updating in each; only some are exhaustive matches.
2. Identity equality is by text per owning `TerminalIdentityTable`, never by index (`identity.rs:22-133`); the rule differs between sites: machine/state identities require non-empty text, while parameter/result/callable text is compared with plain `!=` on `Option<&str>` (`identity.rs:89-98`), so two missing indices compare equal there.
3. `observation.rs` defines trace outcomes and error variants for external completion (`TerminalTraceV1Outcome::ExternalCompletion`, three `*ExternalCompletion*` error variants, `TerminalTraceExternalCompletionSiteRow`) that nothing in `omega-rust` constructs or reads; the builder only finishes with return, edge crash, operation crash, boundary crash or prefix.
4. `typed-trees/src/access_views_and_commitments.rs` recognizes "sealed" toolchain declarations by source path under a Toolchain origin and, in one function, by byte-equality with `include_bytes!` of `layout.omg`/`run.omg`; the other recognizers compare only the path and string names (`"read"`, `"write"`, `"FixedVec"`, `toolchain::Access::...`). It also keeps typed-tree copies of conformance/commitment types whose names repeat the Terminal ones.
5. Header comments name things that are not in the code: `OptimizationReceipt` (`identity.rs:4`), `ObservationProfile`/`FloatContract` (`observation.rs:3`), `TheoremObligation`/`ObligationPhase` (`proof.rs:2`), `WorkScheduleIdentity::TERMINAL_UNIT_V1` and marker `"psi-work-unit-v1"` (`work/src/lib.rs:9,29`; the code uses `WorkScheduleIdentity::new(1)`); each appears once in `omega-rust` (grepped). Also `TerminalModule::boundary_machine` (77 uses) and `find_boundary_machine` (20 uses) are two lookups for the same job, and `suspension_call_plan_count` duplicates `suspension_call_plans.len()`.

## What I am unsure about

- The per-name use counts of alternation greps are lines matched across several names; I did not split them per name except where stated.
- I did not read callers to establish whether the open fail-open items are reachable with a bad input: `FoldedConstantClosure::validate` skipping `candidate: None` (`terminal_module.rs:99-101`), binary search over unsorted `callback_applications` (`terminal_module.rs:1109`), `DeclaredArrayExtent::admits_fixed_index` for recipes (`access_views_and_commitments.rs:1972`), and the `ElementViewWrite.path` omission in `structural_projection_paths_mut` (`types_and_values.rs:2180`).
- `evaluate_nodes` third callback `&|_, _| true` (`terminal_module.rs:495`) has a meaning defined in the `numerics` crate that I did not read.
- Meaning of `unreachable!()` guards (`boundary.rs:382`, `boundary.rs:1096`, `observation.rs:668`) depends on upstream invariants not read here.
- Whether `omega/.../provider-planning/src/sealed_accessors.rs` repeats the sealed-declaration recognition is suspected from a grep hit only; I did not read it.
