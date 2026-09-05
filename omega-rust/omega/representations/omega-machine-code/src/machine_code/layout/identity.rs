//! Exact identity of the function-relative bytes, layout, and replay inputs.

use crate::{
    X86_64SelectedStructuralUnitCallFootprint, X86_64StructuralUnitInternalControlFixup,
    X86_64StructuralUnitInternalControlFixupKind, X86_64StructuralUnitInternalControlFixupState,
};
use omega_calling_conventions::MachineRegister;
use omega_selected_instructions::{
    MachineAlternativeKey, MachineEncodedControlEffect, MachineEncodedEffects,
    MachineEncodedMemoryEffect, MachineEncodedStackEffect, MachineEncodedTrapBehavior,
};
use omega_target::{Architecture, NativeTarget, ObjectFormat};
use sha2::{Digest, Sha256};

use omega_physical_instructions::PostAllocationMachineOptimizationCustody;

use super::{
    ResolvedConditionalBranchPredicate, ResolvedSelectedFunctionLayout,
    ResolvedStructuralUnitFunctionLayout, SelectedFunctionLayoutPolicy,
};
use crate::{
    SelectedFormInternalMachineFixup, SelectedFormInternalMachineFixupKind,
    SelectedFormInternalMachineFixupState,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ResolvedSelectedFormLayoutIdentity(pub(super) [u8; 32]);

impl ResolvedSelectedFormLayoutIdentity {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

const LAYOUT_SCHEMA: &[u8] = b"omega.terminal.resolved-selected-form-layout.v9";

pub fn resolved_machine_layout_identity(
    selected: omega_selected_instructions::SelectedInstructionPlanIdentity,
    machine: omega_physical_instructions::PostAllocationMachineIdentity,
    pre_layout: crate::SelectedFormEncodingIdentity,
    post_allocation_machine_optimization: Option<PostAllocationMachineOptimizationCustody>,
    target: NativeTarget,
    policy: SelectedFunctionLayoutPolicy,
    functions: &[ResolvedSelectedFunctionLayout],
    structural_unit_functions: &[ResolvedStructuralUnitFunctionLayout],
) -> ResolvedSelectedFormLayoutIdentity {
    let mut hasher = Sha256::new();
    hasher.update(LAYOUT_SCHEMA);
    hasher.update(selected.bytes());
    hasher.update(machine.bytes());
    hasher.update(pre_layout.bytes());
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
    hasher.update([match target.architecture {
        Architecture::Aarch64 => 0,
        Architecture::X86_64 => 1,
    }]);
    hasher.update([match target.object_format {
        ObjectFormat::Elf => 0,
        ObjectFormat::MachO => 1,
        ObjectFormat::Coff => 2,
    }]);
    hasher.update((target.pointer_size as u64).to_le_bytes());
    hasher.update((target.pointer_alignment as u64).to_le_bytes());
    hasher.update([match policy {
        SelectedFunctionLayoutPolicy::EntryThenZeroFallthroughThenNonzeroV1 => 0,
        SelectedFunctionLayoutPolicy::SingleEntryBlockV1 => 1,
        SelectedFunctionLayoutPolicy::StructuralUnitCallThenReturnSingleEntryBlockV1 => 2,
        SelectedFunctionLayoutPolicy::EntryThenNotLessFallthroughThenLessV1 => 3,
        SelectedFunctionLayoutPolicy::PerFunctionCanonicalShapeV1 => 4,
    }]);
    hasher.update((functions.len() as u64).to_le_bytes());
    for function in functions {
        hasher.update(function.machine.get().to_le_bytes());
        hasher.update(function.byte_count.to_le_bytes());
        hasher.update((function.blocks.len() as u64).to_le_bytes());
        for block in &function.blocks {
            hasher.update(block.block.0.to_le_bytes());
            hasher.update(block.offset.to_le_bytes());
            hasher.update(block.byte_count.to_le_bytes());
            hasher.update((block.instructions.len() as u64).to_le_bytes());
            for instruction in &block.instructions {
                hasher.update(instruction.instruction.0.to_le_bytes());
                encode_alternative(&mut hasher, instruction.alternative);
                hasher.update(instruction.offset.to_le_bytes());
                hasher.update((instruction.bytes.len() as u64).to_le_bytes());
                hasher.update(&instruction.bytes);
                match &instruction.branch {
                    None => hasher.update([0]),
                    Some(branch) => {
                        hasher.update([1]);
                        hasher.update([match branch.predicate {
                            ResolvedConditionalBranchPredicate::NonZeroV1 => 0,
                            ResolvedConditionalBranchPredicate::U64LessThanV1 => 1,
                            ResolvedConditionalBranchPredicate::I64LessThanV1 => 2,
                        }]);
                        hasher.update(branch.source_block.0.to_le_bytes());
                        hasher.update(branch.when_taken_edge.get().to_le_bytes());
                        hasher.update(branch.when_taken_block.0.to_le_bytes());
                        hasher.update(branch.when_taken_offset.to_le_bytes());
                        hasher.update(branch.when_fallthrough_edge.get().to_le_bytes());
                        hasher.update(branch.when_fallthrough_block.0.to_le_bytes());
                        hasher.update(branch.when_fallthrough_offset.to_le_bytes());
                        hasher.update(branch.byte_displacement.to_le_bytes());
                        encode_views(&mut hasher, &branch.decoded_register_reads);
                        encode_effects(&mut hasher, &branch.decoded_effects);
                    }
                }
                encode_internal_fixup(&mut hasher, instruction.internal_machine_fixup);
            }
        }
    }
    hasher.update((structural_unit_functions.len() as u64).to_le_bytes());
    for function in structural_unit_functions {
        hasher.update(function.machine.get().to_le_bytes());
        hasher.update(function.block.0.to_le_bytes());
        hasher.update(function.offset.to_le_bytes());
        hasher.update(function.byte_count.to_le_bytes());
        match &function.call {
            None => hasher.update([0]),
            Some(call) => {
                hasher.update([1]);
                hasher.update(call.instruction.0.to_le_bytes());
                hasher.update(call.operation.get().to_le_bytes());
                hasher.update(call.callee.get().to_le_bytes());
                hasher.update(call.offset.to_le_bytes());
                hasher.update((call.bytes.len() as u64).to_le_bytes());
                hasher.update(&call.bytes);
                encode_structural_footprint(&mut hasher, &call.footprint);
                encode_structural_fixup(&mut hasher, call.fixup);
            }
        }
        hasher.update(function.return_instruction.instruction.0.to_le_bytes());
        encode_alternative(&mut hasher, function.return_instruction.alternative);
        hasher.update(function.return_instruction.offset.to_le_bytes());
        hasher.update((function.return_instruction.bytes.len() as u64).to_le_bytes());
        hasher.update(&function.return_instruction.bytes);
        debug_assert!(function.return_instruction.branch.is_none());
        hasher.update([0]);
    }
    ResolvedSelectedFormLayoutIdentity(hasher.finalize().into())
}

fn encode_internal_fixup(hasher: &mut Sha256, fixup: Option<SelectedFormInternalMachineFixup>) {
    let Some(fixup) = fixup else {
        hasher.update([0]);
        return;
    };
    hasher.update([1]);
    hasher.update([match fixup.kind {
        SelectedFormInternalMachineFixupKind::X86Relative32FromNextInstructionToInternalMachineV1 => 0,
        SelectedFormInternalMachineFixupKind::Aarch64BranchLinkImmediate26FromInstructionToInternalMachineV1 => 1,
    }]);
    hasher.update([match fixup.state {
        SelectedFormInternalMachineFixupState::UnresolvedZeroFieldV1 => 0,
    }]);
    hasher.update(fixup.callee.get().to_le_bytes());
    hasher.update(fixup.opcode_row_offset.to_le_bytes());
    hasher.update(fixup.patch_row_offset.to_le_bytes());
    hasher.update(fixup.reference_row_offset.to_le_bytes());
    hasher.update([fixup.patch_byte_width]);
    hasher.update(fixup.addend.to_le_bytes());
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
        X86_64StructuralUnitInternalControlFixupKind::Relative32FromNextInstructionToInternalMachineV1 => 0,
    }]);
    hasher.update([match fixup.state {
        X86_64StructuralUnitInternalControlFixupState::UnresolvedZeroFieldV1 => 0,
    }]);
    hasher.update(fixup.callee.get().to_le_bytes());
    hasher.update(fixup.opcode_byte_offset.to_le_bytes());
    hasher.update(fixup.field_byte_offset.to_le_bytes());
    hasher.update(fixup.next_instruction_byte_offset.to_le_bytes());
    hasher.update([fixup.field_byte_width]);
    hasher.update(fixup.addend.to_le_bytes());
}

