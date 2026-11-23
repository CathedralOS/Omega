# Record: unit_020 gap_007 (area 11-native-stages-11-15, crate `image-elf`)

Second pass over ranges of unit 020 the first reader did not read. All paths are under
`source-rust/subsystems/compiler/psi-to-native/pipeline/15_objects-to-image/image-elf/src/`
(abbreviated `E/`). The crate builds a Linux dynamic ELF executable as a chain of numbered stages. Each stage
takes a "validated" owner from the previous stage, derives new contents, replays an independent check of those
contents, and seals the result with a non-authoritative FNV fingerprint (macros `checked::plan_stage!`,
`checked::seal_stage!`, named at `E/dynamic_table.rs:124`, `:171`; the macro bodies are in
`E/dynamic_executable.rs`/`checked`, not in my ranges). Every stage rejects by returning the input owner plus a
`Diagnostic` (`image::rejection_carrier!`, e.g. `E/dynamic_table.rs:74`, `:551`).
Notation: "read" = read line by line in this pass; "grepped" = Grep tool.

## 1. `E/dynamic_table.rs` lines 1-1017 (module `dynamic_table_descriptor` 8-492, module `dynamic_tag_bytes` 494-1016)

### 1a. `dynamic_table_descriptor` (stage 10: `.dynamic` section descriptor), 8-492
1. **Purpose.** Adds one section descriptor (the `.dynamic` row) and its name to the existing eleven-row
   descriptor set and 113-byte section-name seed, so the later header table can describe `.dynamic`.
2. **Shape.** `ValidatedElfDynamicTableSectionDescriptorPlan { payload, contents, non_authoritative_descriptor_compatibility_fingerprint }` (26);
   `ElfDynamicTableSectionDescriptorContents { section_name_table_seed: Vec<u8>, descriptor }` (80);
   `ElfDynamicTableSectionDescriptor { kind, name_offset, section_type, flags, payload_size, alignment, entry_size, link: Option<ElfDynamicSectionKind>, info: Option<..> }` (100);
   entry `plan_elf_dynamic_table_section_descriptor` (118); `derive_contents` (133); `validate_contents` (180);
   `validate_name` (251); fingerprint fn (263). `descriptor_count()` returns a constant `UPSTREAM_DESCRIPTOR_COUNT + 1` (38), not a count of rows.
3. **I/O.** Takes `ValidatedElfDynamicTablePayload` (from 1b). Reads the base name seed through
   `payload.plan().descriptors().contents().section_name_table_seed` (163-169). Produces the seed with `.dynamic\0`
   appended (138-144) and a descriptor with `SHT_DYNAMIC`, `SHF_WRITE|SHF_ALLOC`, align 8, entry size 16,
   `link = DynamicString`, `info = None` (147-160).
4. **How it decides.** Replay in `validate_contents` is keyed on counts and fixed values: descriptor count must equal 11
   (185-188), seed length must equal 113 (190-193), the new seed must be the old seed plus the suffix byte-for-byte
   (199-205), exactly one `DynamicString` row must exist, found by `row.kind ==` (207-219), payload size must be
   `row_count * 16` (221-230), and every descriptor field is compared with constants (234-248). The name check is by
   byte offset into the seed and then a byte compare against `b".dynamic"` (`require_name`, 255-260; `name()` at 92-96).
5. **Cases by shape.** Hard-coded shape: "exact sealed eleven-row base" (186-187 message), "exact 113-byte upstream name seed" (192).
   `DYNAMIC_NAME_OFFSET: u32 = 113` is a literal (22). Any payload whose base seed or row count differs is refused.
6. **Same job elsewhere.** Same derive/validate/fingerprint pattern as `dynamic_tag_bytes` below (633-640 vs 171-178), and the
   later stage in `E/import_sections.rs` (descriptor stage; read in my ranges only in its tests, 1806-2143). Section-name offset checking via `require_name` shared with `import_sections` tests (`E/import_sections.rs:1816` imports `name_at`).
7. **Unused / one-caller.** `plan_elf_dynamic_table_section_descriptor`, `serialize_elf_dynamic_table` and the other stage entries appear in 95 grepped
   occurrences across 6 files of `image-elf/src` (Grep count over `source-rust`: dynamic_executable.rs 12, dynamic_table.rs 17, file_assembly.rs 40, lib.rs 12, load_placement.rs 10, section_headers.rs 4),
   all inside `image-elf`. `descriptor_count`, `appended_descriptor_count`, `section_name_seed_byte_count` are `pub const`/`pub` getters whose only
   callers I saw are the tests here (341-344); not grepped separately.
