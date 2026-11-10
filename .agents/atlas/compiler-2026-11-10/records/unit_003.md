# Unit 3 notes

Files covered (paths relative to `omega-rust/omega/compilation/backend/`):
1. `calling-conventions/src/terminal_layout.rs` (486 lines)
2. `images/image-elf/src/dynamic_executable.rs` (685)
3. `images/image-elf/src/dynamic_table.rs` (2219)
4. `images/image-elf/src/file_assembly.rs` (3947)
5. `images/image-elf/src/import_sections.rs` (3294)
6. `images/image-elf/src/imports.rs` (435)
7. `images/image-elf/src/lib.rs` (697)
8. `images/image-elf/src/load_placement.rs` (3379)
9. `images/image-elf/src/procedure_linkage.rs` (3248)
10. `images/image-elf/src/section_headers.rs` (3183)

Reading status: I read all non-test code of files 1-3 and 6-7 in full, and the non-test code of files 4, 5, 8, 9, 10 (every `pub(crate) mod` body up to its `#[cfg(test)] mod tests`). I did NOT read the `#[cfg(test)] mod tests` bodies of files 4, 5, 8, 9, 10 (5,728 lines; exact ranges at the end). Where I say "tests" for those files I mean only what the grep of `mod tests` line numbers showed. All line numbers below are from reads or greps I ran.

Common structure of the ten ELF files (read): files 2-5 and 8-10 together are the 22-stage "dynamic executable" pipeline. Every stage module has the same skeleton: a `Validated…` carrier struct {previous carrier, `contents`, `non_authoritative_*_compatibility_fingerprint`}, a `…Error` rejection carrier (via `image::rejection_carrier!`) that hands the previous carrier back, a private `Candidate`, a `plan_/serialize_/apply_…` entry that expands `checked::plan_stage!` (dynamic_executable.rs:387-425) -> `derive_contents` -> `validate_candidate` (from `seal_stage!`, :282-363) -> `validate_contents`, and a `non_authoritative_…_fingerprint` fn that FNV-1a hashes the stage's contents on top of the upstream fingerprint (`checked::stage_fingerprint`, :127). Stage order is in `emit_elf_dynamic_executable` (dynamic_executable.rs:614-663).

---

## 1. calling-conventions/src/terminal_layout.rs (1-486, read in full)

**Purpose.** Computes byte size and alignment (`ValueShape`) for each Terminal Psi structural type, once per module, into a dense `IdTable`. Header (:1-8) says it is consumed by instruction lowering and task stack planning.

**Shape.**
- `StructuralLayouts<'a>` :33 {`declarations: IdTable<StructuralTypeId,&StructuralTypeDeclaration>`, `shapes: IdTable<StructuralTypeId, Result<ValueShape,TerminalLayoutError>>`}; `Deref` to the declarations table :131.
- `new` :39, `shape` :61, `field_shape` :70, `sum_layout` :81, `scalar_storage_type` :104, `required_scalar_extents` :108.
- `TerminalLayoutError` :19 (7 variants: RecursiveStructuralType, UnknownStructuralType, UnclosedTargetLayout, UnclosedArrayExtent, StructuralTypeTooLarge, EmptyStructuralType, UnsupportedStructuralSum).
- Free fns: `resolve_structural_shape` :146 (recursive, memo table `known`, cycle set `active`), `conventional_sum_layout_from_parts` :307, `resolve_structural_field_shape` :348, `scalar_storage_type` :365, `scalar_field_shape` :376, `byte_sequence_shape` :426 (pub), `checked_align_up_u32` :444 (pub), `scalar_shape` :453 (pub), `empty_primitive_array` :465.

**Inputs/outputs.** Reads `StructuralTypeDeclaration` rows (`terminal_psi`), `runtime_abi::ScalarReferenceLayout::NATIVE` (:172), `runtime_abi::ScalarStorageCodec::from_scalar` (:373, :480), `crate::evaluate_conventional_sum_layout` (:343, not read). Produces a `ValueShape` per type; `required_scalar_extents` returns the set of array-length applications that are still open (:108-128).

**How it decides.** Keyed on structural type id and the `StructuralTypeShape` variant (:169-295).
- Reference: 8 bytes, or `ScalarReferenceLayout::NATIVE.byte_size` when the referent is a primitive scalar with a storage codec (:170-177).
- Record with `TargetLayout` storage: error `UnclosedTargetLayout` (:185-190). Record with `Authored`/`AccessView`/`Accessor` storage: returns the declared `byte_size`/`byte_alignment` without looking at fields (:191-203). Other records: fields laid out in order, erased fields skipped (:208-211), size aligned up (:229-240).
- Fixed array: literal or application length; stride = element size aligned to element alignment (:258-267). A zero-length array whose innermost element is a storage-codec scalar is size 0, align 1 (:250-252 via `empty_primitive_array` :465-486).
- Sum / Mixed: delegated to `evaluate_conventional_sum_layout` (:343); zero cases is `EmptyStructuralType` (:315).

**Cases by shape.** `scalar_field_shape` (:387-423) handles only Boolean, Integer, BoundedInteger, Binary32, Binary64, ByteSequence/PredicateByteSequence, Atomic variants; `Structural`/`Erased` reaching it is `UnknownStructuralType` (:420-422). `byte_sequence_shape` (:430-441): BorrowedView = 16, BoundedView = 8, BoundedOwned = capacity+8, alignment always 8. `ElementView` is fixed 16/8 (:183). Reference size is only ever 8 or the scalar-reference size.

**Same job elsewhere.**
- Integer size/alignment is computed twice in this file with different alignment caps: `size.next_power_of_two().min(16)` at :398 and :407 (field form) versus `.min(8)` at :458 (`scalar_shape`). Read.
- `scalar_shape` is defined under the same name in many other places (grepped `fn scalar_shape|…` over `omega/compilation`): `calling-conventions/src/structural_shapes.rs:529`, `image-emission/src/object_artifact.rs:3216`, `pipeline/03_…/structural_inputs.rs:72`, `pipeline/03_…/selection_call_abi.rs:1964`, `pipeline/03_…/legalization_scalar_graph.rs:8601`, `pipeline/02_…/scalar.rs:2163`. Suspected duplicates; not read.
- `byte_sequence_shape` and `checked_align_up_u32` in `pipeline/02_…/structural_layout.rs:1172,1180` are thin delegations to this file (read those 10 lines), so not duplicates.
- Other `align_up` helpers (grepped): `backend/layout/src/boundary_values.rs:298`, `runtime/task-plans/src/stack_composition.rs:576`, `calling-conventions/src/plans/call_plan_evaluation.rs:1043`, `machine-emission/src/frame_layout.rs:587,1238`, `image-emission/src/object_artifact.rs:4473`. Suspected; not read.

