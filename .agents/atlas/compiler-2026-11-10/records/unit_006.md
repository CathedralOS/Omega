# Unit 6 notes

Files covered (32), all under `omega-rust/omega/compilation/backend/`:
images/image-macho/src/{dyld_linking,file_layout,lib,shared_component}.rs;
images/image-pe/src/{headers,imports,lib,sections,shared_component}.rs;
images/image/src/{builder,final_image,lib,output,relocation_envelope,relocations}.rs;
images/image/tests/{common/mod,initialized_data_envelope,relocation_rejections,text_relocation_envelope}.rs;
instruction-set-architectures/encoded-scalar-plan/src/{harness,lib}.rs;
instruction-set-architectures/isa-aarch64/src/{floating_control,frame_protocol,generated_entry,hosted_sequences,immediate_offset,lib,machine_effects,post_handoff_writer,preservation_storage,register_model,saturating_forms}.rs

(Paths below are shortened: `macho/` = images/image-macho/src/, `pe/` = images/image-pe/src/, `image/` = images/image/src/, `aarch64/` = instruction-set-architectures/isa-aarch64/src/.)

## macho/dyld_linking.rs (2626 lines, read in full)

**Purpose.** Everything dyld-facing for Mach-O: turning import symbols into 12-byte (AArch64) or 6-byte (x86-64) thunks that jump through an 8-byte `__DATA` pointer slot, encoding the bind and rebase opcode streams, writing every load command, and re-reading the finished container bytes to check they match what was planned. Header comment at 1-7.

**Shape.** Five `pub(crate) mod`s plus one free fn.
- `imports` (9-975): `MachoImportThunk {symbol, bind_symbol, text_offset, data_offset, library, dylib_ordinal}` (25), `InstalledMachoImports {thunks, dylibs}` (35), private `PreparedMachoImport` (41). Entry fns: `install_import_thunks` (75), `patch_import_thunks` (271), `validate_import_thunk_footprints` (337), `macho_bind_info_for_segment` (387), `validate_macho_import_binding_pairing` (480, pub), `thunk_bound_pointer_address` (540). Helpers `plan_dylibs` (55), `ensure_dylib` (49), `patch_aarch64_adrp` (420), `patch_aarch64_ldr_x_from_page` (443).
- `load_commands` (977-1485): submods `dynamic_linking` (`MachoDylib {path: Cow<[u8]>, timestamp, current_version, compatibility_version}` 1018, consts LIBSYSTEM/LIBOBJC/FOUNDATION/APPKIT/COREGRAPHICS 1035-1051, `from_install_name` 1057), `header` (1131), `linkedit` (1213), `segments` (1314); re-exports 1468-1484.
- `loader_fixups` (1487-1884): `MachoRebasePointer`, `MachoImportPointer` (1496, 1502), `validate_macho_object_fixups` (1508, pub), `Stream` byte reader (1641), `validate_macho_loader_fixups_for_segment` (1700).
- `loader_mapping` (1886-2299): `Segment` (1927), `validate_macho_loader_mapping` (1972, pub), `validate_macho_container_mapping` (1992), `validate_section` (2264).
- `rebases` (2301-2612): `MachoRebaseInfo {bytes, sites}` (2317), `macho_rebase_info_for_segment` (2372), `validate_patched_preferred_pointers` (2329).
- `write_uleb128` (2614).

**Inputs/outputs.** `install_import_thunks` reads `image.relocation_table`, `image.symbol_table.{symbols,imports}`, mutates `image.memory.{text,data}`, `image.executable_regions`, `image.data_regions`, and the import symbols' section/offset/size (233-236). It appends thunk text and an 8-byte data slot per *referenced* import (183-191, 214-266). `loader_mapping`/`loader_fixups` only read finished bytes (`bytes: &[u8]`) and return `Result<(), Diagnostic>`.

**How it decides.**
- Which imports get a thunk: keyed on symbol-handle identity: an import is kept iff some relocation's `symbol_handle` equals it (`referenced.contains(&import.symbol_handle)`, 183). Unreferenced imports are validated (kind, plan) but never thunked.
- Dylib roster: de-duplicated by exact install-name byte equality (`ensure_dylib` 49-53); ordinal = position+1 in roster with libSystem pinned to ordinal 1 (56). Legacy ObjC registration is keyed on `library == DARWIN_LIBOBJC_PATH` (124), which adds Foundation, AppKit, CoreGraphics (61-65). Cap: >15 dylibs -> error (66).
- Bootstrap library choice for `StringBackedBootstrap` imports is keyed on source spelling: `darwin_import_library(&symbol.name)` (117) from the symbol name string.
- Normalized import identity collision keyed on `identity_digest()` (159); target compatibility keyed on `locator.target() != isa.target_profile()` and `native_target() != image.target` (147-149).
- Thunk canonical-form check by opcode mask: `adrp & 0x9f00_001f == 0x9000_0010 && ldr & 0xffc0_03ff == 0xf940_0210 && br == 0xd61f_0200` (358-360), repeated at 558-560. x86-64 uses `isa_x86_64::decode_x86_64_import_thunk` (362, 582).
- Thunk/slot pairing (480-538): matches by `symbol` string equality plus decoded address plus byte_count==8 plus section offset arithmetic; requires exactly one match each way.
- Container mapping validator (1992-2261) is a positional decode of the load-command table: segments matched by their 16-byte name (2031-2037), load command kinds by number (2081-2091); unknown command kind -> error (2090). Each command kind allowed once (`singletons` bitmask 2018, 2094).

**Cases by shape.**
- `MachoIsa` only Aarch64/X86_64 (see lib.rs). Import plan match handles `StringBackedBootstrap`, `Normalized(Foreign|Component)`, `None` (115-182); non-Mach-O foreign locator rejected (135-139).
- `loader_mapping` supports exactly: one `__TEXT` with one section (`text.sections.len() != 80`, 2141), optional `__DATA` with at most `__data` + `__bss` (2188), `__LINKEDIT` with none (2253). Anything else -> `invalid()`. Dylib commands cap 15 (`dylibs: [Option<&[u8]>;15]`, 1718).
- `loader_fixups` assumes exactly the emitter's stream grammar: rebase stream is `SET_TYPE(0x11)`, then per pointer `0x20|seg`, ULEB offset, `0x51`, then `0x00` (1809-1823); bind stream per import `0x10|ordinal`, `0x40`, symbol, `0x51`, `0x70|seg`, ULEB, `0x90` (1830-1848). It cannot validate other opcodes; any other byte -> `invalid()`.
- `macho_bind_info_for_segment` emits `0x10 | ordinal` (404): dylib ordinal immediate form only, hence the 15 cap.
- Build version command is hard-coded: platform 1 (macOS), minos/sdk `13 << 16` (1006-1015).
- UUID is a fixed 16 byte constant ("Omega-direct-m01", 1182-1185).

**Same job elsewhere.**
- ULEB128 reading is done three times: `Stream::unsigned` (1659), `exports::unsigned` in macho/shared_component.rs:139, and writer `write_uleb128` (2614). Readers differ: `Stream::unsigned` loops `(0..64).step_by(7)` and rejects payload overflow with `payload > (u64::MAX >> shift)` (1664); `exports::unsigned` loops `(0..70)` and rejects with `shift == 63 && byte & 0x7e != 0` (shared_component.rs:146). (read)
- Thunk opcode decode appears twice in the same file: 355-361 and 555-566 (read).
- `region`/`word`/`wide` byte readers live in `loader_mapping` (1897-1917) and are reused by `loader_fixups` (1493) and shared_component.rs:356. `image::codec_bytes` and `crate::codec_bytes` also provide write helpers (file_layout.rs:9).
- Segment/section name comparisons by literal 16-byte arrays appear in `loader_fixups` (1729, 1734) and `loader_mapping` (2032-2035, 2156-2157, 2197-2198, 2213-2214).
- Suspected: PE import thunk validation does the same job for PE (pe/imports.rs, read below).

**Unused / one-caller code (grepped).**
- `validate_macho_import_binding_pairing`, `validate_macho_object_fixups`, `validate_macho_loader_mapping`: grep over `omega-rust` `*.rs` finds each called from `image-emission/src/image.rs` (lines 843, 851, 858) plus in-crate tests in macho/lib.rs (351, 374, 461). So 1 non-test caller file each (grepped, command: Grep pattern of the five names).
- `validate_macho_container_mapping`: callers macho/lib.rs:876 and dyld_linking.rs:1980 (2 sites, both in-crate).
- `validate_macho_loader_fixups_for_segment`: callers lib.rs:893 and dyld_linking.rs:1628 (2).
- `MachoDylib::LIBOBJC` (1041) is used only inside `from_install_name`'s known list (1060) and a test (1111); not otherwise referenced in this file (read).

**Stated limits.** `plan_dylibs`: "the current canonical bind encoding supports at most 15" (68). `macho_bind_info_for_segment` / rebase: segment index must fit "four-bit immediate" (392, 2377).

