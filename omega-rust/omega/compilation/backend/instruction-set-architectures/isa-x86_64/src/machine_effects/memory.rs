//! Ordinary pointer memory, symbolic outgoing storage, and Unit call effects.

use register_model::{RegisterConstraintKey, ValidatedRegisterConstraintCatalog};
use selected_instructions::{
    MachineAlternative, MachineAlternativeApplicability, MachineAlternativeKey, MachineBarrier,
    MachineCallEffect, MachineCleanupEffect, MachineEffectDeclaration, MachineEncodedControlEffect,
    MachineEncodedEffects, MachineEncodedMemoryEffect, MachineEncodedStackEffect,
    MachineEncodedTrapBehavior, MachineLatencyKnowledge, MachineMemoryEffect, MachineSemanticKind,
    MachineSizeKnowledge, MachineTrapBehavior,
};

pub(super) fn declaration(
    semantic: MachineSemanticKind,
    constraint: RegisterConstraintKey,
    constraints: &ValidatedRegisterConstraintCatalog,
) -> MachineEffectDeclaration {
    let row = constraints
        .catalog()
        .constraint(constraint)
        .expect("canonical memory instruction constraint");
    let stack_pointer = crate::x86_64_physical_register_model_ref()
        .view_named("rsp")
        .expect("canonical stack pointer")
        .id;
    let (memory, trap, reads, writes, encoded_memory, size) = match semantic {
        MachineSemanticKind::PlacedScalarAccess(form) => (
            if form.write {
                MachineMemoryEffect::WritePointerV1
            } else {
                MachineMemoryEffect::ReadPointerV1
            },
            MachineTrapBehavior::MayArchitecturalFaultV1,
            if form.write { vec![0, 1] } else { vec![0] },
            if form.write { vec![] } else { vec![1] },
            if form.write {
                MachineEncodedMemoryEffect::WritePointerV1 { pointer_operand: 0 }
            } else {
                MachineEncodedMemoryEffect::ReadPointerV1 {
                    pointer_operand: 0,
                    byte_count: u16::from(form.bytes),
                }
            },
            MachineSizeKnowledge::EncoderResolved {
                minimum_bytes: 7,
                maximum_bytes: Some(9),
            },
        ),

        MachineSemanticKind::CopyBytes => (
            MachineMemoryEffect::CopyBytesV1,
            MachineTrapBehavior::MayArchitecturalFaultV1,
            vec![0, 1, 2],
            vec![3, 4],
            MachineEncodedMemoryEffect::CopyBytesV1 {
                source_pointer_operand: 0,
                destination_pointer_operand: 1,
                count_operand: 2,
            },
            MachineSizeKnowledge::ExactBytes(36),
        ),
        MachineSemanticKind::ZeroBytes => (
            MachineMemoryEffect::ZeroBytesV1,
            MachineTrapBehavior::MayArchitecturalFaultV1,
            vec![0, 1],
            vec![2],
            MachineEncodedMemoryEffect::ZeroBytesV1 {
                destination_pointer_operand: 0,
                count_operand: 1,
            },
            MachineSizeKnowledge::ExactBytes(25),
        ),
        MachineSemanticKind::LoadEncodedScalar => (
            MachineMemoryEffect::ReadEncodedScalarV1,
            MachineTrapBehavior::MayArchitecturalFaultV1,
            vec![0, 1],
            vec![2, 3, 4, 5],
            MachineEncodedMemoryEffect::ReadEncodedScalarV1 {
                pointer_operand: 0,
                codec_operand: 1,
            },
            MachineSizeKnowledge::EncoderResolved {
                minimum_bytes: 16,
                maximum_bytes: None,
            },
        ),
        MachineSemanticKind::StoreEncodedScalar => (
            MachineMemoryEffect::WriteEncodedScalarV1,
            MachineTrapBehavior::MayArchitecturalFaultV1,
            vec![0, 1, 2],
            vec![3, 4, 5],
            MachineEncodedMemoryEffect::WriteEncodedScalarV1 {
                pointer_operand: 0,
                codec_operand: 1,
            },
            MachineSizeKnowledge::EncoderResolved {
                minimum_bytes: 16,
                maximum_bytes: None,
            },
        ),
        MachineSemanticKind::ScanByteSequence => (
            MachineMemoryEffect::ScanByteSequenceV1,
            MachineTrapBehavior::MayArchitecturalFaultV1,
            vec![0, 1],
            vec![2, 3, 4],
            MachineEncodedMemoryEffect::ScanByteSequenceV1 {
                base_pointer_operand: 0,
                count_operand: 1,
            },
            MachineSizeKnowledge::EncoderResolved {
                minimum_bytes: 16,
                maximum_bytes: None,
            },
        ),
        MachineSemanticKind::CompareByteSequences => (
            MachineMemoryEffect::CompareByteSequencesV1,
            MachineTrapBehavior::MayArchitecturalFaultV1,
            vec![0, 1, 2, 3],
            vec![4, 5, 6, 7],
            MachineEncodedMemoryEffect::CompareByteSequencesV1 {
                left_base_operand: 0,
                left_count_operand: 1,
                right_base_operand: 2,
                right_count_operand: 3,
            },
            MachineSizeKnowledge::EncoderResolved {
                minimum_bytes: 16,
                maximum_bytes: None,
            },
        ),
        MachineSemanticKind::LoadPacked3
        | MachineSemanticKind::LoadPacked5
        | MachineSemanticKind::LoadPacked6
        | MachineSemanticKind::LoadPacked7 => {
            let byte_count = match semantic {
                MachineSemanticKind::LoadPacked3 => 3,
                MachineSemanticKind::LoadPacked5 => 5,
                MachineSemanticKind::LoadPacked6 => 6,
                _ => 7,
            };
            (
                MachineMemoryEffect::ReadPointerV1,
                MachineTrapBehavior::MayArchitecturalFaultV1,
                vec![0],
                vec![1, 2],
                MachineEncodedMemoryEffect::ReadPointerV1 {
                    pointer_operand: 0,
                    byte_count,
                },
                MachineSizeKnowledge::EncoderResolved {
                    minimum_bytes: byte_count * 8 + (byte_count - 1) * 7,
                    maximum_bytes: Some(byte_count * 9 + (byte_count - 1) * 7),
                },
            )
        }
        MachineSemanticKind::StorePacked => (
            MachineMemoryEffect::WritePointerV1,
            MachineTrapBehavior::MayArchitecturalFaultV1,
            vec![0, 1],
            vec![2],
            MachineEncodedMemoryEffect::WritePointerV1 { pointer_operand: 0 },
            MachineSizeKnowledge::EncoderResolved {
                minimum_bytes: 32,
                maximum_bytes: Some(83),
            },
        ),
        MachineSemanticKind::Store => (
            MachineMemoryEffect::WritePointerV1,
            MachineTrapBehavior::MayArchitecturalFaultV1,
            vec![0, 1],
            vec![],
            MachineEncodedMemoryEffect::WritePointerV1 { pointer_operand: 0 },
            MachineSizeKnowledge::EncoderResolved {
                minimum_bytes: 7,
                maximum_bytes: Some(9),
            },
        ),
        MachineSemanticKind::AddressOffset => (
            MachineMemoryEffect::NoneV1,
            MachineTrapBehavior::NeverV1,
            vec![0],
            vec![1],
            MachineEncodedMemoryEffect::NoneV1,
            MachineSizeKnowledge::EncoderResolved {
                minimum_bytes: 7,
                maximum_bytes: Some(8),
            },
        ),
        MachineSemanticKind::Load8Indexed => (
            MachineMemoryEffect::ReadPointerV1,
            MachineTrapBehavior::MayArchitecturalFaultV1,
            vec![0, 1],
            vec![2],
            MachineEncodedMemoryEffect::ReadIndexedPointerV1 {
                pointer_operand: 0,
                index_operand: 1,
                byte_count: 1,
            },
            MachineSizeKnowledge::ExactBytes(9),
        ),
        MachineSemanticKind::Load8 | MachineSemanticKind::Load16 => (
            MachineMemoryEffect::ReadPointerV1,
            MachineTrapBehavior::MayArchitecturalFaultV1,
            vec![0],
            vec![1],
            MachineEncodedMemoryEffect::ReadPointerV1 {
                pointer_operand: 0,
                byte_count: if semantic == MachineSemanticKind::Load8 {
                    1
                } else {
                    2
                },
            },
            MachineSizeKnowledge::EncoderResolved {
                minimum_bytes: 8,
                maximum_bytes: Some(9),
            },
        ),
        MachineSemanticKind::Load32 | MachineSemanticKind::Load64 => (
            MachineMemoryEffect::ReadPointerV1,
            MachineTrapBehavior::MayArchitecturalFaultV1,
            vec![0],
            vec![1],
            MachineEncodedMemoryEffect::ReadPointerV1 {
                pointer_operand: 0,
                byte_count: if semantic == MachineSemanticKind::Load32 {
                    4
                } else {
                    8
                },
            },
            MachineSizeKnowledge::EncoderResolved {
                minimum_bytes: 7,
                maximum_bytes: Some(8),
            },
        ),
        MachineSemanticKind::Store64 => (
            MachineMemoryEffect::WriteFrameStorageV1,
            MachineTrapBehavior::MayArchitecturalFaultV1,
            vec![0],
            vec![1],
            MachineEncodedMemoryEffect::WriteFrameStorageV1 {
                stack_pointer,
                byte_count: 8,
            },
            MachineSizeKnowledge::EncoderResolved {
                minimum_bytes: 15,
                maximum_bytes: Some(31),
            },
        ),
        MachineSemanticKind::FrameAddress => (
            MachineMemoryEffect::NoneV1,
            MachineTrapBehavior::NeverV1,
            vec![],
            vec![0],
            MachineEncodedMemoryEffect::NoneV1,
            MachineSizeKnowledge::EncoderResolved {
                minimum_bytes: 8,
                maximum_bytes: Some(23),
            },
        ),
        MachineSemanticKind::CallUnit => (
            MachineMemoryEffect::NoneV1,
            MachineTrapBehavior::NeverV1,
            row.operands.iter().map(|operand| operand.operand).collect(),
            vec![],
            MachineEncodedMemoryEffect::WriteReturnAddressBelowStackPointerV1 {
                stack_pointer,
                byte_count: 8,
            },
            MachineSizeKnowledge::ExactBytes(5),
        ),
        _ => unreachable!("ordinary address/call declaration only"),
    };
    let is_call = semantic == MachineSemanticKind::CallUnit;
    MachineEffectDeclaration {
        semantic,
        constraint,
        memory,
        trap,
        barrier: if matches!(semantic, MachineSemanticKind::PlacedScalarAccess(_)) {
            MachineBarrier::ExternalEffect
        } else if is_call {
            MachineBarrier::Call
        } else {
            MachineBarrier::None
        },
        call: if is_call {
            MachineCallEffect::InternalNormalReturnV1 {
                pre_call_stack_alignment: 16,
            }
        } else {
            MachineCallEffect::NoneV1
        },
        cleanup: MachineCleanupEffect::NoneV1,
        alternatives: vec![MachineAlternative {
            key: MachineAlternativeKey {
                family: semantic.into(),
                variant: 0,
            },
            applicability: MachineAlternativeApplicability::Always,
            size,
            latency: MachineLatencyKnowledge::StableBaselineUnavailable,
            encoded: MachineEncodedEffects {
                external_operand_reads: reads,
                external_operand_writes: writes,
                implicit_unit_uses: row.implicit_uses.clone(),
                implicit_unit_defs: row.implicit_defs.clone(),
                implicit_unit_clobbers: row.clobbers.clone(),
                memory: encoded_memory,
                stack: if is_call {
                    MachineEncodedStackEffect::CallReturnAddressLifecycleV1 {
                        stack_pointer,
                        return_address_byte_count: 8,
                    }
                } else {
                    MachineEncodedStackEffect::UnchangedV1
                },
                trap: if matches!(
                    semantic,
                    MachineSemanticKind::FrameAddress | MachineSemanticKind::AddressOffset
                ) {
                    MachineEncodedTrapBehavior::NeverV1
                } else {
                    MachineEncodedTrapBehavior::MayArchitecturalFaultV1
                },
                control: if is_call {
                    MachineEncodedControlEffect::DirectRelativeCallV1
                } else {
                    MachineEncodedControlEffect::FallThroughV1
                },
            },
        }],
    }
}
