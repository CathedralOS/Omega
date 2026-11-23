# Crate map: 08-psi-program

Checkout `4f3c255d05599c11b1ea8ac668635f15f2a6eb86`. All paths are relative to the checkout. Area root: `source-rust/subsystems/compiler/psi/` (written `psi/` below). Counts of files and lines come from `.agents/atlas/codebase-2026-11-22/crates.tsv` (rows with area `08-psi-program`: 21 crates). Test counts come from a script I ran that counts `#[test]` attributes in each crate's own files (files under a nested crate are excluded). "Used by N" counts `Cargo.toml` files (grepped under `source-rust`, `omega-rust-v2`, `tools`) that list the crate as a dependency, dev-dependencies included, with the area's own crates included. The notes name the directory of each manifest, so several different crates can share a name (for example `host`).

## 1. What the area does

The area defines Psi, the portable program that source stage 07 publishes and that the interpreter and the native compiler both read, and everything needed to put that program in bytes and to bind it to a run. It holds four things. (a) The program record itself: `struct Psi` (alias `TerminalModule`) and the tables it points to (`psi/psi.rs:8`). (b) The artifact layer: canonical `PSITERM\0` wire encoding, fingerprints, structural validation on decode, the `PSIART` envelope, publication records and the receiving host's trust decision (`psi/artifact/`). (c) Small leaf crates the program record needs: access vocabulary and admission, layout geometry, application identities, service surface, effect contracts, proof applications, schema reflection. (d) The execution-input checks that bind a published program to a target, providers, entry memory and scalar observations before an interpreter run or native compile (`psi/execution-inputs/`). Input comes from source stage 07 (`terminal_psi::Psi` built in memory, then sealed by `terminal-codec`) or from external bytes (decoded once at ingress). Output is a sealed `ClosedTerminalModule` / `CanonicalTerminalArtifact` plus admitted execution inputs for the interpreter, native stages 00-02 and Build. The sibling crates under `psi/semantics/` (judgments, proofs, work accounting) belong to another area; the area-08 crates call them (`terminal-semantics`, `mathematical-kernel`).

## 2. Crates, in pipeline order

Order: leaf vocabularies, then the program record, then encoding, sealing, trust, then execution inputs.

### 2.1 `extent-contracts` (`psi/access/extents`)
- 1 file, 125 lines. Used by 7: access-vocabulary (`policies`), access-admission (`validated-placements`), storage-bounds, interpreter, host, extents, storage-access.
- Purpose: nonzero identity newtypes (address space, mapping, extent lineage, provider, right, external loan and so on), a normalized `ExtentRights` set, `ExtentContentInterpretation` (fingerprint plus SHA-256 commitment) and `LoanPolarity`. Constructors check shape only; nothing here issues or maps storage (`psi/access/extents/src/lib.rs:1-2`).
- Consumes: raw `u64` identities. Produces: the identity types via macro `normalized_extent_identity!` (`lib.rs:14`).
- Entry points: `from_normalized_identity` per type (generated in the macro, `lib.rs:28`); `ExtentContentInterpretation::from_sha256_commitment` (`lib.rs:75`).
- Production deps: `arena`, `record-storage` (foundation).
- Layout: single `src/lib.rs`.
- Tests: none in the crate (grepped).
- Open work: none found in `TASKS.md` or `OWNER_CODE_CONCERNS.md` (grepped by crate name and path).

### 2.2 `access-vocabulary` (`psi/access/policies`)
- 7 files, 474 lines. Used by 10: validated-placements, declaration-encoding, semantics, 05_target-operations-to-selected-instructions, register-homes, source-to-psi, typed-trees, interpreter, host, storage-access.
- Purpose: descriptive access records: operations (`AccessOperation`, `AtomicAccessOperation`, `BorrowPolarity`), per-field permissions (`FieldAccess`, `AccessPermissions`, `AccessExposure`, `ObservationModel`), service reach (`BoundaryReach`) and offered resources (`ResourceProfile`, `ResourceRegion`, `StableCapability`, `TransferRule`). These records cannot admit a placement (`access.rs:1-2`).
- Consumes: nothing beyond identities. Produces: the vocabulary above, re-exported at `access.rs:7-23`; `AccessPlanDiagnostic` (`diagnostic.rs:2`).
- Entry points: no functions of note; consumers construct the types. `BoundaryReach::from_services` (`boundary_reach.rs:16`).
- Production deps: `record-storage`, `language-core`, `extent-contracts`.
- Layout: `access_operations.rs`, `fields.rs`, `resources.rs`, `boundary_reach.rs`, `normalized_identity.rs` (macro), `diagnostic.rs`.
- Tests: none (grepped).
- Open work: none found.

