# Record for unit_033 (areas 12-native-representations and 13-native-targets-driver)

Checkout `C:/Development/Omega/.agents/worktree/worker-2/` at `4f3c255d05599c11b1ea8ac668635f15f2a6eb86`. Read only; nothing was built or run.
Paths are relative to `source-rust/subsystems/compiler/psi-to-native/` (written `P/`). All 52 rows of `units/unit_033.tsv` were read in full (21,993 lines).
Three groups: A = `representations/target-operations/target-vocabulary/src/` (11 files, crate `target-vocabulary`, written `TV/`); B = `targets/calling-conventions/src/` (26 files, written `CC/`), plus `targets/intrinsics`, `targets/layout`; C = `targets/register-environment/src/` (11 files, written `RE/`).
"Grepped" counts below come from `git grep -w -n -- <name> -- source-rust tools` (lines) and `git grep -w -l ...` (files), run at this checkout. Counts include the definition and re-exports unless I list the call sites.

---

## A. target-vocabulary (`P/representations/target-operations/target-vocabulary/src/`)

### A1. `TV/lib.rs` (105 lines)
1. Purpose: crate root. Declares the modules and re-exports the records a target program is made of (call contracts, boundary realizations, call-site owners, structural homes, value expressions), kept apart from `TargetOperationPlan` (`lib.rs:3-17`, read).
2. Shape: `#![forbid(unsafe_code)]` (`:1`); `pub mod` for calls, case_slots, provenance, boundary, operation_records, values, storage, arithmetic, call_contracts, scalar_operands, function_abi, code_catalogs, execution_facts, publication, identity (`:24-99`); re-exports at `:19-22`, `:25-105`.
3. In/out: no logic. Re-exports `calling_conventions::MachineRegister` (`:19`), `operation_evidence` and `operation_operands` types (`:20-22`).
4. Decides: nothing.
5. Cases by shape: none.
6. Same job elsewhere: its header says "Stage 02 lowers to target operations" (`:6`). The map notes the README says stage 04 (read from map, not re-verified).
7. Unused code: n/a (re-exports).
8. Stated limits: none.
9. Fail-open: none seen.

### A2. `TV/identity/normalized_foreign.rs` (376 lines)
1. Purpose: byte encoder for a `LegalizedNormalizedForeignCall` (a foreign call lowered to an import, component, table or syscall) so its identity is one indivisible record (`:1-9`, read).
2. Shape: `encode` (`:25`), `encode_native_callback_argument` (`:95`), `encode_machine_function_identity` (`:131`), `encode_call_binding` (`:138`), `encode_foreign_execution` (`:233`), `encode_locator` (`:247`), `encode_scalar_source` (`:293`), private `encode_scalar_home` (`:286`), `encode_bytes` (`:281`); `canonical_bytes` and `canonical_bytes_with_effects` on `LegalizedNormalizedForeignCall` (`:359-375`).
3. In/out: reads the call record's binding, scalar and structural arguments, result home, callbacks, descriptor guard. Appends bytes to a `Vec<u8>`. Calls `calling::encode_call_plan` etc. from sibling files (not in this unit).
4. Decides: the tag byte for each variant, keyed on the enum variant. `NormalizedForeignCallMechanism` picks a string prefix `"@compiler-import"`, `"@hosted"`, `"@component"`, `"@table"`, or the empty string for `SystemCall` (`:145,158,180,196,203`). The comment says empty target names cannot be import locators, so `""` is reserved for `SystemCall` (`:4-6`). The result variant picks tag 1/0/2/3 (`:44-55`), where 3 means a scalar-reference codec is present.
5. Cases by shape: fixed variant lists (`ForeignLocatorCandidate` four forms `:250-278`; six scalar-source forms `:293-356`; two `NormalizedForeignExecution` forms `:233-244`).
6. Same job elsewhere: structural encoders in sibling `structural.rs`/`structural_types.rs` (same unit). Register-homes has its own codec copy for structural contracts (suspected, `register-homes/src/recovery/structural_contracts.rs`, not read).
7. Unused code: `encode_call_binding` is re-exported at `TV/lib.rs:101`; `encode_machine_function_identity` is re-exported at `:103`. Not counted separately; the file's functions are reached through `identity/*` and the lib re-exports.
8. Stated limits: none.
9. Fail-open: none seen. Every `match` is exhaustive (no `_ =>` arm).

### A3. `TV/identity/structural.rs` (522 lines; tests at `:381-522`)
1. Purpose: byte encoders for boundary settlements, call origins, completion-claim rosters, ownership events, affine cleanup actions and structural result declarations (`:1-8`).
2. Shape: `encode_boundary_settlement` (`:27`), `encode_external_execution` (`:46`), `encode_provider_execution` (`:64`), `encode_call_source` (`:91`), `encode_provider_candidate` (`:137`), `encode_completion_roster` (`:202`), `encode_completion_claim_source` (`:214`), `encode_claim_projection` (`:248`), `encode_content_algebra` (`:259`), `encode_effect` (`:267`), `encode_ownership_roster` (`:272`), `encode_cleanup_action` (`:307`), `encode_operation_result` (`:345`), `encode_result` (`:362`).
3. In/out: reads `LegalizedBoundarySettlement`, `NativeCallOrigin`, `ProviderCandidateConformance`, `OwnershipEvent`, `TerminalAffineCleanupAction`. Writes bytes.
4. Decides: tags per variant. `NativeCallOrigin::InstalledProvider` writes tag 4 when `assumption.is_some()` else 2 (`:114`), and an `assumed-binding-contract\0` marker (`:118`). The ownership event tags are 1-6 (`:276-302`).
5. Cases by shape: fixed enums only (`CompletionClaimSource`, `ContentPlaceSegment` 3 forms `:226-240`, `ContentAlgebraKind` 2 forms `:260`).
6. Same job elsewhere: `encode_content_algebra` is defined in four places (grepped `fn encode_content_algebra`): here `:259`, `abstract-operations/optimization-unit/src/identity.rs:2163`, `register-homes/src/recovery/structural_contracts.rs:881`, `psi/artifact/declaration-encoding/src/terms/content.rs:156`. `encode_claim_projection` is defined here (`:248`) and at `optimization-unit/src/identity.rs:2154` (grepped `fn encode_claim_projection`; other sites not read, so "suspected" duplicate encodings).
7. Unused code: `encode_claim_projection` (here) has 1 call, `structural.rs:243` (grepped, 4 lines/2 files; the second file defines its own). `encode_completion_claim_source` 9 lines/3 files.
8. Stated limits: none.
9. Fail-open: none seen.
Tests (read, `:381-522`): two tests check that changing owned-cleanup path or any of 13 reference-origin mutations changes the encoded bytes (`:390-421`, `:434-521`). Keyed on bytes inequality.

### A4. `TV/identity/structural_types.rs` (897 lines; tests `:586-897`)
1. Purpose: encoders for structural type declarations, parameters, places, paths, arguments, field storage recipes and scalar restrictions, so plan identity and the recovery codec share one wire format (`:1-7`).
2. Shape: `encode_window_root` (`:21`), `encode_structural_parameter` (`:49`), `encode_target_structural_parameter` (`:69`), `encode_projected_qualifications` (`:82`), `encode_structural_argument` (`:93`), `encode_reference_origin` (`:99`), `encode_target_structural_argument` (`:129`), `encode_entry_claim` (`:181`), `encode_structural_path` (`:187`), `encode_structural_place` (`:203`), `encode_structural_type` (`:261`), `encode_authored_field_storage` (`:388`), `encode_structural_fields` (`:437`), `encode_byte_sequence_carrier` (`:455`), `encode_multiplicity` (`:478`), `encode_access` (`:486`), `encode_string` (`:495`), `encode_record_initializers` (`:500`), private `encode_scalar_restriction` (`:524`).
3. In/out: reads `terminal_psi` declarations. For refinements it calls `psi_declaration_encoding::...canonical_proposition_order_key(...).expect("canonical refinement predicate")` (`:531-532`).
4. Decides: tags per `StructuralTypeShape` variant (`:274-385`). `Opaque` encodes 19 and `TypeParameter` 20, with the comment "This tag distinguishes hash inputs; executable codecs reject open storage." (`:275`). A `PrimitiveScalar` encodes 12 if it has a `scalar_restriction`, else 6 (`:284`).
5. Cases by shape: `StructuralRecordStorage` five forms (`:311-332`); only `Authored`, `AccessView`, `Accessor` also encode per-field storage (`:334-343`); `ArrayExtent` three forms (`:348-362`).
6. Same job elsewhere: `encode_structural_type` is referenced by 31 lines in 7 files (grepped). `register-homes` recovery codec is a suspected second implementation.
7. Unused code: `encode_record_initializers` has 2 call sites, both in `representations/target-operations/legalized-operations/src/identity/scalar_graph.rs:415,427` (grepped). `encode_window_root` 6 lines/3 files. `encode_structural_place` 9 lines/4 files.
8. Stated limits: the `:275` comment above.
9. Fail-open risks: `expect("canonical refinement predicate")` at `:532` panics if a refinement has no canonical order key; it does not accept. None seen otherwise (all matches exhaustive). Classification: none.
Tests (read): ten tests check that encodings differ for varying access ceilings, multiplicity, linear-claim flag, block place, projected qualifications, inline carrier, refinement predicate, nominal cleanup selection; one pins the runtime-path segment tag byte (`:602-616`).

### A5. `TV/operation_records.rs` (186 lines)
1. Purpose: leaf records on target operations: `NativeCallOrigin`, `TargetFloatResultPolicy`, `TargetInstallationCall`, `TargetStructuralRuntimeIndex`, and two functions mapping an atomic access event plus a logical type to a storage integer type (`:1-2`).
2. Shape: `NativeCallOrigin` (`:13`, variants `Authored`, `ProgramEntry{boundary, entry, source_signature:[u8;32], ...}`, `InstalledProvider{boundary, provider, assumption, ...}`), `TargetFloatResultPolicy` (`:32`) with `operands` (`:43`) and `semantic_policy` (`:55`), `TargetInstallationCall` (`:72`), `TargetStructuralRuntimeIndex` (`:80`), `atomic_storage_integer_type` (`:89`), `atomic_storage_integer_type_closed` (`:97`).
3. In/out: pure data and a pure function. The function reads an `AtomicAccessEvent` and a closed scalar type.
4. Decides: `atomic_storage_integer_type_closed` returns `None` unless `event.ordering_is_legal()` (`:101`); a `Boolean` is allowed only for `Load` and `Store` and maps to u8 (`:107-114`); an integer must not be an address type and must have a `ValueShape` (`:115-119`); everything else `None` (`:120`). Keyed on event kind and integer width/closure.
5. Cases by shape: only Load/Store booleans and fixed-width (8/16/32/64) non-address integers (`:106-120`; test `:146-157` shows 8,16,32,64 pass and 13,128 fail).
6. Same job elsewhere: none seen.
7. Unused code: `atomic_storage_integer_type` 8 lines/3 files; `_closed` 9 lines/6 files (grepped).
8. Stated limits: the comment at `:104-105` says artifact-backed consumers supply their retained closed descriptor and metadata callers close once in the wrapper.
9. Fail-open: none seen; `None` means refusal.

