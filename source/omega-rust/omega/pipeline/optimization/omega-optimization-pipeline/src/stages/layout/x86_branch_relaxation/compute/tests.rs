use omega_isa_x86_64::{
    encode_x86_64_selected_i64_less_than_branch_form, encode_x86_64_selected_nonzero_branch_form,
    encode_x86_64_selected_short_nonzero_branch_form,
    encode_x86_64_selected_u64_less_than_branch_form, x86_64_physical_register_model,
};
use omega_optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};
use omega_register_model::{ValidatedPhysicalRegisterModel, validate_physical_register_model};
use omega_selected_instructions::{
    MachineAlternativeFamily, MachineAlternativeKey, SelectedBlockId, SelectedInstructionId,
};
use omega_target::NativeTarget;
use psi_core::{EdgeId, MachineId};

use crate::{
    ResolvedConditionalBranchPredicate, ResolvedSelectedBlockLayout,
    ResolvedSelectedFormLayoutIdentity, ResolvedSelectedFormRow, ResolvedSelectedFunctionLayout,
};

use super::{
    branch_inspection::{inspect_production_branch, replay_inspect_branch},
    reflow::{reflow_production_functions, reflow_replay_functions},
    work::charge,
};

use super::super::{
    error::{OptimizedX86BranchRelaxationError, X86BranchRelaxationWorkAxis},
    identity::{RevisionRoots, artifact_identity, revision_identity},
    model::{
        X86BranchRelaxationAction, X86BranchRelaxationAttempt, X86BranchRelaxationAttemptOutcome,
        X86BranchRelaxationPolicy, X86BranchRelaxationRevisionIdentity,
    },
    validation::{compare_replayed_action_evidence, ensure_x86_target},
};

fn physical() -> ValidatedPhysicalRegisterModel {
    validate_physical_register_model(x86_64_physical_register_model()).unwrap()
}

fn alternative() -> MachineAlternativeKey {
    MachineAlternativeKey {
        family: MachineAlternativeFamily::ConditionalBranchNonZero,
        variant: 0,
    }
}

fn function(zero_arm_bytes: usize) -> ResolvedSelectedFunctionLayout {
    conditional_function(
        ResolvedConditionalBranchPredicate::NonZeroV1,
        zero_arm_bytes,
    )
}

fn less_than_function(not_less_arm_bytes: usize) -> ResolvedSelectedFunctionLayout {
    conditional_function(
        ResolvedConditionalBranchPredicate::U64LessThanV1,
        not_less_arm_bytes,
    )
}

fn signed_less_than_function(not_less_arm_bytes: usize) -> ResolvedSelectedFunctionLayout {
    conditional_function(
        ResolvedConditionalBranchPredicate::I64LessThanV1,
        not_less_arm_bytes,
    )
}

