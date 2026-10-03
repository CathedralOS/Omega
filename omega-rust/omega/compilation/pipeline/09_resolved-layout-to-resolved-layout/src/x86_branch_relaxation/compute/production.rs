//! Production fixed-point sweep and batched short-branch commit mechanics.
//!
//! One sweep inspects every conditional branch in layout order, narrows every
//! branch whose prospective rel8 displacement is admissible, reflows the whole
//! roster once and hashes the revision once. The loop ends on the first sweep
//! that selects nothing.
//!
//! Narrowing only ever removes bytes, so the span between any branch and its
//! taken block is non-increasing across the whole computation and an admissible
//! branch stays admissible. Admission is therefore monotone in the set of
//! already-narrowed branches: batching can neither admit a branch that
//! one-at-a-time commits would have left long, nor miss one they would have
//! reached. Both orders are maximal chaotic iterations of the same monotone,
//! irreversible step over a finite branch roster, so both land on its least
//! fixed point. The prospective displacement a sweep measures assumes only its
//! own branch narrows; reflow recomputes every displacement from the committed
//! offsets, and the recomputed value lies between that estimate and zero, so it
//! stays inside `i8`. Were that reasoning ever to fail, the encoders reject an
//! out-of-range rel8 instead of emitting a truncated displacement.

use isa_x86_64::{
    encode_x86_64_selected_i64_less_than_branch_form,
    encode_x86_64_selected_short_nonzero_branch_form,
    encode_x86_64_selected_u64_less_than_branch_form,
};
use optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};
use register_model::ValidatedPhysicalRegisterModel;
use selected_instructions::{MachineAlternativeFamily, MachineAlternativeKey};

use super::super::{
    X86BranchRelaxationAction, X86BranchRelaxationAttempt, X86BranchRelaxationAttemptOutcome,
    X86BranchRelaxationRevisionIdentity,
    error::{OptimizedX86BranchRelaxationError, X86BranchRelaxationWorkAxis},
    identity::{RevisionRoots, revision_identity},
};
use super::{
    RelaxationTrace,
    branch_inspection::inspect_production_branch,
    reflow::reflow_production_functions,
    work::{charge, total_bytes},
};
use machine_code::{
    ResolvedConditionalBranchPredicate, ResolvedSelectedFormRow, ResolvedSelectedFunctionLayout,
};

/// Replacing one six-byte near form with its two-byte short form removes
/// exactly four bytes, so a sweep removes four bytes per committed branch.
const SHORT_FORM_SAVED_BYTES: u64 = 4;

/// One branch a sweep selected, with the prospective short displacement the
/// sweep measured on the sweep's own input layout. That measurement establishes
/// admission; reflow derives the committed displacement from the post-sweep
/// offsets.
struct SweptBranch {
    function_index: usize,
    block_index: usize,
    instruction_index: usize,
    displacement: i64,
}

pub(super) fn compute_trace(
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
        charge(
            &mut usage.iterations,
            budget.iterations(),
            X86BranchRelaxationWorkAxis::Iterations,
        )?;
        let iteration = usage.iterations;
        let input = revision_identity(roots, &functions);
        let previous_bytes = total_bytes(&functions)?;
        let mut swept: Vec<SweptBranch> = Vec::new();
        for (function_index, function) in functions.iter().enumerate() {
            for (block_index, block) in function.blocks.iter().enumerate() {
                for (instruction_index, row) in block.instructions.iter().enumerate() {
                    let Some(branch) = row
                        .branch
                        .as_deref()
                        .and_then(machine_code::ResolvedBranchEvidence::as_conditional)
                    else {
                        continue;
                    };
                    charge(
                        &mut usage.rule_evaluations,
                        budget.rule_evaluations(),
                        X86BranchRelaxationWorkAxis::RuleEvaluations,
                    )?;
                    let (outcome, short_displacement) = inspect_production_branch(
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
                        charge(
                            &mut usage.candidates,
                            budget.candidates(),
                            X86BranchRelaxationWorkAxis::Candidates,
                        )?;
                        charge(
                            &mut usage.validation_steps,
                            budget.validation_steps(),
                            X86BranchRelaxationWorkAxis::ValidationSteps,
                        )?;
                        let displacement = short_displacement.ok_or(
                            OptimizedX86BranchRelaxationError::MalformedBranch(row.instruction),
                        )?;
                        swept.push(SweptBranch {
                            function_index,
                            block_index,
                            instruction_index,
                            displacement,
                        });
                    }
                }
            }
        }
        if swept.is_empty() {
            break;
        }
        let mut narrowed = Vec::with_capacity(swept.len());
        for branch in &swept {
            charge(
                &mut usage.commits,
                budget.commits(),
                X86BranchRelaxationWorkAxis::Commits,
            )?;
            narrowed.push(commit_short_form(&mut functions, branch, physical)?);
        }
        reflow_production_functions(&mut functions, physical)?;
        let expected_saving = u64::try_from(narrowed.len())
            .ok()
            .and_then(|count| count.checked_mul(SHORT_FORM_SAVED_BYTES))
            .ok_or(OptimizedX86BranchRelaxationError::OffsetOverflow)?;
        let current_bytes = total_bytes(&functions)?;
        if previous_bytes.checked_sub(current_bytes) != Some(expected_saving) {
            return Err(OptimizedX86BranchRelaxationError::NonDecreasingByteMeasure);
        }
        let output = revision_identity(roots, &functions);
        for (branch, old) in swept.iter().zip(narrowed) {
            actions.push(relaxation_action(
                &functions, branch, old, iteration, input, output,
            )?);
        }
    }
    Ok(RelaxationTrace {
        usage,
        attempts,
        actions,
        functions,
    })
}

