# Unit 80 notes

Files covered (all under `omega-rust/psi/compilation/representations/terminal-psi/codec/src/`, checkout `worker-23`, main `457adf4a11f`):

1. canonical_artifact.rs (444 lines)
2. foundation_validation.rs (11882 lines)
3. identity_formations.rs (206 lines)
4. lib.rs (182 lines)
5. mathematical_proof_actual.rs (942 lines)
6. module_wire.rs (5138 lines)
7. sections.rs (774 lines)

Total 19568 lines, all read in full. Citations below are `file:line` relative to the folder above.
Method words: "read" = I read the lines; "grepped" = I ran a Grep (count mode where I give a count); "inferred" = reasoning from what I read.

---

## lib.rs (1-182)

**Purpose.** Crate root for the Terminal Psi wire codec (`PSITERM\0`). Declares the modules, defines `CodecError`, and re-exports the public surface (module encode/decode, fingerprints, artifact, manifest, debug map, optimization record, mathematical verifiers, catalog codecs). (read)

**Shape.**
- `mod` list at lib.rs:28-44 (`block_wire`, `wire_fields`, `semantic_module`, `structural_wire`, `term_wire` are declared here but live in other units).
- `CodecError` enum lib.rs:57-107: 28 variants, including string-carrying structured ones `MalformedScalarReturnFoundation` (78), `MalformedOperationFoundation` (86), `MalformedStructuralOperation` (93). `Display` is `write!("{self:?}")` (109-113). `malformed(&'static str)` helper (117-119) builds `MalformedStructuralFoundation`.
- Re-exports at 31-51, 121-182.

**Inputs/outputs.** No logic; re-exports only. `cursor`/`digest_encoder`/`codec_bytes` are re-exported from an `optimization` crate (lib.rs:46, 128, 130) (read).

**How it decides.** Nothing.

**Same job elsewhere.** `Display` via `{self:?}` repeated for `CanonicalTerminalArtifactError` (canonical_artifact.rs:268-272), `ArtifactManifestError` (sections.rs:203-207), `DebugMapError` (sections.rs:527-531); the optimization-record errors prefix the debug text (sections.rs:742-767).

**Unused / one-caller.** `terminal_codec::` appears 434 times across 151 files in `omega-rust` (grep count of `terminal_codec::`, includes tests/examples). `verify_mathematical_proof_actual` (re-exported lib.rs:123-124): grep of the name across `omega-rust/*.rs` finds only its definition (mathematical_proof_actual.rs:593) and the re-export; zero call sites. `verify_identity_formations`, `verify_module_mathematical_actuals`, `verify_indexed_theorem_roots`: only callers are test support files under `omega/compilation/compiler/tests/support/` (identity_formations.rs:102,149; mathematical_actuals.rs:177; indexed_family_controls.rs:345,360,462,543,776). No production caller in `omega-rust` (grepped).

**Stated limits.** None in comments. Header comment lib.rs:11-19 states the design claim "two artifacts have the same fingerprint if and only if their canonical wire bytes are identical" (backed by `decode_module` re-encode compare, semantic_module.rs:4115-4118, outside unit, read).

**Fail-open.** None seen in this file.

---

## canonical_artifact.rs (1-444)

**Purpose.** Container for a whole Terminal Psi artifact: semantic module bytes + optimization record bytes + optional debug-map bytes, wrapped in a `PSIART\0\0` envelope with a manifest of identities. Also defines the "execution code" wrappers (`TerminalProgramProduct` etc.) that distinguish a portable artifact from private invocation code. (read)

**Shape.**
- Constants `ARTIFACT_MAGIC` (17), `ARTIFACT_FORMAT_MARKER = 3` (18).
- `CanonicalTerminalArtifact` (38-45): `semantic_bytes`, `optimization_bytes`, `optimization`, `debug_bytes: Option`, `manifest`, `semantic_module: Arc<TerminalModule>`. `PartialEq` compares manifest + the three byte vectors only (47-54).
- `from_parts` (71-107): encodes module, optimization record, debug map; computes identity via `encoded_fingerprint`; builds manifest via `manifest_for_semantic(semantic, &optimization, None, debug_bytes)` (92-98).
- `to_bytes` (109-137): magic, marker, three u64 lengths (debug length only if present, tag byte 0/1), payloads.
- `from_bytes` (139-184): checks magic/marker/tag/lengths/trailing bytes, then `decode_module`, `decode_psi_optimization_execution_record`, `decode_debug_map`, rebuilds via `from_parts` and requires the three re-encoded sections to equal the input slices (176-182).
- Error types 240-266. `execution_code` module (286-444): `TerminalPublication{Portable,Invocation}`, `TerminalExecutionView`, `TerminalExecutionIdentity{semantic, invocation: Option<InvocationCodeScope>}`, `RetainedTerminalCode`, `InvocationTerminalCode` (requires `module.identities.invocation_scope()` present, 374-386), `TerminalProgramProduct{Portable,Invocation}` with `portable()`/`into_portable()` refusing `Invocation` (420-436, 439-443).

**Inputs/outputs.** Reads `encode_module`/`decode_module`, debug and optimization codecs, `manifest_for_semantic`. Produces bytes and a manifest.

**How it decides.** Canonical-ness is by re-encode-and-compare of bytes (177-182). Portable vs invocation is by presence of `identities.invocation_scope()` (374-379, 309) (identity/presence, not spelling).

**Cases by shape.** Debug section is `None`/`Some` only (tag 0/1, 152-156). Installation section: `from_parts` always passes `None` for the installation record (canonical_artifact.rs:95), so `manifest.installation()` is always `None` for any artifact built or decoded here; `to_bytes` has no installation section at all.

