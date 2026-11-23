# Record: unit 040, gap 018 (second pass), area 08-psi-program

Unit file: `units/gap_018.tsv`. Checkout `4f3c255d05599c11b1ea8ac668635f15f2a6eb86`. Every range below is test code or a test fixture. No production function is defined in any range except where noted. All of it was read in full. Nothing was built or run. Grep tool listings of `fn`/`mod` lines (read) give the test names and test-module boundaries cited below.

Wire format note used throughout: the wire crate is `terminal-wire` (`psi/artifact/wire`). The tests call `encode_module`, `decode_module`, `encode_raw` (an encoder that skips validation), `semantic_fingerprint`. Production items live at `semantic_module.rs:4209-4346` (not in my ranges; listed by Grep at those lines).

---

## 1. `source-rust/subsystems/compiler/psi/artifact/wire/src/semantic_module.rs`

### 1a. Lines 2901-4184 (1284 lines)

**Where it sits (read + Grep listing).** The range starts inside the `module()` fixture builder of `mod routed_scalar_tests` (opens `:2849`, `fn module` `:2867`). It then covers the rest of that module (closes `:4042`) and `pub(crate) mod entry_tests` (opens `:4043`, runs on past `:4184`).

**Purpose.** These tests check the Psi wire round trip and the receiving-side validation of scalar domain membership ("qualifications"), checked specializations (a machine built from a template with arguments), closed reach applications (callback-style machine parameters), and portable entries. Each test encodes a hand-built `TerminalModule`, decodes it, and then breaks one field to confirm the decoder refuses it with a named error.

**Shape (all read).**
- `routed_scalar_tests::module()` tail `:2909-2950`: builds a one-machine module (`TerminalModule::for_entry`, `:2909`) with one `ScalarDomainDeclaration` whose `establishment_routes` is `Some(vec![ExactMachine{..}])` (`:2925`), one `ScalarQualificationSet`, and one `ScalarReturnQualification` with `ScalarReturnOrigin::Established { route: 0 }` (`:2947`).
- `qualify_input` `:2954`: copies the domain predicate into the machine's `requires`, remapped to the input value (`:2965-2966`).
- Tests (`#[test]` fns, names from Grep): `routed_scalar_result_round_trips_and_commits_predicate_and_origin` `:2970`; `scalar_boundary_result_retains_membership_and_rejects_detached_types` `:2988`; `routed_scalar_completion_retains_the_exact_formation_edge` `:3079`; `routed_scalar_result_rejects_detached_or_forged_transport` `:3186`; `routed_scalar_forwarding_preserves_membership_without_issuer_status` `:3255`; `empty_issuer_set_round_trips_without_authorizing_fresh_membership` `:3277`; `declaration_contract_roundtrip_keeps_public_promises_separate_from_body` `:3322`; three `previous_*_format_refuses_before_decoding` tests `:3498`, `:3508`, `:3518`; `const_telescope_wire_round_trips_static_callable_records_and_refuses_bad_profiles` `:3528`; `routed_specialization_retains_guarded_acquisitions_and_exact_issuer` `:3686`. That is 12 tests in `routed_scalar_tests` within this range.
- `entry_tests` (`:4044`): `pre_installation_module_header_rejects_before_reading_tables` `:4055`, `declaration_catalog_round_trip_preserves_absent_default_entry` `:4065`, `portable_entry_requires_one_retained_executable_machine` `:4082`, and `assembly_catalog_target_demand_survives_wire_and_changes_identity` `:4168` (it continues past `:4184`; the rest is outside my ranges). Shared helpers `unit_machine` `:4102` and `identity_table` `:4159` are `pub(crate)` and are used by sibling test modules (see item 6).

**Inputs and outputs.** Inputs are hand-built `TerminalModule` values plus mutations. Outputs are assertions on `decode_module` results and on `CodecError` variants. Nothing is mutated outside the test.

**How it decides (what each test pins; the judgment is by identity/structure, not spelling, except where noted).**
- Scalar return origin: the decoder must reject a returned scalar whose `origins` do not match the domain's issuer routes. Examples: wrong `edge`/`source`/`result` ids, `ScalarReturnOrigin::Forwarded` when the source has no qualification (`:3233-3236`), `Established{route:1}` when only route 0 exists (`:3240-3243`). Keyed on value ids, edge ids and route index. Error text asserted is `MalformedScalarReturnFoundation` containing `origin authorized=false` and `issuer=None` (`:3286-3288`), a substring match on the reason string in the test only.
- Empty issuer set (`establishment_routes = Some(Vec::new())`) must round-trip and change the fingerprint versus `None` (`:3299-3308`). Comment `:3273-3275`: it "can transport an existing qualification, but it cannot make a fresh result." Encoding does not refuse it; the decoder does (`:3280-3289`, comment: "Encoding trusts its stage producer. Receiving formation validation, rather than serialization, must refuse").
- Declaration contract: a public declaration contract (`execution.declaration_contracts`) must carry its own `declaration_envelopes` row (`:3348-3370`), and a declaration-only contract id cannot be the callee of an executable `CallUnit` (`:3397-3424`).
- Specialization and closed reach: decoder cross-checks `template_identity`, `specialization_commitment`, and the retained application. Errors asserted: `"checked specialization has no retained application"` `:3757`; `"closed reach application differs from its checked template identity"` `:3771`; `"...declaration commitment"` `:3782`; `"machine application arguments differ from their retained commitment"` `:3750`, `:3678`. Const arguments are keyed on the canonical encoding string re-decoded (`:3611-3633`, `argument.decode_encoding()` `:3635`); a forged but well-formed alternative encoding still fails because it no longer matches the commitment (`:3658-3681`).
- `crashes none` binder: a selected callee that may crash is refused when the requirement's `upper_bound_crashes_none` is true (`:3861-3910`); the template commitment is recomputed with `closed_reach_template_commitment` in the test (`:3880-3889`).
- Portable entry: `module.entry` must name a retained executable machine; a missing machine id or a `TargetMachineCallable::Boundary` entry is refused with `"portable entry has no retained executable machine"` (`:4088-4099`).
- Format markers: markers 257, 256, 253, 246 and 164 are refused with `UnsupportedFormatMarker` before the payload is read (`:3497-3525`, `:4055-4062`, `:4183-4184`). Keyed on the u16 at bytes 8..10 after the 8-byte magic `PSITERM\0`.

