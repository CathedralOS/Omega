use optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};
use register_model::ValidatedPhysicalRegisterModel;
use selected_instructions::{SelectedInstructionKind, SelectedTerminator};
use selected_instructions_to_register_homes::{ValidatedLiveness, ValidatedSelectedAnalysis};
use target::Architecture;

use crate::rules::peephole_matching::{
    InstructionPairMatch, InstructionPairMatchError, InstructionPairTopology, OperandRelation,
    match_instruction_pair,
};
use crate::*;
use physical_instructions::PostAllocationMachinePlan;
use register_homes_to_post_allocation_machine::ValidatedPostAllocationMachinePlan;

pub(crate) fn compute<S: ValidatedSelectedAnalysis>(
    selected: &S,
    liveness: &ValidatedLiveness,
    source: &ValidatedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    budget: OptimizationWorkBudget,
) -> Result<Aarch64SameViewCopyElisionPlan, Aarch64SameViewCopyElisionError> {
    compute_from_inputs(
        SameViewCopyInputs {
            selected: selected.selected_plan(),
            selected_identity: selected.selected_identity(),
            liveness: liveness.plan(),
            liveness_identity: liveness.receipt().identity(),
            source: source.plan(),
            source_identity: source.receipt().identity(),
            physical,
        },
        budget,
    )
}

pub(crate) fn compute_from_inputs(
    inputs: SameViewCopyInputs<'_>,
    budget: OptimizationWorkBudget,
) -> Result<Aarch64SameViewCopyElisionPlan, Aarch64SameViewCopyElisionError> {
    validate_roots(&inputs)?;
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
        let input = revision(&inputs, &functions);
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
                    instruction: return_instruction,
                    ..
                } = &block.terminator
                else {
                    continue;
                };
                if !matches!(return_instruction.kind, SelectedInstructionKind::ReturnI64) {
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
                        return_instruction.id,
                    ))?;
                let live_copy = live_block.instructions.get(copy_index).ok_or(
                    Aarch64SameViewCopyElisionError::LivenessRosterMismatch(copy.id),
                )?;
                let live_return = live_block
                    .instructions
                    .get(block.instructions.len())
                    .ok_or(Aarch64SameViewCopyElisionError::LivenessRosterMismatch(
                        return_instruction.id,
                    ))?;
                let matched = match_instruction_pair(
                    &super::pattern::AARCH64_SAME_VIEW_COPY_BEFORE_RETURN_V1,
                    InstructionPairTopology::BodyTailAndTerminatorV1,
                    copy,
                    return_instruction,
                    machine_copy,
                    machine_return,
                    live_copy,
                    live_return,
                    live_block,
                    inputs.physical,
                )
                .map_err(map_match_error)?;
                let already_elided =
                    actions
                        .iter()
                        .any(|action: &Aarch64SameViewCopyElisionAction| {
                            action.machine == selected_function.machine
                                && action.block == block.id
                                && action.copy == copy.id
                                && action.consumer == return_instruction.id
                        });
                let outcome = outcome(already_elided, &matched, copy, return_instruction);
                attempts.push(Aarch64SameViewCopyElisionAttempt {
                    iteration,
                    input,
                    machine: selected_function.machine,
                    block: block.id,
                    copy: copy.id,
                    consumer: return_instruction.id,
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
                        return_instruction,
                        matched,
                    ));
                    break 'scan;
                }
            }
        }

        let Some((function_index, block_index, machine, block, copy, returned, matched)) =
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
        let output = revision(&inputs, &functions);
        actions.push(Aarch64SameViewCopyElisionAction {
            iteration,
            input,
            output,
            machine,
            block,
            copy: copy.id,
            consumer: returned.id,
            source: qualified(matched.first_read(0).unwrap()),
            destination: qualified(matched.first_read(1).unwrap()),
            consumed: qualified(matched.second_read(0).unwrap()),
            source_value: copy.provenance.values[0],
        });
    }

    let output_revision = revision(&inputs, &functions);
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
    matched: &InstructionPairMatch,
    copy: &selected_instructions::SelectedInstruction,
    returned: &selected_instructions::SelectedInstruction,
) -> Aarch64SameViewCopyElisionAttemptOutcome {
    if already_elided {
        return Aarch64SameViewCopyElisionAttemptOutcome::AlreadyElided;
    }
    if let Some(relation) = matched.failed_relations().iter().next() {
        match relation {
            OperandRelation::SamePhysicalViewAndStorageUnits(_, _) => {
                return Aarch64SameViewCopyElisionAttemptOutcome::DifferentPhysicalStorage;
            }
            OperandRelation::SameVirtualRegister(_, _) => {
                return Aarch64SameViewCopyElisionAttemptOutcome::DestinationNotConsumed;
            }
        }
    }
    let provenance = &copy.provenance;
    if !provenance.operations.is_empty()
        || provenance.values.len() != 1
        || !provenance.edges.is_empty()
        || !provenance.obligations.is_empty()
        || !provenance.fuel.is_empty()
        || returned.provenance.values != provenance.values
    {
        Aarch64SameViewCopyElisionAttemptOutcome::SemanticProvenance
    } else {
        Aarch64SameViewCopyElisionAttemptOutcome::SelectedForElision
    }
}

