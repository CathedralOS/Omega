# Record: unit_021 (area 11-native-stages-11-15)

Paths are relative to `source-rust/subsystems/compiler/psi-to-native/pipeline/15_objects-to-image/` (call it `P15`).
All seven files were read in full with the Read tool. Counts below come from the Grep tool in count mode over
`source-rust/` (or the stated directory). No build or test was run.

---

## 1. `P15/image-elf/src/section_headers.rs` (3183 lines)

**Purpose.** Steps 11 to 15 of the 22-step dynamic-ELF writer (header comment, `:1-8`). It builds the `.shstrtab`
name table, the 14-row numeric section roster, the serialized `Elf64_Shdr` table with zeroed placement fields,
the per-section payload list with indexed fixups, and the section-relative file layout. It writes nothing to
disk; every step returns a sealed "validated" object that owns the previous step's object.

**Shape.** Five `pub(crate) mod`s in one file, in this source order:
- `relative_section_layout` (`:10`): `ValidatedElfRelativeSectionPayloadLayout` (`:30`),
  `ElfRelativeSectionPayloadRegion` {ReadOnly, ReadExecute, ReadWrite, FileOnly} (`:21`),
  `ElfRelativeSectionPayloadPlacement` (`:94`, index, kind, region, relative_file_offset, byte_size, alignment),
  entry `plan_elf_relative_section_payload_layout` (`:137`), `derive_contents` (`:150`).
- `section_header_bytes` (`:515`): `ValidatedElfSectionHeaderTableTemplate` (`:535`),
  `ElfSectionHeaderPlacementFixup` (`:598`), entry `serialize_elf_section_header_table` (`:612`),
  `encode_contents` (`:625`).
- `section_name_table` (`:1157`): `ValidatedElfSectionNameTablePlan` (`:1173`), entry `plan_elf_section_name_table`
  (`:1265`).
- `section_payload_roster` (`:1639`): `ValidatedElfIndexedSectionPayloadPlan` (`:1660`), rows, procedure fixups,
  procedure constraints, dynamic fixups; entry `plan_elf_indexed_section_payloads` (`:1762`).
- `section_roster` (`:2442`): `ValidatedElfDynamicSectionRoster` (`:2478`), `ElfNumericSectionDescriptor` (`:2536`),
  entry `plan_elf_dynamic_section_roster` (`:2555`).
Each step uses the same skeleton: a private `Candidate`, `checked::plan_stage!` (derive, fingerprint, validate)
and `checked::seal_stage!` (e.g. `:141`, `:197`, `:616`, `:691`). Each defines an error type via
`image::rejection_carrier!` that gives the input plan back on failure (e.g. `:82`).

**Inputs and outputs.** Input to the first listed step (name table) is `ValidatedElfDynamicTableSectionDescriptorPlan`
from `dynamic_table.rs`; each later step takes the previous validated object (name table -> roster -> header
template -> indexed payloads -> relative layout). Output of the last is the layout contents (rows plus per-region
spans) consumed by `load_placement`/`file_assembly` (inferred from `into_parts` at `:65`; not read). Each step
also computes a `non_authoritative_*_compatibility_fingerprint` (FNV via `checked::stage_fingerprint`, e.g. `:303`).

**How it decides.**
- Section kind is an enum (`ElfDynamicRosterSectionKind = crate::ElfPlacedDynamicSectionKind`, `:2533`) and
  its numeric index is the enum discriminant (`index_for_kind`, `:2723`). Order is fixed by `CANONICAL_KINDS`
  (`:2459-2474`). Identity-keyed, not name-keyed.
- Section names are checked by byte offset into the name table: `require_name(... expected_name(kind))`
  (`:2785-2791`, `:2793-2810`) and a hard-coded offset list in `validate_complete_names` (`:1365-1380`).
  This is a source-spelling compare of fixed byte strings (`.interp`, `.dynstr`, ...).
- Memory region of a section is chosen from its `sh_flags` bits (`classify_region`, `:273-287`); alloc+write+exec
  and non-alloc write/exec are rejected (`:283`).
- File order is the fixed array `[0,1,...,9,11,10,12,13]` (`physical_section_order`, `:269-271`), so `.rela.dyn`
  (index 11) is packed before `.rela.plt` (10).
- Placement fixups are created for every non-null row: a virtual-address fixup only if `SHF_ALLOC` is set, plus
  a file-offset fixup (`:647-660`).