### A6. `TV/provenance.rs` (145 lines)
1. Purpose: Terminal Psi provenance and `CallSiteOwner`, the identity of the place a call occurs, plus `ProgramEntryOrigin` (`:1-2`).
2. Shape: `TerminalPsiProvenance` (`:7`), `InstallationCallRole` (`:13`, `Validate`/`Work`), `CallSiteOwner` (`:19`, five variants: `Operation`, `InvokeNoNormal`, `ExternalCompletion`, `CleanupAction{edge, action_ordinal}`, `Installation{operation, role}`) with `operation`, `edge`, `cleanup_action_ordinal` (`:34,43,52`), `From<InvocationSite>` (`:63`), `ProgramEntryOrigin` (`:81`).
3. In/out: data. Converts `InvocationSite` to `CallSiteOwner` one to one.
4. Decides: nothing beyond projections keyed on variant (identity is by id plus ordinal).
5. Cases by shape: five owner kinds.
6. Same job elsewhere: none seen.
7. Unused code: `CallSiteOwner` has 295 lines in 58 files (grepped), so it is widely shared.
8. Stated limits: none.
9. Fail-open: none seen.
Tests (read): two tests check owner/edge/ordinal projections and that cleanup ordinals distinguish owners.

### A7. `TV/publication.rs` (396 lines; tests `:317-396`)
1. Purpose: native publication facts: source identities, quoted byte literals and code catalogs recorded while lowering owns a function, so later forms share a sealed roster (`:1-5`). Also entry custody facts (`NativeEntryFacts`).
2. Shape: `DeploymentEntryFacts` (`:24`), `NativeEntryFacts` (`:33`) with `selected_terminal_entry` (`:47`), `admits_hosted_entry` (`:69`), `native_execution_scope_for` (`:106`); `NativeByteLiteral` (`:141`); `NativeFunctionPublication` (`:148`) with `ordinal`, `byte_literal`, `call_target` (binary search by key, `:161,168,175`); `NativePublicationFacts` (`:184`) with `seal` (`:191`), `function` (`:257`), `identity_bytes` (`:264`), `encode` (`:268`); `NATIVE_PUBLICATION_DOMAIN = b"omega.native-publication-facts.v3\0"` (`:138`); private `encode` (`:281`).
3. In/out: `seal` takes rosters sorted by machine id, and returns an `Arc<[...]>` with a SHA-256 style `identity` from `DomainDigestEncoder` (`:240-245`).
4. Decides: `seal` rejects unordered or repeated machines (`:195-200`), unordered/repeated call targets or calls with no source occurrence (`:203-214`), unordered sites (`:215-217`), site ordinal `>= operation_count` (`:218-224`), unordered literals or literals without a source occurrence (`:225-238`). All keyed on identity (machine id, operation id, site). `selected_terminal_entry` rejects if terminal identity or entry differ, or the profile's native realization differs from the target (`:53-65`).
5. Cases by shape: `PsiProvenance` is `Operation` or `Edge` (`:293-296`).
6. Same job elsewhere: none seen.
7. Unused code: `NativePublicationFacts` 43 lines/11 files (grepped).
8. Stated limits: none.
9. Fail-open risks:
   - `selected_terminal_entry` returns `Ok(None)` when `deployment` is `None` (`:56-58`): an entry with no recorded deployment passes with no target check. Open (callers are in stage crates, not read).
   - `native_execution_scope_for` returns `Ok(None)` when no scope is recorded (`:113-115`): no check that the artifact, target, entry, provider closure match. Open.
   - `admits_hosted_entry` with `deployment == None` falls back to `original_entry == Machine(entry)` (`:81-83`). Open.
Tests (read): `seal` shares payload `Arc`s and commits ordinals and bytes (`:343-375`); `seal` refuses duplicate, missing and out-of-range rows (`:378-395`).

### A8. `TV/publication/emission.rs` (104 lines)
1. Purpose: provenance and obligations carried out of native planning (`:1`).
2. Shape: `EmissionProgramFacts` (`:6`), `EmissionFunctionFacts` (`:13`, fields include `source_occurrences`, `requires_graph_storage_replay`, `descriptors`, `primitive_locals`, `scalar_call_results`, `ram_obligations`), `EmissionOperationProvenance` (`:31`), `DescriptorProducerFact`/`DescriptorProducer` (`:40,46`), `PrimitiveLocalFact` (`:56`), `ScalarCallResultFact`/`ScalarCallOrigin` (`:74,81`), `RamObligation`/`RamSource` (`:87,93`), `OptimizedProgramProjection` (`:100`); `EmissionFunctionFacts::primitive_local` (`:63`).
3. In/out: data only.
4. Decides: `primitive_local` returns a row only if exactly one row has that place (`:64-69`): keyed on place id; ambiguous means `None`.
5. Cases by shape: none.
6. Same job elsewhere: none seen.
7. Unused code: not checked (all `pub` data types re-exported through `publication.rs:8-12`).
8. Stated limits: comment at `:28-29`: "These coordinates carry attribution; they cannot reconstruct executable code."
9. Fail-open: none seen.

### A9. `TV/scalar_operands.rs` (101 lines)
1. Purpose: scalar and call operands shared by the current native forms (`:1`).
2. Shape: `LegalizedValueDefinition` (`:12`), `LegalizedRuntimeIndexOperand` (`:19`, with `accepted_fact`), `LegalizedScalarCall` (`:28`), `LegalizedScalarArgument` (`:43`, `Scalar` or `Structural`), `LegalizedNormalizedForeignCall` (`:56`), `LegalizedStructuralCasePayload` (`:97`); `matches_target` (`:72`), `scalar_source`, `placement` (`:82,88`).
3. In/out: data; `matches_target` compares a semantic index operand to a target index.
4. Decides: `matches_target` compares `value`, `scalar_type`, `stride`, `extent`, `obligation` (`:73-77`); it explicitly leaves ABI location to other rules (`:69-71`).
5. Cases by shape: none.
6. Same job elsewhere: none seen.
7. Unused code: not counted.
8. Stated limits: none.
9. Fail-open: none seen.

### A10. `TV/storage.rs` (166 lines)
1. Purpose: durable scalar and structural home requirements and ABI parameter locations (`:1-2`).
2. Shape: `TargetUnitScalarHomeRequirement` (`:16`), `TargetStructuralHomeLayout` (`:24`, `Aggregate(ValueShape)` or `Sum(ConventionalSumLayout)`), `TargetStructuralHomeOrigin` (`:46`, `OperationResult` or `BlockParameter`), `TargetStructuralHomeRequirement` (`:58`) with projection accessors (`:63-155`), `ScalarParameterLocation` (`:161`, `Register` or `IncomingStack{byte_offset}`).
3. In/out: data; accessors read the origin variant.
4. Decides: `access()` returns `Owned` for an operation result and the declaration's access for a block parameter (`:98-103`); `has_claims` true only for an operation result with claims (`:152-155`).
5. Cases by shape: two origins, two layouts.
6. Same job elsewhere: none seen.
7. Unused code: `TargetStructuralHomeRequirement` 99 lines/21 files (grepped).
8. Stated limits: none.
9. Fail-open: none seen.

### A11. `TV/values.rs` (622 lines)
1. Purpose: target-lowered values pairing semantic scalar/Boolean/integer/view/structural expressions with ABI placements, byte offsets and reference origins (`:1-5`).
2. Shape: `TargetStructuralParameter` (`:19`), `TargetStructuralArgument` (`:30`), `TargetReferenceResult` (`:46`), `TargetReferenceOrigin` (`:52`), `TargetReferenceOriginBase` (`:59`, four forms: `Storage`, `IncomingContents`, `Invocation`, `Provider`), `ReferenceRoot` (`:154`, `Place`, `Ingress`, `Alternatives`), `TargetStructuralArgumentSource` (`:238`, six forms), `TargetByteView` (`:265`), `TargetElementView` (`:303`), `TargetViewExtent` (`:346`), `TargetElementViewRegion` (`:353`), `TargetScalarImmediate` (`:367`), `TargetScalarExpression` (`:385`), `TargetBooleanExpression` (`:418`, 15 variants), `TargetIntegerExpression` (`:494`, 23 variants), `TargetScalarBlockValue` (`:618`).
3. In/out: pure data. `ReferenceRoot::projected_family` (`:180`) rewrites origin paths and sorts and dedups (`:201-203`); collapses to `Ingress` when there is exactly one exact `Storage` origin (`:195-200`).
4. Decides: `exact_path` returns `Some` only for `Storage` base, `Projected` extent, and all `Exact` projections (`:137-150`). `storage_envelope` returns `None` for `Invocation` and `Provider` bases (`:115-117`). `operand_value` returns a value id only for `Immediate`, `Parameter`, `ScalarHome`, `BlockParameter` (`:605-613`; wildcard `_ => None` at `:612`).
5. Cases by shape: expression trees are fixed lists of operators (bitwise and/or/xor/not, widen, exact cast, shift, arithmetic, trapping, comparisons for integer and Boolean). The file's header says this consolidates structural parameter, reference-origin, view and expression records "into one module" (`:2`).
6. Same job elsewhere: none seen.
7. Unused code / canonical claim: header says `ReferenceRoot` and `TargetReferenceOrigin` give "one canonical definition of storage vs. incoming-contents provenance" shared by lowering and selection (`:4`). Grepped `ReferenceRoot`: 21 lines in 4 files (so users are few; the files were not read).
8. Stated limits: none.
9. Fail-open: `operand_value` `_ => None` (`:612`) means "not a plain operand": callers must treat as recipe. Traced: not traced to callers. Classification: open, low risk (returns a refusal-like `None`).