**Cases by shape.**
- Mutation loops enumerate a fixed set of forgeries: `for mutation in 0..4` `:3051`, `0..5` `:3147`, `0..4` `:3465`, `0..5` `:3634`. These are test enumerations, not production case lists.
- `"routed_scalar_forwarding..."` only exercises one callable identity swap (`callable = TerminalIdentityIndex(2)` `:3258`, `:3292`) to make the machine not the issuer.

**Same job elsewhere.**
- Tests build a hand-made `TerminalModule` fixture in many modules of this file: `routed_scalar_tests::module` `:2867`, `normal_qualification_tests::module` `:4384`, `descriptor_module` `:4808`, `task_module` `:4950`, `predicate_module` `:5347`. Each repeats the full `StructuralTypeDeclaration` field list (`declaration_identity`, `requires`, `quotient_carrier`, `owns_linear_claim`, ...). Read.
- Same struct literal also appears in `structural_wire.rs:1557-1576`, `:1724-1740`, `:1769-1785`, `:1811-1843` (read) and in `completion_tests.rs:69-82` (read).

**Unused or one-caller code.** No `pub` production function in range. `entry_tests::unit_machine` and `identity_table` are `pub(crate)` inside `#[cfg(test)]`. Grep `entry_tests::` on `semantic_module.rs`: callers at `:4801`... see item 6 (consumers found at `:4801` `use super::entry_tests::{identity_table, unit_machine}`, `:4938`, `:5225`). Count from the Grep listing in the next block.

**Stated limits.** `:3426` "Fixed model commitments isolate artifact reference integrity. They establish no source application, selection or refinement proof." `:3273-3275`, `:3280-3281` as quoted above.

**Fail-open risks.** None seen in range (all tests assert refusals or round trips). One note: the test at `:3277-3285` records that `encode_module` of a fresh result with an empty issuer set succeeds and only `decode_module` refuses; the comment at `:3280-3281` states this is intended ("stated").

### 1b. Lines 4360-5296 (937 lines)

**Where it sits.** Mid-way through `mod normal_qualification_tests` (opens `:4356`, imports `:4360-4374`) through `mod global_descriptor_table_tests` (`:4800`), `mod descriptor_operation_tests` (`:4937`), and `mod opaque_value_tests` (`:5224`, ends `:5295`). (Grep listing, read.)

**Purpose.** Test the wire round trip and receiving validation of structural boundary declarations (lifetimes, qualifications, required paths), the x86 descriptor-table and task-register and segment-register operations, and opaque structural types.

**Shape (read).**
- `normal_qualification_tests::module()` `:4384-4542`: one entry machine, four structural types (packed record, fixed array of 3, mutable reference, a packed record with two fields) `:4389-4418`, native layout rows `:4421-4433`, one structural domain `:4451`, one `BoundaryMachineDeclaration` with three structural parameters, `requires` on `field(8)` `:4494-4498`, `normal_qualifications` on paths `[]`, `[field(8)]`, `[Referent, AllElements]` `:4500-4517`, and one `BoundaryReferenceOutput` `:4523-4532`.
- Tests: `boundary_logical_result_lifetimes_round_trip_and_reject_missing_frontiers` `:4545`; `an_authored_record_keeps_its_invariant_through_public_module_readback` `:4642`; `structural_wire_preserves_an_explicit_empty_issuer_set` `:4665`; `normal_qualification_round_trip_retains_root_field_and_all_referent_elements` `:4688`; `input_fact_lifetimes_survive_encoding_and_change_the_semantic_identity` `:4697`; `an_input_fact_cannot_replace_missing_membership_evidence` `:4728`; `normal_qualification_and_required_paths_change_whole_semantic_fingerprint` `:4745`; `normal_qualification_public_decoder_rejects_wrong_carrier_and_missing_path` `:4769`.
- `global_descriptor_table_tests`: `descriptor_module(operation)` `:4808`, `refusal(&module) -> String` `:4878`, three tests `:4887`, `:4899`, `:4926`.
- `descriptor_operation_tests`: `task_module()` `:4950`, `refusal` `:5077`, `segment_module(kind)` `:5130`, tests `:5086`, `:5093`, `:5160`, `:5186`.
- `opaque_value_tests`: one test `:5234`.

**Inputs and outputs.** Same pattern: build module, encode, decode, mutate, assert. `refusal` formats the `CodecError` with `{:?}` and the tests assert on substrings of that debug string (`:4906`, `:4914`, `:4922`, `:4931`, `:5100`, `:5107`, `:5114`, `:5117`, `:5124`, `:5127`, `:5194`, `:5201`, `:5208`, `:5211`, `:5218`).

