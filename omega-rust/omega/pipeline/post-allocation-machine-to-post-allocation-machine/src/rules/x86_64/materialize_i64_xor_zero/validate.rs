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
    ValidatedX86XorZeroMaterialization, X86_MOVABS_I64_BYTE_COUNT, X86_XOR_R64_SELF_BYTE_COUNT,
    X86XorZeroInstructionDisposition, X86XorZeroMaterializationAction,
    X86XorZeroMaterializationAttempt, X86XorZeroMaterializationAttemptOutcome,
    X86XorZeroMaterializationBlock, X86XorZeroMaterializationError,
    X86XorZeroMaterializationFunction, X86XorZeroMaterializationIdentity,
    X86XorZeroMaterializationInstruction, X86XorZeroMaterializationPlan,
    X86XorZeroMaterializationPolicy, X86XorZeroMaterializationWorkAxis, X86XorZeroPhysicalWrite,
    x86_xor_zero_materialization_identity, x86_xor_zero_materialization_receipt,
};
use physical_instructions::{
    PhysicalOperandFootprint, PostAllocationMachineInstruction, PostAllocationMachinePlan,
};
use register_homes_to_post_allocation_machine::ValidatedPostAllocationMachinePlan;

/// Independently reconstruct every attempt, liveness verdict, action,
/// disposition, revision, work counter, and content identity.
pub fn validate_x86_xor_zero_materialization<S: ValidatedSelectedAnalysis>(
    selected: &S,
    liveness: &ValidatedLiveness,
    source: &ValidatedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    plan: X86XorZeroMaterializationPlan,
) -> Result<ValidatedX86XorZeroMaterialization, X86XorZeroMaterializationError> {
    validate_from_parts(
        selected.selected_plan(),
        selected.selected_identity(),
        liveness.plan(),
        liveness.receipt().identity(),
        source.plan(),
        source.receipt().identity(),
        physical,
        &plan,
    )?;
    let receipt = x86_xor_zero_materialization_receipt(&plan)?;
    Ok(ValidatedX86XorZeroMaterialization::new(plan, receipt))
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn validate_from_parts(
    selected: &SelectedInstructionPlan,
    selected_identity: selected_instructions::SelectedInstructionPlanIdentity,
    liveness: &LivenessPlan,
    liveness_identity: selected_instructions_to_register_homes::LivenessIdentity,
    source: &PostAllocationMachinePlan,
    source_identity: physical_instructions::PostAllocationMachineIdentity,
    physical: &ValidatedPhysicalRegisterModel,
    plan: &X86XorZeroMaterializationPlan,
) -> Result<(), X86XorZeroMaterializationError> {
    if plan.policy != X86XorZeroMaterializationPolicy::X86SelectXorZeroI64MaterializationV1 {
        return Err(X86XorZeroMaterializationError::ArtifactMismatch);
    }
    let expected = replay_from_parts(
        selected,
        selected_identity,
        liveness,
        liveness_identity,
        source,
        source_identity,
        physical,
        plan.budget,
    )?;
    if *plan != expected {
        return Err(X86XorZeroMaterializationError::ArtifactMismatch);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn replay_from_parts(
    selected: &SelectedInstructionPlan,
    selected_identity: selected_instructions::SelectedInstructionPlanIdentity,
    liveness: &LivenessPlan,
    liveness_identity: selected_instructions_to_register_homes::LivenessIdentity,
    source: &PostAllocationMachinePlan,
    source_identity: physical_instructions::PostAllocationMachineIdentity,
    physical: &ValidatedPhysicalRegisterModel,
    budget: OptimizationWorkBudget,
) -> Result<X86XorZeroMaterializationPlan, X86XorZeroMaterializationError> {
    independently_validate_roots(
        selected,
        selected_identity,
        liveness,
        liveness_identity,
        source,
        source_identity,
        physical,
    )?;
    let rflags = independently_named_rflags_units(physical)?;
    let mut functions = independently_reconstruct_roster(source);
    let mut attempts = Vec::new();
    let mut actions = Vec::new();
    let mut usage = OptimizationWorkUsage::default();

    loop {
        independently_charge(
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
        'candidate_scan: for (function_index, selected_function) in
            selected.functions.iter().enumerate()
        {
            let machine_function = source.functions.get(function_index).ok_or(
                X86XorZeroMaterializationError::FunctionRosterMismatch(function_index),
            )?;
            let live_function = liveness.functions.get(function_index).ok_or(
                X86XorZeroMaterializationError::FunctionRosterMismatch(function_index),
            )?;
            if selected_function.machine != machine_function.machine
                || selected_function.machine != live_function.machine
            {
                return Err(X86XorZeroMaterializationError::FunctionRosterMismatch(
                    function_index,
                ));
            }
            for block_index in 0..selected_function.blocks.len() {
                let block = &selected_function.blocks[block_index];
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
                let expected_instruction_count = block
                    .instructions
                    .len()
                    .checked_add(1)
                    .ok_or(X86XorZeroMaterializationError::CountOverflow)?;
                if machine_block.block != block.id
                    || live_block.block != block.id
                    || machine_block.instructions.len() != expected_instruction_count
                    || live_block.instructions.len() != expected_instruction_count
                {
                    return Err(X86XorZeroMaterializationError::BlockRosterMismatch {
                        function: function_index,
                        block: block_index,
                    });
                }
                for instruction_index in 0..block.instructions.len() {
                    let instruction = &block.instructions[instruction_index];
                    let SelectedInstructionKind::MaterializeI64 { value } = instruction.kind else {
                        continue;
                    };
                    independently_charge(
                        &mut usage.rule_evaluations,
                        budget.rule_evaluations(),
                        X86XorZeroMaterializationWorkAxis::RuleEvaluations,
                    )?;
                    let machine = &machine_block.instructions[instruction_index];
                    let live = &live_block.instructions[instruction_index];
                    let literal_bits = independently_reconstruct_i64_bits(value, instruction.id)?;
                    independently_validate_materialization(instruction, machine, physical)?;
                    if live.instruction != instruction.id {
                        return Err(X86XorZeroMaterializationError::LivenessRosterMismatch(
                            instruction.id,
                        ));
                    }
                    let destination = independently_reconstruct_write(instruction, machine)?;
                    let was_selected =
                        actions
                            .iter()
                            .any(|prior: &X86XorZeroMaterializationAction| {
                                prior.machine == selected_function.machine
                                    && prior.block == block.id
                                    && prior.instruction == instruction.id
                            });
                    let any_flag_live = rflags
                        .iter()
                        .any(|unit| live.unit_live_out.binary_search(unit).is_ok());
                    let outcome = match (was_selected, literal_bits == 0, any_flag_live) {
                        (true, _, _) => X86XorZeroMaterializationAttemptOutcome::AlreadySelected,
                        (false, false, _) => {
                            X86XorZeroMaterializationAttemptOutcome::NonZeroLiteral
                        }
                        (false, true, true) => {
                            X86XorZeroMaterializationAttemptOutcome::RflagsLiveOut
                        }
                        (false, true, false) => {
                            X86XorZeroMaterializationAttemptOutcome::SelectedForRewrite
                        }
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
                        baseline_byte_count: 10,
                        selected_byte_count: 3,
                        outcome,
                    });
                    if outcome == X86XorZeroMaterializationAttemptOutcome::SelectedForRewrite {
                        independently_charge(
                            &mut usage.candidates,
                            budget.candidates(),
                            X86XorZeroMaterializationWorkAxis::Candidates,
                        )?;
                        independently_charge(
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
                        break 'candidate_scan;
                    }
                }
            }
        }

        let Some((function_index, block_index, machine, block, instruction, destination)) =
            candidate
        else {
            break;
        };
        independently_charge(
            &mut usage.commits,
            budget.commits(),
            X86XorZeroMaterializationWorkAxis::Commits,
        )?;
        let disposition = X86XorZeroInstructionDisposition::XorZeroMaterializationV1 {
            destination: destination.clone(),
            rflags_units: rflags.clone(),
            baseline_byte_count: 10,
            selected_byte_count: 3,
        };
        let row = functions[function_index].blocks[block_index]
            .instructions
            .iter_mut()
            .find(|row| row.instruction == instruction)
            .ok_or(X86XorZeroMaterializationError::InstructionRosterMismatch(
                instruction,
            ))?;
        row.disposition = disposition;
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
            baseline_byte_count: 10,
            selected_byte_count: 3,
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
    let mut expected = X86XorZeroMaterializationPlan {
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
    expected.identity = x86_xor_zero_materialization_identity(&expected);
    Ok(expected)
}

#[allow(clippy::too_many_arguments)]
fn independently_validate_roots(
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
    let roots_match = source.identity == source_identity
        && source.selected == selected_identity
        && liveness.selected == selected_identity
        && selected.target == source.target
        && liveness.target == source.target
        && source.physical_register_model == physical.identity()
        && selected.functions.len() == source.functions.len()
        && selected.functions.len() == liveness.functions.len();
    if !roots_match {
        return Err(X86XorZeroMaterializationError::RootMismatch);
    }
    Ok(())
}

fn independently_named_rflags_units(
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<Vec<RegisterUnitId>, X86XorZeroMaterializationError> {
    let view = physical
        .model()
        .views
        .iter()
        .find(|view| view.name == "rflags")
        .ok_or(X86XorZeroMaterializationError::MissingArchitecturalView(
            "rflags",
        ))?;
    if view.units.is_empty() {
        return Err(X86XorZeroMaterializationError::MissingArchitecturalView(
            "rflags",
        ));
    }
    Ok(view.units.clone())
}

fn independently_reconstruct_roster(
    source: &PostAllocationMachinePlan,
) -> Vec<X86XorZeroMaterializationFunction> {
    let mut output = Vec::with_capacity(source.functions.len());
    for function in &source.functions {
        let mut blocks = Vec::with_capacity(function.blocks.len());
        for block in &function.blocks {
            let instructions = block
                .instructions
                .iter()
                .map(|instruction| X86XorZeroMaterializationInstruction {
                    instruction: instruction.instruction,
                    disposition: X86XorZeroInstructionDisposition::RetainedV1,
                })
                .collect();
            blocks.push(X86XorZeroMaterializationBlock {
                block: block.block,
                instructions,
            });
        }
        output.push(X86XorZeroMaterializationFunction {
            machine: function.machine,
            blocks,
        });
    }
    output
}

fn independently_reconstruct_i64_bits(
    value: IntegerValue,
    instruction: selected_instructions::SelectedInstructionId,
) -> Result<u64, X86XorZeroMaterializationError> {
    match value {
        IntegerValue::Signed(raw) => {
            let narrowed = i64::try_from(raw)
                .map_err(|_| X86XorZeroMaterializationError::IntegerOutsideI64Bits(instruction))?;
            Ok(u64::from_ne_bytes(narrowed.to_ne_bytes()))
        }
        IntegerValue::Unsigned(raw) => u64::try_from(raw)
            .map_err(|_| X86XorZeroMaterializationError::IntegerOutsideI64Bits(instruction)),
    }
}

fn independently_validate_materialization(
    selected: &SelectedInstruction,
    machine: &PostAllocationMachineInstruction,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<(), X86XorZeroMaterializationError> {
    let encoded = &machine.alternative.encoded;
    let selected_shape = selected.operands.len() == 1
        && selected.implicit_uses.is_empty()
        && selected.implicit_defs.is_empty()
        && selected.clobbers.is_empty();
    let alternative_shape = machine.instruction == selected.id
        && machine.alternative.key.family == MachineAlternativeFamily::MaterializeI64
        && machine.alternative.key.variant == 0
        && machine.alternative.size == MachineSizeKnowledge::ExactBytes(10)
        && encoded.external_operand_reads.is_empty()
        && encoded.external_operand_writes == [0]
        && encoded.implicit_unit_uses.is_empty()
        && encoded.implicit_unit_defs.is_empty()
        && encoded.implicit_unit_clobbers.is_empty()
        && encoded.memory == MachineEncodedMemoryEffect::NoneV1
        && encoded.stack == MachineEncodedStackEffect::UnchangedV1
        && encoded.trap == MachineEncodedTrapBehavior::NeverV1
        && encoded.control == MachineEncodedControlEffect::FallThroughV1;
    let machine_shape = machine.operands.len() == 1
        && machine.implicit_unit_uses.is_empty()
        && machine.implicit_unit_defs.is_empty()
        && machine.implicit_unit_clobbers.is_empty()
        && machine.unit_uses.is_empty()
        && machine.unit_clobbers.is_empty();
    if !selected_shape || !alternative_shape || !machine_shape {
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
    independently_validate_r64(operand, physical, selected.id)
}

fn independently_validate_r64(
    operand: &PhysicalOperandFootprint,
    physical: &ValidatedPhysicalRegisterModel,
    instruction: selected_instructions::SelectedInstructionId,
) -> Result<(), X86XorZeroMaterializationError> {
    let view = physical
        .model()
        .views
        .iter()
        .find(|candidate| candidate.id == operand.view)
        .ok_or(X86XorZeroMaterializationError::InvalidPhysicalDestination(
            instruction,
        ))?;
    const NAMES: [&str; 15] = [
        "rax", "rbx", "rcx", "rdx", "rsi", "rdi", "rbp", "r8", "r9", "r10", "r11", "r12", "r13",
        "r14", "r15",
    ];
    if !NAMES.contains(&view.name.as_str())
        || view.bits != 64
        || !view.allocatable
        || view.class != operand.class
        || view.units != operand.storage_units
        || view.write_units != operand.write_units
        || operand.write_semantics != Some(view.write_semantics)
    {
        return Err(X86XorZeroMaterializationError::InvalidPhysicalDestination(
            instruction,
        ));
    }
    Ok(())
}

fn independently_reconstruct_write(
    selected: &SelectedInstruction,
    machine: &PostAllocationMachineInstruction,
) -> Result<X86XorZeroPhysicalWrite, X86XorZeroMaterializationError> {
    let operand = machine
        .operands
        .first()
        .ok_or(X86XorZeroMaterializationError::InvalidMaterializationFootprint(selected.id))?;
    let write_semantics = operand
        .write_semantics
        .ok_or(X86XorZeroMaterializationError::InvalidMaterializationFootprint(selected.id))?;
    Ok(X86XorZeroPhysicalWrite {
        instruction: selected.id,
        operand: operand.operand,
        virtual_register: operand.virtual_register,
        class: operand.class,
        view: operand.view,
        storage_units: operand.storage_units.clone(),
        write_units: operand.write_units.clone(),
        write_semantics,
    })
}

fn independently_charge(
    usage: &mut u64,
    budget: u64,
    axis: X86XorZeroMaterializationWorkAxis,
) -> Result<(), X86XorZeroMaterializationError> {
    let next = usage
        .checked_add(1)
        .ok_or(X86XorZeroMaterializationError::BudgetExceeded(axis))?;
    if next > budget {
        return Err(X86XorZeroMaterializationError::BudgetExceeded(axis));
    }
    *usage = next;
    Ok(())
}

const _: [(); X86_MOVABS_I64_BYTE_COUNT as usize] = [(); 10];
const _: [(); X86_XOR_R64_SELF_BYTE_COUNT as usize] = [(); 3];