8. **Stated limits.** None in comments. Test names say "address-free" (337).
9. **Fail-open.** None seen. `into_parts`/`contents` are `pub(crate)` accessors only.

### 1b. `dynamic_tag_bytes` (stage 9: serialize `Elf64_Dyn` table), 494-1016
1. **Purpose.** Turns the semantic dynamic-tag plan into the raw 16-byte-per-row `.dynamic` bytes, leaving address
   values as zero placeholders and listing each placeholder as a fixup to resolve later.
2. **Shape.** `ValidatedElfDynamicTablePayload { plan, contents, fingerprint }` (511);
   `ElfDynamicTablePayloadContents { bytes, address_fixups: Vec<ElfDynamicPayloadAddressFixup> }` (554);
   `ElfDynamicPayloadAddressFixup { row_ordinal: u32, byte_offset, byte_width: u8, kind, target: ElfDynamicAddressTarget }` (566);
   entry `serialize_elf_dynamic_table` (580); `encode_contents` (592); `validate_contents` (642);
   `validate_fixups` (662); `validate_fixup_coverage` (710); fingerprint (732).
3. **I/O.** Reads `plan.contents()` rows (`tag`, `value`) and `address_obligations`. Writes `(row.tag as i64) as u64` and `encoded_value(row.value)` (602-603; `encoded_value` is defined in `dynamic_tags`, not in my range).
4. **How it decides.** Replay decodes the bytes with `decode_dynamic_rows` and compares `tag`/`value` row by row with the semantic plan (647-658). Fixup validation is positional: count equal, each fixup's `row_ordinal`, `byte_offset = ordinal*16+8`, width 8, kind and `target` equal the obligation at the same index (667-690), each placeholder reads 0 (691-694), each row has exactly one fixup iff its value is `AddressPlaceholder` (696-706), and no two fixup byte ranges overlap (710-729, quadratic loop).
5. **Cases by shape.** Single fixup kind `Elf64AbsoluteAddress` (562). Width is fixed to `ELF64_DYNAMIC_VALUE_SIZE` (685).
6. **Same job elsewhere.** Pairwise-overlap check at 710-729 is the same loop as `validate_nonoverlapping_applications` at `E/file_assembly.rs:2339-2364` (read) and the occupied-bitmap check at `E/file_assembly.rs:479-510`. Address-obligation vs row cross-check at 696-706 is the same job as `validate_address_obligations` at `E/dynamic_table.rs:1576-1648` (read in 1438-2219).
7. **Unused / one-caller.** `ElfDynamicPayloadFixupKind` has one variant (561-563); a single-variant enum, kept in fingerprint (746). Not grepped further.
8. **Stated limits.** None.
9. **Fail-open.** None seen. `row.tag as i64` casts at 602/654 are direct enum casts; no fallback.

## 2. `E/dynamic_table.rs` lines 1438-2219 (tail of `dynamic_tags`; module started earlier at ~1018 and is outside my ranges)