---

## B. calling-conventions (`P/targets/calling-conventions/src/`)

### B1. `CC/lib.rs` (107 lines)
1. Purpose: crate root. Three block comments describe the crate as one normalized contract (calling plans, value classification, layouts, callbacks, host operations) and give a reading order (`:1-32`).
2. Shape: private mods `aggregate_layout`, `borrowed_storage`, `boundary_application`, `boundary_values`, `callback_materializations`, `host_operations`, `plans`, `structural_shapes`; pub mods `process_entry`, `terminal_layout` (`:36-47`); big re-export list (`:48-89`); two structs `NativeForeignBridgeRequest` (`:93`) and `PublishedForeignAdapterIdentity` (`:102`).
3. In/out: none.
4. Decides: nothing.
5. Cases by shape: none.
6. Same job elsewhere: none.
7. Unused code: n/a.
8. Stated limits: none.
9. Fail-open: none seen.

### B2. `CC/aggregate_layout.rs` (201 lines)
1. Purpose: conventional storage layout for closed sums and mixed record-sums: `i32` tag at byte 0, common fields after the tag, one shared payload base aligned to the strictest alignment (`:1-9`).
2. Shape: `PackedFieldLayout` (`:14`), `ConventionalSumCaseLayout` (`:20`), `ConventionalSumLayout` (`:25`), `AggregateLayoutError` (`:35`), `evaluate_conventional_sum_layout` (`:41`).
3. In/out: takes common and per-case `ValueShape` lists; delegates the arithmetic to `layout_plans::packing::conventional_sum_geometry` (`:49`) and maps offsets back; tag geometry from `runtime_abi::CASE_TAG_BYTE_SIZE/ALIGNMENT` (`:46-47`).
4. Decides: errors on empty case set, zero alignment, size overflow, `u32`/`u16` narrowing (`:56-60`, `:76-77`, `:93-94`, `:108-109`).
5. Cases by shape: tag fixed at offset 0 (`tag_byte_offset: 0`, `:73`).
6. Same job elsewhere: called by `terminal_layout.rs:940` (`conventional_sum_layout_from_parts`) (read).
7. Unused code: grepped `evaluate_conventional_sum_layout`: 16 lines/8 files.
8. Stated limits: none.
9. Fail-open: none seen (errors on overflow). Tests (`:115-201`) read: payloadless sum is one i32 tag; mixed layout aligns the payload base; empty set and oversized payload fail closed.

### B3. `CC/borrowed_storage.rs` (228 lines)
1. Purpose: describes the physical leaves a host may transfer at a structural boundary: scalar fields, case discriminators and opaque regions (`:1-3`).
2. Shape: `impl NativeGeometry { fn borrowed_storage }` (`:11-32`); private `StorageRecipes` (`:51`) with `collect` (`:62`) and `members` (`:160`); type aliases `Field`, `Discriminator`, `OpaqueRegion` (`:35-49`).
3. In/out: reads a `NativeGeometry` and a root type id; returns `Option<NativeBorrowedStorageLayout>` (`runtime_abi`). Any failure returns `None` via `?`.
4. Decides: by structural type shape. `TypeParameter`/`Opaque` return `None` (`:75`); `PrimitiveScalar` and `AtomicCell` become fields; `Reference`/`ByteSequence`/`ElementView` become opaque regions (`:95-105`); `Record`, `Sum`/`Mixed` (adds a discriminator with native tag = case ordinal, `:115-124`), and `FixedArray` (only literal length, `length.literal()?` `:140`; stride aligned element size). A live field of a sum that has its own authored storage returns `None` (`:179-181`). Cycles are cut by an `active` stack (`:69-73`, comment `:68`).
5. Cases by shape: fixed array only with literal length.
6. Same job elsewhere: leaf collection also in `structural_shapes.rs:44` and `boundary_values.rs:221` (different output: classes versus recipes).
7. Unused code: none counted.
8. Stated limits: module comment `:3` "Missing geometry has no recipe."
9. Fail-open: none seen; missing information gives `None`.

### B4. `CC/boundary_application.rs` (616 lines)
1. Purpose: retains a completed calling-policy application (`BoundaryCallingApplication`) with a source-free signature, plus recovery and commitment verification (`BoundaryCallingApplicationCandidate`, `recomputed_commitment`) (`:1-9`).
2. Shape: `BoundaryCallingApplication` (`:32`), `BoundarySignatureGeometry` (`:48`), `NativeCallProjection` (`:61`, `new` `:70`, `parameters` `:118`, `parameter_ordinal` `:126`), `recomputed_commitment` (`:223`), `BoundaryCallbackBinding` (`:411`), `BoundaryNativeParameterShape/Geometry` (`:432,438`), `BoundaryOpaqueApplication` (`:475`), `BoundaryCallingApplicationCandidate` (`:484`) with `validate` (`:500`). Submodules `callback_catalog`, `native_telescope`, `terminal_telescope` (`:19-21`).
3. In/out: `validate` takes an untrusted candidate, recomputes shapes with `BoundaryValueGraph::classify`, re-validates the plan with `validate_boundary_entry_plan[_with_callback_materializations]`, then requires the stored `commitment` to equal `recomputed_commitment()` (`:608-613`). The commitment is a `DomainDigestEncoder` over domain `"omega.boundary-plan-application.v11"` (`:233-234`).
4. Decides: keyed on identity and structure. `NativeCallProjection::new` rejects a dispatch formal outside the semantic count, a row whose `native_ordinal` differs from its position, a repeated identity, a repeated or out-of-range semantic formal, and any missing semantic formal (`:75-105`). `recomputed_commitment` rejects an ABI whose `requirement_identity` string differs (`:225`, string compare), an empty selected owner (`:248`), stale arena storage (`:231,360,370`), a binder whose `callback_requirement_id(requirement_identity)` differs from the stored id (`:374-376`), and a context whose binders do not match one to one (`:390-402`). `validate` requires `policy.architecture() == initial_regime.architecture()` (`:515`), and that `plan` equals the validated plan (`:591`).
5. Cases by shape: target function-pointer parameter must equal the target's pointer size/alignment (`:551-559`).
6. Same job elsewhere: callback-binder join also checked in `callback_materializations.rs:132-219` (same unit; two layers: plan-level vs application-level).
7. Unused code: `NativeCallProjection` 13 lines/4 files (grepped; one outside file is `pipeline/01.../terminal_signature.rs`); `BoundaryCallingApplicationCandidate` used by `pipeline/01` (map).
8. Stated limits: none.
9. Fail-open: none seen; every inconsistency returns `Err`.
Tests (`:144-209`): dispatch omits the selected formal and keeps callback order; a passed table preserves every parameter; a dispatch selection cannot hide a missing or repeated formal.

### B5. `CC/boundary_application/callback_catalog.rs` (587 lines)
1. Purpose: source-free callback layout geometry (`BoundaryCallbackLayout`, `CallbackDataGeometry`, ...), arena-backed placement fragments (`PrivateCallbackPlacements`), authenticated placement admission and recovery of the Terminal slot selection (`:1-11`).
2. Shape: `CallbackDataGeometry` (`:27`), `CallbackInlineFieldGeometry` (`:48`), `CallbackSlotGeometry` (`:55`), `BoundaryCallbackLayout` (`:69`) with `field_placements` (`:134`) and `encode` (`:206`), `PrivateCallbackPlacements` (`:276`), `encode_private_callback_placements` (`:307`), `BoundaryCallingApplication::authenticated_callback_placements` (`:355`), `callback_destination_slot` (`:499`).
3. In/out: `field_placements` walks the inline path from the root checking containment, composes the terminal slot placements and re-validates against the root (`:134-204`). `callback_destination_slot` reads a `TerminalModule` (`calling_abi_signature`, native layouts) and returns the unique `PrivateCallbackSlotSelection` (`:499-587`).
4. Decides: keyed on identity strings and ids. `field_placements` requires the terminal slot to be pointer-sized (`:176-177`), its `slot` and `requirement` ids to equal ids recomputed from the identity strings (`:173-174`), and a nonzero conformance commitment (`:175`). `authenticated_callback_placements` requires `expected_commitment != [0;32]` and equals both stored and recomputed commitments and a closed context (`:363-367`), and exactly one row per `(binder, destination)` (`:437-446`). `callback_destination_slot` matches fields by `module.identities.text(...) == Some(name)` string compare (`:533`, `:541-542`, `:559`) and demands by `slot_identity == layout.terminal.identity` (`:570`) with a unique match (`:571`).
5. Cases by shape: `BoundaryCallbackLayout::encode` returns Err for a non-`Field` destination (`:222`). Only `NativePlace::Parameter` or `Field` exist.
6. Same job elsewhere: `callback_materializations.rs` validates the same binder/demand join at plan level.
7. Unused code: `authenticated_callback_placements` 2 call sites (`provider-admission/src/settlements_source_imports.rs:462`, `static_interrupts.rs:295`); `callback_destination_slot` 6 lines/5 files (grepped); `callback_layout_plan_id` 1 call (`targets/layout/src/lib.rs:187`).
8. Stated limits: none.
9. Fail-open: `PrivateCallbackPlacements::as_slice` uses `.expect("private callback placement storage")` (`:297`): panics on stale handle, does not accept. None seen otherwise.

### B6. `CC/boundary_application/native_telescope.rs` (258 lines)
1. Purpose: closes semantic formals plus native-only callback parameters into one ordered telescope with fresh identities and contiguous ordinals (`:1-7`).
2. Shape: `NativeParameterTelescope` (`:17`) with `close` (`:37`), `direct_demands` (`:87`); `BoundaryNativeParameterShape::function_pointer` (`:22`); `validate_fresh_native_parameter_identity` (`:102`); `BoundaryCallingApplication::foreign_scalar_parameters` (`:117`).
3. In/out: takes semantic geometry rows and `CallingDirectCallback` rows plus a target; sorts by `native_ordinal` (`:75`) and requires ordinals 0..n (`:76-83`). `foreign_scalar_parameters` maps a formal's scalar type to its native ordinal slot.
4. Decides: keyed on identity and ordinal. Refuses a native-only parameter in the semantic input (`:44-51`), a repeated identity (`:107-111`), a gap or repeat in ordinals (`:76-83`). `function_pointer` refuses any target whose pointer is not 8/8 (`:23-28`).
5. Cases by shape: only 8-byte pointers (stated limit below).
6. Same job elsewhere: `terminal_telescope.rs:96` calls `close` to recompute the telescope.
7. Unused code: `foreign_scalar_parameters` 1 call (`P/driver/runtime/mod.rs:48`); `validate_fresh_native_parameter_identity` 7 lines/4 files.
8. Stated limits: "direct callback parameters currently require an admitted 8/8 target function-pointer shape" (`:24`).
9. Fail-open: none seen. Tests (`:170-257`) read.

