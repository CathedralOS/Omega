# Record: unit_040 (area 08-psi-program)

Checkout `4f3c255d05599c11b1ea8ac668635f15f2a6eb86`. Paths are relative to `source-rust/subsystems/compiler/psi/` (written `psi/`) unless noted. Method: every non-test range was read with the Read tool. Test-only ranges were not read; I listed their function names with Grep and call them "names only" below. "Read" = read in full, "grepped" = Grep result, "inferred" = reasoned from read code.

Grep counts below are Grep count/files-with-matches runs over `source-rust/` (files that contain the name, not call sites; the number of files is stated).

---

## artifact/wire/src/sections.rs (1-829, read)

1. Purpose. Three small codecs that sit beside the semantic module: the artifact manifest (identity of semantic + optimization + installation + debug + publication sections), the debug map (`PSIDBG\0\0`), and the optimization execution record (`PSIOEXE\0`).
2. Shape. `mod artifact_manifest` (`sections.rs:9`): `TerminalArtifactManifest` (`:65`, fields semantic, optimization, installation, debug, publication, identity), `with_publication` (`:96`), `build_artifact_manifest` (`:121`), `manifest_for_semantic` (`:132`), `validate_artifact_manifest` (`:156`), `validate_artifact_manifest_with_publication` (`:173`). `mod debug_map` (`:257`): `encode_debug_map` (`:287`), `decode_debug_map` (`:295`), `validate_debug_map` (`:362`), `subject_exists` (`:496`), `source_digest` (`:276`). `mod optimization_execution` (`:590`): `PsiOptimizationExecutionRecord` (`:625`, fields selections, input_semantic, output_semantic, identity), `new` (`:633`), `validate_output` (`:674`), `build_identity_optimization_execution_record` (`:685`), encode (`:698`) / decode (`:716`). Formats: manifest marker 4 (`:17`), debug marker 1 (`:273`), optimization record marker 2 (`:603`).
3. Inputs/outputs. Manifest takes a `TerminalModule` (identity via `terminal_psi_identity`), an optimization record, optional raw installation and debug bytes; produces hashes (domain-separated, `hash` at `:227`). Debug map decode takes the module and bytes and returns `TerminalDebugMap`.
4. How it decides. Manifest validation recomputes the manifest and compares equality (`:187-189`), keyed on hashes of section bytes. Debug map validation: semantic identity equality (`:366-372`), strictly increasing file ids and subjects (`:373-380`), each site's file id must exist and span must fit `byte_len` (`:382-391`), and `subject_exists` looks the subject up by id in the module (`:496-554`). Optimization record: if selections are empty the input and output identities must be equal (`:638`); `validate_output` compares `output_semantic` to the module identity (`:678`).
5. Cases by shape. `subject_exists` handles nine `DebugSubject` variants (`:496-553`). `DebugSubject::Claim` is found only in `content_entry_claims`, `content_identity_reshuffles` or `content_partition_compositions.input_claims` (`:537-551`).
6. Same job elsewhere. Canonical re-encode-and-compare after decode is done here at `:356` and `:741`, and in `semantic_module.rs:4298-4302`. Hex `Display` for 32-byte values repeated three times (`:39`, `:60`, `:617`).
7. Unused/one-caller. `manifest_for_semantic`, `build_identity_optimization_execution_record`: grep over `source-rust/` of the four names `encode_invocation_prefix|module_decode_count|manifest_for_semantic|build_identity_optimization_execution_record` found 46 occurrences in 26 files; per-name split not measured.
8. Stated limits. None.
9. Fail-open risks. `validate_debug_map` for `Value` subjects accepts a value id found as a machine scalar result, a machine parameter, a block parameter or an operation scalar result (`:504-519`); structural-result value ids are not searched (inferred from `.scalar()` use), so those are refused (fail-closed). None seen that accept a wrong program.

---

## artifact/wire/src/semantic_module.rs (1-5446; read 1-2900, 4185-4359, 5297-5341; names only 2901-4184, 4360-5296, 5342-5446)

1. Purpose. Entry points of the `PSITERM\0` codec: encode, decode, fingerprint, identity, canonical-order validation, dynamic-descriptor admission, and a set of nested wire modules for machines, provider candidates and external providers.
2. Shape. Constants `MAGIC` (`:4201`), `FORMAT_MARKER = 290` (`:4205`), `FINGERPRINT_DOMAIN` (`:4207`). `encode_module` (`:4209`), `encode_closed_execution_module` (`:4222`), `encode_invocation_prefix` (`:4233`), `decode_module` (`:4260`), `decode_closed_parts` (`:4264`), `semantic_fingerprint` (`:4316`), `encoded_fingerprint` (`:4323`), `terminal_psi_identity` (`:4339`). Nested: `canonical_order::validate_canonical_order` (`:56`, ~810 lines), `dynamic_result_admission::validate_dynamic_descriptor_results` (`:1084`), `external_provider_wire` encode/decode (`:1167`, `:1217`), `machine_wire` (`:1294`; wire_record `Machine` at `:1605`; ranked-SCC encode/decode `:1720`, `:1763`), `provider_candidate_wire` (`:1881`).
3. Inputs/outputs. Encode: `TerminalModule` -> bytes plus `ClosedTypeCatalog`. Decode: bytes -> module, canonical bytes, catalog. Decode sequence (`:4268-4303`): magic, format marker, `decode_module_body`, trailing-byte check, scalar qualification validation, `validate_canonical_order`, `validate_storage_foundation`, `take_opaque_storage_shapes`, `validate_structural_foundation`, `terminal_semantics::structural_qualifications::validate` (`:4289`), `validate_identity_formations`, `validate_dynamic_descriptor_results`, restore opaque shapes, re-encode and compare bytes (`:4298-4302`).
4. How it decides. Format marker is an exact `u16` compare (`:4273`). `encode_module` refuses any module with an invocation scope (`:4210`). Canonical order is checked by identity (ids, tuples of ids) with `require_increasing` / `strictly_increasing`; a few checks key on spelling: provider candidates and conformance applications sort by `identities.text(...)` strings (`:303-318`, `:532-552`); callback applications compare `identities.text(boundary.identity)` to `application.requirement` string (`:77-83`). `encoded_fingerprint` hashes bytes, and for invocation-code modules adds invocation-local identity indexes (`:4323-4337`).
5. Cases by shape. `validate_canonical_order` is a long fixed list of tables (task activation demands, boundary machines, provider candidates, evidence terms, machines...). `structural_wire`-style match on `StructuralTypeShape` at `:343-349` handles Record/Sum/Mixed and treats all other shapes with `_ => (&[][..], &[][..])`. Ranked-SCC wire has one tag, `3 => Natural` (`:1768`); `0` is none, other tags refused (`:1827`).
6. Same job elsewhere. Order checks are also done in `psi_declaration_encoding` helpers it calls (`crash_routes_are_canonical`, `canonical_propositions_strictly_increase`). Fingerprint-domain hashing is repeated in `sections.rs:227`. Format-marker + vocabulary marker prefix check is repeated at `provider_candidate_wire::decode_provider_candidate_record` (`:1968-1985`).
7. Unused/one-caller. `module_decode_count` and `encode_invocation_prefix`: see sections.rs item 7 (combined grep). `encode_structural_type_declaration` (`:4245`), `decode_canonical_proposition` (`:4310`): not grepped.
8. Stated limits. Comment `:4203-4204`: "Marker 290 retires the external sealed byte-read intrinsic ... Earlier artifacts are refused." Comment `:337-339`: authored order of `requires` clauses is identity, so only each clause's form is canonical. Comment `:870-871`: "Canonical ordering must not depend on whether code exists."
9. Fail-open risks.
 - `decode_closed_parts` does not check the vocabulary marker itself; the `format_marker_tests` at `:5298-5335` show markers are refused "before payload interpretation", so `decode_module_body` does that (traced by test, body not read). Classification: open for the exact location.
 - `canonical_order` tuple keys using `.unwrap_or_default()` of missing identity text (`:312`, `:445`, `:538-547`): a candidate whose identity text is missing sorts as the empty string, so two such rows would compare by the remaining tuple fields. Classification: open (does not accept a wrong program by itself; later validators run).
 - `validate_dynamic_descriptor_results` skips table slots whose `row` is `None` or whose result is not `Structural` (`:1096-1101`), and only analyzes rows that have a `View` leaf (`:1109-1114`). Skipped rows get no descriptor-region validation here. Classification: stated by structure (only view leaves need region proof); not traced further.
 - Tests (`:2000-5446`, names only): not read, so no claim about what they assert beyond their names.

