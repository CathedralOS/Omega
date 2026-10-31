use crate::{
    MachineBarrier, MachineCallEffect, MachineCleanupEffect, MachineMemoryEffect,
    MachineTrapBehavior, SelectedInstructionKind,
};

use crate::{
    InstructionMachineEffects, SaturatingOperation, saturating_family_tag, trapping_family_tag,
};

use super::provenance::encode_provenance;
use super::values::encode_units;
use crate::codec_bytes::encode_len;
use register_model::encode_constraint_key;

/// Encodes an ordinary CFG instruction, retaining the complete semantic
/// effect row. Scalar calls make these fields nontrivial.
pub(super) fn encode_cfg_instruction(bytes: &mut Vec<u8>, instruction: &InstructionMachineEffects) {
    encode_ordinary_instruction(bytes, instruction);
}

/// Encodes the structural-unit return row, retaining every modeled effect
/// rather than relying on the ordinary-CFG fixed-value contract.
pub(super) fn encode_ordinary_instruction(
    bytes: &mut Vec<u8>,
    instruction: &InstructionMachineEffects,
) {
    encode_common_fields(bytes, instruction);
    bytes.push(match instruction.memory {
        MachineMemoryEffect::CopyBytesV1 => 6,
        MachineMemoryEffect::AtomicFenceV1 => 15,
        MachineMemoryEffect::CatalogFenceV1 => 16,
        MachineMemoryEffect::AtomicPointerV1 => 8,
        MachineMemoryEffect::TemporaryStackStorageV1 => 9,
        MachineMemoryEffect::NoneV1 => 0,
        MachineMemoryEffect::ReadPointerV1 => 1,
        MachineMemoryEffect::HostedReadByteV1 => 5,
        MachineMemoryEffect::HostedWriteByteV1 => 3,
        MachineMemoryEffect::WriteFrameStorageV1 => 2,
        MachineMemoryEffect::ReadFrameStorageV1 => 7,
        MachineMemoryEffect::WritePointerV1 => 4,
        MachineMemoryEffect::ScanByteSequenceV1 => 10,
        MachineMemoryEffect::ReadEncodedScalarV1 => 13,
        MachineMemoryEffect::WriteEncodedScalarV1 => 14,
        MachineMemoryEffect::CompareByteSequencesV1 => 11,
        MachineMemoryEffect::ZeroBytesV1 => 12,
    });
    encode_effect_tail(bytes, instruction);
}

fn encode_effect_tail(bytes: &mut Vec<u8>, instruction: &InstructionMachineEffects) {
    bytes.push(match instruction.trap {
        MachineTrapBehavior::ExplicitCrashV1 => 5,
        MachineTrapBehavior::TrappingIntegerV1 => 6,
        MachineTrapBehavior::NeverV1 => 0,
        MachineTrapBehavior::HostedExitReturnedV1 => 3,
        MachineTrapBehavior::HostedReadFailureV1 => 4,
        MachineTrapBehavior::HostedWriteFailureV1 => 2,
        MachineTrapBehavior::MayArchitecturalFaultV1 => 1,
    });
    encode_barrier(bytes, instruction.barrier);
    match instruction.call {
        MachineCallEffect::NoneV1 => bytes.push(0),
        MachineCallEffect::SupervisorNormalReturnV1 => bytes.push(3),
        MachineCallEffect::InternalNoNormalV1 {
            pre_call_stack_alignment,
        } => {
            bytes.push(4);
            crate::codec_bytes::push_encoded(bytes, pre_call_stack_alignment.to_le_bytes());
        }
        MachineCallEffect::InternalNormalReturnV1 {
            pre_call_stack_alignment,
        } => {
            bytes.push(1);
            crate::codec_bytes::push_encoded(bytes, pre_call_stack_alignment.to_le_bytes());
        }
        MachineCallEffect::ExternalNormalReturnV1 {
            pre_call_stack_alignment,
        } => {
            bytes.push(2);
            crate::codec_bytes::push_encoded(bytes, pre_call_stack_alignment.to_le_bytes());
        }
    }
    bytes.push(match instruction.cleanup {
        MachineCleanupEffect::NoneV1 => 0,
    });
    encode_provenance_and_alternatives(bytes, instruction);
}

