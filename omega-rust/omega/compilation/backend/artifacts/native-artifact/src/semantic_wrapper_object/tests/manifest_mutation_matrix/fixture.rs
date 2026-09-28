//! Canonical object/container/manifest fixture shared by mutation leaves.

use super::super::super::OptimizedProgramStorageSemanticWrapperObjectContainer;
use super::super::{
    OptimizedProgramStorageSemanticWrapperObjectManifest,
    OptimizedProgramStorageSemanticWrapperObjectPlan, child,
    compose_optimized_program_storage_semantic_wrapper_object, composed, construct_manifest,
    encode_optimized_program_storage_semantic_wrapper_object, template,
};
use optimization_core::{
    OptimizedObjectArtifactIdentity, OptimizedObjectArtifactManifestIdentity,
    RelocationFreeObjectContainerIdentity,
};

pub(super) fn manifest_fixture() -> (
    OptimizedProgramStorageSemanticWrapperObjectPlan,
    OptimizedProgramStorageSemanticWrapperObjectContainer,
    OptimizedProgramStorageSemanticWrapperObjectManifest,
) {
    manifest_parts(composed())
}

/// An authentic manifest of a foreign wrapper object: the same child and
/// template composed under a different source artifact, artifact manifest,
/// child container and signature. The mutation matrix uses it as the donor
/// that proves a distinct manifest is real producer output, not a fabricated
/// record.
pub(super) fn foreign_manifest() -> OptimizedProgramStorageSemanticWrapperObjectManifest {
    let object = compose_optimized_program_storage_semantic_wrapper_object(
        [6; 32],
        OptimizedObjectArtifactIdentity::from_canonical_bytes(b"foreign-artifact"),
        OptimizedObjectArtifactManifestIdentity::from_canonical_bytes(b"foreign-manifest"),
        RelocationFreeObjectContainerIdentity::from_canonical_bytes(b"foreign-container"),
        &child(),
        &template(),
    )
    .unwrap();
    manifest_parts(object).2
}

fn manifest_parts(
    object: OptimizedProgramStorageSemanticWrapperObjectPlan,
) -> (
    OptimizedProgramStorageSemanticWrapperObjectPlan,
    OptimizedProgramStorageSemanticWrapperObjectContainer,
    OptimizedProgramStorageSemanticWrapperObjectManifest,
) {
    let container =
        encode_optimized_program_storage_semantic_wrapper_object(&object, &template()).unwrap();
    let manifest = construct_manifest(&object, &container).unwrap();
    (object, container, manifest)
}