1. **Purpose.** End of the semantic-tag stage (stage 8): replay of `DT_NEEDED` rows, the fixed row list, address obligations, relocation closure, and the fingerprint. Header 1-6 says this module executes "stages 8 through 10".
2. **Shape.** `validate_needed_rows` (tail, ends 1464); `validate_fixed_rows` (1466); `address_row` (1569); `validate_address_obligations` (1576); `validate_relocation_closure` (1650); fingerprint `non_authoritative_tag_compatibility_fingerprint` (1684); `tagged_u64` (1734). Types (`ElfDynamicSemanticRow`, `ElfDynamicTag`, `ElfDynamicValue`, `ElfDynamicAddressObligation`) are defined before 1438 and I did not read them.
3. **I/O.** Reads `ValidatedElfProcedureLinkageSectionDescriptorPlan` (`descriptors`), uses `structural_contents(descriptors)` (`.needed`, `.verneed`) and `dynamic_payloads(descriptors)`, both defined outside my ranges. Produces nothing; returns `Result<(), Diagnostic>`.
4. **How it decides.** `validate_fixed_rows` builds the expected whole tail as a `Vec` of 14 rows, adds 3 rows if `!structural.verneed.is_empty()` (1517-1531), adds `Flags = DF_BIND_NOW` and `Null`, then requires `contents.rows.get(base..) == Some(fixed)` (1540-1543). So the decision is keyed on position and exact value of every row, plus presence of version needs. Then PLT relocation bytes must be nonzero, a multiple of `ELF64_RELA_SIZE`, and equal to `relocation_count * 24` (1549-1559), and `verneed.len()` must equal `required_version_object_count()` (1560-1565). `validate_address_obligations` slices a fixed 9-entry `expected` table to 7 or 9 entries by `has_version_requirements` (1615-1616). `DT_NEEDED` rows are checked for non-empty names and strictly ascending byte order via `prior_object < object` (1444-1455, keyed on the raw name bytes read with `name_at` from `dynstr`).
5. **Cases by shape.** Fixed table: "UNVERSIONED_ADDRESS_COUNT + 2" obligations (1615). `validate_relocation_closure` accepts only two targets: `LinuxX64` expects exactly 1 `FutureDynamicSection` GOT absolute fixup, `LinuxArm64` expects 0, any other target is `Err("unsupported relocation-closure target")` (1673-1681). The comment-level claim is the message at 1680: "target GOT policy drifted from the future .dynamic section obligation".
6. **Same job elsewhere.** Fingerprint hashing style repeats at 263-290, 732-750, `E/file_assembly.rs:717-750`, `2394-2460` (each uses `checked::stage_fingerprint` or `Fnv1a::new`, `file_assembly.rs:721` uses `Fnv1a::new` directly instead of `stage_fingerprint`).
7. **Unused / one-caller.** `address_row` is `const fn` used at 1480-1501, 1631 and by a test (2180, 2190). `tagged_u64` used only inside the fingerprint (1701-1721).
8. **Stated limits.** Test name at 2075: "unversioned_component_remains_needed_without_inventing_a_gnu_version"; test at 2203 "finite_program_stack_requires_loader_binding_before_entry" (the `DF_BIND_NOW` row is mandatory, 1532-1535). The mandate is stated only in the test name.
9. **Fail-open.** None seen. `1517` omits version rows when there are no version needs; that is replayed against `verneed` itself, so an executable that advertises version rows without needs is refused (test 2183-2186, 2197-2200).
- Tests 1739-2218 (8 tests): corruption tables, import-permutation identity, component-only import shape (17 rows, 7 obligations, 2171-2176), mixed versioned/component imports (needed 3, verneed 2, 2099-2100).

## 3. `E/file_assembly.rs` lines 451-1377 (end of module `dynamic_file_assembly`, start of `dynamic_file_envelope`)

### 3a. 451-1249: assembling the final file bytes
1. **Purpose.** Final step of the pipeline: lays every fragment (ELF header prefix, source text, source data, 12 section payloads, section-header table) into one byte vector at its absolute file offset, zero-filling gaps, and replays that layout independently.
2. **Shape.** `seal_stage!` for `ValidatedElfAssembledDynamicFile` (454); `validate_contents` (464); `expected_fragments` (523, builds `Vec<Fragment>` capacity 16 then sorts by `(file_offset, fragment_kind_order)`, 589); `section_bytes` (593); `copy_fragment` (608); `validate_file_extents` (627); `expected_file_byte_count` (662); `fragment_range` (679); `load_layout` (697); `fragment_kind_order` (707); fingerprint (717); `hash_fragment_kind` (738). Types `Fragment`, `ElfDynamicFileFragmentKind`, `ElfDynamicFileFragmentPlacement`, `Candidate` are defined before 451 (not read).
3. **I/O.** Reads `ValidatedElfResolvedProcedureLinkage` (envelope, resolved text/data/PLT/GOT/relocation bytes), the placed load layout and indexed payload rows. Output is the replay verdict; the bytes come from `derive_contents` (before 451).
4. **How it decides.** `section_bytes` picks storage by literal section index: `8 => procedure_linkage_bytes`, `9 => procedure_got_bytes`, `10 => ..relocation`, `11 => general_relocation`, `12 => resolved dynamic table`, anything else uses the indexed row bytes (598-605). Index 0 must be Null with no bytes (559-567). Row/placement agreement is keyed on `row.index == placed.index() == position` and section kind (553-558). Overlap uses an `occupied: Vec<bool>` the size of the file (479, 502, 509). Padding must be zero where not occupied (512-519). Program-header file extents must fit the file (633-644); source text/data sizes must equal `image.text_size()` / `data_size()` (652-658).
5. **Cases by shape.** "exact fourteen-row section roster" (530-531, `SECTION_COUNT`). Section index to storage is the literal match at 598-605 (indices 8-12).
6. **Same job elsewhere.** Occupied-bitmap overlap at 479-510 and `copy_fragment` 608-625 repeat the same overlap test (suspected the pair is the derive/replay pair; `copy_fragment` itself is defined here, its caller is before 451). Pairwise overlap loop again at `E/dynamic_table.rs:710-729` and `file_assembly.rs:2339-2364`.
7. **Unused / one-caller.** `copy_fragment`: Grep `copy_fragment` not run in this pass (suspected called only from `derive_contents` before line 451).
8. **Stated limits.** None in comments.
9. **Fail-open.** None seen. The `unreachable!()` at 2714 is test-only.
- Tests 752-1248 (7): both-target assembly (17 fragments, 929), determinism, per-fragment byte flip rejection, truncation/append/ledger edits, fingerprint zero/drift, admitted-image consumption (1171-1218), final-byte admission drift (1221-1247). Types in tests: `ResolvedCustodySnapshot` (776).

