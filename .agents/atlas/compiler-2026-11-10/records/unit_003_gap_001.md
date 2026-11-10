Second pass for unit 3

Ranges covered (paths relative to `omega-rust/omega/compilation/backend/images/image-elf/src/`; 18 ranges, 5,728 lines):

| # | file | lines | count | enclosing production module (header line) |
|---|---|---|---|---|
| 1 | import_sections.rs | 140-339 | 200 | `dynamic_link` (:10), tests module |
| 2 | import_sections.rs | 1002-1301 | 300 | `dynamic_section_bytes` (:340), tests module |
| 3 | import_sections.rs | 1807-2145 | 339 | `dynamic_section_descriptors` (:1302), tests module |
| 4 | import_sections.rs | 2937-3294 | 358 | `dynamic_sections` (:2146), tests module |
| 5 | procedure_linkage.rs | 581-1128 | 548 | `dynamic_import_relocations` (:9), tests module |
| 6 | procedure_linkage.rs | 1590-1954 | 365 | `dynamic_linkage_descriptors` (:1129), tests module |
| 7 | procedure_linkage.rs | 2959-3248 | 290 | `dynamic_linkage_templates` (:1955), tests module (not in a sub-module) |
| 8 | section_headers.rs | 323-514 | 192 | `relative_section_layout` (:10), tests module |
| 9 | section_headers.rs | 868-1156 | 289 | `section_header_bytes` (:515), tests module |
| 10 | section_headers.rs | 1444-1638 | 195 | `section_name_table` (:1157), tests module |
| 11 | section_headers.rs | 2152-2441 | 290 | `section_payload_roster` (:1639), tests module |
| 12 | section_headers.rs | 2982-3183 | 202 | `section_roster` (:2442), tests module |
| 13 | load_placement.rs | 1297-1659 | 363 | `load_layout` (:9), tests module (range ends at the last assert; :1660 closes the fn, :1661 the tests module) |
| 14 | load_placement.rs | 2239-2508 | 270 | `placed_section_headers` (:1861), tests module |
| 15 | load_placement.rs | 3097-3379 | 283 | `resolved_dynamic_table` (:2509), tests module |
| 16 | file_assembly.rs | 753-1250 | 498 | `assembled_dynamic_file` (:10), tests module |
| 17 | file_assembly.rs | 1732-2088 | 357 | `dynamic_file_envelope` (:1251), tests module |
| 18 | file_assembly.rs | 3559-3947 | 389 | `resolved_procedure_linkage` (:2089), tests module; range starts inside the `use super::{ ... }` list at :3558-3563 (tests module opens at :3557) |

All 5,728 lines were read in full (I read each range plus 15-40 lines above it for the module header).

What the ranges are, in one sentence (read): every range is the body of a `#[cfg(test)] mod tests` of one pipeline stage, plus the closing braces of the enclosing production module (import_sections.rs:338, :1300, :2144, :3294; procedure_linkage.rs:1127, :3248; section_headers.rs:513, :3183; load_placement.rs:2507, :3379; file_assembly.rs:1249, :2087, :3947) and, for range 6, the `#[cfg(test)]` attribute line itself (procedure_linkage.rs:1590). No range holds production logic. Where I say "decides", it means "what the tests pin", not what the production code does.

How I verified module headers: read the lines around each range start; module header lines are from the earlier notes and were confirmed by what I read (for example `dynamic_link` closes at import_sections.rs:338).

---

## Common test skeleton (read across all 18 ranges)

Every stage's tests follow one pattern (read):
1. A `candidate(target)` helper that runs the stage's own `derive_contents` and fingerprint function and hand-builds the stage's private `Candidate` struct: import_sections.rs:1042, :1857, :2976; procedure_linkage.rs:716, :1634, :2992; section_headers.rs:360, :901, :1480, :2231, :3009; load_placement.rs:1338, :2269, :3132; file_assembly.rs:856, :1822, :3650.
2. A "positive on both Linux targets" test asserting exact golden numbers (byte counts, offsets, row counts), then `validate_contents(...)` on the stage's own output ("independent replay").
3. An "import permutation preserves identity, target binds identity" test: the stage run on reversed imports gives equal contents and fingerprint, and on LinuxArm64 gives a different fingerprint (import_sections.rs:1138, :2059, :3061; procedure_linkage.rs:793, :1774, :3090; section_headers.rs:440, :1019, :1539, :2319, :3052; load_placement.rs:1485, :2407, :3224; file_assembly.rs:1018, :3783).
4. A "corruptions" list `Vec<Box<dyn Fn(&mut Candidate)>>` where every closure mutates one field/byte of the candidate, then `validate_candidate(candidate)` must return `Err`, and the test asserts the error hands back the upstream carrier unchanged ("custody": compare an upstream fingerprint before/after). Grep count of the literal `Vec<Box<dyn Fn(&mut` in image-elf/src: `import_sections.rs:3, dynamic_table.rs:5, load_placement.rs:4, procedure_linkage.rs:4, section_headers.rs:5` = 21 total (Grep count mode); 16 of those are in my ranges (import_sections.rs:1194, :2084, :3140; load_placement.rs:1503, :1542, :2428, :3254; procedure_linkage.rs:916, :1817, :3119, :3196; section_headers.rs:461, :1087, :1565, :2388, :3078). file_assembly.rs has 0 such lists; it writes one block per mutation instead (e.g. :1073-1089, :1093-1149).
5. A "bounds helpers reject without panicking" test (`checked_sum(usize::MAX, ..)`, `read_u32(&[], ..)`, etc.): import_sections.rs:1263, :2135, :3233; procedure_linkage.rs:996, :1931, :3229; section_headers.rs:499, :1136, :1610, :2432, :3157; load_placement.rs:1655, :2489, :3370.

Same job elsewhere (grepped): helper fns named `candidate`, `refresh_fingerprint`, `custody_snapshot`, `assert_rejected_with_custody`, `assert_rejected_with_exact_custody` are defined per file; Grep count mode of `fn (assert_rejected_with_(exact_)?custody|refresh_fingerprint|custody_snapshot|candidate)\(` over image-elf/src: dynamic_table.rs 3, file_assembly.rs 10, load_placement.rs 3, section_headers.rs 5, procedure_linkage.rs 3, import_sections.rs 3 = 27 definitions. They are not shared in `test_support` (lib.rs); each has a different body.

---

## Range 1: import_sections.rs:140-339 (tests of `dynamic_link`)

**Purpose.** Pins the four ways `plan_elf_dynamic_link_inputs` (import_sections.rs:66) accepts or rejects and that rejection returns both original inputs unchanged.

**Shape.** Helpers `interpreter` :152 (calls `normalize_elf_interpreter_plan`), `image_with_import` :156, `versioned_import` :173. Tests: `exact_interpreter_and_canonical_versioned_import_rows_join_without_mutation` :194, `target_drift_rejects_and_returns_both_original_inputs` :249, `unused_interpreter_and_string_bootstrap_import_reject_recoverably` :270, `canonical_import_rejection_returns_unmodified_inputs` :304.

