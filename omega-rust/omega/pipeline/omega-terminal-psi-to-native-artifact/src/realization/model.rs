use crate::entry_settlement::{
    NativeProgramEntrySettlement, ValidatedNativeProgramEntrySettlement,
};
use omega_installation_evidence::ProviderExecutionEvidence;
use omega_native_artifact::{DynamicElfNativeArtifact, NativeArtifact};
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
builtin_native_realization_conversion!(omega_target_operations::LinuxReadByteRealization);
builtin_native_realization_conversion!(omega_target_operations::ClaimCompletionOnlyRealization);

/// One unconditional Terminal-to-abstract native stage result.
///
/// `native` retains the role-specific ordinary or ranked native authority for
/// every request. A nonempty later-phase selection may additionally retain the
/// optimizer context needed by the transitional physical pipeline; it never
/// replaces or chooses the Terminal-to-abstract entrance.
#[derive(Debug, Clone)]
pub(crate) struct NativeRealizationInput {
    native: omega_psi_to_abstract_operations::NativeArtifactOperationPlan,
    optimization: Option<omega_psi_to_abstract_operations::VerifiedPsiOptimizationInput>,
}

impl NativeRealizationInput {
    pub(crate) fn new(
        native: omega_psi_to_abstract_operations::NativeArtifactOperationPlan,
        optimization: Option<omega_psi_to_abstract_operations::VerifiedPsiOptimizationInput>,
    ) -> Result<Self, &'static str> {
        if optimization.as_ref().is_some_and(|selected| {
            selected.plan().psi != native.plan().psi || selected.plan().entry != native.plan().entry
        }) {
            return Err(
                "native authority and selected physical-optimization context disagree on the Terminal program root",
            );
        }
        Ok(Self {
            native,
            optimization,
        })
    }

    pub(crate) fn plan(&self) -> &omega_abstract_operations::AbstractOperationPlan {
        match &self.optimization {
            Some(input) => input.plan(),
            None => self.native.plan(),
        }
    }

    pub(crate) const fn native(
        &self,
    ) -> &omega_psi_to_abstract_operations::NativeArtifactOperationPlan {
        &self.native
    }

    pub(crate) const fn optimization(
        &self,
    ) -> Option<&omega_psi_to_abstract_operations::VerifiedPsiOptimizationInput> {
        self.optimization.as_ref()
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        omega_psi_to_abstract_operations::NativeArtifactOperationPlan,
        Option<omega_psi_to_abstract_operations::VerifiedPsiOptimizationInput>,
    ) {
        (self.native, self.optimization)
    }

    pub(crate) fn physical_evidence_scope(
        &self,
        checked_scope: Option<
            &psi_checked_trees_to_terminal::CheckedBoundaryOperatorApplicationScope,
        >,
    ) -> omega_native_artifact::NativePhysicalEvidenceScope {
        physical_evidence_scope(self.optimization.is_none(), checked_scope)
    }
}