### B7. `CC/boundary_application/terminal_telescope.rs` (413 lines)
1. Purpose: recovers native argument order and callback binder correspondence from the canonical Terminal declaration and checks a proposed `BoundaryCallingApplication` against it (`:1-7`).
2. Shape: `TerminalCallingTelescope` (`:17`) with `callback_bindings` (`:23`) and `for_application` (`:47`).
3. In/out: reads a `TerminalModule` (`interfaces.calling_signatures`, `calling_abi_signature`, `boundary_machine`, `foreign_reference_descriptor`) and the application. Returns the recomputed parameters and binders or `Err`.
4. Decides: ABI identity equality (`:60-62`), parameter count equality (`:71-77`), recomputed native parameters must equal proposed (`:107-111`), reference geometry class/size/alignment must match the Terminal transport role (`:154-158`; descriptor role uses `runtime_abi::fat_descriptor_layout`, else pointer size, `:139-147`), binders must equal proposed (`:167-171`). A structural parameter whose type is not an owned reference/view is skipped without a geometry check (`:135` `let Some(referent) = referent else { continue }`).
5. Cases by shape: a structural parameter is checked only when `access != Owned`, or the owned type shape is `Reference`, `ByteSequence` or `ElementView` (`:120-134`).
6. Same job elsewhere: none seen.
7. Unused code: `TerminalCallingTelescope::for_application` not grepped by name; it is a `pub` method used by tests here (`:369`) and by pipeline 01 (inferred).
8. Stated limits: none.
9. Fail-open: `:135` skip for owned non-reference structural parameters accepts them without a ref-geometry check. Stated by structure (references only need transport-role checks). Classification: open for other kinds, since nothing at this site checks an owned record's size/alignment.
Tests (read `:179-413`): owner string is part of the commitment; a "phantom local application" with equal geometry cannot borrow another ABI.

### B8. `CC/boundary_values.rs` (685 lines; tests `:277-685`)
1. Purpose: source-free public boundary value graph and per-policy ABI classification of recursive shapes (`:1-9`).
2. Shape: `BoundaryOpaqueRepresentationMovementRole` (`:15`), `BoundaryValueClass` (`:24`), `BoundaryValueShape` (`:33`), `BoundaryValueField` (`:40`), `BoundaryValueGraph` (`:69`) with `classify` (`:75`) and `validate_abi_shape` (`:94`); private `classify_boundary_aggregate` (`:121`), `homogeneous_float_leaf_shape` (`:197`), `collect_boundary_scalar_leaves` (`:221`).
3. In/out: graph of `u16`-indexed shapes and fields; output a `ValueShape` with `ValueClass`.
4. Decides: keyed on policy and recursive leaf offsets/sizes/float flags. Microsoft x64 returns `Integer` for every aggregate (`:126-128`); AAPCS64 and SysV classify four-or-fewer equal float members as HFA (`:135-140`); SysV needs `members > 1 && byte_size <= 16` (`:137`); a single float member is `Float` under SysV (`:141-143`); Darwin non-HFA is `IntegerAggregate` (`:145-147`); anything else over 16 bytes or non-SysV is `Integer` (`:148-150`); then eightbyte class logic (`:151-194`). `collect_boundary_scalar_leaves` refuses depth over 32 (`:228-230`). Leaves crossing an eightbyte are refused (`:158-162`).
5. Cases by shape: HFA 1-4 members of size 4 or 8 only (`:206-208`); two-eightbyte SysV classes only; "one-eightbyte non-homogeneous SSE aggregates" are an error (`:177-182`).
6. Same job elsewhere: this is the same classification as `structural_shapes.rs::classify_scalar_leaves` (`:196-261`) (near-identical body, `u16` vs `u32` leaf offsets) and the HFA test `homogeneous_float_leaves` (`structural_shapes.rs:263`). A third inline HFA rule is in `targets/layout/src/boundary_values.rs:276-283`. Leaf collectors: `:221` vs `structural_shapes.rs:44`. Grepped `homogeneous_float` `fn` definitions: only the two named helpers plus the `ValueShape::homogeneous_float_aggregate` constructor.
7. Unused code: `byte_size_at_offset_zero`, `gpr_code` have 5 files each (grepped). `BoundaryOpaqueRepresentationMovementRole` 11 lines/6 files.
8. Stated limits: `:179` "one-eightbyte non-homogeneous SSE aggregates are not representable in the closed ABI vocabulary".
9. Fail-open: none seen; unknown shapes return `Err`.
Tests read: Darwin composite identity, stack slot packing, HFA cutoffs per policy, malformed graphs rejected.

### B9. `CC/callback_materializations.rs` (429 lines)
1. Purpose: plan-level callback binder to native-place join: every declared static-machine binder and private native-place demand must join one to one with no overlapping destinations (`:1-8`).
2. Shape: macro `nominal_plan_id!` (`:12`) defining `LayoutPlanId`, `LayoutSlotId`; `NativeParameterApplication` (`:33`); re-exports from `semantic_vocabulary` (`:40-44`); `callback_layout_plan_id` (`:46`), `callback_plan_laid_layout_id` (`:62`), `callback_layout_slot_id` (`:82`), `callback_layout_field_slot_id` (`:90`); `NativePlace` (`:99`); `CallbackMaterialization` (`:109`); `CallbackBinderRequirement` (`:115`); `NativeCallbackDemand` (`:121`); `CallbackMaterializationContext` (`:127`); `validate_callback_materializations` (`:132`); `native_places_overlap` (`:232`).
3. In/out: reads rows and a context; returns `Ok` or `PlanDiagnostic`.
4. Decides: by id equality. Refuses a repeated binder id in context (`:136-145`), overlapping demands (`:146-156`), a row naming an unknown binder (`:160-168`), a repeated row binder (`:169-173`), overlapping row destinations (`:174-181`), a row with no demand at that destination (`:182-191`), differing requirement ids (`:192-197`), an unmatched context binder (`:200-208`) or demand (`:209-217`). Overlap: `Parameter` vs `Field` of the same parameter overlap; two `Field`s overlap when layouts differ or one path starts with the other (`:232-265`).
5. Cases by shape: `NativePlace` only `Parameter` or `Field` (`:99`); an empty `field_path` is refused (`:222-229`).
6. Same job elsewhere: binder join repeated at application level in `callback_catalog.rs:383-436` and `boundary_application.rs:390-402`.
7. Unused code: `callback_layout_plan_id` see B5.
8. Stated limits: comment `:5-7` says nominal ids "act as compact discriminators"; authority comes from replaying the owning catalog.
9. Fail-open: none seen. Note identity ids are hashes (`callback_nominal_report_fingerprint`, `u64`) with `.expect("... is nonzero")` (`:59,79,87,95`), so a hash of zero would panic, not accept.

### B10. `CC/host_operations.rs` (568 lines)
1. Purpose: keyed catalogs of host capabilities and operations with process-wide interning of custom names, external binding rows, Darwin and Windows import library maps, and x86-64 interrupt gate address assembly (`:1-13`).
2. Shape: `intern_custom_name` (`:26`, leaks a boxed `str` with `Box::leak` and appends to a `static Mutex<Vec<&'static str>>`, `:20-35`), `HostCapability` (`:57`) with `from_name`/`name` (`:89,94`), `HostOperation` (`:105`, 90 built-in variants) with `from_name`/`name` (`:317,322`), `ExternalBindingRow` (`:333`) with `calling_application` (`:344`) and `boundary_entry_plan` (`:356`), `ExternalBindingKind` (`:366`, six forms), `darwin_import_library` (`:380`), `WINDOWS_IMPORT_ROWS` (`:403`, 52 rows), `windows_import_library` (`:467`), `InterruptGateAddress` (`:475`), `interrupt_gate_address` (`:481`).
3. In/out: name tables read both ways. `interrupt_gate_address` reads `LayoutPlacementReport::Bits` fragments and a table size; returns fragments, envelope offset and gate byte offset.
4. Decides: by spelling. `from_name` does `row_for_spelling` by string compare against the table and otherwise interns a `Custom(index)` (`:89-92`, `:317-320`). `darwin_import_library` decides by symbol prefix (`_objc_`, `_sel_`, `_class_`, `_object_`, `_method_`, `_ivar_`, `_protocol_` give libobjc; `_CG` gives CoreGraphics; everything else libSystem, `:381-394`). `windows_import_library` finds the first row whose symbol (field 4) equals the name (`:467-472`), so fields 1-3 (capability, operation, and library) only matter for library. `interrupt_gate_address` accepts only `Bits` placements (`:489-498`), container width multiple of 8 up to 64 (`:499-512`), distinct source bits (`:516-518`), maps hardware bit `0..=15` to itself and others to `+32` (`:520-523`), requires base `% 128 == 0` and one gate (`:531-533`), all 64 bits covered (`:544`), gate fits table with 16 bytes (`:549-554`).
5. Cases by shape: Windows import table covers Stdin, Stdout, Stderr, Process, Clock, Input, Gui, Filesystem rows only (`:404-464`); the interrupt-gate bit mapping is x86-64 IDT-specific (gate is 16 bytes `:550`).
6. Same job elsewhere: the first two fields of `WINDOWS_IMPORT_ROWS` duplicate `HostCapability`/`HostOperation` spellings (suspected redundancy; no code reads them, `:467-472`).
7. Unused code: grepped `HostCapability`: 18 lines/4 files, but outside `host_operations.rs` the only hits are `lib.rs:76` (re-export) and two unrelated `SymbolKind::HostCapability` uses in `declaration-identity/src/symbol.rs:67` and `source-to-psi/.../read.rs:747`. `HostOperation`: 101 lines/3 files; outside this file only `lib.rs:76` and an unrelated `HostOperation` enum in `tools/xtask/src/boundary_contracts.rs:50`. So `HostCapability`, `HostOperation`, `from_name`, `name` have 0 users outside their file in `source-rust`/`tools`. `ExternalBindingRow` 34 lines/17 files; `darwin_import_library` 2 call sites (`image-macho/src/dyld_linking.rs:117,1684`); `windows_import_library` 1 call site (`image-pe/src/imports.rs:83`); `interrupt_gate_address` 2 call sites (`pipeline/05.../selection_interrupts.rs:37`, `provider-admission/src/static_interrupts/scopes.rs:521`).
8. Stated limits: none.
9. Fail-open risks:
   - `HostCapability::from_name` / `HostOperation::from_name` accept any name as `Custom` (`:89-92`, `:317-320`). Traced: no caller exists, so no program reaches it.
   - `darwin_import_library` returns libSystem for any unrecognized symbol (`:394`). A misspelled or foreign symbol is bound to libSystem. Open (caller `dyld_linking.rs` not read).
   - `custom_name` returns `"<custom>"` for an unknown index (`:43`). Traced: unreachable without callers.
   - The interner never frees (`Box::leak`, `:33`): process-lifetime memory growth by distinct names.