---

## artifact/wire/src/service_schema_wire.rs (1-408, read)

1. Purpose. Wire records for `ServiceSurface`, `TerminalServiceSchema`, `TerminalProgramEntryContract` and `TerminalProviderSelection`, built with `wire_record!` / `wire_enum!`.
2. Shape. Field adapters `ProviderIdentity` (`:36`), `Count` (`:47`), `Identities` (`:58`); `wire_enum!` list `:80-122`; `wire_record!` `Method` (`:156`), `Surface` (`:177`), `Schema` (`:197`), `EntryContract` (`:202`), `ProviderSelection` (`:209`).
3. In/out. Reads/writes through `Writer`/`Reader`; `Identities::read` interns each text and rejects a repeat (`:67-77`).
4. How it decides. Tag-to-variant tables. `Hosted` refuses `ConsoleReadByte` both ways (`:92-93`).
5. Cases by shape. Closed tag lists: `Hosted` has tags 2 and 4 only (`:90-91`), `FixedImage` 1-2, `AuthorityFlow` 1 only (`:103-105`), `ProgressSubject` 1 only (`:119-121`).
6. Same job elsewhere. `PredicatePresence`, `Composition` style enum tables are the same mechanism as `wire_fields.rs:457-478`.
7. Unused. None checked.
8. Stated limits. Test name `obsolete_two_case_read_intrinsic_is_refused_in_both_directions` (`:374`). A `HostedOperationWire` in `structural_wire.rs:854-875` writes only `ProcessExit` as tag 0 and a different tag numbering from `Hosted` here (`ConsoleWriteByte=2`, `ProcessExit=4`); two encodings of `HostedOperation` exist (read both).
9. Fail-open risks. None seen.

---

## artifact/wire/src/spec_tag_tables.rs (1-642, read)

1. Purpose. Test-only module (`#[test]` at `:610`, `:620`) that reads marked tables out of `wiki/spec/terminal-psi/encoding.md` and feeds all 256 `u8` values to each decoder, to catch drift between spec and codec.
2. Shape. `TagTable` (`:52`), `drift` (`:79`), `marked_tables` (`:117`), `listed_tags` (`:156`, `pub(crate)`), `tag_tables()` (`:171`, ~90 tables), `UNCHECKED_TABLES` (`:26`, one entry: `admission-kind-tags`).
3. In/out. Includes the spec page via `include_str!` (`:20`). Output: assertion failure listing drift.
4. How it decides. A tag is "rejected" if the probe returns `InvalidTag(space, tag)` with `space` in the table's allowed spaces and the same tag; compared to the spec's listed set (`:84-86`). Keyed on the string name of the error space.
5. Cases by shape. Each table needs hand-built prefix bytes (for example `structural_type_head` `:194`, `rank_edge_head` `:198`).
6. Same job elsewhere. None.
7. Unused. `listed_tags` is `pub(crate)`; users inside the crate not grepped.
8. Stated limits. `UNCHECKED_TABLES` reason: "the codec has no byte for them" (`:28`).
9. Fail-open risks. A decoder error space named differently from the table's `spaces` list is treated as "accepted" (`:85`); a decoder that rejects with another error before reaching the tag reads as accepting (`drift` `(false,false)` then reports). The test depends on the probe prefixes being valid. Classification: open (test infrastructure only).

---

## artifact/wire/src/structural_wire.rs (1-1896; read 1-385, 560-1555; names only 386-559, 1556-1896)

1. Purpose. Wire codecs for structural fields, places/paths/cleanup actions, operation results, boundary machine signatures, and structural type declarations.
2. Shape. `mod structural_field_wire` (`:8`): `encode/decode_byte_sequence_carrier` (`:26`, `:54`), `encode/decode_structural_field` (`:76`, `:149`), `encode/decode_structural_field_type` (`:91`, `:173`). `mod structural_place_wire` (`:267`): arguments, `encode/decode_affine_cleanup_action` (`:292`, `:335`). `mod structural_result_wire` (`:568`): `validate_reference_sources` (`:581`), records `OperationResult`, `QualificationChange`. `mod structural_signature_wire` (`:686`): `BoundaryMachine` record (`:771`), `Execution` enum tags 0-12 (`:878`), `Completion` (`:977`). `mod structural_type_wire` (`:1084`): `encode_structural_type` (`:1109`), `decode_structural_type` (`:1284`), authored field storage (`:1453`, `:1497`).
3. In/out. Reader/Writer to and from `terminal_psi` declaration types.
4. How it decides. Tag bytes. `RECORD_WITH_REQUIRES = 17` and `SUM_WITH_REQUIRES = 18` (`:1106-1107`) wrap the shape tag when `requires` is non-empty; decode refuses a "requires" tag with zero clauses (`:1425-1431`) to keep one encoding per declaration. Reference-source validation keys on type ids and exact access (`:621-628`).
5. Cases by shape. `encode_structural_type` refuses `TypeParameter` ("open type parameter cannot be serialized", `:1130-1134`), array extent `Parameter` (`:1220-1224`), `Opaque` with a carrier/restriction/requires (`:1135-1143`), `requires` on anything but Record/Sum (`:1268-1275`). `AtomicCell` must be `is_supported()` (`:1163`). Storage tags for records: 1 Packed, 10 Authored, 13 TargetLayout, 14 AccessView, 15 Accessor (`:1187-1193`). `HostedOperationWire` writes only `ProcessExit` and errors on any other value (`:861-864`).
6. Same job elsewhere. Authored-field-storage decode and the record decode for tags 10/14/15 repeat the same field/storage loop (`:1343-1378`).
7. Unused. Not grepped.
8. Stated limits. `:312-313`: "Borrowed-cleanup tags 3 and 5 remain retired." `:1103-1105` record invariant comment.
9. Fail-open risks.
 - `decode_structural_type` accepts the Record tag `17` followed by storage tag set `{1,10,13,14,15}` (`:1314`). Tag 12 (`PrimitiveScalar` with restriction) is accepted whenever the restriction decodes (`:1338-1342`); no check here that the restriction matches the carrier. Classification: open (later `validate_structural_foundation` not read).
 - `decode_byte_sequence_carrier` accepts `access: None` (`0`) for borrowed views (`:59-68`). Classification: open; consumers not read.
 - Tests (`:225-264` read; `:381-559`, `:1548-1896` names only): not read.

