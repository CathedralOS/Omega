# Unit 4 notes

Files covered (paths under `omega-rust/omega/compilation/backend/images/`):
1. `image-elf/src/shared_component.rs` lines 1-727
2. `image-elf/src/static_executable.rs` lines 1-320
3. `image-emission/src/entries.rs` lines 1-5868
4. `image-emission/src/function_fragments.rs` lines 1-12462 (the file is 24,923 lines per `wc -l`; this unit is the first half; the last range read ends inside `placed_reference::precedes_blocks` at :12462, so the enclosing `mod source`, `mod placed_reference`, and the items it refers to but does not define here (`host`, `Error`, `refuse_symbol_collision`, `structural`, `structural_homes`, `spill_values`, `validate_function_fragment_object_artifact`, `call_target_matches`) are in the next unit.)

All "read" below means I read the lines with the Read tool. Greps were run with `grep -rnw --include=*.rs` from `omega-rust/` (every `.rs` file under it).

---------------------------------------------------------------------
## 1. image-elf/src/shared_component.rs (727 lines)

**Purpose.** Builds a separately loadable ET_DYN ELF64 container for Linux x86-64 or AArch64 from a `FinalImage` plus a roster of exported functions. It lays out dynstr/dynsym/SysV hash/versym/verneed/rela.plt/text/data/bss/GOT/dynamic and 5 program headers by hand. The original image and export roster are handed back on both success and failure (`rejection_carrier!`, :704).

**Shape.** `mod contents` :8-662 with `emit` :39-450 (one 410-line function), helpers `invalid` :452, `align` :455, `append` :464, `set_section` :473, `put16/32/64` :489-497, `encode_header` :498, `encode_program` :514, `encode_section` :532, `emit_procedure` :544, `validate_header` :590. Public: `ElfComponentExport {symbol, name: Vec<u8>, slot_commitment: [u8;32]}` :668, `ElfSharedComponentImage` :675 (image, exports, contents, final_text_bytes), `ElfSharedComponentEmissionError` :698, `emit_elf_shared_component` :710. Constants `PAGE=0x1_0000`, `HEADER=64+5*56`, `SECTION_COUNT=13` :21-23.

**Inputs/outputs.** Reads `FinalImage` (`target`, `symbol_table`, `relocation_table`, `memory.{text,data,bss_size,bss_alignment}`), `canonical_referenced_imports(image)` :95 and `derive_shared_import_contents` / `build_sysv_hash` from `crate::dynamic_executable::import_sections::dynamic_sections` :16-18. Clones the image (:274), appends one 16-byte stub per import to text, rebinds every relocation that pointed at the import symbol to the stub symbol (:292-305), calls `image::apply_relocations` (:307), returns `(file bytes, relocated text)`.

**How it decides.**
- Target: equality `image.target == NativeTarget::linux_x64()` / `linux_arm64()` :43-47 (else Err).
- Export acceptable iff non-empty name, no NUL, nonzero `slot_commitment`, no earlier export with same name or same commitment (:56-61), symbol handle valid (:62), symbol kind Function in Text with size>0 and end <= text length (:69-76).
- Any `RelocationKind::Absolute64` anywhere in the image => Err (:82-91).
- bss alignment must be a power of two (:92).
- Import->stub rebind is keyed on `row.symbol_handle == import.symbol_handle` (:296), i.e. symbol identity.

**Cases by shape.** Fixed 13 sections, fixed 5 program headers, one PAGE size, only Linux x86-64 (reloc kind 7) and AArch64 (1026) (:337-340); stub encodings exist only for those two (`emit_procedure` :550-587). Final export check requires each export symbol to lie inside the text load (:441-448).

**Same job elsewhere.** `align` here :455 is a copy of the helpers at `entries.rs:1296` and `:5440` and of `image::align_to` (`image/src/lib.rs:301`, used by `static_executable.rs:102`). grep `fn align(`: 9 definitions in `omega-rust` (3 in this unit). `rejection_carrier!` is used by 25 sites in image-elf/macho/pe (grep), so the carrier shape is shared.

**Unused / one-caller.** `emit_elf_shared_component`: one caller outside re-export, `image-emission/src/object.rs:293` (grep). `canonical_referenced_imports` has users in static_executable.rs:184, shared_component.rs:95, import_sections.rs:88 plus tests in imports.rs.

**Stated limits.** Header comment :1-6 only. `invalid("component absolute pointers require loader-relative relocation custody")` :88-90 (Absolute64 refused).

**Fail-open.** None seen. `validate_header` :590 is run on the output it just built (self-check).

---------------------------------------------------------------------
## 2. image-elf/src/static_executable.rs (320 lines)

**Purpose.** `emit_elf_executable` :169 emits a static ET_EXEC ELF64 with 2 PT_LOAD and no section headers, and refuses any image that still references a dynamic import.