fn physical_evidence_scope(
    identity_physical_path: bool,
    checked_scope: Option<&psi_checked_trees_to_terminal::CheckedBoundaryOperatorApplicationScope>,
) -> omega_native_artifact::NativePhysicalEvidenceScope {
    if identity_physical_path && checked_scope.is_some() {
        omega_native_artifact::NativePhysicalEvidenceScope::UnoptimizedCompleteBoundaryEvidence
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

/// Borrowed source-free body and placement join for one compiler-private
/// callback thunk. The ordinary callback-argument carrier remains separate so
/// target lowering cannot confuse executable body custody with a semantic
/// registrar argument.
#[derive(Debug, Clone, Copy)]
pub struct NativeCallbackThunkSettlement<'artifact> {
    pub terminal_operation: psi_core::OperationId,
    pub placement_index: usize,
    pub callback_function: omega_function_identity::MachineFunctionIdentity,
    pub private_symbol: &'artifact str,
    pub artifact: &'artifact psi_terminal_codec::CanonicalTerminalArtifact,
    pub lowering_receipt: psi_checked_trees_to_terminal::CallbackTerminalLoweringReceipt,
    pub boundary_entry_plan: &'artifact omega_calling_conventions::BoundaryEntryPlan,
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
    pub terminal_authority_policy: crate::realization::TerminalAuthorityPolicy,
    /// Independently accepted exact service-schema/requirement permissions.
    /// Physical classification cannot manufacture or widen these rows.
    pub terminal_authority_permission_policy:
        crate::realization::TerminalAuthorityPermissionPolicy,
    pub program_entry: NativeProgramEntrySettlement<'request>,
    pub optimization_selections: &'request omega_optimization_core::OptimizationSelections,
    pub selected_provider_plans: &'request omega_effects::SelectedProviderPlanFacts,
    pub external_binding_rows: &'request [omega_calling_conventions::ExternalBindingRow],
    pub settlements: &'request [NativeProviderSettlement<'request>],
    pub compiler_builtins: &'request [NativeCompilerBuiltinSettlement<'request>],
    /// Exact source-free D29 demand and realization custody projected by the
    /// compiler product owner. A nonempty checked scope requires this value;
    /// `None` never means an exact empty demand set.
    pub boundary_application_coverage:
        Option<&'request omega_boundary_applications::TerminalBoundaryApplicationCoverage>,
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
    /// Isolated executable bodies paired one-to-one with `native_callbacks`.
    /// Their Terminal machine identities live in separate artifact namespaces.
    pub callback_thunks: &'request [NativeCallbackThunkSettlement<'request>],
}

/// Complete native-realization inputs for authority-distinct image routing.
/// Unlike the compatibility direct request, this carrier has no independent
/// subsystem field that a dynamic route could silently ignore.
pub struct RequestedNativeRealizationRequest<'request> {
    pub target: omega_target::NativeTarget,
    pub image_request: omega_image_emission::ExecutableImageEmissionRequest,
    pub profile: &'request psi_proof_admission::AdmissionProfile,
    pub terminal_authority_policy: crate::realization::TerminalAuthorityPolicy,
    pub terminal_authority_permission_policy:
        crate::realization::TerminalAuthorityPermissionPolicy,
    pub program_entry: NativeProgramEntrySettlement<'request>,
    pub optimization_selections: &'request omega_optimization_core::OptimizationSelections,
    pub selected_provider_plans: &'request omega_effects::SelectedProviderPlanFacts,
    pub external_binding_rows: &'request [omega_calling_conventions::ExternalBindingRow],
    pub settlements: &'request [NativeProviderSettlement<'request>],
    pub compiler_builtins: &'request [NativeCompilerBuiltinSettlement<'request>],
    pub boundary_application_coverage:
        Option<&'request omega_boundary_applications::TerminalBoundaryApplicationCoverage>,
    pub ieee_float_fma:
        &'request [omega_abstract_operations_to_target_operations::AdmittedIeeeFloatFmaSettlement<'request>],
    pub native_callbacks:
        &'request [omega_abstract_operations_to_target_operations::AdmittedNativeCallbackArgument],
    pub callback_thunks: &'request [NativeCallbackThunkSettlement<'request>],
}

pub(crate) struct NativeRealizationCoreRequest<'request> {
    pub target: omega_target::NativeTarget,
    pub profile: &'request psi_proof_admission::AdmissionProfile,
    pub terminal_authority_policy: crate::realization::TerminalAuthorityPolicy,
    pub terminal_authority_permission_policy:
        crate::realization::TerminalAuthorityPermissionPolicy,
    pub program_entry: NativeProgramEntrySettlement<'request>,
    pub optimization_selections: &'request omega_optimization_core::OptimizationSelections,
    pub selected_provider_plans: &'request omega_effects::SelectedProviderPlanFacts,
    pub external_binding_rows: &'request [omega_calling_conventions::ExternalBindingRow],
    pub settlements: &'request [NativeProviderSettlement<'request>],
    pub compiler_builtins: &'request [NativeCompilerBuiltinSettlement<'request>],
    pub boundary_application_coverage:
        Option<&'request omega_boundary_applications::TerminalBoundaryApplicationCoverage>,
    pub ieee_float_fma:
        &'request [omega_abstract_operations_to_target_operations::AdmittedIeeeFloatFmaSettlement<'request>],
    pub native_callbacks:
        &'request [omega_abstract_operations_to_target_operations::AdmittedNativeCallbackArgument],
    pub callback_thunks: &'request [NativeCallbackThunkSettlement<'request>],
}