### 3b. 1251-1377: head of `dynamic_file_envelope`
1. **Purpose.** Builds the ELF64 header plus five program headers (344 bytes) and carries the 14-row section-header table.
2. **Shape.** Constants 1268-1283 (`ELF64_PROGRAM_HEADER_COUNT = 5`, `ELF64_SECTION_HEADER_COUNT = 14`, `ET_EXEC`, `EM_X86_64 = 62`, `EM_AARCH64 = 183`). `ValidatedElfDynamicFileEnvelope` (1287); `ElfDynamicFileEnvelopeContents { header_prefix_bytes, entry_address, section_header_table_file_offset, section_header_table_bytes }` (1336); `DecodedElf64Header` (1350); `DecodedElf64ProgramHeader` (1367).
3-9. See 4 below (the validate function starts at 1453).

## 4. `E/file_assembly.rs` lines 1453-1636 (envelope replay)

1. **Purpose.** Replays the header prefix and section-header fragment against the placed layout.
2. **Shape.** `validate_contents` (1453), `validate_section_header_fragment_geometry` (1549), `decode_header` (1589), `decode_program_headers` (1611).
3. **I/O.** Reads `ValidatedElfResolvedDynamicTable`, `load_layout(resolved)`, calls `expected_entry_address(layout)`, `target_machine(layout.target())`, `section_name_table_index(layout)`, `segment_type` (defined in 1378-1452 and 1637-1706, not in my ranges). No mutation.
4. **How it decides.** Compares decoded header fields to literals: ident bytes `7f 45 4c 46`, class 2, data 1, version 1, OS ABI 0, ABI version 0, padding zero (1472-1483); `e_type == ET_EXEC`, machine from target, `e_flags == 0`, header/phdr/shdr sizes and counts (1484-1496). Entry must equal the entry derived from the retained text entry symbol and the header's `e_entry` (1463-1467). Each decoded program header is compared as a whole struct with its planned row (1505-1520). Read-only load must start at file offset 0 and contain the whole 344-byte prefix (1522-1531). Section-header fragment must be `14*64` bytes, equal the placed table bytes, start at or after the prefix, and lie after the end of every `LoadReadOnly/ReadExecute/ReadWrite` file extent (1563-1585).
5. **Cases by shape.** Exactly five program headers (1500-1503); table size from `ELF64_SECTION_HEADER_COUNT` 14 (1533).
6. **Same job elsewhere.** ELF header field decode/compare duplicates parts of the PT_LOAD congruence checks in `E/load_placement.rs:40-111` (read).
7. **Unused.** None examined.
8. **Stated limits.** Test-only: "non_runnable envelopes" (1870).
9. **Fail-open.** None seen. `.ok_or_else(... "no read-only load")` at 1526 errors when absent.
- Tests 1731-2086 (7). Envelope entry fixtures include `Missing`, `Data`, wrong-kind, zero-size, out-of-range text entry symbols (1928-1946), all expected to reject.

## 5. `E/file_assembly.rs` lines 1707-2088 (end of envelope: fingerprint + tests)
Fingerprint 1716-1729 (`stage_fingerprint` seed `b"omega.elf.dynamic-file-envelope.v1"`); `segment_type` tail 1707-1714 maps `Interpreter -> PT_INTERP`, three `Load*` kinds -> `PT_LOAD`, `Dynamic -> PT_DYNAMIC`. Tests as above (section 4). `overwrite_u64` uses `::wire::codec_bytes::patch_encoded` (1866).