**Unused / outside users (grepped across `omega-rust`).**
- `StructuralLayouts`/`TerminalLayoutError`: 105 occurrences in 8 files; outside this file the real users are `pipeline/02_…/structural_layout.rs` (many), `function_signature.rs`, `control_flow_*.rs`, `pipeline/00_…/native_execution_scope.rs:151`, `runtime/task-plans/src/activation.rs:278`. So the header's "instruction lowering and task stack planning" matches (02 pipeline and task-plans both construct/consume it).
- `required_scalar_extents`: 1 outside call site (`native_execution_scope.rs:153`). `scalar_storage_type`: 1 (`structural_layout.rs:578`). `sum_layout`: 2 (`structural_layout.rs:586`, `provider-admission/.../windows_handle_storage.rs:535`). `field_shape` method: 1 outside (`structural_layout.rs:635`) plus 1 internal (:99).

**Stated limits.** None quoted. `UnsupportedStructuralSum` when `sum_layout` is asked for a non Sum/Mixed type (:92-96).

**Fail-open risks.**
- Records with `Authored/AccessView/Accessor` storage accepted at their declared size with no field cross-check (:191-203). Open (no comment; I did not trace who builds these declarations).
- `known` memo stores only `Some` shapes (:299-303); unresolved (`None`) results from `resolve_structural_shape` are turned into `UnclosedArrayExtent` in `new` (:53) but are accepted by `required_scalar_extents` (:121-125) by design (it is collecting extents). Stated by header/purpose; the closure to an error happens in `new`.
- `shape()` for an id not in the table: Err `UnknownStructuralType` (:65-67). Closed. None else seen.

---

## 2. image-elf/src/dynamic_executable.rs (1-685, read in full)

**Purpose.** Drives the 22-stage dynamic ELF pipeline and holds the shared `checked` helper module (:9-452).

**Shape.**
- `emit_elf_dynamic_executable(image: FinalImage, interpreter: NormalizedElfInterpreterPlan)` :614-663, chaining: plan_elf_dynamic_link_inputs, plan_elf_dynamic_sections, serialize_elf_dynamic_sections, plan_elf_dynamic_section_descriptors, plan_elf_procedure_linkage_relocations, plan_elf_procedure_linkage_templates, plan_elf_procedure_linkage_section_descriptors, plan_elf_dynamic_tags, serialize_elf_dynamic_table, plan_elf_dynamic_table_section_descriptor, plan_elf_section_name_table, plan_elf_dynamic_section_roster, serialize_elf_section_header_table, plan_elf_indexed_section_payloads, plan_elf_relative_section_payload_layout, plan_elf_dynamic_load_layout, apply_elf_section_header_placements, apply_elf_dynamic_address_fixups, serialize_elf_dynamic_file_envelope, apply_elf_procedure_linkage_fixups, assemble_elf_dynamic_file, admit_elf_dynamic_executable.
- `ElfDynamicExecutableEmissionError` :529 (22 boxed variants), `failing_stage` :555 maps each to a hyphenated stage string.
- `checked`: `require` :17, `checked_sum/product/u32` :23-43, `read_u8/16/32/64/i64/read_field` :57-113, `field_mut` :115, `stage_fingerprint` :127, `checked_sum_u64/product_u64` :134-152, `decode_section_header_rows` :168, `decode_dynamic_rows` :249, `CandidateValidationError` :277, `seal_stage!` :282 (3 arms), generic `validate_candidate` :365, `plan_stage!` :387 (2 arms), `plan_candidate` :429.
- Free: `interpreter_path` (cfg(test)) :666; `image_section_tag` :674; `checked_u64` :683.

**How it decides.** `validate_candidate` (:365-385): first `valid(&candidate)`, then `replays(&candidate)` (fingerprint recomputation) else the stage's `replay_diagnostic`. `seal_stage!` arms: (1) fingerprint equality; (2) fingerprint nonzero AND equal (`nonzero`); (3) no fingerprint, `|_| true` as the replay check (:353).

**Pipeline note (read).** Each stage derives contents and immediately re-validates those same contents in the same call (`plan_candidate` :429-451). So in the default emission path every stage validates its own output. (AGENTS.md says default compilation does not re-verify its own stage output; I did not read pipeline.md to see whether this is exempted for image emission.) Inferred.

**Same job elsewhere.** `image_section_tag` :674 (Text=1,Data=2,Bss=3,None=4) is re-implemented as `section_tag` in `procedure_linkage.rs:571-578` with the identical mapping (read both). Used at `section_headers.rs:2145`, `file_assembly.rs:2455`, `procedure_linkage.rs:2410` (image_section_tag) and `procedure_linkage.rs:564` (section_tag) (grep).

**Unused / outside users.** `elf_entry_address` is in lib.rs (see 7). Not grepped: individual `checked::*` helper counts.

**Stated limits.** None.

**Fail-open risks.** `seal_stage!` arm 3 `|_| true` (:353) is used by `dynamic_sections` (import_sections.rs:2669-2674). Traced: that stage's `validate_contents` replays the content fingerprint stored inside `contents` (import_sections.rs:2763-2767), so it is not an unchecked acceptance. Nothing else seen.

---

## 3. image-elf/src/imports.rs (1-435, read in full; tests :202-435)

**Purpose.** Builds canonical `ElfImportRequest`s from a `FinalImage`: each referenced import's symbol handle, a normalized locator, and every relocation that uses it (header :1-6).

**Shape.** `ElfImportLocator` :17 {`StringBackedBootstrap{library,symbol}`, `Component{locator,object,symbol}`, `Versioned{target_profile,compatibility_report_identity,object,symbol,version}`}; `ElfImportRequest` :37 {symbol_handle, locator, relocations}; `canonical_referenced_imports(&FinalImage)` :43-200.

**How it decides.** Per import row:
- duplicate symbol handle via linear `Vec::contains` (:51);
- symbol must be `SymbolKind::Import`, section `None`, offset 0, size 0 (:65-69);
- `StringBackedBootstrap`: library and symbol non-empty and NUL-free (:77-81);
- `Normalized`: Foreign locator must be `ForeignLocatorCandidate::ElfVersioned` (:95-104); Component locators synthesize object `$ORIGIN/<image_filename>` and symbol `export_symbol()` (:114-118); locator target must equal `image.target` and be LinuxArm64/LinuxX64 (:120-125);
- ambiguity keyed on the 64-bit `non_authoritative_compatibility_fingerprint` equality between two imports (:133-137); equality of the full locator only picks the message (:139-143);
- `FinalImageImportPlan::None` rejected (:154-159).
- Imports with no relocation rows are validated and then dropped from `requests` (:169-175).
- Second pass (:178-197): a relocation to an `Import` symbol with no request is an error.