**Shape.** `mod headers` :13-99 (`write_elf_header` :19, `write_text_program_header` :51, `write_data_program_header` :66), `mod layout` :101 (re-export of `image::align_to`, `align_to_u64`), `mod sections` :105-159 (`ElfSections`, `plan_elf_sections` :131; `mod tests {}` empty :157-158), `emit_elf_executable` :169-258, `hex_bytes` :260, one test :270-320. Constants `EM_X86_64=62`, `EM_AARCH64=183` :166-167.

**Inputs/outputs.** Reads `FinalImage`; calls `canonical_referenced_imports`, `elf_entry_address(&image, text_address)` (crate::entry_symbol), `apply_relocations`, `place_executable_regions`, `place_data_regions`. Returns `ExecutableImageOutput` with file name `"omega-program"` and format string `"elf64-aarch64-executable"` / `"elf64-x86-64-executable"` (:170-183, :246).

**How it decides.** Machine by `match (object_format, architecture)` :170-183 (Elf+Aarch64, Elf+X86_64 only). If `imports.first()` exists => Err whose text depends on `ElfImportLocator` variant (StringBackedBootstrap / Component / Versioned) :185-211.

**Cases by shape.** Exactly two PT_LOAD (text flags 5 at :58, data flags 6 at :74); text at first page after headers (`plan_elf_sections` :132-136).

**Stated limits (quoted).** :188 "ELF loader binding is not implemented"; :202 "runnable dynamic ELF emission remains fail-closed before image mutation because no target-owned ELF loader plan carries the exact PT_INTERP bytes".

**Fail-open.** None seen. Overflow uses `.expect` (:61-62, :75, :145) i.e. panics rather than errors (not an accept path).

**Unused/one-caller.** `emit_elf_executable`: callers `image-emission/src/image.rs:1843` and `image-elf/src/imports.rs:336` (a test helper) (grep, 11 hits in 4 files incl. docs/tests).

---------------------------------------------------------------------
## 3. image-emission/src/entries.rs (5868 lines)

Seven top-level modules, all `pub(crate) mod`.

### 3a. `callable_entry_transport` :10-1145
**Purpose.** Decide what CPU state a native body touches (`body_footprint`) and build/verify the entry bridge that adapts an inbound calling convention to the body's internal one.
**Shape.** `body_footprint::collect/collect_from` :82-338 (worklist over callees through `internal_machine_fixup`, collects register/state footprint from `ObjectArtifact.fragment_replay`), `NativeBodyCpuState` :34 (effects, generated_effects, floating_control, opaque_control; `require_without_floating_state` :54), `Footprint` :426, `register_for_unit` :579, `index_encoded_rows` :340, `check_mask_operation` :370, `is_call` :389, `direct_call_effects` :402. Outer: `observe_native_body_cpu_state` :850, `observe_native_function_cpu_state` :860, `observe_native_constant_operand` :872, `CallableEntryTransport` :890, `validate_callable_transport` :900, `create_private_function_object` :914, `create_installed_private_function_object` :931, `create_callable_entry_transport` :962, `native_entry_request` :993, `identity_transport` :1092, `validate_entry_transport` :1114. Tests :639-833.
**Decides keyed on.** Register identity via *view-name strings*: `model.view_named("x0")`, `format!("x{index}")`, `format!("q{index}")/d{index}`, `("rax", X86Rax)...`, `format!("xmm{index}")` (:584-636). Architecture-by-name: `model.view_named("x0").is_some()` selects the AArch64 branch (:584). Mask rule: Masked preemption + `InterruptControl::Enable` or `SystemWrite(Rflags)` => Unsupported (:375-384). Call-ness by `MachineEncodedControlEffect` list (:389-400). Identity transport iff policy/entry_control/result/callbacks/stack alignment/shadow/clobbers equal and each parameter `has_same_parameter_transport` (:1092-1111).
**Cases by shape.** MSR families require x86-64 (:286-289); Direct-call transport must be x86 rel32 or AArch64 BL imm26 (:184-188, "entry internal call has another target transport"). `scalar_codec` only for `BorrowedScalarReference` parameters whose referent is `PrimitiveScalar` (:1046-1075). Callback-bearing indirect calls rejected "interrupt provider callback closure has no retained state footprint" (:221-223).
**Fail-open.** (i) `collect_from` match on `row.alternative.family` ends `_ => {}` (:328): an instruction whose family is not one of the listed privileged families contributes only its decoded encoded footprint (:248). Status: open (not traced to what families exist). (ii) `create_callable_entry_transport` rejects installed bodies (:967-971). None else seen.
**Unused/one-caller.** `create_private_function_object` external caller `native-compiler/src/realization/callback_thunks.rs:382` (+ internal :1129); `create_installed_private_function_object` external `callback_thunks.rs:374` (+ internal :1122); `create_callable_entry_transport` external `component_exports.rs:256` (+ :905, :920 internal); `observe_native_body_cpu_state` callers `image-emission/src/object_artifact.rs:1260` and test support `compiler/tests/support/native_entry_controls.rs:191`; `observe_native_function_cpu_state` only `native_entry_controls.rs:260,275` (test support); `observe_native_constant_operand` only `native_entry_controls.rs:215,299` (test support); `validate_callable_transport` one caller `object.rs:51`; `validate_entry_transport` callers `object.rs:910`, `object_artifact.rs:462`. All by grep -w.

