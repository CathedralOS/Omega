use std::collections::BTreeSet;

use omega_optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};
use omega_regalloc::{
    TerminalBlockLiveness, TerminalInstructionLiveness, ValidatedTerminalLiveness,
    ValidatedTerminalSelectedAnalysis,
};
use omega_register_model::{RegisterOperandAccess, RegisterUnitId, ValidatedPhysicalRegisterModel};
use omega_target::Architecture;
use omega_terminal_selected_instructions::{
    TerminalMachineAlternativeFamily, TerminalMachineEncodedControlEffect,
    TerminalMachineEncodedMemoryEffect, TerminalMachineEncodedStackEffect,
    TerminalMachineEncodedTrapBehavior, TerminalSelectedInstruction,
    TerminalSelectedInstructionKind, TerminalSelectedTerminator,
};

use crate::{
    TerminalAarch64CbnzFusionAction, TerminalAarch64CbnzFusionAttempt,
    TerminalAarch64CbnzFusionAttemptOutcome, TerminalAarch64CbnzFusionBlock,
    TerminalAarch64CbnzFusionError, TerminalAarch64CbnzFusionFunction,
    TerminalAarch64CbnzFusionIdentity, TerminalAarch64CbnzFusionInstruction,
    TerminalAarch64CbnzFusionPlan, TerminalAarch64CbnzFusionPolicy,
    TerminalAarch64CbnzFusionWorkAxis, TerminalAarch64CbnzInstructionDisposition,
    TerminalPhysicalOperandFootprint, TerminalPostAllocationMachineInstruction,
    TerminalQualifiedPhysicalRead, ValidatedTerminalAarch64CbnzFusion,
    ValidatedTerminalPostAllocationMachinePlan, fusion_receipt,
    terminal_aarch64_cbnz_fusion_identity,
};

/// Independently replay the complete deterministic transformation and accept
/// the artifact only when every attempt, action, disposition, work counter,
/// revision, and content identity is reproduced.
pub fn validate_aarch64_cbnz_fusion<S: ValidatedTerminalSelectedAnalysis>(
    selected: &S,
    liveness: &ValidatedTerminalLiveness,
    source: &ValidatedTerminalPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    plan: TerminalAarch64CbnzFusionPlan,
) -> Result<ValidatedTerminalAarch64CbnzFusion, TerminalAarch64CbnzFusionError> {
    if plan.policy
        != TerminalAarch64CbnzFusionPolicy::Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1
    {
        return Err(TerminalAarch64CbnzFusionError::ArtifactMismatch);
    }
    let expected = replay(selected, liveness, source, physical, plan.budget)?;
    if plan != expected {
        return Err(TerminalAarch64CbnzFusionError::ArtifactMismatch);
    }
    let receipt = fusion_receipt(&plan);
    Ok(ValidatedTerminalAarch64CbnzFusion::new(plan, receipt))
}

