use omega_optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};
use omega_selected_instructions::SelectedInstructionKind;

use crate::rules::peephole_matching::{
    InstructionPairMatch, InstructionPairMatchError, InstructionPairPattern,
    InstructionPairTopology, OperandRelation, match_instruction_pair,
};
use crate::*;
use omega_physical_instructions::PostAllocationMachinePlan;

use super::{CompareConsumerContract, CompareProvenanceContract};

pub(in crate::rules::aarch64) fn propose(
    inputs: SameViewCopyInputs<'_>,
    budget: OptimizationWorkBudget,
    contract: CompareConsumerContract,
    pattern: &InstructionPairPattern,
) -> Result<Aarch64SameViewCopyElisionPlan, Aarch64SameViewCopyElisionError> {
    super::roots::validate(&inputs)?;
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
                for (copy_index, pair) in block.instructions.windows(2).enumerate() {
                    let copy = &pair[0];
                    let compare = &pair[1];
                    if !matches!(copy.kind, SelectedInstructionKind::CopyI64)
                        || compare.kind != contract.kind
                    {
                        continue;
                    }
                    charge(
                        &mut usage.rule_evaluations,
                        budget.rule_evaluations(),
                        Aarch64SameViewCopyElisionWorkAxis::RuleEvaluations,
                    )?;
                    let machine_copy = machine_block.instructions.get(copy_index).ok_or(
                        Aarch64SameViewCopyElisionError::InstructionRosterMismatch(copy.id),
                    )?;
                    let machine_compare = machine_block.instructions.get(copy_index + 1).ok_or(
                        Aarch64SameViewCopyElisionError::InstructionRosterMismatch(compare.id),
                    )?;
                    let live_copy = live_block.instructions.get(copy_index).ok_or(
                        Aarch64SameViewCopyElisionError::LivenessRosterMismatch(copy.id),
                    )?;
                    let live_compare = live_block.instructions.get(copy_index + 1).ok_or(
                        Aarch64SameViewCopyElisionError::LivenessRosterMismatch(compare.id),
                    )?;
                    let matched = match_instruction_pair(
                        pattern,
                        InstructionPairTopology::AdjacentBodyInstructionsV1,
                        copy,
                        compare,
                        machine_copy,
                        machine_compare,
                        live_copy,
                        live_compare,
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
                                    && action.consumer == compare.id
                            });
                    let outcome = outcome(
                        already_elided,
                        &matched,
                        selected_function,
                        contract,
                        copy,
                        compare,
                    );
                    attempts.push(Aarch64SameViewCopyElisionAttempt {
                        iteration,
                        input,
                        machine: selected_function.machine,
                        block: block.id,
                        copy: copy.id,
                        consumer: compare.id,
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
                            compare,
                            matched,
                        ));
                        break 'scan;
                    }
                }
            }
        }

        let Some((function_index, block_index, machine, block, copy, compare, matched)) = candidate
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
            compare.id,
        )?;
        let output = revision(&inputs, &functions);
        actions.push(Aarch64SameViewCopyElisionAction {
            iteration,
            input,
            output,
            machine,
            block,
            copy: copy.id,
            consumer: compare.id,
            source: qualified(matched.first_read(0).expect("descriptor matched source")),
            destination: qualified(
                matched
                    .first_read(1)
                    .expect("descriptor matched destination"),
            ),
            consumed: qualified(
                matched
                    .second_read(contract.consumed_operand as u16)
                    .expect("descriptor matched compare input"),
            ),
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
        policy: contract.policy,
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
    function: &omega_selected_instructions::SelectedFunction,
    contract: CompareConsumerContract,
    copy: &omega_selected_instructions::SelectedInstruction,
    compare: &omega_selected_instructions::SelectedInstruction,
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
    if semantic_provenance(function, contract, copy, compare) {
        Aarch64SameViewCopyElisionAttemptOutcome::SemanticProvenance
    } else {
        Aarch64SameViewCopyElisionAttemptOutcome::SelectedForElision
    }
}

fn semantic_provenance(
    function: &omega_selected_instructions::SelectedFunction,
    contract: CompareConsumerContract,
    copy: &omega_selected_instructions::SelectedInstruction,
    compare: &omega_selected_instructions::SelectedInstruction,
) -> bool {
    let provenance = &copy.provenance;
    if !provenance.operations.is_empty()
        || provenance.values.len() != 1
        || !provenance.edges.is_empty()
        || !provenance.obligations.is_empty()
        || !provenance.fuel.is_empty()
    {
        return true;
    }
    match contract.provenance {
        CompareProvenanceContract::ExactCopyValue => compare.provenance.values != provenance.values,
        CompareProvenanceContract::ConsumedOriginAndRetainedValue => {
            !compare.provenance.values.contains(&provenance.values[0])
                || source_value(
                    function,
                    compare.operands[contract.consumed_operand].virtual_register,
                ) != Some(provenance.values[0])
        }
    }
}

fn source_value(
    function: &omega_selected_instructions::SelectedFunction,
    virtual_register: omega_selected_instructions::VirtualRegisterId,
) -> Option<psi_core::ValueId> {
    function
        .virtual_registers
        .iter()
        .find(|register| register.id == virtual_register)
        .map(|register| match register.origin {
            omega_selected_instructions::VirtualRegisterOrigin::EntryParameter {
                source_value,
                ..
            }
            | omega_selected_instructions::VirtualRegisterOrigin::InstructionResult {
                source_value,
                ..
            }
            | omega_selected_instructions::VirtualRegisterOrigin::LegalizationTemporary {
                source_value,
                ..
            } => source_value,
        })
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
    crate::rules::aarch64::same_view_copy_elision::revision_identity(
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
            Aarch64SameViewCopyElisionError::InvalidCompareFootprint(instruction)
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
