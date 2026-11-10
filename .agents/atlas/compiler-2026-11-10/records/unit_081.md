# Unit 81 notes

Checkout: `<checkout>/` (main at 457adf4a11f). All paths below are relative to it.

Files covered (8, 21,669 lines, all read in full):

1. `omega-rust/psi/compilation/representations/terminal-psi/codec/src/semantic_module.rs` (4902)
2. `omega-rust/psi/compilation/representations/terminal-psi/codec/src/structural_wire.rs` (2259)
3. `omega-rust/psi/compilation/representations/terminal-psi/codec/src/term_wire.rs` (2957)
4. `omega-rust/psi/compilation/representations/terminal-psi/codec/src/wire_fields.rs` (1227)
5. `omega-rust/psi/compilation/representations/terminal-psi/semantics/src/access.rs` (2526)
6. `omega-rust/psi/compilation/representations/terminal-psi/semantics/src/bytes.rs` (1143)
7. `omega-rust/psi/compilation/representations/terminal-psi/semantics/src/calls.rs` (2951)
8. `omega-rust/psi/compilation/representations/terminal-psi/semantics/src/content_transformations.rs` (3704)

Path abbreviations: `codec/` = `omega-rust/psi/compilation/representations/terminal-psi/codec/src/`, `sem/` = `omega-rust/psi/compilation/representations/terminal-psi/semantics/src/`.

How the four codec files fit together (read + grepped): `semantic_module.rs`, `structural_wire.rs`, `term_wire.rs` and `wire_fields.rs` are not separate Rust modules of their own; each begins with `pub(crate) use crate::semantic_module::*;` (structural_wire.rs:6, term_wire.rs:7, wire_fields.rs:8 re-imports `CodecError`/`MAX_CONTENT_IDENTITY_BYTES`). Their inner `pub(crate) mod ...` blocks are addressed elsewhere as `crate::sections::semantic_module::<mod>` (e.g. structural_wire.rs:408-411, semantic_module.rs:1839). `codec/src/sections.rs:7` has `pub(crate) use crate::semantic_module;`. I did not read sections.rs/lib.rs, so the include/module wiring is inferred from those paths.

---

## 1. codec/semantic_module.rs

### Purpose
Entry points and validation for the canonical `PSITERM\0` Terminal Psi byte format: `encode_module`, `decode_module`, `semantic_fingerprint`, `terminal_psi_identity`, ordering validators, a dynamic-descriptor admission check, and wire encoders for machines, provider candidates, external providers, checked machine declarations and closed reach applications. About 1,650 of its 4,902 lines are `#[cfg(test)]` modules (50 `#[test]`, grepped).

### Shape (file:line)
- Re-export block 8-19 (pulls `block_wire`, `foundation_validation`, `module_wire`, and the `structural_*`/`term_wire` encoders into scope).
- `mod canonical_order` 21-1247. `validate_canonical_order(&TerminalModule)` at 53 (one long function, 53-865) plus helpers `validate_input_observation_order` 867, `validate_parameter_order` 888, `crash_routes_are_canonical` 924, `validate_crash_route_predicates` 934, `validate_canonical_proposition` 950 (explicit worklist, not recursion), `validate_integer_math_term_depth` 1075, `canonical_propositions_strictly_increase` 1099, `canonical_proposition_order_key` 1119, `canonical_scalar_term_order_key` 1125, `validate_scalar_term_depth_from` 1139, `validate_content_term_depth` 1152, `validate_identity_reshuffles` 1167. Test module 1194-1246.
- `mod dynamic_result_admission` 1248-1324: `validate_dynamic_descriptor_results` 1254.
- `mod external_provider_wire` 1325-1455 (`encode` 1337, `decode` 1383).
- `mod machine_declaration_wire` 1456-1556 (`encode` 1464, `decode` 1495, `validate` 1537).
- `mod machine_wire` 1557-2149: `wire_record! Machine: TerminalMachine {...}` 1841-1877 (22 fields), `wire_enum! ParameterKind/MachineResult` 1879-1886, `AssemblyTarget` 1910, `ReachApplication` 1937, `RankedScc` 1952 field adapters, `encode_ranked_scc` 1991, `decode_ranked_scc` 2034, `encode_rank_values` 2100, `decode_rank_values` 2118. Tests 1559-1815.
- `mod provider_candidate_wire` 2150-2255 (`wire_record!` 2166-2212; `encode/decode_provider_candidate_record` 2227/2237 add the marker header).
- `mod reach_application_wire` 2256-2511 (`encode` 2271, `decode` 2380, telescope 2331/2429, argument 2473/2497).
- Test-only modules: counted_collection_bound_tests 2513, external_provider_tests 2604, numeric_intrinsic_tests 2745, reference_transport_tests 2890, routed_scalar_tests 3238, target_family_tests 3645 (also holds shared fixtures `unit_machine` 3841, `identity_table` 3890), qualified_cleanup_receiving_tests 3932 (one `#[ignore]`d test, 3940, needs env var `OMEGA_QUALIFIED_CLEANUP_CANONICAL_ARTIFACT`), normal_qualification_tests 4193, global_descriptor_table_tests 4484, descriptor_operation_tests 4602, format_marker_tests 4880.
- Crate-level items 3923-4191: `MAGIC` 4044, `FORMAT_MARKER: u16 = 253` 4051 (comment 4046-4050), `FINGERPRINT_DOMAIN` 4053, three depth limits of 256 (4055-4059), `MAX_CONTENT_IDENTITY_BYTES = 1<<20` 4061, `encode_module` 4063, `encode_execution_module` 4072, `encode_structural_type_declaration` 4078, global `MODULE_DECODES: AtomicU64` 4086 + `module_decode_count` 4089, `decode_module` 4093, `encode/decode_crash_route_buckets` 4122/4136, `decode_canonical_proposition` 4144, `semantic_fingerprint` 4154, `encoded_fingerprint` 4161, `terminal_psi_identity` 4177, `fingerprint_bytes` 4184.

### Inputs and outputs
`encode_module` takes `&TerminalModule`, refuses a module whose identity table has an `invocation_scope` (4064-4068), then `encode_execution_module` runs `scalar_qualification_wire::validate` and `validate_canonical_order` and calls `encode_raw` (4072-4076). `decode_module` (4093-4120) reads magic and the u16 marker, calls `decode_module_body`, rejects trailing bytes, then runs, in order: `scalar_qualification_wire::validate` (4107), `validate_canonical_order` (4108), `validate_structural_foundation` (4109), `identity_formations::validate_identity_formations` (4110), `dynamic_result_admission::validate_dynamic_descriptor_results` (4114), then re-encodes with `encode_raw` and compares to the input bytes (4115-4118, error `NonCanonicalEncoding`). The fingerprint is a domain-tagged digest of the encoded bytes (4184-4191); with invocation-local identities it becomes a different digest over bytes plus the local identity list (4161-4175).

### How it decides
- Canonical order: `require_increasing`/`strictly_increasing` over keys. Keys are dense ids (e.g. structural referents by `id` 54-60, machines by `MachineId` 633, blocks by `BlockId` 685), tuples of ids (e.g. 97-106), or identity TEXT looked up through `module.identities.text(...)`: closed conformance applications 313-328 (owner, declaration text, fingerprint), `root_service_reach.installation_dependencies` by requirement text 424-436, provider candidates 516-536 (four texts), proposition declarations/applications compared with their `id` overwritten by `PropositionId(1)` so ordering is by semantic content (582-617).
- Propositions: equality operands must be in ascending canonical-encoded-bytes order (970-974); `IntegerMathEqual` operands use `left > right` (984); conjunction/disjunction may not nest same-kind and members must ascend by encoded bytes (1022-1051, 1065-1069); depth caps 256.
- Dynamic descriptor admission: keyed on the slot's structural result leaves containing a `View` leaf (1272-1283); only those rows are analysed with `DescriptorAnalysis` (1289-1320).
- Format marker: exact `==253` (4100); no range.

### Cases by shape
- Operation kinds with crash continuations: a fixed match of `Call`, `CallUnit`, `CallStructuralScalar`, `CallStructural` (702-720); claim-transfer ordering only for `CallUnit/CallStructuralScalar/CallStructural` (728-740); `BoundaryCall` completion receipts 764-771.
- Structural declaration shapes: `Record`, `Sum`, `Mixed`, `_ =>` ignored (347-354) and a six-arm ordering match (363-403).
- Terminator cases for affine cleanup uniqueness: only `Jump` (781-798); `Crash` site guards 776-780. Conditional and StructuralCase edges are not checked for duplicate residual discards here (read; those terminators are not matched at 775-799).
- Tag spaces in `machine_wire` are single-variant lists: ranked SCC only tag `3` (2006, 2037), comparison `1|2` (2023-2026).
- `decode_kinds` accepts reach parameter kinds `0..=5` (2324).

