//! Optimizer module role: executable entrance.
mod entry_contract;
mod provider_continuation;

pub(crate) use entry_contract::{
    replay_semantic_contract, replay_settlement, validate_entry_shape,
};
pub use provider_continuation::validate_installed_program_storage_continuation_evidence;
pub(crate) use provider_continuation::validate_retained_installed_provider_continuation;

use super::codec::{
    decode_optimized_program_storage_semantic_wrapper_object,
    encode_optimized_program_storage_semantic_wrapper_object,
};
use super::custody::custody;
use super::error::OptimizedProgramStorageSemanticWrapperObjectError;
use super::model::*;
use super::object::{construct_manifest, construct_object, validate_object};
use super::shared::*;

pub fn validate_optimized_program_storage_semantic_wrapper_object(
    staged: &StagedValidatedOptimizedProgramStorageSemanticWrapperObject,
) -> Result<
    OptimizedProgramStorageSemanticWrapperObjectCustodyReceipt,
    OptimizedProgramStorageSemanticWrapperObjectError,
> {
    replay_settlement(&staged.settlement, &staged.source)?;
    validate_optimized_object_artifact(&staged.source)
        .map_err(OptimizedProgramStorageSemanticWrapperObjectError::Source)?;
    validate_retained_installed_provider_continuation(&staged.source)?;
    validate_optimized_program_storage_semantic_wrapper_encoding(&staged.encoding)
        .map_err(OptimizedProgramStorageSemanticWrapperObjectError::Encoding)?;
    let contract = replay_semantic_contract(&staged.settlement, &staged.encoding)?;
    validate_entry_shape(&staged.source, &staged.settlement, &contract)?;
    let expected = construct_object(&staged.settlement, &staged.source, &staged.encoding)?;
    validate_object(&staged.object)?;
    if staged.object != expected {
        return Err(OptimizedProgramStorageSemanticWrapperObjectError::InvalidObject);
    }
    let decoded = decode_optimized_program_storage_semantic_wrapper_object(&staged.container.bytes)
        .map_err(|_| OptimizedProgramStorageSemanticWrapperObjectError::ContainerMismatch)?;
    let container = encode_optimized_program_storage_semantic_wrapper_object(&expected)?;
    if decoded != expected || staged.container != container {
        return Err(OptimizedProgramStorageSemanticWrapperObjectError::ContainerMismatch);
    }
    let manifest = construct_manifest(&expected, &container)?;
    if OptimizedProgramStorageSemanticWrapperObjectManifest::decode(
        &staged.manifest.record.encode(),
    )
    .map_err(|_| OptimizedProgramStorageSemanticWrapperObjectError::ManifestMismatch)?
        != staged.manifest.record
        || staged.manifest.record != manifest
    {
        return Err(OptimizedProgramStorageSemanticWrapperObjectError::ManifestMismatch);
    }
    let expected_custody = custody(&expected, &container, &manifest);
    if staged.custody != expected_custody {
        return Err(OptimizedProgramStorageSemanticWrapperObjectError::ReceiptMismatch);
    }
    Ok(expected_custody)
}