**Inputs/outputs.** Builds images with `crate::test_support::relocation_image(target, 20)` and `import_with_text_relocations(.., &[3, 15])` (:163-169). Reads `ElfImportLocator::Versioned{..}` out of the planned inputs (:216-237) and the relocation offsets `[3, 15]` (:238-245).

**How it decides (what is pinned).**
- Target drift: LinuxX64 image + LinuxArm64 interpreter -> message contains "final-image target" (:249-266), i.e. the check at import_sections.rs:78-85.
- Empty image (`FinalImage::with_capacity(.., 0, 0, 0)`) -> "no referenced" (:271-287), the check at :92-100.
- `StringBackedBootstrap` import -> "string-backed" (:289-300), the check at :106-114.
- Duplicate import row (same row inserted twice) -> "duplicate import rows" (:316-335), originating in imports.rs:51-53 (message at imports.rs:53, grepped).
- `image_with_import` picks the image target from the locator target for `Normalized`, defaulting to `linux_x64` for the other two plans (:157-162).

**Cases by shape.** Only these four situations. The message at import_sections.rs:126 ("versioned ELF import profile does not match the exact interpreter profile") is not asserted by any test: Grep of "exact interpreter profile" over image-elf/src finds it only at import_sections.rs:126 (production). Inferred: after :78 requires `interpreter.target().native_target() == image.target` and imports.rs:120-125 requires the locator target's native target to equal `image.target` (read imports.rs:120-125), the check at import_sections.rs:116 can only fire if a `TargetProfile` maps to a `NativeTarget` many-to-one; I did not check that mapping. The `Component` locator branch (:105) is not exercised by these four tests.

**Same job elsewhere.** Interpreter path literals in tests: see "Duplicated test fixtures" below.

**Unused/one-caller.** n/a (tests).

**Stated limits.** None.

**Fail-open risks.** None seen (tests only). Note the tests assert only message substrings (`contains(..)`), keyed on source spelling (:266, :287, :300, :335).

---

## Range 2: import_sections.rs:1002-1301 (tests of `dynamic_section_bytes`)

**Purpose.** Pins the serialized ELF payload bytes (interpreter, `.dynstr`, `.dynsym`, `.hash`, `.gnu.hash`, `.gnu.version`, `.gnu.version_r`) and that the independent decoders reject every corruption.

**Shape.** Imports `Candidate`, `decode_dynsym/sysv_hash/gnu_hash/verneed/versym`, `encode_payloads`, `validate_candidate`, `validate_payloads`, `read_u16/u32` (:1003-1008). Helpers `interpreter_path` :1014 (delegates to `test_support::import_fixture_interpreter_path`), `structural_plan` :1018, `structural_plan_with_interpreter` :1025 (calls `single_site_import_inputs(target, imports, "__omega_payload_import_", 64, interpreter_path, ..)`), `candidate` :1042, `words` :1054. Tests: :1064, :1138, :1155, :1193, :1263.

**How it decides (golden values pinned).** For `SINGLE_SITE_IMPORTS` (4 imports; lib.rs:396 grep) on both Linux targets: interpreter = path + NUL (:1070-1073); `.dynstr` 64 bytes with exact string `\0OMEGA_1\xfd\0OMEGA_2\0alpha\xfe\0beta\0gamma\0libalpha\xff.so.1\0libbeta.so.2\0` (:1074-1078); `.dynsym` 120 bytes = 5 x 24 and first row zero, second row `[18,0,0,0,0x12,..]` = name offset 18, `STB_GLOBAL<<4|STT_FUNC` (:1079-1086); `.hash` 44 bytes words `[4,5,0,3,1,0,0,2,0,4,0]` (:1087-1091); `.gnu.hash` 44 bytes, header words 1,1,1,5 then bloom `0x0090_2200`/`0x0000_0102`, bucket 1, chains `0xf204_f288, 0xf204_f288, 0x7c94_89a0, 0x0f7d_eae9` (:1092-1108); `.gnu.version` 10 bytes `[0,0,2,0,3,0,2,0,4,0]` (:1109); `.gnu.version_r` 80 bytes with `vn_cnt` 2, file 36, aux 16, next 48, aux other values 2/3/4 (:1110-1122). Identity: fingerprint differs on profile, on interpreter path, on one changed symbol name (:1155-1190). 34 corruption closures (:1195-1244: interpreter pop/push, `dynstr[0]=1`, dynstr pop, dynsym pop/push, `dynsym[24..28].reverse()`, `dynsym[24]=0`, sysv_hash pop/push and four field fills, gnu_hash pop/push and eight field edits, versym pop/push/`[2]=1`, verneed pop/push and four field fills, fingerprint ^= 1); each must be rejected by `validate_candidate`. Adversarial inputs (:1263-1297): short/`0xff` buffers to `decode_sysv_hash`, out-of-range chain and a cyclic chain, short buffers to `decode_gnu_hash(.., 5)` and `decode_verneed(.., 1)`, a cyclic auxiliary chain, `decode_dynsym(&[0;23], 1)`, `decode_versym(&[0], 1)`, `name_at(&[0], u32::MAX, ..)`.

**Cases by shape.** Corruption coverage of `.dynsym` bytes touches only offsets 24..28 and 24 (:1207-1208): bytes `info`, `other`, `shndx`, `value`, `size` of a symbol row are not individually mutated in this list (read; whether the decoder would reject them is not shown by these tests).

**Same job elsewhere.** The `.hash` and `.gnu.hash` golden vectors reappear in range 4 (import_sections.rs:3032-3037, same bloom and chains).

**Stated limits / fail-open.** None quoted. None seen.

---

## Range 3: import_sections.rs:1807-2145 (tests of `dynamic_section_descriptors`)

**Purpose.** Pins the seven address-free section descriptors (name offsets, types, flags, sizes, link, info) and the section-name seed.

**Shape.** Local `IMPORTS: [SingleSiteImportFixture; 3]` :1822-1838 (a different 3-entry list from `test_support::SINGLE_SITE_IMPORTS`, which has 4 entries, lib.rs:396), helpers `payloads` :1840, `candidate` :1857, `row` :1869; tests :1881, :2008, :2059, :2083, :2135. Uses `crate::dynamic_executable::interpreter_path` (`#[cfg(test)]` fn at dynamic_executable.rs:666) at :1817 and :1849.

**How it decides (golden values).** 7 descriptors; seed byte count 79 (:1887-1888); name offsets `[1, 9, 17, 25, 31, 44, 69]` (:1895); `.shstrtab` name offset `SECTION_NAME_TABLE_NAME_OFFSET` = 59 stays resolvable after appending `.dynamic\0.rela.dyn\0` to the seed (:2026-2043). Payload sizes `[interp, 43, 96, 36, 8, 80, 40]` (:1903-1911). Per-kind descriptors (:1914-1997): Interpreter PROGBITS/ALLOC align 1 entsize 0 no link; DynamicSymbol `SHT_DYNSYM` align 8 entsize 24 link DynamicString info 1; SystemVHash `SHT_HASH` align 4 entsize 4 link DynamicSymbol; GnuSymbolVersion `SHT_GNU_VERSYM` align 2 entsize 2 link DynamicSymbol; GnuVersionRequirement `SHT_GNU_VERNEED` align 4 entsize 0 link DynamicString info 2; GnuHash `SHT_GNU_HASH` align 8 entsize 0 link DynamicSymbol. All flags `SHF_ALLOC`. 17 corruptions (:2085-2111). `name_at`/`read_u32` bounds (:2136-2141).