fn encode_common_fields(bytes: &mut Vec<u8>, instruction: &InstructionMachineEffects) {
    crate::codec_bytes::push_encoded(bytes, instruction.instruction.0.to_le_bytes());
    encode_kind(bytes, instruction.kind);
    encode_constraint_key(bytes, instruction.constraint);
    encode_units(bytes, &instruction.unit_uses);
    encode_units(bytes, &instruction.unit_defs);
    encode_units(bytes, &instruction.unit_clobbers);
}

fn encode_provenance_and_alternatives(
    bytes: &mut Vec<u8>,
    instruction: &InstructionMachineEffects,
) {
    encode_provenance(bytes, &instruction.provenance);
    encode_len(bytes, instruction.alternatives.len());
    for alternative in &instruction.alternatives {
        crate::encode_machine_alternative_identity(bytes, alternative);
    }
}

fn encode_barrier(bytes: &mut Vec<u8>, barrier: MachineBarrier) {
    bytes.push(match barrier {
        MachineBarrier::None => 0,
        MachineBarrier::ControlFlow => 1,
        MachineBarrier::ExternalEffect => 3,
        MachineBarrier::Call => 2,
    });
}

/// This envelope's own kind tags. The payload each tag introduces is shared by
/// every envelope through [`crate::kind_encoding`].
pub(crate) fn encode_kind(bytes: &mut Vec<u8>, kind: SelectedInstructionKind) {
    let tag = match kind {
        SelectedInstructionKind::Store { .. } => 24,
        SelectedInstructionKind::AddressOffset { .. } => 25,
        SelectedInstructionKind::CompareI64Zero => 0,
        SelectedInstructionKind::MaterializeI64 { .. } => 1,
        SelectedInstructionKind::CopyBytes => 59,
        // The catalog's semantic tags: 95 + carrier ordinal belongs to the
        // saturating remainder forms, so these cannot reuse 96 and 97.
        SelectedInstructionKind::ScanByteSequence { .. } => 233,
        SelectedInstructionKind::LoadEncodedScalar => 236,
        SelectedInstructionKind::StoreEncodedScalar => 237,
        SelectedInstructionKind::CompareByteSequences => 234,
        SelectedInstructionKind::ZeroBytes => 235,
        SelectedInstructionKind::CopyI64 => 2,
        SelectedInstructionKind::BitwiseAndI64 => 51,
        SelectedInstructionKind::BitwiseXorI64 => 52,
        SelectedInstructionKind::ExactDivideU64 { .. } => 56,
        SelectedInstructionKind::WrappingRemainderI64 { .. } => 57,
        SelectedInstructionKind::WrappingAddI64 => 58,
        SelectedInstructionKind::SaturatingAdd { carrier } => {
            saturating_family_tag(SaturatingOperation::Add, carrier)
        }
        SelectedInstructionKind::SaturatingSubtract { carrier } => {
            saturating_family_tag(SaturatingOperation::Subtract, carrier)
        }
        SelectedInstructionKind::SaturatingDivide { carrier, .. } => {
            saturating_family_tag(SaturatingOperation::Divide, carrier)
        }
        SelectedInstructionKind::SaturatingRemainder { carrier, .. } => {
            saturating_family_tag(SaturatingOperation::Remainder, carrier)
        }
        SelectedInstructionKind::SaturatingMultiply { carrier } => {
            saturating_family_tag(SaturatingOperation::Multiply, carrier)
        }
        SelectedInstructionKind::TrappingInteger { form } => trapping_family_tag(form),
        SelectedInstructionKind::PortWrite { .. } => 239,
        SelectedInstructionKind::RegisterAccess { .. } => 247,
        SelectedInstructionKind::AtomicFence => 238,
        SelectedInstructionKind::AtomicAccess { form } => form.machine_tag(),
        SelectedInstructionKind::AccessorScalarAccess { form, .. } => form.machine_tag(),
        SelectedInstructionKind::NearestIeeeFloatArithmetic { form } => form.machine_tag(),
        SelectedInstructionKind::FloatResultPolicy { form } => form.machine_tag(),
        SelectedInstructionKind::Float32ToBits => 26,
        SelectedInstructionKind::Float64ToBits => 27,
        SelectedInstructionKind::BitsToFloat32 => 28,
        SelectedInstructionKind::BitsToFloat64 => 29,
        SelectedInstructionKind::ZeroExtendU8 => 15,
        SelectedInstructionKind::ZeroExtendU32 => 20,
        SelectedInstructionKind::ZeroExtendU16 => 37,
        SelectedInstructionKind::SignExtendI8 => 38,
        SelectedInstructionKind::SignExtendI16 => 39,
        SelectedInstructionKind::SignExtendI32 => 40,
        SelectedInstructionKind::MaterializeBooleanEqual => 41,
        SelectedInstructionKind::MaterializeBooleanU64LessThan => 42,
        SelectedInstructionKind::MaterializeBooleanI64LessThan => 43,
        SelectedInstructionKind::MaterializeBooleanU64LessOrEqual => 44,
        SelectedInstructionKind::MaterializeBooleanI64LessOrEqual => 45,
        SelectedInstructionKind::ExactAddI64 { .. } => 3,
        SelectedInstructionKind::ExactAddI64Immediate { .. } => 4,
        SelectedInstructionKind::ExactSubtractI64 { .. } => 5,
        SelectedInstructionKind::ConditionalBranchNonZero => 6,
        SelectedInstructionKind::ReturnScalar => 7,
        SelectedInstructionKind::CallAggregate { .. } => 35,
        SelectedInstructionKind::ReturnAggregate { .. } => 36,
        SelectedInstructionKind::ExactSubtractI64Immediate { .. } => 8,
        SelectedInstructionKind::ReturnUnit => 9,
        SelectedInstructionKind::CompareI64 => 10,
        SelectedInstructionKind::CompareI64Immediate { .. } => 53,
        SelectedInstructionKind::ConditionalBranchU64LessThan => 11,
        SelectedInstructionKind::ConditionalBranchI64LessThan => 12,
        SelectedInstructionKind::CallScalar { .. } => 13,
        SelectedInstructionKind::NormalizedForeignCall { .. } => 87,
        SelectedInstructionKind::ExactMultiplyI64 { .. } => 88,
        SelectedInstructionKind::ExactRemainderU64 { .. } => 89,
        SelectedInstructionKind::WrappingSubtractI64 => 90,
        SelectedInstructionKind::WrappingMultiplyI64 => 91,
        SelectedInstructionKind::WrappingDivideI64 { .. } => 92,
        SelectedInstructionKind::ExactDivideI64 { .. } => 112,
        SelectedInstructionKind::ExactRemainderI64 { .. } => 113,
        SelectedInstructionKind::BitwiseOrI64 => 93,
        SelectedInstructionKind::BitwiseNotI64 => 94,
        SelectedInstructionKind::SaveFloatingControl { .. } => 103,
        SelectedInstructionKind::RestoreFloatingControl { .. } => 104,
        SelectedInstructionKind::WrappingShiftLeftI64 => 105,
        SelectedInstructionKind::WrappingShiftRightI64 => 106,
        SelectedInstructionKind::WrappingShiftRightU64 => 107,
        SelectedInstructionKind::ExactShiftLeftI64 { .. } => 108,
        SelectedInstructionKind::ExactShiftRightI64 { .. } => 109,
        SelectedInstructionKind::ExactShiftRightU64 { .. } => 110,
        SelectedInstructionKind::Jump => 14,
        SelectedInstructionKind::Load64 { .. } => 16,
        SelectedInstructionKind::LoadPacked { .. } => 46,
        SelectedInstructionKind::StorePacked { .. } => 47,
        SelectedInstructionKind::Load8 { .. } => 33,
        SelectedInstructionKind::Load16 { .. } => 34,
        SelectedInstructionKind::Load32 { .. } => 30,
        SelectedInstructionKind::Crash => 111,
        SelectedInstructionKind::HostedExitProcessI32 => 31,
        SelectedInstructionKind::HostedReadByte { .. } => 32,
        SelectedInstructionKind::HostedWriteByteI32 { .. } => 23,
        SelectedInstructionKind::ByteViewAddress => 22,
        SelectedInstructionKind::Load8Indexed => 21,
        SelectedInstructionKind::Store64 { .. } => 17,
        SelectedInstructionKind::FrameAddress { .. } => 18,
        SelectedInstructionKind::InvokeNoNormal { .. } => 248,
        SelectedInstructionKind::CallUnit { .. } => 19,
        SelectedInstructionKind::ImageAddress { .. } => 232,
    };
    crate::kind_encoding::encode_kind(bytes, tag, kind);
}

