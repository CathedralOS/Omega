use optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};
use sha2::{Digest, Sha256};
use target::{Architecture, NativeTarget, ObjectFormat};

use crate::{
    ResolvedConditionalBranchPredicate, ResolvedSelectedFormLayoutIdentity,
    ResolvedSelectedFunctionLayout,
};

use super::{
    X86BranchRelaxationAction, X86BranchRelaxationAttempt, X86BranchRelaxationAttemptOutcome,
    X86BranchRelaxationIdentity, X86BranchRelaxationPolicy, X86BranchRelaxationRevisionIdentity,
};

const RELAXATION_SCHEMA: &[u8] = b"omega.terminal.x86-branch-relaxation.v9";
const REVISION_SCHEMA: &[u8] = b"omega.terminal.x86-branch-relaxation-revision.v3";

#[derive(Clone, Copy)]
pub struct RevisionRoots {
    pub source: ResolvedSelectedFormLayoutIdentity,
    pub selected: selected_instructions::SelectedInstructionPlanIdentity,
    pub machine: physical_instructions::PostAllocationMachineIdentity,
    pub pre_layout: crate::SelectedFormEncodingIdentity,
    pub target: NativeTarget,
}

pub fn revision_identity(
    roots: RevisionRoots,
    functions: &[ResolvedSelectedFunctionLayout],
) -> X86BranchRelaxationRevisionIdentity {
    let mut hasher = Sha256::new();
    hasher.update(REVISION_SCHEMA);
    encode_roots(&mut hasher, roots);
    encode_functions(&mut hasher, functions);
    X86BranchRelaxationRevisionIdentity::from_bytes(hasher.finalize().into())
}

#[allow(clippy::too_many_arguments)]
pub fn artifact_identity(
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
    X86BranchRelaxationIdentity::from_bytes(hasher.finalize().into())
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
                selected_instructions::encode_machine_alternative_key_identity(
                    hasher,
                    row.alternative,
                );
                hasher.update(row.offset.to_le_bytes());
                hasher.update((row.bytes.len() as u64).to_le_bytes());
                hasher.update(&row.bytes);
                match row.branch.as_deref() {
                    None => hasher.update([0]),
                    Some(crate::ResolvedBranchEvidence::Conditional(branch)) => {
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
                        selected_instructions::encode_machine_encoded_effects_identity(
                            hasher,
                            &branch.decoded_effects,
                        );
                    }
                    Some(crate::ResolvedBranchEvidence::Jump(jump)) => {
                        hasher.update([2]);
                        hasher.update(jump.source_block.0.to_le_bytes());
                        hasher.update(jump.target_edge.get().to_le_bytes());
                        hasher.update(jump.target_block.0.to_le_bytes());
                        hasher.update(jump.target_offset.to_le_bytes());
                        hasher.update(jump.byte_displacement.to_le_bytes());
                        selected_instructions::encode_machine_encoded_effects_identity(
                            hasher,
                            &jump.decoded_effects,
                        );
                    }
                }
            }
        }
    }
}