**How it decides (what the tests pin).**
- Result lifetimes: a boundary with a borrowed-view result must carry `result_lifetimes` (`None` or `Some(vec![])` both refused, `:4604-4608`). A reference result with no input anchor is accepted (`:4609-4624`, comment: "A provider can return a reference to its own storage with no input anchor"). A result leaf not covered by a declared lifetime fails with `"boundary result leaf has no unique declared lifetime"` `:4635`.
- Input-fact observation `additional_input_qualifications` without matching membership evidence refuses with `NonCanonicalOrder("input observations by exact qualification subject")` `:4740`.
- Path carrier checks: `normal_qualifications[2].path` of `[Referent]` or `[FixedIndex(0)]` fails with `"boundary normal guarantee has the wrong mutable path carrier"` `:4783`; a requirement path `field(12)` (field not on the carrier) fails with `"boundary requirement domain has the wrong path carrier"` `:4793`. Keyed on structural path segments against the carrier type shape.
- Descriptor table ops: refusal substrings `"exact unqualified ten-byte array"` (`:4906`), `"byte borrow exceeds its source access"` (`:4914`), `"declared byte borrow"` (`:4922`), `"authored callable effects differ from their published promises"` (`:4931`). Task register: `"mutable GDT"`, `"exact unqualified"`, `"declared element width"`, `"exact unsigned result"`, `"u16 selector"`. These pin hardware-specific sizes: GDT table is 5 entries, 104 bytes shared view (`:4967`; capacity 104 for 5 entries), 10 byte scratch (`:4971`), u64 elements (`:4973`), u16 selector (`:5028`), u8 result of activate (`:5040`), u16 result of read (`:5065`).
- Opaque type: `StructuralTypeShape::Opaque` round-trips; every child path segment (`Field`, `FixedIndex`, `Case`, `Referent`, `AllElements`, `RepresentationView`) fails `validate_structural_path` `:5267-5286`; `zeroed_structural_shape` is false `:5258`; setting `quotient_carrier` on an opaque type makes `encode_module` fail with text containing `"opaque data cannot supply a visible carrier"` `:5287-5293` (so this refusal is at encode, not only decode).

**Cases by shape.**
- Hard-coded shapes: the x86 operations `ActivateTaskRegister`, `ReloadSegmentDescriptors` (`SegmentReload::Code`/`Data`), `ReadSegmentSelector` with six `SegmentRegister` values (`:5162-5169`: Code, Data, Extra, Stack, Fs, Gs), `GlobalDescriptorTable` `Store`/`Load` (`:4889-4891`). The validator under test accepts these only with fixed extents (10-byte GDT descriptor, `entries * 8 + ...` = 104 for 5 entries inferred from `:4967` and `entries: 5` `:5044`; not stated in the test).
- The `module()` fixture's `no_code_affine_cleanup: position != 2` `:4442` marks only the reference type (index 2) as needing cleanup.

**Same job elsewhere.** The `refusal` helper is defined twice, `:4878` and `:5077` (identical shape: encode then format decode error). Read. Same `TerminalModule` struct-literal repetition as in 1a.

**Unused or one-caller code.** `entry_tests::unit_machine` and `entry_tests::identity_table` are imported at `:4801`, `:4938`, `:5225` (read); `identity_table` is also used inside `entry_tests` itself and by `routed_scalar_tests` (`:2910`, `module.identities = identity_table(4)`). I did not run a repository-wide count (test-only helpers; `pub(crate)`).

**Stated limits.** `:4871-4872` "The fixture authors this service ceiling above; its published contract must retain that same ceiling before operand checks run." `:5070-5071` "Task-register and segment fixtures inherit one explicit service ceiling". `:5235-5236` "Its presence grants neither zero construction nor readable children."

**Fail-open risks.** None seen. `:4609` records an accepted case (reference result, empty inputs) with a stated reason (comment quoted above, "stated").

### 1c. Lines 5342-5446 (105 lines)

**Where it sits.** Inside `mod scalar_predicate_definition_tests` (opens `:5339`). Starts at the end of the `use` block (`:5342-5345`), then `predicate_module()` `:5347` and one test `:5373`.

**Purpose.** Pin the wire form of `ScalarPredicateDefinition` (a named predicate with formal parameters and a body): round trip, identity change, and rejection of malformed definitions.

**Shape (read).** `predicate_module` builds a declarations-only module (`TerminalModule::declarations()` `:5348`) with one definition: one `u32` formal `ValueId(1)` and body `0 <= formal` `:5364-5367`. The test `scalar_predicate_definition_roundtrip_retains_body_and_formal_scope` `:5373`.

**How it decides (pinned by the test).**
- Body referring to a value id that is not a formal (`remap_value_ids(|_| ValueId(2))` `:5384-5387`) fails with `CodecError::MalformedProposition(_)` `:5390`.
- A duplicate formal fails with `NonCanonicalOrder("scalar predicate inputs by ValueId")` `:5401`.
- Two definitions with the same declaration identity, or out of order, fail with `NonCanonicalOrder("scalar predicate definitions by declaration identity")` `:5412`, `:5426`.
- `declaration = TerminalIdentityIndex(u32::MAX)` fails with `MalformedStructuralFoundation(_)` `:5435`. A formal with a nonzero qualification set id fails likewise `:5443`.
- Changing the body changes the semantic fingerprint `:5377-5382`.

**Cases by shape.** None beyond the mutations. **Same job elsewhere.** None seen in range. **Unused code.** None. **Stated limits.** None. **Fail-open risks.** None seen. Note `:5389` and `:5442` assert only the error variant, not the reason (the `_` pattern), so these two tests do not pin which check fired.

---

## 2. `source-rust/subsystems/compiler/psi/artifact/wire/src/structural_wire.rs`

### 2a. Lines 386-559 (174 lines)

**Where it sits.** Inside a nested test module of `structural_wire.rs` (it uses `super::super::module_wire`, `super::super::MAGIC`, so it is two modules deep). I did not read the module's opening line; the range starts at a test's `#[test]` at `:386`. The first test is at `:387`.

