# Record: unit 020, second pass (gap_008), area 11-native-stages-11-15

Files: `source-rust/subsystems/compiler/psi-to-native/pipeline/15_objects-to-image/image-elf/src/load_placement.rs`
(`LP`) and `.../procedure_linkage.rs` (`PL`). Paths below are relative to `image-elf/src/`. Only the four gap
ranges were read; the rest of each file belongs to the first reader of unit 020, so facts about code outside
the ranges (the `derive_contents`/`validate_abi` bodies in LP before 471 and 586-985, PL 1-580 and 1955-2434)
are not recorded here. Several ranges start or end mid-module; the module nesting below is read from the
module headers inside the ranges and from `grep`.

## LP 471-585: tail of `abi_validation` (Linux load-layout ABI replay)

1. **Purpose.** The end of the independent replay that checks a computed ELF load layout against ABI rules:
   the last arms of the deferred placement-constraint check for procedure-linkage fixups, plus small helpers
   that reach retained upstream data and compute absolute placements.
2. **Shape.** `validate_deferred_constraint_envelope` body tail (LP:471-490, function head is before the
   range); `aarch64_page_delta_covers_extent` (LP:492); `retained_target` (LP:512) and `retained_image`
   (LP:530), each a 10-call accessor chain from `ValidatedElfRelativeSectionPayloadLayout` down to the
   interpreter plan's target or the image; `section` (LP:547); `align_for_region` (LP:557);
   `placed_coordinates` (LP:573).
3. **Inputs and outputs.** Reads the placed sections, constraint rows and the target extent; returns
   `Ok(())`/`Err(Diagnostic)`, a bool, a `TargetProfile`, a `&FinalImage`, or `(file_offset, Option<vaddr>)`.
   Nothing mutates.
4. **How it decides.** Keyed on the constraint kind enum (read, LP:471-487): X86 signed-32 span check,
   AArch64 branch-26 needs `source % 4 == 0 && span < 1<<27` (LP:474), page-delta-21 needs every byte of the
   target within a signed 21-bit 4-KiB page delta of the source (LP:492-509, `delta in -(1<<20)..=(1<<20)-1`),
   low-12 load needs target alignment >= 8 and 8-aligned start (LP:483). `section` (LP:547-554) accepts a slot
   only when `sections[kind as usize]` has the same kind and index, so position equals kind discriminant.
   `align_for_region` (LP:562-570) takes the maximum alignment over rows whose `region == Some(region)`,
   defaulting to 1. `placed_coordinates` gives a virtual address only when `allocated` (LP:580).
5. **Cases by shape.** Four constraint kinds only (LP:471-487). A zero-size target returns `false` for the
   page-delta check (LP:497-499), so a zero-length target never satisfies an ADRP constraint.
6. **Same job elsewhere.** `checked_align` is defined twice: LP:1061 (rejects non-power-of-two, rounds up via
   `checked_sum_u64`) and `section_headers.rs:289` (accepts any alignment, uses `% alignment`, `max(1)`)
   (grepped `fn checked_align`: 4 hits including tests; read both bodies). The two disagree on
   non-power-of-two alignments.
7. **Unused or one-caller code.** `retained_target` 20 grep hits over `source-rust/` (includes tests and
   definition); `retained_image` 32; `aarch64_page_delta_covers_extent` 8 (definition, one call at LP:478,
   test imports and uses). Command: `grep -rn "<name>" --include=*.rs source-rust | wc -l`.
8. **Stated limits.** None in this range.
9. **Fail-open risks.** None seen. `align_for_region` returning alignment 1 when no row matches the region
   (LP:569) is "open": a region with no rows silently gets no alignment, but no wrong program reaches it
   that I traced (callers are outside the range).

## LP 986-1294: `compatibility_fingerprint` tail, `load_headers`

1. **Purpose.** Hashes every part of the computed load layout into one `u64`, rounds addresses up to an
   alignment with overflow checks, and defines the public row types for program headers, placed sections,
   section-header resolutions and image-memory placement.
