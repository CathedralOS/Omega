# Record: unit_020 (area 11-native-stages-11-15)

Paths are relative to `source-rust/subsystems/compiler/psi-to-native/pipeline/`. `S14` = `14_placed-functions-to-objects/src/`,
`ELF` = `15_objects-to-image/image-elf/src/`. Reading method per file is stated in each entry's "Read" line.
Test modules (`#[cfg(test)]`) are marked as such; they were read only where noted.

## Stage 14 (crate `object-file`)

### S14/object_plan.rs (598 lines) - read in full
1. **Purpose.** Defines the mutable, arena-backed object plan and relocation plan that the image writers consume, and the one function that turns placed foreign-import calls into relocation records.
2. **Shape.** `ObjectFileLayout` (`:17`: `sections`, `symbols`, `function_symbols` arenas, `normalized_imports: Vec`, `entry_symbol`); `ObjectPlan{target, layout}` (`:96`); `RelocationPlan{target, record_set}` (`:369`); `RelocationRecord` (`:422`: origin, section, offset, byte_width, symbol_handle, addend, kind); `RelocationOrigin` (`:450`: `Instruction`, `SemanticOperation`, `SemanticEdge`, `Materialization`); `RelocationKind` (`:523`, five kinds); `SymbolKind` (`:362`: Function/Import/Object); `SectionKind` (`:302`: Text/Data/Bss). Entry: `ObjectPlan::with_import_relocations` (`:115`) -> `record_import_call` (`:137`). `invocation_relocation_origin` (`:541`), `relocation_kind_tag` (`:563`).
3. **In/out.** `with_import_relocations` reads `PlacedImportCall` slices (`:107`) (field resolution, caller symbol, locator, callbacks from `machine_code`) and returns `(ObjectPlan, RelocationPlan)`; it mutates the plan by adding import symbols (`insert_normalized_import` `:50`).
4. **Decides on.** Caller must be an existing symbol with `kind == Function` and `section == Text` (`:153-160`) - keyed on a symbol handle. Import is found by comparing the locator value to existing `normalized_imports` (`:206-211`), else a new symbol is named by `crate::normalized_import_symbol_name` and refused if any existing symbol has the same name (`:215-224`) - a name string compare. Callback target is found by function identity through `crate::object_function_reference` (`:163`).
5. **Cases by shape.** Callback encodings: only `X86_64Relative32` (1 relocation) and `Aarch64PageAddress` (2) (`:122-125`, `:187-204`). Resolution kinds: two (`:531-538`). Origin mapping from `InvocationSite`: `Operation` -> SemanticOperation; `InvokeNoNormal | ExternalCompletion | Cleanup` -> SemanticEdge (`:545-559`).
6. **Same job elsewhere.** Relocation-kind mapping from entry fixups is repeated in `S14/program_entry_object.rs:24-29`. `FinalImageSection -> u8` tag is repeated at `ELF/dynamic_executable.rs:674` (`image_section_tag`) and `ELF/procedure_linkage.rs:571` (`section_tag`) (read). `relocation_kind_tag` (`:563`) grepped: 15 uses of `relocation_kind_tag|image_section_tag` in 9 files.
7. **Unused/one-caller.** Grep `with_import_relocations|insert_import\(|\.with_capacities\(|with_roots\(` over `source-rust/` = 24 hits in 13 files, mixed with other types of the same method names; not isolated per type. `ObjectFileLayout::with_roots` (`:26`) and `RelocationRecordSet::with_roots` (`:380`) were not individually counted. Not separately confirmed unused.
8. **Stated limits.** None in text.
9. **Fail-open.** `ObjectFileLayout::with_capacity*` set `entry_symbol` to `ObjectSymbolHandle::invalid()` (`:76`, `:90`); `RelocationRecord::default` uses `Handle::invalid()` and kind `Aarch64Branch26` (`:432-446`). Anyone constructing from `Default` gets an invalid handle rather than an error. Open (not traced).