fn encode_views(hasher: &mut Sha256, values: &[omega_register_model::RegisterViewId]) {
    hasher.update((values.len() as u64).to_le_bytes());
    for value in values {
        hasher.update(value.0.to_le_bytes());
    }
}

fn encode_alternative(hasher: &mut Sha256, alternative: MachineAlternativeKey) {
    use omega_selected_instructions::MachineAlternativeFamily as Family;
    hasher.update([match alternative.family {
        Family::CompareI64Zero => 0,
        Family::MaterializeI64 => 1,
        Family::CopyI64 => 2,
        Family::ExactAddI64 => 3,
        Family::ExactAddI64Immediate => 4,
        Family::ExactSubtractI64 => 5,
        Family::ConditionalBranchNonZero => 6,
        Family::ReturnI64 => 7,
        Family::ExactSubtractI64Immediate => 8,
        Family::ReturnUnit => 9,
        Family::CompareI64 => 10,
        Family::ConditionalBranchU64LessThan => 11,
        Family::ConditionalBranchI64LessThan => 12,
        Family::CallI64 => 13,
    }]);
    hasher.update(alternative.variant.to_le_bytes());
}

fn encode_effects(hasher: &mut Sha256, effects: &MachineEncodedEffects) {
    encode_u16s(hasher, &effects.external_operand_reads);
    encode_u16s(hasher, &effects.external_operand_writes);
    encode_units(hasher, &effects.implicit_unit_uses);
    encode_units(hasher, &effects.implicit_unit_defs);
    encode_units(hasher, &effects.implicit_unit_clobbers);
    match effects.memory {
        MachineEncodedMemoryEffect::NoneV1 => hasher.update([0]),
        MachineEncodedMemoryEffect::ReadActivationStackV1 {
            stack_pointer,
            byte_count,
        } => {
            hasher.update([1]);
            hasher.update(stack_pointer.0.to_le_bytes());
            hasher.update(byte_count.to_le_bytes());
        }
        MachineEncodedMemoryEffect::WriteReturnAddressBelowStackPointerV1 {
            stack_pointer,
            byte_count,
        } => {
            hasher.update([2]);
            hasher.update(stack_pointer.0.to_le_bytes());
            hasher.update(byte_count.to_le_bytes());
        }
    }
    match effects.stack {
        MachineEncodedStackEffect::UnchangedV1 => hasher.update([0]),
        MachineEncodedStackEffect::PopBytesV1 {
            stack_pointer,
            byte_count,
        } => {
            hasher.update([1]);
            hasher.update(stack_pointer.0.to_le_bytes());
            hasher.update(byte_count.to_le_bytes());
        }
        MachineEncodedStackEffect::CallReturnAddressLifecycleV1 {
            stack_pointer,
            return_address_byte_count,
        } => {
            hasher.update([2]);
            hasher.update(stack_pointer.0.to_le_bytes());
            hasher.update(return_address_byte_count.to_le_bytes());
        }
    }
    hasher.update([match effects.trap {
        MachineEncodedTrapBehavior::NeverV1 => 0,
        MachineEncodedTrapBehavior::MayArchitecturalFaultV1 => 1,
    }]);
    match effects.control {
        MachineEncodedControlEffect::FallThroughV1 => hasher.update([0]),
        MachineEncodedControlEffect::ConditionalRelativeBranchV1 => hasher.update([1]),
        MachineEncodedControlEffect::ReturnFromActivationStackV1 => hasher.update([2]),
        MachineEncodedControlEffect::ReturnIndirectRegisterV1 { target } => {
            hasher.update([3]);
            hasher.update(target.0.to_le_bytes());
        }
        MachineEncodedControlEffect::DirectRelativeCallV1 => hasher.update([4]),
    }
}

fn encode_u16s(hasher: &mut Sha256, values: &[u16]) {
    hasher.update((values.len() as u64).to_le_bytes());
    for value in values {
        hasher.update(value.to_le_bytes());
    }
}

fn encode_units(hasher: &mut Sha256, values: &[omega_register_model::RegisterUnitId]) {
    hasher.update((values.len() as u64).to_le_bytes());
    for value in values {
        hasher.update(value.0.to_le_bytes());
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