**Contradicts earlier notes?** The earlier notes (unit_003.md item 6) say the descriptor stage "hard-coded 113-byte..." (sentence cut off). In this range the seed is 79 bytes (import_sections.rs:1888); 113 is the size after the linkage-descriptor stage appends 34 bytes (procedure_linkage.rs:1671-1673, range 6). Not a contradiction about the code; the earlier sentence is just incomplete.

**Same job elsewhere.** Descriptor literals again in the production `derive_contents` and `validate_row` (earlier notes); here they appear a third time as test expectations.

**Fail-open.** None seen.

---

## Range 4: import_sections.rs:2937-3294 (tests of `dynamic_sections`)

**Purpose.** Pins the address-free dynamic table contents (strings, symbols, hashes, versions, needed list), cross-table validation, and the hash functions.

**Shape.** Helpers `interpreter_path` :2949 (second local copy of the same delegating fn; first is :1014), `input` :2953, `input_with_interpreter` :2960, `candidate` :2976, `string_at` :2982; tests :2987, :3061, :3084, :3116, :3139, :3193, :3210, :3233.

**How it decides (golden values).** `dynsym.len() == IMPORTS.len() + 1` (:3003); `dynsym` names `[0, 18, 18, 25, 30]` (:3011); every non-null symbol `info == (STB_GLOBAL<<4)|STT_FUNC`, `other == 0`, `section_index == SHN_UNDEF`, `value == 0`, `size == 0` (:3013-3020); `versym[1..] >= 2` (:3023); GNU hash: 1 bucket, symbol_offset 1, 1 bloom word, shift 5, bloom `0x0000_0102_0090_2200`, bucket `[1]`, 4 chains (:3028-3037); `needed == [36, 51]` (:3039); 2 verneed records, `required_version_count() == 3` (:3040-3041); each aux `hash == elf_hash(name)`, `flags == 0`, `other >= 2` (:3042-3051). Version-index test: two imports of symbol `alpha\xfe` get different version indexes (:3116-3136). 26 corruptions (:3141-3176) over interpreter, dynstr, dynsym, both hashes (bucket_count, symbol_offset, bloom_count, bloom_shift, bloom, buckets, chains), versym, verneed (count, auxiliary_offset, next_offset, aux hash, aux next), needed, bindings, fingerprint. Hash vectors: `gnu_hash(b"") == 0x1505`, `"exit"` 0x7c967e3f, `"printf"` 0x156b2bb8, `"memcpy"` 0x0d827590 (:3211-3214). `derive_shared_import_contents(&[])` (the only direct test call, :3195) yields dynstr `[0]`, one default dynsym, empty needed/bindings, bloom `[0]`, buckets `[0]`, no chains (:3196-3202); `validate_gnu_hash_reachability` accepts it and rejects `buckets[0] = 1` (:3203-3206). Malformed hash counts: zero buckets, mismatched bucket/chain counts, `ElfSysvHash` with 0 buckets / 0 chains / out-of-range bucket / a cyclic chain, and GNU hash with cleared bloom, `u32::MAX` bucket, cleared terminator bit (:3233-3291).

**Same job elsewhere.** `derive_shared_import_contents` is `pub(crate)`; the earlier notes counted one non-test outside caller (shared_component.rs:96). Here it has 1 test caller (:3195) (grep of the name was done by the earlier reader; I only confirm this call site).

**Fail-open.** None seen.

---

## Range 5: procedure_linkage.rs:581-1128 (tests of `dynamic_import_relocations`)

**Purpose.** Pins stage 5: how call relocations become logical PLT/GOT slots, JUMP_SLOT rows and general (data) relocation rows; and what a malformed source relocation does.

**Shape.** Helpers `image` :600 (builds a 64-byte text image; for LinuxX64 writes `0xe8` at the instruction offset and relocation at +1, kind `X86_64Relative32`; for LinuxArm64 writes `[0,0,0,0x94]` and relocation at the instruction offset, kind `Aarch64Branch26`, :633-642), `image_with_general_relocations` :660 (adds a 48-byte `.data` and `Absolute64` width-8 relocations on chosen imports), `descriptors_from_image` :692 (runs preflight, sections, serialize, descriptors), `descriptors` :709, `candidate` :716 (uses one general relocation `(0, 8)`). Tests :732, :793, :833, :915, :996, :1028, :1070.

**How it decides (pinned).** With the 3-entry `test_support::IMPORTS`: 3 logical slots, 3 procedure relocations, 4 direct call sites, 0 general rows (:736-739); slot call-site counts `[2, 1, 1]` (:821-829); `dynamic_symbol_index` `[1,2,3]` (:763-770); jump-slot type `R_X86_64_JUMP_SLOT` / `R_AARCH64_JUMP_SLOT` (:750-754). Seven source mutations must reject, each returning the descriptor carrier with identical fingerprint (:835-891): section -> Data, kind -> Absolute64, `byte_width = 8`, `addend = 1`, `offset = usize::MAX`, `text[0] = 0x90`, a duplicate relocation offset. Arm: relocation offset 1 (misaligned BL) rejects (:894-911). 26 closures in the candidate corruption list (:917-973). General rows: offsets `[8, 24]`, request indexes `[0, 2]`, dynamic symbols `[1, 3]`, type `R_X86_64_64` / `R_AARCH64_ABS64`, section Data, addend 0 (:1028-1066). Five malformed general shapes reject: section Bss, width 4, offset 48 (out of the 48-byte data), offset 4 (misaligned), kind `X86_64Relative32`; plus two general relocations at offsets 8 and 12 (overlap) reject (:1070-1124).

**Same job elsewhere.** `image()` and `image_with_general_relocations()` here re-implement what `test_support::insert_text_call_relocations` (lib.rs:554) does for ranges 11, 16, 17 and 18 (read; different loop but same x86/arm placeholder bytes).

**Fail-open.** None seen. Observation of coverage: the mutation `byte_width = 8` and `kind = Absolute64` on a Text-section relocation both land in the "not a direct call shape" branch which then falls to the data-relocation test (`section == Data` required); they reject because the section is Text. That matches the production note (procedure_linkage.rs:296-322 per earlier notes).

---

## Range 6: procedure_linkage.rs:1590-1954 (tests of `dynamic_linkage_descriptors`)

**Purpose.** Pins stage 6/7's section descriptors for `.plt`, `.got.plt`, `.rela.plt`, `.rela.dyn` appended to the 79-byte seed.

