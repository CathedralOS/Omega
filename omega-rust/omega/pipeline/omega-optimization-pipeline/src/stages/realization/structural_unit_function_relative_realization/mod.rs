//! Optimizer module role: executable entrance. Structural-Unit function-relative realization stage.
//!
//! This entrance owns construction-to-independent-replay admission. Source
//! shape checks, manifest reconstruction, and custody projection descend into
//! named leaves; no section-placement or publication authority is granted.

mod construction;
mod custody;
mod manifest;
mod model;
mod source;
mod validation;

pub use model::*;
pub use validation::validate_optimized_structural_unit_function_relative_realization;

use omega_selected_instructions_to_register_homes::RetainedAllocation;

pub fn stage_optimized_structural_unit_function_relative_realization(
    allocation: RetainedAllocation,
) -> Result<
    StagedOptimizedStructuralUnitFunctionRelativeRealization,
    OptimizedStructuralUnitFunctionRelativeRealizationError,
> {
    let staged = construction::construct_structural_unit_function_relative_realization(allocation)?;
    validate_optimized_structural_unit_function_relative_realization(&staged)?;
    Ok(staged)
}
