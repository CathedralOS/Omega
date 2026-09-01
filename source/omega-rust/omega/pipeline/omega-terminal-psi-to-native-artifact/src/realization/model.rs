use crate::entry_settlement::{
    NativeProgramEntrySettlement, ValidatedNativeProgramEntrySettlement,
};
use omega_installation_evidence::ProviderExecutionEvidence;
use omega_native_artifact::NativeArtifact;
use omega_target_operations::BoundaryRealization;

#[derive(Debug, Clone, Copy)]
pub enum NativeBoundaryRealization<'execution> {
    Builtin(BoundaryRealization),
    NormalizedForeignCall(&'execution omega_task_plans::AdmittedSameStackContribution),
}

impl<'execution> From<BoundaryRealization> for NativeBoundaryRealization<'execution> {
    fn from(realization: BoundaryRealization) -> Self {
        Self::Builtin(realization)
    }
}

macro_rules! builtin_native_realization_conversion {
    ($realization:ty) => {
        impl<'execution> From<$realization> for NativeBoundaryRealization<'execution> {
            fn from(realization: $realization) -> Self {
                Self::Builtin(realization.into())
            }
        }
    };
}

builtin_native_realization_conversion!(omega_target_operations::MetadataOnlyPortRealization);
builtin_native_realization_conversion!(omega_target_operations::DirectPortReadU8Realization);
builtin_native_realization_conversion!(omega_target_operations::LinuxWriteLineRealization);
builtin_native_realization_conversion!(omega_target_operations::LinuxExitGroupI32Realization);
builtin_native_realization_conversion!(omega_target_operations::ClaimCompletionOnlyRealization);

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

    pub(crate) fn physical_evidence_scope(
        &self,
        checked_scope: Option<
            &psi_checked_trees_to_terminal::CheckedBoundaryOperatorApplicationScope,
        >,
    ) -> omega_native_artifact::NativePhysicalEvidenceScope {
        let application_count = match checked_scope {
            Some(scope) => scope.application_count(),
            None => usize::MAX,
        };
        physical_evidence_scope(matches!(self, Self::Unoptimized(_)), application_count)
    }
}

const fn physical_evidence_scope(
    unoptimized: bool,
    boundary_operator_application_count: usize,
) -> omega_native_artifact::NativePhysicalEvidenceScope {
    if unoptimized && boundary_operator_application_count == 0 {
        omega_native_artifact::NativePhysicalEvidenceScope::UnoptimizedNoBoundaryOperatorApplications
    } else {
        omega_native_artifact::NativePhysicalEvidenceScope::Unavailable
    }
}

/// Provider-supplied realization input for one Terminal boundary. The exact
/// requirement comes from admitted execution evidence rather than a caller-
/// authored numeric boundary ID.
#[derive(Debug, Clone, Copy)]
pub struct NativeProviderSettlement<'execution> {
    pub provider_execution: &'execution dyn ProviderExecutionEvidence,
    /// Complete selected-plan evidence. The compact report identity remains a
    /// report coordinate and cannot select or authorize a plan by itself.
    pub provider_plan: &'execution omega_effects::provider_plan::ProviderPlan,
    pub realization: NativeBoundaryRealization<'execution>,
}

/// Target-constrained compiler-builtin proposal consumed by the local native
/// lowerer. This carries no provider execution or installation receipt.
#[derive(Debug, Clone, Copy)]
pub struct NativeCompilerBuiltinSettlement<'execution> {
    pub requirement_identity: &'execution str,
    pub provider_plan: &'execution omega_effects::provider_plan::ProviderPlan,
    pub execution: omega_target_operations::CompilerBuiltinExecution,
}

/// Complete build-owned inputs for one target-native realization. Keeping
/// these coupled prevents callers from accidentally carrying entry, target,
/// optimization, and provider custody through separate positional channels.
pub struct NativeRealizationRequest<'request> {
    pub target: omega_target::NativeTarget,
    pub subsystem: u16,
    pub profile: &'request psi_proof_admission::AdmissionProfile,
    /// Receiving target policy used to classify every demanded compiler
    /// intrinsic before native settlement.
    pub terminal_authority_policy: crate::realization::CompilerIntrinsicTerminalAuthorityPolicy,
    pub program_entry: NativeProgramEntrySettlement<'request>,
    pub optimization_selections: &'request omega_optimization_core::OptimizationSelections,
    pub selected_provider_plans: &'request omega_effects::SelectedProviderPlanFacts,
    pub external_binding_rows: &'request [omega_calling_conventions::ExternalBindingRow],
    pub settlements: &'request [NativeProviderSettlement<'request>],
    pub compiler_builtins: &'request [NativeCompilerBuiltinSettlement<'request>],
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

#[cfg(test)]
mod tests {
    use super::physical_evidence_scope;
    use omega_native_artifact::NativePhysicalEvidenceScope;

    #[test]
    fn physical_evidence_requires_both_unoptimized_and_exact_empty_d29_scope() {
        assert_eq!(
            physical_evidence_scope(true, 0),
            NativePhysicalEvidenceScope::UnoptimizedNoBoundaryOperatorApplications,
        );
        assert_eq!(
            physical_evidence_scope(false, 0),
            NativePhysicalEvidenceScope::Unavailable,
        );
        assert_eq!(
            physical_evidence_scope(true, 1),
            NativePhysicalEvidenceScope::Unavailable,
        );
    }
}