fn replay<S: ValidatedTerminalSelectedAnalysis>(
    selected: &S,
    liveness: &ValidatedTerminalLiveness,
    source: &ValidatedTerminalPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    budget: OptimizationWorkBudget,
) -> Result<TerminalAarch64CbnzFusionPlan, TerminalAarch64CbnzFusionError> {
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
            TerminalAarch64CbnzFusionWorkAxis::Iterations,
        )?;
        let iteration = usage.iterations;
        let input = crate::aarch64_cbnz_identity::revision_identity(
            source.receipt().identity(),
            selected.selected_identity(),
            liveness.receipt().identity(),
            machine_plan.target,
            physical.identity(),
            &functions,
        );
        let mut candidate = None;
        'scan: for (function_index, selected_function) in selected_plan.functions.iter().enumerate()
        {
            let machine_function = machine_plan.functions.get(function_index).ok_or(
                TerminalAarch64CbnzFusionError::FunctionRosterMismatch(function_index),
            )?;
            let live_function = live_plan.functions.get(function_index).ok_or(
                TerminalAarch64CbnzFusionError::FunctionRosterMismatch(function_index),
            )?;
            for (block_index, block) in selected_function.blocks.iter().enumerate() {
                let Some(compare) = block.instructions.last() else {
                    continue;
                };
                if !matches!(
                    compare.kind,
                    TerminalSelectedInstructionKind::CompareI64Zero
                ) {
                    continue;
                }
                let TerminalSelectedTerminator::ConditionalBranch {
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
                    TerminalAarch64CbnzFusionWorkAxis::RuleEvaluations,
                )?;
                let machine_block = machine_function.blocks.get(block_index).ok_or(
                    TerminalAarch64CbnzFusionError::BlockRosterMismatch {
                        function: function_index,
                        block: block_index,
                    },
                )?;
                let live_block = live_function.blocks.get(block_index).ok_or(
                    TerminalAarch64CbnzFusionError::BlockRosterMismatch {
                        function: function_index,
                        block: block_index,
                    },
                )?;
                let compare_index = block.instructions.len() - 1;
                let machine_compare = machine_block.instructions.get(compare_index).ok_or(
                    TerminalAarch64CbnzFusionError::InstructionRosterMismatch(compare.id),
                )?;
                let machine_branch = machine_block
                    .instructions
                    .get(block.instructions.len())
                    .ok_or(TerminalAarch64CbnzFusionError::InstructionRosterMismatch(
                        branch.id,
                    ))?;
                let live_compare = live_block.instructions.get(compare_index).ok_or(
                    TerminalAarch64CbnzFusionError::LivenessRosterMismatch(compare.id),
                )?;
                let live_branch = live_block
                    .instructions
                    .get(block.instructions.len())
                    .ok_or(TerminalAarch64CbnzFusionError::LivenessRosterMismatch(
                        branch.id,
                    ))?;
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
                let already_fused =
                    actions
                        .iter()
                        .any(|action: &TerminalAarch64CbnzFusionAction| {
                            action.machine == selected_function.machine
                                && action.block == block.id
                                && action.compare == compare.id
                                && action.branch == branch.id
                        });
                let outcome = if already_fused {
                    TerminalAarch64CbnzFusionAttemptOutcome::AlreadyFused
                } else if !compare.provenance.fuel.is_empty() {
                    TerminalAarch64CbnzFusionAttemptOutcome::CompareCarriesFuel
                } else if flags_live_out(live_block, live_branch, &nzcv) {
                    TerminalAarch64CbnzFusionAttemptOutcome::NzcvLiveOut
                } else {
                    TerminalAarch64CbnzFusionAttemptOutcome::SelectedForFusion
                };
                attempts.push(TerminalAarch64CbnzFusionAttempt {
                    iteration,
                    input,
                    machine: selected_function.machine,
                    block: block.id,
                    compare: compare.id,
                    branch: branch.id,
                    outcome,
                });
                if outcome == TerminalAarch64CbnzFusionAttemptOutcome::SelectedForFusion {
                    charge(
                        &mut usage.candidates,
                        budget.candidates(),
                        TerminalAarch64CbnzFusionWorkAxis::Candidates,
                    )?;
                    charge(
                        &mut usage.validation_steps,
                        budget.validation_steps(),
                        TerminalAarch64CbnzFusionWorkAxis::ValidationSteps,
                    )?;
                    candidate = Some((
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
        )) = candidate
        else {
            break;
        };
        charge(
            &mut usage.commits,
            budget.commits(),
            TerminalAarch64CbnzFusionWorkAxis::Commits,
        )?;
        let source_read = qualified_read(compare, machine_compare)?;
        apply_dispositions(
            &mut functions[function_index].blocks[block_index],
            compare.id,
            branch.id,
            &source_read,
        )?;
        let output = crate::aarch64_cbnz_identity::revision_identity(
            source.receipt().identity(),
            selected.selected_identity(),
            liveness.receipt().identity(),
            machine_plan.target,
            physical.identity(),
            &functions,
        );
        actions.push(TerminalAarch64CbnzFusionAction {
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

    let output_revision = crate::aarch64_cbnz_identity::revision_identity(
        source.receipt().identity(),
        selected.selected_identity(),
        liveness.receipt().identity(),
        machine_plan.target,
        physical.identity(),
        &functions,
    );
    let mut expected = TerminalAarch64CbnzFusionPlan {
        identity: TerminalAarch64CbnzFusionIdentity::from_bytes([0; 32]),
        source: source.receipt().identity(),
        selected: selected.selected_identity(),
        liveness: liveness.receipt().identity(),
        target: machine_plan.target,
        physical_register_model: physical.identity(),
        policy: TerminalAarch64CbnzFusionPolicy::Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1,
        budget,
        usage,
        output_revision,
        attempts,
        actions,
        functions,
    };
    expected.identity = terminal_aarch64_cbnz_fusion_identity(&expected);
    Ok(expected)
}

fn validate_roots<S: ValidatedTerminalSelectedAnalysis>(
    selected: &S,
    liveness: &ValidatedTerminalLiveness,
    source: &ValidatedTerminalPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<(), TerminalAarch64CbnzFusionError> {
    let machine = source.plan();
    if selected.selected_plan().target.architecture != Architecture::Aarch64
        || machine.target.architecture != Architecture::Aarch64
        || physical.model().architecture != Architecture::Aarch64
    {
        return Err(TerminalAarch64CbnzFusionError::UnsupportedTarget(
            machine.target,
        ));
    }
    if machine.selected != selected.selected_identity()
        || selected.selected_plan().target != machine.target
        || liveness.receipt().selected() != selected.selected_identity()
        || liveness.plan().selected != selected.selected_identity()
        || liveness.plan().target != machine.target
        || machine.physical_register_model != physical.identity()
        || selected.selected_plan().functions.len() != machine.functions.len()
        || selected.selected_plan().functions.len() != liveness.plan().functions.len()
    {
        return Err(TerminalAarch64CbnzFusionError::RootMismatch);
    }
    Ok(())
}

fn baseline_roster(
    source: &ValidatedTerminalPostAllocationMachinePlan,
) -> Vec<TerminalAarch64CbnzFusionFunction> {
    source
        .plan()
        .functions
        .iter()
        .map(|function| TerminalAarch64CbnzFusionFunction {
            machine: function.machine,
            blocks: function
                .blocks
                .iter()
                .map(|block| TerminalAarch64CbnzFusionBlock {
                    block: block.block,
                    instructions: block
                        .instructions
                        .iter()
                        .map(|instruction| TerminalAarch64CbnzFusionInstruction {
                            instruction: instruction.instruction,
                            disposition: TerminalAarch64CbnzInstructionDisposition::RetainedV1,
                        })
                        .collect(),
                })
                .collect(),
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn validate_pair(
    compare: &TerminalSelectedInstruction,
    branch: &TerminalSelectedInstruction,
    machine_compare: &TerminalPostAllocationMachineInstruction,
    machine_branch: &TerminalPostAllocationMachineInstruction,
    live_compare: &TerminalInstructionLiveness,
    live_branch: &TerminalInstructionLiveness,
    physical: &ValidatedPhysicalRegisterModel,
    nzcv: &[RegisterUnitId],
    pc: &[RegisterUnitId],
) -> Result<(), TerminalAarch64CbnzFusionError> {
    if machine_compare.instruction != compare.id
        || live_compare.instruction != compare.id
        || !matches!(
            compare.kind,
            TerminalSelectedInstructionKind::CompareI64Zero
        )
        || compare.operands.len() != 1
    {
        return Err(TerminalAarch64CbnzFusionError::InstructionRosterMismatch(
            compare.id,
        ));
    }
    if machine_branch.instruction != branch.id
        || live_branch.instruction != branch.id
        || !matches!(
            branch.kind,
            TerminalSelectedInstructionKind::ConditionalBranchNonZero
        )
        || !branch.operands.is_empty()
    {
        return Err(TerminalAarch64CbnzFusionError::InstructionRosterMismatch(
            branch.id,
        ));
    }
    validate_compare(compare, machine_compare, physical, nzcv)?;
    validate_branch(branch, machine_branch, nzcv, pc)?;
    if live_compare.unit_live_out != live_branch.unit_live_in
        || !nzcv
            .iter()
            .all(|unit| live_compare.unit_live_out.contains(unit))
        || !nzcv.iter().all(|unit| live_branch.unit_uses.contains(unit))
    {
        return Err(TerminalAarch64CbnzFusionError::LivenessRosterMismatch(
            branch.id,
        ));
    }
    Ok(())
}

fn validate_compare(
    selected: &TerminalSelectedInstruction,
    machine: &TerminalPostAllocationMachineInstruction,
    physical: &ValidatedPhysicalRegisterModel,
    nzcv: &[RegisterUnitId],
) -> Result<(), TerminalAarch64CbnzFusionError> {
    let encoded = &machine.alternative.encoded;
    if machine.alternative.key.family != TerminalMachineAlternativeFamily::CompareI64Zero
        || machine.alternative.key.variant != 0
        || encoded.external_operand_reads != [0]
        || !encoded.external_operand_writes.is_empty()
        || !encoded.implicit_unit_uses.is_empty()
        || encoded.implicit_unit_defs != nzcv
        || !encoded.implicit_unit_clobbers.is_empty()
        || encoded.memory != TerminalMachineEncodedMemoryEffect::NoneV1
        || encoded.stack != TerminalMachineEncodedStackEffect::UnchangedV1
        || encoded.trap != TerminalMachineEncodedTrapBehavior::NeverV1
        || encoded.control != TerminalMachineEncodedControlEffect::FallThroughV1
        || machine.operands.len() != 1
    {
        return Err(TerminalAarch64CbnzFusionError::InvalidCompareFootprint(
            selected.id,
        ));
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
        return Err(TerminalAarch64CbnzFusionError::InvalidCompareFootprint(
            selected.id,
        ));
    }
    validate_x_view(operand, physical, selected.id)
}

fn validate_branch(
    selected: &TerminalSelectedInstruction,
    machine: &TerminalPostAllocationMachineInstruction,
    nzcv: &[RegisterUnitId],
    pc: &[RegisterUnitId],
) -> Result<(), TerminalAarch64CbnzFusionError> {
    let encoded = &machine.alternative.encoded;
    let expected_uses = union(nzcv, pc);
    if machine.alternative.key.family != TerminalMachineAlternativeFamily::ConditionalBranchNonZero
        || machine.alternative.key.variant != 0
        || !machine.operands.is_empty()
        || !encoded.external_operand_reads.is_empty()
        || !encoded.external_operand_writes.is_empty()
        || encoded.implicit_unit_uses != expected_uses
        || encoded.implicit_unit_defs != pc
        || !encoded.implicit_unit_clobbers.is_empty()
        || encoded.memory != TerminalMachineEncodedMemoryEffect::NoneV1
        || encoded.stack != TerminalMachineEncodedStackEffect::UnchangedV1
        || encoded.trap != TerminalMachineEncodedTrapBehavior::MayArchitecturalFaultV1
        || encoded.control != TerminalMachineEncodedControlEffect::ConditionalRelativeBranchV1
        || machine.implicit_unit_uses != expected_uses
        || machine.implicit_unit_defs != pc
    {
        return Err(TerminalAarch64CbnzFusionError::InvalidBranchFootprint(
            selected.id,
        ));
    }
    Ok(())
}

fn validate_x_view(
    operand: &TerminalPhysicalOperandFootprint,
    physical: &ValidatedPhysicalRegisterModel,
    instruction: omega_terminal_selected_instructions::TerminalSelectedInstructionId,
) -> Result<(), TerminalAarch64CbnzFusionError> {
    let view = physical
        .model()
        .views
        .iter()
        .find(|view| view.id == operand.view)
        .ok_or(TerminalAarch64CbnzFusionError::InvalidPhysicalSource(
            instruction,
        ))?;
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
        return Err(TerminalAarch64CbnzFusionError::InvalidPhysicalSource(
            instruction,
        ));
    }
    Ok(())
}

fn flags_live_out(
    block: &TerminalBlockLiveness,
    branch: &TerminalInstructionLiveness,
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
    selected: &TerminalSelectedInstruction,
    machine: &TerminalPostAllocationMachineInstruction,
) -> Result<TerminalQualifiedPhysicalRead, TerminalAarch64CbnzFusionError> {
    let operand =
        machine
            .operands
            .first()
            .ok_or(TerminalAarch64CbnzFusionError::InvalidCompareFootprint(
                selected.id,
            ))?;
    Ok(TerminalQualifiedPhysicalRead {
        source_instruction: selected.id,
        operand: operand.operand,
        virtual_register: operand.virtual_register,
        class: operand.class,
        view: operand.view,
        units: operand.read_units.clone(),
    })
}

fn apply_dispositions(
    block: &mut TerminalAarch64CbnzFusionBlock,
    compare: omega_terminal_selected_instructions::TerminalSelectedInstructionId,
    branch: omega_terminal_selected_instructions::TerminalSelectedInstructionId,
    read: &TerminalQualifiedPhysicalRead,
) -> Result<(), TerminalAarch64CbnzFusionError> {
    let compare_row = block
        .instructions
        .iter_mut()
        .find(|row| row.instruction == compare)
        .ok_or(TerminalAarch64CbnzFusionError::InstructionRosterMismatch(
            compare,
        ))?;
    compare_row.disposition =
        TerminalAarch64CbnzInstructionDisposition::ElidedCompareI64ZeroV1 { consumer: branch };
    let branch_row = block
        .instructions
        .iter_mut()
        .find(|row| row.instruction == branch)
        .ok_or(TerminalAarch64CbnzFusionError::InstructionRosterMismatch(
            branch,
        ))?;
    branch_row.disposition =
        TerminalAarch64CbnzInstructionDisposition::FusedBranchNonZeroToCbnzV1 {
            compare,
            source_read: read.clone(),
        };
    Ok(())
}

fn named_units(
    physical: &ValidatedPhysicalRegisterModel,
    name: &'static str,
) -> Result<Vec<RegisterUnitId>, TerminalAarch64CbnzFusionError> {
    physical
        .model()
        .view_named(name)
        .map(|view| view.units.clone())
        .ok_or(TerminalAarch64CbnzFusionError::MissingArchitecturalView(
            name,
        ))
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
    axis: TerminalAarch64CbnzFusionWorkAxis,
) -> Result<(), TerminalAarch64CbnzFusionError> {
    *usage = usage
        .checked_add(1)
        .ok_or(TerminalAarch64CbnzFusionError::BudgetExceeded(axis))?;
    if *usage > budget {
        return Err(TerminalAarch64CbnzFusionError::BudgetExceeded(axis));
    }
    Ok(())
}
