use omega_optimization_unit::EffectLink;
use omega_selected_instructions::{
    MachineCleanupEffect, MachineTrapBehavior, SelectedMicrosoftX64OwnedIndirectPairLayout,
    StructuralUnitCallBarrier, StructuralUnitCallEffect, StructuralUnitCallEffectDeclaration,
    StructuralUnitCallFrameEffect, StructuralUnitCallMemoryEffect,
};
use omega_target_operations::MachineRegister;

use crate::StructuralUnitCallMachineEffects;

use super::ownership::encode_ownership;
use super::provenance::encode_provenance;
use super::values::{encode_constraint_key, encode_len, encode_units};

pub(crate) fn encode_structural_call(bytes: &mut Vec<u8>, call: &StructuralUnitCallMachineEffects) {
    bytes.extend_from_slice(&call.instruction.0.to_le_bytes());
    bytes.extend_from_slice(&call.operation.get().to_le_bytes());
    bytes.extend_from_slice(&call.callee.get().to_le_bytes());
    encode_constraint_key(bytes, call.constraint);
    encode_units(bytes, &call.unit_uses);
    encode_units(bytes, &call.unit_defs);
    encode_units(bytes, &call.unit_clobbers);
    encode_layout(bytes, call.layout);
    encode_effect_link(bytes, call.effect);
    encode_ownership(bytes, &call.ownership);
    encode_len(bytes, call.claim_transfers.len());
    for transfer in &call.claim_transfers {
        bytes.extend_from_slice(&transfer.claim.get().to_le_bytes());
        bytes.extend_from_slice(&transfer.argument_index.to_le_bytes());
    }
    encode_provenance(bytes, &call.provenance);
    encode_declaration(bytes, call.declaration);
}

pub(crate) fn encode_effect_link(bytes: &mut Vec<u8>, effect: EffectLink) {
    bytes.extend_from_slice(&effect.input.to_le_bytes());
    bytes.extend_from_slice(&effect.output.to_le_bytes());
}

fn encode_layout(bytes: &mut Vec<u8>, layout: SelectedMicrosoftX64OwnedIndirectPairLayout) {
    bytes.extend_from_slice(&layout.shadow_byte_count.to_le_bytes());
    bytes.extend_from_slice(&layout.outgoing_frame_byte_count.to_le_bytes());
    bytes.extend_from_slice(&layout.pre_call_stack_alignment.to_le_bytes());
    for binding in layout.bindings {
        bytes.extend_from_slice(&(binding.parameter_index as u64).to_le_bytes());
        encode_machine_register(bytes, binding.pointer);
        bytes.extend_from_slice(&binding.copy_stack_byte_offset.to_le_bytes());
        bytes.extend_from_slice(&binding.byte_count.to_le_bytes());
        bytes.extend_from_slice(&binding.alignment.to_le_bytes());
    }
}

fn encode_machine_register(bytes: &mut Vec<u8>, register: MachineRegister) {
    use MachineRegister as R;

    let tag = match register {
        R::X86Rax => 0,
        R::X86Rcx => 1,
        R::X86Rdx => 2,
        R::X86Rbx => 3,
        R::X86Rsp => 4,
        R::X86Rbp => 5,
        R::X86Rsi => 6,
        R::X86Rdi => 7,
        R::X86R8 => 8,
        R::X86R9 => 9,
        R::X86R10 => 10,
        R::X86R11 => 11,
        R::X86R12 => 12,
        R::X86R13 => 13,
        R::X86R14 => 14,
        R::X86R15 => 15,
        R::X86Xmm(index) => {
            bytes.push(16);
            bytes.push(index);
            return;
        }
        R::Aarch64X(index) => {
            bytes.push(17);
            bytes.push(index);
            return;
        }
        R::Aarch64V(index) => {
            bytes.push(18);
            bytes.push(index);
            return;
        }
    };
    bytes.push(tag);
}

fn encode_declaration(bytes: &mut Vec<u8>, declaration: StructuralUnitCallEffectDeclaration) {
    encode_constraint_key(bytes, declaration.constraint);
    match declaration.memory {
        StructuralUnitCallMemoryEffect::ReadOwnedIndirectPairWriteCallerCopiesV1 {
            root_byte_count,
            copy_stack_byte_offsets,
        } => {
            bytes.push(1);
            bytes.extend_from_slice(&root_byte_count.to_le_bytes());
            for offset in copy_stack_byte_offsets {
                bytes.extend_from_slice(&offset.to_le_bytes());
            }
        }
    }
    match declaration.frame {
        StructuralUnitCallFrameEffect::BalancedCallerFrameV1 {
            frame_byte_count,
            shadow_byte_count,
            pre_call_stack_alignment,
        } => {
            bytes.push(1);
            bytes.extend_from_slice(&frame_byte_count.to_le_bytes());
            bytes.extend_from_slice(&shadow_byte_count.to_le_bytes());
            bytes.extend_from_slice(&pre_call_stack_alignment.to_le_bytes());
        }
    }
    bytes.push(match declaration.trap {
        MachineTrapBehavior::NeverV1 => 0,
        MachineTrapBehavior::MayArchitecturalFaultV1 => 1,
    });
    bytes.push(match declaration.barrier {
        StructuralUnitCallBarrier::CallV1 => 1,
    });
    bytes.push(match declaration.call {
        StructuralUnitCallEffect::DirectInternalUnitV1 => 1,
    });
    bytes.push(match declaration.cleanup {
        MachineCleanupEffect::NoneV1 => 0,
    });
}
