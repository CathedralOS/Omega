//! Optimizer module role: executable entrance.
mod compute;
mod frame_application;
pub use frame_application::*;
mod current;
mod custody;
mod error;
mod model;
mod replay;
mod source;
mod validation;

pub use error::{FunctionFragmentEmissionError, FunctionFragmentEmissionManifestDecodeError};
pub use model::{
    FunctionFragmentEmissionManifest, FunctionFragmentEmissionSourceKind,
    FunctionFragmentEmissionStage, FunctionFragmentEmissionStatistics,
    FunctionFragmentEmissionUnavailableData, StagedFunctionFragmentEmissionCustodyReceipt,
    StagedOptimizedFunctionFragmentEmission, ValidatedFunctionFragmentEmissionManifest,
};
#[cfg(any(test, feature = "test-support"))]
pub use replay::FunctionFragmentReplayInputs;
#[cfg(not(any(test, feature = "test-support")))]
pub(crate) use replay::FunctionFragmentReplayInputs;
pub use source::StagedOptimizedFunctionFragmentEmissionSource;

use compute::compute;
use custody::{receipt, validate_source};

/// Canonical join from one validated function-relative realization into
/// replayable function fragments and their v10 manifest custody.
pub fn stage_optimized_function_fragment_emission(
    source: StagedOptimizedFunctionFragmentEmissionSource,
) -> Result<StagedOptimizedFunctionFragmentEmission, FunctionFragmentEmissionError> {
    validate_source(&source)?;
    let (fragments, manifest) = compute(&source)?;
    let custody = receipt(&manifest, &fragments);
    let staged = StagedOptimizedFunctionFragmentEmission {
        source,
        fragments: std::sync::Arc::new(fragments),
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
    crate::validate_resolved_function_fragments(staged.source.program(), &staged.fragments)?;
    validation::manifest(staged)?;
    let expected = receipt(&staged.manifest, &staged.fragments);
    if staged.custody != expected {
        return Err(FunctionFragmentEmissionError::ReceiptMismatch);
    }
    Ok(expected)
}