### 3b. `foreign_adapter` :1147-1285
**Purpose.** Wrap a `NativeForeignBridge` in a one-symbol `FinalImage` and emit a shared function container (`emit_shared_function_container`).
**Shape.** `NativeForeignAdapterImage` :1158, `NativeForeignAdapterEmissionError` :1185, `emit_native_foreign_adapter` :1192. Export name `__omega_foreign_frame_<hex of digest of application, locator identity, bridge bytes>` (:1204-1215); Mach-O gets a `_` prefix (:1216-1220). Relocation kind by architecture (:1260-1263).
**Decides.** Rejects `application == [0;32]` or `locator.target().native_target() != bridge.request().target` (:1198-1203).
**Callers.** `native-artifact/src/runtime_provisions.rs:91` plus 3 test call sites (grep).

### 3c. `hosted_entry_storage` :1287-1510
**Purpose.** Compute and provision the BSS partitions for a hosted entry: saved continuation (16 bytes), private stack, then private-RAM recipes with observation headers.
**Shape.** `HostedEntryPartitions` :1311, `partitions` :1319, `HostedEntryStorage` :1357, `provision` :1364 (rejects >1 existing BSS section :1387-1389; symbols `omega_hosted_saved_continuation`, `omega_hosted_stack_top`, `omega_hosted_private_entry_ram_{i}`, `omega_hosted_private_ram_observation_header_{i}`), `validate_partitions` :1466.
**Decides.** Stack bytes = align16(max(ceiling,16)+call_overhead) (:1329-1337). Validation checks offsets and 16-byte alignment of `bss+offset` (:1482-1506).

### 3d. `hosted_receiver` :1512-3320
**Purpose.** Hand-assembled machine-code "receiver" shims that switch to a private stack, set up private RAM, call the semantic entry with pointer arguments, then write observation bytes to a pipe/FD 3 and exit; plus byte-exact replay validators.
**Shape.** `aarch64::prepare` :1558 (words, with `ReceiverForm` for macOS and Linux :1520-1556); `instructions::aarch64::validate` :1706 with `Reader` :1832; `instructions::windows_x86_64::validate` :2054 with `Reader` :1996; `instructions::validate_x86_64` :2310; `windows_x86_64::prepare` :2467; `prepare_linux_x86_64` :2814; `finish_receiver_shim` :2779; `bind_hosted_receiver` :2903; `validate_binding` :2927; `physical_contract_row` :2960; `physical_contract_matches` :3013; `validate_entry_arguments` :3048; `receiver_pointer_matches` :3180; `validate_image` :3200; `elf_entry_points_to` :3308; `HostedReceiverBinding` :2660.
**Decides (keyed on).** Target equality chains (`target == NativeTarget::macos_arm64()` ... :1530-1538, :2968-3008, :3056-3062, :3094-3100). Windows import resolution by normalised locator `PeByName kernel32.dll` and `region.origin == ImportThunk && region.symbol == name` — string compare of symbol name (:2079-2091). Windows validator offsets are tied to literal bytes, e.g. `failed = bytes.len()-13` (:2104).
**Cases by shape (quoted).** Windows argument registers only ordinals 0-4: `0 => rcx, 1 => rdx, 2 => r8, 3 => r9, 4 => stack 32`, `_ => return Err` (:3148-3158, :2549-2556, :2202-2209); AArch64 `if ordinal >= 8 { return Err }` (:1578); Linux x86 `[rdi,rsi,rdx,rcx,r8,r9]` (:2896-2901, :2837-2847); `validate_entry_arguments` requires one of four hosted targets and `private_entry_ram` non-empty (:3056-3065), no visible params, Unit result, no entry claims (:3085-3092), `function.structural_parameters.len() == private_entry_ram.len()` (:3101). Windows validator requires `stack_byte_count >= 80` (:2068). `physical_contract_row` has rows for macOS arm64/x64, Linux x64/arm64, Windows x64 only (:2960-3010); macOS x64 has a contract row but `validate_entry_arguments`/`prepare` do not list it (:3056-3062, :2768-2776).
**Same job elsewhere.** Every platform has an emitter and a separate byte-replay validator that re-derives the expected bytes: AArch64 `prepare` :1558 vs `validate` :1706; Windows `prepare` :2467 vs `validate` :2054; Linux x86 `prepare_linux_x86_64` :2814 vs `validate_x86_64` :2310; free-Unit `owned_stack::encode_x86_64` :3422 vs `validate_x86_64` :3576; physical `stack_switch::emit` :4730 vs `validate_image` :5300. The AArch64 emitter is also reused by the Unit entry (`prepare_aarch64_entry(artifact,&[],...)` :3371). Two different functions named `elf_entry_points_to`: `hosted_receiver` :3308 (checks only e_entry == text_address+offset, 8 bytes at 24..32) and `hosted_unit_entry` :4095 (checks header, machine, e_entry, PT_LOAD flags and the shim bytes). grep `fn elf_entry_points_to|pe_entry_points_to|main_points_to`: 4 definitions, all in entries.rs.
**Unused.** `bind_hosted_receiver`: one caller `native-artifact/src/fragment_object_publication.rs:145` (grep). Empty `#[cfg(any(test, feature="test-support"))] impl crate::ObjectArtifact {}` at :2668-2669 (and function_fragments.rs:3953-3954).
**Fail-open.** (i) AArch64 validator: `Reader` demands exact words; fail-closed. (ii) `validate_image` for ELF uses the weak `elf_entry_points_to` :3241 but then also replays shim bytes (:3297-3304); traced: the shim bytes are matched by `validate_x86_64`/`validate_aarch64`, so a wrong entry address is caught by the comparison `output.bytes[24..32]`. None seen beyond that.