### B11. `CC/plans.rs` (511 lines)
1. Purpose: pairs a `CallPlan` with a `StatePlan` into boundary entry plans, validates accepted/rejected policy outputs, and verifies state-footprint and generated-entry evidence (`:1-11`).
2. Shape: `PlanDiagnostic` (`:36`), `BoundaryPlanDiagnostic` (`:47`), `StateFootprintEvidence` (`:75`), `StateEffectsEvidence` (`:114`), `GeneratedEntryControl/Instruction/Evidence` (`:224,242,250`) with `from_target_instructions` (`:256`), `BoundaryEntryPlan` (`:315`) with `contract_commitment_digest` (`:322`), `BoundaryPlanResult` (`:331`), `CallingPolicyRejection` (`:337`), `ValidatedBoundaryEntryPlan` (`:354`), `evaluate_ordinary_boundary_entry_plan` (`:373`), `validate_boundary_entry_plan` (`:415`), `validate_boundary_entry_plan_with_callback_materializations` (`:424`), `validate_boundary_state_plan` (`:435`), `validate_boundary_plan_result` (`:481`), `canonicalize_boundary_entry_plan` (`:492`).
3. In/out: `ValidatedBoundaryEntryPlan` is a newtype with a private field, constructible only inside this file (`:354`, `:478`). Two identities: `contract_report_fingerprint` (64-bit FNV, `:361-366`) and `contract_commitment_digest` (strong, domain `"omega.boundary-calling-plan.v2"`, `:323`).
4. Decides: `validate_boundary_state_plan` refuses mismatched architectures (`:439`), saved state not within interrupted (`:444-452`), saved != restored (`:453-457`), permitted use overlapping interrupted state that is not saved (`:458-467`), ordinary clobbers outside permitted use (`:468-477`). `GeneratedEntryEvidence::from_target_instructions` refuses an unaccounted `LocalCall` when no body call field (`:270-274`), gaps/overlaps/empty rows (`:275-277`), overflow, byte excess, uncovered bytes (`:278-291`), call field mismatch (`:292-294`) and branch targets not on instruction boundaries (`:295-306`). Canonicalization sorts value locations by byte offset and callback rows by binder (`:493-503`).
5. Cases by shape: `evaluate_ordinary_boundary_entry_plan` sets `EntryStack::ProviderSelected`, empty interrupted/saved/restored sets (`:401-408`). It adds `Flags`, `InstructionPointer`, `StackPointer`, `ControlState` to permitted use (`:388-393`).
6. Same job elsewhere: `process_entry.rs:18` builds a second `StatePlan` with a different permitted set (includes `GeneralRegisters`, `VectorRegisters`).
7. Unused code: `validate_boundary_plan_result` 9 lines/4 files; `from_target_instructions` 2 call sites (`target-aarch64/src/generated_entry.rs:165`, `target-x86_64/src/generated_entry.rs:560`).
8. Stated limits: none.
9. Fail-open: `validate_boundary_entry_plan_with_callback_materializations` (`:424`) skips `validate_call_plan`'s refusal of nonempty callback rows (`:165-170` of `call_plan_evaluation.rs`) and instead uses the supplied `CallbackMaterializationContext`; an empty context with empty rows passes. That is intended by design (stated at `:165-168` of call_plan_evaluation via the message). Classification: stated.

### B12. `CC/plans/vocabulary.rs` (877 lines)
1. Purpose: vocabulary for calling plans and machine-state entry contracts: registers, register sets, value classes and shapes, locations, policies, `CallPlan`, state contracts, and the canonical `StatePlan` byte encoding (`:1-12`).
2. Shape: `MachineRegister` (`:19`, 19 variants incl. `X86Xmm(u8)`, `Aarch64X(u8)`, `Aarch64V(u8)`) with `codec_index`, `wire_class_index`, `architecture` (`:42,50,74`); `RegisterSet` (`:99`, sorted deduped vec); `SystemVEightbyteClass` (`:123`); `ValueClass` (`:129`, 7 variants); `ValueShape` (`:154`) with constructors; `CallSignature` (`:281`); `ValueLocation` (`:296`, `Register`/`Stack`/`Indirect`); `ValuePlacement` (`:403`) with `is_complete_scalar_result` (`:409`) and `has_same_parameter_transport` (`:430`); `CallingPolicy` (`:487`, six policies) with `native_for_target` (`:549`), `native_syscall_for_target` (`:564`), `ordinary_clobbers` (`:579`); `EntryControl`; `CallPlan` (`:640`); `MachineState` (`:653`, 9 states); `MachineStateSet(u16)` (`:666`); `MachineRegime`; `EntryStack`; `Preemption`; `StatePlan` (`:733`); `encode_state_plan_identity` (`:743`); `gpr_code` (`:790`); macro `register_or_stack_pair_pattern` (`:798`).
3. In/out: data and small pure functions. `ValueShape::standalone_integer` and `field_integer` read `layout_planning` geometry and `expect` the result (`:187,196-201`).
4. Decides: `ValueShape::for_closed_integer` returns a shape only for address-64 or fixed-carrier widths 8/16/32/64 (`:167-178`). `for_closed_scalar` maps callable to 8/8, Boolean to 1/1 (`:209-216`). `native_for_target` and `native_syscall_for_target` match `(architecture, object_format)`; the `Aarch64`+`Coff` pair calls `panic!` (`:556-560`, `:571-575`). `has_same_parameter_transport` accepts a `BorrowedReference` internal placement against an integer(8,8) boundary placement when the indirect pointer register or stack slot equals the external one (`:430-479`).
5. Cases by shape: `MachineStateSet` has nine states fitting `u16` (`:653-663`). `CallingPolicy` has fixed per-policy clobber lists (`:579-626`).
6. Same job elsewhere: two byte encodings of `StatePlan` (here `:743` one-based, and `plan_identity.rs:127` zero-based) are declared separate (`:8-11`).
7. Unused code: `byte_size_at_offset_zero` 9 lines/5 files; `gpr_code` 9 lines/5 files (grepped).
8. Stated limits: comment at `:493-497` on Darwin stack packing; none "unsupported".
9. Fail-open: `panic!` on `(Aarch64, Coff)` is fail-closed by crash. None seen otherwise. Tests (`:805-877`) read: integer shapes need closure.

### B13. `CC/plans/call_plan_evaluation.rs` (1085 lines)
1. Purpose: evaluates a deterministic `CallPlan` for each policy and validates any proposed `CallPlan` for coverage, register/stack disjointness, alignment and architecture (`:1-9`).
2. Shape: `evaluate_call_plan` (`:25`), `validate_signature_shapes` (`:78`), `validate_call_plan` (`:160`), `validate_call_plan_structure` (`:174`), `validate_value_placement` (`:319`), `evaluate_microsoft_x64` (`:452`), `evaluate_system_v_amd64` (`:532`), `evaluate_aapcs64` (`:579`), `evaluate_split_bank_call` (`:613`), `register_fragments` (`:811`), `spill_integer_fragments` (`:829`), `indirect_pointer` (`:849`), `reserve_indirect_copies` (`:867`), `evaluate_linux_syscall_x86_64/aarch64` (`:897,916`), `evaluate_syscall` (`:928`), `result_placement` (`:996`), `align_up`, `stack_end` (`:1075,1081`).
3. In/out: signature (shapes) to `CallPlan`; zero-sized non-borrowed parameters are removed before evaluation and reinserted with empty locations (`:30-66`).
4. Decides: by `ValueClass`, `byte_size`, policy. Microsoft x64: first four slots rcx/rdx/r8/r9 (or xmm0-3 for floats), sizes other than 1/2/4/8 passed indirectly, 32-byte shadow, aggregate result indirect via rcx (`:452-530`). SysV: six integer registers, 8 xmm; HFA split in eightbytes; MEMORY-class over 16 bytes goes to the stack (`:730-743`); all-or-nothing rollback (`:657-686`). AAPCS64: over 16 bytes passed by pointer (`:731-739`), 16-aligned values round to even register only for non-Darwin (`:749-751`), close float bank after a float spill (`:772-774`), Darwin packs stack at natural alignment (`:775-781`). Validation: parameter count equals signature (`:178`), result shape equals (`:185`), stack alignment power of two (`:190`), registers and stack ranges disjoint (`:210-296`), borrowed values need exactly one direct indirect location (`:324-336`), exact byte coverage (`:429-448`), clobbers and `SupervisorCall` register of the right architecture (`:300-315`).
5. Cases by shape: "Microsoft x64 aggregate classification is not normalized yet" is an error for HFA and SysV classes (`:465-472`). Syscall policies accept only `ValueClass::Integer` parameters up to six 8-byte words and integer results up to 8 bytes (`:938-980`). Signature shapes: `IntegerAggregate` only under Darwin, `Float` 4 or 8 bytes, SysV aggregate 9-16 bytes with an SSE eightbyte (`:104-155`).
6. Same job elsewhere: `evaluate_split_bank_call` serves both SysV and AAPCS64; no second evaluator seen. The map says calling policies are being moved to Omega policies (`TASKS.md`, from map).
7. Unused code: `evaluate_call_plan` used widely (not counted).
8. Stated limits: `:470` "Microsoft x64 aggregate classification is not normalized yet".
9. Fail-open: `validate_call_plan_structure` checks structure only (coverage, disjointness, alignment, architecture); it does not check that a supplied plan equals what the policy would produce, so any structurally valid placement from an authored policy is accepted (`:174-317`). Classification: stated (module comment `:4-6`: validates that "any proposed CallPlan satisfies exact value coverage, register/stack disjointness, alignment, and architecture constraints"). `debug_assert!(runtime_parameters.next().is_none())` at `:66` is compiled out in release.

