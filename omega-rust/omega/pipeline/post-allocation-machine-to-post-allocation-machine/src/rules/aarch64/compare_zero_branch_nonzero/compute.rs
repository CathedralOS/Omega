use optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};
use register_model::{RegisterUnitId, ValidatedPhysicalRegisterModel};
use selected_instructions::{SelectedInstructionKind, SelectedTerminator};
use selected_instructions_to_register_homes::{ValidatedLiveness, ValidatedSelectedAnalysis};
use target::Architecture;

use crate::{
    Aarch64CbnzFusionAction, Aarch64CbnzFusionAttempt, Aarch64CbnzFusionAttemptOutcome,
    Aarch64CbnzFusionBlock, Aarch64CbnzFusionError, Aarch64CbnzFusionFunction,
    Aarch64CbnzFusionInstruction, Aarch64CbnzFusionPlan, Aarch64CbnzFusionPolicy,
    Aarch64CbnzFusionWorkAxis, Aarch64CbnzInstructionDisposition, CbnzFusionInputs,
    QualifiedPhysicalRead, aarch64_cbnz_fusion_identity,
};
use physical_instructions::Aarch64CbnzFusionIdentity;
use register_homes_to_post_allocation_machine::ValidatedPostAllocationMachinePlan;

use crate::rules::peephole_matching::{
    InstructionPairMatchError, InstructionPairTopology, match_instruction_pair,
};