---

## artifact/wire/src/term_wire.rs (1-1651, read)

1. Purpose. Wire codecs for the mathematical certificate (`PSICORE\0`), proof terms, and scalar qualification catalogs; re-exports other term codecs from `psi_declaration_encoding` / `psi_callable_encoding`.
2. Shape. `mod contract_wire` (`:11`, `Successor` record), `mod mathematical_certificate_wire` (`:39`): `encode_mathematical_certificate` (`:58`), `decode_mathematical_certificate` (`:116`), `MathematicalTermWriter` (`:173`) which dedups terms by their serialized bytes (`by_bytes` `:177`, `:500-507`), `decode_term` (`:549`, tags 1-28), `decode_level` (`:727`). `mod proof_term_wire` (`:818`): tags 1-9 (`:827-835`), `ProofTermTable`. `mod scalar_qualification_wire` (`:1262`): `encode` (`:1283`), `decode` (`:1381`), `validate` (`:1482`).
3. In/out. Certificate bytes <-> `TermArena`/`MathematicalCertificate`. Proof terms <-> `ProofTerm` tables with child handles.
4. How it decides. Certificate decode: a child index must be `< handles.len()` ("does not precede its parent", `:557-561`); root indexes must be in the table (`:770`). Encoder refuses `Term::Dummy` (`:258`, `:280`). `decode_canonical` re-encodes and compares (`:119-170`). Proof-term Mathematical rows decode the nested certificate (`:1000-1002`) to refuse malformed certificates, but the certificate is not otherwise judged here. Mixed vs plain application tag is chosen by whether any argument is `Scalar` (`:898-905`). `validate` for qualification catalogs checks ordering, nonempty origins, no duplicate member sets, `ordered()` integer ranges, equal float formats (`:1482-1577`).
5. Cases by shape. `Term::children` lists 22 variants and `_ => Vec::new()` for leaves (`:193-240`); a new `Term` variant would silently have no children here (inferred; `emit` has no wildcard so it would fail to compile).
6. Same job elsewhere. Explicit-stack level encode/decode (`:523`, `:727`) vs term worklist (`:245`): same technique, two sites.
7. Unused. `DecodedMathematicalCertificate` is `pub`; not grepped.
8. Stated limits. Comment `:725-726`: nesting "bounded only by the bytes present". Test `deep_levels_round_trip_on_the_default_stack` uses 131_072 levels (`:794`).
9. Fail-open risks. None seen: unknown tags refuse (`:713`, `:721`, `:746`, `:1112`).

---

## artifact/wire/src/wire_fields.rs (1-547, read)

1. Purpose. Field adapters (the `Field<T>` impls) for the complete artifact, layered on shared fields from `psi_declaration_encoding::fields`.
2. Shape. `Argument` (`:30`), `Integer`, `Float`, `byte_enums!` macro (`:70`), `Resident`, `Region` (`:124`), `ByteOperand` (`:167`), `ScalarCaseFields`, `RecordFields` (`:227`), `argument_claims!` (`:272`: `ClaimTransfers`, `CompletionReceipts`), `SelectedEvidenceRow` records (`:302-339`), `optional_packages!` (`:384`, four zero-package refusal messages), `Custody` (`:418`, tags 1-5), `GlobalDescriptorTable`, `SegmentReloadKind`, `SegmentSelectorRegister` (`:457-477`).
3. In/out. Reader/Writer plus semantic types.
4. How it decides. Tag tables. `optional_packages` refuses a zero package digest via `from_digest` returning `None` (`:404-406`).
5. Cases by shape. `Predicate` byte enum accepts only 1, 2, 4, 8 (`:92`).
6. Same job elsewhere. `Custody` `Package` branch reuses `ProviderPackage` (`:433`).
7. Unused. Not grepped.
8. Stated limits. None.
9. Fail-open risks. None seen.

---

## execution-inputs/entry-memory/entry_memory.rs (1-331, read)

1. Purpose. Checks a receiver-supplied RAM recipe against an exact borrowed entry row and admitted target layout, returning `CheckedPrivateEntryRamRecipe`.
2. Shape. `PrivateEntryRamInitialization` (`:11`), `PrivateEntryRamRecipe` (`:17`), `CheckedPrivateEntryRamRecipe` (`:23`, private fields), `PrivateEntryRamScalarInitialization` (`:36`), `admit_private_entry_ram` (`:105`), `private_entry_ram_identity_pair` (`:87`), `decode_private_entry_observations` (`:301`).
3. In/out. Reads `TerminalArtifactView`, `AdmittedTargetLayouts`, entry `MachineId`, the recipe; calls `terminal_semantics::{access_view_input_parameter, access_ram_plan_correspondence, reference_storage_layout, reference_storage_projection, reference_storage_byte_width, scalar_field_accepts_literal}`.
4. How it decides. Row identity (`recipe.input` is in `module.access.access_view_inputs`, machine equals entry, access is Shared/Mutable borrow, `:112-121`). Parameter must have no qualifications and not be `Linear` (`:124-129`). Plan found by `carrier == parameter.structural_type` (`:134-138`). Size is `1..=65536` and must equal `recipe.contents.len()` (`:155`). Any `structural_domains` entry whose carrier is the schema type refuses (`:160-166`). Fields are decoded by byte offset from the layout projection, compared with the declared scalar (`:214`), canonical zero padding required (`:281-287`).
5. Cases by shape. Schema must be a `Record` (`:171-173`: "currently constructs plain scalar records"); each field must be `Scalar(Boolean | Integer)` not erased (`:183-193`); integer must be whole-byte and non-address (`:233-238`, `:242-243`), `width <= 16`. Observation header is the literal `OMEGARAM` plus fingerprint, placement commitment, positions, length (`:69-77`).
6. Same job elsewhere. Scalar-from-bytes decode with sign extension is a second implementation of storage-codec decoding; the interpreter likely has one (suspected, not read).
7. Unused. `PrivateEntryRamInitialization`: grepped, 14 occurrences in 8 files. `PrivateEntryRamRecipe`: 52 occurrences in 20 files. `decode_private_entry_observations|private_entry_ram_identity_pair`: 5 occurrences in 4 files.
8. Stated limits. `:172`: "currently constructs plain scalar records"; `:128`: "does not supply qualifications or linear custody"; invented size cap 65536 at `:155`.
9. Fail-open risks. `private_entry_ram_identity_pair` returns `None` on overflow only. None seen that accepts a wrong recipe.

---

## execution-inputs/providers/component.rs (1-149, read)

