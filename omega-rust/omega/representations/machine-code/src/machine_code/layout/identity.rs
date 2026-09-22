//! Exact identity of the function-relative bytes, layout, and replay inputs.

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

const LAYOUT_SCHEMA: &[u8] = b"omega.terminal.resolved-selected-form-layout.v16";

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
                selected_instructions::encode_machine_alternative_key_identity(
                    &mut hasher,
                    instruction.alternative,
                );
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
                        selected_instructions::encode_machine_encoded_effects_identity(
                            &mut hasher,
                            &branch.decoded_effects,
                        );
                    }
                    Some(super::ResolvedBranchEvidence::Jump(jump)) => {
                        hasher.update([2]);
                        hasher.update(jump.source_block.0.to_le_bytes());
                        hasher.update(jump.target_edge.get().to_le_bytes());
                        hasher.update(jump.target_block.0.to_le_bytes());
                        hasher.update(jump.target_offset.to_le_bytes());
                        hasher.update(jump.byte_displacement.to_le_bytes());
                        selected_instructions::encode_machine_encoded_effects_identity(
                            &mut hasher,
                            &jump.decoded_effects,
                        );
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
