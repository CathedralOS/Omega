use omega_optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};
use omega_register_model::{
    RegisterOperandAccess, RegisterWriteSemantics, ValidatedPhysicalRegisterModel,
};
use omega_selected_instructions::{
    MachineAlternativeFamily, MachineEncodedControlEffect, MachineEncodedMemoryEffect,
    MachineEncodedStackEffect, MachineEncodedTrapBehavior, MachineSizeKnowledge,
    SelectedInstruction, SelectedInstructionKind, SelectedInstructionPlan,
};
use omega_selected_instructions_to_register_homes::ValidatedSelectedAnalysis;
use omega_target::Architecture;
use psi_core::IntegerValue;

use crate::{
    PhysicalOperandFootprint, PostAllocationMachineInstruction, PostAllocationMachinePlan,
    ValidatedPostAllocationMachinePlan, X86_MOV_R32_IMM32_BASELINE_BYTE_COUNT,
    X86_MOV_R32_IMM32_EXTENDED_REGISTER_BYTE_COUNT, X86_MOV_R32_IMM32_LOW_REGISTER_BYTE_COUNT,
    X86MovR32Imm32InstructionDisposition, X86MovR32Imm32MaterializationAction,
    X86MovR32Imm32MaterializationAttempt, X86MovR32Imm32MaterializationAttemptOutcome,
    X86MovR32Imm32MaterializationBlock, X86MovR32Imm32MaterializationError,
    X86MovR32Imm32MaterializationFunction, X86MovR32Imm32MaterializationIdentity,
    X86MovR32Imm32MaterializationInstruction, X86MovR32Imm32MaterializationPlan,
    X86MovR32Imm32MaterializationPolicy, X86MovR32Imm32MaterializationWorkAxis,
    X86MovR32Imm32PhysicalWrite, x86_mov_r32_imm32_materialization_identity,
};

pub(crate) fn compute<S: ValidatedSelectedAnalysis>(
    selected: &S,
    source: &ValidatedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    budget: OptimizationWorkBudget,
) -> Result<X86MovR32Imm32MaterializationPlan, X86MovR32Imm32MaterializationError> {
    compute_from_parts(
        selected.selected_plan(),
        selected.selected_identity(),
        source.plan(),
        source.receipt().identity(),
        physical,
        budget,
    )
}