- Link/info numbers are verified against fixed literals (`:2940-2952`: e.g. `.dynsym` link 2 info 1,
  `.rela.plt` info 9).
- Each step's `validate_contents` re-derives the expected value independently and compares
  (e.g. `:206-267`, `:700-737`, `:1317-1362`, `:1934-1955`, `:2736-2779`).

**Cases by shape (fixed rosters).**
- Exactly 14 sections: `require(... == SECTION_COUNT ... "exact fourteen-row payload roster")` (`:155-158`),
  `:212-217`, `:1939-1942`, `:2748-2751`.
- Exactly 122-byte name table equal to a constant: `COMPLETE_SECTION_NAME_TABLE` (`:1170`), `UPSTREAM_NAME_SEED_SIZE`
  `:1167`, check `:1326-1341`. The `.shstrtab` payload is just the upstream seed copied (`:1285`).
- Exactly 25 fixups in one place (`PLACEMENT_FIXUP_COUNT = 13 + 12`, `:530-532`); error text says "twenty-three"
  (`:746`) and "eleven allocated rows" (`:754`) while the constants give 25 and 12 (the test at `:925` asserts 25).
  The message strings and the constants disagree; read only, not run.
- Fixed section indices hard-coded: `.dynamic` storage index 12 (`:1813`, `:2045`), plt 8, got.plt 9, rela.plt 10,
  rela.dyn 11 (`:1881-1905`); `.dynsym` link literal 3 (`:2876`).
- Fixed 64-byte null header (`:729`).

**Same job elsewhere.** The skeleton `plan_stage!`/`seal_stage!` is used 38 times across the six step modules
of `image-elf/src` (grep count over `P15`, files `dynamic_table.rs` 6, `file_assembly.rs` 6, `import_sections.rs` 6,
`load_placement.rs` 4, `procedure_linkage.rs` 6, `section_headers.rs` 10). The section roster and header bytes
are also written separately for shared components in `shared_component.rs:122-403` (see item 2), which builds
its own 13-section table without this module.
The `call_site`-like many-field `ElfNumericSectionDescriptor` is rebuilt four times in `derive_contents`
(`:2571-2652`) and a fifth time as expected value in `validate_row_against_owner` (`:2812-2925`).

**Unused or one-caller code.** `placement_fixup_count` (`:555`) and `dynamic_fixup_count` (`:1675`) are
`#[cfg(test)]`. `appended_descriptor_count` (`:1188`) returns the constant 1; grep
`appended_descriptor_count` over `source-rust` not run (not counted). Not otherwise checked; no "single/shared"
claims in this file.

**Stated limits.** None in prose. Test names claim "exact" counts (e.g. `:919`).

**Fail-open risks.** None seen in the production code. Checks use `require`/`?` and error out. One note,
"traced": `checked_align` treats alignment 0 as 1 (`:290`); the Null row is the only place alignment is 0
and it is handled separately (`:167-174`), so a non-null section with alignment 0 would be packed unaligned
rather than refused. Roster validation (`:2736-2779`) pins each row to the owner's descriptor, so a 0 alignment
would have to come from the owner descriptor.

---

## 2. `P15/image-elf/src/shared_component.rs` (729 lines)

**Purpose.** Emits an `ET_DYN` Linux ELF shared component (header comment `:1-6`) with exported functions in
`.dynsym`, a PLT-like import stub per referenced import, a GOT, a SysV hash, and version-needed records. It is a
separate, hand-laid writer, not built on the 22-step pipeline.

**Shape.**
- `ElfComponentExport {symbol, name, slot_commitment}` (`:670`), `ElfSharedComponentImage` (`:677`),
  `ElfSharedComponentEmissionError` (`:700`, returns the image and exports back).
- Public entry `emit_elf_shared_component(image, exports)` (`:712`), which calls private `contents::emit`
  (`:39`, about 410 lines). Helpers `align`, `append`, `set_section`, `put16/32/64`, `encode_header`,
  `encode_program`, `encode_section`, `emit_procedure` (`:546`), `validate_header` (`:592`).

**Inputs and outputs.** Reads a `FinalImage`, the export roster, `canonical_referenced_imports(image)` (`:95`,
from `crate::imports`), and `derive_shared_import_contents`/`build_sysv_hash` from `import_sections`.
Returns file bytes and the relocated text bytes (`:451`). It clones the image, appends one 16-byte import stub per
import to text, re-points relocations from the import symbol to that stub (`:278-308`), and calls
`image::apply_relocations` (`:309`).

