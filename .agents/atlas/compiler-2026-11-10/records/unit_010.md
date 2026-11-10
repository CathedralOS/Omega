# Unit 10 notes

Unit 10. Files covered (paths under `omega-rust/omega/compilation/backend/`):
- machine-emission/src: frame_application.rs, frame_layout.rs, frame_protocol.rs, function_realization.rs, lib.rs, native_bridge_encoding.rs, native_entry_bridge.rs, native_foreign_bridge.rs, text_placement.rs
- machine-emission/tests/native_foreign_bridge.rs
- object/object-file/src: fragment_container.rs, lib.rs, names.rs, object_plan.rs, target_matrix.rs, relocation_free_object/{mod,publication,text_section}.rs
- plans/backend-plan/src: callback_artifacts.rs, direct_boundary_leaves.rs, lib.rs, synchronous_invocations.rs, target_providers.rs
- plans/effects/src: external_providers.rs, lib.rs, provider_evidence.rs, provider_evidence/{authority,component_eras,executable_scopes}.rs, provider_evidence/providers/mod.rs

All paths below are relative to `omega-rust/omega/compilation/backend/` unless they start with another crate root. "read" = I read the lines; "grepped" = Grep tool count/content over `omega-rust`.

---------------------------------------------------------------------------------------------------
## A. machine-emission (native function text, frames, bridges)

### A1. machine-emission/src/lib.rs (82 lines)
1. Purpose: crate root. Header comment (1-25) names a four-stage route (function_realization -> fragment_emission -> frame_application -> text_placement) plus `code_layout` and the two bridges. Declares modules and re-exports (32-78).
2. Shape: `count_where` helper (80). Modules `code_layout`, `exit_contract`, `fragment_emission` are named here but live outside this unit.
4. Decides: nothing.
7. Unused/one-caller: `count_where` is `pub(crate)`; used in frame_layout.rs:788,870,871,887 and frame_protocol.rs:322-323 (read).
8. Stated limits: none.
Doc drift (read): lib.rs:22 names `emit_installed_fatal_entry` and `emit_installed_interrupt_entry`; the real functions are `emit_installed_interrupt_entry_bridge` (native_entry_bridge.rs:840, pub) and `emit_installed_fatal_entry_bridge` (native_entry_bridge.rs:1021, private `fn`). There is no pub fn by the name lib.rs gives for the fatal path.
9. Fail-open: none seen.

### A2. frame_protocol.rs (428 lines) - prologue/epilogue byte arena
1. Purpose: builds and replays the prologue/epilogue bytes for every function from a validated frame layout, packing them in one byte arena addressed by `FrameProtocolByteSpan{offset,length}`.
2. Shape: `FrameProtocolCodec{X86_64,Aarch64}` (22); `TargetFrameProtocolEncodingError` (28); `TargetFrameProtocolEncodingReceipt` (48); `ValidatedTargetFrameProtocolEncoding{plan:Arc<..>,receipt}` (74); `stage_target_frame_protocol_encoding` (93); `validate_target_frame_protocol_encoding` (102); `frame_protocol_codec(target)` (122); `encode_frame_function` (133); `derive` (189); `append` (237); `validate_bytes` (253); `validate_span` (296); `seal` (314). Plan type itself lives in `machine_code` (re-exported 10-14).
3. In/out: reads `ValidatedTargetFrameLayout`, `ValidatedTargetRegisterEnvironment`; calls `isa_x86_64::encode_system_v_amd64_frame_protocol` (152) or `isa_aarch64::encode_aapcs64_frame_protocol` (175). Produces `TargetFrameProtocolEncodingPlan`.
4. Decides: codec chosen by `(architecture, object_format)` match (122-131): x86-64 with Elf/Coff/MachO -> X86_64 codec; Aarch64 with Elf/MachO -> Aarch64; `(Aarch64, Coff)` -> None. Per function: `committed_extent(function)` must exist (211); `continuation.admits(function.return_address)` (213). Replay (validate_bytes 253-294) re-encodes every function and compares span offsets, lengths and bytes (validate_span 296).
5. Cases by shape: the only policy is `CanonicalFixedFrameV1` (194, 258); anything else -> `UnsupportedPolicy`. The x86-64 codec is the System V encoder for ALL x86-64 object formats including Coff/Windows (124-126, 152) - the Windows target goes through `encode_system_v_amd64_frame_protocol` (read; the function name is "system_v").
6. Same job elsewhere: the same `(arch, object_format)` matrix with `(Aarch64, Coff) -> None` is written separately in frame_layout.rs:223-232 (`stack_commit_granule_bytes`), frame_layout.rs:234-270 (`frame_unwind_policy`), object-file/target_matrix.rs:36-60 (`object_target_policy`), and isa-x86_64/isa-aarch64 `machine_effects.rs` (grepped: `Aarch64, ObjectFormat::Coff` appears in 5 non-test files: isa_x86_64 machine_effects.rs, isa_aarch64 machine_effects.rs, target_matrix.rs, frame_layout.rs x2, frame_protocol.rs x1; command: Grep count, glob `!**/tests/**`). A test (frame_protocol.rs:395-427) asserts codec and unwind matrices declare the same pairs.
7. Unused: none seen.
9. Fail-open: none seen. The replay compares against re-encoded bytes, so a candidate arena with extra trailing bytes is rejected (290).

### A3. frame_layout.rs (2527 lines; tests from 1603) - callee-save storage and frame geometry
1. Purpose: (a) maps each function's modified callee-saved register units to abstract area-relative save slots via the target's preservation catalog; (b) computes the concrete stack frame (outgoing args/shadow area, locals, callee-save area, saved link register, red zone, probe schedule, unwind restores) and replays both by independent re-derivation.
2. Shape: errors `TargetFrameLayoutError` (50), `NonAuthoritativeCalleeSaveStorageError` (69); receipts (92, 136); `ValidatedNonAuthoritativeCalleeSaveStorage` (121); `ValidatedTargetFrameLayout{plan:Arc,receipt}` (166); `FrameContinuationCustody{CallerActivationStack,LinkRegister}` (184) with `admits` (195); `FrameUnwindPolicy` (211); `stack_commit_granule_bytes` (223); `frame_unwind_policy` (234); `call_site_stack_contract` (272); `body_writes_link_register` (298); `contains_call` (323); `stable_address_loans` (336); `unwind_restores` (353); `committed_extent` (383); `stage_non_authoritative_callee_save_storage` (399); `validate_non_authoritative_callee_save_storage` (415); `admit_storage_plan` (456); `derive_storage_function` (524); `reconstruct_storage_functions` (600); `storage_usage` (672); `non_authoritative_callee_save_storage_identity` (731); `stage_target_frame_layout` (824); `validate_target_frame_layout` (835); `derive_frame_layout` (897); `function_layout` (987); `probe_plan` (1220); `align_up_frame` (1238); `align_to_residue` (1248); `validate_frame_layout_plan` (1260); `minimal_aligned_extent` (1582); `save_region_is_disjoint` (1589). `MAX_FRAME_BYTES = i32::MAX` (43).
3. In/out: reads `StagedOptimizedPostAllocationMachinePlan`, `ValidatedAllocatedCalleeSavedRequirements`, storage plan, `ValidatedTargetRegisterEnvironment`, `selected_abi_preservation`, `selected_preservation_storage_catalog`. Produces `NonAuthoritativeCalleeSaveStoragePlan` and `TargetFrameLayoutPlan` (types in `machine_code`).
4. Decides:
   - A call is any instruction whose `alternative.key.family` is one of `CallScalar|InvokeNoNormal|CallUnit|CallAggregate|NormalizedForeignCall` (323-334): keyed on the machine-alternative family.
   - Stable address loans = distinct non-Spill local slots used by `FrameAddress` operands (336-351).
   - Red-zone residency only when ABI is SystemVAMD64 AND no call AND no callee-save slots AND frame_size != 0 AND frame_size <= red_zone_bytes AND all local slots are `LocalStorageSlotId::Spill` (1128-1142; mirrored in `committed_extent` 383-397 and validation 1418-1432).
   - Link register saved when `contains_call || writes_link_register || policy==CanonicalSavedReturnAddressFrameV1` (1163-1166), else live (1180-1185).
   - Windows shadow area is 32 bytes when ABI is MicrosoftX64 and function contains a call (1002-1006).
   - Probes: `touches = ceil(frame/interval)` when frame > interval (1226-1231).
   - Replay checks padding minimality (`minimal_aligned_extent`) rather than recomputing in one place.