fn validate_roots(inputs: &SameViewCopyInputs<'_>) -> Result<(), Aarch64SameViewCopyElisionError> {
    if inputs.selected.target.architecture != Architecture::Aarch64
        || inputs.source.target.architecture != Architecture::Aarch64
        || inputs.physical.model().architecture != Architecture::Aarch64
    {
        return Err(Aarch64SameViewCopyElisionError::UnsupportedTarget(
            inputs.source.target,
        ));
    }
    if inputs.source.identity != inputs.source_identity
        || inputs.source.selected != inputs.selected_identity
        || inputs.selected.target != inputs.source.target
        || inputs.liveness.selected != inputs.selected_identity
        || inputs.liveness.target != inputs.source.target
        || inputs.source.physical_register_model != inputs.physical.identity()
        || inputs.selected.functions.len() != inputs.source.functions.len()
        || inputs.selected.functions.len() != inputs.liveness.functions.len()
    {
        return Err(Aarch64SameViewCopyElisionError::RootMismatch);
    }
    Ok(())
}

fn baseline_roster(source: &PostAllocationMachinePlan) -> Vec<Aarch64SameViewCopyElisionFunction> {
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
    copy: selected_instructions::SelectedInstructionId,
    consumer: selected_instructions::SelectedInstructionId,
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

fn qualified(
    read: &crate::rules::peephole_matching::MatchedPhysicalRead,
) -> QualifiedPhysicalOperand {
    QualifiedPhysicalOperand {
        instruction: read.source_instruction,
        operand: read.operand,
        virtual_register: read.virtual_register,
        class: read.class,
        view: read.view,
        storage_units: read.storage_units.clone(),
    }
}

fn revision(
    inputs: &SameViewCopyInputs<'_>,
    functions: &[Aarch64SameViewCopyElisionFunction],
) -> Aarch64SameViewCopyElisionRevisionIdentity {
    super::super::same_view_copy_elision::revision_identity(
        inputs.source_identity,
        inputs.selected_identity,
        inputs.liveness_identity,
        inputs.source.target,
        inputs.physical.identity(),
        functions,
    )
}

fn map_match_error(error: InstructionPairMatchError) -> Aarch64SameViewCopyElisionError {
    match error {
        InstructionPairMatchError::MissingArchitecturalView(name) => {
            Aarch64SameViewCopyElisionError::MissingArchitecturalView(name)
        }
        InstructionPairMatchError::FirstRoster(instruction)
        | InstructionPairMatchError::SecondRoster(instruction) => {
            Aarch64SameViewCopyElisionError::InstructionRosterMismatch(instruction)
        }
        InstructionPairMatchError::FirstFootprint(instruction) => {
            Aarch64SameViewCopyElisionError::InvalidCopyFootprint(instruction)
        }
        InstructionPairMatchError::SecondFootprint(instruction) => {
            Aarch64SameViewCopyElisionError::InvalidReturnFootprint(instruction)
        }
        InstructionPairMatchError::FirstPhysicalSource(instruction)
        | InstructionPairMatchError::SecondPhysicalSource(instruction) => {
            Aarch64SameViewCopyElisionError::InvalidPhysicalOperand(instruction)
        }
        InstructionPairMatchError::Liveness(instruction) => {
            Aarch64SameViewCopyElisionError::LivenessRosterMismatch(instruction)
        }
        InstructionPairMatchError::Topology => Aarch64SameViewCopyElisionError::ArtifactMismatch,
    }
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