**Same job elsewhere.** Section-length encode/decode `encode_section_len`/`section_len` (232-238, 278-284) vs `Writer::len`/`bounded_count` used in sections.rs (e.g. 266, 363) — different framing helpers for the same "length prefix" job (inferred, not compared).

**Unused / one-caller.** `CanonicalTerminalArtifact::retain_code` used by `TerminalProgramProduct::retain_code` (406-411) and externally (not counted). `InvocationTerminalCode::new` / `TerminalExecutionIdentity::portable` / `custody_identity` — grep of the three names across `omega-rust/*.rs`: 43 matches in 17 files (count mode; includes tests); not separately broken down.

**Stated limits.** `#[must_use = "a canonical Terminal artifact owns the Psi-to-Omega handoff bytes"]` (37). Error text "invocation code is not a portable Terminal artifact" (441).

**Fail-open.** None seen. `from_bytes` fails closed on every mismatch (141-182). The installation hash in the manifest can never be exercised from this constructor (see above), which is a gap, not an accept.

---

## identity_formations.rs (1-206)

**Purpose.** Receiver-side check of the "Id formation" catalog (`module.identity_formations`): each row says a source subject becomes a kernel `Id` type; the receiver re-checks the proof-kernel certificate and, in the pub verifier, compares it with roots the receiver computed from its own checked source. (read)

**Shape.**
- `validate_identity_formations` (14-19, `pub(crate)`): called from `decode_module` at semantic_module.rs:4110 with `Budget::default()` (grepped/read).
- `receive_catalog` (21-92): empty rows + empty certificate -> `Ok(None)`; empty rows + certificate -> error (27-33). Otherwise decodes the certificate and requires level arity 0, empty context, `term == TwoZero`, `expected == Two` (35-43); runs `verify_mathematical_certificate` (44); checks rows strictly ordered by `declaration` (49-52), origins/carriers lengths and `start <= end` (53-57), non-empty carrier identity texts (58-65), declaration has a body (74-76), peels one lambda per subject carrier (77-82), `weak_head_normalize`, body must be `Term::Id` (83-89).
- `ExpectedIdentityFormation` / `ExpectedIdentityFormations` (96-110, pub).
- `verify_identity_formations` (115-206, pub): row count equal; `receive_catalog`; shared signature prefix re-encoded must equal `expected.signature` (128-146); per row compares `source_owner`, `source_carrier`, `subject_origins`, `subject_carriers` as identity **text** against the trusted strings (148-164), checks domain/binder correspondence by `structurally_equal` (172-188), re-encodes the judgment and compares bytes to `trusted.judgment` (189-203).

**Inputs/outputs.** Reads `TerminalModule.identity_formations`, `identities` text table, `proof_admission` kernel. Output is `Result<(),CodecError::InvalidIdentityFormation(String)>`.

**How it decides.** Keyed on identity table **text** (`module.identities.text(...) == Some(trusted.x.as_str())`, 149-159) and on byte equality of re-encoded certificates (142, 199). Ordering keyed on `declaration` index (49).

**Cases by shape.** Only the closed judgment `TwoZero : Two` signature form (35-43).

**Same job elsewhere.** The "decode certificate, require empty context and level arity 0, compare carrier key bytes" pattern also appears in mathematical_proof_actual.rs:31, 624, 904-917 and module_wire.rs:5055-5067.

**Unused / one-caller.** `validate_identity_formations`: 1 caller (semantic_module.rs:4110, grepped). `verify_identity_formations`: 2 call sites, both in test support (see lib.rs section).

**Stated limits.** Doc comments 1-6 and 94-96: "They describe formation, never Id truth." Comment 71-73 says the shared signature check quantified the declaration's universe telescope and source-bound verification compares it later (the latter is only in the pub verifier, which has no production caller).

**Fail-open.** `verify_identity_formations` returns `Ok(())` after the row-count check when `receive_catalog` gives `None` (124-126): both rosters empty, so nothing to compare - traced, correct. `decode_module` runs only `validate_identity_formations` (no expected roots), so a decoded module is accepted if its own certificate is internally valid; truth against source is only checked by the pub verifier ("traced": no production caller of the verifier in `omega-rust`).

---

## mathematical_proof_actual.rs (1-942)

**Purpose.** Receiver-side checks that mathematical (kernel-level) proof actuals carried in a module's call/jump slots match a trusted expected module: carrier keys, source type identities, kernel type-checking of the certificates, and that assumptions used are granted. Also verifies "indexed theorem roots". (read)

**Shape.**
- `MathematicalProofActualError` (568-578).
- `mathematical_carrier_key` (580-591): encodes the certificate with `term := expected`.
- `verify_mathematical_proof_actual` (593-605) -> `prepare_mathematical_proof_actual` (607-634) + kernel verify + `check_grants` (636-651).
- `verify_module_mathematical_actuals` (653-827): collects slots from both modules, binds rosters, pairs by `slot.key`, checks per-actual term kind (`Mathematical`, `MathematicalApplication`, `Formal`/`InvocationFormal`), batch-verifies all certificates, then grants.
- `mod applications` (6-158): `prepare` walks `MathematicalArgument`s against `Pi` domains, `infer_sort`, key compares.
- `mod slots` (159-555): `Slot` (168-176) with `key: (u64,u64,u8,u64,u64)`; `contains_mathematical` (178-212); `same_formals` (221-246); `bind_slot` (260-291); `collect` (293-496); `bind_rosters` (498-554).
- `mod indexed_theorem_roots` (829-942): `verify_indexed_theorem_roots` (843-941).

**Inputs/outputs.** Reads two `TerminalModule`s (`mathematical_carriers`, `machines`, blocks, terminators, `indexed_theorems`, `identities`), `proof_admission` kernel, and a caller-supplied `grants` closure. Output `Result<(), MathematicalProofActualError>`.

