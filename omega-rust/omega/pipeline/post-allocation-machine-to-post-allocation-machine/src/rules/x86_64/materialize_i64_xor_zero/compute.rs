use optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};
use register_model::{RegisterOperandAccess, RegisterUnitId, ValidatedPhysicalRegisterModel};
use selected_instructions::{
    MachineAlternativeFamily, MachineEncodedControlEffect, MachineEncodedMemoryEffect,
    MachineEncodedStackEffect, MachineEncodedTrapBehavior, MachineSizeKnowledge,
    SelectedInstruction, SelectedInstructionKind, SelectedInstructionPlan,
};
use selected_instructions_to_register_homes::{
    LivenessPlan, ValidatedLiveness, ValidatedSelectedAnalysis,
};
use semantic_vocabulary::IntegerValue;
use target::Architecture;

use crate::{
    X86_MOVABS_I64_BYTE_COUNT, X86_XOR_R64_SELF_BYTE_COUNT, X86XorZeroInstructionDisposition,
    X86XorZeroMaterializationAction, X86XorZeroMaterializationAttempt,
    X86XorZeroMaterializationAttemptOutcome, X86XorZeroMaterializationBlock,
    X86XorZeroMaterializationError, X86XorZeroMaterializationFunction,
    X86XorZeroMaterializationIdentity, X86XorZeroMaterializationInstruction,
    X86XorZeroMaterializationPlan, X86XorZeroMaterializationPolicy,
    X86XorZeroMaterializationWorkAxis, X86XorZeroPhysicalWrite,
    x86_xor_zero_materialization_identity,
};
use physical_instructions::{
    PhysicalOperandFootprint, PostAllocationMachineInstruction, PostAllocationMachinePlan,
};
use register_homes_to_post_allocation_machine::ValidatedPostAllocationMachinePlan;

