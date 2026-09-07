use std::collections::BTreeMap;

use register_model::ValidatedPhysicalRegisterModel;
use selected_instructions_to_register_homes::ValidatedSelectedAnalysis;

use post_allocation_machine_to_selected_form_encoding::{
    StagedOptimizedSelectedFormEncoding,
    validate_optimized_layout_independent_selected_form_encoding,
};
use register_homes_to_post_allocation_machine::StagedOptimizedPostAllocationMachinePlan;

use super::error::OptimizedResolvedSelectedFormLayoutError;
use super::model::StagedOptimizedResolvedSelectedFormLayout;
use super::ordinary::{instructions, layout, select};
use machine_code::{ResolvedMachineLayout, resolved_machine_layout_identity as layout_identity};

pub(super) fn compute<S: ValidatedSelectedAnalysis>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    pre_layout: &StagedOptimizedSelectedFormEncoding,
) -> Result<StagedOptimizedResolvedSelectedFormLayout, OptimizedResolvedSelectedFormLayoutError> {
    validate_optimized_layout_independent_selected_form_encoding(
        selected,
        machine,
        physical,
        pre_layout.program().frame.as_ref(),
        pre_layout,
    )
    .map_err(OptimizedResolvedSelectedFormLayoutError::PreLayout)?;
    let selected_plan = selected.selected_plan();
    let machine_plan = machine.machine().plan();
    if pre_layout.selected() != selected.selected_identity()
        || pre_layout.machine() != machine.machine().receipt().identity()
        || selected_plan.target != machine_plan.target
        || selected_plan.target.architecture != physical.model().architecture
        || selected_plan.functions.len() != machine_plan.functions.len()
        || pre_layout.post_allocation_machine_optimization().is_some()
    {
        return Err(OptimizedResolvedSelectedFormLayoutError::RootMismatch);
    }

    let policy = select(selected_plan)?;
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
            policy,
        )?);
    }
    if pre_rows.next().is_some() {
        return Err(OptimizedResolvedSelectedFormLayoutError::RootMismatch);
    }

    let selected_root = selected.selected_identity();
    let machine_root = machine.machine().receipt().identity();
    let pre_layout_root = pre_layout.identity();
    let target = selected_plan.target;
    let identity = layout_identity(
        selected_root,
        machine_root,
        pre_layout_root,
        None,
        target,
        policy,
        &functions,
    );
    let artifact = StagedOptimizedResolvedSelectedFormLayout::from_program(ResolvedMachineLayout {
        selected: selected_root,
        machine: machine_root,
        pre_layout: pre_layout_root,
        post_allocation_machine_optimization: None,
        target,
        policy,
        identity,
        functions,
    });
    Ok(artifact)
}