5. Cases by shape: `stack_commit_granule_bytes` 4096 for x86-64 (all formats) and Aarch64 Elf, 16384 for Aarch64 MachO, None for Aarch64 Coff (223-232). `frame_unwind_policy` has one row per supported pair (234-270). `call_site_stack_contract` entry residue 8 for x86-64 ABIs, 0 for Aarch64 ABIs, anything else None (280-291). Saved-return-address policy variant: see 7.
6. Same job elsewhere: callee-save slot assignment is done twice on purpose: `derive_storage_function` (524-585) and `reconstruct_storage_functions` (600-670, keyed-group reconstruction) with the alignment arithmetic duplicated inline at 635-641 vs `align_up_storage` (587). Three alignment helpers: `align_up_storage` (587), `align_up_frame` (1238), `align_to_residue` (1248), plus an inline mask in the replay (1404). The frame replay (1260-1580) re-derives the same geometry that `function_layout` (987-1218) derives, written independently.
7. Unused/one-caller (grepped, command `Grep "stage_function_frame\(|TargetFrameLayoutPolicy::"` over omega-rust): `TargetFrameLayoutPolicy::CanonicalSavedReturnAddressFrameV1` is never passed by any caller outside tests: the only producer of a policy is native-compiler/src/pipeline.rs:161 which passes `CanonicalOrdinaryCallFrameV1`; function_realization.rs:473 hardcodes Ordinary in validation, and function_realization.rs:347 rejects any other policy. The branches that test it (frame_layout.rs:1165, 1532) are therefore reachable only from tests/direct `function_layout` calls. `stack_commit` submodule (45-47) is a re-export used by native_entry_bridge.rs:683,958,1109 and native_foreign_bridge.rs:247.
8. Stated limits: none by comment. Error `FrameExceedsAddressableExtent` (59-61) says frames above `MAX_FRAME_BYTES` are not addressable by nonnegative i32 displacements.
9. Fail-open: none seen. `committed_extent` returns None (i.e. reject) when `resident != 0` and any of the red-zone conditions fail (386-396). Budget check in `admit_storage_plan` (490-496) rejects over budget.

### A4. function_realization.rs (1126 lines)
1. Purpose: stages the per-function frame (requirements -> save storage -> layout -> protocol) before instruction encoding, then after encoding/layout assembles a `FunctionRelativeOptimizationRealizationManifest` (wire v12, magic `OMGFRM\0\0`) and a custody receipt that binds all stage identities.
2. Shape: `FunctionRelativeFrame{requirements,storage,layout,protocol}` (146); `StagedFixedFrameFunctionRelativeRealization` (164); manifest struct (99-132, 7 "unavailable" fields at 125-131); `stage_function_frame` (305); `validate_frame` (341); `assemble_fixed_frame_realization` (373); `validate_fixed_frame_function_relative_realization` (433); `expected_fixed_frame_manifest` (517); `validate_layout_optimization_manifest_roots` (686); `function_relative_statistics` (708); `encode_manifest_content` (808); `decode_manifest_content` (894); `decode_optimization` (1086).
3. In/out: reads allocation output, optimized target operations, machine plan, selected-form encoding, baseline and relaxed resolved layouts, exit contract. Produces manifest + `StagedFixedFrameFunctionRelativeRealizationCustodyReceipt`.
4. Decides: `expected_fixed_frame_manifest` (553-573) maps `AllocationEvidence` variants to expected transformation lists; `RuntimeSpill(identity)` is accepted only if `identity == post.identity` (569); any other variant -> `RootMismatch` (572). Then a long conjunction of identity equalities (583-615). Keyed on content identities (digests), not names.
5. Cases by shape: `decode_optimization` accepts only tags 10 and 13-20 (1086-1105) (nine named optimizations); tags 11, 12 and everything else error. `encode_optional_custody` writes `custody.optimization() as u8` (1037), so the wire tag is the enum discriminant. Layout-policy wire tags 1,2,4,5 (851-856; tag 3 unused). Decode rejects manifests carrying both `x86_branch_relaxation` and `post_allocation_machine_optimization` (792-798). `post_allocation_machine_optimization` is always `None` when built (641).
6. Same job elsewhere: another v-numbered manifest codec with the same shape (magic, version, identity, content) in text_placement/publication: publication.rs:133-140 uses `names::encode_manifest_envelope` (names.rs:142), but function_realization.rs:761-768 inlines the same envelope by hand (suspected duplication; not read elsewhere).
7. Unused: `validate_fixed_frame_function_relative_realization` is `pub(crate)`; I did not grep further than seeing no use in this unit.
9. Fail-open: none seen; unmatched `AllocationEvidence` -> reject (572).

### A5. frame_application.rs (2571 lines; tests from 1725)
1. Purpose: inserts each function's prologue at offset 0 and an epilogue before every retained return instruction, shifts block/instruction/fixup offsets, widens short conditional branches that the insertion pushed out of reach, and re-encodes every branch. Also provides a replay validator.
2. Shape: `StagedFunctionFragmentFrameApplication` (111) with `validate_relative_branch_rows` (134, pub); receipt (215); `FunctionFragmentFrameApplicationError` (282) and a second private error `FrameApplicationError` (338) mapped by `From` (315); `WIDENING_GROWTH = 4` (63); `ShortConditionalForm` (385); `short_conditional_form` (403); `apply_frame_protocol_to_fragments` (445); `apply_function` (510); `validate_and_collect_returns` (635); `reencode_branches` (785); `encode_branch` (928); `widen_out_of_range_branches` (1029); `widen` (1076); `validate_frame_protocol_application` (1150); `validate_function` (1229); `validate_widths` (1411); `validate_branch_row` (1505); `decode_branch_row` (1625).
3. In/out: reads `FunctionFragmentEmissionPlan`, `TargetFrameProtocolEncodingPlan`, physical register model; calls isa_x86_64/isa_aarch64 branch encoders. Produces `FunctionFragmentFrameApplication` (framed fragments + per-function epilogue list).
4. Decides:
   - Return sites = rows whose `control` is `FunctionFragmentControlProvenance::Return` (586, 667); a return must be last in its block (668).
   - Rows with control `HostedExitProcess|OperationTrap|Crash|InternalCall{owner:InvokeNoNormal}` must be last in their block and nonempty (654-666) - these get no epilogue.
   - Empty prologue XOR epilogue -> `UnsupportedFramedControl` (517).
   - Short-form widening: x86-64 only for predicate `NonZeroV1` (408-414: width 2, reach i8); Aarch64 only when `decoded_register_reads` is empty (415-425: width 4, imm19 reach). Widening repeats to a fixpoint (1034-1040) and grows each short branch by 4 bytes (63, 1094).
   - Aarch64 NonZero with exactly one register read is re-encoded as CBNZ (977-992, 1691-1703).