pub(crate) fn compute_from_parts(
    selected: &SelectedInstructionPlan,
    selected_identity: omega_selected_instructions::SelectedInstructionPlanIdentity,
    source: &PostAllocationMachinePlan,
    source_identity: crate::PostAllocationMachineIdentity,
    physical: &ValidatedPhysicalRegisterModel,
    budget: OptimizationWorkBudget,
) -> Result<X86MovR32Imm32MaterializationPlan, X86MovR32Imm32MaterializationError> {
    validate_roots(
        selected,
        selected_identity,
        source,
        source_identity,
        physical,
    )?;
    let mut functions = baseline_roster(source);
    let mut attempts = Vec::new();
    let mut actions = Vec::new();
    let mut usage = OptimizationWorkUsage::default();

    loop {
        charge(
            &mut usage.iterations,
            budget.iterations(),
            X86MovR32Imm32MaterializationWorkAxis::Iterations,
        )?;
        let iteration = usage.iterations;
        let input = super::identity::revision_identity(
            source_identity,
            selected_identity,
            source.target,
            physical.identity(),
            &functions,
        );
        let mut candidate = None;
        'scan: for (function_index, selected_function) in selected.functions.iter().enumerate() {
            let machine_function = source.functions.get(function_index).ok_or(
                X86MovR32Imm32MaterializationError::FunctionRosterMismatch(function_index),
            )?;
            if machine_function.machine != selected_function.machine {
                return Err(X86MovR32Imm32MaterializationError::FunctionRosterMismatch(
                    function_index,
                ));
            }
            for (block_index, block) in selected_function.blocks.iter().enumerate() {
                let machine_block = machine_function.blocks.get(block_index).ok_or(
                    X86MovR32Imm32MaterializationError::BlockRosterMismatch {
                        function: function_index,
                        block: block_index,
                    },
                )?;
                if machine_block.block != block.id
                    || machine_block.instructions.len() != block.instructions.len() + 1
                {
                    return Err(X86MovR32Imm32MaterializationError::BlockRosterMismatch {
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
                        X86MovR32Imm32MaterializationWorkAxis::RuleEvaluations,
                    )?;
                    let machine = machine_block.instructions.get(instruction_index).ok_or(
                        X86MovR32Imm32MaterializationError::InstructionRosterMismatch(
                            instruction.id,
                        ),
                    )?;
                    let literal_bits = integer_bits(value, instruction.id)?;
                    validate_materialization(instruction, machine, physical)?;
                    let destination = qualified_write(instruction, machine, physical)?;
                    let selected_byte_count = selected_byte_count(&destination, physical)?;
                    let already_selected =
                        actions
                            .iter()
                            .any(|action: &X86MovR32Imm32MaterializationAction| {
                                action.machine == selected_function.machine
                                    && action.block == block.id
                                    && action.instruction == instruction.id
                            });
                    let outcome = if already_selected {
                        X86MovR32Imm32MaterializationAttemptOutcome::AlreadySelected
                    } else if literal_bits > u64::from(u32::MAX) {
                        X86MovR32Imm32MaterializationAttemptOutcome::IntegerOutsideZeroExtendedU32
                    } else {
                        X86MovR32Imm32MaterializationAttemptOutcome::SelectedForRewrite
                    };
                    attempts.push(X86MovR32Imm32MaterializationAttempt {
                        iteration,
                        input,
                        machine: selected_function.machine,
                        block: block.id,
                        instruction: instruction.id,
                        literal_bits,
                        destination: destination.clone(),
                        baseline_byte_count: X86_MOV_R32_IMM32_BASELINE_BYTE_COUNT,
                        selected_byte_count,
                        outcome,
                    });
                    if outcome == X86MovR32Imm32MaterializationAttemptOutcome::SelectedForRewrite {
                        charge(
                            &mut usage.candidates,
                            budget.candidates(),
                            X86MovR32Imm32MaterializationWorkAxis::Candidates,
                        )?;
                        charge(
                            &mut usage.validation_steps,
                            budget.validation_steps(),
                            X86MovR32Imm32MaterializationWorkAxis::ValidationSteps,
                        )?;
                        candidate = Some((
                            function_index,
                            block_index,
                            selected_function.machine,
                            block.id,
                            instruction.id,
                            literal_bits,
                            destination,
                            selected_byte_count,
                        ));
                        break 'scan;
                    }
                }
            }
        }

        let Some((
            function_index,
            block_index,
            machine,
            block,
            instruction,
            literal_bits,
            destination,
            selected_byte_count,
        )) = candidate
        else {
            break;
        };
        charge(
            &mut usage.commits,
            budget.commits(),
            X86MovR32Imm32MaterializationWorkAxis::Commits,
        )?;
        let row = functions[function_index].blocks[block_index]
            .instructions
            .iter_mut()
            .find(|row| row.instruction == instruction)
            .ok_or(X86MovR32Imm32MaterializationError::InstructionRosterMismatch(instruction))?;
        row.disposition = X86MovR32Imm32InstructionDisposition::MovR32Imm32MaterializationV1 {
            literal_bits,
            destination: destination.clone(),
            baseline_byte_count: X86_MOV_R32_IMM32_BASELINE_BYTE_COUNT,
            selected_byte_count,
        };
        let output = super::identity::revision_identity(
            source_identity,
            selected_identity,
            source.target,
            physical.identity(),
            &functions,
        );
        actions.push(X86MovR32Imm32MaterializationAction {
            iteration,
            input,
            output,
            machine,
            block,
            instruction,
            literal_bits,
            destination,
            baseline_byte_count: X86_MOV_R32_IMM32_BASELINE_BYTE_COUNT,
            selected_byte_count,
        });
    }

    let output_revision = super::identity::revision_identity(
        source_identity,
        selected_identity,
        source.target,
        physical.identity(),
        &functions,
    );
    let mut plan = X86MovR32Imm32MaterializationPlan {
        identity: X86MovR32Imm32MaterializationIdentity::from_bytes([0; 32]),
        source: source_identity,
        selected: selected_identity,
        target: source.target,
        physical_register_model: physical.identity(),
        policy: X86MovR32Imm32MaterializationPolicy::X86SelectMovR32Imm32ZeroExtendedI64MaterializationV1,
        budget,
        usage,
        output_revision,
        attempts,
        actions,
        functions,
    };
    plan.identity = x86_mov_r32_imm32_materialization_identity(&plan);
    Ok(plan)
}