**Purpose.** Pin the structural path wire forms: declared-lifetime families, reference-origin families and ordinary operand paths are three different encodings, and cleanup actions use a fixed tag.

**Shape (read).** Tests: `declared_lifetime_families_round_trip_without_becoming_operand_paths` `:387`; `origin_family_wire_is_distinct_from_an_executable_path` `:464`; `exact_origin_projection_retains_existing_wire_bytes` `:489`; `owned_cleanup_round_trip_retains_source_path_transfers_and_proofs` `:507`; `borrowed_cleanup_tags_are_retired` `:545`; `owned_cleanup_wire_change_rejects_previous_module_version` `:555` (the range ends mid-test at `:559`; the rest is outside my ranges, so the assertion after `assert!(matches!(` at `:559` was not read).

Functions under test (names from calls; defined elsewhere, not read): `encode_structural_path`, `decode_structural_path`, `encode_declared_lifetime_path`, `decode_declared_lifetime_path`, `encode_reference_origin_path`, `decode_reference_origin_path`, `encode_affine_cleanup_action`, `decode_affine_cleanup_action`, and `ResultLifetime::{write,read}` from `module_wire::closed_conformance_wire`.

**How it decides (pinned by the test).**
- A path containing `Case` or `AllElements` encoded as a lifetime family decodes via `ResultLifetime::read` (`:411-415`) but is refused by the ordinary path encoder with `"returned-origin interpretation is not an operand or owned path"` `:420-425`. The ordinary decoder refuses the family bytes the same way `:428`.
- A reference-origin path with an exact `AllElements` is refused: `"qualification wildcard is not a reference origin"` `:434-444`.
- A dynamic segment (`FixedRange`) is refused as a declared lifetime path: `"declared lifetime path is not a static family"` `:446-460`.
- Origin family bytes decoded as an ordinary path fail with `InvalidTag("StructuralPathSegment", 9)` `:482-485` (tag 9 is the `Element` projection tag, inferred from the origin path at `:468` having `Element`).
- An all-`Exact` origin path encodes to the same bytes as the ordinary path `:495-503`.
- `InvokeOwned` cleanup encodes with first byte 6 (`assert_eq!(encoded[0], 6)` `:535`); tags 3 and 5 are retired and decode to `InvalidTag("TerminalAffineCleanupAction", tag)` `:545-551`. Keyed on the byte tag.
- Previous module versions 275 and 276 are listed as refused (`:556`).

**Cases by shape.** Fixed tag list: tags 3, 5 retired; 6 is owned cleanup. Read.

**Same job elsewhere.** The "previous format marker refuses" pattern is repeated: `semantic_module.rs:3498-3525`, `:4055-4062`, `:4183-4184`; `structural_wire.rs:556`, `:1757-1762`, `:1799-1804`. Each uses a different hard-coded marker (275, 276, 183, 167 here; 257, 256, 253, 246, 164 there). Read. Grep for the marker constant `FORMAT_MARKER` is outside my ranges (crate-map says `semantic_module.rs:4205`, not read by me).

**Unused code.** None in range. **Stated limits.** None. **Fail-open risks.** None seen.

### 2b. Lines 1556-1896 (341 lines)

**Where it sits.** Inside a nested test module that tests `encode_structural_type`/`decode_structural_type`. Range starts at the fixture `invariant_record` `:1556` and ends at `:1896`, which closes the test module (`:1895`) and the file or parent module (`:1896`).

**Purpose.** Pin the wire form of `StructuralTypeDeclaration`: `requires` clauses, nominal cleanup selection, multiplicity, linear ownership, authored record geometry, and authored field storage.

**Shape (read).**
- Fixtures: `invariant_record(storage)` `:1556` (a packed/other-storage record with `requires = [Truth, Falsehood]` `:1559-1562`); `shape_offset(bytes)` `:1579` (re-reads the header field by field to find the byte offset of the shape tag; it hard-codes the declaration header layout: a bool, a `StructuralTypeId`, two `u32`, three bools, a multiplicity, a bool `:1580-1590`).
- Tests: `selected_cleanup_retains_exact_declaration_and_application` `:1594`; `record_requires_preserve_the_storage_shape_and_ordered_clauses` `:1619`; `record_requires_refuse_nonrecords_empty_clauses_and_truncation` `:1680`; `declared_multiplicity_round_trips_and_rejects_unknown_tags` `:1717`; `declaration_owned_linearity_round_trips_and_changes_bytes` `:1766`; `declaration_owned_linearity_rejects_the_previous_module_schema` `:1798`; `authored_record_geometry_round_trips_without_widening_its_scalar` `:1808`; `authored_field_wire_preserves_absence_stride_and_fragmentation` `:1856`.

**How it decides (pinned by the test).**
- `requires` is carried by a wrapper tag in the shape position: `RECORD_WITH_REQUIRES` for records of five storage kinds (Packed, Authored, TargetLayout, AccessView, Accessor) `:1626-1650` and `SUM_WITH_REQUIRES` for sums `:1673`. With `requires` empty the bytes are the ordinary shape bytes, with the wrapper byte inserted otherwise (`:1648-1649`). Wrapper tag on a non-record shape tag (2, 6, 19, or the sum wrapper on a record) fails with `"record requires wrapper must contain an ordinary record storage shape"` `:1686-1694`. A wrapper with zero clauses fails with `"a declaration tagged with requires clauses declares none"` `:1708-1712`. Every proper prefix of the bytes from `offset` fails `is_err()` `:1696-1698`.
- Multiplicity tag values 0, 4, 255 fail with `InvalidTag("StructuralMultiplicity", tag)` `:1749-1753` (valid values are therefore 1, 2, 3, inferred from the three round-trip cases and the invalid set). The multiplicity byte is found at `shape_offset(&bytes) - 2` `:1748`.
- `nominal_cleanup` selection keeps both its declaration identity and its cleanup machine id: four combinations all give distinct bytes `:1596-1615`.
- `owns_linear_claim` changes the bytes `:1766-1795`.
- Authored field storage forms tested: absent, `Repeated{byte_offset, element_stride}`, `Direct{byte_offset}`, `Fragmented(AuthoredBitFragments)` `:1857-1882`; the stored scalar (`i8`) of an authored scalar field is distinct from its logical type (`i64`) `:1809-1842`.