**Shape.** `IMPORTS` = `PAIR_IMPORTS` (:1609-1610; `PAIR_IMPORTS` defined lib.rs:617), helper `templates` :1613 (full chain preflight -> sections -> payloads -> descriptors -> relocations -> templates), `candidate` :1634, `row` :1646; tests :1658, :1774, :1816, :1902, :1931.

**How it decides (pinned).** Base descriptors 7, base seed 79; total 11, appended 4; seed becomes 113 (:1663-1673); the suffix equals `PROCEDURE_LINKAGE_NAME_SUFFIX` at offset 79 (:1679-1683). Name offsets: `.plt` 79, `.got.plt` 84, `.rela.plt` 93, `.rela.dyn` 103 (:1689, :1718, :1735, :1754). `.plt`: PROGBITS align 16 entsize 0, flags ALLOC|EXEC on x64 and ALLOC|EXEC|`SHF_AARCH64_PURECODE` on arm; payload 48 (x64) / 64 (arm) for 2 imports (:1695-1712). `.got.plt`: ALLOC|WRITE align 8 payload 40 (:1715-1727). `.rela.plt`: `SHT_RELA` ALLOC|`SHF_INFO_LINK` payload 48 entsize 24 link DynamicSymbol info RelocatedSection(ProcedureGot) (:1728-1746). `.rela.dyn`: payload 0, same flags/link, info None (:1747-1763). 20 corruptions (:1818-1879) including `flags ^= SHF_WRITE`, `link`/`info` swaps, `alignment = 3`. AArch64 test: clearing the PURECODE bit rejects (:1921-1927). `validate_name` edge cases (:1946-1950).

**Same job elsewhere.** The `.plt` payload sizes here (48/64 for two imports) are 16+2x16 and 32+2x16, matching the constants in range 7's expectations (3 imports: 64/80, :3014-3015).

**Fail-open.** None seen.

---

## Range 7: procedure_linkage.rs:2959-3248 (tests of `dynamic_linkage_templates`)

**Purpose.** Pins stage 7's template bytes (`.plt`, `.got.plt`, `.rela.plt`), the fixup list and the placement-constraint list.

**Shape.** Imports `Candidate` and `derive_contents` from `...dynamic_linkage_templates::candidates` and the validators from `...::template_validation` by full path (:2965-2966), so those are reachable `pub(crate)`. Helpers `linkage` :2972, `candidate` :2992; tests :3005, :3062, :3090, :3118, :3195, :3229. The whole `use super::{..}` list (:2960) and the imports at :2965-2968 are single unwrapped lines.

**How it decides (pinned).** With 3 imports: `.got.plt` 48 bytes (all zero, :3032), `.rela.plt` 72 bytes with `r_offset 0`, `r_info = (sym<<32)|type`, `r_addend 0` (:3041-3055), `.plt` 64 (x64) / 80 (arm), fixups 19 (x64) / 22 (arm), 12 placement constraints (:3009-3027). AArch64: `.plt[..32] == AARCH64_PLT_HEADER`, `.plt[32..48] == AARCH64_PLT_ENTRY`, `.got.plt[..24]` zero, no `GotPlt` fixup below byte 24, exactly 3 `GotPlt` fixups target `PltHeader`, none targets `FutureDynamicSection` (:3062-3087). 26 closures over policy, bytes, fixup fields (storage, byte_offset, width, mask, kind, target), constraints, fingerprint (:3120-3172). AArch64-only list of 9 (:3196-3209): `plt[0]^=1`, `plt[4]|=0x20`, `plt[8]|=0x04`, `plt[12]|=0x04`, `plt[32]|=0x20`, `got_plt[0]/[8]/[16] = 1`, `fixups[0].target = FutureDynamicSection`. `validate_fixup_coverage(&[overflowing, overflowing])` rejects (:3237-3245).

**Cases by shape.** The x64 `.plt` bytes are not asserted literally in this range (only `plt[0]^=1`, `plt[2]=1` mutations at :3124-3125); AArch64 bytes are compared to the named constants (:3066-3067).

**Fail-open.** None seen.

---

## Range 8: section_headers.rs:323-514 (tests of `relative_section_layout`)

**Purpose.** Pins stage 15's relative layout: per-row region, size, alignment, offset and region spans, with no absolute address.

**Shape.** Helpers `payloads` :336 (one import `liblayout.so`, 32-byte text, `import_call_indexed_payloads`, lib.rs:526), `candidate` :360; tests :373, :440, :460, :499.

**How it decides (pinned).** Row count `SECTION_COUNT` (:376); row 0 has offset 0, size 0, region `None` (:381-383); rows 1..=7 ReadOnly, 8 ReadExecute, 9 ReadWrite, 10 and 11 ReadOnly, 12 ReadWrite, 13 FileOnly (:384-412); all four region spans non-zero (:413-420); walking `physical_section_order()` (`[0..9, 11, 10, 12, 13]`, section_headers.rs:269-271) each row starts at or after the previous end in its region and satisfies `offset % alignment.max(1) == 0` (:421-434). 14 corruptions (:462-479). `classify_region` (section_headers.rs:273-287, read): `SHF_WRITE` alone, `SHF_EXECINSTR` alone, and ALLOC|WRITE|EXEC are errors (:504-506); `SHF_ALLOC | (1<<63)` is ReadOnly (:507-510), so flag bits other than ALLOC/WRITE/EXECINSTR are ignored by classification (production code at :275-277 masks only those three). `checked_align(7, 0, ..) == 7` and `checked_align(u64::MAX, 8, ..)` errors (:502-503).

**Same job elsewhere.** `checked_align` here (modulo-based, section_headers.rs:289-297) is a different function from `compatibility_fingerprint::checked_align` in load_placement.rs (mask-based; tested in range 13 at :1659 where `checked_align(7, 3, ..)` must error). Here `checked_align(7, 0)` returns 7; there `alignment <= 1` is identity (earlier notes). The two have different behavior for non-power-of-two alignments: not tested here (no non-power-of-two input to this version).

**Fail-open.** "Stated" by the test itself: unknown extra flag bits do not change classification (:507-510). Not reachable from outside input as far as these ranges show (the roster descriptors are literals; I did not trace every flag source).

---

## Range 9: section_headers.rs:868-1156 (tests of `section_header_bytes`)

**Purpose.** Pins the 14-row `Elf64_Shdr` template (896 bytes) and the 25 zero placement fixups.

**Shape.** `IMPORTS` = `PAIR_IMPORTS` (:882-883), helpers `roster` :886 (via `test_support::dynamic_section_names`, then `plan_elf_dynamic_section_roster`), `candidate` :901, `row` :913; tests :919, :951, :1019, :1044, :1086, :1136.

