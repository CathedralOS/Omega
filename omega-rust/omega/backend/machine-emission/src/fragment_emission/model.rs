use machine_code::FunctionFragmentEmissionPlan;
use optimization_core::{
    FunctionFragmentEmissionIdentity, FunctionFragmentEmissionManifestIdentity,
    FunctionRelativeOptimizationRealizationManifestIdentity,
};
use std::sync::Arc;

use super::source::StagedOptimizedFunctionFragmentEmissionSource;
pub use machine_code::{
    FunctionFragmentEmissionManifest, FunctionFragmentEmissionSourceKind,
    FunctionFragmentEmissionStage, FunctionFragmentEmissionStatistics,
    FunctionFragmentEmissionUnavailableData,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedFunctionFragmentEmissionManifest {
    pub(super) record: Arc<FunctionFragmentEmissionManifest>,
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
    pub(super) source: StagedOptimizedFunctionFragmentEmissionSource,
    pub(super) fragments: Arc<FunctionFragmentEmissionPlan>,
    pub(super) manifest: ValidatedFunctionFragmentEmissionManifest,
    pub(super) custody: StagedFunctionFragmentEmissionCustodyReceipt,
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
    ) -> &crate::ValidatedFunctionRelativeOptimizationRealizationManifest {
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
    pub(super) source_realization: FunctionRelativeOptimizationRealizationManifestIdentity,
    pub(super) fragments: FunctionFragmentEmissionIdentity,
    pub(super) manifest: FunctionFragmentEmissionManifestIdentity,
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