### Same job elsewhere
- Ordering/structural validation of the same tables also exists in `foundation_validation` (`codec/src/foundation_validation.rs`, not read, suspected) and as separate semantic validators in `sem/` (this unit: access.rs 503, calls.rs 1078/1491/1945, content_transformations.rs 3651). This file's `validate_canonical_order` checks only ordering/canonical shape; deeper semantics are elsewhere.
- Two different format markers: this file's `FORMAT_MARKER = 253` (4051) and `mathematical_certificate_wire::FORMAT_MARKER = 3` with magic `PSICORE\0` (term_wire.rs:441-442). Grepped `FORMAT_MARKER` in `omega-rust/psi`: 22 occurrences in 5 codec files (semantic_module 5, sections 8, term_wire 3, module_wire 3, canonical_artifact 3).
- Parameter order validation `validate_parameter_order` (888) is the only definition found in this unit; machines, boundaries and blocks all call it (635, 485, 657).

### Unused or one-caller code (greps over `omega-rust`, `output_mode: count`)
- `canonical_proposition_order_key`: 20 occurrences in 11 files; used outside (e.g. optimization-unit unit_validation.rs 4, register-homes structural_declarations.rs 4).
- `canonical_scalar_term_order_key`: 7 occurrences, 3 users outside (06 proofs.rs 3, emission_cleanup.rs 2).
- `module_decode_count`: 16 occurrences; users are tests (`terminal_decode_once.rs` 12, `retained_runtime.rs` 2).
- `encode_structural_type_declaration`: 4 occurrences; 2 uses in native-artifact `physical/derivation/hashing.rs`.
- `decode_canonical_proposition`: 4 occurrences; 2 outside (register-homes recovery).
- `encode_provider_candidate_record` / `decode_provider_candidate_record` (regex `(en|de)code_provider_candidate_record`): 3 occurrences total = 2 definitions here (2227, 2237) + 1 in `codec/src/lib.rs`. No call site anywhere in `omega-rust` (re-export only).
- `terminal_psi_identity`: 45 occurrences across 25 files; widely used.
- `decode_crash_route_buckets`/`encode_crash_route_buckets`: not separately grepped; exercised in this file's tests 2548-2572.
- Comment 2-4 and 4044 call these the "canonical" entry points. Users outside the file exist for `encode_module`/`decode_module` (not counted; widely used by the pipeline crates per the `terminal_psi_identity` spread), so not a finding.
- `MODULE_DECODES` (4086): a process-wide mutable counter incremented on every `decode_module` (4094) and read only by `module_decode_count` (tests, see above).

### Stated limits
- 4046-4050 (comment on `FORMAT_MARKER`): "Both independently extended 252 layouts, and all earlier layouts, are incompatible." Tests at 2996-3004, 3658-3666, 3898-3920 and 4880-4901 pin refusal of markers 223, 246, 164, 250-252.
- 3940: `#[ignore = "explicit qualification supplies OMEGA_QUALIFIED_CLEANUP_CANONICAL_ARTIFACT"]` (the only ignored test in the unit).

### Fail-open risks
- `unwrap_or_default()` on identity-text lookups inside ordering keys (323, 433, 522-531): a dangling `TerminalIdentityIndex` becomes `""` in the key. Input that reaches it: closed conformance applications, installation dependencies, or provider candidates whose identity index is not in the table. Effect: ordering is checked on `""` instead of rejecting. Status: "open" (I did not read `identity_formations`/`foundation_validation`, which may reject dangling indices).
- `validate_canonical_order` callback loop (74-80): `boundary: None => !application.entry_admitted` accepts a callback application with no boundary when it is not entry-admitted. No comment at the site; status "open".
- `decode_proof_term` branches `PROOF_TERM_FORMAL`/`INVOCATION_FORMAL` (term_wire.rs 1543-1548) skip `term.validate()` (see term_wire section); referenced here because `decode_module` relies on it.
- `encode_raw` is `pub(crate)` and skips every validator; tests use it to forge bad modules and assert `decode_module` rejects (e.g. 2714, 3113, 3589, 3822). `encode_module` (public) does not call `validate_structural_foundation`; only the decoder does. This is the stated trust-boundary design in AGENTS.md ("Verify at trust boundaries") but there is no comment at 4072-4076. Status: "traced" for decode (4109), "open" for what encoders downstream rely on.
- Otherwise none seen: every `None`/missing case in `dynamic_result_admission` returns an error (1259-1263, 1292-1310) except slots with no row or non-structural result, which are skipped (1266-1271) because only `View`-leaf rows need analysis (stated by the code shape at 1272-1283).

---

## 2. codec/structural_wire.rs

### Purpose
Wire encoders/decoders for structural field declarations, byte-sequence carriers, structural places and paths, affine cleanup actions, operation results, boundary-machine signatures, and structural type declarations (record/sum/mixed/array/reference/atomic/byte-sequence).

### Shape
- `mod structural_field_wire` 8-403: IEEE format/comparison tags (28-71), canonical field paths (111-166), `ByteSequenceCarrier` 168-216, `StructuralFieldDeclaration` 218-310, `StructuralFieldType` 232-361 (11 variants, tags 1-10; `AtomicBoundedInteger` 10, `Atomic` 8, `RefinedScalar` 9).
- `mod structural_place_wire` 405-883: `encode_structural_arguments` 420, `encode_affine_cleanup_action` 433 (tags 1,2,3,4,5), path encode/decode 466/648, `encode_reference_origin_path` 479, `encode_projection` 501 (tags 1-10), `encode_structural_place_kind` 549, `require_operand_path` 706, qualification path 763-780. Tests 782-882.
- `mod structural_result_wire` 885-1133: `validate_reference_sources` 906, `validate_reference_source_order` 959, `encode_function_result` 977 / `decode_function_result` 1017, `wire_record! OperationResult` 1076, `Multiplicity` 1115.
- `mod structural_signature_wire` 1135-1727: nested `reference_outputs` 1161-1250; `wire_record! BoundaryMachine` (27 fields) 1252-1281, `Callable`, `DomainRequirement`, `RootIntroduction`, `StructuralParameter`, `BorrowPlace`, `BorrowCustody` (14 fields) 1282-1346; `wire_enum!` tables 1348-1387 (`Execution` tags 0-9); content projection expressions 1528-1650; access tags 1652-1684.
- `mod structural_type_wire` 1729-2259: `encode_structural_type` 1747, `decode_structural_type` 1872, authored field storage 1997-2089; tests 2091-2258.

### Inputs and outputs
Pure functions over `Writer`/`Reader` (defined in wire_fields.rs) and `terminal_psi` / `semantic_vocabulary` types. Only `validate_reference_sources` (906) takes the module and machine and reads `module_foundation_validation::validate_structural_path` and `validate_reference_origin_family` (outside the unit).

### How it decides
- Every enum is encoded as a hand-numbered tag; decoding is `match reader.u8()` with `InvalidTag(label, tag)` for the rest. Tag spaces differ by type: `StructuralPathSegment` projection tags 1,2,3,4,6,7,8,10 and origin-only `9` (501-539, 674-704); `CanonicalStructuralPathSegment` tags 1,2,3,4 with `Referent` = 4 (129-142) vs. `StructuralPathSegment::Referent` = 3 (516). Two tag spaces for the same concept, pinned by test 372-401 ("both canonical tag spaces").
- `require_operand_path` (706-719) refuses `RepresentationView` and `AllElements` in executable operand/owned paths: "returned-origin interpretation is not an operand or owned path". Origin paths refuse `AllElements` (488-491, 666-667).
- Record storage tags: `Packed` 1, `Authored` 10, `TargetLayout` 13, `AccessView` 14, `Accessor` 15 (1800-1806); decoder shares one arm for 14|15 (1906-1923). `PrimitiveScalar` tag 6, or 12 when `scalar_restriction` is set (1768-1773; decode 1892, 1901-1905). `AtomicCell` tag 16 (1784, 1893) only if `cell.is_supported()` and no restriction (1779-1783).
- Reference-result validation compares source `access` to the reference type's access and, for `Projected` extent, the endpoint to the referent (942-953).

