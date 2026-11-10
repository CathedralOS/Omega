# Unit 8 notes

Unit 8. Checkout `<checkout>/` (main `457adf4a11f`). All paths below are under
`omega-rust/omega/compilation/backend/instruction-set-architectures/isa-x86_64/src/` (written `src/`).

Files covered (line ranges from the unit file):
1. `src/machine_effects.rs` 1-2315
2. `src/post_handoff_writer.rs` 1-392
3. `src/preservation_storage.rs` 1-232
4. `src/register_model.rs` 1-3684
5. `src/selected_form_encoding.rs` 1-12010 (the file is 24019 lines; `wc -l` confirmed; lines 12011-24019 are another reader's unit. Submodules referenced from my half but defined after line 12010 and NOT read by me: `instruction_bytes`, `request_validation`, `saturating_forms`, `trapping_forms`, `installed_interrupt`, `hosted_*`, `table_call`, `tests`, `ValidatedX86_64SelectedFormEncoding`, `X86_64SelectedFormFootprint`, `encode_x86_64_selected_form`, `validate_x86_64_selected_form_encoding`.)

Method: every line of every range read with the Read tool in order. Counts below come from Grep (count or content mode, uncapped except where said).

---------------------------------------------------------------------

## 1. `src/machine_effects.rs` (2315 lines)

**Purpose.** Builds the x86-64 "machine effect catalog": for every selected-instruction semantic (`MachineSemanticKind`) a `MachineEffectDeclaration` that says which register operands are read/written, implicit units used/defined/clobbered, memory, stack, trap and control effects, and byte size (exact or a min/max range). Also builds the register-constraint-key roster per target (`x86_64_selected_constraint_keys`). Header comment: lines 1-7.

**Shape.**
- `mod memory` (21-336): macro `declared_semantics!` (32-57) lists 22 semantics (AccessorScalarAccess, CopyBytes, ZeroBytes, LoadEncodedScalar, StoreEncodedScalar, ScanByteSequence, CompareByteSequences, LoadPacked3/5/6/7, StorePacked, Store, AddressOffset, Load8Indexed, Load8/16/32/64, Store64, FrameAddress, CallUnit). `memory::declaration` (60-335) is one big `match semantic` that returns a tuple (memory, trap, reads, writes, encoded_memory, size) with hard-coded operand index lists and sizes, e.g. `CopyBytes => ExactBytes(36)` (104), `ZeroBytes => ExactBytes(25)` (115), `Load8Indexed => ExactBytes(9)` (222), `CallUnit => ExactBytes(5)` (274), packed loads computed `byte_count*8+(byte_count-1)*7` (182-185). Final `_ => unreachable!("ordinary address/call declaration only")` (276).
- `mod scalar_call` (337-594): `user_direct_call_effects` (347), `declaration` (370, call size 5, alignment 16, `Some(12)` extra only for `NormalizedForeignCall`, 383-390), `x86_64_normalized_foreign_call_declaration` (393-536, `pub`), `normalized_foreign_constraint` (538-564). One `#[test]` (566-593).
- `x86_64_machine_effect_catalog` (619-756): builds `MachineEffectCatalog { target, register_constraints, selected_keys, declarations }`. Declarations come from `selected_keys.declaration_keys()` mapped by a chain of `if` tests on `semantic` that dispatch to dedicated modules inside `selected_form_encoding.rs`: `installed_interrupt` (638), `memory_fence` (649), `register_access` (654), `port_write` (661), `atomic_fence` (666), `crash` (671, 680), `float_result_policy` (687), `ieee_float_arithmetic` (694), `hosted_exit_process` (702), `hosted_read_byte` (710), `floating_control` (717), `hosted_write_byte` (727); then `memory::declaration` (734), `scalar_call_declaration` for CallScalar/CallAggregate/InvokeNoNormal/NormalizedForeignCall (736-743), ReturnAggregate/ReturnScalar cloned from the ReturnScalar declaration (744-749), else the generic `declaration` (751).
- `X86_64SelectedAbi {SystemV, Microsoft}` (758) and `x86_64_selected_abi` (764-778).
- `x86_64_selected_constraint_keys` (780-961, `pub`): a ~170-line struct literal `TargetRegisterEnvironmentConstraintKeys` plus `completed.invoke_no_normal = completed.ordinary_call_constraints()` (958).
- Generic `declaration` (963-1058), `binary_alias_ladder` (1060-1103), `alternative` (1105), `encoded_effects` (1123-1416), `saturating_form` (1418), `size` (1430-1536).
- Tests 1538-2315 (build the catalog and corrupt one field at a time, asserting validation fails).
- Macro `isa_machine_effect_catalog_plumbing!` (611-617) generates `X86_64MachineEffectCatalogValidationError` and `validate_x86_64_machine_effect_catalog`.

**Inputs/outputs.** Reads `ValidatedRegisterConstraintCatalog` (rows by key), `crate::x86_64_physical_register_model_ref()` (for `rsp` view id, 69-72, 379-382, 409-411), `selected_instructions::calls::constraints::*` (521-525, 558), `SelectedNormalizedForeignCall` records. Produces catalogs (values), no mutation of shared state.

**How it decides.**
- Per-semantic tables keyed on the `MachineSemanticKind` enum variant (identity), e.g. `memory::declaration` 73-277, `encoded_effects` 1139-1272 (reads/writes) and 1273-1404 (implicit effects/memory/stack/trap/control), `size` 1430-1536.
- ABI keyed on `(target.architecture, target.object_format)` only: Elf|MachO -> SystemV, Coff -> Microsoft, Aarch64 -> Err (767-777). Pointer size/alignment are not consulted here.
- Hosted syscall rows included when `crate::hosted_syscall_target(target)` (855-860; defined `src/hosted_kernel.rs:46`, grepped). Linux system-call keys are added when `target.object_format == ObjectFormat::Elf` (902-906), a different test than the hosted one. (Read; not traced what happens for a non-Linux Elf target.)
- `x86_64_normalized_foreign_call_declaration` returns `None` unless the passed `row` equals `normalized_foreign_constraint(record)` exactly (`*row != expected`, 397-400): equality on the whole `RegisterInstructionConstraint` value.
- Callback-destination classification (416-462) keyed on `NativePlace` variant and `placement.locations` slice shape (see "Cases by shape").
- Optimization gating: `MaterializeI64` variant 1 (3 bytes) is `OptimizationSelected{ X86SelectXorZeroI64MaterializationV1 }` (1004-1015); variant 0 is `Always`.

**Cases by shape.**
- Callback materialization, 421-442: `Parameter` destinations accepted only when `placement.locations` is exactly one `Register` (counts 7 bytes) or exactly one `Stack` whose `stack_byte_offset <= i32::MAX` (12 bytes if `<= i8::MAX`, else 15); anything else `return None` (439). `Field` destinations accepted only for exactly one `Register` location (445-449) and a `callback.field_placements` that must be `Some` (453).
- `if supervisor && !record.call.callbacks.is_empty() { return None; }` (463-465): system calls with callbacks refused.
- Normalized foreign-call byte count: supervisor 12, table-dispatch 10, else `5 + callback_byte_count` (469-475).
- Variant number is a 3-bit mask of which destination kinds appear (466-468): register=1, field=2, stack=4.
- `binary_alias_ladder` (1060-1103): exactly 4 alternatives for subtract (`[3,3,6,6]`, 988) and multiply (`[4,4,4,7]`, 991).
- `ConditionalBranchU64LessThan/I64LessThan`: one alternative `ExactBytes(6)` (993-995), shifts also `ExactBytes(6)` (996-1003). `ConditionalBranchNonZero` falls to `_ =>` and uses `encoder_resolved(2, Some(6))` (1504-1506). The test pins `alternatives.len() == 1` for both less-than branches (1970, 1977). Note: `branch_forms` in `selected_form_encoding.rs` accepts `variant < 2` for these two families (`variant_count: 2`, 774-786, check at 810) while the catalog declares only variant 0. I did not read the consumer that decides which variant it asks for (see item 6).

**Same job elsewhere.**
- Same builder for aarch64: `isa-aarch64/src/machine_effects.rs` invokes the same macro (`grep isa_machine_effect_catalog_plumbing!`: 2 hits, `isa-aarch64/src/machine_effects.rs:263` and `src/machine_effects.rs:611`). Suspected parallel structure; not read.
- Sizes declared here duplicate sizes baked into encoders in `selected_form_encoding.rs`: CopyBytes 36 here (104) vs `Vec::with_capacity(36)` and `bytes.len() != 36` in `copy_bytes` (1544, 1576); ZeroBytes 25 here (115) vs `ENCODED_BYTES = 25` (11895); `AccessorScalarAccess` size `encoder_resolved(7, Some(9))` appears at 91 and again in `size()` at 1451 (the latter arm is not reached for the generic path, because `memory::declared_semantics!` takes that semantic first at 734; inferred from the dispatch order at 734-751, not by a call trace). `float_result_policy::size` and `ieee_float_arithmetic::size` are instead derived by running the encoder (5777-5784, 6602-6607), so there are two styles: hand-copied constant vs computed.
- Four hand-written x86 instruction byte decoders in this half of `selected_form_encoding.rs` (see item 5) do overlapping work (decode `xor/test/cmp/inc/movzx` reg-reg forms).

**Unused / one-caller.**
- `x86_64_normalized_foreign_call_declaration` (`pub`): grep over `omega-rust`: non-test callers `selected-instructions/analyses/src/machine_effects.rs:862` (1), plus 3 sites inside `isa-x86_64/src/selected_form_encoding.rs` at 17102, 17381, 19750 (beyond my unit; listed, not read) and the re-export `lib.rs:243`.
- `x86_64_selected_constraint_keys`, `x86_64_selected_abi`, `x86_64_register_constraint_catalog_for`: 38 grep hits total across 5 files (`register-environment/src/catalog.rs` 1, isa-x86_64 lib.rs 2, machine_effects.rs 7, register_model.rs 3, selected_form_encoding.rs 25). Not unused.
- `X86_64SelectedAbi` is `pub(crate)`; used outside `machine_effects.rs`? Not grepped separately.

**Stated limits.** None found. The grep for `unsupported|not supported|migration|TODO|FIXME|for now|not yet|only supports|legacy` over these four small files returns only error-variant names (`UnsupportedTargetAbi` etc.).

**Fail-open risks.**
- `encoded_effects` second `match semantic` has a `_ =>` default arm (1395-1403) that yields no implicit uses/defs/clobbers, `MachineEncodedMemoryEffect::NoneV1`, `UnchangedV1` stack, `NeverV1` trap, `FallThroughV1` control. Which wrong input reaches it: a newly added semantic that is not listed in the arms above and does have a side effect would be catalogued as effect-free. The first `match` (reads/writes, 1139-1272) has no wildcard, so a new variant fails to compile there; the second has the wildcard. Class: "open" for new variants (no comment says why defaulting is right). Mitigation I read: `validate_isa_machine_effect_catalog` compares the supplied catalog with `canonical_machine_effect_catalog(build)` (`selected-instructions/src/effects/catalog.rs:1624-1642`, read), i.e. it rebuilds with the same builder, so a wrong default here is wrong on both sides and not caught by the validator.
- Generic `declaration` default `_ => vec![always(size(semantic))]` (1016): same kind of default, but `size()` is an exhaustive match (1430-1536, no wildcard), so a new semantic fails to compile unless sized.
- `x86_64_normalized_foreign_call_declaration` returns `Option`; `None` means "no declaration" (fail-closed for the caller `selected_form_encoding.rs:19750` which uses `.ok_or_else(invalid)`, seen in grep output only).
- Builders in `register_model.rs` `let ... else { return; }` silently stop appending rows (see item 4); not in this file.
- Otherwise: none seen.

---------------------------------------------------------------------

## 2. `src/post_handoff_writer.rs` (392 lines)

**Purpose.** Lowers a validated `GeneratedPostHandoffWriterFragmentPlan` (a list of bit-fragment steps writing into a private memory context after control handoff) into position-independent x86-64 bytes, and reports clobbers (rax, rcx, rdx, r10, r11) and flags effect. Header comment 1-6.

**Shape.**
- `mod fragment` (8-167): `append_fragment` (12-84, `pub(crate)`), helpers `shift` (86), `register_binary` (97), `memory` (105), `load` (117), `store` (141).
- `generated_post_handoff_writer_clobbers` (179), `generated_post_handoff_writer_additional_machine_state` (189), `generated_post_handoff_writer_width` (193), `encode_generated_post_handoff_writer_bytes` (200-240), `validate_generated_post_handoff_writer` (242), `validate_common_geometry` (259-287), `encode_private_pointer_to_r10` (289-296). Tests 298-392.

**Inputs/outputs.** Reads the plan (`steps()`, `source_slot_count()`, `context_abi()`, `byte_order()`), `calling_conventions::gpr_code`, `layout_plans::validate_fragment`, `post_handoff_writer_plan::validate_step`. Produces `Vec<u8>` or `Diagnostic`. `append_fragment` returns `Option<()>` and appends to a byte vector.

**How it decides.**
- Context ABI must equal `POST_HANDOFF_WRITER_CONTEXT_ABI_V1` (245) and byte order little-endian (251).
- Plan must have >=1 step and >=1 source slot (263); private context width must be <= `i32::MAX` (256, 270); source slots must be a dense exact set (277-285).
- `append_fragment` rejects duplicate registers among the four argument registers (24-31), and `LayoutPlacementReport::IntegerAt` (47) returns `None`. Whole-container writes (`destination_lsb == 0 && width == container_width`, 61) skip the read-modify-write.
- Fixed register roles: source=rax, root=r11, fragment=rdx, container=rcx (225-228); pointer moved to r10 then `4d 8b 1a` (mov r11,[r10]) hard-coded (206-207).

**Cases by shape.**
- `load`/`store` match on `count` in `{1,2,4,8}`, any other width takes a byte-by-byte loop (118-138, 142-165).
- `append_fragment` calls `layout_plans::validate_fragment(i32::MAX as usize, "private address", ...)` (49-58), then `u32::try_from(offset)` (59).
- Container width is `(container_width / 8) as u8` (60) with no check for a non-multiple of 8 in this file (validate_fragment is the only guard; I did not read it).

**Same job elsewhere.** `isa-aarch64/src/post_handoff_writer.rs` has same-named API (`generated_post_handoff_writer_clobbers` at line 159; `append_fragment` called at `isa-aarch64/.../selected_form_encoding.rs:13294`). `validate_common_geometry` appears only here by grep (1 definition at `isa-x86_64/src/post_handoff_writer.rs:259`; aarch64 has none by that name; suspected aarch64 does its own checks). Not read.

**Unused / one-caller.** Grep (all of `omega-rust`, excluding this file) for `generated_post_handoff_writer_(width|clobbers|additional_machine_state)|encode_generated_post_handoff_writer_bytes|append_fragment`:
- `append_fragment` x86: 1 outside caller, `isa-x86_64/src/selected_form_encoding.rs:17198` (beyond my unit).
- The four `pub fn`s are only re-exported at `isa-x86_64/src/lib.rs:247-249` in the search output. The grep output shows **no caller outside the isa crates** for the x86_64 versions (the only hits are `lib.rs` re-exports and the aarch64 mirror in `isa-aarch64/src/lib.rs:138-140`). Callers may use a trait or generic path by another spelling; I did not search other spellings. So: 0 direct call sites found for `generated_post_handoff_writer_clobbers/_width/_additional_machine_state/encode_generated_post_handoff_writer_bytes` (name grep), only tests in this file use them (298-392).

**Stated limits.** None.

**Fail-open risks.** None seen. `append_fragment` returns `None` on any failed check. One note: the `_ =>` byte loop in `load`/`store` for widths other than 1/2/4/8 accepts widths like 3, 5, 6, 7 (118-138); `container_width/8` could be 3 only if `validate_fragment` allowed width 24; unverified (class: open, in another file).

---------------------------------------------------------------------

## 3. `src/preservation_storage.rs` (232 lines)

**Purpose.** Builds and validates the callee-saved "preservation storage" catalog: one group per preserved register view with size/alignment, per convention (System V vs Microsoft x64). Header 1-7.

**Shape.** `X86_64PreservationStorageCatalogError` (19: UnsupportedTarget, PhysicalRegisterModelMismatch, InvalidCatalog), `x86_64_preservation_storage_catalog(model, target)` (30-84). Tests 86-231.

**Inputs/outputs.** Reads the validated physical model, `NativeTarget`, `x86_64_preservation_convention_for_target` (`register_model.rs:95`). Returns `ValidatedPreservationStorageCatalog` via `validate_preservation_storage_catalog`.

**How it decides.**
- Model identity must equal `canonical_x86_64_physical_register_model_identity()` (34).
- Group list picked by struct equality on `NativeTarget`: `target == linux_x64() || target == macos_x64()` -> `rbx, rbp, r12..r15` (38-42); `target == windows_x64()` -> 8 GPRs + `xmm6..xmm15` (43-48); else `UnsupportedTarget` (50). Vector groups are detected by `name.starts_with("xmm")` (62) and sized 16/16, GPR 8/8 (70-71).
- `uefi_x64()` and `windows_x64()` are field-for-field identical (`backend/target/src/target_profile.rs:695-711`: both `X86_64, Coff, 8, 8`; grepped/read), so the header comment's "`uefi_x86_64`" is reached through the `windows_x64()` comparison (inferred from struct equality).
- Compare with `x86_64_preservation_convention_for_target` (`register_model.rs:95-107`): it keys on `object_format` only (Elf|MachO -> system-v, Coff -> microsoft) and does not look at pointer size. So `NativeTarget{X86_64, Elf, pointer_size:4,...}` gets a convention from that function but is rejected here with `UnsupportedTarget` (the test at 186-196 exercises exactly this; the `.ok_or(UnsupportedTarget)` at 52-53 is not reached for it because 38-50 fails first). Two different keys (exact target value vs object format) select the same ABI in two places.

**Cases by shape.** Exactly three target values accepted (linux_x64, macos_x64, windows_x64 = uefi_x64). Fixed lists at 39 and 44-47.

**Same job elsewhere.** `isa-aarch64/src/preservation_storage.rs` (exists by Glob; suspected same role; not read).

**Unused / one-caller.** `x86_64_preservation_storage_catalog`: grep outside this file: 2 hits, `isa-x86_64/src/lib.rs:251` (re-export) and `register-environment/src/lib.rs:289` (arch match `Architecture::X86_64 =>` call). 1 real caller.

**Stated limits.** None.

**Fail-open risks.** None seen (`.expect(...)` at 61, 65 panics on a missing canonical view, not accept).

---------------------------------------------------------------------

## 4. `src/register_model.rs` (3684 lines)

**Purpose.** Defines the canonical x86-64 physical register model (units, views, classes, conventions, reservations), the roster of register-constraint keys (numeric ids per operation/call arity/ABI), the builder of the register-constraint catalog (one `RegisterInstructionConstraint` row per key), and its validator. Header 1-7.

**Shape (modules in file order).**
- Top-level helpers 9-80: `fixed_operand`, `allocatable_operand`, `operand_lane`, `early_clobber`, `view_of` (panics on missing name, 64-68), `units`, `units_of`.
- `mod call_keys` (87-164): key constants and generators (`Key::call(n)`): Microsoft register call keys `(10..=14)` (113-115), SysV unit call keys `740+arity` for 0..=6 (117-119), Microsoft unit `700` for arity 2 else `720+arity` (121-125), SysV register call keys `[4,5,3,6,7,8,9]` (127-129), SysV aggregate calls `1000..1014` (131-133), SysV aggregate returns `ret(10..12)`, Microsoft aggregate `1020..1025`, return `ret(12)`, normalized foreign `call(3000)` SysV / `call(3300)` Microsoft (147-153), `x86_64_preservation_convention_for_target` (95-107).
- `mod constraint_catalog` (165-1427): `x86_64_register_constraint_catalog(model)` (210-1351) is one ~1140-line function building ~all rows; `X86_64RegisterConstraintCatalogValidationError` (1353); `validate_x86_64_register_constraint_catalog` (1366-1412); `x86_64_register_constraint_catalog_for` (1414-1426, `OnceLock` cache for the canonical model).
- `mod float_scalar_calls` (1428-1631), `float_transport_tests` (1632-1661), `mod indirect_results` (1662-1725), `mod mixed_aggregate_calls` (1726-1917), `mod mixed_calls` (1918-2046), `mod operation_keys` (2047-2372: ~100 `pub const X86_64_* : Key = Key::instruction(n)`, array `X86_64_ORDINARY_REGISTER_CONSTRAINTS: [Key;102]` at 2216-2319, `x86_64_required_register_constraints` 2321-2346), `mod packed_memory` (2373-2418), `mod physical_model` (2419-2746), `mod system_calls` (2747-2890), `mod tests` (2892-3617), then `pub use` re-exports (3618-3684).

**Inputs/outputs.** `x86_64_register_constraint_catalog` reads the `ValidatedPhysicalRegisterModel` (views by name via `view_named`, conventions by name) and returns a `RegisterConstraintCatalog { architecture, required, constraints }` with constraints sorted by key and ids renumbered by position (1341-1345). `physical_model` builds with `PhysicalModelBuilder`.

**How it decides.**
- Register identity is looked up by **string name** everywhere: `view("rsp")`, `units_of(&["rdx","condition_flags"])`, `view_named("xmm0")`, `convention("system-v-amd64")` (e.g. 216-240, 255-256, 304-313, 2454-2483). `view_of` panics if a name is missing (64-68).
- Rows keyed by `RegisterConstraintKey` (kind + number). ABI-specific rows chosen by hard-coded key ranges (see `call_keys`, `operation_keys`).
- Validation (1366-1412): (a) model arch must be X86_64 (1371), (b) model identity must equal canonical identity (1376), (c) structural validation via `validate_register_constraint_catalog` (1379), (d) for every key in the canonical `required` list the supplied row must exist and be `==` the canonical row (1383-1398), (e) every supplied row must be in the canonical required list (1399-1410). So the validator checks equality against a rebuild from the same builder.
- Physical model decisions: units are `rax.bits0_7, bits8_15, bits16_31, bits32_63` lanes per GPR (2533-2538); views for r64/r32 (ZeroExtendsParent)/r16/r8 low (PreservesUnwritten)/r8 high only for rax/rbx/rcx/rdx (2508-2523, 2582-2593); `rsp` unit kind StackPointer, non-allocatable (2528, 2539); 16 xmm units of 128 bits (2597-2615); `rflags` split into `condition` (6 bits) and `control` (58 bits) (2616-2633); `rip`, `mxcsr` units; conventions `system-v-amd64` (callee rbx rbp r12-r15, red zone 128) and `microsoft-x64` (callee rbx rbp rdi rsi r12-r15 + `xmm6..15`, red zone 0) (2668-2699); reservations: `x86.stack-pointer` (rsp), `x86.frame-pointer` (rbp), `omega.x86.dispatch` (r14), `omega.x86.metering` (r15), plus rip and mxcsr (2701-2728).
- Linux system calls: argument registers `rdi rsi rdx r10 r8 r9`, 14 keys `100..114`, `position/2` = argument count, odd positions return a value in rax, clobbers `rax rcx r11 rflags` minus result when returning (2753-2800).

**Cases by shape (fixed lists that refuse the rest).**
- SysV scalar call keys: arity 0..6 only, 7 keys (`[4,5,3,6,7,8,9]`, 127-129); Microsoft register calls arity 0..4 (113-115); SysV register unit calls arity 0..=6 (117-119); Microsoft register unit calls arity 0..=4 (121-125).
- Mixed int/float calls: SysV `general_count 0..=6 x float_count 1..=8` = 56 keys (`call_constraint_keys(800..856)`, 1928; loops 1974-1989); Microsoft `arity 1..=4` with every non-zero float mask = 26 keys (`900..926`, 1931; 1998-2019).
- Mixed aggregate calls: SysV 112 keys `1100..1212` (1733, 2 fragments x 56), Microsoft 26 keys `1220..1246` (1737, 1 fragment) (1748-1788).
- Aggregate results: SysV `fragments 1..=2` (`rax`,`rdx`) for arity 0..6: 14 keys (892-915); Microsoft 5 keys (arity 0..4, 940-966) with a single result (`rax`).
- Indirect aggregate (hidden result pointer in `rdi`/`rcx`): SysV 54 keys `2200+`, Microsoft 15 keys `2300+` (1672-1674). The code enumerates input rows that have a pointer-register operand and moves it last (1700-1722); if the pre-sized key range runs out it `return`s silently (1712-1714).
- Float scalar call keys: base `1600` SysV / `1800` Microsoft, count = register-unit keys + mixed keys (1438-1449). Float return key `ret(1600)` SysV / `ret(1601)` Microsoft (1451-1453). Result in `xmm0` (calls) or `rax` for mixed (1482-1489).
- Normalized foreign-call templates: one key per ABI (147-153), cloned from the unit call with zero args (`x86_64_*_register_unit_call_keys()[0]`, 2022-2045).
- Encoded scalar rows use `operand_lane(6, input_count)` with fixed lane counts 5/3/6/8 for copy/zero/encoded/compare (1205-1236).
- FP control rows: `mxcsr`, `xmm3..xmm5` scratch fixed (1115-1171).

**Same job elsewhere.** `isa-aarch64/src/register_model.rs` (exists; suspected analogous roster; not read). Inside this unit: `x86_64_system_v_aggregate_call_keys` rows (892-915) and `mixed_aggregate_calls::append_constraints` (1744-1789) and `float_scalar_calls::append_constraints` (1470-1520) all derive new rows by cloning an existing row, changing key, appending a result operand and removing `rax/rdx` write-units from clobbers (`row.clobbers.retain(!view.write_units.contains)`): lines 910-913, 963-964, 1504-1505, 1783. Four copies of the "result becomes a Def, not a clobber" step. The Microsoft/SysV loops at 940-966 and 1070-1092 duplicate the "unit call = scalar call minus result operand plus rax clobber" derivation.
- Key-number layout is owned in `call_keys`/`operation_keys`; the same ranges are repeated as literals in tests (e.g. key variants `1100 | 1155 | 1156 | 1211 | 1220 | 1245`, line 1890).

**Unused / one-caller.** Greps over `omega-rust`:
- `x86_64_fixed_register_view`: 11 hits in 5 files; outside isa-x86_64: `register-environment/src/lib.rs` 2 hits (count mode). Used.
- `X86_64_MICROSOFT_CALL|X86_64_SYSTEM_V_CALL|X86_64_MICROSOFT_CALL_UNIT`: 39 hits in 5 files (25 in register_model.rs, 3 in isa-x86_64 lib.rs, rest in register-environment). Used.
- `x86_64_indirect_aggregate_call_keys|x86_64_system_v_mixed_aggregate_call_keys|x86_64_microsoft_mixed_aggregate_call_keys`: 39 hits across 4 files, all inside isa-x86_64 (selected_form_encoding 9, register_model 23, machine_effects 4, lib 3). No outside consumers.
- `x86_64_linux_system_call_keys`: 17 hits across 7 files, including `07_post-allocation-machine-to-selected-form-encoding/src/row_encoding.rs` (1), `machine-emission/src/fragment_emission.rs` (1), `exit_contract.rs` (2).
- `x86_64_physical_register_model_ref|validated_x86_64_physical_register_model|x86_64_register_constraint_catalog`: 16 hits in 7 files outside isa-x86_64 (e.g. `machine-emission/src/exit_contract.rs` 6).
- `X86_64_INLINE_ASSEMBLY_DEFAULT`, `X86_64_LINUX_SYSTEM_CALL` are `pub(crate)`; both appear in the required list (2224, 2223).
- "Single/canonical" claims: the header says "canonical x86-64 physical register model". `physical_model::x86_64_physical_register_model_ref` (2486) is the one static; users outside isa-x86_64 counted above (16 hits, 7 files). Outside users exist.

**Stated limits.** Comment at 1612 only. None else.

**Fail-open risks.**
- `let Some(..) = ... else { return; }` in builders silently stops adding rows: `float_scalar_calls::append_constraints` (1474-1477, 1492-1494, 1495-1497, 1513-1515), `indirect_results::append_constraints` (1684-1689, 1702-1704, 1712-1714), `mixed_aggregate_calls::append_constraints` (1763-1769, 1770-1772, 1775-1777), `mixed_calls::append_constraints` (1966-1972, 1976, 1990-1996, 2000), `append_row` (1955-1957 returns without pushing), `append_normalized_foreign_constraints` (2036-2041), `packed_memory::append_constraints` (2386-2388). What wrong input reaches it: a model lacking `xmm0`/`rdi`/... or a catalog missing a base row. Class: "traced" for the canonical model (the model is the canonical static, all names exist; and `required` lists every key, so a missing row fails `validate_x86_64_register_constraint_catalog` at 1384-1388 with `TargetSemanticMismatch`). The skipped rows therefore turn into a validation error, not an accepted catalog. (I traced via 1383-1398, read.)
- `x86_64_register_constraint_catalog_for` (1414-1426): non-canonical model gets a freshly built owned catalog (1424) rather than an error; the caller `normalized_foreign_constraint` passes the canonical physical model (555-556) so not reachable there. Class: "stated"? No comment; "traced" for that caller only; other callers of `_for`: grep count above (5 files, `register-environment/catalog.rs` 1).
- `validate_x86_64_register_constraint_catalog` compares rows to the rebuilt canonical catalog; it cannot detect an error that the builder itself contains (same-builder equality). Not a fail-open path but a property of the check.
- Duplicate `RegisterConstraintId` values are used transiently (ids 0, 6 twice at 659-669 etc.), overwritten at 1342-1345 after sort. Not an accept path.
- Otherwise: none seen.

---------------------------------------------------------------------

## 5. `src/selected_form_encoding.rs` lines 1-12010 (of 24019)

**Purpose (this half).** Per-form x86-64 byte encoders, hand-written byte decoders/validators ("structural decode then compare against the expected stream") and footprints for selected instruction forms. Each form tends to have a module with `encode` (build bytes, then immediately `validate` them), `validate` (decode bytes and compare to the expected program for the given registers), and a footprint/effects function. Header comment lines 1-8 mentions call templates, hosted I/O and installed interrupt sequences; those live after line 12010 and are not read.

**Shape (modules in file order, `pub(crate) mod` unless stated).**
| Module | Lines | Role |
|---|---|---|
| `atomic` | 10-462 | `encode(form, registers)` (151), `effects` (24), `footprint` (55), lock-prefixed forms; bitwise fetch loops (`and/or/xor`) with `cmpxchg` retry and `jne -N` (221-251) |
| `atomic_fence` | 463-566 | declaration: 3 bytes `0f ae f0` |
| `boolean_materialization_tests` | 567-740 | test-only |
| `branch_forms` (private mod, `pub fn`s re-exported) | 741-969 | `BranchRequest`, families NONZERO (cond 0x5, 1 variant), U64_LESS_THAN (0x2, 2 variants), I64_LESS_THAN (0xc, 2 variants); 6 `pub fn` encode/validate pairs (894-968) |
| `byte_literal_address_tests`, `byte_view_address_tests` | 970-1123 | tests |
| `compare_byte_sequences` | 1124-1524 | labeled-stream loop, own `decode_one` (1304) |
| `copy_bytes` | 1525-1766 | fixed 36-byte loop, validated by byte-offset checks (1576-1592) |
| `crash` | 1767-1914 | `ud2` / privileged stop declarations |
| `decoding` | 1915-4124 | `DecodedInstruction` enum (1937-2140), `decode_all` (2149), `decode_one` (2174), `decode_atomic` (2613), `validate_decoded` (2800-3394), `expected_saturating` (3407), `expected_trapping` (3649), `footprint` (3902-4123) |
| `encoded_scalar` | 4125-4567 | `Step` plan (from `encoded_scalar_plan`), own assemble/decode (4177, 4275) |
| `float_bits` | 4568-4729 | `movd/movq` xmm<->gpr |
| `float_result_policy` | 4730-5557 | program + `Replay` validator (5078-5251) |
| `floating_control` | 5558-5848 | `stmxcsr/ldmxcsr [rsp+disp]` |
| `ieee_float_arithmetic` | 5849-8380 | 25-variant `Instruction` enum (6640-6749), `encode_instruction` (6751), `decode_instruction` (6938), `program` (7422), `classification_program` (7223), `conversions` submodule (5866-6515) |
| `memory_fence` | 8381-8519 | `0f ae e8/f8/f0` |
| `port_write` | 8520-8631 | `validate` against `x86_encoding::encode_immediate_port_write` |
| `register_access` (with `descriptor_registers` 8633-9430, `installed_table` 9431-9920) | 8632-11122 | control/system register, GDT, task register, segment reload, MSR, port, flags |
| `scan_byte_sequence` | 11123-11882 | UTF-8/ASCII/NoNul scanners as a labeled stream |
| `zero_bytes` | 11883-12010+ | 25-byte loop |

**Inputs/outputs.** Inputs: `ValidatedPhysicalRegisterModel` (identity must be canonical), `SelectedInstructionKind`, `MachineAlternativeKey`, operand `RegisterViewId` slices, byte slices. Outputs: `ValidatedX86_64SelectedFormEncoding { bytes, footprint }` or `X86_64SelectedFormEncodingError` variants (`MalformedEncoding`, `EncodedFormMismatch`, `AlternativeMismatch`, `OperandCountMismatch`, `NonCanonicalPhysicalModel`, `BranchDisplacementOutsideI8/I32`, `UnknownOrNonGpr64View`). Type definitions live after line 12010 (not read).

**How it decides.** Almost everything is "decode, then compare with a freshly constructed expected program for the same operands":
- Equality on decoded instruction lists, e.g. `decoded == [DecodedInstruction::Xor{..}]` in `validate_decoded` (2800-3394), `decoded != resolved_plan(...)` (4470), `decoded != program(...) || bytes != canonical(...)` (7848), `validate_program` via `Replay::require` (5168-5251), `validate_stream` (9287-9319).
- Keys: `Kind` (selected instruction kind enum), `alternative.variant` integer (e.g. subtract variants 0..3 at 3285-3325, multiply 3326-3361, materialize variant 1 at 3011), register numbers (u8 hardware codes 0-15; e.g. `divide_register_layout`: `registers[0]==0 && registers[2]==0 && registers[3]==2 && registers[1]!=2` at 2796-2798, i.e. rax/rdx fixed), `form` enums.
- Register resolving goes through `request_validation::resolve_registers` etc. (defined after 12010; unread).
- `branch_forms::BranchRequest::new` (803-823): canonical-model guard, `alternative.family != row.family || alternative.variant >= row.variant_count` -> `AlternativeMismatch`; width chosen by `WidthRule` (Near / Short / Variant: variant 0 = near).
- Branch displacement checks: near `i32::try_from` (850), short `i8::try_from` (856); `validate_bytes` re-parses and compares displacement (861-891).

**Cases by shape (fixed shape lists; quoted).**
- `atomic::validate_registers` (122-149): operand count must equal `form.operand_count()`; every register `<= 15`; each output register distinct from all earlier operands (132-137); CompareExchange(+Once) requires `registers[1] == 0` (rax) (138-144); bitwise fetch rejects any operand equal to rax (145-147). `retry` displacement fits `i8` or error (247-248).
- `atomic::encode` handles exactly `Load, Store, CompareExchange(+Once), Swap, FetchAdd/FetchSub, FetchAnd/Or/Xor` (160-251); widths 8/16/32/64.
- `decoding::decode_one` (2174-2611): a long `if let [..]` chain over specific byte patterns: `48 99` cqo, `74/75/eb` short jumps, `71..73`, `48 3d` cmp rax imm32, `rex c1 /5`, `rex 83 /7`, `rex 0f 40/42/47/4c/4f` cmovcc (2247-2279 `matches!(*opcode, 0x40 | 0x42 | 0x47 | 0x4c | 0x4f)`), `0f af` imul, set-boolean only for opcodes `0x92 0x94 0x96 0x9c 0x9e` (2317), `0f b6/b7/be/bf`, `63`, `89`, `ff 25` (indirect jump rip), privileged stop, `0f 0b`, `c3`, atomics, `lea`, `81 /7`, and a reg-reg opcode list `0x89 0x85 0x39 0x31 0x29 0x01 0x21 0x09 0x19 0xf7 0xd3` (2493-2496); everything else `MalformedEncoding`.
- `validate_decoded` match (2806-3388): `Kind::NearestIeeeFloatArithmetic | FloatResultPolicy => false` (3003), `layout_dependent_selected_transfers!() | dedicated_selected_forms!() => false` (3387), `InstalledInterrupt*/RegisterAccess => Err(EncodedFormMismatch)` (2807-2812), `PortWrite => unreachable!` (2813-2815). Shift forms require `registers[1] == 1` (rcx) (3382).
- `ExactDivideU64 ... ExactRemainderI64 ...` require the fixed rax/rdx layout (3127, 3135, 3148, 3165, 3175, 3186).
- Float result policy: `Replay::exponent` hard-codes shift counts 1 and 53/24 and constants 2047/255 (5116-5140); shift decode accepts counts only in `1..(32|64)` (5047).
- `ieee_float_arithmetic::program` (7422-7633): explicit match over `IeeeFloatArithmeticOperation` (DirectedBinary, DirectedSquareRoot, FusedMultiplyAdd, classification family, Compare, MultiplyThenAdd, Negate, SquareRoot, ConvertFormat, TruncateToInteger (trapping vs other), FromInteger by width, and a final `binary =>` arm using `binary_opcode` (7621-7631), which returns `Error::EncodedFormMismatch` if the operation is not Add/Multiply/Subtract/Minimum/Divide/Maximum (7376-7386)).
- `ieee_float_arithmetic::conversion_control` (6530-6549) lists operations that clobber flags (classification family, `FromInteger(U64)`, `TruncateToInteger{U64,..}`, trapping truncations).
- `conversions::interval` (6073-6085): 8 carriers, hard-coded `f64` bit-pattern bounds per carrier.
- `register_access::encode` (10215-10386): accepts only `TaskRegisterActivate [7,6,2,1,8,0]`, `SegmentReload [7,2,1,8,0]` (9056, 9071: exact register arrays), `SegmentRead`/`TaskRegisterRead` with one register `<16` and `!=4` (9059, 9074), plus per-operation special cases; `_ => return Err(Error::AlternativeMismatch)` (10383) for any other two-operand op. `registers.iter().any(|r| *r > 15)` and `produces_value && *second == 4` rejected (10326-10330).
- `installed_table` (9431-9920): `X86InstalledCpuStateRegisters::FIXED` (rdi, rsi, rcx, rax, r9, r10, r11) required exactly (9593, 9691, 9735); GDT extent 8..=65536 and multiple of 8 (9596); IDT extent 16..=4096 and multiple of 16 (9738).
- `scan_byte_sequence::layout` (11210-11364): predicates NonEmpty, ValidUtf8, NoNul, AsciiOnly, with a UTF-8 state machine over lead bytes `C2, E0, ED, F0, F4, F5` (11264-11298).
- `compare_byte_sequences::request` (1417-1434): displacement must be 0 and `registers[4] != 4` (rsp).
- `floating_control::request` (5631-5660): displacement in `0..=i32::MAX` and `% 8 == 0`, slot must be `LocalStorageSlotId::Boundary{..}` (5638-5646); decode accepts only shortest displacement form (5711-5733).

**Same job elsewhere (inside my unit; cited both sides).**
1. **Hand-written x86 byte decoders, four copies of reg-reg decode patterns.** `compare_byte_sequences::decode_one` (1304-1396), `decoding::decode_one` (2174-2611), `encoded_scalar::decode_one` (4275-4391), `scan_byte_sequence::decode_one` (11495-11665). Each re-implements the same REX/modrm field extraction (`(mode>>3)&7 | (prefix&4)<<1`) and the same `xor reg,reg`, `inc`, `test reg,reg`, `cmp reg,reg`, `movzx [base+index+disp32]` patterns: e.g. xor-self at 1320, 4296, 11516; inc at 1330, 4329 (add/sub 1), 11524; indexed byte load `0f b6 ... sib` at 1361, 4345, 11579. Grep `fn decode_one\b` over `omega-rust`: exactly these 4 hits, all in this file (1304, 2174, 4275, 11495). Plus other private decoders: `float_result_policy::decode_instruction` (5010), `ieee_float_arithmetic::decode_instruction` (6938), `atomic` decode via `decoding::decode_atomic` (2613), `register_access::descriptor_registers::decode` (9083), `register_access::decode` (10418), `floating_control::decode` (5711), `float_bits::validate` inline (4637-4679).
2. **Encode-side emitters** for the same instructions: `instruction_bytes::{append_self_clear, append_increment, append_register_compare, append_self_test, append_indexed_byte_load, assemble_labeled_stream}` are imported from a module after line 12010 (used at 1131, 1531, 8635, 11129, 11890), but `encoded_scalar::assemble` (4177-4265) writes its own `xor/mov/or/cmp` bytes with `rex()`/`modrm()` and `ieee_float_arithmetic::encode_instruction` (6751-6936) writes its own `Transfer/Test64/MoveImmediate64/Jump` bytes; `register_access::descriptor_registers::emit` (8949-9052) writes its own `mov r,r`, `lea`, `cmp/or` etc.; `post_handoff_writer.rs:86-166` writes `shift`/`mov`/`load`/`store` again; and `register_access::move_register` (10200) and `swap_bytes` (10207) again. (Counting: I did not tabulate every duplicated opcode; the above are the ones I read.)
3. **Labeled-stream jump patching** is done by shared `assemble_labeled_stream` (unread definition) in `compare_byte_sequences` (1413), `scan_byte_sequence` (11773), `register_access` (9057, 9072, 9542, 9558, 9575); but `encoded_scalar` has its own `positions/assemble/append_jump/branch_target` (4162-4273), `float_result_policy` computes displacements by hand (4903-4954), `ieee_float_arithmetic::classification_program` computes branch fixups by hand (7341-7372) and `conversions::choose` (5928-5943) computes lengths by encoding. Same job (resolve label displacements), at least five mechanisms.
4. **Branch width selection** done two ways: NONZERO picks near/short by calling a different public function (894 vs 913; `WidthRule::Near`/`Short`) while the two less-than families pick by `alternative.variant` (`WidthRule::Variant`, 937, 956). Consumers of these 8 `pub fn`s (grep, outside this file): `09_resolved-layout-to-resolved-layout/src/x86_branch_relaxation.rs` (lines 10-16, 47-60, plus tests), `08_selected-form-encoding-to-resolved-layout/src/ordinary.rs:948-962`, `.../validation.rs:542-558`, `machine-emission/src/frame_application.rs:33-39, 942-963, 1656-1680, 2074`. Three crates each switch over the same four families (suspected duplication; I read only the grep lines).
5. **Size constants repeated** between `machine_effects.rs` catalog and encoders (see item 1).
6. `validate_decoded` expected programs for saturating/trapping (3407-3849) re-state the byte-level programs whose encoders live in `saturating_forms`/`trapping_forms` (after 12010, unread). So each of those forms has a generator and an independently written expected decoded list (3396-3849), by design ("decode and compare").

**Unused / one-caller.**
- 8 `pub fn` in `branch_forms` (894-968): each has callers (grep above: x86_branch_relaxation.rs, ordinary.rs, validation.rs, frame_application.rs). Not unused.
- `X86InstalledCpuStateRegisters`, `encode_x86_64_installed_*` (9537-9586): not grepped for outside users (these may be re-exported at crate level; unverified).
- `memory_fence::encode`/`decode` (8402-8419): used inside `register_access` (10229, 10600).
- `pub(crate) mod atomic::writes_rflags` (20) used at 36, 65.
- "Single/canonical" comment claims: `decoding` header? none; none of my range says THE single way.

**Stated limits.** The only match in this half is the installed-table error strings: `UnsupportedRegisters` ("installed table operand registers differ from the explicit fixed-register contract", 9526) and GDT/IDT extent messages (9527-9528). No "remains/unsupported/migration" comments.

**Fail-open risks.**
- `decoding::decode_one` early shape tests accept by prefix only: e.g. `[0x74, displacement, ..]` (2180), `[0x75,..]` (2188), `[0xeb,..]` (2196), `[0x48,0x3d,..]` (2215) return a decoded instruction without checking trailing bytes. Acceptance is gated afterward by comparing the whole decoded vector with the expected list in `validate_decoded`, which also fixes lengths via `decode_all` consuming `length` bytes, so extra bytes decode as extra instructions and fail the compare. Class: "traced" (read `decode_all` 2149-2165 and `validate_decoded`).
- `validate_decoded` computes `-(retry_length as i8)` for the bitwise-fetch retry displacement (2993): `retry_length` is a small usize (< 30), cast safe. Not an accept path.
- `Kind::AtomicAccess` branch reads `registers[0]` etc. without a length guard in `validate_decoded` (2843-2927); a short `registers` panics rather than accepts. Class: "open" (caller guarantees via `request_validation`, unread).
- `ieee_float_arithmetic::decode_instruction` generic scalar arm accepts any opcode when `rex == 0x40` and prefix `f2/f3` (7138-7149: `[0x0f, opcode, modrm, rest @ ..] if rex == 0x40 => Instruction::Scalar{opcode}`), and `validate` then requires `decoded == program(...) && bytes == canonical(...)` (7848). Class: "traced" (second condition makes it exact).
- `register_access::validate_bits` for port/MSR ops (10671-10791) uses a bit-provenance simulation instead of byte equality: any byte sequence that decodes to the allowed instruction set (`Move`, `Swap`, `Join`, one `Physical(op)`) and yields the same register-bit flows and only allows clobbers of registers `0,1,2,10,11` (10778-10782) is accepted; different byte spellings of the same dataflow pass. That is by design (semantic check) but it is the one place in this half where validation is not byte-for-byte. Class: "stated" by function shape (no comment). Note `clobbered` uses fixed register numbers (rax, rcx, rdx, r10, r11) for all non-port ops (10781: `_ => [0,1,2,10,11]`).
- `floating_control::decode` (5711-5733): `(2, 4 bytes)` rejects offset < 128 and `(1,[offset])` accepts 1..=127; `(0, [])` yields displacement 0; any `mode>>6 == 3` falls to `_ => None`. Fail-closed.
- `Replay::branch` in `float_result_policy` (5100-5115) compares the displacement to a destination computed from `bytes.len() - 5` (5196) for the saturating form: the "raw destination" is assumed to be the final 5-byte transfer. Not an accept path.
- None else seen.

---------------------------------------------------------------------

## Coverage

- Read in full, in order: `src/machine_effects.rs` 1-2315 (2315); `src/post_handoff_writer.rs` 1-392 (392); `src/preservation_storage.rs` 1-232 (232); `src/register_model.rs` 1-3684 (3684); `src/selected_form_encoding.rs` 1-12010 (12010). Total read: 18633 lines.
- Skimmed: none.
- Not read: none inside the unit. (Lines 12011-24019 of `selected_form_encoding.rs` are outside this unit and unread; they define the submodules/types listed at the top.)
- Long hex-vector tests (e.g. 8254-8314) and test bodies were read but only lightly analysed; analysis effort went to non-test code.

## Five things a maintainer most needs to know about this unit

1. Almost every x86 form is verified by "decode the bytes with a hand-written decoder, rebuild the expected program for the same registers, compare for equality". That means four separate hand-written decoders for overlapping instruction patterns in one file (`selected_form_encoding.rs:1304, 2174, 4275, 11495`) and more form-specific ones (5010, 6938, 9083, 10418). A change to how one pattern is encoded must be mirrored in the generator, the decoder, the expected list, and the catalog size.
2. Catalog validation is "structural check + equality with a rebuild from the same builder" (`selected-instructions/src/effects/catalog.rs:1624-1642`; `register_model.rs:1383-1410`). It detects a changed or removed row but not a wrong row emitted by the builder itself. Effect defaults (`machine_effects.rs:1395-1403`, `1016`) are therefore only as correct as the builder.
3. Register-constraint keys are numeric ranges chosen by hand (`register_model.rs:87-164, 1928-1931, 1733-1737, 1672-1674, 2047-2372`) with arity/fragment enumeration loops that stop silently if a key range is too short (`1712-1714`, `1770-1772`, `1976`, `2000`). The only guard is the required-key list in the validator.
4. ABI/target selection uses two different keys: `machine_effects.rs:764-778` and `register_model.rs:95-107` key on `object_format` only, while `preservation_storage.rs:38-50` keys on whole `NativeTarget` equality (so UEFI == Windows because `uefi_x64()` and `windows_x64()` are identical values, and an Elf target with 4-byte pointers gets a convention but no preservation catalog). Hosted rows use `hosted_syscall_target`, Linux syscall rows use `object_format == Elf` (`machine_effects.rs:855-860, 902-906`).
5. Branch width is chosen by two different mechanisms (function choice for NONZERO vs `alternative.variant` for U64/I64 less-than), the catalog declares a single 6-byte alternative for the less-than families (`machine_effects.rs:993-995`, test 1970-1982) while the encoder accepts variants 0 and 1 (`selected_form_encoding.rs:777, 785, 810`), and three crates consume the same four families (`x86_branch_relaxation.rs`, `ordinary.rs`/`validation.rs`, `frame_application.rs`).

## What I am unsure about

- Whether the less-than short variant (variant 1) is ever requested by consumers given the catalog declares only variant 0; I did not read `x86_branch_relaxation.rs` / `frame_application.rs` beyond grep lines.
- Whether `validate_fragment` (in `layout_plans`) guarantees `container_width % 8 == 0` and widths in {8,16,32,64} for `post_handoff_writer::append_fragment`; unread.
- Whether the `AccessorScalarAccess` arms in `encoded_effects` (1176-1182) and `size` (1451) are dead: inferred from dispatch order (734 before 751), not traced with a call search.
- Behaviour for `ObjectFormat::Elf` targets that are not Linux/hosted (Linux syscall rows still added at 902-906; consequences unread).
- Contents of everything defined after line 12010 in `selected_form_encoding.rs` (type definitions, `request_validation`, `instruction_bytes`, `saturating_forms`, `trapping_forms`, `installed_interrupt`, `hosted_*`, `tests` helpers), on which several judgments here depend (resolve_registers, byte_loop_registers, assemble_labeled_stream).
- Counts of "number of keys" I derived from range literals (e.g. 54 SysV indirect aggregate keys, 112 mixed aggregate keys) are read from literals (1672-1674, 1733), not from a run.