**How it decides.**
- Slot identity = tuple `(machine id, block id, kind code, ordinal, operation/edge id)` (`slots.rs`-style at 379-385, 418-424, 479): kind codes 1 Call, 2 CallUnit, 3 CallStructuralScalar, 4 CallStructuralWithScalarArguments (321-341), 5 Jump, 6/7 Conditional true/false (435-458), 8 module invocation (417-424). Keyed on numeric ids.
- Carrier correspondence is keyed on identity **text**: `module.identities.text(type_identity)` compared across modules (50, 93-95, 101-103, 149-155 equivalents at 701-704, 726-731, 795-798) and on `carrier_key` byte equality (63, 105, 132, 153, 706, 734).
- Formal terms compare `source_position` and non-empty text (`same_formals` 227-245).

**Cases by shape.**
- Slots exist only for four call kinds plus Jump/Conditional edges plus module invocation (321-341, 396-433, 434-460); `CallDynamic` with math content is rejected outright (342-355).
- `MathematicalArgument::Scalar` requires `ScalarTerm::Value` (72-74).
- `applications::prepare` requires at least one argument (26-28).

**Same job elsewhere.**
- "find carrier row by type identity, compare `carrier_key`" appears four times: 53-65, 89-107, 697-708, 729-736.
- "find unique machine/block by id with duplicate detection" (`candidates.next()` / `candidates.next().is_some()`): 359-366, 397-404, 462-466, 513-519, 536-542.
- `mathematical_carrier_key`-style certificate decode/compare also in indexed roots (910-919).
- `value lookup by ValueId` pattern lives in foundation_validation.rs (see below), not here.

**Unused / one-caller.** `verify_mathematical_proof_actual`: 0 call sites (grepped, see lib.rs section). `slots::contains_mathematical`: used in `collect` (346, 370, 409, 470) and in the verifier (805). `Slot.callee` etc. used by `bind_slot`.

**Stated limits.** None in comments.

**Fail-open.**
- 776: `let _authorized = grants(carrier)?;` for `Formal`/`InvocationFormal` actuals: the grant result is computed and discarded; no assumption-closure check is done for a formal reference. Status "open" (no comment); wrong program class: a formal that forwards a mathematical argument is trusted to have been checked where the formal was first bound (not traced).
- 814 `_ => {}`: an actual that is not mathematical against a formal with no carrier and with no mathematical content is accepted without comparing to `source_term`. Status "traced" (formal carries no mathematical carrier, so no kernel judgment is at stake, per the guard at 804-813).
- `collect` creates slots only when the callee formals or actuals have mathematical content (368-377, 407-416, 468-477); a call whose target formals and actuals both look non-mathematical produces no slot on either side, so the two modules are never compared on it. Status "traced" for the pairing (both sides use the same predicate), but a producer that hides mathematical content in a non-`Mathematical` `ProofTerm` shape would only be caught by `contains_mathematical` (178-212), which looks into `Construction` and `Array` fields only.

---

## sections.rs (1-774)

**Purpose.** Codecs and validators for the companion sections of an artifact: `artifact_manifest` (identities and domain-separated hashes), `debug_map` (`PSIDBG\0\0` source-span map), `optimization_execution` (`PSIOEXE\0` record of optimization selections). (read)

**Shape.**
- `pub(crate) use crate::semantic_module` (7).
- `artifact_manifest` 9-210: marker 4 (17); domains `psi-terminal-installation-section\0`, `psi-terminal-debug-section\0`, `psi-terminal-artifact-manifest\0` (18-20); `SectionFingerprint`, `TerminalArtifactIdentity` (32-byte newtypes with hex Display); `TerminalArtifactManifest{semantic, optimization, installation: Option, debug: Option, identity}` (65-71); `build_artifact_manifest` (95-104), `manifest_for_semantic` (106-127), `validate_artifact_manifest` (129-146, rebuild and compare), `hash` (181-187).
- `debug_map` 211-534: marker 1; `source_digest` (230-239); `encode_debug_map` (241-247), `decode_debug_map` (249-318), `validate_debug_map` (320-355), `subject_exists` (444-499) over nine `DebugSubject` variants (384-442).
- `optimization_execution` 535-774: marker 2; `PsiOptimizationExecutionRecord::new` (578-597); `validate_output` (619-627); `build_identity_optimization_execution_record` (630-641); encode/decode (643-690).

**Inputs/outputs.** Reads `TerminalModule` (for identity and subject existence), `optimization::PsiOptimizationSelections`, `Reader`/`Writer`. Produces bytes, identities, errors.

**How it decides.**
- Debug sites keyed on `DebugSubject` ids (`machine(id).is_some()`, block/operation/edge by id, claims via `content_entry_claims`/`content_identity_reshuffles`/`content_partition_compositions`, 444-499) and on `DebugFileId` + byte spans (`start <= end <= byte_len`, 347-349).
- Manifest equality by value (142).
- `PsiOptimizationExecutionRecord::new`: if `selections.is_empty()` then input identity must equal output identity (583-585).

**Cases by shape.** Nine `DebugSubject` tags (384-442); origin tags 1/2 (271-275).

**Same job elsewhere.** Terminal identity encode/decode: `append_terminal_identity` (700-703) and `append_identity_to_digest` (692-698) are two encoders of the same two fields; `decode_terminal_identity` (705-716) and the inline marker+fingerprint read in `decode_debug_map` (261-265) are two decoders. Hex printing: `write_hex` (189-194) vs inline hex in `PsiOptimizationExecutionIdentity::fmt` (560-566).

**Unused / one-caller.** Grepped names `build_artifact_manifest|validate_artifact_manifest|build_identity_optimization_execution_record|append_terminal_identity|encode_structural_type_declaration` across `omega-rust/*.rs`: 26 matches in 11 files (count mode, includes definitions/tests). `TerminalArtifactManifest::installation()` is never non-`None` from `CanonicalTerminalArtifact` (canonical_artifact.rs:95).