1. Purpose. Match a receiving boundary requirement in one artifact to a selected provider in a separately published artifact.
2. Shape. `AdmittedComponentProvider` (`:15`), `admit_component_provider` (`:68`), membership lookups `input_membership`/`result_membership` (`:46`, `:57`).
3. In/out. Receiver view, receiving boundary id, provider artifact view, `ProviderInstallationSelection`. Calls `admit_provider_installation` on the provider artifact (`:125`), then `memberships::admit_*`.
4. How it decides. Requirement identity is by identity text equality (non-empty and equal, `:89-92`) plus `owner.same_identity_as` (`:93-96`), then `same_identity_as` on contracts (`:104-108`) plus equal completion, termination, parameter order, scalar parameters (`:108-111`), plus `boundary_result_lifetimes_match` (`:115-121`).
5. Cases by shape. None beyond the above.
6. Same job elsewhere. The assumed-binding check `selections.rs:457-472` compares the same fields within one module.
7. Unused. Not grepped.
8. Stated limits. None.
9. Fail-open risks. None seen.

---

## execution-inputs/providers/component/memberships.rs (1-618, read; tests `:286-618`)

1. Purpose. Map structural-domain memberships between two modules' domain tables so a component import and export agree on qualification theories.
2. Shape. `admit_input_memberships` (`:10`), `admit_result_memberships` (`:37`), `admit_memberships` (`:70`), `same_domain` (`:111`), `required_domain` (`:174`), `same_domain_definition` (`:187`).
3. In/out. Two `TerminalModule`s and two boundary declarations in; `Vec<(source_domain, target_domain)>` per input out.
4. How it decides. Domains pair when their carrier, access policy (`terminal_semantics::structural_qualifications::same_access_policy`), predicate proposition, byte predicate, content projection, `complete_predicate_theory`, and establishment routes agree. Identity comparison uses `NominalDeclarationIdentity::same_identity_as` (identity tables compared by text across modules, `:155-159`, `:201-205`). Routes are compared as sorted `(kind, owner string, name string)` rows (`:247-284`) built from `identities.text(...)`, i.e. source-spelling keyed. A route with invocation-local identity or empty text makes `routes()` return `None`, so the domain is "not same" (fail-closed, `:261-271`).
5. Cases by shape. `admit_result_memberships` handles only `(Unit,Unit)`, `(Scalar,Scalar)` equal, `(Structural,Structural)` with no projected qualifications (`:43-66`); anything else is "component result kinds or scalar contracts differ". Predicates must have equal `proposition` and "observe no storage" (`:232-246`).
6. Same job elsewhere. Domain-definition comparison is a second copy of domain equality; `terminal_semantics::structural_qualifications::same_access_policy` is the shared part. Suspected others in `terminal-semantics` not read.
7. Unused. `Memberships` types are `pub(super)`.
8. Stated limits. Error text `:54`: "component result requires projected membership transfer".
9. Fail-open risks. `admit_memberships` skips a source domain with no matching target domain (`:87-93`: only pushes when a match exists). Then `mapped` is built only for `source_required` domains and must equal `target_required` (`:94-107`), so a domain that exists in the source but has no target match and is not required does not block admission. Classification: traced; bad input is an unrequired membership theory present only on the import side; it would pass.

---

## execution-inputs/providers/logical_reach.rs (1-184, read)

1. Purpose. Follow calls (including scalar evaluation roots and chosen provider bodies) to find which boundaries are reached, and close provider selection to a fixed point.
2. Shape. `boundary_call_reach` (`:7`, a wrapper that also adds scalar roots), `ClosedLogicalProviderReach` (`:66`), `close_providers` (`:92`).
3. In/out. `TerminalModule`, entry, implementations, target; uses `terminal_semantics::{synchronous_calls::SynchronousCallGraph, callable_values::boundary_call_reach, logical_providers::ClosedLogicalProviders}`.
4. How it decides. Fixed-point loop (`:104-183`): reach boundaries, select providers, collect `CallableBoundaryImplementation` from `Checked` realizations of non-`Independent` providers, compare to previous bindings. Competing bodies for one boundary refuse (`:160-166`). A previously selected body disappearing refuses (`:174-181`).
5. Cases by shape. `Independent` composition and non-`Checked` realizations are skipped (`:137-148`), with comments.
6. Same job elsewhere. Name `boundary_call_reach` exists here and in `terminal_semantics::callable_values` (called at `:48`); the local one wraps it.
7. Unused. `boundary_call_reach`, `close_providers`: not grepped.
8. Stated limits. `:135-136` comment: independent requirement closure belongs to the supplier artifact.
9. Fail-open risks. None seen. Termination of the loop relies on the monotone check at `:174` (inferred).

---

## execution-inputs/providers/selections.rs (1-590, read)