### 3e. `hosted_unit_entry` :3322-4404
**Purpose.** Entry shim for a "free Unit" entry (no params, Unit result): saves loader stack, switches to private stack, calls entry, restores, exits; per-target forms; plus loaders' entry-point verifiers.
**Shape.** `owned_stack::{prepare :3354, encode_x86_64 :3422, validate :3478, validate_x86_64 :3576}`, `EntryShim` :3737, `EntrySurface` :3745 (11 variants), `PreparedEntry` :3776, `unit_entry_form` :3817 (macOS arm64/x64, Windows, Linux x64, Linux arm64), `prepare` :3880, `install_entry_shim` :3900, `unique_region` :3929 (matches by `region.symbol == name` string, origin CompilerFunction, section_offset, byte_count, exactly 1 :3936-3948), `validate_unit_shim` :3973, `pe_entry_points_to` :4008, `elf_entry_points_to` :4095, `main_points_to` :4162. Tests :4253-4403.
**Decides.** `has_free_unit_entry` (replay.rs, function_fragments.rs:3972) gate; `call_overhead` 0/40/8 by target (:3344-3352). Windows adds `sub rsp,0x20` (:3442-3444); Linux x64 tail `encode_hosted_exit_process_i32(0)` else `xor eax,eax; ret` (:3459-3463).
**Fail-open.** `prepare` returns `Ok(None)` if the artifact is not a free-Unit entry or the target has no `unit_entry_form` (:3891-3896): such an artifact gets no shim (open; not traced to caller behaviour).

### 3f. `physical_entry` :4406-5525 (UEFI x64 only)
**Purpose.** Entry for an explicit physical arrival (UEFI PE x86-64): a prefix that saves RSP/RAX/R10 to BSS, switches to a private stack, copies stack-passed arguments, calls the bridge, restores, returns.
**Shape.** `requirements::{partitions :4558, derive :4583, cpu::derive :4440}`, `PhysicalEntryRequirements` :4516, `stack_switch::emit` :4730, `PhysicalEntryBinding` :5027, `PhysicalEntryPartitions` :5040 / `derive` :5096, `bind_physical_entry` :5123, `validate_binding` :5187, `prepare` :5242, `generated_state_effects` :5287, `validate_image` :5300, `provision_stack` :5468.
**Decides.** Hard gates: `contract.target_slot().owner != TargetProfile::UefiX64` (:4592, :5143) and `object_format == Coff && architecture == X86_64` ("explicit physical arrival currently requires x86-64 PE" :5133-5139). `stack_switch::emit` requires x64 `CallReturn` entry control and alignment >=16 power of two (:4733-4739). `cpu::derive` yields the single requirement `X86LongModeCpl0WithoutCet` whenever installed scopes exist, failing unless each scope has a matching installed-interrupt bridge (:4486-4490).
**Same job elsewhere.** Partition model (`PhysicalEntryPartitions::derive` :5096, saved state 24 bytes) duplicates `hosted_entry_storage::partitions` :1319 (saved 16 bytes) and `provision_stack` :5468 duplicates `hosted_entry_storage::provision` :1364; `align` here :5440 duplicates :1296.
**Unused.** `bind_physical_entry`: one caller `fragment_object_publication.rs:123`.
**Fail-open.** `validate_binding` returns `Ok(())` when `artifact.physical_entry` is None (:5188-5190). Traced by structure: callers dispatch on `physical_entry_binding().is_some()` (`hosted_unit_entry::prepare` :3885).

