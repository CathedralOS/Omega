//! Optimizer module role: executable entrance.
mod composition;
mod manifest;
mod validation;

pub(crate) use composition::compose_object;
pub(crate) use manifest::{construct_manifest, valid_manifest_shape, validate_manifest};
pub(crate) use validation::validate_object;

use crate::optimized_semantic_wrapper_object::error::*;
use crate::optimized_semantic_wrapper_object::model::*;
use crate::optimized_semantic_wrapper_object::shared::*;

pub(crate) fn construct_object(
    settlement: &ValidatedNativeProgramEntrySettlement,
    source: &StagedValidatedOptimizedObjectArtifact,
    encoding: &StagedOptimizedProgramStorageSemanticWrapperEncoding,
) -> Result<
    OptimizedProgramStorageSemanticWrapperObjectPlan,
    OptimizedProgramStorageSemanticWrapperObjectError,
> {
    if settlement.target() != source.artifact().target
        || source.artifact().semantic_entry != settlement.checked_entry().terminal_entry()
    {
        return Err(OptimizedProgramStorageSemanticWrapperObjectError::TargetMismatch);
    }
    let child_stage = source.source();
    let child = child_stage.object();
    if child.identity != source.artifact().object
        || child_stage.container().identity != source.artifact().object_container
    {
        return Err(OptimizedProgramStorageSemanticWrapperObjectError::SourceObjectMismatch);
    }
    compose_object(
        settlement.source().identity().bytes(),
        source.artifact().identity,
        source.manifest().record().identity,
        child_stage.container().identity,
        child,
        encoding,
    )
}