**Cases by shape.** Only ELF-versioned foreign locators and component locators accepted for ELF (:95-104); Linux x64/arm64 only (:120-125).

**Same job elsewhere.** `ordered_import_requests` (import_sections.rs:2417-2469) matches the same three locator variants again; `request_compatibility_report_identity` (procedure_linkage.rs:236-251) matches them a third time (read both).

**Unused / outside users.** `canonical_referenced_imports`: called from `import_sections.rs` dynamic_link (:88); not grepped for others (static_executable.rs, shared_component.rs are outside my unit; suspected users).

**Stated limits.** `import_sections.rs:97` rejects zero referenced imports in the dynamic path; imports.rs itself returns an empty list for them.

**Fail-open risks.**
- :178-185 `continue` when a relocation's `symbol_handle` is not a valid symbol: such a relocation is skipped silently here. A relocation naming an invalid handle is not reported by this function. Open (I did not trace later stages; `relocate_image_local_memory` at file_assembly.rs:2476-2509 calls `final_image_imports_symbol` on every relocation, not read).
- Duplicate-import detection and ambiguity are by symbol handle and 64-bit fingerprint, closed by erroring on collision (:133-150). A genuinely different locator that collides on the fingerprint is rejected, not accepted (traced).

---

## 4. image-elf/src/lib.rs (1-697, read in full)

**Purpose.** Crate root: module wiring, three small shared modules, public exports, a long list of `#[cfg(test)] pub(crate) use` re-exports (:137-197), and a `test_support` module (:199-697).

**Shape.**
- `bytes` :45-78: `write_u16/32/64` re-exported from `image::codec_bytes`, `name_at` :49, `require_name` :64.
- `constants` :80-96: ELF_HEADER_SIZE 64, PROGRAM_HEADER_SIZE 56, PROGRAM_HEADER_COUNT 2, IMAGE_BASE 0x400000, PAGE_SIZE 0x1000, SECTION_COUNT 14, ELF64_SECTION_HEADER_SIZE 64, ELF64_DYNAMIC_ROW_SIZE 16, ELF64_DYNAMIC_VALUE_OFFSET 8, ELF64_DYNAMIC_VALUE_SIZE 8, ELF64_RELA_SIZE 24, SHF_* flags.
- `entry_symbol::elf_entry_address` :110 = text_address + `final_image_entry_symbol(image,"ELF")?.offset`.
- `pub use` of `emit_elf_shared_component` (shared_component.rs), `emit_elf_executable` (static_executable.rs), `emit_elf_dynamic_executable`, `ValidatedElfDynamicExecutable`.

**Header vs code.** The header (:15-18) names three page constants: `PAGE_SIZE` (0x1000), `DYNAMIC_MAX_PAGE_SIZE` (0x1_0000), `AARCH64_RELOCATION_PAGE_SIZE` (0x1000). Only `PAGE_SIZE` is in lib.rs `constants` (:85); the other two are defined in load_placement.rs:1795 and :1799 (grepped). Header says "22-stage" (read; matches).

**Duplicated constants (read/grepped).** lib.rs constants vs file_assembly.rs:1268-1274 (`ELF64_HEADER_SIZE` 64, `ELF64_PROGRAM_HEADER_SIZE` 56, `ELF64_SECTION_HEADER_COUNT` 14 redefine `ELF_HEADER_SIZE`, `PROGRAM_HEADER_SIZE`, `SECTION_COUNT`); load_placement.rs:1875-1881 and section_headers.rs:527-532 both define the section-header field offsets 16/24, field size 8, and the counts 13/12/25 (`FILE_OFFSET_*`, `VIRTUAL_ADDRESS_*`, `PLACEMENT_*`); `SECTION_NAME_TABLE_INDEX = 13` at load_placement.rs:1881 and section_headers.rs:2457; PLT/GOT sizes at procedure_linkage.rs:2907-2917 (usize) and file_assembly.rs:3531-3541 (u64); `ELF64_RELA_SIZE` at lib.rs:92 and procedure_linkage.rs:1138; `PLACEMENT_FIXUP_COUNT = 25` at load_placement.rs:1791 as well as `PLACEMENT_COUNT = 25` at :1880.
- `EM_X86_64`/`EM_AARCH64`/ELF header size names also appear in static_executable.rs (grep count 11 for those names across the 4 files in the crate; static_executable.rs not in my unit).

**Unused/one-caller.** `elf_entry_address`: 2 call sites (static_executable.rs:214, file_assembly.rs:1677) (grep). Note envelope code (file_assembly.rs:1637-1678) additionally checks entry kind==Function, size!=0, in-bounds, which `elf_entry_address` alone does not (static path may or may not; not read).

**Stated limits.** Header :6-8: static executables "fail closed before image mutation if any referenced import exists" (code in static_executable.rs, not in my unit).

**Fail-open risks.** None seen in this file. `test_support` uses `unreachable!()` for non-Linux targets, test only.

---

## 5. image-elf/src/dynamic_table.rs (1-2219, read in full)

**Purpose.** Pipeline stages 8-10 (header :1-6): semantic `.dynamic` tag plan, serialized `Elf64_Dyn` bytes, `.dynamic` section descriptor.

**Shape.**
- `dynamic_table_descriptor` :8 (plan fn :118, `derive_contents` :133, `validate_contents` :180, fingerprint :263, tests :292-491).
- `dynamic_tag_bytes` :494 (`serialize_elf_dynamic_table` :580, `encode_contents` :592, `validate_contents` :642, `validate_fixups` :662, `validate_fixup_coverage` :710, tests :752).
- `dynamic_tags` :1018 (`ElfDynamicTag` enum :1114, `ElfDynamicValue` :1137, `plan_elf_dynamic_tags` :1187, `derive_contents` :1199, `validate_contents` :1371, `validate_fixed_rows` :1466, `validate_address_obligations` :1576, `validate_relocation_closure` :1650, tests :1739).

**How it decides.**
- Tag row order is fixed: DT_NEEDED prefix, PLTRELSZ, PLTGOT, HASH, GNU_HASH, STRTAB, SYMTAB, STRSZ, SYMENT, PLTREL, JMPREL, RELA, RELASZ, RELAENT, then (only if `verneed` non-empty) VERSYM, VERNEED, VERNEEDNUM, then FLAGS=DF_BIND_NOW (0x8), NULL (:1199-1315). Row count formula `needed + 15 + 3*has_verneed` (:1205, :1393).
- Preconditions: sealed base has exactly 11 descriptors (:1375-1378) and 7 base-name descriptors in the descriptor stage (:185-193); target LinuxX64/LinuxArm64 (:1379-1385); at least one DT_NEEDED (:1387-1390); DT_NEEDED names strictly ascending byte-wise (:1449-1454).
- Relocation closure keyed on target: expected number of `GotPlt Absolute64 FutureDynamicSection` fixups is 1 for LinuxX64, 0 for LinuxArm64 (:1673-1681).
- `.dynamic` descriptor: name offset 113 into a hard-coded 113-byte upstream seed (:20-22, :113-193), flags WRITE|ALLOC, link `.dynstr`.