5. Cases by shape: x86-64 widening only handles `NonZeroV1`; `U64LessThanV1` and `I64LessThanV1` are always encoded with the same (long) form and never widened (955-968). Unconditional jumps are re-encoded in place and must keep the same byte length or fail (850) - there is no jump widening. Function-level framed control: see 4.
6. Same job elsewhere: widening closure is computed twice, `out_of_reach_short_rows`+`widen` (1043-1140) in the forward path and `validate_widths` (1411-1503) in replay; the "owner/provenance" checking in text_placement duplicates (see A9).
7. Unused/one-caller (grepped, Grep count "validate_relative_branch_rows"): 3 occurrences: definition + image-emission/src/object_artifact.rs (1) + compiler/tests/support/native_entry_controls.rs (1). So one production caller outside the unit.
8. Stated limits: none.
9. Fail-open: `validate_and_collect_returns` only checks end-of-block position for the listed terminal control kinds; other control kinds are not constrained (645-678). A function with no `Return` row at all still receives a prologue and zero epilogues (traced in `apply_function` 549-571, empty return_sites); the tests (1900-2020) show this is intended for crash/exit-only functions ("crash keeps frame without return cleanup"). Classification: stated by test names, not by a comment at the site.

### A6. native_bridge_encoding.rs (588 lines)
1. Purpose: shared byte-emission primitives for the two native bridges: split values into word pieces, allocate snapshot homes in the adapter frame (with over-alignment padding), emit loads/stores/address computations for x86-64 and Aarch64.
2. Shape: `NativeBridgeEncodingError` (12), `WordLocation` (31), `ValuePiece` (37), `ValueStorage{Direct,Indirect}` (43), `value_storage` (77), `value_pieces` (109), `allocate_value` (181), `allocate_home` (209), `Encoding{architecture,model,scratch,address_scratch,aligned_homes}` (195), `Encoding::{view,memory,address,address_register,constant,place_address,pointer_memory,copy_indirect,distribute_argument,capture_registers,capture_stack,distribute,copy_word}` (239-587).
4. Decides: `value_pieces` (109-179) accepts register pieces only of size 1,2,4,8 (120); stack pieces are cut greedily into 8/4/2/1 (`widest_word` 23); indirect pointer only for borrowed classes (146-150); all pieces must tile `[0,byte_size)` exactly (167-177). `Encoding::view` (239-264) names registers by spelling: `format!("x{n}")`, `d{n}`, `xmm{n}`, a hard-coded x86 name list, then `view_named(&name)` on the register model (259-263) - keyed on source spelling of register names. `rsp` is refused (252).
5. Cases by shape: widths 1/2/4/8 only (24, 120). `Encoding::view` only accepts `Aarch64X < 31`, `Aarch64V < 32`, `X86Xmm < 16`, x86 GPR index < 16 (240-257).
7. All functions are `pub(crate)`, used by native_entry_bridge.rs and native_foreign_bridge.rs (imports at entry_bridge 20-23, foreign 13-16). No other users (module is private, lib.rs:39).
9. Fail-open: none seen.

### A7. native_foreign_bridge.rs (419 lines) + tests/native_foreign_bridge.rs (229 lines)
1. Purpose: emit the adapter that lets the host call into an admitted C-ABI plan with `(arguments_array, result_buffer)`; it snapshots arguments into aligned frame storage, fills registers/stack, sign-extends narrow signed scalars, emits one unresolved direct call, copies the register result out. `validate_native_foreign_bridge` replays by re-emitting.
2. Shape: `NativeForeignBridgeRequest{target,foreign,scalar_parameters}` (19), `NativeForeignBridge` (26), `NativeForeignBridgeError` (52), `emit_native_foreign_bridge` (88), `extend_signed_scalar` (374), `validate_native_foreign_bridge` (414).
4. Decides: plan compatibility keyed on `foreign.policy == CallingPolicy::native_for_target(target)`, `entry_control == CallReturn`, `stack_alignment == 16`, no callback materializations (104-108). Scalar type join: each `Some(scalar)` must equal `ValueShape::for_scalar(scalar)` of the parameter (93-101). Direct register result only (stack result -> `UnsupportedValueTransport` 217-221). Sign extension when scalar is signed 8 or 16 bits (322-331, 383-393).
5. Cases by shape: uses `encode_system_v_amd64_frame_protocol` for every x86-64 target (266), so a Windows foreign bridge uses the SysV prologue encoder (read). Scratch registers fixed: Aarch64 x16/x17, x86 r10/r11 (130-133).
6. Same job elsewhere: frame-size + probe-count computation is repeated in native_entry_bridge.rs:677-690, :958-965, :1104-1116 and here 235-254 (read; four copies).
7. Unused: `emit_native_foreign_bridge` callers (grepped "validate_native_foreign_bridge|validate_native_entry_bridge": 19 occurrences in 5 files incl. image-emission entries.rs:1, object.rs:1).
9. Fail-open: none seen.
Test file (read): builds assembly from the bridge bytes and links against `samples/cli/facts/foreign_retained_shared_borrow/fixture/provider.c` using the host C compiler (158-163); uses `NativeTarget::host()` only (73), so only the host target is exercised. Includes `native_foreign_bridge.c` (155) which is not in this unit. A second test (200-229) asserts a plan for the other policy is rejected on four targets.

