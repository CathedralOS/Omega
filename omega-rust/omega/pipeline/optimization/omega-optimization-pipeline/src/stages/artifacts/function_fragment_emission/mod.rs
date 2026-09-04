//! Optimizer module role: executable entrance.
mod compute;
mod custody;
mod error;
mod manifest;
mod model;
mod source;

pub use error::{FunctionFragmentEmissionError, FunctionFragmentEmissionManifestDecodeError};
pub use model::{
    FunctionFragmentEmissionManifest, FunctionFragmentEmissionSourceKind,
    FunctionFragmentEmissionStage, FunctionFragmentEmissionStatistics,
    FunctionFragmentEmissionUnavailableData, StagedFunctionFragmentEmissionCustodyReceipt,
    StagedOptimizedFunctionFragmentEmission, ValidatedFunctionFragmentEmissionManifest,
};
pub use source::StagedOptimizedFunctionFragmentEmissionSource;

use compute::compute;
use custody::{receipt, validate_source};

/// Canonical join from one validated function-relative realization into
/// replayable function fragments and their v9 manifest custody.
pub fn stage_optimized_function_fragment_emission(
    source: StagedOptimizedFunctionFragmentEmissionSource,
) -> Result<StagedOptimizedFunctionFragmentEmission, FunctionFragmentEmissionError> {
    validate_source(&source)?;
    let (fragments, manifest) = compute(&source)?;
    let custody = receipt(&manifest, &fragments);
    let staged = StagedOptimizedFunctionFragmentEmission {
        source,
        fragments,
        manifest,
        custody,
    };
    validate_optimized_function_fragment_emission(&staged)?;
    Ok(staged)
}

pub fn validate_optimized_function_fragment_emission(
    staged: &StagedOptimizedFunctionFragmentEmission,
) -> Result<StagedFunctionFragmentEmissionCustodyReceipt, FunctionFragmentEmissionError> {
    validate_source(&staged.source)?;
    let (expected_fragments, expected_manifest) = compute(&staged.source)?;
    if staged.fragments.recomputed_identity() != staged.fragments.identity
        || staged.fragments != expected_fragments
    {
        return Err(FunctionFragmentEmissionError::ArtifactMismatch);
    }
    if staged.manifest != expected_manifest {
        return Err(FunctionFragmentEmissionError::ManifestMismatch);
    }
    let expected = receipt(&expected_manifest, &expected_fragments);
    if staged.custody != expected {
        return Err(FunctionFragmentEmissionError::ReceiptMismatch);
    }
    Ok(expected)
}