**Cases by shape.** `:1619-1621` comment: "Geometry and predicates are independent. Each existing storage shape must keep its clauses in the same authored order." Fixed list of five storage shapes plus sum. Read.

**Same job elsewhere.** `StructuralTypeDeclaration` literal repetition (see 1a). `shape_offset` hard-codes the encoder's field order and would need changing with the encoder; I did not read the encoder (`encode_structural_type`) so I cannot confirm that it matches except via the passing tests (not run).

**Unused code.** None. **Stated limits.** None. **Fail-open risks.** None seen. Note `:1696-1698` asserts only `is_err()` for truncation, so the error kind is unpinned.

---

## 3. `source-rust/subsystems/compiler/psi/execution-inputs/scalar-applications/completion_tests.rs` (lines 1-408, whole file)

**Purpose.** Tests for the scalar-application completion path used for calling-ABI storage: a "demand" naming one calling ABI declaration and a target, observations of its array extents, and a closure that must prove all extent conditions before ABI storage can be used. The file is declared as `mod completion_tests;` at `scalar_applications.rs:16` (grepped), so it is a child module of the crate root of `scalar-applications` and starts with `use super::*;` `:1`.

**Shape (read).**
- `fixture(with_extents: bool, false_helper: bool) -> (CanonicalTerminalArtifact, Vec<CallingAbiIdentity>)` `:11-153`. Builds a declarations module with two `CallingAbiDeclaration` rows (`Receiver<u8>`, `Receiver<u16>`) `:22-47`. With `with_extents`: three structural types (a `u64`, a `FixedArray` whose length is `ArrayExtent::Application(ScalarApplicationId(1))`, and one with `(2)`) `:49-83`, and two `ScalarApplication::LayoutObservation` rows (Size, Alignment) `:84-94`. The calling ABI result type is `StructuralTypeId(3)` only when `with_extents` `:38`. With `false_helper`: a proof-scope callable declaration plus one retained predicate that is `Boolean(false)` labelled `"actual ABI extent helper debt"` `:96-144`.
- `facts(target)` `:155` builds `LayoutTargetFacts` (pointer 8/8, case tag 4/4, little endian).
- Tests: `abi_storage_request_requires_exact_canonical_roster` `:167`; `abi_storage_receipt_binds_full_key_and_grants_no_entry_or_product_authority` `:203`; `target_context_binds_artifact_without_selecting_an_entry_body` `:247`; `abi_storage_request_refuses_other_artifact_and_target` `:336`; `abi_storage_completion_refuses_actual_false_helper_debt` `:363`; `demand_retains_the_exact_declared_extent_roster_and_artifact` `:393`.

Types and functions used (defined elsewhere; not read): `CallingAbiStorageDemand::new(view, target, key)` (`.applications()`, `.declaration()`, `.target()`, `.require_artifact`), `ScalarApplicationObservations::new(view, &targets, &providers, &facts)` with `.with_abi_storage(&demand)` and `.conditions(target, &roster)`, `ScalarApplicationClosure::for_abi_storage(view, &demand)` with `.close_scope(observed, conditions, Ok(None))` and `.finish()`, `psi_target_layouts::terminal_target_context`, `psi_provider_selections::admit_provider_installation`, `TerminalTargetContext::resolve_entry`.

**Inputs and outputs.** Builds a `CanonicalTerminalArtifact` through `terminal_codec::build_identity_optimization_execution_record` and `CanonicalTerminalArtifact::from_parts` `:147-150`, `:279-282`. Asserts on `Result` values and error substrings.

**How it decides (pinned by the tests).**
- The extent roster passed to `observations.conditions(target, &roster)` must equal exactly the demand's roster, in canonical order with no duplicates; `[]`, `[a]`, `[b,a]`, `[a,b,a]` each fail with a message containing `"canonical extent roster"` `:188-199`. Keyed on `ScalarApplicationId` sequence.
- A closure built for a different `CallingAbiIdentity` key than the observations fails with `"roots differ"` `:226-231`. Keyed on the full `(requirement_identity, declaring_application, operands)` key: the two fixture keys differ only in `declaring_application` (`:22-27`).
- A completed ABI-storage closure `require_product_formation()` is an error `:238` and `admits_machine(.., MachineId(1))` is false `:240-243` (name: "grants no entry or product authority").
- Demand from another artifact fails with `"another Terminal artifact"` `:340-345`; observations for a target different from the demand's fail with `"another selected target"` `:349-359`. Keyed on artifact identity and target string (`"macos_arm64"` vs `"linux_arm64"`).
- A retained false predicate on the helper blocks completion: `discharge_ready` fails with the debt label `:383-384` and `close_scope` fails the same `:386-389`. Keyed on the predicate's `label` text in the assertion (`error.contains("actual ABI extent helper debt")`), a substring match in the test only.
- Entry context: `context.entry(identity, module, TargetMachineCallable::Machine(m))` returns `Ok(m)` for a retained machine, `Err(TerminalIdentityMismatch)` for another artifact's identity `:295-302`, `Err(MissingMachine)` `:304-311`, `Err(NonExecutableEntry)` for a `Boundary` callable `:312-322`. `TerminalTargetContext::resolve_entry(None, ..)` with no context returns the machine unchanged `:324-332`. Both `"macos_arm64"` and `"linux_arm64"` are tried `:284`.

