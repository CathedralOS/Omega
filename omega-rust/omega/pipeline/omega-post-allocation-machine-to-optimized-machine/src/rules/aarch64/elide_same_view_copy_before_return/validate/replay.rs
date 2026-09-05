use omega_optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};
use omega_selected_instructions::{SelectedInstructionKind, SelectedTerminator};

use crate::*;

pub(super) fn replay(
    inputs: &SameViewCopyInputs<'_>,
    budget: OptimizationWorkBudget,
) -> Result<Aarch64SameViewCopyElisionPlan, Aarch64SameViewCopyElisionError> {
    super::roots::validate(inputs)?;
    let mut functions = baseline_roster(inputs.source);
    let mut attempts = Vec::new();
    let mut actions = Vec::new();
    let mut usage = OptimizationWorkUsage::default();

    loop {
        charge(
            &mut usage.iterations,
            budget.iterations(),
            Aarch64SameViewCopyElisionWorkAxis::Iterations,
        )?;
        let iteration = usage.iterations;
        let input = revision(inputs, &functions);
        let mut candidate = None;

        'scan: for (function_index, selected_function) in
            inputs.selected.functions.iter().enumerate()
        {
            let machine_function = inputs.source.functions.get(function_index).ok_or(
                Aarch64SameViewCopyElisionError::FunctionRosterMismatch(function_index),
            )?;
            let live_function = inputs.liveness.functions.get(function_index).ok_or(
                Aarch64SameViewCopyElisionError::FunctionRosterMismatch(function_index),
            )?;
            for (block_index, block) in selected_function.blocks.iter().enumerate() {
                let Some(copy) = block.instructions.last() else {
                    continue;
                };
                if !matches!(copy.kind, SelectedInstructionKind::CopyI64) {
                    continue;
                }
                let SelectedTerminator::Return {
                    instruction: returned,
                    ..
                } = &block.terminator
                else {
                    continue;
                };
                if !matches!(returned.kind, SelectedInstructionKind::ReturnI64) {
                    continue;
                }
                charge(
                    &mut usage.rule_evaluations,
                    budget.rule_evaluations(),
                    Aarch64SameViewCopyElisionWorkAxis::RuleEvaluations,
                )?;
                let machine_block = machine_function.blocks.get(block_index).ok_or(
                    Aarch64SameViewCopyElisionError::BlockRosterMismatch {
                        function: function_index,
                        block: block_index,
                    },
                )?;
                let live_block = live_function.blocks.get(block_index).ok_or(
                    Aarch64SameViewCopyElisionError::BlockRosterMismatch {
                        function: function_index,
                        block: block_index,
                    },
                )?;
                let copy_index = block.instructions.len() - 1;
                let machine_copy = machine_block.instructions.get(copy_index).ok_or(
                    Aarch64SameViewCopyElisionError::InstructionRosterMismatch(copy.id),
                )?;
                let machine_return = machine_block
                    .instructions
                    .get(block.instructions.len())
                    .ok_or(Aarch64SameViewCopyElisionError::InstructionRosterMismatch(
                        returned.id,
                    ))?;
                let live_copy = live_block.instructions.get(copy_index).ok_or(
                    Aarch64SameViewCopyElisionError::LivenessRosterMismatch(copy.id),
                )?;
                let live_return = live_block
                    .instructions
                    .get(block.instructions.len())
                    .ok_or(Aarch64SameViewCopyElisionError::LivenessRosterMismatch(
                        returned.id,
                    ))?;
                let evidence = super::footprints::validate_pair(
                    copy,
                    returned,
                    machine_copy,
                    machine_return,
                    live_copy,
                    live_return,
                    live_block,
                    inputs.physical,
                )?;
                let already_elided =
                    actions
                        .iter()
                        .any(|action: &Aarch64SameViewCopyElisionAction| {
                            action.machine == selected_function.machine
                                && action.block == block.id
                                && action.copy == copy.id
                                && action.consumer == returned.id
                        });
                let outcome = outcome(already_elided, &evidence, copy, returned);
                attempts.push(Aarch64SameViewCopyElisionAttempt {
                    iteration,
                    input,
                    machine: selected_function.machine,
                    block: block.id,
                    copy: copy.id,
                    consumer: returned.id,
                    outcome,
                });
                if outcome == Aarch64SameViewCopyElisionAttemptOutcome::SelectedForElision {
                    charge(
                        &mut usage.candidates,
                        budget.candidates(),
                        Aarch64SameViewCopyElisionWorkAxis::Candidates,
                    )?;
                    charge(
                        &mut usage.validation_steps,
                        budget.validation_steps(),
                        Aarch64SameViewCopyElisionWorkAxis::ValidationSteps,
                    )?;
                    candidate = Some((
                        function_index,
                        block_index,
                        selected_function.machine,
                        block.id,
                        copy,
                        returned,
                        evidence,
                    ));
                    break 'scan;
                }
            }
        }

        let Some((function_index, block_index, machine, block, copy, returned, evidence)) =
            candidate
        else {
            break;
        };
        charge(
            &mut usage.commits,
            budget.commits(),
            Aarch64SameViewCopyElisionWorkAxis::Commits,
        )?;
        apply_disposition(
            &mut functions[function_index].blocks[block_index],
            copy.id,
            returned.id,
        )?;
        let output = revision(inputs, &functions);
        actions.push(Aarch64SameViewCopyElisionAction {
            iteration,
            input,
            output,
            machine,
            block,
            copy: copy.id,
            consumer: returned.id,
            source: evidence.source,
            destination: evidence.destination,
            consumed: evidence.returned,
            source_value: copy.provenance.values[0],
        });
    }

    let output_revision = revision(inputs, &functions);
    let mut plan = Aarch64SameViewCopyElisionPlan {
        identity: Aarch64SameViewCopyElisionIdentity::from_bytes([0; 32]),
        source: inputs.source_identity,
        selected: inputs.selected_identity,
        liveness: inputs.liveness_identity,
        target: inputs.source.target,
        physical_register_model: inputs.physical.identity(),
        policy: Aarch64SameViewCopyElisionPolicy::Aarch64ElideSameViewCopyI64BeforeReturnV1,
        budget,
        usage,
        output_revision,
        attempts,
        actions,
        functions,
    };
    plan.identity = aarch64_same_view_copy_elision_identity(&plan);
    Ok(plan)
}

