//! Optimizer module role: executable entrance. Target encoding of the semantic ProgramStorage wrapper plan.
//!
//! The address-free plan above names its own target obligation
//! (`TargetEncodingRequiredV1`). This entrance discharges it: it consumes the
//! validated plan by value, projects its canonical steps onto the x86-64 ISA
//! owner's wrapper request, lets that owner encode the template, and admits
//! the result only after replaying plan, projection, and template together.
//! The projection reads the same step grammar `recipe.rs` produces, so the
//! two change together here rather than across the realization coordinator.

mod projection;

use isa_x86_64::{
    ValidatedX86_64SemanticUnitWrapperTemplate, X86_64SemanticUnitWrapperEncodingError,
    X86_64SemanticUnitWrapperEncodingRequest, encode_x86_64_semantic_unit_wrapper_template,
    validate_x86_64_semantic_unit_wrapper_template,
};

use super::{
    OptimizedProgramStorageSemanticWrapperPlan, validate_optimized_program_storage_semantic_wrapper,
};
use projection::project_request;

/// The semantic plan, the target request projected from it, and the ISA
/// owner's validated template. The object stage composes the template and
/// replays the plan's entry contract; neither may drift from the others.
#[derive(Debug)]
#[must_use = "target wrapper encoding custody must be retained through continuation resolution"]
pub struct StagedOptimizedProgramStorageSemanticWrapperEncoding {
    source: OptimizedProgramStorageSemanticWrapperPlan,
    request: X86_64SemanticUnitWrapperEncodingRequest,
    template: ValidatedX86_64SemanticUnitWrapperTemplate,
}

impl StagedOptimizedProgramStorageSemanticWrapperEncoding {
    pub const fn source(&self) -> &OptimizedProgramStorageSemanticWrapperPlan {
        &self.source
    }

    pub const fn request(&self) -> X86_64SemanticUnitWrapperEncodingRequest {
        self.request
    }

    pub const fn template(&self) -> &ValidatedX86_64SemanticUnitWrapperTemplate {
        &self.template
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizedProgramStorageSemanticWrapperEncodingError {
    InvalidSemanticPlan,
    SemanticStepShapeMismatch,
    Target(X86_64SemanticUnitWrapperEncodingError),
    TemplateMismatch,
}

impl std::fmt::Display for OptimizedProgramStorageSemanticWrapperEncodingError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "optimized ProgramStorage semantic wrapper encoding failed: {self:?}"
        )
    }
}

impl std::error::Error for OptimizedProgramStorageSemanticWrapperEncodingError {}

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

/// Independently replay the retained plan, re-project its request, and
/// re-validate the retained template bytes against that request.
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

#[cfg(test)]
mod tests;