**How it decides (pinned).** `row_count 14`, `byte_count 896`, `placement_fixup_count 25`, row 0 all zero (:923-926). Decoded rows (:928-941): row 3 `.dynsym` name 17, type 11, flags 2, size 72, link 2, info 1, align 8, entsize 24; row 7 `.gnu.hash` type `0x6fff_fff6` link 3; row 10 link 3 info 9; row 11 link 3 info 0; row 12 link 2; row 13 name 59 size 122 address 0 file offset 0. Fixup coordinates (:955-997): `[0]` row 1 Interpreter VirtualAddress at byte 80; `[1]` byte 88; `[20]` row 11 GeneralRelocation VirtualAddress at 720; `[21]` 728; `[22]` row 12 DynamicTable VirtualAddress at 784; `[23]` 792; `[24]` row 13 SectionNameTable FileOffset at 856; 12 VirtualAddress fixups total (:998-1008); every fixup field currently reads 0 (:1009-1015). Boundary test (:1044-1083): for rows 0, 1 and `SECTION_COUNT-1`, flip the first and last byte of each of the 10 header fields; every flip must reject with a message containing "decoded ELF64 section header drifted" (:1067-1072). 11 closures (:1088-1115).

**Stated limits / stale messages.** The test at :919 is named "twenty_five_zero_fixups", and asserts 25 (:925), while the production error text says "exactly twenty-three" (earlier notes, section_headers.rs:746); the test constant disagrees with that message.

**Fail-open.** None seen.

---

## Range 10: section_headers.rs:1444-1638 (tests of `section_name_table`)

**Purpose.** Pins stage 11: the complete 122-byte `.shstrtab` payload and its descriptor.

**Shape.** `IMPORTS` = `PAIR_IMPORTS` (:1462), helpers `dynamic_table` :1463 (runs tags -> serialize -> dynamic-table descriptor), `candidate` :1480; tests :1493, :1539, :1564, :1610.

**How it decides (pinned).** Upstream descriptor count 12, this stage 13, appended 1, byte_count 122 (:1497-1500); `contents.bytes == COMPLETE_SECTION_NAME_TABLE` (section_headers.rs:1170, read via grep) and `== upstream_name_seed(..)` (:1501-1505); descriptor: name_offset 59, `SHT_STRTAB`, flags 0, size 122, align 1, entsize 0, link/info `None` (:1518-1531). 14 corruptions (:1566-1589). `validate_complete_names(&[])`, a 101-byte prefix, and a trailing NUL all error (:1613-1617); `validate_name` edge cases (:1630-1634).

**Fail-open.** None seen.

---

## Range 11: section_headers.rs:2152-2441 (tests of `section_payload_roster`)

**Purpose.** Pins stage 14: each of 14 section rows gets its exact payload bytes; procedure fixups and 9 dynamic-table fixups are indexed by section.

**Shape.** Local `struct ImportFixture { object, symbol, version, sites }` :2166-2172 (a fourth import-fixture shape; see below), `IMPORTS` :2174-2187 (alpha at sites `[0, 32]`, beta at `[16]`), helper `headers` :2189 (builds the image by hand, includes a local interpreter-path `match` at :2221-2225), `candidate` :2231; tests :2247, :2277, :2319, :2344, :2387, :2432.

**How it decides (pinned).** `row_count 14`, `dynamic_fixup_count 9`; row 0 empty; row 7 36 bytes; row 11 0 bytes; row 12 320 bytes; row 13 122 bytes (:2250-2256); `rows[i].bytes == upstream_payload(.., row.kind)` and length equals roster `payload_size` (:2257-2267). Procedure fixups: `fixup.target_section_index == procedure_target_section(fixup.target)` (:2294-2297; this is the same function that production code uses per earlier notes, :1907-1917, so the test compares a field to its own producer, inferred); `Section{index, kind}` storage matches the row kind (:2298-2300). Dynamic fixup target section indexes are `[9, 4, 7, 2, 3, 10, 11, 5, 6]` (:2308-2315). Test :2344 flips the first and last byte of every row (or pushes a byte into an empty row) and expects "indexed ELF payload row drifted" (:2369-2374). 15 closures (:2389-2411), including `dynamic_fixups[0].storage_section_index = 9` (:2409).

**Same job elsewhere.** Interpreter-path literals `/lib64/ld-linux-\xfc-x86-64.so.2` and `/lib/ld-linux-\xfb-aarch64.so.1` are repeated (:2222-2223), at lib.rs:692-693 and at dynamic_executable.rs:668-669 (read all three).

**Fail-open.** None seen.

---

## Range 12: section_headers.rs:2982-3183 (tests of `section_roster`)

**Purpose.** Pins stage 13's numeric section roster.

**Shape.** `IMPORTS` = `PAIR_IMPORTS` (:2991), helpers `section_names` :2995, `candidate` :3009; tests :3022, :3052, :3077, :3122, :3157.

**How it decides (pinned).** `section_count 14`, `section_name_table_index 13`, kinds equal `CANONICAL_KINDS` (:3026-3037), `row.index == position` and `validate_name`/`validate_row_against_owner` pass for each row (:3038-3042). 15 corruptions including `section_name_table_index = 10` (:3079-3101). Reference drift (:3122-3154): ten `(kind, field)` pairs are bumped by 1 with `saturating_add` and must reject: link of DynamicSymbol, SystemVHash, GnuHash, GnuSymbolVersion, GnuVersionRequirement, ProcedureRelocation, DynamicTable; info of ProcedureRelocation, DynamicSymbol, GnuVersionRequirement. Row 3 `link = u32::MAX` rejects (:3178-3180).

**Cases by shape.** The ten pairs are the ones `validate_references` checks by name (earlier notes, :2927-2953). Rows not in the list (`.interp`, `.dynstr`, `.plt`, `.got.plt`, `.rela.dyn`, `.shstrtab`) are covered only by the generic `rows[1].link = 2` / `rows[1].info = 1` closures (:3096-3097), which hit `.interp` only.

**Fail-open.** None seen.

---

## Range 13: load_placement.rs:1297-1659 (tests of `load_layout`)

**Purpose.** Pins stage 16's absolute placement: program headers, PT_LOAD congruence, W^X, owned memory, section header resolutions, and the deferred AArch64/x86 constraint envelope.

**Shape.** Helpers `relative` :1315, `relative_with_bss_alignment` :1319 (uses `import_image(target, bss_alignment, 1)`, lib.rs:218), `candidate` :1338 (calls `derive_contents(&relative, target)` and builds the private `Candidate` from its 5-tuple return, :1341-1371); tests :1375, :1425, :1455, :1485, :1502, :1541, :1582, :1640, :1655.

**How it decides (pinned).** Program header kinds in order Interpreter, LoadReadOnly, LoadReadExecute, LoadReadWrite, Dynamic (:1385-1398); `max_page_alignment == 0x1_0000` (:1380); the three loads satisfy `offset % align == va % align`, `paddr == vaddr`, `PF_R` set, not both W and X (:1399-1408); flags exactly `PF_R`, `PF_R|PF_X`, `PF_R|PF_W` (:1409-1411); memory text 32, data 13, bss 23 align 16 (:1412-1415); interpreter and dynamic headers alias the placed sections (:1431-1439); `.shstrtab` is FileOnly, no VA, after the RW load; section-header table after `.shstrtab`, size `14 * 64` (:1440-1451); 25 resolutions, each 8 wide, matching placed VA/offset, template bytes unchanged (:1455-1482). 9 geometry/fingerprint closures (:1504-1520, including `max_page_alignment >>= 1`); 11 ABI-replay closures (:1543-1559) and an explicit data/dynamic overlap case (:1567-1578). AArch64 page delta (:1582-1598): with source `0x2_0000_0000`, `+(2^20-1)` pages covers a 0x1000 extent but not 0x1001; `-2^20` pages covers 1 byte, one page lower does not; extent size 0 returns `false` (production at load_placement.rs:497-499, read). Arm constraint kinds include `Aarch64PageDelta21`, `Aarch64Load64Low12Aligned`, `Aarch64Branch26` (:1609-1621); PLT VA `1<<40` and GOT VA + 1 reject in `validate_deferred_constraint_envelope` (:1623-1636). BSS alignment 0 and 3 reject returning the relative layout (:1640-1651). `checked_align(7, 3, ..)` errors (:1659).

