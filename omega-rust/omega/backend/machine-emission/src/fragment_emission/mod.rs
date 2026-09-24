//! Fragment emission: the second machine-emission stage.
//!
//! [`stage_optimized_function_fragment_emission`] admits one function-relative
//! realization by replaying its custody (`custody`), projects the realized
//! machine layout into unplaced function fragments and seals their manifest
//! (`compute`), and returns only after
//! [`validate_optimized_function_fragment_emission`] replays the join: the
//! independent fragment checker, then the manifest fields (`validation`).
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

pub use error::{FunctionFragmentEmissionError, FunctionFragmentEmissionManifestDecodeError};
#[cfg(any(test, feature = "test-support"))]
pub use replay::FunctionFragmentReplayInputs;
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

    pub fn shared_record(&self) -> Arc<FunctionFragmentEmissionManifest> {
        Arc::clone(&self.record)
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
    pub fn shared_fragments(&self) -> Arc<FunctionFragmentEmissionPlan> {
        Arc::clone(&self.fragments)
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

    #[cfg(any(test, feature = "test-support"))]
    pub fn fragments_mut(&mut self) -> &mut FunctionFragmentEmissionPlan {
        Arc::make_mut(&mut self.fragments)
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn manifest_record_mut(&mut self) -> &mut FunctionFragmentEmissionManifest {
        Arc::make_mut(&mut self.manifest.record)
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn corrupt_custody_source_realization_for_test(&mut self) {
        self.custody.source_realization =
            FunctionRelativeOptimizationRealizationManifestIdentity::from_canonical_bytes(
                b"corrupt function-fragment source realization",
            );
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn corrupt_custody_fragments_for_test(&mut self) {
        self.custody.fragments = FunctionFragmentEmissionIdentity::from_canonical_bytes(
            b"corrupt function-fragment emission",
        );
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn corrupt_custody_manifest_for_test(&mut self) {
        self.custody.manifest = FunctionFragmentEmissionManifestIdentity::from_canonical_bytes(
            b"corrupt function-fragment manifest",
        );
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
