//! Optimizer module role: executable entrance.

use super::StagedValidatedOptimizedProgramStorageSemanticWrapperObject;
use native_artifact::{
    OptimizedProgramStorageSemanticWrapperObjectCustodyReceipt,
    decode_optimized_program_storage_semantic_wrapper_object,
    encode_optimized_program_storage_semantic_wrapper_object_preserving_seal,
};
use object_file::validate_optimized_object_artifact;
use program_entry_plan::validate_optimized_program_storage_semantic_wrapper_encoding;
mod entry_contract;
mod provider_continuation;

pub(crate) use entry_contract::{
    bind_semantic_contract, receiver_layout, replay_semantic_contract, replay_settlement,
    validate_entry_shape,
};
pub use provider_continuation::validate_installed_program_storage_continuation_evidence;
pub(crate) use provider_continuation::validate_retained_installed_provider_continuation;

use super::error::OptimizedProgramStorageSemanticWrapperObjectError;
use super::object::construct_object;

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
    let contract = replay_semantic_contract(&staged.settlement, &staged.encoding, &staged.source)?;
    validate_entry_shape(&staged.source, &staged.settlement, &contract)?;
    let expected = construct_object(&staged.settlement, &staged.source, &staged.encoding)?;
    // `staged.object` reached custody already sealed by `construct_object`;
    // the digest conjunct cannot differ on the unchanged in-memory value, and
    // the `!= expected` join below rejects any stale seal against the freshly
    // recomposed plan with the same `InvalidObject` — so the shape and
    // template checks run without reserializing the identity.
    staged
        .object
        .validate_preserving_seal(staged.encoding.template())?;
    if staged.object != expected {
        return Err(OptimizedProgramStorageSemanticWrapperObjectError::InvalidObject);
    }
    // The container decode keeps the full wire-boundary identity check; the
    // re-encode of the just-sealed replayed plan skips only that conjunct.
    let decoded = decode_optimized_program_storage_semantic_wrapper_object(&staged.container.bytes)
        .map_err(|_| OptimizedProgramStorageSemanticWrapperObjectError::ContainerMismatch)?;
    let container = encode_optimized_program_storage_semantic_wrapper_object_preserving_seal(
        &expected,
        staged.encoding.template(),
    )?;
    if decoded != expected || staged.container != container {
        return Err(OptimizedProgramStorageSemanticWrapperObjectError::ContainerMismatch);
    }
    staged.manifest.replay(&expected, &container)?;
    let expected_custody = OptimizedProgramStorageSemanticWrapperObjectCustodyReceipt::from_records(
        &expected,
        &container,
        staged.manifest.record(),
    );
    if staged.custody != expected_custody {
        return Err(OptimizedProgramStorageSemanticWrapperObjectError::ReceiptMismatch);
    }
    Ok(expected_custody)
}
