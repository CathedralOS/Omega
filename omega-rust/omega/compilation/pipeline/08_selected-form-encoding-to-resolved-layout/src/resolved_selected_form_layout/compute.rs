use std::collections::BTreeMap;

use register_model::ValidatedPhysicalRegisterModel;
use selected_instructions::CurrentSelectedProgram;

use machine_code::SelectedFormEncoding;
use physical_instructions::PostAllocationMachinePlan;

use super::StagedOptimizedResolvedSelectedFormLayout;
use super::error::OptimizedResolvedSelectedFormLayoutError as Error;
use super::ordinary::{instructions, layout, select};
use machine_code::{ResolvedMachineLayout, resolved_machine_layout_identity as layout_identity};

pub(super) fn compute(
    selected: CurrentSelectedProgram<'_>,
    machine: &PostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    pre_layout: &SelectedFormEncoding,
) -> Result<StagedOptimizedResolvedSelectedFormLayout, Error> {
    // The encoding was produced by stage 07 in this compile; it is not
    // re-encoded here. The root checks below bind it to these inputs.
    let selected_plan = selected.plan;
    let machine_plan = machine;
    if pre_layout.selected() != selected.identity
        || pre_layout.machine() != machine.identity
        || selected_plan.target != machine_plan.target
        || selected_plan.target.architecture != physical.model().architecture
        || selected_plan.functions.len() != machine_plan.functions.len()
        || pre_layout.post_allocation_machine_optimization().is_some()
    {
        return Err(Error::RootMismatch);
    }

    let policy = select(selected_plan)?;
    let mut pre_rows = pre_layout.rows().iter();
    let mut functions = Vec::with_capacity(selected_plan.functions.len());
    for (function, machine_function) in selected_plan.functions.iter().zip(&machine_plan.functions)
    {
        let mut function_pre_rows = BTreeMap::new();
        for block in &function.blocks {
            for instruction in instructions(block) {
                let row = pre_rows
                    .next()
                    .ok_or(Error::MissingInstruction(instruction.id))?;
                if row.instruction != instruction.id {
                    return Err(Error::MissingInstruction(instruction.id));
                }
                if function_pre_rows.insert(instruction.id, row).is_some() {
                    return Err(Error::DuplicateInstruction(instruction.id));
                }
            }
        }
        let machine_rows = machine_function
            .instructions()
            .map(|instruction| (instruction.instruction, instruction))
            .collect::<BTreeMap<_, _>>();
        functions.push(layout(
            selected_plan.target.architecture,
            function,
            &function_pre_rows,
            &machine_rows,
            physical,
            policy,
            pre_layout.encoded(),
        )?);
    }
    if pre_rows.next().is_some() {
        return Err(Error::RootMismatch);
    }

    let selected_root = selected.identity;
    let machine_root = machine.identity;
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