### Cases by shape
- Decoder accepts only tags enumerated; encoder arms for `StructuralTypeShape` are exhaustive (1761-1868).
- `ContentProjectionScalar` / `ContentProjectionExpression` depth hard-coded `> 256` (1593, 1620), not tied to a named constant; failure is reported with `InvalidTag("...Depth", 0)` (1594, 1621), a tag error used for depth.
- `decode_authored_field_storage` fragmented recipe requires `bounded_count(reader, 40)` and `count != 0` (2063-2072); 40 is 5 u64 fields.

### Same job elsewhere
- Path-segment encoding exists three times in this unit: `structural_field_wire::encode_canonical_structural_path` 121 (canonical paths), `structural_place_wire::encode_projection` 501 (`StructuralPathSegment`), and `wire_fields::FieldPath` 464-489 (decodes the canonical path a third time, hand-copying the `match` of `decode_canonical_structural_field` 147-166).
- `structural_signature_wire::Execution`/`Completion`/etc. use `wire_enum!`/hand impls interchangeably (e.g. `Completion` is hand-written 1441-1459, `Role` is `wire_enum!` 1361).
- Authored/target-layout geometry also appears in the access views in `sem/access.rs` (e.g. `access_view_plans` 1646-2109) — different job (validation), same data.

### Unused or one-caller code
- Not individually grepped; the `pub(crate)` encoders are reached through `wire_fields.rs` adapters (`Path` 438, `Argument` 448, `QualificationPath` 1106, `CleanupAction` 1162 in wire_fields.rs) and `semantic_module.rs`.

### Stated limits
- None in comments (this file has no prose comments beyond the header 1-4).

### Fail-open risks
- `decode_structural_type` tag 12 (1901-1905): `scalar_restriction = Some(decode_structural_field_type(...))` accepts any `StructuralFieldType` as a restriction (`Scalar`, `Structural`, `Erased` ...). Rejection of wrong restriction kinds is not here; `validate_canonical_order` only checks `RefinedScalar` predicates (semantic_module.rs 342-346). Status: "open" (I did not read `foundation_validation`).
- `decode_structural_type` tag 13 (`TargetLayout`) takes a raw `u32` `LayoutApplicationId` (1942-1945) with no range check here. Status: "open".
- Otherwise none seen: unknown tags/booleans are errors.

---

## 3. codec/term_wire.rs

### Purpose
Wire encoders/decoders for propositions, scalar terms, integer-math terms, content terms and transformations, machine contracts, mathematical certificates (the `PSICORE\0` kernel proof format), proof declarations and proof terms, scalar qualification catalogs, and scalar/integer primitive types.

### Shape
- `mod content_wire` 9-251: `wire_record!` set 35-90 (entry claims, partition compositions, derivations, boundary body requirements, address transport, `ContentPlace`), `wire_enum!` 92-103, field adapters `ContentPlaceDeclaration` 105 (rejects `BlockParameter` places, tag 8, at 111 and 121), `Conservation` 128, `Term` 144, term encode/decode 182/225 (tags 1 projection, 2 separate).
- `mod contract_wire` 253-330: `Successor` and `Contract: MachineContract` records 270-305; `CrashCause` tags 1,2 and `CrashRouteGuard` tags 0,1 (307-313).
- `mod integer_math_term_wire` 332-428 (tags 1-7).
- `mod mathematical_certificate_wire` 430-1189: `encode_mathematical_certificate` 451, `decode_mathematical_certificate` 509, `MathematicalTermWriter` 573 (28 term tags 1-28 at 592-833, hash-consed by node bytes), levels 845-874 / 1110-1126, `decode_term` 876-1100 (tracks per-node depth), tests 1139-1188. Constants `MAGIC = b"PSICORE\0"` 441, `FORMAT_MARKER = 3` 442, depth limits 256 at 443-444.
- `mod proof_declaration_wire` 1191-1291.
- `mod proof_term_wire` 1293-1740: tags 1-9 (`PROOF_TERM_*` 1303-1311), `encode_proof_term` 1324, `decode_proof_term` 1446, erased proof formals 1571-1599.
- `mod proposition_wire` 1742-1977: encode 1762 (tags 1-18), decode 1885.
- `mod scalar_qualification_wire` 1979-2269: `encode` 2000, `decode` 2091, `validate` 2182.
- `mod scalar_term_wire` 2271-2853: encode 2300 (tags 1-41), decode 2682, helper constructors `ScalarTerm::*` called with `fn` pointers (2635-2680).
- `mod scalar_wire` 2855-2957.

### Inputs and outputs
All pure over `Writer`/`Reader`. `mathematical_certificate_wire` uses `proof_admission::{TermArena, MathematicalCertificate, ...}`; uses `HashMap<TermHandle,u32>` and `HashMap<Vec<u8>,u32>` (576-577, import 432).

### How it decides
- Depth: separate budgets restart at 0 for scalar terms nested inside propositions (e.g. 1779-1780, 1897-1898) and for content terms (1819-1821); proposition depth 256, scalar 256, content 256 (semantic_module.rs 4055-4059).
- Mathematical certificate: encoder refuses `Term::Dummy` (603-607). Decoder requires each child index `< handles.len()` ("term child does not precede its parent", 885-889), per-node depth ≤ 256 (529-533), roots in range (1128-1137). Canonicality is enforced by `decode_canonical` re-encoding (509-571; helper in wire_fields.rs 297-311).
- `scalar_qualification_wire::validate` (2182-2268): keyed on ids/tuples ascending, origin lists nonempty and strictly increasing, route identities not absent, set members unique across sets (2225-2229), float ranges same format (2246-2252), integer ranges ordered (2260-2266).
- Mixed vs plain mathematical application: tag 8 iff any argument is `Scalar` (1348-1355); decoder accepts tag 8 with all-formal argument kinds, rejected later by the canonical re-encode (test 1674-1682).

### Cases by shape
- `encode_integer_type` (2892-2902): `(IntegerCarrier::Address, IntegerSign::Signed) => unreachable!("address carriers are unsigned")` (2897-2899). A panic, not an error, if such a type reaches the encoder.
- Byte predicate tag only `1,2,4,8` (2771-2776).
- Scalar term encode matches every variant (2308-2581); decode accepts the 41 numbered tags (2689-2807).

### Same job elsewhere
- Level encoding appears twice: inline levels in `encode_level` (845) and `decode_level` (1110), used by `Term::Constant` (754-760, 1033) and `encode_sort` (836).
- Scalar term operand-depth validation duplicates between `canonical_order::validate_scalar_term_depth_from` (semantic_module.rs 1139) and the encoder's own `depth > MAX_SCALAR_TERM_DEPTH` check (2305).
- `scalar_wire::{encode,decode}_integer_value` 2904/2948 and `wire_fields::Integer` 503 are the same functions wrapped.

### Unused or one-caller code
- Grepped: no individual `pub` function here flagged unused; `encode_scalar_terms`/`decode_scalar_terms` are used by `wire_fields::ScalarTerms` 737.

### Stated limits
- Comments: only header 1-5. No limit statements.

### Fail-open risks
- `decode_proof_term` `PROOF_TERM_FORMAL` and `PROOF_TERM_INVOCATION_FORMAL` (1543-1548) return the term without `term.validate()`, unlike every other proof-term branch (1461, 1508, 1529, 1540, 1551, 1564). Input reaching it: a proof term with any formal `position`. `validate()` for `Formal` may be trivially true; I did not read `ProofTerm::validate`. Status: "open".
- `decode_mathematical_certificate` does not check that a `Constant { declaration, .. }` index is in range or that variable indices are bound (754-760, 893); those are the kernel checker's job in `proof_admission` (not read). Status: "open".
- `scalar_qualification_wire::decode` (2091-2180) does not validate `site`/`origins` consistency; `validate` (2182) does only ordering and nonemptiness. Cross-reference validation is outside this file (semantic_module tests 3341-3643 show it happening in `decode_module`). Status: "traced" via those tests that forged rows are rejected.

---

## 4. codec/wire_fields.rs

### Purpose
The byte-level cursor types (`wire::Writer`, `wire::Reader`) and the declarative adapters (`Field<T>` trait, `wire_record!`, `wire_enum!`, `delegated_fields!`, `integer_fields!`, `byte_enums!`, `argument_claims!`, `copied_fields!`, `u32_newtypes!`, `optional_packages!`) used to describe wire layouts as data.