fn conditional_function(
    predicate: ResolvedConditionalBranchPredicate,
    fallthrough_arm_bytes: usize,
) -> ResolvedSelectedFunctionLayout {
    let physical = physical();
    let displacement = i64::try_from(fallthrough_arm_bytes).unwrap();
    let branch_alternative = match predicate {
        ResolvedConditionalBranchPredicate::NonZeroV1 => alternative(),
        ResolvedConditionalBranchPredicate::U64LessThanV1 => MachineAlternativeKey {
            family: MachineAlternativeFamily::ConditionalBranchU64LessThan,
            variant: 0,
        },
        ResolvedConditionalBranchPredicate::I64LessThanV1 => MachineAlternativeKey {
            family: MachineAlternativeFamily::ConditionalBranchI64LessThan,
            variant: 0,
        },
    };
    let near = match predicate {
        ResolvedConditionalBranchPredicate::NonZeroV1 => {
            encode_x86_64_selected_nonzero_branch_form(&physical, branch_alternative, displacement)
        }
        ResolvedConditionalBranchPredicate::U64LessThanV1 => {
            encode_x86_64_selected_u64_less_than_branch_form(
                &physical,
                branch_alternative,
                displacement,
            )
        }
        ResolvedConditionalBranchPredicate::I64LessThanV1 => {
            encode_x86_64_selected_i64_less_than_branch_form(
                &physical,
                branch_alternative,
                displacement,
            )
        }
    }
    .unwrap();
    let entry = SelectedBlockId(0);
    let fallthrough = SelectedBlockId(1);
    let taken = SelectedBlockId(2);
    let fallthrough_offset = 6;
    let taken_offset = fallthrough_offset + u64::try_from(fallthrough_arm_bytes).unwrap();
    ResolvedSelectedFunctionLayout {
        machine: MachineId::new(1).unwrap(),
        byte_count: taken_offset + 1,
        blocks: vec![
            ResolvedSelectedBlockLayout {
                block: entry,
                offset: 0,
                byte_count: 6,
                instructions: vec![ResolvedSelectedFormRow {
                    instruction: SelectedInstructionId(0),
                    alternative: branch_alternative,
                    offset: 0,
                    bytes: near.bytes().to_vec(),
                    branch: Some(Box::new(crate::ResolvedConditionalBranchEvidence {
                        predicate,
                        source_block: entry,
                        when_taken_edge: EdgeId::new(1).unwrap(),
                        when_taken_block: taken,
                        when_taken_offset: taken_offset,
                        when_fallthrough_edge: EdgeId::new(2).unwrap(),
                        when_fallthrough_block: fallthrough,
                        when_fallthrough_offset: fallthrough_offset,
                        byte_displacement: displacement,
                        decoded_register_reads: vec![],
                        decoded_effects: near.footprint().encoded.clone(),
                    })),
                    internal_machine_fixup: None,
                }],
            },
            ResolvedSelectedBlockLayout {
                block: fallthrough,
                offset: fallthrough_offset,
                byte_count: u64::try_from(fallthrough_arm_bytes).unwrap(),
                instructions: vec![ResolvedSelectedFormRow {
                    instruction: SelectedInstructionId(1),
                    alternative: MachineAlternativeKey {
                        family: MachineAlternativeFamily::ReturnI64,
                        variant: 0,
                    },
                    offset: fallthrough_offset,
                    bytes: vec![0x90; fallthrough_arm_bytes],
                    branch: None,
                    internal_machine_fixup: None,
                }],
            },
            ResolvedSelectedBlockLayout {
                block: taken,
                offset: taken_offset,
                byte_count: 1,
                instructions: vec![ResolvedSelectedFormRow {
                    instruction: SelectedInstructionId(2),
                    alternative: MachineAlternativeKey {
                        family: MachineAlternativeFamily::ReturnI64,
                        variant: 0,
                    },
                    offset: taken_offset,
                    bytes: vec![0xc3],
                    branch: None,
                    internal_machine_fixup: None,
                }],
            },
        ],
    }
}

#[test]
fn eligible_near_branch_shrinks_and_both_reflow_implementations_agree() {
    let physical = physical();
    let source = function(127);
    assert_eq!(
        inspect_production_branch(&source, 0, 0, &physical).unwrap(),
        (
            X86BranchRelaxationAttemptOutcome::SelectedForRelaxation,
            Some(127),
        )
    );
    assert_eq!(
        replay_inspect_branch(&source, 0, 0, &physical).unwrap(),
        (
            X86BranchRelaxationAttemptOutcome::SelectedForRelaxation,
            Some(127),
        )
    );

    let short =
        encode_x86_64_selected_short_nonzero_branch_form(&physical, alternative(), 127).unwrap();
    let mut produced = vec![source.clone()];
    produced[0].blocks[0].instructions[0].bytes = short.bytes().to_vec();
    let mut replayed = produced.clone();
    reflow_production_functions(&mut produced, &physical).unwrap();
    reflow_replay_functions(&mut replayed, &physical).unwrap();
    assert_eq!(produced, replayed);
    assert_eq!(source.byte_count - produced[0].byte_count, 4);
    assert_eq!(produced[0].blocks[0].instructions[0].bytes, [0x75, 0x7f]);
    assert_eq!(produced[0].blocks[1].offset, 2);
    assert_eq!(produced[0].blocks[2].offset, 129);
    assert_eq!(
        produced[0].blocks[0].instructions[0]
            .branch
            .as_deref()
            .unwrap()
            .byte_displacement,
        127
    );
    assert_eq!(
        inspect_production_branch(&produced[0], 0, 0, &physical).unwrap(),
        (X86BranchRelaxationAttemptOutcome::AlreadyShort, None,)
    );
    assert_eq!(
        replay_inspect_branch(&produced[0], 0, 0, &physical).unwrap(),
        (X86BranchRelaxationAttemptOutcome::AlreadyShort, None,)
    );

    let fixed_point = produced.clone();
    reflow_production_functions(&mut produced, &physical).unwrap();
    reflow_replay_functions(&mut replayed, &physical).unwrap();
    assert_eq!(produced, fixed_point);
    assert_eq!(replayed, fixed_point);
}