2. **Shape.** `non_authoritative_layout_compatibility_fingerprint` (LP:987); `target_tag` (LP:1047);
   `checked_align` (LP:1061). `load_headers` module (LP:1079): `ElfLoadProgramHeaderKind` (Interpreter,
   LoadReadOnly, LoadReadExecute, LoadReadWrite, Dynamic; LP:1085), `ElfPlacedDynamicSectionKind` (14
   variants Null..SectionNameTable; LP:1095), `public_section_kind` (LP:1112, identity function on a type that
   must be the same type as the roster kind), `ElfLoadProgramHeader` (LP:1119), `ElfPlacedDynamicSection`
   (LP:1165), `ElfSectionPlacementResolutionKind` (VirtualAddress/FileOffset; LP:1203),
   `ElfResolvedSectionHeaderPlacement` (LP:1209), `ElfLoadImageMemoryPlacement` (LP:1219). The rest is
   trivial getters.
3. **Inputs and outputs.** The fingerprint reads the relative layout's own fingerprint, target, image base,
   max page alignment, headers, image memory, sections, resolutions and header-table offset
   (LP:987-997); returns a `u64` through `stage_fingerprint` plus byte/`bytes` updates (read).
4. **How it decides.** `target_tag` keys on the `TargetProfile` variant (LP:1047-1058): nine variants mapped
   to 1..9, including targets ELF never uses (`UefiX64`, `MacosArm64`, ...). `checked_align` returns the value
   unchanged for alignment <= 1 (LP:1066), rejects non-powers of two (LP:1069), then `(v + mask) & !mask`.
5. **Cases by shape.** Fixed five-header layout (Interpreter, RO, RX, RW, Dynamic) and fixed 14-section roster
   expressed by enum discriminants (LP:1085-1110). `DYNAMIC_LOAD_POLICY_TAG` is hashed as a constant (LP:1003).
6. **Same job elsewhere.** `ElfPlacedDynamicSectionKind` (LP:1095) and the roster's `ElfDynamicRosterSectionKind`
   describe the same 14 kinds; `public_section_kind` returns its argument unchanged (LP:1112-1116), so they are
   one type under two names (inferred from the identity body; grep `public_section_kind`: 16 hits).
   The FNV hashing recipe repeats in every stage of this file (LP:998, 2222, 3076) and in PL (2477).
7. **Unused or one-caller code.** `target_tag` here has one definition and one call (LP:1002, grep `fn target_tag`
   gives two definitions, the second a different function at LP:3054). `ElfLoadProgramHeader::physical_address`
   etc. are getters used by tests (not counted).
8. **Stated limits.** None.
9. **Fail-open risks.** None seen. The fingerprint is a compatibility check, named `non_authoritative`;
   tests only compare it for equality with a replayed value (LP:1528 area), so it detects drift, not tamper.

## LP 1296-1661: `load_layout` tests

Tests of the load layout (`#[cfg(test)] mod tests`, LP:1296). One short entry for the group: nine `#[test]`
functions on LinuxX64 and LinuxArm64 (LP:1374-1660), mutating a `Candidate` and expecting
`validate_candidate`/`validate_abi` to reject, plus exact numbers: text size 32, data 13, bss 23, bss
alignment 16 (LP:1412-1415), 25 resolutions (LP:1458), 14*64-byte header table (LP:1451), page max 0x10000
(LP:1380). Test shape: "every corruption in a vector of closures must reject and the error must return
the same upstream fingerprint" (LP:1502-1538). Only these two targets are tested.

## LP 1663-1858: `validated_layout`, re-exports, constants, `plan_elf_dynamic_load_layout`

1. **Purpose.** The sealed result type for the load layout, its error type that gives back the upstream plan,
   and the entry function.
2. **Shape.** `ValidatedElfDynamicLoadLayout` (LP:1677, fields `relative`, `target`, `image_base`,
   `max_page_alignment`, `program_headers`, `image_memory`, `sections`, `section_header_table_file_offset`,
   `section_header_resolutions`, fingerprint); `ElfDynamicLoadLayoutError` (LP:1762, carries the relative
   layout and a `Diagnostic`); `plan_elf_dynamic_load_layout` (LP:1807). Constants: `PLACEMENT_FIXUP_COUNT = 25`
   (LP:1791), `DYNAMIC_PROGRAM_HEADER_COUNT = 5` (LP:1793), `DYNAMIC_LOAD_POLICY_TAG = 1`,
   `AARCH64_RELOCATION_PAGE_SIZE = 0x1000`, `PF_X/PF_W/PF_R = 1/2/4`.
3. **Inputs and outputs.** Takes the relative payload layout by value; on derive failure returns it inside the
   error (LP:1817-1825); otherwise builds a `Candidate`, calls `validate_candidate` (LP:1852) and returns the
   sealed layout or the error with `relative` handed back.