### Shape
- `mod wire` 10-312: `Writer` 37-128 (`len` checks `u32::try_from`, `string` caps at `MAX_CONTENT_IDENTITY_BYTES`), `Reader` 130-219 (`take` uses `checked_add`; `boolean` accepts only 0/1; `id` rejects zero via `PsiSemanticId::new`), `bounded_count` 231-245 (refuses counts whose minimum encoded size exceeds remaining input), `decode_counted` 247 (min element size 1), `strictly_increasing` 266, `require_increasing` 270, `decode_canonical` 297.
- `trait Field<T>` 342; `ResultField` 347; label helper `label::<T>()` 368 (uses `std::any::type_name`); adapters `Id` 373, `OptionalId` 384, `Ids` 395, `U8..U64`/`Flag` 416-436, `Path` 438, `Argument` 448, `FieldPath` 464, `Bytes` 491, `Integer`, `Float`, `byte_enums!` 525-549, `Resident` 551, `Region` 579, `ByteOperand` 622, `ScalarCaseFields` 660, `RecordFields` 682, `argument_claims!` 741-769, `SelectedEvidenceRow` records 771-808, `List<K>` 858, `Optional<K>` 872, `wire_record!` 890-916, `wire_enum!` 934-972, `optional_packages!` 1171-1203, GDT and segment enums 1205-1227.

### Inputs and outputs
Pure; `Reader` borrows `&[u8]`; `Writer` owns a `Vec<u8>`.

### How it decides
- All list lengths are u32 (`Writer::len` 74-77). Reads refuse list counts that cannot fit in remaining bytes (231-245) but the minimum element size is 1 byte for `decode_counted` (251) and 4 for `strings` (204), 1 for `decode_bytes` (282).
- `label::<T>()` (368-371) names an error by the Rust type name's last path segment; it is only an error-message label.
- Wire enums have the tag numbers inline in each macro invocation; `IeeeFloatComparisonOperation` tags 0-5 (543-545) are a different numbering from `IeeeFloatComparisonKind` tags 1-6 (structural_wire.rs 35-47).

### Cases by shape
- `Ids::write` always labels errors "operation identities" (398) regardless of the id type.
- `RecordFields`/`ScalarCaseFields` encode fixed record shapes with a 2-variant tag (1 scalar, 2 structural) (693-702).

### Same job elsewhere
- `Reader`/`Writer` has the sibling helper functions `encode_optional_id` (221) vs `OptionalId` adapter (384), and `decode_counted` (247) vs `List<K>` (858). `OptionalField` (405-414) encodes `Option<StructuralFieldId>` as a raw u64 with `0 = None` rather than the `0/1` + id used by `OptionalId` (221-229); two encodings for the same idea.
- `FieldPath` (464-489) duplicates `structural_field_wire::decode_canonical_structural_field`'s `match` (structural_wire.rs 147-166) (same tag set, different error label).

### Unused or one-caller code
- Grepped: `ResultField` — not grepped. `copied_fields!` is used once for `Access`/`Multiplicity`/`Scalar` (825-832).

### Stated limits
- None in comments.

### Fail-open risks
- None seen. `Reader::boolean` rejects non-0/1 (183-189); `decode_optional_id` rejects tags other than 0/1 (286-295); `Reader::id` rejects zero (212-218). The exception is `OptionalField` (405-414), where raw `0` is `None` by design (StructuralFieldId::new(0) is `None`) and any other u64 is a field id; there is no unique-spelling issue because each id has one encoding.

---

## 5. sem/access.rs

### Purpose
Semantic judgments about "access" (placed memory/RAM) lifecycles and views in a `TerminalModule`: that the access lifecycle plans and their result shapes (`View`/`Initialize`/`Relocate`/`MoveOut`) agree with their boundary declarations, that RAM correspondence holds, which operation formed a descriptor that reaches a given call, that the access view catalog is internally consistent, and the exact derivation of accessor scalar-boundary demands.

### Shape
- `mod access_lifecycles` 6-1079: `result_contract` 13-495 (`AccessResultContract` enum 23-51, `access_view_reference_slot` 53, `access_result_contract` 80-385, helpers 387-494); `validate_access_lifecycle_plans` 503-788; `validate_access_view_content` 790-849; `validate_image_fields` 851-937; `validate_storage` 939-1078.
- `mod access_ram_correspondence` 1080-1295: `access_reference_correspondence` 1091, `access_ram_correspondence` 1122, `access_ram_plan_correspondence` 1145.
- `mod access_ram_origins` 1296-1644: `AccessRamFormation` 1314, `AccessRamOrigin` 1322 (`Formation`, `ProviderFormation`, `PrivateEntry`), `AccessRamOriginAnalysis` 1341, `boundary_formations` 1373, `resolve`/`resolve_origins` 1476/1488.
- `mod access_view_plans` 1645-2180: `AccessViewPlanError` (22 variants) 1656, `access_field_leaf` 1686, `validate_access_view_catalog` 1780-2046, `validate_access_view_plans` 2048, `contains_access_carrier(_in_catalog)` 2111/2119.
- `mod access_view_referent` 2181-2332: `validate_access_view_referent` 2199, `access_view_input_parameter` 2291.
- `mod accessor_scalar_boundaries` 2333-2526: `access_region_projection` 2344, `accessor_scalar_boundary_demand` 2400.

### Inputs and outputs
Read `&TerminalModule` tables (`boundary_machines`, `access_lifecycle_plans`, `access_view_plans`, `access_view_inputs`, `structural_types`, `structural_domains`, `identities`) and call crate-level helpers (`crate::reference_storage_*`, `layout_plans::LayoutTargetFacts`, `crate::descriptors::*`). All validators return `Result<(), &'static str>` or `Result<_, AccessViewPlanError>`; none mutate.

### How it decides
- Operation names come out of `TerminalAccessLifecycleOperation` and are compared as sum cases by position: `[ready, rejected] = cases` (98, 158, 277), so decisions are keyed on POSITION (case order) and field counts, not case names (e.g. 103, 131, 163, 166, 280, 285, 368).
- Qualifications on result paths are located by `StructuralPathQualification::select(...)` on constructed paths using `StructuralPathSegment::Case(id)` and `Field(identity)` (184-187, 240-243, 254-258, 294-298, 322-328, 340-345, 373-378).
- Strings appear as keys in two places: `canonical_member = format!("#{identity}")` or the layout entry's `field` name compared with `identities.text(field.schema_field)` (1894-1900); and `operation.kind` mapped to "read"/"take"/"write" compared with `access.access().operation_names()` (1971-1977).
- Identity comparisons through `module.identities.text(...)` equality: profile supply callable vs boundary identity (610-617), `canonical_access_view_identity(policy, schema)` vs `view_identity` text (1846-1852).
- Uniqueness of plan keys: `(boundary, target string)` strictly increasing (523-530).

### Cases by shape
- `access_result_contract` (80): exactly two outcomes Ready/Rejected for View/Initialize/Relocate (98-102, 158-162, 277-279) and exactly one `Moved` for MoveOut (365-367); exactly three failure cases (407-411, "placement rejection lost its three core failure cases"); range/alignment cases carry exactly two `u64` fields (412-431); third case must be empty (432-434).
- `validate_access_lifecycle_plans`: Relocate takes exactly four structural parameters (649-651), View two (677), Initialize 2 or 3 (691), MoveOut 1 or 2 (714).
- `validate_access_view_content` (790-849): nonresident `View` accepts primitive scalar non-address integers, `FixedArray`, `Record` with scalar (non-address integer) / `IeeeFloat` / structural fields; everything else refused (839-843 "requires explicit content validity or custody evidence", 831-835 "total field decoding evidence"). `Boolean` is not accepted by `scalar` (794-800 `ScalarType::Integer(_) if !integer.is_address()` only).
- `access_field_leaf` (1686): only `FixedArray` source with `FixedArray` accessor, literal equal nonzero lengths (1708-1731).
- `validate_access_view_catalog` signature matching enumerates (Read|Take|Write) x (scalar | structural) (1992-2023).
- `access_ram_plan_correspondence` array leaf walk handles only `FixedArray` with nonzero literal length then `PrimitiveScalar` (1209-1226).
- `validate_storage`: `TerminalAccessOwner::Extent | Run` only (950-1005).

