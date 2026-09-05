//! Optimizer module role: executable entrance. Object publication and source custody.
//! Object-format construction and independent correspondence live in the backend.
pub use omega_object_file::{
    FunctionFragmentObjectContainerManifest, FunctionFragmentObjectContainerManifestDecodeError,
    FunctionFragmentObjectContainerStage, FunctionFragmentObjectContainerStatistics,
    FunctionFragmentObjectContainerUnavailableData,
};
use omega_object_file::{
    RelocationFreeObjectContainer, RelocationFreeObjectDecodeError, RelocationFreeObjectError,
    RelocationFreeObjectFromTextError, RelocationFreeObjectPlan,
    construct_relocation_free_object_from_text, encode_relocation_free_object,
};
use omega_optimization_core::{
    FunctionFragmentObjectContainerManifestIdentity, FunctionFragmentTextSectionManifestIdentity,
    RelocationFreeObjectContainerIdentity, RelocationFreeObjectPlanIdentity,
    TerminalRelocationFreeTextSectionIdentity,
};
mod model;
mod reconstruction;
mod source;
mod validation;

pub use model::*;
use reconstruction::*;
pub use source::*;
pub use validation::validate_optimized_relocation_free_object_container;

pub fn stage_optimized_relocation_free_object_container(
    source: impl Into<StagedOptimizedObjectTextSectionSource>,
) -> Result<StagedOptimizedRelocationFreeObjectContainer, RelocationFreeObjectContainerError> {
    let source = source.into();
    source.validate()?;
    let object = construct_relocation_free_object_from_text(
        source.text_section(),
        source.manifest().record().selections,
    )
    .map_err(object_error)?;
    let container = encode_relocation_free_object(&object)
        .map_err(RelocationFreeObjectContainerError::InvalidObject)?;
    let manifest = construct_manifest(&source, &object, &container)?;
    let custody = receipt(&manifest, &object, &container);
    let staged = StagedOptimizedRelocationFreeObjectContainer {
        source,
        object: std::sync::Arc::new(object),
        container: std::sync::Arc::new(container),
        manifest,
        custody,
    };
    validate_optimized_relocation_free_object_container(&staged)?;
    Ok(staged)
}

fn object_error(error: RelocationFreeObjectFromTextError) -> RelocationFreeObjectContainerError {
    match error {
        RelocationFreeObjectFromTextError::InvalidObject(error) => {
            RelocationFreeObjectContainerError::InvalidObject(error)
        }
        RelocationFreeObjectFromTextError::LengthOverflow => {
            RelocationFreeObjectContainerError::LengthOverflow
        }
        RelocationFreeObjectFromTextError::MissingSemanticEntry => {
            RelocationFreeObjectContainerError::MissingSemanticEntry
        }
        RelocationFreeObjectFromTextError::SourceMismatch => {
            RelocationFreeObjectContainerError::ArtifactMismatch
        }
    }
}