1. Purpose. Admit provider installation selections and assumed bindings against one artifact's provider candidates and boundaries.
2. Shape. `ProviderInstallationSelection` (`:16`: boundary, provider_identity text, provider_declaration text, candidate), `AssumedBindingSelection` (`:26`), `AdmittedProviderSelections` (`:41`), `admit_provider_selections` (`:292`), `admit_assumed_bindings` (`:403`), `operational_contract_fits` (`:389`), `same_structural_input_carriers` (`:370`), `admit_folded_constant_closure` (`:74`), `AssumedBindingRejection` (`:226`).
3. In/out. Module, execution identity, selections, assumptions in; `BTreeMap<BoundaryMachineId, AdmittedProviderSelection>` + `Vec<AdmittedAssumedBinding>` out.
4. How it decides. A selection matches a candidate by boundary, `identities.text(candidate.provider_identity) == selection.provider_identity`, same for declaration, and `candidate.candidate == selection.candidate` (`:322-330`): spelling-keyed identity text. Refuses empty text, non-callable boundary, repeats, or two matching candidates (`:332-337`). Assumed bindings: boundary must be a `Callable` role boundary, no checked supply for the same requirement (`:424`), no duplicate assumption, no untracked content/qualification authority (`:448-456`), equal runtime shape (`:457-472`), `operational_contract_fits` (`:389-401`: equal `kind`, `completion`, `crash_routes`; supplied termination at least required's; supplied ceiling subset of required ceiling). Supply chain is walked until a checked provider is found; cycle -> `CyclicSupply`, missing link -> `UnselectedSupply` (`:478-494`).
5. Cases by shape. Assumed bindings refuse any boundary with `program_local_root_introductions`, `content_guarantees` or qualified structural result (`:448-456`, variant `UnsupportedAssumedAuthority`). `same_structural_input_carriers` compares position, type, multiplicity, access only (`:370-381`), not qualifications.
6. Same job elsewhere. Runtime-shape comparison duplicated in `component.rs:104-114`.
7. Unused. `admit_provider_installation_from_artifact`, `admit_provider_execution_installation`, `AdmittedProviderSelections::is_subset_of`: not individually grepped.
8. Stated limits. Doc comment `:383-388` on why owner is not reach.
9. Fail-open risks. `terminal_semantics::boundary_result_lifetimes_match(...).unwrap_or(false)` (`:464-467`): an error makes the shape "incompatible" (closed). `admit_folded_constant_closure` accepts when no dependency has a `candidate` (`:106-108` `continue`) after validating the folded constants (`:86-103`). Classification: stated by structure; not traced.

---

## execution-inputs/scalar-applications/abi_storage.rs (1-58, read)

1. Purpose. Ties an ABI declaration to the scalar applications whose extents its storage needs, for one program and target.
2. Shape. `CallingAbiStorageDemand` (`:7`), `new` (`:15`), `require_artifact` (`:49`).
3. In/out. Uses `module.calling_abi_signature(id)` and `terminal_semantics::calling_abi_extent_dependencies`.
4. How it decides. The retained signature's `identity(...)` must equal the supplied `CallingAbiIdentity` (`:24`); artifact identity equality (`:53`).
5-8. None. Users: `CallingAbiStorageDemand` grepped, 18 occurrences in 7 files.
9. Fail-open risks. None seen.

---

## execution-inputs/scalar-applications/scalar_applications.rs (1-99, read)

1. Purpose. Module root and shared records for the scalar-application checks.
2. Shape. `ScalarConditionScope` (`:24`: Source, SourceRoots, Invocation, AbiStorage), `ApplicationObservations` (`:48`), `EntryResult` (`:54`), `CompletedScalarConditions` (`:64`), `ScalarApplicationObservations` (`:85`), `ScalarApplicationValues` (`:99`).
3-9. Data declarations; `#![forbid(unsafe_code)]` (`:2`). No decisions. `completion_tests` module is `#[cfg(test)]` (`:15`).

---

## execution-inputs/scalar-applications/completion.rs (1-542, read)

1. Purpose. Records a completed scalar scope and checks that every scope required by the artifact was completed with every retained condition discharged.
2. Shape. `ScalarScopeInputs` (`:16`), `ScalarEntryMembership` (`:27`: Source / Selected / None), `AdmittedScalarApplications` (`:72`), `CompletedScalarScope` (`:84`), `ScalarApplicationClosure` (`:168`) with `new` (`:188`), `for_invocation` (`:261`), `for_abi_storage` (`:282`), `with_deployment_scopes` (`:295`), `close_scope` (`:319`), `finish` (`:494`); `admits_machine` (`:50`); `restore_scope` (`:111`).
3. In/out. Takes `ScalarApplicationObservations`, `ScalarConditionSelection`, an address model result.
4. How it decides. `close_scope`: selection must match artifact and scope (`:330-349`); scope must be one of the requested targets and not completed twice (`:355-363`); any application that has a value but whose range/predicate/completion condition is not selected is refused as "debt" (`:365-406`); address model must match target, be an address carrier, version != 0 (`:412-418`). `finish`: every scope complete (`:495`); for `Source` kind, conditions whose subject is not an `Application` must have been selected (`:503-528`). Scope is keyed on target string equality (`:137-143`, `:359`).
5. Cases by shape. `product_formation` is false for Invocation and AbiStorage scopes (`:534-537`).
6. Same job elsewhere. Condition selection in `conditions.rs`.
7. Unused. `AdmittedScalarApplications`: via `scalar_applications.rs:14`; not grepped.
8. Stated limits. Comments `:73-76` (invocation code keeps its process-local scope).
9. Fail-open risks.
 - `admits_machine` returns `true` when entry membership is `Source`, and when the module has no conditions (`:56`, `:60-62`). Input reaching it: any module with no scalar conditions, any Source-scoped closure. Classification: open (comment absent).
 - `finish` only enforces retained non-application conditions for the `Source` kind (`:503`, `:514`, `:522`); Deployment, Invocation and AbiStorage kinds skip that check. Classification: traced by reading; stated reasons absent.

---

## execution-inputs/scalar-applications/completion_tests.rs (1-408, names only)

Test fixture and six tests (names: `abi_storage_request_requires_exact_canonical_roster`, `abi_storage_receipt_binds_full_key_and_grants_no_entry_or_product_authority`, `target_context_binds_artifact_without_selecting_an_entry_body`, `abi_storage_request_refuses_other_artifact_and_target`, `abi_storage_completion_refuses_actual_false_helper_debt`, `demand_retains_the_exact_declared_extent_roster_and_artifact`). Not read.

---

## execution-inputs/scalar-applications/conditions.rs (1-690, read; tests `:334-690`)

1. Purpose. Choose which retained scalar conditions (range, predicate, completion) apply to one target/scope, and discharge them as observations become ready.
2. Shape. `ScalarConditionSelection` (`:28`), `select` (`:42`), `include_completed` (`:117`), `include_evaluation_entry` (`:129`), `matches` (`:178`), `include_applications` (`:206`), `discharge_ready` (`:233`), `observations` (`:279`), `applications` (`:312`), `invocation_membership` (`:14`).
3. In/out. `ProofApplicationScope`, `ScalarConditions`, target, provider edges, demanded applications, scope kind.
4. How it decides. `applies(subject, address_model)` (`:72-81`): `Module` always applies; `Declaration(owner)` applies if owner is in the reached machine set, or (for address-model conditions) in the declarations list or (source formation and primary) in all declarations; `Application(a)` applies if demanded. Keyed on `ProofDeclarationId` and `ScalarApplicationId`, target string equality via the scope.
5. Cases by shape. Four scope kinds; `AbiStorage` returns empty membership (`:65`).
6. Same job elsewhere. `entry_checks.rs:94-119` has a second `applies` over subjects.
7. Unused. Not grepped.
8. Stated limits. None.
9. Fail-open risks. `ScalarConditionSubject::Module` conditions always apply (`:73`). `discharge_ready` skips `AddressModel` range requirements (`:263-265`). Classification: open for the skip; discharged later at `close_scope`.

---

## execution-inputs/scalar-applications/entry_checks.rs (1-459, read)

1. Purpose. Prepare and admit prerequisites that a scalar evaluation entry must satisfy from its own operand occurrences.
2. Shape. `ScalarEntryPrerequisites` (`:21`), `prepare` (`:49`), `admit` (`:214`), `ScalarEntryChecks` (`:328`), `EntryCheck::discharge_ready` (`:377`), `EntryCondition` (`:341`).
3. In/out. From `ScalarApplicationObservations` and an application id; produces `AdmittedScalarEntry`.
4. How it decides. Prerequisite rows are range/predicate conditions with role `Prerequisite`, subject Declaration/Application (predicates: any non-Module) (`:58-77`). Rows are retained when the declaration is in the invocation's dependencies (`:96-98`), or the application is this one (`:99`), or another `Evaluate` application whose entry's declaration is in the dependencies (`:100-106`). `admit` requires equal artifact, target, providers and entry (`:220-232`), `require_crash_free_evaluation` (`:241`), then evaluates each row; unmet integer bounds or false predicate refuse (`:430-433`, `:441-453`).
5. Cases by shape. Only integer operands (`:406`, "is not an integer"). `ScalarApplication::LayoutObservation` owners return `false` (not applicable) (`:107-109`); any other declaration kind refuses ("owner application is absent", `:110-114`).
6. Same job elsewhere. See conditions.rs item 6.
7. Unused. `ScalarEntryChecks`, `AdmittedScalarEntry`: not grepped.
8. Stated limits. None.
9. Fail-open risks. A prerequisite whose subject is a `LayoutObservation` application is dropped from the rows (`:107-109`): no comment says why. Input: a prerequisite on a layout-observation application. Classification: open. If `rows` is empty from the start, the root/invocation lookup is skipped (`:78`), so `prepare` succeeds without the entry having a checked declaration.

---

## execution-inputs/scalar-applications/observations.rs (1-793, read)

1. Purpose. Store scalar application values and entry-result captures, observe layout measurements, and discharge range/predicate conditions against them.
2. Shape. `ScalarApplicationObservations::new` (`:43`), `with_completed_scope` (`:99`), `with_application_roots` (`:205`), `with_invocation_roots` (`:246`), `with_abi_storage` (`:273`), `conditions` (`:289`), `entry` (`:337`), `record_value` (`:369`), `record_entry_result` (`:417`), `entry_result` (`:476`), `layout_dependencies` (`:506`), `observe_layout` (`:527`), `extent` (`:590`), `observation_ready` (`:609`), `integer` (`:656`), `discharge_range` (`:685`), `discharge_predicate` (`:726`); `validate_value` (`:783`).
3. In/out. Artifact view, optional `TerminalTargetContext`, `AdmittedProviderSelections`, optional `LayoutTargetFacts`. Mutates an `Arena<ApplicationObservations>` behind `Arc`.
4. How it decides. `new` refuses mismatched artifact identity (`:52-56`) and a folded-constant target that differs from the target or providers (`:57-64`). Layouts are admitted only when both a target and layout facts are given; one without the other refuses (`:65-72`). `entry` requires the application to be `Evaluate`, the entry machine to have no parameters and a scalar result of the declared carrier (`:358-366`). Values are stored once (`:407-409`); extension of a completed scope must equal the old value (`:393-400`). Range discharge: fixed bounds or address-model bounds; `minimum > maximum` refuses (`:711`); computed `[low, high]` must lie inside (`:718-722`). Predicate must evaluate to `Boolean(true)`.
5. Cases by shape. Layout observation: only `SEMANTICS_VERSION` match (`:547-551`) and integer carrier (`:573-577`). Extents accept unsigned or non-negative signed integers (`:593-606`).
6. Same job elsewhere. Integer-to-BigInt conversion appears at `:673-680` and `entry_checks.rs:397-406`.
7. Unused. Not grepped individually.
8. Stated limits. None.
9. Fail-open risks. `discharge_range` / `discharge_predicate` return `Ok(())` when the position is in the inherited completed scope (`:690-695`, `:727-732`): conditions already discharged in a previous scope are trusted. Classification: stated (doc `:96-98`: values and discharged conditions "still belong to the same code, providers and physical target"), checked by `restore_scope` (`completion.rs:131-153`).

---

## execution-inputs/target-layouts/admission.rs (1-332, read)

1. Purpose. Check selected layouts and lifecycle plans for a target and produce `AdmittedTargetLayouts`.
2. Shape. `AdmittedTargetLayouts` (`:31`), `TargetLayoutGeometry` (`:75`), `admit_target_layouts` (`:155`), `lifecycle_target_matches` (`:258`), `TargetLayoutSelectionError` (`:111`).
3. In/out. Execution view, `TerminalTargetContext`, `LayoutTargetFacts`; copies the module's records with storage replaced by `Authored(realization.storage)` and per-field storage filled in.
4. How it decides. Identity: artifact identity equals context's (`:161`); `target.profile == selections.target()` (string, `:167`); per application, realization found by target string (`:176-179`); realization geometry must match facts or the application is "stale" (`:184-191`); record must still be `TargetLayout(application.id)` (`:200`); every non-erased field needs a row (`:208-214`). Lifecycle plans: a plan with a `target` input applies only when `input.target == target.profile` and the rest of the geometry matches, else error (`:258-278`); two plans for one boundary refuse (`:236-246`).
5. Cases by shape. Only `Record` shape layouts (`:197-199`).
6. Same job elsewhere. None.
7. Unused. `TargetLayoutGeometry::unscoped` used in `observations.rs:199`.
8. Stated limits. `:80` doc: "unresolved target layouts remain unresolved".
9. Fail-open risks. An application with no realization for the selected target is skipped with `continue` (`:182`); its record keeps `TargetLayout` storage in the resulting geometry. Input: a layout application authored for other targets only. Classification: open (no comment).

---

## execution-inputs/target-layouts/target_context.rs (1-82, read)

1. Purpose. Bind a target name to one artifact identity; resolve an entry callable to a machine.
2. Shape. `TerminalTargetContext` (`:6`), `resolve_entry` (`:12`), `entry` (`:30`), `terminal_target_context` (`:73`), `TerminalEntryError` (`:49`).
3-4. `resolve_entry` refuses a context for another artifact (`:18`), a non-`Machine` root (`:21-23`), an absent machine (`:24`). With `selected == None` no identity check is made.
7. Unused. `resolve_entry(` plus `.entry(...)` grep: 18 occurrences in 8 files.
9. Fail-open risks. `resolve_entry(None, ...)` skips the identity check (`:18`); callers: `observations.rs:346` passes `self.targets` which can be `None`. Classification: traced to caller; with no target context the artifact is the one the view already carries.

---

## execution/callable_reach.rs (1-161, read)

1. Purpose. Records of calls and possible callable values reached from execution roots; comment says the checker, interpreter and native compiler share them.
2. Shape. `CallableValuePossibilities` (`:29`) with `join` (`:46`), `ReachableCallableInvocation` (`:108`), `ReachableCallableInvocations` (`:127`) with `site` (`:135`, binary search) and `operation` (`:146`, linear, refuses ambiguity).
4. `is_resolved` requires non-empty descriptors with no open inputs or ceilings (`:36-40`); `targets_are_resolved` requires `selected_machines` non-empty and a resolved target (`:116-123`); `Opaque` is never resolved.
7. Shared-use claim (`:2`): grep of `ReachableCallableInvocations|CallableValuePossibilities|ReachedCallableTarget` found 130 occurrences in 25 files, including native stages 04, 05, 15, register-homes, `psi/semantics`, `interpreter/instructions/execution.rs`, source stage 06 `ceilings.rs`.
9. Fail-open risks. `site` uses `binary_search_by_key`, which assumes `invocations` is sorted by `(caller, site)`; an unsorted table would miss rows (returns `None`, not accept). None seen that accepts.

---

## execution/callable_values.rs (1-188, read)

1. Purpose. Catalog of callable contracts, descriptors and envelopes (reach, suspend/block, termination premises).
2. Shape. `TerminalCallableCatalog` (`:17`), `TerminalCallableContract` (`:50`), `TerminalMachineDescriptor` (`:79`), `TerminalCallableEnvelope` (`:96`), lookups `contract`, `machine_envelope`, `boundary_envelope`, `descriptor` (index = id - 1, filtered by id equality, `:154-187`), `declaration_envelope` (binary search; ambiguity yields `None`, `:126-140`), `validate_scalar_type` (`:142`).
4. Lookups key on dense ids and verify that the stored id equals the requested one.
6. Same job elsewhere. Dense-id-minus-one indexing repeated four times (`:155`, `:162`, `:171`, `:183`).
9. Fail-open risks. None seen; absent -> `None`.

---

## execution/contracts.rs (1-149, read)

1. Purpose. `impl Psi` accessors for declaration contracts and invocation contracts (`declaration_contract` `:5`, `invocation_machine_contract` `:18`, `invocation_requires/ensures/structural_parameters`, `callable_requires/ensures/structural_parameters`).
4. How it decides. When a `required` contract id is given, the contract's `selected_application.realization` must equal the callee (`:27-29`). A `Boundary` callee with a `required` id yields `None` (`:41`, `:56`, `:84`).
9. Fail-open risks. When `required` is `None` the body's own contract is returned (`:24-26`) with no association check. Classification: stated in doc `:15-17` (reading a header grants no permission).

---

## execution/declarations.rs (1-14), dynamic_dispatch.rs (1-102), mod.rs (1-26), static_reach.rs (1-43), task_activation.rs (1-146), unoptimized_behavior.rs (1-115), roots.rs (1-281), suspension.rs (1-530) (all read)

- `declarations.rs`: `ExecutionDeclarations` (`:4`), eight fields.
- `dynamic_dispatch.rs`: `TerminalDynamicDispatchCatalog` (`:7`), `TerminalDynamicSlot::matches_operation_result` (`:48`) compares sorted claim paths of an operation result to `result_claim_paths`; lookups by `usize::try_from(id)` index (`:91-101`, ids are positions, not checked against a stored id field, unlike callable_values).
- `mod.rs`: module list and re-exports.
- `static_reach.rs`: records only. Doc comment `:1-3` "These checking records ... are not executable rows." grep of `StaticReachSummary|StaticReachApplication|StaticReachParameter|StaticReachTerm`: 45 occurrences in 10 files (mostly `psi/semantics/src/static_reach.rs` and source stage 05/06).
- `task_activation.rs`: data records for task start demands (`TerminalTaskActivationDemand` `:102`). Many raw `u64` fields (`place`, `value_type`, `claims`, `callee`, `identity`) typed as bare integers (`:37-40`, `:74`, `:93`, `:121`).
- `unoptimized_behavior.rs`: `UnoptimizedMachineBehavior::capture` (`:40`) lists direct invocations, boundary invocations, instruction-service operations, crashes, nominal cleanups. Callers: 7 call sites (grep `UnoptimizedMachineBehavior::capture`: native `behavior-exclusions/closure.rs:795`, wire `foundation_validation.rs:10648`, semantics `body_services.rs:172` and `static_reach.rs:318`, source stage 05 `lib.rs:2155`, two tests). It lists service operations only for six `OperationKind`s (`:59-72`), matching a fixed list.
- `roots.rs`: `PreparedNominalRoot` (`:22`), `PreparedTerminalRoots::new` (`:109`): any `Available` declaration with an invocation-local identity (machine, state, semantic declaration, specialization template/entry/semantic) is downgraded to `Unavailable` (`:115-137`); `retained_roots` (`:143`) requires exactly one retained machine whose declaration has the same identity (by `same_identity_as`, tables compared by text) (`:155-169`); `resolve` (`:49`) returns multiplicity errors for 0 or >1. grep `PreparedNominalRoot|PreparedTerminalRoots`: 33 occurrences in 13 files.
- `suspension.rs`: records for suspension call plans plus `suspension_frontier_commitment` (`:189`), domain `terminal-suspension-frontier-v9\0` (`:194`), hashing every live value, loan row and arena handle (index and generation via `hash_handle` `:297`). Field path segments are hashed by identity text (`hash_loan_path_segment` `:439-444`, `identities.text(*field).unwrap_or_default()`), so a missing text hashes as empty. `hash_carry_policy` hashes four booleans (`:503-517`) not the policy enum. Fail-open: none seen; hash only.

Stated limits (all above): `static_reach.rs:1-3`; `callable_reach.rs:3` shared-use claim (counted above).

---

## exports.rs (1-152, read)

1. Purpose. Flat re-export list for the Psi crate. `pub use crate::Psi as TerminalModule` (`:3`); also re-exports names from `language_core`, `semantic_vocabulary`, `psi_effect_contracts`, `application_identity::const_value::FiniteScalarValue` (`:132`).
4-9. No decisions. `pub(crate) use ::wire::digest_encoder` (`:152`).

---

## identities/applications/src/callback_declarations.rs (1-111, read)

1. Purpose. Identity records for private callback slots and boundary telescopes.
2. Shape. `PrivateCallbackSlotSelection` (`:4`) with `has_complete_identity` (`:16`), `validate_identity_encoding` (`:26`); `CallbackRegistrationDeclaration` (`:44`); `CallbackDeclaration` (`:49`); constants `BOUNDARY_NATIVE_PARAMETER_CAPACITY = 32`, `BOUNDARY_CALLBACK_CAPACITY = 64` (`:56`, `:58`); `BoundaryTelescope` and its row records (`:60-89`).
4. Validation: all five identity strings non-empty, commitment non-zero, encoding non-empty (`:16-25`); recomputes report fingerprint and commitment from the encoding (`:32-38`). String fields keyed on spelling.
7. Unused. `BOUNDARY_NATIVE_PARAMETER_CAPACITY|BOUNDARY_CALLBACK_CAPACITY`: 15 occurrences in 4 files (grepped).
8. Stated limits. The two fixed capacities (32, 64) are invented caps with no comment.
9. Fail-open risks. None seen.

---

## identities/applications/src/conformance_identity.rs (1-539, read; tests from `:294`)

1. Purpose. Builds the canonical byte description of a conformance application and its commitments (FNV-1a 64 report fingerprint plus domain-hash commitment), and the machine-application commitment.
2. Shape. `ConformanceIdentityDescription` (`:5`) with `encoded_bytes` (`:65`) and `fingerprint_and_commitment` (`:148`); `MachineApplicationCommitment` (`:157`), `requirement_application_commitment` (`:233`), `source_identity_commitments` (`:255`; domain `omega.psi.closed-conformance-application.v4\0`).
4. How it decides. `MachineApplicationCommitment` refuses empty callable/type/implementation/machine identity and non-canonical static const values (`:160-215`). `requirement_application_commitment` accepts only `Type` and `Const` operands, non-empty; any other operand (`Conformance`) errors (`:240-249`).
5. Cases by shape. Doc `:230-232`: "another operand category needs its complete contract before use here."
6. Same job elsewhere. `MachineApplicationCommitment` implements `std::hash::Hasher` so `Hash` impls feed bytes (`:221-228`), `finish()` returns 0 for the `Hasher` method (`:222`).
7. Unused. `requirement_application_commitment`: grepped, 5 occurrences in 5 files (including this file's definition).
8. Stated limits. `:230-232`.
9. Fail-open risks. `encoded_bytes` writes `machine_arguments` without a count (`:124-126`); `machine_operands` has a count. Two argument lists that differ in how texts split are distinguished by per-text length prefixes (`append_text`), so no ambiguity (inferred).

---

## identities/applications/src/const_value.rs (1-1018; read 1-700; names only 701-1018)

1. Purpose. Canonical text encoding of compile-time constants (`CanonicalConstValue`) and its decoder, application keys, and finite scalar values.
2. Shape. `mod encoding` (`:9`), `DecodedCanonicalConstValue` (`:121`), `FiniteScalarValue` (`:157`), `CanonicalConstValue` (`:200`), `CanonicalConstIdentity` (`:215`), `decode_declaration_value` (`:249`), `decode_node` (`:430`), `decode_fields` (`:557`), `take_canonical_piece` (`:577`).
3. In/out. Strings in and out. `framed(tag, pieces)` writes `tag` then `len:payload` pieces (`:419-428`).
4. How it decides. Decode accepts only canonical spellings: integer type name must round-trip (`:472`), decimal spelling must round-trip (`:476`), value must fit the carrier (`:479`), floats only in declaration mode and non-NaN with fixed-width hex (`:441-461`), duplicate or empty field names refuse (`:566`), length prefixes may not have leading zeros (`:580-585`). `is_canonical_static_value` requires non-empty `type_name` and that re-encoding gives the same string (`:285-290`). Keyed on text spelling by design.
5. Cases by shape. Tags: float, integer, boolean, static-callable, array, record, variant (`:441-552`).
6. Same job elsewhere. `take_length_delimited` (`:596`) vs `take_canonical_piece` (`:577`): two length-prefix parsers; the first accepts leading zeros and non-digit signs via `parse`.
7. Unused. Not grepped.
8. Stated limits. `MAX_ENCODING_BYTES = 64 * 1024`, `MAX_DECODED_NODES = 4_096`, `MAX_DECODE_DEPTH = 64` (`:116-118`), re-exported as `MAXIMUM_*` (`:254-256`). Doc `:131-132`: static callables "grant no execution or source authority".
9. Fail-open risks. `from_atom` / `take_length_delimited` take any `type_name` and do not validate the encoding (`:385-394`); validity is checked later by `decode_encoding`/`is_canonical_static_value`. Classification: traced inside file only; consumers outside the unit not read.

---

## identities/applications/src/lib.rs (1-12), opaque_representations.rs (1-88) (read)

- `lib.rs`: module list; doc `:1-2` "No source files, mutable symbol tables or preparation entry points."; `#![forbid(unsafe_code)]`.
- `opaque_representations.rs`: `SelectedOpaqueApplication` (`:44`), `selected_application_commitment` (`:21`, domain `omega.opaque-representation-application.v1`), `validate_identity_encoding` (`:59`). Single-variant enums: lifecycle `Inert` (`:7`), origin `NamedConformance` (`:18`). Validation: non-empty identities and encoding, schema version equals 1, recomputed commitments equal (`:60-85`).

## identities/applications/src/tests.rs (1-733, names only)

Type-identity visitor and grammar tests (names listed by grep: `every_spelling_of_the_vocabulary_is_unique_and_lexes_whole`, `retired_dynamic_trait_identities_are_rejected`, `only_binder_replacements_reach_the_consumer`, `retired_scalar_interval_identity_tags_are_refused`, and others). Not read.

---

## Coverage

Read in full:
- `artifact/wire/src/sections.rs` 1-829 (829); `service_schema_wire.rs` 1-408 (408); `spec_tag_tables.rs` 1-642 (642); `term_wire.rs` 1-1651 (1651); `wire_fields.rs` 1-547 (547).
- `semantic_module.rs` 1-2900, 4185-4359, 5297-5341 (3120).
- `structural_wire.rs` 1-385, 560-1555 (1381).
- `const_value.rs` 1-700 (700).
- `entry_memory.rs` 331; `component.rs` 149; `memberships.rs` 618; `logical_reach.rs` 184; `selections.rs` 590; `abi_storage.rs` 58; `completion.rs` 542; `conditions.rs` 690; `entry_checks.rs` 459; `observations.rs` 793; `scalar_applications.rs` 99; `admission.rs` 332; `target_context.rs` 82.
- `execution/`: `callable_reach.rs` 161, `callable_values.rs` 188, `contracts.rs` 149, `declarations.rs` 14, `dynamic_dispatch.rs` 102, `mod.rs` 26, `roots.rs` 281, `static_reach.rs` 43, `suspension.rs` 530, `task_activation.rs` 146, `unoptimized_behavior.rs` 115; `exports.rs` 152.
- `identities/applications/src/`: `callback_declarations.rs` 111, `conformance_identity.rs` 539, `lib.rs` 12, `opaque_representations.rs` 88.
- Total read in full: 16,862 lines.

Skimmed (function names listed with Grep, bodies not read), 4,300 lines, all test code:
- `semantic_module.rs` 2901-4184, 4360-5296, 5342-5446 (2,326)
- `structural_wire.rs` 386-559, 1556-1896 (515)
- `completion_tests.rs` 1-408 (408)
- `const_value.rs` 701-1018 (318)
- `identities/applications/src/tests.rs` 1-733 (733)

Not read at all: none beyond the skimmed list (the skimmed ranges are the not-read ranges).

## Five things a maintainer most needs to know about this unit

1. Decode in `semantic_module.rs:4264-4303` is a fixed pipeline of validators, then a re-encode-and-compare for canonicality (`:4298-4302`). The accepted bytes are exactly the bytes the encoder would write. Format marker is `290` (`:4205`); the area crate map says specs mention 291.
2. Cross-artifact and cross-module identity matching in this unit is by identity text, not ids: provider selection (`selections.rs:322-330`), component providers (`component.rs:89-96`), domain routes (`memberships.rs:247-284`), canonical-order sorts of provider candidates (`semantic_module.rs:303-318`, `:532-552`).
3. Admission loops silently skip rows that do not apply: a layout application with no realization for the target (`admission.rs:176-183`), a prerequisite on a layout-observation application (`entry_checks.rs:107-109`), a source domain with no matching target domain that is not required (`memberships.rs:87-107`). None has a comment explaining it.
4. Scalar-application closure (`completion.rs`, `conditions.rs`, `observations.rs`, `entry_checks.rs`) has four scope kinds (Source, Deployment/SourceRoots, Invocation, AbiStorage). Retained non-application conditions are enforced at `finish` only for the `Source` kind (`completion.rs:503-528`).
5. Fixed numeric limits appear without stated reason in several places: `65536` entry RAM (`entry_memory.rs:155`), `64*1024` / `4096` / `64` const encoding bounds (`const_value.rs:116-118`), `32` / `64` boundary telescope capacities (`callback_declarations.rs:56-58`), and the sizes in `HostedOperation` wire tags that differ between `service_schema_wire.rs:90-91` and `structural_wire.rs:861-864`.

## What you are unsure about

- Whether `decode_module_body` (not in this unit) checks the vocabulary marker; tests at `semantic_module.rs:5298-5335` imply it does.
- Whether the two different `HostedOperation` encodings (`service_schema_wire.rs:89-94` tags 2 and 4; `structural_wire.rs:861-864` tag 0) are intentional (different contexts) or drift; I read both but not their users.
- Per-name call counts for `encode_invocation_prefix`, `module_decode_count`, `manifest_for_semantic`, `build_identity_optimization_execution_record` (only a combined count was taken).
- Test bodies in the 4,300 skimmed lines: I make no claim about what they assert.
- Whether `ReachableCallableInvocations` is actually shared by checker, interpreter and native compiler: grep shows files in all three, but I did not read them to confirm each uses the same records rather than a copy.