**Stated limits.** None in comments.

**Fail-open.**
- `build_artifact_manifest(..., installation_record: Option<&[u8]>, ...)` (95-104) hashes whatever bytes are given (116) with no decoding or validation of an installation record. Status "open": no comment; callers outside the unit not traced.
- `subject_exists` `Value` check iterates machine/block/operation scalar results only (452-467); a debug site naming a structural-place value would be rejected, not accepted (fail-closed).
- None else seen; `decode_debug_map` validates and re-encodes (313-316).

---

## module_wire.rs (1-5138)

**Purpose.** The wire encoders/decoders for every table inside a `TerminalModule`, written as many small `pub(crate) mod *_wire` modules (mostly using the `wire_record!` / `wire_enum!` / `delegated_fields!` macros from `wire_fields`, defined elsewhere), then the top-level `Module` record (4909-4968) and `encode_raw` / `decode_module_body` (5128-5138). It also contains 19 `#[test]` functions (grepped count). (read)

**Shape (module list with first line).** `borrow_wire` 7, `boundary_application_wire` 78, `callback_application_wire` 366, `calling_signature_wire` 460, `carry_and_suspension_wire` 627, `installed_interrupt_wire` 1527, `opaque_representation_wire` 1551, `quotient_wire` 1597, `recursive_component_wire` 1670, `unoptimized_behavior_wire` 1721, `access_lifecycle_wire` 1746, `access_view_plan_wire` 1965, `access_view_wire` 2386, `closed_conformance_wire` 2432, `declaration_wire` 2517, `dynamic_dispatch_wire` 2608, `evidence_wire` 2680, `float_meaning_wire` 2716, `folded_constant_wire` 2836, `native_layout_wire` 2853, `operation_crash_contract_wire` 2985, `proof_output_wire` 2999, `proof_scope_wire` 3057, `scalar_application_wire` 3716, `scalar_block_invariant_wire` 3792, `scalar_condition_wire` 3807, `scalar_predicate_wire` 4222, `target_family_wire` 4544, `target_layout_wire` 4632, `task_activation_wire` 4734.
- Top-level: `Module` record lists 53 fields in order (4910-4963) and `RootServiceReach` (4964-4967); field adapters `Vocabulary` (4970-4983, accepts only `VocabularyMarker::CURRENT`), `Identities` (4985-5007), `IndexedTheorem` (5009-5042), `MathematicalCarrier` (5044-5073), `IdentityFormation(s)` (5075-5126).
- Hand-written (non macro) codecs: `access_lifecycle_wire::encode/decode` (1774-1963), `access_view_plan_wire` (1998-2384), `boundary_application_wire` (89-238), `carry_and_suspension_wire` handle/span/loan graph (752-1037), `proof_scope_wire` (3074-3567), `scalar_application_wire` (3724-3790), `scalar_condition_wire` (3819-4055), `scalar_predicate_wire` (4236-4370), `target_family_wire` (4552-4630), `native_layout_wire::Catalog` (2871-2908).

**Inputs/outputs.** Each reads/writes `Writer`/`Reader` from `semantic_module::wire` and types from `terminal_psi`, `semantic_vocabulary`, `access_plans`, `layout_plans`, `numerics`. Output is bytes or the typed table.

