//! Exact identity of the function-relative bytes, layout, and replay inputs.

use target::NativeTarget;

use physical_instructions::PostAllocationMachineOptimizationCustody;

use super::{ResolvedSelectedFunctionLayout, SelectedFunctionLayoutPolicy};
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
    let mut hasher = crate::digest_encoder::DomainDigestEncoder::new(LAYOUT_SCHEMA);
    hasher.raw(&selected.bytes());
    hasher.raw(&machine.bytes());
    hasher.raw(&pre_layout.bytes());
    match post_allocation_machine_optimization {
        None => hasher.byte(0),
        Some(custody) => {
            hasher.byte(1);
            hasher.byte(custody.optimization() as u8);
            hasher.raw(&custody.artifact_identity());
            hasher.raw(&custody.selections().bytes());
            hasher.raw(&custody.post_allocation_machine_selections().bytes());
            hasher.raw(&custody.source().bytes());
            hasher.u64(custody.action_count() as u64);
            hasher.raw(&custody.baseline_bytes().to_le_bytes());
            hasher.raw(&custody.selected_bytes().to_le_bytes());
        }
    }
    hasher.raw(&target.identity_bytes());
    hasher.byte(match policy {
        SelectedFunctionLayoutPolicy::EntryThenZeroFallthroughThenNonzeroV1 => 0,
        SelectedFunctionLayoutPolicy::SingleEntryBlockV1 => 1,
        SelectedFunctionLayoutPolicy::EntryThenNotLessFallthroughThenLessV1 => 3,
        SelectedFunctionLayoutPolicy::PerFunctionCanonicalShapeV1 => 4,
    });
    hasher.u64(functions.len() as u64);
    for function in functions {
        hasher.raw(&function.machine.get().to_le_bytes());
        hasher.raw(&function.byte_count.to_le_bytes());
        hasher.u64(function.blocks.len() as u64);
        for block in &function.blocks {
            hasher.raw(&block.block.0.to_le_bytes());
            hasher.raw(&block.offset.to_le_bytes());
            hasher.raw(&block.byte_count.to_le_bytes());
            hasher.u64(block.instructions.len() as u64);
            for instruction in &block.instructions {
                hasher.raw(&instruction.instruction.0.to_le_bytes());
                selected_instructions::encode_machine_alternative_key_identity(
                    &mut hasher,
                    instruction.alternative,
                );
                hasher.raw(&instruction.offset.to_le_bytes());
                hasher.u64(instruction.bytes.len() as u64);
                hasher.raw(&instruction.bytes);
                match instruction.branch.as_deref() {
                    None => hasher.byte(0),
                    Some(super::ResolvedBranchEvidence::Conditional(branch)) => {
                        hasher.byte(1);
                        hasher.byte(branch.predicate.rank());
                        hasher.raw(&branch.source_block.0.to_le_bytes());
                        hasher.raw(&branch.when_taken_edge.get().to_le_bytes());
                        hasher.raw(&branch.when_taken_block.0.to_le_bytes());
                        hasher.raw(&branch.when_taken_offset.to_le_bytes());
                        hasher.raw(&branch.when_fallthrough_edge.get().to_le_bytes());
                        hasher.raw(&branch.when_fallthrough_block.0.to_le_bytes());
                        hasher.raw(&branch.when_fallthrough_offset.to_le_bytes());
                        hasher.raw(&branch.byte_displacement.to_le_bytes());
                        encode_views(&mut hasher, &branch.decoded_register_reads);
                        selected_instructions::encode_machine_encoded_effects_identity(
                            &mut hasher,
                            &branch.decoded_effects,
                        );
                    }
                    Some(super::ResolvedBranchEvidence::Jump(jump)) => {
                        hasher.byte(2);
                        hasher.raw(&jump.source_block.0.to_le_bytes());
                        hasher.raw(&jump.target_edge.get().to_le_bytes());
                        hasher.raw(&jump.target_block.0.to_le_bytes());
                        hasher.raw(&jump.target_offset.to_le_bytes());
                        hasher.raw(&jump.byte_displacement.to_le_bytes());
                        selected_instructions::encode_machine_encoded_effects_identity(
                            &mut hasher,
                            &jump.decoded_effects,
                        );
                    }
                }
                encode_internal_fixup(&mut hasher, instruction.internal_machine_fixup);
                // A row without an image address keeps its earlier identity.
                if let Some(fixup) = instruction.image_address_fixup {
                    let mut identity = b"image-address-v2".to_vec();
                    fixup.reference.encode_identity(&mut identity);
                    fixup.fields.encode_identity(&mut identity);
                    hasher.raw(&identity);
                }
            }
        }
    }
    ResolvedSelectedFormLayoutIdentity(hasher.finish())
}

fn encode_internal_fixup(
    hasher: &mut crate::digest_encoder::DomainDigestEncoder,
    fixup: Option<SelectedFormInternalMachineFixup>,
) {
    let Some(fixup) = fixup else {
        hasher.byte(0);
        return;
    };
    hasher.byte(1);
    hasher.byte(match fixup.kind {
        SelectedFormInternalMachineFixupKind::X86Relative32FromNextInstructionToInternalMachineV1 => 0,
        SelectedFormInternalMachineFixupKind::Aarch64BranchLinkImmediate26FromInstructionToInternalMachineV1 => 1,
    });
    hasher.byte(match fixup.state {
        SelectedFormInternalMachineFixupState::UnresolvedZeroFieldV1 => 0,
    });
    hasher.raw(&fixup.callee.get().to_le_bytes());
    hasher.raw(&fixup.opcode_row_offset.to_le_bytes());
    hasher.raw(&fixup.patch_row_offset.to_le_bytes());
    hasher.raw(&fixup.reference_row_offset.to_le_bytes());
    hasher.byte(fixup.patch_byte_width);
    hasher.raw(&fixup.addend.to_le_bytes());
}

fn encode_views(
    hasher: &mut crate::digest_encoder::DomainDigestEncoder,
    values: &[register_model::RegisterViewId],
) {
    hasher.u64(values.len() as u64);
    for value in values {
        hasher.raw(&value.0.to_le_bytes());
    }
}
