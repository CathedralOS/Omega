use omega_isa_x86_64::{
    validate_x86_64_register_constraint_catalog, x86_64_register_constraint_catalog,
};
use omega_machine_optimizer::{
    Aarch64CbnzInstructionDisposition, Aarch64SameViewCopyInstructionDisposition,
};
use omega_regalloc::ValidatedSelectedAnalysis;
use omega_register_model::ValidatedPhysicalRegisterModel;
use omega_selected_instructions::{SelectedInstruction, SelectedTerminator};
use omega_target::Architecture;

use crate::{
    StagedOptimizedPostAllocationMachineOptimization, StagedOptimizedPostAllocationMachinePlan,
};

use super::{
    OptimizedSelectedFormEncodingError, SelectedFormEncodingCounts, SelectedFormEncodingRow,
    SelectedFormEncodingState, SelectedFormMachineDisposition,
    SelectedStructuralUnitFunctionEncoding, StagedOptimizedSelectedFormEncoding,
    custody::validate_optimization_roots, identity::encoding_identity,
    materialization::MaterializationPlan, row_encoding::encode_row,
    structural_encoding::encode_structural_function,
};

pub(super) fn compute<S: ValidatedSelectedAnalysis>(
    selected: &S,
    staged: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    optimization: Option<&StagedOptimizedPostAllocationMachineOptimization>,
) -> Result<StagedOptimizedSelectedFormEncoding, OptimizedSelectedFormEncodingError> {
    let machine = staged.machine().plan();
    if machine.selected != selected.selected_identity() {
        return Err(OptimizedSelectedFormEncodingError::SelectedRootMismatch);
    }
    if machine.physical_register_model != physical.identity() {
        return Err(OptimizedSelectedFormEncodingError::PhysicalModelMismatch);
    }
    let post_allocation_machine_optimization = optimization
        .map(|optimization| validate_optimization_roots(selected, staged, physical, optimization))
        .transpose()?;
    let fusion = optimization.and_then(|optimization| match optimization {
        StagedOptimizedPostAllocationMachineOptimization::Aarch64Cbnz(fusion) => Some(fusion),
        _ => None,
    });
    let copy_elision = optimization.and_then(|optimization| match optimization {
        StagedOptimizedPostAllocationMachineOptimization::Aarch64SameViewCopyElision(elision) => {
            Some(elision)
        }
        _ => None,
    });
    let materialization = MaterializationPlan::from_optimization(optimization);
    let selected_plan = selected.selected_plan();
    if selected_plan.functions.len() != machine.functions.len() {
        return Err(OptimizedSelectedFormEncodingError::FunctionRosterMismatch);
    }
    let mut rows = Vec::new();
    for (function_index, (selected_function, machine_function)) in selected_plan
        .functions
        .iter()
        .zip(&machine.functions)
        .enumerate()
    {
        if selected_function.machine != machine_function.machine
            || selected_function.blocks.len() != machine_function.blocks.len()
        {
            return Err(OptimizedSelectedFormEncodingError::FunctionRosterMismatch);
        }
        let fusion_function = fusion
            .map(|fusion| {
                fusion
                    .fusion()
                    .plan()
                    .functions
                    .get(function_index)
                    .ok_or(OptimizedSelectedFormEncodingError::FunctionRosterMismatch)
            })
            .transpose()?;
        let materialization_function = materialization
            .map(|materialization| materialization.function(function_index))
            .transpose()?;
        let copy_elision_function = copy_elision
            .map(|elision| {
                elision
                    .elision()
                    .plan()
                    .functions
                    .get(function_index)
                    .ok_or(OptimizedSelectedFormEncodingError::FunctionRosterMismatch)
            })
            .transpose()?;
        if fusion_function.is_some_and(|row| row.machine != selected_function.machine) {
            return Err(OptimizedSelectedFormEncodingError::FunctionRosterMismatch);
        }
        if materialization_function.is_some_and(|row| {
            !row.matches(selected_function.machine, selected_function.blocks.len())
        }) {
            return Err(OptimizedSelectedFormEncodingError::FunctionRosterMismatch);
        }
        if copy_elision_function.is_some_and(|row| row.machine != selected_function.machine) {
            return Err(OptimizedSelectedFormEncodingError::FunctionRosterMismatch);
        }
        for (block_index, (selected_block, machine_block)) in selected_function
            .blocks
            .iter()
            .zip(&machine_function.blocks)
            .enumerate()
        {
            if selected_block.id != machine_block.block
                || selected_block.instructions.len() + 1 != machine_block.instructions.len()
            {
                return Err(OptimizedSelectedFormEncodingError::BlockRosterMismatch);
            }
            let fusion_block = fusion_function
                .map(|function| {
                    function
                        .blocks
                        .get(block_index)
                        .ok_or(OptimizedSelectedFormEncodingError::BlockRosterMismatch)
                })
                .transpose()?;
            let materialization_block = materialization_function
                .map(|function| function.block(block_index))
                .transpose()?;
            let copy_elision_block = copy_elision_function
                .map(|function| {
                    function
                        .blocks
                        .get(block_index)
                        .ok_or(OptimizedSelectedFormEncodingError::BlockRosterMismatch)
                })
                .transpose()?;
            if fusion_block.is_some_and(|row| {
                row.block != selected_block.id
                    || row.instructions.len() != machine_block.instructions.len()
            }) {
                return Err(OptimizedSelectedFormEncodingError::BlockRosterMismatch);
            }
            if materialization_block.is_some_and(|row| {
                !row.matches(selected_block.id, machine_block.instructions.len())
            }) {
                return Err(OptimizedSelectedFormEncodingError::BlockRosterMismatch);
            }
            if copy_elision_block.is_some_and(|row| {
                row.block != selected_block.id
                    || row.instructions.len() != machine_block.instructions.len()
            }) {
                return Err(OptimizedSelectedFormEncodingError::BlockRosterMismatch);
            }
            for (index, machine_instruction) in machine_block.instructions.iter().enumerate() {
                let selected_instruction = if index < selected_block.instructions.len() {
                    &selected_block.instructions[index]
                } else {
                    terminator_instruction(&selected_block.terminator)
                };
                if selected_instruction.id != machine_instruction.instruction {
                    return Err(OptimizedSelectedFormEncodingError::InstructionRosterMismatch);
                }
                let disposition = fusion_block
                    .map(|block| {
                        block
                            .instructions
                            .get(index)
                            .ok_or(OptimizedSelectedFormEncodingError::InstructionRosterMismatch)
                    })
                    .transpose()?;
                if disposition.is_some_and(|row| row.instruction != selected_instruction.id) {
                    return Err(OptimizedSelectedFormEncodingError::InstructionRosterMismatch);
                }
                let materialization_disposition = materialization_block
                    .map(|block| block.disposition(index, selected_instruction.id))
                    .transpose()?;
                let copy_elision_disposition = copy_elision_block
                    .map(|block| {
                        block
                            .instructions
                            .get(index)
                            .filter(|row| row.instruction == selected_instruction.id)
                            .ok_or(OptimizedSelectedFormEncodingError::InstructionRosterMismatch)
                    })
                    .transpose()?;
                let machine_disposition = match (disposition, copy_elision_disposition) {
                    (Some(row), None) => match &row.disposition {
                        Aarch64CbnzInstructionDisposition::RetainedV1 => {
                            SelectedFormMachineDisposition::RetainedV1
                        }
                        Aarch64CbnzInstructionDisposition::ElidedCompareI64ZeroV1 { consumer } => {
                            SelectedFormMachineDisposition::Aarch64ElidedCompareI64ZeroV1 {
                                consumer: *consumer,
                            }
                        }
                        Aarch64CbnzInstructionDisposition::FusedBranchNonZeroToCbnzV1 {
                            compare,
                            source_read,
                        } => SelectedFormMachineDisposition::Aarch64FusedBranchNonZeroToCbnzV1 {
                            compare: *compare,
                            source_read: source_read.clone(),
                        },
                    },
                    (None, Some(row)) => match &row.disposition {
                        Aarch64SameViewCopyInstructionDisposition::RetainedV1 => {
                            SelectedFormMachineDisposition::RetainedV1
                        }
                        Aarch64SameViewCopyInstructionDisposition::ElidedSameViewCopyI64V1 {
                            consumer,
                        } => SelectedFormMachineDisposition::Aarch64ElidedSameViewCopyI64V1 {
                            consumer: *consumer,
                        },
                    },
                    (None, None) => SelectedFormMachineDisposition::RetainedV1,
                    (Some(_), Some(_)) => {
                        return Err(OptimizedSelectedFormEncodingError::ArtifactMismatch);
                    }
                };
                rows.push(encode_row(
                    selected_plan.target.architecture,
                    selected_instruction,
                    machine_instruction,
                    physical,
                    machine_disposition,
                    materialization_disposition,
                )?);
            }
        }
    }
    let effect_plan = staged.effects().effects().plan();
    if selected_plan.structural_unit_functions.len() != machine.structural_unit_functions.len()
        || selected_plan.structural_unit_functions.len()
            != effect_plan.structural_unit_functions.len()
    {
        return Err(OptimizedSelectedFormEncodingError::StructuralFunctionRosterMismatch);
    }
    let structural_constraints = if selected_plan.structural_unit_functions.is_empty() {
        None
    } else {
        if selected_plan.target.architecture != Architecture::X86_64 {
            return Err(OptimizedSelectedFormEncodingError::StructuralFunctionRosterMismatch);
        }
        let constraints = validate_x86_64_register_constraint_catalog(
            x86_64_register_constraint_catalog(physical),
            physical,
        )
        .map_err(|_| OptimizedSelectedFormEncodingError::StructuralConstraintCatalogMismatch)?;
        if constraints.identity() != machine.register_constraints
            || constraints.identity() != effect_plan.register_constraints
        {
            return Err(OptimizedSelectedFormEncodingError::StructuralConstraintCatalogMismatch);
        }
        Some(constraints)
    };
    let mut structural_unit_functions =
        Vec::with_capacity(selected_plan.structural_unit_functions.len());
    for ((selected_function, machine_function), effect_function) in selected_plan
        .structural_unit_functions
        .iter()
        .zip(&machine.structural_unit_functions)
        .zip(&effect_plan.structural_unit_functions)
    {
        structural_unit_functions.push(encode_structural_function(
            selected_plan.target,
            selected_plan,
            selected_function,
            machine_function,
            effect_function,
            physical,
            structural_constraints
                .as_ref()
                .ok_or(OptimizedSelectedFormEncodingError::StructuralConstraintCatalogMismatch)?,
        )?);
    }
    let counts = encoding_counts(&rows, &structural_unit_functions)?;
    let selected_root = selected.selected_identity();
    let machine_root = staged.machine().receipt().identity();
    let identity = encoding_identity(
        selected_root,
        machine_root,
        post_allocation_machine_optimization,
        &rows,
        &structural_unit_functions,
        counts,
    );
    Ok(StagedOptimizedSelectedFormEncoding {
        selected: selected_root,
        machine: machine_root,
        post_allocation_machine_optimization,
        identity,
        rows,
        structural_unit_functions,
        counts,
    })
}