### 2.3 `layout-plans` (`psi/layouts/geometry`)
- 5 files, 2908 lines. Used by 30 (includes access-admission, declaration-encoding, wire, target-layouts, semantics, native stages 01/05/13/14, isa-aarch64, isa-x86_64, image-emission, register-homes, foreign-calls, calling-conventions, target, interpreter materialization, storage-access).
- Purpose: chosen storage geometry, layout reports, field and symbolic coordinates, placement constraints, stored-integer fit contracts and symbolic write records, with canonical fingerprints and offset packing (`psi/layouts/geometry/README.md`). It does not choose a default layout or write storage.
- Consumes: supplied sizes and alignments. Produces: `StorageGeometry` (`geometry.rs:2`), `ConventionalSumGeometry` (`geometry.rs:8`), `PlacementConstraints` (`layout.rs:130`), `MaterializationContext` (`layout.rs:275`), `LayoutPlanReport`, `SymbolicFieldValue` (`layout.rs:728`), `SymbolicFieldInnerLayout` (`layout.rs:821`).
- Entry points: `validate_private_callback_placements` (`layout.rs:375`), `compose_private_callback_placements` (`layout.rs:470`), `PlacementConstraints::new` (`layout.rs:139`), `validate_site` (`layout.rs:223`); packing in `packing.rs`.
- Production deps: `wire`, `record-storage`. Feature `test-support` exposes helpers (`layout.rs:1090-1306`).
- Layout: `geometry.rs`, `packing.rs`, `layout.rs` (main, ~1300 lines including test support), `layout_reports.rs`, `materialization_records.rs`.
- Tests: 4 `#[test]` in `layout.rs`.
- Open work: `TASKS.md` P2 item **ADDRESS-EXPOSURE** (symbolic storage written after handoff) touches this family (inferred from its text, `TASKS.md:272-274`).

### 2.4 `schema-reflection` (`psi/types/reflection`)
- 5 files, 954 lines. Used by 7: validated-placements, declaration-encoding, providers (dev), semantics, source-to-psi, 05_prepared-typed-trees-to-lowered-program, typed-trees.
- Purpose: a schema description of a declaration (fields, cases, nominal references) as visible to its requester, with a runtime image encoding and a revision hash. Descriptions grant no call or storage permission (`schema.rs:1-3`).
- Produces: `SemanticSchemaGraph` (`schema.rs:20`), `SchemaNode`, `SchemaQueryAuthority`, `DeclarationDescription` (re-exported `schema.rs:15-18`).
- Entry points: `from_runtime_image` (`runtime_image.rs:368`), `runtime_image` (`runtime_image.rs:400`).
- Production deps: `application-identity`, `numerics`, `symbols`, `wire`, `record-storage`.
- Layout: `nodes.rs`, `revision.rs` (hash helpers), `runtime_image.rs`, `tests.rs`.
- Tests: 3 in `tests.rs`.
- Open work: none found.

### 2.5 `application-identity` (`psi/identities/applications`)
- 7 files, 4569 lines. Used by 22 (includes terminal-psi, schema-reflection, callable-encoding, declaration-encoding, wire, source stages 02-07, typed-trees, host, evidence, native stages 01 and 04).
- Purpose: canonical text identities for types, constants, conformances, callback destinations and opaque representations, with codecs and commitments. Source typing writes them; Psi, native code and package review read them (`README.md`).
- Entry points: `compiler_type_identity` (`type_identity.rs:18`), `package_declaration_identity` (`type_identity.rs:1590`), `closed_callable_matches_calling_abi` (`type_identity.rs:1676`), `validate_trait_application` (`type_identity.rs:1968`), `CanonicalConstIdentity::integer` (`const_value.rs:221`), `decode_declaration_value` (`const_value.rs:249`), `ConformanceIdentityDescription` (`conformance_identity.rs:5`).
- Production deps: `bignum`, `numeric-literals`, `numerics`, `record-storage`, `semantic-vocabulary`, `wire`.
- Layout: `type_identity.rs` (2068 lines), `const_value.rs`, `conformance_identity.rs`, `callback_declarations.rs`, `opaque_representations.rs`, `tests.rs`.
- Tests: 37 (`tests.rs` 16, `const_value.rs` 13).
- Open work: the `OWNER_CODE_CONCERNS.md` entry "A resumed Build refuses itself whenever it is typed again, because region owners print as symbol slots" concerns identities produced in `source-to-psi` type_identity and compared in semantics, not this crate (`OWNER_CODE_CONCERNS.md:78-84`).

### 2.6 `psi-program-identity` (`psi/identities/program`)
- 1 file, 56 lines. Used by 2: receipts, conditions (plus terminal-psi via the area).
- Purpose: `VocabularyMarker` (a unit type whose `get()` returns 142, `identity.rs:17-20`) and `SemanticFingerprint` (32 bytes), plus `TerminalPsiIdentity`.
- Entry points: `VocabularyMarker::new` (`identity.rs:9`), `get` (`identity.rs:17`).
- Production deps: `record-storage`. Tests: none.
- Open work: none found.