**How it decides.**
- Target: `image.target == NativeTarget::linux_x64()` gives machine 62, `linux_arm64()` gives 183, otherwise
  error (`:43-51`). Identity of the target, not a name.
- Export acceptance (`:55-81`): name non-empty and without NUL; slot commitment not all zeros; no earlier export
  with the same name or commitment (quadratic scan, `:59-61`); symbol handle valid; symbol is `Function` in
  `Text` with non-zero size and inside the text bytes.
- Rejects any `Absolute64` relocation (`:82-91`) and a non-power-of-two BSS alignment (`:92`).
- Version records are emitted only if `tables.verneed` is non-empty (`:252`, comment `:250-251`). Relocation
  entries only if imports non-empty (`:243`).
- Final self-check `validate_header` (`:592-663`) re-reads the header and 5 program headers; then each export
  must lie inside the text load (`:443-450`).

**Cases by shape.** Fixed layout: 13 sections (`SECTION_COUNT = 13`, `:23`), 5 program headers
(`HEADER = 64 + 5*56`, `:22`), page size `0x1_0000` (`:21`), 16-byte import stubs (`:279`). The x86-64 stub is
`jmp [rip+disp32]` (`:564`); the AArch64 stub is adrp/ldr/br/nop (`:582`). Relocation type 7 (x86 JUMP_SLOT)
or 1026 (AArch64 JUMP_SLOT) (`:339-342`). All raw ELF constants are inline numbers (`0x6fff_fffe`, `0x6474_e551`).

**Same job elsewhere.** Writes ELF header/program/section headers by hand (`encode_header` `:500`,
`encode_program` `:516`, `encode_section` `:534`) while `static_executable.rs:13-99` and `crate::bytes::write_*`
write the same headers separately. `image-pe/src/shared_component.rs` and `image-macho/src/shared_component.rs`
do the same job per format (suspected; not read). `emit_procedure` creates an import stub; `procedure_linkage.rs`
in the same crate creates PLT stubs for executables (suspected; not read).

**Unused or one-caller code.** `emit_elf_shared_component`: 6 occurrences in 3 files (`image-elf/src/lib.rs` 3,
`shared_component.rs` 2, `image-emission/src/object.rs` 1), so one call from outside the crate (grep count over
`source-rust`). `ElfComponentExport` fields are `pub`.

**Stated limits.** `:88-90`: "component absolute pointers require loader-relative relocation custody", i.e.
absolute pointers in components are refused.

**Fail-open risks.** None seen. The `exported_symbols` bounds check (`:443-450`) runs after bytes are produced and
errors rather than accepting. "stated" comment at `:250-251` explains why version tables are skipped when empty.
One "open" note: `validate_header` checks `flags & 3 == 3` (W+X) only via a mask that reads flag bits 0-1
(`:646`); for ELF `p_flags` X=1, W=2, R=4, so `& 3 == 3` is W+X. Fine. The data load segment has flags 6 (RW) and
text has 5 (RX) (`:414-431`).

---

## 3. `P15/image-elf/src/static_executable.rs` (372 lines)

**Purpose.** Writes a static `ET_EXEC` ELF64 with a 64-byte header, two `PT_LOAD` headers, no section headers; it
refuses any image with a referenced dynamic import (`:1-5`).

**Shape.** Private modules `headers` (`:13`), `layout` (re-exports `image::align_to`, `:101`), `sections`
(`plan_elf_sections`, `:136`, `ElfSections` `:112`). Public `emit_elf_executable(image)` (`:176`); `hex_bytes`
(`:267`). Constants `EM_X86_64 = 62`, `EM_AARCH64 = 183` (`:173-174`).

**Inputs and outputs.** Reads `FinalImage`; computes text offset (aligned `0x1000`, `:134`, `:137`), data offset
(aligned to `ELF_MAX_PAGE_SIZE`, `:143`), BSS address. Calls `canonical_referenced_imports`, `elf_entry_address`,
`apply_relocations`, `place_executable_regions`, `place_data_regions` (`:191-225`). Returns `ExecutableImageOutput`
with bytes, file name `"omega-program"`, format string and region inventories (`:247-264`).