## 6. `E/file_assembly.rs` lines 2209-2461 (resolved procedure linkage: replay and fingerprint)
1. **Purpose.** Replays the application of every PLT/GOT/relocation fixup onto copies of the relocated text/data/PLT/GOT/relocation bytes.
2. **Shape.** `validate_contents` (2217), `validate_nonoverlapping_applications` (2339, `pub(crate)`), `validate_unchanged_bytes` (2366), fingerprint module `compatibility_fingerprint` (2394-2460) with `hash_target` (2430).
3. **I/O.** Reads `ValidatedElfDynamicFileEnvelope`, `relocate_image_local_memory(load_layout(envelope))` (2222, defined in `image_local_relocations`, not read), `payloads.procedure_fixups`, `contents.applications`. Calls `storage_address`, `semantic_target_address`, `encode_field`, `decode_rejoins_target`, `read_field` (read_field defined outside my ranges).
4. **How it decides.** Per fixup, by ordinal position: application fields must equal the indexed fixup (2265-2274); source/target address must equal recomputed absolute addresses (2276-2282); the original field masked by `mutable_mask` must be 0 (2289-2292); `encoded_field` and the actual stored field must equal `encode_field(...)` (2293-2309); bits outside the mask must be unchanged (2310-2314); the encoded field must decode back to the target (2315-2320). Then every byte outside any application's `[offset, offset+width)` must equal its upstream byte, per storage (2376-2389, quadratic: `applications.iter().any` per byte, using `saturating_add`, 2383).
5. **Cases by shape.** Four storages iterated (2323-2328: SourceText, ProcedureLinkage, ProcedureGot, ProcedureRelocation). `GeneralRelocation` storage is not in that loop even though it has applications (see 2338-2346 test at 3735 and `storage_fields` 3346). Inferred: the general-relocation bytes are not byte-compared against upstream here; `.rela.dyn` r_offset fields are checked only by the per-application loop (2259-2321). I did not read the first 450+ lines of this module to confirm.
6. **Same job elsewhere.** Same per-byte "unowned byte" test as the fragment gap check at 512-519.
7. **Unused.** `validate_nonoverlapping_applications` called by test at 3945 and `validate_contents` (2257).
8. **Stated limits.** None.
9. **Fail-open.** One to note, **open**: `GeneralRelocation` is absent from the unchanged-bytes loop (2323-2328), so bytes of `.rela.dyn` outside any application field are not compared to upstream here. Whether another check covers them is not traced.
- Fingerprint hashes all five byte vectors and each application (2409-2426).

## 7. `E/file_assembly.rs` lines 2662-3948

### 7a. 2662-2831: tail of callback test module
Test helpers (`file_offset_of` scans PT_LOAD, panics if unbacked, 2673-2691; `materialized_callback_address` decodes x86 RIP-relative `lea` or AArch64 `adrp+add`, 2693-2716) and two tests: dynamic image resolves callback address and data pointer to the private function (2719), and substituted callback displacement/data pointer reject on replay (2770, messages "unowned byte" and "source data").

### 7b. 2833-2997: module `linkage_fixups`
1. **Purpose.** Public types for applied fixups. `ElfAppliedProcedureLinkageStorage` (5 variants, 2840), `ElfAppliedProcedureLinkageKind` (7 variants, 2850), `ElfAppliedProcedureLinkageTarget` (7 variants, 2861), `ElfAppliedProcedureLinkageFixup` (2883, 10 fields), `ValidatedElfResolvedProcedureLinkage` (2936, fields `envelope`, `contents`, fingerprint), `ElfProcedureLinkageApplicationError` (2988).
2. **Notes.** Several accessors are `#[cfg(test)]` (2897, 2906, 2915, 2928); `ordinal`, `byte_offset`, `kind`, `encoded_field` exist only in tests.