### 2.7 `service-surface` (`psi/interfaces/service-surface`)
- 3 files, 1020 lines. Used by 19 (build, plans, wire, semantics, native stage 01, program-entry-contracts, component-description, native-bindings, provider-admission, direct-boundaries, target-providers, source-to-psi 05/06/07, typed-trees, host, package-manager, evidence).
- Purpose: service surface records (`ServiceSurface` `service_surface.rs:70`, `ServiceMethod` `:163`, `ServiceReceiverApplication` `:79`), a provider identity table (`ProviderIdentityTable` `:33`) and a contract commitment over them (`commitment.rs`).
- Entry points: `declared_type_signature` (`commitment.rs:24`), `declaration_commitment` (`commitment.rs:47`), `encode_contract` (`commitment.rs:56`), `validate_receiver_applications` (`service_surface.rs:353`), `matches_receiving_application` (`:305`).
- Production deps: `language-core`, `semantic-vocabulary`, `wire`; dev `record-storage`.
- Tests: 4 (`commitment.rs` 2, `segment_tests.rs` 2). Open work: none found.

### 2.8 `psi-effect-contracts` (`psi/machines/effect-contracts`)
- 4 files, 590 lines. Used by 3: callable-encoding, wire, evidence (and terminal-psi).
- Purpose: effects a Psi program retains over parameter positions: capability flow kinds (`CapabilityFlowKind` `effect_contracts.rs:15`), permission carry records (`carry.rs`), and the commitment and dependency check for published template contracts.
- Entry points: `template_contract_commitment`, `validate_published_dependency` (both from `commitment.rs`, re-exported `effect_contracts.rs:8`).
- Production deps: `record-storage`, `wire`. Tests: 2 (`tests.rs`). Open work: none found.

### 2.9 `proof-applications` (`psi/proofs/applications`)
- 1 file, 1246 lines. Used by 6: admission, scalars, wire, scalar-applications (dev), semantics, 05_prepared-typed-trees-to-lowered-program (and terminal-psi).
- Purpose: query proof applications (machines, states, binders, dynamic realizations) from declarations and calls in one scope. Generic over a `Declaration` type (`ProofApplicationMachine<Declaration>` `proof_applications.rs:7`).
- Production deps: `arena`, `record-storage`. Tests: 6 inline. Open work: **PROOF-KERNEL-CORE** (covers PSI-CLAIM-ADMISSION) in `TASKS.md:288` names the claim-admission design; it is not specific to this crate (inferred).

### 2.10 `access-admission` (`psi/access/validated-placements`)
- 7 files, 2797 lines. Used by 8: declaration-encoding, semantics, 05_target-operations-to-selected-instructions, register-homes, source-to-psi, typed-trees, interpreter, storage-access (and terminal-psi).
- Purpose: check an access plan against an explicit layout and resource profile, and issue validated placements, field keys and resource profiles that keep their exact identities (`admission.rs:1-3`, `README.md`).
- Consumes: `AccessPlan` (`plan.rs:11`), `AccessFields` (`access_fields.rs:68`), `ResourceProfile` from access-vocabulary, `LayoutPlanReport` from layout-plans. Produces: `ValidatedPlacement` (`plan.rs:188`), `ValidatedAccessFields` (`access_fields.rs:334`), `ValidatedResourceProfile` (`resources.rs:484`), `AccessResourceCompatibility` (`resources.rs:35`).
- Entry points: `normalize_access_plan` (`plan.rs:117`), `validate_placement` (`plan.rs:221`), `validate_access_fields` (re-export `admission.rs:23`), `validate_resource_profile` (`resources.rs:549`), `validate_access_resources` (`resources.rs:195`), `validate_initialization_resources` (`resources.rs:59`), `validate_run_relocation_resources` (`resources.rs:709`).
- Production deps: `access-vocabulary`, `schema-reflection`, `extent-contracts`, `wire`, `record-storage`, `layout-plans`. Feature `test-support`.
- Layout: `access_fields.rs`, `plan.rs`, `plan_policy.rs` (905), `resources.rs` (800), `test_support.rs`, `tests.rs`.
- Tests: 3 in `tests.rs` (with `layout-plans` `test-support`). Open work: **ADDRESS-EXPOSURE** (`TASKS.md:272-274`) covers device extent access and symbolic materialization (inferred relevance).

