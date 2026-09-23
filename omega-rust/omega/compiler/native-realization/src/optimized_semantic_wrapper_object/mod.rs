//! Optimizer module role: executable entrance. Owning object join for the optimized semantic ProgramStorage wrapper.
//!
//! The stage entrance composes one compiler-owned wrapper with one validated
//! relocation-free child, independently replays the join, and grants custody.
//! The wrapper object's durable records, their construction, shape checks,
//! and codec belong to their representation owner,
//! `native_artifact::semantic_wrapper_object`; this stage binds them to the
//! settlement, source artifact, and encoding custody it retains.

use crate::{
    StagedOptimizedProgramStorageSemanticWrapperEncoding, ValidatedNativeProgramEntrySettlement,
    validate_optimized_program_storage_semantic_wrapper_encoding,
};
use native_artifact::{
    OptimizedProgramStorageSemanticWrapperObjectContainer,
    OptimizedProgramStorageSemanticWrapperObjectCustodyReceipt,
    OptimizedProgramStorageSemanticWrapperObjectPlan,
    ValidatedOptimizedProgramStorageSemanticWrapperObjectManifest,
    encode_optimized_program_storage_semantic_wrapper_object_preserving_seal,
};
use object_file::{StagedValidatedOptimizedObjectArtifact, validate_optimized_object_artifact};

mod error;
mod object;
mod validation;

pub use error::{
    InstalledProgramStorageContinuationEvidenceError,
    OptimizedProgramStorageSemanticWrapperObjectError,
};
pub(crate) use validation::{bind_semantic_contract, receiver_layout};
pub use validation::{
    validate_installed_program_storage_continuation_evidence,
    validate_optimized_program_storage_semantic_wrapper_object,
};

use object::construct_object;
use validation::{
    replay_semantic_contract, replay_settlement, validate_entry_shape,
    validate_retained_installed_provider_continuation,
};

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
    let contract = replay_semantic_contract(&settlement, &encoding, &source)?;
    validate_entry_shape(&source, &settlement, &contract)?;
    let object = construct_object(&settlement, &source, &encoding)?;
    // `construct_object` sealed the plan one statement ago; the encode join
    // re-runs every shape and template check without reserializing the seal.
    let container = encode_optimized_program_storage_semantic_wrapper_object_preserving_seal(
        &object,
        encoding.template(),
    )?;
    let manifest = ValidatedOptimizedProgramStorageSemanticWrapperObjectManifest::construct(
        &object, &container,
    )?;
    let custody = OptimizedProgramStorageSemanticWrapperObjectCustodyReceipt::from_records(
        &object,
        &container,
        manifest.record(),
    );
    let staged = StagedValidatedOptimizedProgramStorageSemanticWrapperObject {
        settlement,
        source,
        encoding,
        object,
        container,
        manifest,
        custody,
    };
    validate_optimized_program_storage_semantic_wrapper_object(&staged)?;
    Ok(staged)
}

#[derive(Debug)]
#[must_use = "semantic-wrapper object custody retains settlement, encoding, and Omega object sources"]
pub struct StagedValidatedOptimizedProgramStorageSemanticWrapperObject {
    settlement: ValidatedNativeProgramEntrySettlement,
    source: StagedValidatedOptimizedObjectArtifact,
    encoding: StagedOptimizedProgramStorageSemanticWrapperEncoding,
    object: OptimizedProgramStorageSemanticWrapperObjectPlan,
    container: OptimizedProgramStorageSemanticWrapperObjectContainer,
    manifest: ValidatedOptimizedProgramStorageSemanticWrapperObjectManifest,
    custody: OptimizedProgramStorageSemanticWrapperObjectCustodyReceipt,
}

impl StagedValidatedOptimizedProgramStorageSemanticWrapperObject {
    pub const fn settlement(&self) -> &ValidatedNativeProgramEntrySettlement {
        &self.settlement
    }

    pub const fn source(&self) -> &StagedValidatedOptimizedObjectArtifact {
        &self.source
    }

    pub const fn encoding(&self) -> &StagedOptimizedProgramStorageSemanticWrapperEncoding {
        &self.encoding
    }

    pub const fn object(&self) -> &OptimizedProgramStorageSemanticWrapperObjectPlan {
        &self.object
    }

    pub const fn container(&self) -> &OptimizedProgramStorageSemanticWrapperObjectContainer {
        &self.container
    }

    pub const fn manifest(&self) -> &ValidatedOptimizedProgramStorageSemanticWrapperObjectManifest {
        &self.manifest
    }

    pub const fn custody(&self) -> OptimizedProgramStorageSemanticWrapperObjectCustodyReceipt {
        self.custody
    }
}

#[cfg(test)]
mod tests;