**Cases by shape.** Hard-coded: 11 upstream descriptors, 113-byte seed, 7 or 9 address obligations (:1037-1039, :1615), DF_BIND_NOW always (:1310). No lazy-binding option.

**Same job elsewhere (read).** `derive_contents` builds the row list at :1218-1315 and `validate_fixed_rows` rebuilds the same list at :1472-1539; the address-target list appears at :1225-1243 and :1580-1614; `dynamic_payloads`/`structural_contents`/`target` accessor chains repeat across modules (e.g. `retained_target` load_placement.rs:512-528, `target` procedure_linkage.rs:2009, :1419, dynamic_tags.rs:1354).

**Unused / outside users.** `ValidatedElfDynamicTagPlan::has_version_requirements` used in load_placement.rs:2550 (read). `needed_row_count`, `appended_descriptor_count`, etc. are cfg(test). Others not grepped.

**Stated limits.** None quoted; messages such as "ELF dynamic tags require at least one owned DT_NEEDED row" (:1389).

**Fail-open risks.** None seen: every check is `require`. `&expected[..expected_count]` (:1616) slices only 7 or 9 of 9 entries. `validate_address_obligations` zips obligations to `expected`, with count checked first (:1617-1620).

---

## 6. image-elf/src/import_sections.rs (partially read: 1-139, 340-1001, 1302-1806, 2146-2936 read; 140-339, 1002-1301, 1807-2145, 2937-3294 not read)

**Purpose.** Stages 1-4 (header :1-8): preflight inputs, plan dynamic section contents, serialize them, describe them.

**Shape.**
- `dynamic_link` :10 -> `PlannedElfDynamicLinkInputs` {image, interpreter, imports} :18, `plan_elf_dynamic_link_inputs` :66.
- `dynamic_section_bytes` :340 -> `serialize_elf_dynamic_sections` :451, `encode_payloads` :463, decoders `decode_dynsym` :670, `decode_sysv_hash` :702, `decode_gnu_hash` :760, `decode_versym` :855, `decode_verneed` :867, fingerprint :973.
- `dynamic_section_descriptors` :1302 -> `plan_elf_dynamic_section_descriptors` :1446, `derive_contents` :1461, `validate_contents` :1589, `validate_row` :1669, `validate_cross_payloads` :1756.
- `dynamic_sections` :2146 -> `plan_elf_dynamic_sections` :2313, `derive_import_contents` :2350, `derive_shared_import_contents` :2343 (pub(crate)), `ordered_import_requests` :2417, `build_dynstr` :2504, `assign_version_indexes` :2523, `build_sysv_hash` :2560 (pub(crate)), `build_gnu_hash` :2586, `build_version_needs` :2614, `validate_contents` :2676, hash fns `elf_hash` :2841, `gnu_hash` :2854.

**How it decides.**
- Link-input preflight rejects: interpreter target != image target (:78), no imports (:92-100), `StringBackedBootstrap` locator (:106-114), import target != interpreter target or not Linux arm64/x64 (:116-129).
- Imports sorted by (symbol, object, version, fingerprint) (:2454-2467); `.dynstr` is the sorted unique set of object/symbol/version strings (:2471-2481); each dynsym is `STB_GLOBAL<<4|STT_FUNC`, `SHN_UNDEF`, value 0 (:2367-2372).
- Version index: empty version = index 1; else 2 + index in sorted unique (object, version) pairs, must be <= 0x7fff (:2523-2558). Component imports get empty version (:2447).
- GNU hash: fixed 1 bucket, 1 bloom word, shift 5, symoffset 1 (:2160-2163, :2586-2612). SysV hash: bucket count = max(symbols,1) (:2561).
- Descriptor stage: hard-coded 113-byte... (it is the 8-name seed, `SECTION_NAME_TABLE_SEED` :1327-1328) with fixed name offsets 1,9,17,25,31,44,59,69 (:1318-1325); all seven sections are `SHF_ALLOC` only (:1474-1540).
- Replay: `validate_payloads` re-decodes bytes and compares to the plan (:598-668); `dynamic_sections::validate_contents` rebuilds everything from `inputs` with the same builder functions (:2676-2768) and compares.

**Cases by shape.** Imports are only functions (STT_FUNC) (:2367); component imports unversioned; string-backed bootstrap refused (:111, :2449-2451). `decode_verneed` accepts only canonical chains with `vn_aux == 16` and `next` equal to the canonical group size (:884-949).

**Same job elsewhere (read).** Descriptor literals duplicated in `derive_contents` (:1469-1547) and `validate_row` (:1678-1742). `dynamic_sections::validate_contents` reuses the derive builders rather than an independent encoding, so for this stage validation is a recomputation, not an independent check (inferred from :2688-2762). `derive_shared_import_contents` (:2343) and `build_sysv_hash` (:2560) are reused by `shared_component.rs` (grep: shared_component.rs:96 and :121, i.e. 1 non-test outside caller each; test at import_sections.rs:3195 — inside a range I did not read).

**Unused / outside users.** `derive_shared_import_contents`: 1 non-test caller outside (shared_component.rs:96), 1 inside tests (grep over `omega-rust`). `build_sysv_hash`: callers shared_component.rs:121, import_sections.rs:2386, :2740.

**Stated limits.** `import_sections.rs:111` "string-backed ELF bootstrap imports cannot enter normalized dynamic-link planning"; `:2535-2536` "required-version count exceeds the unhidden version-index domain".

**Fail-open risks.** None seen in read ranges. `validate_row` compares to constants (:1743-1753). `decode_sysv_hash` chain walk bounded by `chain_count` (:740-751).

Unread: 140-339 (tests of dynamic_link), 1002-1301 (tests), 1807-2145 (tests), 2937-3294 (tests).

---

## 7. image-elf/src/procedure_linkage.rs (partially read: 1-580, 1129-1589, 1955-2958 read; 581-1128, 1590-1954, 2959-3248 not read, all test modules per grep)

**Purpose.** Stages 5-7 (header :1-7): relocation classification into logical PLT/GOT slots and JUMP_SLOT/RELA rows; PLT/GOT/RELA template bytes and fixups for x86-64 and AArch64; section descriptors for `.plt`, `.got.plt`, `.rela.plt`, `.rela.dyn`.

