use omega_selected_instructions::{
    MachineBarrier, MachineCallEffect, MachineCleanupEffect, MachineMemoryEffect,
    MachineTrapBehavior, SelectedInstructionKind,
};

use crate::InstructionMachineEffects;

use super::alternative::encode_alternative;
use super::provenance::encode_provenance;
use super::values::{encode_constraint_key, encode_len, encode_units};

/// Encodes an ordinary CFG instruction. At this analysis boundary memory,
/// trap, call, and cleanup are fixed protocol values; only the barrier varies.
pub(super) fn encode_cfg_instruction(bytes: &mut Vec<u8>, instruction: &InstructionMachineEffects) {
    encode_common_fields(bytes, instruction);
    bytes.push(0); // memory: NoneV1
    bytes.push(0); // trap: NeverV1
    encode_barrier(bytes, instruction.barrier);
    bytes.push(0); // call: NoneV1
    bytes.push(0); // cleanup: NoneV1
    encode_provenance_and_alternatives(bytes, instruction);
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
    });
    bytes.push(match instruction.trap {
        MachineTrapBehavior::NeverV1 => 0,
        MachineTrapBehavior::MayArchitecturalFaultV1 => 1,
    });
    encode_barrier(bytes, instruction.barrier);
    bytes.push(match instruction.call {
        MachineCallEffect::NoneV1 => 0,
    });
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
    });
}

fn encode_kind(bytes: &mut Vec<u8>, kind: SelectedInstructionKind) {
    bytes.push(match kind {
        SelectedInstructionKind::CompareI64Zero => 0,
        SelectedInstructionKind::MaterializeI64 { .. } => 1,
        SelectedInstructionKind::CopyI64 => 2,
        SelectedInstructionKind::ExactAddI64 { .. } => 3,
        SelectedInstructionKind::ExactAddI64Immediate { .. } => 4,
        SelectedInstructionKind::ExactSubtractI64 { .. } => 5,
        SelectedInstructionKind::ConditionalBranchNonZero => 6,
        SelectedInstructionKind::ReturnI64 => 7,
        SelectedInstructionKind::ExactSubtractI64Immediate { .. } => 8,
        SelectedInstructionKind::ReturnUnit => 9,
    });
    match kind {
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
        | SelectedInstructionKind::CopyI64
        | SelectedInstructionKind::ConditionalBranchNonZero
        | SelectedInstructionKind::ReturnI64
        | SelectedInstructionKind::ReturnUnit => {}
    }
}

fn encode_integer(bytes: &mut Vec<u8>, value: psi_core::IntegerValue) {
    match value {
        psi_core::IntegerValue::Signed(value) => {
            bytes.push(0);
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        psi_core::IntegerValue::Unsigned(value) => {
            bytes.push(1);
            bytes.extend_from_slice(&value.to_le_bytes());
        }
    }
}
