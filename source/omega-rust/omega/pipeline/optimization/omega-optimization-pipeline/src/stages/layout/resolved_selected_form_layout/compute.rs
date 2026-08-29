use std::collections::BTreeMap;

use omega_regalloc::ValidatedSelectedAnalysis;
use omega_register_model::ValidatedPhysicalRegisterModel;

use crate::{
    StagedOptimizedPostAllocationMachineOptimization, StagedOptimizedPostAllocationMachinePlan,
    StagedOptimizedSelectedFormEncoding, stage_optimized_layout_independent_selected_form_encoding,
    validate_optimized_layout_independent_selected_form_encoding_with_post_allocation_machine_optimization,
};

use super::error::OptimizedResolvedSelectedFormLayoutError;
use super::identity::layout_identity;
use super::model::{SelectedFunctionLayoutPolicy, StagedOptimizedResolvedSelectedFormLayout};
use super::optimization::{validate_layout_byte_savings, validate_optimization_custody};
use super::ordinary::{instructions, layout, select};
use super::structural::layout_structural_unit_function;

pub(super) fn compute<S: ValidatedSelectedAnalysis>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    pre_layout: &StagedOptimizedSelectedFormEncoding,
    optimization: Option<&StagedOptimizedPostAllocationMachineOptimization>,
) -> Result<StagedOptimizedResolvedSelectedFormLayout, OptimizedResolvedSelectedFormLayoutError> {
    validate_optimized_layout_independent_selected_form_encoding_with_post_allocation_machine_optimization(
        selected,
        machine,
        physical,
        optimization,
        pre_layout,
    )
    .map_err(OptimizedResolvedSelectedFormLayoutError::PreLayout)?;
    let normalized = validate_optimization_custody(machine, pre_layout, optimization)?;
    let fusion = optimization.and_then(|optimization| match optimization {
        StagedOptimizedPostAllocationMachineOptimization::Aarch64Cbnz(fusion) => Some(fusion),
        StagedOptimizedPostAllocationMachineOptimization::Aarch64Movn(_)
        | StagedOptimizedPostAllocationMachineOptimization::X86MovR32Imm32(_)
        | StagedOptimizedPostAllocationMachineOptimization::X86XorZero(_) => None,
    });
    let selected_plan = selected.selected_plan();
    let machine_plan = machine.machine().plan();
    if pre_layout.selected() != selected.selected_identity()
        || pre_layout.machine() != machine.machine().receipt().identity()
        || selected_plan.target != machine_plan.target
        || selected_plan.target.architecture != physical.model().architecture
        || selected_plan.functions.len() != machine_plan.functions.len()
        || selected_plan.structural_unit_functions.len()
            != machine_plan.structural_unit_functions.len()
        || selected_plan.structural_unit_functions.len()
            != pre_layout.structural_unit_functions().len()
        || pre_layout.post_allocation_machine_optimization() != normalized
    {
        return Err(OptimizedResolvedSelectedFormLayoutError::RootMismatch);
    }

    let has_ordinary = !selected_plan.functions.is_empty();
    let has_structural = !selected_plan.structural_unit_functions.is_empty();
    if has_ordinary && has_structural {
        return Err(OptimizedResolvedSelectedFormLayoutError::MixedOrdinaryAndStructuralFunctions);
    }
    if has_structural && optimization.is_some() {
        return Err(OptimizedResolvedSelectedFormLayoutError::RootMismatch);
    }
    let policy = if has_structural {
        SelectedFunctionLayoutPolicy::StructuralUnitCallThenReturnSingleEntryBlockV1
    } else {
        select(selected_plan)?
    };
    let mut pre_rows = pre_layout.rows().iter();
    let mut functions = Vec::with_capacity(selected_plan.functions.len());
    for (function, machine_function) in selected_plan.functions.iter().zip(&machine_plan.functions)
    {
        let mut function_pre_rows = BTreeMap::new();
        for block in &function.blocks {
            for instruction in instructions(block) {
                let row = pre_rows.next().ok_or(
                    OptimizedResolvedSelectedFormLayoutError::MissingInstruction(instruction.id),
                )?;
                if row.instruction != instruction.id {
                    return Err(
                        OptimizedResolvedSelectedFormLayoutError::MissingInstruction(
                            instruction.id,
                        ),
                    );
                }
                if function_pre_rows.insert(instruction.id, row).is_some() {
                    return Err(
                        OptimizedResolvedSelectedFormLayoutError::DuplicateInstruction(
                            instruction.id,
                        ),
                    );
                }
            }
        }
        let machine_rows = machine_function
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .map(|instruction| (instruction.instruction, instruction))
            .collect::<BTreeMap<_, _>>();
        functions.push(layout(
            selected_plan.target.architecture,
            function,
            &function_pre_rows,
            &machine_rows,
            physical,
            fusion,
        )?);
    }
    if pre_rows.next().is_some() {
        return Err(OptimizedResolvedSelectedFormLayoutError::RootMismatch);
    }

    let mut structural_unit_functions =
        Vec::with_capacity(selected_plan.structural_unit_functions.len());
    for ((selected_function, machine_function), pre_function) in selected_plan
        .structural_unit_functions
        .iter()
        .zip(&machine_plan.structural_unit_functions)
        .zip(pre_layout.structural_unit_functions())
    {
        if selected_function.machine != machine_function.machine
            || selected_function.machine != pre_function.machine
            || selected_function.entry_block != machine_function.block
            || selected_function.entry_block != pre_function.block
        {
            return Err(
                OptimizedResolvedSelectedFormLayoutError::StructuralFunctionRosterMismatch(
                    selected_function.machine,
                ),
            );
        }
        match (
            &selected_function.call,
            &machine_function.call,
            &pre_function.call,
        ) {
            (None, None, None) => {}
            (Some(selected_call), Some(machine_call), Some(pre_call))
                if selected_call.id == machine_call.instruction
                    && selected_call.id == pre_call.instruction
                    && selected_call.operation == machine_call.operation
                    && selected_call.operation == pre_call.operation
                    && selected_call.callee == machine_call.callee
                    && selected_call.callee == pre_call.callee => {}
            (Some(selected_call), _, _) => {
                return Err(
                    OptimizedResolvedSelectedFormLayoutError::StructuralCallRosterMismatch(
                        selected_call.id,
                    ),
                );
            }
            (None, Some(machine_call), _) => {
                return Err(
                    OptimizedResolvedSelectedFormLayoutError::StructuralCallRosterMismatch(
                        machine_call.instruction,
                    ),
                );
            }
            (None, None, Some(pre_call)) => {
                return Err(
                    OptimizedResolvedSelectedFormLayoutError::StructuralCallRosterMismatch(
                        pre_call.instruction,
                    ),
                );
            }
        }
        let selected_return = &selected_function.terminator.instruction;
        if selected_return.id != machine_function.return_instruction.instruction
            || selected_return.id != pre_function.return_instruction.instruction
            || machine_function.return_instruction.alternative.key
                != pre_function.return_instruction.alternative
        {
            return Err(
                OptimizedResolvedSelectedFormLayoutError::StructuralReturnRosterMismatch(
                    selected_return.id,
                ),
            );
        }
        structural_unit_functions.push(layout_structural_unit_function(pre_function)?);
    }

    let selected_root = selected.selected_identity();
    let machine_root = machine.machine().receipt().identity();
    let pre_layout_root = pre_layout.identity();
    let target = selected_plan.target;
    let identity = layout_identity(
        selected_root,
        machine_root,
        pre_layout_root,
        normalized,
        target,
        policy,
        &functions,
        &structural_unit_functions,
    );
    let artifact = StagedOptimizedResolvedSelectedFormLayout {
        selected: selected_root,
        machine: machine_root,
        pre_layout: pre_layout_root,
        post_allocation_machine_optimization: normalized,
        target,
        policy,
        identity,
        functions,
        structural_unit_functions,
    };
    if let Some(custody) = normalized {
        let baseline_encoding =
            stage_optimized_layout_independent_selected_form_encoding(selected, machine, physical)
                .map_err(OptimizedResolvedSelectedFormLayoutError::PreLayout)?;
        let baseline = compute(selected, machine, physical, &baseline_encoding, None)?;
        validate_layout_byte_savings(&baseline, &artifact, custody)?;
    }
    Ok(artifact)
}