### 2.11 `terminal-psi` (`psi/`, library `terminal_psi`, entry `psi.rs`)
- 109 files, 17712 lines (the root crate only; nested crates are separate rows). Used by 101 manifests (nearly every native stage, Build, the interpreter, the CLI, package manager). Production deps (`Cargo.toml`): service-surface, psi-program-identity, proof-applications, access-vocabulary, schema-reflection, application-identity, psi-effect-contracts, access-admission, layout-plans and foundation crates (`source-text`, `bignum`, `wire`, `symbols`, `record-storage`, `arena`, `numerics`, `semantic-vocabulary`, `language-core`). Dev dep: `source-provenance`.
- Purpose: the Psi program record and its schema. `pub struct Psi` (`psi.rs:8`) carries `vocabulary_marker`, `identities`, `entry`, `machines`, `types`, `scalars`, `interfaces`, `access`, `execution`, `proofs`, `layouts` (`psi.rs:10-34`). Almost everything else is declaration types, with little logic.
- Consumes: nothing (it is the representation). Produces: `Psi`, re-exported as `TerminalModule` (`exports.rs:3`). Constructors `Psi::for_entry` (`psi.rs:46`), `Psi::declarations` (`psi.rs:53`).
- Entry points: struct construction by source stage 07 (`source-to-psi/pipeline/07_checked-program-to-psi`, not read); roots in `execution/roots.rs` (`PreparedNominalRoot::new` `:30`, `RetainedTerminalRoot::new` `:109`); `TerminalIdentityTableBuilder` (`identities/table.rs:309`); `find_boundary_machine` (`interfaces/boundary_machines.rs`); closed-conformance and closed-reach commitments (`interfaces/conformances.rs`, `reach_applications.rs`).
- Layout (one line per module):
  - `machines/` (`machine.rs:10` `TerminalMachine`, `block.rs:7` `Block`, `operation.rs:19` `Operation` and `:88` `OperationKind` (643 lines), `terminator.rs:11` `Terminator`, `ranking.rs`, `invocations.rs`, `direct_calls.rs`, `generic_calls.rs`, `atomic.rs`, `trapping_integer.rs`).
  - `identities/` (`table.rs:24` `TerminalIdentityTable`, `declarations.rs:36` `CheckedMachineDeclaration`, private input and result records).
  - `types/` (`TypeDeclarations` `mod.rs:20`; `structural/`, `qualifications/`, `quotients.rs`, `values.rs`, `representation_views/`, `descriptor_locations.rs`, `structural_path_encoding.rs`, `opaque_storage.rs`).
  - `scalars/` (`ScalarDeclarations`, folded constants, scalar applications and conditions).
  - `interfaces/` (`ProgramInterfaces`; boundary machines `boundary_machines.rs` 799 lines, conformances, providers, callback declarations, calling policies, closed reach, service schemas).
  - `access/` (`AccessDeclarations`; borrows, capabilities, claims, cleanup, placement `placement.rs` 325 lines).
  - `execution/` (`ExecutionDeclarations`; callable values and reach, dynamic dispatch, suspension `suspension.rs` 530 lines, task activation, roots, unoptimized behavior).
  - `proofs/` (`ProgramProofs`; `certificates.rs`, `evidence/`, `proof_application_scope.rs`, `theorems.rs`).
  - `layouts/` (`LayoutDeclarations`; native layouts, target layout applications, record storage).
  - `exports.rs` (152 lines, flat re-export list; also re-exports `language_core` and `semantic_vocabulary` names).
- Tests: 36 inline `#[test]` plus `tests/ir_schema.rs` (63 lines), which scans the crate's sources and fails on string-typed identity fields (`ir_schema.rs:44-56`).
- Open work: **STRUCTURAL-BORROW-IDENTITY** and **NORMALIZED-ABI-LOWERING** (`TASKS.md:282-286`) change Psi vocabulary (inferred); `TASKS.md:48` asks for marker 291 (see section 4).

### 2.12 `psi-declaration-encoding` (`psi/artifact/declaration-encoding`)
- 29 files, 5399 lines. Used by 6: callable-encoding, wire, optimization-unit, optimization-unit-semantics, structural-storage, target-vocabulary.
- Purpose: canonical byte encoders for Psi declarations (structural domains, access views and lifecycles, quotients, scalar and proposition terms, dispatch, crash routes, places, invocations). The complete artifact and native program identities share these same field definitions (`artifact/README.md`).
- Consumes: `terminal_psi` declaration types. Produces: bytes via `Writer` (`wire.rs:15`) and `CodecError` (`error.rs`).
- Entry points: `encode_access_lifecycle_catalog` (`access/lifecycles.rs:10`), `encode_structural_domain_declaration` (`domains.rs:245`), `encode_quotient_catalog` (`quotients.rs:62`); decode functions beside each encoder.
- Production deps: application-identity, access-vocabulary, schema-reflection, arena, semantic-vocabulary, numerics, terminal-psi, access-admission, layout-plans.
- Layout: `terms/` (`scalar.rs` 613, `proposition.rs` 494, `content.rs`, `float_meaning.rs`, `integer.rs`, `order.rs`, `table.rs`), `access/` (`views.rs`, `lifecycles.rs`), `domains.rs`, `fields.rs`, `invocations.rs`, `places.rs`, `dispatch.rs`, `wire.rs`.
- Tests: 18 inline plus `tests/identity.rs` (46 lines, 3 tests). Open work: none found.