### A8. native_entry_bridge.rs (1983 lines; tests from 1250)
1. Purpose: emit bridges that adapt an external calling convention (or an x86-64 interrupt arrival) to a function's internal calling plan: snapshot inbound values, permute them as parallel moves, save/restore clobbers the inbound promise excludes, call the internal body. Also emits the installed timer interrupt bridge (acknowledgement token handling) and the fatal-context interrupt bridge.
2. Shape: `NativeEntryParameter{inbound_ordinal,scalar_codec}` (26), `NativeEntryBridgeRequest` (32), `NativeEntryBridge` (40), `NativeEntryBridgeError` (79), `finish_native_entry` (163), `select_entry_protocol` (229), `select_interrupt_protocol` (250), `validate_fatal_entry_protocol` (298), `value_transport` (341), `emit_native_entry_bridge` (382), `emit_installed_interrupt_entry_bridge` (840, pub), `emit_installed_fatal_entry_bridge` (1021, private), `validate_native_entry_bridge` (1239). Hand-coded x86 helpers `push_or_pop`, `require_scratch_equal`, `add_one_without_overflow`, `descriptor_store`, `compare_eax`, `require`, `halt` (1201-1237).
3. In/out: reads `CallPlan`, `BoundaryEntryPlan`, optional `StateEffectsEvidence` body footprint, `runtime_abi::static_interrupts` geometry. Produces bytes, frame size, call-field offset, generated-entry evidence (decoded from the emitted bytes via `decode_x86_64_generated_entry` / `decode_aarch64_generated_entry`, 171-174).
4. Decides:
   - Entry protocol keyed on `(architecture, entry_control)` (230-247): `CallReturn` any arch; `InterruptReturn` only x86-64; else `UnsupportedEntryControl`.
   - Interrupt protocol requires `saved_state == restored_state == {GeneralRegisters, Flags, InstructionPointer, StackPointer}`, 16-byte alignment, no result (252-267) and body footprint within `permitted_transitive_use` and within that restored set (269-271, 326-333).
   - Value transport (341-380) accepts: borrowed reference from an 8-byte integer with codec 0; borrowed scalar reference from an 8-byte integer with a decodable `ScalarStorageCodec` (nonzero codec); or identical non-borrowed shape (size AND alignment equal, codec 0). Else `UnsupportedValueTransport`.
   - Scratch registers: fixed candidate lists (475-496) filtered by "not occupied by any piece" and preferring volatile ones (497-508).
   - Preserved registers: every register the bridge writes (internal clobbers + scratch + destination registers) that the inbound promise does not clobber and that is not a direct result (647-658).
5. Cases by shape (read):
   - Installed interrupt bridge: exactly one parameter, owned, size `ack::BYTES` (40), alignment `ALIGNMENT`, no result, codec 0, ordinal 0 (867-893); destination register indices < 16 (895-899); dedicated stack, masked preemption (862-865). Error text 106-108: "exactly one owned 40-byte acknowledgement and no result".
   - Fatal bridge: exactly one parameter, inbound shape `ValueShape::integer(8,8)`, internal class `BorrowedReference` (1068-1072); stack `Interrupted` or `Dedicated` (1045-1048); "installed fatal arrival requires exactly one shared context reference and no result" (109-111).
   - Interrupt entries refuse any vector register (xmm/v) in sources, destinations, or writes (614-636).
   - Hand-written machine bytes for the fatal bridge (1140-1197) include literals such as `[0x48, 0x83, 0xe4, 0xf0]`, descriptor-table checks; not derived from an encoder.
6. Same job elsewhere: frame size + stack-probe computation duplicated four times (see A7). The "frame_bytes + 8 on x86-64" rule appears as `frame_alignment_padding` (here 219, 234-236, 273-274) and as a literal `+8` in native_foreign_bridge.rs:239-243.
7. Unused: `validate_native_entry_bridge` has 15 occurrences, mostly in this file's tests. `NativeEntryBridge::call_stack_bias` (73) public.
8. Stated limits (error display text, 106-130): "this target cannot emit the requested entry return"; "interrupt entry requires the supported x86-64 integer-register save/restore plan and 16-byte stack alignment".
9. Fail-open: `emit_native_entry_bridge` for non-interrupt entries with `body_footprint: None` is accepted at the selection stage; `finish_native_entry` always calls `request.checked_complete_effects` which returns `MissingInterruptBodyFootprint` when the footprint is None (141-149, 176), so a missing footprint is rejected even for ordinary entries (traced; tests supply `Some(StateEffectsEvidence::empty())` at 1295). No accept-on-missing found.

### A9. text_placement.rs (1799 lines)
1. Purpose: concatenates framed function fragments into one dense text section with no padding, patches internal machine-call displacements, records section-relative foreign-call and image-address fix-up fields as unresolved relocations, and seals a v10 text-section manifest and receipt. Provides a replay validator.
2. Shape: `stage_optimized_fixed_frame_text_section` (52), `validate_optimized_fixed_frame_text_section` (65), `StagedOptimizedFixedFrameTextSection` (103), `RelocationFreeTextSectionPlacementError` (159), `TextPlacementError` (330), `TextPlacementInput` (352, one variant `InternalCalls`), `place_fixed_frame` (525), `resolve` (807), `resolve_foreign` (928), `place_image_address` (767), `check_placed_section` (1150), `check_placed_functions` (1239), `check_internal_call` (1484), `check_foreign_call` (1599), `unchanged_bytes` (1687).
3. In/out: reads framed fragments; produces `RelocationFreeTextSectionPlacement` (type in machine_code) and manifest.
4. Decides:
   - Function order = fragment order, offsets are running sums; AArch64 spans must be 4-aligned (461-472, 528-531).
   - Internal call patching: x86-64 call must be exactly `[0xe8,0,0,0,0]` with patch at +1 and reference at +5 (833-839); Aarch64 `bl` must be `0x9400_0000` with word-aligned displacement within +/-2^25 words (856-873). Keyed on exact bytes and offsets.
   - Call owner join (582-608): owner variants `Operation`, `InvokeNoNormal`, `ExternalCompletion`, `Installation`, `CleanupAction` each have their own provenance condition; `_ =>` -> `SourceShapeMismatch` (607).
   - Relocation requirement: `UnresolvedObjectSymbolFieldsV1` if any image address, else `UnresolvedNormalizedForeignImportFieldsV1` if any foreign call, else `ProvenNone...` (714-720, 1202-1215).
5. Cases by shape: callback materializations fixed to two arch forms: x86 `X86_64Relative32` with patch at relative offset 3 (1019), Aarch64 `Aarch64PageAddress` with patches at 0 and 4 (1061); Aarch64 `OutgoingStack` destination refused (1055-1057). Image-address fixup words must match `word & 0x9fff_ffe0 == 0x9000_0000` (ADRP) and `word & 0xffff_fc00 == 0x9100_0000` (ADD imm) (756-758).
6. Same job elsewhere (read): the call-owner provenance match is written twice, in `place_fixed_frame` 582-608 and `check_placed_functions` 1313-1338, with slightly different operation checks (`exact_operation(..) == operation` vs `last() == Some(&operation)`). Also `validate_alignment` (461) and the inline alignment checks in `record_source_function` (1232), `check_block_span` (1757), `check_instruction_span` (1784).
7. Unused/dead (grepped "TextPlacementInput|UnsupportedRelocationShape": 20 occurrences, all in text_placement.rs): `TextPlacementInput` has one variant, so `matches!(input, TextPlacementInput::InternalCalls(_))` at 1303, 1397, 1434 is always true and `UnsupportedRelocationShape` (returned at those lines) cannot be produced by them; `validate_placement_source` (381-385) always returns Ok.
8. Stated limits: none.
9. Fail-open: none seen. Statistics path rejects remaining unresolved internal fixups (455-457).

---------------------------------------------------------------------------------------------------
## B. object-file (two object lanes)