**Unused/one-caller (grep across `omega-rust`).** `aarch64_page_delta_covers_extent` (`pub(crate)`, load_placement.rs:492): 1 production call (:478), 5 test calls (:1586-1598), 1 import (:1307). `validate_deferred_constraint_envelope` (`pub(crate)`, :425): 1 production call (:410), 2 test calls (:1626, :1635), 1 import (:1307). Both are `pub(crate)` only to be callable from the tests module (inferred from the sole non-test caller being in the same file).

**Fail-open.** None seen.

---

## Range 14: load_placement.rs:2239-2508 (tests of `placed_section_headers`)

**Purpose.** Pins stage 17: writing the 25 resolved addresses/offsets into the section-header template.

**Shape.** Helpers `load_layout` :2254 (`import_image(target, 16, 1)` + `import_call_load_layout`, lib.rs:544), `candidate` :2269; tests :2282, :2325, :2349, :2407, :2427, :2464, :2489.

**How it decides (pinned).** Output 896 bytes, 25 applications each width 8, value equals the upstream resolution, upstream template bytes unchanged (:2284-2320). Unchanged-bytes: every byte outside the 25 eight-byte fields equals the template (:2325-2346). Ledger split: 13 FileOffset and 12 VirtualAddress (:2349-2371, counts at :2360, :2370); no application on row 0; `SECTION_NAME_TABLE_INDEX` (13, load_placement.rs:1881) has a FileOffset application and no VirtualAddress application (:2378-2385); decoded row 0 all zero; `.shstrtab` header address 0, file offset non-zero (:2386-2403). 8 closures (:2429-2443). Direct byte tampering: flip a resolved field byte, flip row 1's payload-size byte (offset `ELF64_SECTION_HEADER_SIZE + 32`, asserted not to be inside any application, :2470-2477), truncate, append (:2464-2486).

**Fail-open.** None seen.

---

## Range 15: load_placement.rs:3097-3379 (tests of `resolved_dynamic_table`)

**Purpose.** Pins stage 18: the nine `.dynamic` address fields (`d_ptr`) get absolute virtual addresses and every other byte stays untouched.

**Shape.** Helpers `placed` :3110 (versioned import with caller-chosen symbol), `standard_placed` :3128, `candidate` :3132, `write_application_value` :3147; tests :3155, :3201, :3224, :3253, :3306, :3336, :3370.

**How it decides (pinned).** 9 applications whose targets equal `EXPECTED_TARGETS` (load_placement.rs:2535, read via grep; :3158-3170); each has `storage_section_index == 12`, width 8, `Elf64AbsoluteAddress`, `byte_offset == row_ordinal*16 + 8`, and value equal to the target section's virtual address, also read back from `resolved.bytes()` (:3172-3196). Exactly 72 bytes (9 x 8) are mutable; every other byte equals the upstream `.dynamic` payload and the mutable ones were 0 upstream (:3201-3221). A different imported symbol name changes the fingerprint (:3231-3249). 14 closures including fingerprint = 0 (:3255-3285). Substitution tests (:3306-3333): a sibling target (`sections()[4]` as SystemVHash) with consistent value and bytes rejects; a file-only substitution rejects; a null target (index 0, kind Null, value 0) rejects. Byte tests: flip applied byte, byte-reversed (big-endian) write, tag byte `[0]`, literal `[8]`, final null row byte, truncate, append (:3336-3366).

**Inconsistency inside the test (read).** The "file-only" substitution uses `sections()[12]`, names it `shstrtab`, and sets `target_section_kind = SectionNameTable` (:3319-3323). Index 12 is `DynamicTable` (`DynamicTable = 12`, `SectionNameTable = 13`, load_placement.rs:1108-1109; `SECTION_NAME_TABLE_INDEX = 13`, :1881). So the test substitutes the `.dynamic` section's file offset under the name-table kind; it rejects, but not for being a FileOnly section by index.

**Same job elsewhere.** Hard-coded `12` (storage section index) appears in :3173 as an expectation and as `storage_section_index = 9` mutation at :3269 and in section_headers.rs:2409 (same value 9 as a wrong storage index).

**Fail-open.** None seen.

---

## Range 16: file_assembly.rs:753-1250 (tests of `assembled_dynamic_file`)

**Purpose.** Pins stage 21 (assembly of the complete file from fragments) and stage 22 (admission of the final bytes).

**Shape.** `ResolvedCustodySnapshot` :776-786; helpers `standard_resolved_linkage` :788, `resolved_linkage` :794 (hand-built image: 64-byte text, 13 data bytes `0x50..`, bss 23 align 16, entry via `text_entry(.., 24)`, executable region offset 0 count 28 "relocated-caller", data region offset 0 count 13 "caller-data", two imports of `libassembled-file.so`, call relocations `(0, imports[0]), (8, imports[0]), (16, imports[1])`; then the stage chain to `apply_elf_procedure_linkage_fixups`, :845-853), `candidate` :856, `refresh_fingerprint` :871, `custody_snapshot` :879, `assert_rejected_with_exact_custody` :896, `byte_range` :913; tests :920, :1018, :1052, :1093, :1152, :1171, :1221.

**How it decides (pinned).** 17 fragment placements; file length == section-header-table offset + its bytes; every byte outside fragments is 0; bss contributes no bytes and sits inside the RW load's `memory_size - file_size`; no fragment is for section index 0 (:929-1013). Each fragment mutation (first byte of each non-empty placement, with the fingerprint refreshed) rejects with "fragment bytes do not replay" (:1057-1071); a non-zero gap byte rejects with "padding is not zero-filled" (:1073-1089). Messages asserted: "file length drifted" (:1100, :1109), "ledger coverage drifted" (:1118), "ledger drifted" (:1127), "fingerprint does not replay" (:1158, :1166); kind, offset, byte_count and ordinal edits reject (:1130-1148). Admission (:1171-1218): `admit_elf_dynamic_executable(assembled)` returns an image whose text is the resolved text, data/bss unchanged, `output().bytes == assembled bytes`, `final_text_bytes == resolved text`, `imports == 2`, `relocations == 3`, bytes start with `\x7fELF`, `format` contains "dynamic-executable", executable-region inventory validates against the admitted text but not against the original text, 1 executable region + 1 unclassified gap, 1 data region.