### S14/program_entry_object.rs (73 lines) - read in full
1. **Purpose.** Wraps an already-laid-out hosted entry or physical entry with its relocation kinds.
2. **Shape.** `HostedProgramObject<Context>` (`:11`) with `from_layout` (`:19`) and `relocations()` (`:47`); `PhysicalProgramObject<Context>` (`:55`) with `from_layout` (`:62`), `into_parts` (`:70`).
3. **In/out.** Reads `ResolvedHostedProgram` / `ResolvedPhysicalProgram` from stage 12 (`placement::program_entry_layout`), `EntryFixupKind`s; outputs object wrappers.
4. **Decides on.** Maps each `EntryFixupKind` (4 variants) to a `RelocationKind` by exhaustive match (`:24-29`).
5. **Cases by shape.** Four fixup kinds only.
6. **Same job elsewhere.** See object_plan `From<NormalizedForeignCallResolutionKind>` (`:531`).
7. **Unused.** Grep `HostedProgramObject|PhysicalProgramObject` = 8 hits in 4 files (driver `object_emission.rs` 2, this file 4, `image-emission/src/entries.rs` 1, `hosted_program.rs` 1).
8. Stated limits: none. 9. Fail-open: none seen.

### S14/target_matrix.rs (109 lines) - read in full
1. **Purpose.** Closed table from (architecture, object format) to entry symbol name, section names and text alignment.
2. **Shape.** `ObjectTargetPolicy` (`:12`), `object_target_policy` (`:36`).
3. **Decides on.** Match on `(Architecture, ObjectFormat)` pairs (`:37-59`). Elf/aarch64 -> `main`, `.text`, align 4; MachO aarch64 -> `_main`, `__TEXT,__text`, 4; x86_64 Elf|Coff -> align 1; x86_64 MachO -> 1; `(Aarch64, Coff)` -> `None` (`:58`).
5. **Cases by shape.** "five supported `(architecture, object_format)` pairs" (header comment `:1-7`); Coff for x86_64 shares the Elf row.
7. Grep `object_target_policy` = 20 hits in 6 files (names.rs 4, relocation_free_object/mod.rs 2, text_section.rs 3, this file 9, bridge_object.rs 1, lib.rs 1).
9. **Fail-open.** None seen; unlisted pair returns `None` and callers map it to `NonCanonicalTarget` (`relocation_free_object/mod.rs:272-274`, `text_section.rs:36-40`) - traced for those two callers only.

### S14/relocation_free_object/mod.rs (1631 lines) - read in full (tests at `:1073-1631` read)
1. **Purpose.** Defines the immutable, hash-identified relocation-free object plan (`OMGTRO` v6) with its validator, encoder and decoder.
2. **Shape.** `RelocationFreeObjectPlan` (`:118`: identity, source_text_section, psi, work_schedule, selected, selections, target, text_section, symbols, semantic_entry, normalized_imports, unresolved_normalized_foreign_calls, unresolved_image_addresses, relocation_record_count, relocation_requirements); `RelocationFreeObjectRelocationRequirements` (`:77`, three variants); `validate_relocation_free_object` (`:266`); `encode_relocation_free_object` (`:475`); `decode_relocation_free_object` (`:492`); `encode_plan_content` (`:519`); `decode_plan_content` (`:734`); constants `CONTAINER_MAGIC=b"OMGTRO\0\0"`, `CONTAINER_VERSION=6` (`:36-37`).
3. **In/out.** Validate reads a plan; encode writes magic+version+identity+content bytes; decode re-parses and calls validate (`:508`). Identity is recomputed from `PLAN_SCHEMA` + content (`:139-147`).
4. **Decides on.** Identity equality (`:269`); target via `object_target_policy` and pointer size/alignment both 8 (`:272-277`); text section name and alignment against the policy (`:278-283`); symbol ids by position (`:366-371`); symbol names compared to the string `__omega_terminal_machine_{id}` (`:244-246`, `:381`); reserved names `"main"`/`"_main"` by string compare (`:384`); symbol intervals dense by running cursor (`:387-395`); exactly one semantic entry (`:417-421`); imports sorted by `(boundary, ordinal)` (`:431-437`) and named by `__omega_terminal_normalized_foreign_import_{b}_{o}` (`:248-257`, `:438`); each foreign call's coordinate must equal its import's coordinate (`:453`); field width must be 4 (`:259-264`, `:457`); every import used (`:469`).
5. **Cases by shape.** Three relocation requirements, each with fixed rules (`:293-358`). Foreign call kinds: two (`:259`). Image-address fields: `X86_64Relative32` or `Aarch64PageAddress` (`:342-350`). Register decode table is a fixed list: x86 rax..r15 (family 1), xmm0-15 (2), aarch64 x0-30 (3), v0-31 (4) (`:937-957`).
6. **Same job elsewhere.** `ELF` builds its own canonical import list from `FinalImage` (`ELF/imports.rs:43`); same "sorted, no duplicates, every import used" shape on a different representation. Wire codec for registers duplicates `MachineRegister::wire_class_index` (called `:677`) with a decode table written by hand (`:937`) - suspected parallel, `calling_conventions` not read.
7. **Unused.** Grep `encode_relocation_free_object|decode_relocation_free_object|validate_relocation_free_object_from_text|construct_relocation_free_object_from_text` = 44 hits in 4 files (fragment_container.rs 7, lib.rs 3, this file 23, text_section.rs 11).
8. **Stated limits.** `:1407` test "rejects the old split container version" (versions 3, 4, 5 refused); only version 6 accepted (`:500`).
9. **Fail-open.** `RelocationFreeObjectSymbolPolicy` and `Linkage` have one variant each; decode accepts only tag 1 (`:759-779`). `decode_machine_register` returns an error on unknown pairs, not a default (`:957`). Image-address `fields` bound is checked only for `UnresolvedObjectSymbolFieldsV1` (`:319-357`); for `UnresolvedNormalizedForeignImportFieldsV1` the validator rejects non-empty image addresses (`:309`). none seen otherwise. The validator does not check that `semantic_entry` is a machine present in `symbols` beyond requiring exactly one symbol marked entry (`:396-421`): traced, covered by the `entry_count` check. none seen.

