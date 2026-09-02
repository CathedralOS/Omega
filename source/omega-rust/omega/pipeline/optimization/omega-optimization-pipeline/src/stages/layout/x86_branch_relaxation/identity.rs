use omega_optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};
use omega_selected_instructions::{
    MachineAlternativeFamily, MachineAlternativeKey, MachineEncodedControlEffect,
    MachineEncodedEffects, MachineEncodedMemoryEffect, MachineEncodedStackEffect,
    MachineEncodedTrapBehavior,
};
use omega_target::{Architecture, NativeTarget, ObjectFormat};
use sha2::{Digest, Sha256};

use crate::{ResolvedSelectedFormLayoutIdentity, ResolvedSelectedFunctionLayout};

use super::model::{
    X86BranchRelaxationAction, X86BranchRelaxationAttempt, X86BranchRelaxationAttemptOutcome,
    X86BranchRelaxationIdentity, X86BranchRelaxationPolicy, X86BranchRelaxationRevisionIdentity,
};

const RELAXATION_SCHEMA: &[u8] = b"omega.terminal.x86-branch-relaxation.v2";
const REVISION_SCHEMA: &[u8] = b"omega.terminal.x86-branch-relaxation-revision.v2";

#[derive(Clone, Copy)]
pub(super) struct RevisionRoots {
    pub(super) source: ResolvedSelectedFormLayoutIdentity,
    pub(super) selected: omega_selected_instructions::SelectedInstructionPlanIdentity,
    pub(super) machine: omega_machine_optimizer::PostAllocationMachineIdentity,
    pub(super) pre_layout: crate::SelectedFormEncodingIdentity,
    pub(super) target: NativeTarget,
}

pub(super) fn revision_identity(
    roots: RevisionRoots,
    functions: &[ResolvedSelectedFunctionLayout],
) -> X86BranchRelaxationRevisionIdentity {
    let mut hasher = Sha256::new();
    hasher.update(REVISION_SCHEMA);
    encode_roots(&mut hasher, roots);
    encode_functions(&mut hasher, functions);
    X86BranchRelaxationRevisionIdentity(hasher.finalize().into())
}

#[allow(clippy::too_many_arguments)]
pub(super) fn artifact_identity(
    roots: RevisionRoots,
    policy: X86BranchRelaxationPolicy,
    budget: OptimizationWorkBudget,
    usage: OptimizationWorkUsage,
    output: ResolvedSelectedFormLayoutIdentity,
    output_revision: X86BranchRelaxationRevisionIdentity,
    attempts: &[X86BranchRelaxationAttempt],
    actions: &[X86BranchRelaxationAction],
    functions: &[ResolvedSelectedFunctionLayout],
) -> X86BranchRelaxationIdentity {
    let mut hasher = Sha256::new();
    hasher.update(RELAXATION_SCHEMA);
    encode_roots(&mut hasher, roots);
    hasher.update([match policy {
        X86BranchRelaxationPolicy::X86RelaxConditionalBranchesToRel8V1 => 0,
    }]);
    hasher.update(budget.encode());
    hasher.update(usage.encode());
    hasher.update(output.bytes());
    hasher.update(output_revision.bytes());
    hasher.update((attempts.len() as u64).to_le_bytes());
    for attempt in attempts {
        hasher.update(attempt.iteration.to_le_bytes());
        hasher.update(attempt.input.bytes());
        hasher.update(attempt.instruction.0.to_le_bytes());
        hasher.update(attempt.offset.to_le_bytes());
        hasher.update(attempt.byte_displacement.to_le_bytes());
        hasher.update([attempt.encoded_bytes]);
        hasher.update([match attempt.outcome {
            X86BranchRelaxationAttemptOutcome::AlreadyShort => 0,
            X86BranchRelaxationAttemptOutcome::NearDisplacementOutsideI8 => 1,
            X86BranchRelaxationAttemptOutcome::SelectedForRelaxation => 2,
        }]);
    }
    hasher.update((actions.len() as u64).to_le_bytes());
    for action in actions {
        hasher.update(action.iteration.to_le_bytes());
        hasher.update(action.input.bytes());
        hasher.update(action.output.bytes());
        hasher.update(action.instruction.0.to_le_bytes());
        hasher.update(action.old_offset.to_le_bytes());
        hasher.update(action.new_offset.to_le_bytes());
        hasher.update(action.old_displacement.to_le_bytes());
        hasher.update(action.new_displacement.to_le_bytes());
        hasher.update((action.old_bytes.len() as u64).to_le_bytes());
        hasher.update(&action.old_bytes);
        hasher.update((action.new_bytes.len() as u64).to_le_bytes());
        hasher.update(&action.new_bytes);
    }
    encode_functions(&mut hasher, functions);
    X86BranchRelaxationIdentity(hasher.finalize().into())
}

fn encode_roots(hasher: &mut Sha256, roots: RevisionRoots) {
    hasher.update(roots.source.bytes());
    hasher.update(roots.selected.bytes());
    hasher.update(roots.machine.bytes());
    hasher.update(roots.pre_layout.bytes());
    hasher.update([match roots.target.architecture {
        Architecture::Aarch64 => 0,
        Architecture::X86_64 => 1,
    }]);
    hasher.update([match roots.target.object_format {
        ObjectFormat::Elf => 0,
        ObjectFormat::MachO => 1,
        ObjectFormat::Coff => 2,
    }]);
    hasher.update((roots.target.pointer_size as u64).to_le_bytes());
    hasher.update((roots.target.pointer_alignment as u64).to_le_bytes());
}

fn encode_functions(hasher: &mut Sha256, functions: &[ResolvedSelectedFunctionLayout]) {
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
            for row in &block.instructions {
                hasher.update(row.instruction.0.to_le_bytes());
                encode_alternative(hasher, row.alternative);
                hasher.update(row.offset.to_le_bytes());
                hasher.update((row.bytes.len() as u64).to_le_bytes());
                hasher.update(&row.bytes);
                match &row.branch {
                    None => hasher.update([0]),
                    Some(branch) => {
                        hasher.update([1]);
                        hasher.update(branch.source_block.0.to_le_bytes());
                        hasher.update(branch.when_nonzero_edge.get().to_le_bytes());
                        hasher.update(branch.when_nonzero_block.0.to_le_bytes());
                        hasher.update(branch.when_nonzero_offset.to_le_bytes());
                        hasher.update(branch.when_zero_edge.get().to_le_bytes());
                        hasher.update(branch.when_zero_block.0.to_le_bytes());
                        hasher.update(branch.when_zero_offset.to_le_bytes());
                        hasher.update(branch.byte_displacement.to_le_bytes());
                        encode_effects(hasher, &branch.decoded_effects);
                    }
                }
            }
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