**Shape.**
- `dynamic_import_relocations` :9 -> `plan_elf_procedure_linkage_relocations` :140, `derive_contents` :155, `target_relocation_spec` :216, `canonical_relocation_classes` :253, `is_direct_call_shape` :286, `general_slot` :296, `call_site` :324, `validate_contents` :434.
- `dynamic_linkage_descriptors` :1129 -> `plan_elf_procedure_linkage_section_descriptors` :1267, `derive_contents` :1282, `validate_contents` :1439.
- `dynamic_linkage_templates` :1955 with sub-modules `candidates` :1957 (`derive_contents` :1985, `encode_template_bytes` :2021, `derive_fixups` :2105, `fixup_field` :2290, `derive_constraints` :2302), `compatibility_fingerprint` :2347, `template_plans` :2417 (types), `template_validation` :2576 (`validate_contents` :2597, `replay_x86_plt` :2687, `replay_aarch64_plt` :2716, `replay_rela_rows!` macro :2743), constants :2907-2941, `plan_elf_procedure_linkage_templates` :2943.

**How it decides.**
- Per-relocation classification: "direct call shape" = Text section, kind equals the target's expected kind (X86_64Relative32 or Aarch64Branch26), width 4, addend 0 (:286-294); otherwise it must be a Data `Absolute64` of width 8, offset multiple of 8, within `.data` (:296-322). Anything else: error "ELF dynamic import requires a non-procedure or malformed source relocation" (:307, :335).
- Call-site bytes must be exact placeholders: x86 `0xe8` opcode followed by `00 00 00 00`; AArch64 `00 00 00 94` and 4-byte alignment (:342-361).
- Target tables: LinuxX64 uses R_X86_64_JUMP_SLOT (7) / R_X86_64_64 (1); LinuxArm64 R_AARCH64_JUMP_SLOT (1026) / R_AARCH64_ABS64 (257) (:20-23, :216-234).
- PLT layout constants: x86 header 16, entry 16; AArch64 header 32, entry 16; GOT header 3 words (:2907-2917). x86 PLT entry is `ff 25 .. 68 <idx> e9 ..` (:2049-2051); AArch64 entries are one fixed 16-byte stub (:2936-2941). Policy enum names: `X86_64SmallMediumLazy`, `Aarch64StandardLazy` (:2503-2506).
- Constraint kinds (placement envelopes) derived from fixup kinds (:2302-2332).
- `.plt` flags on AArch64 include `SHF_AARCH64_PURECODE` (0x2000_0000) (:1142, :1331).

**Cases by shape.** Only Linux x64/arm64 (:216-234, :1329-1337); one PLT slot per import binding (`slots.len() == bindings.len()`, :442-446); general (data) relocations only 8-byte absolute (:303-306). Stale-limit-free comments: none.

**Same job elsewhere (read).**
- `section_tag` (:571-578) duplicates `image_section_tag` (dynamic_executable.rs:674).
- `hash_semantic_target` (:2385-2414, tags 1..7) vs `hash_procedure_target` (section_headers.rs:2120-2149, tags 0..6) vs `hash_target` (file_assembly.rs:2430-2459, tags 1..7): three hashers over the same fixup-target concept with different tag numbering and different enum types (`ElfProcedureLinkageSemanticTarget` vs `ElfAppliedProcedureLinkageTarget`).
- `target()` accessor chains duplicated (:1419-1428, :2009-2019).
- Derive/validate duplication of descriptor tables (:1339-1386 vs :1481-1530).

**Unused / outside users.** `ElfProcedureLinkageFixupKind` is a type alias to `crate::ElfAppliedProcedureLinkageKind` (:2536), defined in the downstream stage (file_assembly.rs:2849) and re-exported from lib.rs:129; i.e. an upstream stage's type is owned by a later stage. Read.

**Stated limits.** None quoted.

**Fail-open risks.**
- `validate_semantic_target` (:2827-2859): `FutureDynamicSection | PltHeader => true` without further check. Traced: those variants carry no ordinal/offset, so there is nothing to resolve; the others are checked against `slots`/`GOT_PLT_HEADER_WORDS`/`general_relocations`. Not a risk.
- `require_unique_semantic_rows` (:497-517) compares only ordinal, identity, and dynamic symbol index across slots; not a risk.
- None else seen.

Unread: 581-1128, 1590-1954, 2959-3248 (tests).

---

## 8. image-elf/src/section_headers.rs (partially read: 1-322, 515-867, 1157-1443, 1639-2151, 2442-2981 read; 323-514, 868-1156, 1444-1638, 2152-2441, 2982-3183 not read, test modules per grep)

**Purpose.** Stages 11-15 (header :1-8): `.shstrtab` plan, numeric section roster, `Elf64_Shdr` template serialization with placement fixups, indexed payload plan, and relative offsets/regions.

**Shape.**
- `relative_section_layout` :10 (`plan_elf_relative_section_payload_layout` :137, `derive_contents` :150, `validate_contents` :206, `physical_section_order` :269, `classify_region` :273, `checked_align` :289, regions `ElfRelativeSectionPayloadRegion` {ReadOnly, ReadExecute, ReadWrite, FileOnly} :21).
- `section_header_bytes` :515 (`serialize_elf_section_header_table` :612, `encode_contents` :625, `validate_contents` :700, `validate_fixups` :739).
- `section_name_table` :1157 (`plan_elf_section_name_table` :1265, `validate_contents` :1317, `validate_complete_names` :1364; fixed 122-byte `COMPLETE_SECTION_NAME_TABLE` :1170).
- `section_payload_roster` :1639 (`plan_elf_indexed_section_payloads` :1762, `derive_contents` :1775, `upstream_payload` :1852, `indexed_procedure_storage` :1881, `procedure_target_section` :1907, `validate_contents` :1934).
- `section_roster` :2442 (`plan_elf_dynamic_section_roster` :2555, `derive_contents` :2567, `validate_contents` :2736, `validate_row_against_owner` :2812, `validate_references` :2927).