impl<'request> NativeRealizationRequest<'request> {
    pub(crate) fn into_core(self) -> NativeRealizationCoreRequest<'request> {
        NativeRealizationCoreRequest {
            target: self.target,
            profile: self.profile,
            terminal_authority_policy: self.terminal_authority_policy,
            terminal_authority_permission_policy: self.terminal_authority_permission_policy,
            program_entry: self.program_entry,
            optimization_selections: self.optimization_selections,
            selected_provider_plans: self.selected_provider_plans,
            external_binding_rows: self.external_binding_rows,
            settlements: self.settlements,
            compiler_builtins: self.compiler_builtins,
            boundary_application_coverage: self.boundary_application_coverage,
            ieee_float_fma: self.ieee_float_fma,
            native_callbacks: self.native_callbacks,
            callback_thunks: self.callback_thunks,
        }
    }
}

impl<'request> RequestedNativeRealizationRequest<'request> {
    pub(crate) fn into_parts(
        self,
    ) -> (
        omega_image_emission::ExecutableImageEmissionRequest,
        NativeRealizationCoreRequest<'request>,
    ) {
        (
            self.image_request,
            NativeRealizationCoreRequest {
                target: self.target,
                profile: self.profile,
                terminal_authority_policy: self.terminal_authority_policy,
                terminal_authority_permission_policy: self.terminal_authority_permission_policy,
                program_entry: self.program_entry,
                optimization_selections: self.optimization_selections,
                selected_provider_plans: self.selected_provider_plans,
                external_binding_rows: self.external_binding_rows,
                settlements: self.settlements,
                compiler_builtins: self.compiler_builtins,
                boundary_application_coverage: self.boundary_application_coverage,
                ieee_float_fma: self.ieee_float_fma,
                native_callbacks: self.native_callbacks,
                callback_thunks: self.callback_thunks,
            },
        )
    }
}

/// Source-free native result selected by the exact object-bound image request.
/// Dynamic ELF remains a separate, non-installable authority class.
#[derive(Debug)]
#[must_use = "requested native realization retains its authority-distinct image custody"]
pub enum RequestedNativeArtifact {
    Direct(NativeArtifact),
    DynamicElf(DynamicElfNativeArtifact),
}

/// Failed requested realization with the complete image input recoverable.
#[derive(Debug)]
#[must_use = "requested native realization failure retains the exact image request"]
pub struct RequestedNativeArtifactError {
    pub(crate) image_request: omega_image_emission::ExecutableImageEmissionRequest,
    pub(crate) diagnostics: Vec<psi_diagnostics::Diagnostic>,
}

impl RequestedNativeArtifactError {
    pub fn into_parts(
        self,
    ) -> (
        omega_image_emission::ExecutableImageEmissionRequest,
        Vec<psi_diagnostics::Diagnostic>,
    ) {
        (self.image_request, self.diagnostics)
    }

    pub fn diagnostics(&self) -> &[psi_diagnostics::Diagnostic] {
        &self.diagnostics
    }
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
    fn physical_evidence_requires_identity_physical_path_and_exact_d29_custody() {
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
            NativePhysicalEvidenceScope::UnoptimizedCompleteBoundaryEvidence,
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
            NativePhysicalEvidenceScope::UnoptimizedCompleteBoundaryEvidence,
        );
        assert_eq!(
            physical_evidence_scope(true, None),
            NativePhysicalEvidenceScope::Unavailable,
        );
    }
}