4. **How it decides.** Image base and max page alignment come from constants `IMAGE_BASE`, `ELF_MAX_PAGE_SIZE`
   (LP:1826-1827; values 0x400000 and 0x10000 in `lib.rs:85,90`). Target comes from the retained interpreter
   plan (LP:1810). The accept/reject judgment is inside `validate_candidate` (outside this range).
5. **Cases by shape.** `final_image_layout()` exposes text, data and bss addresses only (LP:1715-1721).
6. **Same job elsewhere.** The `plan_x` / `Candidate` / `seal_stage!` / `rejection_carrier!` pattern repeats
   at LP:1948, 2701, PL 3123 and `checked::plan_stage!` macro (grepped; the macro is defined in
   `dynamic_executable.rs`, not read).
7. **Unused or one-caller code.** `into_relative` has 2 grep hits (definition and one caller);
   `section_header_resolutions` 22 hits.
8. **Stated limits.** None.
9. **Fail-open risks.** None seen.

## LP 1861-2237: `placed_section_headers`

1. **Purpose.** Copies the 14-row ELF section-header template and writes the resolved file offsets and
   virtual addresses into 25 zero-filled 8-byte fields, then replays the result against the roster.
2. **Shape.** `ValidatedElfPlacedSectionHeaderTable` (LP:1886), `ElfSectionHeaderPlacementApplicationError`
   (LP:1922), `ElfPlacedSectionHeaderContents { bytes, applications }` (LP:1933), `apply_elf_section_header_placements`
   (LP:1944), `derive_contents` (LP:1957), `validate_contents` (LP:2024), `validate_decoded_row` (LP:2168),
   `validate_application_coverage` (LP:2189), fingerprint (LP:2218).
3. **Inputs and outputs.** Reads the template bytes, `placement_fixups` and the load layout's resolutions;
   produces 896-byte table (test asserts 896, LP:2286) and an `applications` ledger of 25 rows.
4. **How it decides.** Position and shape, not identity: each application must match its fixup on row index,
   kind, offset, width (LP:1983-1990), width must be 8 (LP:1991), the target field must be all zero
   (LP:2001); in validation, the field offset must equal `row*64 + 16` (address) or `row*64 + 24` (file offset)
   (LP:2082-2098); row 0 and the section-name table (row 13) have no address, row 13 has an offset (LP:2147-2153);
   decoded rows are compared field by field with the roster (LP:2174-2184).
5. **Cases by shape.** Hard counts: 25 placements, 13 file offsets, 12 virtual addresses, 14 rows
   (LP:1878-1881, 2041-2128). The `SECTION_COUNT * ELF64_SECTION_HEADER_SIZE` size is called "exact 832-byte"
   in the message (LP:1966) while the test asserts 896 (LP:2286); 14*64 = 896. The 832 and the messages
   "exactly twelve file offsets" (LP:2117) and "exactly eleven virtual addresses" (LP:2128) disagree with the
   constants 13 and 12 (LP:1878-1879). Messages only; the checks use the constants.
6. **Same job elsewhere.** Same derive/seal/validate pattern as `resolved_dynamic_table` (LP:2509) and the
   template stage in PL; overlap check in `validate_application_coverage` (LP:2189-2216) is the same
   quadratic check as `validate_fixup_coverage` in PL:3035-3060.
7. **Unused or one-caller code.** `ElfAppliedSectionHeaderPlacement` is a type alias of
   `ElfResolvedSectionHeaderPlacement` (LP:1882). `applied_placements` is test-only (LP:1901).
8. **Stated limits.** None.
9. **Fail-open risks.** `address_by_row[ordinal].unwrap_or(0)` and `offset_by_row[..].unwrap_or(0)`
   (LP:2158-2159): a row with no ledger entry is required to have address 0 and offset 0 in the decoded
   header. Stated by the checks at LP:2147-2153 only for rows 0 and 13; for other rows 14 rows x 2 fields = 25
   placements means 3 fields are absent (row 0 twice, row 13 address), so no other row can be absent once the
   counts at LP:2108-2129 pass. Classification: traced.

## LP 2239-2506: tests for `placed_section_headers`

Group entry. Seven tests (LP:2281-2505) on LinuxX64 and LinuxArm64: exact 25 little-endian writes
(LP:2283-2321), unchanged bytes copied (LP:2325), 13/12 split and file-only section name table (LP:2349-2404),
determinism, eight corruption closures (LP:2428-2444), byte-level corruption and truncation, arithmetic helper
failures.