### Same job elsewhere
- Structural type lookup by `declarations.iter().find(|row| row.id == ...)` is repeated dozens of times in this file (e.g. 1170-1172, 1811-1818, 1906-1909, 2136, 2319-2323); `module.structural_type(id)` is also used (84-86, 106-108). Both styles coexist.
- Ordering/uniqueness patterns: hand-rolled `BTreeSet` visited sets (1692, 1741, 1762, 1786-1789, 2059, 2125); 19 `HashMap/BTreeMap/BTreeSet` occurrences in this file (grepped with `HashMap|BTreeMap|BTreeSet`). AGENTS.md says dense ids should use flat tables (`tools/dense_id_maps.txt`); I did not compare against that allowlist.
- The "exactly one match" idiom: this file uses `crate::exactly_one::exactly_one` once (access_view_referent 2250) and `let (Some(a), None) = (it.next(), it.next())` elsewhere (1134, 1392, 2416, 2233-2234).
- Descriptor origin tracing in `access_ram_origins` parallels descriptor analysis in `sem/descriptors.rs` (via `DescriptorAnalysis`, imported 1300; not read).

### Unused or one-caller code
- `access_view_reference_slot` (53): 5 occurrences: access.rs 2 (def + `pub use`), lib.rs 1, 2 outside (optimization-unit `descriptor_results.rs`, `access_references.rs`).
- `validate_access_view_content` (790): 5 occurrences: access.rs 2 (def + call at 730), lib.rs 1, 2 outside (storage_references.rs interpreter, provider-admission `access_lifecycles.rs`).
- `access_ram_correspondence` (1122): 5 occurrences = access.rs 3 (def, call at 1117, re-export) + lib.rs 2. No call site outside `sem/`.
- `access_reference_correspondence` (1091): 5 occurrences: 2 outside (storage_references.rs interpreter) + foundation_validation 2.
- `AccessRamOriginAnalysis` (1341): 6 occurrences; used by interpreter `engine_execution.rs` 2 and provider-admission `compiler_builtins.rs` 1.

### Stated limits
- None in comments (file has only the 3-line header).

### Fail-open risks
- `validate_access_lifecycle_plans` first loop (504-520) only requires a plan for boundaries whose declaration names a placement operation; a boundary with `SelectedProvider` execution is accepted if it is a Trait kind (567-571). Status: "stated" by structure (provider supplies lifecycle). No comment.
- `contains_access_carrier_in_catalog` returns `true` for a missing declaration (2136-2138): fails closed (treats unknown as a carrier).
- `access_ram_origins::resolve_origins` returns `None` for any unknown origin kind (1637) and `None` when no callers (1626): fails closed.
- `validate_image_fields` size-vs-presence rule: `(size != 0) != plan.image_fields.contains(&field.id)` rejects zero-byte fields represented and nonzero omitted (930-933). None seen fail-open.
- None seen otherwise.

---

## 6. sem/bytes.rs

### Purpose
Judgments about byte-sequence carriers and their qualification predicates (`ValidUtf8`, `NoNul`, `AsciiOnly`, `NonEmpty`): which predicates a byte place is known to satisfy after copies, concatenations and subslices; proof goals for concatenation capacity and indexed stores; and constant-origin analysis for scalars.

### Shape
- `mod byte_carriers` 6-67: `byte_carrier_scalar_layout` 38, `leaf_scalar_carriers` 24.
- `mod byte_sequence_concatenation` 68-129: capacity goal 76, length equation 99.
- `mod byte_sequence_predicates` 130-1143: `byte_predicate_requirement` 146, `byte_field_root` 176, `byte_parameter_predicates*` 224-273, `byte_predicate_carrier` 275, `pure_byte_qualifications` 295, call-result helpers 320-433, `shared_byte_field_copy_predicates` 435, `byte_sequence_concatenation_predicates` 456, `byte_copied_field_predicates_at_transfer` 473-597, `concatenation_operand_predicates` 599, `may_change_owned_source` 657, `byte_sequence_source_predicates` 678-843 (memoized with a work budget), `indexed_byte_store_goal` 845, literal/array qualification holds 876-957, constant-origin code 959-1142 (`ScalarConstantOrigin` 959, `machine_scalar_constant_origin` 984, `MachineScalarIndex` 1054).

### Inputs and outputs
Read `TerminalModule`/`TerminalMachine` operations and domain tables; return `Option<ByteSequencePredicateSet>` / `bool` / `Proposition`. No mutation.

### How it decides
- A domain qualifies a byte carrier only if `domain.carrier == carrier`, no content projection, no establishment routes, no required memberships, and it has a `byte_predicate` (262-270). Keyed on the domain ids in the sorted `qualifications` list (257-259 refuses unsorted).
- `byte_sequence_source_predicates` (678-843): keyed on the place KIND of the source (`Parameter`/`BlockParameter`, `ByteSequenceLiteral`, `OperationResult`) and the producing operation kind (`ByteSequenceSubslice`, `StructuralLeafCopy`, `EstablishByteSequenceConcatenation`). Work budget is `structural_places.len() + 1` (840) and a memo `Vec` with a linear `find` (690).
- `byte_copied_field_predicates_at_transfer` (473-597): accepts a copy only if the copied place is consumed once by owned whole-place arguments (538-549), no other operation can change or alias the source (550-562), and it is not carried through any terminator (568-594).
- `byte_predicate_requirement` returns the conjunction of `ScalarTerm::ByteSequencePredicate == true` for predicates not implied (153-173).

### Cases by shape
- `leaf_scalar_carriers` (24-36): "`Scalar(Integer)` with bits%8==0, `Scalar(Boolean)`, `BoundedInteger` with bits%8==0"; everything else `None`.
- `byte_predicate_carrier` (275-293): `ByteSequence(_)` or `FixedArray` of `u8` (unsigned, 8 bits, non-address).
- `byte_field_root` (176-222): only `Parameter`/`BlockParameter`/`OperationResult` places; results must be Unrestricted or Affine with no qualifications/claims.
- `ScalarConstantOrigin` only handles `OperationKind::IntegerConstant` literals (1013).
- `indexed_byte_store_goal` (845-874): `AsciiOnly|ValidUtf8` add `<= 0x7f`; `NoNul` adds `1 <=`; `NonEmpty` adds nothing.

### Same job elsewhere
- Constant-origin resolution is written twice in this file: `machine_scalar_constant_origin` + `scalar_constant_origin_with` + `incoming_scalar_origin` (984-1052; linear scans) and `MachineScalarIndex::constant_origin` (1117-1141; sorted tables + `partition_point`). Both call `incoming_scalar_origin` (1040, 1140). Users: `machine_scalar_constant_origin` has 2 uses in `codec/src/foundation_validation.rs`; `MachineScalarIndex` has 3 uses in `sem/descriptors.rs` (grepped: `MachineScalarIndex` 6 occurrences in 3 files, `machine_scalar_constant_origin` 4 in 3 files).
- Predicate derivation for call results is written for owned (`byte_owned_call_result` 353) and view (`byte_view_call_result` 369) results with near-identical bodies.
- Byte literal qualification: `byte_literal_qualifications_hold` 876 vs `_in` 885 and the "`ByteSequencePredicateSet::holding(bytes)`" use at 733: two ways of evaluating predicates over literal bytes.

### Unused or one-caller code
- `byte_sequence_concatenation_length_equation` (99): grepped `byte_sequence_concatenation_length_equation`: 2 occurrences total = bytes.rs def + `lib.rs` re-export. No call site in `omega-rust`.
- `machine_scalar_constant_origin` (984): 2 outside uses (foundation_validation.rs).
- `scalar_constant_value` (967): 9 occurrences, used in optimization-unit `rewrite.rs` 2 and others.
- `byte_sequence_concatenation_capacity_goal` (76): not grepped.

### Stated limits
- None in comments (header 1-4 only).

### Fail-open risks
- `scalar_array_qualifications_hold` (908-957): `if qualifications.is_empty() { return true; }` (921-923). Vacuous truth: no qualification claimed means nothing to prove. "stated" only by the shape of the check.
- `concatenation_operand_predicates` (622-632): for a `Whole` operand with a path that is a `ByteSequence` field of a record, if `runtime_structural_field` returns `None` or the field type lacks a byte carrier, the code returns `ByteSequencePredicateSet::EMPTY` (628-631) instead of `None`. `EMPTY` is the weakest set (guarantees nothing), so a wrong program gets fewer guarantees, not more. "traced".
- `byte_sequence_source_predicates` returns `None` on exhausted budget (693) so an unprovable source yields no predicates: fail-closed.