### B1. object-file/src/lib.rs (115 lines)
1. Purpose: crate root; header comment (1-59) declares two lanes: mutable `ObjectPlan`/`RelocationPlan` (OMGOBJ, consumed by image builders) and immutable `RelocationFreeObjectPlan` (OMGTRO, plus OMGTOM manifest). Re-exports.
2. Notable stated claims: five-pair target policy; operation/edge identities kept as distinct variants (37-40); lookups "scan the entire remaining table after a match and return an invalid handle (or `None`) if a duplicate exists" (41-43) - confirmed in names.rs:35-40 and 103-111. OMGOBJ version 6 and the wire layout are described (14-31) but the OMGOBJ encoder is not in this unit.
7. Outside users of the "canonical" lane items (grepped): `ObjectPlan` 103 occurrences in 14 files (image, image-emission, image-macho...), so the mutable lane is widely used. `relocation_kind_tag|invocation_relocation_origin|RelocationRecordSet|ObjectContainerOutput`: 29 occurrences in 9 files (object_plan.rs 9, lib.rs 3, image-emission 5, image-elf 2, image 2, native-artifact 8).
9. Fail-open: none seen.

### B2. target_matrix.rs (109 lines)
1. Purpose: `object_target_policy(target) -> Option<ObjectTargetPolicy>` (36): entry symbol (`main` / `_main`), text/data/bss section names, text alignment (4 for Aarch64, 1 for x86-64) for five pairs; `(Aarch64, Coff)` -> None (58).
4. Keyed on `(architecture, object_format)` only.
6. Same job elsewhere: see A2 item 6.
9. Fail-open: none seen (None is a rejection; users `.expect` in names.rs:117-120,124-125 which panic for Aarch64+Coff, "no object policy is declared").

### B3. object_plan.rs (449 lines) - mutable lane
1. Purpose: data types for the arena-backed object plan and relocation plan.
2. Shape: `ObjectFileLayout` (17) with `insert_import` (42), `insert_normalized_import` (50); `ObjectPlan{target,layout}` (96); `ObjectContainerOutput` (135); `SectionPlan/SectionKind/SymbolPlan/SymbolKind/SymbolSection` (147-228); `NormalizedImportPlan` (183); `FunctionSymbolPlan` (189); `RelocationPlan` (231); `RelocationRecord` (284); `RelocationOrigin{Instruction,SemanticOperation,SemanticEdge,Materialization}` (312); `RelocationKind` (385); `invocation_relocation_origin` (393); `relocation_kind_tag` (414).
4. Decides: `invocation_relocation_origin` maps `InvocationSite::Operation` -> `SemanticOperation`, `InvokeNoNormal|ExternalCompletion` -> `SemanticEdge` (397-411). Tag table `relocation_kind_tag` 1..5 (414-422).
9. Fail-open: none seen (data only). `Default` impls create a `RelocationRecord` with `byte_width: 0` and an invalid handle (294-309).

### B4. names.rs (278 lines)
1. Purpose: symbol/section spellings and fail-closed symbol lookups on `ObjectPlan`; manifest envelope encoder.
2. Shape: `object_symbol_handle_by_foreign_locator` (16), `object_symbol_handle_by_import_locator` (26), `normalized_foreign_import_symbol_name` (56), `normalized_import_symbol_name` (63), `object_symbol_name` (79), `object_function_symbol` (87), `object_function_reference` (96), `entry_symbol_name` (116), `section_name` (123), `encode_manifest_envelope` (142, pub(crate)).
4. Decides: import lookup is keyed on the exact locator (`import.locator.as_ref() == locator`, 34), requires exactly one match, a valid handle, and the symbol to be `Import` with no section, offset 0, size 0 (35-53). Function lookup is keyed on `MachineFunctionIdentity` equality, unique match, symbol kind `Function` (103-113); `object_function_symbol` additionally requires `Section(Text)` and `size > 0` (92).
   - Import symbol name: `format!("__omega_foreign_import_{:016x}", locator.non_authoritative_compatibility_fingerprint())` (58-60); component form is hex of `identity_digest()` (70-74). Name is derived from a 64-bit "non-authoritative" fingerprint; lookup does not use the name.
5. Cases: none.
7. Counts (grepped, pattern list, `names.rs` 14 incl. tests; image-emission files 14; native-artifact foreign_calls.rs 2): public lookups have outside callers in image-emission/src/{entries,image,object,function_fragments}.rs and native-artifact.
9. Fail-open: `object_symbol_name` returns `""` for an invalid handle (79-85) - empty string rather than an error (open; callers not traced).

### B5. fragment_container.rs (271 lines)
1. Purpose: wraps a placed text section into a `RelocationFreeObjectPlan`, encodes the OMGTRO container, builds the OMGTOM manifest and a custody receipt; `validate_...` replays text-section validation, object reconstruction, decode, manifest and receipt.
2. Shape: `stage_optimized_relocation_free_object_container` (32), `validate_optimized_relocation_free_object_container` (53), `construct_manifest` (94), `StagedOptimizedRelocationFreeObjectContainer` (174), errors (250).
4. Decides: equality of reconstructed vs retained objects/manifests; container identity is the SHA-256 of bytes (66). Decoded object must equal the retained object (72).
9. Fail-open: none seen. Note: `external_entry_bridge`, `executable_image`, `installation`, `publication` are hard-coded `Unavailable` (99, 119-122).

### B6. relocation_free_object/mod.rs (1623 lines; tests from 1065)
1. Purpose: immutable `RelocationFreeObjectPlan`, its admission rules, and the OMGTRO v5 wire codec.
2. Shape: `ObjectLocalSymbolId` (40); symbol enums (61-81); `RelocationFreeObjectPlan` (118); errors (158, 195); `canonical_private_machine_symbol_name` (244) = `__omega_terminal_machine_{n}`; `canonical_normalized_foreign_import_symbol_name` (248); `validate_relocation_free_object` (266-473); `encode_relocation_free_object` (475); `decode_relocation_free_object` (492); `encode_plan_content` (519); decoders (585-1063).
4. Decides (validate, read): target pair via `object_target_policy` and pointer size/alignment == 8 (272-277); text section name and alignment equal the policy row (278-283); relocation requirement enum determines which rosters must be empty/non-empty and the record count (293-359); symbols must be dense, ordered, one per machine, named exactly `canonical_private_machine_symbol_name(machine)`, not `main`/`_main`, exactly one semantic entry with `SemanticEntryV1` role (365-421); imports sorted by `(boundary, ordinal)`, canonical names, each used by at least one foreign call (423-471).
5. Cases by shape: `symbol_policy` and `linkage` are single-variant but written as tag `1` (534, 543) and decode rejects any other (760, 777). Decode tables hand-written: image-address reference tags 0-4 (592-612), field encodings 1-2 (618-631), machine register `(family,index)` table (924-951), callback destinations 1-3, encodings 1-2. Foreign-call field width fixed at 4 for both kinds (259-264).
6. Same job elsewhere: image-address tag numbering is encoded in `machine_code::PlacedImageAddress::encode_identity` (called at 579, defined outside this unit; suspected separate tag table vs the decoder at 592-612). Register wire form: encoder uses `MachineRegister::wire_class_index()` (677), decoder hand-lists the inverse (929-949).
9. Fail-open: `validate_relocation_free_object` does not check unresolved image address entries other than field bounds (337-358): in `ProvenNone...` the image-address roster must be empty (298) and in `UnresolvedNormalizedForeignImportFieldsV1` too (309-311), so no gap seen there. Image addresses are not cross-checked against symbols/instruction bytes at the object level (bytes check is in text_placement and the from-text equality in text_section.rs:79).

