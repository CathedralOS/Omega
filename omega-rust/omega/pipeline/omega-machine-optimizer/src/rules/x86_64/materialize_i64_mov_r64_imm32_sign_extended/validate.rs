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
    ValidatedPostAllocationMachinePlan, ValidatedX86MovR64Imm32SignExtendedMaterialization,
    X86_MOV_R64_IMM32_SIGN_EXTENDED_BASELINE_BYTE_COUNT,
    X86_MOV_R64_IMM32_SIGN_EXTENDED_EXTENDED_REGISTER_BYTE_COUNT,
    X86_MOV_R64_IMM32_SIGN_EXTENDED_LOW_REGISTER_BYTE_COUNT,
    X86MovR64Imm32SignExtendedInstructionDisposition,
    X86MovR64Imm32SignExtendedMaterializationAction,
    X86MovR64Imm32SignExtendedMaterializationAttempt,
    X86MovR64Imm32SignExtendedMaterializationAttemptOutcome,
    X86MovR64Imm32SignExtendedMaterializationBlock, X86MovR64Imm32SignExtendedMaterializationError,
    X86MovR64Imm32SignExtendedMaterializationFunction,
    X86MovR64Imm32SignExtendedMaterializationIdentity,
    X86MovR64Imm32SignExtendedMaterializationInstruction,
    X86MovR64Imm32SignExtendedMaterializationPlan, X86MovR64Imm32SignExtendedMaterializationPolicy,
    X86MovR64Imm32SignExtendedMaterializationWorkAxis, X86MovR64Imm32SignExtendedPhysicalWrite,
    x86_mov_r64_imm32_sign_extended_materialization_identity,
    x86_mov_r64_imm32_sign_extended_materialization_receipt,
};

/// Independently reconstruct every attempt, destination view, action,
/// disposition, revision, work counter, and content identity.
pub fn validate_x86_mov_r64_imm32_sign_extended_materialization<S: ValidatedSelectedAnalysis>(
    selected: &S,
    source: &ValidatedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    plan: X86MovR64Imm32SignExtendedMaterializationPlan,
) -> Result<
    ValidatedX86MovR64Imm32SignExtendedMaterialization,
    X86MovR64Imm32SignExtendedMaterializationError,
> {
    validate_from_parts(
        selected.selected_plan(),
        selected.selected_identity(),
        source.plan(),
        source.receipt().identity(),
        physical,
        &plan,
    )?;
    let receipt = x86_mov_r64_imm32_sign_extended_materialization_receipt(&plan)?;
    Ok(ValidatedX86MovR64Imm32SignExtendedMaterialization::new(
        plan, receipt,
    ))
}

pub(crate) fn validate_from_parts(
    selected: &SelectedInstructionPlan,
    selected_identity: omega_selected_instructions::SelectedInstructionPlanIdentity,
    source: &PostAllocationMachinePlan,
    source_identity: crate::PostAllocationMachineIdentity,
    physical: &ValidatedPhysicalRegisterModel,
    plan: &X86MovR64Imm32SignExtendedMaterializationPlan,
) -> Result<(), X86MovR64Imm32SignExtendedMaterializationError> {
    if plan.policy
        != X86MovR64Imm32SignExtendedMaterializationPolicy::X86SelectMovR64Imm32SignExtendedI64MaterializationV1
    {
        return Err(X86MovR64Imm32SignExtendedMaterializationError::ArtifactMismatch);
    }
    let expected = replay_from_parts(
        selected,
        selected_identity,
        source,
        source_identity,
        physical,
        plan.budget,
    )?;
    if *plan != expected {
        return Err(X86MovR64Imm32SignExtendedMaterializationError::ArtifactMismatch);
    }
    Ok(())
}

