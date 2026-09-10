use crate::{
    MachineBarrier, MachineCallEffect, MachineCleanupEffect, MachineMemoryEffect,
    MachineTrapBehavior, SelectedInstructionKind,
};

use crate::InstructionMachineEffects;

use super::alternative::encode_alternative;
use super::provenance::encode_provenance;
use super::values::{encode_constraint_key, encode_len, encode_units};

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
        MachineMemoryEffect::NoneV1 => 0,
        MachineMemoryEffect::ReadPointerV1 => 1,
        MachineMemoryEffect::HostedReadByteV1 => 5,
        MachineMemoryEffect::HostedWriteByteV1 => 3,
        MachineMemoryEffect::WriteFrameStorageV1 => 2,
        MachineMemoryEffect::WritePointerV1 => 4,
    });
    encode_effect_tail(bytes, instruction);
}

fn encode_effect_tail(bytes: &mut Vec<u8>, instruction: &InstructionMachineEffects) {
    bytes.push(match instruction.trap {
        MachineTrapBehavior::NeverV1 => 0,
        MachineTrapBehavior::HostedExitReturnedV1 => 3,
        MachineTrapBehavior::HostedReadFailureV1 => 4,
        MachineTrapBehavior::HostedWriteFailureV1 => 2,
        MachineTrapBehavior::MayArchitecturalFaultV1 => 1,
    });
    encode_barrier(bytes, instruction.barrier);
    match instruction.call {
        MachineCallEffect::NoneV1 => bytes.push(0),
        MachineCallEffect::DirectInternalNormalReturnV1 {
            pre_call_stack_alignment,
        } => {
            bytes.push(1);
            bytes.extend_from_slice(&pre_call_stack_alignment.to_le_bytes());
        }
    }
    bytes.push(match instruction.cleanup {
        MachineCleanupEffect::NoneV1 => 0,
    });
    encode_provenance_and_alternatives(bytes, instruction);
}