#[cfg(test)]
mod outgoing_slot_tests {
    use super::{SelectedInstructionKind, encode_kind};
    use crate::{FrameStorageSlotId, OutgoingArgumentSlotId, OutgoingArgumentSlotRole};
    use semantic_vocabulary::OperationId;

    #[test]
    fn outgoing_frame_roles_preserve_argument_bytes_and_use_a_distinct_copy_tag() {
        let argument = OutgoingArgumentSlotId {
            invocation_site: (OperationId::new(43).unwrap()).into(),
            argument_index: 7,
            role: OutgoingArgumentSlotRole::Argument,
        };
        let copy = OutgoingArgumentSlotId {
            role: OutgoingArgumentSlotRole::ValueCopy,
            ..argument
        };
        for (tag, argument_kind, copy_kind) in [
            (
                17,
                SelectedInstructionKind::Store64 {
                    slot: FrameStorageSlotId::Outgoing(argument),
                    byte_offset: 8,
                },
                SelectedInstructionKind::Store64 {
                    slot: FrameStorageSlotId::Outgoing(copy),
                    byte_offset: 8,
                },
            ),
            (
                18,
                SelectedInstructionKind::FrameAddress {
                    slot: FrameStorageSlotId::Outgoing(argument),
                    byte_offset: 8,
                },
                SelectedInstructionKind::FrameAddress {
                    slot: FrameStorageSlotId::Outgoing(copy),
                    byte_offset: 8,
                },
            ),
        ] {
            let mut legacy = vec![tag, 0, 0];
            crate::codec_bytes::push_encoded(&mut legacy, 43_u64.to_le_bytes());
            crate::codec_bytes::push_encoded(&mut legacy, 7_u32.to_le_bytes());
            crate::codec_bytes::push_encoded(&mut legacy, 8_u32.to_le_bytes());
            let mut encoded = Vec::new();
            encode_kind(&mut encoded, argument_kind);
            assert_eq!(encoded, legacy);
            encoded.clear();
            encode_kind(&mut encoded, copy_kind);
            legacy[1] = 3;
            assert_eq!(encoded, legacy);
        }
    }
}

#[cfg(test)]
mod float_result_policy_tests {
    use super::encode_kind;
    use crate::{FloatResultPolicyForm, SelectedInstructionKind};

    #[test]
    fn float_result_policy_instruction_kind_pins_both_identity_bytes() {
        for form in FloatResultPolicyForm::ALL {
            let mut bytes = Vec::new();
            encode_kind(
                &mut bytes,
                SelectedInstructionKind::FloatResultPolicy { form },
            );
            assert_eq!(bytes, [231, form.ordinal()]);
        }
    }
}

#[cfg(test)]
mod port_write_tests {
    use super::*;
    #[test]
    fn port_write_identity_retains_both_operands_and_an_append_only_tag() {
        for (port, value) in [(0, 0), (0x3f8, 0x41), (u16::MAX, u8::MAX)] {
            let mut bytes = Vec::new();
            encode_kind(
                &mut bytes,
                SelectedInstructionKind::PortWrite { port, value },
            );
            let [low, high] = port.to_le_bytes();
            assert_eq!(bytes, [239, low, high, value]);
        }
    }
}
