use super::error::OptimizedProgramStorageSemanticWrapperEncodingError;
use super::model::StagedOptimizedProgramStorageSemanticWrapperEncoding;
use super::projection::project_request;
use omega_isa_x86_64::validate_x86_64_semantic_unit_wrapper_template;
use omega_program_entry_plan::validate_optimized_program_storage_semantic_wrapper;

pub fn validate_optimized_program_storage_semantic_wrapper_encoding(
    staged: &StagedOptimizedProgramStorageSemanticWrapperEncoding,
) -> Result<(), OptimizedProgramStorageSemanticWrapperEncodingError> {
    validate_optimized_program_storage_semantic_wrapper(&staged.source)
        .map_err(|_| OptimizedProgramStorageSemanticWrapperEncodingError::InvalidSemanticPlan)?;
    let expected_request = project_request(&staged.source)?;
    if staged.request != expected_request {
        return Err(OptimizedProgramStorageSemanticWrapperEncodingError::TemplateMismatch);
    }
    let expected =
        validate_x86_64_semantic_unit_wrapper_template(expected_request, staged.template.bytes())
            .map_err(OptimizedProgramStorageSemanticWrapperEncodingError::Target)?;
    if staged.template != expected {
        return Err(OptimizedProgramStorageSemanticWrapperEncodingError::TemplateMismatch);
    }
    Ok(())
}