### B14. `CC/plans/plan_identity.rs` (266 lines)
1. Purpose: compact 64-bit FNV-1a fingerprint and optional 32-byte domain-separated strong commitment over `CallPlan`, `StatePlan`, `StateFootprintEvidence` (`:1-7`).
2. Shape: `Fnv1a` (`:18`, `compact` plus `Option<DomainDigestEncoder>`), `new`, `with_strong_domain`, `finish`, `finish_strong` (`:24-46`), `call_plan` (`:71`), `state_plan` (`:127`), `entry_control`, `value_placement`, `value_shape`, `register_set`.
3. In/out: writes the same bytes to both hashers (`:48-53`).
4. Decides: variant codes zero-based (`:72-79`, `:236-251`). Empty callback catalogs omit the callback domain tag (`:95`).
5. Cases by shape: six policies, three entry controls.
6. Same job elsewhere: see B12 (`encode_state_plan_identity`).
7. Unused code: n/a.
8. Stated limits: none. The map's TASKS note says `plan_identity.rs` is to be derived from the normalized contract.
9. Fail-open: none seen. `finish_strong` panics if called without a strong domain (`:45`).

### B15. `CC/plans/tests.rs` + four test files (70 + 231 + 272 + 494 + 451 lines) and `CC/tests.rs` (6 lines)
1. Purpose: unit tests of plan evaluation: fixtures (`call_signature`, `integer_signature`, `strict_x86_entry`) in `tests.rs`; entry validation and pinned `StatePlan` bytes in `entries_footprints_and_policies.rs`; target-to-policy selection, syscalls and Darwin packing in `native_policies_and_syscalls.rs`; parameter/aggregate placement in `parameter_and_aggregate_placement.rs`; SysV records and borrowed references in `system_v_records_and_references.rs`. `CC/tests.rs` only declares `mod clang_crosscheck` (`:6`).
2. Shape: files grouped as one kind (tests); all read in full.
3. In/out: call `evaluate_call_plan`, `validate_call_plan`, `validate_boundary_entry_plan`.
4. Decides: pass/fail on exact locations and on error substrings (`"does not save"`, `"stack alignment"`, `"clobbers exceed"`, `"at least one SSE eightbyte"`, `"parameter-only"`, `"32-bit storage"`).
5. Cases by shape: placement cases listed per policy; syscall six-word capacity.
6. Same job elsewhere: the clang cross-check (B16) tests the same placements against clang.
7. Unused code: n/a.
8. Stated limits: tests assert `native_for_target`/`native_syscall_for_target` panic on `(Aarch64, Coff)` via `catch_unwind` (`native_policies_and_syscalls.rs:54-57,91-95`).
9. Fail-open: none seen.

### B16. `CC/tests/clang_crosscheck.rs` (387), `clang_mir.rs` (812), `signatures.rs` (253)
1. Purpose: cross-check calling plans against clang on four hosted targets. `signatures.rs` generates 1440 C signatures from a fixed seed (`clang_crosscheck.rs:32-33`; SplitMix64 `signatures.rs:138`); `clang_mir.rs` runs a tracer over clang MIR (`-stop-after=finalize-isel`) to find where each byte of each argument and result goes; `clang_crosscheck.rs` compares.
2. Shape: `clang_crosscheck` test (`:66`), `compare_target` (`:155`), `omega_plan` (`:196`), `omega_placements` (`:235`), `compared_rows` (`:318`); in `clang_mir.rs`, `caller_bodies` (`:84`), `Tracer` (`:255`), `trace_caller` (`:667`), `clang_placements` (`:743`); in `signatures.rs`, `generated_signatures` (`:208`), `c_source` (`:223`).
3. In/out: spawns `clang` (env `OMEGA_CLANG`, default `clang`), writes C into a temp dir, reads `.mir` files, compares with `evaluate_call_plan` output. It uses `classify_scalar_leaves` (`:21`, `:208`) from `structural_shapes.rs`.
4. Decides: keyed on source spelling of MIR opcodes, e.g. `"MOVi64imm"`, `"LDRXui"`, `"ADD64rr_DB"` prefix and equality matching (`clang_mir.rs:350-478`, `access_size` `:141-195`). An unknown opcode becomes a `HarnessFailure` (`:478`) and the signature is counted as a harness failure, not a mismatch (stated `:11-13`). Omega-refused signatures count as `refused` and are not failures (`clang_crosscheck.rs:164-168`: "A refused signature is safe").
5. Cases by shape: only the signature mix generated (`signatures.rs:176-206`: bare ints/floats, records of up to four scalars including `i128`, float records 1-4, narrow integer records). It "only understands the instructions clang 18 emits" (`clang_mir.rs:9`).
6. Same job elsewhere: none seen.
7. Unused code: n/a (test only).
8. Stated limits: `#[ignore]`, needs clang with AArch64 and X86 backends (`:12-15`).
9. Fail-open: the test returns early and passes when clang cannot run (`:69-78`, prints "skipped"); it is `#[ignore]` so it only runs with `--ignored`. Padding bytes are not compared (`value_bytes` covers leaves only, `signatures.rs:96-100`). Stated at `:1-16`.

### B17. `CC/process_entry.rs` (270 lines)
1. Purpose: target-authored process-entry calling conventions, arrival shapes and stack guarantees (`:1`).
2. Shape: `process_entry_state_plan` (`:18`), `exact_hosted_physical_boundary_entry_plan` (`:39`), `PhysicalArrival` (`:46`), `ARRIVALS: [PhysicalArrival; 6]` (`:108-163`), `physical_arrival` (`:165`), `ordinary_arrival` (`:172`), `linux_x86_64_arrival` (`:180`), `linux_arm64_arrival` (`:218`), `close_uefi_entry_stack` (`:257`).
3. In/out: from a `ProgramEntryPhysicalContractPackage` returns a `ValidatedBoundaryEntryPlan` or `None`.
4. Decides: `physical_arrival` finds the row by equality of `package` and `expect`s that one exists (`:165-170`). The Linux rows hand-build plans (Linux x86-64 `SupervisorCall` immediate 231 via `rax`, Linux arm64 immediate 94 via `x8`, `:207-209,246-248`).
5. Cases by shape: six packages (WindowsX64, MacosX64, MacosArm64, LinuxX86_64, LinuxArm64, UefiX64). The UEFI row has `replay: None` (`:161`) so `exact_hosted_physical_boundary_entry_plan` returns `None` for UEFI.
6. Same job elsewhere: `plans.rs:373` builds an ordinary state plan; this file builds a different state plan with a larger permitted set (`:24-33`).
7. Unused code: `exact_hosted_physical_boundary_entry_plan` has 1 non-test call (`verified-optimization-input/src/hosted_receiver.rs:155`) and 1 test call (`program-entry-contracts/physical_contract.rs:162`) (grepped).
8. Stated limits: none.
9. Fail-open: returning `None` for UEFI (`:42`, `:161`) means a caller that treats `None` as "no check" would skip the physical entry plan. Not traced (caller `hosted_receiver.rs:155` not read). Open.

### B18. `CC/structural_shapes.rs` (561 lines; tests `:281-561`)
1. Purpose: ABI classification of structural Terminal types over native geometry, using actual scalar leaves and authored offsets (`:1-5`).
2. Shape: `InvalidStructuralShape` (`:15`), `classified_boundary_shape` (`:17`), `collect_scalar_leaves` (`:44`), `collect_field_leaves` (`:144`), `classify_scalar_leaves` (`:196`, `pub(crate)`), `homogeneous_float_leaves` (`:263`).
3. In/out: reads a `NativeGeometry`; leaves are `(offset, size, is_float)`.
4. Decides: `classified_boundary_shape` accepts `Record`/`FixedArray` (via `record_or_fixed_array_shape_pattern!`), `PrimitiveScalar`, `Reference`, `ByteSequence(BorrowedView)`, `ElementView` (class taken from geometry) and refuses any other with `InvalidStructuralShape` (`:29-40`). Fixed array only with `ArrayExtent::Literal` (`:115-139`; others fall to `_ => Err` at `:139`). A field with fragmented storage fails through `field_placement` (see B19). Bounded-owned byte sequences expand to one-byte integer leaves (`:176-190`).
5. Cases by shape: above.
6. Same job elsewhere: duplicates `boundary_values.rs:121-261`.
7. Unused code: `classified_boundary_shape` 12 lines/4 files (grepped); `classify_scalar_leaves` also used by the clang test (`clang_crosscheck.rs:21`).
8. Stated limits: none.
9. Fail-open: none seen; unclassifiable gives `Err`.