pub(crate) fn compute<S: ValidatedSelectedAnalysis>(
    selected: &S,
    liveness: &ValidatedLiveness,
    source: &ValidatedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    budget: OptimizationWorkBudget,
) -> Result<Aarch64CbnzFusionPlan, Aarch64CbnzFusionError> {
    compute_from_inputs(
        CbnzFusionInputs {
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
    inputs: CbnzFusionInputs<'_>,
    budget: OptimizationWorkBudget,
) -> Result<Aarch64CbnzFusionPlan, Aarch64CbnzFusionError> {
    validate_roots(&inputs)?;
    let selected_plan = inputs.selected;
    let machine_plan = inputs.source;
    let live_plan = inputs.liveness;
    let nzcv = named_units(inputs.physical, "nzcv")?;
    let pc = named_units(inputs.physical, "pc")?;
    let mut functions = baseline_roster(machine_plan);
    let mut attempts = Vec::new();
    let mut actions = Vec::new();
    let mut usage = OptimizationWorkUsage::default();

    loop {
        charge(
            &mut usage.iterations,
            budget.iterations(),
            Aarch64CbnzFusionWorkAxis::Iterations,
        )?;
        let iteration = usage.iterations;
        let input = super::identity::revision_identity(
            inputs.source_identity,
            inputs.selected_identity,
            inputs.liveness_identity,
            machine_plan.target,
            inputs.physical.identity(),
            &functions,
        );
        let mut selected_candidate = None;
        'scan: for (function_index, selected_function) in selected_plan.functions.iter().enumerate()
        {
            let machine_function = machine_plan.functions.get(function_index).ok_or(
                Aarch64CbnzFusionError::FunctionRosterMismatch(function_index),
            )?;
            let live_function = live_plan.functions.get(function_index).ok_or(
                Aarch64CbnzFusionError::FunctionRosterMismatch(function_index),
            )?;
            for (block_index, block) in selected_function.blocks.iter().enumerate() {
                let Some(compare) = block.instructions.last() else {
                    continue;
                };
                if !matches!(compare.kind, SelectedInstructionKind::CompareI64Zero) {
                    continue;
                }
                let SelectedTerminator::ConditionalBranch {
                    instruction: branch,
                    when_nonzero,
                    when_zero,
                } = &block.terminator
                else {
                    continue;
                };
                charge(
                    &mut usage.rule_evaluations,
                    budget.rule_evaluations(),
                    Aarch64CbnzFusionWorkAxis::RuleEvaluations,
                )?;
                let machine_block = machine_function.blocks.get(block_index).ok_or(
                    Aarch64CbnzFusionError::BlockRosterMismatch {
                        function: function_index,
                        block: block_index,
                    },
                )?;
                let live_block = live_function.blocks.get(block_index).ok_or(
                    Aarch64CbnzFusionError::BlockRosterMismatch {
                        function: function_index,
                        block: block_index,
                    },
                )?;
                let compare_index = block.instructions.len() - 1;
                let machine_compare = machine_block.instructions.get(compare_index).ok_or(
                    Aarch64CbnzFusionError::InstructionRosterMismatch(compare.id),
                )?;
                let machine_branch = machine_block
                    .instructions
                    .get(block.instructions.len())
                    .ok_or(Aarch64CbnzFusionError::InstructionRosterMismatch(branch.id))?;
                let live_compare = live_block
                    .instructions
                    .get(compare_index)
                    .ok_or(Aarch64CbnzFusionError::LivenessRosterMismatch(compare.id))?;
                let live_branch = live_block
                    .instructions
                    .get(block.instructions.len())
                    .ok_or(Aarch64CbnzFusionError::LivenessRosterMismatch(branch.id))?;
                let matched = match_instruction_pair(
                    &super::pattern::AARCH64_CBNZ_INSTRUCTION_PAIR_V1,
                    InstructionPairTopology::BodyTailAndTerminatorV1,
                    compare,
                    branch,
                    machine_compare,
                    machine_branch,
                    live_compare,
                    live_branch,
                    live_block,
                    inputs.physical,
                )
                .map_err(map_match_error)?;
                let read = matched
                    .first_read(0)
                    .ok_or(Aarch64CbnzFusionError::InvalidCompareFootprint(compare.id))?;
                let source_read = QualifiedPhysicalRead {
                    source_instruction: read.source_instruction,
                    operand: read.operand,
                    virtual_register: read.virtual_register,
                    class: read.class,
                    view: read.view,
                    units: read.units.clone(),
                };
                let already_fused = actions.iter().any(|action: &Aarch64CbnzFusionAction| {
                    action.machine == selected_function.machine
                        && action.block == block.id
                        && action.compare == compare.id
                        && action.branch == branch.id
                });
                let outcome = if already_fused {
                    Aarch64CbnzFusionAttemptOutcome::AlreadyFused
                } else if matched.dead_sets_live_out() {
                    Aarch64CbnzFusionAttemptOutcome::NzcvLiveOut
                } else {
                    Aarch64CbnzFusionAttemptOutcome::SelectedForFusion
                };
                attempts.push(Aarch64CbnzFusionAttempt {
                    iteration,
                    input,
                    machine: selected_function.machine,
                    block: block.id,
                    compare: compare.id,
                    branch: branch.id,
                    outcome,
                });
                if outcome == Aarch64CbnzFusionAttemptOutcome::SelectedForFusion {
                    charge(
                        &mut usage.candidates,
                        budget.candidates(),
                        Aarch64CbnzFusionWorkAxis::Candidates,
                    )?;
                    charge(
                        &mut usage.validation_steps,
                        budget.validation_steps(),
                        Aarch64CbnzFusionWorkAxis::ValidationSteps,
                    )?;
                    selected_candidate = Some((
                        function_index,
                        block_index,
                        selected_function.machine,
                        block,
                        compare,
                        branch,
                        source_read,
                        when_nonzero,
                        when_zero,
                    ));
                    break 'scan;
                }
            }
        }
        let Some((
            function_index,
            block_index,
            machine,
            block,
            compare,
            branch,
            source_read,
            when_nonzero,
            when_zero,
        )) = selected_candidate
        else {
            break;
        };
        charge(
            &mut usage.commits,
            budget.commits(),
            Aarch64CbnzFusionWorkAxis::Commits,
        )?;
        apply_dispositions(
            &mut functions[function_index].blocks[block_index],
            compare.id,
            branch.id,
            &source_read,
        )?;
        let output = super::identity::revision_identity(
            inputs.source_identity,
            inputs.selected_identity,
            inputs.liveness_identity,
            machine_plan.target,
            inputs.physical.identity(),
            &functions,
        );
        actions.push(Aarch64CbnzFusionAction {
            iteration,
            input,
            output,
            machine,
            block: block.id,
            compare: compare.id,
            branch: branch.id,
            source_read,
            nzcv_units: nzcv.clone(),
            pc_units: pc.clone(),
            when_nonzero_edge: when_nonzero.psi_edge,
            when_nonzero_block: when_nonzero.block,
            when_zero_edge: when_zero.psi_edge,
            when_zero_block: when_zero.block,
        });
    }

    let output_revision = super::identity::revision_identity(
        inputs.source_identity,
        inputs.selected_identity,
        inputs.liveness_identity,
        machine_plan.target,
        inputs.physical.identity(),
        &functions,
    );
    let mut plan = Aarch64CbnzFusionPlan {
        identity: Aarch64CbnzFusionIdentity::from_bytes([0; 32]),
        source: inputs.source_identity,
        selected: inputs.selected_identity,
        liveness: inputs.liveness_identity,
        target: machine_plan.target,
        physical_register_model: inputs.physical.identity(),
        policy: Aarch64CbnzFusionPolicy::Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1,
        budget,
        usage,
        output_revision,
        attempts,
        actions,
        functions,
    };
    plan.identity = aarch64_cbnz_fusion_identity(&plan);
    Ok(plan)
}

fn validate_roots(inputs: &CbnzFusionInputs<'_>) -> Result<(), Aarch64CbnzFusionError> {
    let machine = inputs.source;
    if inputs.selected.target.architecture != Architecture::Aarch64
        || machine.target.architecture != Architecture::Aarch64
        || inputs.physical.model().architecture != Architecture::Aarch64
    {
        return Err(Aarch64CbnzFusionError::UnsupportedTarget(machine.target));
    }
    if machine.identity != inputs.source_identity
        || machine.selected != inputs.selected_identity
        || inputs.selected.target != machine.target
        || selected_instructions_to_register_homes::liveness_identity(inputs.liveness)
            != inputs.liveness_identity
        || inputs.liveness.selected != inputs.selected_identity
        || inputs.liveness.target != machine.target
        || machine.physical_register_model != inputs.physical.identity()
    {
        return Err(Aarch64CbnzFusionError::RootMismatch);
    }
    if inputs.selected.functions.len() != machine.functions.len()
        || inputs.selected.functions.len() != inputs.liveness.functions.len()
    {
        return Err(Aarch64CbnzFusionError::RootMismatch);
    }
    Ok(())
}

fn baseline_roster(
    source: &physical_instructions::PostAllocationMachinePlan,
) -> Vec<Aarch64CbnzFusionFunction> {
    source
        .functions
        .iter()
        .map(|function| Aarch64CbnzFusionFunction {
            machine: function.machine,
            blocks: function
                .blocks
                .iter()
                .map(|block| Aarch64CbnzFusionBlock {
                    block: block.block,
                    instructions: block
                        .instructions
                        .iter()
                        .map(|instruction| Aarch64CbnzFusionInstruction {
                            instruction: instruction.instruction,
                            disposition: Aarch64CbnzInstructionDisposition::RetainedV1,
                        })
                        .collect(),
                })
                .collect(),
        })
        .collect()
}

fn map_match_error(error: InstructionPairMatchError) -> Aarch64CbnzFusionError {
    match error {
        InstructionPairMatchError::MissingArchitecturalView(name) => {
            Aarch64CbnzFusionError::MissingArchitecturalView(name)
        }
        InstructionPairMatchError::FirstRoster(instruction)
        | InstructionPairMatchError::SecondRoster(instruction) => {
            Aarch64CbnzFusionError::InstructionRosterMismatch(instruction)
        }
        InstructionPairMatchError::FirstFootprint(instruction) => {
            Aarch64CbnzFusionError::InvalidCompareFootprint(instruction)
        }
        InstructionPairMatchError::SecondFootprint(instruction) => {
            Aarch64CbnzFusionError::InvalidBranchFootprint(instruction)
        }
        InstructionPairMatchError::FirstPhysicalSource(instruction)
        | InstructionPairMatchError::SecondPhysicalSource(instruction) => {
            Aarch64CbnzFusionError::InvalidPhysicalSource(instruction)
        }
        InstructionPairMatchError::Liveness(instruction) => {
            Aarch64CbnzFusionError::LivenessRosterMismatch(instruction)
        }
        InstructionPairMatchError::Topology => Aarch64CbnzFusionError::ArtifactMismatch,
    }
}

fn apply_dispositions(
    block: &mut Aarch64CbnzFusionBlock,
    compare: selected_instructions::SelectedInstructionId,
    branch: selected_instructions::SelectedInstructionId,
    read: &QualifiedPhysicalRead,
) -> Result<(), Aarch64CbnzFusionError> {
    let compare_row = block
        .instructions
        .iter_mut()
        .find(|row| row.instruction == compare)
        .ok_or(Aarch64CbnzFusionError::InstructionRosterMismatch(compare))?;
    compare_row.disposition =
        Aarch64CbnzInstructionDisposition::ElidedCompareI64ZeroV1 { consumer: branch };
    let branch_row = block
        .instructions
        .iter_mut()
        .find(|row| row.instruction == branch)
        .ok_or(Aarch64CbnzFusionError::InstructionRosterMismatch(branch))?;
    branch_row.disposition = Aarch64CbnzInstructionDisposition::FusedBranchNonZeroToCbnzV1 {
        compare,
        source_read: read.clone(),
    };
    Ok(())
}

fn named_units(
    physical: &ValidatedPhysicalRegisterModel,
    name: &'static str,
) -> Result<Vec<RegisterUnitId>, Aarch64CbnzFusionError> {
    physical
        .model()
        .view_named(name)
        .map(|view| view.units.clone())
        .ok_or(Aarch64CbnzFusionError::MissingArchitecturalView(name))
}

fn charge(
    usage: &mut u64,
    budget: u64,
    axis: Aarch64CbnzFusionWorkAxis,
) -> Result<(), Aarch64CbnzFusionError> {
    *usage = usage
        .checked_add(1)
        .ok_or(Aarch64CbnzFusionError::BudgetExceeded(axis))?;
    if *usage > budget {
        return Err(Aarch64CbnzFusionError::BudgetExceeded(axis));
    }
    Ok(())
}
