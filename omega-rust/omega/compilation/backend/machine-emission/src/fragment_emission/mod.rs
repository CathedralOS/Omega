//! Fragment emission: the second machine-emission stage.
//!
//! [`stage_optimized_function_fragment_emission`] projects one
//! function-relative realization's machine layout into unplaced function
//! fragments and seals their manifest (`compute`).
//! [`validate_optimized_function_fragment_emission`] replays the join — the
//! realization's custody (`custody`), the independent fragment checker, then
//! the manifest fields (`validation`) — for retained-artifact readers and
//! tests; the stage does not replay what it just produced.
//!
//! `source` is the stage input: the current program (`current`) beside the
//! producer-stage objects only replay reads (`replay`). `projection` owns the
//! byte projection and its checker over current data; this level owns source
//! admission and custody.

mod compute;
mod current;
mod custody;
mod error;
pub(crate) mod projection;
mod replay;
mod source;
mod validation;

pub(crate) use error::FunctionFragmentEmissionError;
pub use error::FunctionFragmentEmissionManifestDecodeError;
pub use source::StagedOptimizedFunctionFragmentEmissionSource;

use crate::function_realization::ValidatedFunctionRelativeOptimizationRealizationManifest;
use compute::compute;
use custody::{receipt, validate_source};
use machine_code::FunctionFragmentEmissionPlan;
pub use machine_code::{
    FunctionFragmentEmissionManifest, FunctionFragmentEmissionStage,
    FunctionFragmentEmissionStatistics, FunctionFragmentEmissionUnavailableData,
};
use optimization_core::{
    FunctionFragmentEmissionIdentity, FunctionFragmentEmissionManifestIdentity,
    FunctionRelativeOptimizationRealizationManifestIdentity,
};
use std::sync::Arc;

/// Canonical join from one validated function-relative realization into
/// replayable function fragments and their v10 manifest custody.
pub fn stage_optimized_function_fragment_emission(
    source: StagedOptimizedFunctionFragmentEmissionSource,
) -> Result<StagedOptimizedFunctionFragmentEmission, FunctionFragmentEmissionError> {
    let (fragments, manifest) = compute(&source)?;
    let custody = receipt(&manifest, &fragments);
    Ok(StagedOptimizedFunctionFragmentEmission {
        source,
        fragments: std::sync::Arc::new(fragments),
        manifest,
        custody,
    })
}

pub(crate) fn validate_optimized_function_fragment_emission(
    staged: &StagedOptimizedFunctionFragmentEmission,
) -> Result<StagedFunctionFragmentEmissionCustodyReceipt, FunctionFragmentEmissionError> {
    validate_source(&staged.source)?;
    projection::validate_resolved_function_fragments(staged.source.program(), &staged.fragments)?;
    validation::manifest(staged)?;
    let expected = receipt(&staged.manifest, &staged.fragments);
    if staged.custody != expected {
        return Err(FunctionFragmentEmissionError::ReceiptMismatch);
    }
    Ok(expected)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedFunctionFragmentEmissionManifest {
    record: Arc<FunctionFragmentEmissionManifest>,
}

impl ValidatedFunctionFragmentEmissionManifest {
    pub fn record(&self) -> &FunctionFragmentEmissionManifest {
        &self.record
    }
}

#[derive(Debug)]
pub struct StagedOptimizedFunctionFragmentEmission {
    source: StagedOptimizedFunctionFragmentEmissionSource,
    fragments: Arc<FunctionFragmentEmissionPlan>,
    manifest: ValidatedFunctionFragmentEmissionManifest,
    custody: StagedFunctionFragmentEmissionCustodyReceipt,
}

impl StagedOptimizedFunctionFragmentEmission {
    pub const fn source(&self) -> &StagedOptimizedFunctionFragmentEmissionSource {
        &self.source
    }
    pub fn fragments(&self) -> &FunctionFragmentEmissionPlan {
        &self.fragments
    }
    pub const fn manifest(&self) -> &ValidatedFunctionFragmentEmissionManifest {
        &self.manifest
    }
    pub const fn custody(&self) -> StagedFunctionFragmentEmissionCustodyReceipt {
        self.custody
    }

    pub fn verified_input(
        &self,
    ) -> &terminal_psi_to_abstract_operations::VerifiedPsiOptimizationInput {
        self.source.verified_input()
    }

    pub fn provider_installation(
        &self,
    ) -> Option<&terminal_psi_to_abstract_operations::AdmittedProviderInstallation> {
        self.source.provider_installation()
    }

    pub const fn function_relative_manifest(
        &self,
    ) -> &ValidatedFunctionRelativeOptimizationRealizationManifest {
        self.source.function_relative_manifest()
    }

    pub fn post_allocation_manifest(
        &self,
    ) -> &selected_instructions_to_register_homes::ValidatedPostAllocationOptimizationManifest {
        self.source.post_allocation_manifest()
    }

    pub fn pre_physical_manifest(
        &self,
    ) -> &abstract_operations_to_abstract_operations::validation::ValidatedPrePhysicalOptimizationManifest{
        self.source.pre_physical_manifest()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StagedFunctionFragmentEmissionCustodyReceipt {
    source_realization: FunctionRelativeOptimizationRealizationManifestIdentity,
    fragments: FunctionFragmentEmissionIdentity,
    manifest: FunctionFragmentEmissionManifestIdentity,
}

impl StagedFunctionFragmentEmissionCustodyReceipt {
    pub const fn source_realization(
        self,
    ) -> FunctionRelativeOptimizationRealizationManifestIdentity {
        self.source_realization
    }
    pub const fn fragments(self) -> FunctionFragmentEmissionIdentity {
        self.fragments
    }
    pub const fn manifest(self) -> FunctionFragmentEmissionManifestIdentity {
        self.manifest
    }
}
