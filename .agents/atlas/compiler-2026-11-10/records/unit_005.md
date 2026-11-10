# Unit 5 notes

Unit 5 covers (all under `<checkout>/`, paths below relative to `omega-rust/omega/compilation/backend/images/`):

- `image-emission/src/function_fragments.rs` lines 12463-24923 (12461 lines; the file is 24923 lines, lines 1-12462 belong to another reader)
- `image-emission/src/image.rs` 1-2028
- `image-emission/src/installation_record.rs` 1-237
- `image-emission/src/lib.rs` 1-135
- `image-emission/src/object.rs` 1-931
- `image-emission/src/object_artifact.rs` 1-5506
- `image-macho/src/code_signature.rs` 1-225

Method: I read with the Read tool; counts come from Grep (count/content mode) over `omega-rust`, command named per claim. Test modules were the first thing I skipped when short of room (see Coverage). "Source-file" below means `function_fragments.rs` unless stated. Module map for the file (from grep `mod`): `source` = 4177-20184; inside it `placed_reference` 10979-13454, `primitive_accesses` 13456, `spill_values` 13552, `stack_calls` 13920, `structural_case` 14024, `structural_fields` 14464, `structural_homes` 18174, `tests` 19719; then `structural` 20187-24100 (byte_input, byte_output, established_views, primitive_locals, process_exit, scalar_result, validation + helpers 23425-24099); `validation` 24102-24700; error enum 24702; `host` 24910; `refuse_symbol_collision` 24914.

---------------------------------------------------------------------------------------------------

## 1. lib.rs (1-135) - read in full

**Purpose.** Crate root of `image_emission`. Declares modules, a tiny byte-reader helper module, an ordering helper, and re-exports the public surface (object-artifact builders, entry bindings, image emitters, component images, validators). Header comments (lib.rs:1-37) describe the pipeline: `function_fragments.rs` builds `ObjectArtifact`, `object.rs` composes private/component objects, `entries.rs` binds entry shims, `image.rs` emits images, `installation_record.rs` "defines the installation error surface".

**Shape.** `mod codec_bytes` (lib.rs:41-53: `u16_at`, `u64_at` local; `u32_at` re-exported from `optimization_core`). `mod order { strictly_increasing }` (lib.rs:82-86). `mod object` aliases: lib.rs:56 `pub(crate) mod function_fragments`, :59 `pub(crate) mod object_artifact`; `object.rs:432-433` re-exports them as `crate::object::function_fragments` / `object_artifact`, so each path is reachable two ways.

**Inputs/outputs.** Pure re-export file; `pub use machine_code::BoundaryExecutionRecord` at lib.rs:130.

**How it decides.** Nothing.

**Cases by shape.** None.

**Same job elsewhere.** `u16_at/u64_at` duplicate the pattern of `optimization_core::codec_bytes::u32_at` (lib.rs:42 re-export) - the crate wraps one function from outside and writes two more locally (grepped `\bu16_at\b|\bu64_at\b` in image-emission: lib.rs 2, entries.rs 4 -> used only by `entries.rs`, a file outside my unit).

**Unused / one-caller / shared claims.** `strictly_increasing` (lib.rs:83): grepped `strictly_increasing` across omega-rust within image-emission -> 1 caller, `function_fragments.rs:325` (plus the import at :10). The comment lib.rs:11 says `function_fragments.rs` is "the sole builder of `ObjectArtifact`": grepped `\bObjectArtifact \{` over omega-rust -> the only struct literal construction is `function_fragments.rs:3821` (other hits are `impl`, `fn ... -> &ObjectArtifact`). Composition (`object.rs:467`) makes new artifacts with `.clone()` and mutation, not a literal. So the claim holds for literals; clones mutated in `object.rs` are a second path that creates modified artifacts.
`can_emit_executable_image` is re-exported at lib.rs:125 but grepped across omega-rust shows only the definition (image.rs:1806), the re-export and one call (image.rs:1824): zero callers outside image.rs.
`InstallationError` re-exported at lib.rs:129: see section 3.

**Stated limits.** None.

**Fail-open risks.** None seen.

---------------------------------------------------------------------------------------------------

## 2. code_signature.rs (image-macho, 1-225) - read in full

**Purpose.** Builds and parses the ad-hoc Mach-O code signature (SuperBlob with CodeDirectory, an empty requirements blob, an empty entitlements blob) over the finished container prefix, one SHA-256 per page (16 KiB on AArch64, 4 KiB on x86-64 per header comment code_signature.rs:1-6).

**Shape.** `CodeSignatureLayout` (:18-29) with `for_code_limit` (:32) computing offsets; `code_signature_size` (:60, `pub(super)`); `macho_ad_hoc_code_signature_for_container` (:64, `pub(super)`) writes big-endian blob (magic 0xfade0cc0, 3 sub-blobs, CodeDirectory 0xfade0c02, version 0x20400, 2 special slots, SHA-256 hash size 32, hash type 2); `requirements_blob` (:150, 12 constant bytes) and `empty_entitlements_blob` (:154); `pub fn code_signature_identifier` (:158) parses an executable's LC_CODE_SIGNATURE (cmd 0x1d) and returns the identifier string.

**Inputs/outputs.** Reads code bytes, `executable_segment_limit`, identifier, `MachoIsa`, `main_binary`; writes a `Vec<u8>` padded to 16 (:142). Parser reads Mach-O header offsets (`command_count` at byte 16, load commands from byte 32, :168-169).

**How it decides.** Parser keys on load-command number 0x1d and on exact magic constants (:174, :188, :192); more than one 0x1d command -> `None` (:175-183). Hash only the page prefix up to `code_limit` (:131-136).

**Cases by shape.** The special-slot count is hard-coded to 2 (`SPECIAL_SLOT_COUNT`, :16: requirements + entitlements slots; the first slot hash is of the requirements blob, the second is 32 zero bytes at :127-128, i.e. slot for entitlements is zeros even though an empty entitlements blob is appended). Hash is always SHA-256 (:102-103). Parser assumes a 64-bit header (32-byte header, :169) and little-endian.

**Same job elsewhere.** `code_signature_identifier` is called by `image-macho/src/shared_component.rs:580`, `native-artifact/src/package_publication.rs:330` and `:409`, and in tests (`image-macho/src/lib.rs:578-592`). Grepped `code_signature_identifier\(|macho_ad_hoc_code_signature_for_container|code_signature_size\(` in `omega/compilation/backend`: `code_signature_size` 1 non-test caller (`file_layout.rs:194`), `macho_ad_hoc_code_signature_for_container` 1 non-test caller (`lib.rs:867`). The method `ExecutableImageEmissionRequest::code_signature_identifier` (image.rs:1637) shares the name but is a different function.

**Unused/one-caller.** See previous line; the two `pub(super)` helpers have one production caller each.

**Stated limits.** None.

**Fail-open risks.** Parser: every malformed input returns `None` (uses `?`). Package publication turns `None` into an error (`package_publication.rs:330` `ok_or_else`, grepped, not read further). None seen.

---------------------------------------------------------------------------------------------------

## 3. installation_record.rs (1-237) - read in full

**Purpose.** Holds one helper and one error enum. The header comment (:1-4) says it defines "structural graph helpers and `InstallationError` diagnostics for canonical installation records over sealed executable images".

**Shape.** `graph_structural::indirect_source_location` (:9-26) maps `IndirectPointerLocation::{Register,Stack}` to `StructuralSourceLocation::{IncomingIndirectPointer, IncomingIndirectStackPointer}`. `InstallationError` (:36-230) is a ~190-variant enum (all unit/tuple/struct variants naming limits like `TooManyInstalledFunctions`, `NonCanonicalEncoding`, `InvalidMagic` ...), `diagnostics::debug_display!` (:232), `impl Error` (:234).

**How it decides.** Nothing (data only).