## LP 2509-3094: `resolved_dynamic_table`

1. **Purpose.** Writes the virtual address of each referenced section into the 8-byte value of the matching
   `Elf64_Dyn` rows of `.dynamic`, and replays the result against the semantic tag plan.
2. **Shape.** `ElfDynamicAddressApplicationTarget` (nine variants, LP:2566), `ElfAppliedDynamicAddress`
   (LP:2579), `ValidatedElfResolvedDynamicTable` (LP:2640), `apply_elf_dynamic_address_fixups` (LP:2698),
   `derive_contents` (LP:2710), `application_for_fixup` (LP:2752), `validate_contents` (LP:2808), helpers
   `target_section_index` (LP:3008), `target_section_kind`, `target_tag` (LP:3054), fingerprint (LP:3072).
   `DYNAMIC_SECTION_INDEX = 12` (LP:2534). `EXPECTED_TARGETS` (LP:2535) has nine entries.
3. **Inputs and outputs.** Reads the indexed payloads' `dynamic_fixups`, the dynamic-table payload's
   serialized fixups and semantic rows, and the load layout's section placements; produces patched `.dynamic`
   bytes and the application ledger.
4. **How it decides.** Keyed on the fixed target enum, not on names. The expected order is a hard-coded list
   (GOT, SysV hash, GNU hash, strings, symbols, procedure relocations, general relocations, then version
   symbol and version requirement, LP:2535-2545); the last two are dropped when
   `has_version_requirements()` is false (LP:2550-2554). Each target maps to a fixed section index
   (LP:3008-3019); `application_for_fixup` needs the section at that index to have the same index, same kind
   and a virtual address (LP:2772-2782). Value read from the placed section (LP:2792).
5. **Cases by shape.** Fixed order of nine (or seven) targets (LP:2535, 2553); comment-free but enforced by
   "indexed ELF dynamic address targets are not in canonical order" (LP:2731). Only one fixup kind exists,
   `Elf64AbsoluteAddress` (LP:2560), whose enum is `#[cfg(test)]` only; the checks compare `kind_tag` to the
   upstream kind (LP:2890).
6. **Same job elsewhere.** Two tables of fixed section index per kind: `target_section_index` here (LP:3008)
   and the 14-kind enum discriminants (LP:1095-1110), plus `section_headers.rs:1920` calls into it. The same
   mapping exists three times: the enum values, `target_section_index`, and `target_section_kind`
   (read; kept consistent by `application_for_fixup` checking index and kind, LP:2775-2777).
7. **Unused or one-caller code.** `public_target` has 7 hits (identity function, LP:3004, like
   `public_section_kind`); `target_section_index` is called at LP:2767 and `section_headers.rs:1920`.
8. **Stated limits.** None.
9. **Fail-open risks.** `expected_targets` (LP:2547-2555) decides the version-aware count from
   `has_version_requirements()` of the semantic plan, and `validate_contents` checks it against
   `address_obligations` too (LP:2818-2823), so a plan that lies on both sides is not caught here. Classification:
   open (depends on upstream plan, not traced). `.expect("filtered allocated ELF dynamic target")` at LP:2794
   cannot fail after the filter at LP:2778.

## LP 3096-3378: tests for `resolved_dynamic_table`

Group entry. Seven tests (LP:3154-3377): nine addresses for both targets, 72 mutable bytes
(LP:3201-3221, 9 x 8), determinism and sensitivity to target and imported symbol name (LP:3224-3250), fourteen
ledger corruptions (LP:3254-3286), sibling/file-only/null target substitution (LP:3306-3333), byte-level
tag, endian and truncation corruption (LP:3336-3366), arithmetic helpers.

## PL 581-1127: tests of `dynamic_import_relocations` (procedure-linkage relocation plan)

Group entry; I did not read the module's production code, which sits before line 581. Tests build a
`FinalImage` with 64 text bytes, insert import fixtures and call relocations (PL:600-658), a general-relocation
variant that adds 48 data bytes and `Absolute64` relocations (PL:660-690), and a `descriptors_from_image`
pipeline helper (PL:692-707). Cases: both Linux targets give 3 slots, 3 procedure relocations, 4 call sites
(PL:733-790); permuting imports gives the same fingerprint and shares a slot among calls (PL:793-830);
seven image mutations and a misaligned AArch64 BL reject (PL:833-912); 26 closure corruptions of the candidate
(PL:916-974); overflow guards (PL:996-1025); general relocation rows (PL:1028-1067); five malformed general
shapes and an overlap (PL:1070-1125). The tests only use `LinuxX64` and `LinuxArm64` (`_ => unreachable!`).

