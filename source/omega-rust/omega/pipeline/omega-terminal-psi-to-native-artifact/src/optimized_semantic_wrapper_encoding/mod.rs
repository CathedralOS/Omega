//! Selection and replay of the target-owned semantic ProgramStorage wrapper.
//!
//! This entrance consumes the address-free semantic plan by value, projects it
//! into one target request, encodes it, and admits it only after replay.

mod error;
mod model;
mod projection;
mod validation;

pub use error::OptimizedProgramStorageSemanticWrapperEncodingError;
pub use model::StagedOptimizedProgramStorageSemanticWrapperEncoding;
pub use validation::validate_optimized_program_storage_semantic_wrapper_encoding;

use omega_isa_x86_64::encode_x86_64_semantic_unit_wrapper_template;
use omega_program_entry_plan::{
    OptimizedProgramStorageSemanticWrapperPlan, validate_optimized_program_storage_semantic_wrapper,
};
use projection::project_request;

pub fn select_optimized_program_storage_semantic_wrapper_encoding(
    source: OptimizedProgramStorageSemanticWrapperPlan,
) -> Result<
    StagedOptimizedProgramStorageSemanticWrapperEncoding,
    OptimizedProgramStorageSemanticWrapperEncodingError,
> {
    validate_optimized_program_storage_semantic_wrapper(&source)
        .map_err(|_| OptimizedProgramStorageSemanticWrapperEncodingError::InvalidSemanticPlan)?;
    let request = project_request(&source)?;
    let template = encode_x86_64_semantic_unit_wrapper_template(request)
        .map_err(OptimizedProgramStorageSemanticWrapperEncodingError::Target)?;
    let staged = StagedOptimizedProgramStorageSemanticWrapperEncoding {
        source,
        request,
        template,
    };
    validate_optimized_program_storage_semantic_wrapper_encoding(&staged)?;
    Ok(staged)
}

#[cfg(test)]
mod tests;