**How it decides.**
- Section roster is a fixed 14-row table in a fixed order (Null, .interp, .dynstr, .dynsym, .hash, .gnu.version, .gnu.version_r, .gnu.hash, .plt, .got.plt, .rela.plt, .rela.dyn, .dynamic, .shstrtab) (:2459-2474). Section index equals the enum discriminant of `ElfDynamicRosterSectionKind` (`index_for_kind = kind as u32` :2723-2725; enum in load_placement.rs:1095-1110).
- Region classification keyed on flags (`SHF_ALLOC`, `SHF_WRITE`, `SHF_EXECINSTR`): (no flags)=FileOnly, ALLOC=ReadOnly, ALLOC|EXEC=ReadExecute, ALLOC|WRITE=ReadWrite, else error (:273-287). Physical file order is `[0,1,2,3,4,5,6,7,8,9,11,10,12,13]` (:269-271), i.e. `.rela.dyn` (11) before `.rela.plt` (10).
- Hard-coded section indexes: storage/target indexes 8,9,10,11,12 in `indexed_procedure_storage`/`procedure_target_section` (:1881-1917), `storage_section_index: 12` (:1813), `validate_references` link/info expectations `link == 2/3`, `info == 1`, ProcedureRelocation `info == 9` (:2940-2951), `link: ... => 3` (:2876), roster indices 12/13 (:2890, :2909).
- Name table: fixed bytes equal `COMPLETE_SECTION_NAME_TABLE` (:1335-1341); each name must appear at a fixed offset list (:1365-1380).
- Placement fixups: for each non-null row, one FileOffset fixup; for allocated rows also a VirtualAddress fixup (:647-660); expected totals 25 = 13 + 12 (:530-532).

**Stale / inconsistent messages (read).** Error strings disagree with constants: `section_header_bytes::validate_contents` says "exact thirteen-row roster" (:706) but compares to `SECTION_COUNT` = 14; `validate_fixups` says "exactly twenty-three" (:746) but `PLACEMENT_FIXUP_COUNT` = 25; "exactly eleven allocated rows" (:754) vs `VIRTUAL_ADDRESS_FIXUP_COUNT` = 12. The same pattern in load_placement.rs: "exactly twelve file offsets" (:2117) vs `FILE_OFFSET_PLACEMENT_COUNT` = 13; "exactly eleven virtual addresses" (:2128) vs 12. Only the messages are wrong; the checks use the constants.

**Same job elsewhere (read).**
- `checked_align` :289 (rounds by modulo, `alignment.max(1)`, no power-of-two check) vs `checked_align` load_placement.rs:1061-1076 (mask-based, requires power of two, `alignment<=1` is identity) vs `checked_align_up_u32` terminal_layout.rs:444 (different crate).
- The 13/12/25 counts and offsets 16/24 duplicated in section_headers.rs:527-532 and load_placement.rs:1875-1881.
- `derive_contents` (:2567-2657) and `validate_row_against_owner` (:2812-2925) build the same rows twice; the arm 2874-2877 uses literal `3` where derive uses `index_for_dynamic_kind(DynamicSymbol)` (:2614).
- `hash_procedure_target` (:2120-2149): see item 7.

**Unused / outside users.** `ElfRelativeSectionPayloadRegion` is `pub`; used by load_placement.rs (read). Others not grepped. `ElfDynamicRosterSectionKind` is a type alias for `crate::ElfPlacedDynamicSectionKind` (:2533); `ElfSectionHeaderPlacementFixupKind` alias for `crate::ElfSectionPlacementResolutionKind` (:595).

**Stated limits.** `classify_region` error text (:284): "rejects write/execute flags without allocation or writable executable sections" (W+X rejected).

**Fail-open risks.** None seen. `validate_references` only checks `row.link < SECTION_COUNT` for all rows and exact link/info for 10 named sections (:2927-2953); `sh_info` of `.dynsym` must be 1 (:2942). Rows not named there (e.g. `.interp`, `.dynstr`, `.plt`, `.got.plt`) have `link` bounded but not equal to 0; but `validate_row_against_owner` (:2812-2925) rebuilds each row from its owner and compares, so closed (traced).

Unread: 323-514, 868-1156, 1444-1638, 2152-2441, 2982-3183.

---

## 9. image-elf/src/load_placement.rs (partially read: 1-1296, 1660-1806, 1807-2238, 2509-3096 read; 1297-1659, 2239-2508, 3097-3379 not read; all test modules per grep)

**Purpose.** Stages 16-18 (header :1-7): choose file offsets, virtual addresses, and program headers; patch section-header addresses/offsets; patch `.dynamic` address fields.

**Shape.**
- `load_layout` :9 with sub-modules `abi_validation` :11 (`validate_abi` :40, `validate_deferred_constraint_envelope` :425, `aarch64_page_delta_covers_extent` :492, `retained_target` :512, `retained_image` :530, `section` :547, `align_for_region` :557, `placed_coordinates` :573), `candidates` :587 (`Candidate` :618, `derive_contents` :641, `validate_candidate` :907), `compatibility_fingerprint` :978 (fingerprint :987, `target_tag` :1047, `checked_align` :1061), `load_headers` :1079 (`ElfLoadProgramHeaderKind` :1085, `ElfPlacedDynamicSectionKind` :1095 (discriminants 0..13), `public_section_kind` :1112, `ElfLoadProgramHeader` :1119, `ElfPlacedDynamicSection` :1165, `ElfLoadImageMemoryPlacement` :1219), `validated_layout` :1663 (`ValidatedElfDynamicLoadLayout` :1677, `ElfDynamicLoadLayoutError` :1762), constants :1791-1805 (PLACEMENT_FIXUP_COUNT 25, DYNAMIC_PROGRAM_HEADER_COUNT 5, DYNAMIC_MAX_PAGE_SIZE 0x1_0000, DYNAMIC_LOAD_POLICY_TAG 1, AARCH64_RELOCATION_PAGE_SIZE 0x1000, PF_X/W/R), `plan_elf_dynamic_load_layout` :1807.
- `placed_section_headers` :1861 (`apply_elf_section_header_placements` :1944, `derive_contents` :1957, `validate_contents` :2024).
- `resolved_dynamic_table` :2509 (`apply_elf_dynamic_address_fixups` :2698, `derive_contents` :2710, `application_for_fixup` :2752, `validate_contents` :2808, `target_section_index` :3008, `target_section_kind` :3022, `target_tag` :3054).

**How it decides.**
- Layout: read-only load starts at file offset 0 / `IMAGE_BASE` and owns the ELF+program headers (5 program headers); read-execute base = read-only end aligned to 0x1_0000, source text first then RX sections; read-write base likewise aligned to 0x1_0000, RW sections, then source data, then BSS; file-only (`.shstrtab`) after data; section-header table after `.shstrtab` aligned 8 (:677-781). Virtual address = `IMAGE_BASE + file_offset` for every allocated placement (`placed_coordinates` :573-584). Program header order is fixed Interpreter, LoadReadOnly, LoadReadExecute, LoadReadWrite, Dynamic (:40-56, :802-857).
- `validate_candidate` (:907-975) re-derives and compares, checks fingerprint is nonzero and equal, then runs `validate_abi`: PT_LOAD alignment/congruence, strict W^X (W and X not both set) :65-68, permissions exactly R / RX / RW :74-79, no overlap, every allocated section lies in its permission load, text/data/BSS contained, pointer alignment, `.shstrtab` file-only after RW end, section-header table after `.shstrtab`.
- Deferred constraint envelope (:425-490): x86 signed-32 span, AArch64 branch-26 span < 2^27 and 4-aligned, ADRP page delta within +/-2^20 pages, LDR low-12 target 8-aligned.
- Dynamic table address application: expected target list is the 9-element `EXPECTED_TARGETS` (:2535-2545), truncated by 2 when no version requirements (:2547-2555); section index per target is a literal table (:3008-3019).