**Final-byte admission is tested only through the test-only function.** `final_byte_admission_rejects_independent_output_drift` (:1221-1247) calls `derive_executable_output` and then `validate_executable_output` directly (:1229-1246), not `admit_elf_dynamic_executable`. Grep of `validate_executable_output` across `omega-rust` (content mode): 1 definition (file_assembly.rs:353), 1 import (:759), 5 calls (:1230, :1234, :1238, :1242, :1246), 0 elsewhere. This agrees with the earlier notes' finding (unit_003.md item 10 fail-open (a)): the verifier that the header calls "re-verifying final-byte admission" runs only under `cfg(test)`. The positive admission test (:1171-1218) checks magic bytes and a format substring, not a replay.

**Same job elsewhere.** The seven-call chain `resolved_linkage -> section_header_table_from_linkage -> plan_elf_indexed_section_payloads -> plan_elf_relative_section_payload_layout -> plan_elf_dynamic_load_layout -> apply_elf_section_header_placements -> apply_elf_dynamic_address_fixups` is written out by hand at file_assembly.rs:845-851, :1805-1811, :3640-3646 (and :2660, outside my ranges), and in production at dynamic_executable.rs:653 (Grep of `apply_elf_dynamic_address_fixups(placed)` found these 5 lines); `test_support` has `import_call_indexed_payloads` (lib.rs:526) and `import_call_load_layout` (lib.rs:544) for the earlier prefix of the chain but none reaches the resolved-table or envelope stage.

**Fail-open.** None seen in tests; see the admission paragraph above for the production gap this range documents.

---

## Range 17: file_assembly.rs:1732-2088 (tests of `dynamic_file_envelope`)

**Purpose.** Pins stage 19: ELF header + 5 program headers prefix, the entry address, and the section header table placement.

**Shape.** `enum EntryFixture {Text{offset}, TextExplicit{offset,size,kind}, Data, Missing}` :1749-1760; `CustodySnapshot` :1763; helpers `resolved` :1769 (`import_image(target, 16, 2)`, entry symbol `_start` inserted by hand, :1786-1793; one call relocation at `(0, imported)`), `standard_resolved` :1814 (entry `Text{offset: 8}` = size 4, `SymbolKind::Function`), `candidate` :1822, `custody_snapshot` :1837, `assert_rejected_with_custody` :1845 (compares upstream fingerprint and a full custody snapshot), `overwrite_u64` :1865 (uses `optimization_core::codec_bytes::patch_encoded`; grep shows that crate path only at file_assembly.rs:1866 and :1968 in image-elf, both in tests); tests :1870, :1927, :1962, :1989, :2020, :2077.

**How it decides (pinned).** Header prefix 344 bytes (= 64 + 5 x 56), section-header table 896; `header.machine` is `EM_X86_64` / `EM_AARCH64` per target; `e_shstrndx` row has type `SHT_STRTAB` (local `const SHT_STRTAB: u32 = 3`, :1885) and `sh_offset` equals the placed `.shstrtab` file offset; `header.entry == envelope.entry_address()`; `section_header_offset` matches; program header types `[PT_INTERP, PT_LOAD, PT_LOAD, PT_LOAD, PT_DYNAMIC]` (:1870-1923). Five invalid entries reject and return the complete upstream carrier through `into_parts()` (:1927-1959): missing entry, entry in Data, entry of kind Object, size 0, and offset 31 with size 4 (past the end of a text smaller than 35 bytes). The loop runs only on `TargetProfile::LinuxX64` (:1947). Substitutions: first header byte, machine field patched to EM_AARCH64 on an x64 candidate, program-header byte, section-header byte 65, truncated prefix (:1963-1985). Self-consistent substitutions (entry + 4 with the header and fingerprint recomputed; section header table offset + 4096 likewise) still reject (:1990-2016). Overlap messages asserted: "overlaps the header prefix" (offset 128), "overlaps a loadable file extent" (offset = prefix size), "fragment end overflows" (offset `u64::MAX - 100`) (:2020-2074). Fingerprint zero or drifted rejects (:2077-2085).

**Cases by shape.** All entry fixtures use text offsets 8 or 31; no fixture has a non-4-aligned entry offset on AArch64 (the 31-offset case is LinuxX64 only). Whether production code checks AArch64 entry alignment was not in my ranges.

**Fail-open.** None seen.

---

## Range 18: file_assembly.rs:3559-3947 (tests of `resolved_procedure_linkage`)

**Purpose.** Pins stage 20: applying the final PLT/GOT/RELA fixups and the image-local text relocations.