## PL 1129-1953: `dynamic_linkage_descriptors` (+ its tests)

1. **Purpose.** Appends four section descriptors (`.plt`, `.got.plt`, `.rela.plt`, `.rela.dyn`) and their
   names to the 7-row dynamic section descriptor set and the section-name seed.
2. **Shape.** `ValidatedElfProcedureLinkageSectionDescriptorPlan` (PL:1151), `ElfProcedureLinkageSectionKind`
   (PL:1217), `...Link` (PL:1237), `...Info` (PL:1243), `ElfProcedureLinkageSectionDescriptor` (PL:1249),
   `plan_elf_procedure_linkage_section_descriptors` (PL:1267), `derive_contents` (PL:1282), `descriptor`
   (PL:1394), `validate_contents` (PL:1439), `validate_name` (PL:1548), fingerprint (PL:1560).
   Constants: `UPSTREAM_DESCRIPTOR_COUNT = 7`, `APPENDED_DESCRIPTOR_COUNT = 4`,
   `PROCEDURE_LINKAGE_NAME_SUFFIX = ".plt\0.got.plt\0.rela.plt\0.rela.dyn\0"` (PL:1146-1148).
3. **Inputs and outputs.** Reads the template plan's payload byte lengths and base name seed; outputs an
   extended seed (append-only, PL:1454-1460) and four rows.
4. **How it decides.** Keyed on target: `.plt` flags are `ALLOC|EXECINSTR` for `LinuxX64` and add
   `SHF_AARCH64_PURECODE` for `LinuxArm64`, any other `TargetProfile` returns an error (PL:1329-1337,
   1486-1494). Name offsets come from the seed length plus fixed name lengths (PL:1286-1319); `validate_name`
   is `require_name` against the kind's name bytes (PL:1552).
5. **Cases by shape.** Exactly seven base rows and four appended, in fixed order `.plt`, `.got.plt`,
   `.rela.plt`, `.rela.dyn` (PL:1462-1479). `.rela.plt` links to the dynamic symbol table and info points at
   `.got.plt`; `.rela.dyn` has link but no info (PL:1510-1529). The "fixed Linux x86-64 or AArch64 profile"
   check is a match with `_ =>` returning an error.
6. **Same job elsewhere.** The row metadata appears twice, once in `derive_contents` (PL:1339-1386) and again in
   `validate_contents` (PL:1481-1530) as an independent copy; this is the "independent replay" pattern.
   The first-pass section roster for the other 7 rows is in `section_headers.rs` (suspected; not read).
7. **Unused or one-caller code.** `appended_descriptor_count` is `#[cfg(test)]` (PL:1166).
8. **Stated limits.** None.
9. **Fail-open risks.** None seen. `unreachable!("descriptor count checked above")` (PL:1474) follows the count
   check at PL:1461.

Tests (PL:1590-1952): group entry. Both targets give names at offsets 79, 84, 93, 103, total seed 113 bytes
and 11 descriptors (PL:1671-1673, 1689, 1719, 1735, 1754); permutation identity; 27 corruption closures
(PL:1817-1880); AArch64 purecode flag removal rejects (PL:1902-1928); `validate_name` bounds (PL:1931-1951).

## PL 2435-2463: `candidates` tail (`derive_constraints`, `got_slot_byte_offset`)

1. **Purpose.** Turns each procedure-linkage fixup into at most one placement constraint, and computes the GOT
   slot byte offset.
2. **Shape.** `derive_constraints` (PL:2420; body from 2435 in range), `got_slot_byte_offset` (PL:2452).
3. **How it decides.** Keyed on fixup kind: `X86PcRelative32` gives `X86Signed32`, `Aarch64Branch26` gives
   `Aarch64Branch26`, (arms before 2435 not read; the Page21 and Load64 mapping is outside the range, inferred
   from the enum names `Aarch64PageDelta21`, `Aarch64Load64Low12Aligned` at PL:2688-2689). `Aarch64AddLow12`,
   `Absolute64`, `Elf64RelaOffset` give no constraint (PL:2438-2440), matched by name, with no `_` arm.