### 2.13 `psi-callable-encoding` (`psi/artifact/callable-encoding`)
- 10 files, 2071 lines. Used by 2: wire, target-vocabulary.
- Purpose: encodings for callable descriptors: signatures, machine contracts, proof records, certificates (postorder proof nodes), closed service reach, results and evidence lanes. Shared by native program identities and the complete artifact (`lib.rs:1`).
- Entry points: `encode_callable_catalog` (`catalogs.rs:93`), `decode_callable_catalog` (`catalogs.rs:99`), `canonical_certificate_bytes` (`certificates.rs:14`), `reach::encode` (`reach.rs:15`) and `reach::decode` (`reach.rs:146`), `machine_declarations::encode`/`decode` (`:9`, `:40`).
- Production deps: application-identity, psi-declaration-encoding, psi-effect-contracts, semantic-vocabulary, terminal-psi. Re-exports `CodecError`, `fields`, `wire` from declaration-encoding (`lib.rs:15`).
- Layout: `catalogs.rs`, `certificates.rs` (677), `contracts.rs`, `reach.rs` (567), `results.rs`, `signatures.rs`, `machine_declarations.rs`, `proof_declarations.rs`, `evidence.rs`.
- Tests: 1 (`catalogs.rs`). Open work: none found.

### 2.14 `optimization` (`psi/artifact/optimization-selections`)
- 1 file, 270 lines. Used by 9: codec, wire, native stages 01 and 03, source-to-psi, stages 06 and 07, checked-program, optimization-selections.
- Purpose: nine requested optimization names with fixed tags (`PsiOptimization` via macro `psi_optimization_vocabulary!`, `lib.rs:24-60`), a canonical sorted set `PsiOptimizationSelections` and its encoding (`PSIOPT\0\0`, version 1) and identity.
- Entry points: `PsiOptimizationSelections::new` (`lib.rs:57`), `encode` (`:82`), `decode` (`:93`), `identity` (`:138`).
- Production deps: `wire`. Tests: 3 inline. Open work: none found.

### 2.15 `psi-publication-records` (`psi/artifact/publication`)
- 4 files, 326 lines. Used by 9: codec, image-emission, verified-optimization-input, program-entry-contracts, target-bound-program, stages 06 and 07, checked-program, host.
- Purpose: source declaration custody and entry receipts that accompany a published program: `PublishedEntryMetadata` (`published_entry.rs:11`), `TerminalMachineProvenance` (`machine_provenance.rs:6`), `CheckedProgramEntryTerminalReceipt` (`checked_program_entry.rs:6`).
- Entry points: `PublishedEntryMetadata::new` (`published_entry.rs:19`), `validate_declarations` (`:37`), `TerminalMachineProvenance::for_root` (`machine_provenance.rs:12`).
- Production deps: terminal-psi, semantic-vocabulary, record-storage, source-text. Tests: 1. Open work: none found.

### 2.16 `terminal-wire` (`psi/artifact/wire`)
- 17 files, 35350 lines (about 13800 are `foundation_validation.rs`, about 5400 `semantic_module.rs`, about 4200 `module_wire.rs`; many of these lines are inline tests). Used by 4: codec, register-homes, selected-instructions, legalized-operations.
- Purpose: the canonical `PSITERM\0` codec for `Psi`: encode, decode, re-encode comparison, structural-foundation validation during decode, fingerprints and companion sections (artifact manifest, debug map, optimization execution record, evidence seals). It cannot name the sealed module type (`lib.rs:1-12`).
- Consumes: `TerminalModule` or bytes. Produces: bytes, `TerminalModule`, `SemanticFingerprint`, `TerminalPsiIdentity`.
- Entry points: `encode_module` (`semantic_module.rs:4209`), `decode_module` (`:4260`), `decode_closed_parts` (`:4264`), `semantic_fingerprint` (`:4316`), `terminal_psi_identity` (`:4339`), `encode_raw` (`module_wire.rs:4184`), `build_artifact_manifest` (`sections.rs:121`), `validate_artifact_manifest` (`:156`), `encode_debug_map` (`:287`), `decode_debug_map` (`:295`), `verify_identity_formations` (`identity_formations.rs:130`). Format marker `FORMAT_MARKER: u16 = 290` (`semantic_module.rs:4205`).
- Production deps: callable-encoding, declaration-encoding, service-surface, effect-contracts, proof-applications, application-identity, numeric-literals, bignum, wire, language-core, arena, semantic-vocabulary, optimization, numerics, mathematical-kernel, terminal-psi, terminal-semantics, layout-plans. Feature `test-support` (lets codec tests encode malformed modules).
- Layout: `semantic_module.rs` (entry points and canonical order), `module_wire.rs` (ordered sections), `block_wire.rs` (operations and terminators), `structural_wire.rs`, `term_wire.rs`, `service_schema_wire.rs`, `wire_fields.rs`, `foundation_validation.rs` (decode-time closure checks), `obligation_evidence.rs` (evidence seals), `mathematical_proof_actual.rs`, `sections.rs` (manifest, debug map, optimization record), `identity_formations.rs`, `closed_types.rs` (`ClosedTypeCatalog` `:14`), `contract_forms.rs`, `debug_map_records.rs`, `spec_tag_tables.rs` (tests that read tag tables out of `wiki/spec/terminal-psi/encoding.md`).
- Tests: 178 inline (`semantic_module.rs` 62, `foundation_validation.rs` 32, `obligation_evidence.rs` 22, `module_wire.rs` 20, `structural_wire.rs` 16). `autotests = false` and no `tests/` directory.
- Open work: **OBSERVATION-PROFILE** (`TASKS.md:278-280`) asks for an observation profile section in this crate; **PROOF-KERNEL-CORE** (`TASKS.md:288`) covers claim admission, which the README says splits decode-time analysis from preparation (`wire/README.md`).