### 7c. 2999-3519: module `storage_fields`
1. **Purpose.** Computes source and target addresses of each fixup from the placed layout, encodes and decodes each fixup kind, maps storage names, and writes fields.
2. **Shape.** `storage_address` (3022), `semantic_target_address` (3051), `procedure_linkage_header_size` (3166), `exact_section` (3178), `encode_field` (3191), `decode_rejoins_target` (3260), `sign_extend` (3310), `checked_add_signed` (3315), `upstream_storage_bytes` (3327), `indexed_row_bytes` (3352), `storage_bytes` (3365), `storage_bytes_mut` (3384), `write_field` (3405), `public_storage` (3434), `public_kind` (3461, identity: `ElfAppliedProcedureLinkageKind` is the same type as `ElfProcedureLinkageFixupKind`, return `kind`), `public_target` (3467), `public_section_kind` (3499, identity), `load_layout` (3505), `indexed_payloads` (3514).
3. **I/O.** Reads `ValidatedElfDynamicLoadLayout`, `ElfIndexedProcedureFixup`. Pure address arithmetic with `checked_sum_u64`/`checked_product_u64`.
4. **How it decides.**
   - Target address by semantic target (3051-3164): `FutureDynamicSection -> (DynamicTable, 0)`, `PltHeader -> (ProcedureLinkage, 0)`, `PltEntry -> header_size + ordinal*16`, `PltLazyTail -> header_size + ordinal*16 + 6`, `GotPltHeaderWord -> index*8`, `GotPltSlot -> (3 + ordinal)*8` (3084-3142). The target section is chosen by section index, then its kind is required to equal the expected kind and offset+1 <= byte_size (3147-3155).
   - `encode_field` per kind (3198-3252): x86 rel32 range-checked to i32 (3206-3215); AArch64 ADRP page delta in +-2^20 pages (3220-3226); Load64Low12 requires 8-byte-aligned page offset (3229-3234); AddLow12 is `(target & 0xfff) << 10` with no range need (3236); Branch26 requires `delta % 4 == 0` and 26-bit signed range (3238-3248); Absolute64 and `Elf64RelaOffset` are the target address (3250-3251). Result must fit the mutable mask (3253-3256).
   - `procedure_linkage_header_size` keys on `layout.target()`: LinuxX64 -> 16, LinuxArm64 -> 32, else Err (3169-3175).
   - `write_field` accepts width 4 or 8 only (3416-3429).
5. **Cases by shape.** Constants 3532-3542: `X86_PLT_HEADER_SIZE = 16`, `AARCH64_PLT_HEADER_SIZE = 32`, `PROCEDURE_LINKAGE_ENTRY_SIZE = 16`, `X86_PLT_LAZY_TAIL_OFFSET = 6`, `GOT_PLT_HEADER_WORDS = 3`. `PltLazyTail` offset adds the x86 constant 6 even if the target is AArch64 (3101-3119); `procedure_linkage_header_size` is target-aware but the lazy-tail constant is not (inferred; the lazy-tail target presumably only arises on x86).
6. **Same job elsewhere.** x86 rel32 and AArch64 ADRP/LDR/ADD/B encodings also exist in the stage-11 encoders (`isa-x86_64`, `isa-aarch64`) and in the stage-15 Mach-O/PE images (suspected; not read). Here they are re-encoded for PLT fixups. `public_kind` / `public_section_kind` are identity functions with the same job as type aliases.
7. **Unused.** `public_kind` and `public_section_kind` return their argument unchanged (3461-3465, 3499-3503), used at 2272 and 556/3033/444; not counted by grep.
8. **Stated limits.** `unreachable!("image-section targets resolve through image memory placement")` at 3144 states the invariant for the `RelocatedImageSection` arm (handled before at 3055-3078).
9. **Fail-open.** None seen. `.get(index).filter(...)` returns an error on absence (3182-3188).

### 7d. 3521-3555: `apply_elf_procedure_linkage_fixups`
Entry (3544) using `plan_stage!` with `derive_contents` and the module-level fingerprint. Imports at 3521-3529; the nonzero `seal_stage!` is at ~2209.

### 7e. 3557-3947: tests (7)
Standard 64-byte text fixture with two versioned imports and three call relocations (3590-3649). Expected fixup counts: x86-64 14, AArch64 16 (3689-3690). Data-slot import relocations emit a 24-byte `.rela.dyn` row with `r_info = (1<<32) | type` where x86-64 type = 1 (`R_X86_64_64`) and AArch64 type = 257 (3736-3739, 3753). Fail-closed helpers: range/alignment/mask/overlap tests (3880-3946).

## 8. `E/import_sections.rs` lines 1806-2144 (tests of the section-descriptor stage)
Tests only. Seven descriptors expected: name offsets `[1, 9, 17, 25, 31, 44, 69]`, payload sizes `[interp, 43, 96, 36, 8, 80, 40]`, seed length 79 (1887-1912); exact rows for Interpreter (`SHT_PROGBITS`), DynamicSymbol (`SHT_DYNSYM`, align 8, entry 24, link DynamicString, info 1), SystemVHash, GnuSymbolVersion, GnuVersionRequirement (info 2), GnuHash (1915-1997). Corruption table of 17 mutations (2084-2113). These tests pin the constants the non-test code in `import_sections.rs` (not read) must produce: interpreter, `.dynstr`, `.dynsym`, `.hash`, `.gnu.version`, `.gnu.version_r`, `.gnu.hash`.