**Fail-open risks.**
- `validate_macho_loader_fixups_for_segment` 1771-1777: if there is no `LC_DYLD_INFO_ONLY` (0x8000_0022) command and both `rebases` and `imports` are empty it returns `Ok` without examining the bytes. Wrong program that gets through: none by that route, because the caller supplies the expected lists; with empty lists no fixups are required. traced (callers lib.rs:893 and 1628 build the lists from the image).
- `loader_fixups` 1764 `_ => {}` ignores unknown load commands; `loader_mapping` rejects unknown ones (2090). So the two validators disagree on unknown commands; in the emit path the mapping validator runs first (lib.rs:876), so unknown commands cannot arrive there. traced.
- `validate_macho_object_fixups` 1520-1527 `referenced_imports` filters by "any relocation names this import"; an import with no relocation gets no pointer expected. Matches `install_import_thunks` 183. stated by symmetry (no comment).
- `loader_mapping` 2005-2006 header flags check is exact equality (0x20_0085 / 0x85), so unknown flags are rejected.
- `install_import_thunks` 193-210: a relocation whose `symbol_handle` is not valid is skipped (`continue`, 194-200) rather than rejected. A text relocation with an invalid handle therefore passes this stage; later `apply_relocations` is the stage that consumes handles (not read in this unit's Mach-O files; image/relocations.rs read below). open.

## macho/file_layout.rs (313 lines, read in full)

**Purpose.** Computes, in one pass, every offset and size for the Mach-O file: load-command bytes, `__TEXT` offset, page-aligned `__DATA`, bss address, `__LINKEDIT` pieces (rebase, bind, export trie, code signature). Header comment 1-6.

**Shape.** `bytes` (8): `write_be_u32/u64`, `write_fixed_string_16` (asserts <= 16 bytes, 21), `write_reserved_u32s`; re-exports `image::codec_bytes::{write_u32,write_u64}`. `constants` (34): load-command sizes and header flags. `layout` (53): `alignment_power`, re-export of `image::{align_to, align_to_u64}`. `plan` (61): `MachOImagePlan` (75-97, 21 fields), `macho_data_segment_index` (109), `plan_macho_container` (118).

**Inputs/outputs.** Reads `image.memory.{text,data,bss_size,bss_alignment}`, the dylib roster, the code-signature identifier, `MachoIsa`, optional dylib identity; calls `code_signature_size` (194). Pure: returns a plan.

**How it decides.** `has_dyld_info = rebase_size > 0 || has_imports` (131); `has_data_segment` is true iff data non-empty or bss_size>0 (132-134). Command count is `7 + root_commands + dylibs.len() + data + dyld_info` (146-150), and `sizeofcmds` is a parallel sum (151-162). The dylib identity (`Option<&MachoDylib>`) decides executable (3 root commands) vs dylib (2) (136-145). `macho_data_segment_index` returns `Some(pagezero + 1)` (113-114).

**Cases by shape.** Exactly executable (`MH_EXECUTE`) or dylib; the `7` in the command count is a literal bundling TEXT, UUID, build version, symtab, dysymtab, LINKEDIT, code signature (146); not named.

**Same job elsewhere.** The number of commands and their sizes is also enforced independently by `validate_macho_container_mapping` (dyld_linking.rs:2081-2091, `expected_size` table). Two tables of command sizes: constants here and literals there (read).

**Unused / one-caller.** `plan_macho_container`: callers lib.rs:744, 758 (2 non-test) (grepped via Read of lib; the Grep for the name was not run separately; count from reading lib.rs). `MACHO_HEADER_SIZE` is `pub(super)` (36).

**Stated limits.** none.

**Fail-open risks.** none seen (pure arithmetic; overflow panics via `expect`, 178, 198, 202, i.e. fails closed by panic).

## macho/lib.rs (911 lines, read in full)

**Purpose.** Crate entry. Emits a PIE, ad-hoc-signed Mach-O executable or dylib from a `FinalImage`, in a fixed order, then re-reads its own output through the validators.

**Shape.** `emit_macho_executable` (671) -> `emit_macho_executable_signed` (675) -> private `emit_macho_container_contents` (714), returning `MachoContainerContents {image, bytes, layout, executable_regions, data_regions}` (706). `mod isa` (92): `MachoIsa {Aarch64, X86_64}` (97) with methods cpu_type, page_size (0x4000 vs 0x1000), import_thunk_size (12 vs `isa_x86_64::X86_64_IMPORT_THUNK_BYTE_COUNT`), entry_alignment, `thunk_footprint`. `macho_isa_for` (102). `mod entry_symbol` (55). `OMEGA_EXECUTABLE_LEAF = "omega-program"` (669). Re-exports (647-664).

**Inputs/outputs.** In: `FinalImage` by value, signing identifier, isa, optional dylib identity, export roster. Out: `ExecutableImageOutput` with `final_text_bytes`, `final_data_bytes`, bytes, region inventories (687-704); `final_import_data_bytes: Vec::new()` and `import_data_regions: PlacedDataRegionInventory::empty()` always (691, 703).

**How it decides.** ISA keyed on `(object_format, architecture)` pair (116-119). Executable vs dylib keyed on `identity.is_some()`, and the code asserts `identity.is_some() == !exports.is_empty()` (721). Wrong-target check (731). Layout computed twice for dylibs because the export-trie size changes `sizeofcmds`; second plan must keep `text_offset` and `data_segment_index` (756-774).

**Cases by shape.** Target must be macOS arm64 or x64 (103-107). Executable entry: `plan.text_offset + macho_entry_text_offset` (776); dylibs have none.

**Same job elsewhere.** ISA-from-target mapping `MachoIsa::from_format` also used in shared_component.rs:312. The identical "single-use `isa`/`target_profile`" mapping for PE is `pe/lib.rs` (read below).

**Unused / one-caller.** `emit_macho_executable` and `emit_macho_executable_signed` (671, 675): see the count line after the macho/shared_component.rs entry; both have callers in image-emission (not read). `OMEGA_EXECUTABLE_LEAF` (669) is also hard-coded as the `file_name` of every executable output (693) regardless of the signing identifier passed.

**Stated limits.** Header comment 5-7 says imports bind "through `__DATA,__got` pointer slots". The code names the section `__data` (dyld_linking.rs:1438) and no `__got` section is emitted anywhere in the Mach-O files (read). The comment does not match the emitter.

**Fail-open risks.** none seen; it ends with `validate_macho_container_mapping` (876) and `validate_macho_loader_fixups_for_segment` (893) on the produced bytes.

## macho/shared_component.rs (635 lines, read in full)

**Purpose.** Emits an `MH_DYLIB` that exports selected text functions through an export trie, and re-reads the trie to verify names and offsets.

**Shape.** `MachoComponentExport {symbol, name: Vec<u8>, slot_commitment: [u8;32]}` (17), `MachoSharedComponentImage` (24), `MachoSharedComponentEmissionError` (47, via `image::rejection_carrier!` 53), `mod exports` (59): `encode` (73), `validate` (160), `unsigned` (139). `selected_addresses` (251), `export_trie` (292), `emit_macho_shared_component` (303), `validate_export_trie` (350).

**Inputs/outputs.** Reads `FinalImage` and the export roster; produces dylib bytes and a final text copy. On error returns the whole original image and exports (342-346).

**How it decides.** Export acceptance keyed on: non-empty name without NUL, non-zero `slot_commitment`, no earlier export with the same name OR same commitment, valid symbol handle (258-265); symbol must be `Function` in `Text`, size>0, in range (271-278). Install name is `@rpath/libomega-component-<hex>.dylib` where the hex is a SHA-style digest of target name + sorted (name, slot_commitment) pairs (319-331). The trie is built by byte-wise prefix tree (73-137) with offsets iterated to a fixed point (122-133, bound `nodes*10+1`).

**Cases by shape.** Only regular symbols: terminal is `flags=0` then address (105-106); validate rejects any other flags (184). Child count is one byte (112).

**Same job elsewhere.** `exports::unsigned` duplicates `loader_fixups::Stream::unsigned` (see above). The PE shared component (pe/shared_component.rs) does the same job of building an export table (read below).

**Unused / one-caller.** `emit_macho_shared_component` and the `MachoSharedComponentImage` accessors: counted in the grep line after this entry (callers exist in image-emission/src/image.rs and object.rs, not read).

**Stated limits.** none besides error strings ("outside the supported byte extent", 78).

**Fail-open risks.** `validate_export_trie` 408 `0x1d if signature.replace(..).is_some()`: only duplicates are rejected; absent signature is rejected at 427. `fixup_end.or(linkedit)` (421-423): if no dyld-info command exists the start of the trie falls back to the LINKEDIT file offset. Fail-closed otherwise (gap bytes must be zero, 428-438). none seen.

(Count for the macho exports/emit symbols: Grep count of `emit_macho_executable(_signed)?|emit_macho_shared_component|MachoSharedComponentImage|MachoComponentExport` over `*.rs` in omega-rust: 47 matches in 6 files; outside image-macho: image-emission/src/object.rs 3, image-emission/src/image.rs 2, native-artifact/src/package_publication.rs 1, products/.../package_path_tests.rs 1. So both emit entry points have non-test callers in image-emission.)

## pe/headers.rs (129 lines, read in full)

**Purpose.** Byte serializers for the DOS stub, COFF + PE32+ optional header and 40-byte section headers.

**Shape.** `PeHeaderInput` (12, 17 fields), `write_dos_header` (32), `write_pe_headers` (39), `write_section_header` (106).

**Inputs/outputs.** Appends to `Vec<u8>`; reads only the `PeHeaderInput` and constants.

**How it decides.** DLL characteristics: `0x0100 | (has_reloc ? 0x0040 : 0)` (71): NX_COMPAT always; DYNAMIC_BASE iff a `.reloc` section exists. Machine is fixed `0x8664` (41). Data directory slots 0 (export), 1 (import), 5 (reloc), 12 (IAT) are filled, others zero (80-103). OS/subsystem versions fixed at 6.0 (60-65). Stack/heap reserve/commit are literals 0x100000/0x1000 (73-76).

**Cases by shape.** Only x86-64 PE32+ (`0x20b`, 49). Section name truncated silently to 8 bytes (117-118): `name.as_bytes()` copy of `min(len, 8)`.

**Same job elsewhere.** Mach-O header writer `write_macho_header_for` (dyld_linking.rs:1188) is the analogue.

**Unused / one-caller.** All `pub(crate)`; sole caller `emit_pe_x86_64` (lib.rs:247-310) (read).

**Stated limits.** none. **Fail-open risks.** `size_of_code as u32`, `section_count as u16` casts (42, 52) truncate silently if values exceed range; reachable only for >65535 sections (not possible here, max 5) or >4 GiB text. open (not traced).

## pe/imports.rs (624 lines, read in full)

**Purpose.** Installs 6-byte `jmp [rip+disp32]` thunks at the end of `.text` for every import row, builds the `.rdata` import directory (descriptors, ILT, IAT, DLL names, hint/name table), patches thunk displacements to IAT slots, and re-checks that thunk opcodes survived.

**Shape.** `PeImportLookup {ByName(Vec<u8>), ByOrdinal(u16)}` (24), `PeImportThunk {symbol, library, lookup, text_offset}` (30), `install_import_thunks` (37), `PeImportTable {bytes, iat_table_offsets, import_directory_rva, import_directory_size, iat_rva, iat_size}` (184), `build_import_table` (193), `patch_import_thunks` (295), `validate_import_thunk_footprints` (319).

**Inputs/outputs.** `install_import_thunks` reads `image.symbol_table.{symbols,imports}`; appends `ff 25 00 00 00 00` to `image.memory.text` (156), rewrites the import symbol as Text/offset/size 6 (158-161), pushes an `ImportThunk` executable region (163-170). `build_import_table` returns bytes plus per-import IAT offsets. `patch_import_thunks` writes disp32 into text (313).

**How it decides.**
- Library name for `StringBackedBootstrap`: if the carried `library` string is empty it is chosen by `calling_conventions::windows_import_library(&symbol).unwrap_or("KERNEL32.dll")` (82-86), i.e. keyed on the symbol's spelling with a KERNEL32 default.
- Normalized foreign locators accepted only when `locator.target() == WindowsX64`, `native_target() == image.target` and subsystem not in `10..=12` (93-96); PE locators handled by name or ordinal; ELF and Mach-O locator cases rejected (109-120). Component locators: target check only (126-128).
- Libraries grouped by exact byte equality of the library name (207). Imports are NOT filtered by whether any relocation references them: every import row gets a thunk (49-178). (Mach-O filters by reference, dyld_linking.rs:183.)
- Thunk validation checks only the first two bytes equal `[0xff, 0x25]` (340); the displacement bytes are not checked here.

**Cases by shape.** Target must be x86-64 + COFF (41-43). Hint/name entries use hint 0 (280). ILT and IAT both carry the same lookup value (269-270).

**Same job elsewhere.** `install_import_thunks` / `patch_import_thunks` / `validate_import_thunk_footprints` repeat the Mach-O trio in macho/dyld_linking.rs:75, 271, 337 for a different container. Duplicate-row check, unresolved-zero-width-symbol check and invalid-library check are written out again in each (PE 50-73 vs Mach-O 92-114). The Mach-O version also decodes thunk bytes at pairing time (`validate_macho_import_binding_pairing`, 480); the PE crate has no equivalent re-decode of the final bytes in this unit (read; image-emission/src/image.rs not read, suspected to hold the PE pairing check).

**Unused / one-caller.** All `pub(crate)`; callers are lib.rs:183-206 only (read). `PeImportThunk.symbol` is used for region matching (353).

**Stated limits.** header comment 5-6: "re-verify that every thunk's `ff 25` opcode survived". The check covers 2 bytes only (340).

**Fail-open risks.**
- 83-84 `windows_import_library(&symbol).unwrap_or("KERNEL32.dll")`: a bootstrap import with empty library and a name the catalog does not know is silently bound to KERNEL32.dll; a wrong program would load with an unresolved import failing at load time rather than at compile time. traced to: StringBackedBootstrap library comes from `symbol.import_library` (image/src/builder.rs:90-92); empty if the object symbol had none. Not traced further.
- 302 `thunks.iter().zip(table.iat_table_offsets.iter())` truncates silently if lengths differ; both are built from the same `imports` vector (193-293 sizes `iat_table_offsets` to `imports.len()`), so equal in practice. traced.
- 305-306 `iat_address as i64 - next_instruction as i64` no overflow guard before `i32::try_from`; addresses are bounded by `IMAGE_BASE + u32`, so safe. traced.
- 95 only the Foreign locator branch rejects UEFI subsystems (10..=12); the `StringBackedBootstrap` and `Component` branches do not, so an import-bearing EFI image is not rejected here. (Test at lib.rs:412 only covers the import-free EFI case.) open.

## pe/lib.rs (575 lines, read in full)

**Purpose.** PE32+ emitter: runs the eight ordered steps in the header comment and writes the file.

**Shape.** modules `bytes` (48), `constants` (83), `entry_symbol` (94), `layout` (139); `PeImageKind {Executable, SharedComponent{exports}}` (166); `emit_pe_x86_64_executable` (171) -> `emit_pe_x86_64` (178). Constants: IMAGE_BASE 0x1_4000_0000, SECTION_ALIGNMENT 0x1000, FILE_ALIGNMENT 0x200, TEXT_RVA 0x1000 (83-92).

**Inputs/outputs.** In: `FinalImage`, subsystem u16, kind. Out: `ExecutableImageOutput` (343) with `final_import_data_bytes: import_table.bytes` and `import_data_regions` placed over the `.rdata` bytes with IAT slots as `ImportBindingSlot` regions (210-229). file_name `omega-program.exe` for both kinds (349) and format `pe64-x86_64-executable` (350), even for the DLL kind.

**How it decides.** `file_characteristics` 0x0022 (executable) or 0x2022 (DLL) (235-238). Entry RVA for executables via `final_image_entry_symbol` (232); 0 for DLLs (233). Optional sections `.rdata/.data/.reloc/.bss` written only when their `has_*` flag is true (279-322). Subsystem is passed through unchanged (264); `emit_pe_shared_component` passes 3 (console) (pe/shared_component.rs:74).

**Cases by shape.** One `.text`, then the fixed order text, rdata, data, reloc, bss. No `.idata`/`.edata` section: both directories live in `.rdata` (header comment 23-31). Export directory placed after the import table, 4-byte aligned (189, 198).

**Same job elsewhere.** Mach-O equivalent is `emit_macho_container_contents` (macho/lib.rs:714) (read). Emission-then-re-verify pattern shared; PE re-verifies only thunk opcode and footprint (206), Mach-O replays the whole container (macho/lib.rs:876-902).

**Unused / one-caller.** `emit_pe_x86_64_executable` and `emit_pe_shared_component`: Grep count `emit_pe_x86_64_executable|emit_pe_shared_component|PeComponentExport` = 31 matches in 4 files: image-pe/src/shared_component.rs 14, image-pe/src/lib.rs 14, image-emission/src/image.rs 1, image-emission/src/object.rs 2. So non-test callers exist in image-emission.

**Stated limits.** none. **Fail-open risks.** Output is not re-read after writing (no PE mapping replay in this crate); correctness of headers/section table rests on `plan_pe_sections`. The `as u32`/`as u16` casts in headers/sections are unchecked (see sections). open.

## pe/sections.rs (256 lines, read in full)

**Purpose.** One-pass PE section layout (`PeSections`) and base-relocation (`.reloc`) block builder.

**Shape.** `PeSections` (15), `final_image_layout` (41), `rdata_rva_after_text` (50), `plan_pe_sections` (54); `relocations::build_base_relocations` (167).

**How it decides.** `.reloc` contains one DIR64 entry for every `Absolute64` relocation whose section is Text or Data (176-183); Bss/None relocations are dropped without error (181 `return None`). Entries sorted, grouped by 4 KiB page, padded to an even count with an ABSOLUTE entry (200-206). `has_data = !data.is_empty()`, `has_bss = bss_size > 0`, `has_rdata = rdata_virtual_size > 0` (62-64).

**Cases by shape.** Fixed order text/rdata/data/reloc/bss with each placed at the next 0x1000 boundary (50-61). Only DIR64 entry type (160).

**Fail-open risks.**
- 181 an `Absolute64` relocation with section Bss or None is omitted from `.reloc`. Wrong program that gets through this function: an absolute pointer in bss would run unrelocated under ASLR. traced: `apply_relocations` runs next (pe/lib.rs:205) and rejects Bss (`image/src/relocations.rs:59-63`, test relocation_rejections.rs:331) and None/invalid handles, so such an image does not emit.
- 183, 51, 57, 60, 61, 86: `as u32` on `relocation.offset` and lengths unchecked. open.

## pe/shared_component.rs (320 lines, read in full)

**Purpose.** DLL (component) emission wrapper plus the PE export directory (`IMAGE_EXPORT_DIRECTORY`) builder.

**Shape.** `PeComponentExport {symbol, name, slot_commitment}` (19), `PeSharedComponentImage` (26), `PeSharedComponentEmissionError` (49, `image::rejection_carrier!`), `emit_pe_shared_component` (61), `export_directory_bytes` (93), `directory_rva_of` (194). Constant `COMPONENT_DIRECTORY_NAME = "omega-component.dll"` (16).

**How it decides.** Export acceptance (107-123): non-empty name without NUL, non-zero `slot_commitment`, no duplicate name, `slot_commitment` unique across all exports (count == 1, 113-117), valid symbol handle; symbol must be Function/Text/size>0/within text (125-132). Exports sorted by raw byte name (104). Target must equal `NativeTarget::windows_x64()` exactly (65).

**Cases by shape.** Always exported by name, ordinal base 1 (166), ordinals = sorted index (187). DLL name is fixed constant, not derived from content (contrast Mach-O install name derived from a digest: macho/shared_component.rs:319-331).

**Same job elsewhere.** Export validation (name/commitment/symbol checks) is the same job as `selected_addresses` in macho/shared_component.rs:251-290, written twice; the PE copy compares the commitment by count==1, the Mach-O copy by prefix scan (`exports[..position]`), and the Mach-O copy additionally rejects `name == prior.name` and `commitment == prior.commitment` in one test. Suspected third copy in ELF component emitter (not in this unit).

**Unused / one-caller.** `PeSharedComponentImage::{image,exports,final_text_bytes,contents}` public accessors; counts not separately grepped.

**Stated limits.** none. **Fail-open risks.** The export table is not replayed against the emitted bytes (the Mach-O side replays via `validate_export_trie`, macho/shared_component.rs:350). none seen for acceptance; verification gap noted.

## image/builder.rs (188 lines, read in full)

**Purpose.** Builds a `FinalImage` from an `ObjectPlan` + `RelocationPlan` + encoded text/data bytes.

**Shape.** `FinalImageInput {target, object, relocations, text_bytes, data_bytes}` (151), `build_final_image` (159), private `copies::{copy_object_symbols, copy_object_executable_regions, copy_object_data_regions, copy_object_imports, copy_object_relocations}`, `sections::{section_size, section_alignment}`.

**How it decides.**
- Executable regions are made for every Text-section symbol: Function -> `CompilerFunction`, Object -> `CompilerReadOnlyData`, Import -> skipped (44-48).
- One `CompilerData` region covers all of `.data` when non-empty (61-71).
- Import plan: `Normalized(locator)` if any `normalized_imports` row has the same symbol handle, else `StringBackedBootstrap{library: symbol.import_library}` (82-92).
- Relocation `symbol_handle` that is invalid or not valid in the new arena becomes `Handle::invalid()` (110-114) rather than an error. bss size/alignment from the first section of kind Bss, defaulting to size 0 / alignment 1 (125-143).

**Fail-open risks.** 110-114: a relocation naming a missing symbol is carried forward as an invalid handle. traced: `apply_relocations` (relocations.rs:25-37) rejects it as "unknown symbol", and Mach-O/PE install steps skip it before that (dyld_linking.rs:194-200), so it is caught later at patch time. Anything that consumes a `FinalImage` without calling `apply_relocations` would not see it (not checked).

## image/final_image.rs (1883 lines, read in full)

**Purpose.** Defines `FinalImage` and its tables, and the placed-region inventories with digests used to prove every byte of final `.text`, `.data` and PE import data is either in a classified region or in a recorded gap.

**Shape.** `mod data_regions` (10-998): `FinalDataRegionOrigin {CompilerData, ImportBindingSlot, AlignmentPadding}` (24), `FinalDataRegion` (31), `PlacedDataRegion` (39), `PlacedDataGap` (50), `PlacedDataRegionInventory` (59), `place_data_regions` (77), `place_data_extent` (89), `validate_placed_data_region_inventory` (199). `mod executable_regions` (1000-1502): `FinalExecutableRegionOrigin {CompilerFunction, ImportThunk, CompilerReadOnlyData}` (1012), `FinalExecutableRegion` (1019, with `footprint`, `entry_effects`), `PlacedExecutableRegion` (1029), `PlacedExecutableRegionInventory` (1051), `place_executable_regions` (1062), `place_executable_extent` (1073), `validate_placed_executable_region_inventory` (1182), digest helpers (1336-1493). `layout` (1505: `FinalImageLayout{text,data,bss addresses}`), `memory` (1527: `FinalImageMemory{text,data,bss_size,bss_alignment}`), `relocations` (1563), `root` (1615: `FinalImage{target,memory,symbol_table,relocation_table,executable_regions,data_regions}`), `symbols` (1675: `FinalImageSymbol{name,section,offset,size,kind}`, `FinalImageImportPlan{None,StringBackedBootstrap,Normalized}` 1801, `final_image_symbol_digest` 1682).

**Inputs/outputs.** `place_*` take the final byte slice, a region list and a base address; sort regions by `section_offset` (95, 1078); reject zero width, range overflow, overrun and overlap; record each uncovered stretch as an "unclassified gap" (not an error) (131-138, 1114-1121, 159-166, 1142-1149); return an inventory carrying SHA-256-style domain digests and FNV-1a "report fingerprints". `validate_*` recompute everything from the bytes and the inventory's own rows.

**How it decides.** Judgment is keyed on offset/extent arithmetic and byte digests, not on names; `symbol` strings are included in the inventory digests (382-383, 1364-1365) and in the fingerprints. The validators compare an inventory against the final bytes and its own rows; they do not compare against the `FinalImage.executable_regions` / `data_regions` source list (read: the validate fns take only `inventory` and bytes, 199-202, 1182-1185). So a forged inventory whose rows and digests were recomputed consistently would validate (the tests call `reidentify` at 572-587 to do exactly that for the one-field-substitution matrix and expect rejection only for rows that mismatch the bytes).

**Cases by shape.** Fixed origin enums (3 data, 3 executable). `symbol_digest` architecture/format byte maps are exhaustive matches (1686-1694).

**Same job elsewhere.** The two halves are near-copies: `place_data_extent` (89) vs `place_executable_extent` (1073); `validate_placed_data_region_inventory` (199) vs `validate_placed_executable_region_inventory` (1182); `placed_gap_from_bytes` (331, 1313); `data_inventory_digest` (354) vs `executable_inventory_digest` (1336); `data_inventory_report_fingerprint` (399) vs `executable_inventory_report_fingerprint` (1441). Differences seen: executable digest domain is `...inventory.sha256.v2` (1344) vs data `.v1` (362); the footprint digest writes register index as `to_le_bytes` (1405) while the entry-effects digest writes it as one byte (1422); executable rows carry `footprint`/`entry_effects`, data rows do not.

**Unused / one-caller.** Grep count (`validate_placed_data_region_inventory|validate_placed_executable_region_inventory|place_executable_extent|final_image_symbol_digest|validate_relative_relocation_target|emitted_direct_executable_output|validate_final_text_relocation_envelope|build_final_image`, `*.rs`): 173 matches in 20 files; heavy users are image-emission/src/image.rs (23), native-artifact (13+4+10+2+2+4), image-elf/src/file_assembly.rs (6). So these are widely used (not single-caller). `FinalDataRegionOrigin::AlignmentPadding` is constructed in macho/dyld_linking.rs:222 (read).

**Stated limits.** none (module comment 1-8 only).

**Fail-open risks.** Gaps are accepted by design (stated in header: "explicit gap digests"). `Default for FinalImageSymbol` has kind `Object`, section `None` (1773-1782); `FinalImageImportPlan::default()` is `None` (1803), rejected by both Mach-O (176) and PE (138) import installers. none seen beyond: validators check self-consistency, not provenance (see above), classified as "traced" by reading both validate fns.

## image/lib.rs (470 lines, read in full)

**Purpose.** Crate root: macros (`image_digest!` 44, `rejection_carrier!` 62), `codec_bytes` (81), `patch_bytes` (101), `symbols` helpers (193), re-exports, `align_to`, `align_to_u64` (301, 306).

**Shape / how it decides.** `final_image_symbol_handle` reuses `(arena_index, generation)` of the object handle with no remap (200-206; comment at 11-16 states why). `final_image_symbol_address` returns None for a handle not valid in the arena or a symbol with section None (208-222), and does `section_address + symbol.offset` unchecked (221). `final_image_entry_symbol` requires the entry handle to be valid and in `Text` (239-261).

**Same job elsewhere.** `align_to` (301) is shared: Mach-O file_layout.rs:58 re-exports `image::{align_to, align_to_u64}` and PE layout re-exports `image::align_to` (pe/lib.rs:140), with an extra `align_to_u32` in PE (142). Little-endian writers: `image::codec_bytes::{write_u16,write_u32,write_u64}` (81-92) plus `optimization_core::codec_bytes::patch_encoded` (95) plus PE-local `write_*_at` (pe/lib.rs:52-62) plus Mach-O-local `write_u32_at` (dyld_linking.rs:470) plus `patch_bytes::write_*` (101-142): at least four byte-writer families in this unit.

**Stated limits.** Comment 34-42: `CompilerEntryRegionBindingEvidence` and `CompilerEntryFootprintBindingEvidence` "remain load-bearing across crate boundaries". Grep of the field names `compiler_entry_region_binding|compiler_entry_footprint_binding`: both producers set them to `None` (image-emission/src/image.rs:1434-1435; image/src/output.rs:240-241); consumers destructure/hash them in native-artifact (native_artifact.rs:354-355, 393-394; identity.rs:41-42, 76, 84, 163-164). Type names appear only in image/src/output.rs and image/src/lib.rs (Grep count 6). So no non-`None` constructor of these two carriers was found by grep in this repo (the test at native-artifact tests.rs:403 builds them from an `evidence` value; not read). "Documented as load-bearing, no live producer" is a finding.

**Fail-open risks.** `final_image_symbol_address` `Some(section_address + offset)` for Bss/Data symbols does not check the offset against section size (221). open.

## image/output.rs (387 lines, read in full)

**Purpose.** Output carriers and the compiler-text/function validation evidence records.

**Shape.** `ExecutableImageOutput` (15), `EmittedImageOutput` (35, 4 `Option` evidence fields), `CompilerEntryRegionBindingEvidence` (77), `CompilerEntryFootprintBindingEvidence` (95), `CompilerFunctionValidationEvidence` (110, `evidence_digest` 124), `CompilerTextValidationEvidence` (165, `recomputed_derivation_digest` 181, `has_valid_derivation_digest` 206), `ImageOutputKind {DirectExecutable}` (212, single variant), `emitted_direct_executable_output` (216) which fills `callback_placement_identity_report_fingerprint: 0` and all four evidence options `None` (223, 238-241).

**How it decides.** `has_valid_derivation_digest` compares the stored digest to a recompute from the record's own fields (206-208): self-consistency only.

**Unused / one-caller.** `ImageOutputKind`: Grep count 7 in 3 files (image-emission/src/image.rs 2, output.rs 4, lib.rs 1). Single-variant enum; `callback_placement_identity_report_fingerprint`, `compiler_function_validation`, `CompilerFunctionValidationEvidence` have 56 matches in 8 files, mostly native-artifact/src/publication.rs (28).

**Fail-open risks.** none seen.

## image/relocation_envelope.rs (334 lines, read in full)

**Purpose.** After patching, proves that only declared relocation fields changed between encoded and final bytes.

**Shape.** `validate_final_initialized_data_relocation_envelope` (19), `validate_final_text_relocation_envelope` (86, returns `CompilerTextValidationEvidence`).

**How it decides.** Data: every Data-section relocation must be `Absolute64`, width 8, offset % 8 == 0, in bounds and non-overlapping (37-67); only those 8-byte slots may differ (71-82); data length must match exactly (24). Text: per-byte bit masks from `relocation_field_geometry(kind)` (105-113); declared width must equal the geometry width (106); overlap rejected (124-132); changed bits outside the mask rejected (148-153); final text may be longer than encoded (91, only the prefix is checked, 98).

**Cases by shape.** Asymmetric by design (header comment 1-8 and image/lib.rs:17-20): Text prefix only, Data exact.

**Same job elsewhere.** The mask table `relocation_field_geometry` is used by both patcher (relocations.rs:244, 258, 285) and this verifier (112) and `validate_relative_relocation_target` (relocations.rs:138): one source of truth, as stated.

**Fail-open risks.** The envelope admits ANY value inside a declared field (test comment initialized_data_envelope.rs:164-170 "a byte inside the declared slot may differ"); it proves "nothing else changed", not that the field holds the right address. stated by the test comments. Bytes of `final_text` beyond `encoded_text_bytes.len()` are not examined (98). stated (test text_relocation_envelope.rs:405-422).

## image/relocations.rs (357 lines, read in full)

**Purpose.** Patch driver for relocation kinds.

**Shape.** `apply_relocations` (17), `validate_relative_relocation_target` (122), `relocation_field_geometry` (185), `patch_aarch64_adrp` (224), `patch_aarch64_add_page_offset` (251), `patch_aarch64_branch26` (265), `kind_architecture` (163).

**How it decides.** Keyed on `relocation.kind` and `image.target.architecture`; a kind whose architecture differs from the image is rejected (47-54); Absolute64 has no architecture. Symbol resolution keyed on handle validity: `final_image_symbol_address` None -> error, with a distinct message if `final_image_imports_symbol` (28-37). Page21/PageOffset12/Branch26 require section Text (210-222). Range checks: ADRP +-2^20 pages (235), branch +-2^25 instructions and 4-byte aligned (273-283), rel32 via i128 delta to `i32` (74-79).

**Cases by shape.** Five kinds: `Absolute64`, `X86_64Relative32`, `Aarch64Page21`, `Aarch64PageOffset12`, `Aarch64Branch26` (56-116). `PageOffset12` patches only the ADD-immediate form: `page_offset << 10` with no size scaling (257-261).

**Same job elsewhere.** ADRP and LDR-from-page patching is written again in Mach-O (dyld_linking.rs:420-468, `patch_aarch64_adrp` and `patch_aarch64_ldr_x_from_page`), including its own copy of the +-2^20 page check (430). ADRP bit-decode again at dyld_linking.rs:567-568.

**Unused / one-caller.** `validate_relative_relocation_target`: Grep count over `*.rs` = 3 matches in 3 files (definition relocations.rs, re-export lib.rs, one use in image-emission/src/image.rs): one caller. `apply_relocations` is called at macho/lib.rs:783 and pe/lib.rs:205.

**Fail-open risks.** `relocation.byte_width` is not read by `apply_relocations`; patch widths come from the kind (e.g. 8 bytes for Absolute64, 57-65). A record whose `byte_width` disagrees is caught only by the envelope validators (relocation_envelope.rs:106, 37), which run in image-emission. traced to that extent.

## image/tests/common/mod.rs (16), initialized_data_envelope.rs (409), relocation_rejections.rs (753), text_relocation_envelope.rs (486) (all read in full)

Test-only. `common` gives one text/data/bss address triple (0x40_0000/0x41_0000/0x42_0000). The three test targets drive `build_final_image` -> `apply_relocations` -> envelope/inventory validators on x86-64 and AArch64 object plans, pin each rejection arm (unknown symbol, import vs sectionless, rel32 +-i32 edge, ADRP 2^20 pages, B 2^25 instructions, alignment, non-text instruction fields, Bss targets, overlapping/malformed slots, truncated/extended data), and pin that compact FNV fingerprints cannot substitute for SHA-256 digests (initialized_data_envelope.rs:341-409). Same job elsewhere: unit tests in macho/dyld_linking.rs and pe/lib.rs repeat several of these shapes per container.

Addendum to macho/dyld_linking.rs: `darwin_import_library` (calling-conventions/src/host_operations.rs:380, read lines 380-392 only) picks libobjc when the symbol starts with `_objc_`, `_sel_`, `_class_`, `_object_`, `_method_`, `_ivar_` or `_protocol_`, and CoreGraphics when it starts with `_CG`; the rest of that function was not read. This is the spelling-keyed judgment behind Mach-O `StringBackedBootstrap` imports (dyld_linking.rs:117, 124). The PE counterpart `windows_import_library` (host_operations.rs:467) is an exact-match lookup in `WINDOWS_IMPORT_ROWS` by symbol string, returning `Option`.

## encoded-scalar-plan/src/lib.rs (161 lines, read in full)

**Purpose.** Target-neutral program for variable-width encoded scalar loads and stores (a byte loop), so x86-64 and AArch64 emit the same access order.

**Shape.** `enum Step` (34, 16 variants: Zero, Copy, ByteCount, SubOne, AddOne, LoadByte, StoreByte, SignByte, ShiftLeft8, ShiftRight8, Or, Compare, TestSigned, ZeroBranch, LessBranch, Branch), `plan(load, registers: [u8;6], finish: Step) -> Vec<Step>` (53), feature-gated `replay::execute` (93-160), `pub mod harness` (90).

**How it decides.** Step-index branch targets are literals inside `plan`: load uses targets 8, 15, 9 and store uses 3 (63-84); the load program is 16 steps with `finish` as step 15 (72). Signedness is bit 8 of the codec word (`TestSigned`, replay line 142 `& 256`); byte count is `codec & 255` (118).

**Cases by shape.** Fixed two shapes (load, store), six registers each (55, 75). Replay caps execution at 128 steps and panics beyond (106, 159).

**Same job elsewhere.** Consumers: Grep count `encoded_scalar_plan` over `*.rs` = 4 matches in 2 files (isa-aarch64/src/selected_form_encoding.rs 2, isa-x86_64/src/selected_form_encoding.rs 2). The header says it is the single shared definition; it has exactly the two stated outside users.

**Unused / one-caller.** `replay` and `harness` are `test-support` only (89-93).

**Stated limits.** none. **Fail-open risks.** none seen (test support code; `replay` indexes `memory[address]` and panics on out-of-range).

## encoded-scalar-plan/src/harness.rs (163 lines, read in full)

**Purpose.** Shared conformance sweeps run by both ISAs' tests: `exact_footprint_and_extension` (54) checks store/load of codecs `1..=8` signed/unsigned plus 516, 520, 1025, 2056 (`codecs()` 36-40) for exact byte access order and sign/zero extension; `rejects_every_single_bit_mutation` (112) flips every bit of the encoded stream and requires refusal, and requires scratch-register aliasing to be rejected while input aliasing is accepted (138-161). `kind`/`alternative` (17, 25) map `load: bool` to `SelectedInstructionKind` and `MachineAlternativeKey{variant: 0}`.

**Fail-open risks.** none seen (test-only). Test-only by `test-support` feature (lib.rs:89).

## isa-aarch64/src/floating_control.rs (102 lines, read in full)

**Purpose.** Encodes FPCR save (MRS x9, FPCR; STR) and restore (LDR; MSR FPCR, x9) around returning foreign calls, using x9 for the value and x16 as address scratch when the offset exceeds the scaled 12-bit reach.

**Shape.** `encode_save_fpcr_to_sp_displacement` (25), `encode_restore_fpcr_from_sp_displacement` (31), private `stack_access` (37), consts `SCRATCH_REGISTER = 9`, `SAVE_ADDRESS_REGISTER = 16` (15-16), `MAX_FPCR_SEQUENCE_BYTES` (22).

**How it decides.** Offset must be a multiple of 8 (38); `byte_offset / 8 <= 0xfff` uses one instruction (43), else page steps through `append_page_steps` (47-53).

**Same job elsewhere.** Large-offset addressing is the same page-step scheme as `frame_protocol::append_sized_memory` (frame_protocol.rs:359-375) and `immediate_offset::address_words` (immediate_offset.rs:60).

**Stated limits.** The header says "encode and decode" but this file contains only encoders (grep of the file read in full: no decode function). Decoding is not in this file (suspected in `selected_form_encoding`, not read).

**Unused / one-caller.** Re-exported at isa-aarch64 lib.rs:124-126; `append_page_steps`/`append_offset_steps`/`decode_address_words` counts below.

**Fail-open risks.** none seen (rejects unaligned offset).

## isa-aarch64/src/frame_protocol.rs (750 lines, read in full)

**Purpose.** Encodes the AAPCS64 fixed-frame prologue/epilogue, with optional page-granule stack probes, plus frame/pointer memory accesses, frame addresses and integer constants.

**Shape.** `Aarch64FrameSlot {view, offset_bytes, size_bytes}` (17), `Aarch64StackProbe {interval_bytes, touches}` (24), `Aarch64FrameProtocolError` (32, 6 variants), `encode_aapcs64_frame_protocol` (48), `encode_aarch64_frame_memory` (229), `encode_aarch64_pointer_memory` (260), `encode_aarch64_frame_address` (289), `encode_aarch64_aligned_frame_address` (306), `encode_aarch64_integer_constant` (327), private `append_sized_memory` (345).

**How it decides.**
- Canonical model check by identity equality with the canonical model (168).
- Register view -> register: keyed on the view's NAME string: split first char as prefix (`x`, `d`, `q`) and parse the rest as `u8` (178-192). `x` 0..=30, `d`/`q` 0..=31 only. This is a source-spelling decode of the physical register model's row names.
- Probe validity: interval power of two <= 65_536 (103-107); `touches` must equal `ceil(frame/interval)` when `frame > interval`, else 0 (108-115). A probe is a `ldr xzr,[sp]` word `0xf940_03ff` (120) after each `interval` chunk (63-76).
- Slots: size exactly 8, offset multiple of 8, within frame, ascending and non-overlapping (146-151). Frame size multiple of 16 and <= u32::MAX (55).
- Scratch for large offsets: first of x16/x17 not used as a slot register; else error (199-216).
- Access forms: sizes 1/2/4/8/16 with register class pairs (X,1|2|4|8), (D,4|8), (Q,16) (377-386); anything else -> `OffsetOutOfRange` (385).

**Cases by shape.** Slots are 8-byte only (146). The frame/pointer memory encoders accept exactly the register-class/size table at 377-386.

**Same job elsewhere.** Address stepping duplicates the page-step pattern in floating_control.rs:46-55; frame and hosted-sequence sizes duplicate `immediate_offset.rs` constants (`MAX_ADDRESS_BYTES`, 143).

**Stated limits.** Header says "encode and validate" (1); validation here is the up-front checks `validate_probe` (99) and `validate_slots` (136) before encoding; no post-hoc decoder in this file.

**Unused / one-caller.** pub fns re-exported at lib.rs:127-131; not separately grepped.

**Fail-open risks.** `register()` 183 `name.split_at(1)` would panic on a view whose name starts with a multi-byte character; reachable only with a non-canonical model, which `require_canonical_model` (54, 237, 268...) rejects first. traced. `frame_scratch` 199 returns `Ok(None)` when every slot offset/8 <= 4095, so no scratch is chosen even if the register-offset path in `append_sized_memory` is later needed; it is needed only when `offset/8 > 4095`, same condition. traced.

## isa-aarch64/src/generated_entry.rs (220 lines, read in full)

**Purpose.** Decodes a generated AArch64 entry stream word by word into register reads/writes, stack/flag state and a control effect, trusting nothing from the producer.

**Shape.** private `Effects` (16), `instruction(word, offset)` (51), `valid_logical_immediate` (125), `decode_aarch64_generated_entry(bytes, body_call_field)` (136) returning `GeneratedEntryEvidence`.

**How it decides.** A closed whitelist by bit masks: RET (via `decode_words`, 56), `BL` (`word & 0xfc00_0000 == 0x9400_0000`, 60), `NOP` `0xd503_201f` (66), 64-bit ADD/SUB immediate (`word>>31==1 && word & 0x1f80_0000 == 0x1100_0000`, 67), a list of 14 load/store opcode masks (74-90), MOVN/MOVZ/MOVK 64-bit (99-105), logical-immediate with non-31 source/dest (106-113), `ORR x, xzr, x` register move `0xaa00_03e0` (114). Any other word: `Err("unsupported generated AArch64 instruction ...")` (118). A `BL` whose byte offset equals `body_call_field` stays a `Call`; any other `BL` becomes `LocalCall` with target computed from the sign-extended displacement (150-157).

**Cases by shape.** The whitelist above is the shape list; no `unsupported` fallthrough accepts.

**Same job elsewhere.** The same instruction classes are decoded in `selected_form_encoding::decoding` (called at 8, 56; not read). Logical-immediate validity is implemented here (125-134) and presumably also in the encoder (suspected, not read).

**Unused / one-caller.** `decode_aarch64_generated_entry`: Grep count of the hosted/entry function group (`decode_aarch64_generated_entry|encode_hosted_*|encode_linux_*|encode_macos_*`) = 69 matches in 14 files; callers outside this crate include image-emission/src/entries.rs (4), object.rs, object_artifact.rs, native-artifact hosted_builtins.rs.

**Fail-open risks.** `Effects::read/write` (23-40): register 31 with `stack == false` and not vector is silently treated as XZR (no effect recorded). A word using register 31 as SP in a form where `stack` was passed false (loads/stores pass `stack=true` for the base only, 93) is classed zero register. Not a way to add authority beyond what the whitelisted forms do. traced by reading all call sites in this file.

## isa-aarch64/src/hosted_sequences.rs (370 lines, read in full)

**Purpose.** Hand-assembled, import-free syscall leaves: process exit, single-byte write from w9, read byte to stack, literal line write.

**Shape.** `encode_hosted_exit_process_i32(target, value)` (10), `encode_hosted_write_byte_i32_from_w9()` (32), `encode_linux_read_byte_to_stack` (68), `encode_macos_read_byte_to_stack` (89), `encode_linux_write_line_literal(literal)` (114), plus word encoders (`encode_movz` 185, `encode_svc` 212, ...).

**How it decides.** Exit is keyed on exact target equality: `== NativeTarget::linux_arm64()` -> x8/94/svc #0; `== macos_arm64()` -> x16/1/svc #0x80; else error (14-23). The write-byte leaf takes no target and hard-codes x8=64, `svc #0` (39-40) i.e. the Linux ABI; the write-line leaf is named Linux. Header (1-6) says Linux and Darwin cover "console writes" and "literal line writes"; only exit and read-byte have a Darwin form in this file.

**Cases by shape.** `encode_linux_write_line_literal` rejects payload >= 1<<20 (122) and ADR distance outside +-1 MiB (157). Compare immediate limited to <= 4095: "AArch64 MVP encoder cannot compare value `{value}` yet" (222-224).

**Same job elsewhere.** Exit-code sequence and syscall numbering also appear in `selected_form_encoding::hosted_exit_process` and `hosted_write_byte` (referenced in lib.rs:184-234, not read); Darwin hosted write byte has its own constraint row `AARCH64_DARWIN_HOSTED_WRITE_BYTE_I32` (register_model.rs:1803-1804, 642-651), so its bytes come from elsewhere.

**Unused / one-caller.** See the 69-match group above (isa-aarch64 selected_form_encoding.rs 6, hosted_sequences.rs 16 incl. tests, lib.rs 4, plus isa-x86_64 3/1/19 for its own similarly named items).

**Stated limits.** "AArch64 MVP encoder cannot compare value ... yet" (223).

**Fail-open risks.** none seen (unsupported targets error at 20).

## isa-aarch64/src/immediate_offset.rs (277 lines, read in full)

**Purpose.** Canonical sequence of 64-bit ADD/SUB immediate instructions realizing a u32 byte offset; also decode and validate that exact form.

**Shape.** `ADD_IMMEDIATE`, `SUBTRACT_IMMEDIATE` (10-11), `append_page_steps` (23), `append_offset_steps` (40), `address_words` (60), `aligned_frame_address_words` (69), `validate_aligned_frame_address_words` (101), `address_word_count` (134), `decode_address_words` (148), `MAX_ADDRESS_BYTES`, `MAX_STORE64_BYTES` (143, 145).

**How it decides.** Pages stepped in 4095-page chunks (13, 33), then one unshifted remainder immediate (49-57). `decode_address_words` accepts only the canonical split: each shifted step except the last must be exactly 4095 pages, no zero immediates after the first word, remainder last (154-175). Alignment helper requires `alignment > 16`, power of two, `frame_offset % 16 == 0` (75, 108).

**Same job elsewhere.** Used by frame_protocol.rs, floating_control.rs and selected_form_encoding.rs. Grep count `decode_address_words|validate_aligned_frame_address_words|aligned_frame_address_words|append_page_steps|append_offset_steps` = 42 matches in 4 files: immediate_offset.rs 24, selected_form_encoding.rs 12, frame_protocol.rs 4, floating_control.rs 2.

**Stated limits.** Header: offsets "beyond one shifted-plus-unshifted 12-bit pair (16 MiB - 1) step in full 4095-page increments" (4-6).

**Fail-open risks.** `step()` casts `immediate as u32` without range check (15-21); callers cap at 4095 (33, 49). `decode_address_words` returns `None` for an empty slice (176), i.e. rejects. none seen.

## isa-aarch64/src/lib.rs (247 lines, read in full)

**Purpose.** Crate root: module wiring and a large re-export surface (about 150 names). Two header comments claim every byte producer has an independent decoder and shared tables (11-20).

**Shape / decisions.** No logic besides `ORDINARY_DIRECT_CALL_BYTES = 0x9400_0000` and `ORDINARY_RETURN_BYTES = 0xd65f_03c0` (122-123) and test helpers `reject_every_bit_flip` (78).

**Same job elsewhere.** The claim that each producer has an in-crate decoder is not true of `floating_control.rs`, `frame_protocol.rs`, `hosted_sequences.rs`, `immediate_offset.rs` (only `decode_address_words`) in this unit (read). Decoders for selected forms live in `selected_form_encoding` (declared at 121, not read).

**Stated limits.** none besides header claims. **Fail-open risks.** none seen.

## isa-aarch64/src/machine_effects.rs (1729 lines, read in full)

**Purpose.** Builds and validates the per-target machine-effect catalog: for each semantic kind selected for the target, one declaration with size knowledge, encoded read/write operands, implicit units, memory/trap/control effects.

**Shape.** `mod scalar_call` (21-249): `declaration` (31) for calls, `aarch64_normalized_foreign_call_declaration` (60), `normalized_foreign_constraint` (193). `isa_machine_effect_catalog_plumbing!` macro invocation (263) defines `Aarch64MachineEffectCatalogValidationError` and `validate_aarch64_machine_effect_catalog`. `aarch64_machine_effect_catalog` (271), `Aarch64SelectedAbi {Aapcs64, Darwin}` (404), `aarch64_selected_abi` (409), `aarch64_selected_constraint_keys` (424, a 170-field struct literal 438-592), `declaration` (617), `encoded_effects` (711), `size` (1106), `packed_load_width` (1191).

**How it decides.**
- ABI keyed on `(architecture, object_format)`: ELF -> AAPCS64, Mach-O -> Darwin, COFF -> error (412-421).
- Hosted rows keyed on exact equality `target == NativeTarget::linux_arm64()` / `macos_arm64()` (481-501); any other AArch64 ELF/Mach-O target (e.g. 4-byte pointers) gets `None` for the three hosted keys (test 1700-1727).
- Linux system-call keys are added when `target.object_format == ObjectFormat::Elf` (535-537).
- Per-semantic dispatch is an if-chain over `MachineSemanticKind` (290-396) delegating to `selected_form_encoding::*::declaration` modules (not read), else `scalar_call_declaration` for calls, else the local `declaration`.
- Normalized foreign-call declaration is accepted only if the supplied constraint row equals the recomputed one (64-67); callbacks must target the invocation site's operation (80-82); system-call variants with callbacks rejected (113); callback destinations only `Parameter` (single register location) or `Field` (84-110).

**Cases by shape.** `encoded_effects` has an explicit per-semantic table of (reads, writes) at 729-879; `PortRegisterRead ... PortWrite` (x86 only) hit `unreachable!` (755); `PrivilegedCrash` -> `unreachable!` (775); `HostedExitProcessI32`, `HostedReadByte`, `SaveFloatingControl`, `RestoreFloatingControl`, `HostedWriteByteI32`, `CallUnit` -> `panic!("memory and Unit call forms are not admitted on this target")` (862-869, repeated in `size` 1151-1158) because they use dedicated declarations.

**Same job elsewhere.** Size and effects for saturating forms come from `SaturatingRealization` (saturating_forms.rs:99-150); sizes for the same forms are re-pinned in the test at 1269-1324 (16, 28, 8, 4, 24). Return/call/scalar declarations repeat the effect templates in `scalar_call::declaration` (31-58) and `declaration` (617-709).

**Unused / one-caller.** pub fns re-exported at lib.rs:132-136; `aarch64_selected_abi` and `Aarch64SelectedAbi` are `pub(crate)` (404, 409); not separately counted.

**Stated limits.** `panic!("memory and Unit call forms are not admitted on this target")` (868, 1157).

**Fail-open risks.**
- 993-998 default arm in `encoded_effects`: any semantic not named in the implicit-effect table gets no implicit uses/defs, `NeverV1` trap and `FallThroughV1` control. 1187 default arm in `size`: `ExactBytes(4)`. Which wrong declaration reaches it: a semantic that is added to the (reads, writes) table at 729-879 but not to the effect table at 880-998 would be declared pure, non-trapping, fall-through. The semantics listed explicitly in the reads/writes match as fallthrough arithmetic (e.g. `WrappingAddI64`) depend on this default by design. open (no comment says so).
- The validator `validate_aarch64_machine_effect_catalog` is generated by the macro at 263 (not read); tests at 1233-1730 pin rejection of altered size, family, reads and applicability.

## isa-aarch64/src/post_handoff_writer.rs (507 lines, read in full)

**Purpose.** Lowers a post-handoff writer plan (list of bit-fragment stores into a private destination, from numbered source slots) into address-free A64 words; reports its clobber set x9..x13.

**Shape.** `mod fragment::append_fragment` (14), `mod generated_writer` (142): consts CONTEXT 9, DESTINATION 10, SOURCE 11, FRAGMENT 12, CONTAINER 13 (153-157), `generated_post_handoff_writer_clobbers` (159), `..._additional_machine_state` (169), `..._width` (173), `encode_generated_post_handoff_writer_bytes` (180), `validate_generated_post_handoff_writer` (230); `mod writer_encoding` (374).

**How it decides.** Pointer register must be `Aarch64X(n)` with n < 31 (234-243); context ABI must equal `POST_HANDOFF_WRITER_CONTEXT_ABI_V1` ("PHWRITR1", 244-249); little-endian only (250); plan non-empty, source slots dense and each used (255-277); context width <= `4096 * SLOT_WIDTH` (262). `append_fragment` rejects registers > 30 or duplicate (23-30) and `IntegerAt` placements (46); whole-container writes skip the load (60-63). Containers of 1/2/4/8 bytes use single loads/stores; any other byte count falls to a byte-by-byte loop (101-117, 123-137).

**Cases by shape.** Container widths 8/16/32/64 get direct forms; other widths (`_` arm 104, 126) use the byte loop, which encodes `index * 8` into a 6-bit shift field (112) so it is only correct for byte counts up to 8. The upper bound on `container_width` is enforced by `layout_plans::validate_fragment` (48; not read) so reachability of counts > 8 is open.

**Same job elsewhere.** Offset-scaled load/store word encoders here (`encode_load_w_from_x` 396 etc.) repeat the word encoding in `frame_protocol::append_sized_memory` (377-391); different offset rules (no scratch/page steps here, errors at offset > 4095).

**Unused / one-caller.** pub fns re-exported at lib.rs:137-141. **Stated limits.** error strings "cannot load a u64 at offset" (443). **Fail-open risks.** none seen beyond the open item above.

## isa-aarch64/src/preservation_storage.rs (189 lines, read in full)

**Purpose.** Builds the callee-saved storage catalog for `linux_arm64` and `macos_arm64`: one 8-byte group per preserved register.

**Shape.** `Aarch64PreservationStorageCatalogError` (18), `aarch64_preservation_storage_catalog(model, target)` (35).

**How it decides.** Exact target equality with `linux_arm64()` or `macos_arm64()` (42), canonical model identity (39). Group names are generated from strings: `x19..=x29` then `d8..=d15` (47-50) and looked up with `view_named(name)` (55-58, `expect`).

**Stated limits / mismatch.** Header (3-5) says `x19`..`x28`; the code and the test list x19..=x29 (19 groups, 152 bytes, test 98-115). x29 is included; the header omits it.

**Fail-open risks.** none seen.

## isa-aarch64/src/register_model.rs (2973 lines, read in full)

**Purpose.** Defines the canonical AArch64 physical register model, the key numbering for every register-constraint row, and the generated constraint catalog plus its validator.

**Shape (all in sub-modules, re-exported at 2916-2973).** `call_keys` (9): key ranges (e.g. AAPCS64 scalar `[3,4,2,5..10]` 59, Darwin `11..=19` 35, unit-call 700..=708 / 720..=728, aggregate 1000/1020, normalized foreign 3000 / 3300). `constraint_catalog` (92): `aarch64_register_constraint_catalog` (160-1065, one 900-line builder), `validate_aarch64_register_constraint_catalog` (1067), `aarch64_register_constraint_catalog_for` (1109, memoised in a `OnceLock` for the canonical model). `float_scalar_calls` (1123), `indirect_results` (1344), `mixed_calls` (1389), `operation_keys` (1627: ~85 `RegisterConstraintKey::instruction(n)` constants and `AARCH64_REQUIRED_REGISTER_CONSTRAINTS: [_; 120]` 1822), `physical_model` (1957: class ids 0..10, `build_aarch64_physical_register_model` 2014, `aarch64_fixed_register_view` 1990, `validated_aarch64_physical_register_model` 2247, identity 2257), `system_calls` (2262: keys 100..114).

**Inputs/outputs.** Builds `PhysicalRegisterModel` (units per x0..x30, sp, zero, v0..v31 low/high halves, nzcv, fpcr, fpsr, pc; views x/w/q/d/s) and `RegisterConstraintCatalog` rows (operands with fixed or class constraints, implicit uses/defs, clobbers).

**How it decides.**
- Views and units are named by strings: `view("x9")`, `"nzcv"`, `"fpcr"` etc. throughout the builder (e.g. 166-170, 207, 257-285); `aarch64_fixed_register_view` converts a `MachineRegister` into a view by formatting `x{n}` / `d{n}` (1997-2002).
- Convention selection by object format: ELF -> `aapcs64`, Mach-O -> `darwin-aapcs64`, COFF -> None (22-26). Darwin differs by x18 being fixed (2199-2201).
- Reservations (2235-2243): sp, zero, pc, frame pointer x29, Darwin platform x18, Omega dispatch x27, Omega metering x28.
- Validator: structural validation, then for every key in the catalog's `required` list the row must be equal to the freshly built canonical row, and any row not in `required` is rejected (1084-1105). The "canonical" row set is produced by the same builder (1109-1120), so the check is builder-vs-builder.
- Call-clobber sets: caller-saved units minus x0 and the link/pc defs (263-270).

**Cases by shape.** Fixed counts baked into key ranges: aggregate call keys are 18 per ABI (40), mixed unit keys 72 per ABI (1400, 1404), mixed aggregate 144 (1417), indirect aggregate 81 (1352), system calls 14 (2269: 7 argument counts x returns-or-not, `position / 2`, `position % 2`, 2289-2290 with arguments x0..x5 only). Mixed calls enumerate `general 0..=8` x `float 1..=8` (1431-1432). Scalar call arity 0..=8 via `x0..x7` (541-547). Float/mixed/indirect builders end with `let ... else { return; }` when a needed view or base row is missing (1156, 1166, 1169, 1190, 1361, 1372, 1427, 1434, 1452, 1466, 1476).

**Same job elsewhere.** `AARCH64_REQUIRED_REGISTER_CONSTRAINTS` (1822) is a hand-listed inventory that must stay in step with the builder; the catalog's `required` is built from it plus range helpers (1045-1062). Preservation conventions here (2202-2229) are consumed by preservation_storage.rs:45 and frame code. Constraint ids are assigned as dense ids after sort by key (1038-1042), after being filled with duplicates `RegisterConstraintId(0)`/`(6)` as placeholders (e.g. 333-415).

**Unused / one-caller.** `AARCH64_INLINE_ASSEMBLY_DEFAULT` and `aarch64_register_unit_call_keys`: Grep count of both = 13 matches, all in register_model.rs (pub(crate) const at 1709; the default inline-assembly row is created at 398-405 and is in the required list 1866). No use of `AARCH64_INLINE_ASSEMBLY_DEFAULT` found outside this file by that grep (counted together with the other pattern, so not separable; "suspected" no outside user).

**Stated limits.** none besides panics ("validated AArch64 model must define {name}" 169, 204).

**Fail-open risks.**
- The silent `return` branches listed under Cases by shape drop the rest of a builder step if a view is absent from the supplied model. Wrong input that reaches them: a non-canonical physical model passed to `aarch64_register_constraint_catalog` (160; only `assert_eq!(architecture)` at 164). traced: the validator then checks every `required` key is present and equal to the canonical row, and returns `TargetSemantics`/missing-required (1084-1095), and `validate_aarch64_register_constraint_catalog` rejects a non-canonical model first (1077). Test `aarch64_catalog_validation_rejects_same_architecture_forged_physical_model` (2838) pins it.
- The validator compares the supplied catalog against a catalog built by the same code, so a defect in the builder is accepted by construction. stated by structure, not by a comment.

## isa-aarch64/src/saturating_forms.rs (151 lines, read in full)

**Purpose.** Classifies each (saturating operation, carrier) pair into one of nine realization shapes and answers size/operand/flag questions about each.

**Shape.** `enum SaturatingRealization` (14, 9 variants), `of` (34), `of_kind` (57), `of_semantic` (78), `operand_count` (99), `defines_nzcv` (113), `upper_condition` (117), `clamp_bounds` (127), `byte_size` (140).

**How it decides.** Pure match on (operation, carrier); signed narrow carriers and non-U64/I64 fall to `ClampNarrow` via the last arm (53). `byte_size`: `4 + 12 * clamp_bounds().len()` for `ClampNarrow` (144); others 4/8/16/24 literals (142-148). `upper_condition` is 0x8 only for unsigned narrow multiply else 0xc (117-125).

**Stated limits / mismatch.** Header (1-7) says this file specifies "instruction-word realizations"; the file holds classification and size tables only, no instruction words (read). The words are in selected_form_encoding.rs (Grep count of `SaturatingRealization` = 41 matches in 3 files: selected_form_encoding.rs 34, saturating_forms.rs 3, machine_effects.rs 4), so the "single source of truth" is shared by 2 outside users (catalog and encoder files).

**Fail-open risks.** none seen.

## Cross-cutting findings for this unit

- Image crate: `FinalImage` construction, relocation patching and the placed-region inventories are shared by ELF/PE/Mach-O (image-elf file_assembly.rs, image-emission/src/image.rs use them; counts above). PE and Mach-O each carry their own import-thunk installer, patcher, footprint validator and export builder (pe/imports.rs, pe/shared_component.rs; macho/dyld_linking.rs, macho/shared_component.rs).
- Mach-O re-reads its finished bytes through two validators (`validate_macho_container_mapping`, `validate_macho_loader_fixups_for_segment`, macho/lib.rs:876-899) and a third pair of byte validators is run by image-emission (image.rs:843-858, 3 call lines). PE has no re-read of its finished bytes in this unit.
- Spelling-keyed judgments seen: Mach-O bootstrap library by symbol-name prefix (dyld_linking.rs:117); PE library by exact symbol-name lookup with KERNEL32 default (pe/imports.rs:82-86); AArch64 register decode by view-name prefix (frame_protocol.rs:178-192); AArch64 register-model rows looked up by name string (register_model.rs, throughout).
- Exact-target equality (not format shape) decides hosted rows and preservation catalogs: machine_effects.rs:481-501, preservation_storage.rs:42, hosted_sequences.rs:15-17, pe/shared_component.rs:65.

- **Coverage:** read in full, 19511 of 19511 lines (all 32 files, every range). Skimmed: none. Not read: none. Line counts: macho 2626+313+911+635 = 4485; pe 129+624+575+256+320 = 1904; image src 188+1883+470+387+334+357 = 3619; image tests 16+409+753+486 = 1664; encoded-scalar-plan 163+161 = 324; isa-aarch64 102+750+220+370+277+247+1729+507+189+2973+151 = 7515. Non-unit files consulted by grep only: calling-conventions/src/host_operations.rs lines 380-392, 467-472.

- **Five things a maintainer most needs to know about this unit.**
  1. PE and Mach-O each reinstall import thunks differently: PE thunks every import row (pe/imports.rs:49-178), Mach-O only those a relocation references (dyld_linking.rs:183), and Mach-O alone re-reads the finished container (macho/lib.rs:876-899); PE verifies just the 2-byte `ff 25` opcode (pe/imports.rs:340).
  2. The placed-region validators (`validate_placed_*_inventory`, final_image.rs:199, 1182) prove an inventory is consistent with the final bytes and its own rows, not that it matches `FinalImage.executable_regions`/`data_regions`; the relocation envelopes (relocation_envelope.rs) prove nothing outside declared fields changed but accept any value inside a field.
  3. `CompilerEntryRegionBindingEvidence`/`CompilerEntryFootprintBindingEvidence` are documented as load-bearing (image/lib.rs:34-42) but both producers set the fields to `None` (image-emission/src/image.rs:1434-1435, output.rs:240-241); no non-`None` constructor was found by grep.
  4. `register_model.rs` builds the AArch64 constraint catalog in one 900-line function from register NAME strings, and the validator compares against a catalog from the same builder (1084-1120); required-key inventory is a hand list of 120 keys plus ranges (1822, 1045-1062).
  5. `machine_effects.rs` has default arms (993-998, 1187) that give any unlisted semantic fall-through, non-trapping, 4-byte effects, alongside `panic!`/`unreachable!` arms for forms "not admitted on this target" (755, 775, 868, 1157).

- **What you are unsure about.**
  - Whether `container_width` for post-handoff writer fragments can exceed 64 bits (post_handoff_writer.rs:104, 126); depends on `layout_plans::validate_fragment`, not read.
  - Whether PE export tables and PE thunk/slot pairing are verified in image-emission/src/image.rs (not read); Mach-O has `validate_macho_import_binding_pairing` called there (image.rs:858) but the PE equivalent was not seen.
  - Whether the decoders for the AArch64 frame/FPCR/hosted encoders exist in `selected_form_encoding` (not read); this unit's headers imply they do.
  - The count for `AARCH64_INLINE_ASSEMBLY_DEFAULT` outside register_model.rs (the grep pattern was combined with another and returned 13 matches, all in register_model.rs).
  - `native-artifact` tests.rs:403 builds the two entry-binding evidence fields from a variable named `evidence`; I did not read it to see which type that is.
  - Counts quoted as "match" counts are Grep count-mode lines (occurrences per file), not call-site counts.