**Same job elsewhere.** A different type with the same name exists in `omega/compilation/backend/runtime/executable-installation/src/installation.rs:85` (a struct, re-exported at that crate's lib.rs:56). There is also `ProviderInstallationError` in `provider-admission`.

**Unused/one-caller (greps).**
- `indirect_source_location`: grepped `indirect_source_location|graph_structural` in image-emission/src -> 1 caller, `function_fragments.rs:23632` (inside `incoming_location`, which I read).
- `InstallationError` enum: grepped `InstallationError::` in `**/image-emission/**` -> 0 matches; grepped `image_emission::InstallationError|use image_emission::.*InstallationError|image_emission::\{[^}]*InstallationError` over omega-rust -> 0 matches (single-line forms only; a multi-line `use` list could be missed). The only references in image-emission are the definition and `lib.rs:129` re-export. So the ~190-variant error enum has no constructor and no named importer that I could find: **a documented crate surface ("defines the installation error surface", lib.rs:16) with no users found.** Marked: grepped, multi-line `use` not excluded.

**Stated limits.** None.

**Fail-open risks.** None seen.

---------------------------------------------------------------------------------------------------

## 4. object.rs (1-931) - read in full

**Purpose.** Three submodules: `component_object` (turn a body `ObjectArtifact` plus an entry transport into a component function image with an exported slot symbol), `component_shared_image` (emit that image as an ELF/Mach-O/PE shared component and re-check the entry evidence), and `private_object_composition` (link private callback function objects into a root artifact without colliding symbol names).

**Shape.**
- `ComponentFunctionImage` (:18-24: `body`, `entry_bridge`, `image: FinalImage`, `export_symbol`, `slot_commitment: [u8;32]`), `prepare_component_function_image` (:35), `component_export_symbol` (:149; name = `__omega_slot_` + 64 hex digits).
- `ComponentSharedContainer` enum Elf/MachO/Pe (:164), `ComponentSharedImage` (:194) with `stack_demand` (:215), `ComponentSharedImageEmissionError` (:233), `emit_component_shared_image` (:238), `emit_shared_function_container` (:286), `validate_component_entry_effects` (:343).
- `compose_private_function_objects` (:449), `private_symbol` (:526), `append_body` (:549), `align_section` (:772), `section_offset` (:797), `remap` (:808), `require_definitions` (:821), `ObjectNamespace` (:836), `namespaces` (:864), `validate_composition` (:888).

**Inputs/outputs.** Reads `ObjectArtifact` fields (`object`, `relocations`, `text_bytes`, `data_bytes`, `private_functions`, `composition_source`), the `image` crate (`build_final_image`), `machine_emission::native_entry_bridge`, the per-format emitters `image_elf/image_macho/image_pe`. Produces a `FinalImage` with an appended bridge in `.text` (16-byte aligned, :84-92) and one relocation to the body (:100-115); `compose` produces a new `ObjectArtifact` whose `composition_source` is the original (`Arc`, :466-468) and appended `ObjectCompilerPrivateFunction` rows (:754-768).

**How it decides.**
- Component prepare: rejects zero slot commitment, hosted-receiver binding, physical-entry binding, or any private function on the body (:42-50); requires entry bridge request to equal a recomputed `native_entry_request` (:75-81).
- Emission dispatch keys on `image.target.object_format`: Elf, MachO, or `Coff` only if target == `windows_x64()` (:292-340).
- Entry-effect validation keys on byte spans: finds the region with identical `section_offset`/`byte_count` as the export symbol (:351-356) and requires `entry_effects` present and equal to `bridge.complete_effects()` (:394), then decodes the bridge bytes with `isa_x86_64::decode_x86_64_generated_entry` / `isa_aarch64::...` (:411-419) and compares to `bridge.generated()`.
- Composition: private bodies must have unique identity, be demanded transitively by function-symbol references (:471-486, :488), have `callback_thunk_placement_index()` (:490) and no nested composition/hosted receiver/physical entry (:491-495).

**Cases by shape.** `emit_shared_function_container` match :292-340 (ELF, Mach-O, PE only for `windows_x64`; other targets: "shared component requires an admitted ELF, Mach-O, or Windows PE container", :337). `section_offset` only Text and Data sections (:797-805); `compose` only Text/Data sections (:520). `align_section` pads to 16 (:782).

**Same job elsewhere.** `entry bridge + relocation` appended in `prepare_component_function_image` (:93-115) and in `append_body` (:714-748) do the same thing (append bridge bytes, add Aarch64Branch26/X86_64Relative32 call relocation to body symbol) in two places; also `image.rs entry_effects::validate` (image.rs:370-430) re-decodes the same bridge bytes the same way as `validate_component_entry_effects` (:411-426) - near-identical decode/compare blocks. Read both.

**Unused/one-caller.** Grepped `compose_private_function_objects|create_private_function_object|create_installed_private_function_object` over omega-rust: 12 occurrences in 5 files (callback_thunks.rs 2, fragment_object_publication.rs 1, object.rs 2, lib.rs 3, entries.rs 4) - in use. `derive_stack_demand|derive_body_stack_demand`: 13 occurrences in 5 files, used. Comment claim: object.rs header says `private_object_composition` embeds private callbacks; lib.rs:21-29 says "we embed complete private callback objects via `compose_private_function_objects`". Users outside image-emission: `native-compiler/src/realization/callback_thunks.rs` (2) - grepped only.

**Stated limits.** `Error::Unsupported("private object section kind")` (:520, :780, :804) is the only stated limit.

**Fail-open risks.**
- `validate_composition` returns `Ok(())` when `composition_source` is `None` (:889-891). Traced: it is called from `validation::validate_function_fragment_object_artifact` only inside `if artifact.composition_source.is_some()` (function_fragments.rs:24407-24416), and also from other places I did not read; for an artifact with no composition there is nothing to compare, so no wrong program gets through here (traced for the one site read).
- `compose_private_function_objects` with empty `private_objects` returns `source` unchanged after `require_definitions` (:457-460). Traced: `validate_installed_stack_requirements` runs before it (:453), so installed stack guards are still checked.
- None else seen.

---------------------------------------------------------------------------------------------------

## 5. image.rs (1-2028) - read in full

**Purpose.** Turns a sealed `ObjectArtifact` into an executable image through the direct ELF/Mach-O/PE writers or a dynamic-ELF lane, attaches entry-shim effect evidence to executable regions, and re-validates a final image against the object (terminal image replay).

**Shape.**
- `dynamic_elf` (:8-278): `RequestedDynamicElfImage` (:24), `validate_requested_dynamic_elf_image` (:43), `DynamicElfImageEmission` (:82), `DynamicElfOrchestrationError` (:115), `emit_dynamic_elf_image` (:152), `validated_output` (:190), `validate_dynamic_elf_image_emission` (:220), `derive_output` (:234).
- `entry_effects` (:280-499): `attach` (:286), `validate` (:347), `validate_body_call_target` (:481).
- `final_image_validation` (:501-1530): `implementation_tables::validate_targets` (:523), `native_code_layouts::validate_targets` (:629), `LinkedNativeCodeLayout`/`LinkedNativeCodeRegion` (:733-746), `inspect_native_code_layouts` (:748), `validate_terminal_image` (:803), `validate_terminal_image_with_import_count` (:833), `validate_byte_literal_address_targets` (:1081), `encoded_final_data_with_image_import_slots` (:1147), `validate_callback_relocation_targets` (:1182), `decode_x86_relative_target` (:1329), `decode_aarch64_page_target` (:1348), tests (:1386-1529).
- `image_output` (:1532-2028): `ImageApplicationMetadataError` + `settle_application_metadata` (:1533-1599), `ExecutableImageEmissionRequest` (:1619), `settle_elf_loader` (:1651), `RequestedExecutableImage(Error)` (:1696, :1712), `emit_executable_image` (:1745), `can_emit_executable_image` (:1806), `emit_direct_executable_image_signed` (:1819), `validate_direct_executable_image(_output)` (:1882, :1889), `ExecutableImage` (:1969).

**Inputs/outputs.** Reads `ObjectArtifact`, `hosted_unit_entry::prepare/prepared_parts` (entries.rs, not read), `image::build_final_image`, per-format emitters (`image_elf::emit_elf_executable`, `image_macho::emit_macho_executable[_signed]`, `image_pe::emit_pe_x86_64_executable`, `emit_elf_dynamic_executable`). Produces `EmittedImageOutput` with `compiler_text_validation` and `compiler_function_validation` evidence attached (:1859-1865) and an `ExecutableImage` (:1866-1879).

**How it decides.**
- Dynamic vs direct: `requires_dynamic_elf` = target is ELF and `layout.normalized_imports` is non-empty (:1689-1692). Matching (requires_dynamic, request) at :1756-1797 with four arms; mismatches produce errors.
- Final-image validation keyed on counts, offsets and byte equality: import count equals count of `SymbolKind::Import` symbols (:816-821, :865); relocation count equals `relocations.record_count()` (:870); compiler function regions are matched to functions by `(symbol name, section_offset, byte_count)` and must match exactly once (:944-960); private helpers likewise (:991-1015); count of compiler regions equals `artifact.functions.len()` + private bodies' functions + bridges + 1 if shim (:928-943).
- `settle_application_metadata` (:1563-1597): if Direct and `subsystem` differs -> `SubsystemMismatch`; identifier mismatch compares strings (`requested != authored.as_str()`, :1578-1583); Mach-O GUI intent without identifier -> error.
- Entry shim switch (:1017-1053) keys on `EntrySurface` enum (ProvisionedUnit/Physical/*Receiver/*Unit).
- Callback relocations are validated by matching `RelocationOrigin::SemanticOperation { function_symbol_handle, operation_identity }` (:1232-1237) - an identity key.

**Cases by shape.**
- `can_emit_executable_image` (:1806-1817): "pointer_size == 8 && pointer_alignment == 8" and exactly (Elf,Aarch64), (Elf,X86_64), (MachO,Aarch64), (MachO,X86_64), (Coff,X86_64).
- `validate_terminal_image_with_import_count` Mach-O-only extra checks (:842-864).
- `encoded_final_data_with_image_import_slots` only does something for Mach-O (:1153-1155): adds an 8-byte-aligned 8-byte slot per referenced import.
- `decode_aarch64_page_target` handles ADRP+ADD only (:1348-1384); x86 only rel32 (:1329-1346); callbacks only X86_64Relative32 or Aarch64PageAddress (:1238-1260).
- `entry_effects::validate` physical entry replay uses `isa_x86_64::decode_x86_64_generated_entry` only (:467): physical entry is x86-only in this check. Private-entry bridge replay chooses by architecture (:414-421).
- `validate_callback_relocation_targets` accepts only `CallSiteOwner::Operation` owners; other owners error (:1197-1207).

**Same job elsewhere.**
- The sequence "prepare entry -> build_final_image -> entry_effects::attach -> emit -> validate_terminal_image" is written three times: `emit_dynamic_elf_image` (:156-188), `derive_output` (:238-253 re-building the image to replay), `emit_direct_executable_image_signed` (:1830-1858), and a fourth time in `validate_direct_executable_image_output` (:1911-1925). Each builds `FinalImageInput { target, object, relocations, text_bytes, data_bytes }` by hand (:164, :242, :1833, :1914).
- `decode_x86_relative_target` / `decode_aarch64_page_target` (:1329-1384) vs `image::validate_relative_relocation_target` called at :497 (outside crate, not read).
- Implementation-table and native-layout address checks (:523-731) both decode the same two address-field encodings (:551-563 and :711-723) and the byte-literal check (:1125-1137) a third time - three copies of the `match address.fields { X86_64Relative32 .. Aarch64PageAddress .. }` decode.

**Unused/one-caller (greps).** `can_emit_executable_image`: 1 caller (image.rs:1824), pub-re-exported (lib.rs:125), 0 outside callers. `validate_requested_dynamic_elf_image|validate_direct_executable_image_output|validate_direct_executable_image\(`: 12 occurrences in 6 files (corpus_runner.rs 2, native_entry_controls.rs 2, lib.rs 2, image.rs 4, native_artifact.rs 1, dynamic_elf.rs 1) - used by tests and native-artifact. `inspect_native_code_layouts` not grepped.

**Stated limits.** Doc comment :62-79: `DynamicElfImageEmission` "is intentionally not [`crate::ExecutableImage`], so existing installation ... APIs cannot mistake final-byte custody" - a compile_fail doctest. Error message :1681 "missing output configuration: dynamically linked ELF output requires an interpreter pathname".

**Fail-open risks.**
- `implementation_tables::validate_targets` and `native_code_layouts::validate_targets` return `Ok(())` when the pool is `None` (:528-530, :633-635). Traced: with no pool there are no addresses/descriptors to check; a program that placed such tables without a pool would not reach here since validation of the pool presence is in `function_fragments::implementation_tables::validate` (outside unit). Not traced further.
- `validate_terminal_image_with_import_count` `EntrySurface::ProvisionedUnit => {}` (:1019): the unit-shim byte check is NOT run inside this function; callers `validate_direct_executable_image_output` (:1941-1952) and `derive_output` (:264-275) run `validate_unit_shim`, but `emit_direct_executable_image_signed` (:1851-1858) and `emit_dynamic_elf_image` -> `validated_output` (:208-215) do not run it at emission time. Traced from the callers I read: emission-time output is not shim-checked for ProvisionedUnit; replay validators are. A bad ProvisionedUnit shim emitted by the emitter would be returned from `emit_*` without that check and only caught if a later `validate_*` is run.
- `settle_application_metadata` (:1570-1589): if `self` is `DynamicElf` the subsystem/identifier checks are skipped (matches only `Self::Direct`); then only the Mach-O GUI identifier check applies (:1590-1595), which for `DynamicElf` reads `code_signature_identifier()` = `None` -> would error for Mach-O+GUI, but DynamicElf is ELF only. No wrong program found.
- `entry_effects::attach` loops private functions and errors if region not found (:307-317) - fail-closed. None else seen.

---------------------------------------------------------------------------------------------------

## 6. object_artifact.rs (1-5506) - read in full

**Purpose.** Defines the sealed `ObjectArtifact` carrier and all its sub-records, error enum, installed-interrupt stack composition, normalized foreign-call custody derivation (reconstructing argument/result transport from selected instructions and placed fragment spans), boundary-settlement replay helpers, structural layout replay, and terminal stack-demand derivation.

**Shape (with lines).**
- `carriers` (:9-537): `ObjectArtifact` (:22-51, 24 fields incl. `projected`, `hosted_receiver`, `physical_entry`, `fragment_replay`, `composition_source: Option<Arc<ObjectArtifact>>`, `requires_graph_storage_replay`, pools), `ObjectByteLiteralPool/Address` (:54-68), accessor methods (:70-297), `ObjectFunction` (:334-374, ~35 optional/vec evidence fields), `ObjectCompilerPrivateFunction` (:400), `ObjectPrivateFunctionEntry` (:415), `PrivateFunctionObject` (:449), stack records (:470-499), `ObjectBoundarySettlement`, `ObjectForeignCall` (:509), `ObjectPortEffect`, `ObjectCodeAttribution`.
- `errors` (:539-829): `ObjectError` ~110 variants.
- `installed_stack` (:831-1658): `arrival::compose` (:853), `histories` (:995), `faults::ram::validate` (:1089), `faults::derive` (:1192), `correlate_native_instruction_sources` (:1373), `installed_interrupt_stack_requirement` (:1430), `validate_installed_stack_requirements` (:1484), `root_reserve` (:1579), `requires_root_provision` (:1626), `require_installed_root_provision` (:1650).
- `normalized_foreign_calls` (:1660-3485): `descriptor_input::validate` (:1668), `floating_control::derive` (:1772), `derive_normalized_foreign_call_custody` (:1916), `fragment_instruction_span` (:2245), `transported_span_end` (:2254), `unique_block_instruction` (:2272), `durable_home_offsets` (:2292), `producer_instruction` (:2348), `scalar_observation_producer` (:2378), `callback_address_custody` (:2405), `foreign_scalar_argument_custody` (:2486), `structural_result_capture` (:2769), `foreign_result_operand_count` (:3016), `foreign_scalar_result_custody` (:3048), `scalar_result_normalization_kind` (:3186), `scalar_immediate_span` (:3221), `rematerializes_argument` (:3366), `scalar_call_result_producer` (:3408), `image_foreign_calls_match_object` (:3461), `foreign_invocation_provenance_matches` (:3468).
- `replay` (:3487-5125): `boundary::byte_sequence_custody::linux_write_line_custody_is_exact` (:3494), `completion_receipts` (:3557-4020), `result_placement::hosted_read_byte_declaration_is_valid` (:4028), `runtime_scalar_custody` (:4149-4465, process_exit decode/shape/bytes), `structural::condition_layout` (:4469-5123: `replay_structural_shape` :4487, `replay_sum_layout` :4578, `replay_structural_sum_layout` :4620, `replay_structural_value_shape` :4648, `replay_structural_projection` :4661, `replay_runtime_projection` :4826, `replay_structural_field_shape` :4873).
- `stack_demand` (:5127-5467): `derive_stack_demand` (:5135), `derive_body_stack_demand` (:5151), `derive_namespace_stack_demand` (:5163), `derive_terminal_stack_peak` (:5263), `StackDemand` (:5376).
- Tail re-exports (:5469-5485), `test_support` (:5487).

**Inputs/outputs.** `derive_normalized_foreign_call_custody(source: &StagedOptimizedRelocationFreeObjectContainer)` (:1916) reads the container's text section, selected plan, frame layout, optimized abstract plan, and writes `Vec<ObjectForeignCall>`; it rejects with `ObjectError::InvalidForeignCallProjection`/`...Argument` on any mismatch. `derive_stack_demand` reads `functions[*].unit_stack/scalar_stack/unit_call_stacks/scalar_call_stacks`, `foreign_calls` and `private_functions` and returns the peak byte ceiling.

**How it decides (keys).**
- Spans and offsets: custody is accepted when emitted spans are contiguous and ordered (e.g. `transported_span_end` :2254-2270 requires `previous end == start`; `floating_control::derive` :1853-1872).
- Identity: `unique_block_instruction` (:2272) finds an instruction id exactly once in a function; per-record checks compare `record.invocation_site` etc.
- Source value: producer found by `scalar_observation_producer` (:2378) = instruction whose operand register origin is `InstructionResult { instruction, source_value }`, unique.
- Stack: `peak = max(local peak, caller_live_bytes + callee_peak, caller_live + same_stack_contribution + callback_peak)` (:5295-5368); cycles via `active` set -> `TerminalStackCycle` (:5283-5285).
- Settlement/execution pairing (`completion_receipts::validate_completion_custody` :3598-3629) is a match on (execution, realization) pairs.
- Arrival composition keys on x86 vector numbers (`gate/16`, :922-936), dedicated stack classes (`EntryStack::Dedicated{class}`, :927-933).
- Installed fault analysis (`faults::derive` :1192) classifies each physical instruction by `Trap::NeverV1`/`MayArchitecturalFaultV1` and the selected instruction `Kind` list (:1311-1332) plus privileged ops (:1333-1341).

**Cases by shape (quoted).**
- `arrival::compose` :954 `(0..32).all(|vector| vectors.binary_search(&vector).is_ok())` - requires all 32 architectural exception vectors; :1055 `for vector in [2, 18]` - only vectors 2 (NMI) and 18 (machine check) are expanded as potentially nested arrivals; error text :956 "fatal installation does not cover every architectural exception destination".
- `faults::derive` memory-kind whitelist :1311-1332 (CopyBytes, ZeroBytes, Load*/Store*, AtomicAccess, ReturnUnit/Scalar/Aggregate ...) and privileged list :1333-1341 (PrivilegedCrash, PortWrite, RegisterAccess{ProcessorHalt | InterruptControl(Disable) | MemoryFence | PortWrite}); any other `MayArchitecturalFault` instruction without relative-branch or call-target excuse -> error (:1345-1353).
- `ram::validate` :1144-1171 only `EstablishAccessReference`, `BoundaryCall`, `ExternalCompletion` are examined; `_ => continue` for all other operation kinds.
- `linux_write_line_custody_is_exact` :3505-3509 only ELF x86_64/aarch64.
- `process_exit::shape_is_exact` :4198-4207: byte_count 12 for linux_x64/macos_x64, 16 for linux_arm64/macos_arm64, else false (so windows_x64 has no accepted process-exit shape here).
- `selected_byte_output_shape_is_exact` :4372-4389: byte_count 40 (linux_x64), 42 (macos_x64), aarch64 computed by `isa_aarch64::aarch64_hosted_write_byte_i32_byte_count`, else false. `decode_selected_byte_output` :4256-4264 same four targets, else `None`. (Cross-reference: function_fragments `byte_output`/`process_exit` replay depend on these: function_fragments.rs:20760-20762 and 22184-22189.)
- `hosted_read_byte_declaration_is_valid` :4031-4039: Sum with exactly two cases `[empty, byte]`, `byte` with exactly one field, field type i32 or bounded i32 containing 0 and 255 (:4048-4063).
- `foreign_result_operand_count` :3016-3046: (Unit,None)->0, (Scalar,Some)->1, (Structural, single Indirect register pointer)->1, (Structural, all Register locations)->len; everything else `None`.
- `scalar_immediate_span` :3234-3363: handles only one `ValueLocation::Register` placement or one `ValueLocation::Stack` placement with `value_byte_offset 0` and full size; else `None`.
- `scalar_result_normalization_kind` :3186-3214: float32/64 bits, bool/u8/u16/u32 zero-extend, i8/i16/i32 sign-extend, 64-bit copy; other integer widths -> `None`.
- `condition_layout::replay_structural_shape` :4503-4572: Reference -> 16 bytes if referent is a scalar with storage codec else 8; ByteSequence/ElementView -> `None` (:4531-4532); Record/FixedArray/Sum/Mixed handled; `replay_structural_projection` :4679-4818 handles Case, Field, FixedIndex; RuntimeIndex/Referent/AllElements/FixedRange/RepresentationView -> `None` (:4681, :4738-4741). Integer alignment `size.next_power_of_two().min(8)` in `replay_structural_shape` (:4523) but `.min(16)` in `replay_structural_field_shape` (:4909, :4917): two different caps.
- `stack_demand` constant `stack_alignment: 16` (:5255) for all targets.

**Same job elsewhere.**
- Layout computation (size/alignment of records, arrays, sums): `condition_layout` here (:4469-4950) re-implements structural layout; suspected same job in `calling-conventions/src/terminal_layout.rs` (`scalar_shape`, `byte_sequence_shape`, `checked_align_up_u32`), `calling-conventions/src/structural_shapes.rs`, `calling-conventions/src/aggregate_layout.rs`, `02_abstract-operations-to-target-operations/src/structural_layout.rs` (`checked_align_up_u32` :1180). Evidence: grepped `fn checked_align_up` -> five separate definitions across omega-rust (`recasts.rs:3950`, `structural_layout.rs:1180`, `layouts.rs:7016`, this file :4473, `terminal_layout.rs:444`); grepped `fn replay_structural_shape|ScalarStorageCodec::from_scalar|fn evaluate_conventional_sum_layout` -> 22 occurrences in 11 files (03_target-operations-to-selected-instructions 4 files, 02 1, calling-conventions 3, image-emission 3). Not read outside this unit -> "suspected".
- Users of `condition_layout` functions outside this file: grepped `replay_structural_value_shape|replay_structural_projection|replay_runtime_projection|replay_structural_sum_layout` excluding object_artifact.rs -> 3 uses, all in function_fragments.rs (:10635, :10640 outside my unit; :13346 in `case_tag_load_matches`). One consumer file, so this replay layout is not shared with the compiler stages that compute the real layout (the grep over omega-rust shows no other consumer).
- `scalar_call_result_producer`/`scalar_observation_producer` vs function_fragments `source::scalar_result::result` (22323): both find a call's scalar result definition and check the register origin is `InstructionResult { instruction, source_value }` (:3452-3457 and function_fragments.rs:22416-22425).
- `foreign_invocation_provenance_matches` (:3468) is the provenance equality used in several places in this file (:1972, :2753, :3298, :3324).

**Unused/one-caller / "shared" claims (greps).** Header (:1-7) lists `installed_stack`, `derive_stack_demand`, "boundary/structural replay checks". Counts: `derive_stack_demand|derive_body_stack_demand` 13 occurrences/5 files (image-emission object_artifact 5, object.rs 1, lib.rs 1, entries.rs 5, native-artifact fragment_object_publication.rs 1). `ObjectError` is `pub(crate)` re-exported (:5478) and lib.rs:108-111 re-exports it again `pub(crate)`. `find_object_function` (:324, re-exported :5476): not grepped. `CompletionCustodyError`: `pub(crate)`; users: object.rs? not grepped.

**Stated limits.** `:1146-1149` "external placed backing needs its own mapped-residence proof" (installed RAM closure rejects `EstablishAccessReference`); `:1161` "nested installation is not a closed Context source"; `:1176` "boundary has no unique concrete RAM-preserving body call"; `:1208` "installed quiet closure has an unresolved foreign or private execution dependency"; `:1301` "dynamic call has no closed fault analysis"; `:931` "arrival stack domain is unresolved".

**Fail-open risks.**
- `completion_receipts::validate_completion_custody` `execution_realization_pair_is_exact` `_ => true` (:3628). Arms: (AdmittedProvider, HostedWriteByteI32) -> true; three (CompilerBuiltin(x), matching realization) -> true; `(CompilerBuiltin(_), _)` -> false; `(_, HostedExitProcessI32|HostedReadByte)` -> false; everything else -> true, i.e. any `AdmittedProvider` execution with any other realization (e.g. `ClaimCompletionOnly`, `MetadataOnlyPort`, `LinuxWriteLine`) is accepted by this pairing check. Status: open (no comment). What gets through: an AdmittedProvider paired with a realization that is not role-specific; later check `derive_completion_provider_custody` (:3633-3641) still must equal the record's custody.
- `ram::validate` `_ => continue` (:1170): operations other than the three listed are not checked. Stated by the enclosing function's purpose (closed RAM source) but no comment at the site: open.
- `descriptor_input::validate` returns `Some(())` when `record.call.descriptor_input` is `None` (:1673-1675): nothing to validate; traced to the single caller (:1969) which gets `Some(())` only for no guard or a verified guard; wrong guard shapes return `None` -> error. Fail-closed for guarded calls; a call that should have a guard but whose record omits it would pass: the omission would have to come from the same record being validated. Open.
- `durable_home_offsets` returns `Vec::new()` on `u32` overflow (:2335-2337, :2340-2343): later `home_offsets.iter().find(...)` misses and errors "scalar home has no allocated durable offset" (:2656, :3165-3171) for homes, but arguments without homes proceed. Traced: only arguments/results needing homes are affected. Low risk.
- `foreign_scalar_argument_custody` for `TargetUnitScalarArgumentSource::Parameter|BlockParameter|BooleanImmediate|IeeeFloatImmediate` (:2594-2609) records the whole call instruction span as the argument span without checking the transport (no `scalar_immediate_span` call). Open: these four source kinds are accepted without an emitted-argument transport check, unlike IntegerImmediate/Home.
- `derive_terminal_stack_peak` memo hit returns `*peak` without re-checking (:5279-5282): sound if `functions` is unchanged between calls (traced: map built once per call at :5204-5208).
- `validate_terminal_entry` :204-219: when `fragment_replay` is `None`, `installed_entry` is `None` and the entry is taken from `module.entry` (Machine) or error for Boundary/None. Traced: not fail-open.
- None other seen.

---------------------------------------------------------------------------------------------------

## 7. function_fragments.rs, lines 12463-24923

General shape of this slice: the file implements "replay" validation - for an `ObjectArtifact` built from a function-fragment emission plan it re-derives, from the staged container (`StagedOptimizedRelocationFreeObjectContainer`), that every abstract operation of every function is accounted for by target operations plus selected instructions plus emitted bytes. The central entry is `source::admit` (18534); it is called from `validation::validate` (24432) (grepped `source::admit|admit\(source\)|fn admit` -> 1 caller; there is also a second `fn admit` at :2855 in `mixed_scalar_abi`, outside unit). The container is reached through chains of `.source().source().source().source()` (e.g. 18546, 23449-23454, 24526) to get `selected_plan()`, `optimized_target()`, `frame_layout()`, `fragments()`.

### 7a. `source::placed_reference` tail (12463-13454)
**Purpose.** Replay that a pointer-valued source address in selected instructions equals the exact projected place the abstract operation named, and that an `EstablishAccessReference` (a reference stored into an 8-byte slot) is placed correctly.
**Shape.** `DefinitionOccurrence` (:12475 Missing/Unique/Repeated); `SelectedAddressInputs` (:12483, built in `new` :12492, sorted vectors `operation_instructions`, `locations`); `address_output` (:12607); `source_value` (:12621, pub(super)); `resolved_source_value` (:12658); `definition` (:12700); `scaled_index` (:12735); `operation_rows` (:12769); `AddressInputs` (:12778); `AddressReplay` (:12809, lazily builds `inputs`, `spills`); `AddressReplay::address` (:12872, recursion bounded by `remaining = virtual_registers.len()+1` :12863); `retained` (:13022) for `EstablishAccessReference`; `address_root` (:13145); `block_entry_pointer` (:13175); `geometry_address` (:13267); `case_tag_load_matches` (:13314); `borrowed_address_matches` (:13361); `pointer_definitions` (:13408, debug string, chain depth 8).
**Inputs/outputs.** Reads `TargetFunction`, `SelectedFunction` (virtual registers, local storage slots, memory accesses, instruction rows), `structural_homes::HomeInputs`, `spill_values::SpillValues`, `backing_geometry::BackingGeometry`. Produces `Option<Address>` (`Address { base: AddressBase, loads: Vec<(offset, indices)>, offset, indices }`, :12377-12388) or a boolean/`Result<u32,String>`; `AddressReplay.failure` carries a `&'static str` reason.
**How it decides.** Keyed on register origins (`VirtualRegisterOrigin::{StructuralParameter, AbiTransport, StructuralObservation, InstructionScratch, SpillAddress, InstructionResult, EntryParameter, BlockParameter}`), instruction kind (`FrameAddress`, `AddressOffset`, `CopyI64`, `Load64`, `ByteViewAddress`), provenance operations containing the operation id (:12712), slot identity (`LocalStorageSlotId::{Spill, Structural, StructuralBlockParameter}`), and position order (`precedes`). The reconstructed `Address` must equal the expected address by `==` (:13120, :13357-13358, :13394).
**Cases by shape.** `address_output` (:12607-12617): FrameAddress/MaterializeI64 -> operand 0; AddressOffset/Load64/CopyI64 -> operand 1; ByteViewAddress/WrappingMultiplyI64 -> operand 2; `_ => return None`. `address` (:12976-13017) supports only FrameAddress(Local slot), AddressOffset, CopyI64, Load64, ByteViewAddress; `_ => None`. `retained`: destination slot must be size 8 / alignment 8 (:13050), store is `Store{byte_offset:0, byte_size:8}` (:13086-13092). `case_tag_load_matches`: `layout.tag_byte_offset != 0 || tag_shape != integer(4,4)` -> reject (:13350-13353) i.e. only 4-byte tag at offset 0.
**Same job elsewhere.** Three "precedes" helpers in this file: `placed_reference::precedes` (:12411) + `precedes_blocks` (:12442), `SelectedAddressInputs::precedes` (:12568) (indexed copy using `locations` table, calls `precedes_blocks`), and `source::...::precedes` at :5185 (outside unit, suspected same job). `operation_rows` is defined twice: :12769 (read) and :9782 (grepped `fn precedes\(|fn precedes_blocks|fn operation_rows`, not read). `source_value` (:12621) computes definitions by a full-function scan `all_instructions(selected).filter(...)` per call while `SelectedAddressInputs::new` (:12492-12552) builds the same definitions table once: two implementations of "definition occurrence of a register" (:12635-12648 vs :12547-12550).
**Unused/one-caller.** `structural_homes::resolve` (free fn, :18264): grepped `structural_homes::resolve\(` -> 2 callers, both outside my unit (:12108, :12260), while `HomeInputs::resolve` (:18246) is used in my unit (:13037, :13330). Both end in `resolve_candidates` (:18321). `pointer_definitions` (:13408) used only to format an error string (:13402).
**Stated limits.** Comment :12481 "Address checks distinguish ordinary output candidates from all defining rows. Retain both input associations while checking each use afresh." Comment :12541 "The old predicate counted an instruction once even when more than one of its operands defines this register." :13311 "A large tag displacement may move from the load into an earlier pointer adjustment." Failure reasons e.g. :12950 "pointer reload has no exact private spill address".
**Fail-open risks.** None seen: every `None` is propagated to `Option`/`Result` and the callers I read treat `None` as not-retained (e.g. :13118-13119 `captured?`). One quirk: `address()` on a `StructuralParameter` root requires `!= Owned` and definition `Missing` (:12894-12900) else `None`.

### 7b. `primitive_accesses` (13456-13550)
**Purpose.** For `StorageAddress`, `PrimitiveScalarRead`, `WriteOnlyPrimitiveStore`, check that the target graph holds exactly one corresponding operation with the same result/source/path and runtime selectors.
**Shape.** single fn `retained(source: &AbstractOperation, target: &TargetFunction) -> bool` (:13460).
**How it decides.** Keyed on `psi_operation` identity (`retained == psi_operation`), then field equality; "exactly one" via `queries.next()` then `queries.next().is_none()` (:13484-13487). Store compares `stored.source_value() == value.value && stored.scalar_type() == value.scalar_type` (:13543).
**Cases by shape.** `match` has 3 arms plus `_ => false` (:13547).
**Same job elsewhere.** Same "find the unique target op by `psi_operation`, compare fields" pattern appears for `byte_concatenation_retained` (:19038), `ieee_literal_retained` (:19119), `structural_fields::*_retained`, `control_flow::retained` (outside), etc.: at least 12 near-identical `filter ... next() ... next().is_none()` sites in this slice (grepped `fn \w+_retained|fn retained` in function_fragments.rs -> 32 definitions, whole file).
**Fail-open.** None seen.

### 7c. `spill_values` (13552-13918)
**Purpose.** Prove a reload from a compiler spill slot reads a value written by exactly one reaching store, so address reconstruction can see through spills.
**Shape.** `flow::Writer` (:13562) with `merge` (:13569), `flow::entries` (:13579, worklist dataflow); tests :13606-13666 (six unit tests, read). `Slot { valid, stores, entries }` (:13670), `SpillValues::new` (:13682), `private_store` (:13833), `private_address` (:13846), `writer` (:13859), `invalidate` (:13911).
**How it decides.** A slot is valid only if byte_size == 8 and alignment >= 8 (:13697), the store's operand roles are `Use` for operand 0 and `Def` otherwise (:13770-13786), the frame-address register is defined exactly once and only used as a load operand (`definition_counts`, `non_load_bits`, :13812-13821); floating-control save/restore invalidate (:13728-13732). `writer` returns the reaching store from per-block `stores` partition at position or `entries[block]`, requires `Writer::Store` (not Unreachable/Missing/Different, :13893-13903).
**Cases by shape.** Only `Kind::Load64{byte_offset:0}` reloads (:13872) and `Kind::Store64{slot: Spill, byte_offset:0}` stores (:13835-13837).
**Unused/one-caller.** `private_store` (:13833) and `private_address` (:13846) are `pub(super)`; grepped within image-emission: only `spills.as_mut()?.writer(...)` is called from `address` (:12962); I did not find callers of `private_store`/`private_address` in the lines I read (not grepped).
**Fail-open.** None seen (unknown/duplicate -> `Err`).

### 7d. `stack_calls` (13920-14022)
**Purpose.** List call edges for stack composition: internal resolved calls plus every possible target of a dynamic call.
**Shape.** `CallStackEdge { owner, callee, section_offset }` (:13927), `stack_calls` (:13943).
**How it decides.** Dynamic call edges come from every dispatch table of the called interface: for each table, row at `slot`, `Some(target)` must have an emitted function (:13995-14011); targets sorted/deduped and attached at the call instruction's `span.section_offset` (:14012-14018).
**Cases.** Dynamic targets only; `Machine` targets come from `resolved_internal_machine_calls` (:13948-13953).
**Same job elsewhere.** `call_target_matches` (:19501) vs `NativeCallTarget` comparisons here.
**Fail-open.** A table row `None` (slot without implementation) is skipped (:14003) - this means traps/empty slots add no edge; stated? no comment; the table rows model "trap" slots (see image.rs:575-585 where `None` row must equal 0 address). Traced: consistent with image replay that rows None == trap.

### 7e. `structural_case` (14024-14462)
**Purpose.** Check a `StructuralCase` abstract terminator against the target graph's `StructuralCase` terminator and the selected conditional-branch/jump successors (case tag, payload offsets, bindings).
**Shape.** `retained` (:14032), `projected_source_matches` (:14142), `source_slot_matches` (:14197), tests (:14268-14461, read).
**How it decides.** Case identity by `psi_edge` equality; `target_case.case == case.case`, `target == target`, bindings equal (:14075-14081); the selected successor with `role == Semantic && psi_edge == Some(...)` must be unique (:14095-14105); payload `field`, `field_byte_offset`, `parameter.value`, `scalar_type` equal (:14130-14138). `source_slot_matches`: for `Borrowed` dispatch requires exactly one declaring owner among block entries or function parameters and access Owned or reading-borrow (:14211-14243); for `Local` slots by slot variant (:14246-14265).
**Cases.** `projected_source_matches` :14150: if path contains `Referent` then dispatch must be `Borrowed` and `is_static_structural_path(path)`; else `source_slot_matches` directly. Only `ConditionalBranch` and `Jump` terminators are searched (:14084-14091).
**Fail-open.** `cases.is_empty()` -> `false` (:14041-14043, :14067). None seen.

### 7f. `structural_fields` (14464-18172; tests 14472-16796)
**Purpose.** Replay structural scalar/byte-sequence field reads, stores, replacements, and their memory-access footprints (`SelectedMemoryAccess` rows) against target operations.
**Shape.** Tests (14473-16796). Production: `retained_selectors` (:16798), `retained_field_root` (:16810), `indexed_store_retained` (:16872), `runtime_footprint_retained` (:16937), `indexed_store_footprint_retained` (:17018), `indexed_read_retained` (:17056), `field_root_type` (:17107), `loads_backing_pointer` (:17123), `indexed_read_footprint_retained` (:17143), `projected_field_rows` (:17286), `projected_selector_rows` (:17299), `view_replacement_retained` (:17353), `view_replacement_footprints_retained` (:17396), `replacement_retained` (:17457), `replacement_footprints_retained` (:17544), `retained` (:17676), `AccessPurpose` (:17786), `write_destination` (:17792), `scalar_store_retained` (:17948), `pointer_read_before` (:17983), `owned_observation_storage` (:18002), `parameter_access` (:18011), `read_access` (:18038).
**Inputs/outputs.** Abstract op + `TargetFunction` + `SelectedMemoryAccess` slices -> bool.
**How it decides.** `read_access` (:18038-18171) derives the access mode (`StructuralAccess`) and type of a place by scanning function parameters, block-entry parameters and operation results; any place with more than one declaration returns `None` (`access.replace(..).is_some()` :18064-18067, :18085-18088, :18123-18126, :18163-18166). Multiplicity/qualification/claim emptiness decide custody (:18114-18121, :18137-18157). Footprints are matched by `SelectedMemoryAccess { instruction, origin, place, byte_offset, byte_count, role }` where role is `ReadPlace | WritePlace | ReadIndexedPrimitive | WriteIndexedStructural | WriteByteSpan | ReadByteSpan | ReadByteSequence | WriteByteSequence | ReadReturnedView` and the byte offset/length are exact (e.g. :17653-17673 requires read offset 8 if the source is a bounded-owned byte sequence else 0, metadata write 8 bytes, `metadata_write.byte_offset + 8 == write.byte_offset`).
**Cases by shape (quoted).**
- `indexed_store_retained` :16930-16933: index scalar must be `u64` and value `u8` (`IntegerType::new(Unsigned, 64)` / `(Unsigned, 8)`); read index u64 (:17102-17103).
- `write_destination` (:17792-17946): non-`Referent` path accepts `Owned` only for `AccessPurpose::ByteLength`, else MutableBorrow/WriteOnlyBorrow; a `Referent` path requires a single `CallStructural` producer with `AbstractCallTarget::Machine`, `Affine` multiplicity, a `MutableBorrow` reference type to a record, an `Aggregate(integer(8,8))` result home and exactly one leaf `reference_results` row with sorted origins (:17821-17939).
- `replacement_footprints_retained` pattern `byte_sequence_store_pair_pattern!` (:17554) - only those two store ops.
- `indexed_read_footprint_retained` handles: view-root special case (:17191-17214), pointer slots then descriptor offset, final `ReadReturnedView` tail or one `ReadByteSequence` at payload offset (:17252-17283).
**Same job elsewhere.** `retained_selectors` (:16798) and many `...retained` fns do "`filter(psi_operation == ...)`, `next()`, `next().is_none()`" (see 7b). `read_access`' declaration scan duplicates `structural_homes::HomeInputs::new` roots (:18195-18231): both collect parameter / block-parameter / operation-result homes of a place (suspected overlap; read both).
**Unused/one-caller.** `projected_field_rows` (:17286) only called from `view_replacement_footprints_retained` (:17448-17449); `owned_observation_storage` called from `parameter_access` and `read_access`.
**Stated limits.** Comment :17189 "Selection may already hold a root view's backing in a register. Its optional descriptor load is separate from nested crossings." :17215 "Use the selector's exact projection regions: a descriptor field can require reference loads before its own backing-pointer load."
**Fail-open risks.**
- `runtime_footprint_retained` returns `true` when the operation has no runtime indices (:16982-16984) - for `PrimitiveScalarRead`, `StructuralByteSequenceFieldLength`, `WriteOnlyPrimitiveStore`, `StructuralScalarFieldStore`. Meaning: for non-indexed accesses the footprint rows are not checked here. Open (no comment at the site); what could get through: a non-indexed byte-length read/primitive access whose selected memory-access rows are wrong/absent, if no other replay checks them (I did not trace other checkers).
- `retained` (:17676-17783): `read_access` scans `target.graph.parameters.get(position)?` etc.; `None` -> `false`. Not fail-open.
- None other seen.

### 7g. `structural_homes` (18174-18402)
**Purpose.** Resolve the storage home of a structural place: allocation slot, borrowed formal parameter, or pointer-slot block parameter.
**Shape.** `SourceHome` (:18179), `HomeInputs` (:18187; sorted `roots`, `allocations`), `HomeInputs::new` (:18194), `HomeInputs::resolve` (:18246), free `resolve` (:18264), `resolve_candidates` (:18321), `thin_address_join` (:18365).
**How it decides.** Exactly one root declaration and at most one allocated slot ("source place has multiple declarations or result homes" :18331, "multiple allocated homes" :18335); the allocation's slot id must equal the declaring producer's slot (`Some(allocation.id) != producer` :18338); with no allocation, the place must be a non-Owned parameter (:18357).
**Cases.** `thin_address_join` (:18365-18401): non-`StructuralBlockParameter` slots -> `Some(false)`; entry block or not an address join -> `Some(false)`; scalar-descriptor primitive scalar types with a storage codec -> `Some(false)` (i.e. not an indirect pointer slot); else `Some(true)`.
**Same job elsewhere.** `resolve` (free) vs `HomeInputs::resolve`: same decision through `resolve_candidates`, one per call-scan vs one indexed (callers 12108, 12260 outside unit use the free one).
**Fail-open.** None seen. `thin_address_join` returns `Some(false)` for entry-block parameters, i.e. "not indirect" - traced: callers treat `Some(false)` as direct storage; for an entry block parameter this is expected (parameters are function parameters, not block-join slots).

### 7h. `source` top-level functions (18404-19778) incl. `admit`
**Purpose.** Gather helpers and the central admission replay.
**Shape.** `requires_graph_storage_replay` (:18405), `fragments` (:18480), `function` (:18486, unique abstract+target function for a machine), `frame` (:18520), `admit` (:18534-19036), `byte_concatenation_retained` (:19038), `ieee_float_builtin_retained` (:19060), `program_image_retained` (:19102), `ieee_literal_retained` (:19119), `fixed_byte_length_is_static` (:19142), `byte_operation_retained` (:19179), `element_operation_retained` (:19270), `parameter_abi` (:19359), `fragment_metadata` (:19387), `temporary_stack_bytes_by_function` (:19409), `temporary_stack_bytes_for_selected_functions` (:19437), `temporary_stack_peak` (:19486), `call_target_matches` (:19501), `empty_run_release_retained` (:19528), `access_lifecycle_retained` (:19577), `placed_scalar_retained` (:19618), tests (:19719-20183; read to :19778, rest unread).
**`requires_graph_storage_replay` (:18405-18478).** Returns true if any operation has an `InvokeNominal` cleanup action, a unit/scalar `CallStructural` with an Owned structural argument, or is one of ~45 operation kinds listed in `matches!` (SelectImplementation, InvokeNoNormal, ExternalCompletion, dynamic CallStructural, AtomicEvent, RegisterAccess, descriptor ops, Establish*, ReturnStructural, PrimitiveLocalStore, StorageAddress, PrimitiveScalarRead, WriteOnlyPrimitiveStore, StructuralScalarField*, byte-length/field-read, byte-sequence store pair, StructuralCaseMembership, StructuralLeafCopy, MoveStructuralValue, TransformContent ...). Rest -> false. This is a "fixed list of program shapes": the artifact flag `requires_graph_storage_replay` (object_artifact.rs:27) is set from it (function_fragments.rs:3617-3618) and re-checked at :24470-24479 (`artifact.requires_graph_storage_replay != ...` -> mismatch).
**`admit` (18534).** Steps per `FunctionFragmentEmissionPlan.functions` entry: refuses empty roster or non-64-bit pointer (:18538-18544, "shared image publication requires a nonempty 64-bit function roster"); `privileged_termination::validate`, `backing_geometry::validate_activation_residence`, builds optional `BackingGeometry` only if some operation is `EstablishAccessReference` (:18558-18580); per function: `static_interrupts::PreparedHelperReplay`, `installed_calls::validate`, `installed_interrupts::validate`, `AddressReplay`, `DescriptorReplay`; checks function declaration metadata equals canonical source (:18620-18629); ABI consistency (:18630-18653); parameter/result ABI equality (:18657-18697); then a `match operation` over every `AbstractOperation` (:18699-19025) yielding `admitted: bool`; `!admitted` -> `Error::UnaccountedOperation { machine, operation_ordinal, operation_description }` (:19026-19032).
**How `admit` decides.** Per operation kind the justification is chosen by a match arm, e.g. unique selected instruction with provenance equal to `[psi_operation]` (RegisterAccess :18713-18727, SelectImplementation :18831-18833 `.count()==1`), exactly one matching selected call row (:18854-18862, :18920-18927 `.count()==1`), byte/element ops by one target node with the same `psi_operation` (:19189-19267, :19280-19355 `count()==1`), structural fields via `structural_fields::*` + footprint checks (:18747-18790).
**Cases by shape (verbatim anchors).** The arm list :18699-19024; the final arm `_ => false` (:19024). Operations explicitly **admitted with no check** (`true`):
- `AbstractOperation::AtomicEvent { event: Load | Store | ReadModifyWrite | Swap | CompareExchange | CompareExchangeOnce | Fence }` => `true` (:18735-18746).
- `IntegerConstant | BooleanConstant` => `true` (:18864-18865).
- `BooleanEqual | IeeeFloatCompare | integer_equal_less_pair | IntegerLessOrEqual | BooleanNot | integer_widen_or_exact_cast | bitwise_and_or_view_pair | bitwise_or_xor_pair | IntegerBitwiseNot | IntegerArithmetic | IntegerShift | TrappingInteger | FloatResultPolicy` => `true` (:18995-19007).
- `Return { cleanup_actions }` => `cleanup_actions.is_empty() || control_flow::retained(...)` (:18986-18989); `ReturnUnit` adds `|| read_result_cleanup_actions_match` (:18990-18994).
- `BoundaryCall` (:18938-18947) is a disjunction of 10 alternatives (`selected.boundary_settlements`, `installed_scopes`, `program_image_retained`, `ieee_float_builtin_retained`, `access_lifecycle_retained`, `empty_run_release_retained`, `static_interrupts::retained(...)?`, `placed_scalar_retained`, `selected.calls ... InstalledProvider|TargetFamily|ProgramEntry`, `normalized_foreign_calls`): any one true admits; none checks the arguments except the individual helpers.
- `PortWrite` uses `.any(...)` rather than a unique-count (:18728-18733).
- `ByteSequenceLength` admitted by `byte_operation_retained || fixed_byte_length_is_static` (:18885-18897).
**Fail-open risks (admit).**
- Arms returning `true` unconditionally: AtomicEvent (:18745), constants (:18864-18865), the scalar/arithmetic family (:18995-19007). No comment at the sites. Status: open. What gets through: if the selected/machine layers lowered such an operation incorrectly (wrong instruction, dropped store), `admit` would not notice; attribution replay (`attribution::validate(...)`, called at function_fragments.rs:24627) is the other line of defense (I did not read it - it is in lines 8-1472). Atomic events and arithmetic are exactly the operations where "selected instruction exists and has the right semantics" would be checked per operation; the replay does not.
- `Return` and `ReturnUnit` with empty `cleanup_actions` => true without examining selected return instructions (:18988, :18992). Open.
- `PortWrite` `.any` (:18729): accepts if any selected instruction of that kind with provenance `[psi_operation]` exists, not that it is unique or in position. Open.
- `fixed_byte_length_is_static` (:19142-19177) returns false when the root type cannot be resolved; accepts length ops whose type has a fixed byte extent without checking any target op - intended (static length) - traced by reading the function.
**Same job elsewhere.** `temporary_stack_bytes_*` (:19409-19499): "temporary_stack_peak" (max of `TemporaryBytesV1`) vs `object_artifact::stack_demand` and `validation::stack::validate` (24111): three places compute stack numbers from different sources (encoded effects here, `unit_stack/scalar_stack` in object_artifact.rs:5295-5303, frame+call in `stack::validate`). `parameter_abi` (:19359) used at :18657, :18660, :24543, :18851, :18919 etc.
**Unused/one-caller.** `temporary_stack_bytes_for_function` (:19421) is `#[cfg(test)]`. `fragment_metadata` (:19387): 1 caller seen at :24533. `call_target_matches` (:19501): grepped -> callers :10270, :17892, :18937, :22358 (4 sites).
**Stated limits.** Messages: :18542 "shared image publication requires a nonempty 64-bit function roster"; :18650 "shared function has unsupported ABI or boundary effects"; :18514 "shared function has no unique current source".

### 7i. `structural::byte_input` (20188-20709; tests 20481-20709 not read except 20481-20489)
**Purpose.** Build and validate the `ObjectBoundarySettlement` record for a hosted byte read (`HostedReadByte`) result: the selected instruction, frame slot, decoded home offset in the emitted bytes.
**Shape.** `decoded_home` (:20205), `cleanup_actions_match` (:20219, re-exported as `read_result_cleanup_actions_match` :20965), `settlement` (:20279), `validate` (:20371), `result_declaration` (:20464).
**How it decides.** Decodes the home offset from the instruction bytes via `isa_x86_64::decode_x86_64_selected_hosted_read_byte` / `isa_aarch64::...` (:20209-20216) and requires frame slot offset == decoded offset, sizes equal layout shape, red-zone residency 0 (:20318-20328, :20433-20440); every non-byte-read argument list in the record must be empty (:20451-20457). `cleanup_actions_match` accepts only `DiscardRoot` actions of Affine unqualified read-byte results, exactly once per place (:20249-20271).
**Cases.** Target must pass `HostedReadByteRealization::supports_target` (:20206); function result must be `Unit` (:20227-20232).
**Fail-open.** None seen.

### 7j. `structural::byte_output` (20711-20963)
**Purpose.** Same for `HostedWriteByteI32`.
**Shape.** `settlement` (:20726), `validate` (:20824), `target_settlement` (:20907).
**How it decides.** Slot `LocalStorageSlotId::Boundary { operation }` size 1 / align 1 (:20770-20771, :20891-20892); bytes decoded by `decode_selected_byte_output` (object_artifact.rs:4252) and verified by `selected_byte_output_bytes_are_exact` (object_artifact.rs:4267). `target_settlement` accepts (AdmittedProvider, HostedWriteByteI32) or (CompilerBuiltin(HostedWriteByteI32), HostedWriteByteI32) (:20937-20948) and requires a single such row (`found.is_some()` -> error :20936).
**Cases by shape.** Scalar type exactly `i32` (:20777-20779, :20888); targets per object_artifact.rs:4256 (four targets).
**Fail-open.** None seen. Note this module's `validate` is the fall-through for any settlement payload that is not ClaimCompletion/HostedReadByte/HostedExitProcessI32: `validate_settlements` `_ => byte_output::validate(...)` (:22973) and `settlements` `_ => byte_output::settlement` (:24036); `byte_output::validate` fails with "selected byte output publication custody" for other payloads because of the `let ... else` on `HostedWriteByteI32` (:20831-20838). Fail-closed.

### 7k. `structural::established_views` (20966-21263)
**Purpose.** Prove that a structural call argument that is an established byte/element view (or a block parameter view) lives in the exact local 16-byte (or 8-byte for bounded views) descriptor slot and compute its `StructuralSourceLocation::Stack`.
**Shape.** `location` (:20979), `block_location` (:21056), `local_location` (:21114).
**How it decides.** Producer must be the unique `ByteSequenceSubslice | ElementViewSubslice | EstablishElementView | EstablishByteSequenceLiteral` with `psi_operation == producer` whose result place/type match the argument (:20993-21040); descriptor size 16 for BorrowedView/ElementView, 8 for BoundedView, else error (:21130-21135); the `FrameAddress` instruction for the slot must be unique, with an `AbiTransport` register origin (:21199-21257).
**Cases.** `local_location` branches on `argument.path.is_empty()`: element loan (non-empty path of Field|FixedIndex|RuntimeIndex segments, not Owned, no fixed_array_length / element_stride, :21136-21152) vs whole view (reading borrow, `ValueShape::borrowed_reference(byte_size, 8)`, offset 0, :21153-21169).
**Fail-open.** None seen. `SelectedMemoryAccessOrigin::Edge(_) => true` at :21236 is inside a "reject" predicate (`|| match origin {... Edge(_) => true}` yields error), i.e. rejects.

### 7l. `structural::primitive_locals` (21265-22121; tests 21660-22121 not read)
**Purpose.** Replay `EstablishPrimitiveLocal` / `PrimitiveLocalStore`: the local primitive's frame slot, its single address+store pair, and the frame offset used for borrowed pointers to it.
**Shape.** `access::operation_retained` (:21274; re-exported :21656 and as `primitive_operation_retained` at :22122), `establishment::{producer, primitive_size, establishment}` (:21361, :21388, :21404), `frame_location::{location, frame_location, argument_matches_source}` (:21535, :21591, :21618).
**How it decides.** `primitive_size` accepts integers of 8/16/32/64 bits, Boolean (1), Binary32 (4), Binary64 (8), else `None` (:21388-21401). `establishment` requires Unrestricted multiplicity, no projected qualifications/claims/establishments, exactly one declaration and one primitive-scalar type declaration, slot size/alignment == `size`, two memory accesses (AddressLocal then WritePlace), same block, address before store, `Store{byte_offset:0, byte_size:size}`, store source register scalar_type == value type with origin Entry/Block/Instruction result of `value.value` (:21412-21517). `argument_matches_source` (:21618-21653) accepts a borrowed-scalar view of the local with `scalar_reference_view_is_total`.
**Cases (shape lists).** `frame_location::location` requires access Shared/Mutable/WriteOnly borrow and `ValueShape::borrowed_reference(size, size)` (:21559-21569).
**Fail-open.** None seen; `operation_retained` returns false for non-matching op kinds (`_ => return false`, :21307).

### 7m. `structural::process_exit` (22123-22315)
**Purpose.** Settlement for `HostedExitProcessI32` terminator (build and validate).
**Shape.** `settlement` (:22138), `validate` (:22234).
**How it decides.** `block.terminator` must be `SelectedTerminator::HostedExitProcess { instruction, completion_edge }` at `instruction_index == block.instructions.len()` (:22165); edge equals completion edge; instruction kind `HostedExitProcessI32`; bytes decoded by `runtime_scalar_custody::process_exit::decode` (object_artifact.rs:4165) and checked by `bytes_are_exact` (:4222).
**Cases.** i32 only (:22191-22193); targets: see object_artifact.rs:4198-4207 (no windows).
**Fail-open.** None seen.

### 7n. `structural::scalar_result` (22317-22490)
**Purpose.** Find the expected scalar result of a published call (internal or installed-boundary) from the abstract function.
**Shape.** `result` (:22323), `definition_is_exact` (:22416), test (:22427-22489 read).
**How it decides.** `contract.call.target.machine()` None or no `result_placement` => `Ok(None)` (:22328-22333). Else finds exactly one `CallStructural{Scalar}` (when `NativeCallOrigin::Authored`) or `BoundaryCall{Scalar}` (when installed/program-entry/target-family, key = boundary id) with matching owner (:22345-22375), requires callee has `mixed_structural_scalar_abi` and equal scalar type / placement / call_plan (:22377-22391), and the selected call instruction kind `CallScalar{Machine(callee)}` whose last operand's register is defined by that instruction with the right source value (:22392-22412).
**Fail-open.** `Ok(None)` at :22328-22333 means "no scalar result expected". Traced: the only caller comparison `actual.semantic_result != scalar_result::result(...)?` (:22607, and `populate` :23974) means a record that carries a result when `None` is expected is rejected; a call with machine callee and a result placement but no `mixed_structural_scalar_abi` errors (:22377-22380). Not fail-open.

### 7o. `structural::validation` (22492-23424; tests 23196-23424 not read)
**Purpose.** Validate an `ObjectFunction`'s structural parameter records, parameter homes, internal call records (`internal_unit_calls`), and boundary settlements against the selected plan and emitted bytes.
**Shape.** `validate_function` (:22504), `validate_incoming_location` (:22850), `validate_settlements` (:22952), `validate_settlement_attributions` (:23037), `validate_settlement_position` (:23106), `validate_copy` (:23135).
**How it decides.** Rosters: counts of records equal counts of selected contract parameters (`records.len() != parameters.len()`, :22546), `internal_unit_calls.len()` equals count of `published_call` rows (:22548-22553); pairwise field equality for parameter records (:22559-22564), home records (:22570-22577); per call: owner, target, semantic result (via `scalar_result::result`), argument counts, `operation_ordinal` equals the attributed span's ordinal, `code_offset == host(call_span.offset)`, `byte_count == call_span.bytes.len()`, internal-machine fixup callee equals target (:22604-22631); call source variants compared by value (:22634-22697); argument source variants matched to `established_views::location` / `primitive_locals::location` / `block_location` results (:22757-22830); owned arguments get `validate_copy` (:23135) which checks the span bytes against fragment bytes (:23188). Settlement validate: ClaimCompletion records compared field by field (:22979-23009) plus `validate_completion_custody` (:23027).
**Cases by shape.** `validate_incoming_location` (:22850-22950): Owned -> one `ValueLocation::Indirect` with a copy offset, integer class; borrowed -> Register / Stack (8 bytes, align 8) / Indirect (copy_stack_byte_offset None, with `indirect_pointer_supports_copy`); anything else `invalid()`. `validate_settlements` match at :22970-22974: HostedReadByte -> byte_input, HostedExitProcessI32 -> process_exit, `_ => byte_output::validate` (see 7j).
**Same job elsewhere.** `validate_incoming_location` (:22850-22950) vs `incoming_location` (:23607-23682): same placement-shape decision (owned indirect+copy; borrowed register/stack/indirect) written once to derive (`incoming_location` used by `populate` and `source_location`) and once to check (`validate_incoming_location` compares to given location). Two parallel copies of the case list. Also `settlement` fns in byte_input/byte_output/process_exit vs their `validate` fns are a build/replay pair that each re-derive the same record (`settlement` :20279 vs `validate` :20371; :20726 vs :20824; :22138 vs :22234).
**Fail-open.** None seen; mismatches -> `Err(invalid())`.

### 7p. `structural` helpers (23425-24099)
**Purpose.** Shared helpers and the producer-side `populate`/`settlements`.
**Shape.** `selected` (:23445), `fragment` (:23458), `scalar_type` (:23466), `published_call` (:23485), `requires_call_graph_replay` (:23563), `inline_owned_placement` (:23575), `source_location` (:23590), `incoming_location` (:23607), `copy_extent` (:23684), `settlement_instruction` (:23719), `settlement_offset` (:23734), `placed_text_offset` (:23749), `populate` (:23758, fills `unit_parameters`, `unit_parameter_homes`, `internal_unit_calls`), `settlement_attributions` (:23994), `settlements` (:24021), `indirect_pointer_supports_copy` (:24081), `call_site_owner_semantic_site` (:24092).
**How it decides.** `published_call` (:23485-23561) decides which selected calls are recorded as `internal_unit_calls` (and which instead require "call graph replay"): machine target, no BorrowedScalarReference structural argument, owner is `CallSiteOwner::Operation`, no structural result, no argument that is inline-owned or an owned `Placement` of an inline-owned parameter or a `StructuralHome` or an address-join/non-view `BlockParameter`; and (result_placement is none or some structural argument exists) (:23555-23560). Anything not published goes to graph replay: `requires_call_graph_replay` (:23563-23573) = any selected call not published.
**Cases by shape.** `inline_owned_placement` (:23575-23589): Owned and (zero-size integer(0,1) with no locations, or all locations are the `register_or_stack_pair_pattern!`). `indirect_pointer_supports_copy` (:24081-24089): Register -> true; Stack with alignment 8 and offset multiple of 8; `_ => false`. `incoming_location`: borrowed Register size 8 / Stack size 8 align 8 / Indirect; error "borrowed publication requires exact pointer placement" (:23676). `call_site_owner_semantic_site` returns `None` for `ExternalCompletion` and `Installation`.
**Same job elsewhere.** `scalar_type` (:23466) finds a scalar type by scanning virtual registers (three origin kinds); `foreign_scalar_argument_custody` in object_artifact.rs:2578 uses `source.scalar_type()` directly instead. `settlement_instruction` (:23719) fetches instruction or terminator by index; also done inline at :22062-22075 (`validate_settlement_attributions`).
**Unused/one-caller.** `source_location` (:23590): callers `populate` (:23864) only (read). `copy_extent` (:23684) callers `populate` (:23895); `validate_copy` has its own extent logic (:23135-23193): two implementations of "extent of owned copy" - `copy_extent` returns `(start, end - start)` of the memory-access spans; `validate_copy` recomputes the same from `fragment.instructions()` and compares.
**Stated limits.** `Error::Unsupported("Unit call scalar publication")` (:23846); `"legacy call mirror requires a direct machine target"` (:23979) - word "legacy" on `internal_unit_calls` records; also `"shared fragments cannot carry legacy singular cleanup"` (:22510) and `lib.rs:6-8` "without reconstructing legacy machine-code carriers".
**Fail-open risks.**
- `published_call` `_ => false` inside the `any` over argument sources (:23552): an argument source variant not listed (e.g. `EstablishedPrimitiveLocal`, `EstablishedByteView`, `EstablishedElementView`) is treated as "does not force graph replay", so those calls ARE published. Traced: `populate` handles exactly those variants (:23854-23881) and `validate_function` verifies them (:22759-22810). OK.
- `call_site_owner_semantic_site` `None` for ExternalCompletion/Installation (:24096): `.map(...).unwrap_or(false)` at :22596-22599 and :23811-23813 -> row not found -> error ("Unit call has no attributed span"). Fail-closed.

### 7q. `validation::stack` and `validation` (24102-24700)
**Purpose.** `stack::validate` (:24111) checks recorded call-stack rows of a function against frame size, temporary-stack bytes, alignment and call edges; `validate_function_fragment_object_artifact` (:24402) and `validate` (:24426) are the top-level object-artifact replay.
**Shape.** `stack::validate(unit, (frame_bytes, alignment, contains_call), architecture, temporary_bytes, calls, unit_stack, scalar_stack, unit_call_stacks, scalar_call_stacks)`; tests (:24196-24392, read). `validate_function_fragment_object_artifact` (:24402): physical entry binding validate (:24406); if composition: root custody, `validate_composition`, hosted receiver (:24407-24421); else `validate` (:24423). `validate` (:24426): container validation `object_file::validate_optimized_relocation_free_object_container` (:24430), `source::admit` (:24432), pools (`byte_literals`, `implementation_tables`, `native_code_layouts`, `image_residence`, :24444-24447), private-address checks (:24448-24457), counts of text bytes / symbols / sections (:24479-24504), per-function loop (:24523-24630: symbol names `omega_terminal_machine_{n}` or entry symbol name (:24538-24542), attribution bounds, `stack::validate`, settlement attributions, `structural::validate_function`), private function symbol loop (:24636-24667), settlements (:24668), imports (:24669), hosted receiver (:24670-24674). `empty_unsupported_records` (:24678) requires 14 evidence fields be absent/empty on every `ObjectFunction`.
**How it decides.** `stack::validate`: return bytes = 8 on x86-64, 0 on AArch64 (:24130-24133); `caller_live_bytes == frame_bytes + return_bytes` and multiple of alignment (:24160-24164); peak = max(frame+temporary, caller_live..) and must equal the recorded `local_peak_bytes` (:24178-24192). Function symbols compared by name string `omega_terminal_machine_{machine}` (:24541) - a spelled-name comparison (keyed on string format of `placed.machine.get()`), entry symbol name from `entry_symbol_name(target)` (:24539).
**Cases by shape.** `stack::validate` only two architectures; `unit` vs scalar rosters are exclusive (:24123-24129). `empty_unsupported_records` enumerates the fields that must be empty (structural_call_scalar_return, x86_floating_control, internal_unit_scalar_calls, installed_provider_unit_scalar_calls, unit_scalar_homes, unit_integer_constants, unit_affine_scalar_records, unit_structural_scalar_field_stores, unit_write_only_primitive_stores, scalar_structural_scalar_field_stores, unit_continuations, scalar_affine_cleanup, scalar_control_affine_cleanups, structural_return) - these are the legacy carriers the lib.rs:6-8 comment says are not reconstructed.
**Same job elsewhere.** Stack accounting: see 7h; also object_artifact.rs `stack_demand`.
**Unused/one-caller.** `validation::stack::validate` 1 caller (:24603); `empty_unsupported_records` 1 caller (:24568). The many fields of `ObjectFunction` that `empty_unsupported_records` requires empty are still `pub` fields of the struct (object_artifact.rs:341-373): ~14 dead carrier fields (suspected unused for production artifacts; not grepped individually).
**Stated limits.** `"shared object root or unsupported effect roster changed"` (:24502), `artifact.x86_feature_profile.is_some()` -> mismatch (:24487): x86 feature profile unsupported.
**Fail-open risks.** `if text.functions.is_empty() { BTreeMap::new() }` (:24517-24519): with no placed functions the temporary-stack map is empty and the later function loop does not run; `source::admit` already refused an empty function roster (:18538-18544), traced. None else seen.

### 7r. Error enum and tail (24702-24923)
`FunctionFragmentObjectArtifactError` (:24703-24778, 24 variants incl. `UnaccountedOperation`, `Mismatch(&'static str)`, `Unsupported(&'static str)`, `Installation*`), `Display` (:24780-24905), `host` (:24910: `usize::try_from`), `refuse_symbol_collision` (:24914). Two string-based variants (`Mismatch`, `Unsupported`) carry most failures (message strings, not machine-readable); grepped counts of `Error::Mismatch(` not run.

---------------------------------------------------------------------------------------------------

## Coverage

All of unit 5 = 21523 lines (12461 + 2028 + 237 + 135 + 931 + 5506 + 225).

- **Read in full: 19328 lines.** `lib.rs` 1-135; `installation_record.rs` 1-237; `code_signature.rs` 1-225; `object.rs` 1-931; `image.rs` 1-2028; `object_artifact.rs` 1-5506; `function_fragments.rs` 12463-15861, 16770-19778, 20170-20489, 20704-21663, 22123-23201, 23425-24923.
- **Skimmed: 0.** (Test code I did read, I read to understand what shapes it exercises, not line by line for correctness: function_fragments.rs 14473-15861, 19719-19778, 22427-22489, 24196-24392 and the tests in image.rs/object_artifact.rs.)
- **Not read: 2195 lines**, all inside test modules (`#[cfg(test)]`) of `function_fragments.rs`, except the first entry which I did not look at beyond its first lines:
  - `function_fragments.rs:15862-16769` (908 lines, `structural_fields::tests`)
  - `function_fragments.rs:19779-20169` (391 lines, `source::tests` incl. `temporary_stack`)
  - `function_fragments.rs:20490-20703` (214 lines, `byte_input::tests`)
  - `function_fragments.rs:21664-22122` (459 lines, `primitive_locals::tests` + one `pub(super) use` at 22122)
  - `function_fragments.rs:23202-23424` (223 lines, `validation::tests`)
  (Sum: 908+391+214+459+223 = 2195. Check: 19328 + 2195 = 21523.)

## Five things a maintainer most needs to know about this unit

1. **The slice is a replay validator, and `source::admit` (function_fragments.rs:18534) is its gate.** It walks every abstract operation of every function and demands an arm in one big `match` (:18699-19025) that justifies it; unmatched kinds fail with `UnaccountedOperation` (:19024-19032). It is called from one place, `validation::validate` (:24432). Adding a new `AbstractOperation` variant means adding an arm here or it is rejected.
2. **Several arms in `admit` accept without checking anything**: `AtomicEvent` (:18745), integer/boolean constants (:18864-18865), the arithmetic/comparison family (:18995-19007), `Return`/`ReturnUnit` with empty cleanup (:18988, :18992). Other arms (BoundaryCall :18938-18947) are an OR of ten predicates. Whether another layer (attribution replay, lines 8-1472, outside my unit) covers the unchecked arms is not established here.
3. **Same decision written twice (build + check) with hard-coded per-target lists**: byte-read/byte-write/process-exit settlements (`settlement` vs `validate` in byte_input/byte_output/process_exit), `incoming_location` vs `validate_incoming_location`, `copy_extent` vs `validate_copy`. The target lists are hard-coded in object_artifact.rs:4198-4207 (process exit byte counts 12/16), :4372-4389 (byte output 40/42), :4256-4264 (decoder) and cover linux_x64, macos_x64, linux_arm64, macos_arm64 only - no windows_x86_64 branch in those shapes. `can_emit_executable_image` (image.rs:1806) does list (Coff, X86_64).
4. **Fixed shape lists drive acceptance**: `requires_graph_storage_replay` (:18405) lists ~45 operation kinds; `address_output`/`address` support five instruction kinds; `condition_layout` replays only Field/Case/FixedIndex projections and re-implements structural layout (object_artifact.rs:4469-4950) with one consumer file; the layout is a fifth copy of `checked_align_up` (:4473) with a different integer-alignment cap (min 8 at :4523 vs min 16 at :4909).
5. **Dead/unreferenced surface**: `InstallationError` (installation_record.rs:36, ~190 variants, re-exported lib.rs:129) has no constructor and no named importer in omega-rust by grep; `can_emit_executable_image` has no external caller; `empty_unsupported_records` (:24678) demands ~14 legacy `ObjectFunction` carrier fields be empty, which are still public struct fields (object_artifact.rs:341-373). `image.rs` emission paths do not run `validate_unit_shim` for `ProvisionedUnit` (image.rs:1019, :1851-1858) but the replay validators do (:1941-1952, :264-275).

## What I am unsure about

- Whether the unconditional-`true` arms in `admit` (atomic events, constants, arithmetic, empty-cleanup returns) are covered by checks in `attribution::validate` or in the machine layer; I did not read lines 1-12462 of the same file.
- Whether `validation::validate_function_fragment_object_artifact` runs on the normal compile path or only in harness/validator requests: grep shows callers in object.rs (composition), `function_fragments.rs:4108` (`replay::validate`, outside unit), image.rs:224/1894 (replay), but I did not read `production` (3572-3850) to see if `build_function_fragment_object_artifact` calls it.
- The multi-line `use` forms that could import `InstallationError` from `image_emission`; the grep was single-line.
- Test code (2195 lines) unread: any behaviour pinned there that contradicts what I recorded would not be noticed.
- `private_store`/`private_address` in `spill_values` (13833, 13846): I saw no caller in the lines I read but did not grep them.
- Which of `source_value` (12621) and `SelectedAddressInputs` is used by outside-unit callers (grepped only `pub(super)` visibility, not callers).