## 9. `E/import_sections.rs` lines 2936-3294 (tests of the dynamic-section contents stage)
Tests only. They pin: interpreter bytes + NUL (2994-2996), `.dynstr` layout `\0OMEGA_1\xfd\0OMEGA_2\0alpha\xfe\0beta\0gamma\0libalpha\xff.so.1\0libbeta.so.2\0` (3000-3002), dynsym names `[0,18,18,25,30]`, GNU hash single bucket, bloom shift 5, bloom word `0x0000_0102_0090_2200` and chains `[0xf204f288, 0xf204f288, 0x7c9489a0, 0x0f7deae9]` (3028-3037), `needed [36, 51]`, verneed count 2. Known GNU-hash vectors for `""`, `exit`, `printf`, `memcpy` (3211-3214). 26-item corruption table (3140-3177). Pure-component imports yield `dynstr [0]`, one null symbol, empty chains (3193-3207). Names are raw bytes (including non-UTF-8 `\xff`).

## 10. `E/load_placement.rs` lines 1-470 (stage 16-18: load layout; module `load_layout::abi_validation`)
1. **Purpose.** Header comment: plans virtual segment and section placements (`plan_elf_dynamic_load_layout`), patches placed offsets into the section-header table (`apply_elf_section_header_placements`), resolves addresses in `.dynamic` (`apply_elf_dynamic_address_fixups`) (1-7). Lines 1-470 are the independent ABI replay `validate_abi` and the start of `validate_deferred_constraint_envelope`; the planning code is after 470.
2. **Shape.** `validate_abi(candidate: &Candidate) -> Result<(), Diagnostic>` (40); `range_contains` (413); `validate_deferred_constraint_envelope` (425, cut off at 470); macro `s!` (31) shorthand for `checked_sum_u64`.
3. **I/O.** Reads a `Candidate` (program_headers, sections, image_memory, `section_header_table_file_offset`, `section_header_resolutions`, `relative` payload layout, `max_page_alignment`, `image_base`). Mutates nothing.
4. **How it decides.** All keyed on position and exact numbers:
   - Program headers must be exactly 5 in the order Interpreter, ReadOnly, ReadExecute, ReadWrite, Dynamic (41-56).
   - PT_LOAD rows are indexes 1..=3 (57, 87-95, 115-176): alignment equals `max_page_alignment`, a power of two, file offset congruent with virtual address modulo alignment (58-64); readable, never writable+executable (65-68); flags exactly R, RX, RW (74-79); physical == virtual (80-86); first load at file offset 0, vaddr = `image_base`, size >= 64 + 56*5 (87-94); loads ordered and non-overlapping in both file and memory (95-114).
   - Per section: Null section canonical (116-126); otherwise has a region (127-131), power-of-two alignment, file offset aligned, address aligned (132-140); `FileOnly` has no address (141-147); mapped sections must satisfy `virtual_address == image_base + file_offset` and fit inside the load of their region, with `load_index` chosen from `ReadOnly -> 1`, `ReadExecute -> 2`, `ReadWrite -> 3` (148-176). `.shstrtab` must be file-only (178-186).
   - Source text sits inside the RX load at `image_base + file_offset`; data/BSS inside the RW load (189-253); alignments use `retained_image(..).target.pointer_alignment` (254-274). Source text must end before the first RX section start and the last RW section must end before data (275-299). `.shstrtab` after the RW load and the section header table after `.shstrtab` (300-305).
   - Section-header placement fixups must match one-for-one by position with resolutions; the value is the section's virtual address or file offset (306-347).
   - PT_INTERP and PT_DYNAMIC must alias their sections exactly (356-367), with PT_INTERP flags R, align 1, inside the read-only load (368-389), and PT_DYNAMIC flags RW, align equal to `.dynamic` alignment, inside the RW load (390-409).
   - Deferred constraint envelope: for each `procedure_constraints` entry, finds the fixup by `upstream_ordinal`, computes source and target spans, and applies the range limit by constraint kind (`X86Signed32` => span <= `i32::MAX`, 468-470; remaining arms are after 470, not read).
