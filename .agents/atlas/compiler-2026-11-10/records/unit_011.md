# Unit 11 notes (unit number: 11; reader 11 of 113)

Files covered (67), all under `omega-rust/omega/compilation/backend/`:
plans/effects/src/{provider_evidence/providers/provider_plan.rs, provider_plan/{digest_encoder,evaluated_bindings,retained_inputs,service_schemas}.rs, provider_grants.rs, source_guards.rs};
plans/post-handoff-writer-plan/src/{lib,tests}.rs; plans/program-entry-plan/src/{entry_signature,lib,optimized_semantic_entry,optimized_semantic_wrapper,post_handoff_writer,program_entry_physical,selected_entry,source_guards}.rs;
register-environment/src/{catalog,lib,tests}.rs + tests/{abi_call_clobbers,supported_targets}.rs + tests/abi_call_clobbers/{arithmetic,branch,call,cleanup,memory,provenance}_effects.rs, register_calls.rs, return_effects.rs;
register-model/src/{constraint_catalog,identities,lib,preservation_storage,register_vocabulary,reservation_profiles,test_support,tests}.rs + tests/{constraint_catalog,declared_identities}.rs;
runtime/executable-installation/src/{artifacts,authority_digests,installation,lib,placement,post_handoff_writer}.rs; runtime/installation-evidence/src/{installation_environment,lib}.rs;
runtime/runtime-abi/src/{borrowed_storage,descriptor_io,host_numeric,lib,network_address,scalar_reference,static_interrupts,windows_provider}.rs;
runtime/task-plans/src/{activation,identities,lib,source_guards,stack_composition}.rs; target/src/{compatibility_fingerprint,component_locator,elf_loader,explicit_target_set,foreign_locator,lib}.rs.

(Path prefix `E` below = `omega-rust/omega/compilation/backend/plans/effects/src/provider_evidence/providers/`.)

---

## E provider_plan.rs (963 lines) — read in full

**Purpose.** Defines `ProviderPlan`: one value per (provider type, service schema, target) that lists, for each method of a service/boundary slot, how it is bound (foreign import, syscall, intrinsic, vtable/table slot, or a checked adapter machine). Also computes its two identities (SHA-256 `ProviderPlanDigest` and a 64-bit FNV "report fingerprint"), a selection ordering, and schema-coverage validation. Header comment: "Construction is free, and no author-selected trust field exists" (provider_plan.rs:1-8).

**Shape.** `enum ProviderBinding` :38 (Import{evaluated}, Syscall{number:i64}, CompilerIntrinsic{machine:String}, VtableSlot{index}, VtableField/TableFunction{table,field,storage:Result<TableFieldStorage,String>}, CheckedAdapter{machine_identity,machine_package_identity}); `TableFieldStorage` :68; `ProviderPlanRow` :74 (method String, requirement_identity, requirement_lifetime_partition Vec<u32>, admitted_same_stack_bytes, binding); `ProviderPlan` :83 (name, provider_type, provider_application id, target String, `schema: ServiceSchema`, `identities: ProviderIdentityTable`, rows, origin_package(_identity), `digest_memo`); `ProviderPlanDigestMemo` :98 (three OnceLocks; Clone returns empty, PartialEq always true :104-116). Entry fns: `identity_digest` :204, `identity_preimage` :225, `encode_identity` :230, `report_fingerprint` :284 / `compute_report_fingerprint` :291, `selection_order` :479, `validate_against_schema` :499, `validate_candidate_against_schema` :520, `covers_schema` :948. Re-exports at :10-33.

**Inputs/outputs.** Reads own fields + `ServiceSchema` (service_schemas.rs). Outputs digest (SHA-256 via `ProviderPlanDigestEncoder`), `Vec<u8>` preimage, `u64` FNV fingerprint (`semantic_vocabulary::Fnv1a`, :473), `Vec<String>` error lists.

**How it decides.**
- Equality `PartialEq for ProviderPlan` :142 compares identity *text* (`identity_text`) not ids, so two plans with different interning tables compare equal; ignores `digest_memo` and `identities` table. Row equality `eq_in` :165 compares CheckedAdapter machine identity by text, other bindings by derived `==`, but `requirement_identity` is compared as raw `ProviderIdentityId` (:172) against possibly different tables (read; each plan has its own `identities` table, see :90) — so the same requirement interned in different order would compare unequal. (inferred from reading; not traced to a caller).
- Row-to-method binding keyed on `row.method == method.name` AND requirement identity text resolved in the declaration table (`row_binds_method`, service_schemas.rs:275-288).
- Fingerprint/sort keys use method name + requirement-identity text; fingerprint is built from a `format!` string with `{:?}` of `method.completion` (:331), `{binding:?}` Debug of any binding not Import/CompilerIntrinsic/CheckedAdapter (:462), `{:?}` of `row.requirement_lifetime_partition` (:466), `{:?}` of `calling.abi` (:421). So the fingerprint depends on Debug spelling of those values (read).
- `selection_order` :479 keys on name, origin_package_identity, provider_type_package_identity, trait_package_identity, then `report_fingerprint()` (the 64-bit FNV).
- Validation (:520-946) is keyed on identity validity (`is_valid()`), emptiness of strings, strict increasing orders of `entry_claims`, `result_claims`, `service_reach`, `synchronous_invocations`; Import row's `locator.target().target_name()` string-compared to `self.target` (:831); Syscall number must fit u32 (:842); CheckedAdapter requires non-empty `provider_type` and `machine_package_identity == origin_package_identity` (:893, :903).

**Cases by shape.**
- Progress premise establishment routes: only `BoundaryRequirement` accepted, anything else is an error "has a non-boundary establishment route" (:628-636).
- Entry/result claims must be `CarryPolicy::STRICT` ("not born-strict", :696, :773).
- `ServiceProgressSubject` has only `Parameter(index)` (:355, :653); `ServiceEntryAuthorityFlow` has only `Accepts` (digest_encoder.rs:181).
- `BoundaryCompletion::External` with `has_result` rejected (:730).

**Same job elsewhere.**
- Package-identity hex rendering is written out three times in this file: `push_package_identity` :124, inline in `compute_report_fingerprint` :310-318 (same logic), and `format!("{byte:02x}")` for receipt at :435-440.
- Calling identity ordering: `calling_for_requirement` sorts by `abi` (service_schemas.rs:96); the digest encoders re-sort methods/claims/premises independently in provider_plan.rs:243, :261, :319, :344, :381, :411 and digest_encoder.rs:163, :199, :213 and service_schemas.rs:176. Same sort-key logic (method name then requirement text; premise profile/subject/projections) appears in both fingerprint (:344-350) and digest_encoder (:213-219) and service_schemas digest (:176-182) — duplicate code paths for the "canonical order" job (read).
- `validate_candidate_against_schema` count-of-binding checks (:912-927) duplicate `covers_schema` :948-962 (same predicate; also `validate_against_schema` :501-516 computes a third time the count==0 case).

**Unused / one-caller.** Greps pending (see below).

**Stated limits.** None beyond header. "retired provider binding tag" appears in retained_inputs.rs:174.

**Fail-open risks.**
- `PartialEq for ProviderPlanDigestMemo` returns `true` always (:110-114): intentional so memo does not affect plan equality — "stated" only by structure (no comment). Wrong plan would not get through since the memo is derived data; traced: memo is filled from `self` only (`get_or_init`).
- `ProviderPlanDigestMemo::clone` returns empty memo (:104-107) so a cloned-then-mutated plan (pub fields) recomputes; but a plan mutated AFTER digest first computed (fields are `pub`) keeps a stale memo (:204-209, :284-289). Open: any caller mutating a plan after calling `identity_digest()`/`report_fingerprint()` gets stale values. (inferred; I did not trace callers.)
- `validate_candidate_against_schema` on `VtableSlot`/`Syscall` etc. checks only range; `ProviderBinding::VtableField/TableFunction`'s `storage: Err(String)` is accepted by validation (no check at :867-882) and is hashed as a refusal (digest_encoder.rs:348-357). Stated by test `table_field_correspondence_is_committed_and_received_in_full` (retained_inputs.rs:392) which treats `Err("field is not addr")` as valid. So a table binding with a refused storage still validates; whether a later stage rejects it is not traced (open).

---

## E provider_plan/digest_encoder.rs (358 lines) — read in full

**Purpose.** SHA-256 domain-separated framed encoder for provider plans and service schemas; also the `DigestFeed` trait (raw/byte/bool/len/u64/i64/bytes/string/strings).

**Shape.** `trait DigestFeed` :15; `PROVIDER_PLAN_DOMAIN = b"omega.provider-plan.sha256.v7\0"` :55; `struct ProviderPlanDigestEncoder(Sha256, Option<Vec<u8>>)` :57 (second field = optional preimage capture); ctors `for_provider_plan` :69, `for_provider_plan_preimage` :75, `for_service_schema` :85 (domain `...service-schema.sha256.v6\0` :87); `receiver_applications` :95, `package_identity` :116, `carry_policy` :129, `service_method` :148, `provider_binding` :263, `table_field_storage` :348.

**Inputs/outputs.** Reads `ServiceMethod`, `ProviderBinding`, identity tables; writes into Sha256 and optionally a Vec<u8>. Identities are always fed as text (header :4-5).

**How it decides.** Binding tags: Import=0, Syscall=2, CompilerIntrinsic=3, VtableSlot=4, VtableField=5, TableFunction=6, CheckedAdapter=7 (tag 1 is skipped; retired, see retained_inputs.rs:174). Locator candidate tags 0..3 (:274-301). `operand.kind as u8` (:255) feeds an enum discriminant.

**Cases by shape.** `match` on `ServiceEntryAuthorityFlow::Accepts` only (:181-183); `BoundaryCompletion` NormalReturn/External(ProcessExit) only (:186-191); `ForeignLocatorCandidate` four variants (PeByName, PeByOrdinal, ElfVersioned, MachODylibSymbol).

**Same job elsewhere.** `crate::provider_evidence::digest_encoder::DomainDigestEncoder` is used by evaluated_bindings.rs:163 — another domain-separated SHA-256 encoder (suspected duplicate of this one; I read only its use site). `DigestFeed` is `pub(crate)` and re-exported at provider_plan.rs:11.

**Fail-open.** None seen. `finish_preimage` `expect("preimage encoder")` panics if called on a non-preimage encoder (:82) — panic not accept.

---

## E provider_plan/evaluated_bindings.rs (543 lines) — read in full

**Purpose.** Receipts and carriers for foreign bindings (imports, syscalls) that were produced by running a Build-time evaluator. Each carrier verifies its receipt's `locator_identity_digest` equals the digest of the normalized locator/syscall before it can be constructed.

**Shape.** `macro evaluated_binding_digest!` :14 → three non-zero 32-byte newtypes (ProducerClosure, Evaluation, Materialization digests :35-37); `EvaluatedBindingUsage` :40 (10 numeric fields; `from_evaluator` :56 rejects zero schema/step marker only); `BuildNativeBindingOrigin` :130 (+`validate` :140, `feed` :160); `EvaluatedBindingReceipt` :176 (`from_evaluation` :193, `with_build_origin` :240, `reintern_producer` :261); `receipt_identity_digest` :328; `EvaluatedForeignImport` :378 (`from_retained_evidence` :385); `EvaluatedForeignSyscall` :408 (`from_retained_evidence` :417); `evaluated_syscall_identity_digest` :463 → delegates to `target::`; `EvaluatedNativeBinding` :471 {Import, Syscall} with `provider_binding` :519.

**How it decides.** Import: `receipt.locator_identity_digest() == locator.identity_digest()` (:389). Syscall: target must be `LinuxArm64 | LinuxX64` only (:422), number must fit u32 (:428), digest equality (:431). Origin validation: non-empty requirement text, nonzero digests/schedules (:144-155). `with_build_origin` requires `build_origin.is_none() && producer_package.is_some()` (:246).

**Cases by shape.** "if !matches!(target, TargetProfile::LinuxArm64 | TargetProfile::LinuxX64)" → error "not applicable" (:422); Windows/macOS syscalls refused.