---

## 7. sem/calls.rs

### Purpose
Judgments about call sites: composition schemas for each call operation kind, argument/loan compatibility for borrowed byte views and fixed windows, closed (static) conformance applications and their canonical ordering, closed service-reach applications, and runtime evidence input checks at calls.

### Shape
- `mod call_composition` 6-754: `bounded_byte_owner` 8-110 (`bounded_byte_owner_capacity*`), `fixed_byte_view` 111-412 (`FixedCollectionWindow` 122, `borrowed_referent_presentation` 143, `fixed_byte_extent` 181, `fixed_byte_array_window` 214, `fixed_byte_storage_window` 263, `fixed_collection_storage_window` 273, `fixed_element_array_extent` 304, `fixed_byte_array_loan` 370), `boundary_buffer_capacity` 434, `shared_boundary_buffer_capacity` 449, `stored_view_field_presentation` 464, `inline_byte_field_capacity` 488, ten rule enums 531-599, `CallCompositionSchema` 602, `CallCompositionSemanticRow` 658, `call_composition_rows!` macro 673-713 and its 6-row table 715-722, `validate_call_composition_semantic_rows` 731, `call_composition_semantic_row` 745.
- `mod closed_conformances` 755-1466: `is_checked_body_family_contract` 762, `validate_boundary_callable` 795, `validate_definition` 993 (parses the schema definition bytes), `argument_matches` 1052, `validate_closed_conformances` 1078-1408, `StaticApplicationOrderKey` 1419, `static_application_order_key` 1427.
- `mod closed_reach` 1467-1800: `validate_telescope` 1471, `validate_closed_reach_applications` 1491-1799.
- `mod evidence_calls` 1801-2904: runtime evidence structs (`inputs`) 1807-1928, `terminal_inputs::validate` 1953-2120, `validate_runtime_evidence_inputs_in` 2123-2349, rosters/outputs/observation helpers 2351-2887, `require_completed_capture` 2889.
- `mod invocation_inputs` 2905-2951: `validate_empty_erased_invocation_inputs` 2910.

### Inputs and outputs
Read `TerminalModule` tables: `boundary_machines`, `target_machine_families`, `closed_conformance_applications`, `machines`, `services`, `proof_output_calls`, `evidence_contract_lanes`, `evidence_terms`, `proposition_applications`; also the descriptor graph (`descriptor_results::terminal_graph`, `DescriptorAnalysis`, `PayloadMembershipHistory`, outside the unit). Output `Result<(), &'static str>`.

### How it decides
- Closed conformances key on identity TEXT: `module.identities.text(id)` equality (1063, 1202, 1224-1225), sorted-by-text schema keys (1090-1097), registry identities `previous >= callable_identity` (1185-1189), static application order key made of identity texts plus arguments and commitment (1427-1465). `declared_callable_identity` builds a string `format!("specialized-machine|callable={}:{}|application={commitment}")` from the 32-byte commitment as hex (947-962) and compares it to the registry text at 1202.
- Closed reach: keys on `MachineId`, `ServiceId`, `OperationId`, `InvocationSite` via `BTreeMap` built per call of `validate_closed_reach_applications` (1495-1507, 1537-1553).
- Runtime evidence: callee callable identity string from `CheckedMachineDeclaration::callable_evidence_identity(callable, commitment)` compared with `module.identities.text(invocation.target_machine_identity)` (2183-2193).
- `validate_definition` parses a binary blob with domain `omega.requirement-schema.v3\0` (995) and compares lengths/kind bytes against `schema.parameter_kinds`.

### Cases by shape
- Call composition table: six rows only (`Call`, `CallUnit`, `CallStructuralScalar`, `CallStructural`, `CallStructuralWithScalarArguments`, `BoundaryCall`) at 715-722. `lib.rs:350` (grepped) tags `CallDynamic` as cohort `CallComposition` (`"algebra:call:call-dynamic"`) but there is no `CallDynamic` row in this table. I did not read the cohort macro in `semantic_rows`, so what `call_composition_semantic_row(CallDynamic)` returns is "inferred: no row".
- Schema kinds: `Nongeneric` (no lifetime params, no kinds, no tuples), `Open`, `Finite` with `finite_tuples` where all kinds are `2|3` and `lifetime_parameter_count == 0` (1030-1049). Binder categories: 1..=6 (1011), multiplicity 1..=3 (1014), argument category mapping `Type=1, Const=2, Proposition=6, Machine=4` with `2` accepted where `3` is expected (1238-1247).
- Closed reach parameter kinds: telescope kind bytes `0=Type, 1=Const, 2=Proposition, 3=Machine` (1478-1483); requirement parameter kinds `<= 5` (1642). Codec `decode_kinds` also `<= 5` (semantic_module.rs 2324). Three numbering spaces for "kind" (1..6, 0..3, 0..5).
- `fixed_element_array_extent` (304-344): path must be all `Field` segments (315-319), no qualifications on either side.
- `bounded_byte_owner_capacity_in` (39-109): both types must be `BoundedOwned` and capacity must be equal (101).
- `fixed_byte_array_loan` (370-411): when the window is not the full backing, offered predicates are reduced to `AsciiOnly|NoNul` only (394-406), i.e. `ValidUtf8` and `NonEmpty` are dropped for subwindows.

