use std::collections::BTreeSet;

use omega_optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};
use omega_regalloc::{
    BlockLiveness, InstructionLiveness, ValidatedLiveness, ValidatedSelectedAnalysis,
};
use omega_register_model::{RegisterOperandAccess, RegisterUnitId, ValidatedPhysicalRegisterModel};
use omega_selected_instructions::{
    MachineAlternativeFamily, MachineEncodedControlEffect, MachineEncodedMemoryEffect,
    MachineEncodedStackEffect, MachineEncodedTrapBehavior, SelectedInstruction,
    SelectedInstructionKind, SelectedTerminator,
};
use omega_target::Architecture;

use crate::{
    Aarch64CbnzFusionAction, Aarch64CbnzFusionAttempt, Aarch64CbnzFusionAttemptOutcome,
    Aarch64CbnzFusionBlock, Aarch64CbnzFusionError, Aarch64CbnzFusionFunction,
    Aarch64CbnzFusionIdentity, Aarch64CbnzFusionInstruction, Aarch64CbnzFusionPlan,
    Aarch64CbnzFusionPolicy, Aarch64CbnzFusionWorkAxis, Aarch64CbnzInstructionDisposition,
    PhysicalOperandFootprint, PostAllocationMachineInstruction, QualifiedPhysicalRead,
    ValidatedPostAllocationMachinePlan, aarch64_cbnz_fusion_identity,
};

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
                validate_pair(
                    compare,
                    branch,
                    machine_compare,
                    machine_branch,
                    live_compare,
                    live_branch,
                    physical,
                    &nzcv,
                    &pc,
                )?;
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
                } else if flags_live_out(live_block, live_branch, &nzcv) {
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
                        machine_compare,
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
            machine_compare,
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
        let source_read = qualified_read(compare, machine_compare)?;
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

#[allow(clippy::too_many_arguments)]
fn validate_pair(
    compare: &SelectedInstruction,
    branch: &SelectedInstruction,
    machine_compare: &PostAllocationMachineInstruction,
    machine_branch: &PostAllocationMachineInstruction,
    live_compare: &InstructionLiveness,
    live_branch: &InstructionLiveness,
    physical: &ValidatedPhysicalRegisterModel,
    nzcv: &[RegisterUnitId],
    pc: &[RegisterUnitId],
) -> Result<(), Aarch64CbnzFusionError> {
    if machine_compare.instruction != compare.id
        || live_compare.instruction != compare.id
        || !matches!(compare.kind, SelectedInstructionKind::CompareI64Zero)
        || compare.operands.len() != 1
    {
        return Err(Aarch64CbnzFusionError::InstructionRosterMismatch(
            compare.id,
        ));
    }
    if machine_branch.instruction != branch.id
        || live_branch.instruction != branch.id
        || !matches!(
            branch.kind,
            SelectedInstructionKind::ConditionalBranchNonZero
        )
        || !branch.operands.is_empty()
    {
        return Err(Aarch64CbnzFusionError::InstructionRosterMismatch(branch.id));
    }
    validate_compare(compare, machine_compare, physical, nzcv)?;
    validate_branch(branch, machine_branch, nzcv, pc)?;
    if live_compare.unit_live_out != live_branch.unit_live_in
        || !nzcv
            .iter()
            .all(|unit| live_compare.unit_live_out.contains(unit))
        || !nzcv.iter().all(|unit| live_branch.unit_uses.contains(unit))
    {
        return Err(Aarch64CbnzFusionError::LivenessRosterMismatch(branch.id));
    }
    Ok(())
}

fn validate_compare(
    selected: &SelectedInstruction,
    machine: &PostAllocationMachineInstruction,
    physical: &ValidatedPhysicalRegisterModel,
    nzcv: &[RegisterUnitId],
) -> Result<(), Aarch64CbnzFusionError> {
    let encoded = &machine.alternative.encoded;
    if machine.alternative.key.family != MachineAlternativeFamily::CompareI64Zero
        || machine.alternative.key.variant != 0
        || encoded.external_operand_reads != [0]
        || !encoded.external_operand_writes.is_empty()
        || !encoded.implicit_unit_uses.is_empty()
        || encoded.implicit_unit_defs != nzcv
        || !encoded.implicit_unit_clobbers.is_empty()
        || encoded.memory != MachineEncodedMemoryEffect::NoneV1
        || encoded.stack != MachineEncodedStackEffect::UnchangedV1
        || encoded.trap != MachineEncodedTrapBehavior::NeverV1
        || encoded.control != MachineEncodedControlEffect::FallThroughV1
        || machine.operands.len() != 1
    {
        return Err(Aarch64CbnzFusionError::InvalidCompareFootprint(selected.id));
    }
    let selected_operand = &selected.operands[0];
    let operand = &machine.operands[0];
    if selected_operand.operand != 0
        || selected_operand.access != RegisterOperandAccess::Use
        || operand.operand != 0
        || operand.virtual_register != selected_operand.virtual_register
        || operand.class != selected_operand.class
        || operand.access != RegisterOperandAccess::Use
        || operand.read_units != operand.storage_units
        || !operand.write_units.is_empty()
        || operand.write_semantics.is_some()
        || machine.implicit_unit_defs != nzcv
    {
        return Err(Aarch64CbnzFusionError::InvalidCompareFootprint(selected.id));
    }
    validate_x_view(operand, physical, selected.id)
}

fn validate_branch(
    selected: &SelectedInstruction,
    machine: &PostAllocationMachineInstruction,
    nzcv: &[RegisterUnitId],
    pc: &[RegisterUnitId],
) -> Result<(), Aarch64CbnzFusionError> {
    let encoded = &machine.alternative.encoded;
    let expected_uses = union(nzcv, pc);
    if machine.alternative.key.family != MachineAlternativeFamily::ConditionalBranchNonZero
        || machine.alternative.key.variant != 0
        || !machine.operands.is_empty()
        || !encoded.external_operand_reads.is_empty()
        || !encoded.external_operand_writes.is_empty()
        || encoded.implicit_unit_uses != expected_uses
        || encoded.implicit_unit_defs != pc
        || !encoded.implicit_unit_clobbers.is_empty()
        || encoded.memory != MachineEncodedMemoryEffect::NoneV1
        || encoded.stack != MachineEncodedStackEffect::UnchangedV1
        || encoded.trap != MachineEncodedTrapBehavior::MayArchitecturalFaultV1
        || encoded.control != MachineEncodedControlEffect::ConditionalRelativeBranchV1
        || machine.implicit_unit_uses != expected_uses
        || machine.implicit_unit_defs != pc
    {
        return Err(Aarch64CbnzFusionError::InvalidBranchFootprint(selected.id));
    }
    Ok(())
}

fn validate_x_view(
    operand: &PhysicalOperandFootprint,
    physical: &ValidatedPhysicalRegisterModel,
    instruction: omega_selected_instructions::SelectedInstructionId,
) -> Result<(), Aarch64CbnzFusionError> {
    let view = physical
        .model()
        .views
        .iter()
        .find(|view| view.id == operand.view)
        .ok_or(Aarch64CbnzFusionError::InvalidPhysicalSource(instruction))?;
    let valid_index = view
        .name
        .strip_prefix('x')
        .and_then(|name| name.parse::<u8>().ok())
        .is_some_and(|index| index <= 30);
    if !valid_index
        || view.bits != 64
        || !view.allocatable
        || view.class != operand.class
        || view.units != operand.storage_units
    {
        return Err(Aarch64CbnzFusionError::InvalidPhysicalSource(instruction));
    }
    Ok(())
}

fn flags_live_out(
    block: &BlockLiveness,
    branch: &InstructionLiveness,
    nzcv: &[RegisterUnitId],
) -> bool {
    branch.unit_live_out.iter().any(|unit| nzcv.contains(unit))
        || block.unit_live_out.iter().any(|unit| nzcv.contains(unit))
        || block
            .successors
            .iter()
            .flat_map(|successor| &successor.unit_live)
            .any(|unit| nzcv.contains(unit))
}

fn qualified_read(
    selected: &SelectedInstruction,
    machine: &PostAllocationMachineInstruction,
) -> Result<QualifiedPhysicalRead, Aarch64CbnzFusionError> {
    let operand = machine
        .operands
        .first()
        .ok_or(Aarch64CbnzFusionError::InvalidCompareFootprint(selected.id))?;
    Ok(QualifiedPhysicalRead {
        source_instruction: selected.id,
        operand: operand.operand,
        virtual_register: operand.virtual_register,
        class: operand.class,
        view: operand.view,
        units: operand.read_units.clone(),
    })
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

fn union(left: &[RegisterUnitId], right: &[RegisterUnitId]) -> Vec<RegisterUnitId> {
    left.iter()
        .chain(right)
        .copied()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
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
