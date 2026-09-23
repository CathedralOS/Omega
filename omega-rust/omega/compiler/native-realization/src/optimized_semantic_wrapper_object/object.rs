//! Optimizer module role: executable entrance.
//!
//! Binds the wrapper object's composition to the stage's retained custody:
//! the settlement, source artifact, and child object must name one another
//! before the record owner composes and seals the plan.

use super::error::OptimizedProgramStorageSemanticWrapperObjectError;
use crate::ValidatedNativeProgramEntrySettlement;
use native_artifact::{
    OptimizedProgramStorageSemanticWrapperObjectPlan,
    compose_optimized_program_storage_semantic_wrapper_object,
};
use object_file::StagedValidatedOptimizedObjectArtifact;
use program_entry_plan::StagedOptimizedProgramStorageSemanticWrapperEncoding;

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
    Ok(compose_optimized_program_storage_semantic_wrapper_object(
        settlement.source().identity().bytes(),
        source.artifact().identity,
        source.manifest().record().identity,
        child_stage.container().identity,
        child,
        encoding.template(),
    )?)
}