### 2.17 `terminal-codec` (`psi/artifact/codec`)
- 6 files, 1542 lines. Used by 45 manifests (build, evaluation, compiler, authorization, execution-inputs, native stages 00-04, native-artifact, interpreter, host, retained-image, package-manager, source stages 05-07 and others).
- Purpose: seal a `Psi` program with its canonical bytes and identity, package semantic, optimization and optional debug sections into the `PSIART\0\0` envelope, and rejoin entry receipts to artifacts. Re-exports all of `terminal-wire` (`lib.rs:30`). It confirms valid structure, not that the checker accepted the program or that its producer is trusted (`README.md`).
- Consumes: `TerminalModule` (from stage 07) or external bytes. Produces: `ClosedTerminalModule` (`closed_module.rs:14`), `CanonicalTerminalArtifact` (`canonical_artifact.rs:51`), `TerminalExecutionView` and `RetainedTerminalCode` (`canonical_artifact.rs:447`, `:528`), `InvocationTerminalCode` (`:543`), `TerminalProgramProduct` (`:643`).
- Entry points: `ClosedTerminalModule::new` (`closed_module.rs:22`), `from_semantic_bytes` (`:27`); `CanonicalTerminalArtifact::from_parts` (`canonical_artifact.rs:84`), `from_closed_parts` (`:94`), `with_publication` (`:132`), `to_bytes` (`:195`), `from_bytes` (`:241`), `view` (`:323`), `retain_code` (`:333`); `prepare_program_entry_receipt` (`entry_receipt.rs:41`), `rejoin_program_entry_receipt` (`:65`).
- Production deps: publication, diagnostics, record-storage, wire, optimization, semantic-vocabulary, terminal-psi, terminal-wire (dev: terminal-wire `test-support`).
- Layout: `closed_module.rs`, `canonical_artifact.rs` (921), `entry_receipt.rs` and `entry_receipt/application.rs`, `qualified_cleanup_tests.rs`.
- Tests: 11 inline. `mbx test -p terminal-codec --lib` is the documented run (`wire/README.md`). Open work: none found by crate name.

### 2.18 `psi-authorization` (`psi/artifact/authorization`)
- 3 files, 1833 lines. Used by 15 (application, build, evaluation, compiler, psi-to-native, native stages 00-03, host-providers, verified-optimization-input, target-bound-program, source-to-psi, stage 07, retained-image).
- Purpose: keep the receiving host's producer-trust decision beside the exact artifact it authorizes. `HostProducerTrust::accept` (`authorization.rs:23`) records the host's decision; `AuthorizedTerminalArtifact` cannot be decoded from bytes (`authorization.rs:48`). A second module admits source selection questions: a program may use only declarations its package can select (`selection_questions.rs:1-3`).
- Entry points: `authorize_terminal_artifact` (`authorization.rs:133`), `AuthorizedTerminalArtifact::require_artifact` (`:77`), `validate_selection_questions` (`selection_questions.rs:393`), `admit_selection_questions` (`:836`), `admit_selection_questions_with_consent` (`:846`), `ProductSchemaCustody::from_selections` (`:44`).
- Production deps: declaration-scope, semantic-vocabulary, terminal-psi, terminal-codec; dev terminal-semantics.
- Tests: 9 (`selection_questions/tests.rs` 667 lines, 6 tests; 3 in `authorization.rs`). Open work: none found.

### 2.19 `psi-target-layouts` (`psi/execution-inputs/target-layouts`)
- 2 files, 414 lines. Used by 14 (evaluation, scalars, entry-memory, scalar-applications, native stages 01 and 02, execution-scope, verified-optimization-input, target-bound-program, provider-admission, source-to-psi, interpreter, host).
- Purpose: bind the receiving target to an exact program and check its selected layouts (`admission.rs:1`). `TerminalTargetContext` ties target-dependent observations to one artifact and does not choose a provider (`target_context.rs:3-5`).
- Produces: `AdmittedTargetLayouts` (`admission.rs:31`), `TargetLayoutClosure` (`:13`), `TerminalTargetContext` (`target_context.rs:6`).
- Entry points: `admit_target_layouts` (`admission.rs:155`), `terminal_target_context` (`target_context.rs:73`), `TargetLayoutGeometry::unscoped` (`admission.rs:81`).
- Production deps: layout-plans, semantic-vocabulary, terminal-codec, terminal-psi. Tests: 1. Open work: none found.