### B19. `CC/terminal_layout.rs` (1296 lines; tests `:1101-1296`)
1. Purpose: native storage geometry from canonical Terminal declarations: `StructuralLayoutResolver` resolves demanded roots once; `StructuralLayouts` materializes the same computation for all types (`:1-9`).
2. Shape: `TerminalLayoutError` (`:22`, 12 variants), `NativeGeometry` (`:39`), `StructuralLayoutResolver` (`:150`, `RefCell` scratch `known` and `active` tables), `StructuralLayouts` (`:375`), `OwnedSequenceLayout` (`:508`), `borrowed_slice_element` (`:537`), `borrowed_element_index` (`:562`), `element_view_length_offset/at` (`:585,589`), `opaque_borrow_transport` (`:610`), `is_native_view_descriptor[_at]` (`:622,629`), `sequence_capacity` (`:637`), `owned_sequence_layout` (`:651`), `resolve_structural_shape` (`:683`), `conventional_sum_layout_from_parts` (`:904`), `scalar_field_shape` (`:981`), `byte_sequence_shape` (`:1026`), `checked_align_up_u32` (`:1044`), `scalar_shape` (`:1066`), `empty_primitive_array` (`:1078`).
3. In/out: reads `StructuralTypeDeclaration` rows (dense ids, `IdTable`) and owned-sequence bounds `&[(StructuralTypeId, u64)]`; returns `ValueShape`. Failed queries leave no cached shape (`:895-899`).
4. Decides: by structural type shape and id. `TypeParameter` gives `UnclosedTargetLayout` and `Opaque` gives `UnclosedOpaqueRepresentation` (`:712-717`). `Reference` is 8/8, or `ScalarReferenceLayout::NATIVE.byte_size` when the referent is a primitive scalar with a storage codec (`:718-730`). Owned `ElementView` needs an entry in `owned_sequences`, found by binary search; a duplicate gives `InvalidStorageGeometry` (`:637-649`). Borrowed `ElementView` is 16/8 (`:755-763`). `Record` with `TargetLayout` storage is `UnclosedTargetLayout` (`:765-770`); with `Authored`/`AccessView`/`Accessor` storage the declared size and alignment are returned if alignment is a power of two (`:771-786`); otherwise fields are laid conventionally (`:787-825`). `FixedArray` needs a literal length or an extent callback (`:828-836`); zero-length arrays of storage-codec scalars are 0/1 (`:837-839`). `Sum`/`Mixed` use `conventional_sum_layout_from_parts` (`:862-889`) with an empty case list refused (`:912-914`). `field_placement` refuses fragmented fields (`:234`) and any field extending past the owner size (`:257-262`).
5. Cases by shape: owned sequences need an explicit bound. `required_scalar_extents` collects unresolved `ScalarApplicationId`s instead of failing (`:472-493`).
6. Same job elsewhere: inside the file, `sum_layout` is written three times (`NativeGeometry` `:110` delegating; `StructuralLayoutResolver` `:318`; `StructuralLayouts` `:445` with the same body); `owned_sequence` twice (`:59`, `:395`); `field_shape` twice (`:212`, `:434`). Array stride rule here (`checked_align_up_u32(size, alignment)`, `:851`) versus `targets/layout/src/boundary_values.rs:165` (`size * length`, no alignment round-up).
7. Unused code / canonical claim: header says ABI classification and artifact checking use this owner "not independent approximations" (`:5-6`). Grepped `StructuralLayoutResolver`: 15 lines/2 files; `StructuralLayouts` 69 lines/10 files; `NativeGeometry` 333 lines/28 files. `borrowed_element_index` 2 call sites (`pipeline/04.../structural_layout.rs:1113`, `legalized-operations/src/structural_inputs.rs:253`); `element_view_length_offset` 2 non-test sites (`structural_inputs.rs:2303,2545`); `is_native_view_descriptor_at` 1 non-test site (`pipeline/05.../selection_inputs.rs:1445`); `required_scalar_extents` 1 site (`target-bound-program/scalar_demand.rs:211`).
8. Stated limits: none.
9. Fail-open: an `Authored` record returns its declared size without checking that its fields fit inside it (`:771-786`); containment is checked only when `field_placement` is later called (`:257-262`). A record whose declared size is smaller than its fields would get a shape. Open (not traced to callers).
Tests read: width closure, owned sequence bounds (unclosed, wrong owner, duplicate, overflow, zero).

### B20. Not in this unit
`register-environment/src/tests/supported_targets.rs` and `tests/abi_call_clobbers/return_effects.rs` exist (Glob) and are declared by `tests.rs:10` and `abi_call_clobbers.rs:17` but are not rows of `unit_033.tsv`. Not read.

---

## C. targets/intrinsics and targets/layout

### C1. `P/targets/intrinsics/intrinsics.rs` (285 lines)
1. Purpose: names the exact built-in operation selected for a native boundary (`CompilerIntrinsicExecutionIdentity`) so receipts and lowering share one byte identity (`:1-3`).
2. Shape: `CompilerIntrinsicExecutionIdentity` (`:7`, 19 variants), `CompilerAccessorScalarOperation` (`:44`), `From<NumericIntrinsic>` (`:50`), `is_numeric` (`:80`), `checked_register` (`:90`), `compiler_accessor_scalar_contract_identity` (`:160`, SHA-256 over `"omega.sealed-placed-scalar-catalog.v1\0"`), `compiler_intrinsic_execution_identity_bytes` (`:180`).
3. In/out: `checked_register` maps a `RegisterOperation` to a `BuiltinFunction` or `None`.
4. Decides: `checked_register` refuses `SystemWrite` of `Cr2` (`:144`) and of `EsrEl1`/`FarEl1` (`:153`), returning `None`. The identity bytes put a discriminant in byte 0; `EmptyRunRelease` uses 16 (`:204`) and `StaticInterrupt` also uses 16 (`:186`). Two variants share tag byte 16.
5. Cases by shape: fixed lists.
6. Same job elsewhere: none seen.
7. Unused code: `checked_register` 1 call (`provider-admission/.../reachability.rs:263`); `compiler_accessor_scalar_contract_identity` 1 call (`provider-admission/src/compiler_builtins.rs:265`); `compiler_intrinsic_execution_identity_bytes` 4 lines/3 files.
8. Stated limits: none.
9. Fail-open: tag collision between `StaticInterrupt` (byte 0 = 16, byte 1 = operation code) and `EmptyRunRelease` (byte 0 = 16, byte 1 = 0): `StaticInterrupt` with `operation.code() == 0` would encode identically to `EmptyRunRelease` if `code()` can be 0. Not checked (`code()` not read). Open.

### C2. `P/targets/layout/src/lib.rs` (230 lines)
1. Purpose: native layout over source-free records: pointer and descriptor geometry, `LayoutTarget`, callback layout identities, and the `TerminalRecordStorage` resolver (`:1-23`).
2. Shape: `CallbackLayoutCoordinates` (`:44`), `TypeLayout` (`:53`), `fat_descriptor_layout`, `pointer_layout` (`:71,76`), `LayoutTarget` (`:81`) with `for_profile` (`:87`), `TerminalLayoutIdentity` (`:124`), `terminal_layout_identity` (`:133`), `TerminalRecordStorage` (`:196`) implementing `RecordStorageResolver::realization` (`:202`).
3. In/out: reads a `TerminalModule` and `TargetProfile`.
4. Decides: `terminal_layout_identity` returns `Ok(None)` if the record has no realization for the target (`:142-147`); requires nonempty data and policy identity strings (`:148-157`); if `realization.target` is `Some`, requires target name, pointer size/alignment and endianness to equal the profile's (`:166-173`); if it is `None`, no check. `TerminalRecordStorage::realization` finds the application by id and record, the realization by target-name string, then checks pointer size, alignment and endianness (`:207-227`).
5. Cases by shape: n/a.
6. Same job elsewhere: none seen.
7. Unused code: `terminal_layout_identity` 1 call (`pipeline/01.../callback_layouts.rs:117`); `LayoutTarget::for_profile` not grepped.
8. Stated limits: none.
9. Fail-open: `if let Some(facts) = &realization.target && mismatch` (`:166-173`): a realization whose `target` field is `None` is accepted with no geometry check. Open (caller `callback_layouts.rs` not read).

### C3. `P/targets/layout/src/boundary_values.rs` (1147 lines; tests `:616-1147`)
1. Purpose: builds bounded native boundary-value signature graphs (`BoundaryValueLayout`) and Terminal storage projections (`TerminalBoundaryRecipes`) from retained structural types (`:1-8`).
2. Shape: constants `BOUNDARY_VALUE_SHAPE_CAPACITY = 256`, `BOUNDARY_VALUE_FIELD_CAPACITY = 256` (`:31-32`); `BoundaryOpaqueOccurrence` (`:35`); `BoundaryValueLayout` (`:40`) with `matches_value` (`:67`), `value_type` (`:131`), private `record` (`:221`); `append_boundary_value_shape` (`:314`); `abi_boundary_shape` (`:328`); `TerminalBoundaryRecipes` (`:351`) with `scalar` (`:412`), `reference` (`:443`), `value` (`:457`), `value_recipe` (`:479`), `record` (`:549`).
3. In/out: Terminal module in, `BoundaryValueRecipe` handles (`recipes.rs`) then `ValueShape` plus `u16` roots. Shapes are capped at 256 entries (`:318-322`) and sizes at 65535 (`:337-338`, `:272-273`).
4. Decides: keyed on structural declaration shape. `Sum`/`Mixed` return "case data is not yet classifiable as a boundary value; pass it by reference" (`:539-544`); `Opaque` and `TypeParameter` refuse (`:499-509`); a `Record` with zero size refuses (`:267-271`); a record is an HFA only if 1-4 same-size float members at offsets `i*member_size` and total size and alignment fit (`:276-283`). `scalar` refuses callable and non-byte widths (`:424-435`). `record` refuses a field with an authored `element_stride` (`:574-583`). Recursion is cut by `visiting` (`:468-470`).
5. Cases by shape: HFA rule (third copy, see B8).
6. Same job elsewhere: HFA logic at `:276-283` duplicates `calling-conventions` (B8); integer recipe alignment `byte_size.clamp(1,8)` (`:140`) versus `ValueShape::standalone_integer` (B12). For a 3-byte integer the recipe gives alignment 3, not a power of two (inferred, not traced; `call_plan_evaluation.rs:91-95` rejects non-power-of-two alignment).
7. Unused code: `matches_value` has 1 non-test call (`verification/native-bindings/bindings.rs:598`); `append_boundary_value_shape` 6 lines/2 files; `BoundaryValueLayout` 14 lines/4 files; `TerminalBoundaryRecipes` 16 lines/4 files.
8. Stated limits: `:541` "case data is not yet classifiable as a boundary value; pass it by reference"; `:580` "foreign calling recipes do not yet retain authored repeated-field stride".
9. Fail-open: `matches_value` indexes `self.fields[first..first + length]` (`:105`) and slices `actual.fields.get(...)`; `self` is built locally so indices are trusted (`:77-78` comment). Classification: stated. None seen otherwise.

### C4. `P/targets/layout/src/boundary_values/recipes.rs` (50 lines)
1. Purpose: scratch tables used only while native layout prepares a boundary signature (`:1`).
2. Shape: `BoundaryValueRecipes` (`:6`, two arenas), `BoundaryValueRecipe` (`:12`, `Absent` default, `Integer`, `Float`, `Reference{descriptor}`, `FixedArray`, `Record`, `Opaque`), `BoundaryFieldRecipe` (`:42`), `BoundaryRecordStorage` (`:47`).
3. In/out: data.
4. Decides: nothing. `Absent` is the arena default and `value_type` refuses it (`boundary_values.rs:138`).
5-9. None seen.

---