4. **Unused or one-caller code.** n/a.
5. **Fail-open risks.** None seen; the match lists the remaining kinds explicitly (PL:2438-2440), so a new
   `ElfProcedureLinkageFixupKind` variant fails to compile instead of being silently skipped. Traced (read).

## PL 2465-2533: `compatibility_fingerprint`

Hashes policy, the four byte vectors, each fixup (storage, offset, width, mask, kind, semantic target) and
each constraint (PL:2473-2501), with a per-variant byte tag for `ElfProcedureLinkageSemanticTarget`
(PL:2503-2532, tags 1..7). The hash recipe is the same as LP:998 and PL:1564 (`stage_fingerprint` plus FNV).

## PL 2535-2692: `template_plans` (types)

Types only. `ValidatedElfProcedureLinkageTemplatePlan` (PL:2542), error type with `linkage` custody
(PL:2601), `ElfProcedureLinkageTemplateContents { policy, bytes, fixups, constraints }` (PL:2612),
`ElfProcedureLinkageTemplatePolicy` (X86_64SmallMediumLazy, Aarch64StandardLazy; PL:2621),
`ElfProcedureLinkageFixup` (PL:2635, with `mutable_mask: u64`), `ElfProcedureLinkageFixupStorage` (SourceText,
Plt, GotPlt, RelaPlt, RelaDyn), `ElfProcedureLinkageFixupKind` as an alias of `crate::ElfAppliedProcedureLinkageKind`
(PL:2654; the enum is at `file_assembly.rs:2850`), `ElfProcedureLinkageSemanticTarget` (seven variants,
PL:2657), constraints and constraint kinds (PL:2679-2691). Several accessor methods are `#[cfg(test)]`
(PL:2553-2579).

## PL 2694-3061: `template_validation`

1. **Purpose.** Independently replays the generated GOT/PLT template against the target's fixed bytes and the
   semantic linkage rows.
2. **Shape.** `validate_contents` (PL:2715), `validate_encoded_resources` (PL:2744), `validate_fixed_template_bytes`
   (PL:2799), `replay_x86_plt` (PL:2861), `replay_aarch64_plt` (PL:2890), macro `replay_rela_rows!` giving
   `replay_rela` and `replay_general_rela` (PL:2917-2960), `validate_fixup_placeholders` (PL:2962),
   `validate_semantic_target` (PL:3001), `validate_fixup_coverage` (PL:3035).
3. **Inputs and outputs.** Reads linkage rows, image import program and text, template bytes and fixups;
   returns `Ok(())` or a `Diagnostic`.
4. **How it decides.** Keyed on `TargetProfile`: LinuxX64 or LinuxArm64, else error (PL:2721-2725).
   PLT header and entry bytes are compared exactly with the constants `X86_PLT_HEADER` etc. (PL:2866, 2880,
   2895, 2910); x86 entry opcode bytes `ff 25 .. 68 <ordinal> e9 ..` are matched literally (PL:2880-2883). GOT
   bytes must all be zero (PL:2854). RELA rows must have `r_offset == 0`, `r_info == sym<<32|type`,
   addend equal (PL:2929-2946). Fixups must equal `derive_fixups(..)` (PL:2731-2735) and be zero-filled where
   `mutable_mask` selects (PL:2988). Encoded import bodies must have exact effects/resource sets per target:
   x86 reads and writes RSP with 8 stack bytes; AArch64 resolver reads X16/X17/X30, writes X16/X17, 16 bytes;
   other import bodies read/write X16/X17, 0 bytes (PL:2755-2781, `expected(resolver)`).
5. **Cases by shape.** `validate_semantic_target` (PL:3011-3030): `FutureDynamicSection | PltHeader => true`
   for any input; ordinals must be a valid slot; `GotPltHeaderWord.word_index < 3`; `RelocatedImageSection`
   must match a general relocation by section and offset.
6. **Same job elsewhere.** Overlap check at PL:3035-3060 matches `validate_application_coverage` at LP:2189.
   Hard-coded 16/32-byte PLT sizes at PL:3083-3091 are `const`s defined after the modules that use them.
7. **Unused or one-caller code.** `validate_fixup_coverage` is `pub(crate)`; grep gives its definition, the
   call in `validate_contents` (PL:2737) and one test import (PL:3140).