### 2.20 `psi-provider-selections` (`psi/execution-inputs/providers`)
- 4 files, 1541 lines. Used by 20 (build, admission, evaluation, scalars, scalar-applications, psi-to-native, provider-demand, native stage 01, native-artifact, verified-optimization-input, target-bound-program, provider-admission, runtime-publication, target-providers, source-to-psi, stage 07, published-target-providers, interpreter, host).
- Purpose: verify requested providers and assumed bindings against one artifact's callable catalog. The admitted selection keeps the execution identity, receiving applications and supply paths; component selections also check the receiving catalog against the supplying artifact (`selections.rs:1-5`).
- Entry points: `admit_provider_installation` (`selections.rs:262`), `admit_provider_installation_from_artifact` (`:238`), `admit_provider_execution_installation` (`:249`), `admit_provider_installation_with_assumed_bindings_from_artifact` (`:275`), `AdmittedProviderSelections::admit_folded_constant_closure` (`:74`), `admit_component_provider` (`component.rs:68`), `close_providers` (`logical_reach.rs:92`), `boundary_call_reach` (`logical_reach.rs:7`).
- Production deps: diagnostics, record-storage, semantic-vocabulary, terminal-codec, terminal-psi, terminal-semantics; dev schema-reflection.
- Layout: `selections.rs`, `component.rs` with `component/memberships.rs` (618), `logical_reach.rs`.
- Tests: 4. Open work: **ASSUMED-BINDING-CONTRACTS** is part of **ADDRESS-EXPOSURE** (`TASKS.md:274`).

### 2.21 `psi-entry-memory` (`psi/execution-inputs/entry-memory`)
- 1 file, 331 lines. Used by 13 (application, compiler, psi-to-native, native stage 02, image-emission, native-artifact, verified-optimization-input, program-entry-contracts, target-bound-program, provider-admission, runtime-publication, interpreter, host).
- Purpose: check the receiver's supplied entry memory against the exact borrowed row and admitted target layout before the interpreter or native code uses it (`entry_memory.rs:1-2`).
- Produces: `CheckedPrivateEntryRamRecipe` (`:23`), `PrivateEntryRamInitialization` (`:11`).
- Entry points: `admit_private_entry_ram` (`:105`), `private_entry_ram_identity_pair` (`:87`), `decode_private_entry_observations` (`:301`).
- Production deps: semantic-vocabulary, psi-target-layouts, terminal-codec, terminal-psi, terminal-semantics. Tests: none. Open work: **ENTRY-CONTENT-ROOTS** (`TASKS.md:268`) covers provisioned entry receivers (inferred relevance).

### 2.22 `psi-scalar-applications` (`psi/execution-inputs/scalar-applications`)
- 7 files, 3049 lines. Used by 11 (invocations, scalars, psi-to-native, scalar-applications, native stages 01 and 02, native-artifact, verified-optimization-input, runtime-publication, source-to-psi, interpreter).
- Purpose: check scalar observations and the prerequisite, entry, ABI and product scopes of published scalar applications against the same provider and layout selections (`scalar_applications.rs:1`). The Build scalar evaluator records results; these checks bind each result to its application (`execution-inputs/README.md`).
- Entry points: `ScalarApplicationClosure::new` (`completion.rs:188`), `close_scope` (`completion.rs:319`), `ScalarConditionSelection` (`conditions.rs:28`), `ScalarEntryPrerequisites::prepare` (`entry_checks.rs:49`), `admit` (`entry_checks.rs:214`), `ScalarApplicationObservations::new` (`observations.rs:43`), `AdmittedScalarApplications` (`completion.rs:72`), `CallingAbiStorageDemand` (`abi_storage.rs`).
- Production deps: arena, bignum, numerics, psi-provider-selections, psi-target-layouts, semantic-vocabulary, terminal-codec, terminal-psi, terminal-semantics; dev proof-applications.
- Tests: 11 (`completion_tests.rs` 6, `conditions.rs` 5). Open work: none found.

## 3. How data flows through the area