### 3g. `private_entry_origin` :5527-5868
**Purpose.** Collect and cross-check the admitted callback recipes (ordinary and installed-interrupt) across a root artifact and its private function bodies, merge by function identity, and verify each private entry's metadata against it.
**Shape.** `EntryOrigins::new` :5557, `verify` :5603, `collect` :5652, `conflicting_recipes` :5811. Merge keyed on `row.0.to_bytes()` (function identity) after `sort_unstable_by_key` (:5569-5599). For installed entries: plan is `InstalledInterruptEntryPlan::new(NoErrorCode)` if the entry's application equals the anchor's, else `fatal_context(...)` from the single matching fatal entry (:5758-5787).
**Cases by shape.** Exactly two kinds: timer/acknowledgement anchor entry and entries listed in the scope's fatal roster; anything else => "private entry is absent from the admitted fatal roster" (:5772-5776).
**Callers.** `EntryOrigins` has 4 line hits in 3 files (grep).
**Fail-open.** Merge: when two rows share an identity, `prior.3/4/5` are filled from the later row only when the earlier is None, and conflicts are checked only between two `Some` values (:5575-5594): a row with `None` never conflicts with a `Some`. Status: open (not traced).

---------------------------------------------------------------------
## 4. image-emission/src/function_fragments.rs lines 1-12462

**Purpose.** Project an admitted fragment container into an `ObjectArtifact` (functions, symbols, relocations, byte-literal pool, implementation tables, native layout descriptors, private function symbols, port effects, boundary settlements) and replay every claim against the retained abstract/target/selected plans. The module comment (:1-6) says "verifying that every abstract and target operation is accounted for in the final native instructions".

### 4.1 `attribution` :8-1470
Builds/validates `SemanticCodeAttribution {site, operation_ordinal, code_offset, byte_count}` rows. Key: `SemanticCodeSite::{Operation(OperationId), Edge(EdgeId)}` (identity). `produce` :148, `validate` :294, `ordinal` :103 (linear scan) and `build_ordinal_index`+`lookup_ordinal` :45-101 (index) are two implementations of "site -> unique ordinal in AbstractFunction" (`ordinal` is used by `port_effects` :3325). `supports` :623 decides which instruction may own which site. `PhysicalSpans` :484 is a binary-search fast path; comment :489-490: "Canonical physical spans let interval checks visit only overlapping bytes. Other input shapes retain the original full scan and its error order." with the free `interval_is_exact` :675 as the fallback (`PhysicalSpans::interval_is_exact` :564 vs :675 are the same check twice). Fixed cases: successor roles `Semantic`, `CaseDispatchContinuation`, `OperationContinuation` (:26-43); `Control` variants enumerated :169-262 and :350-415. Zero-byte rows (conditional fallthrough edges) accepted when some instruction `supports` them (:679-685). Tests :720-1469.

### 4.2 `byte_literals` :1472-1723
Packs `EstablishByteSequenceLiteral` octets into a pool appended to **.text** (symbol `omega_byte_literal_pool`, 16-byte aligned, :1483-1484, :1503-1511), deduplicating identical octets with `BTreeMap<&[u8],usize>` (:1646; keyed on byte value). `pool_contents` :1609, `validate` :1545, `records` :1534, `literal_octets` :1664 (requires exactly one matching AbstractOperation by `(psi_operation, place.id)`).

### 4.3 `image_addresses` :1725-1861
`records` :1733 turns `ImageAddressFields` into relocation records: only `X86_64Relative32` (1 record) and `Aarch64PageAddress` (Page21 + PageOffset12) (:1750-1765). `rows_for_symbols` :1768 selects relocation rows whose `symbol_handle == symbol` **or** whose origin is `Materialization{object_symbol_handle: symbol}`.

### 4.4 `image_residence` :1863-1994
Only `SelectedImageAddressReference::RetainedEntryStorageStart` observes residence (:1874-1876); requires `object.target == uefi_x64()` else Unsupported (:1883-1887) and x86 rel32 fields (:1935-1939). Symbol `omega_retained_entry_storage` (SymbolSection::None until physical placement; `physical_entry::provision_stack` fills it, entries.rs:5497-5505).

### 4.5 `implementation_tables` :1996-2479
8-byte-slot tables in .data per `TerminalDynamicDispatchCatalog` table (`omega_implementation_table_{id}` :2061); empty tables get one 8-byte slot (`rows.len().max(1)*8` :2050-2055); `None` rows are skipped when relocating (`let Some(machine) = machine else continue` :2128-2130) leaving zero. Absolute64 relocations (:2145). `validate_image_data_relocations` :2272 walks `private_object_composition::namespaces`. Note shared_component.rs:82-91 refuses Absolute64 in components, so a component containing an implementation table is refused there (inferred, not traced).

### 4.6 `imports` :2481-2847
`publish` :2495 / `validate` :2606 for normalised foreign call fields and callback address relocations. `call_for_import_field` :2815 matches by `(caller machine, owner, text_offset)`; `relocation_kind` :2841 has two kinds. `validate` computes the expected symbol position by arithmetic over pool counts (`functions.len() + function_symbols.len() + usize::from(pool.is_some()) + ...` :2659-2672), i.e. symbol order is an implicit contract.