### Same job elsewhere
- Unique-row lookups by `let mut rows = ...filter(); rows.next()?; rows.next().is_none()` are hand-written across `evidence_calls` (1888-1890, 1893-1898, 1901-1906, 1923-1925, 2474-2479, 2485-2495, 2514-2524) while `crate::exactly_one::exactly_one` exists (85 `exactly_one(` uses across the repo's `omega-rust`: structural.rs 20, descriptors.rs 16, storage.rs 7, review-evidence capture files ~25, etc.; none in calls.rs). In the four `sem/` files of this unit, `.next().is_some()/.is_none()` appears 38 times (calls.rs 20, content_transformations.rs 16, access.rs 1, bytes.rs 1), `exactly_one(` appears 2 in content_transformations.rs and 1 in access.rs.
- `validate_runtime_evidence_inputs_in` has a second caller, `optimization-unit/semantics/src/unit_validation.rs` (2 occurrences), which supplies its own `RuntimeEvidenceInputs`; `terminal_inputs::validate` (1953) builds that struct from a `TerminalModule`. This is a deliberate adapter (struct `RuntimeEvidenceBody` 1869 copies machine data) rather than a duplicate.
- Closed-reach telescope kinds are also checked in the codec (`reach_application_wire::decode_kinds` semantic_module.rs 2321) and here (`validate_telescope` 1471).

### Unused or one-caller code
- `call_composition_semantic_row`, `validate_call_composition_semantic_rows` (regex): 8 occurrences = calls.rs 6 + lib.rs 2. `CallCompositionSemanticRow|CallCompositionSchema` 19 occurrences = calls.rs 16 + lib.rs 3. No user outside `sem/` in `omega-rust`. (The ten `Call*Rule` enums are only used to define this table.) This table is documented in the header as "Semantic judgments for direct, indirect, and boundary call sites"; the schema fields (`target`, `result`, `arguments`, ..., `work: ConsumeOne`) are not read by any caller I grepped.
- `fixed_element_array_extent`: 7 occurrences: foundation_validation 2, unit_validation 1, `02_..._target-operations/unit.rs` 1.
- `boundary_buffer_capacity|shared_boundary_buffer_capacity`: 12 occurrences across 6 files (used).
- `stored_view_field_presentation`: 5 occurrences (foundation_validation 2, unit_validation 1).
- `static_application_order_key`: 3 occurrences: calls.rs 2 (def + internal use 1211) + `06_.../retention.rs` 1.
- `validate_empty_erased_invocation_inputs`: 7 occurrences (grepped): calls.rs 1 (def), lib.rs 1, interpreter `engine_execution.rs` 2, `00_terminal-psi-to-abstract-operations/src/entry_settlement.rs` 3.

### Stated limits
- No limit comments. Error strings: "closed reach finite union differs from its published service row" (1710) etc.

### Fail-open risks
- `validate_boundary_callable` (795-801): `let Some(checked) = &boundary.declaration else { return Ok(()) }`. A boundary with no checked declaration is accepted without checks; a boundary that has one and `role != Callable` is rejected (802-804). No comment; the logic is that nothing exists to validate. "traced" (role is checked only when a declaration exists; a `ContractEvidence` boundary with no declaration is the normal case).
- `validate_runtime_evidence_inputs_in` early return (2127-2137): when the module has no `Requires` evidence lane and no proof-output call with both a runtime call and evidence arguments, it returns `Ok(())` before the final check at 2340-2347 ("ordinary runtime evidence invocation has no authenticated call operation"). Input that reaches it: a `ProofOutputCall` with `runtime_call: Some(site)` that names no real operation, with empty `evidence_arguments`, in a module with no Requires lanes. The nonreturning/`InvokeNoNormal` sites are separately checked in `terminal_inputs::validate` 1954-1991, but ordinary sites are not. "open" (I did not read whether `validate_structural_foundation`/`module_wire` checks the site).
- Same function, 2154-2159: a runtime call with no matching `ProofOutputCall` is skipped when `expected.is_empty()`. "traced": no declared evidence input exists, so nothing is missing.
- Same function, 2160-2162: `invocation.static_requirement_dispatch.is_some() => continue` skips all input checks for static-dispatch invocations. "traced": `validate_closed_conformances` 1348-1406 requires a unique application, unique schema, unique static application and a runtime call to the dispatched callee for each dispatch.
- `terminal_inputs::validate` (1992-1998): returns `Ok` before building bodies if there are no lanes and no snapshot-consuming calls. Same condition as above.
- `closed_reach` (1561-1572): a machine with no `closed_reach_application` is accepted unless it has a checked specialization or an invocation marker. "stated" by the two returned errors.

---

## 8. sem/content_transformations.rs

### Purpose
Judgments for the `TransformContent` operation (content-conservation equations over owned places), for "nominal retirement" of claims, for boundary-supplied custody of content, for preserving domain memberships across stores, and for quotient (opaque representation) catalogs. The header (2-4) calls it "Shared operation semantics for content transformations, domain-membership preservation across mutations and transfers (`domain_preservation`), and quotient representation opacity (`quotients`)."

### Shape
- `mod ancestry` 18-1147: `arrivals` 26-171, `retirement` 172-798 (`ClaimLocation` 183, `advance_owned_frontier` 295-468, `apply_nominal_disposals` 470-573, `bind_successor_claims` 575-618, `entry_claim_is_retired` 620-797), `OriginTraversal` 802-828 (work budget 65,536 at 815; depth 128 at 637, 1081), `validate_frontier` 830, `validate_producers` 873, `authenticate_step` 919, `call_input_origins` 974, `local_roots` 1053.
- `mod boundary_correspondence` 1148-1647: `validate_relation` 1161, `validate_authority` 1258, `validate_backing` 1401, `owned_addresses` 1462.
- Top level: `structural_path` 1649, `content_path` 1682, `claim_term` 1713, `place_type` 1735, `validate_shape` 1749, `projection_owner` 1819, `validate_parent_memberships` 1836, `validate_contract` 1896-2028, `validate_memberships` 2030.
- `mod domain_preservation` 2085-2745: `term_dependencies` 2099, `predicate_dependencies` 2181, `domain_predicate_is_closed` 2300, `domain_predicate_observes_no_storage` 2308, `domain_write_is_disjoint` 2395, `store_preserves_qualifications` 2471, `store_preserves_domain_content` 2497, `scalar_field_store_preserves_domain_content` 2521, `reference_preserves_ancestor_qualifications` 2540, `scalar_field_store_preserves_content` 2562, `store_preserves_content` 2581; one test 2656.
- `mod quotients` 2747-3704: `opacity` 2754-2974, `operation` 2975-3207, `origins` 3208-3338, `representations` 3339-3558, `quotient_place_type` 3565, `quotient_copy_adaptation` 3575, `validate_quotient_catalog` 3651.

### Inputs and outputs
Reads `TerminalModule`/`TerminalMachine` tables (`structural_types`, `structural_domains`, `identities`, `provider_candidates`, `boundary_machines`, `quotients`) and `crate::control_graph::dominators`, `crate::descriptors::descriptor_results::*`. Output `Result<(), &'static str>`/`bool`/`Option`. The traversal structs mutate only local `Vec`s.

### How it decides
- Content places are named by IDENTITY TEXT: `ContentPlaceSegment::Field(String)`/`Case(String)` compared and converted through `identities.index_of(text)` / `identities.text(identity)` (1649-1711). So content equations compare field/case spellings through the identity table.
- Equations are checked structurally: rebuilds `ContentConservation::new(algebra, claim_term(inputs), claim_term(outputs))` and compares to `derived.conservation` (1947-1957).
- Qualification owner = `(domain.id, domain.semantic_domain, projection)` must be identical across a derivation's bindings when custody is `Derived` (1984-1987).
- Disjointness: `structural_paths_may_overlap(dependency, write)` (2419, 2467, 2488, 2514, 2555, 2613).
- Frontier arithmetic is keyed on `ClaimId` and `(MachineId, ClaimId)` with `Vec` memo tables searched linearly (629-635, 1062-1068).

### Cases by shape
- `owned_addresses` / `visit` (1518-1634): closed owned storage = `AtomicCell`, `PrimitiveScalar`, `FixedArray`, `Record`, `Sum`, `Mixed`; `Reference`, `ByteSequence`, `ElementView` refused (1623-1627); fields of kind byte-sequence/predicate byte sequence refused (1497-1500, "borrowed byte descriptor") and `Atomic` refused (1501-1503). Only `address` integer leaves count as addresses (1505, 1538).
- `apply_nominal_disposals` (470-573): requires a `Record` subject with no `Structural`/`ByteSequence`/`PredicateByteSequence` non-erased fields (527-549).
- `term_dependencies` (2099-2179): `_ => return None` for every `ScalarTerm` not in {Boolean, Integer, float literal, Boolean/Integer/float fields, ByteSequencePredicate, StructuralRead with static steps, unary/binary macros} (2110-2167).
- `predicate_dependencies::visit`: `_ => None` for propositions outside truth/falsehood/integer-math/equal/less/float comparisons/conjunction/disjunction/implication/case-membership (2284).
- Quotient transport whitelists: result of quotient type must come from `StructuralLeafCopy | MoveStructuralValue | StructuralCaseLeafCopy | CallStructural | CallStructuralWithScalarArguments` (2868-2875); projections observing a quotient allowed only for 9 listed operation kinds (2938-2949).
- `validate_quotient_representations` (3345): each declared quotient has exactly one formation row; reflexive/duplicate rows rejected; shapes equal via `same_carrier_shape` (3509-3557).

### Same job elsewhere
- `content_path` (1682) here converts `StructuralPathSegment` to `ContentPlaceSegment` over resolved types; `05_typed-trees-to-checked-trees/src/checks_content.rs:3916` defines a different `content_path` over checker `PlaceSegment` (read lines 3916-3943). Different input type, same name and purpose; this file's has 1 outside caller (`06_.../unit_structural_values.rs:2363`).
- `structural_path` here (1649); other `structural_path` functions exist at `psi/compilation/validation/src/reference_result_custody.rs:6752`, `omega/packages/review-evidence/.../structural_expressions.rs:103`, `.../callable_policy/validation/structural.rs:147` (grepped, not read; suspected unrelated by type).
- Path-segment type ↔ id helpers `place_type` (1735) and `quotient_place_type` (3565) have identical bodies (parameter type, else producing operation type): same job twice in one file.
- `validate_producers` (873) and `local_roots` (1053) both walk transform producers; `retirement::entry_claim_is_retired` (620) traverses the callee CFG with its own frontier merge logic; `bytes.rs byte_copied_field_predicates_at_transfer` does a similar "no other consumer" scan (bytes.rs 550-594).
- Hand-written unique-match idiom 16 times (see calls.rs note).

### Unused or one-caller code
- `claim_term` (1713): 6 occurrences = this file 3 + `unit_structural_values.rs` 3.
- `content_path` (1682): 1 caller outside (`unit_structural_values.rs:2363`).
- `validate_parent_memberships` (1836): 3 occurrences: def + `engine_execution.rs` (interpreter) 1 + `unit_validation.rs` 1. `lib.rs` has no mention, so it must be reached through `pub use content_transformations::*` or a path (inferred; I did not read lib.rs).
- `domain_predicate_observes_no_storage` (2308): 5 occurrences: 2 here (def + test 2730), lib.rs 1, `provider_and_target_installation.rs` 2.
- `reference_preserves_ancestor_qualifications` (2540): 8 occurrences (users: descriptors.rs 1, foundation_validation 2, unit_validation 3, lib.rs 1).
- `scalar_field_store_preserves_domain_content` (2521): 7 occurrences, used in interpreter `engine_structural.rs` and `lowering_storage.rs`.
- Domain preservation functions as a group (`domain_write_is_disjoint|store_preserves_*`): 33 occurrences across 9 files.
- `scalar_field_store_preserves_content` (2562): not individually grepped.

### Stated limits
- 441-443: "opaque or dynamic custody transfer is not an authenticated nominal retirement".
- 451-452: "borrowed-storage custody exit needs its own exact restoration evidence, not a nominal-disposal inference".
- 493-495: "nominal retirement needs the completed descendant partition, not a retaining hook".
- 639-641: "nominal retirement call graph is circular or exceeds its receiving depth" (128).
- 825: "content ancestry receiving work budget exhausted" (65,536 visits).
- 1555-1557: backing addresses "require a closed array extent".

### Fail-open risks
- `validate_relation`, `validate_authority`, `validate_backing` all open with `let ... Boundary {..} = &derived.custody else { return Ok(()) }` (1164-1170, 1263-1265, 1408-1410). Derived custody has no boundary law to check, so skipping is the intended path. "traced": the `Derived` arm is checked by `validate_contract`/`validate_memberships` instead.
- `validate_memberships` (2042-2044): boundary-custody derivations are skipped (`continue`), so boundary outputs are not added to expected result memberships; and `validate_contract` only enforces "output membership is present in the result" when `ordinary` (2004-2013). For boundary custody the result's `qualifications` must therefore equal the inherited set exactly (2079). "traced" for the equality; whether that is intended is not commented.
- `validate_quotient_opacity` (2858-2860): `if module.quotients.representations.is_empty() { return Ok(()) }`. Module with a type carrying `quotient_carrier: Some(_)` but no representation rows would skip opacity. "traced": `validate_quotient_catalog` runs `validate_quotient_representations` first (3652), which errors if a quotient declaration has no unique formation row (3350-3358), and calls opacity second (3666).
- `domain_content_write_is_disjoint` (2452-2454): `domain.content_projection.is_none() => true`. A domain with no content projection observes no content, so a write is disjoint from it. "traced" by name; no comment.
- `reference_preserves_ancestor_qualifications` (2549-2551): shared borrow or empty path returns `true` (a shared borrow cannot change qualifications; an empty path has no ancestors). "traced".
- `store_preserves_content` (2590-2641): claims with empty `projections` return `false`; unrelated claims (root != place) are filtered out (2591). None seen fail-open.
- `quotient_copy_adaptation`/`validate_quotient_catalog`: unknown quotient adaptation reference returns error (3392-3401). None seen fail-open.

---

## Cross-file checks

- AGENTS.md "no new JavaScript or Python" and "dense IDs" were not evaluated beyond the `HashMap|BTreeMap|BTreeSet` counts I grepped: term_wire.rs 5 (HashMap in `mathematical_certificate_wire` 432, 576-577), semantic_module.rs 2 (`BTreeSet` in the affine discard check at 785 and import 23), access.rs 19, calls.rs 6 (`BTreeMap`/`BTreeSet` built per call inside `validate_closed_reach_applications` 1492-1553).
- Error-message labels in `reach_application_wire` are shifted between encode and decode: encode uses "reach type argument" for `Type` (2340), "reach const argument" for `Const` (2344), "reach proposition argument" for `Proposition` (2348), "reach requirement" for the machine requirement (2354); decode uses "reach const argument" for `Type` (2433), "reach proposition argument" for `Const` (2436), "reach requirement" for `Proposition` (2439), "reach selected identity" for the nominal requirement (2443). Labels appear only in `StringTooLong`/`InvalidUtf8` errors.
- `calls.rs` closed-conformance kind numbering (1..=6, with 1 type, 2 const, 3 const-finite, 4 machine, 6 proposition) vs closed-reach kinds in the codec and `validate_telescope` (0 type, 1 const, 2 proposition, 3 machine): different numbering for the same four binder categories in two places.

---

## Coverage

- Read in full: all 8 files, lines 1..N each: semantic_module.rs 4902, structural_wire.rs 2259, term_wire.rs 2957, wire_fields.rs 1227, access.rs 2526, bytes.rs 1143, calls.rs 2951, content_transformations.rs 3704. Total 21,669 lines.
- Skimmed: none (but test-only modules in semantic_module.rs lines 1194-1246, 1559-1815, 2513-3237, 3238-3921, 3932-4038, 4193-4902 were read for what invariants they pin, not traced for correctness).
- Not read: none.

## Five things a maintainer most needs to know about this unit

1. The codec files are fragments of one logical module: `semantic_module.rs` holds the top-level entry points and `validate_canonical_order`; `structural_wire.rs`, `term_wire.rs`, `wire_fields.rs` each re-import `semantic_module::*` and contribute `pub(crate) mod` blocks addressed as `crate::sections::semantic_module::*` (semantic_module.rs:1839, structural_wire.rs:408-411). `decode_module` (semantic_module.rs:4093-4120) is validator-then-re-encode: bytes must round-trip identically (4115-4118), and `encode_raw` (the unvalidated encoder) is what tests use to forge bad inputs.
2. Canonical order is enforced by `validate_canonical_order` (semantic_module.rs:53-865) as a flat list of `require_increasing` calls on ids, tuples and, in four places, identity text via `identities.text(..).unwrap_or_default()` (323, 433, 522-531). Any new table in `TerminalModule` needs a line there or it is only order-checked by the encode/re-encode comparison.
3. The wire formats use hand-numbered tags with several parallel tag spaces for the same concept: `StructuralPathSegment` (Referent=3) vs `CanonicalStructuralPathSegment` (Referent=4); IEEE comparison kind (1..6, structural_wire.rs 35-47) vs `IeeeFloatComparisonOperation` (0..5, wire_fields.rs 543-545); closed-reach kinds 0..5 (codec) vs 0..3 (calls.rs 1478) vs 1..6 (calls.rs 1011). The format marker is 253; `PSICORE\0` certificates have their own marker 3.
4. `sem/` files are validators returning `&'static str` errors that read module tables by linear `iter().find(...)` and hand-written "exactly one match" loops (38 occurrences in these four files). Several `pub` entry points have no caller outside `sem/lib.rs`: `byte_sequence_concatenation_length_equation` (bytes.rs 99), `access_ram_correspondence` (access.rs 1122), `call_composition_semantic_row` and the whole `CallComposition*` table (calls.rs 658-754; `CallDynamic` is cohort-tagged at lib.rs:350 but has no row), and `encode/decode_provider_candidate_record` in the codec (semantic_module.rs 2227/2237).
5. Two validators accept early before cross-checking: `validate_runtime_evidence_inputs_in` (calls.rs 2127-2137) returns `Ok` when there are no Requires lanes and no snapshot-consuming calls, skipping the check that each ordinary `runtime_call` names a real operation (2340-2347); and `decode_proof_term` returns `Formal`/`InvocationFormal` terms without `validate()` (term_wire.rs 1543-1548). Whether other validators close those gaps was not read.

## What you are unsure about

- How `foundation_validation.rs`, `identity_formations`, `module_wire.rs`, `block_wire.rs` (not in this unit) close the "open" items above (dangling identity indices ordered as `""`, `TargetLayout` id range, `scalar_restriction` kind, runtime-call site existence).
- `sem/lib.rs` wiring: I read only lines 336-354 and grep hits; whether `CallDynamic` is covered by the call-composition cohort (via `operation_semantic_cohort!` in `semantic_rows`) is not verified.
- Occurrence counts include re-exports and tests; "no outside call site" claims for `byte_sequence_concatenation_length_equation`, `access_ram_correspondence`, `call_composition_semantic_row`, and `{en,de}code_provider_candidate_record` are within `omega-rust` only (grep scope), not the whole repository.
- Whether the `HashMap`/`BTreeSet` uses conflict with `tools/dense_id_maps.txt` (not opened).
- Whether any of the `unreachable!` sites (term_wire.rs 371 and 2897-2899 `unreachable!` in encoders; structural_wire.rs 1580 `_ => unreachable!()` in `encode_capacity_scalar`) are reachable from decoded input: encoders are driven by already-typed values, so I believe not, but I did not trace `IntegerType::address`.