**Cases by shape.** The fixture hard-codes two receivers and three structural types: "one to two" ABI declarations and two array extents (`:22`, `:49-63`). The roster order and the `Receiver<u8>` vs `Receiver<u16>` pair are fixed by the test, not a production list.

**Same job elsewhere.** None in my ranges. `fixture` builds `TerminalModule` and `StructuralTypeDeclaration` by hand like the wire tests (item 1a).

**Unused code.** None (test file).

**Stated limits.** `:139` the string `"actual ABI extent helper debt"` is a label; the test name says "false helper debt". No limit comment. Note that the label "debt" names the word used by the production check, but I did not read it.

**Fail-open risks.** None seen. One thing to record: `target_context_binds_artifact_without_selecting_an_entry_body` `:324-332` pins that `resolve_entry(None, ...)` (no target context) returns `Ok(machine)` without checking the artifact identity; the test treats this as intended but gives no comment (not traced; "open" for the production function, which I did not read).

---

## 4. `source-rust/subsystems/compiler/psi/identities/applications/src/const_value.rs` (lines 701-1018, 318 lines)

**Where it sits.** Inside `#[cfg(test)] mod tests` (opens `:608-609`, grepped). Range starts inside the first static-callable test (the test's name and opening are at `:608-700`, outside my range); the visible part loops over three mutations of a `StaticCallable` atom (`:701-712`), invalid `StaticCallable` values `:713-734`, and a no-application case `:735-749`. Helpers `decode` `:615` and `framed` `:419` (production, not read).

**Purpose.** Test the canonical const-value encoding of `CanonicalConstValue` and its decoder `decode_encoding`, `from_atom`, `from_decoded`, identity, and the bounds on decoding.

**Shape (read).** Tests starting in this range:
- (end of an earlier test) `:701-770`: callable-contract atoms.
- `structural_writer_preserves_existing_canonical_encoding` `:773`
- `canonical_const_atom_round_trips_delimiters_and_unicode` `:786`
- `boolean_literal_spellings_round_trip_through_the_canonical_atom` `:793`
- `semantic_identity_omits_display_and_integer_builder_is_canonical` `:807`
- `canonical_inner_scalars_decode` `:825`
- `canonical_inner_compounds_preserve_order_and_decode_variants` `:851`
- `malformed_tags_arities_and_scalar_spellings_are_rejected` `:893`
- `malformed_framing_and_nested_trailing_bytes_are_rejected` `:918`
- `empty_names_and_duplicate_fields_are_rejected` `:933`
- `declaration_floats_do_not_become_structural_indices` `:949`
- `decoder_enforces_depth_node_and_byte_bounds` `:1001`

Production items referenced (defined in this file, outside my range, not read): `CanonicalConstValue::{new, boolean, from_atom, from_decoded, from_decoded_in_type, decode_encoding, atom, identity, index_identity, scalar_index_identity, is_canonical_static_value}`, `CanonicalConstIdentity::{integer, decode_encoding, decode_declaration_encoding}`, `DecodedCanonicalConstValue::{Boolean, Integer, Record, Array, Variant, StaticCallable, Float}`, `boolean_literal_spelling`, `LandedIntegerType`, and constants `MAX_ENCODING_BYTES = 64*1024` (`:116`), `MAX_DECODED_NODES = 4_096` (`:117`), `MAX_DECODE_DEPTH = 64` (`:118`) (grepped).

**Inputs and outputs.** Strings in a length-prefixed frame (`"integer3:i641:1"` style: tag, then pieces as `<len>:<bytes>`; inferred from `:921-925` and `framed("integer", ["u8","256"])`). Outputs are `Option<DecodedCanonicalConstValue>`; `None` means rejected.

**How it decides (pinned by the tests).**
- `decode_encoding` returns `None` for: unknown tag, wrong arity, `+1`, leading-zero `01`, `-0`, an `i64` literal one past `i64`'s range written as `170141183460469231731687303715884105728` (2^127) `:901-904` (note this is the i128 max + 1, so the limit tested is a 128-bit bound, not an i64 range check), `TRUE` (non-lowercase boolean), extra pieces, empty names, duplicate record fields, non-ASCII length digits, nested trailing bytes `:893-946`.
- Integer of type `u8` with value `256` is refused by `is_canonical_static_value()` and `index_identity()` is `None` `:752`, `:761-762`.
- Static callable: empty `declaration`, or `Some("")` application or requirement, are refused `:713-734`; a missing application and requirement round-trip `:742-749`; changing declaration, application or requirement changes the identity `:701-711`.
- Identity omits the display text: `"2 + 2"` and `"4"` give the same identity `:807-813`.
- Floats: scalar and record/array floats (`float:f32:80000000`, `float:f64:8000000000000000`) are refused by `decode_encoding` and by the identity's `decode_encoding` (structural index) but accepted by `decode_declaration_encoding` `:964-971`. The test name says declaration floats "do not become structural indices". NaN bits (`7fc00000`, `7ff8000000000000`), wrong hex width, uppercase hex, and `f16` are refused by the declaration decoder `:979-997`. Keyed on the bit-pattern spelling.
- Bounds: depth of `MAX_DECODE_DEPTH` nested arrays fails; `MAX_DECODED_NODES` children fails even though total bytes are under `MAX_ENCODING_BYTES` `:1013`; `MAX_ENCODING_BYTES + 1` bytes fails `:1001-1016`.
- `Variant` decode keeps field order and nested array/record/integer types `:851-890`.

**Cases by shape.** The decoder accepts a fixed vocabulary of tags: `integer`, `boolean`, `array`, `record`, `variant`, `static-callable`, `float:` (declaration only) (from the tests at `:893-912`, `:949-977`; the decoder itself not read). Anything else decodes to `None`.

**Same job elsewhere.** "Escape a string and frame it" is also done by the test helpers in `tests.rs` (`escape` `tests.rs:330`, `atom` `:337`, `compound` `:341`) for a different grammar (type identity strings with `\(` escapes), so two escape/format schemes exist in this crate: length-prefixed frames (const values) and backslash escapes with parentheses (type identities). Read. Whether the two are used by the same consumers: not traced.

**Unused code.** Test file portion; none.

**Stated limits.** None quoted in range.

**Fail-open risks.** None seen; all `None` results mean "refuse". One thing to record: `from_atom(&value.atom())` on a float value returns `Some` but `decode_encoding` on the result is `None` `:968-969` (test pins this as intended).

---

## 5. `source-rust/subsystems/compiler/psi/identities/applications/src/tests.rs` (lines 1-733, whole file)

**Purpose.** Tests for the type-identity string grammar: `visit_type_identity_package_owners` walks a type identity string, reports each package-owner digest to a visitor, and refuses malformed or unadmitted text. Also `validate_source_type_description`, `malformed_identity_offset`, `compiler_type_name`, `compiler_type_identity`, and `vocabulary::audit`. Declared in `lib.rs:12` as `mod tests;` (grepped).

**Shape (read).** Three modules:
- `array_extent_identity` `:1-173`: a visitor `Owners` `:6-37` that implements `TypeIdentityPackageOwnerVisitor` with methods `enter`, `leave`, `reserve`, `package_owner`, `embedded_name`. `enter` fails with `ResourceLimitExceeded` at depth 32 `:14-16`. Four tests `:49`, `:68`, `:94`, `:132` cover array extents whose length is a `call(...)`, a `cast(...)`, static arguments, and `generic(...)` arguments.
- `type_identity` `:175-289`: `PackageOwnerCounter` `:181`; tests `compiler_type_name_and_complete_identity_keep_the_published_frames` `:203`, `retired_dynamic_trait_identities_are_rejected` `:215`, `generic_identity_reads_const_index_arguments` `:234`, `source_type_description_reads_an_open_toolchain_application` `:254`, `malformed_identity_offset_names_the_byte_the_grammar_stopped_on` `:268`.
- `type_identity_vocabulary` `:291-733`: visitor `Owners` with `seen`, `embedded`, `refused`, `depth` `:297-328`; builder helpers `escape`, `atom`, `compound`, `owner`, `nominal`, `named`, `constant`, `index` `:330-363`; `visit` `:365`, `reads_owners` `:372`, `refuses` `:382`. Tests: `every_spelling_of_the_vocabulary_is_unique_and_lexes_whole` `:387` (calls `vocabulary::audit()`), `selected_algebra_reads_the_provider_operand_and_visits_every_owner` `:392`, `derived_declarations_expose_their_package_owners` `:447`, `every_index_form_reads_with_the_owners_it_carries` `:504`, `refusals_name_why_no_retained_policy_holds_the_production` `:671`, `only_binder_replacements_reach_the_consumer` `:703`, `retired_scalar_interval_identity_tags_are_refused` `:721`.

**Inputs and outputs.** Inputs: identity strings built with `format!`. Output: the visitor's `seen` list and a `Result<(), TypeIdentityVisitError>`. Error variants asserted: `UnknownPackage`, `MalformedIdentity`, `ResourceLimitExceeded` (defined but only raised by the test's own visitor), `Unadmitted(Unadmitted::InvalidIdentity)`, `Unadmitted(Unadmitted::ApplicationScope)`.

