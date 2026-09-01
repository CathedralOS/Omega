use crate::optimized_semantic_wrapper_object::error::*;
use crate::optimized_semantic_wrapper_object::model::*;
use crate::optimized_semantic_wrapper_object::shared::*;

pub(crate) fn construct_manifest(
    object: &OptimizedProgramStorageSemanticWrapperObjectPlan,
    container: &OptimizedProgramStorageSemanticWrapperObjectContainer,
) -> Result<
    OptimizedProgramStorageSemanticWrapperObjectManifest,
    OptimizedProgramStorageSemanticWrapperObjectError,
> {
    let unavailable = OptimizedProgramStorageSemanticWrapperObjectUnavailableData::Unavailable;
    let mut manifest = OptimizedProgramStorageSemanticWrapperObjectManifest {
        identity:
            OptimizedProgramStorageSemanticWrapperObjectManifestIdentity::from_canonical_bytes(
                b"pending",
            ),
        stage:
            OptimizedProgramStorageSemanticWrapperObjectStage::ValidatedResolvedCompositeObjectV1,
        object: object.identity,
        container: container.identity,
        source_artifact: object.source_artifact,
        source_artifact_manifest: object.source_artifact_manifest,
        source_object: object.source_object,
        source_object_container: object.source_object_container,
        source_signature: object.source_signature,
        psi: object.psi,
        target: object.target,
        wrapper_symbol: object.wrapper_symbol,
        continuation_symbol: object.continuation_symbol,
        text_byte_count: u64::try_from(object.text_bytes.len())
            .map_err(|_| OptimizedProgramStorageSemanticWrapperObjectError::LengthOverflow)?,
        symbol_count: u64::try_from(object.symbols.len())
            .map_err(|_| OptimizedProgramStorageSemanticWrapperObjectError::LengthOverflow)?,
        relocation_record_count: object.relocation_record_count,
        physical_entry_bridge: unavailable,
        executable_image: unavailable,
        installation: unavailable,
        publication: unavailable,
    };
    manifest.identity = manifest.recomputed_identity();
    if !valid_manifest_shape(&manifest) {
        return Err(OptimizedProgramStorageSemanticWrapperObjectError::ManifestMismatch);
    }
    Ok(manifest)
}

pub(crate) fn validate_manifest(
    object: &OptimizedProgramStorageSemanticWrapperObjectPlan,
    container: &OptimizedProgramStorageSemanticWrapperObjectContainer,
    manifest: &OptimizedProgramStorageSemanticWrapperObjectManifest,
) -> Result<(), OptimizedProgramStorageSemanticWrapperObjectError> {
    let decoded = OptimizedProgramStorageSemanticWrapperObjectManifest::decode(&manifest.encode())
        .map_err(|_| OptimizedProgramStorageSemanticWrapperObjectError::ManifestMismatch)?;
    let expected = construct_manifest(object, container)?;
    if decoded != *manifest || *manifest != expected {
        return Err(OptimizedProgramStorageSemanticWrapperObjectError::ManifestMismatch);
    }
    Ok(())
}

pub(crate) fn valid_manifest_shape(
    manifest: &OptimizedProgramStorageSemanticWrapperObjectManifest,
) -> bool {
    manifest.stage
        == OptimizedProgramStorageSemanticWrapperObjectStage::ValidatedResolvedCompositeObjectV1
        && manifest.target == NativeTarget::uefi_x64()
        && manifest.wrapper_symbol != manifest.continuation_symbol
        && manifest.text_byte_count > X86_64_SEMANTIC_UNIT_WRAPPER_FUNCTION_BYTE_COUNT as u64
        && manifest.symbol_count >= 2
        && manifest.relocation_record_count == 0
        && manifest.physical_entry_bridge
            == OptimizedProgramStorageSemanticWrapperObjectUnavailableData::Unavailable
        && manifest.executable_image
            == OptimizedProgramStorageSemanticWrapperObjectUnavailableData::Unavailable
        && manifest.installation
            == OptimizedProgramStorageSemanticWrapperObjectUnavailableData::Unavailable
        && manifest.publication
            == OptimizedProgramStorageSemanticWrapperObjectUnavailableData::Unavailable
}
