use optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};
use target::NativeTarget;

use crate::machine_code::identity_encoding::{encode_conditional_branch_arm, encode_jump_arm};
use crate::{ResolvedSelectedFormLayoutIdentity, ResolvedSelectedFunctionLayout};

use super::{
    X86BranchRelaxationAction, X86BranchRelaxationAttempt, X86BranchRelaxationAttemptOutcome,
    X86BranchRelaxationIdentity, X86BranchRelaxationPolicy, X86BranchRelaxationRevisionIdentity,
};

const RELAXATION_SCHEMA: &[u8] = b"omega.terminal.x86-branch-relaxation.v10";
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
    let mut hasher = crate::digest_encoder::DomainDigestEncoder::new(REVISION_SCHEMA);
    encode_roots(&mut hasher, roots);
    encode_functions(&mut hasher, functions);
    X86BranchRelaxationRevisionIdentity::from_bytes(hasher.finish())
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
    let mut hasher = crate::digest_encoder::DomainDigestEncoder::new(RELAXATION_SCHEMA);
    encode_roots(&mut hasher, roots);
    hasher.byte(match policy {
        X86BranchRelaxationPolicy::X86RelaxConditionalBranchesToRel8V1 => 0,
    });
    hasher.raw(&budget.encode());
    hasher.raw(&usage.encode());
    hasher.raw(&output.bytes());
    hasher.raw(&output_revision.bytes());
    hasher.u64(attempts.len() as u64);
    for attempt in attempts {
        hasher.raw(&attempt.iteration.to_le_bytes());
        hasher.raw(&attempt.input.bytes());
        hasher.raw(&attempt.instruction.0.to_le_bytes());
        hasher.raw(&attempt.offset.to_le_bytes());
        hasher.raw(&attempt.byte_displacement.to_le_bytes());
        hasher.byte(attempt.encoded_bytes);
        hasher.byte(match attempt.outcome {
            X86BranchRelaxationAttemptOutcome::AlreadyShort => 0,
            X86BranchRelaxationAttemptOutcome::NearDisplacementOutsideI8 => 1,
            X86BranchRelaxationAttemptOutcome::SelectedForRelaxation => 2,
        });
    }
    hasher.u64(actions.len() as u64);
    for action in actions {
        hasher.raw(&action.iteration.to_le_bytes());
        hasher.raw(&action.input.bytes());
        hasher.raw(&action.output.bytes());
        hasher.raw(&action.instruction.0.to_le_bytes());
        hasher.raw(&action.old_offset.to_le_bytes());
        hasher.raw(&action.new_offset.to_le_bytes());
        hasher.raw(&action.old_displacement.to_le_bytes());
        hasher.raw(&action.new_displacement.to_le_bytes());
        hasher.u64(action.old_bytes.len() as u64);
        hasher.raw(&action.old_bytes);
        hasher.u64(action.new_bytes.len() as u64);
        hasher.raw(&action.new_bytes);
    }
    encode_functions(&mut hasher, functions);
    X86BranchRelaxationIdentity::from_bytes(hasher.finish())
}

fn encode_roots(hasher: &mut crate::digest_encoder::DomainDigestEncoder, roots: RevisionRoots) {
    hasher.raw(&roots.source.bytes());
    hasher.raw(&roots.selected.bytes());
    hasher.raw(&roots.machine.bytes());
    hasher.raw(&roots.pre_layout.bytes());
    hasher.raw(&roots.target.identity_bytes());
}

fn encode_functions(
    hasher: &mut crate::digest_encoder::DomainDigestEncoder,
    functions: &[ResolvedSelectedFunctionLayout],
) {
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
            for row in &block.instructions {
                hasher.raw(&row.instruction.0.to_le_bytes());
                selected_instructions::encode_machine_alternative_key_identity(
                    hasher,
                    row.alternative,
                );
                hasher.raw(&row.offset.to_le_bytes());
                hasher.u64(row.bytes.len() as u64);
                hasher.raw(&row.bytes);
                match row.branch.as_deref() {
                    None => hasher.byte(0),
                    Some(crate::ResolvedBranchEvidence::Conditional(branch)) => {
                        encode_conditional_branch_arm(hasher, branch);
                        selected_instructions::encode_machine_encoded_effects_identity(
                            hasher,
                            &branch.decoded_effects,
                        );
                    }
                    Some(crate::ResolvedBranchEvidence::Jump(jump)) => {
                        encode_jump_arm(hasher, jump);
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
