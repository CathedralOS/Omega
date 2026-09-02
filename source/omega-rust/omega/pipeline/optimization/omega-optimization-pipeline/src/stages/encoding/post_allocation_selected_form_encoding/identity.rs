use omega_calling_conventions::MachineRegister;
use omega_isa_x86_64::{
    X86_64SelectedStructuralUnitCallFootprint, X86_64StructuralUnitInternalControlFixup,
};
use omega_machine_optimizer::PostAllocationMachineIdentity;
use omega_register_model::{RegisterUnitId, RegisterViewId};
use omega_selected_instructions::{
    MachineAlternativeFamily, MachineAlternativeKey, MachineEncodedEffects,
};
use sha2::{Digest, Sha256};

use crate::PostAllocationMachineOptimizationCustody;

use super::{
    DeferredControlEncodingReason, SelectedFormEncodingCounts, SelectedFormEncodingIdentity,
    SelectedFormEncodingRow, SelectedFormEncodingState, SelectedFormMachineDisposition,
    SelectedStructuralUnitFunctionEncoding,
};

const ENCODER_SCHEMA: &[u8] = b"omega.terminal.layout-independent-selected-form-encoding.v8";

pub(super) fn encoding_identity(
    selected: omega_selected_instructions::SelectedInstructionPlanIdentity,
    machine: PostAllocationMachineIdentity,
    post_allocation_machine_optimization: Option<PostAllocationMachineOptimizationCustody>,
    rows: &[SelectedFormEncodingRow],
    structural_unit_functions: &[SelectedStructuralUnitFunctionEncoding],
    counts: SelectedFormEncodingCounts,
) -> SelectedFormEncodingIdentity {
    let mut hasher = Sha256::new();
    hasher.update(ENCODER_SCHEMA);
    hasher.update(selected.bytes());
    hasher.update(machine.bytes());
    match post_allocation_machine_optimization {
        None => hasher.update([0]),
        Some(custody) => {
            hasher.update([1]);
            hasher.update([custody.optimization() as u8]);
            hasher.update(custody.artifact_identity());
            hasher.update(custody.selections().bytes());
            hasher.update(custody.post_allocation_machine_selections().bytes());
            hasher.update(custody.source().bytes());
            hasher.update((custody.action_count() as u64).to_le_bytes());
            hasher.update(custody.baseline_bytes().to_le_bytes());
            hasher.update(custody.selected_bytes().to_le_bytes());
        }
    }
    hasher.update((rows.len() as u64).to_le_bytes());
    for row in rows {
        encode_encoding_row(&mut hasher, row);
    }
    hasher.update((structural_unit_functions.len() as u64).to_le_bytes());
    for function in structural_unit_functions {
        hasher.update(function.machine.get().to_le_bytes());
        hasher.update(function.block.0.to_le_bytes());
        match &function.call {
            None => hasher.update([0]),
            Some(call) => {
                hasher.update([1]);
                hasher.update(call.instruction.0.to_le_bytes());
                hasher.update(call.operation.get().to_le_bytes());
                hasher.update(call.callee.get().to_le_bytes());
                hasher.update((call.bytes.len() as u64).to_le_bytes());
                hasher.update(&call.bytes);
                encode_structural_footprint(&mut hasher, &call.footprint);
                encode_structural_fixup(&mut hasher, call.fixup);
            }
        }
        encode_encoding_row(&mut hasher, &function.return_instruction);
    }
    encode_counts(&mut hasher, counts);
    SelectedFormEncodingIdentity(hasher.finalize().into())
}

fn encode_encoding_row(hasher: &mut Sha256, row: &SelectedFormEncodingRow) {
    hasher.update(row.instruction.0.to_le_bytes());
    encode_alternative(hasher, row.alternative);
    encode_machine_disposition(hasher, &row.machine_disposition);
    match &row.state {
        SelectedFormEncodingState::Encoded { bytes, footprint } => {
            hasher.update([0]);
            hasher.update((bytes.len() as u64).to_le_bytes());
            hasher.update(bytes);
            encode_views(hasher, &footprint.register_reads);
            encode_views(hasher, &footprint.register_writes);
            encode_units(hasher, &footprint.implicit_defs);
            encode_units(hasher, &footprint.implicit_clobbers);
            encode_effects(hasher, &footprint.encoded);
        }
        SelectedFormEncodingState::DeferredControl { reason } => {
            hasher.update([1]);
            hasher.update([match reason {
                DeferredControlEncodingReason::RequiresResolvedBranchLayout => 0,
            }]);
        }
    }
}