**Cases by shape.** Exactly 5 program headers and 3 PT_LOADs with fixed permissions; Linux x64/arm64 only (:645-648); every one of the four relative regions must be non-empty (:654-664) (so an image missing e.g. any read-write section is rejected); `max_page_alignment` is a single 64 KiB value for both architectures (`DYNAMIC_MAX_PAGE_SIZE` :1795, header lib.rs:16-18 gives the reason). BSS alignment must be a power of two (:702-705).

**Same job elsewhere (read).**
- `target_section_index` (:3008-3019) is a hand-written table of what `target_section_kind(target) as u32` (:3022-3052, with discriminants :1095-1110) would give; both in the same file. Likewise `retained_target`/`retained_image` (:512-545) are accessor chains also written in dynamic_tags.rs:1354, procedure_linkage.rs:1419 and :2009, and in file_assembly.rs:274-286.
- Identity conversions: `public_section_kind` :1112-1116 returns its argument; `public_target` :3004-3006 returns its argument; `public_kind` file_assembly.rs:3460-3464 returns its argument; a private duplicate `public_section_kind` at file_assembly.rs:3498-3502. Grep: `public_section_kind` 16 occurrences in load_placement.rs (12) and file_assembly.rs (4); `public_kind(`/`public_target(` 9 occurrences in the same two files.
- `ElfDynamicAddressTarget` (dynamic_table.rs:1172) is a type alias for `ElfDynamicAddressApplicationTarget` defined here (:2566).
- Two `checked_align`s (see item 8).

**Unused / outside users.** `ElfDynamicAddressApplicationKind` has a single variant and is `#[cfg(test)]` only (:2557-2562). `ElfAppliedDynamicAddress` getters are all `#[cfg(test)]` (:2591-2636).

**Stated limits.** Several diagnostics are phrased as scope limits: "dynamic ELF load placement requires an exact Linux x86-64 or AArch64 target" (:647), "requires every closed relative region" (:662).

**Fail-open risks.** None seen. In `abi_validation`, `ElfRelativeSectionPayloadRegion::FileOnly => unreachable!()` (:152) is guarded by the preceding `continue` for FileOnly (:141-147) (traced). `placed.virtual_address.unwrap_or(0)` in the program-header alias check (:362) compares against a header whose address was earlier required to be `Some`, so a missing address would mismatch rather than pass (traced for the dynamic section; for interpreter the derive requires `Some` at :807-811).

Unread: 1297-1659, 2239-2508, 3097-3379.

---

## 10. image-elf/src/file_assembly.rs (partially read: 1-752, 1251-1731, 2089-3558 read; 753-1250, 1732-2088, 3559-3947 not read; all test modules per grep)

**Purpose.** Stages 19-22 (header :1-7): build the ELF64 header + program-header prefix, apply `.plt`/`.got.plt`/`.rela` and image-local relocations, assemble the file, and "admit" the result.

**Shape.**
- `assembled_dynamic_file` :10 (`ElfDynamicFileFragmentKind` :35, `assemble_elf_dynamic_file` :172, `admit_elf_dynamic_executable` :184, `derive_executable_output` :213, `procedure_got_import_custody` :254, `validate_executable_output` (cfg(test)) :353, `dynamic_executable_format` :393, `recover_retained_image` :403, `expected_fragments` :523, `section_bytes` :593, `validate_file_extents` :627, `fragment_kind_order` :707).
- `dynamic_file_envelope` :1251 (`serialize_elf_dynamic_file_envelope` :1378, `derive_contents` :1391, `validate_contents` :1453, `decode_header` :1589, `decode_program_headers` :1611, `expected_entry_address` :1637, `target_machine` :1696).
- `resolved_procedure_linkage` :2089 with `candidates` :2090 (`derive_contents` :2119, `derive_application` :2167, `validate_contents` :2217), `compatibility_fingerprint` :2394, `image_local_relocations` :2462 (`relocate_image_local_memory` :2476), `linkage_fixups` :2832 (types), `storage_fields` :2998 (`storage_address` :3021, `semantic_target_address` :3050, `encode_field` :3190, `decode_rejoins_target` :3259, `upstream_storage_bytes` :3326), constants :3531-3541, `apply_elf_procedure_linkage_fixups` :3543.

**How it decides.**
- Fragments: header prefix at 0, source text at its placed offset, source data, every non-null section at its placed offset, and the section-header table; sorted by (offset, kind order) (:535-590, :707-715). No fragment may overlap (:618-621, :501-504) and bytes outside fragments must be zero (:512-519).
- Header: ET_EXEC; machine = EM_X86_64 (62) for LinuxX64, EM_AARCH64 (183) for LinuxArm64, else error (:1696-1704); 5 program headers; 14 section headers; entry address must come from an entry symbol that is `SymbolKind::Function`, in `Text`, non-zero size, inside text (:1637-1678).
- Fixup encoding by kind (:3190-3257): x86 PC-rel32 must fit signed 32; AArch64 ADRP within +/-2^20 pages; LDR low12 must be 8-aligned; B/BL within +/-2^25 words; Absolute64 / RelaOffset store the target address. Every encoded value must fit the typed mutable mask (:3252-3255). `decode_rejoins_target` (:3259-3307) re-decodes and compares.
- `.got.plt` import custody: builds `FinalDataRegion`s per slot with the symbol spelled as UTF-8 (`String::from_utf8`, error if not UTF-8) (:331-346), using a `BTreeMap` keyed by logical ordinal (:288).

**Cases by shape.** Linux x64/arm64 only (:393-401, :1696-1704, :2496-2503). Image-local relocations are applied by `image::apply_relocations` after dropping every relocation that targets an import (:2480-2486). Output `file_name` is the literal "omega-program" and `format` is "elf64-x86-64-dynamic-executable" / "elf64-aarch64-dynamic-executable" (:240-241, :393-396).