pub(crate) fn replay_from_parts(
    selected: &SelectedInstructionPlan,
    selected_identity: omega_selected_instructions::SelectedInstructionPlanIdentity,
    source: &PostAllocationMachinePlan,
    source_identity: crate::PostAllocationMachineIdentity,
    physical: &ValidatedPhysicalRegisterModel,
    budget: OptimizationWorkBudget,
) -> Result<
    X86MovR64Imm32SignExtendedMaterializationPlan,
    X86MovR64Imm32SignExtendedMaterializationError,
> {
    independently_validate_roots(
        selected,
        selected_identity,
        source,
        source_identity,
        physical,
    )?;
    let mut functions = independently_reconstruct_roster(source);
    let mut attempts = Vec::new();
    let mut actions = Vec::new();
    let mut usage = OptimizationWorkUsage::default();

    loop {
        independently_charge(
            &mut usage.iterations,
            budget.iterations(),
            X86MovR64Imm32SignExtendedMaterializationWorkAxis::Iterations,
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
        'candidate_scan: for (function_index, selected_function) in
            selected.functions.iter().enumerate()
        {
            let machine_function = source.functions.get(function_index).ok_or(
                X86MovR64Imm32SignExtendedMaterializationError::FunctionRosterMismatch(
                    function_index,
                ),
            )?;
            if selected_function.machine != machine_function.machine {
                return Err(
                    X86MovR64Imm32SignExtendedMaterializationError::FunctionRosterMismatch(
                        function_index,
                    ),
                );
            }
            for block_index in 0..selected_function.blocks.len() {
                let block = &selected_function.blocks[block_index];
                let machine_block = machine_function.blocks.get(block_index).ok_or(
                    X86MovR64Imm32SignExtendedMaterializationError::BlockRosterMismatch {
                        function: function_index,
                        block: block_index,
                    },
                )?;
                let expected_instruction_count = block
                    .instructions
                    .len()
                    .checked_add(1)
                    .ok_or(X86MovR64Imm32SignExtendedMaterializationError::CountOverflow)?;
                if machine_block.block != block.id
                    || machine_block.instructions.len() != expected_instruction_count
                {
                    return Err(
                        X86MovR64Imm32SignExtendedMaterializationError::BlockRosterMismatch {
                            function: function_index,
                            block: block_index,
                        },
                    );
                }
                for instruction_index in 0..block.instructions.len() {
                    let instruction = &block.instructions[instruction_index];
                    let SelectedInstructionKind::MaterializeI64 { value } = instruction.kind else {
                        continue;
                    };
                    independently_charge(
                        &mut usage.rule_evaluations,
                        budget.rule_evaluations(),
                        X86MovR64Imm32SignExtendedMaterializationWorkAxis::RuleEvaluations,
                    )?;
                    let machine = &machine_block.instructions[instruction_index];
                    let literal_bits = independently_reconstruct_i64_bits(value, instruction.id)?;
                    independently_validate_materialization(instruction, machine, physical)?;
                    let destination =
                        independently_reconstruct_write(instruction, machine, physical)?;
                    let selected_byte_count =
                        independently_selected_byte_count(&destination, physical)?;
                    let was_selected = actions.iter().any(
                        |prior: &X86MovR64Imm32SignExtendedMaterializationAction| {
                            prior.machine == selected_function.machine
                                && prior.block == block.id
                                && prior.instruction == instruction.id
                        },
                    );
                    let sign_extended =
                        (i64::from(literal_bits as u32 as i32) as u64) == literal_bits;
                    let outcome = match (was_selected, sign_extended) {
                        (true, _) => X86MovR64Imm32SignExtendedMaterializationAttemptOutcome::AlreadySelected,
                        (false, false) => X86MovR64Imm32SignExtendedMaterializationAttemptOutcome::IntegerOutsideSignExtendedI32,
                        (false, true) => X86MovR64Imm32SignExtendedMaterializationAttemptOutcome::SelectedForRewrite,
                    };
                    attempts.push(X86MovR64Imm32SignExtendedMaterializationAttempt {
                        iteration,
                        input,
                        machine: selected_function.machine,
                        block: block.id,
                        instruction: instruction.id,
                        literal_bits,
                        destination: destination.clone(),
                        baseline_byte_count: X86_MOV_R64_IMM32_SIGN_EXTENDED_BASELINE_BYTE_COUNT,
                        selected_byte_count,
                        outcome,
                    });
                    if outcome == X86MovR64Imm32SignExtendedMaterializationAttemptOutcome::SelectedForRewrite {
                        independently_charge(
                            &mut usage.candidates,
                            budget.candidates(),
                            X86MovR64Imm32SignExtendedMaterializationWorkAxis::Candidates,
                        )?;
                        independently_charge(
                            &mut usage.validation_steps,
                            budget.validation_steps(),
                            X86MovR64Imm32SignExtendedMaterializationWorkAxis::ValidationSteps,
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
                        break 'candidate_scan;
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
        independently_charge(
            &mut usage.commits,
            budget.commits(),
            X86MovR64Imm32SignExtendedMaterializationWorkAxis::Commits,
        )?;
        let disposition = X86MovR64Imm32SignExtendedInstructionDisposition::MovR64Imm32SignExtendedMaterializationV1 {
            literal_bits,
            destination: destination.clone(),
            baseline_byte_count: X86_MOV_R64_IMM32_SIGN_EXTENDED_BASELINE_BYTE_COUNT,
            selected_byte_count,
        };
        let row = functions[function_index].blocks[block_index]
            .instructions
            .iter_mut()
            .find(|row| row.instruction == instruction)
            .ok_or(
                X86MovR64Imm32SignExtendedMaterializationError::InstructionRosterMismatch(
                    instruction,
                ),
            )?;
        row.disposition = disposition;
        let output = super::identity::revision_identity(
            source_identity,
            selected_identity,
            source.target,
            physical.identity(),
            &functions,
        );
        actions.push(X86MovR64Imm32SignExtendedMaterializationAction {
            iteration,
            input,
            output,
            machine,
            block,
            instruction,
            literal_bits,
            destination,
            baseline_byte_count: X86_MOV_R64_IMM32_SIGN_EXTENDED_BASELINE_BYTE_COUNT,
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
    let mut expected = X86MovR64Imm32SignExtendedMaterializationPlan {
        identity: X86MovR64Imm32SignExtendedMaterializationIdentity::from_bytes([0; 32]),
        source: source_identity,
        selected: selected_identity,
        target: source.target,
        physical_register_model: physical.identity(),
        policy: X86MovR64Imm32SignExtendedMaterializationPolicy::X86SelectMovR64Imm32SignExtendedI64MaterializationV1,
        budget,
        usage,
        output_revision,
        attempts,
        actions,
        functions,
    };
    expected.identity = x86_mov_r64_imm32_sign_extended_materialization_identity(&expected);
    Ok(expected)
}

fn independently_validate_roots(
    selected: &SelectedInstructionPlan,
    selected_identity: omega_selected_instructions::SelectedInstructionPlanIdentity,
    source: &PostAllocationMachinePlan,
    source_identity: crate::PostAllocationMachineIdentity,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<(), X86MovR64Imm32SignExtendedMaterializationError> {
    if selected.target.architecture != Architecture::X86_64
        || source.target.architecture != Architecture::X86_64
        || physical.model().architecture != Architecture::X86_64
    {
        return Err(
            X86MovR64Imm32SignExtendedMaterializationError::UnsupportedTarget(source.target),
        );
    }
    let roots_match = source.identity == source_identity
        && source.selected == selected_identity
        && selected.target == source.target
        && source.physical_register_model == physical.identity()
        && selected.functions.len() == source.functions.len();
    if !roots_match {
        return Err(X86MovR64Imm32SignExtendedMaterializationError::RootMismatch);
    }
    Ok(())
}

fn independently_reconstruct_roster(
    source: &PostAllocationMachinePlan,
) -> Vec<X86MovR64Imm32SignExtendedMaterializationFunction> {
    let mut output = Vec::with_capacity(source.functions.len());
    for function in &source.functions {
        let mut blocks = Vec::with_capacity(function.blocks.len());
        for block in &function.blocks {
            let instructions = block
                .instructions
                .iter()
                .map(
                    |instruction| X86MovR64Imm32SignExtendedMaterializationInstruction {
                        instruction: instruction.instruction,
                        disposition: X86MovR64Imm32SignExtendedInstructionDisposition::RetainedV1,
                    },
                )
                .collect();
            blocks.push(X86MovR64Imm32SignExtendedMaterializationBlock {
                block: block.block,
                instructions,
            });
        }
        output.push(X86MovR64Imm32SignExtendedMaterializationFunction {
            machine: function.machine,
            blocks,
        });
    }
    output
}

fn independently_reconstruct_i64_bits(
    value: IntegerValue,
    instruction: omega_selected_instructions::SelectedInstructionId,
) -> Result<u64, X86MovR64Imm32SignExtendedMaterializationError> {
    match value {
        IntegerValue::Signed(raw) => {
            let narrowed = i64::try_from(raw).map_err(|_| {
                X86MovR64Imm32SignExtendedMaterializationError::IntegerOutsideI64Bits(instruction)
            })?;
            Ok(u64::from_ne_bytes(narrowed.to_ne_bytes()))
        }
        IntegerValue::Unsigned(raw) => u64::try_from(raw).map_err(|_| {
            X86MovR64Imm32SignExtendedMaterializationError::IntegerOutsideI64Bits(instruction)
        }),
    }
}

fn independently_validate_materialization(
    selected: &SelectedInstruction,
    machine: &PostAllocationMachineInstruction,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<(), X86MovR64Imm32SignExtendedMaterializationError> {
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
        return Err(
            X86MovR64Imm32SignExtendedMaterializationError::InvalidMaterializationFootprint(
                selected.id,
            ),
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
            X86MovR64Imm32SignExtendedMaterializationError::InvalidMaterializationFootprint(
                selected.id,
            ),
        );
    }
    independently_validate_r64(operand, physical, selected.id)
}

fn independently_validate_r64(
    operand: &PhysicalOperandFootprint,
    physical: &ValidatedPhysicalRegisterModel,
    instruction: omega_selected_instructions::SelectedInstructionId,
) -> Result<(), X86MovR64Imm32SignExtendedMaterializationError> {
    let view = physical
        .model()
        .views
        .iter()
        .find(|candidate| candidate.id == operand.view)
        .ok_or(
            X86MovR64Imm32SignExtendedMaterializationError::InvalidPhysicalDestination(instruction),
        )?;
    const NAMES: [&str; 15] = [
        "rax", "rbx", "rcx", "rdx", "rsi", "rdi", "rbp", "r8", "r9", "r10", "r11", "r12", "r13",
        "r14", "r15",
    ];
    if !NAMES.contains(&view.name.as_str())
        || view.bits != 64
        || !view.allocatable
        || view.units.len() != 4
        || view.write_semantics != RegisterWriteSemantics::ExactView
        || view.class != operand.class
        || view.units != operand.storage_units
        || view.write_units != operand.write_units
        || operand.write_semantics != Some(view.write_semantics)
    {
        return Err(
            X86MovR64Imm32SignExtendedMaterializationError::InvalidPhysicalDestination(instruction),
        );
    }
    Ok(())
}

fn independently_reconstruct_write(
    selected: &SelectedInstruction,
    machine: &PostAllocationMachineInstruction,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<X86MovR64Imm32SignExtendedPhysicalWrite, X86MovR64Imm32SignExtendedMaterializationError>
{
    let operand = machine.operands.first().ok_or(
        X86MovR64Imm32SignExtendedMaterializationError::InvalidMaterializationFootprint(
            selected.id,
        ),
    )?;
    let destination_write_semantics = operand.write_semantics.ok_or(
        X86MovR64Imm32SignExtendedMaterializationError::InvalidMaterializationFootprint(
            selected.id,
        ),
    )?;
    let destination = physical
        .model()
        .views
        .iter()
        .find(|view| view.id == operand.view)
        .ok_or(
            X86MovR64Imm32SignExtendedMaterializationError::InvalidPhysicalDestination(selected.id),
        )?;
    independently_encoded_view_name(&destination.name).ok_or(
        X86MovR64Imm32SignExtendedMaterializationError::InvalidPhysicalDestination(selected.id),
    )?;
    Ok(X86MovR64Imm32SignExtendedPhysicalWrite {
        instruction: selected.id,
        operand: operand.operand,
        virtual_register: operand.virtual_register,
        class: operand.class,
        destination_view: operand.view,
        destination_storage_units: operand.storage_units.clone(),
        destination_write_units: operand.write_units.clone(),
        destination_write_semantics,
        encoded_view: destination.id,
        encoded_storage_units: destination.units.clone(),
        encoded_write_units: destination.write_units.clone(),
        encoded_write_semantics: destination.write_semantics,
    })
}

fn independently_encoded_view_name(destination: &str) -> Option<&'static str> {
    Some(match destination {
        "rax" => "rax",
        "rbx" => "rbx",
        "rcx" => "rcx",
        "rdx" => "rdx",
        "rsi" => "rsi",
        "rdi" => "rdi",
        "rbp" => "rbp",
        "r8" => "r8",
        "r9" => "r9",
        "r10" => "r10",
        "r11" => "r11",
        "r12" => "r12",
        "r13" => "r13",
        "r14" => "r14",
        "r15" => "r15",
        _ => return None,
    })
}

fn independently_selected_byte_count(
    destination: &X86MovR64Imm32SignExtendedPhysicalWrite,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<u8, X86MovR64Imm32SignExtendedMaterializationError> {
    let name = physical
        .model()
        .views
        .iter()
        .find(|view| view.id == destination.destination_view)
        .map(|view| view.name.as_str())
        .ok_or(
            X86MovR64Imm32SignExtendedMaterializationError::InvalidPhysicalDestination(
                destination.instruction,
            ),
        )?;
    Ok(
        if matches!(
            name,
            "r8" | "r9" | "r10" | "r11" | "r12" | "r13" | "r14" | "r15"
        ) {
            X86_MOV_R64_IMM32_SIGN_EXTENDED_EXTENDED_REGISTER_BYTE_COUNT
        } else if independently_encoded_view_name(name).is_some() {
            X86_MOV_R64_IMM32_SIGN_EXTENDED_LOW_REGISTER_BYTE_COUNT
        } else {
            return Err(
                X86MovR64Imm32SignExtendedMaterializationError::InvalidPhysicalDestination(
                    destination.instruction,
                ),
            );
        },
    )
}

fn independently_charge(
    usage: &mut u64,
    budget: u64,
    axis: X86MovR64Imm32SignExtendedMaterializationWorkAxis,
) -> Result<(), X86MovR64Imm32SignExtendedMaterializationError> {
    let next = usage
        .checked_add(1)
        .ok_or(X86MovR64Imm32SignExtendedMaterializationError::BudgetExceeded(axis))?;
    if next > budget {
        return Err(X86MovR64Imm32SignExtendedMaterializationError::BudgetExceeded(axis));
    }
    *usage = next;
    Ok(())
}

const _: [(); X86_MOV_R64_IMM32_SIGN_EXTENDED_BASELINE_BYTE_COUNT as usize] = [(); 10];
const _: [(); X86_MOV_R64_IMM32_SIGN_EXTENDED_LOW_REGISTER_BYTE_COUNT as usize] = [(); 7];
const _: [(); X86_MOV_R64_IMM32_SIGN_EXTENDED_EXTENDED_REGISTER_BYTE_COUNT as usize] = [(); 7];