#[test]
fn u64_less_than_relaxation_uses_jb_and_changes_only_the_alternative_variant() {
    let physical = physical();
    let source = less_than_function(127);
    assert_eq!(source.blocks[0].instructions[0].bytes[..2], [0x0f, 0x82]);
    assert_eq!(source.blocks[0].instructions[0].alternative.variant, 0);
    assert_eq!(
        inspect_production_branch(&source, 0, 0, &physical).unwrap(),
        (
            X86BranchRelaxationAttemptOutcome::SelectedForRelaxation,
            Some(127),
        )
    );
    assert_eq!(
        replay_inspect_branch(&source, 0, 0, &physical).unwrap(),
        (
            X86BranchRelaxationAttemptOutcome::SelectedForRelaxation,
            Some(127),
        )
    );

    let short_alternative = MachineAlternativeKey {
        family: MachineAlternativeFamily::ConditionalBranchU64LessThan,
        variant: 1,
    };
    let short = encode_x86_64_selected_u64_less_than_branch_form(&physical, short_alternative, 127)
        .unwrap();
    let mut produced = vec![source.clone()];
    produced[0].blocks[0].instructions[0].alternative = short_alternative;
    produced[0].blocks[0].instructions[0].bytes = short.bytes().to_vec();
    let mut replayed = produced.clone();
    reflow_production_functions(&mut produced, &physical).unwrap();
    reflow_replay_functions(&mut replayed, &physical).unwrap();
    assert_eq!(produced, replayed);
    assert_eq!(source.byte_count - produced[0].byte_count, 4);
    assert_eq!(produced[0].blocks[0].instructions[0].bytes, [0x72, 0x7f]);
    assert_eq!(
        produced[0].blocks[0].instructions[0].alternative,
        short_alternative
    );
    assert_eq!(
        inspect_production_branch(&produced[0], 0, 0, &physical).unwrap(),
        (X86BranchRelaxationAttemptOutcome::AlreadyShort, None)
    );
    assert_eq!(
        replay_inspect_branch(&produced[0], 0, 0, &physical).unwrap(),
        (X86BranchRelaxationAttemptOutcome::AlreadyShort, None)
    );
}

#[test]
fn i64_less_than_relaxation_uses_jl_and_replay_rejects_jb_substitution() {
    let physical = physical();
    let source = signed_less_than_function(127);
    assert_eq!(source.blocks[0].instructions[0].bytes[..2], [0x0f, 0x8c]);
    let short_alternative = MachineAlternativeKey {
        family: MachineAlternativeFamily::ConditionalBranchI64LessThan,
        variant: 1,
    };
    let short = encode_x86_64_selected_i64_less_than_branch_form(&physical, short_alternative, 127)
        .unwrap();
    let mut produced = vec![source.clone()];
    produced[0].blocks[0].instructions[0].alternative = short_alternative;
    produced[0].blocks[0].instructions[0].bytes = short.bytes().to_vec();
    let mut replayed = produced.clone();
    reflow_production_functions(&mut produced, &physical).unwrap();
    reflow_replay_functions(&mut replayed, &physical).unwrap();
    assert_eq!(produced, replayed);
    assert_eq!(source.byte_count - produced[0].byte_count, 4);
    assert_eq!(produced[0].blocks[0].instructions[0].bytes, [0x7c, 0x7f]);

    let mut unsigned_opcode = produced[0].clone();
    unsigned_opcode.blocks[0].instructions[0].bytes[0] = 0x72;
    assert!(replay_inspect_branch(&unsigned_opcode, 0, 0, &physical).is_err());
}

#[test]
fn branch_predicate_and_opcode_cannot_be_substituted() {
    let physical = physical();
    let mut less = less_than_function(1);
    less.blocks[0].instructions[0].alternative = alternative();
    less.blocks[0].instructions[0].bytes = vec![0x0f, 0x85, 1, 0, 0, 0];
    assert!(replay_inspect_branch(&less, 0, 0, &physical).is_err());

    let mut nonzero = function(1);
    nonzero.blocks[0].instructions[0].alternative = MachineAlternativeKey {
        family: MachineAlternativeFamily::ConditionalBranchU64LessThan,
        variant: 0,
    };
    nonzero.blocks[0].instructions[0].bytes = vec![0x0f, 0x82, 1, 0, 0, 0];
    assert!(replay_inspect_branch(&nonzero, 0, 0, &physical).is_err());
}