fn validate_roots(
    selected: &SelectedInstructionPlan,
    selected_identity: omega_selected_instructions::SelectedInstructionPlanIdentity,
    source: &PostAllocationMachinePlan,
    source_identity: crate::PostAllocationMachineIdentity,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<(), X86MovR32Imm32MaterializationError> {
    if selected.target.architecture != Architecture::X86_64
        || source.target.architecture != Architecture::X86_64
        || physical.model().architecture != Architecture::X86_64
    {
        return Err(X86MovR32Imm32MaterializationError::UnsupportedTarget(
            source.target,
        ));
    }
    if source.identity != source_identity
        || source.selected != selected_identity
        || selected.target != source.target
        || source.physical_register_model != physical.identity()
        || selected.functions.len() != source.functions.len()
    {
        return Err(X86MovR32Imm32MaterializationError::RootMismatch);
    }
    Ok(())
}

fn baseline_roster(
    source: &PostAllocationMachinePlan,
) -> Vec<X86MovR32Imm32MaterializationFunction> {
    source
        .functions
        .iter()
        .map(|function| X86MovR32Imm32MaterializationFunction {
            machine: function.machine,
            blocks: function
                .blocks
                .iter()
                .map(|block| X86MovR32Imm32MaterializationBlock {
                    block: block.block,
                    instructions: block
                        .instructions
                        .iter()
                        .map(|instruction| X86MovR32Imm32MaterializationInstruction {
                            instruction: instruction.instruction,
                            disposition: X86MovR32Imm32InstructionDisposition::RetainedV1,
                        })
                        .collect(),
                })
                .collect(),
        })
        .collect()
}

fn integer_bits(
    value: IntegerValue,
    instruction: omega_selected_instructions::SelectedInstructionId,
) -> Result<u64, X86MovR32Imm32MaterializationError> {
    match value {
        IntegerValue::Signed(value) => i64::try_from(value)
            .map(|value| value as u64)
            .map_err(|_| X86MovR32Imm32MaterializationError::IntegerOutsideI64Bits(instruction)),
        IntegerValue::Unsigned(value) => u64::try_from(value)
            .map_err(|_| X86MovR32Imm32MaterializationError::IntegerOutsideI64Bits(instruction)),
    }
}

fn validate_materialization(
    selected: &SelectedInstruction,
    machine: &PostAllocationMachineInstruction,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<(), X86MovR32Imm32MaterializationError> {
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
        return Err(
            X86MovR32Imm32MaterializationError::InvalidMaterializationFootprint(selected.id),
        );
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
        return Err(
            X86MovR32Imm32MaterializationError::InvalidMaterializationFootprint(selected.id),
        );
    }
    validate_r64_view(operand, physical, selected.id)
}

fn validate_r64_view(
    operand: &PhysicalOperandFootprint,
    physical: &ValidatedPhysicalRegisterModel,
    instruction: omega_selected_instructions::SelectedInstructionId,
) -> Result<(), X86MovR32Imm32MaterializationError> {
    let view = physical
        .model()
        .views
        .iter()
        .find(|view| view.id == operand.view)
        .ok_or(X86MovR32Imm32MaterializationError::InvalidPhysicalDestination(instruction))?;
    if !is_canonical_allocatable_r64(&view.name)
        || view.bits != 64
        || !view.allocatable
        || view.units.len() != 4
        || view.write_semantics != RegisterWriteSemantics::ExactView
        || view.class != operand.class
        || view.units != operand.storage_units
        || view.write_units != operand.write_units
        || Some(view.write_semantics) != operand.write_semantics
    {
        return Err(X86MovR32Imm32MaterializationError::InvalidPhysicalDestination(instruction));
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
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<X86MovR32Imm32PhysicalWrite, X86MovR32Imm32MaterializationError> {
    let operand = machine
        .operands
        .first()
        .ok_or(X86MovR32Imm32MaterializationError::InvalidMaterializationFootprint(selected.id))?;
    let destination = physical
        .model()
        .views
        .iter()
        .find(|view| view.id == operand.view)
        .ok_or(X86MovR32Imm32MaterializationError::InvalidPhysicalDestination(selected.id))?;
    let (_, encoded_name, _) = canonical_view_names(&destination.name)
        .ok_or(X86MovR32Imm32MaterializationError::InvalidPhysicalDestination(selected.id))?;
    let encoded = physical
        .model()
        .view_named(encoded_name)
        .ok_or(X86MovR32Imm32MaterializationError::InvalidPhysicalDestination(selected.id))?;
    if encoded.bits != 32
        || !encoded.allocatable
        || encoded.write_semantics != RegisterWriteSemantics::ZeroExtendsParent
        || encoded.units.as_slice() != &destination.units[..3]
        || encoded.write_units != destination.units
    {
        return Err(X86MovR32Imm32MaterializationError::InvalidPhysicalDestination(selected.id));
    }
    Ok(X86MovR32Imm32PhysicalWrite {
        instruction: selected.id,
        operand: operand.operand,
        virtual_register: operand.virtual_register,
        class: operand.class,
        destination_view: operand.view,
        destination_storage_units: operand.storage_units.clone(),
        destination_write_units: operand.write_units.clone(),
        destination_write_semantics: operand.write_semantics.ok_or(
            X86MovR32Imm32MaterializationError::InvalidMaterializationFootprint(selected.id),
        )?,
        encoded_view: encoded.id,
        encoded_storage_units: encoded.units.clone(),
        encoded_write_units: encoded.write_units.clone(),
        encoded_write_semantics: encoded.write_semantics,
    })
}

fn selected_byte_count(
    destination: &X86MovR32Imm32PhysicalWrite,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<u8, X86MovR32Imm32MaterializationError> {
    let name = &physical
        .model()
        .views
        .iter()
        .find(|view| view.id == destination.destination_view)
        .ok_or(
            X86MovR32Imm32MaterializationError::InvalidPhysicalDestination(destination.instruction),
        )?
        .name;
    let (_, _, extended) = canonical_view_names(name).ok_or(
        X86MovR32Imm32MaterializationError::InvalidPhysicalDestination(destination.instruction),
    )?;
    Ok(if extended {
        X86_MOV_R32_IMM32_EXTENDED_REGISTER_BYTE_COUNT
    } else {
        X86_MOV_R32_IMM32_LOW_REGISTER_BYTE_COUNT
    })
}

fn canonical_view_names(name: &str) -> Option<(&'static str, &'static str, bool)> {
    Some(match name {
        "rax" => ("rax", "eax", false),
        "rbx" => ("rbx", "ebx", false),
        "rcx" => ("rcx", "ecx", false),
        "rdx" => ("rdx", "edx", false),
        "rsi" => ("rsi", "esi", false),
        "rdi" => ("rdi", "edi", false),
        "rbp" => ("rbp", "ebp", false),
        "r8" => ("r8", "r8d", true),
        "r9" => ("r9", "r9d", true),
        "r10" => ("r10", "r10d", true),
        "r11" => ("r11", "r11d", true),
        "r12" => ("r12", "r12d", true),
        "r13" => ("r13", "r13d", true),
        "r14" => ("r14", "r14d", true),
        "r15" => ("r15", "r15d", true),
        _ => return None,
    })
}

fn charge(
    usage: &mut u64,
    budget: u64,
    axis: X86MovR32Imm32MaterializationWorkAxis,
) -> Result<(), X86MovR32Imm32MaterializationError> {
    *usage = usage
        .checked_add(1)
        .ok_or(X86MovR32Imm32MaterializationError::BudgetExceeded(axis))?;
    if *usage > budget {
        return Err(X86MovR32Imm32MaterializationError::BudgetExceeded(axis));
    }
    Ok(())
}