**How it decides (pinned by the tests).**
- The decision is made on the SPELLING of the identity string. The type identity is a text grammar (`nominal(package-owner(32:<64 hex>),path(size))` `:39`, escapes `\(` `\)` `\,` `\\` `:40-44`). The visitor receives package owners as 32-byte digests parsed from hex, and the visit order is the textual order of appearance, except `match` and `destructure` where the order is as the test lists it (`:665` expects `[7, 8, 1, 2, 3, 4, 5, 6, 9]`; the scrutinee, then the wildcard arm, then pattern pieces, then the arm body).
- Unknown package owner: the visitor's `package_owner` hook returns `UnknownPackage` and the visit stops with that error `:61-66`, `:318-320`, `:478-479`. So the grammar reader does not decide package admission; the consumer's hook does. In `derived_declarations...` the visit stops at the first refused owner `[6;32],[7;32]` `:479`, so owner 8 is not reported.
- `Wrapping` cast policy: `cast(..., domain(Wrapping))` is read; replacing `Wrapping` with `Invented` gives `MalformedIdentity` `:84-89`. So the set of cast domain names is a fixed list in the production reader (not read; inferred from this test).
- `hidden-owner(7)` in place of `integer(7)` gives `MalformedIdentity` `:125-129`, `:161-165`: operands must be one of the known const forms.
- Retired spellings are refused: `dynamic-trait(...)` `:215-229`; `constrained(...range(...exclusive-end...))` and a bare `exclusive-end` `:721-731`; `invalid-derived-origin` gives `Unadmitted(InvalidIdentity)` `:491-500`.
- Open-scope text (`toolchain-owner`, `unresolved-owner`, `region(...)`, `elided-region`, `unresolved-region`) is refused by the owner visitor (`:261`, `:677-682`) with `ApplicationScope`, but `validate_source_type_description` accepts `toolchain-owner` and refuses `unresolved-owner` `:259-264`. So there are two entry points with different admission of open text: the owner visitor for sealed identities and `validate_source_type_description` for source-level text.
- `malformed_identity_offset` returns the byte offset where the grammar stopped: `None` for a valid string, `Some(0)` for `""` or `unknown(x)`, `Some(4)` for `ref(unknown(x))`, etc. `:268-288`.
- Visitor `embedded_name` is called only for `named(name(type-parameter:N))` binder replacements; `named(name(integer\(5\)))`, `name\(x\)` and `generic\(name\(x\)\)` are refused `:703-718`.

