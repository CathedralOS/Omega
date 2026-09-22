//! Optimizer module role: executable entrance. Optimized object-artifact stage entrance.
//!
//! This file owns the terminal/object build-and-replay join. Artifact and
//! manifest contracts live in `model`, semantic reconstruction in
//! `reconstruction`, and canonical bytes in `codec`.

use crate::artifact_custody::reconstruction::{
    construct_artifact, construct_manifest, receipt, replay_artifact, validate_terminal_join,
};
use optimization_core::{
    FunctionFragmentEmissionManifestIdentity, FunctionFragmentObjectContainerManifestIdentity,
    FunctionFragmentTextSectionManifestIdentity,
    FunctionRelativeOptimizationRealizationManifestIdentity, OptimizationSelectionIdentity,
    OptimizedObjectArtifactIdentity, OptimizedObjectArtifactManifestIdentity,
    PostAllocationOptimizationManifestIdentity, PrePhysicalOptimizationManifestIdentity,
    RelocationFreeObjectContainerIdentity, RelocationFreeObjectPlanIdentity,
};
use semantic_vocabulary::MachineId;
use target::{Architecture, NativeTarget, ObjectFormat};
use terminal_psi::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

use crate::{
    FunctionFragmentObjectContainerManifest, RelocationFreeObjectContainerError,
    StagedOptimizedRelocationFreeObjectContainer,
    validate_optimized_relocation_free_object_container,
};

const ARTIFACT_MAGIC: &[u8; 8] = b"OMGOTA\0\0";
const ARTIFACT_VERSION: u32 = 1;
const MANIFEST_MAGIC: &[u8; 8] = b"OMGOTM\0\0";
const MANIFEST_VERSION: u32 = 1;

mod codec;
mod model;
mod reconstruction;

pub use model::{
    OptimizedObjectArtifactCustodyReceipt, OptimizedObjectArtifactError,
    OptimizedObjectArtifactManifest, OptimizedObjectArtifactManifestDecodeError,
    OptimizedObjectArtifactRecord, OptimizedObjectArtifactRecordDecodeError,
    OptimizedObjectArtifactStage, OptimizedObjectArtifactStatistics,
    OptimizedObjectArtifactUnavailableData, StagedValidatedOptimizedObjectArtifact,
    ValidatedOptimizedObjectArtifactManifest,
};

pub fn stage_validated_optimized_object_artifact(
    terminal: terminal_codec::CanonicalTerminalArtifact,
    source: StagedOptimizedRelocationFreeObjectContainer,
) -> Result<StagedValidatedOptimizedObjectArtifact, OptimizedObjectArtifactError> {
    stage_validated_optimized_object_artifact_shared(terminal, std::sync::Arc::new(source))
}

/// The same staging over shared container custody: callers that must keep the
/// relocation-free container available to another retained artifact — the
/// emitted fragment object and the semantic-entry object are one content —
/// hold a shared stage rather than a moved one. Validation is identical.
pub fn stage_validated_optimized_object_artifact_shared(
    terminal: terminal_codec::CanonicalTerminalArtifact,
    source: std::sync::Arc<StagedOptimizedRelocationFreeObjectContainer>,
) -> Result<StagedValidatedOptimizedObjectArtifact, OptimizedObjectArtifactError> {
    validate_optimized_relocation_free_object_container(&source)
        .map_err(OptimizedObjectArtifactError::Source)?;
    validate_terminal_join(&terminal, &source)?;
    let artifact = construct_artifact(&terminal, &source)?;
    let manifest = construct_manifest(&artifact);
    let custody = receipt(&artifact, &manifest);
    let staged = StagedValidatedOptimizedObjectArtifact {
        terminal,
        source,
        artifact,
        manifest,
        custody,
    };
    validate_optimized_object_artifact(&staged)?;
    Ok(staged)
}

pub fn validate_optimized_object_artifact(
    staged: &StagedValidatedOptimizedObjectArtifact,
) -> Result<OptimizedObjectArtifactCustodyReceipt, OptimizedObjectArtifactError> {
    validate_optimized_relocation_free_object_container(&staged.source)
        .map_err(OptimizedObjectArtifactError::Source)?;
    validate_terminal_join(&staged.terminal, &staged.source)?;
    let expected_artifact = replay_artifact(&staged.terminal, &staged.source)?;
    if OptimizedObjectArtifactRecord::decode(&staged.artifact.encode())
        .map_err(|_| OptimizedObjectArtifactError::ArtifactMismatch)?
        != staged.artifact
        || staged.artifact != expected_artifact
    {
        return Err(OptimizedObjectArtifactError::ArtifactMismatch);
    }
    let expected_manifest = construct_manifest(&expected_artifact);
    if OptimizedObjectArtifactManifest::decode(&staged.manifest.record.encode())
        .map_err(|_| OptimizedObjectArtifactError::ManifestMismatch)?
        != staged.manifest.record
        || staged.manifest != expected_manifest
    {
        return Err(OptimizedObjectArtifactError::ManifestMismatch);
    }
    let expected_receipt = receipt(&expected_artifact, &expected_manifest);
    if staged.custody != expected_receipt {
        return Err(OptimizedObjectArtifactError::ReceiptMismatch);
    }
    Ok(expected_receipt)
}