fn encode_common_fields(bytes: &mut Vec<u8>, instruction: &InstructionMachineEffects) {
    bytes.extend_from_slice(&instruction.instruction.0.to_le_bytes());
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
        encode_alternative(bytes, alternative);
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

fn encode_kind(bytes: &mut Vec<u8>, kind: SelectedInstructionKind) {
    bytes.push(match kind {
        SelectedInstructionKind::Store { .. } => 24,
        SelectedInstructionKind::AddressOffset { .. } => 25,
        SelectedInstructionKind::CompareI64Zero => 0,
        SelectedInstructionKind::MaterializeI64 { .. } => 1,
        SelectedInstructionKind::CopyI64 => 2,
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
        SelectedInstructionKind::ExactAddI64 { .. } => 3,
        SelectedInstructionKind::ExactAddI64Immediate { .. } => 4,
        SelectedInstructionKind::ExactSubtractI64 { .. } => 5,
        SelectedInstructionKind::ConditionalBranchNonZero => 6,
        SelectedInstructionKind::ReturnI64 => 7,
        SelectedInstructionKind::CallAggregate { .. } => 35,
        SelectedInstructionKind::ReturnAggregate { .. } => 36,
        SelectedInstructionKind::ExactSubtractI64Immediate { .. } => 8,
        SelectedInstructionKind::ReturnUnit => 9,
        SelectedInstructionKind::CompareI64 => 10,
        SelectedInstructionKind::ConditionalBranchU64LessThan => 11,
        SelectedInstructionKind::ConditionalBranchI64LessThan => 12,
        SelectedInstructionKind::CallI64 { .. } => 13,
        SelectedInstructionKind::Jump => 14,
        SelectedInstructionKind::Load64 { .. } => 16,
        SelectedInstructionKind::Load8 { .. } => 33,
        SelectedInstructionKind::Load16 { .. } => 34,
        SelectedInstructionKind::Load32 { .. } => 30,
        SelectedInstructionKind::HostedExitProcessI32 => 31,
        SelectedInstructionKind::HostedReadByte { .. } => 32,
        SelectedInstructionKind::HostedWriteByteI32 { .. } => 23,
        SelectedInstructionKind::ByteViewAddress => 22,
        SelectedInstructionKind::Load8Indexed => 21,
        SelectedInstructionKind::Store64 { .. } => 17,
        SelectedInstructionKind::FrameAddress { .. } => 18,
        SelectedInstructionKind::CallUnit { .. } => 19,
    });
    match kind {
        SelectedInstructionKind::Store {
            byte_offset,
            byte_size,
        } => {
            bytes.extend_from_slice(&byte_offset.to_le_bytes());
            bytes.push(byte_size);
        }
        SelectedInstructionKind::AddressOffset { byte_offset } => {
            bytes.extend_from_slice(&byte_offset.to_le_bytes());
        }
        SelectedInstructionKind::Load64 { byte_offset }
        | SelectedInstructionKind::Load8 { byte_offset }
        | SelectedInstructionKind::Load16 { byte_offset }
        | SelectedInstructionKind::Load32 { byte_offset } => {
            bytes.extend_from_slice(&byte_offset.to_le_bytes())
        }
        SelectedInstructionKind::Store64 { slot, byte_offset }
        | SelectedInstructionKind::FrameAddress { slot, byte_offset } => {
            match slot {
                crate::FrameStorageSlotId::Incoming {
                    parameter_index,
                    abi_stack_byte_offset,
                } => {
                    bytes.push(2);
                    bytes.extend_from_slice(&parameter_index.to_le_bytes());
                    bytes.extend_from_slice(&abi_stack_byte_offset.to_le_bytes());
                }
                crate::FrameStorageSlotId::Outgoing(slot) => {
                    bytes.push(0);
                    bytes.extend_from_slice(&slot.operation.get().to_le_bytes());
                    bytes.extend_from_slice(&slot.argument_index.to_le_bytes());
                }
                crate::FrameStorageSlotId::Local(slot) => {
                    bytes.push(1);
                    slot.encode_identity(bytes);
                }
            }
            bytes.extend_from_slice(&byte_offset.to_le_bytes());
        }
        SelectedInstructionKind::HostedReadByte { slot }
        | SelectedInstructionKind::HostedWriteByteI32 { slot } => slot.encode_identity(bytes),
        SelectedInstructionKind::MaterializeI64 { value } => encode_integer(bytes, value),
        SelectedInstructionKind::ExactAddI64 {
            obligation,
            accepted_fact,
        }
        | SelectedInstructionKind::ExactSubtractI64 {
            obligation,
            accepted_fact,
        } => {
            bytes.extend_from_slice(&obligation.get().to_le_bytes());
            bytes.extend_from_slice(&accepted_fact.bytes());
        }
        SelectedInstructionKind::ExactAddI64Immediate {
            immediate,
            obligation,
            accepted_fact,
        }
        | SelectedInstructionKind::ExactSubtractI64Immediate {
            immediate,
            obligation,
            accepted_fact,
        } => {
            encode_integer(bytes, immediate);
            bytes.extend_from_slice(&obligation.get().to_le_bytes());
            bytes.extend_from_slice(&accepted_fact.bytes());
        }
        SelectedInstructionKind::CompareI64Zero
        | SelectedInstructionKind::CompareI64
        | SelectedInstructionKind::CopyI64
        | SelectedInstructionKind::Float32ToBits
        | SelectedInstructionKind::Float64ToBits
        | SelectedInstructionKind::BitsToFloat32
        | SelectedInstructionKind::BitsToFloat64
        | SelectedInstructionKind::ZeroExtendU8
        | SelectedInstructionKind::ZeroExtendU32
        | SelectedInstructionKind::ZeroExtendU16
        | SelectedInstructionKind::SignExtendI8
        | SelectedInstructionKind::SignExtendI16
        | SelectedInstructionKind::SignExtendI32
        | SelectedInstructionKind::ByteViewAddress
        | SelectedInstructionKind::Load8Indexed
        | SelectedInstructionKind::ConditionalBranchNonZero
        | SelectedInstructionKind::ConditionalBranchU64LessThan
        | SelectedInstructionKind::ConditionalBranchI64LessThan
        | SelectedInstructionKind::Jump
        | SelectedInstructionKind::HostedExitProcessI32
        | SelectedInstructionKind::ReturnI64
        | SelectedInstructionKind::ReturnUnit => {}
        SelectedInstructionKind::ReturnAggregate { fragment_count } => bytes.push(fragment_count),
        SelectedInstructionKind::CallI64 { callee }
        | SelectedInstructionKind::CallAggregate { callee }
        | SelectedInstructionKind::CallUnit { callee } => {
            bytes.extend_from_slice(&callee.get().to_le_bytes());
        }
    }
}

fn encode_integer(bytes: &mut Vec<u8>, value: semantic_vocabulary::IntegerValue) {
    match value {
        semantic_vocabulary::IntegerValue::Signed(value) => {
            bytes.push(0);
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        semantic_vocabulary::IntegerValue::Unsigned(value) => {
            bytes.push(1);
            bytes.extend_from_slice(&value.to_le_bytes());
        }
    }
}