### 4.7 `mixed_scalar_abi::admit` :2849-2908
One long conjunction comparing `MixedStructuralScalarFunctionAbi` to the abstract and target function and the selected structural contract (:2865-2902). Rejects when `structural_parameters.is_empty()`.

### 4.8 `native_code_layouts` :2910-3271
Per machine, a 24-byte header + 16 bytes per region in .data (`omega_native_code_layout_{machine}` :2992), with Absolute64 relocations to the function symbol (:3043-3071). Its `records` :3031 re-implements the x86/Aarch64 relocation split inline (:3102-3116) instead of calling `image_addresses::records` (the other pools do). `validate` rebuilds via `place` and compares (:3186-3221). `image_data_records` :3227.

### 4.9 `port_effects` :3273-3337
Only `AbstractOperation::PortWrite`; finds exactly one instruction with alternative `MachineAlternativeKey{family: PortWrite, variant: 0}` and `provenance.operations == [psi_operation]` (:3302-3315).

### 4.10 `private_addresses` :3339-3570
Declares function symbols for private functions referenced by callbacks or `SelectedImageAddressReference::PrivateFunction` (`unique_identities` :3373: caller must equal `psi`; dedupe on `identity.to_bytes()`), then emits address relocations. Name from `identity.private_symbol()`; collision by name compare against a sorted list (:3403-3416). `records` returns empty if no private addresses (:3436-3438).

### 4.11 `production` :3572-3849
`build_function_fragment_object_artifact` :3592: one pass per placed function building `ObjectFunction` with fixed-empty vectors for many fields (:3686-3733), stack numbers (`linkage` 8 on x86-64, 0 on AArch64 :3641-3644; `unit` if result is Unit). Symbol names: entry via `entry_symbol_name(target)`, others `omega_terminal_machine_{id}` (:3623-3627). Order of pools placed: private addresses -> byte literals -> implementation tables -> native layouts -> image residence (:3763-3784). Callers: 4 hits in 3 files (grep): used by `native-artifact`.

### 4.12 `replay` :3851-4113 and `reporting` :4115-4175
`FragmentReplay(Arc<StagedOptimizedRelocationFreeObjectContainer>)` :3857; `PartialEq` compares object, container, manifest record (:3943-3949). `verify_retained_target_operations` :3859 (recursive over private functions; callers: corpus_runner.rs:1060,1070 (tests), lib.rs re-export). `validate` :3988-4112: a set of rules that fire only when `fragment_replay.is_none()` (:3991-4106: graph storage, structural params without homes, borrowed pointers, selected Unit call, selected boundary output, hosted byte input) and, when replay exists, delegates to `validate_function_fragment_object_artifact` (:4107-4109; defined after this unit). `reporting::summarize` returns `Ok(None)` without replay (:4128-4130).
**Fail-open.** `replay::validate` ends `Ok(())` :4111 when `fragment_replay.is_none()` and none of the listed shapes is present: such an artifact is accepted with no replay of abstract/target/selected evidence. Status: open (would need to trace what creates artifacts without replay; the comment strings say the listed shapes "require common-pipeline replay evidence").