fn encoding_counts(
    rows: &[SelectedFormEncodingRow],
    structural: &[SelectedStructuralUnitFunctionEncoding],
) -> Result<SelectedFormEncodingCounts, OptimizedSelectedFormEncodingError> {
    let mut counts = SelectedFormEncodingCounts::default();
    for row in rows {
        let count = match row.state {
            SelectedFormEncodingState::Encoded { .. } => &mut counts.ordinary_encoded,
            SelectedFormEncodingState::DeferredControl { .. } => {
                &mut counts.ordinary_deferred_control
            }
        };
        *count = count
            .checked_add(1)
            .ok_or(OptimizedSelectedFormEncodingError::CountOverflow)?;
    }
    for function in structural {
        counts.structural_encoded_returns = counts
            .structural_encoded_returns
            .checked_add(1)
            .ok_or(OptimizedSelectedFormEncodingError::CountOverflow)?;
        if function.call.is_some() {
            counts.structural_encoded_call_templates = counts
                .structural_encoded_call_templates
                .checked_add(1)
                .ok_or(OptimizedSelectedFormEncodingError::CountOverflow)?;
            counts.structural_deferred_internal_control = counts
                .structural_deferred_internal_control
                .checked_add(1)
                .ok_or(OptimizedSelectedFormEncodingError::CountOverflow)?;
            counts.structural_internal_fixups = counts
                .structural_internal_fixups
                .checked_add(1)
                .ok_or(OptimizedSelectedFormEncodingError::CountOverflow)?;
        }
    }
    Ok(counts)
}

fn terminator_instruction(terminator: &SelectedTerminator) -> &SelectedInstruction {
    match terminator {
        SelectedTerminator::ConditionalBranch { instruction, .. }
        | SelectedTerminator::ConditionalBranchU64LessThan { instruction, .. }
        | SelectedTerminator::Return { instruction, .. } => instruction,
    }
}