fn encode_structural_footprint(
    hasher: &mut Sha256,
    footprint: &X86_64SelectedStructuralUnitCallFootprint,
) {
    encode_units(hasher, &footprint.implicit_unit_uses);
    encode_units(hasher, &footprint.implicit_unit_defs);
    encode_units(hasher, &footprint.implicit_unit_clobbers);
    for read in footprint.root_reads {
        encode_machine_register(hasher, read.root);
        hasher.update(read.byte_offset.to_le_bytes());
        hasher.update(read.byte_count.to_le_bytes());
    }
    for write in footprint.caller_copy_writes {
        hasher.update(write.stack_byte_offset.to_le_bytes());
        hasher.update(write.byte_count.to_le_bytes());
    }
    for register in footprint.scratch_register_writes {
        encode_machine_register(hasher, register);
    }
    for write in footprint.argument_pointer_writes {
        encode_machine_register(hasher, write.register);
        hasher.update(write.stack_byte_offset.to_le_bytes());
    }
    hasher.update([u8::from(footprint.writes_rflags)]);
    hasher.update(footprint.frame_byte_count.to_le_bytes());
    hasher.update(footprint.shadow_byte_count.to_le_bytes());
    hasher.update(footprint.pre_call_stack_alignment.to_le_bytes());
    hasher.update([u8::from(footprint.frame_is_balanced)]);
    hasher.update([match footprint.trap {
        omega_selected_instructions::MachineTrapBehavior::NeverV1 => 0,
        omega_selected_instructions::MachineTrapBehavior::MayArchitecturalFaultV1 => 1,
    }]);
    hasher.update([match footprint.barrier {
        omega_selected_instructions::StructuralUnitCallBarrier::CallV1 => 0,
    }]);
    hasher.update([match footprint.call {
        omega_selected_instructions::StructuralUnitCallEffect::DirectInternalUnitV1 => 0,
    }]);
    hasher.update([match footprint.cleanup {
        omega_selected_instructions::MachineCleanupEffect::NoneV1 => 0,
    }]);
}

fn encode_structural_fixup(hasher: &mut Sha256, fixup: X86_64StructuralUnitInternalControlFixup) {
    hasher.update([match fixup.kind {
        omega_isa_x86_64::X86_64StructuralUnitInternalControlFixupKind::Relative32FromNextInstructionToInternalMachineV1 => 0,
    }]);
    hasher.update([match fixup.state {
        omega_isa_x86_64::X86_64StructuralUnitInternalControlFixupState::UnresolvedZeroFieldV1 => 0,
    }]);
    hasher.update(fixup.callee.get().to_le_bytes());
    hasher.update(fixup.opcode_byte_offset.to_le_bytes());
    hasher.update(fixup.field_byte_offset.to_le_bytes());
    hasher.update(fixup.next_instruction_byte_offset.to_le_bytes());
    hasher.update([fixup.field_byte_width]);
    hasher.update(fixup.addend.to_le_bytes());
}

fn encode_counts(hasher: &mut Sha256, counts: SelectedFormEncodingCounts) {
    for count in [
        counts.ordinary_encoded,
        counts.ordinary_deferred_control,
        counts.structural_encoded_call_templates,
        counts.structural_encoded_returns,
        counts.structural_deferred_internal_control,
        counts.structural_internal_fixups,
    ] {
        hasher.update(count.to_le_bytes());
    }
}

fn encode_machine_disposition(hasher: &mut Sha256, disposition: &SelectedFormMachineDisposition) {
    match disposition {
        SelectedFormMachineDisposition::RetainedV1 => hasher.update([0]),
        SelectedFormMachineDisposition::Aarch64ElidedCompareI64ZeroV1 { consumer } => {
            hasher.update([1]);
            hasher.update(consumer.0.to_le_bytes());
        }
        SelectedFormMachineDisposition::Aarch64FusedBranchNonZeroToCbnzV1 {
            compare,
            source_read,
        } => {
            hasher.update([2]);
            hasher.update(compare.0.to_le_bytes());
            hasher.update(source_read.source_instruction.0.to_le_bytes());
            hasher.update(source_read.operand.to_le_bytes());
            hasher.update(source_read.virtual_register.0.to_le_bytes());
            hasher.update(source_read.class.0.to_le_bytes());
            hasher.update(source_read.view.0.to_le_bytes());
            encode_units(hasher, &source_read.units);
        }
        SelectedFormMachineDisposition::Aarch64ElidedSameViewCopyI64V1 { consumer } => {
            hasher.update([3]);
            hasher.update(consumer.0.to_le_bytes());
        }
    }
}

