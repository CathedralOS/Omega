use crate::entry_settlement::{
    NativeProgramEntrySettlement, ValidatedNativeProgramEntrySettlement,
};
use installation_evidence::ProviderExecutionEvidence;
use native_artifact::{DynamicElfNativeArtifact, NativeArtifact};
use target_operations::BoundaryRealization;

#[derive(Debug, Clone, Copy)]
pub enum NativeBoundaryRealization<'execution> {
    Builtin(BoundaryRealization),
    NormalizedForeignCall(&'execution task_plans::AdmittedSameStackContribution),
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

builtin_native_realization_conversion!(target_operations::MetadataOnlyPortRealization);
builtin_native_realization_conversion!(target_operations::DirectPortReadU8Realization);
builtin_native_realization_conversion!(target_operations::LinuxWriteLineRealization);
builtin_native_realization_conversion!(target_operations::LinuxExitGroupI32Realization);
builtin_native_realization_conversion!(target_operations::LinuxReadByteRealization);
builtin_native_realization_conversion!(target_operations::ClaimCompletionOnlyRealization);

/// Native authority is independent of optimization selection.
/// The ranked role retains its checked countdown evidence; ordinary authority
/// does not duplicate the current abstract-operation plan.
#[derive(Debug, Clone)]
pub(crate) enum NativeRealizationAuthority {
    Ordinary,
    RankedU32Countdown(abstract_operations::RankedNativeAbstractOperationPlan),
}

/// One current verified abstract input and its separately admitted native role.
#[derive(Debug, Clone)]
pub(crate) struct NativeRealizationInput {
    authority: NativeRealizationAuthority,
    optimization_input: terminal_psi_to_abstract_operations::VerifiedPsiOptimizationInput,
}

impl NativeRealizationInput {
    pub(crate) fn new(
        native: terminal_psi_to_abstract_operations::NativeArtifactOperationPlan,
        optimization_input: terminal_psi_to_abstract_operations::VerifiedPsiOptimizationInput,
    ) -> Result<Self, &'static str> {
        if optimization_input.plan() != native.plan() {
            return Err(
                "native authority and abstract-optimization context disagree on the complete abstract program",
            );
        }
        let authority = match native {
            terminal_psi_to_abstract_operations::NativeArtifactOperationPlan::Ordinary(_) => {
                NativeRealizationAuthority::Ordinary
            }
            terminal_psi_to_abstract_operations::NativeArtifactOperationPlan::RankedU32Countdown(ranked) => {
                NativeRealizationAuthority::RankedU32Countdown(ranked)
            }
        };
        Ok(Self {
            authority,
            optimization_input,
        })
    }

    pub(crate) fn plan(&self) -> &abstract_operations::AbstractOperationPlan {
        self.optimization_input.plan()
    }

    pub(crate) const fn authority(&self) -> &NativeRealizationAuthority {
        &self.authority
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        NativeRealizationAuthority,
        terminal_psi_to_abstract_operations::VerifiedPsiOptimizationInput,
    ) {
        (self.authority, self.optimization_input)
    }
}

pub(crate) fn physical_evidence_scope(
    identity_physical_path: bool,
    checked_scope: Option<&checked_trees_to_terminal_psi::CheckedBoundaryOperatorApplicationScope>,
) -> native_artifact::NativePhysicalEvidenceScope {
    if identity_physical_path && checked_scope.is_some() {
        native_artifact::NativePhysicalEvidenceScope::UnoptimizedCompleteBoundaryEvidence
    } else {
        native_artifact::NativePhysicalEvidenceScope::Unavailable
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
    pub provider_plan: &'execution effects::provider_plan::ProviderPlan,
    pub realization: NativeBoundaryRealization<'execution>,
}

/// Target-constrained compiler-builtin proposal consumed by the local native
/// lowerer. This carries no provider execution or installation receipt.
#[derive(Debug, Clone, Copy)]
pub struct NativeCompilerBuiltinSettlement<'execution> {
    pub requirement_identity: &'execution str,
    pub provider_plan: &'execution effects::provider_plan::ProviderPlan,
    pub execution: target_operations::CompilerBuiltinExecution,
}

/// Borrowed source-free body and placement join for one compiler-private
/// callback thunk. The ordinary callback-argument carrier remains separate so
/// target lowering cannot confuse executable body custody with a semantic
/// registrar argument.
#[derive(Debug, Clone, Copy)]
pub struct NativeCallbackThunkSettlement<'artifact> {
    pub terminal_operation: semantic_vocabulary::OperationId,
    pub placement_index: usize,
    pub callback_function: function_identity::MachineFunctionIdentity,
    pub private_symbol: &'artifact str,
    pub artifact: &'artifact terminal_codec::CanonicalTerminalArtifact,
    pub lowering_receipt: checked_trees_to_terminal_psi::CallbackTerminalLoweringReceipt,
    pub boundary_entry_plan: &'artifact calling_conventions::BoundaryEntryPlan,
}