#[test]
fn out_of_range_near_branch_is_a_verified_no_change_attempt() {
    let physical = physical();
    let source = function(128);
    assert_eq!(
        inspect_production_branch(&source, 0, 0, &physical).unwrap(),
        (
            X86BranchRelaxationAttemptOutcome::NearDisplacementOutsideI8,
            None,
        )
    );
    assert_eq!(
        replay_inspect_branch(&source, 0, 0, &physical).unwrap(),
        (
            X86BranchRelaxationAttemptOutcome::NearDisplacementOutsideI8,
            None,
        )
    );
}

#[test]
fn malformed_short_opcode_and_work_overrun_fail_closed() {
    let physical = physical();
    let mut source = function(1);
    source.blocks[0].instructions[0].bytes = vec![0x74, 1];
    assert!(matches!(
        replay_inspect_branch(&source, 0, 0, &physical),
        Err(OptimizedX86BranchRelaxationError::MalformedBranch(
            SelectedInstructionId(0)
        ))
    ));

    let mut usage = 0;
    assert_eq!(
        charge(&mut usage, 0, X86BranchRelaxationWorkAxis::Commits),
        Err(OptimizedX86BranchRelaxationError::BudgetExceeded(
            X86BranchRelaxationWorkAxis::Commits
        ))
    );
}

#[test]
fn non_x86_target_is_rejected_before_any_relaxation_work() {
    let physical = physical();
    assert_eq!(
        ensure_x86_target(NativeTarget::linux_arm64(), &physical),
        Err(OptimizedX86BranchRelaxationError::UnsupportedTarget(
            NativeTarget::linux_arm64()
        ))
    );
}

#[test]
fn corrupted_action_changes_identity_and_is_rejected_by_replay_comparison() {
    let roots = RevisionRoots {
        source: ResolvedSelectedFormLayoutIdentity::from_bytes([1; 32]),
        selected: omega_selected_instructions::SelectedInstructionPlanIdentity::from_bytes([2; 32]),
        machine: omega_machine_optimizer::PostAllocationMachineIdentity::from_bytes([3; 32]),
        pre_layout: crate::SelectedFormEncodingIdentity::from_bytes([4; 32]),
        target: NativeTarget::linux_x64(),
    };
    let functions = vec![function(1)];
    let input = revision_identity(roots, &functions);
    let action = X86BranchRelaxationAction {
        iteration: 1,
        input,
        output: X86BranchRelaxationRevisionIdentity::from_bytes([5; 32]),
        instruction: SelectedInstructionId(0),
        old_offset: 0,
        new_offset: 0,
        old_displacement: 1,
        new_displacement: 1,
        old_bytes: vec![0x0f, 0x85, 1, 0, 0, 0],
        new_bytes: vec![0x75, 1],
    };
    let attempts = vec![X86BranchRelaxationAttempt {
        iteration: 1,
        input,
        instruction: SelectedInstructionId(0),
        offset: 0,
        byte_displacement: 1,
        encoded_bytes: 6,
        outcome: X86BranchRelaxationAttemptOutcome::SelectedForRelaxation,
    }];
    let budget = OptimizationWorkBudget::new(8, 8, 8, 8, 8).unwrap();
    let usage = OptimizationWorkUsage {
        rule_evaluations: 1,
        candidates: 1,
        validation_steps: 1,
        commits: 1,
        iterations: 2,
    };
    let output = ResolvedSelectedFormLayoutIdentity::from_bytes([6; 32]);
    let output_revision = X86BranchRelaxationRevisionIdentity::from_bytes([7; 32]);
    let identity = artifact_identity(
        roots,
        X86BranchRelaxationPolicy::X86RelaxConditionalBranchesToRel8V1,
        budget,
        usage,
        output,
        output_revision,
        &attempts,
        std::slice::from_ref(&action),
        &functions,
    );
    let mut corrupted = action.clone();
    corrupted.new_bytes = vec![0x74, 1];
    let corrupted_identity = artifact_identity(
        roots,
        X86BranchRelaxationPolicy::X86RelaxConditionalBranchesToRel8V1,
        budget,
        usage,
        output,
        output_revision,
        &attempts,
        std::slice::from_ref(&corrupted),
        &functions,
    );
    assert_ne!(identity, corrupted_identity);
    assert_eq!(
        compare_replayed_action_evidence(
            &attempts,
            std::slice::from_ref(&corrupted),
            &attempts,
            std::slice::from_ref(&action),
        ),
        Err(OptimizedX86BranchRelaxationError::ArtifactMismatch)
    );
}
