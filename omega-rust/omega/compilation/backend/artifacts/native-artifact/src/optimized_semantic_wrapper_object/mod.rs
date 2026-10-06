//! Optimizer module role: executable entrance. Owning object join for the optimized semantic ProgramStorage wrapper.
//!
//! The stage entrance composes one compiler-owned wrapper with one
//! relocation-free child staged in this compile and grants custody. It does
//! not replay the settlement, encoding or child its producers just built;
//! `validate_optimized_program_storage_semantic_wrapper_object` replays the
//! whole join for retained readers.
//! The wrapper object's durable records, their construction, shape checks,
//! and codec belong to their representation owner,
//! `crate::semantic_wrapper_object`; this stage binds them to the
//! settlement, source artifact, and encoding custody it retains.

use crate::{
    OptimizedProgramStorageSemanticWrapperObjectContainer,
    OptimizedProgramStorageSemanticWrapperObjectCustodyReceipt,
    OptimizedProgramStorageSemanticWrapperObjectPlan,
    ValidatedOptimizedProgramStorageSemanticWrapperObjectManifest,
};
use crate::semantic_wrapper_object::encode_sealed_optimized_program_storage_semantic_wrapper_object;
use object_file::StagedValidatedOptimizedObjectArtifact;
use program_entry_plan::StagedOptimizedProgramStorageSemanticWrapperEncoding;
use terminal_psi_to_abstract_operations::ValidatedNativeProgramEntrySettlement;

mod error;
mod object;
mod validation;

pub use error::{
    InstalledProgramStorageContinuationEvidenceError,
    OptimizedProgramStorageSemanticWrapperObjectError,
};
pub use validation::{bind_semantic_contract, receiver_layout};
pub use validation::{
    validate_installed_program_storage_continuation_evidence,
    };

use object::construct_object;
use validation::validate_entry_shape;

pub fn stage_validated_optimized_program_storage_semantic_wrapper_object(
    settlement: ValidatedNativeProgramEntrySettlement,
    source: StagedValidatedOptimizedObjectArtifact,
    encoding: StagedOptimizedProgramStorageSemanticWrapperEncoding,
) -> Result<
    StagedValidatedOptimizedProgramStorageSemanticWrapperObject,
    OptimizedProgramStorageSemanticWrapperObjectError,
> {
    // The receiver must be zero-valid so the wrapper's zero-fill establishes
    // it: an admission rule of this composition, not a replay of an input.
    let contract = validation::bind_semantic_contract(&settlement)?;
    validate_entry_shape(&source, &settlement, &contract)?;
    let object = construct_object(&settlement, &source, &encoding)?;
    let container = encode_sealed_optimized_program_storage_semantic_wrapper_object(&object)?;
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