### B7. relocation_free_object/publication.rs (368 lines)
1. Purpose: OMGTOM v1 manifest record, statistics, codec. Decode ends with identity recompute (239-241).
4. Decides: tags fixed at 1 for stage, symbol policy, and each of four "unavailable" fields (156-161, 182, 206-212).
9. Fail-open: none seen.

### B8. relocation_free_object/text_section.rs (339 lines)
1. Purpose: builds a `RelocationFreeObjectPlan` from a placed text section, deduplicating `(boundary, ordinal)` foreign imports into sorted import symbols after the function symbols (93-110), and replays by rebuilding and comparing (`validate_relocation_free_object_from_text` 72-84).
4. Decides: semantic entry symbol = function whose machine equals `text.semantic_entry` (46); target pair via policy (36).
9. Fail-open: none seen; missing semantic entry -> error (66).

---------------------------------------------------------------------------------------------------
## C. backend-plan (provider and callback plan records)

### C1. backend-plan/src/lib.rs (58 lines)
Purpose: crate root + `NativeProviderSources{declarations,family_realizations}` (45) and `SelectedFamilyRealizationSource` (51) with string identity fields. Header comments (1-21) describe these as carriers of "exact declaration custody".
7. Counts: `NativeProviderSources` not grepped.

### C2. callback_artifacts.rs (248 lines)
1. Purpose: records pairing produced Terminal callback thunk bodies with checked placements and joins them to the caller's program.
2. Shape: `NativeCallbackPlacement` (14), `PrivateCallbackDestinationApplication` (31), `BoundCallbackPrivateMaterialization` (58), `TerminalCallbackThunkArtifact::for_application` (80), `TerminalCallbackOccurrenceProposal` (126), `NativeCallbackThunkSettlement` (187), `validate_callback_thunk_joins` (198), `native_callback_thunks` (232).
4. Decides: thunk body is the unique machine whose declaration `same_identity_as` the application body (88-102); join checks artifact identity equals caller identity (205), `application.registrar == terminal_operation` and `MachineFunctionIdentity::callback_thunk(caller.identity(), placement_index) == callback_thunk_identity` (216-222).
7. Counts (grepped "native_callback_thunks|validate_callback_thunk_joins": 6 occurrences in 3 files: this file 2, lib.rs 2, pipeline/00_terminal-psi-to-abstract-operations/src/retained_terminal.rs 2). So one outside consumer file.
9. Fail-open: none seen.

### C3. direct_boundary_leaves.rs (163 lines)
1. Purpose: `DirectBoundaryLeaf` holds a satisfied service contract + its foreign or intrinsic execution projection; `validate_calling_custody` and `validate_bound_calling_custody` authenticate the calling attachments.
4. Decides: exactly one plan row and one schema method (30-35); route text equality across row/method/trait (36-50); original method contract equality via `projected.eq_in` (80-88).
9. Fail-open (read, 89-91): `let DirectBoundaryExecution::Foreign(Ok(binding)) = &self.execution else { return Ok(()); }` - an `Intrinsic(_)` or `Foreign(Err(_))` leaf skips every calling-roster/commitment check below it (92-128). Comment at field 15: "Preserves exact refusal until executable demand activates this row." - classification "stated" for Foreign(Err); for Intrinsic it is "open". Also line 96: if `expected` (the selected calling roster) is empty, the binding's applications are not compared to any roster (116 guard) - "open".
7. Counts (grepped "DirectBoundaryLeaf|validate_bound_calling_custody|validate_calling_custody": 22 occurrences in 8 files; outside users include provider-planning provider_rows.rs (3), selected-dispatch (2), pipeline/00 retained_terminal.rs (6) and retained_terminal_codec.rs (4)).

### C4. synchronous_invocations.rs (397 lines)
1. Purpose: whole-plan cycle check over synchronous `invokes` edges across selected provider plans.
2. Shape: `validate_declared_synchronous_invocation_cycles` (10), `SynchronousBoundaryTarget{Application,Selected}` (72), `matches` (85), `validate_selected_synchronous_invocation_cycles` (134), `exact_selected_synchronous_plans` (247), `synchronous_cycle_from` (302, 3-color DFS), `exact_row_for_schema_method` (325), `exact_authored_invocations` (370).
4. Decides: edges only to plans that match an invoked target; `Application{application:Some}` matches receiver applications by text and trait package (89-100); `Application{application:None}` matches trait name or any receiver trait name, with `package.is_none()` meaning any package (101-119); more than one match -> ambiguity error (206-212). Identity by exact string text.
9. Fail-open (read, 200-205): a target that matches no selected plan (`[] => {}`) adds no edge and no diagnostic - an invocation of a boundary not realized by any selected plan is not reported here (open, whether another stage catches it is not traced). Package `None` wildcard at 108 (open).

### C5. target_providers.rs (1361 lines; tests from 782)
1. Purpose: joins Build-selected target provider plans to canonical Terminal provider candidate rosters; closes checked specializations; validates synchronous calls; computes folded-constant dependencies; admits the roster.
2. Shape: `TerminalTargetProviderProposal` (13), `TerminalTargetProviderSelection` (23) with `from_plans` (161) and `admit` (173), `TerminalTargetProviderPlan` (30), `selected_checked_adapter_header` (36), `selected_checked_invocation_rows` (95), `admit_selected_checked_providers` (198), `admit_checked_roster` (214), `validate_synchronous_calls` (273), `matching_folded_constant_dependencies` (353), `checked_candidate_roster` (432), `close_checked_provider_specializations` (457), `require_selected_provider_declarations` (498), `selected_checked_candidates` (559), `selected_provider_candidate_rows` (600), `receiving_application_matches` (763).
4. Decides: candidates join on many string equalities: owner declaration text, package identity, provider type text, source requirement text, source candidate text (619-636); target membership filter (643-653). Header selection: `product_eligible`, package, `source_target` None-or-equal, callable text (66-77). Duplicate canonical realizations rejected by sorting `requirement_identity` and checking adjacent (657-681). `from_plans` always sets `assumed_bindings: Vec::new()` (168).
5. Cases: folded-constant correspondences: `(Some(candidate), CheckedAdapter)` or `(None, CompilerIntrinsic)` only (390-413); exactly one match required (420).
9. Fail-open: line 643-653: a provider candidate that belongs to no `target_machine_families` member passes the target filter (the `!family.members.any(..)` arm) - "open". Lines 698-722: `obligations.selecting_declaration.as_ref().is_none_or(..)` - an obligations row without a selecting declaration is accepted for any owner/binding (open). `with_assumed_bindings` rows are not validated here.
7. Counts: none computed beyond the pub list in lib.rs.