### S14/relocation_free_object/publication.rs (368 lines) - read in full
1. **Purpose.** Defines the `OMGTOM` v1 manifest that records identities and counts of a validated object container, and states that executable image, installation and publication are unavailable.
2. **Shape.** `FunctionFragmentObjectContainerManifest` (`:51`), `...Statistics` (`:40`), `relocation_free_object_statistics` (`:106`), `encode` (`:133`), `decode` (`:142`), `recomputed_identity` (`:127`).
3. **Decides on.** Magic/version (`:146-154`), stage tag 1 (`:156-161`), four "unavailable" bytes must each be 1 (`:206-212`), recomputed identity equals stored (`:239`).
4. As above. 5. Fixed fields `external_entry_bridge`, `executable_image`, `installation`, `publication` have one variant `Unavailable` (`:35-37`).
7. Grep `FunctionFragmentObjectContainerManifest` = 58 hits in 7 files (this file 37, fragment_container.rs 15, others 4).
9. Fail-open: decode does not re-check statistics against any object (only the manifest's own identity, `:239`). A manifest with wrong counts but a self-consistent identity decodes. Traced: no object is available to the decoder; callers not read. Open.

### S14/relocation_free_object/text_section.rs (339 lines) - read in full
1. **Purpose.** Builds a `RelocationFreeObjectPlan` from a placed text section and checks a plan equals the one rebuilt from its text.
2. **Shape.** `construct_relocation_free_object_from_text` (`:32`), `validate_relocation_free_object_from_text` (`:72`), `assemble_object` (`:86`).
3. **In/out.** Reads `RelocationFreeTextSectionPlacement` (`machine_code`) and `OptimizationSelectionIdentity`; returns the plan.
4. **Decides on.** Entry role: `function.machine == text.semantic_entry` (`:46`). Import symbols: deduplicated set of `(boundary, ordinal)` in sorted order (`:95-110`). Validation = `validate_relocation_free_object` then full equality with a fresh construction (`:77-83`).
5. Relocation requirements: three-way match (`:119-129`).
9. **Fail-open.** `import_symbols[&(...)]` index (`:115`) panics rather than accepting if a call is missing from the set; it cannot be missing because the set is built from the same list. none seen.

## Stage 15 ELF writer (crate `image-elf`)

### ELF/lib.rs (704 lines) - read in full
1. **Purpose.** Crate root: documents the static and 22-stage dynamic ELF writers, defines shared byte/name helpers and constants, entry address helper, and a large `#[cfg(test)]` re-export block plus `test_support` fixtures (`:204-704`).
2. **Shape.** `mod bytes` (`:46`: `name_at`, `require_name`), `mod constants` (`:81`: `IMAGE_BASE=0x400000`, `ELF_MAX_PAGE_SIZE=0x1_0000`, `SECTION_COUNT=14`, header sizes), `mod entry_symbol::elf_entry_address` (`:111-120`), public re-exports `emit_elf_executable`, `emit_elf_dynamic_executable`, `emit_elf_shared_component`, `ValidatedElfDynamicExecutable` (`:40-44`, `:129-132`).
3. **Decides on.** `name_at` requires a NUL terminator inside the table (`:58-61`). `elf_entry_address = text_address + entry_symbol.offset` (`:119`), through `image::final_image_entry_symbol`.
5. `PROGRAM_HEADER_COUNT = 2` (static) (`:84`). Dynamic uses 5 headers (`file_assembly.rs:1270`).
7. Callers outside `image-elf`: grep `emit_elf_dynamic_executable|emit_elf_shared_component|emit_elf_executable` excluding `**/image-elf/**` = 4 hits in 2 files (image-emission `image.rs` 3, `object.rs` 1).
8. **Stated limits.** Header comment: static emission fails closed "if any referenced import exists" (`:8`).
9. **Fail-open.** `entry_symbol::tests` is an empty module (`:122-123`). none seen otherwise.

### ELF/imports.rs (435 lines) - read in full
1. **Purpose.** Extracts the list of referenced ELF imports (symbol handle, normalized locator, every relocation site) from a `FinalImage`, rejecting malformed rows.
2. **Shape.** `ElfImportLocator` (`:17`: `StringBackedBootstrap`, `Component`, `Versioned`), `ElfImportRequest` (`:37`), `canonical_referenced_imports` (`:43`).
3. **Decides on.** Duplicate `symbol_handle` rows (`:51`); import symbol must be `Import`, section `None`, offset 0, size 0 (`:65-69`); bootstrap rows need a non-empty library and symbol without NUL (`:77-80`); locator target must be Linux arm64 or x64 and equal image target (`:120-125`); two locators with the same `non_authoritative_compatibility_fingerprint` are refused as ambiguous (`:133-150`) - a hash compare of a value named non-authoritative; relocation sites are the relocations whose `symbol_handle` equals the import (`:162-168`); an import with no relocations is silently skipped (`:169`); relocations naming an import without a request are refused (`:178-197`).
5. Three locator kinds; `FinalImageImportPlan::None` refused (`:154-159`). Component object spelled `"$ORIGIN/{image_filename}"` (`:116`).
7. Grep `canonical_referenced_imports` = 17 hits in 4 files (imports.rs 11, import_sections.rs 2, shared_component.rs 2, static_executable.rs 2).
9. **Fail-open.** (a) Unreferenced imports are dropped (`:169-175`): "stated" by header ("rejecting unreferenced ... rows" claims rejection at `:3-5`, but the code skips them) - the comment and code disagree; traced here: `requests` contains only imports with relocations. (b) Relocations whose `symbol_handle` is invalid are skipped (`:179-185`, `continue`): a relocation naming a dead handle passes this function; open.

### ELF/dynamic_executable.rs (685 lines) - read in full
1. **Purpose.** Hosts the shared `checked` validation helpers and macros, the stage error enum, and the 22-call driver `emit_elf_dynamic_executable`.
2. **Shape.** `mod checked` (`:9`: `require`, `checked_sum/product`, `read_*`, `field_mut`, `decode_section_header_rows` `:168`, `decode_dynamic_rows` `:249`, `seal_stage!` `:282`, `plan_stage!` `:387`, `plan_candidate` `:429`, `validate_candidate` `:365`); `ElfDynamicExecutableEmissionError` (`:529`, 22 boxed variants); `emit_elf_dynamic_executable` (`:614`); `image_section_tag` (`:674`); `checked_u64` (`:683`).
3. **In/out.** Takes `FinalImage` + `NormalizedElfInterpreterPlan`, returns `ValidatedElfDynamicExecutable` or the boxed failing stage error holding the stage's input.
4. **Decides on.** Each stage: derive contents, compute fingerprint, rebuild contents again in `validate_*` and compare ("replays") (`:365-385`). Fingerprint is Fnv1a (`:15`, `:127`).
5. A fixed 22-stage chain (`:619-662`); stage list mirrored by 22 enum variants and `failing_stage` strings (`:555-590`).
6. **Same job elsewhere.** Every stage module repeats the derive-then-replay shape (`seal_stage!`). `image_section_tag` (`:674`) duplicates `procedure_linkage.rs:571`.
7. `checked_u64`, `checked` re-exported from lib.rs `:110`; not individually counted.
8. Stated limits: none. 9. Fail-open: `seal_stage!` third form uses `|_| true` for the replay check and message `""` (`:353-354`) - used by stages without fingerprint (e.g. `dynamic_sections`, `import_sections.rs:2669-2674`); "stated" by absence of a fingerprint field. Fingerprint-bearing stages use `candidate.$fingerprint == $fingerprint(...)`; the `nonzero` form also rejects 0 (`:327`).

### ELF/import_sections.rs (3294 lines) - read lines 1-1805 and 2145-2935 in full; 1806-2144 and 2936-3294 (test modules) not read
1. **Purpose.** Stages 1 to 4 of the dynamic ELF pipeline: join image + interpreter into link inputs; plan `.dynsym/.dynstr/.hash/.gnu.hash/.gnu.version/.gnu.version_r` contents; serialize them to bytes with an independent decoder; plan the seven section descriptors.
2. **Shape.** `mod dynamic_link` (`:10`: `PlannedElfDynamicLinkInputs` `:18`, `plan_elf_dynamic_link_inputs` `:66`); `mod dynamic_section_bytes` (`:340`: `serialize_elf_dynamic_sections` `:451`, `encode_payloads` `:463`, `validate_payloads` `:598`, decoders `decode_dynsym/sysv_hash/gnu_hash/versym/verneed` `:670-971`); `mod dynamic_section_descriptors` (`:1302`: `plan_elf_dynamic_section_descriptors` `:1446`, `derive_contents` `:1461`, `validate_contents` `:1589`); `mod dynamic_sections` (`:2146`: `plan_elf_dynamic_sections` `:2313`, `derive_import_contents` `:2350`, `derive_shared_import_contents` `:2343`, `build_sysv_hash` `:2560`, `build_gnu_hash` `:2586`, `build_version_needs` `:2614`, `validate_contents` `:2676`).
3. **In/out.** `FinalImage` + interpreter -> `ElfDynamicSectionContents` (symbols, hashes, versym, verneed, `needed`, `bindings`) -> byte payloads -> descriptor rows with fixed name offsets (`:1318-1325`) into a fixed seed `"\0.interp\0.dynstr...\0"` (`:1327-1328`).
4. **Decides on.** Interpreter target equals image target (`:78`); imports must be non-empty (`:92`); each import's profile equals interpreter profile and is Linux arm64/x64 (`:116-121`); string-backed bootstrap imports refused (`:106`). Imports ordered by `(symbol, object, version, fingerprint)` byte compare (`:2454-2467`). Version index = position of `(object, version)` in a sorted vec plus 2 (`:2523-2542`); empty version -> index 1 (`:2549`). All matching is by byte-string spelling of library/symbol/version.
5. **Cases by shape.** GNU hash is fixed at one bucket, one bloom word, shift 5, symbol offset 1 (`:2160-2163`), validated to exactly that (`:2803-2817`). SysV hash buckets = max(1, symbol count) (`:2561`). Seven descriptor kinds in fixed order (`:1329-1337`, validated `:1612-1630`). Symbol info fixed `STB_GLOBAL<<4 | STT_FUNC`, section `SHN_UNDEF` (`:2367-2369`); Component imports get empty version (`:2447`).
6. **Same job elsewhere.** `derive_shared_import_contents` (`:2343`) is the shared-library variant of the same derivation; grep `derive_shared_import_contents|StringBackedBootstrap`-style combined count = 11 hits in 5 files, so a `shared_component.rs` caller exists (suspected, file not read). `validate_row` (`:1669`) re-spells the same table as `derive_contents` (`:1469-1547`) on purpose (independent replay). `elf_hash` `:2841` and `gnu_hash` `:2854`: grep `fn elf_hash|fn gnu_hash|fn sysv_hash` found only this file's definitions (plus a test name).
7. **Unused/one-caller.** Grep `canonical_referenced_imports` shows it is called here (2 hits) and from `shared_component.rs` and `static_executable.rs`.
8. **Stated limits.** None in prose. Constants cap versions: error "ELF required-version count exceeds the unhidden version-index domain" (`:2535`).
9. **Fail-open.** `decode_dynsym` etc. fail on malformed input. `validate_payloads` skips symbol 0 when checking names (`:647`): "stated" by ELF reserved row. `name_at` applied to `needed` offsets. none seen.

### ELF/procedure_linkage.rs (3466 lines) - read lines 1-580 and 1955-2434 in full; lines 581-1954 and 2435-3466 not read (outline obtained by grep only)
1. **Purpose.** Stages 5 to 7: group import relocations into call sites and data slots, build `.plt`, `.got.plt`, `.rela.plt` and `.rela.dyn` templates for x86-64 and AArch64 with fixups, and plan their section descriptors.
2. **Shape (read).** `mod dynamic_import_relocations` (`:9`): `ElfLogicalProcedureLinkageSlot` (`:98`), `ElfDirectImportCallSite` (`:108`), `plan_elf_procedure_linkage_relocations` (`:140`), `canonical_relocation_classes` (`:253`). `mod dynamic_linkage_templates` (`:1955`): `candidates::derive_contents` (`:1985`), `place_template_bytes` (`:2021`), `place_import_fixups` (`:2122`), `derive_fixups` (`:2207`), `append_data_fixups` (`:2304`), `fixup_field` (`:2408`). Outline only (not read): `mod dynamic_linkage_descriptors` (`:1129`), `plan_elf_procedure_linkage_templates` (`:3117`), `validate_*` functions (`:2715-3035`), `replay_x86_plt`/`replay_aarch64_plt` (`:2861`, `:2890`).
3. **In/out.** Reads descriptor plan (with image text/data and import list); writes template bytes and fixup tables.
4. **Decides on.** A relocation is a "direct call" when `section == Text && kind == expected_kind && byte_width == 4 && addend == 0` (`:286-294`); otherwise it must be a data slot: `Data`, `Absolute64`, width 8, offset multiple of 8, inside `.data` (`:302-313`), else refusal "non-procedure or malformed source relocation". Call sites verified by bytes: x86 `0xe8` opcode at offset-1 and four zero bytes (`:347-350`); AArch64 `[0,0,0,0x94]` and offset multiple of 4 (`:355-358`). Call-site spans must not overlap (`:377-417`).
5. **Cases by shape.** Targets: only `LinuxX64` and `LinuxArm64`; others refused (`:216-234`, `:1992`). x86 import entries need exactly one `X86SignedOrdinal32` binding-ordinal operand; AArch64 none (`:2043-2076`). Operand kinds accepted: `X86Relative32`, `Aarch64Page21`, `Aarch64PageOffset12{scale:3}` (load), `{scale:0}` (add); other -> "unsupported encoded ELF import operand" (`:2164-2173`). Fixed template sizes: x86 PLT header 16, entry 16; AArch64 header 32, entry 16, GOT header 3 words (`:3081-3091` by grep).
6. **Same job elsewhere.** `section_tag` (`:571`) duplicates `dynamic_executable.rs:674`.
7. **Unused.** `derive_fixups` (`:2207`): grep `derive_fixups\(` = 2 hits, definition and a call at `:2731` inside `validate_contents`; this is the replay side, production derivation uses `place_import_fixups` (`:2122`) plus `append_data_fixups`. These two are separate derivations of the same fixup list (the non-test copy at `:2731` is read as part of validation by outline only).
8. **Stated limits.** "unsupported encoded ELF import operand" (`:2173`), "unsupported ELF GOT/PLT fixup target" (`:2297`, `:2341`).
9. **Fail-open.** `ordinals.next()` for non-x86: if the import body has a binding ordinal operand on AArch64 it is refused (`:2072`). `append_encoded_operands` skips `BindingOrdinal` operands with `continue` after verifying kind and presence of ordinal (`:2156-2163`): "stated" (`// Filled from the canonical import table`). none seen otherwise in read ranges.

### ELF/dynamic_table.rs (2219 lines) - read lines 1018-1437 in full; lines 1-1017 and 1438-2219 not read (outline grep only)
1. **Purpose.** Stages 8 to 10: plan the `.dynamic` rows and their address obligations, serialize them, and plan the `.dynamic` section descriptor. (Modules per grep: `dynamic_table_descriptor` `:8`, `dynamic_tag_bytes` `:494`, `dynamic_tags` `:1018`.)
2. **Shape (read).** `ElfDynamicTag` enum (`:1114`, 19 tags), `ElfDynamicValue` (`:1137`), `ElfDynamicAddressObligation` (`:1175`), `plan_elf_dynamic_tags` (`:1187`), `derive_contents` (`:1199`), `validate_contents` (`:1371`).
3. **In/out.** Reads the eleven-section descriptor carrier; outputs row list (`NEEDED*`, sizes, address placeholders, `FLAGS = DF_BIND_NOW`, `NULL`) and address obligations for the later address-fix stage.
4. **Decides on.** Requires descriptor count exactly 11 (`:1376`), profile Linux x64/arm64 (`:1380`), at least one `DT_NEEDED` (`:1388`), row count `needed + 15 (+3 if version requirements)` (`:1391-1399`).
5. **Cases by shape.** Fixed 15-row unversioned layout plus 3 versioned rows (`:1037`, `:1205`); `DF_BIND_NOW` always set (`:1310`).
7-8. Not counted. 9. Fail-open: none seen in read range.

### ELF/load_placement.rs (3379 lines) - read lines 586-985 in full; lines 1-585, 986-3379 not read (outline grep only)
1. **Purpose.** Stages 16 to 18: choose absolute file offsets and virtual addresses for the dynamic image (`load_layout`), apply section header placements (`placed_section_headers` `:1861`), and apply dynamic address fixups (`resolved_dynamic_table` `:2509`).
2. **Shape (read).** `mod candidates` of `load_layout`: `derive_contents` (`:641`) lays out: header prefix, read-only region, read-execute region (page aligned to `ELF_MAX_PAGE_SIZE`), read-write region (page aligned), file-only region; builds five program headers (`Interpreter`, `LoadReadOnly`, `LoadReadExecute`, `LoadReadWrite`, `Dynamic`) (`:802-857`); `validate_candidate` (`:907`) replays derivation.
3. **Decides on.** Target Linux x64/arm64 only (`:645-648`); relative roster has exactly 14 rows (`:651`); every one of 4 regions non-empty (`:654-664`); BSS alignment non-zero power of two (`:702`); section-header fixup count exactly 25 (`:860-863`); `IMAGE_BASE` and `ELF_MAX_PAGE_SIZE` fixed (`:920-921`).
5. **Cases by shape.** `DYNAMIC_PROGRAM_HEADER_COUNT=5`, `PLACEMENT_FIXUP_COUNT=25`, `SECTION_COUNT=14` (grep `:1791`, `:1793`, `lib.rs:92`).
8. Stated limit: none. 9. Fail-open: none seen in read range; `physical_address` of the interpreter header uses `unwrap_or(0)` (`:812`) after a preceding `ok_or_else` on the same value (`:807`), so unreachable.

### ELF/file_assembly.rs (3948 lines) - read 1-450, 1378-1452, 1637-1706, 2089-2208, 2462-2661 in full; remaining ranges not read (outline grep only)
1. **Purpose.** Stages 19 to 22: serialize the ELF header and program headers (`dynamic_file_envelope` `:1251`), apply PLT/GOT fixups and image-local relocations (`resolved_procedure_linkage` `:2089`), assemble the file (`assembled_dynamic_file` `:10`), admit final bytes.
2. **Shape (read).** `ValidatedElfAssembledDynamicFile` (`:78`), `ValidatedElfDynamicExecutable` (`:105`), `assemble_elf_dynamic_file` (`:172`), `admit_elf_dynamic_executable` (`:184`), `derive_executable_output` (`:213`), `procedure_got_import_custody` (`:254`), `serialize_elf_dynamic_file_envelope` (`:1378`), `expected_entry_address` (`:1637`), `target_machine` (`:1696`), `derive_contents` for resolved linkage (`:2119`), `derive_application` (`:2167`), `relocate_image_local_memory` (`:2476`).
3. **In/out.** Resolved linkage in, `ExecutableImageOutput` (bytes, regions, format string) out; header written with `e_type = ET_EXEC` (`:1406`), 5 program headers (`:1415`), 14 section headers (`:1417`).
4. **Decides on.** Entry symbol must be a `Function` in Text with non-zero size, within text (`:1652-1671`). Each fixup field must be an exact zero placeholder under its mutable mask (`:2181-2184`). Slots bound by logical ordinal in a `BTreeMap`; double binding refused (`:288-309`). Slot symbol must be UTF-8 and the locator `Versioned` or `Component` (`:331-340`). Format string from target match (`:393-401`). Image-local relocations are those whose symbol is not an import (`:2482-2486`) and are applied with `image::apply_relocations` (`:2505`).
5. **Cases by shape.** Linux x64/arm64 only (`:1696-1703`, `:2497-2503`). `ELF64_PROGRAM_HEADER_COUNT=5`, `ELF64_SECTION_HEADER_COUNT=14` (`:1270`, `:1273`). Output file name is the literal `"omega-program"` (`:240`).
7. `validate_executable_output` (`:353`) is `#[cfg(test)]`; grep `validate_executable_output` = 7 hits, all in this file (definition `:353`, test import `:759`, test calls `:1230-1246`).
8. **Stated limits.** Header: "re-verifying final-byte admission" (`:5-7`).
9. **Fail-open.** (a) `admit_elf_dynamic_executable` (`:184-211`) builds the output through `derive_executable_output` and does not call `validate_executable_output` in non-test builds, so stage 22 "re-verification" in the header does not run byte checks at production time; the bytes were already sealed by stage 21's `validate_contents`. Traced for this file (grep above); callers' extra checks not traced. (b) `recover_retained_image` (`:403-427`) unwinds all 22 stages and discards each stage's contents with `_`; the returned image takes text/data from the output (`:203-204`). none seen otherwise.

## Coverage

Full reads (lines): object_plan 598; program_entry_object 73; relocation_free_object/mod 1631; publication 368; text_section 339; target_matrix 109; image-elf lib 704; imports 435; dynamic_executable 685; import_sections 2596 (1-1805, 2145-2935); procedure_linkage 1060 (1-580, 1955-2434); dynamic_table 420 (1018-1437); load_placement 400 (586-985); file_assembly 915 (1-450, 1378-1452, 1637-1706, 2089-2208, 2462-2661). Total about 10333 of 21248.

Skimmed (grep outline of declarations/keywords only, no body reading): the unread parts of `procedure_linkage`, `dynamic_table`, `load_placement`, `file_assembly`.

Not read:
- `ELF/import_sections.rs`: 1806-2144, 2936-3294 (test modules)
- `ELF/procedure_linkage.rs`: 581-1954, 2435-3466
- `ELF/dynamic_table.rs`: 1-1017, 1438-2219
- `ELF/load_placement.rs`: 1-585, 986-3379
- `ELF/file_assembly.rs`: 451-1377, 1453-1636, 1707-2088, 2209-2461, 2662-3948

## Five things a maintainer most needs to know

1. Every dynamic-ELF stage builds its output and then rebuilds it from scratch to compare ("replay"); changing a field in a `derive_contents` requires changing the matching `validate_*` (`ELF/dynamic_executable.rs:365-385`; `import_sections.rs:1461` vs `:1669`).
2. The final-byte admission at stage 22 does not re-run byte checks in production; the checking function is test-only (`file_assembly.rs:353`, 7 hits in one file).
3. Everything dynamic is specialised to Linux x86-64/AArch64 with exact counts: 14 sections, 5 program headers, 25 section-header fixups, 11 section descriptors, 15 fixed dynamic rows, one-bucket GNU hash (`load_placement.rs:651,860`; `dynamic_table.rs:1376,1391`; `import_sections.rs:2160-2163`).
4. `canonical_referenced_imports` skips an import with no relocations and skips relocations naming invalid handles, while its header comment says it rejects unreferenced rows (`imports.rs:3-5`, `:169`, `:179-185`).
5. The relocation-free object validator ties names to strings (`__omega_terminal_machine_{id}`, `main`/`_main` reserved) and the target matrix to five pairs; target or naming changes must be mirrored in `names.rs`, `target_matrix.rs` and the validator (`relocation_free_object/mod.rs:244,381-385`; `target_matrix.rs:36`).

## What I am unsure about

- About half of the unit (the second halves of four large ELF files) was not read; findings there are limited to grep outlines.
- Whether `names.rs`, `bridge_object.rs` and `fragment_container.rs` (outside this unit) use `RelocationFreeObjectPlan` the way the validator assumes.
- Whether callers of `emit_elf_dynamic_executable` in `image-emission` re-check output bytes after stage 22 (not traced).
- `shared_component.rs` and `static_executable.rs` were not in the unit; claims about their use of `derive_shared_import_contents` and `canonical_referenced_imports` rest on grep counts only.