**Same job elsewhere.** Receipt digests use `DomainDigestEncoder` (:339) vs `ProviderPlanDigestEncoder` in digest_encoder.rs — two SHA-256 domain encoders in one crate.

**Fail-open.** `EvaluatedBindingUsage::from_evaluator` accepts any values for work_units/work_budget/peak etc. (no check `work_units <= work_budget`) — :68-73 only nonzero markers; traced: no other check in this file. `ProviderBinding::Syscall` (provider_plan.rs:841) re-checks u32 only, not the target; target gating for syscalls exists only on the evaluated path (:422). A `ProviderBinding::Syscall` built directly (pub enum, pub fields) on e.g. windows target would pass `validate_candidate_against_schema` (which only checks u32 range) — open (not traced to later stage).

---

## E provider_plan/retained_inputs.rs (776 lines; code 1-335, tests 337-776) — read in full

**Purpose.** `ProviderPlanInput::read` parses the v7 provider-plan preimage bytes without constructing a `ProviderPlan`, checks the whole-buffer SHA-256 against an expected digest, validates canonical ordering and bounds, and exposes provider/target/origin package plus rows (requirement text + optional checked-adapter coordinates).

**Shape.** `ProviderPlanInput` :18, `ProviderPlanInputRow` :27, `read` :33, private `Reader` :219 (take, number, count, blob, string, nonempty_string, strings, tag, package, carry, calling_applications :293). Limits `MAX_BYTES = 8 MiB`, `MAX_ROWS = 4096` :14-15 (`count()` applies the 4096 bound to every length, including string lists and blob counts, :244-250; `blob()` length itself is not bounded other than by remaining bytes).

**How it decides.** Digest equality; domain prefix bytes; strictly increasing keys (method name, requirement), (position, carrier, domain), (profile, parameter, projections), (route kind, requirement); row keys (name, requirement, partition) strictly increasing; row must be in `methods` list; `rows.len() == methods.len()` (:202); no trailing bytes (:206). Tag bound checks via `tag(max)`.

**Cases by shape.** Binding tag match (:150-196): 0 import (locator tags 0|3 two blobs; 1 blob+2 raw bytes ordinal; 2 three blobs), 1 "retired provider binding tag" error, 2|4 number, 3 string, 5|6 table field, 7 checked adapter. Progress subject must be tag 1 (:106). Hard-coded positional skip of fixed field sequences (`reader.tag(1)` ×N) mirrors `service_method` in digest_encoder.rs:148-261 — layout is duplicated by position, not shared.

**Same job elsewhere.** The layout parsed here is the inverse of `encode_identity` (provider_plan.rs:230) + `service_method` (digest_encoder.rs:148). Two hand-synchronized copies.