### 4.13 `source` :4177-(continues past 12462)
**`descriptor_tables`** :4178-4784 (segment read/reload, task register, GDT ops). `DescriptorReplay::new` :4409, `validate` :4509, operand positions hard-coded per `RegisterOperation` (:4549-4605), table extent `entries*8` / TSS 104 / GDT 10 bytes (:4625-4634). `dedicated_descriptor_owner` :4717: a hardware descriptor instruction without a descriptor target row is skipped (`Ok(false)`) iff its operation id is an `InstallStaticInterrupt` (:4456-4458), else Err.
**`installed_calls`** :4786-5027: each `InstallStaticInterrupt` must have exactly a Validate call (position 0, scalar Boolean result) and a Work call (`argument_position` 5, no result) (:4936-4947), compared field by field with the selected `CallScalar` / `InvokeNoNormal` instructions.
**`installed_interrupts`** :5029-9627 (the largest block, ~4600 lines). A symbolic verifier for the code that installs an IDT/TSS/stack set. `control::Graph` :5048 (CFG, iterative dominators :5115-5143; `precedes` :5185, `bounded_region` :5245, `protected_interval` :5293, `require_guard` :5448, `counted_loop` :5609, `selected_iteration` :5710, `only_success_path` :5576). `expressions` :6171 (`Expr(Arc<Word>)` symbolic words: Constant/Parameter/Value/Frame/Image/RootBoundary/Load/Binary/Cpu/Call :6230-6253; `Expr::eq` :6268 ignores the `instruction` field of `Load` and `RootBoundary`, comparing by address/bytes/kind only; `definition` :6638 supports only a fixed list of selected kinds, else Err "unsupported selected definition" :6740-6744). `guards::Contract::derive` :6853 (hard-coded control-block offsets: 48 scratch, 64 table index, 72 arrival index, 80 stack index, 88 count, 96 root, 104 top, 112/120 pair captures; 128-byte allocation :7856; gate type byte 0x8e :7004; descriptor masks 0xfc/0x98, 0x60/0x20 :7015-7016; external vectors 32..=255 :7023). `inputs::Inputs::derive` :7283 takes exactly `[StructuralArgument; 6]` (:7286) and special-cases ordinals 3 and 4 as views (:7293). `instructions::validate` :7340 (closed list of allowed selected kinds :7415-7451; counts must be exactly 1 each :7388-7393). `pairs` :7456-7821 (simultaneous dedicated entries: reciprocal placement lists, distinct IST, disjoint captured ranges). `publication` :7823-8346 (root init order, activation before task commit, commit before table publication before work; `reads` :8055 accepts reads inside input loads, control offsets 64-96 (112/120 if pairs) or the table; all others must lie in an authenticated interval and be guarded). `refusal_result::validate` :8358 (refusal Boolean carriers must be constant false and defined before Validate). `table::gate` :8416 (decodes bit placements of the entry address into the IDT gate; requires all 64 pointer bits covered), `table::materialization` :8489. `writes::Writes` :8643 (closed roster of stores, `complete()` :8737 rejects extra writes; `bits_equal` :8831 compares expressions bitwise up to 8 bytes). `validate` :8898 and `loops` :9314 tie it together.
**`privileged_termination::validate`** :9629: crash instruction kind must be `PrivilegedCrash` iff execution is an installed domain, else `Crash` (:9668-9679).
**`static_interrupts`** :9686-9966: `retained` :9791 and `root_accesses` :9852 verify context/BeginCompletion/FinishCompletion helper accesses on the root words (`expected` offsets per operation :9898-9916); `Operation::Install => return false` :9915.
**`aggregate_results`** :10015-10307: `header` :10022 and `operation` :10061 compare ~20 `AbstractOperation` variants (activation claims, structural copies/moves/stores, records/cases/arrays, references, `CallStructural`, `ReturnStructural`) with exactly one matching `TargetUnitOperation` (`.count() == 1`); `_ => false` :10304.
**`control_flow`** :10309-10977: `retained` :10314 matches `InvokeNoNormal/ExternalCompletion/Crash/Return*/Jump/Conditional` against `TargetControlTerminator` by `psi_edge`; `cleanup_rosters_match` :10562 and `edge_cleanup_calls_match` :10592 (nominal cleanup calls compared to the target `CleanupCall`s by `CallSiteOwner::CleanupAction{edge, action_ordinal}`; `DiscardRoot/RetireHome/DiscardResidual` actions skipped :10609-10611). Tests :10680-10976.
**`placed_reference`** :10979-... : `backing_geometry::activation_residence` :10991 (activation backing must be a direct inline container with a non-indirect allocation home :10243-10269 [see 11243-11270]), `incoming_calls` :11362 (rows for Call/CleanupCall/InstallStaticInterrupt/InvokeNoNormal/ExternalCompletion; cleanup and installation-helper arrivals are given `inputs: Err("... has no reconstructed backing observation")` :11540, :11552), `BackingGeometry::proves` :12010 (Resident backing checks carrier/schema/commitment :12024-12043; `BorrowedExtent` => Err "requires a successful View lifecycle" :12021-12023), `check_origin` :12165 (checks schema equality, byte length, alignment modulus, base residue, extent within allocation :12307-12316), `establish_geometry` :12327 (returns Ok if no `EstablishAccessReference` in any function :12334-12343), `precedes` :12411 / `precedes_blocks` :12442 (path reachability from entry avoiding the producer; cut off at 12462).

**Same job elsewhere (inside this file).**
- "producer precedes consumer on all paths": `installed_interrupts::control::Graph::precedes` :5185 (dominator sets) vs `placed_reference::precedes` :12411 (DFS from entry). Suspected same semantics, different implementations (read both; did not compare equivalence).
- `unique` / `unique_index`: `installed_interrupts::unique` :8881 vs `installed_calls::unique_index` :4810.
- Per-site relocation builders: see 4.3 vs 4.8.
- Source-site tables: `incoming_calls::source_sites::SourceSites` :11389 vs `descriptor_tables`/`installed_calls` lookups (not compared).

**Unused/one-caller (grep, whole `omega-rust`).** `build_function_fragment_object_artifact` 4 line hits/3 files; `verify_retained_target_operations` 5 hits/3 files (2 external callers, both in `compiler/tests/corpus_runner.rs`); `FragmentReplay` pub(crate).

**Stated limits found.** :489-490 (full scan fallback); :4727-4729 "descriptor hardware instruction has no descriptor or installation source operation"; :12021-12023 "borrowed extent requires a successful View lifecycle"; :11540 "cleanup arrival has no reconstructed backing observation"; :11552 "installation helper arrival has no reconstructed backing observation"; :12208-12210 "entry or dynamic input has no closed physical backing supply"; :1884-1887 (image residence only on UEFI); :5136-5139 (physical entry only x86-64 PE).

