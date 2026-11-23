# Unit 029 record: area 12-native-representations

Checkout `C:/Development/Omega/.agents/worktree/worker-2/` at `4f3c255d`. Read-only. Paths are relative to `source-rust/subsystems/compiler/psi-to-native/representations/` (written `R/`). "read" = I read the lines; "grepped" = a Grep call (counts from the Grep tool's count mode, given in each place); "inferred" = deduced without reading the other site.

Unit contents: 103 files, 21,975 lines (sum of the `.tsv` line counts), in seven crates:
`machine-code/encoding-admission` (5,958), `machine-code/frame-admission` (1,205), `machine-code/scalar-encoding` (324), `machine-code` itself (8,431), `machine-code/stack-demand` (452), `operation-evidence` (+ `boundary-applications`) (1,067), `operation-operands` (907), `optimization-records` (3,631 in this unit; `record_encoding.rs`, `manifest/work_usage.rs`, `manifest/tests/pass.rs`, `manifest/tests/work_usage.rs` are named by `mod` lines but are not in this unit).

---

## A. `R/machine-code/encoding-admission/src/` (independent checker for stages 11 and 12)

### A1. `control_encoding.rs` (142)
1. Purpose: decodes the baseline, widened and shortened branch byte templates that stage 11 stored for one branch or jump row, and checks each equals what the ISA decoder says it should be.
2. Shape: `validate(target, kind, machine, physical, forms)` `:35`; macro `alternative!` `:13` builds an `EncodedBranchAlternative`.
3. In/out: reads `EncodedControlAlternatives` (from `machine_code`), the post-allocation instruction and physical register model; calls `isa_x86_64::validate_x86_64_selected_*_form` and `isa_aarch64::validate_aarch64_selected_*_form`; returns `Ok(())` or `ArtifactMismatch`/`X86_64(..)`/`Aarch64(..)`.
4. Decides on: target architecture and `SelectedInstructionKind` (`:43-44` widened present iff AArch64 and not jump; shortened present iff x86 and not jump); form position 0/1/2; final equality `*row != expected` `:137`.
5. Cases by shape: only `Jump`, `ConditionalBranchNonZero`, `ConditionalBranchU64LessThan`, `ConditionalBranchI64LessThan`; others return `ArtifactMismatch` `:82`, `:114`. Hard-coded field offsets: x86 short `Relative8` offset 1 reference 2 `:86`; x86 near jump 1/5, conditional 2/6 `:92-93`; AArch64 jump `Branch26` 0/0 `:105`; widened `Branch26` 4/4 `:130`; `Conditional19` 0/0 `:132`. `key.variant = 1` for shortened forms of the non-NonZero predicates `:53-55`.
6. Same job elsewhere: displacement-field geometry is also in `R/machine-code/src/branch_encoding.rs:175-182`; checks of the same templates happen in stage 12/13 (suspected, not read; grepped `EncodedBranchAlternative|EncodedControlIndex|next_wider` = 67 occurrences in 13 files, incl. `pipeline/13.../frame_application.rs` 18, `pipeline/12.../x86_branch_relaxation.rs` 13).
7. Unused: module-private (`mod control_encoding`, `lib.rs:9`); one caller `validation.rs:331`.
8. Stated limits: none.
9. Fail-open: none seen.

### A2. `exit_requirements.rs` (1,058)
1. Purpose: builds, from the post-allocation program and the encoding, the per-instruction "exit requirements" (returns, traps, process exit, non-return effects) that later stages compare against emitted bytes.
2. Shape: `prepare_exit_requirements` `:18`, `prepare_non_return` `:155`, `scalar_return_view` `:187`, `prepare_return_value` `:229`, `prepare_trap` `:300`, private `prepare_process_exit` `:360`, `balanced_temporary_stack` `:427`, `validate_non_return_effects` `:581`.
3. In/out: reads `PostAllocationMachineProgram`, `SelectedFormEncoding`, register model, `register_environment::TargetExitAbi`; produces `EncodedExitRequirements` (type in `R/machine-code/src/exit_requirements.rs`). Several fields are `Result`s so a failure for one row is carried, not returned (`ExitSourceRequirements.returned/trap/process_exit/non_return`, `R/machine-code/src/exit_requirements.rs:169-172`). Called by `validate_non_return_effects` for every instruction: `:105-113`.
4. Decides on: `SelectedInstructionKind` (match arms `:591-628`, `:633-1050`); source-spelling register view names: `"xmm0"`/`"d0"` `:212`, `"rip"`/`"pc"` `:307`, `"control_flags"` `:325`, `"rsp"` `:442,:484,:515,:564` (`view_named`, 7 hits in this crate: Grep count on `view_named(` in `machine-code/` = encoding-admission `exit_requirements.rs` 7, `frame_layout.rs` 3); also compares `PhysicalAddressOperation` variants by equality.
5. Cases by shape: `ReturnAggregate { fragment_count }` only `1..=2` `:237`; `ReturnScalar`, `ReturnUnit` `:264,:288`; other kinds `ReturnOperandMismatch` `:294`. Trap only `Crash` or x86-only `PrivilegedCrash` `:315-333`. Temporary-stack instruction families hard-coded to byte counts 8/16 (`:462-470`, `:497-505`, `:536-561`): interrupt check/commit, segment reload, task register activation, float rounding, `Rflags` read/write. `balanced_temporary_stack` returns `false` for any other architecture/kind `:437-439`, `:475`, `:560`.
6. Same job elsewhere: the memory-effect/address correspondence in `validate_non_return_effects` repeats the per-kind address table in `validation.rs::admits_address_operation` `:378-433` (read) and in `encoding_identity.rs:77-189`.
7. Unused: `prepare_*` callers (grepped `prepare_exit_requirements|prepare_non_return|prepare_return_value|scalar_return_view|prepare_trap` = 18 hits in 5 files: stage 11 `lib.rs` 1, stage 13 `exit_contract.rs` 5, `published-objects/src/lib.rs` 1, `encoding-admission/lib.rs` 1, this file 10).
8. Stated limits: none.
9. Fail-open: (a) `(_, MachineEncodedMemoryEffect::NoneV1, _) => !matches!(kind, CopyBytes|ZeroBytes|ScanByteSequence|LoadEncodedScalar|StoreEncodedScalar|CompareByteSequences|FloatResultPolicy|AtomicFence|AtomicAccess|SaveFloatingControl|RestoreFloatingControl)` `:1035-1048`: any other kind (e.g. `Load64`, `Store`, `AddressOffset`) whose effects carry `NoneV1` memory is accepted with any `address` value. Class: open. (b) `CopyBytes` `:783` and `ScanByteSequence` `:827` arms return `true` after matching only the operand index shape; no trap or operand-list check. Class: open. (c) `_ => MachineEncodedControlEffect::FallThroughV1` `:627`: unlisted kinds must carry fall-through; wrong effect fails, so this fails closed. (d) hard-coded `isa_x86_64::validated_x86_64_physical_register_model()` + `.expect(..)` `:440-444,:482-486,:513-517,:562-566` instead of the program's own model; reachable only after architecture checks `:437,:475`. Class: traced for the guards, open for the panic.

### A3. `fragment_origins.rs` (492)
1. Purpose: projects per-instruction source correspondence (calls, foreign calls, image addresses, control provenance) from the post-allocation program into `FragmentOrigins` before the instruction graph is dropped.
2. Shape: `prepare_fragment_origins` `:12`, `prepare_function_origin` `:28`, `prepare_fixup_origin` `:76`, `control_provenance` `:107`, `successor_provenance` `:252`, `check_control_provenance` `:267`, `check_successor_provenance` `:474`.
3. In/out: reads `SelectedFunction`/`SelectedBlock`/`SelectedTerminator`, `function.calls`, `function.normalized_foreign_calls`; produces `FragmentOrigins` with `control: Result<..>` and `import_free: Result<bool,..>` per row `:44-68`.
4. Decides on: instruction id equality with terminal id (`terminal.id == instruction.id`, e.g. `:185,:191`); call owner variants `CallSiteOwner::InvokeNoNormal`, `Installation{Work}` `:129-161`; `ordinal` index into `normalized_foreign_calls` `:56,:168`.
5. Cases by shape: terminator kinds `EliminateEmpty, OperationTrap, Crash, Jump, ConditionalBranch (3 predicates), HostedExitProcess, Return` `:181-248`; every other terminator or non-terminal instruction gets `None` `:248`. Calls handled through `selected_call_pattern!` `:112`.
6. Same job elsewhere: `control_provenance` `:112-166` and `check_control_provenance` `:275-328` repeat the same call-row lookup, `InvokeNoNormal` terminator match and owner test almost line for line; the checker is run on the producer's own result in the same function `:44-48`, so it is a self-check. Consumers: `pipeline/13.../fragment_emission.rs` (7 hits), `published-objects` 1 (grepped `prepare_fragment_origins|prepare_function_origin|prepare_fixup_origin|check_successor_provenance` = 18 hits in 4 files).
7. Unused: n/a (each fn has a caller per the grep).
8. Stated limits: header `:1` "Source correspondence is checked before the instruction graph is retired."
9. Fail-open: `_ => FunctionFragmentControlProvenance::None` `:248` for any terminator not listed. Class: open (a new terminator variant would be recorded as plain fall-through provenance; `check_control_provenance` matches `block.terminator` exhaustively `:345-456` so it would fail to compile first; inferred).

### A4. `frame_address.rs` (1,330; tests from `:596`, 10 `#[test]`)
1. Purpose: checks frame geometry roots and resolves/validates each instruction's symbolic frame or pointer address against the placed frame layout.
2. Shape: `validate_frame_root` `:63`, `resolve` `:333`, `validate_address` `:432`; helpers `unique` `:150`, `slot_region` `:159`, `function_geometry` `:196`, `frame_address_alignment` `:204`, `committed_extent` `:245`, `frame_displacement` `:252`, `hosted_window_displacement` `:279`, `check_address_loan` `:322`, `incoming_frame_base_bytes` `:578`.
3. In/out: reads `PostAllocationMachinePlan`/`PostAllocationMachineFunction`, `TargetFrameLayoutPlan`; `resolve` returns `ResolvedPhysicalAddress{symbolic, displacement, frame_alignment_bytes}`.
4. Decides on identity: `LocalStorageSlotId`, `MachineId`, `OutgoingArgumentSlotId` (`unique` by id `:150-157`); numeric geometry (offset, size, alignment, i32 range). Loan test: a non-spill local `FrameAddress` must be in `stable_address_loans`; a spill must not `:326-328`.
5. Cases by shape: hosted windows have fixed width/alignment: `SaveFloatingControl`/`RestoreFloatingControl` 8/8, `HostedWriteByteI32` 1/1, `HostedReadByte` 8/4 `:286-289`; slot classes `Boundary` vs `Structural` `:264-277`; `Store` byte sizes `1|2|4|8` `:383,:499`; over-aligned locals only if ABI alignment is 16 and no red zone `:235-241`; `ActivationBase` and `Incoming` only with `byte_offset: 0` `:343-357,:451-468`.
6. Same job elsewhere: local-slot placement rules (alignment, no overlap, extent) appear here `:93-125` and again in `frame-admission/src/frame_layout.rs:907-935` (read). Stage 11 uses the same `resolve` as the checker (grepped `frame_address::resolve|validate_address|validate_frame_root` hits `pipeline/11.../lib.rs:176,214`, so producer and checker share the equations; `lib.rs:3` of this crate says so).
7. Unused: `resolve` has 1 non-test external caller (stage 11 `lib.rs:214`); `validate_frame_root` 2 callers (`lib.rs:176` of stage 11 and `validation.rs:83`).
8. Stated limits: header comment `:1-29` describes the `:FrameAddressEquations` replay; `validate_address` checks "without invoking `resolve`" `:27`.
9. Fail-open: `frame_address_alignment` returns `Ok(1)` for any address that is not `FrameAddress`/`Store64` or a slot that is not a local or `ValueCopy` outgoing `:209-211,:230`; this is the no-alignment case, not an accept of a wrong program. `validate_frame_root` with `frame == None` accepts only when no function has outgoing arguments or locals `:67-75`. Otherwise none seen.

### A5. `frame_protocol.rs` (255)
1. Purpose: verifies the frame prologue/epilogue byte arena against bytes regenerated from the frame layout.
2. Shape: `validate_target_frame_protocol_encoding` `:19`, `encode_frame_function` `:35`, `validate_bytes` `:91`, `validate_span` `:134`; re-exports from `frame-admission` and `machine_code` `:4-14`.
3. In/out: reads `ValidatedTargetFrameLayout`, `ValidatedTargetRegisterEnvironment`, `TargetFrameProtocolEncodingPlan`; returns the protocol identity digest.
4. Decides on: target `(arch, object_format)` via `frame_protocol_codec` and `frame_unwind_policy` `:108-112`; policy `CanonicalFixedFrameV1` only `:96`; spans must be contiguous from cursor 0 `:143-147`.
5. Cases by shape: codec X86_64 or Aarch64 only (`encode_frame_function` `:41-87`).
6. Same job elsewhere: stage 9 `frame_protocol.rs` (producer, 2 hits for the matrix functions). Bytes come from `isa_x86_64::encode_system_v_amd64_frame_protocol` / `isa_aarch64::encode_aapcs64_frame_protocol`, the same functions the producer uses (`frame-admission/src/frame_protocol.rs:41-42` says "retains the existing shared ISA recipes").
7. Unused: single external caller (grep count above: stage 11 `frame_protocol.rs` 1).
8. Stated limits: "retains the existing shared ISA recipes" (in `frame-admission/frame_protocol.rs:41`); tests assert codec and unwind matrices declare the same pairs `:222-254`.
9. Fail-open: none seen.

### A6. `layout/geometry.rs` (360)
1. Purpose: computes block and instruction byte offsets of already-encoded rows, including AArch64 conditional-branch widening, and selects the function layout policy.
2. Shape: `select` `:32`, `derive_plan` `:69` (fixpoint loop `:75-112`), `derive_once` `:115`, `widening_candidate` `:148`, `instruction_size` `:177`, `branch_size` `:259` (x86 6, AArch64 4), `conditional_branch_descriptor` `:270`, `declared_size_matches` `:326`, `is_admitted_canonical_shape` `:339`, `is_single_entry` `:345`; `LayoutPlan` `:24` holds three `BTreeMap`s and a `BTreeSet` keyed by `SelectedBlockId`/`SelectedInstructionId` `:25-29`.
3. In/out: reads `EncodedLayoutFunction`/`EncodedLayoutBlock`, pre-layout `SelectedFormEncodingRow`s; returns `LayoutPlan`.
4. Decides on: machine disposition (`Aarch64Elided*`, `Aarch64Fused*`, `RetainedV1`) `:183-212`; encoding state `:213-256`; whether displacement lies in `isa_aarch64::AARCH64_CONDITIONAL_BRANCH_BYTE_REACH` `:104`.
5. Cases by shape: policy selection `:33-66`: all functions single-entry (one block ending in `Return`) → `SingleEntryBlockV1`; none single-entry and all first blocks end in `ConditionalBranch` → `EntryThenZeroFallthroughThenNonzeroV1`; first blocks end in `U64`/`I64LessThan` → `EntryThenNotLessFallthroughThenLessV1`; all functions have a non-empty block list and an entry block → `PerFunctionCanonicalShapeV1` `:60`; else `UnsupportedFunctionShape(functions[single_entry_count].machine)` `:63-65`. Widening applies only to `RetainedV1` + `DeferredControl{RequiresResolvedBranchLayout}` conditional rows `:148-163`.
6. Same job elsewhere: `derive_order` here is in `layout/validation.rs:297`; the producer has its own `derive_order` at `pipeline/12.../ordinary.rs:34` (grepped), as `:1-3` says ("producer and checker retain their separate block-order derivations").
7. Unused: `branch_size` and `byte_displacement` used by `validation.rs:18-20,:596`.
8. Stated limits: `:1-3` header.
9. Fail-open: `is_admitted_canonical_shape` accepts any function with at least one block whose entry id exists `:339-343`. Class: open (policy name `PerFunctionCanonicalShapeV1` is then validated by `derive_order` in `validation.rs:297`, so shape errors fail there; traced). The error in `select` names `functions[single_entry_count]`, the function at that position, not necessarily the non-conforming one (inferred).

### A7. `layout/mod.rs` (160)
1. Purpose: error enum and the three entry points for validating a resolved layout.
2. Shape: `OptimizedResolvedSelectedFormLayoutError` `:18`; `check_control_alternatives` `:51`; `validate_optimized_resolved_selected_form_layout` `:88`; `validate_current_resolved_layout` `:119`; `validate_baseline_resolved_layout` `:131`; `validate_layout_forms` `:137`; `ResolvedLayoutValidation` `:81`.
3. In/out: reads `ResolvedMachineProgram`, `SelectedFormEncoding`; returns `Ok(())`.
4. Decides on: whether `X86RelaxConditionalBranchesToRel8V1` is in `program.context.publication.selections()` `:122-126` (selection identity); count equality of rows `:56-77`.
5. Cases by shape: `allow_x86_short` with a non-x86 target is `RootMismatch` `:150-152`.
6. Same job elsewhere: stage 12 `settled.rs` and `relaxation.rs` (grep of `ResolvedMachineProgram|OptimizedMachineLayout|FramedMachineProgram` 44 hits in 12 files) consume these.
7. Unused: 3 pub validators; `validate_baseline_resolved_layout` has callers per grep of `validate_baseline_resolved_layout|validate_current_resolved_layout|...` (35 hits across 12 files for the whole set).
8. Stated limits: none.
9. Fail-open: none seen.

### A8. `layout/validation.rs` (697)
1. Purpose: replays a function-relative layout: block order, byte spans (including AArch64 widening and x86 short forms), row bytes, fixups and decoded branch evidence.
2. Shape: `validate` `:30`, `validate_roots` `:70`, `validate_identity` `:90`, `validate_ordinary` `:106`, `validate_function` `:146`, `current_x86_spans` `:243`, `collect_pre_layout_rows` `:276`, `derive_order` `:297`, `validate_row` `:360`, `validate_branch` `:489`, `checked_delta` `:695`.
3. In/out: reads `EncodedLayoutRequirements`, `ValidatedPhysicalRegisterModel`, `SelectedFormEncoding`, `ResolvedMachineLayout`.
4. Decides on identity (`SelectedInstructionId`, `SelectedBlockId`, `MachineId` in `BTreeMap`s `:164-168,:280,:302`); on byte equality with pre-layout bytes `:404`; on `machine_disposition`/`state` pair `:402-485`; `short_branch` is inferred from `candidate.bytes.len() == 2` `:376`.
5. Cases by shape: row states handled: `Encoded`, `UnresolvedInternalMachineCall`, `UnresolvedNormalizedForeignCall`, `UnresolvedImageAddress`, `DeferredControl{RequiresResolvedBranchLayout}`; everything else including non-`RetainedV1` dispositions `_ => ArtifactMismatch` `:484`. x86 jump end `offset+5` `:524`; conditional size 2 (short) / 8 (widened) / arch default `:591-597`.
6. Same job elsewhere: `derive_plan` in `geometry.rs:69` (shared); `validate_function` here and `validate_row` `validation.rs` (stage 11 checker, `A11`).
7. Unused: `derive_order`, `validate_function`, `validate_row` re-exported at `layout/mod.rs:15`; callers in stage 12 `validation_tests.rs` (3) and `lib.rs` (1) per grep.
8. Stated limits: block comment `:2-4`; `// A relaxed x86 layout admits two exact conditional widths ... without retaining or reconstructing the earlier near-branch layout` `:239-242`.
9. Fail-open: with `allow_x86_short` on x86, spans come from the candidate's own row lengths `:264-267` and are not compared with an independent computation; each row is then decoded and the fall-through offset must equal `instruction_offset + branch_size` `:601`. Class: traced (a wrong width fails the decode at `:612-641` or the fall-through test).

### A9. `layout_requirements.rs` (128)
1. Purpose: builds `EncodedLayoutRequirements` (control topology + alternative declarations) from the post-allocation program, so layout can run without source operands.
2. Shape: `prepare_layout_requirements` `:5`, `prepare_function_control` `:25`, `prepare_block_control` `:34`, `prepare_row_control` `:83`, `prepare_function_alternatives` `:102`, `prepare_alternative` `:117`.
3. In/out: reads `PostAllocationMachineProgram`; produces `EncodedLayoutRequirements`.
4. Decides on `SelectedTerminator` variant `:42-76` and `SelectedInstructionKind` `:85-99`.
5. Cases by shape: terminators `Return`, `Jump`, `ConditionalBranch`, two `LessThan` kinds get their own variant; every other terminator becomes `Stop` `:75`; every other instruction kind becomes `Op::Ordinary` `:98` (kinds listed: `CompareI64Zero, CopyI64, three conditional branches, calls, NormalizedForeignCall, ImageAddress`).
6. Same job elsewhere: the type definitions are in `R/machine-code/src/layout_requirements.rs` (A/B below).
7. Unused: called at `lib.rs:101`, `layout/mod.rs:103`, `exit_requirements.rs:118` (`prepare_alternative`).
8. Stated limits: none.
9. Fail-open: `_ => Stop` `:75` and `_ => Ordinary` `:98` classify unlisted terminators/kinds without checking. Class: open (inference: layout then places them as ordinary, non-branch rows).

### A10. `lib.rs` (286)
1. Purpose: crate root; error enum, entry validators for the layout-independent encoding, row iteration, counters, integrity check.
2. Shape: `OptimizedSelectedFormEncodingError` `:42`; `SelectedFormEncodingValidation` `:78`; `validate_optimized_layout_independent_selected_form_encoding` `:85`; `validate_current_selected_form_encoding` `:117`; `normalized_foreign_call_record` `:127`; `for_each_row` `:149`; `counts_of` `:205`; `validate_encoded_integrity` `:246`; `program_context` `:265`.
3. In/out: reads the original post-allocation program (`PhysicalProgramReplay`) and the artifact; recomputes `EncodedProgramContext`, `EncodedLayoutRequirements`, `EncodedExitRequirements` and compares to the artifact `:96-108`.
4. Decides on equality of identities and recomputed values (`:96-111`, `:251-261`); foreign-call record by `ordinal` index plus instruction id, boundary id, native target and valid entry mechanism `:133-145`.
5. Cases by shape: `counts_of` maps each `SelectedFormEncodingState` to a counter `:210-236`.
6. Same job elsewhere: `validate_encoded_integrity` is used from `layout/mod.rs:142`.
7. Unused: `counts_of` called in `validation.rs:152` and by stage 11 (not counted).
8. Stated limits: `:1-6`: "Frame equations and contract projections are shared with the producer"; "call template comparison retains its existing shared-recipe limitation".
9. Fail-open: none seen.

### A11. `validation.rs` (1,050; one test `:1022`)
1. Purpose: independent replay of `SelectedFormEncoding`: per row, decode bytes with the ISA validators and compare footprint, size, fixups and identity.
2. Shape: `validate` `:76`, `validate_ordinary` `:111`, `validate_aggregate` `:151`, `validate_row` `:162`, `admits_address_operation` `:378`, `validate_baseline` `:435`, `admit_decoded_footprint` `:533`, `validate_size` `:572`, `validate_scalar_call` `:625`, `validate_normalized_foreign_call` `:743`, `x86_64_callback_replay` `:922`, `aarch64_callback_replay` `:966`.
3. In/out: reads machine program, frame, encoding rows and the encoded row store; no output but `Ok`.
4. Decides on `SelectedInstructionKind` (match `:178-375`); for foreign calls on binding variant (`table_dispatch()`, `system_call()`, else import fixup) `:759,:790,:828`; dynamic scalar calls on `kind.call_target().indirect_code_offset().is_some()` `:634-636`; fixup `kind`/`state`/`addend == 0` `:674,:834`.
5. Cases by shape: memory/frame/hosted kinds listed in one or-pattern `:179-192`; separate arm for calls `:296`, foreign calls `:302`, control `:317-338`, `ImageAddress` `:340-373` (re-tagged to `Encoded` and validated as baseline); `kind => validate_baseline` `:374` for everything else. AArch64 callbacks must be `Register` or `RecordField`, `OutgoingStack` is an error `:996-998`.
6. Same job elsewhere: stage 11 producer builds the same rows (stage 11 `lib.rs`); address-kind table `admits_address_operation` is a second copy of the kind→address table in `encoding_identity.rs:81-189` and `exit_requirements.rs:633-1033` (read).
7. Unused: `validate_row`, `validate_size` re-exported `lib.rs:39`.
8. Stated limits: header comment `:1-25`.
9. Fail-open: `kind => validate_baseline(..)` `:374`: any kind not named earlier is accepted if the ISA decoder for that kind accepts the bytes. Class: traced (the ISA validators are the independent decode).

---

## B. `R/machine-code/frame-admission/src/`

### B1. `frame_layout.rs` (1,156)
1. Purpose: checks callee-save storage by keyed reconstruction and frame geometry by equations over explicit demands; also holds per-target stack/unwind matrices.
2. Shape: `ValidatedNonAuthoritativeCalleeSaveStorage` `:115`; `ValidatedTargetFrameLayout` `:163`; `FrameContinuationCustody` `:186`; `stack_commit_granule_bytes` `:225`; `frame_unwind_policy` `:236`; `call_site_stack_contract` `:274`; `body_writes_link_register` `:300`; `contains_call` `:325`; `stable_address_loans` `:338`; `unwind_restores` `:355`; `committed_extent` `:385`; `validate_non_authoritative_callee_save_storage` `:401`; `admit_storage_plan` `:442`; `prepare_frame_demands` `:722`; `validate_target_frame_demands` `:752`; `validate_frame_layout_plan` `:815`; `minimal_aligned_extent` `:1137`; `save_region_is_disjoint` `:1144`; `MAX_FRAME_BYTES = i32::MAX` `:41`.
3. In/out: reads `MachineFrameDemands`, `AllocatedCalleeSavedRequirements`, storage plan, environment, candidate `TargetFrameLayoutPlan`; returns sealed `Validated*` with receipts.
4. Decides on: `(Architecture, ObjectFormat)` pair in `match` tables `:226-233,:237-271,:282-293`; MachineId order; family test for `contains_call`: `CallScalar|InvokeNoNormal|CallUnit|CallAggregate|NormalizedForeignCall` `:327-334`; view names `"rsp"`, `"sp"`, `"x30"` `:241,:257,:265`. `BTreeMap` used for unit→group `:503` and modified units `:515`.
5. Cases by shape: shadow area 32 bytes only for `MicrosoftX64` with calls `:865-869`; red zone only `SystemVAMD64`, no calls, no callee-save slots `:980-986`; stack probe touches computed from commit granule `:988-1003`; AArch64 `Coff` returns `None` from the matrices `:232,:270`.
6. Same job elsewhere: local-slot placement also at `encoding-admission/src/frame_address.rs:93-125`; stage 9 `frame_layout.rs` produces plans (23 hits for `minimal_aligned_extent|save_region_is_disjoint|call_site_stack_contract|stack_commit_granule_bytes|frame_unwind_policy` there; total 48 in 6 files; 15 in this file).
7. Unused: n/a (each fn has callers per the grep).
8. Stated limits: header `:1-3`.
9. Fail-open: `contains_call` `:325-336` uses a fixed family list; an instruction family that calls but is not listed is treated as non-calling, so shadow space and call alignment are not required. Class: open.

### B2. `frame_protocol.rs` (42)
Purpose: target→codec matrix and error enum. `frame_protocol_codec` `:30-39` (x86 Elf/Coff/MachO → X86_64; AArch64 Elf/MachO → Aarch64; AArch64 Coff → `None`). Comment `:41-42` states the replay "retains the existing shared ISA recipes". Same matrix shape as `frame_unwind_policy` (`B1:236`), checked equal by a test in `A5:222-254`. Fail-open: none seen.

### B3. `lib.rs` (7)
Declares `frame_layout`, `frame_protocol` and `count_where` `:3-7`. One caller of `count_where` (this crate's `frame_layout.rs:686,:788,:789,:805`).

---

## C. `R/machine-code/scalar-encoding/src/`

### C1. `lib.rs` (161)
1. Purpose: target-neutral step program for variable-width encoded scalar loads and stores that both ISA encoders lower.
2. Shape: `Step` enum, 16 variants `:33-51`; `plan(load, registers, finish)` `:53`; test-support `replay::execute` `:96`.
3. In/out: `plan` returns `Vec<Step>`; `execute` mutates a 32-register state and byte buffer and returns accessed addresses.
4. Decides on: codec word layout: width = `codec & 255`, signed = bit `256` (`replay` `:118,:142`).
5. Cases by shape: branch targets are literal vector indices (`TestSigned(codec, 8)`, `ZeroBranch(cursor, 15)`, `Branch(9)` for load; `LessBranch(3)` for store) `:63-71,:84`; replay stops after 128 steps with a panic `:106,:159`.
6. Same job elsewhere: consumers `isa-aarch64` and `isa-x86_64` (grepped `scalar_encoding::|scalar-encoding|scalar_encoding =` = 4 hits, only in those two `Cargo.toml` files).
7. Unused: `harness` and `replay` are `test-support` only `:89-93`.
8. Stated limits: header explains why steps are shared `:15-22`.
9. Fail-open: none seen.

### C2. `harness.rs` (163)
Purpose: shared test sweeps `exact_footprint_and_extension` `:54` and `rejects_every_single_bit_mutation` `:112`. Codec words: widths 1..=8 plain and with bit 256, plus `516, 520, 1025, 2056` `:36-40`. Test only. Fail-open: none seen.

---

## D. `R/machine-code/src/` (data forms, identities, codecs; no producer logic except where noted)

### D1. `boundary.rs` (252)
Purpose: records of boundary execution role, completion custody, port effects and settlements. `BoundaryExecutionRecord` `:34`, `ProviderExecutionRecord::new` `:89` (refuses any zero identity `:96-104`), `derive_completion_provider_custody` `:56`, `PortEffectRecord` `:159`, `BoundarySettlementRecord` `:213`, `BoundaryByteSequenceArgumentRecord` `:232`. Decides: for a non-provider execution, sources and receipts must both be empty `:62`; for a provider, each receipt takes the first source whose `claim()` equals `receipt.claim` `:67-69`, else the whole result is `None`. Extra sources are not rejected (inferred from `:64-76`). `From<ProviderExecutionBinding>` `:115-126` copies without the zero check that `new` makes. Callers: grepped `derive_completion_provider_custody|ProviderExecutionRecord::new(` = 14 hits in 7 files. Fail-open: unmatched sources are ignored; class: open.

### D2. `boundary_settlements.rs` (42)
`PhysicalBoundarySettlements::function` `:29`: unique machine row else an error string `:36,:38`; each function holds `rows: Result<Vec<..>, &'static str>` `:17`. Stored error as string (source spelling). Users: grepped `PhysicalBoundarySettlements|PhysicalCallOrigins|PhysicalForeignCallPlans|PhysicalInternalCallPlans|PhysicalCpuOrigins|PhysicalFaultOrigins|ImageAddressOrigins` = 51 hits in 21 files (stage 9 has files of the same names, 2-4 hits each; these are consumers, the structs are defined only here: grepped `struct ...` shows one definition each in this crate).

### D3. `branch_encoding.rs` (416, tests `:291`)
Purpose: encoded control alternatives and their displacement fields; layout chooses a supplied alternative and patches it. `BranchDisplacementField` `:8`, `EncodedBranchAlternative` `:16`, `EncodedControlAlternatives` `:27` (`next_wider` `:35`, `matching` `:53`), `EncodedControlIndex` `:70` (sorted `Vec` + `binary_search_by_key` `:158`), `read_target` `:189`, `field_value` `:239`, `reaches` `:256`, `patch` `:265`; geometry table `(width, shift, bits, scale)` `:175-182`. Decides: `next_wider` uses `std::ptr::eq` identity `:42,:45`; `read_target` accepts only all-zero templates for x86 `:212,:215`. Fail-open: none seen. Same job elsewhere: control checker `A1`.

### D4. `call_origins.rs` (48)
Data: `PhysicalCallOrigins` `:8`, `PhysicalInternalCallOrigin` `:32` (comment `:36-37`: "Absence is not a call: a consumer demanding this origin must require one exact instruction"). `function` lookup unique `:16-20`.

### D5. `calls.rs` (518)
Data records for calls, in six inline modules (`arguments :35`, `callbacks :178`, `fixups :238`, `foreign :309`, `internal :345`, `results :460`). Notable: `internal_arg_source_triple_pattern!` `:169` (`#[macro_export]`; grepped = 1 hit, its own definition: no user), `unit_call_arguments` `:370`, `InternalUnitCallSource::{ProgramEntry, Authored, InstalledProvider}` `:440`. Cases by shape: `InternalUnitScalarArgumentSourceRecord` 7 variants `:95-131`. `InstalledProvider` uses `operation_evidence::completion_claims::CompletionClaimSource` `:454`, while `boundary.rs:15` imports it through `target_vocabulary` (re-export of the same type: `target-vocabulary/src/lib.rs:20`). `encode_callback_destination_identity` `:211` is `pub(crate)`.

### D6. `code_layout.rs` (25), `companion_code.rs` (14), `cpu_origins.rs` (44), `fault_origins.rs` (30), `foreign_call_plans.rs` (94), `fragment_origins.rs` (63), `frame_demands.rs` (22), `function_metadata.rs` (18), `hosted_entry.rs` (45), `image_origins.rs` (57), `internal_call_plans.rs` (102), `placed_body.rs` (80), `program_context.rs` (65): plain data records (one entry for the group).
Notes with citations: `NativeCodeLayout` `code_layout.rs:12`; `PhysicalInstructionFaultOrigin.description: String` `fault_origins.rs:22` and `ModelSpecificRegisterSource.selector: Result<Option<u64>, String>` `cpu_origins.rs:35` carry free text; `ImageAddressOrigins::require` `image_origins.rs:38` looks the key up by `binary_search_by_key` over `(machine, block, instruction)` and compares `operation` and `reference` `:43-51` (requires `rows` sorted; sort is not checked here, inferred); `ImageAddressOrigins::indexes` `:29` is a fixed 3-variant match; `InternalCallPlanError::{Mismatch(&str), Unsupported(&str), Overflow}` `internal_call_plans.rs:15`; `PhysicalInternalCallPlans::function` unique-or-error `:34-48`; `EncodedProgramContext` `program_context.rs:5` with `BoundEncodingSubject::from_encoded` `:42` (compares `roots.psi/entry/work_schedule/pre_physical` `:51-55`); `PlacedInstructionEffects::from_footprint` `placed_body.rs:40` stores `Option<Arc<..>>` of a footprint (a bad handle yields `None`, `share_footprint` `encoding.rs:185`); `EncodedHostedEntry` `hosted_entry.rs:38` with `HostedEntryMode` 3 variants `:5` and `EntryFixupKind` 4 `:22`. `ForeignCallPlanError` has `reason: &'static str` fields `foreign_call_plans.rs:70-94`.

### D7. `control_flow.rs` (114)
Data for scalar control-flow evidence: `ScalarControlFlowEvidence` 5 variants `:14-35` (`Linear`, `Acyclic`, `LinearWithDivisionBranches`, `ConditionalTree`, `BooleanSharedConvergence`). Header `:1-9` says object replay must check stack depth and division diamonds. Users: grepped combined record names (`UnitAffineCleanupRecord|ScalarControlFlowEvidence|...`) = 183 hits in 71 files.

### D8. `encoding.rs` (394, tests `:343`)
Purpose: `SelectedFormEncoding` `:34`, `SelectedFormEncodingRow` `:246`, `SelectedFormEncodingState` `:213` (5 variants), `SelectedFormMachineDisposition` `:231` (4), `SelectedFormEncodedRowStore` `:117` (one byte `Vec<u8>` plus `SharedFootprints`), `SharedFootprints` `:281` (`rows: Vec<Arc<..>>` and `interned: HashMap<[u8;32], Vec<Weak<..>>>`, digest-keyed `:283`).
Decides: `bytes(span)` returns `&[]` if the span is out of range or overflows `:152-163`; `footprint(handle)` returns `None` for handle 0 or out of range `:178-183,:207-209`; interning compares full footprints inside a digest bucket `:305-314`. `bytes_mut` returns `&mut []` on a bad span `:165-176`.
Fail-open: a stale or out-of-range span reads as empty bytes. `encode_row_contents` hashes nothing for a missing footprint `encoding_identity.rs:244-246`. Class: open (callers `validation.rs:encoded_row` use `encoded.encoded(row).ok_or(..)` `:530`, which returns `None` for a missing footprint but `Some(&[])` bytes for a bad span; inferred).
Stated limit: `:12-18` boxing of sparse payloads.

### D9. `encoding_identity.rs` (717)
Canonical SHA-256 identity for `SelectedFormEncoding` (schema `omega.terminal.layout-independent-selected-form-encoding.v31` `:27`) and footprint buckets `:250`. `encoding_identity` `:29`, per-row `encode_encoding_row` `:70` (address arm tags 1..16 `:82-189`), `encode_normalized_foreign_fixup` `:287`, `encode_internal_fixup` `:336`, `encode_counts` `:355` (image-address counter hashed only when non-zero `:369-372`). Cases by shape: a fixed tag table per `PhysicalAddressOperation` variant. Tests pin a 303-byte canonical preimage and digest `:585-629`. Fail-open: see D8.

### D10. `exit_requirements.rs` (190)
Data for exit requirements (see A2). `WholeFunctionExitContractError` 24 variants `:12-38` (name differs from `WholeFunctionExitContract`, in `functions.rs`). `ExitInstructionEncoding::from_row` `:96` maps encoding state/disposition. `EncodedExitRequirements::function` linear `find` `:140`. `ExitSourceRequirements` holds four `Result` fields `:169-172`.

### D11. `fragments.rs` (354), `fragments/identity.rs` (422), `fragments/publication.rs` (371)
Purpose: `FunctionFragmentEmissionPlan` `fragments.rs:41` (functions → blocks → instruction spans with branch evidence, three fixup kinds, provenance, control provenance); identity (schema `omega.terminal.function-fragment-emission.v25` `identity.rs:29`); wire manifest `OMGFFE\0\0` v15 `publication.rs:21-22`, decode checks magic, version, stage tags 1|2, vocabulary marker, work schedule, six "unavailable" bytes equal 1, trailing bytes, identity recompute `:111-203`.
Observations (read): `encode_control` uses tag `10` for both `EliminateEmpty` `identity.rs:225` and `InternalCall` `:246` (payloads differ in shape); image-address fixup is hashed only when present with no presence byte `:75-80`. `publication.rs:355-370` test states the codec "checks integrity not the truth of claimed statistics": `statistics` and `stage` are retained claims, not re-derived (`:5-6` header says so).
Cases by shape: manifest `stage` has two values; `unavailable` fields exactly one value.

### D12. `functions.rs` (341)
Whole-function exit contract: `WholeFunctionExitContract` `:161`, `WholeFunctionExitPolicy` 8 variants `:42-51`, `WholeFunctionReturnValueEvidence` `:99`, identity schema `omega.terminal.whole-function-exit-contract.v14\0` `:196`, `whole_function_exit_contract_identity` `:198`, `exit_policy_tag` `:330` (tag values 1-4, 7-9, 11). `functions: Box<Vec<..>>` `:181`. Lookup `function(machine)` linear `:185`.

### D13. `import_code.rs` (128)
`ImportCodeIdentity::{Normalized, StringBackedBootstrap{library: String, symbol: String}}` `:8-11` (users: grepped `StringBackedBootstrap` = 27 hits in image crates, the final-image import plans, `image-pe/src/imports.rs:464`, `image/src/import_code.rs:38`). `EncodedImportProgram::body` unique-or-`None` `:73-80`; `join` `:83-118` refuses different targets, disagreeing resolver bodies or differing bodies under one identity, else appends. `EncodedBodyBindings` `:124`.

### D14. `layout.rs` (945), `layout/relaxation_record.rs` (98)
Purpose: resolved layout types (`ResolvedMachineLayout` `:186`, `ResolvedSelectedFormRow` `:145`, `ResolvedBranchEvidence` `:88`), program bundles (`FramedMachineProgram` `:271`, `EncodedMachineProgram` `:284`, `ResolvedMachineProgram` `:300`, `OptimizedMachineLayout` `:323`), layout identity (schema `omega.terminal.resolved-selected-form-layout.v17` `:77`; `resolved_machine_layout_identity` `:347`), x86 relaxation identities (`:455-648`; schemas `omega.terminal.x86-branch-relaxation.v10` and `...-revision.v3` `:514-515`), `relaxation_policy` module `:651-800` (rule catalog `FUNCTION_RELATIVE_LAYOUT_RULE_CATALOG` one entry `:727`; `x86_rel8_selected` `:773` accepts empty phase or exactly `[X86RelaxConditionalBranchesToRel8V1]`, else `UnsupportedSelection`/`UnsupportedComposition` `:794-797`), `BoundEncodingRequirements` `:803` and `EncodedBodyContracts`/`FramePlacementContracts`/`EncodedBodyPublication` `:864-945`. `X86BranchRelaxationRecord` `relaxation_record.rs:17` is explicitly "not an admission seal" `:1-3`.
Cases by shape: `SelectedFunctionLayoutPolicy` 4 variants `:80-85`, `PerFunctionCanonicalShapeV1` comment "Entry-first conditional-fallthrough chains; remaining chain roots follow selected block roster" `:84`.
Observations (read): the resolved-layout identity hashes bytes, branch evidence, internal and image fixups but not `normalized_foreign_call_fixup` `:400-424`; the revision identity (`encode_functions` `:599-641`) hashes bytes and branch evidence only, not fixups. Class: open (foreign-call fixup metadata affects object relocation but is outside the layout digest; the fixup is carried inside the row and is re-checked by `validate_row` `A8:431-448`; traced).
`BoundEncodingRequirements::finish` `:833` compares subject, `retain_original` and original identity `:842-846,:928-933`. Unused: `ResolvedMachineProgram|OptimizedMachineLayout|FramedMachineProgram` 44 hits in 12 files.
Doc-test (compile_fail) `:903-907` pins that `PhysicalEncodingReadback` has no `allocation` field.

### D15. `layout/text_section.rs` (553), `layout/text_section_publication.rs` (447)
Purpose: `RelocationFreeTextSectionPlacement` `text_section.rs:53`; `PlacedFunctionFragment`, `PlacedBlockSpan`, `PlacedInstructionSpan`, `PlacedInternalMachineCallResolution` `:134`, `PlacedNormalizedForeignCallResolution` `:188`; identity schema `omega.terminal.relocation-free-text-section.v13` `:38`, `relocation_free_text_section_identity` `:211` (image addresses only hashed when non-empty `:309-315`); manifest wire `OMGTSP\0\0` v16 `text_section_publication.rs:30-31`, stage tag 2 only `:140-142`, relocation requirement tags 1..3 `:185-196`, six unavailable bytes equal 1 `:209-215`. Policy has one variant `TextSectionPlacementPolicy` `:41`. `identity` hashes `PlacedCallbackAddressMaterialization`? Observation: the text-section identity does not hash `callback_materializations` of `PlacedNormalizedForeignCallResolution` (`:283-308` omit them; read). Class: open. Tests: `text_section_identity_binds_...` `:339`, `...internal_machine_resolution_roster...` `:508`.

### D16. `layout_requirements.rs` (138)
Data types `EncodedLayoutRequirements` `:11`, `EncodedLayoutOperation` 9 variants `:41`, `EncodedLayoutTerminator` 6 variants `:74` (producer `A9`). Linear lookups `block` `:25`.

### D17. `lib.rs` (545)
Crate root and shared helpers: `identity_encoding` `:68-188` (`encode_conditional_branch_arm` `:90`, `encode_jump_arm` `:116`, `encode_call_owner` `:128` tags 0..4, `encode_invocation_site` `:161`), `image_addresses` `:200-335` (`ImageAddressFields::shifted/unshifted` `:236,:240`, `PlacedImageAddress` `:282`), `instructions::x86_fma::X86FloatingControlRecord` `:344`, `ownership` `:362-403`, `provenance::semantic_code_site` `:410`, plus ~100 lines of `pub use`. `:545 pub use native_stack_demand as physical_stack`. Stated limit `:25-34`: constructing or decoding a record "checks wire and digest consistency only; it never grants lowering, optimization, or publication authority". `:9` "Omega 07-09 -> Machine Emission" is a stage label.

### D18. `storage.rs` (337), `storage_frames.rs` (402)
`storage.rs`: ABI parameter homes and structural store records (`UnitParameterHomeRecord :51`, `StructuralSourceLocation :96` 5 variants, `UnitStructuralScalarFieldStoreRecord :184`, `UnitWriteOnlyPrimitiveStoreRecord :268`, `ScalarStructuralScalarFieldStoreRecord :283`); comment `:59` "Valid only with exact ABI pointer placement; never denotes an invented stack home". `storage_frames.rs`: frame layout, storage and protocol plans (`TargetFrameLayoutPlan :183`, `FunctionTargetFrameLayout :156`, `ReturnAddressFrameCustody :90`), identities `target_frame_layout_identity :265` (schema `omega.target-frame-layout.v10`), `target_frame_protocol_encoding_identity :354` (`...v1`), `function_fragment_frame_application_identity :379` (`...v3`). `FrameProtocolByteSpan::bytes` returns `None` on bad range `:207-212`. Policy variants `TargetFrameLayoutPolicy` 2 `:84-87` (comment: "Every AArch64 function gives incoming link an exact frame slot whether or not it calls").

---

## E. `R/machine-code/stack-demand/` (crate `native_stack_demand`, re-exported as `machine_code::physical_stack`)

### E1. `physical_stack.rs` (82), `demand.rs` (254), `tests.rs` (116)
1. Purpose: composes the worst-case stack of a call graph from per-function local extents, call edges and foreign extents, without bytes.
2. Shape: `LocalStackExtent` `:9`, `ForeignStackExtent` `:23` (None/Admitted/Hosted/CompilerImport) with `from_binding` `:31`, `PhysicalStackProgram` `:72`; `derive_physical_stack_demand` `demand.rs:42`, recursive `visit` `:90`, `StackDemand` `:179`.
3. In/out: reads `PhysicalStackProgram`, entry machine, private callback demands; returns `StackDemand` (ceiling bytes, stack alignment 16 `:80`).
4. Decides on `MachineId` (sorted `Vec` + `binary_search`) `:47-57,:113`; memoised `peaks: Vec<Option<u64>>` and `active: Vec<bool>` for cycle detection `:59-60,:99-103`; `BTreeSet`s for machines, reports, commitments, hosted, compiler imports `:34-40`. Compiler imports must match program target `:68-74`.
5. Cases by shape: `ForeignStackExtent::Hosted{receipt: None}` is `InvalidPrivateFunctionIdentity` `:138`; `LocalStackExtent::Conflicting`/`Unaccounted` are errors `:106-110`; `None` and `SystemCall` add only `caller_live_bytes` `:131-141`; hosted or compiler-import extents set `requires_host_stack()` so `ceiling_bytes()` is `None` `:215-227`.
6. Same job elsewhere: stage 9 `stack_demand.rs` and `stack_demand/physical.rs` call these (grepped `derive_physical_stack_demand|ForeignStackExtent::from_binding` = 18 hits in 8 files; `body_resources.rs` 2, `object_artifact.rs` 3).
7. Unused: `StackDemand::with_entry_cost` `:197` used by `tests.rs:54` (other callers not counted).
8. Stated limits: none. `StackDemand::known_stack_bytes` comment excludes "unbounded foreign frames of a hosted import" `:219-220`.
9. Fail-open: none seen. A foreign call with `ForeignStackExtent::None` is accepted at zero extra bytes `:128-142`; class: stated by design (system call).

---

## F. `R/operation-evidence/`

### F1. `src/completion_claims.rs` (28), `src/provider_calls.rs` (36), `src/provenance.rs` (74), `src/lib.rs` (9)
Data. `CompletionClaimSource` `completion_claims.rs:10` is THE type re-exported by `abstract_operations` `ownership.rs:1` and `target_vocabulary lib.rs:20`; grepped `use .*CompletionClaimSource` shows 12 import sites (target-operations, register-homes, target-vocabulary, provider-admission, abstract-operations, `pipeline/04`, `pipeline/15`). `input()` `:21` has `(None, None) => unreachable!()` `:25`. `InstalledAssumedBindingEvidence::matches` `provider_calls.rs:13` compares by ids (`required`, `implementation`, `selected_requirement`). `provenance.rs`: `PsiProvenance`, `WorkSettlement`, `ValueDefinitionSite` + `encode_value_definition_site_identity` `:40`, `OwnershipEvent` 6 variants `:67`. Header: "Every form ... keeps these unchanged, so they have one owner" `:3-4`.

### F2. `src/ownership_frontier.rs` (178)
`OwnershipFrontierFact` `:49` with `new` computing identity `:58`; `ownership_frontier_fact_identity` `:80` (domain `omega.psi-ownership-frontier-fact.v3\0` `:87`); `has_canonical_identity` `:74`. `encode_frontier_len` `.expect(..)` `:140`. Fail-open: none seen. Header says stage 01 proves them, stage 02 attaches, later forms carry unchanged `:1-5`.

### F3. `boundary-applications/` (5 files: `boundary_applications.rs` 92, `coverage.rs` 259, `opaque_applications.rs` 91, `realization.rs` 240, `source_guards.rs` 60)
1. Purpose: replay of the Terminal boundary-operator demand set against role-specific native realizations; coverage references are domain-separated digests.
2. Shape: `TerminalBoundaryApplicationCoverage::new` `boundary_applications.rs:31`, `validate_for_terminal` `:53`; `OperatorApplicationCoverageRef` `coverage.rs:32`; `coverage_identity` `:71` (domain `omega.d29-operator-application-coverage.sha256.v1\0` `:20`); `BoundaryApplicationRealization` 4 variants `realization.rs:21`; `BoundaryApplicationRealizationCompanion::new` `:91` / `native_binding` `:115`; `TerminalBoundaryApplicationRealizations::new` `:160` (sorts by operation id, requires one-to-one with demands `:164-174`); `BoundaryOpaqueRepresentationApplications::new` `opaque_applications.rs:29`.
3. In/out: `TerminalBoundaryApplicationDemands` + realization rows → coverage refs.
4. Decides on: operation id order alignment `realization.rs:187-191`; `has_strong_commitments` (non-zero digests) `:60-80`; opaque rows deduped and conflicting by `(abi, shape_root)` `opaque_applications.rs:36-45`.
5. Cases by shape: `NativeBinding` cannot carry a selected-plan digest `realization.rs:96-101`; all other roles require a non-zero plan digest `:102-104`; `ExactCompilerIntrinsic` always counts as strong `:78`.
6. Same job elsewhere: none seen.
7. Unused: `source_guards.rs` is `#[cfg(test)]` (`boundary_applications.rs:9`); it scans `.rs` files for string fields such as `identity: String` `:40-47`.
8. Stated limits: none.
9. Fail-open: `validate_for_terminal` `boundary_applications.rs:53-63` validates demands, realizations and references but not `opaque_applications` (set after construction through `with_opaque_applications` `:45`). Class: open.

---

## G. `R/operation-operands/src/`

### G1. `access_lifecycles.rs` (237), `access_references.rs` (92), `calls.rs` (138), `integer_arithmetic.rs` (260), `structural_stores.rs` (106), `values.rs` (63), `lib.rs` (11)
1. Purpose: operand records carried unchanged from abstract ops to machine forms.
2. Shape: `AccessLifecycleOperands::from_arguments` `access_lifecycles.rs:35`; `AccessReferenceBacking` `access_references.rs:11` (`address_source` `:38`); `StructuralCallFormal` `calls.rs:12`, `AbstractCallTarget` `:59`; `IntegerArithmetic::new` `integer_arithmetic.rs:61`, `IntegerShiftPolicy` `:211`; `StructuralScalarStoreDestination` `structural_stores.rs:29`; `AbstractResult`, `ValueBinding` `values.rs`.
3. Decides on: `StructuralAccess` of each argument and operand count: Relocate 4 (`Owned,Owned,SharedBorrow,SharedBorrow`), View 2 (reading borrow + shared), Initialize 3, MoveOut 2 `access_lifecycles.rs:39-84`, else `Err("...differ from the operation's ownership roles")` `:84`. `IntegerArithmetic::new` requires an obligation iff policy is `Exact` or operator is a division `:54-67`.
4. Cases by shape: four lifecycle operations only; 15 policy×operator pairs plus shifts `integer_arithmetic.rs:103-206`; `AbstractCallTarget::operand_type` for `Dynamic` is `u64` selector via `.expect` `:92-97`.
5. Same job elsewhere: stage 5 `selection_access_lifecycles.rs` consumes (11 hits for `AccessLifecycleOperands`, 13 in the defining file; 34 total in 8 files); `IntegerArithmetic|IntegerShiftPolicy` 185 hits in 31 files (`candidates.rs` 38).
6. Unused: `BorrowedExtentBacking` returns `None` from `address_source` `access_references.rs:47`.
7. Stated limits: `integer_arithmetic.rs:1-20` explains replacing 19 operation variants with (policy, operator, obligation); fields kept private so `new` refuses missing or extra obligation.
8. Fail-open: none seen.

---

## H. `R/optimization-records/src/` (crate `optimization-core`)

### H1. `lib.rs` (57), `canonical_rows.rs` (104, test only)
Root exports; `canonical_rows.rs` pins golden digests for a decision record, a baseline log, an external point and log `:21-104`. Re-exports `optimization_selections::*` `lib.rs:52-57`.

### H2. `contracts/` (analysis 129, budget 101, decision 97, error 21, mod 23, rule 123, tests 135)
1. Purpose: closed contracts: 21-variant `AnalysisKind` and 8-byte bitsets, 5-axis nonzero `OptimizationWorkBudget`, `OptimizationSafetyClass` (5), `OptimizationReasonCode` (12), `OptimizationCandidateVerdict`, 97-byte `OptimizationRuleContract` (`OMGRUL\0\0` v1).
2. Shape: `AnalysisKind` `analysis.rs:10`, `AnalysisSet` `:55`, `OptimizationWorkBudget::new` `budget.rs:34`, `for_extent` `:62` (scales by `div_ceil(1<<16)`, saturating), `OptimizationRuleContract::new` `rule.rs:37`, `decode` `:71`.
3. Decides on: unknown bits (`bits & !KNOWN_ANALYSIS_BITS`) rejected `analysis.rs:97`; zero budget axis rejected `budget.rs:48-58`; zero rule version rejected `rule.rs:45`; verdict `(1, reason!=0)` rejected `decision.rs:91`.
4. Cases by shape: fixed lengths 8, 40, 97, 2, 1 with `WrongLength` errors.
5. Users: grepped `OptimizationRuleContract|OptimizationWorkBudget::decode|AnalysisInvalidationSet` outside this crate = 159 hits in 14 files (rules.rs 31, rewrite.rs 35, sccp.rs 25).
6. Fail-open: none seen. `OptimizationRuleContract::new` does not check that the pass/rule pair is consistent (no such check is stated).

### H3. `decisions/baseline.rs` (373), `decisions/external_schema/{mod 301, codec 183, identity 74, tests 363}`, `decisions/mod.rs` (18)
1. Purpose: baseline decision log (`OMGBDL\0\0` v1) and external decision log/point schema v2 (`OMGEXD\0\0`/`OMGEXP\0\0`).
2. Shape: `BaselineDecisionLogBuilder::record_validated_outcome` `baseline.rs:193`; `BaselineDecisionLog::{concatenate :115, encode :128, decode :140}`; `ExternalDecisionPoint::new` `external_schema/mod.rs:37`, `ExternalDecisionLog::new` `:120`; codec `decode_*` `codec.rs:46-181`.
3. Decides on: candidate identity ordering `(predicted_cost_delta, candidate)` and dedup `baseline.rs:241-242`; `Choose` must name a considered candidate `:204-210`; baseline `Skip` only `NotProfitable` `:211-215` and decode only accepts `NotProfitable` `:166-171`; external decode accepts `Skip(NotProfitable)` only `codec.rs:173-178` but `ExternalDecisionPoint::new` places no limit on the skip reason `mod.rs:54-60` (only `Choose` is checked). So a point made with another reason encodes (`encode_action` `identity.rs:69-72` writes `reason as u8`) but does not decode (inferred from reading both).
4. Cases by shape: `Choose` / `Skip` actions; external facts must be strictly ascending `codec.rs:146`.
5. Users: baseline types: `pass_manager.rs` (22) and `validation.rs` (4) in stage 3; `External*` appear only in `pass_manager.rs` (81) and `optimization-unit/src/ledger.rs` (2) outside the crate (grepped).
6. Stated limits: `decisions/mod.rs:3-5`: "Neither module executes candidate selection policy".
7. Fail-open: none seen beyond the skip-reason asymmetry above (class: traced).

### H4. `identities/` (artifacts 39, bundle 155, decisions 13, error 21, mod 123, rules_and_facts 39, stages 30, tests 211)
Purpose: ~45 domain-separated 32-byte identities via macros `canonical_identity!`/`s!` `mod.rs:36-84` (digest of domain, `u64` length, bytes `:12-16`); `OptimizationIdentityBundle` (`OMGIDB\0\0` v1) `bundle.rs:22` with optional decision-log and workload-profile identities framed `0/1` `:104-112,:147-155`; `OptimizationRuleSetIdentity::from_ordered_rules` `rules_and_facts.rs:18` refuses duplicates. Users: grepped `OptimizationIdentityBundle|OptimizationPassManifestRecord` outside crate = 45 hits in 13 files.

### H5. `manifest/` (codec 5, decision 229, error 40, fact_reference 71, mod 51, pass 159, tests/decision 213, tests/fact_reference 74, tests/mod 56)
Purpose: decision record `OMGDEC\0\0` v5 and pass record `OMGPAR\0\0` v1. `OptimizationDecisionRecord::new` `decision.rs:53` refuses `Applied` without validator and non-ascending facts `:62-67`; `OptimizationFactReference` 33-byte tagged union (4 tags) `fact_reference.rs`; `OptimizationPassManifestRecord::new` `pass.rs:49` checks ordered-rule-set digest, no duplicate rules, each decision's rule scheduled, no duplicate decision or candidate ids `:58-76`. `DECISION_FIXED_WIDTH = 155` `mod.rs:39-48`. Users: `OptimizationDecisionRecord` 8 hits in `pass_manager.rs` only outside the crate (grepped).
Fail-open: none seen.

---

## Coverage
Read in full (every line of every range in `units/unit_029.tsv`, 103 files): 21,975 lines.
Skimmed: none.
Not read: none.
Counts of `#[test]` functions, usage counts and file lists above come from the Grep tool (count or content mode, no head limit except where noted "head_limit 30/40" on the files-with-matches lists for `IntegerArithmetic`, `BaselineDecisionLog` and `ResolvedMachineProgram`, which therefore may under-list the file names; their totals are the tool's total line).

## Five things a maintainer most needs to know about this unit
1. Two crates here (`encoding-admission`, `frame-admission`) are checkers that re-run producer logic: frame equations are shared with stage 11 (`frame_address::resolve` is called by both: `pipeline/11.../lib.rs:214`), frame-protocol bytes are regenerated with the producer's ISA recipes (`frame-admission/frame_protocol.rs:41-42`), and `fragment_origins.rs:44-48` runs `check_control_provenance` on the output of `control_provenance` in the same function.
2. `exit_requirements.rs` (encoding-admission) accepts any non-listed instruction kind whose memory effect is `NoneV1` with any address (`:1035-1048`), and `CopyBytes`/`ScanByteSequence` with only operand-index checks (`:783,:827`); it also uses the x86 register model directly (`:440-566`).
3. Identities are hand-hashed per record with versioned schema strings (`v31` encoding, `v17` layout, `v25` fragments, `v13` text section, `v14` exit, `v10` frame layout); several omit fields: resolved-layout identity omits `normalized_foreign_call_fixup` (`layout.rs:400-424`), text-section identity omits callback materializations (`text_section.rs:283-308`), fragment identity has no presence byte for image fixups and reuses tag 10 (`fragments/identity.rs:75-80,:225,:246`).
4. Layout policy and shape coverage are narrow by design: `select` accepts four policies, `is_admitted_canonical_shape` is only "non-empty with an entry" (`geometry.rs:339-343`), AArch64 conditional widening is the only layout growth, x86 short form is detected from `bytes.len() == 2` (`validation.rs:376`).
5. Wire manifests (`OMGFFE`, `OMGTSP`) check framing and digest only; their statistics and stage tags are claims (`publication.rs` header and test `:355-370`), and unavailable-data fields are fixed bytes equal 1.

## What I am unsure about
- Whether the shared-ISA-recipe limitation (`lib.rs:5`, `frame_protocol.rs:41`) is the only reason `validate_target_frame_protocol_encoding` is not independent; I did not read `isa_x86_64`/`isa_aarch64` encoders.
- Whether `contains_call` (`frame_layout.rs:325`) misses any call-bearing family; I did not grep the `MachineAlternativeFamily` enum.
- Whether the omissions listed under D14/D15 (foreign-call fixup and callback materialization not hashed into the identity) are covered by another digest at stage 13/14 (not read).
- `ImageAddressOrigins::require` assumes `rows` is sorted; I did not read the producer in stage 9.
- Counts for `AccessLifecycleOperands` (34 in 8 files), `IntegerArithmetic|IntegerShiftPolicy` (185 in 31), `BaselineDecisionLog|ExternalDecisionLog|ExternalDecisionPoint` (128 in 12), `ResolvedMachineProgram|...` (44 in 12) include definitions and tests; I did not separate non-test callers.
