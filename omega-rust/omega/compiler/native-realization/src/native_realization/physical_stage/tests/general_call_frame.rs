//! A caller keeping earlier call results live across later calls must preserve
//! them in callee-saved storage: the validated layout records the exact save
//! slots, the frame protocol emits one save in the prologue and one restore in
//! the epilogue per slot, and the whole program still reaches ordinary callable
//! publication. On AArch64 the non-leaf caller also gives the incoming link
//! register an exact frame slot; on x86-64 the return address stays in the
//! caller's activation record and Microsoft targets reserve the 32-byte shadow
//! inside the outgoing ABI area.

use semantic_vocabulary::{
    BlockId, ContractId, EdgeId, IntegerSign, IntegerType, IntegerValue, MachineId, OperationId,
    ScalarType, ValueId,
};
use terminal_psi::*;

const CALLER: u64 = 1;
const CALLEE: u64 = 2;
const CALLER_BASE: u64 = 41_000;
const CALLEE_BASE: u64 = 42_000;

/// Two register arguments keep `left` and `right` live across the first call;
/// the first and second results stay live across the second and third calls.
/// Every live-across-call value can only occupy callee-saved storage, so the
/// caller's frame carries real preservation slots on every target.
pub(super) fn preserving_call_artifact() -> (Vec<u8>, Vec<u8>) {
    let u64_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    let value = |id: u64| ValueDeclaration {
        qualifications: Default::default(),
        id: ValueId::new(id).unwrap(),
        scalar_type: u64_type,
    };
    let left = ValueId::new(CALLER_BASE + 1).unwrap();
    let right = ValueId::new(CALLER_BASE + 2).unwrap();
    let first = ValueId::new(CALLER_BASE + 3).unwrap();
    let second = ValueId::new(CALLER_BASE + 4).unwrap();
    let third = ValueId::new(CALLER_BASE + 5).unwrap();
    let operation = |id: u64, result: u64, kind: OperationKind| Operation {
        static_reach_binding: None,
        id: OperationId::new(id).unwrap(),
        result: OperationResult::Scalar(value(result)),
        kind,
    };
    let call = |id: u64, result: u64, arguments: Vec<ValueId>| {
        operation(
            id,
            result,
            OperationKind::Call {
                erased_arguments: Vec::new(),
                callee: MachineId::new(CALLEE).unwrap(),
                arguments,
                requirement_obligations: Vec::new(),
                crash_continuations: Vec::new(),
            },
        )
    };
    let caller = TerminalMachine {
        closed_reach_application: None,
        declared_service_reach: Vec::new(),
        id: MachineId::new(CALLER).unwrap(),
        attachment: None,
        parameters: Vec::new(),
        structural_parameters: Vec::new(),
        ranked_scc: None,
        result: TerminalMachineResult::Scalar(value(CALLER_BASE + 6)),
        structural_places: Vec::new(),
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: BlockId::new(CALLER_BASE).unwrap(),
        blocks: vec![Block {
            erased_scalar_formals: Vec::new(),
            structural_parameters: Vec::new(),
            id: BlockId::new(CALLER_BASE).unwrap(),
            parameters: Vec::new(),
            operations: vec![
                operation(
                    CALLER_BASE + 1,
                    CALLER_BASE + 1,
                    OperationKind::IntegerConstant {
                        value: IntegerValue::Unsigned(7),
                    },
                ),
                operation(
                    CALLER_BASE + 2,
                    CALLER_BASE + 2,
                    OperationKind::IntegerConstant {
                        value: IntegerValue::Unsigned(9),
                    },
                ),
                call(CALLER_BASE + 3, CALLER_BASE + 3, vec![left, right]),
                call(CALLER_BASE + 4, CALLER_BASE + 4, vec![left, right]),
                call(CALLER_BASE + 5, CALLER_BASE + 5, vec![first, second]),
            ],
            terminator: Terminator::Return {
                edge: EdgeId::new(CALLER_BASE + 7).unwrap(),
                value: third,
                cleanup_actions: Vec::new(),
            },
        }],
        contract: MachineContract {
            erased_scalar_formals: Vec::new(),
            id: ContractId::new(CALLER_BASE + 8).unwrap(),
            requires: Vec::new(),
            ensures: Vec::new(),
            outcome_specific_ensures: Vec::new(),
            crash_routes: Vec::new(),
        },
    };
    let callee = TerminalMachine {
        closed_reach_application: None,
        declared_service_reach: Vec::new(),
        id: MachineId::new(CALLEE).unwrap(),
        attachment: None,
        parameters: vec![value(CALLEE_BASE), value(CALLEE_BASE + 1)],
        structural_parameters: Vec::new(),
        ranked_scc: None,
        result: TerminalMachineResult::Scalar(value(CALLEE_BASE + 2)),
        structural_places: Vec::new(),
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: BlockId::new(CALLEE_BASE).unwrap(),
        blocks: vec![Block {
            erased_scalar_formals: Vec::new(),
            structural_parameters: Vec::new(),
            id: BlockId::new(CALLEE_BASE).unwrap(),
            parameters: Vec::new(),
            operations: Vec::new(),
            terminator: Terminator::Return {
                edge: EdgeId::new(CALLEE_BASE + 3).unwrap(),
                value: ValueId::new(CALLEE_BASE).unwrap(),
                cleanup_actions: Vec::new(),
            },
        }],
        contract: MachineContract {
            erased_scalar_formals: Vec::new(),
            id: ContractId::new(CALLEE_BASE + 4).unwrap(),
            requires: Vec::new(),
            ensures: Vec::new(),
            outcome_specific_ensures: Vec::new(),
            crash_routes: Vec::new(),
        },
    };
    let module = TerminalModule {
        scalar_qualifications: Default::default(),
        scalar_block_invariants: Vec::new(),
        operation_crash_contracts: Vec::new(),
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: MachineId::new(CALLER).unwrap(),
        structural_types: Vec::new(),
        structural_domains: Vec::new(),
        services: Vec::new(),
        root_service_reach: Default::default(),
        placed_view_inputs: Vec::new(),
        reborrow_root_handoffs: Vec::new(),
        reborrow_restored_call_uses: Vec::new(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        float_meaning_projections: Vec::new(),
        float_meaning_equalities: Vec::new(),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        evidence_terms: Vec::new(),
        evidence_contract_lanes: Vec::new(),
        proof_output_calls: Vec::new(),
        proof_recursive_components: Vec::new(),
        closed_conformance_applications: Vec::new(),
        dynamic_dispatch: Default::default(),
        suspension_call_plan_count: 0,
        suspension_call_sites: Vec::new(),
        suspension_call_plans: Vec::new(),
        quotient_correspondences: Vec::new(),
        machines: vec![caller, callee],
    };
    (
        terminal_codec::encode_module(&module).unwrap(),
        terminal_codec::encode_proof_section(&module, &ProofBundle::default()).unwrap(),
    )
}

fn lower(
    target: target::NativeTarget,
) -> (
    abstract_operations_to_target_operations::ValidatedOptimizedTargetOperations,
    optimization_core::PostTerminalOptimizationSelectionProjection,
) {
    let (semantic, proof) = preserving_call_artifact();
    let input = terminal_psi_to_abstract_operations::lower_artifact_for_optimization(
        terminal_psi_to_abstract_operations::ArtifactSections {
            semantic_bytes: &semantic,
            proof_bytes: &proof,
            obligation_ledger_bytes: None,
        },
        &proof_admission::AdmissionProfile::default(),
    )
    .and_then(|admitted| admitted.try_into_optimization_input())
    .unwrap();
    let optimized = crate::optimize_verified_abstract_input(
        input,
        crate::compiler_baseline_request_v1(&optimization_core::OptimizationSelections::default()),
    )
    .unwrap();
    let post_terminal = optimized.selections().project_post_terminal();
    let target_program =
        abstract_operations_to_target_operations::lower_optimized_to_target_operations(
            optimized,
            abstract_operations_to_target_operations::OptimizedTargetLoweringRequest::new(target),
        )
        .unwrap();
    (target_program, post_terminal)
}

/// Decode the callee-save stores an x86-64 prologue emits after the stack
/// adjustment: `mov [rsp + offset], reg` per slot. Returns `(register name,
/// frame offset)` in emitted order. This frame never needs probing, so the
/// adjustment is a single `sub rsp` prefix.
fn x86_64_prologue_saves(prologue: &[u8]) -> Vec<(u8, u64)> {
    let mut cursor = match prologue {
        [0x48, 0x83, 0xec, _, ..] => 4,
        [0x48, 0x81, 0xec, ..] => 7,
        _ => panic!("prologue must open with the stack adjustment: {prologue:02x?}"),
    };
    let mut saves = Vec::new();
    while cursor < prologue.len() {
        let rex = prologue[cursor];
        assert!(
            matches!(rex, 0x48 | 0x4c),
            "rex {rex:#x} in {prologue:02x?}"
        );
        assert_eq!(prologue[cursor + 1], 0x89, "mov-store opcode");
        let modrm = prologue[cursor + 2];
        assert_eq!(modrm & 0b111, 0b100, "SIB follows");
        assert_eq!(prologue[cursor + 3], 0x24, "rsp-based SIB");
        let register = (modrm >> 3) & 7 | ((rex & 4) << 1);
        let (offset, width) = match modrm >> 6 {
            0b00 => (0, 4),
            0b01 => (u64::from(prologue[cursor + 4]), 5),
            0b10 => (
                u64::from(u32::from_le_bytes(
                    prologue[cursor + 4..cursor + 8].try_into().unwrap(),
                )),
                8,
            ),
            _ => unreachable!(),
        };
        saves.push((register, offset));
        cursor += width;
    }
    saves
}

/// Decode the callee-save restores an x86-64 epilogue emits before the stack
/// release: `mov reg, [rsp + offset]` per slot in reverse prologue order.
fn x86_64_epilogue_restores(epilogue: &[u8]) -> Vec<(u8, u64)> {
    let mut restores = Vec::new();
    let mut cursor = 0;
    while cursor + 4 <= epilogue.len() && epilogue[cursor + 1] == 0x8b {
        let rex = epilogue[cursor];
        assert!(matches!(rex, 0x48 | 0x4c));
        let modrm = epilogue[cursor + 2];
        assert_eq!(modrm & 0b111, 0b100);
        assert_eq!(epilogue[cursor + 3], 0x24);
        let register = (modrm >> 3) & 7 | ((rex & 4) << 1);
        let (offset, width) = match modrm >> 6 {
            0b00 => (0, 4),
            0b01 => (u64::from(epilogue[cursor + 4]), 5),
            0b10 => (
                u64::from(u32::from_le_bytes(
                    epilogue[cursor + 4..cursor + 8].try_into().unwrap(),
                )),
                8,
            ),
            _ => unreachable!(),
        };
        restores.push((register, offset));
        cursor += width;
    }
    match &epilogue[cursor..] {
        [0x48, 0x83, 0xc4, _] | [0x48, 0x81, 0xc4, ..] => {}
        tail => panic!("epilogue must end with the stack release: {tail:02x?}"),
    }
    restores
}

fn x86_64_register_name(code: u8) -> String {
    match code {
        0..=7 => {
            ["rax", "rcx", "rdx", "rbx", "rsp", "rbp", "rsi", "rdi"][usize::from(code)].to_string()
        }
        _ => format!("r{code}"),
    }
}

/// Decode the register saves an AAPCS64 prologue emits after `sub sp`: one
/// `str` per save slot including the saved link register.
fn aarch64_prologue_saves(prologue: &[u8]) -> Vec<(String, u64)> {
    let mut words = prologue
        .as_chunks::<4>()
        .0
        .iter()
        .map(|chunk| u32::from_le_bytes(*chunk));
    let first = words.next().expect("prologue adjusts the stack pointer");
    assert_eq!(
        first & 0xff00_03ff,
        0xd100_03ff,
        "prologue must open with sub sp: {first:#010x} in {prologue:02x?}"
    );
    words
        .map(|word| {
            let store = word & 0xffc0_03e0;
            let prefix = match store {
                0xf900_03e0 => "x",
                0xfd00_03e0 => "d",
                _ => panic!("expected a slot store: {word:#010x}"),
            };
            (
                format!("{prefix}{}", word & 0x1f),
                u64::from((word >> 10) & 0xfff) * 8,
            )
        })
        .collect()
}

/// Decode the register restores an AAPCS64 epilogue emits before `add sp`:
/// one `ldr` per saved slot in reverse prologue order.
fn aarch64_epilogue_restores(epilogue: &[u8]) -> Vec<(String, u64)> {
    let words = epilogue
        .as_chunks::<4>()
        .0
        .iter()
        .map(|chunk| u32::from_le_bytes(*chunk))
        .collect::<Vec<_>>();
    let (loads, tail) = words.split_at(words.len() - 1);
    assert_eq!(
        tail[0] & 0xff00_03ff,
        0x9100_03ff,
        "epilogue must end with add sp: {:#010x} in {epilogue:02x?}",
        tail[0]
    );
    loads
        .iter()
        .map(|word| {
            let load = word & 0xffc0_03e0;
            let prefix = match load {
                0xf940_03e0 => "x",
                0xfd40_03e0 => "d",
                _ => panic!("expected a slot restore: {word:#010x}"),
            };
            (
                format!("{prefix}{}", word & 0x1f),
                u64::from((word >> 10) & 0xfff) * 8,
            )
        })
        .collect()
}

#[test]
fn preserved_call_frame_replays_exact_save_accesses_through_callable_publication() {
    let caller = MachineId::new(CALLER).unwrap();
    let callee_id = MachineId::new(CALLEE).unwrap();
    for target in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::windows_x64(),
        target::NativeTarget::uefi_x64(),
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::macos_arm64(),
    ] {
        let (target_program, post_terminal) = lower(target);
        let physical = crate::stage_optimized_verified_physical_pipeline(
            target_program,
            post_terminal.selections(),
        )
        .unwrap_or_else(|error| panic!("{target:?}: {error:?}"));
        let realization = physical.fixed_frame_for_test();
        let layout = realization.frame().plan();
        let row = layout
            .functions
            .iter()
            .find(|function| function.machine == caller)
            .unwrap_or_else(|| panic!("{target:?}: missing caller frame row"));
        // The general-call frame plan records its call-site alignment contract:
        // every call in this function executes with the ABI stack alignment.
        assert!(row.contains_call, "{target:?}");
        assert_eq!(row.pre_call_stack_alignment, 16, "{target:?}");
        assert_eq!(row.abi_stack_alignment_bytes, 16, "{target:?}");
        assert_eq!(row.stack_probe.touches, 0, "{target:?}: small frame");
        // Call-site alignment evidence: the post-prologue stack pointer is
        // 16-aligned, so every call in the body executes at the ABI alignment.
        // On x86-64 `call` pushed the 8-byte return address onto an aligned
        // entry RSP, so the frame itself keeps residue 8; on AArch64 `bl`
        // leaves SP untouched, so the frame keeps residue 0.
        let expected_residue = match target.architecture {
            target::Architecture::X86_64 => 8,
            target::Architecture::Aarch64 => 0,
        };
        assert!(row.frame_size_bytes > 0, "{target:?}");
        assert_eq!(
            row.frame_size_bytes % u64::from(row.pre_call_stack_alignment),
            expected_residue,
            "{target:?}: post-prologue stack pointer must be aligned at call sites: {:?}",
            row.frame_size_bytes
        );
        if let target::ObjectFormat::Coff = target.object_format {
            assert_eq!(row.outgoing_abi_area.byte_size, 32, "{target:?}");
            assert_eq!(row.outgoing_abi_area.shadow_bytes, 32, "{target:?}");
        } else {
            assert_eq!(row.outgoing_abi_area.byte_size, 0, "{target:?}");
            assert_eq!(row.outgoing_abi_area.shadow_bytes, 0, "{target:?}");
        }
        // The layout's callee-save roster is the exact frame evidence: each
        // slot is aligned, placed past the outgoing ABI area, inside the
        // frame, and disjoint from every sibling slot.
        assert!(
            !row.callee_save_slots.is_empty(),
            "{target:?}: live-across-call values must occupy preservation storage"
        );
        for (index, slot) in row.callee_save_slots.iter().enumerate() {
            assert_eq!(slot.size_bytes, 8, "{target:?}");
            assert!(
                slot.frame_offset_bytes.is_multiple_of(slot.alignment_bytes),
                "{target:?}: {slot:?}"
            );
            assert!(
                slot.frame_offset_bytes >= row.outgoing_abi_area.byte_size,
                "{target:?}: save slot escapes the outgoing ABI area: {slot:?}"
            );
            assert!(
                slot.frame_offset_bytes + slot.size_bytes <= row.frame_size_bytes,
                "{target:?}: save slot escapes the frame: {slot:?}"
            );
            for other in &row.callee_save_slots[index + 1..] {
                assert!(
                    slot.frame_offset_bytes + slot.size_bytes <= other.frame_offset_bytes
                        || other.frame_offset_bytes + other.size_bytes <= slot.frame_offset_bytes,
                    "{target:?}: overlapping save slots {slot:?} {other:?}"
                );
            }
        }
        let current = realization.allocation().current();
        let physical_model = current.register_environment().physical();
        let view_name = |view| {
            physical_model
                .model()
                .views
                .iter()
                .find(|entry| entry.id == view)
                .unwrap_or_else(|| panic!("unknown view {view:?}"))
                .name
                .clone()
        };
        // The saved-roster the protocol must encode: every callee-save slot,
        // and on AArch64 the saved link register's frame slot as well.
        let mut expected_saves = row
            .callee_save_slots
            .iter()
            .map(|slot| (view_name(slot.storage_view), slot.frame_offset_bytes))
            .collect::<Vec<_>>();
        match (target.architecture, row.return_address) {
            (
                target::Architecture::X86_64,
                machine_code::ReturnAddressFrameCustody::CallerActivationStack {
                    post_prologue_offset_bytes,
                    size_bytes,
                },
            ) => {
                assert_eq!(
                    post_prologue_offset_bytes, row.frame_size_bytes,
                    "{target:?}"
                );
                assert_eq!(size_bytes, 8, "{target:?}");
            }
            (
                target::Architecture::Aarch64,
                machine_code::ReturnAddressFrameCustody::SavedLinkRegister {
                    view,
                    frame_offset_bytes,
                    size_bytes,
                },
            ) => {
                assert_eq!(size_bytes, 8, "{target:?}");
                assert!(frame_offset_bytes.is_multiple_of(8), "{target:?}");
                assert!(
                    frame_offset_bytes + 8 <= row.frame_size_bytes,
                    "{target:?}: saved link escapes the frame"
                );
                expected_saves.push((view_name(view), frame_offset_bytes));
            }
            custody => panic!("{target:?}: unexpected return-address custody {custody:?}"),
        }
        // The leaf callee makes no calls and needs no preservation storage.
        // On AArch64 it keeps the incoming link live in the architectural
        // register instead of a frame slot — the leaf exemption the caller's
        // non-leaf plan cannot take.
        let callee_row = layout
            .functions
            .iter()
            .find(|function| function.machine == callee_id)
            .unwrap();
        assert!(!callee_row.contains_call, "{target:?}");
        assert!(
            callee_row.callee_save_slots.is_empty(),
            "{target:?}: {callee_row:?}"
        );
        match target.architecture {
            target::Architecture::X86_64 => assert!(
                matches!(
                    callee_row.return_address,
                    machine_code::ReturnAddressFrameCustody::CallerActivationStack { .. }
                ),
                "{target:?}: {:?}",
                callee_row.return_address
            ),
            target::Architecture::Aarch64 => assert!(
                matches!(
                    callee_row.return_address,
                    machine_code::ReturnAddressFrameCustody::LiveLinkRegister { .. }
                ),
                "{target:?}: {:?}",
                callee_row.return_address
            ),
        }
        let protocol = realization.protocol().plan();
        let encoding = protocol
            .functions
            .iter()
            .find(|function| function.machine == caller)
            .unwrap();
        let prologue = encoding.prologue.bytes(&protocol.bytes).unwrap();
        let epilogue = encoding.epilogue.bytes(&protocol.bytes).unwrap();
        // Replay the emitted preservation accesses against the validated
        // layout: each prologue store names the slot's storage view and frame
        // offset; the epilogue restores the same roster in reverse order.
        let (emitted_saves, emitted_restores) = match target.architecture {
            target::Architecture::X86_64 => (
                x86_64_prologue_saves(prologue)
                    .into_iter()
                    .map(|(code, offset)| (x86_64_register_name(code), offset))
                    .collect::<Vec<_>>(),
                x86_64_epilogue_restores(epilogue)
                    .into_iter()
                    .map(|(code, offset)| (x86_64_register_name(code), offset))
                    .collect::<Vec<_>>(),
            ),
            target::Architecture::Aarch64 => (
                aarch64_prologue_saves(prologue),
                aarch64_epilogue_restores(epilogue),
            ),
        };
        assert_eq!(emitted_saves, expected_saves, "{target:?} prologue");
        let mut expected_restores = expected_saves.clone();
        expected_restores.reverse();
        assert_eq!(emitted_restores, expected_restores, "{target:?} epilogue");
        let emitted = machine_emission::stage_optimized_function_fragment_emission(
            physical.into_function_fragment_emission_source(),
        )
        .unwrap_or_else(|error| panic!("{target:?}: {error:?}"));
        let applied = machine_emission::stage_function_fragment_frame_application(emitted)
            .unwrap_or_else(|error| panic!("{target:?}: {error:?}"));
        // One return per function: the caller's epilogue restores its saved
        // roster before returning, and the leaf callee releases its own frame.
        assert_eq!(
            applied.receipt().epilogue_application_count(),
            2,
            "{target:?}"
        );
        let text = machine_emission::stage_optimized_fixed_frame_text_section(applied)
            .unwrap_or_else(|error| panic!("{target:?}: {error:?}"));
        let calls = &text.text_section().resolved_internal_machine_calls;
        assert_eq!(calls.len(), 3, "{target:?}");
        assert!(
            calls
                .iter()
                .all(|call| call.caller == caller && call.callee == callee_id),
            "{target:?}: {calls:?}"
        );
        let object = object_file::stage_optimized_relocation_free_object_container(text)
            .unwrap_or_else(|error| panic!("{target:?}: {error:?}"));
        let (semantic, proof) = preserving_call_artifact();
        let module = terminal_codec::decode_module(&semantic).unwrap();
        let proof = terminal_codec::decode_proof_section_for(&module, &proof).unwrap();
        let optimization =
            terminal_codec::build_identity_optimization_execution_record(&module, &proof).unwrap();
        let terminal = terminal_codec::CanonicalTerminalArtifact::from_parts(
            &module,
            &proof,
            &optimization,
            None,
        )
        .unwrap();
        let artifact = object_file::stage_validated_optimized_object_artifact(terminal, object)
            .unwrap_or_else(|error| panic!("{target:?}: {error:?}"));
        let callable = native_artifact::stage_validated_optimized_ordinary_callable_entry(artifact)
            .unwrap_or_else(|error| panic!("{target:?}: {error:?}"));
        assert_eq!(callable.entry().returns.len(), 1, "{target:?}");
    }
}