/// Complete build-owned inputs for one target-native realization. Keeping
/// these coupled prevents callers from accidentally carrying entry, target,
/// optimization, and provider custody through separate positional channels.
pub struct NativeRealizationRequest<'request> {
    pub target: target::NativeTarget,
    pub subsystem: u16,
    pub profile: &'request proof_admission::AdmissionProfile,
    /// Receiving target policy used to classify every demanded compiler
    /// intrinsic before native settlement.
    pub terminal_authority_policy: crate::realization::TerminalAuthorityPolicy,
    /// Independently accepted exact service-schema/requirement permissions.
    /// Physical classification cannot manufacture or widen these rows.
    pub terminal_authority_permission_policy: crate::realization::TerminalAuthorityPermissionPolicy,
    pub program_entry: NativeProgramEntrySettlement<'request>,
    pub optimization_selections: &'request optimization_core::PostTerminalOptimizationSelections,
    pub selected_provider_plans: &'request effects::SelectedProviderPlanFacts,
    pub external_binding_rows: &'request [calling_conventions::ExternalBindingRow],
    pub settlements: &'request [NativeProviderSettlement<'request>],
    pub compiler_builtins: &'request [NativeCompilerBuiltinSettlement<'request>],
    /// Exact source-free D29 demand and realization custody projected by the
    /// compiler product owner. A nonempty checked scope requires this value;
    /// `None` never means an exact empty demand set.
    pub boundary_application_coverage:
        Option<&'request boundary_applications::TerminalBoundaryApplicationCoverage>,
    /// Exact retained nearest-FMA occurrences admitted by the source/Terminal
    /// proposal. The ordinary Abstract-to-Target stage consumes these rows;
    /// they are never inferred from a selected-plan report coordinate.
    pub ieee_float_fma:
        &'request [abstract_operations_to_target_operations::AdmittedIeeeFloatFmaSettlement<
            'request,
        >],
    /// Exact target-owned callback arguments rejoined by Terminal operation.
    /// This custody is consumed only by ordinary target lowering and physical
    /// assignment; machine emission remains a later, explicitly fenced rung.
    pub native_callbacks:
        &'request [abstract_operations_to_target_operations::AdmittedNativeCallbackArgument],
    /// Isolated executable bodies paired one-to-one with `native_callbacks`.
    /// Their Terminal machine identities live in separate artifact namespaces.
    pub callback_thunks: &'request [NativeCallbackThunkSettlement<'request>],
}

/// Complete native-realization inputs for authority-distinct image routing.
/// Unlike the compatibility direct request, this carrier has no independent
/// subsystem field that a dynamic route could silently ignore.
pub struct RequestedNativeRealizationRequest<'request> {
    pub target: target::NativeTarget,
    pub image_request: image_emission::ExecutableImageEmissionRequest,
    pub profile: &'request proof_admission::AdmissionProfile,
    pub terminal_authority_policy: crate::realization::TerminalAuthorityPolicy,
    pub terminal_authority_permission_policy: crate::realization::TerminalAuthorityPermissionPolicy,
    pub program_entry: NativeProgramEntrySettlement<'request>,
    pub optimization_selections: &'request optimization_core::PostTerminalOptimizationSelections,
    pub selected_provider_plans: &'request effects::SelectedProviderPlanFacts,
    pub external_binding_rows: &'request [calling_conventions::ExternalBindingRow],
    pub settlements: &'request [NativeProviderSettlement<'request>],
    pub compiler_builtins: &'request [NativeCompilerBuiltinSettlement<'request>],
    pub boundary_application_coverage:
        Option<&'request boundary_applications::TerminalBoundaryApplicationCoverage>,
    pub ieee_float_fma:
        &'request [abstract_operations_to_target_operations::AdmittedIeeeFloatFmaSettlement<
            'request,
        >],
    pub native_callbacks:
        &'request [abstract_operations_to_target_operations::AdmittedNativeCallbackArgument],
    pub callback_thunks: &'request [NativeCallbackThunkSettlement<'request>],
}

pub(crate) struct NativeRealizationCoreRequest<'request> {
    pub target: target::NativeTarget,
    pub profile: &'request proof_admission::AdmissionProfile,
    pub terminal_authority_policy: crate::realization::TerminalAuthorityPolicy,
    pub terminal_authority_permission_policy: crate::realization::TerminalAuthorityPermissionPolicy,
    pub program_entry: NativeProgramEntrySettlement<'request>,
    pub optimization_selections: &'request optimization_core::PostTerminalOptimizationSelections,
    pub selected_provider_plans: &'request effects::SelectedProviderPlanFacts,
    pub external_binding_rows: &'request [calling_conventions::ExternalBindingRow],
    pub settlements: &'request [NativeProviderSettlement<'request>],
    pub compiler_builtins: &'request [NativeCompilerBuiltinSettlement<'request>],
    pub boundary_application_coverage:
        Option<&'request boundary_applications::TerminalBoundaryApplicationCoverage>,
    pub ieee_float_fma:
        &'request [abstract_operations_to_target_operations::AdmittedIeeeFloatFmaSettlement<
            'request,
        >],
    pub native_callbacks:
        &'request [abstract_operations_to_target_operations::AdmittedNativeCallbackArgument],
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
        image_emission::ExecutableImageEmissionRequest,
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
    pub(crate) image_request: image_emission::ExecutableImageEmissionRequest,
    pub(crate) diagnostics: Vec<diagnostics::Diagnostic>,
}

impl RequestedNativeArtifactError {
    pub fn into_parts(
        self,
    ) -> (
        image_emission::ExecutableImageEmissionRequest,
        Vec<diagnostics::Diagnostic>,
    ) {
        (self.image_request, self.diagnostics)
    }

    pub fn diagnostics(&self) -> &[diagnostics::Diagnostic] {
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
    use native_artifact::NativePhysicalEvidenceScope;

    #[test]
    fn physical_evidence_requires_identity_physical_path_and_exact_d29_custody() {
        let empty_checked = checked(
            r#"
                data Main {}
                machine Main::launch() {}
            "#,
        );
        let empty =
            checked_trees_to_terminal_psi::produce_terminal_artifact_with_checked_boundary_operator_scope(
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
            checked_trees_to_terminal_psi::produce_terminal_artifact_with_checked_boundary_operator_scope(
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
