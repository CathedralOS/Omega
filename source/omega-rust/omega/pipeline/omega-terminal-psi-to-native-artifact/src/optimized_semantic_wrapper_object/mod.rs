//! Optimizer module role: executable entrance. Owning object join for the optimized semantic ProgramStorage wrapper.
//!
//! The stage entrance composes one compiler-owned wrapper with one validated
//! relocation-free child, independently replays the join, and grants custody.

mod codec;
mod custody;
mod error;
mod model;
mod object;
mod shared;
mod validation;

pub use codec::{
    decode_optimized_program_storage_semantic_wrapper_object,
    encode_optimized_program_storage_semantic_wrapper_object,
};
pub use error::{
    InstalledProgramStorageContinuationEvidenceError,
    OptimizedProgramStorageSemanticWrapperObjectDecodeError,
    OptimizedProgramStorageSemanticWrapperObjectError,
};
pub use model::*;
pub use validation::{
    validate_installed_program_storage_continuation_evidence,
    validate_optimized_program_storage_semantic_wrapper_object,
};

use custody::custody;
use object::{construct_manifest, construct_object};
use shared::*;
use validation::{
    replay_semantic_contract, replay_settlement, validate_entry_shape,
    validate_retained_installed_provider_continuation,
};

#[cfg(test)]
use object::{compose_object, validate_object};

pub fn stage_validated_optimized_program_storage_semantic_wrapper_object(
    settlement: ValidatedNativeProgramEntrySettlement,
    source: StagedValidatedOptimizedObjectArtifact,
    encoding: StagedOptimizedProgramStorageSemanticWrapperEncoding,
) -> Result<
    StagedValidatedOptimizedProgramStorageSemanticWrapperObject,
    OptimizedProgramStorageSemanticWrapperObjectError,
> {
    replay_settlement(&settlement, &source)?;
    validate_optimized_object_artifact(&source)
        .map_err(OptimizedProgramStorageSemanticWrapperObjectError::Source)?;
    validate_retained_installed_provider_continuation(&source)?;
    validate_optimized_program_storage_semantic_wrapper_encoding(&encoding)
        .map_err(OptimizedProgramStorageSemanticWrapperObjectError::Encoding)?;
    let contract = replay_semantic_contract(&settlement, &encoding)?;
    validate_entry_shape(&source, &settlement, &contract)?;
    let object = construct_object(&settlement, &source, &encoding)?;
    let container = encode_optimized_program_storage_semantic_wrapper_object(&object)?;
    let manifest = construct_manifest(&object, &container)?;
    let custody = custody(&object, &container, &manifest);
    let staged = StagedValidatedOptimizedProgramStorageSemanticWrapperObject {
        settlement,
        source,
        encoding,
        object,
        container,
        manifest: ValidatedOptimizedProgramStorageSemanticWrapperObjectManifest {
            record: manifest,
        },
        custody,
    };
    validate_optimized_program_storage_semantic_wrapper_object(&staged)?;
    Ok(staged)
}

#[cfg(test)]
mod tests;
