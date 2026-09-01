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
        physical_evidence_scope(matches!(self, Self::Unoptimized(_)), checked_scope)
    }
}

fn physical_evidence_scope(
    unoptimized: bool,
    checked_scope: Option<&psi_checked_trees_to_terminal::CheckedBoundaryOperatorApplicationScope>,
) -> omega_native_artifact::NativePhysicalEvidenceScope {
    if unoptimized
        && checked_scope.is_some_and(
            psi_checked_trees_to_terminal::CheckedBoundaryOperatorApplicationScope::is_empty,
        )
    {
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
    /// Exact retained nearest-FMA occurrences admitted by the source/Terminal
    /// proposal. The ordinary Abstract-to-Target stage consumes these rows;
    /// they are never inferred from a selected-plan report coordinate.
    pub ieee_float_fma:
        &'request [omega_abstract_operations_to_target_operations::AdmittedIeeeFloatFmaSettlement<'request>],
    /// Exact target-owned callback arguments rejoined by Terminal operation.
    /// This custody is consumed only by ordinary target lowering and physical
    /// assignment; machine emission remains a later, explicitly fenced rung.
    pub native_callbacks:
        &'request [omega_abstract_operations_to_target_operations::AdmittedNativeCallbackArgument],
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
    use crate::tests::fixtures::checked_source::checked;
    use omega_native_artifact::NativePhysicalEvidenceScope;

    #[test]
    fn physical_evidence_requires_both_unoptimized_and_exact_empty_d29_scope() {
        let empty_checked = checked(
            r#"
                data Main {}
                machine Main::launch() {}
            "#,
        );
        let empty =
            psi_checked_trees_to_terminal::produce_terminal_artifact_with_checked_boundary_operator_scope(
                &empty_checked,
                "Main::launch",
            )
            .expect("empty exact D29 scope");
        assert_eq!(
            physical_evidence_scope(true, Some(empty.boundary_operator_scope())),
            NativePhysicalEvidenceScope::UnoptimizedNoBoundaryOperatorApplications,
        );
        assert_eq!(
            physical_evidence_scope(false, Some(empty.boundary_operator_scope())),
            NativePhysicalEvidenceScope::Unavailable,
        );

        let demand_checked = checked(
            r#"
                boundary operator == Number::equal(left: i32, right: i32) -> bool;

                machine launch(left: i32, right: i32) -> bool {
                    left == right
                }
            "#,
        );
        let [demand] = demand_checked
            .facts
            .operators
            .boundary_applications
            .as_slice()
        else {
            panic!("one exact checked boundary-operator demand")
        };
        // Source-free operation matching is not implemented yet. Retain the
        // real checked demand beside a known-lowerable artifact to exercise
        // only the exact-scope eligibility fence closed by this milestone.
        let mut nonempty_checked = empty_checked.clone();
        nonempty_checked.facts.operators.boundary_applications = vec![demand.clone()];
        let nonempty =
            psi_checked_trees_to_terminal::produce_terminal_artifact_with_checked_boundary_operator_scope(
                &nonempty_checked,
                "Main::launch",
            )
            .expect("nonempty exact D29 scope");
        assert_eq!(
            physical_evidence_scope(true, Some(nonempty.boundary_operator_scope())),
            NativePhysicalEvidenceScope::Unavailable,
        );
        assert_eq!(
            physical_evidence_scope(true, None),
            NativePhysicalEvidenceScope::Unavailable,
        );
    }
}