**How it decides.** Target must be `(Elf, Aarch64)` or `(Elf, X86_64)` else error (`:177-190`). If
`canonical_referenced_imports` returns any import, it returns an error whose text depends on the locator variant
(`:192-218`): `StringBackedBootstrap`, `Component`, or `Versioned`. The decision is keyed on the presence of a
referenced import, then on locator variant for the message only.

**Cases by shape.** Exactly two program headers (`PROGRAM_HEADER_COUNT`, constant, not read here), one text and one
data. Only the first import is reported (`imports.first()`, `:192`).

**Same job elsewhere.** ELF header writing is duplicated in `shared_component.rs:500-515` and in the 22-step
pipeline (`file_assembly.rs`, suspected; not read).

**Unused or one-caller code.** `emit_elf_executable` is the crate's entry (map says `lib.rs` re-exports it).
Not counted separately.

**Stated limits.** Error text at `:209`: "runnable dynamic ELF emission remains fail-closed before image mutation".
Error text at `:195`: "ELF loader binding is not implemented". These two messages describe the static path
as not supporting versioned/string-backed imports; dynamic ELF is handled by another entry
(`emit_elf_dynamic_executable`).

**Fail-open risks.** None seen. `expect("ELF text segment size overflow")` (`:61-62`, `:75`, `:77`, `:155`) panics
rather than errors on overflow (not a fail-open).

---

## 4. `P15/image-emission/src/callable_calls.rs` (48 lines)

**Purpose.** Two helpers that turn a native call owner into the callable-invocation site keyed in the prepared
invocation summary, and fetch that summary for an entry.

**Shape.** `prepare_with(calls, contracts, entry)` (`:5`): returns `&ReachableCallableInvocations` from
`contracts.execution().callable_invocations_from(entry)` or an error string. `call_site(owner)` (`:27`): maps
`target_operations::CallSiteOwner` to `terminal_semantics::callable_values::CallableInvocationSite`.

**How it decides.** Keyed on identity: `entry` machine id must have a function in `calls` (`:10`). The match on
`CallSiteOwner` (`:33-47`) maps `Operation`, `InvokeNoNormal`, `ExternalCompletion`, `CleanupAction` and rejects
`Installation` with "installed call cannot replace a callable source occurrence" (`:44-46`).

**Same job elsewhere.** `call_site` is duplicated almost line for line in stage 09:
`09_post-allocation-machine-to-framed-machine/src/stack_demand/physical.rs:209-231` (read), same match,
same error meaning (`&'static str` instead of `String`). Callers: `callable_calls::call_site` is used at
`function_fragments.rs:12340` (one call site, grep); `prepare_with` is used at `entries.rs:69` (one call site).

**Unused or one-caller code.** `prepare_with`: 1 call site (`entries.rs:69`). `call_site`: 1 call site
(`function_fragments.rs:12340`). (Grep `callable_calls::(prepare_with|call_site)` over `source-rust`.)

**Stated limits.** None.

**Fail-open risks.** None seen.

---

## 5. `P15/image-emission/src/companion_images.rs` (26 lines)

**Purpose.** Builds extra completed files that must ship beside an image. Today that is only the Windows
`omega_host.dll`.

**Shape.** `NativeCompanionImage {filename: &'static str, contents}` (`:3`), `emit(object)` (`:16`): if
`object.windows_internet` is `Some(code)`, it produces one companion named `"omega_host.dll"` using
`image_pe::emit_windows_internet_provider(code)` (`:18-24`).

**How it decides.** Keyed on the presence of `windows_internet` code on the object, not on the target. File name
is a string literal.

**Cases by shape.** One companion kind only.

**Same job elsewhere.** Called at `image.rs:1742` and `object.rs:272` to build and `image.rs:1771` to recompute and
compare (grep `companion_images`); consumed by `build/publication/publication.rs:120,226`.

**Unused or one-caller code.** `emit`: 3 call sites in `image-emission` (grep).

**Fail-open risks.** None seen. If `windows_internet` is `None` nothing is emitted, which is correct for a program
with no Windows import provider.

---

## 6. `P15/image-emission/src/entries.rs` (4550 lines)

**Purpose.** Binds execution entry points to sealed object bodies and verifies them. Header comment `:1-8` lists
`physical_entry`, `callable_entry_transport`, `hosted_entry_storage`, `hosted_receiver`, `hosted_unit_entry`,
`foreign_adapter`. The file also holds `private_entry_origin`. Nearly all of it is inline `pub(crate) mod` blocks.

