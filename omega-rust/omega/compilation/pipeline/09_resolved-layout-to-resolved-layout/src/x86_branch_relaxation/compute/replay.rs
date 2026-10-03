//! Independent ordered replay of each sweep, byte validation, and action
//! reconstruction.
//!
//! Replay reconstructs the same sweep shape the producer claims: every branch
//! inspected in layout order, every admissible branch narrowed in that sweep,
//! one dense reflow and one revision hash per sweep, and a terminal sweep that
//! selects nothing. It reaches that shape through its own ordered location
//! roster, its own byte spellings and its own validators, so agreement is
//! evidence rather than a shared code path.

use isa_x86_64::{
    validate_x86_64_selected_i64_less_than_branch_form,
    validate_x86_64_selected_short_nonzero_branch_form,
    validate_x86_64_selected_u64_less_than_branch_form,
};
use optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};
use register_model::ValidatedPhysicalRegisterModel;
use selected_instructions::{MachineAlternativeFamily, MachineAlternativeKey};

use machine_code::{
    ResolvedConditionalBranchPredicate, ResolvedSelectedFormRow, ResolvedSelectedFunctionLayout,
};

use super::super::{
    X86BranchRelaxationAction, X86BranchRelaxationAttempt, X86BranchRelaxationAttemptOutcome,
    error::{OptimizedX86BranchRelaxationError, X86BranchRelaxationWorkAxis},
    identity::{RevisionRoots, revision_identity},
};
use super::{
    RelaxationTrace,
    branch_inspection::replay_inspect_branch,
    reflow::reflow_replay_functions,
    work::{ordered_branch_locations, replay_charge, total_bytes},
};

/// Replay re-derives the per-branch saving rather than reading the producer's.
const SHORT_FORM_SAVED_BYTES: u64 = 4;

pub(super) fn replay_trace(
    roots: RevisionRoots,
    source: &[ResolvedSelectedFunctionLayout],
    physical: &ValidatedPhysicalRegisterModel,
    budget: OptimizationWorkBudget,
) -> Result<RelaxationTrace, OptimizedX86BranchRelaxationError> {
    let mut functions = source.to_vec();
    let mut attempts = Vec::new();
    let mut actions = Vec::new();
    let mut usage = OptimizationWorkUsage::default();
    loop {
        replay_charge(
            &mut usage.iterations,
            budget.iterations(),
            X86BranchRelaxationWorkAxis::Iterations,
        )?;
        let iteration = usage.iterations;
        let input = revision_identity(roots, &functions);
        let before = total_bytes(&functions)?;
        let locations = ordered_branch_locations(&functions);
        let mut chosen = Vec::new();
        for (function_index, block_index, instruction_index) in locations {
            replay_charge(
                &mut usage.rule_evaluations,
                budget.rule_evaluations(),
                X86BranchRelaxationWorkAxis::RuleEvaluations,
            )?;
            let row =
                &functions[function_index].blocks[block_index].instructions[instruction_index];
            let branch = row
                .branch
                .as_deref()
                .and_then(machine_code::ResolvedBranchEvidence::as_conditional)
                .ok_or(OptimizedX86BranchRelaxationError::MalformedBranch(
                    row.instruction,
                ))?;
            let (outcome, displacement) = replay_inspect_branch(
                &functions[function_index],
                block_index,
                instruction_index,
                physical,
            )?;
            attempts.push(X86BranchRelaxationAttempt {
                iteration,
                input,
                instruction: row.instruction,
                offset: row.offset,
                byte_displacement: branch.byte_displacement,
                encoded_bytes: u8::try_from(row.bytes.len())
                    .map_err(|_| OptimizedX86BranchRelaxationError::OffsetOverflow)?,
                outcome,
            });
            if outcome == X86BranchRelaxationAttemptOutcome::SelectedForRelaxation {
                replay_charge(
                    &mut usage.candidates,
                    budget.candidates(),
                    X86BranchRelaxationWorkAxis::Candidates,
                )?;
                replay_charge(
                    &mut usage.validation_steps,
                    budget.validation_steps(),
                    X86BranchRelaxationWorkAxis::ValidationSteps,
                )?;
                let displacement = displacement.ok_or(
                    OptimizedX86BranchRelaxationError::MalformedBranch(row.instruction),
                )?;
                chosen.push((function_index, block_index, instruction_index, displacement));
            }
        }
        if chosen.is_empty() {
            break;
        }
        let mut narrowed = Vec::with_capacity(chosen.len());
        for &(function_index, block_index, instruction_index, displacement) in &chosen {
            replay_charge(
                &mut usage.commits,
                budget.commits(),
                X86BranchRelaxationWorkAxis::Commits,
            )?;
            narrowed.push(narrow_one_branch(
                &mut functions[function_index].blocks[block_index].instructions[instruction_index],
                displacement,
                physical,
            )?);
        }
        reflow_replay_functions(&mut functions, physical)?;
        let expected_saving = u64::try_from(narrowed.len())
            .ok()
            .and_then(|count| count.checked_mul(SHORT_FORM_SAVED_BYTES))
            .ok_or(OptimizedX86BranchRelaxationError::OffsetOverflow)?;
        if before.checked_sub(total_bytes(&functions)?) != Some(expected_saving) {
            return Err(OptimizedX86BranchRelaxationError::NonDecreasingByteMeasure);
        }
        let output = revision_identity(roots, &functions);
        for (&(function_index, block_index, instruction_index, _), old) in
            chosen.iter().zip(narrowed)
        {
            let new =
                &functions[function_index].blocks[block_index].instructions[instruction_index];
            actions.push(X86BranchRelaxationAction {
                iteration,
                input,
                output,
                instruction: old.instruction,
                old_offset: old.offset,
                new_offset: new.offset,
                old_displacement: replay_conditional_displacement(&old)?,
                new_displacement: replay_conditional_displacement(new)?,
                old_bytes: old.bytes,
                new_bytes: new.bytes.clone(),
            });
        }
    }
    Ok(RelaxationTrace {
        usage,
        attempts,
        actions,
        functions,
    })
}

