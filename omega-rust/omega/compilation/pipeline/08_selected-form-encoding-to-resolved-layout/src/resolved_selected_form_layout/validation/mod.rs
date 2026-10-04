//! Optimizer module role: executable entrance. Independent admission of candidate function-relative layout.
//!
//! Policy, ordinary rows, structural rows, and aggregate identity descend
//! separately. Candidate branch bytes are accepted only by target decoders.

use register_model::ValidatedPhysicalRegisterModel;
use selected_instructions_to_register_homes::ValidatedSelectedAnalysis;

use post_allocation_machine_to_selected_form_encoding::StagedOptimizedSelectedFormEncoding;
use register_homes_to_post_allocation_machine::StagedOptimizedPostAllocationMachinePlan;

use super::{OptimizedResolvedSelectedFormLayoutError, StagedOptimizedResolvedSelectedFormLayout};

mod aggregate;
mod branch;
mod ordinary;
mod policy;
mod row;

pub(super) fn validate<S: ValidatedSelectedAnalysis>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    pre_layout: &StagedOptimizedSelectedFormEncoding,
    artifact: &StagedOptimizedResolvedSelectedFormLayout,
) -> Result<(), OptimizedResolvedSelectedFormLayoutError> {
    // The layout is checked against the encoding it was resolved from; the
    // encoding itself is replayed only by callers that admit a retained one.
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
    let expected_policy = policy::derive(selected_plan)?;
    aggregate::validate_roots(
        selected,
        machine,
        pre_layout,
        None,
        expected_policy,
        artifact,
    )?;
    ordinary::validate(
        selected,
        machine,
        physical,
        pre_layout,
        expected_policy,
        artifact,
    )?;
    aggregate::validate_identity(artifact)
}
