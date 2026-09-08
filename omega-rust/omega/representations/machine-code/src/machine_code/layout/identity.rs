//! Exact identity of the function-relative bytes, layout, and replay inputs.

use selected_instructions::{
    MachineAlternativeKey, MachineEncodedControlEffect, MachineEncodedEffects,
    MachineEncodedMemoryEffect, MachineEncodedStackEffect, MachineEncodedTrapBehavior,
};
use sha2::{Digest, Sha256};
use target::{Architecture, NativeTarget, ObjectFormat};

use physical_instructions::PostAllocationMachineOptimizationCustody;

use super::{
    ResolvedConditionalBranchPredicate, ResolvedSelectedFunctionLayout,
    SelectedFunctionLayoutPolicy,
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

const LAYOUT_SCHEMA: &[u8] = b"omega.terminal.resolved-selected-form-layout.v14";

pub fn resolved_machine_layout_identity(
    selected: selected_instructions::SelectedInstructionPlanIdentity,
    machine: physical_instructions::PostAllocationMachineIdentity,
    pre_layout: crate::SelectedFormEncodingIdentity,
    post_allocation_machine_optimization: Option<PostAllocationMachineOptimizationCustody>,
    target: NativeTarget,
    policy: SelectedFunctionLayoutPolicy,
    functions: &[ResolvedSelectedFunctionLayout],
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
                match instruction.branch.as_deref() {
                    None => hasher.update([0]),
                    Some(super::ResolvedBranchEvidence::Conditional(branch)) => {
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
                    Some(super::ResolvedBranchEvidence::Jump(jump)) => {
                        hasher.update([2]);
                        hasher.update(jump.source_block.0.to_le_bytes());
                        hasher.update(jump.target_edge.get().to_le_bytes());
                        hasher.update(jump.target_block.0.to_le_bytes());
                        hasher.update(jump.target_offset.to_le_bytes());
                        hasher.update(jump.byte_displacement.to_le_bytes());
                        encode_effects(&mut hasher, &jump.decoded_effects);
                    }
                }
                encode_internal_fixup(&mut hasher, instruction.internal_machine_fixup);
            }
        }
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

fn encode_views(hasher: &mut Sha256, values: &[register_model::RegisterViewId]) {
    hasher.update((values.len() as u64).to_le_bytes());
    for value in values {
        hasher.update(value.0.to_le_bytes());
    }
}

fn encode_alternative(hasher: &mut Sha256, alternative: MachineAlternativeKey) {
    use selected_instructions::MachineAlternativeFamily as Family;
    hasher.update([match alternative.family {
        Family::CompareI64Zero => 0,
        Family::MaterializeI64 => 1,
        Family::CopyI64 => 2,
        Family::ZeroExtendU8 => 15,
        Family::ZeroExtendU32 => 20,
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
        Family::Jump => 14,
        Family::Store => 24,
        Family::AddressOffset => 25,
        Family::Load64 => 16,
        Family::LinuxWriteByteI32 => 23,
        Family::ByteViewAddress => 22,
        Family::Load8Indexed => 21,
        Family::Store64 => 17,
        Family::FrameAddress => 18,
        Family::CallUnit => 19,
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
        MachineEncodedMemoryEffect::ReadIndexedPointerV1 {
            pointer_operand,
            index_operand,
            byte_count,
        } => {
            hasher.update([5]);
            hasher.update(pointer_operand.to_le_bytes());
            hasher.update(index_operand.to_le_bytes());
            hasher.update(byte_count.to_le_bytes());
        }
        MachineEncodedMemoryEffect::LinuxWriteByteV1 { stack_pointer } => {
            hasher.update([6]);
            hasher.update(stack_pointer.0.to_le_bytes());
        }
        MachineEncodedMemoryEffect::WritePointerV1 { pointer_operand } => {
            hasher.update([7]);
            hasher.update(pointer_operand.to_le_bytes());
        }
        MachineEncodedMemoryEffect::NoneV1 => hasher.update([0]),
        MachineEncodedMemoryEffect::ReadPointerV1 {
            pointer_operand,
            byte_count,
        } => {
            hasher.update([3]);
            hasher.update(pointer_operand.to_le_bytes());
            hasher.update(byte_count.to_le_bytes());
        }
        MachineEncodedMemoryEffect::WriteFrameStorageV1 {
            stack_pointer,
            byte_count,
        } => {
            hasher.update([4]);
            hasher.update(stack_pointer.0.to_le_bytes());
            hasher.update(byte_count.to_le_bytes());
        }
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
        MachineEncodedTrapBehavior::LinuxWriteFailureV1 => 2,
        MachineEncodedTrapBehavior::MayArchitecturalFaultV1 => 1,
    }]);
    match effects.control {
        MachineEncodedControlEffect::LinuxWriteReturnOrTrapV1 => hasher.update([6]),
        MachineEncodedControlEffect::FallThroughV1 => hasher.update([0]),
        MachineEncodedControlEffect::ConditionalRelativeBranchV1 => hasher.update([1]),
        MachineEncodedControlEffect::ReturnFromActivationStackV1 => hasher.update([2]),
        MachineEncodedControlEffect::ReturnIndirectRegisterV1 { target } => {
            hasher.update([3]);
            hasher.update(target.0.to_le_bytes());
        }
        MachineEncodedControlEffect::DirectRelativeCallV1 => hasher.update([4]),
        MachineEncodedControlEffect::UnconditionalRelativeBranchV1 => hasher.update([5]),
    }
}

fn encode_u16s(hasher: &mut Sha256, values: &[u16]) {
    hasher.update((values.len() as u64).to_le_bytes());
    for value in values {
        hasher.update(value.to_le_bytes());
    }
}

fn encode_units(hasher: &mut Sha256, values: &[register_model::RegisterUnitId]) {
    hasher.update((values.len() as u64).to_le_bytes());
    for value in values {
        hasher.update(value.0.to_le_bytes());
    }
}