## D. register-environment (`P/targets/register-environment/src/`)

### D1. `RE/catalog.rs` (109 lines)
1. Purpose: selects the facts each target declares: physical register model, constraint catalog, baseline reservation profile and ISA-owned constraint-key roster (`:1-8`).
2. Shape: `target_physical_register_model` (`:27`), `prepared_target_constraint_catalog` (`:34`), `conservative_baseline_reservation_profile` (`:46`), `selected_constraint_keys` (`:65`), a test `float_result_policy_keys_are_declared_for_every_hosted_target` (`:72`), test-only `scalar_call_constraint_key` (`:97`).
3. In/out: switches on `target.architecture` (x86_64 or aarch64) and calls into `target-x86_64`/`target-aarch64`.
4. Decides: the baseline profile includes every overlay in the model except one named `"darwin.aarch64.platform"`, which is kept only when `object_format == MachO` (`:50-57`): keyed on the string name of the overlay. Profile name is `"omega.conservative-baseline-v1"` (`:60`).
5. Cases by shape: two architectures.
6. Same job elsewhere: `lib.rs:120-127` rejects the same overlay name on a non-MachO target (same string literal in two places).
7. Unused code: n/a.
8. Stated limits: none.
9. Fail-open: `scalar_call_constraint_key` returns `None` for other combinations (`:107`), test-only.

### D2. `RE/lib.rs` (396 lines)
1. Purpose: joins a target's register model, constraint catalog and reservation profile into one validated allocator environment, cached per target (`:1-30`).
2. Shape: `baseline_target_register_environment` (`:64`), `build_baseline_target_register_environment` (`:87`), `assemble_target_register_environment` (`:98`), `ValidatedTargetRegisterEnvironment` (`:148`) with `admits_pointer_displacement` (`:180`), `selected_keys`, `fixed_register_view` (`:225`); `selected_abi_preservation` (`:251`); `TargetExitAbi` (`:290`) with `from_environment`, `preservation`, `allows_system_call` (`:298-350`); `selected_preservation_storage_catalog` (`:353`); `TargetRegisterEnvironmentValidationError` (`:371`, 9 variants); a `#[global_allocator]` mimalloc in test builds (`:395`).
3. In/out: `static BUILT: Mutex<Vec<(NativeTarget, Result<..>)>>` (`:67-72`) memoizes results, including `Err`; if the lock is poisoned it rebuilds without caching (`:73,79`). Returns clones.
4. Decides: `assemble` rejects architecture mismatch (`:104`), constraint architecture mismatch (`:112`), constraint physical identity mismatch (`:115`), missing selected keys (`:118`, "UnsupportedSelectedInstructionAbi"), the Darwin overlay on non-MachO (`:120-127`) and reservation validation errors (`:128`). `admits_pointer_displacement`: size must be 1/2/4/8; x86 offset `<= i32::MAX`; aarch64 offset multiple of size and `offset/size <= 4095` (`:180-190`). `selected_abi_preservation` selects by `(architecture, object_format)` (`:255-281`); `(Aarch64, Coff)` and similar fall to `_ =>` Err (`:278`). `TargetExitAbi::from_environment` picks syscall keys by `target == NativeTarget::linux_x64()` or `linux_arm64()` equality (`:311-317`) and finds the preservation convention slot by pointer equality (`std::ptr::eq`, `:308`).
5. Cases by shape: system-call keys exist only for the two Linux targets (`:311-317`); the rest get an empty list (so `allows_system_call` is false).
6. Same job elsewhere: see D1 (overlay name checked twice).
7. Unused code: `baseline_target_register_environment` 86 lines/21 files; `selected_abi_preservation` 19 lines/6 files; `selected_preservation_storage_catalog` 2 non-doc call sites (`machine-code/frame-admission/src/frame_layout.rs:407,467`); `TargetExitAbi` 9 lines/4 files; `admits_pointer_displacement` 10 lines/4 files; `fixed_register_view` 27 lines/8 files (grepped).
8. Stated limits: none.
9. Fail-open: `TargetExitAbi::from_environment` turns a failed `selected_abi_preservation` into `preservation = None` (`:301-310`), and `preservation()` then returns `None` (`:341`). A consumer that treats `None` as "no obligation" would skip the preservation check. Not traced (consumer in `encoding-admission`, not read). Open. Unknown targets get an empty system-call list, which refuses system calls (fail-closed).

### D3. `RE/tests.rs` (300) + `RE/tests/abi_call_clobbers.rs` (519) + `register_calls.rs` (108), `call_effects.rs` (264), `branch_effects.rs` (411), `cleanup_effects.rs` (444), `memory_effects.rs` (760), `provenance_effects.rs` (352), `arithmetic_effects.rs` (1064)
1. Purpose: test suites that check the register environment and the per-ISA machine-effect catalogs for six targets (linux_x64, windows_x64, uefi_x64, linux_arm64, macos_arm64): exact ABI call rows, clobbers, preservation, rules per instruction family, and rejection of each corrupted fact (`tests.rs:1-7`, each file header).
2. Shape: grouped as one kind (tests). Shared fixtures in `abi_call_clobbers.rs`: `ScalarAbiCase` (`:46`), `scalar_abi_cases` (`:97`, five rows: two x86-64 Linux/Windows and `uefi_x64` via `microsoft_x64`, two Arm64), helpers `validated_effects`, `produced_effects`, `validate_effects`, `declaration_mut`, `encoded_mut` (`:198-396`); `EffectRejection` (`:344`). `tests.rs` holds join tests (`:22-300`).
3. In/out: call `baseline_target_register_environment`, `x86_64_machine_effect_catalog`, `aarch64_machine_effect_catalog` and their validators.
4. Decides: assertions on exact register names (`"rcx"`, `"rdi"`, `"x0"` etc.), expected clobber sets, expected byte sizes per instruction (e.g. `arithmetic_effects.rs:261-343`), and error values. Keyed on rule key and `MachineSemanticKind`.
5. Cases by shape: hard-coded roster sizes: 71 arithmetic rules (`arithmetic_effects.rs:543`), 4 branch rules (`branch_effects.rs:95-98`), 16 memory rules (`memory_effects.rs:440-443`).
6. Same job elsewhere: the corruption sweeps in each file use the same `reject_encoded!` macro pattern (copied per file, `arithmetic_effects.rs:730`, `branch_effects.rs:229`, `memory_effects.rs:606`).
7. Unused code: n/a.
8. Stated limits: `tests.rs:165-166` comment: raw tables are test inputs for the validators. A test `environment_identity_binds_each_component_and_explicit_policy` builds a x86-64 environment with `pointer_size: 4` and expects it to validate (`tests.rs:277-288`).
9. Fail-open: none seen in tests. `scalar_abi_cases()[3]` indexes by position (`abi_call_clobbers.rs:481`).
Not read: `return_effects.rs` and `supported_targets.rs` (not in the unit).

---

## Coverage

- Read in full (52 files, 21,993 lines): all files in `units/unit_033.tsv`. Group A 3,620 lines; B 11,934 (calling-conventions) + 285 (intrinsics) + 1,427 (layout); C/D 4,727 (register-environment).
- Skimmed: none. Not read: none within the unit. Files outside the unit that I only grepped: `register-environment/src/tests/supported_targets.rs`, `.../abi_call_clobbers/return_effects.rs`, all callers named under "unused code", `optimization-unit/src/identity.rs`, `register-homes/src/recovery/structural_contracts.rs`, `psi/artifact/declaration-encoding/src/terms/content.rs`.

## Five things a maintainer most needs to know about this unit

1. ABI classification of aggregates is implemented three times: `calling-conventions/src/boundary_values.rs:121-261`, `.../structural_shapes.rs:196-279` (nearly identical bodies, `u16` vs `u32` leaves), and an inline HFA rule in `targets/layout/src/boundary_values.rs:276-283`. The clang cross-check calls the `structural_shapes.rs` copy (`clang_crosscheck.rs:21,208`), not the boundary-graph copy.
2. `validate_call_plan_structure` (`plans/call_plan_evaluation.rs:174`) accepts any structurally sound placement (coverage, disjointness, alignment, architecture); it never compares a supplied plan to what the policy would evaluate. Microsoft x64 refuses HFA/SysV classes ("not normalized yet", `:470`), and callback function pointers require 8/8 (`native_telescope.rs:24`).
3. Identities are string- and hash-keyed: calling application commitment (`boundary_application.rs:223`), callback slot ids derived from identity strings (`callback_catalog.rs:173-174`), and the register-environment overlay name `"darwin.aarch64.platform"` compared as a string in two places (`catalog.rs:54`, `lib.rs:123`).
4. `HostCapability`, `HostOperation` and their `from_name`/`name` in `calling-conventions/src/host_operations.rs` have no users outside the file and the `lib.rs` re-export (grepped); unknown names become `Custom` via a leaked process-wide interner. Only the Darwin/Windows import-library functions and `ExternalBindingRow` are used (2, 1 and 17 files).
5. Several "no entry means accept" paths exist: `NativeEntryFacts::selected_terminal_entry` and `native_execution_scope_for` return `Ok(None)` without deployment/scope (`publication.rs:56-58,113-115`); `terminal_layout_identity` skips geometry when `realization.target` is `None` (`layout/src/lib.rs:166`); `exact_hosted_physical_boundary_entry_plan` returns `None` for UEFI (`process_entry.rs:154-161`); `darwin_import_library` maps unknown symbols to libSystem (`host_operations.rs:394`). Whether callers treat each `None` as acceptance was not traced.

## What I am unsure about

- Whether any caller treats the `None`/`Ok(None)` results listed in item 5 as acceptance: callers are in other units and I did not read them.
- Whether `StaticInterrupt` with operation code 0 collides with `EmptyRunRelease` in `compiler_intrinsic_execution_identity_bytes` (both use byte 0 = 16, `intrinsics.rs:186,204`); `StaticInterruptOperation::code()` was not read.
- The inferred non-power-of-two alignment for 24/40/48/56-bit boundary integers from `byte_size.clamp(1,8)` (`layout/src/boundary_values.rs:140`); not traced through `ClosedScalarType` closure, which may refuse these widths earlier.
- Duplicated encoders (`encode_content_algebra` in four files, `encode_claim_projection` in two) are "suspected" duplicates: I read only this unit's copies, not the others.
- Test counts and which of the listed tests actually run: I did not count `#[test]` per file; only read them.