**How it decides (the decoders' accept/reject judgments).**
- Tag bytes -> variants, unknown tag -> `InvalidTag(label, byte)` (e.g. 440, 1878-1912, 3380, 3772, 3928-3945).
- `Identities::read` (4985-5007): first entry must be `""`; others non-empty and unique (by string, `BTreeSet<&str>`).
- `IndexedTheorem::read` (5020-5041): certificate must decode (5026); `assumption_closure` strictly increasing; `type_identity` present.
- `MathematicalCarrier::read` (5052-5072): `structurally_equal(term, expected)`, empty context, level arity 0.
- `HandleKind::read` (759-766): `(slot==0 && gen!=0) || (slot!=0 && gen!=1)` -> `NonCanonicalEncoding`. `SpanKind::read` (776-783): `(count==0) == start.is_valid()` -> noncanonical.
- `decode_suspension_call_plan` -> `validate_loan_coordinates` (870-1025): graph shape rules over loan resources (`IncomingReference`, `CompletedOutput`, `LifetimeDependency`, `OutputInput`, `DescriptorTransfer`, 929-983) and backing/capture pairings (984-1015).
- `scalar_predicate_wire::decode`: a node may reference only earlier nodes (`reference` 4249-4256); root must be an earlier node.
- `scalar_condition_wire::decode_integer`: delegates to `BigInt::from_canonical_words` (3835).
- `decode_placement` (access_view_plan_wire 2128-2193): finishes with `AccessPlanSnapshot::validate()` (2191).

**Cases by shape.**
- `access_view_plan_wire::decode_access` (2327-2384) `Region` observation accepts only tags 0 (Stable) and 1 (External) (2334-2337) while `encode_access` writes `Atomic => 2` for Region (2262-2266). So an encoded `Region` with `ObservationModel::Atomic` could not be decoded. "traced": the access-plans policy rejects `Region` with `observation == Atomic` (`plan_policy.rs:63-66` in `psi/foundation/access-plans`, read), so a validated plan cannot hold one.
- `callback_application_wire` etc. use fixed tag tables; retired tags are simply absent: `CallTarget` tags 1,2,5 (a test names 3 and 4 as retired, 1079-1085); `Route` tags start at 4 (2551-2553); `ScalarConditionSubject` tags 1,2,4 (3980-3991); `Storage` tags 2,3,4 (702-706).
- `OpaqueRepresentation` origin/lifecycle accept only tag 1 each (1579-1580).
- `FloatMeaningSource` has a variant literally named `TransitionalInput` (2815).

**Same job elsewhere.**
- `Digest` field adapter for `[u8;32]` is defined twice (opaque_representation_wire 1586-1595; native_layout_wire 2963-2972) while `callback_application_wire` and others import `wire_fields::Digest` (369).
- `ApplicationId` Field for `LayoutApplicationId` is defined twice (target_layout_wire 4711-4720; task_activation_wire 4856-4865).
- `RecordStorage: AuthoredRecordStorage {byte_size, byte_alignment}` record is defined twice (target_layout_wire 4679; task_activation_wire 4831).
- `optional_u64` write/read pair (access_view_plan_wire 2221-2233) vs `Optional<U64>` in `native_layout_wire` (2923-2929): same job.
- Placement/layout entry encoding (`LayoutPlacementReport` At/IntegerAt/Bits) is hand-written in `access_view_plan_wire` (2035-2072, 2142-2168) and macro-written in `native_layout_wire` (2956-2960).
- Target-layout input record (`TargetLayoutInput`) written by `access_lifecycle_wire` (1780-1787, 1869-1880) by hand and by `target_layout_wire::Input` (4660-4667) via macro.
- Suspected: `semantic_module::wire::Writer::len` bounds vs the u64 length helpers in canonical_artifact.rs (not compared).

**Unused / one-caller.** Public wrappers re-exported in lib.rs: `encode/decode_access_lifecycle_catalog` (1756, 1764), `encode/decode_access_view_catalog` (1980, 1988), `encode/decode_access_view_input_catalog` (2416, 2424), `encode/decode_quotient_catalog` (1659, 1666). I did not grep per-function call counts for them (not done).

**Stated limits.** None as comments beyond error strings, e.g. "unknown physical calling owner" (549), "unknown calling operand kind" (598), "layout alternative has absent field storage" (4728).

**Fail-open.**
- None seen that accept an unknown tag; all tag matches have a rejecting arm.
- `decode_loans` (850-868) inserts decoded rows without cross-checking; the checks happen afterwards in `validate_loan_coordinates` (809). Traced: `decode_suspension_call_plan` calls it before returning.
- `proof_scope_wire::decode` casts `reader.count()? as usize` (3395) with no range handling; `count()` is defined elsewhere (not read), so I cannot say whether it is bounded ("open").
- `decode_integer` (3828-3837) loops `reader.count()` times pushing u64 words into an unbounded `Vec::new()` (3831-3834); bounded only by input length via `reader.u64()` failing (inferred).

---

## foundation_validation.rs (1-11882)

**Purpose.** The "structural foundation" validator that `decode_module` runs on every decoded module (semantic_module.rs:4109). It checks that all cross-references in the module are closed and well-typed: call operations, boundary declarations, structural types/places/paths, storage operations, scalar qualifications, layouts, task-activation demands, services, target families, proof scope. It returns `CodecError::Malformed*` strings and has no output except accept/reject. (read; `validate_structural_foundation` is `pub(crate)` with one caller, semantic_module.rs:4109, grepped.)

**Shape.** One flat file of nested `pub(crate) mod`s. Map with first line:
- `call_foundations` 7: `validate_call_tail` 29, `validate_module_invocation_entry` 61, `validate_no_normal_invocation` 128, `validate_call_unit` 142, `validate_call_dynamic` 168, `validate_call_structural_scalar` 372, `validate_call_structural` 401, `validate_structural_call_result_declaration` 548, `validate_result_claim_shape` 580, `validate_external_completion` 629, `validate_boundary_call` 706, `validate_port_write` 818, `validate_register_access` 834, `validate_outcome_subjects` 873.
- `declaration_foundations` 933: nested `calling_signatures` 948 (`validate` 956, `validate_census` 1037, `validate_telescope` 1288), `opaque_representations` 1349 (`validate` 1357, `same_shape` 1469, `same_record_storage` 1506, test 1558); `validate_domains` 1697, `validate_services` 1748, `validate_boundary_machine` 1769, `validate_provider_candidate` 1868, `validate_boundary_scalar_rows` 1945, `validate_provider_scalar_contract` 2000.
- `installed_interrupts` 2119; `machine_foundations` 2254 (`validate_machine` 2276); `native_layouts` 2642; `proof_scope` 2882; `reference_outputs` 3369; `scalar_applications` 3526; `scalar_boundary_contracts` 3924; `scalar_fields` 4028; `scalar_observations` 4320; `scalar_qualifications` 4688.
- `storage_foundations` 5104: `byte_sequences` 5113, `element_views` 5533, `representation_geometry` 5863, `structural_fields` 5979, `structural_transfers` 6723 (`StructuralTransferInputs` 6733 caches a descriptor graph for one machine), then free fns 7118-8005 (`reference_store_carrier` 7118, `owned_store_carrier` 7190, `validate_establish_access_reference` 7252, `validate_establish_reference` 7298, `validate_reference_memberships` 7436, `validate_release_reference` 7641, `field_store_carrier` 7673, `place_structural_type` 7748, `validate_global_descriptor_table` 7777, `validate_hardware_array_loan` 7813, `validate_descriptor_operation` 7932).
- `structural_type_foundations` 8007 (`validate_catalog_type_shape` 8074, authored-storage/fragment checks 8356-8537); `target_layouts` 8808; `task_activation` 9201; `unoptimized_behavior` 9420; `value_foundations` 9512 (constructors/establishment validators 9532-10157).
- Free fns: `declared_scalar_type` 10199, `qualification_domains` 10218, `scalar_domain` 10233, `values` 10244, **`validate_structural_foundation` 10256** (the driver), `validate_boundary_application_operations` 10483, `validate_callback_registrars` 10505, `validate_target_families` 10531, `validate_provider_attachment_foundation` 10613, `validate_structural_path` 10627, `validate_reference_origin_path` 10653, `validate_structural_parameters` 10824, `operation_foundation_error` 10851, **`validate_operation_foundation` 10877** (per-operation dispatch), `validate_structural_qualifications` 11224, `callee_exact_payloadless_return` 11265, `validate_structural_arguments` 11333, `owned_byte_view_presentation` 11571, `structural_place_type` 11610, `block_parameter` 11641, `validate_claim_actions` 11695, `validate_structural_type_graph` 11741, `validate_service_parent_graph` 11805, `has_service` 11851, `require_known_services` 11858, `require_unique_nonempty_identities` 11871.
- Counts (grepped): 618 `malformed(` calls in the file; 37 `unreachable!("dispatched ...")` (each per-operation validator re-matches its own `OperationKind` and panics otherwise); 9 `#[test]` functions in the file.

**Order of work in the driver (10256-10481).** scalar applications -> task activation -> folded constants -> scalar qualifications -> boundary scalar contracts -> scalar fields -> quotient/closed-reach/closed-conformance (calls into `terminal_semantics`) -> access-view inputs, plans, lifecycles -> target families -> boundary application operations -> callback registrars -> installed interrupts -> structural identity uniqueness -> target layouts -> native layouts -> type shapes + type graph -> dynamic reference regions -> domains/services -> external providers -> unoptimized behavior -> calling signatures -> boundary machines -> provider candidates -> provider instance requirements (10401-10471) -> every machine (`validate_machine` 2276) -> activation storage -> reference memberships -> runtime evidence inputs.

**Inputs/outputs.** Reads the whole `TerminalModule` plus ~100 helper functions from `terminal_semantics` (imported at 10186-10197 and in each module). Mutates nothing except the `StructuralTransferInputs` cache (6733-6759). Produces `Result<(), CodecError>`; `operation_foundation_error` (10851-10875) rewrites a `MalformedStructuralFoundation(reason)` into `MalformedOperationFoundation{machine text, state text, block, operation, reason}`; other variants pass through.

**How it decides (what the judgments key on).**
- Ids and positions: most checks key on ids (`PlaceId`, `StructuralTypeId`, `ValueId`, `OperationId`) and positions (parameter position, argument index, `call_ordinal`). Typical lookups are linear scans such as `has_service` (11851-11856), `module.structural_type(id)`, `machine.structural_places.iter().find(...)` (27 uses of `place_structural_type`/`structural_place_type`, grepped).
- Identity text (spelling): the module's `identities` text table is used as the canonical name in several accept/reject decisions:
  - `CallingSignatureId::for_application(requirement_text, application_text, operands)` must equal `row.id` (1073-1081);
  - `callback_requirement_id(identity_text) == binder.requirement` (1322);
  - installed-interrupt `registrar_requirement` text must equal `registrar.source.callable` text (2171-2174), and `slot_commitment` must equal a hash of `slot_encoding` (2186-2189);
  - native layout slot identity must equal `format!("{}#{:016x}", declaration_identity, application_report_fingerprint)` (2810-2814), and `layout_subject_identity == policy` text (2817-2819);
  - task-activation frame identity must equal an FNV-1a hash of the callable text and entry text with a leading byte `0x57` (9347-9358);
  - scalar domain issuer `ExactMachine` compares `callable_evidence_identity(callable text, specialization commitment)` to the issuer identity text (4896-4899);
  - target-layout targets by string (`realization.target == input.target`, 8901; targets strictly increasing as strings, 8884-8889);
  - scalar-return error text uses `format!` of `Debug` (4991) and `format!("{:?}", machine.id)` (511-517, 7006, 10866) but only to build error messages, not to decide.
- Structure comparisons: provider contract conjunct sets (`conjuncts`, 2108-2117, 2077-2100) by `Proposition` equality; `same_shape` for opaque representations by field-by-field equality (1445-1504); `same_record_storage` rebases field ids by ordinal position then compares whole applications (1506-1556).

**Cases by shape (fixed lists that refuse the rest).**
- `validate_external_completion` (648-661): "external completion disagrees with the canonical process-exit signature": exactly one scalar param of signed 32-bit non-address integer, no structural params, unit result.
- `validate_descriptor_operation` (7932-8005) and `validate_hardware_array_loan`: x86 descriptor-table shapes baked in: GDT entry count `(1..=8192)` for selector reload (7953), `(2..=8192)` for task activation (7974), scratch exactly ten bytes (8, 10), task state exactly 104 bytes (7988), GDT element 64-bit (7962, 7987). Error text: "descriptor-table operand must be an exact unqualified ten-byte array" (7924).
- `validate_native_code_layout` (10095-10156): "native code layout needs an entry address and region sequence" (10129): exactly 2 fields, then region record exactly 2 fields (address, u64 length).
- `validate_trivial_affine_local`-style (`validate_establish_trivial_affine_local` 9787-9821): "trivial affine local must have an empty record type".
- `target_layouts::validate` (8820-9075): `realization.schema_argument.abi_version != 4` (8912), more than 32 fields (8913), more than 32 retired identities (8920), "layout schema ABI supports records only" (8867).
- `scalar_applications::validate` (3535-3731): layout observation result must be the "byte-count carrier": fixed unsigned 64-bit (3612-3615); `Evaluate` entry must have empty params and structural params (3626-3629, 3636-3638).
- `element_views::element_view_result_place` (5543-5576): access must be `SharedBorrow` for `Unrestricted`, `MutableBorrow` for `Affine`, never for `Linear` (5563-5567).
- `validate_register_access` (834-871): result/input carriers must be unsigned, non-address integers of exact register width.
- `validate_provider_attachment_foundation` (10613-10625): any machine with a `ProviderAttachment` place is rejected ("provider-backed attachment specialization is incomplete").
- `validate_reference_origin_path` (10653-10822): rejects `RepresentationView`, `AllElements`, `FixedRange` segments with messages "returned-origin interpretation is not an operand or owned path" (10636), "universal qualification is not an executable storage path" (10693), "fixed byte ranges require call-scoped byte-view custody" (10725).
- `validate_call_structural` (401-546): the `exact_payloadless` shape (436-462) - callee with only owned/unrestricted/no-qualification params, empty contract and claims, whose every return is an `EstablishScalarCase` with no fields (11265-11325) - is the only shape allowed to carry `selected_evidence` (465).
- `validate_call_dynamic` (168-370): slot-roster rules for family calls (198-225).
- `native_layouts::validate` (2642-2880): every record structural type must have exactly one native layout entry in id order (2649-2665).

**Same job elsewhere (inside this file unless noted).**
- Scalar declaration lookup by `ValueId` (parameters + block parameters + operation scalar results) is written out at 80-91, 675-686, 6141-6150, 9953-9962, versus the helpers `declared_scalar_type` (10199-10216) and `values` (10244-10254), which are used elsewhere in the file (e.g. 6169, 10966).
- Structural place type lookup: `structural_place_type` (11610-11639) and `place_structural_type` (7748-7775) give overlapping answers with different rules (the first scans `structural_parameters` then places and handles `TrivialAffineLocal`; the second uses `signature_structural_parameter`, `block_structural_parameters`, and only `OperationResult`/`ByteSequenceLiteral` places). 27 uses of the two names in the file (grepped, count mode).
- `has_structural_type(module, id)` (11737) vs `has_structural_type(catalog, id)` (8039): two functions with the same name, one for a module, one for a catalog of types.
- "declared memberships of a place" `declared_memberships` (6513-6537) re-does the parameter/operation-result lookup that `declared_scalar_type`/`values` do for scalars.
- Hardware array checks: `validate_hardware_array_loan` is shared by descriptor-table and descriptor operations (7810, 7962-7989).
- Outside the unit: the same pre-checks of calls (arity, callee result) likely exist in `terminal_semantics` and in the `validation` crate (suspected, not read).
- `exact bytes plain` closure `plain` (7896-7900) and `structural_type_foundations` field-kind matches (8203-8238, 8269-8296, 8322-8349) repeat the same atomic/structural/erased field checks three times for record, sum and mixed shapes.

**Unused / one-caller.**
- `validate_structural_foundation`: 1 caller (semantic_module.rs:4109, grepped).
- `validate_structural_arguments`: 4 call sites in the file: 36 (via `validate_call_tail`), 354 (inside a loop over dynamic table rows), 697, 792 (grepped, content mode). `validate_claim_actions`: 3 sites (43, 362, 799). `validate_call_tail`: 1 site (119). Neither function is defined anywhere else in `omega-rust` (grepped).
- `validate_provider_attachment_foundation(_module, machine)` (10613): first parameter is unused (underscore).
- `validate_call_structural` ends with `if exact_payloadless { return Ok(()); } Ok(())` (542-545): both paths return `Ok(())`.
- `OMEGA_COMPILER_CHECK_SITES` is read directly via `std::env::var_os` at 7024 and in 29 other places across 12 other files (grepped count: 30 total across 13 files for `var_os("OMEGA_COMPILER_CHECK_SITES")`); one other file defines a `trace_flags.rs` switch for the same variable (`psi/compilation/validation/src/trace_flags.rs:21`). The debug dump `trace_payload_producer_chain` (7043-7090) exists only for this flag.
- `validate_provider_candidate`, `validate_boundary_machine`: 5 matches across the repo, all inside this file (grepped, with `validate_domains`).

**Stated limits (comments or messages).**
- "provider-backed attachment specialization is incomplete" (8231, 10622).
- "fixed byte ranges require call-scoped byte-view custody" (10725).
- "scalar record field has invalid local result custody" etc. are conditions, not limits.
- No `TODO`/"migration" comments seen in this file (read).

**Fail-open risks.**
1. `validate_operation_foundation` final arm `_ => { if !matches!(operation.result, OperationResult::Scalar(_)) { return malformed("scalar operation declares a Unit result") } }` (11215-11219). Any `OperationKind` without an explicit arm only gets "result is scalar". A new operation kind added to the enum would silently receive only this check; I did not list which current kinds fall here. Status "open".
2. `validate_call_unit` (142-166), `validate_call_structural_scalar` (372-399) and `validate_call_structural` (401-546) check arities, callee result and the result place, but none calls `validate_structural_arguments` or `validate_claim_actions` for the operation's own `structural_arguments`/`claim_transfers` (grepped: those two helpers are only called at 36, 43, 354, 362, 697, 792, 799). A call whose structural arguments have the wrong concrete type, wrong access, or unknown claims is not rejected by this file for these three operation kinds. Status "open": there may be another checker in `terminal_semantics` or `validation` that I did not read.
3. `validate_call_dynamic` (168-370): `validate_structural_arguments` is called (354) inside `for row in tables...filter_map(table.rows.get(slot))`; when no table has a row for that slot the arguments are checked only for count (228-229). Status "open".
4. `validate_boundary_application_operations` (10483-10503): checks each demand names an existing operation id (binary search) and that ids are not repeated, but never checks the named operation is a `BoundaryCall`. Status "open"; `scalar_applications::validate_operator_association` (3733-3760) cross-checks demands only for `Operator` boundaries.
5. `unoptimized_behavior::validate` (9426-9483): `let Some(machine) = module.machine(body.machine) else { continue; }` (9467-9469) skips the completeness check for bodies whose machine is not in `module.machines`; `same_contribution` (9485-9510) compares only callee/boundary/service/cause and ignores `block`/`site`/`operation`, and extra retained operations are allowed (only "current ⊆ retained" is enforced, 9470-9480). Status "inferred" intent: unoptimized snapshot may outlive the optimized machine.
6. Refusal rows: `calling_signatures` (974, 982-984), `validate_census` (1099, 1205, 1216-1217, 1245, 1264, 1276) accept any row whose `Err(reason)` has a non-empty string and skip the remaining checks. Status "stated" by structure (rows are recorded refusals); what a downstream reader does with an `Err` row was not traced.
7. `scalar_observations::validate` (4542-4617): the proposition walk has `_ => {}` (4613); proposition variants not named (anything other than integer-math comparison, `StructuralIdentity`, `Equal/LessThan/LessOrEqual/ScalarIeeeFloatComparison`, `Conjunction/Disjunction`, `Implication`) are not term-type-checked. Status "open".
8. `proof_scope::validate`: zero ids are tolerated as "absent" for `pending_evaluation_conditions` (3027) and call fields `binder/callee/state/operator` (3190); no comment explains it. Status "open".
9. `task_activation::validate` returns `Ok` immediately when all four bundles are empty (9210-9216); crossings are checked only for non-zero identities and unique non-zero place/type/claims (9401-9415), `carry_obligations` and the relation between crossings and frames are not checked. Status "open".
10. `validate_structural_type_graph` (11741-11803) and `validate_service_parent_graph` (11805-11849) use `.expect("... validated before graph traversal")` (11757, 11819, 11837). Traced: `validate_services` checks parents exist first (1750-1756); type references are checked by `validate_type_shape` for Structural fields, arrays, views (8203-8254), and `task_activation` runs `validate_catalog_type_shape` before the graph (9246-9249). Not panicking for valid orderings; panics if order changes.
11. `native_layouts::validate` checks identity indexes only for presence (`text(...).is_none()` -> error, 2678, 2689-2693), not non-emptiness. Status "traced": fail-closed for out-of-range, accepts empty strings.
12. `validate_boundary_scalar_rows` / `scalar_boundary_contracts::validate_arguments` (3985-4007): domain check only for requirements listed on the boundary; arguments with extra qualifications are fine (`validate_provider` 4009-4026 is the reverse check).

---

## Coverage

- Read in full: canonical_artifact.rs 1-444 (444), foundation_validation.rs 1-11882 (11882), identity_formations.rs 1-206 (206), lib.rs 1-182 (182), mathematical_proof_actual.rs 1-942 (942), module_wire.rs 1-5138 (5138), sections.rs 1-774 (774). Total 19568 lines.
- Skimmed: none.
- Not read: none.
- Files outside the unit that I opened for context: semantic_module.rs 4085-4130 (read, 45 lines), `psi/foundation/access-plans/src/plan_policy.rs` 50-71 (read). Others only grepped.

## Five things a maintainer most needs to know about this unit

1. `decode_module` accepts a byte stream only if: it decodes, `validate_canonical_order`, `validate_structural_foundation` (foundation_validation.rs:10256) and `validate_identity_formations` pass, and re-encoding equals the input bytes (semantic_module.rs:4107-4118). foundation_validation.rs is the largest single file in the codec (11882 lines, 618 `malformed(` sites) and its per-operation dispatch `validate_operation_foundation` (10877-11222) ends in a catch-all arm that only checks "result is scalar".
2. The three `verify_*` functions that compare a module with a trusted expected module (`verify_identity_formations`, `verify_module_mathematical_actuals`, `verify_indexed_theorem_roots`) have callers only in test support code under `omega/compilation/compiler/tests/support/`; `verify_mathematical_proof_actual` has none (grepped). `decode_module` itself does only self-consistency checks on identity formations (identity_formations.rs:14-19).
3. Several decisions are keyed on identity **text** or hashes of text rather than ids: calling signature id (foundation_validation.rs:1073-1081), callback requirement id (1322), task frame identity (9347-9358), native layout slot identity (2810-2814), carrier correspondence across modules (mathematical_proof_actual.rs:50, 93-103, 701-704), Id-formation carriers (identity_formations.rs:148-159).
4. Fixed-shape checks are embedded in the generic validator: x86 descriptor-table sizes (7953, 7974, 7988, 8, 10, 104), process-exit signature (648-661), layout ABI version 4 and 32-field cap (8912-8920), native code layout two-field shape (10125). Adding another target shape means editing these in place.
5. The artifact container never fills the manifest's installation section (canonical_artifact.rs:95, `None`), while `build_artifact_manifest` (sections.rs:95) hashes any installation bytes it is given without parsing them. Also `Region` access encodes `ObservationModel::Atomic` (module_wire.rs:2265) that `decode_access` cannot read back (2334-2337); a validated access plan cannot hold that combination (plan_policy.rs:63-66).

## What you are unsure about

- Whether `validate_call_unit`/`validate_call_structural_scalar`/`validate_call_structural` argument typing and claim-transfer checks happen in another place (`terminal_semantics`, the `validation` crate, or an earlier decode validator in `semantic_module.rs`); I only know they do not happen in this file.
- Whether `Reader::count()` and `wire::bounded_count` (defined in `semantic_module/wire.rs`, not read) cap collection sizes, which determines whether the `Vec::new()` loops at module_wire.rs:3831, 4332 and the `count()? as usize` at 3395 can be driven to large allocations.
- Which `OperationKind` variants currently reach the catch-all arm at foundation_validation.rs:11215.
- Whether the `Err(reason)` rows accepted at foundation_validation.rs:974-984 are refused by every downstream consumer.
- The exact production consumers of `build_artifact_manifest` / `validate_artifact_manifest` (the grep count mixes definitions, re-exports and tests).