**Fail-open (summary, this file).**
1. `replay::validate` :3988-4112 accepts artifacts with no `fragment_replay` unless they hit one of six listed shapes — open.
2. `dedicated_descriptor_owner` :4717-4731 skips hardware descriptor instructions owned by an installation operation (`Ok(false)`); safety depends on `installed_interrupts::validate` :8898 being run on the same function (its caller is in the next unit) — open.
3. `publication::reads` :8083-8103 accepts reads of the input loads, control words 64-96, and the table region without guards — stated by structure (these are the declared inputs; `footprints` :8171 and `Writes::complete` :8737 constrain the rest).
4. `establish_geometry` :12334-12343 returns Ok when no `EstablishAccessReference` exists — traced by structure (nothing to prove).
5. `implementation_tables::records_for_symbols` :2128-2130 skips `None` rows (trap rows) — stated by test name `trap_rows_keep_slot_offsets_and_callable_relocations` :2385 and test :2455.
6. `attribution::interval_is_exact` zero-byte rows :679-685 accepted if any instruction `supports` the site — traced (support requires exact offset/provenance).
Others: none seen in the ranges read.

---------------------------------------------------------------------
## Coverage
- `image-elf/src/shared_component.rs`: 1-727 read in full (727 lines).
- `image-elf/src/static_executable.rs`: 1-320 read in full (320 lines).
- `image-emission/src/entries.rs`: 1-5868 read in full (5868 lines).
- `image-emission/src/function_fragments.rs`: 1-12462 read in full (12462 lines). Large `#[cfg(test)]` modules inside it (attribution tests :720-1469, installed_interrupts `control_tests` :5883-6169, `cleanup_tests` :10680-10976, implementation_tables tests :2348-2478, byte_literals tests, entries.rs tests :639-833, :1925-1986, :3653-3728, :4253-4403, :4873-5024) were read at lower attention than the production code.
- Total read: 19,377 lines. Skimmed: 0 (by line count). Not read: 0.

## Five things a maintainer most needs to know about this unit
1. Every hand-written machine-code shim (AArch64/x86 receivers, Windows receiver, Unit entry, UEFI physical entry) has two independent implementations: an emitter and a byte-for-byte replay validator (entries.rs :1558/:1706, :2467/:2054, :2814/:2310, :3422/:3576, :4730/:5300). A change to one emitter requires the matching validator, and the numbers (offsets 13, 46-byte write blocks, 0x90 fills, `failed = len-13`) are literal.
2. Target dispatch is by equality against `NativeTarget::...()` constants and by string names (view names for registers, symbol names for regions and imports), repeated in about 53 `NativeTarget::` mentions in entries.rs (grep -c); adding a target means editing `receiver_form`, `physical_contract_row`, `unit_entry_form`, `receiver_call_overhead`, `validate_entry_arguments`, `call_overhead`, and `main_points_to`/`pe_`/`elf_entry_points_to` separately.
3. `function_fragments.rs` is a replay verifier: for each abstract/target/selected fact it re-derives the expected object content and compares (pools: byte_literals, implementation_tables, native_code_layouts, image_residence, private_addresses, imports). Symbol order in `ObjectPlan.layout.symbols` is an implicit contract (`imports::validate` :2659-2672 skips a computed number of symbols).
4. `source::installed_interrupts` (:5029-9627) is a ~4,600-line symbolic proof that the compiled installation code guards, orders and bounds its IDT/TSS/stack writes; it uses fixed control-block offsets (48-128), a fixed 6-argument roster, and a closed instruction-kind list. Any change to the selected-instruction lowering of installation breaks it with "unaccounted instruction" style errors.
5. Fail-closed except where no replay evidence exists: `replay::validate` has six explicit rules for the `fragment_replay.is_none()` case and otherwise passes (:3988-4111).

## What I am unsure about
- Whether `Graph::precedes` (:5185) and `placed_reference::precedes` (:12411) are equivalent; I did not compare them or trace their users.
- Whether `dedicated_descriptor_owner` skipping (:4456-4458) is always backed by `installed_interrupts::validate` being invoked; the call site is in the next unit.
- Whether unknown `MachineAlternativeFamily` values in `body_footprint::collect_from` (:328) can reach it in practice (needs `machine_code` enum read).
- Whether macOS x64 reaches `hosted_receiver::prepare` (a `physical_contract_row` exists :2979 but `validate_entry_arguments` rejects it :3056-3062; Unit entry has a macOS x64 form :3827).
- The function_fragments.rs range ends mid-function (:12462); anything after belongs to the next reader. Callers of `validate_function_fragment_object_artifact`, `source::*::validate` entry points were not seen.
- Test-code details (see Coverage) were read quickly.