1. Source stage 07 (`source-to-psi/pipeline/07_checked-program-to-psi`, outside this area) builds a `terminal_psi::Psi` in memory (`psi/psi.rs:8`), using `PublishedEntryMetadata` and `TerminalMachineProvenance` from `psi-publication-records` for source custody.
2. `terminal_codec::ClosedTerminalModule::new` (`closed_module.rs:22`) calls `terminal_wire::encode_module` (`semantic_module.rs:4209`), which uses the declaration and callable encoders (`psi-declaration-encoding`, `psi-callable-encoding`), computes the fingerprint (`semantic_fingerprint` `:4316`) and seals bytes, closed type catalog and identity together.
3. `CanonicalTerminalArtifact::from_closed_parts` (`canonical_artifact.rs:94`) and `with_publication` (`:132`) add optimization bytes (`optimization`), debug bytes and the manifest (`build_artifact_manifest`, `sections.rs:121`). `to_bytes` (`:195`) writes the `PSIART\0\0` envelope.
4. External bytes come in through `CanonicalTerminalArtifact::from_bytes` (`:241`) or `ClosedTerminalModule::from_semantic_bytes` (`closed_module.rs:27`), which call `decode_module` (`semantic_module.rs:4260`): marker check, re-encode comparison and `validate_structural_foundation`.
5. The receiving host calls `authorize_terminal_artifact` (`psi-authorization`, `authorization.rs:133`) with a `HostProducerTrust`; the result is an `AuthorizedTerminalArtifact`. Source selection questions go through `admit_selection_questions` (`selection_questions.rs:836`).
6. A run or compile request supplies inputs, and each is checked against the artifact: `terminal_target_context` and `admit_target_layouts` (target and layouts), `admit_provider_installation` and `close_providers` (providers), `admit_private_entry_ram` (entry memory), `ScalarApplicationClosure` and `ScalarEntryPrerequisites::admit` (scalar observations).
7. The interpreter, native stages 00-02 and Build read the `TerminalExecutionView` (`canonical_artifact.rs:447`) and the admitted inputs; `terminal-psi` types are the common vocabulary throughout.

## 4. Where the map differs from the documents

- `TASKS.md:48` and `wiki/spec/terminal-psi/encoding.md:45` say the current semantic marker is 291 (the spec: "Current marker 291 removes the four policy bytes from suspension plans and live values"). The code writes and accepts `FORMAT_MARKER = 290` (`artifact/wire/src/semantic_module.rs:4205`; crate doc at `semantic_module.rs:3`). Either the spec ran ahead of the code or the code did not move; I did not check which commit introduced 291 in the spec.
- `psi/artifact/wire/README.md` says the `observation.rs` profile types were deleted and no profile code exists; I found no observation profile code in `artifact/wire/src` (grepped file list), consistent with `TASKS.md:278-280`. No difference.
- `psi/README.md` says `entry` is the "Optional callable selected as the program entry" and links `semantic-records/src/lib.rs#L30`; line 30 is `pub enum TargetMachineCallable` (checked). No difference.
- `AGENTS.md` says the wire format is `PSITERM\0`; the code also has an outer `PSIART\0\0` envelope (`codec/src/canonical_artifact.rs:18`) and a `PSIOPT\0\0` optimization section (`optimization-selections/src/lib.rs`). `AGENTS.md` names only the first. This is a gap of naming, not of behavior.
- `psi/tests/ir_schema.rs` forbids `String` identity fields in the terminal-psi crate (`ir_schema.rs:44-56`). The `application-identity` crate (area 08, a dependency of terminal-psi) is built around text identities (`type_identity.rs:18`, `const_value.rs:171`). The two are consistent only because the test scans just the root crate's directories and skips directories containing a `Cargo.toml` (`ir_schema.rs:20-24`).
- `AGENTS.md` and `OWNER_CODE_CONCERNS.md` do not name any area-08 crate as having an open code concern (grepped by crate and path names).

## 5. What I did not read, and what I am unsure about

Not read (only headed, grepped or skimmed): the bodies of `terminal-wire` (`foundation_validation.rs`, `semantic_module.rs`, `module_wire.rs`, `structural_wire.rs`, `term_wire.rs`, `block_wire.rs`), most of `psi-declaration-encoding/src/terms/`, `psi-callable-encoding/src/certificates.rs` and `reach.rs`, `application-identity/src/type_identity.rs` (2068 lines), `layout-plans/src/layout.rs`, `psi-authorization/selection_questions.rs`, and the interior of the `terminal-psi` tables (`interfaces/boundary_machines.rs`, `execution/suspension.rs`, `proofs/evidence/*`, `types/structural/*`). I read module headers, exports and signatures only. I did not read the spec pages `wiki/spec/terminal-psi/encoding.md` beyond the marker paragraphs, `product.md`, `wiki/drafts/designs/psi_claim_admission.md` or `check_on_lowered_form.md`. I did not read source stage 07 or any consumer.

Unsure: (a) the "used by" counts include dev-dependencies and the area's own crates, and the manifest names are directory names, not always unique; (b) several "open work" links are my inference from item text, marked "inferred" above; (c) the line counts in `crates.tsv` for `terminal-wire` include inline test modules, so 35350 overstates production code; I did not measure the split; (d) whether `psi/semantics/*` crates (listed in `psi/Cargo.toml` consumers) belong to area 07 rather than this area: they are absent from this area's `crates.tsv` rows, so I treated them as outside.
