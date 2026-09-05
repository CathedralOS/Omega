//! Optimizer module role: executable entrance. Independent admission of candidate function-relative layout.
//!
//! Policy, ordinary rows, structural rows, and aggregate identity descend
//! separately. Candidate branch bytes are accepted only by target decoders.

use register_model::ValidatedPhysicalRegisterModel;
use selected_instructions_to_register_homes::ValidatedSelectedAnalysis;

use crate::selected_form_encoding::{
    StagedOptimizedSelectedFormEncoding,
    validate_optimized_layout_independent_selected_form_encoding_with_post_allocation_machine_optimization,
};
use post_allocation_machine_to_post_allocation_machine::StagedOptimizedPostAllocationMachineOptimization;
use register_homes_to_post_allocation_machine::StagedOptimizedPostAllocationMachinePlan;

use super::{
    OptimizedResolvedSelectedFormLayoutError, StagedOptimizedResolvedSelectedFormLayout,
    optimization::validate_optimization_custody,
};

mod aggregate;
mod branch;
mod ordinary;
mod policy;
mod row;
mod structural;

pub(super) fn validate<S: ValidatedSelectedAnalysis>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    pre_layout: &StagedOptimizedSelectedFormEncoding,
    optimization: Option<&StagedOptimizedPostAllocationMachineOptimization>,
    artifact: &StagedOptimizedResolvedSelectedFormLayout,
) -> Result<(), OptimizedResolvedSelectedFormLayoutError> {
    validate_optimized_layout_independent_selected_form_encoding_with_post_allocation_machine_optimization(
        selected,
        machine,
        physical,
        optimization,
        pre_layout,
    )
    .map_err(OptimizedResolvedSelectedFormLayoutError::PreLayout)?;
    let normalized = validate_optimization_custody(machine, pre_layout, optimization)?;
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
    let expected_policy = policy::derive(selected_plan)?;
    aggregate::validate_roots(
        selected,
        machine,
        pre_layout,
        normalized,
        expected_policy,
        artifact,
    )?;
    ordinary::validate(
        selected,
        machine,
        physical,
        pre_layout,
        optimization,
        expected_policy,
        artifact,
    )?;
    structural::validate(selected, machine, pre_layout, artifact)?;
    aggregate::validate_identity(artifact)
}