/// Spell the short form by hand, validate it against the independent decoder,
/// install it and return the row replay observed before the narrowing.
fn narrow_one_branch(
    row: &mut ResolvedSelectedFormRow,
    displacement: i64,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<ResolvedSelectedFormRow, OptimizedX86BranchRelaxationError> {
    let old = row.clone();
    let predicate = old
        .branch
        .as_deref()
        .and_then(machine_code::ResolvedBranchEvidence::as_conditional)
        .ok_or(OptimizedX86BranchRelaxationError::MalformedBranch(
            old.instruction,
        ))?
        .predicate;
    let (short_alternative, bytes) = match predicate {
        ResolvedConditionalBranchPredicate::NonZeroV1 => {
            let bytes = [0x75, displacement as i8 as u8];
            validate_x86_64_selected_short_nonzero_branch_form(
                physical,
                old.alternative,
                displacement,
                &bytes,
            )
            .map_err(OptimizedX86BranchRelaxationError::X86_64)?;
            (old.alternative, bytes)
        }
        ResolvedConditionalBranchPredicate::U64LessThanV1 => {
            let alternative = MachineAlternativeKey {
                family: MachineAlternativeFamily::ConditionalBranchU64LessThan,
                variant: 1,
            };
            let bytes = [0x72, displacement as i8 as u8];
            validate_x86_64_selected_u64_less_than_branch_form(
                physical,
                alternative,
                displacement,
                &bytes,
            )
            .map_err(OptimizedX86BranchRelaxationError::X86_64)?;
            (alternative, bytes)
        }
        ResolvedConditionalBranchPredicate::I64LessThanV1 => {
            let alternative = MachineAlternativeKey {
                family: MachineAlternativeFamily::ConditionalBranchI64LessThan,
                variant: 1,
            };
            let bytes = [0x7c, displacement as i8 as u8];
            validate_x86_64_selected_i64_less_than_branch_form(
                physical,
                alternative,
                displacement,
                &bytes,
            )
            .map_err(OptimizedX86BranchRelaxationError::X86_64)?;
            (alternative, bytes)
        }
    };
    row.alternative = short_alternative;
    row.bytes = bytes.to_vec();
    Ok(old)
}

fn replay_conditional_displacement(
    row: &ResolvedSelectedFormRow,
) -> Result<i64, OptimizedX86BranchRelaxationError> {
    Ok(row
        .branch
        .as_deref()
        .and_then(machine_code::ResolvedBranchEvidence::as_conditional)
        .ok_or(OptimizedX86BranchRelaxationError::MalformedBranch(
            row.instruction,
        ))?
        .byte_displacement)
}