---------------------------------------------------------------------------------------------------
## D. effects (provider evidence)

### D1. effects/src/lib.rs (79 lines), providers/mod.rs (12 lines)
Purpose: crate root and re-exports. `provider_grants` is declared (38) outside this unit; `providers/mod.rs` declares `foreign_locator` (re-export from `target`) and `provider_plan` (12; module body outside unit). lib.rs:77-78 has `#[cfg(test)] mod source_guards;` (outside unit).

### D2. external_providers.rs (171 lines)
1. Purpose: joins checked external provider declarations in Terminal Psi to selected provider plans; reconciles numeric meanings with compiler intrinsics.
2. Shape: `RequirementPlanIdentity` (13) with `matches` (22), `matches_requirement_schema` (42), `selected_numeric_provider` (76), `matches_external_selection` (125), `reconcile_numeric_meaning` (153).
4. Decides: exact text and package-identity equalities (28-38, 132-150); doc comment (3-6) says "Display names serve only as consistency checks" but `method.name == self.method` and `method.requirement_owner == self.owner` (37-38) are string compares on names.
7. Counts (grepped "reconcile_numeric_meaning|selected_numeric_provider|matches_requirement_schema|matches_external_selection": 21 occurrences in 4 files: selected-dispatch numeric_provider.rs 1, provider-selection bodies.rs 4, this file 6, provider-admission compiler_builtins/retained.rs 10).
9. Fail-open (153-171): `reconcile_numeric_meaning((None, _, false))` returns Ok - no meaning and a non-numeric selection pass; (None, Some(numeric)) fails. Traced from the match only; "stated" by shape not comment.

### D3. provider_evidence.rs (1396 lines)
1. Purpose: `SelectedProviderPlanFacts` - the selected provider closure with specialization rows, external applications, family realization rows, installation reach resolutions; SHA-256 closure digest; recovery candidate validation.
2. Shape: `SelectedProviderPlanFacts` (89), `SelectedProviderPlanCandidate::validate` (1111), `from_selected_plans` (200), `with_toolchain_settled_plan` (265), `with_direct_boundary_plans` (332), `identity_digest` (402), `execution_scope` (454), `with_installation_reach_resolutions` (470), `with_specialized_requirement_rows` (550), `with_external_requirement_applications` (813), `with_family_realization_rows` (884), `validate_family_realization_rows` (921), `selected_requirement_row` (762), `resolve_installation_requirement` (1078); fingerprints (1206, 1215); `SelectedProviderClosureDigestEncoder` (1285).
4. Decides: uniqueness keyed on `report_fingerprint()` (64-bit FNV, 222-234) and `selection_slot_identity()` (235-242); zero fingerprint is reserved and rejected (223). Requirement mapping is by string text through `TerminalIdentityTable`; missing index -> `""` (`identity_text` 1199-1204).
5. Cases: external requirement applications must bind to exactly one `ProviderBinding::Import` row (859); specialization rows bind CheckedAdapter by machine text, Import/CompilerIntrinsic/Syscall by equality of identities (609-624), everything else `false` (623).
6. Same job elsewhere: `selected_requirement_identity`, `selected_row_matches_requirement`, `selected_requirement_row` each repeat the same three-way lookup family -> specialization -> external (725-797).
7. Dead/never-constructed (grepped "IsolatedProvider": 2 occurrences, provider_evidence.rs:1307 and executable_scopes.rs:28 only): `ExecutionScope::IsolatedProvider` is never constructed; `execution_scope()` always returns `CallerAddressSpace` (454-456). `encoder.len(0)` at 414 writes a constant zero count in the digest (a placeholder field). `with_direct_boundary_plans|with_toolchain_settled_plan|plan_by_exact_evidence|rekey_requirement_identity`: 13 occurrences in 10 files (grepped), outside users in provider-selection, selected-dispatch, provider-admission, package-compilation, pipeline/00 and backend-plan/target_providers.rs:1.
8. Stated limits: none.
9. Fail-open: `selected_row_matches_requirement` falls through to `requirement == selected_requirement` when no mapping row exists (759); `selected_requirement_identity` returns the input unchanged when unmapped (732-735) (stated by shape; traced no further). `assumed_supplier_identity` follows supplier chain and returns the last name even when it names nothing (160-162). `with_assumed_bindings` stores rows without validating them (143-146).

### D4. provider_evidence/authority.rs (1294 lines; tests from 1231)
1. Purpose: role-tagged terminal mechanism identities and their canonical byte encodings, authority dispositions, permission records, and the closure review receipt (v5 digest, big-endian).
2. Shape: `CompilerIntrinsicExecutionIdentity` (68), `checked_register` (153), `compiler_intrinsic_execution_identity_bytes` (243), `TerminalMechanismIdentity` (463), `terminal_mechanism_identity_bytes` (501), `TerminalAuthorityDisposition` (646), `TerminalAuthorityClosureLeaf::{new,program_root,validate}` (793-899), `TerminalAuthorityClosureReviewReceipt::from_reviewed_leaves` (963), `compare_closure_leaves` (1055), `terminal_authority_closure_review_identity` (1114).
4. Decides: leaf validation (835-899) keyed on mechanism-vs-origin pairing: `CheckedPhysical` mechanism iff `CheckedPhysical` origin (850-858); `CheckedIntrinsic` origin requires `CompilerIntrinsic` mechanism (859-867); `CheckedBoundary` origin requires `NormalizedForeign` mechanism with a nonzero Checked argument contract, nonzero context, and site not `InvokeNoNormal` (836-849). Permission check `permitted.first_missing_class(exercised)` (885-897).
5. Cases by shape: `checked_register` maps `RegisterOperation` to a builtin; `SystemWrite` of `Cr2`, `EsrEl1`, `FarEl1` -> `None` (207, 216). `compiler_intrinsic_execution_identity_bytes` hand-assigns tag bytes 0..16.
   Identity-byte collision (read + grepped): `StaticInterrupt(op)` sets `bytes[0]=16; bytes[1]=op.code()` (248-251) and `EmptyRunRelease` sets `bytes[0]=16` (267). `StaticInterruptOperation::Install.code()` is `0` (psi/foundation/language-core/src/lib.rs:361), so `StaticInterrupt(Install)` and `EmptyRunRelease` encode to the identical 8 bytes `[16,0,0,0,0,0,0,0]` (traced).
6. Same job elsewhere: `float_format_tag`, `arithmetic_domain_tag` here; numeric tags imported from language_semantics (600-605).
7. Counts: `compiler_intrinsic_execution_identity_bytes` 4 occurrences in 3 files (boundary-applications coverage.rs 1, effects lib.rs 1, this file 2).
9. Fail-open: in `validate`, when `permitted` is `None` the `exercised`/`permitted` consistency check is skipped (885), so a `SelectedRequirement` leaf with `exercised: None, permitted: None` is accepted (the unit test builds exactly this at 1245-1255). The closure-review receipt requires all leaves to agree on whether a permission policy exists (979-984). The later consumer `validate_physical_authority_closure` (executable_scopes.rs:131-141) turns `exercised == None` into a violation when physical exclusions are requested. Classification: traced across the two files.