fn outcome(
    already_elided: bool,
    evidence: &super::footprints::PairEvidence,
    copy: &omega_selected_instructions::SelectedInstruction,
    returned: &omega_selected_instructions::SelectedInstruction,
) -> Aarch64SameViewCopyElisionAttemptOutcome {
    if already_elided {
        Aarch64SameViewCopyElisionAttemptOutcome::AlreadyElided
    } else if !evidence.same_storage {
        Aarch64SameViewCopyElisionAttemptOutcome::DifferentPhysicalStorage
    } else if !evidence.destination_returned {
        Aarch64SameViewCopyElisionAttemptOutcome::DestinationNotConsumed
    } else if semantic_provenance(copy, returned) {
        Aarch64SameViewCopyElisionAttemptOutcome::SemanticProvenance
    } else {
        Aarch64SameViewCopyElisionAttemptOutcome::SelectedForElision
    }
}

fn semantic_provenance(
    copy: &omega_selected_instructions::SelectedInstruction,
    returned: &omega_selected_instructions::SelectedInstruction,
) -> bool {
    let provenance = &copy.provenance;
    !provenance.operations.is_empty()
        || provenance.values.len() != 1
        || !provenance.edges.is_empty()
        || !provenance.obligations.is_empty()
        || !provenance.fuel.is_empty()
        || returned.provenance.values != provenance.values
}

fn baseline_roster(
    source: &omega_physical_instructions::PostAllocationMachinePlan,
) -> Vec<Aarch64SameViewCopyElisionFunction> {
    source
        .functions
        .iter()
        .map(|function| Aarch64SameViewCopyElisionFunction {
            machine: function.machine,
            blocks: function
                .blocks
                .iter()
                .map(|block| Aarch64SameViewCopyElisionBlock {
                    block: block.block,
                    instructions: block
                        .instructions
                        .iter()
                        .map(|instruction| Aarch64SameViewCopyElisionInstruction {
                            instruction: instruction.instruction,
                            disposition: Aarch64SameViewCopyInstructionDisposition::RetainedV1,
                        })
                        .collect(),
                })
                .collect(),
        })
        .collect()
}

fn apply_disposition(
    block: &mut Aarch64SameViewCopyElisionBlock,
    copy: omega_selected_instructions::SelectedInstructionId,
    consumer: omega_selected_instructions::SelectedInstructionId,
) -> Result<(), Aarch64SameViewCopyElisionError> {
    let row = block
        .instructions
        .iter_mut()
        .find(|row| row.instruction == copy)
        .ok_or(Aarch64SameViewCopyElisionError::InstructionRosterMismatch(
            copy,
        ))?;
    row.disposition =
        Aarch64SameViewCopyInstructionDisposition::ElidedSameViewCopyI64V1 { consumer };
    Ok(())
}

fn revision(
    inputs: &SameViewCopyInputs<'_>,
    functions: &[Aarch64SameViewCopyElisionFunction],
) -> Aarch64SameViewCopyElisionRevisionIdentity {
    super::super::super::same_view_copy_elision::revision_identity(
        inputs.source_identity,
        inputs.selected_identity,
        inputs.liveness_identity,
        inputs.source.target,
        inputs.physical.identity(),
        functions,
    )
}

fn charge(
    usage: &mut u64,
    budget: u64,
    axis: Aarch64SameViewCopyElisionWorkAxis,
) -> Result<(), Aarch64SameViewCopyElisionError> {
    *usage = usage
        .checked_add(1)
        .ok_or(Aarch64SameViewCopyElisionError::BudgetExceeded(axis))?;
    if *usage > budget {
        return Err(Aarch64SameViewCopyElisionError::BudgetExceeded(axis));
    }
    Ok(())
}
