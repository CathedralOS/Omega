//! Optimized object-artifact stage entrance.
//!
//! This file owns the terminal/object build-and-replay join. Artifact and
//! manifest contracts live in `model`, semantic reconstruction in
//! `reconstruction`, and canonical bytes in `codec`.

use omega_optimization_core::{
    FunctionFragmentEmissionManifestIdentity, FunctionFragmentObjectContainerManifestIdentity,
    FunctionFragmentTextSectionManifestIdentity,
    FunctionRelativeOptimizationRealizationManifestIdentity, OptimizationSelectionIdentity,
    OptimizedObjectArtifactIdentity, OptimizedObjectArtifactManifestIdentity,
    PostAllocationOptimizationManifestIdentity, PrePhysicalOptimizationManifestIdentity,
    RelocationFreeObjectContainerIdentity, RelocationFreeObjectPlanIdentity,
};
use omega_target::{Architecture, NativeTarget, ObjectFormat};
use psi_core::MachineId;
use psi_terminal::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

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

pub use model::*;

use reconstruction::*;

pub fn stage_validated_optimized_object_artifact(
    terminal: psi_terminal_codec::CanonicalTerminalArtifact,
    source: StagedOptimizedRelocationFreeObjectContainer,
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