**Cases by shape.** A closed list of index forms is exercised at `:515-608`: `indexed`, `member`, `borrow`, `borrow-mut`, `borrow-write`, `range-inclusive`, `range-exclusive`, `array-literal` (0 and 2 items), `type-expression`, `zero-value`, `atomic`, `struct-literal`, `cast`, `call`, `saved-subject`, `undetermined-nan`, `float`, `boolean`, `case_membership`. Also `match` with `wildcard` / pattern arms, `destructure` with `fields`/`rest`/guard/`proof-selectors` `:613-667`. Note `case_membership` uses an underscore while all others use hyphens `:602`; this is the real spelling in the test.

**Same job elsewhere.** Two near-identical `Owners` visitor structs `:6-37` and `:297-328` and a third `PackageOwnerCounter` `:181-200`, each implementing the five-method trait (read). Also `nominal`-escape code is duplicated: the escape chain `.replace('\\',...).replace('(',...)...` appears at `:40-44`, `:99-103`, `:135-139` and as `escape` at `:330-335`. A production `escape` for the grammar is not in this file; I did not locate it.

**Unused code.** The `ResourceLimitExceeded` variant is raised only by this test's visitor (`:15`); whether production visitors raise it: not grepped.

**Stated limits.** `:273-274` comment: "The comma introduces a region operand. The ordinary reader stops at the unsupported operand after consuming the separator." `:671` test name: "refusals name why no retained policy holds the production".

**Fail-open risks.** None seen in the tests. Observation: `owners.seen` is asserted exactly for each form, so a form that the reader silently skipped would fail the test `reads_owners`; this protects against silent drops for the listed forms but not for forms absent from the list.

---

## Coverage

Read in full (all lines of each range):
- `semantic_module.rs` 2901-4184 (1284), 4360-5296 (937), 5342-5446 (105)
- `structural_wire.rs` 386-559 (174), 1556-1896 (341)
- `scalar-applications/completion_tests.rs` 1-408 (408)
- `applications/src/const_value.rs` 701-1018 (318)
- `applications/src/tests.rs` 1-733 (733)

Total read in full: 4300 lines. Skimmed: 0. Not read: none of my ranges. Outside my ranges and not read: `semantic_module.rs` 1-2900, 4185-4359 (includes production `encode_module` etc.), 5297-5341, 5447-end; `structural_wire.rs` 1-385, 560-1555, 1897-end; `const_value.rs` 1-700 (includes the production decoder); `type_identity.rs` (the production grammar). Production code under test was not read, so statements about what the production code does are inferred from the tests and marked as such.

## Five things a maintainer most needs to know about this unit

1. All 4300 lines are tests. They pin that `decode_module` (not `encode_module`) is where receiving validation lives: `encode_module` succeeds on forged retained input and `decode_module` refuses it (`semantic_module.rs:3280-3289`, `:4879-4883`, `:5078-5082`). The one exception recorded is an opaque type with a `quotient_carrier`, refused at encode (`:5287-5293`). `encode_raw` is used to bypass any encode-side check.
2. Most refusal tests assert a reason by substring or exact `&'static str` (for example `"boundary result leaf has no unique declared lifetime"` `:4635`), so renaming a refusal message breaks tests. Two tests assert only the error variant with `_` (`:5390`, `:5435`, `:5443`) and the truncation tests assert only `is_err()` (`structural_wire.rs:1696-1698`).
3. The x86 descriptor-table, task-register and segment operations have hard-coded sizes in the fixtures (5 entries, 104-byte shared view, 10-byte scratch, `u16` selector, `u8` activate result, `u16` read result; `semantic_module.rs:4967-4973`, `:5028-5065`). The refusal substrings (`:5100-5127`) show the validator checks exact extents and widths.
4. Old wire format markers are refused by number before payload reading (164, 183 in `structural_wire.rs:1758`, 167 `:1800`, 246, 253, 256, 257 in `semantic_module.rs`, 275, 276 `structural_wire.rs:556`). The tests do not name a current marker; the current one (`FORMAT_MARKER = 290` per the crate map, not read by me) is not checked here.
5. Type identities are a text grammar and `tests.rs` pins decisions made on its spelling: tag names (`dynamic-trait` retired, `exclusive-end` retired, `case_membership` with underscore), the `Wrapping` cast domain name, and escape rules. Package admission is delegated to the visitor's `package_owner` hook (`tests.rs:26-33`, `:316-322`), so the grammar reader itself accepts any well-formed 32-byte owner.

## What I am unsure about

- Where `structural_wire.rs:386` sits in the module nesting; I did not read the opening line (outside my range).
- Whether the assertion after `structural_wire.rs:559` checks `UnsupportedFormatMarker`; the range ends mid-statement.
- Whether `ScalarApplicationObservations`, `CallingAbiStorageDemand` and `ScalarApplicationClosure` are used anywhere except these tests and native stages; not grepped.
- Whether `ResourceLimitExceeded` is raised by any production visitor.
- The meaning of "origin authorized=false / issuer=None": inferred from the test text, since the validator was not read.
- The `170141183460469231731687303715884105728` bound at `const_value.rs:903`: I read it as 2^127 (an i128 boundary) from its digits; the decoder's real rule was not read.
