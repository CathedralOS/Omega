//! Canonical object/container/manifest fixture shared by mutation leaves.

use super::super::*;

pub(super) fn manifest_fixture() -> (
    OptimizedProgramStorageSemanticWrapperObjectPlan,
    OptimizedProgramStorageSemanticWrapperObjectContainer,
    OptimizedProgramStorageSemanticWrapperObjectManifest,
) {
    let object = composed();
    let container = encode_optimized_program_storage_semantic_wrapper_object(&object).unwrap();
    let manifest = construct_manifest(&object, &container).unwrap();
    (object, container, manifest)
}
