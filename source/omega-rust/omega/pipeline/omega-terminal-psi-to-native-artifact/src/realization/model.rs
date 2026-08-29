use crate::entry_settlement::{
    NativeProgramEntrySettlement, ValidatedNativeProgramEntrySettlement,
};
use omega_installation_evidence::ProviderExecutionEvidence;
use omega_native_artifact::NativeArtifact;
use omega_target_operations::BoundaryRealization;

pub(crate) enum NativeRealizationInput {
    Unoptimized(omega_psi_to_abstract_operations::NativeArtifactOperationPlan),
    ExplicitOptimization(omega_psi_to_abstract_operations::VerifiedPsiOptimizationInput),
}

impl NativeRealizationInput {
    pub(crate) fn plan(&self) -> &omega_abstract_operations::AbstractOperationPlan {
        match self {
            Self::Unoptimized(input) => input.plan(),
            Self::ExplicitOptimization(input) => input.plan(),
        }
    }
}

/// Provider-supplied realization input for one Terminal boundary. The exact
/// requirement comes from admitted execution evidence rather than a caller-
/// authored numeric boundary ID.
#[derive(Debug, Clone, Copy)]
pub struct NativeProviderSettlement<'execution> {
    pub provider_execution: &'execution dyn ProviderExecutionEvidence,
    pub realization: BoundaryRealization,
}

/// Complete build-owned inputs for one target-native realization. Keeping
/// these coupled prevents callers from accidentally carrying entry, target,
/// optimization, and provider custody through separate positional channels.
pub struct NativeRealizationRequest<'request> {
    pub target: omega_target::NativeTarget,
    pub subsystem: u16,
    pub profile: &'request psi_proof_admission::AdmissionProfile,
    pub program_entry: NativeProgramEntrySettlement<'request>,
    pub optimization_selections: &'request omega_optimization_core::OptimizationSelections,
    pub selected_provider_plans: &'request omega_effects::SelectedProviderPlanFacts,
    pub settlements: &'request [NativeProviderSettlement<'request>],
}

/// Compatibility-preserving result for the receipt-requiring native path.
#[derive(Debug)]
pub struct SettledNativeArtifact {
    pub(crate) artifact: NativeArtifact,
    pub(crate) program_entry: ValidatedNativeProgramEntrySettlement,
}

impl SettledNativeArtifact {
    pub const fn artifact(&self) -> &NativeArtifact {
        &self.artifact
    }

    pub const fn program_entry(&self) -> &ValidatedNativeProgramEntrySettlement {
        &self.program_entry
    }

    pub fn into_parts(self) -> (NativeArtifact, ValidatedNativeProgramEntrySettlement) {
        (self.artifact, self.program_entry)
    }
}
