//! Optimizer module role: executable entrance. Active-resident rematerialization function-relative realization.
//!
//! This entrance owns construction-to-independent-replay admission. Source
//! artifact projection, manifest reconstruction, and custody remain explicit
//! lower rungs with no publication authority.

mod construction;
mod custody;
mod manifest;
mod model;
mod source;
mod validation;

#[cfg(test)]
mod test_support;

pub use model::*;
pub use validation::validate_optimized_active_resident_rematerialization_function_relative_realization;

#[cfg(test)]
pub(crate) use test_support::*;

use crate::StagedOptimizedActiveResidentRematerializationResolvedSelectedFormLayout;

pub fn stage_optimized_active_resident_rematerialization_function_relative_realization(
    source: StagedOptimizedActiveResidentRematerializationResolvedSelectedFormLayout,
) -> Result<
    StagedOptimizedActiveResidentRematerializationFunctionRelativeRealization,
    OptimizedActiveResidentRematerializationFunctionRelativeRealizationError,
> {
    let staged = construction::construct_active_resident_function_relative_realization(source)?;
    validate_optimized_active_resident_rematerialization_function_relative_realization(&staged)?;
    Ok(staged)
}