**Shape (module map with lines).**
- `callable_entry_transport` (`:10-957`), inner `body_footprint` (`:11`): `collect_from` (`:63`) walks the
  entry and everything it calls and builds a CPU-state footprint (`NativeBodyCpuState`, `:32`). Public
  `observe_native_body_cpu_state` (`:779`), `observe_native_function_cpu_state` (`:789`),
  `observe_native_constant_operand` (`:801`); `bind_private_function_entry` (`:841`);
  `check_entry_contract` (`:862`); `native_entry_request` (`:913`); `validate_callable_transport` (`:822`);
  `validate_entry_transport` (`:941`).
- `foreign_adapter` (`:959-1104`): `NativeForeignAdapterImage` (`:967`), `emit_native_foreign_adapter` (`:1055`).
- `hosted_entry_storage` (declared `:1106`, file in item 7).
- `hosted_receiver` (`:1108-2052`): byte-by-byte validators for the hosted receiver entry: `aarch64::validate`
  (`:1114`, with a `Reader` over instruction words `:1240`), `windows_x86_64::validate` (`:1462`),
  `validate_x86_64` (`:1718`), plus `validate_binding` (`:1866`), `validate_entry_arguments` (`:1899`),
  `validate_image` (`:1920`).
- `hosted_unit_entry` (`:2054-3277`): `loader_stack::validate` (`:2083`), `owned_stack::validate` (`:2158`),
  `EntryShim` (`:2453`), `EntrySurface` (11 variants, `:2461`), `UnitEntryForm` and `unit_entry_form(target)`
  (`:2535`, `:2542`), `prepare` (`:2605`), `install_entry_shim` (`:2616`), `unique_region` (`:2645`),
  `validate_unit_shim` (`:2702`), and header checkers `pe_entry_points_to` (`:2737`), `elf_entry_points_to`
  (`:2824`), `main_points_to` (`:2891`).
- `physical_entry` (`:3279-4211`): `PhysicalEntryRequirements`, `PhysicalEntryBinding` (`:3726`),
  `PhysicalEntryPartitions` (`:3739`, `derive` `:3795`), `bind_encoded_physical_entry` (`:3829`),
  `validate_binding` (`:3867`), `prepare` (`:3928`), `validate_image` (`:3986`), `provision_stack` (`:4154`).
- `private_entry_origin` (`:4213-4550`): `EntryOrigins::new` (`:4243`), `verify` (`:4289`), `collect` (`:4338`).

**Inputs and outputs.** Inputs: an `ObjectArtifact` (this crate), the physical facts, call origins, frame
protocols, encoded rows, target profile, `image::EmittedImageOutput`. Outputs: mostly validators returning
`Result<(), Diagnostic>`; the producers are `prepare` (new object, text, relocations and an `EntryShim`),
`bind_*` (mutate or build bindings), `provision_stack` (clones the object and grows its BSS section).

**How it decides.**
- Target branch is by identity of `NativeTarget` (`==` against `macos_arm64()`, `linux_x64()`, ...) in
  `unit_entry_form` (`:2543-2602`), `validate_image` (`:1953-1962`, `:2008-2037`), `call_overhead` (`:2148-2156`).
  An unknown target returns `None`, which `validate_unit_shim` converts to the error "Unit entry has no
  mechanics row" (`:2723-2724`).
- Entry bytes are accepted only if they equal exact expected opcodes and addresses. AArch64: `Reader` compares
  each word to a literal (`:1148-1233`). x86-64 Linux: byte-offset compares (`:1729-1849`). Windows: sequence of
  `bytes`/`pointer` reads with a list of kernel32 imports `GetEnvironmentVariableA, CreateFileA, WriteFile,
  FlushFileBuffers, CloseHandle` (`:1480-1486`), looked up by symbol name (`region.symbol == name`, `:1498`).
  The Windows pipe name is a literal `OMEGA_PRIVATE_OBSERVATION_PIPE` (`:1533`).
- Image headers are matched by parsing the file: PE `.text` section name `b".text\0\0\0"` (`:2789`), Mach-O
  `__text` (`:2967`) and `LC_MAIN` command `0x8000_0028` (`:2935`), ELF `e_entry` plus an executable `PT_LOAD`
  containing it (`:2873-2880`).