fn encode_effects(hasher: &mut Sha256, effects: &MachineEncodedEffects) {
    hasher.update((effects.external_operand_reads.len() as u64).to_le_bytes());
    for operand in &effects.external_operand_reads {
        hasher.update(operand.to_le_bytes());
    }
    hasher.update((effects.external_operand_writes.len() as u64).to_le_bytes());
    for operand in &effects.external_operand_writes {
        hasher.update(operand.to_le_bytes());
    }
    encode_units(hasher, &effects.implicit_unit_uses);
    encode_units(hasher, &effects.implicit_unit_defs);
    encode_units(hasher, &effects.implicit_unit_clobbers);
    use omega_selected_instructions::{
        MachineEncodedControlEffect as Control, MachineEncodedMemoryEffect as Memory,
        MachineEncodedStackEffect as Stack, MachineEncodedTrapBehavior as Trap,
    };
    match effects.memory {
        Memory::NoneV1 => hasher.update([0]),
        Memory::ReadActivationStackV1 {
            stack_pointer,
            byte_count,
        } => {
            hasher.update([1]);
            hasher.update(stack_pointer.0.to_le_bytes());
            hasher.update(byte_count.to_le_bytes());
        }
    }
    match effects.stack {
        Stack::UnchangedV1 => hasher.update([0]),
        Stack::PopBytesV1 {
            stack_pointer,
            byte_count,
        } => {
            hasher.update([1]);
            hasher.update(stack_pointer.0.to_le_bytes());
            hasher.update(byte_count.to_le_bytes());
        }
    }
    hasher.update([match effects.trap {
        Trap::NeverV1 => 0,
        Trap::MayArchitecturalFaultV1 => 1,
    }]);
    match effects.control {
        Control::FallThroughV1 => hasher.update([0]),
        Control::ConditionalRelativeBranchV1 => hasher.update([1]),
        Control::ReturnFromActivationStackV1 => hasher.update([2]),
        Control::ReturnIndirectRegisterV1 { target } => {
            hasher.update([3]);
            hasher.update(target.0.to_le_bytes());
        }
    }
}

fn encode_alternative(hasher: &mut Sha256, alternative: MachineAlternativeKey) {
    hasher.update([match alternative.family {
        MachineAlternativeFamily::CompareI64Zero => 0,
        MachineAlternativeFamily::MaterializeI64 => 1,
        MachineAlternativeFamily::CopyI64 => 2,
        MachineAlternativeFamily::ExactAddI64 => 3,
        MachineAlternativeFamily::ExactAddI64Immediate => 4,
        MachineAlternativeFamily::ExactSubtractI64 => 5,
        MachineAlternativeFamily::ConditionalBranchNonZero => 6,
        MachineAlternativeFamily::ReturnI64 => 7,
        MachineAlternativeFamily::ExactSubtractI64Immediate => 8,
        MachineAlternativeFamily::ReturnUnit => 9,
        MachineAlternativeFamily::CompareI64 => 10,
    }]);
    hasher.update(alternative.variant.to_le_bytes());
}

fn encode_views(hasher: &mut Sha256, views: &[RegisterViewId]) {
    hasher.update((views.len() as u64).to_le_bytes());
    for view in views {
        hasher.update(view.0.to_le_bytes());
    }
}

fn encode_units(hasher: &mut Sha256, units: &[RegisterUnitId]) {
    hasher.update((units.len() as u64).to_le_bytes());
    for unit in units {
        hasher.update(unit.0.to_le_bytes());
    }
}

fn encode_machine_register(hasher: &mut Sha256, register: MachineRegister) {
    let (tag, index) = match register {
        MachineRegister::X86Rax => (1, 0),
        MachineRegister::X86Rcx => (2, 0),
        MachineRegister::X86Rdx => (3, 0),
        MachineRegister::X86Rbx => (4, 0),
        MachineRegister::X86Rsp => (5, 0),
        MachineRegister::X86Rbp => (6, 0),
        MachineRegister::X86Rsi => (7, 0),
        MachineRegister::X86Rdi => (8, 0),
        MachineRegister::X86R8 => (9, 0),
        MachineRegister::X86R9 => (10, 0),
        MachineRegister::X86R10 => (11, 0),
        MachineRegister::X86R11 => (12, 0),
        MachineRegister::X86R12 => (13, 0),
        MachineRegister::X86R13 => (14, 0),
        MachineRegister::X86R14 => (15, 0),
        MachineRegister::X86R15 => (16, 0),
        MachineRegister::X86Xmm(index) => (17, index),
        MachineRegister::Aarch64X(index) => (18, index),
        MachineRegister::Aarch64V(index) => (19, index),
    };
    hasher.update([tag, index]);
}