**Same job elsewhere (read).**
- Hard-coded section indexes 8..12 in `section_bytes` (:598-605), `derive_contents` (:2128-2149), `validate_contents` (:2231-2249), `upstream_storage_bytes` (:3335-3346): the same index/kind pairs as section_headers.rs:1881-1917 and load_placement.rs:3008-3019.
- `hash_fragment_kind` (:738-750) and `hash_target` (:2430-2459): see item 7.
- ELF header constants (:1268-1274) duplicate lib.rs constants (item 4).
- Entry-symbol checks (:1637-1678) overlap with `image::final_image_entry_symbol` used by `elf_entry_address` (lib.rs:110-115).
- `recover_retained_image` (:403-427) unwinds 17 nested carriers via `into_parts()`.

**Unused / outside users.** `validate_executable_output` is `#[cfg(test)]` (:352-353); grep shows its 7 hits are all in file_assembly.rs (test module lines 759-1246).

**Stated limits.** Header :6-7 says `admit_elf_dynamic_executable` is "re-verifying final-byte admission"; see fail-open (a).

**Fail-open risks.**
- (a) `admit_elf_dynamic_executable` (:184-211) only derives an `ExecutableImageOutput` from the assembled file (`derive_executable_output` :213-252); the function that compares that output to the assembled file and checks `\x7fELF`/class/data bytes (`validate_executable_output` :353-391) is test-only. In production, admission does not re-verify the final bytes. Traced within this file and by grep. The bytes come from `assemble_elf_dynamic_file`, which did validate itself at stage 21 (`validate_contents` :464-521), so a wrong program would have to pass that.
- (b) `resolved_procedure_linkage::candidates::validate_contents` (:2217-2337) does not check the length of `general_relocation_bytes` (:2227-2252 check text, plt, got, rela_plt only), does not include `GeneralRelocation` in the unchanged-bytes loop (:2323-2335 lists SourceText, ProcedureLinkage, ProcedureGot, ProcedureRelocation), and the stage fingerprint does not hash `general_relocation_bytes` (:2409-2413; hashes text, data, plt, got, rela_plt). `derive_contents` clones the upstream `.rela.dyn` bytes (:2146-2151) and writes only the offset fields; per-fixup field checks do cover `RelaDyn` fixups (via `upstream_storage_bytes` :3345-3347 and `storage_bytes` :3377-3379). So a changed byte of `.rela.dyn` outside the fixup-owned r_offset words would not be caught at this stage, and later stages `section_bytes` (:602) and assembly (:464-521) replay from the same value. Open; not reachable from input in the normal flow because derive and validate run back to back on unmodified data (traced), but it is a gap in the replay check.
- (c) `validate_nonoverlapping_applications` and `validate_unchanged_bytes` use O(n^2) scans (:2339-2391); not accept/reject relevant.

Unread: 753-1250, 1732-2088, 3559-3947.

---

## Coverage

- **Read in full:** terminal_layout.rs 1-486 (486); dynamic_executable.rs 1-685 (685); imports.rs 1-435 (435); lib.rs 1-697 (697); dynamic_table.rs 1-2219 (2219).
- **Read in part (non-test code):** import_sections.rs 1-139, 340-1001, 1302-1806, 2146-2936 (2,097 lines); procedure_linkage.rs 1-580, 1129-1589, 1955-2958 (2,045); section_headers.rs 1-322, 515-867, 1157-1443, 1639-2151, 2442-2981 (2,015); load_placement.rs 1-1296, 1660-1806, 1807-2238, 2509-3096 (2,463); file_assembly.rs 1-752, 1251-1731, 2089-3558 (2,703).
- **Total read:** 15,845 of 21,573 lines.
- **Skimmed:** none.
- **Not read (5,728 lines):** import_sections.rs 140-339, 1002-1301, 1807-2145, 2937-3294 (1,197); procedure_linkage.rs 581-1128, 1590-1954, 2959-3248 (1,203); section_headers.rs 323-514, 868-1156, 1444-1638, 2152-2441, 2982-3183 (1,168); load_placement.rs 1297-1659, 2239-2508, 3097-3379 (916); file_assembly.rs 753-1250, 1732-2088, 3559-3947 (1,244). Per my grep of `mod tests` / `#[cfg(test)]` line numbers these are the test modules of those files (lines 1297-1659 of load_placement.rs begin at `#[cfg(test)]` 1296; I did not confirm every line of every unread range is test code).

## Five things a maintainer most needs to know about this unit

1. The ELF dynamic emitter is a chain of 22 value-consuming stages, each of which derives its contents and re-validates them in the same call (`plan_stage!`, dynamic_executable.rs:387-451); the validator is mostly a second derivation built from the same literals and sometimes the same builder functions (e.g. import_sections.rs:2688-2762), so a bug in the shared builder passes both sides.
2. The roster is fixed: 14 sections, 5 program headers, 3 PT_LOADs, one GNU-hash bucket, Linux x86-64/AArch64 only, at least one DT_NEEDED import, versioned or component locators only. Section indexes 8-13 and link/info values (2, 3, 9) are literals repeated across section_headers.rs, load_placement.rs and file_assembly.rs instead of derived from one table.
3. Final-byte admission (`admit_elf_dynamic_executable`) does not re-verify in production; its verifier `validate_executable_output` is `#[cfg(test)]` (file_assembly.rs:184-211 vs :352-391), despite the header's wording at :5-7.
4. The resolved-procedure-linkage stage does not check or hash `.rela.dyn` bytes outside the fixup-owned fields (file_assembly.rs:2227-2252, :2323-2335, :2409-2413).
5. Several small helpers exist two or three times with different details: `image_section_tag` vs `section_tag`; three target-hashers with different tag numbering; two `checked_align` (modulo vs mask, power-of-two check only in one); integer alignment cap 16 vs 8 in terminal_layout.rs:398 vs :458; identity `public_*` conversion functions; and error messages whose spelled-out counts (thirteen, twenty-three, eleven) disagree with the constants (14, 25, 12, 13).

## What I am unsure about

- The 5,728 test-module lines I did not read could contain non-test helpers that I have not seen (the grep showed `#[cfg(test)]` at the start of each; I did not verify the end of each tests module).
- Whether any of the `public_*`/alias indirections are required by visibility rules rather than leftovers (I only read their definitions).
- Whether `static_executable.rs` and `shared_component.rs` (outside this unit) repeat the header/constant/import-matching logic from items 3-4; I only grepped counts.
- Whether `final_image_imports_symbol` / `apply_relocations` (image crate) catch the invalid relocation symbol handle that imports.rs:179-185 skips.
- Whether the "validate your own output in the default path" pattern is intended for image emission under the AGENTS.md verification-placement rule (pipeline.md not read).
- Whether `evaluate_conventional_sum_layout` (called from terminal_layout.rs:343, not read) shares the `.min(16)`/`.min(8)` alignment split I noted.