pub(crate) fn compute<S: ValidatedSelectedAnalysis>(
    selected: &S,
    liveness: &ValidatedLiveness,
    source: &ValidatedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    budget: OptimizationWorkBudget,
) -> Result<X86XorZeroMaterializationPlan, X86XorZeroMaterializationError> {
    compute_from_parts(
        selected.selected_plan(),
        selected.selected_identity(),
        liveness.plan(),
        liveness.receipt().identity(),
        source.plan(),
        source.receipt().identity(),
        physical,
        budget,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn compute_from_parts(
    selected: &SelectedInstructionPlan,
    selected_identity: selected_instructions::SelectedInstructionPlanIdentity,
    liveness: &LivenessPlan,
    liveness_identity: selected_instructions_to_register_homes::LivenessIdentity,
    source: &PostAllocationMachinePlan,
    source_identity: physical_instructions::PostAllocationMachineIdentity,
    physical: &ValidatedPhysicalRegisterModel,
    budget: OptimizationWorkBudget,
) -> Result<X86XorZeroMaterializationPlan, X86XorZeroMaterializationError> {
    validate_roots(
        selected,
        selected_identity,
        liveness,
        liveness_identity,
        source,
        source_identity,
        physical,
    )?;
    let rflags = named_units(physical, "rflags")?;
    let mut functions = baseline_roster(source);
    let mut attempts = Vec::new();
    let mut actions = Vec::new();
    let mut usage = OptimizationWorkUsage::default();

    loop {
        charge(
            &mut usage.iterations,
            budget.iterations(),
            X86XorZeroMaterializationWorkAxis::Iterations,
        )?;
        let iteration = usage.iterations;
        let input = super::identity::revision_identity(
            source_identity,
            selected_identity,
            liveness_identity,
            source.target,
            physical.identity(),
            &functions,
        );
        let mut candidate = None;
        'scan: for (function_index, selected_function) in selected.functions.iter().enumerate() {
            let machine_function = source.functions.get(function_index).ok_or(
                X86XorZeroMaterializationError::FunctionRosterMismatch(function_index),
            )?;
            let live_function = liveness.functions.get(function_index).ok_or(
                X86XorZeroMaterializationError::FunctionRosterMismatch(function_index),
            )?;
            if machine_function.machine != selected_function.machine
                || live_function.machine != selected_function.machine
            {
                return Err(X86XorZeroMaterializationError::FunctionRosterMismatch(
                    function_index,
                ));
            }
            for (block_index, block) in selected_function.blocks.iter().enumerate() {
                let machine_block = machine_function.blocks.get(block_index).ok_or(
                    X86XorZeroMaterializationError::BlockRosterMismatch {
                        function: function_index,
                        block: block_index,
                    },
                )?;
                let live_block = live_function.blocks.get(block_index).ok_or(
                    X86XorZeroMaterializationError::BlockRosterMismatch {
                        function: function_index,
                        block: block_index,
                    },
                )?;
                if machine_block.block != block.id
                    || live_block.block != block.id
                    || machine_block.instructions.len() != block.instructions.len() + 1
                    || live_block.instructions.len() != block.instructions.len() + 1
                {
                    return Err(X86XorZeroMaterializationError::BlockRosterMismatch {
                        function: function_index,
                        block: block_index,
                    });
                }
                for (instruction_index, instruction) in block.instructions.iter().enumerate() {
                    let SelectedInstructionKind::MaterializeI64 { value } = instruction.kind else {
                        continue;
                    };
                    charge(
                        &mut usage.rule_evaluations,
                        budget.rule_evaluations(),
                        X86XorZeroMaterializationWorkAxis::RuleEvaluations,
                    )?;
                    let machine = machine_block.instructions.get(instruction_index).ok_or(
                        X86XorZeroMaterializationError::InstructionRosterMismatch(instruction.id),
                    )?;
                    let live = live_block.instructions.get(instruction_index).ok_or(
                        X86XorZeroMaterializationError::LivenessRosterMismatch(instruction.id),
                    )?;
                    let literal_bits = integer_bits(value, instruction.id)?;
                    validate_materialization(instruction, machine, physical)?;
                    if live.instruction != instruction.id {
                        return Err(X86XorZeroMaterializationError::LivenessRosterMismatch(
                            instruction.id,
                        ));
                    }
                    let destination = qualified_write(instruction, machine)?;
                    let already_selected =
                        actions
                            .iter()
                            .any(|action: &X86XorZeroMaterializationAction| {
                                action.machine == selected_function.machine
                                    && action.block == block.id
                                    && action.instruction == instruction.id
                            });
                    let outcome = if already_selected {
                        X86XorZeroMaterializationAttemptOutcome::AlreadySelected
                    } else if literal_bits != 0 {
                        X86XorZeroMaterializationAttemptOutcome::NonZeroLiteral
                    } else if flags_live_out(live, &rflags) {
                        X86XorZeroMaterializationAttemptOutcome::RflagsLiveOut
                    } else {
                        X86XorZeroMaterializationAttemptOutcome::SelectedForRewrite
                    };
                    attempts.push(X86XorZeroMaterializationAttempt {
                        iteration,
                        input,
                        machine: selected_function.machine,
                        block: block.id,
                        instruction: instruction.id,
                        literal_bits,
                        destination: destination.clone(),
                        rflags_units: rflags.clone(),
                        baseline_byte_count: X86_MOVABS_I64_BYTE_COUNT,
                        selected_byte_count: X86_XOR_R64_SELF_BYTE_COUNT,
                        outcome,
                    });
                    if outcome == X86XorZeroMaterializationAttemptOutcome::SelectedForRewrite {
                        charge(
                            &mut usage.candidates,
                            budget.candidates(),
                            X86XorZeroMaterializationWorkAxis::Candidates,
                        )?;
                        charge(
                            &mut usage.validation_steps,
                            budget.validation_steps(),
                            X86XorZeroMaterializationWorkAxis::ValidationSteps,
                        )?;
                        candidate = Some((
                            function_index,
                            block_index,
                            selected_function.machine,
                            block.id,
                            instruction.id,
                            destination,
                        ));
                        break 'scan;
                    }
                }
            }
        }

        let Some((function_index, block_index, machine, block, instruction, destination)) =
            candidate
        else {
            break;
        };
        charge(
            &mut usage.commits,
            budget.commits(),
            X86XorZeroMaterializationWorkAxis::Commits,
        )?;
        let row = functions[function_index].blocks[block_index]
            .instructions
            .iter_mut()
            .find(|row| row.instruction == instruction)
            .ok_or(X86XorZeroMaterializationError::InstructionRosterMismatch(
                instruction,
            ))?;
        row.disposition = X86XorZeroInstructionDisposition::XorZeroMaterializationV1 {
            destination: destination.clone(),
            rflags_units: rflags.clone(),
            baseline_byte_count: X86_MOVABS_I64_BYTE_COUNT,
            selected_byte_count: X86_XOR_R64_SELF_BYTE_COUNT,
        };
        let output = super::identity::revision_identity(
            source_identity,
            selected_identity,
            liveness_identity,
            source.target,
            physical.identity(),
            &functions,
        );
        actions.push(X86XorZeroMaterializationAction {
            iteration,
            input,
            output,
            machine,
            block,
            instruction,
            destination,
            rflags_units: rflags.clone(),
            baseline_byte_count: X86_MOVABS_I64_BYTE_COUNT,
            selected_byte_count: X86_XOR_R64_SELF_BYTE_COUNT,
        });
    }

    let output_revision = super::identity::revision_identity(
        source_identity,
        selected_identity,
        liveness_identity,
        source.target,
        physical.identity(),
        &functions,
    );
    let mut plan = X86XorZeroMaterializationPlan {
        identity: X86XorZeroMaterializationIdentity::from_bytes([0; 32]),
        source: source_identity,
        selected: selected_identity,
        liveness: liveness_identity,
        target: source.target,
        physical_register_model: physical.identity(),
        policy: X86XorZeroMaterializationPolicy::X86SelectXorZeroI64MaterializationV1,
        budget,
        usage,
        output_revision,
        attempts,
        actions,
        functions,
    };
    plan.identity = x86_xor_zero_materialization_identity(&plan);
    Ok(plan)
}

#[allow(clippy::too_many_arguments)]
fn validate_roots(
    selected: &SelectedInstructionPlan,
    selected_identity: selected_instructions::SelectedInstructionPlanIdentity,
    liveness: &LivenessPlan,
    _liveness_identity: selected_instructions_to_register_homes::LivenessIdentity,
    source: &PostAllocationMachinePlan,
    source_identity: physical_instructions::PostAllocationMachineIdentity,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<(), X86XorZeroMaterializationError> {
    if selected.target.architecture != Architecture::X86_64
        || liveness.target.architecture != Architecture::X86_64
        || source.target.architecture != Architecture::X86_64
        || physical.model().architecture != Architecture::X86_64
    {
        return Err(X86XorZeroMaterializationError::UnsupportedTarget(
            source.target,
        ));
    }
    if source.identity != source_identity
        || source.selected != selected_identity
        || liveness.selected != selected_identity
        || selected.target != source.target
        || liveness.target != source.target
        || source.physical_register_model != physical.identity()
        || selected.functions.len() != source.functions.len()
        || selected.functions.len() != liveness.functions.len()
    {
        return Err(X86XorZeroMaterializationError::RootMismatch);
    }
    Ok(())
}

fn baseline_roster(source: &PostAllocationMachinePlan) -> Vec<X86XorZeroMaterializationFunction> {
    source
        .functions
        .iter()
        .map(|function| X86XorZeroMaterializationFunction {
            machine: function.machine,
            blocks: function
                .blocks
                .iter()
                .map(|block| X86XorZeroMaterializationBlock {
                    block: block.block,
                    instructions: block
                        .instructions
                        .iter()
                        .map(|instruction| X86XorZeroMaterializationInstruction {
                            instruction: instruction.instruction,
                            disposition: X86XorZeroInstructionDisposition::RetainedV1,
                        })
                        .collect(),
                })
                .collect(),
        })
        .collect()
}

fn integer_bits(
    value: IntegerValue,
    instruction: selected_instructions::SelectedInstructionId,
) -> Result<u64, X86XorZeroMaterializationError> {
    match value {
        IntegerValue::Signed(value) => i64::try_from(value)
            .map(|value| value as u64)
            .map_err(|_| X86XorZeroMaterializationError::IntegerOutsideI64Bits(instruction)),
        IntegerValue::Unsigned(value) => u64::try_from(value)
            .map_err(|_| X86XorZeroMaterializationError::IntegerOutsideI64Bits(instruction)),
    }
}

fn validate_materialization(
    selected: &SelectedInstruction,
    machine: &PostAllocationMachineInstruction,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<(), X86XorZeroMaterializationError> {
    let encoded = &machine.alternative.encoded;
    if machine.instruction != selected.id
        || selected.operands.len() != 1
        || !selected.implicit_uses.is_empty()
        || !selected.implicit_defs.is_empty()
        || !selected.clobbers.is_empty()
        || machine.alternative.key.family != MachineAlternativeFamily::MaterializeI64
        || machine.alternative.key.variant != 0
        || machine.alternative.size != MachineSizeKnowledge::ExactBytes(10)
        || !encoded.external_operand_reads.is_empty()
        || encoded.external_operand_writes != [0]
        || !encoded.implicit_unit_uses.is_empty()
        || !encoded.implicit_unit_defs.is_empty()
        || !encoded.implicit_unit_clobbers.is_empty()
        || encoded.memory != MachineEncodedMemoryEffect::NoneV1
        || encoded.stack != MachineEncodedStackEffect::UnchangedV1
        || encoded.trap != MachineEncodedTrapBehavior::NeverV1
        || encoded.control != MachineEncodedControlEffect::FallThroughV1
        || machine.operands.len() != 1
        || !machine.implicit_unit_uses.is_empty()
        || !machine.implicit_unit_defs.is_empty()
        || !machine.implicit_unit_clobbers.is_empty()
        || !machine.unit_uses.is_empty()
        || !machine.unit_clobbers.is_empty()
    {
        return Err(X86XorZeroMaterializationError::InvalidMaterializationFootprint(selected.id));
    }
    let selected_operand = &selected.operands[0];
    let operand = &machine.operands[0];
    if selected_operand.operand != 0
        || selected_operand.access != RegisterOperandAccess::Def
        || operand.operand != 0
        || operand.virtual_register != selected_operand.virtual_register
        || operand.class != selected_operand.class
        || operand.access != RegisterOperandAccess::Def
        || !operand.read_units.is_empty()
        || machine.unit_defs != operand.write_units
        || operand.write_semantics.is_none()
    {
        return Err(X86XorZeroMaterializationError::InvalidMaterializationFootprint(selected.id));
    }
    validate_r64_view(operand, physical, selected.id)
}

fn validate_r64_view(
    operand: &PhysicalOperandFootprint,
    physical: &ValidatedPhysicalRegisterModel,
    instruction: selected_instructions::SelectedInstructionId,
) -> Result<(), X86XorZeroMaterializationError> {
    let view = physical
        .model()
        .views
        .iter()
        .find(|view| view.id == operand.view)
        .ok_or(X86XorZeroMaterializationError::InvalidPhysicalDestination(
            instruction,
        ))?;
    if !is_canonical_allocatable_r64(&view.name)
        || view.bits != 64
        || !view.allocatable
        || view.class != operand.class
        || view.units != operand.storage_units
        || view.write_units != operand.write_units
        || Some(view.write_semantics) != operand.write_semantics
    {
        return Err(X86XorZeroMaterializationError::InvalidPhysicalDestination(
            instruction,
        ));
    }
    Ok(())
}

fn is_canonical_allocatable_r64(name: &str) -> bool {
    matches!(
        name,
        "rax"
            | "rbx"
            | "rcx"
            | "rdx"
            | "rsi"
            | "rdi"
            | "rbp"
            | "r8"
            | "r9"
            | "r10"
            | "r11"
            | "r12"
            | "r13"
            | "r14"
            | "r15"
    )
}

fn qualified_write(
    selected: &SelectedInstruction,
    machine: &PostAllocationMachineInstruction,
) -> Result<X86XorZeroPhysicalWrite, X86XorZeroMaterializationError> {
    let operand = machine
        .operands
        .first()
        .ok_or(X86XorZeroMaterializationError::InvalidMaterializationFootprint(selected.id))?;
    Ok(X86XorZeroPhysicalWrite {
        instruction: selected.id,
        operand: operand.operand,
        virtual_register: operand.virtual_register,
        class: operand.class,
        view: operand.view,
        storage_units: operand.storage_units.clone(),
        write_units: operand.write_units.clone(),
        write_semantics: operand
            .write_semantics
            .ok_or(X86XorZeroMaterializationError::InvalidMaterializationFootprint(selected.id))?,
    })
}

fn flags_live_out(
    live: &selected_instructions_to_register_homes::InstructionLiveness,
    rflags: &[RegisterUnitId],
) -> bool {
    live.unit_live_out.iter().any(|unit| rflags.contains(unit))
}

fn named_units(
    physical: &ValidatedPhysicalRegisterModel,
    name: &'static str,
) -> Result<Vec<RegisterUnitId>, X86XorZeroMaterializationError> {
    physical
        .model()
        .view_named(name)
        .filter(|view| !view.units.is_empty())
        .map(|view| view.units.clone())
        .ok_or(X86XorZeroMaterializationError::MissingArchitecturalView(
            name,
        ))
}

fn charge(
    usage: &mut u64,
    budget: u64,
    axis: X86XorZeroMaterializationWorkAxis,
) -> Result<(), X86XorZeroMaterializationError> {
    *usage = usage
        .checked_add(1)
        .ok_or(X86XorZeroMaterializationError::BudgetExceeded(axis))?;
    if *usage > budget {
        return Err(X86XorZeroMaterializationError::BudgetExceeded(axis));
    }
    Ok(())
}