### D5. provider_evidence/component_eras.rs (132 lines)
1. Purpose: three receipt structs and rejection enums for component era entry, quiescence, retirement; `recoverable_error!` macro builds error types holding the receipt back.
7. Counts (grepped "ComponentEraEntryReceipt|...EraEntryRejection": 13 occurrences in 2 files: component_eras.rs 12, effects lib.rs 1). The receipt structs have private fields, no constructor, and no `impl` other than the error macro; none is constructed anywhere in omega-rust (grepped; no occurrence outside this file). The error types are re-exported by lib.rs:63-65 but have no outside user (13 total). The `*Rejection` enums are `pub` inside a `pub(crate)` module and not re-exported, so are unnameable outside the crate.
9. Fail-open: none (no logic).

### D6. provider_evidence/executable_scopes.rs (820 lines)
1. Purpose: `BehaviorExclusions` (crash causes, services, physical authority classes) and `establish_behavior_exclusions`, a walk over pre-optimization Terminal behavior across selected entries and provider realizations.
2. Shape: `ExecutionScope` (26), `BehaviorExclusions` (62), `validate_physical_authority_closure` (122), `establish_behavior_exclusions` (318), `boundary_realization_coverage` (372), `inspect_machine` (493), `inspect_boundary` (629), `resolve_authored_behavior_exclusion` (792), `authored_behavior_exclusion_set_in` (811).
4. Decides: BFS over machines (350-367); `Crash` operation prohibited if cause excluded (554-563); `InstructionService` prohibited if service excluded (540-553); boundary calls prohibited by excluded service reach (716-731) and excluded crash causes only when `needs_contract` (751-762). `needs_contract` is false when every row is a matched `CheckedAdapter` realization (463-477); the realization bodies are then inspected instead (763-774).
5. Cases: unknown callee/boundary/service owner become evidence gaps (`EvidenceGapKind`, 240-251) -> verdict `InsufficientEvidence` (306-315).
9. Fail-open: `establish_behavior_exclusions` returns an empty (Satisfied) report when `exclusions.is_empty()` (326-328); `validate_physical_authority_closure` returns Ok when no physical classes are excluded (126-128). `authored_behavior_exclusion_set_in` drops (via `filter_map`) any authored Service exclusion whose identity is not found in `module.services` (803-807, 816-818) - the exclusion is silently not enforced (traced: only caller path compiler/src/terminal/behavior_exclusions.rs:52 then `if exclusions.is_empty()` returns Ok at :53-58; the diagnostics at :88 count `authored.len()` only after a violation). So an authored `Service` exclusion naming a service absent from the module yields no violation and, if it is the only exclusion, skips the whole check. "traced".
   Verdict handling at the caller requires `Satisfied` (behavior_exclusions.rs:75).

---------------------------------------------------------------------------------------------------
## Coverage
- Read in full: every range in the unit file: frame_application.rs 1-2571; frame_layout.rs 1-2527; frame_protocol.rs 1-428; function_realization.rs 1-1126; lib.rs (machine-emission) 1-82; native_bridge_encoding.rs 1-588; native_entry_bridge.rs 1-1983; native_foreign_bridge.rs 1-419; text_placement.rs 1-1799; tests/native_foreign_bridge.rs 1-229; fragment_container.rs 1-271; object-file lib.rs 1-115; names.rs 1-278; object_plan.rs 1-449; relocation_free_object/mod.rs 1-1623; publication.rs 1-368; text_section.rs 1-339; target_matrix.rs 1-109; callback_artifacts.rs 1-248; direct_boundary_leaves.rs 1-163; backend-plan lib.rs 1-58; synchronous_invocations.rs 1-397; target_providers.rs 1-1361; external_providers.rs 1-171; effects lib.rs 1-79; provider_evidence.rs 1-1396; authority.rs 1-1294; component_eras.rs 1-132; executable_scopes.rs 1-820; providers/mod.rs 1-12. Total 21,435 lines.
- Skimmed: none. Not read: none.
- Test modules inside files were read, but I recorded findings from them only where they state intent (e.g. frame_application tests, frame_protocol matrix tests).

## Five things a maintainer most needs to know about this unit
1. Every stage here is derive-then-replay: a `stage_*` function builds an artifact, a `validate_*` function independently re-derives it and compares (frame_protocol.rs:93-120, frame_layout.rs:415-454/835-858, frame_application.rs:1150, text_placement.rs:65-88, fragment_container.rs:53-81). Two derivations of the same geometry exist in the same file (frame_layout.rs `function_layout` 987 vs `validate_frame_layout_plan` 1260), so a rule change must be made in both.
2. The `(architecture, object_format)` matrix with `(Aarch64, Coff) -> None` is written out separately at frame_protocol.rs:122, frame_layout.rs:223 and :234, object-file/target_matrix.rs:36 and in two ISA crates; tests assert some pairs agree (frame_protocol.rs:395) but nothing ties object-file's matrix to the frame ones. All x86-64 formats, including Coff, use the System V frame-protocol encoder (frame_protocol.rs:152, native_*_bridge frame code).
3. Branch widening in frame application is closed-form only for x86-64 `NonZeroV1` conditionals and Aarch64 register-free conditionals; unconditional jumps and the two x86-64 less-than predicates have no widening path and fail if their size would change (frame_application.rs:408, 850, 955-968).
4. Three pieces are present but have no producer: `TargetFrameLayoutPolicy::CanonicalSavedReturnAddressFrameV1` (only tests), `ExecutionScope::IsolatedProvider` (never constructed), and the `ComponentEra*Receipt` types (no constructor anywhere). `TextPlacementInput` has a single variant so three "UnsupportedRelocationShape" guards cannot fire.
5. Several acceptance paths read as "accept when the thing being checked is absent": `DirectBoundaryLeaf` skips calling checks for non-`Foreign(Ok)` executions (direct_boundary_leaves.rs:89), the synchronous cycle check ignores invoked targets with no selected plan (synchronous_invocations.rs:201), `authored_behavior_exclusion_set_in` drops unresolved Service exclusions (executable_scopes.rs:818), and `EmptyRunRelease` shares its identity bytes with `StaticInterrupt(Install)` (authority.rs:249,267).

## What I am unsure about
- I did not read `fragment_emission`, `exit_contract`, `code_layout`, the `machine_code` types (`FunctionFragment`, `PlacedImageAddress::encode_identity`), `provider_plan.rs`, `provider_grants`, or any image-emission caller, so statements about who consumes these outputs rest on grep counts only.
- Whether the wildcard `package.is_none()` match (synchronous_invocations.rs:108) and the family-filter pass-through (target_providers.rs:643-653) can admit a wrong program was not traced.
- The "duplicate envelope" claim between function_realization.rs:761 and names.rs:142 is suspected (I did not read other manifest codecs).
- The `OMGOBJ` encoder named in object-file lib.rs:14-31 is outside this unit; I did not verify the version/layout claims there.
- I did not verify that `StaticInterruptOperation::code()` output is the only source of `bytes[1]` collisions (other `EmptyRunRelease` uses were counted, 55 occurrences of the name in 31 files, but not read).