- CPU footprint (`collect_from`) selects by `MachineAlternativeFamily` (`:269-350`): code/data segment reload,
  control register ops, MSR read/write, selector read; everything else falls to `_ => {}` (`:349`).
- Callable call targets are resolved by exact occurrence and instruction id, rejecting ambiguity (`:177-225`).
- `EntryOrigins::new` sorts entries by identity bytes (`:4255`) and merges duplicates; a conflict in
  callback entry, installed flag, plan, assumption or scope returns `PrivateEntryRecipeConflict` (`:4258-4269`).
- Physical entry: only target profile `UefiX64` is accepted (`:3478`, `:3841`); installed CPU requirement
  only for x86-64 (`:3318`).

**Cases by shape.**
- Hosted receiver: up to 8 receiver arguments on AArch64 (`if ordinal >= 8`, `:1176`); on Windows 5 (`_ => return
  Err`, `:1616`); on x86-64 Linux 6 (`argument_encodings` has 6 entries, `:1776-1783`).
- AArch64 receivers: only `macos_arm64` and `linux_arm64` (`:1124-1131`). Unit entries: five targets in
  `unit_entry_form` (`:2542-2602`). Windows receiver validator hard-codes five imports and a `bytes.len() - 13`
  failure tail (`:1512`).
- x86-64 hosted unit stack: Windows adds a 0x20 shadow space (`:2293-2295`); the stack tail on Linux x64 is
  `LINUX_X64_EXIT_ZERO`, other targets `[0x31, 0xc0, 0xc3]` (`:2304-2308`). `call_overhead` is 0 on AArch64,
  40 on Windows, 8 otherwise (`:2148-2156`).
- Physical entry: `X86_64Relative32` relocations only (`:3962`); `PhysicalEntryStackRequirement::CheckedPrivateImage`
  only (`:3499`); the stack switch is x86 (relocation kind is fixed).
- `collect_from` supports only the direct-call transports `X86Relative32FromNextInstructionToInternalMachineV1`
  on x86-64 and `Aarch64BranchLinkImmediate26FromInstructionToInternalMachineV1` on AArch64 (`:149-153`), else
  "entry internal call has another target transport".

**Same job elsewhere.**
- `elf_entry_points_to` exists twice with different signatures: `hosted_receiver::elf_entry_points_to`
  (`:2040`, checks only the `e_entry` value) and `hosted_unit_entry::elf_entry_points_to` (`:2824`, also checks
  machine, class, a `PT_LOAD` segment and the shim bytes).
- x86 RIP-relative reader: `rip_relative` (`:1707`), `Reader::pointer` in Windows validator (`:1425`) and
  `read_pointer` in `owned_stack` (`:2325`) do the same decode three times.
- Alignment arithmetic `(value + a - 1) & !(a - 1)` appears at `:1161-1164`, `:1584-1587`, `:1745-1748`,
  `:4126-4134`, and `checked_next_multiple_of` in `hosted_entry_storage.rs:35,49`.
- Receiver private-RAM layout loop is written separately for AArch64 (`:1153-1174`), Windows (`:1577-1597`),
  x86-64 Linux (`:1737-1768`) and again in `hosted_entry_storage::place` (item 7).
- `callable_calls::prepare_with` is the single use at `:69`.
- Physical-entry BSS growth (`provision_stack`, `:4154`) and hosted `provision` (`hosted_entry_storage.rs:91`)
  both clone the object, grow or insert one BSS section and add symbols.
- `original_bss` (`:4136`) duplicates the "at most one BSS section" check in `hosted_entry_storage::provision`
  (`:105-114`), and `owned_stack::validate` (`:2193-2206`), and `validate_image` (`:1967-1978`).

**Unused or one-caller code (grepped, `source-rust`, count mode).**
- `observe_native_body_cpu_state | observe_native_function_cpu_state | observe_native_constant_operand`:
  7 occurrences in 4 files (`entries.rs` 3, `image-emission/src/lib.rs` 2, `object_artifact.rs` 1,
  `psi-to-native/tests/physical_publication/mod.rs` 1), so they are called only from tests and from one
  non-test file outside the definitions (not traced further).
- `emit_native_foreign_adapter | bind_private_function_entry | bind_encoded_physical_entry`: 19 occurrences in
  10 files, including `driver/image/callback_thunks.rs`, `native-artifact/src/fragment_object_publication.rs`,
  `runtime_provisions.rs`, `native-sponsor/sponsor.rs`, `native-worker` tests.