**Fail-open.** Fields are read and discarded without semantic validation (e.g. `reader.carry()` accepts any 4 tags 0/1; the claim's carry policy value is not checked to be STRICT here, :80-82; `reader.tag(1)?` for completion/has_result/etc. accept either). The digest commits to them so a modified value changes the digest; but a *validly signed/committed* preimage from a plan that had non-strict claims would be accepted — "traced": producer `validate_candidate_against_schema` rejects non-strict (provider_plan.rs:696) but this reader does not re-check (consumer trusts commitment). Also the `Import` row `reader.blob()?.len() != 32` check is the only width check (:169). Package tag: any tag value != 0 treated as present (`package()` :277-284 uses `_` arm after tag(1) bound, so only 0/1).
Row `requirement` uniqueness checked via `rows.iter().any(previous.requirement == requirement)` (:141-143), quadratic in rows (≤4096).

**Tests** (:337-776): selected_plan fixture; tests cover roundtrip, truncation at every prefix, domain v4/v5/v6/v8 rejection, trailing bytes, calling application order.

---

## E provider_plan/service_schemas.rs (485 lines; tests :329-485) — read in full

**Purpose.** Pairs the target-neutral `ServiceSurface` (from `language_semantics::service_surface`) with per-target calling-plan applications, computes `ServiceSchemaDigest`, and implements receiver/schema compatibility checks.

**Shape.** `BoundaryCallingPlanCommitment([u8;32])` :18; `ProviderPlanDigest`, `ServiceSchemaDigest` via `evidence_digest!` macro :34-42; `ServiceSchema{declaration:Arc<ServiceSurface>, calling_applications}` :51; `ServiceCallingPlan{report_fingerprint:u64, commitment}` :57; `ServiceCallingApplication{requirement, abi:CallingAbiIdentity, plan}` :63; methods `calling_application` :77 (linear find by abi), `calling_for_requirement` :87 (filter + sort), `calling_matches` :100, `validate_calling_applications` :138, `identity_digest` :163, `declaration_identity_digest` :167, `supplies_receiver_schema` :220, `receiver_application_index` :224, `shares_receiver_application` :232, `receiver_contract_matches` :242, `row_binds_method` :275, `method_for_row` :290. Record impls for storage :302-327.

**How it decides.** Receiver supply: root receiver application of the demanded schema must `eq_in` some application of the supplier, and every requirement listed must have a method that `eq_in` a method of the other schema AND matching calling-plan (abi + plan) (:242-272). Calling application validation: requirement valid + exists in methods + abi unique + abi.validate + abi requirement text equals requirement identity text + nonzero fingerprint and commitment (:138-160).

**Same job elsewhere.** `PartialEq for ServiceSchema` :199 — compares declarations, lengths of calling roster, and per-method calling for zip'd methods; methods zip assumes same order (not checked for requirement text, only `calling_matches` between positionally paired methods).

**Fail-open.** `PartialEq` :199-215: `self.declaration == other.declaration` is compared first (so methods are equal in order via derived/impl PartialEq in language_semantics, not read). Calling equality only checks per-method positional pairs; two schemas whose calling rosters have the same length but where applications attach to a requirement id not in any method would be ignored in the comparison — but `validate_calling_applications` rejects such. Traced: only when validation is skipped. `receiver_application_index` returns `None` when the demanded schema has no receiver applications (`first()?` :225), i.e. fail-closed.

---

## E provider_grants.rs (188 lines) — read in full

**Purpose.** Resolves root "trust grants" (strings authored in the root) against candidate provider plans and the selected plan set, binding each to exactly one plan by plan name or by provider slot (trait name text).

**Shape.** `ProviderGrantSelectorKind{PlanName,ProviderSlot}` :15; `ResolvedSelectedProviderGrant` :25 (+`commitment` :41 formatting `"provider plan: {name}"`/`"provider slot: {text}"`, `replays_selected_plan` :57); `ResolvedAuthoredSelectedProviderGrant` :34; `resolve_selected_provider_grants` :64; `resolve_authored_selected_provider_grants` :166.

**How it decides.** Keyed on source spelling: `plan.name == *grant` and `identity_text(trait_name) == grant` (:73, :77-79, :93, :99-101). `selected_subjects_coincide` :20 is `plan == slot_plan` (the text-based PartialEq). Errors for ambiguity, unselected, distinct subjects, candidate-count != 1.

**Fail-open.** :80-82 `if plan_name_candidates.is_empty() && !slot_is_known { continue; }` — a grant string that names neither a candidate plan nor a slot is silently skipped (no diagnostic, no resolved grant). What passes: a misspelled/stale grant is ignored. "open" — I did not trace whether a caller validates that every root grant resolved (grep below).

---

## E source_guards.rs (116 lines) — read in full

**Purpose.** Test-only source audit guarding that provider types hold `ProviderIdentityId` handles rather than `String` identity fields. Uses `source_audit::{source_lines, violations}`. Tests: `interned_fields_hold_provider_identity_ids` :28, `provider_types_carry_no_text_trait_or_machine_identities` :51, `service_schemas_carry_no_text_requirement_identities` :69, `crate_carries_no_string_identity_fields` :95 (ALLOWED exception: `executable_scopes.rs` `Service { identity: String, name: String },` :89-92). Keyed on source text of fields. Note: `ProviderBinding::CompilerIntrinsic{machine:String}` and VtableField `table:String` are not covered by the needles (only `trait_name`, `machine_identity`, `requirement_identity`, and `identity: String` shapes).

---

## plans/post-handoff-writer-plan/src/lib.rs (575 lines) — read in full

**Purpose.** Turns a `layout_plans::SymbolicMaterializationPlan` into a post-handoff writer plan (list of bit-field writes into a destination buffer), then lowers it to an address-free reusable "fragment" (static geometry) plus "invocation" (source slots, placement, stored-integer-fit evidence) that providers replay after control leaves the loader.

**Shape.** `derive_post_handoff_writer` :37; `PostHandoffWriterSource{Resolved(u64),Resolve(RelocationTarget)}` :72; consts `POST_HANDOFF_WRITER_CONTEXT_ABI_V1 = 0x5048_5752_4954_5231` :77, `..SOURCE_SLOTS_OFFSET=8`, `..SOURCE_SLOT_WIDTH=8` :78-79; `post_handoff_writer_context_byte_len` :81; `GeneratedPostHandoffWriterStep` :88; `GeneratedPostHandoffWriterFragmentPlan` :98 (pub(crate) fields, FNV `report_fingerprint`); `PostHandoffWriterSourceSlot` :134; `PostHandoffWriterFitConstraint` :140; `PostHandoffWriterInvocationPlan` :154 (+`validate_destination` :182, `validate_structure` :197, `validate_source_values` :312); `PostHandoffWriterStep` :353; `PostHandoffWriterPlan` :359 (+`lower_reusable_fragment` :367); `generated_post_handoff_writer_report_fingerprint` :466 (FNV-1a, domain bytes `omega.post-handoff-writer.v1`, `.max(1)`); free `validate_step` :497; `test_support::single_address_writer` :552 behind feature `test-support`.

**Inputs/outputs.** Reads `layout_plans` types and validators (`validate_fragment`, `validate_write`, `validate_stored_integer_fit*`). Outputs plans or `MaterializationDiagnostic(String)`.

**How it decides.** Action kind: `ResolvedWrite` → `Resolved`, `RuntimeWriter` → `Resolve(target)`, `NativePointerRelocation` → error "loader-native relocation cannot enter a post-handoff writer program" (:54-58). Source slots are one per distinct `RelocationTarget` (position search :378-381); same target with a different `source` across steps → error "inconsistent invocation values" (:382). `validate_structure` checks ABI constant, nonempty, alignment nonzero, slot-count agreement, distinct targets, `Resolve(target)` matches slot target, canonical first-occurrence slot order, no unused slot, each fit constraint binds a step with container width == stored width == step width, destination_lsb == 0, source_lsb == 0 (:282-289), and finally recomputed fingerprint equality (:298-308).

**Cases by shape.** Fit-constraint binding accepts only a whole-container step ("step.width == stored_width_bits && destination_lsb == 0 && source_lsb == 0", :283-289). Free `validate_step` :510 accepts container widths `8 | 16 | 32 | 64` only.

**Same job elsewhere.** `validate_step` (:497-543) checks container width/bit range/bounds, the same job as `layout_plans::validate_fragment` called in `validate_structure` :244 — two bit-range validators (read, not compared against layout_plans). grep for `validate_step` callers below.

**Fail-open.** `validate_source_values` for `Resolve` sources accepts any supplied value (only `Resolved` is compared, :325-332) — stated by design (resolver fills at runtime); fit constraints are still checked (:334-347). `validate_step` sets `*used = true` BEFORE any other check (:509) so a rejected step still marks the slot used (`used_slots` is caller-owned) — open (callers not traced; see grep).

## plans/post-handoff-writer-plan/src/tests.rs (385 lines) — read in full
Tests for derive/lower/validate: shared slot across three fragments of one target (:30-93), inconsistent values (:96-131), destination length/phase (:134-172), preresolved value substitution (:175-194), tamper of ABI/slot/fingerprint (:197-243), empty plan rejected (:246-267), whole pointer → loader-native (:270-310), numbered renamed fields rejoin by identity (:313-349), fit evidence (:352-385). Uses `layout_plans::test_support::{entry, split_layout, stored_integer_layout}`.

---

## plans/program-entry-plan/src/entry_signature.rs (398 lines) — read in full

**Purpose.** Captures the source-independent signature of a selected `ProgramEntry` declaration (target slot, machine/state names, normalized callable identity, visible parameters with `Extent` layout) and computes a durable 32-byte `ProgramEntrySignatureIdentity`.

**Shape.** `SOURCE_SIGNATURE_SCHEMA = b"omega.program-entry.source-signature.v4\0"` :10; `ProgramEntrySignatureIdentity` :13; `ProgramEntryResultSignature{Unit}` :26; `ProgramEntryExtentValueLayout` :58 (+`from_checked_record` :72, `validate_capture` :98); `ProgramEntryVisibleParameterSignature` :118; `SelectedProgramEntrySignature` :159 (`from_checked_declaration` :171, `identity` :226, `validate_capture` :313, `visible_parameter` ctor :339); helper `push_*` fns :360-398.

**How it decides / Cases by shape.** Extent must be exactly `{base: addr@0, length: u64@8}` with total shape integer(16,8) — error text "selected source Extent must be the exact {base: addr@0, length: u64@8} x86-64 record" (:98-111). Signature capture accepts only `result == Unit` (the enum has one variant :26-28), `target_slot == owner.program_entry_slot()`, non-empty machine/state, present callable identity; every parameter must be non-const, non-mutable, power-of-two alignment, nonzero size, and an Extent layout whose shape equals `value_shape` (:313-337). `push_shape` has `debug_assert!` that shape class is Integer (:392) — only in debug builds.

**Inputs/outputs.** Reads `target::ProgramEntrySlotDeclaration` fields; writes canonical bytes then hashes with `crate::digest_encoder::DomainDigestEncoder::new(b"")` (:307) — the schema string is hashed as bytes inside `canonical`, domain is empty.

**Fail-open.** `identity()` uses `.unwrap_or_default()` when a callable/type identity text is missing (:265-267, :286-288) → an absent identity hashes as an empty string. `validate_capture` rejects absent identities via `is_absent()` (:317, :323) at construction, but fields of `identities` table lookups at identity time could still be `None` if the table does not contain the index; traced: constructor does not check the index resolves in `identities` (only `is_absent()`), so an out-of-range index would hash as "" — open.

## plans/program-entry-plan/src/lib.rs (71 lines) — read in full
Crate root; re-exports (optimized_semantic_entry/wrapper, `installed-writer` feature-gated post_handoff_writer :28-29, :49-54, program_entry_physical, selected_entry); `pub(crate) use optimization_core::{codec_bytes, digest_encoder}` :23-24. `EFI_STATUS_ERROR_BIT = 1<<63` :67 and `uefi_os_handoff_exhaustion_requires_error` :69 (checks bit 63 of status).

## plans/program-entry-plan/src/selected_entry.rs (150 lines) — read in full

**Purpose.** Binds the target-owned `ProgramStorageEntry::enter` slot to a service schema and requirement identity (`SelectedProgramStorageEntryPlan`), optionally attaching a physical contract.

**Shape.** consts `PROGRAM_STORAGE_ENTRY_OWNER = "ProgramStorageEntry"`, `..._METHOD = "enter"` :9-10 (same two constants repeated in optimized_semantic_entry.rs:22-23); `ProgramStorageEntryDiagnostic(String)` :13; `ProgramStorageEntryRootRole{Image,InitialStorage}` :18; `SelectedProgramStorageEntryPlan` :24; `from_target_slot` :33; `with_physical_contract` :117.

**How it decides.** Keyed on `slot == slot.owner.program_entry_slot()` (struct equality of target-declared slot), `slot.semantic_arrival_requirement == format!("ProgramStorageEntry::enter")` (string compare :49-51), boundary schema name compared as text to `schema.identity_text(trait_name)` (:65), exactly one method whose requirement identity id matches (:78-93), owner/name strings compared (:94-95).

**Cases by shape.** `ProgramStorageApplication` requires `ImageAndInitialStorage` params; `HostedApplication` requires `None` (:38-46) — both slot schemas allowed here although the method is always "ProgramStorageEntry::enter".

**Fail-open.** None seen.

## plans/program-entry-plan/src/source_guards.rs (52 lines) — read in full
Test-only (`#![cfg(test)]`): audit that crate has no `identity: String` etc. fields, with an ALLOWED list :20-31. Two allow entries name `selected_entry/service_establishment.rs` (:24-28) — the unit lists no such file in this crate; whether it exists is grepped below.

## plans/program-entry-plan/src/optimized_semantic_entry.rs (548 lines) — read in full

**Purpose.** Validates and mints `OptimizedProgramStorageSemanticEntryContract`: checks the selected UEFI `ProgramStorageEntry::enter` slot, source Extent roots, the semantic calling-plan application and the paired physical plan are mutually consistent.

**Shape.** `bind_optimized_program_storage_semantic_entry_contract` :27; `OptimizedProgramStorageSemanticCallingApplication` :39; `OptimizedProgramStoragePhysicalEntryDisposition{PlannedNotInvokedV1}` :81; `OptimizedProgramStorageSemanticRoot` :86; `OptimizedProgramStorageSemanticEntryContract` :128 (roots `[_;2]`); private `validate` :205, `selected_method` :313, `validate_method` :339, `validate_claim` :407, `validate_source_root` :435, `construct` :463, `semantic_root` :519. `ValidatedSemanticEntryInputs.requirement_identity` is declared `pub(super)` inside a private struct :199.

**How it decides.** Hard-wired to UEFI x86-64: `target != NativeTarget::uefi_x64() || slot != TargetProfile::UefiX64.program_entry_slot() ... MicrosoftX64` (:212-219). Exactly two source roots (Image then InitialStorage, :231-241), each exact by-value Extent (16 bytes, 8 align, fields at 0 and 8, non-const, non-mutable :435-461). Boundary plan must be MicrosoftX64, 2 params, no result, shapes equal source shapes (:244-254). Method must be owner `ProgramStorageEntry`, name `enter`, 2 params, no result, no result claims, entry claims `[image,storage]` indexes 0 and 1 (:346-357). Claims: carrier identity text non-empty, domain non-empty, predicate present, `CarryPolicy::STRICT`, `Accepts` (:412-422). Physical plan must differ from semantic plan: fingerprints must differ (:283-284) and physical must have a result (:285-289).

**Cases by shape.** "optimized semantic ProgramStorage entry requires the exact UEFI x86-64 target slot" (:220-222); two roots only.

**Same job elsewhere.** `selected_method` :313-337 repeats the single-method match already in `SelectedProgramStorageEntryPlan::from_target_slot` selected_entry.rs:78-93 (read).

**Fail-open.** :275-280 `physical_contract.identities().text(physical_contract.requirement_identity()) == Some(selected.schema().identity_text(method.requirement_identity))` is in the *reject* disjunction — meaning physical and semantic requirement text equal → rejects "conflates"; if the physical requirement text is `None`, `Some(..) == None` is false so it does not reject on that term (accepts) — traced: `ProgramEntryPhysicalContractPlan::new` interns the requirement so text resolves; low risk. `selected_method` uses `.unwrap_or_default()` for the requirement text (:326): missing text → "" → `id_at("")` result compared; traced: constructor interned it. Otherwise none seen.

## plans/program-entry-plan/src/optimized_semantic_wrapper.rs (635 lines) — read in full

**Purpose.** Plans, validates, projects and encodes the fixed x86-64 Microsoft-ABI wrapper recipe for UEFI `ProgramStorageApplication` that copies two incoming indirect `Extent` args (RCX, RDX) to caller-owned stack, passes addresses, calls a private terminal continuation, releases frame, returns.

**Shape.** consts `SHADOW_BYTE_COUNT=32`, `OUTGOING_FRAME_BYTE_COUNT=72`, `PRE_CALL_STACK_ALIGNMENT=16` :24-28; `plan_optimized_program_storage_semantic_wrapper` :30; `validate_optimized_program_storage_semantic_wrapper` :64; `OptimizedProgramStorageSemanticWrapperStep` enum :153 (EnterFunction, Reserve…, CopyIncomingIndirectExtentWord, BindOutgoingExtentCopyAddress, CallPrivateTerminalContinuation, Release…, ReturnUnit); `…WrapperPlan` :186; `Staged…Encoding` :245; `…EncodingError` :266; `select_optimized_program_storage_semantic_wrapper_encoding` :280; `validate_optimized_program_storage_semantic_wrapper_encoding` :300 (pub(crate)); `project_request` :318; `expected_steps` :442; `expected_relocation` :542; `validate_contract_surface` :554; `validate_root_placement` :594; `replay_steps` :625.

**How it decides.** Plan validity = `plan.steps == expected_steps(fingerprint)` full equality against a recomputed fixed 11-step list (:625-635); relocation must equal `expected_relocation(call_step_index)`. `project_request` pattern-matches exact slice shape `[Enter, Reserve, c1..c4, b1, b2, call, release, ReturnUnit]` (:327-350) and hard-codes `expected_call_index = 8` (:374). Register/offset constants hard-coded: Image from RCX copy offsets 32/40, InitialStorage from RDX offsets 48/56 (:450-481). Root placement must be `Indirect{Register(RCX|RDX), copy offset 32|48, 16 bytes, align 8}` (:594-621).

**Cases by shape.** Entire module is one fixed shape: UEFI x64, 2 Extent roots, no result (:557-572). Error enum `InvalidSemanticPlan` hides inner reason: `.map_err(|_| InvalidSemanticPlan)` :287, :304 (drops the diagnostic text).

**Same job elsewhere.** Hard-coded register/offset facts also appear in `calling_conventions` (not read) and isa_x86_64 wrapper encoder (not read; suspected).

**Fail-open.** None seen; `expect("the canonical recipe always emits one continuation call")` :46 panics rather than accepts.

## plans/program-entry-plan/src/post_handoff_writer.rs (330 lines) — read in full (feature `installed-writer`, lib.rs:28)

**Purpose.** Lowers a `PostHandoffWriterPlan` into target-specific bytes (x86-64 or AArch64), records footprint, and binds the lowered invocation to a provider-populated context and a validated destination.

**Shape.** `LoweredPostHandoffWriterFragment` :17; `LoweredPostHandoffWriter` :53; `PreparedPostHandoffEntryWriterInvocation` :69; `PostHandoffEntryWriterBindingError` :76; `lower_post_handoff_writer_fragment` :114; `bind_post_handoff_entry_writer_invocation` :166; `preflight_post_handoff_entry_writer_binding` :234; `validate_lowered_post_handoff_writer` :261 (re-encodes bytes and compares for exact replay); `emitted_writer_report_fingerprint` :315 (FNV, `.max(1)`).

**How it decides.** Keyed on `target.architecture` match `X86_64 | Aarch64` (:126, :276) — no other architecture exists in the `Architecture` match here. Binding requires installed architecture == target architecture (:243), `context.binds_invocation(&lowered.invocation)` and fingerprint equality (:203-206).

**Same job elsewhere.** The architecture-split encode + footprint tuple is repeated twice (:126-147 and :276-297).

**Fail-open.** None seen.

## plans/program-entry-plan/src/program_entry_physical.rs (498 lines) — read in full

**Purpose.** Defines target-fixed physical process-entry contracts: per-package arrival table, boundary-entry-plan replay, the "type identity" strings for entry params, and digests of the bundled `entry.omg` package sources.

**Shape.** Type-identity consts :19-27 (e.g. `"named(name(u32))"`); `process_entry_state_plan` :33; `exact_hosted_physical_boundary_entry_plan` :51 (returns `None` for UEFI since its `replay` is `None`); `ProgramEntryPhysicalContractPackageSourceDigest` :58; `exact_physical_contract_package_source_digest` :63; `ProgramEntryPhysicalContractPlan` :108 (`new` :123); private `PhysicalArrival` :253 and `ARRIVALS: [PhysicalArrival; 6]` :307-362; `physical_arrival` :364 (linear find, `.expect`); `ordinary_arrival` :371; `linux_x86_64_arrival` :379 (exit_group syscall `immediate: 231`, :406-409); `linux_arm64_arrival` :417 (`immediate: 94`, :445-448); `close_uefi_entry_stack` :456; `exact_package_source` :471 (`include_bytes!` of `source/library/std/targets/<target>/entry.omg`).

**How it decides.** Rows keyed on `Package` enum: WindowsX64 (0 params, i32 result), MacosX64 and MacosArm64 (4 params: i32,addr,addr,addr), LinuxX86_64 / LinuxArm64 (1 param: addr), UefiX64 (2 addr params, u64 result; no replay closure). `new` validates the slot equals the owner's declared slot, package matches, calling convention equals the row's, param count, nonempty identities, nonzero fingerprint, plan policy/param count/result presence (:134-173).

**Cases by shape.** Six hard-coded rows (`ARRIVALS: [PhysicalArrival; 6]`); Linux entry plans hand-written (:383-411, :421-449) with the entry stack pointer modelled as parameter in `X86Rsp` register (x86) / stack offset 0 (arm64).

**Same job elsewhere.** `ProgramEntrySlotDeclaration` data lives in `target` crate (this unit, target/src — grep below) and is mirrored here by `ARRIVALS` (convention/policy/params).

**Stated limits.** None.

**Fail-open.** `exact_hosted_physical_boundary_entry_plan` returns `None` for UEFI — callers must handle; not traced. `non_authoritative_target_package_source_report_fingerprint` is carried but named non-authoritative.

---

# register-environment (path prefix `R` = `omega-rust/omega/compilation/backend/register-environment/src/`)

## R catalog.rs (105 lines) — read in full
**Purpose.** Per-architecture selection of the register model, constraint catalog, conservative reservation profile and ISA-owned selected-constraint-key roster.
**Shape.** `target_physical_register_model` :25, `target_constraint_catalog` :32, `conservative_baseline_reservation_profile` :42, `selected_constraint_keys` :61, test-only `scalar_call_constraint_key` :93 (cfg(test) const fn); an in-file `#[test] float_result_policy_keys_are_declared_for_every_hosted_target` :68 (the `#[test]` is not under `#[cfg(test)]`).
**How it decides.** Keyed on `target.architecture` (X86_64 | Aarch64) for every selector (:26, :36, :62). Overlay filter is keyed on string name: `overlay.name != "darwin.aarch64.platform" || target.object_format == ObjectFormat::MachO` (:50). Profile name constant `"omega.conservative-baseline-v1"` :56. `scalar_call_constraint_key` keyed on (architecture, object_format) pairs: (X86_64, Elf|MachO)→System V, (X86_64, Coff)→Microsoft, (Aarch64, Elf)→AAPCS64, (Aarch64, MachO)→Darwin, else None (:96-103).
**Cases by shape.** Only two architectures; `ObjectFormat` pairs above.
**Same job elsewhere.** `(architecture, object_format)` → ABI-kind match is repeated in lib.rs `selected_abi_preservation` :259-279 (read) — same four-way table, different payload.
**Fail-open.** none seen (`selected_constraint_keys` returns `Option`; caller rejects `None`, lib.rs:122-123).

## R lib.rs (326 lines) — read in full
**Purpose.** Builds one validated per-target register environment (physical model + constraint catalog + reservation profile + selected keys) with a process-wide cache, and provides ABI preservation lookups.
**Shape.** `baseline_target_register_environment` :66 (function-local `static BUILT: Mutex<Vec<(NativeTarget, Result<..>)>>` :69-74, linear lookup by `==`; caches Err too); `build_baseline_target_register_environment` :89; `validate_target_register_environment_join` :100 (pub(crate)); `ValidatedTargetRegisterEnvironment` :152 (fields target, physical, constraints, reservations, selected_keys, identity); `admits_pointer_displacement` :184; `fixed_register_view` :234; `SelectedAbiPreservation` :245; `selected_abi_preservation` :255; `selected_preservation_storage_catalog` :285; `TargetRegisterEnvironmentValidationError` :303; `#[global_allocator] MiMalloc` under `#[cfg(test)]` :324-326.
**How it decides.** Join validation: architecture of model vs target (:106); constraint catalog validated by the architecture's validator (:114-121); selected keys must exist (:122); reservation overlay with the string name `"darwin.aarch64.platform"` rejected if `object_format != MachO` (:124-131); identity computed from target, physical, constraints, reservations, selected-key rows (:134-140). `admits_pointer_displacement`: byte_size must be 1|2|4|8; x86-64 admits `byte_offset <= i32::MAX`; AArch64 admits `offset % size == 0 && offset/size <= 4095` (:184-194). `selected_abi_preservation` keyed on (architecture, object_format) :259-279 returning `UnsupportedTargetConvention` for other pairs.
**Cases by shape.** `(Architecture::X86_64, ObjectFormat::Elf | MachO)` System V; `(X86_64, Coff)` Microsoft; `(Aarch64, Elf)` AAPCS64; `(Aarch64, MachO)` Darwin AAPCS64; `_` → error (:259-279). A `Coff` AArch64 target would be an error.
**Inputs/outputs.** Reads `isa_x86_64`, `isa_aarch64`, `register_model`, `selected_instructions`. Returns validated env or error enum.
**Same job elsewhere.** The darwin-overlay applicability rule appears twice: catalog.rs:50 (filter when building) and lib.rs:124-131 (reject in join) — both by overlay name string (read).
**Fail-open.** `if let Ok(built) = BUILT.lock()` (:75) — a poisoned mutex silently skips the cache and rebuilds; result is recomputed from the same inputs so nothing wrong is accepted ("traced": value is a pure function of `target`). `selected_abi_preservation` `.map(...).ok_or(...)` fails closed.
**Stated limits.** Header: "Rather than exposing a separate decoded-environment replay path" (:3-4) — no limit statement.

## R tests.rs (127 lines) — read in full
Two tests: `baseline_profile_is_exact_conservative_and_platform_applicable` :22 (darwin overlay present for macos_arm64, absent for linux_arm64, inapplicable overlay on Linux → `InapplicableReservationOverlay`); `environment_identity_binds_each_component_and_explicit_policy` :65 (identity changes with reservation profile, pointer_size, target; Windows and Linux x64 share physical and constraint identities but not environment identity :117-126).

## R tests/supported_targets.rs (253 lines) — read in full
Two tests over six targets (linux_x64, macos_x64, windows_x64, uefi_x64, linux_arm64, macos_arm64, :20-27, :54-61): selected keys equal the ISA effect catalog's; selected keys for copy/add/subtract/load/store/frame address/hosted exit/hosted write byte exist per architecture. Hard-codes `expected_exit` per target (linux/macos x64 → `X86_64_HOSTED_EXIT_PROCESS_I32`, linux_arm64, macos_arm64 → their keys, others `None`, :114-123) and `expected_write` by (arch, object format) (:138-149). Windows/UEFI have no hosted exit/write keys.

## R tests/abi_call_clobbers.rs (520 lines) — read in full
Shared harness for the effect-catalog tests: `ScalarAbiCase` :47 and `scalar_abi_cases()` :98 (six rows: linux_x64, windows_x64, uefi_x64, linux_arm64, macos_arm64; note: `macos_x64` is not in this matrix, though supported_targets.rs tests it), helper fns `named_view`, `convention_for` :170, `units_for_names` :183, `validated_effects` :199, `produced_effects` :219, `validate_effects` :352 (maps both ISA validators' errors into `EffectRejection`), `declaration_mut`, `encoded_mut`; tests `every_supported_target_selects_its_exact_scalar_call_abi` :400, `platform_abi_differences_are_explicit_call_clobber_facts` :466 (x18 clobbered on Linux ARM64, preserved/reserved on macOS; rsi/xmm6 clobbered on Linux x64 but preserved on Windows), `microsoft_unit_call_clobbers_match_every_coff_target` :496. `scalar_abi_cases()` has 5 entries (linux_x64, windows_x64, uefi_x64, linux_arm64, macos_arm64); provenance_effects.rs:138 asserts distinct targets == `len() - 1` (4). That holds because `NativeTarget` has only four fields (architecture, object_format, pointer_size, pointer_alignment; `target/src/target_profile.rs:541-546`, grepped) and `NativeTarget::windows_x64()` (:695-702) and `NativeTarget::uefi_x64()` (:704-711) are field-for-field equal (X86_64, Coff, 8, 8). So `NativeTarget` cannot tell Windows from UEFI; every `NativeTarget`-keyed table here (the process-wide environment cache `BUILT` in lib.rs:69-74, `target != NativeTarget::uefi_x64()` in optimized_semantic_entry.rs:212) treats them as one target. `macos_x64` is tested in supported_targets.rs but absent from this ABI matrix.

## R tests/abi_call_clobbers/arithmetic_effects.rs (1064 lines) — read in full
Tests the ISA machine-effect catalog's arithmetic rules per target: `selected_arithmetic_rules` :23 lists (key, semantic) pairs (asserted count 71 per target :544); `arithmetic_contract` :353 hard-codes expected operand access, flags custody (x86 clobbers flags, AArch64 mostly none), fixed views (`rax`/`rdx`/`rcx` for divide/remainder on x86), faulting (x86 divide), alternatives incl. alias-dependent subtract/multiply (:411-438) and an optimization-selected xor-zero alternative named `"X86SelectXorZeroI64MaterializationV1"` (:374). `expected_size` :258 hard-codes encoded byte sizes per semantic and architecture (e.g. x86 `MaterializeI64` 10 bytes, :269). Forgery test `every_selected_arithmetic_rule_rejects_encoded_effect_forgery_on_every_target` :721 corrupts each declared fact (control, memory, stack, trap, reads/writes, implicit uses/defs/clobbers, family, applicability, size, variant) and asserts a specific rejection (`InvalidEncodedEffects`, `SemanticMismatch`, `NonCanonicalAlternatives`, …). Keyed on semantic kind enum values and target architecture.

## R tests/abi_call_clobbers/branch_effects.rs (411 lines) — read in full
Same pattern for 3 conditional branches + `Jump` (count 4, :96): barrier `ControlFlow`, flags/program-counter implicit use, encoded control `ConditionalRelativeBranchV1` / `UnconditionalRelativeBranchV1`, hard-coded sizes (x86 `Jump` 5 bytes, AArch64 4, :59-87). Forgery tests :217-411.

## R tests/abi_call_clobbers/call_effects.rs (264 lines) — read in full
Call rules (call_unit, call_unit_mixed, call_scalar, call_aggregate, call_normalized_foreign, invoke_no_normal): per-key expected `MachineCallEffect` variant by semantic and whether the key family is `SystemCall` (supervisor) (:80-98); every operand pinned to a caller-saved view (:129-145); x86 user-space calls carry return-address memory/stack lifecycle, AArch64 storage-free (:216-238). Forgery test only covers one corruption (lost encoded clobber, :245-264).

## R tests/abi_call_clobbers/cleanup_effects.rs (444 lines) — read in full
Activation-stack lifecycle: `lifecycle_owner` :29 returns Call/Return owner only for x86-64 and non-`SystemCall` keys (:34-37); `temporary_stack_shape` :52 hard-codes x86-only keys (IEEE directed float, flags snapshot/restore, task register activate, segment reloads, installed-interrupt checks) with byte counts 16/8; forgery tests :306-444 (dropped/cross-class/zero/mismatched/foreign-pointer lifecycles; `MemoryFence | AtomicFence` expect `BarrierMismatch` :364-370).

## R tests/abi_call_clobbers/memory_effects.rs (760 lines) — read in full
Pointer displacement limits test :23 (AArch64: `4095*width` ok, `4096*width` not; x86 `i32::MAX` ok, `1<<31` not; widths 0/3/16 refused); memory rules (copy/zero bytes, packed loads 3/5/6/7, store, loads, indexed load, store64, frame address; count 16 :442), floating-control storage test :131 (mxcsr 4 bytes on x86, fpcr 8 on AArch64; AArch64 clobbers x9 (+x16 on save)), `expected_size` :390 hard-coded per architecture, forgery tests :595-760.

## R tests/abi_call_clobbers/provenance_effects.rs (352 lines) — read in full
Catalog provenance: effect catalog binds target, constraint-catalog identity and selected keys (:65-139); fused-multiply-add declared on all targets, x86 unfused substitution rejected (:142-171); provenance forgery (foreign architecture target → `TargetArchitectureMismatch`, sibling target → `SemanticMismatch`, forged root → `RegisterConstraintRootMismatch`, duplicate/swapped/dropped/appended selected keys → `DuplicateSelectedConstraintKey` / `DeclarationRosterMismatch`) :174-266; declaration roster forgery :269-352 (reordered → `NonCanonicalDeclarations`, unknown row → `UnknownConstraint`, cross-bound selection → `SemanticMismatch`).

## R tests/abi_call_clobbers/register_calls.rs (108 lines) — read in full
Every scalar-call arity key (and float scalar-call keys) retains exact ABI argument views, result view, implicit uses `rsp,rip`/`sp,pc`, and clobbers equal to the generic call row (`generic = environment.constraint(case.call)`); float rows derive clobbers by removing the result view's write units (:99-105).

## R tests/abi_call_clobbers/return_effects.rs (267 lines) — read in full
Return rules (return_i64, return_float, return_unit, return_aggregate): operands pinned to integer/float result views, x86 `ReadActivationStackV1`/`PopBytesV1`/`ReturnFromActivationStackV1`, size 1 byte; AArch64 `ReturnIndirectRegisterV1{target: x30}`, size 4 (:166-218); forgery tests :224-267.

**Register-environment tests — same job:** every test file repeats a per-family "corrupt one fact, expect specific rejection" loop (arith :721, branch :217, memory :595, cleanup :306, provenance :174/:269, return :224, call :245). The `reject_encoded!` macro is defined separately in arithmetic_effects.rs:730, branch_effects.rs:229, memory_effects.rs:606 (three copies, read).
**Fail-open (tests).** Tests only; none seen.

---

# register-model (path prefix `M` = `omega-rust/omega/compilation/backend/register-model/src/`)

## M lib.rs (510 lines) — read in full
**Purpose.** Root of the declarative physical-register model: units/views/classes/conventions/reservations, a builder, and `validate_physical_register_model`.
**Shape.** `PreservationConvention` :87 (name, argument_views, result_views, caller_saved, callee_saved, fixed, stack_alignment, red_zone_bytes); `PhysicalRegisterModel` :99 (+ linear `view_named` :109, `view` :113, `unit` :117, `class` :121, `convention_named` :125, `aliases` :131); `PhysicalModelBuilder` :142; helpers `units_for` :223 (indexes a `BTreeMap` with `[]`, panics on missing name), `complement` :230, `view_ids` :241 (`[]` panic); `ValidatedPhysicalRegisterModel` :246; `RegisterModelValidationError` :267; `validate_physical_register_model` :298; `validate_convention` :458.
**How it decides.** Structural only: sequential ids (`validate_sequential_ids` :416), unique names, nonzero widths, view bits ≤ sum of unit bits (:335-342), view units ⊆ write_units (:346), write-semantics checks (ExactView ⇒ units==write_units, ZeroExtendsParent ⇒ units≠write_units, Discards ⇒ all units kind Zero, :355-377), class↔view membership, every unit covered by some view, per-convention partition of ALL units into caller_saved/callee_saved/fixed with no overlap and no omission (:473-495), nonempty reservations.
**Inputs/outputs.** Takes owned model; returns `ValidatedPhysicalRegisterModel{model, identity}` with identity from `identities::physical_register_model_identity` (:412).
**Cases by shape.** `write_semantics` variants `PreservesUnwritten | ZeroExtendsWithinUnit | InstructionDefined` accepted unchecked (`_ => {}` :376).
**Same job elsewhere.** `Sha256RegisterDigest` (register_vocabulary.rs:73) overlaps `optimization_core::digest_encoder::DomainDigestEncoder` (impl at register_vocabulary.rs:93) and identities.rs byte-pushers (`u64_value`, `string`...) — three ways to feed a canonical digest in this crate (read). Name uniqueness check `unique_names` :428.
**Unused/one-caller.** `units_for`, `complement`, `view_ids` are `pub` helpers; grep counts below.
**Fail-open.** `validate_ordered_views` :499 only checks uniqueness and existence (no class check). `.expect("view unit validated above")` panics, not accepts. none seen.

## M register_vocabulary.rs (119 lines) — read in full
Plain data: `RegisterUnitId/ViewId/ClassId(u16)` :8-14, `RegisterUnitKind` (IntegerLane, VectorLane, Flags, StackPointer, InstructionPointer, Zero, FloatingControl) :17, `RegisterUnit` :28, `RegisterWriteSemantics` :36, `RegisterView` :46 (+`footprints_overlap` :58), `RegisterClass` :67; `trait Sha256RegisterDigest` :73 with impls for `sha2::Sha256` and `DomainDigestEncoder`; `sha256_length` :107, `sha256_register_units` :111.

## M reservation_profiles.rs (151 lines) — read in full
`ReservationReason` (9 variants) :14, `RegisterReservationOverlay{name,reason,units}` :27, `RegisterReservationProfile{name, active_overlays: Vec<String>}` :34, `validate_register_reservation_profile` :85: name non-empty, architecture match, overlay names strictly increasing, each overlay name must exist in the model (`UnknownOverlay`), reserved units = union. Keyed on overlay NAME strings (:107-115). `sorted_units` :134, `reservation_overlay` :141.

## M constraint_catalog.rs (658 lines) — read in full
**Purpose.** Defines instruction register-constraint rows and the named selection roster `TargetRegisterEnvironmentConstraintKeys`; validates a catalog against a physical model.
**Shape.** `RegisterConstraintFamily{Call,Return,SystemCall,InlineAssembly,Instruction}` :21; `RegisterConstraintKey{family,variant:u32}` :42; `TargetRegisterEnvironmentConstraintKeys` :79-185 (about 110 named fields: ~68 `Option<RegisterConstraintKey>`, ~32 required keys, 8 `Vec` keys); `RegisterOperandConstraint` :225; `RegisterInstructionConstraint` :235 (operand/clobber/implicit lists); `RegisterConstraintCatalog` :267 (+ linear `constraint` :274, `row_matching` :278); `ValidatedRegisterConstraintCatalog` :298 (its `constraint` uses `binary_search_by_key` :317); `validate_register_constraint_catalog` :389; `validate_required_constraint_inventory` :515; `validate_operand_ties` :551; `call_constraint_keys` :640; `find_register_constraint(_mut)` :646/:653 (linear).
**How it decides.** Architecture match; ids are 0..n in order; `required` and constraint keys strictly increasing; `required` must equal the set of constraint keys exactly (merge walk :524-548); operand ordinals increasing; class exists; fixed view exists and has same class, else class must contain an allocatable view; early_clobber requires a writing access; ties: tied_to < operand, exists, tied operand has no tie, same class, fixed views equal if both, pair reads and writes (:551-603); unit sets strictly increasing and known; implicit_defs ∩ clobbers = ∅ (:492-503).
**Same job elsewhere.** The two lookups (`find_register_constraint` linear :646, `ValidatedRegisterConstraintCatalog::constraint` binary search :314) do the same job; the validated one relies on key ordering validated at :408-414 (read). `RegisterConstraintCatalog::constraint` :274 uses the linear form.
**Stated limits.** Field comments describe per-field contracts, e.g. ":135 Target-owned resultless call keys indexed by argument count".
**Fail-open.** none seen. NOTE: `TargetRegisterEnvironmentConstraintKeys.ieee_float_directed_unary` (:150) is not fed to `target_register_environment_identity` (see identities.rs below).

## M identities.rs (479 lines) — read in full
**Purpose.** Domain-separated SHA-256 identities for the physical model, constraint catalog, reservation profile and whole target register environment.
**Shape.** `domain_digest` :20; `identity!` macro :31 (types `PhysicalRegisterModelIdentity` domain `...physical-register-model-identity.v1`, `RegisterConstraintCatalogIdentity` `.v1`, `RegisterReservationProfileIdentity` `.v1`, `TargetRegisterEnvironmentIdentity` `...target-register-environment-identity.v31` :68); `physical_register_model_identity` :71; `register_constraint_catalog_identity` :116; `register_reservation_profile_identity` :134; `infrastructure_environment_agrees` :152; `target_register_environment_identity` :172 (enumerates fields of the key roster by hand: ~63 optional keys :184-250, then vecs, then 32 required keys :275-308); `native_target` :320 (architecture, object_format, pointer_size, pointer_alignment).
**How it decides.** Hashes every listed field in fixed order. Absent optional = tag 0.
**Fail-open / gap (grepped).** Comparing the struct at constraint_catalog.rs:79-185 to the lists at identities.rs:184-250 and :275-308, the field `ieee_float_directed_unary` is not hashed. `grep ieee_float_directed_unary omega-rust` returns 9 hits: constraint_catalog.rs:150 (definition), identities.rs none, tests/declared_identities.rs:210 (set `None`, never mutated), plus selected-instructions (catalog.rs:99, :102, :229, catalog/identity.rs:924, analyses/machine_effects.rs:322) and isa-x86_64/machine_effects.rs:917, isa-aarch64/machine_effects.rs:554. So two environments differing only in this key get the same `TargetRegisterEnvironmentIdentity`. Status: "traced" for the identity function (read), "open" for whether another identity (selected-instructions catalog identity) covers it. Also `return_float` is only hashed when non-empty (:311-316) and carries no framing tag when empty. `call_scalar` length is written before `call_unit_mixed` but its elements after (:257-264) — ordering oddity, still deterministic.
**Same job elsewhere.** `native_target` :320 encodes (arch, object_format, pointer size/alignment) — `NativeTarget` also has OS-level fields? (see target/src/lib.rs notes below).

## M preservation_storage.rs (461 lines; tests :261-461) — read in full
`FrameAbiPreservationConvention{SystemVAMD64,MicrosoftX64,Aapcs64,DarwinAapcs64}` :17 (`abi_tag` 0..3), `PreservationStorageGroup` :39, `PreservationStorageCatalog` :49 (convention name String), `ValidatedPreservationStorageCatalog` :56, identity domain `omega.preservation-storage-catalog-identity.v1` :84, `validate_preservation_storage_catalog` :139: catalog bound to model identity, convention found by NAME (:146-150), group ids sequential, group names unique, preserved units sorted/known/in `callee_saved` (binary_search :205)/disjoint, storage view's units == preserved units, view bits multiple of 8, size == bits/8, alignment power of two, coverage of every callee-saved unit, and `ordered_coverage == convention.callee_saved` (:253). Tests :348-460.

## M reservation/test files
## M test_support.rs (138 lines; feature `test-support`) — read in full
Shared helpers for ISA register-model tests: `row`, `row_mut`, `ForeignTemplateRoster`, `assert_templates_retain_call_effects` :42, `assert_templates_reject_changed_effects` :56 (5 mutations), `AggregateFragmentRoster`, `assert_aggregate_fragments_are_definitions` :105.

## M tests.rs (281 lines) — read in full
Miniature model/catalog fixtures (`miniature_model` :63, `miniature_catalog` :143), `assert_rejected` :26, tests for model validation, reservation profile identity/validation.

## M tests/constraint_catalog.rs (292 lines) — read in full
One test per `RegisterConstraintCatalogValidationError` family (inventory, canonical ids/keys/operands, fixed-view class, unknown/unallocatable class, ties/early clobbers, implicit effects, architecture) plus a lookup test that compares `find_register_constraint` against `validated.constraint` (binary search) for all families and variants including `u32::MAX` (:194-292).

## M tests/declared_identities.rs (410 lines) — read in full
Mutation tests that each declared model/catalog field changes its identity (:22-123) and an environment-identity test that mutates ~55 named selected keys (:338-409). The fixture sets `ieee_float_directed_unary: None` (:210) and no mutation touches it; `ieee_float_directed_binary` is mutated (:283-290).

---

# runtime/executable-installation (prefix `X` = `omega-rust/omega/compilation/backend/runtime/executable-installation/src/`)

Crate-wide finding (grepped): `Grep "InstalledCode \{|CodePlacement \{|FrozenPlacement \{|ValidatedPlacement \{|AdmittedArtifact \{|Artifact\(Arc|Arc::new\(ArtifactRecord" omega-rust *.rs` returns only type definitions and `impl` headers (X post_handoff_writer.rs:27, placement.rs:45/:63/:71, installation.rs:103/:109, artifacts.rs:121/:126) and an unrelated `RecordedArtifact` in psi build-time-evaluation. `grep "ArtifactRecord {"` finds only the definition at artifacts.rs:41. `grep ArtifactAuthorityCommitments` finds only artifacts.rs:12/:56 and authority_digests.rs:103. So no code in `omega-rust` constructs an `Artifact`, `ArtifactRecord`, `CodePlacement`, `FrozenPlacement`, `ValidatedPlacement`, `AdmittedArtifact` or `InstalledCode` (as struct literals). The only users of crate `executable_installation` are `program-entry-plan/src/post_handoff_writer.rs` (feature `installed-writer`), grep `executable_installation::|executable-installation` in `*.rs` (3 files listed: post-handoff-writer-plan/src/lib.rs, program-entry-plan/src/post_handoff_writer.rs, installation-evidence/src/lib.rs — the latter two as text/doc; I did not check whether post-handoff-writer-plan's hit is code or a comment).

## X artifacts.rs (152 lines) — read in full
**Purpose.** Decoded-artifact carriers with no authority (`Artifact`), and `AdmittedArtifact` that can resolve an entry id into a `RelocationTarget`.
**Shape.** `ArtifactRelocationKind` :24 (Absolute64, X86Relative32, Aarch64Page21/PageOffset12/Branch26); `DecodedArtifactRelocation` :33; `ArtifactRecord` :41 (pub(crate)); `ArtifactEntry` :60; `Artifact(Arc<ArtifactRecord>)` :76 with `entry` :111 (binary search by `EntryStubId`); `AdmittedArtifact` :121 (+`selected_entry_target` :127); `InstallationAudience` :148 (`#[expect(dead_code, reason = "...await TASKS.md COMPONENT-SUBSTRATE")]` :144-147).
**How it decides.** `selected_entry_target` accepts iff the entry id exists in the sorted entries vec (binary search relies on entries being sorted by identity — nothing in this crate sorts or checks it; `grep` shows no constructor, see crate-wide finding).
**Stated limits.** `:146` "spec audiences ... await TASKS.md COMPONENT-SUBSTRATE" (dead enum).
**Fail-open.** none seen (no constructors).

## X authority_digests.rs (158 lines) — read in full
Macro-generated newtypes: `normalized_id!` (13 non-zero u64 ids :37-52), `normalized_digest!` (7 digests :67-73, no constructors, only `.digest()`), `canonical_authority_digest!` :75 (4 types :88-100; the `$domain:literal` parameter is accepted but never used in the macro body :75-86), `ArtifactAuthorityCommitments` :103 (private fields, no constructor), five empty `impl X {}` blocks :114-122, `non_authoritative_fingerprint64!` (3 types :147-158; reject zero). Same job elsewhere: 64-bit nonzero-id newtype pattern is also hand-rolled in `evaluated_bindings.rs` (digest macro rejecting all-zero, effects crate) — different crate, suspected.

## X installation.rs (156 lines) — read in full
`InstallAuthority` :21, `WxEnforcement{HardwareEnforced,ConventionOnly,Unsupported}` :44, `InstallationReceipt` :51, `InstallationError` :85 (returns linear inputs), `InstalledCode` :103 (+ accessors; `selected_entry_target` :138), `InstallationDiagnostic(String)` :150. No function in this file or crate checks `wx` or `visibility_complete` (grep not run; read: only stored at :54-55, :106). Header says "W^X enforcement status" only as data.

## X placement.rs (121 lines) — read in full
`MaterializedArtifactBytes` :18, `CodePlacement` :45 (carries `extents::Extent` "W+NX"), `FrozenPlacement` :63, `ValidatedPlacement` :71, `ValidatedPlacementEvidence` :77 with `from_validated` :98 (copies final bytes `to_vec()` :116). No transition functions (W+NX → R+NX freeze, validate) exist in this file; header :5-6 says the types "carry an admitted artifact ... to a frozen R+NX placement".

## X lib.rs (59 lines) — read in full
Docs describe "five domain modules" (:26-37) and re-exports; says each transition "consumes linear evidence from the previous step" (:7-8) — but the transitions themselves are not in the crate (see crate-wide finding).

## X post_handoff_writer.rs (416 lines) — read in full
**Purpose.** Resolves a `PostHandoffWriterPlan` against an `InstalledCode` into a `ResolvedPostHandoffEntryWriterContext` (word 0 = destination base, then dense source slots), and validates prepared destination mappings.
**Shape.** `impl InstalledCode` :27 (`resolve_post_handoff_entry_writer_source_values` :28, `resolve_admitted_entry_target` :54, `populate_post_handoff_entry_writer_context` :82); `ResolvedPostHandoffEntryWriterContext` :117 (packed_words, FNV-1a 64 `non_authoritative_fingerprint`); `DestinationPreparationReceipt` :128 (+ `from_admitted_provider` :137 — any caller can pass `pinned`/`unpublished` booleans); `PreparedPostHandoffWriterDestination` :159 (`claim` :181); `ValidatedPreparedPostHandoffWriterDestination` :176; `validate_post_handoff_destination_binding` :237; `DestinationClaimError` :268; `non_authoritative_post_handoff_entry_writer_context_fingerprint` :352.
**How it decides.** Entry resolution: `RelocationTarget::Entry(id)` looked up in the artifact; `Data(_)` → None → error (:58-63); address = placement base `checked_add(code_offset)`. Pre-resolved source must equal the resolved address (:39). Destination claim checks, in order (:243-261): receipt mapping == mapping receipt context, pinned, unpublished, has a writer right, mapping rights ⊇ required rights, `site.phase == PostHandoff`, `site.base_address == mapping.base()`, `mapping.length() as usize == byte_len`.
**Cases by shape.** Only `Entry` targets resolve; `Data` targets are refused (:62).
**Stated limits.** `:161` and `:166` `#[expect(dead_code, reason = "custody carrier ...; TASKS.md COMPONENT-SUBSTRATE wires its reader")]` on `mapping` and `receipt` fields (unread).
**Fail-open.** `DestinationPreparationReceipt::from_admitted_provider` :137 is `pub` and takes unchecked `pinned`/`unpublished` flags; the claim check trusts them. Whoever builds the receipt can assert pinning; "open" (provider-supplied by design, name says "from_admitted_provider").

# runtime/installation-evidence (prefix `I` = `.../runtime/installation-evidence/src/`)

## I lib.rs (183 lines) — read in full
Trait and value carriers read by backend lowerers: `InstalledArtifactOccurrenceDigest` :44; traits `ProviderExecutionEvidence` :56, `ProviderInstallationEvidence` :107, `ObjectEvidence` :112, `StackDemandEvidence` :122 (all implemented elsewhere; "orchestration owns the sealed values" :8-9); structs `InstalledProviderCompletionClaimSource` :66, `InstalledAssumedBindingEvidence` :73 (`matches` :82 compares ids), `InstalledProviderCallEvidence` :94, `InstalledInterruptStackRequirement` :134 (`required_bytes` :150: `own.max(arrival.domain_bytes)`; `own_bytes` :159 checked_add), `InstalledBodyFaultPremises` :167, `InstalledArrivalStackDemand` :172, `InstalledPhysicalStackRoot` :180. The doc says "without public authority constructors" (:7) but every struct has all-`pub` fields, so any crate can build one (read, :66-183).
## I installation_environment.rs (178 lines) — read in full
`InstallationEnvironmentAssumption{Bootstrap, InterruptDelivery}` :10 with `BootstrapEnvironmentAssumption` :16 and `X86InterruptDeliveryAssumption` :43, and validation `validate_description` :168: digest nonzero; source/caller provenance non-empty after trim, ≤4096 bytes, no control chars. Interrupt-delivery requires `profile.native_target().architecture == X86_64` (:67). `description()` :79 returns fixed English strings for each condition. `X86InterruptDeliveryProfile` single variant :61. These are "caller premises" (header :4-6).
**Fail-open.** The assumptions are accepted on a digest being nonzero and prose being non-empty (:168-178) — no relation between the digest and any content is checked here; stated: "explicit caller premises rather than hardware observations" (:5-6).

# runtime/runtime-abi (prefix `A` = `.../runtime/runtime-abi/src/`)

## A lib.rs (150 lines) — read in full
`FatDescriptorLayout` :76 and `fat_descriptor_layout` :81 (size = pointer_size.saturating_mul(2), align = pointer_alignment), `target_carrier_layout` :88, `NativeForeignFrame` #[repr(C)] 16 bytes :103 (compile-time asserts size 16, align 8 :113-114), `capture_host_x86_flags` :116 (inline asm `pushfq`, only on x86_64 host, else None). Contains the crate's `unsafe`.
## A borrowed_storage.rs (268 lines) — read in full
`NativeBorrowedStorageLayout` :84 built via a 3-arg `new` :100 with large tuple parameters; arenas for fields, paths, repetitions, discriminators, cases, opaque regions; accessor methods :193-267. Padding/pointers/inactive sum cases kept in `opaque_regions` (header :7-9). No validation in `new` (no check offsets fit `byte_size`; read).
## A descriptor_io.rs (183 lines) — read in full
`descriptor_import_identity` :37 recognises libc `read`/`write` by TARGET + exact normalized locator: Darwin `/usr/lib/libSystem.B.dylib` symbols `_read`/`_write`; Windows `msvcrt.dll` `_read`/`_write`; Linux `libc.so.6` `read`/`write` with versions `GLIBC_2.2.5` (x64) / `GLIBC_2.17` (arm64) (:47-71). `descriptor_system_call` :85: Linux x64 read=0 write=1; Linux arm64 read=63 write=64 (:89-96). `descriptor_import` :28 is a one-line wrapper that calls `descriptor_import_identity` — same job twice (grep below). Keyed on target profile + locator equality via `normalize_foreign_locator(candidate, target) == locator` (:74-78).
**Cases by shape.** `_ => return None` for any other `TargetProfile` (:45) incl. UEFI.
## A host_numeric.rs (190 lines) — read in full
FFI leaves: `host_affine_transform` (macOS CoreGraphics only, else None) :37, `host_integer_absolute` (None for `i32::MIN`) :57, `host_integer_division` (None for zero divisor and `i64::MIN/-1`) :82, `parse_host_long` (requires trailing NUL and base 0 or 2..=36; unix 64-bit only, else None) :103. `host_integer_absolute` and `lldiv` are linked on Windows via `msvcrt`/`ucrt` (:51, :77). Note `parse_host_long` returns `None` if `offset >= bytes.len()` (:112).
## A network_address.rs (99 lines) — read in full
`HostInternetAddress::make`/`host_number` call libc on unix, else the Rust classful definition (`classful_internet_address` :50, `classful_local_address` :64) which is also what `windows_provider.rs` encodes in machine code (header :6-9).
## A scalar_reference.rs (146 lines) — read in full
`ScalarReferenceLayout::NATIVE` :24 (16 bytes: address@0, codec@8), `ScalarStorageCodec` :35 with `from_scalar` :43 (Boolean, 64-bit address, fixed byte-multiple integers ≤64 bits, IEEE float), `encode` :74 (unsigned int `=byte_size`, signed `|256`, Boolean 1025, Address 2056, f32 516, f64 520), `decode` :92 (20 admitted words asserted by test :143). Hard-coded numeric codes.
## A static_interrupts.rs (132 lines) — read in full
Offsets for root/acknowledgement/permit tokens (:15-40), `InstalledInterruptEntryPlan` :60 with fixed `root_slot_from_top: 56, return_frame_bytes: 40, saved_register_bytes: 120` (:73-75, :101-103), `fatal_context` :79 (offsets multiples of 16, distinct, `16 <= table_bytes <= 4096`), `has_canonical_geometry` :107 (equality with a freshly built plan), `hardware_frame_bytes` :123.
## A windows_provider.rs (319 lines) — read in full
Synthesizes `omega_host.dll` (PE32+, two sections `.text`/`.rdata`) exporting `inet_makeaddr`/`inet_lnaof` with hand-assembled x86-64 machine code bytes (`make_body` :136, `host_body` :182) and hand-written PE header offsets (:77-128). `image_imports_windows_internet_provider` :14 is a raw substring search for `omega_host.dll` anywhere in the image bytes. Same job elsewhere (suspected, unread): PE/x86 encoding also lives in the image-emission and `isa_x86_64` crates.
**Fail-open.** `image_imports_windows_internet_provider` returns true for any image containing the 14-byte string anywhere (e.g. in data) — what would wrongly pass: an image that merely contains the text; consumer not traced (open).

---

# runtime/task-plans (prefix `T` = `.../runtime/task-plans/src/`)

## T lib.rs (496 lines) — read in full
**Purpose.** Task activation plan types, canonical argument-marshalling layout, suspension-crossing validation, and the activation plan fingerprint.
**Shape.** `TaskPlanDiagnostic(String)` :68; `TaskActivationPlanFact` :93; `TaskActivationPlanSet` :105; `StackPlan` :121; `TaskArgumentLayout` :135 (+`new` :143, `is_canonical` :156); `pack_task_argument_fields` :172 (alignment ≤ `MAX_TASK_ARGUMENT_ALIGNMENT = 1<<16` :161, overflow-checked); `LiveCarryStorage` :231; `LiveCarryDemand` :239; `CanonicalSuspensionCrossing` :248; `ActivationCarryObligations` :257; `ActivationPlanCandidate` :282; `ValidatedActivationPlan` :297; `validate_wcsu_activation_plan` :319; `validate_activation_plan_shape` :341; `activation_plan_report_fingerprint` :444 (FNV).
**How it decides.** Stack bytes nonzero and alignment power of two; `may_suspend` ⇔ crossings non-empty (:350-359); every crossing must have `suspension_allowed`; crossing identities strictly increasing (:371-381); no duplicate live places/claims; no live value with `suspension == Forbidden`; per-crossing `preserve_cpu` must EQUAL `any(live.cpu == Origin)` and likewise host_thread (:406-422); argument layout must equal its recomputed canonical packing (:424-429); activation-wide `carry_obligations` may not UNDERSTATE the crossings' requirement (:433-440) but may overstate. Projection must have a valid identity (`has_valid_identity`) and `candidate.stack_plan == projection.stack_plan()` (:324-334).
**Cases by shape.** `may_block` is carried and hashed (:289, :462) but never validated against anything in this file (read). `terminal_outcome_layout`, `calling_plan`, `cancellation_required` are copied through unvalidated.
**Same job elsewhere.** `normalized_id!` here (identities.rs:10-31) and in executable-installation authority_digests.rs:14-35 are two copies of the "nonzero u64 newtype" macro, differing only in error type and field visibility (read).
**Stated limits.** none.
**Fail-open.** `validate_wcsu_activation_plan` does not check `projection.unresolved_calls()`; a plan whose composed stack covers only part of the call graph validates. Stated by design: stack_composition.rs:9-12 "keeps the composed demand partial so a runtime never mistakes a covered subgraph for the whole call graph". Grepped `unresolved_calls\(\)` under `omega/compilation`: only stack_composition.rs:378, :695 and its test :733 — no outside reader (so who rejects a partial plan is "open", not traced beyond this crate).

## T identities.rs (93 lines) — read in full
`normalized_id!` (nonzero u64, `pub(crate)` field) used for 20 ids :33-91; `commitment_digest!` (32-byte wrapper with `from_digest`, `is_zero`) :36-55. Used by lib.rs:84 (`TaskSpecializationCommitment`) and stack_composition.rs:36-37.

## T source_guards.rs (53 lines) — read in full
Test-only: no `checked_trees::`, `typed_trees::`, `TypeReferenceHandle`, `SymbolHandle` text in the crate; no `identity: String`-like fields; ALLOWED empty. The crate nevertheless has `pub provider_plan_name: String` (lib.rs:89) and `UnresolvedCallSite.state: String` (stack_composition.rs:157), which the needles do not cover.

## T activation.rs (512 lines) — read in full
**Purpose.** `realize_task_activations` closes Terminal task activation demands under a selected `TaskRuntime` provider plan and a target profile: computes storage geometry, stack demand, crossings and a `TaskActivationPlanSet`.
**Shape.** `realize_task_activations` :20; `selected_task_runtime_provider` :151; `TaskStorage` :233 (`new` :242, `value` :318, `frame` :338); `append` :417; `align` :425; `crossing` :440; `signature_layout_identity` :479; `normalized_id` :503; `plan_diagnostic` :510.
**How it decides.** Provider selection filters selected plans on: `demand.requirement_in_scope`, trait name TEXT == requirement owner path, `trait_package_identity == requirement_owner_package`, and some method whose `requirement_owner` text, owner package and requirement-identity TEXT equal the demand's (:160-181); exactly one plan and exactly one method must match, and exactly one plan row must bind that method (:183-224). `requirement_name` ("start"/"try_start") is used ONLY in messages (:191-194, :205, :209, :221), not in matching. Runtime id = `plan.report_fingerprint()` (the 64-bit FNV of the provider plan, :225). Calling plan id is a FNV over target wire identity + entry fingerprint + layout identities (:59-63), not a computed ABI plan. Per-state frame bytes start at one pointer-size slot (:344-348) and take the max over states and crossing argument sets.
**Cases by shape.** Layout of structural types goes through `StructuralLayouts` per the target's record realizations; target layout applications require "exactly one selected target realization" matching `profile.target_name()` (:250-258) — string compare. `TerminalTaskValueType::{Unit, Scalar, Structural}` only.
**Fail-open.** `if demands.demands.is_empty() { return Ok(default) }` :26 — no demands → empty set, by design. `TaskStorage::new` ends with `layouts.shape(identity).ok()` mapping failures to `None` (:313) but earlier validates each referenced structural type (:301-309). none seen otherwise.

## T stack_composition.rs (743 lines; test :700-743) — read in full
**Purpose.** Whole-call-graph worst-case stack usage for one fixed-stack task: frame summaries, same-stack contribution admission, composition and projection.
**Shape.** `SameStackContributionAdmissionCandidate` :27; `AdmittedSameStackContribution` :40; `admit_same_stack_contribution` :85; `StackCallContribution{Checked,AdmittedSameStack}` :143; `UnresolvedCallSite` :155 (state is a `String`); `TaskStackFrameSummary` :175; `validate_task_stack_frame_summary` :193; `task_stack_frame_validation_identity` :237; `ComposedTaskStackDemand` :278 (private; includes `evidence: TaskStackCompositionEvidence{root, frames}` :272, :288, :508); `WcsuStackPlanProjection` :334 (+`has_valid_identity` :393 pub(crate)); `project_wcsu_stack_plan` :402; `compose_task_stack_demand` :417; `collect_reachable` :512; `compose_frame` :542 (memoised recursion); FNV fingerprints :583-698; `admitted_contribution_commitment` :655 (SHA-256, domain `omega.task.same-stack-contribution.v1\0`).
**How it decides.** Admission requires: nonzero selected plan report identity and commitment, candidate's report identity == selected, candidate commitment nonzero and == selected, selected requirement identity present and == candidate's, alignment power of two (:91-125). Composition: no duplicate frames, root present, every `Checked` callee present, DFS cycle detection (error "non-lowered call cycle"), every frame reachable from root (else error "unreachable frame"). Peak of a frame = max(local, max over children of `align_up(local, child_alignment) + child_bytes`); siblings share capacity (children are all placed after the parent's whole local area, :564-568).
**Fail-open.** (1) `validate_task_stack_frame_summary` (:193-235) never recomputes or checks `summary.validation` against `task_stack_frame_validation_identity` (:237); the supplied id is trusted. In this crate the one producer (activation.rs:398-413) computes it correctly ("traced"); `grep validate_task_stack_frame_summary` shows other producers may exist (not traced: "open"). (2) `AdmittedSameStackContribution.bytes` is taken from the candidate (:137) — admission checks provider identity/commitment, not that the bytes come from the plan. (3) `ComposedTaskStackDemand.evidence` (:288, :508) is written and never read in the crate (grepped `\.evidence\b` in task-plans: none; hits elsewhere are other types). (4) `has_valid_identity` (:393) recomputes the projection id from the stored demand's own fields (:669-698), not from re-composing the frames in `evidence`; fields are private and the only constructor is `project_wcsu_stack_plan` (:402), so a bad demand cannot be forged outside the crate ("traced" for field privacy).
**Same job elsewhere.** Three FNV-based fingerprint writers over the same frame/call structure (`task_stack_frame_validation_identity` :237, `composition_report_fingerprint` :583, `hash_unresolved_calls` :620). Alignment helpers `align` (activation.rs:425) and `align_up` (stack_composition.rs:576) and the inline mask in `pack_task_argument_fields` (lib.rs:187-189, :206-208) do the same round-up three times (`align_up` does not check `alignment == 0` or power-of-two itself; `alignment - 1` would underflow for 0 — callers validate alignment first via `validate_alignment`).

# target (prefix `G` = `omega-rust/omega/compilation/backend/target/src/`)

## G lib.rs (116 lines) — read in full
Module list and re-exports; `HostedApplicationIntent{Console,Gui}` :69; `NativeApplicationMetadata` :75 (`subsystem: u16`, optional name/intent/identifier/elf_interpreter); `ApplicationIdentifier::new` :87 (ASCII, charset `A-Za-z0-9.-`, no empty `.` segment, non-empty). The `target_profile.rs`, `target_semantics.rs`, `x86_features.rs` files named here are NOT in this unit (not read) — only referenced.
## G compatibility_fingerprint.rs (40 lines) — read in full
`Fnv1a` (length-framed `bytes`) wrapping `semantic_vocabulary::Fnv1a::{OFFSET_BASIS, PRIME}`. Second FNV implementation next to `semantic_vocabulary::Fnv1a` (used at e.g. effects `provider_plan.rs:473` and task-plans `lib.rs:64`) — header says the difference is the length framing in `bytes` (:3-7).
## G component_locator.rs (228 lines) — read in full
`NormalizedComponentLocator` :12 (new :20: target must be Linux x64/arm64, macOS x64/arm64 or Windows x64 — UEFI/others rejected :25-34; nonzero image/slot commitments; SHA-256 identity over target name, image, slot, domain `omega.component-import-locator.v1` (no NUL terminator unlike other domains)), `for_image` :51 (hashes image bytes under `omega.component-shared-image.v1`), `image_filename` :83 (suffix by target: so/dylib/dll; `_ => unreachable!`), `export_symbol` :95 (`__omega_slot_<hex>`), `NormalizedImportLocator{Foreign, Component}` :101, `NormalizedImportLocatorRef` :115, `component_export_slot` :188 (SHA-256 of vocabulary u16, subject, calling application). `non_authoritative_compatibility_fingerprint` for components takes the first 8 digest bytes (:75-81, :179-183) — duplicated in two places.
## G elf_loader.rs (242 lines) — read in full
`ELF_INTERPRETER_TARGET_PROFILES = [LinuxArm64, LinuxX64]` :12; `declared_elf_interpreter_default_path` :15 hard-codes `/lib64/ld-linux-x86-64.so.2` and `/lib/ld-linux-aarch64.so.1` and lists every other `TargetProfile` explicitly (:19-25); `normalize_elf_interpreter_plan` :89: target in the profile list, non-empty, starts with `/`, no NUL byte; stores raw bytes + FNV fingerprint (domain `omega.elf-interpreter-plan.v1`).
## G explicit_target_set.rs (176 lines) — read in full
`ExplicitTargetSet::from_caller_names` :17: rejects empty, rejects the spellings `"all"` and `"*"` (string compare :33), maps each name through `TargetProfile::from_omega_target_name`, de-duplicates, orders by `TargetProfile::ALL`. Tests include retired aliases `linux_x64`, `windows_x64`, `uefi_x64` rejected (:105) and accepted spellings `linux_x86_64`, `windows_x86_64`, `uefi_x86_64`, `cross_platform_cli`, `local_unchecked`.
## G foreign_locator.rs (787 lines; tests :297-787) — read in full
**Purpose.** Validates and seals `ForeignLocatorCandidate` (PeByName, PeByOrdinal, ElfVersioned, MachODylibSymbol) for a target; produces a FNV compatibility fingerprint and a SHA-256 identity digest.
**Shape.** `NormalizedForeignLocator` :16; `ForeignLocatorIdentityDigest` :24; `evaluated_syscall_identity_digest` :37 (`#[doc(hidden)]`, domain `omega.evaluated-syscall-binding.sha256.v1\0`); `ForeignLocatorValidationError` :68; `normalize_foreign_locator` :121; `validate_target` :138; `validate_coordinates` :162; `non_authoritative_compatibility_fingerprint` :217; `identity_digest` :257.
**How it decides / cases by shape.** Applicability is by exact `TargetProfile`: `PeBy*` only `WindowsX64` (not UEFI, "does not treat all COFF as PE import hosts", test :375); `ElfVersioned` only LinuxArm64/LinuxX64; `MachODylibSymbol` only MacosArm64/MacosX64 (:142-152). Coordinates must be non-empty and NUL-free; `PeByOrdinal` ordinal nonzero.
**Same job elsewhere.** The `match locator` serialization is written twice here (:224-253 FNV, :264-293 SHA-256) with tags 1..4, and a third time in `digest_encoder.rs:269-302` (effects crate) with tags 0..3 and the PE ordinal written raw (no length frame, :282). `normalize_foreign_locator` is also called by `runtime_abi::descriptor_import_identity` (descriptor_io.rs:74) to rebuild the expected candidates by string literal.
**Fail-open.** none seen.

# Grepped counts and cross-site facts (commands are Grep tool calls over `omega-rust`, `*.rs`)

- `validate_step` (post-handoff-writer-plan lib.rs:497): 2 external callers, isa_x86_64 `post_handoff_writer.rs:279` and isa_aarch64 `post_handoff_writer.rs:271`; both then check `used_slots.iter().any(!used)` -> "not a dense exact set". The `*used = true` before checks (lib.rs:509) therefore only matters on the success path (both callers return early with `?`). Not "unused". `validate_fragment` in `validate_structure` (:244) is a separate bit-range validator; same job as `validate_step` for the container/bit checks, two implementations (read, layout_plans side not read).
- `runtime_abi::descriptor_import`: 3 external callers (target-operations/descriptor_input.rs:195, terminal-runtime host_imports/descriptor_io.rs:194, host_imports/filesystem.rs:368); `descriptor_import_identity`: 1 external caller (terminal-runtime native_provider.rs:181) plus its use inside `descriptor_import`. `descriptor_import` has no logic of its own (descriptor_io.rs:28-35).
- `infrastructure_environment_agrees` (register-model identities.rs:152): 3 external callers (05_selected-instructions-to-register-homes/home_assignment.rs:2384, selected-instructions/analyses/spill_choice.rs:128, fixed_precolored_segment_homes.rs:770).
- `uefi_os_handoff_exhaustion_requires_error` (program-entry-plan lib.rs:69): the grep found only its definition; 0 external call sites in `omega-rust` `*.rs`.
- `exact_hosted_physical_boundary_entry_plan`: 1 external caller (image-emission/src/entries.rs:3024).
- `ProviderPlanInput::read`: 2 non-test external callers (compiler/tests/support/native_component_descriptions.rs:175 is a test support file; artifacts/component-description/src/component_verification.rs:804 is non-test).
- `resolve_selected_provider_grants`: effects provider_grants.rs:64; callers provider-selection/binding.rs:68, trust-model/admission_settlement.rs:41, accepted_machine_grants.rs:29; `resolve_authored_selected_provider_grants` caller binding.rs:70. In accepted_machine_grants.rs:25-60 a root grant not resolved as a provider grant falls through to `resolve_non_provider_trust_grant`, so the silent `continue` at provider_grants.rs:80-82 does not by itself drop the grant in that caller ("traced" for that caller; in binding.rs:68-76 `bind_selected_provider_plan_facts` I read only the call and `validate_qualification_grants`; whether an unresolved root grant is later rejected there is not traced: open).
- `validate_against_schema` callers: effects provider_evidence.rs:214, component_verification.rs:294; `validate_candidate_against_schema`: provider_evidence.rs:276, provider-planning prepared_providers.rs:279; `covers_schema`: provider-selection choice/selection.rs:117 (1 caller).
- `DigestFeed` / `ProviderPlanDigestEncoder` appear in 4 files (16 occurrences, count mode): provider_evidence.rs (2), provider_plan.rs (5), service_schemas.rs (3), digest_encoder.rs (6). `DigestFeed` is declared the shared framed-feed trait (digest_encoder.rs:1-5 comment); outside this crate: 0 users.
- `with_build_origin`: 2 external callers (provider-planning native_bindings/plans.rs:331; 00_terminal-psi-to-abstract-operations/retained_terminal_vocabulary.rs:805 and :896). `EvaluatedForeignImport::from_retained_evidence`: external callers provider-planning binding_evaluation.rs:195, native_bindings/plans.rs:344; `EvaluatedForeignSyscall::from_retained_evidence`: binding_evaluation.rs:200, plans.rs:347.
- `SelectedProgramStorageEntryPlan::from_target_slot` and `ProgramEntryPhysicalContractPlan::new`: 1 external caller each, provider-planning/src/program_entry.rs:710 and :721.
- `bind_optimized_program_storage_semantic_entry_contract`, `plan_optimized_program_storage_semantic_wrapper`, `select_optimized_program_storage_semantic_wrapper_encoding`: grep over `omega-rust` `*.rs` finds only the definitions and the three re-exports at program-entry-plan lib.rs:41/46/47 — 0 callers outside the crate. `grep "OptimizedProgramStorage|optimized_program_storage"` files: program-entry-plan lib.rs, optimized_semantic_entry.rs, optimized_semantic_wrapper.rs, and 4 files in optimization-core (not read; identity types). So about 1183 lines (548 + 635) of the unit have no call sites. `validate_optimized_program_storage_semantic_wrapper_encoding` (wrapper :300) is `pub(crate)` with no caller other than `select_...` :296.
- `installed-writer` feature: grep finds program-entry-plan Cargo.toml:13 and lib.rs:28, :49 only; no other Cargo.toml enables it. So `program-entry-plan/src/post_handoff_writer.rs` (330 lines) and its dependency on `executable_installation` are compiled only when someone passes that feature (none found).
- `executable_installation`: no code in `omega-rust` constructs `Artifact`, `InstalledCode`, `ValidatedPlacement` etc. (see crate section), so `populate_post_handoff_entry_writer_context` has no value to be called on outside tests that may not exist.
- `units_for(`, `view_ids(`, `complement(` (register-model lib.rs:223/:241/:230): counted 20 total lines across 3 files: register-model/lib.rs (2 = definitions), isa-x86_64 register_model.rs (10), isa-aarch64 register_model.rs (8). All outside callers are the two ISA crates.
- `Sha256RegisterDigest`/`sha256_register_units`/`sha256_length` callers: register-homes lib.rs:283/:598, 05_selected-instructions-to-register-homes/preservation.rs:460-472, machine-emission/frame_layout.rs:741-761.
- `component_export_slot`: `target::component_export_slot` called from target-operations boundary/realizations.rs:232, native-artifact component_product.rs:61, provider-admission settlements.rs:357, and wrapped by `native-compiler/src/realization/component_exports.rs:117` (a second function of the same name) which is called at component_exports.rs:100 and component_calls.rs:65. `fat_descriptor_layout`: `runtime_abi::fat_descriptor_layout` wrapped by `backend/layout/src/lib.rs:67` (another function of the same name) and called from calling-conventions terminal_telescope.rs:140.
- `NativeTarget::windows_x64()` vs `uefi_x64()`: identical values (target_profile.rs:695-711).
- Stale test allow-list: `find . -name 'service_establishment*'` under `omega-rust/omega/compilation` returned nothing, but program-entry-plan `source_guards.rs:24-28` lists five `selected_entry/service_establishment.rs` entries.

# Coverage
- Read in full: all 67 files in the unit file, every line (total 21,967 lines; `awk` sum of the line-count column = 21967 over 67 rows). Skimmed: none. Not read: none.
- Files named in comments but outside the unit and not read: `target/src/target_profile.rs` (only grepped lines 541-546, 668-711), `target_semantics.rs`, `x86_features.rs`, `layout_plans`, `language_semantics::service_surface`, `optimization_core`, ISA post_handoff_writer files (read 12 lines each around the `validate_step` call), provider-selection/binding.rs (read ~45 lines), trust-model accepted_machine_grants.rs (read ~36 lines).

# Five things a maintainer most needs to know about this unit
1. `executable-installation` has no constructor for its own carriers: grep shows no struct literal for `Artifact(Arc<ArtifactRecord>)`, `ArtifactRecord`, `CodePlacement`, `FrozenPlacement`, `ValidatedPlacement`, `AdmittedArtifact` or `InstalledCode` anywhere in `omega-rust`; `placement.rs` and `installation.rs` hold types without transitions; `InstallationAudience` and two struct fields carry `#[expect(dead_code ...)]` citing TASKS.md COMPONENT-SUBSTRATE. The only consumer is `program-entry-plan` behind the `installed-writer` feature, which nothing enables.
2. `ProviderPlan` has four separate identity/ordering renderings that must be kept in sync by hand: the SHA-256 encoder (`digest_encoder.rs`), its inverse reader (`retained_inputs.rs`, which skips fields by position and tag), the `format!`-and-Debug text feeding the 64-bit FNV `report_fingerprint` (`provider_plan.rs:291-477`, which includes `{:?}` of `completion`, `abi`, partitions and of any binding not special-cased), and the equality impl (`provider_plan.rs:142`, text-based). `selection_order` and `TaskRuntimeId` (task-plans activation.rs:225) key on the 64-bit FNV.
3. `TargetRegisterEnvironmentIdentity` omits `ieee_float_directed_unary` (constraint_catalog.rs:150 not hashed in identities.rs:184-308; tests never mutate it, declared_identities.rs:210). `NativeTarget` cannot distinguish Windows x64 from UEFI x64, so NativeTarget-keyed logic (environment cache, `optimized_semantic_entry.rs:212`) treats them as one.
4. The "optimized semantic ProgramStorage" entry/wrapper pair (`optimized_semantic_entry.rs`, `optimized_semantic_wrapper.rs`, ~1,183 lines) is hard-wired to UEFI x86-64 Microsoft ABI with literal registers (RCX/RDX), offsets (32/40/48/56), frame (72) and call index (8), and has 0 call sites outside the crate's own re-exports.
5. Task stack composition accepts partial call graphs by design (`unresolved_calls`), and no reader of `unresolved_calls()` exists outside `stack_composition.rs`; `evidence` inside `ComposedTaskStackDemand` is never read; `validate_task_stack_frame_summary` trusts its `validation` field.

# What you are unsure about
- Whether the stale-memo risk in `ProviderPlanDigestMemo` (pub fields mutated after `identity_digest()`) is reachable: I did not grep mutation sites of `ProviderPlan` fields.
- Whether `ProviderPlan::eq_in` comparing `requirement_identity` as raw ids across different tables (provider_plan.rs:172) is hit with plans built in different intern orders; I did not trace construction sites.
- Whether the root-grant `continue` at provider_grants.rs:80-82 is caught downstream in `bind_selected_provider_plan_facts` (only partly read).
- The one `executable_installation` hit in `post-handoff-writer-plan/src/lib.rs` (grepped as text `executable_installation::|executable-installation`) — I did not check whether it is a comment or a dependency.
- Whether the other ISA/pipeline crates re-derive hard-coded values duplicated here (Linux exit_group `231`/`94` in program_entry_physical.rs:406-409/:445-448, register choices in the UEFI wrapper): marked "suspected".
- I did not read `layout_plans::validate_fragment` to confirm it duplicates `post_handoff_writer_plan::validate_step`.