/// Replace one near form with its short form in place and return the row the
/// sweep observed. Offsets and recorded displacements are still the sweep's
/// input values; the single reflow that follows the whole batch republishes
/// them.
fn commit_short_form(
    functions: &mut [ResolvedSelectedFunctionLayout],
    branch: &SweptBranch,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<ResolvedSelectedFormRow, OptimizedX86BranchRelaxationError> {
    let row = &mut functions[branch.function_index].blocks[branch.block_index].instructions
        [branch.instruction_index];
    let old = row.clone();
    let predicate = conditional_evidence(&old)?.predicate;
    let short_alternative = match predicate {
        ResolvedConditionalBranchPredicate::NonZeroV1 => old.alternative,
        ResolvedConditionalBranchPredicate::U64LessThanV1 => MachineAlternativeKey {
            family: MachineAlternativeFamily::ConditionalBranchU64LessThan,
            variant: 1,
        },
        ResolvedConditionalBranchPredicate::I64LessThanV1 => MachineAlternativeKey {
            family: MachineAlternativeFamily::ConditionalBranchI64LessThan,
            variant: 1,
        },
    };
    let encoded = match predicate {
        ResolvedConditionalBranchPredicate::NonZeroV1 => {
            encode_x86_64_selected_short_nonzero_branch_form(
                physical,
                short_alternative,
                branch.displacement,
            )
        }
        ResolvedConditionalBranchPredicate::U64LessThanV1 => {
            encode_x86_64_selected_u64_less_than_branch_form(
                physical,
                short_alternative,
                branch.displacement,
            )
        }
        ResolvedConditionalBranchPredicate::I64LessThanV1 => {
            encode_x86_64_selected_i64_less_than_branch_form(
                physical,
                short_alternative,
                branch.displacement,
            )
        }
    }
    .map_err(OptimizedX86BranchRelaxationError::X86_64)?;
    row.alternative = short_alternative;
    row.bytes = encoded.bytes().to_vec();
    Ok(old)
}

fn relaxation_action(
    functions: &[ResolvedSelectedFunctionLayout],
    branch: &SweptBranch,
    old: ResolvedSelectedFormRow,
    iteration: u64,
    input: X86BranchRelaxationRevisionIdentity,
    output: X86BranchRelaxationRevisionIdentity,
) -> Result<X86BranchRelaxationAction, OptimizedX86BranchRelaxationError> {
    let new = &functions[branch.function_index].blocks[branch.block_index].instructions
        [branch.instruction_index];
    let old_displacement = conditional_evidence(&old)?.byte_displacement;
    let new_displacement = conditional_evidence(new)?.byte_displacement;
    Ok(X86BranchRelaxationAction {
        iteration,
        input,
        output,
        instruction: old.instruction,
        old_offset: old.offset,
        new_offset: new.offset,
        old_displacement,
        new_displacement,
        old_bytes: old.bytes,
        new_bytes: new.bytes.clone(),
    })
}

fn conditional_evidence(
    row: &ResolvedSelectedFormRow,
) -> Result<&machine_code::ResolvedConditionalBranchEvidence, OptimizedX86BranchRelaxationError> {
    row.branch
        .as_deref()
        .and_then(machine_code::ResolvedBranchEvidence::as_conditional)
        .ok_or(OptimizedX86BranchRelaxationError::MalformedBranch(
            row.instruction,
        ))
}