- `unit_entry_form(`, `validate_unit_shim(`, `prepared_parts(`, `install_entry_shim(`: 15 occurrences in 3 files
  (`entries.rs` 6, `hosted_program.rs` 2, `image.rs` 7).
- `pe_entry_points_to` is used in `entries.rs` at `:1960`, `:2566`, `:4109` plus tests; `main_points_to` at
  `:1954`, `:2548`, `:2557`.
- `EntrySurface` variants `Physical`, `ProvisionedUnit`, `DarwinReceiver`, `LinuxReceiver`, `LinuxArm64Receiver`,
  `WindowsReceiver`: only `Physical` (`:3969`) and the unit variants are constructed in this file; the receiver
  variants are built elsewhere (not searched; "suspected" in `hosted_program.rs`).

**Stated limits (quoted, under 20 words).**
- `:143-145`: "masked interrupt handler can enable nested interrupts" (refused).
- `:242`: "interrupt provider callback closure has no retained state footprint" (callbacks refused).
- `:303`: "MSR architectural state requires x86-64".
- `:2055-2057`: comment says ordinary hosted imports "have no finite foreign-stack bound" so the loader's stack is kept.
- `:4377`..: none. `:3847`: "Only storage placement remains: stack size, effects, calls and frame geometry were fixed".

**Fail-open risks.**
- `collect_from` match `_ => {}` at `:349`: any `MachineAlternativeFamily` not listed adds no extra state to
  the footprint beyond the decoded effects (`footprint.decoded`, `:268`). A privileged instruction family not
  in the list whose effects the decoder does not report would be under-reported. Classification: "open" (no
  comment at the site); the decoded footprint at `:268` is the only evidence taken for such rows.
- `EntryOrigins::new` merge (`:4258-4281`): when both rows for one identity have `None` for plan/assumption/scope
  the comparisons via `matches!((prior.3, row.3), (Some(l), Some(r)) if l != r)` do not fire, and when one side is
  `None` the other is adopted (`:4270-4281`). So a recipe present in only one of two bodies is accepted and merged.
  "open" (no comment); the later `verify` (`:4289`) compares the merged record with the private function.
- `check_entry_contract`: `None if !needs_bridge && ... == effects => Ok(())` (`:899-906`) accepts a missing
  bridge only when the recomputed effects equal the stated ones (traced from the match, not run).
- `elf_entry_points_to` (hosted_receiver, `:2040-2051`) checks only `e_entry`. The caller `validate_image`
  separately checks region uniqueness and shim bytes via `output.final_text_bytes`, so it is not accepting wrong
  bytes (traced `:1944-1952`).
- `validate_x86_64` (`:1718`) is called for any non-Windows non-AArch64 target (`:2028-2037`), including macOS
  x86-64 if it reached here, using Linux syscall numbers (`0xb8, 1,...`, `0xe7`). Whether a macOS x64 receiver
  can reach `validate_image` was not traced ("open").
- Stack bound for `physical_entry::partitions`: the finite foreign-stack ceiling is required
  (`ok_or_else ... "requires an admitted finite foreign-stack bound"`, `:3427-3429`), so unknown demand is refused.

---

## 7. `P15/image-emission/src/entries/hosted_entry_storage.rs` (283 lines)

**Purpose.** Reserves space in the image's BSS for a hosted entry: a 16-byte saved-continuation cell, the fixed
hosted stack, and one private-RAM block (contents plus observation header) per receiver recipe, placed after any
BSS the program already has. It also checks the final placed image against that plan.

**Shape.** `HostedEntryPartitions {saved_continuation_offset, stack_offset, stack_byte_count, allocation_offset,
end_offset}` (`:7`), private `PrivateRamPlacement` (`:16`), `place(target, layout, existing_bss)` (`:28`),
`partitions` (`:76`, `pub(super)`), `provision(artifact, layout)` (`:91`, returns the grown `ObjectPlan`,
partitions and `HostedEntryStorage` symbol handles `:84`), `validate_partitions` (`:182`). One test (`:237`).

**Inputs and outputs.** Reads `HostedStackLayout` from `physical_recipes`, the artifact's object (clone), and
for validation `StackDemand` plus `image::EmittedImageOutput`. Produces symbols `omega_hosted_saved_continuation`,
`omega_hosted_stack_top`, `omega_hosted_private_entry_ram_{i}`, `omega_hosted_private_ram_observation_header_{i}`
(`:150-168`) in a BSS section.

