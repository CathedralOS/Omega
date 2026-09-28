//! Canonical object/container/manifest fixture shared by mutation leaves.

use super::super::super::OptimizedProgramStorageSemanticWrapperObjectContainer;
use super::super::{
    OptimizedProgramStorageSemanticWrapperObjectManifest,
    OptimizedProgramStorageSemanticWrapperObjectPlan, composed, construct_manifest,
    encode_optimized_program_storage_semantic_wrapper_object, template,
};
pub(super) fn manifest_fixture() -> (
    OptimizedProgramStorageSemanticWrapperObjectPlan,
    OptimizedProgramStorageSemanticWrapperObjectContainer,
    OptimizedProgramStorageSemanticWrapperObjectManifest,
) {
    let object = composed();
    let container =
        encode_optimized_program_storage_semantic_wrapper_object(&object, &template()).unwrap();
    let manifest = construct_manifest(&object, &container).unwrap();
    (object, container, manifest)
}
