use crate::optimized_semantic_wrapper_object::model::*;

pub(crate) fn custody(
    object: &OptimizedProgramStorageSemanticWrapperObjectPlan,
    container: &OptimizedProgramStorageSemanticWrapperObjectContainer,
    manifest: &OptimizedProgramStorageSemanticWrapperObjectManifest,
) -> OptimizedProgramStorageSemanticWrapperObjectCustodyReceipt {
    OptimizedProgramStorageSemanticWrapperObjectCustodyReceipt {
        source_artifact: object.source_artifact,
        source_signature: object.source_signature,
        object: object.identity,
        container: container.identity,
        manifest: manifest.identity,
    }
}
