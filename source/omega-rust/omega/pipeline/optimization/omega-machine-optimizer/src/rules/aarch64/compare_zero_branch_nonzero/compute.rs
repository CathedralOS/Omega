use omega_optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};
use omega_regalloc::{ValidatedLiveness, ValidatedSelectedAnalysis};
use omega_register_model::{RegisterUnitId, ValidatedPhysicalRegisterModel};
use omega_selected_instructions::{SelectedInstructionKind, SelectedTerminator};
use omega_target::Architecture;

use crate::{
    Aarch64CbnzFusionAction, Aarch64CbnzFusionAttempt, Aarch64CbnzFusionAttemptOutcome,
    Aarch64CbnzFusionBlock, Aarch64CbnzFusionError, Aarch64CbnzFusionFunction,
    Aarch64CbnzFusionIdentity, Aarch64CbnzFusionInstruction, Aarch64CbnzFusionPlan,
    Aarch64CbnzFusionPolicy, Aarch64CbnzFusionWorkAxis, Aarch64CbnzInstructionDisposition,
    QualifiedPhysicalRead, ValidatedPostAllocationMachinePlan, aarch64_cbnz_fusion_identity,
};

use crate::rules::peephole_matching::{TerminalPairMatchError, match_terminal_pair};

pub(crate) fn compute<S: ValidatedSelectedAnalysis>(
    selected: &S,
    liveness: &ValidatedLiveness,
    source: &ValidatedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    budget: OptimizationWorkBudget,
) -> Result<Aarch64CbnzFusionPlan, Aarch64CbnzFusionError> {
    validate_roots(selected, liveness, source, physical)?;
    let selected_plan = selected.selected_plan();
    let machine_plan = source.plan();
    let live_plan = liveness.plan();
    let nzcv = named_units(physical, "nzcv")?;
    let pc = named_units(physical, "pc")?;
    let mut functions = baseline_roster(source);
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
            source.receipt().identity(),
            selected.selected_identity(),
            liveness.receipt().identity(),
            machine_plan.target,
            physical.identity(),
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
                let matched = match_terminal_pair(
                    &super::pattern::AARCH64_CBNZ_TERMINAL_PAIR_V1,
                    compare,
                    branch,
                    machine_compare,
                    machine_branch,
                    live_compare,
                    live_branch,
                    live_block,
                    physical,
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
                } else if !compare.provenance.fuel.is_empty() {
                    Aarch64CbnzFusionAttemptOutcome::CompareCarriesFuel
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
            source.receipt().identity(),
            selected.selected_identity(),
            liveness.receipt().identity(),
            machine_plan.target,
            physical.identity(),
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
        source.receipt().identity(),
        selected.selected_identity(),
        liveness.receipt().identity(),
        machine_plan.target,
        physical.identity(),
        &functions,
    );
    let mut plan = Aarch64CbnzFusionPlan {
        identity: Aarch64CbnzFusionIdentity::from_bytes([0; 32]),
        source: source.receipt().identity(),
        selected: selected.selected_identity(),
        liveness: liveness.receipt().identity(),
        target: machine_plan.target,
        physical_register_model: physical.identity(),
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

fn validate_roots<S: ValidatedSelectedAnalysis>(
    selected: &S,
    liveness: &ValidatedLiveness,
    source: &ValidatedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<(), Aarch64CbnzFusionError> {
    let machine = source.plan();
    if selected.selected_plan().target.architecture != Architecture::Aarch64
        || machine.target.architecture != Architecture::Aarch64
        || physical.model().architecture != Architecture::Aarch64
    {
        return Err(Aarch64CbnzFusionError::UnsupportedTarget(machine.target));
    }
    if machine.selected != selected.selected_identity()
        || selected.selected_plan().target != machine.target
        || liveness.receipt().selected() != selected.selected_identity()
        || liveness.plan().selected != selected.selected_identity()
        || liveness.plan().target != machine.target
        || machine.physical_register_model != physical.identity()
    {
        return Err(Aarch64CbnzFusionError::RootMismatch);
    }
    if selected.selected_plan().functions.len() != machine.functions.len()
        || selected.selected_plan().functions.len() != liveness.plan().functions.len()
    {
        return Err(Aarch64CbnzFusionError::RootMismatch);
    }
    Ok(())
}

fn baseline_roster(source: &ValidatedPostAllocationMachinePlan) -> Vec<Aarch64CbnzFusionFunction> {
    source
        .plan()
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

fn map_match_error(error: TerminalPairMatchError) -> Aarch64CbnzFusionError {
    match error {
        TerminalPairMatchError::MissingArchitecturalView(name) => {
            Aarch64CbnzFusionError::MissingArchitecturalView(name)
        }
        TerminalPairMatchError::FirstRoster(instruction)
        | TerminalPairMatchError::SecondRoster(instruction) => {
            Aarch64CbnzFusionError::InstructionRosterMismatch(instruction)
        }
        TerminalPairMatchError::FirstFootprint(instruction) => {
            Aarch64CbnzFusionError::InvalidCompareFootprint(instruction)
        }
        TerminalPairMatchError::SecondFootprint(instruction) => {
            Aarch64CbnzFusionError::InvalidBranchFootprint(instruction)
        }
        TerminalPairMatchError::FirstPhysicalSource(instruction)
        | TerminalPairMatchError::SecondPhysicalSource(instruction) => {
            Aarch64CbnzFusionError::InvalidPhysicalSource(instruction)
        }
        TerminalPairMatchError::Liveness(instruction) => {
            Aarch64CbnzFusionError::LivenessRosterMismatch(instruction)
        }
    }
}

fn apply_dispositions(
    block: &mut Aarch64CbnzFusionBlock,
    compare: omega_selected_instructions::SelectedInstructionId,
    branch: omega_selected_instructions::SelectedInstructionId,
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
