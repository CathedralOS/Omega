//! Optimizer module role: executable entrance. Ordinary-function layout admission.
//!
//! This entrance owns the function roster traversal. Each function then
//! descends through source-row custody, canonical block order, size planning,
//! and exact row admission.

use register_model::ValidatedPhysicalRegisterModel;
use selected_instructions::CurrentSelectedProgram;

use machine_code::SelectedFormEncoding;
use machine_code::SelectedFormEncodingRow;
use physical_instructions::PostAllocationMachinePlan;

use super::super::OptimizedResolvedSelectedFormLayoutError as Error;
use super::super::StagedOptimizedResolvedSelectedFormLayout;

mod function;
mod order;
mod roster;

pub(super) fn validate(
    selected: CurrentSelectedProgram<'_>,
    machine: &PostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    pre_layout: &SelectedFormEncoding,
    policy: super::super::SelectedFunctionLayoutPolicy,
    artifact: &StagedOptimizedResolvedSelectedFormLayout,
) -> Result<(), Error> {
    let selected_plan = selected.plan;
    let machine_plan = machine;
    if artifact.functions().len() != selected_plan.functions.len() {
        return Err(Error::ArtifactMismatch);
    }
    let mut pre_rows = pre_layout.rows().iter();
    for ((selected_function, machine_function), candidate) in selected_plan
        .functions
        .iter()
        .zip(&machine_plan.functions)
        .zip(artifact.functions())
    {
        function::validate(
            selected_plan.target.architecture,
            selected_function,
            machine_function,
            physical,
            policy,
            &mut pre_rows,
            candidate,
            pre_layout.encoded(),
        )?;
    }
    if pre_rows.next().is_some() {
        return Err(Error::RootMismatch);
    }
    Ok(())
}

pub(super) type PreLayoutRows<'a> = std::slice::Iter<'a, SelectedFormEncodingRow>;