**How it decides.** All arithmetic is checked (`checked_*` plus a single `invalid(target)` diagnostic).
`provision` refuses more than one BSS section (`:112-114`). `validate_partitions` requires: continuation not below
the original BSS, no overlap with stack, `stack_byte_count >= demand.ceiling_bytes()` (error if the demand is
unknown, `:200-205`), `stack_end == allocation_offset`, and `end_offset == output.bss_bytes` (`:198-209`);
then four 16-aligned address checks (`:213-227`).

**Cases by shape.** Alignment must be a power of two (`:46`). Continuation cell fixed at 16 bytes (`:38`, `:154`).
Alignment of the stack is fixed at 16 in the placement and validation, independent of target (`:213-218`).

**Same job elsewhere.** Placement arithmetic is repeated in the three receiver validators in `entries.rs`
(see item 6) and in `physical_entry::PhysicalEntryPartitions::derive` (`entries.rs:3795`).

**Unused or one-caller code (grepped).** `provision`: 1 call (`hosted_program.rs:49`); `partitions`: 2 calls
(`entries.rs:1990`, `:2218`); `validate_partitions`: 2 calls (`entries.rs:1991`, `:2230`).

**Stated limits.** None. Doc comment `:1`: "Place the hosted stack and private receivers at the final BSS origin."

**Fail-open risks.** None seen. Unknown stack demand is an error (`:200-205`).

---

## Coverage

Read in full (Read tool, every line):
- `P15/image-elf/src/section_headers.rs` lines 1-3183 (3183)
- `P15/image-elf/src/shared_component.rs` lines 1-729 (729)
- `P15/image-elf/src/static_executable.rs` lines 1-372 (372)
- `P15/image-emission/src/callable_calls.rs` lines 1-48 (48)
- `P15/image-emission/src/companion_images.rs` lines 1-26 (26)
- `P15/image-emission/src/entries.rs` lines 1-4550 (4550)
- `P15/image-emission/src/entries/hosted_entry_storage.rs` lines 1-283 (283)

Total read in full: 9191 lines. Skimmed: 0. Not read: none. Large test blocks (about 40 percent of
`section_headers.rs` and `entries.rs`) were read but are described only briefly.

## Five things a maintainer most needs to know about this unit

1. `section_headers.rs` is five sequential, each-validated planning steps of one fixed 14-section ELF layout; the
   section order, indices (8 to 13), link/info values and a 122-byte name table are all hard-coded
   (`section_headers.rs:269-271`, `:1170`, `:2940-2952`), and each step re-derives and compares its own output.
2. `shared_component.rs` is a separate hand-written ELF writer with its own 13-section layout and constants; it
   does not use the 22-step pipeline or the shared `bytes::write_*` helpers (`shared_component.rs:8-452`).
3. `entries.rs` validates entry shims by matching exact machine code for each hosted target (AArch64 words,
   x86-64 and Windows byte sequences); changing any generated entry byte requires changing the validator in
   the same file (`entries.rs:1148-1233`, `:1729-1849`, `:1524-1673`, `:2269-2314`).
4. The same small jobs appear several times in `entries.rs` and its neighbour: RIP-relative decoding (3 copies),
   power-of-two alignment (4+ copies), "at most one BSS section" (4 copies), two functions named
   `elf_entry_points_to`, and `call_site` is duplicated in stage 09 (`physical.rs:209`).
5. Size/count messages in `section_headers.rs` disagree with constants: the fixup-count error says "twenty-three"
   and "eleven allocated rows" (`:746`, `:754`) but the constants are 25 and 12 (`:530-532`), and the module
   docs for the 14-row roster say "thirteen-row roster" at `:706`.

## What you are unsure about

- Whether `validate_x86_64` in `hosted_receiver` can be reached for macOS x86-64 (`entries.rs:2028-2037`).
- Whether the `_ => {}` arm in `collect_from` (`entries.rs:349`) covers any family that actually carries
  architectural state not already in the decoded footprint; I did not read the decoder or the family list.
- The other-format shared component writers (`image-pe`, `image-macho`) and `procedure_linkage.rs` were not
  read, so duplication claims against them are suspected.
- The number of non-test callers of `observe_native_*` was not traced beyond the grep count.
- `appended_descriptor_count` (`section_headers.rs:1188`) caller count not grepped.