**Shape.** Helpers `standard_envelope` :3581, `envelope` :3589 (hand-built image: 64-byte text, 13 data bytes `0x5a`, bss 23 align 16, entry at 24, two `libresolved-procedure.so` imports, three call relocations, plus `general_slots: &[(usize, i64)]` of `Absolute64` width-8 Data relocations on `imports[0]`, :3626-3638, then the stage chain to `serialize_elf_dynamic_file_envelope`, :3640-3647), `candidate` :3650 (`standard_envelope`, i.e. no general slots), `refresh_fingerprint` :3662, `assert_rejected_with_custody` :3670 (checks only the upstream envelope fingerprint and a non-empty diagnostic; the other two files' versions of this name also compare messages or full snapshots); tests :3686, :3734, :3783, :3820, :3848, :3879.

**How it decides (pinned).** Applied fixups: 14 on LinuxX64, 16 on LinuxArm64, ordinals `0..n`, every one of SourceText / ProcedureLinkage / ProcedureGot / ProcedureRelocation present, source text changed, retained image text unchanged (:3686-3731). One general slot `(0, 7)`: `.rela.dyn` is exactly 24 bytes with `r_offset == data_virtual_address`, `r_info == (1<<32)|type` (type 1 on x64, 257 on arm), `r_addend == 7`; one `GeneralRelocation` fixup of kind `Elf64RelaOffset`, byte_offset 0, width 8, target `RelocatedImageSection{Data, 0}` (:3734-3779). Corruptions (all with refreshed fingerprint, :3820-3876): `source_text_bytes[1]` (mutable field), `[0]` (opcode), `applications[0].target_address + 4`, `source_address + 4`, `kind = Absolute64`, pop, duplicate, swap, `procedure_linkage_bytes.pop()`; fingerprint 0 and ^= 1. `encode_field` error cases (:3879-3939): x86 PC-relative past `i32`; Branch26 misaligned target (`0x1002`) and out of range (`1<<27`); Page21 distance `1<<32`; Load64Low12 with value 3 (not 8-aligned); Absolute64 into a 16-bit mask with `1<<20`; two overlapping applications reject (:3941-3944).

**Gap in what is exercised (grepped).** `general_relocation_bytes` appears in file_assembly.rs only at :602, :2109, :2146, :2966-2967, :3378, :3399 and :3745 (Grep, content mode). In the tests the only use is the read at :3745. No test mutates `general_relocation_bytes` through `validate_candidate`, and `candidate()` (:3650) is built without general slots. This corroborates the earlier notes' open fail-open item (unit_003.md item 10 (b)): the replay check on `.rela.dyn` bytes outside the fixup-owned words has no negative test.

**Fail-open.** None seen in this range; see the paragraph above for what tests do not exercise.

---

## Duplicated test fixtures (read + grepped, whole image-elf/src)

- Interpreter paths `/lib64/ld-linux-\xfc-x86-64.so.2` and `/lib/ld-linux-\xfb-aarch64.so.1`: `dynamic_executable.rs:666-672` (`#[cfg(test)] fn interpreter_path`), `lib.rs:690-696` (`import_fixture_interpreter_path`), inline `match` at `section_headers.rs:2221-2225`, plus two local wrappers `fn interpreter_path` at import_sections.rs:1014 and :2949 that delegate to lib.rs. Grep of `fn interpreter_path|fn import_fixture_interpreter_path` found these (elf_loader.rs:50 is an unrelated method).
- Import fixture shapes: `ImportFixture` (lib.rs:589; `DynamicImportFixture` is a type alias for it, lib.rs:312), `SingleSiteImportFixture` (lib.rs:390), and a local `ImportFixture` struct in section_headers.rs:2167-2172. Import lists: `IMPORTS` (3 entries, lib.rs:596), `PAIR_IMPORTS` (2 entries, lib.rs:617), `SINGLE_SITE_IMPORTS` (4 entries, lib.rs:396), a local 3-entry `IMPORTS` at import_sections.rs:1822 (note: another test module in the same file imports `SINGLE_SITE_IMPORTS as IMPORTS` at :1011 and :2946), a local 2-entry one at section_headers.rs:2174. `const IMPORTS` definitions found by Grep: dynamic_table.rs:306/:780/:1751, lib.rs:596, procedure_linkage.rs:1609, import_sections.rs:1822, section_headers.rs:882/:1462/:2174/:2991.

## Stated limits (this unit)

None of the 18 ranges contains a comment stating a limit. The test names carry statements ("non_runnable_envelopes" file_assembly.rs:1870; "twenty_five_zero_fixups" section_headers.rs:919); no quotes needed.

## Fail-open risks (this unit)

None seen in the ranges (tests assert rejection). The risks recorded above are what the tests do not exercise: no negative test for `.rela.dyn` bytes (file_assembly.rs:3650 candidate has no general slot, grep shows `general_relocation_bytes` read once in tests at :3745); final-byte admission verifier is called only by tests (file_assembly.rs:1230-1246). Both agree with the earlier notes.

## Disagreements with the earlier notes (unit_003.md)

- unit_003.md item 6 says the descriptor stage uses a "hard-coded 113-byte..." (cut off) seed. This pass: seed is 79 bytes (import_sections.rs:1888, procedure_linkage.rs:1663-1670); 113 is the total after the four linkage names are appended (procedure_linkage.rs:1673). Other numbers in the earlier notes that I could check from tests agree: 14 rows (section_headers.rs:3026), 25 fixups (13+12; load_placement.rs:2360, :2370), 122-byte `.shstrtab` (section_headers.rs:1500), 896-byte section header table (section_headers.rs:924), `.rela.dyn` before `.rela.plt` physically (section_headers.rs:269-271 read), `link`/`info` literals 2/3/9 (section_headers.rs:933-938), dynamic fixup count 9 (load_placement.rs:3158), storage section 12 (load_placement.rs:3173).
- unit_003.md "Unread ... (tests)" with the caveat that not every line was confirmed as test code: confirmed here. Each range is inside a `#[cfg(test)] mod tests` body, plus the closing braces of the enclosing module and (range 6) the `#[cfg(test)]` attribute line.

---

## Coverage

- **Read in full:** all 18 ranges, 5,728 lines: import_sections.rs 140-339 (200), 1002-1301 (300), 1807-2145 (339), 2937-3294 (358); procedure_linkage.rs 581-1128 (548), 1590-1954 (365), 2959-3248 (290); section_headers.rs 323-514 (192), 868-1156 (289), 1444-1638 (195), 2152-2441 (290), 2982-3183 (202); load_placement.rs 1297-1659 (363), 2239-2508 (270), 3097-3379 (283); file_assembly.rs 753-1250 (498), 1732-2088 (357), 3559-3947 (389).
- **Skimmed:** none (0 lines).
- **Not read:** none (0 lines).
- Extra context lines read outside my ranges (do not count): import_sections.rs 60-139 and 985-1001; imports.rs 92-128; lib.rs 686-697; dynamic_executable.rs 662-676; section_headers.rs 266-301; load_placement.rs 488-513; procedure_linkage.rs 560-580.

## Five things a maintainer most needs to know about this unit

1. All 5,728 lines are test code. Every stage tests itself with the same recipe: derive, hand-build `Candidate`, corrupt one field, expect rejection with the upstream carrier handed back intact (21 corruption lists in the crate, 16 in these ranges; 27 per-file helper definitions, none shared).
2. The tests pin exact golden numbers that are repeated as literals in production code: 14 section rows, 25 fixups (13 FileOffset + 12 VirtualAddress), 9 dynamic address fixups, storage section index 12, 344-byte header prefix, 896-byte section header table, 122-byte `.shstrtab`, 79-byte then 113-byte name seed (section_headers.rs:924-925, load_placement.rs:2360/:2370/:3173, file_assembly.rs:1877-1878, procedure_linkage.rs:1673).
3. The only test of final-byte admission calls the test-only `validate_executable_output` directly (file_assembly.rs:1230-1246; grep: 1 definition, 5 calls, all in the file's tests). The production admission test checks the magic bytes and a "dynamic-executable" substring (file_assembly.rs:1194-1195).
4. No test mutates `.rela.dyn` bytes (`general_relocation_bytes`) through `validate_candidate`; the stage-20 candidate has no general slot (file_assembly.rs:3585, :3650, grep of the field).
5. Test fixtures are duplicated: three interpreter-path functions plus an inline match, three import-fixture types, several local `IMPORTS` constants, and the seven-stage chain from linkage to resolved dynamic table is rewritten by hand in three tests (file_assembly.rs:845-851, :1805-1811, :3640-3646). One test mislabels index 12 as the section-name table (load_placement.rs:3319-3323).

## What I am unsure about

- Whether `TargetProfile` to `NativeTarget` is one-to-one; it decides whether the profile check at import_sections.rs:116-129 (no test asserts its message) can ever fire after :78 and imports.rs:120-125.
- Whether the production `derive_contents` in section_payload_roster uses `procedure_target_section` to fill `target_section_index` (I inferred that from the earlier notes; the test at section_headers.rs:2294-2297 only shows the equality holds).
- Whether `validate_candidate` for `.dynsym` rows would reject changes to `info/other/shndx/value/size` bytes: no corruption in import_sections.rs:1195-1244 touches them, and I did not read the decoder (outside my ranges).
- Whether the missing AArch64 entry-alignment fixture (file_assembly.rs:1931-1945, LinuxX64 only) reflects a missing production check; I did not read `expected_entry_address` in this pass.