5. **Cases by shape.** `candidate.program_headers[1..=3]`, `[0]`, `[4]` literal indexing (57, 75-77, 187-188, 357-358); `PLACEMENT_FIXUP_COUNT` constant row count (307). The list of three load kinds and the single dynamic segment is fixed.
6. **Same job elsewhere.** Header-field checks overlap with `E/file_assembly.rs:1484-1531` (header replay), and extent containment with `validate_file_extents` `E/file_assembly.rs:627-660`. The derive side of the layout is after line 470 (suspected same file, not read).
7. **Unused.** `range_contains` has callers at 374, 381, 394, 401 in the range. `validate_abi` is `pub(crate)`; Grep `fn validate_abi` shows one definition (`E/load_placement.rs:40`); callers not counted.
8. **Stated limits.** Messages state policy: "strict readable W^X policy" (66-67). `"executable section roster is empty"` (283) and `"writable section roster is empty"` (294) refuse an image with no executable or no writable section.
9. **Fail-open.** None seen in 1-470. Note `placed.virtual_address.unwrap_or(0)` at 362: if `.interp` or `.dynamic` had no address, the program header's virtual address is compared to 0 (a program header with address 0 would pass); traced: `shstrtab` is the only file-only section required (182-186), and PT_INTERP/PT_DYNAMIC alignment require `region` mapped at 148-176 only for non-file-only sections, so a file-only `.interp` would be caught only by the header-vs-section comparison. **open**.

## Coverage
Read in full (all assigned ranges), per the unit file `units/gap_007.tsv`:
- `E/dynamic_table.rs` 1-1017 (1017 lines) and 1438-2219 (782 lines).
- `E/file_assembly.rs` 451-1377 (927), 1453-1636 (184), 1707-2088 (382), 2209-2461 (253), 2662-3948 (1287).
- `E/import_sections.rs` 1806-2144 (339) and 2936-3294 (359).
- `E/load_placement.rs` 1-470 (470).
Total read in full: 6000 lines. Skimmed: 0. Not read: none of my assigned ranges. (Lines outside these ranges, such as `dynamic_table.rs` 1018-1437 and `file_assembly.rs` 1-450, 1378-1452, 1637-1706, 2089-2208, 2462-2661, belong to other readers; statements above that depend on them are marked inferred.)
Commands run: Grep count over `source-rust` for the stage entry names (95 occurrences / 6 files); Grep `fn (validate_abi|encode_field|decode_rejoins_target|checked_sum_u64)` (one definition each); Grep for `fn read_u64|read_u32|...` in `15_objects-to-image` (shared helpers in `E/dynamic_executable.rs:17-84`, `E/lib.rs:50,65`).

## Five things a maintainer most needs to know
1. Every stage here is derive-then-replay: the replay code recomputes expected values independently from the previous owner and compares byte for byte, with the rejected input handed back (`image::rejection_carrier!`). There is no tolerant path in these ranges.
2. Fixed ELF shape is hard-coded: 5 program headers in one order, 14 sections, 11 base descriptors, a 113-byte name seed, `.dynamic` name offset 113 (`E/dynamic_table.rs:22`, `E/file_assembly.rs:1271-1275`, `E/load_placement.rs:41-47`). Adding a section means changing literals and tests together.
3. Procedure-linkage addresses are computed by literal section indices 8-12 in `section_bytes` (`E/file_assembly.rs:598-605`) and `upstream_storage_bytes` (`:3333-3349`), and PLT sizes are per target constants (`:3532-3542`); only LinuxX64 and LinuxArm64 are accepted (`:3169-3175`, `E/dynamic_table.rs:1673-1681`).
4. The unchanged-bytes replay in resolved linkage skips the `GeneralRelocation` storage (`E/file_assembly.rs:2323-2328`); other stages' coverage of `.rela.dyn` bytes was not traced.
5. Fingerprints are labelled `non_authoritative` and are only replay-checked (a zero or drifted fingerprint rejects, tests at `E/file_assembly.rs:1152-1168`, `:2077-2085`), but `file_assembly.rs:721` hashes with `Fnv1a::new()` while others use `checked::stage_fingerprint`.

## Unsure about
- Whether `.rela.dyn` bytes outside application fields are verified elsewhere (point 4).
- The definitions of `plan_stage!`, `seal_stage!`, `Candidate`, `derive_contents` for the assembled-file stage and `ElfDynamic*` semantic types lie outside my ranges; claims about them are inferred from use.
- The remaining arms of the deferred-constraint match after `load_placement.rs:470`.
- Whether `PltLazyTail`'s fixed offset 6 (`E/file_assembly.rs:3115`) is reachable on AArch64.