8. **Stated limits.** `FutureDynamicSection` target name suggests deferral (PL:2658); no comment. 
9. **Fail-open risks.** `validate_semantic_target` accepts `FutureDynamicSection` and `PltHeader` without any
   check (PL:3013-3014); a fixup that wrongly names these targets passes this check. It would still have to
   equal `derive_fixups` output (PL:2733), so it is "traced": a bad one is caught by the equality, not here.
   `validate_encoded_resources` fails on `unreachable!` for non-Linux targets (PL:2767), but `validate_contents`
   calls it before the target match at PL:2720-2725; callers validated the target earlier (traced only as
   far as `derive_contents` taking the same `target(linkage)`; inferred).

## PL 3063-3130: re-exports, constants, templates entry

`plan_elf_procedure_linkage_templates` (PL:3117) runs `checked::plan_stage!` with `derive_contents` and the
fingerprint. Constants (PL:3081-3115): GOT word 8, x86 PLT header/entry 16/16, AArch64 header/entry 32/16,
`GOT_PLT_HEADER_WORDS = 3`, header and entry byte arrays with disassembly comments (x86 `pushq GOT[1]`,
`jmpq *GOT[2]`; AArch64 `stp x16,x30`, `adrp`, `ldr`, `add`, `br x17`).
The x86 PLT entries (PL:2880) and AArch64 entries (PL:3110) are lazy-binding stubs; the policy enum names them
"SmallMediumLazy" and "StandardLazy" (PL:2622-2623).

## PL 3132-3465: tests of the template stage

Group entry. Seven tests: encoded import program mutation (four changes) must reject (PL:3179-3220),
exact sizes (x86 PLT 64, AArch64 80, GOT 48, RELA 72, fixups 19 or 22, constraints 12; PL:3227-3245),
AArch64 keeps the 3 header words zero and targets PLT0 (PL:3280-3305), permutation identity, 26 corruptions
(PL:3337-3391), 9 AArch64 corruptions (PL:3414-3427), arithmetic guards.

## Coverage

- Read in full: `load_placement.rs` 471-585 (115), 986-3379 (2394); `procedure_linkage.rs` 581-1954 (1374),
  2435-3466 (1032). Total 4915 lines, all in the unit file `units/gap_008.tsv`.
- Skimmed: none.
- Not read: none of the unit's ranges. Code outside the ranges (before LP:471, LP:586-985, PL 1-580,
  PL 1955-2434, anything after LP:3379/PL:3466) was not read; mentions of it are marked inferred or grepped.

## Five things a maintainer most needs to know about this unit

1. Every stage here is the same three-part pattern: `plan_stage!` derive, `seal_stage!` validate, plus a
   `Candidate` that a test corrupts and an `FNV` "non_authoritative" fingerprint (LP:1948, 2701, PL:1273, 3123).
   The validator re-derives rules by hand a second time (PL:1339 vs 1481), so a rule change needs two edits.
2. Only `LinuxX64` and `LinuxArm64` are accepted. Other `TargetProfile` values error in descriptors
   (PL:1332) and templates (PL:2724), while `target_tag` (LP:1047) hashes nine profiles.
3. Counts are hard-coded: 25 placements (13 file offsets, 12 addresses), 14 rows, 9 or 7 dynamic address
   targets, 4 appended descriptors on 7 base rows, 5 program headers (LP:1791-1793, 1878-1881, 2535, PL:1146-1147).
   Several error messages say other numbers (832 bytes, twelve, eleven; LP:1966, 2117, 2128).
4. Each section kind is identified by position: array index equals the enum discriminant (LP:553, 2772-2777,
   3008-3019), and `public_section_kind`/`public_target` are identity functions (LP:1112, 3004).
5. `checked_align` exists twice with different rules (LP:1061 power-of-two required; `section_headers.rs:289`
   any alignment).

## What you are unsure about

- Whether `ElfPlacedDynamicSectionKind` and `ElfDynamicRosterSectionKind` are one type; the identity body at
  LP:1112 compiles only if so, but I did not open `section_roster`.
- The `PageDelta21` and `Load64Low12Aligned` arms of `derive_constraints` (before PL:2435) were not read.
- What `ElfDynamicAddressApplicationKind` being `#[cfg(test)]` only means for release builds beyond `kind_tag`
  comparison (LP:2890); I did not trace the callers.
- Counts of "grep hits" include the definition, tests and imports, so they are upper bounds on call sites.
