//! Optimizer module role: executable entrance. Receiver-free Unit function-relative realization.
//!
//! This entrance owns construction-to-independent-replay admission. Unit
//! source-shape checks, manifest reconstruction, and custody projection remain
//! explicit lower rungs with no publication authority.

mod construction;
mod custody;
pub(in crate::function_realization) mod frame;
mod manifest;
mod model;
mod source;
mod validation;

pub use model::*;
pub use source::validate_unit_shape;
pub use validation::validate_optimized_unit_function_relative_realization;

use selected_instructions_to_register_homes::RetainedAllocation;

pub fn stage_optimized_unit_function_relative_realization(
    allocation: RetainedAllocation,
    machine: register_homes_to_post_allocation_machine::StagedOptimizedPostAllocationMachinePlan,
) -> Result<
    StagedOptimizedUnitFunctionRelativeRealization,
    OptimizedUnitFunctionRelativeRealizationError,
> {
    let staged = construction::construct_unit_function_relative_realization(allocation, machine)?;
    validate_optimized_unit_function_relative_realization(&staged)?;
    Ok(staged)
}
